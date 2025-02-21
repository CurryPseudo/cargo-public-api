use std::io::stdout;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use arg_types::{Color, DenyMethod, OutputFormat};
use output_formatter::OutputFormatter;
use public_api::diff::PublicItemsDiff;
use public_api::{
    public_api_from_rustdoc_json_str, Options, PublicItem, MINIMUM_RUSTDOC_JSON_VERSION,
};

use clap::Parser;
use rustdoc_json::BuildError;

mod arg_types;
mod error;
mod git_utils;
mod markdown;
mod output_formatter;
mod plain;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
pub struct Args {
    /// Path to `Cargo.toml`.
    #[clap(long, name = "PATH", default_value = "Cargo.toml", parse(from_os_str))]
    manifest_path: PathBuf,

    /// Raise this flag to make items part of blanket implementations such as
    /// `impl<T> Any for T`, `impl<T> Borrow<T> for T`, and `impl<T, U> Into<U>
    /// for T where U: From<T>` be included in the list of public items of a
    /// crate.
    ///
    /// Blanket implementations are not included by default since the the vast
    /// majority of users will find the presence of these items to just
    /// constitute noise, even if they formally are part of the public API of a
    /// crate.
    #[clap(long)]
    with_blanket_implementations: bool,

    /// Usage: --diff-git-checkouts <COMMIT_1> <COMMIT_2>
    ///
    /// Rudimentary wrapper "script" to diff the public API across two different
    /// commits. The following steps are performed:
    ///
    /// 1. Do a literal in-tree, in-place `git checkout` of the first commit
    ///
    /// 2. Collect public API
    ///
    /// 3. Do a literal in-tree, in-place `git checkout` of the second commit
    ///
    /// 4. Collect public API
    ///
    /// 5. Print the diff between public API in step 2 and step 4
    ///
    /// Do not use non-fixed commit references such as `HEAD^` since the meaning
    /// of `HEAD^` is different depending on what commit is the current commit.
    ///
    /// While potentially annoying and in worst case destructive, doing this in
    /// the current git repo has the benefit of making it likely for the build
    /// to succeed. If we e.g. were to git clone a temporary copy of a commit
    /// ourselves, the risk is high that additional steps are needed before a
    /// build can succeed. Such as the need to set up git submodules.
    ///
    /// Tip: Make the second commit the same as your current commit, so that
    /// the working tree is restored to your current state after the diff
    /// has been printed.
    #[clap(long, min_values = 2, max_values = 2)]
    diff_git_checkouts: Option<Vec<String>>,

    /// Usage: --diff-rustdoc-json <RUSTDOC_JSON_PATH_1> <RUSTDOC_JSON_PATH_2>
    ///
    /// Diff the public API across two different rustdoc JSON files.
    #[clap(long, min_values = 2, max_values = 2)]
    diff_rustdoc_json: Option<Vec<String>>,

    /// Exit with failure if the specified API diff is detected. all = deny
    /// additions, changes, and removals of public items in the API
    #[clap(long, arg_enum)]
    deny: Option<Vec<DenyMethod>>,

    /// What output format to use. You can select between "plain" and "markdown".
    #[clap(long, name = "FORMAT", default_value = "plain")]
    output_format: OutputFormat,

    /// Whether or not to use colors. You can select between "auto", "never", "always".
    /// If "auto" (the default), colors will be used if stdout is a terminal. If you pipe
    /// the output to a file, colors will be disabled by default.
    #[clap(long, default_value = "auto")]
    color: Color,

    /// Show detailed info about processing. For debugging purposes. The output
    /// is not stable and can change across patch versions.
    #[clap(long, hide = true)]
    verbose: bool,

    /// Allows you to build rustdoc JSON with a toolchain other than `+nightly`.
    /// Useful if you have built a toolchain from source for example.
    #[clap(long, hide = true, default_value = "+nightly")]
    rustdoc_json_toolchain: String,
}

fn main_() -> Result<()> {
    let args = get_args();

    let diff = if let Some(commits) = &args.diff_git_checkouts {
        Some(print_diff_between_two_commits(&args, commits)?)
    } else if let Some(files) = &args.diff_rustdoc_json {
        Some(print_diff_between_two_rustdoc_json_files(&args, files)?)
    } else {
        print_public_items_of_current_commit(&args)?;
        None
    };

    check_diff(&args, diff)
}

fn check_diff(args: &Args, diff: Option<PublicItemsDiff>) -> Result<()> {
    match (&args.deny, diff) {
        // We were requested to deny diffs, so make sure there is no diff
        (Some(_deny), Some(diff)) => {
            if diff.is_empty() {
                Ok(())
            } else {
                Err(anyhow!(error::Error::DiffDenied))
            }
        }

        // We were requested to deny diffs, but we did not calculate a diff!
        (Some(_), None) => Err(anyhow!("`--deny` can only be used when diffing")),

        // No diff related stuff to care about, all is Ok
        _ => Ok(()),
    }
}

fn print_public_items_of_current_commit(args: &Args) -> Result<()> {
    let public_items = collect_public_items_from_commit(None)?;
    args.output_format
        .formatter()
        .print_items(&mut stdout(), args, public_items)?;

    Ok(())
}

fn print_diff_between_two_commits(args: &Args, commits: &[String]) -> Result<PublicItemsDiff> {
    let old_commit = commits.get(0).expect("clap makes sure first commit exist");
    let old = collect_public_items_from_commit(Some(old_commit))?;

    let new_commit = commits.get(1).expect("clap makes sure second commit exist");
    let new = collect_public_items_from_commit(Some(new_commit))?;

    print_diff(args, old, new)
}

fn print_diff_between_two_rustdoc_json_files(
    args: &Args,
    files: &[String],
) -> Result<PublicItemsDiff> {
    let options = get_options(args);

    let old_file = files.get(0).expect("clap makes sure first file exists");
    let old = public_api_from_rustdoc_json_path(old_file, options)?;

    let new_file = files.get(1).expect("clap makes sure second file exists");
    let new = public_api_from_rustdoc_json_path(new_file, options)?;

    print_diff(args, old, new)
}

fn print_diff(args: &Args, old: Vec<PublicItem>, new: Vec<PublicItem>) -> Result<PublicItemsDiff> {
    let diff = PublicItemsDiff::between(old, new);
    args.output_format
        .formatter()
        .print_diff(&mut stdout(), args, &diff)?;

    Ok(diff)
}

/// Get CLI args via `clap` while also handling when we are invoked as a cargo
/// subcommand
fn get_args() -> Args {
    // If we are invoked by cargo as `cargo public-api`, the second arg will
    // be "public-api". Remove it before passing args on to clap. If we are
    // not invoked as a cargo subcommand, it will not be part of args at all, so
    // it is safe to filter it out also in that case.
    let args = std::env::args_os().filter(|x| x != "public-api");

    Args::parse_from(args)
}

/// Figure out what [`Options`] to pass to
/// [`public_items::sorted_public_items_from_rustdoc_json_str`] based on our
/// [`Args`]
fn get_options(args: &Args) -> Options {
    let mut options = Options::default();
    options.with_blanket_implementations = args.with_blanket_implementations;
    options
}

/// Collects public items from either the current commit or a given commit.
fn collect_public_items_from_commit(commit: Option<&str>) -> Result<Vec<PublicItem>> {
    let args = get_args();

    // Do a git checkout of a specific commit unless we are supposed to simply
    // use the current commit
    if let Some(commit) = commit {
        let git_root = git_utils::git_root_from_manifest_path(args.manifest_path.as_path())?;
        git_utils::git_checkout(commit, &git_root)?;
    }

    let json_path = match rustdoc_json::build(&args.rustdoc_json_toolchain, &args.manifest_path) {
        Err(BuildError::VirtualManifest(manifest_path)) => virtual_manifest_error(&manifest_path)?,
        res => res?,
    };
    if args.verbose {
        println!("Processing {:?}", json_path);
    }
    let options = get_options(&args);

    public_api_from_rustdoc_json_path(json_path, options)
}

fn public_api_from_rustdoc_json_path<T: AsRef<Path>>(
    json_path: T,
    options: Options,
) -> Result<Vec<PublicItem>> {
    let rustdoc_json = &std::fs::read_to_string(&json_path)
        .with_context(|| format!("Failed to read rustdoc JSON at {:?}", json_path.as_ref()))?;

    public_api_from_rustdoc_json_str(rustdoc_json, options).with_context(|| {
        format!(
            "Failed to parse rustdoc JSON at {:?}.\n\
            This version of `cargo public-api` requires at least:\n\n    {}\n\n\
            If you have that, it might be `cargo public-api` that is out of date. Try\n\
            to install the latest versions with `cargo install cargo-public-api`",
            json_path.as_ref(),
            MINIMUM_RUSTDOC_JSON_VERSION,
        )
    })
}

fn virtual_manifest_error(manifest_path: &Path) -> Result<PathBuf> {
    Err(anyhow!(
        "`{:?}` is a virtual manifest.

Listing or diffing the public API of an entire workspace is not supported.

Either do

    cd specific-crate
    cargo public-api

or

    cargo public-api --manifest-path specific-crate/Cargo.toml
",
        manifest_path
    ))
}

/// Wrapper to handle <https://github.com/rust-lang/rust/issues/46016>
fn main() -> Result<()> {
    match main_() {
        Err(e) => match e.root_cause().downcast_ref::<std::io::Error>() {
            Some(io_error) if io_error.kind() == std::io::ErrorKind::BrokenPipe => {
                std::process::exit(141)
            }
            _ => Err(e),
        },
        result => result,
    }
}
