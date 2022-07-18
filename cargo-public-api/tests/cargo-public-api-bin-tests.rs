//! To update expected output it is in many cases sufficient to run
//! ```bash
//! ./scripts/bless-expected-output-for-tests.sh
//! ```

use std::path::{Path, PathBuf};

use assert_cmd::Command;
use serial_test::serial;

#[serial]
#[test]
fn list_public_items() {
    let cmd = Command::cargo_bin("cargo-public-api").unwrap();
    assert_presence_of_own_library_items(cmd);
}

#[serial]
#[test]
fn list_public_items_with_lint_error() {
    let mut cmd = Command::cargo_bin("cargo-public-api").unwrap();
    cmd.args(["--manifest-path", "../test-apis/lint_error/Cargo.toml"]);
    cmd.assert()
        .stdout(
            "pub mod lint_error\n\
            pub struct lint_error::MissingDocs\n\
            ",
        )
        .success();
}

#[serial]
#[test]
fn custom_toolchain() {
    let mut cmd = Command::cargo_bin("cargo-public-api").unwrap();
    cmd.args(["--rustdoc-json-toolchain", "+nightly"]);
    assert_presence_of_own_library_items(cmd);
}

#[serial]
#[test]
fn list_public_items_explicit_manifest_path() {
    let mut cmd = Command::cargo_bin("cargo-public-api").unwrap();
    cmd.arg("--manifest-path");
    cmd.arg(current_dir_and("Cargo.toml"));
    assert_presence_of_own_library_items(cmd);
}

#[serial]
#[test]
fn virtual_manifest_error() {
    let mut cmd = Command::cargo_bin("cargo-public-api").unwrap();
    cmd.arg("--manifest-path");
    cmd.arg(current_dir_and("tests/virtual-manifest/Cargo.toml"));
    cmd.assert()
        .stdout("")
        .stderr(predicates::str::contains(
            "Listing or diffing the public API of an entire workspace is not supported.",
        ))
        .failure();
}

// We must serially run tests that touch the test crate git repo to prevent
// ".git/index.lock: File exists"-errors.
#[serial]
#[test]
fn diff_public_items() {
    ensure_test_crate_is_cloned();

    let mut cmd = Command::cargo_bin("cargo-public-api").unwrap();
    cmd.current_dir(test_crate_path());
    cmd.arg("--color=never");
    cmd.arg("--diff-git-checkouts");
    cmd.arg("v0.0.4");
    cmd.arg("v0.0.5");
    cmd.assert()
        .stdout(include_str!(
            "./expected-output/test_crate_diff_v0.0.4_to_v0.0.5.txt"
        ))
        .success();
}

#[serial]
#[test]
fn diff_public_items_with_manifest_path() {
    ensure_test_crate_is_cloned();

    let mut cmd = Command::cargo_bin("cargo-public-api").unwrap();
    cmd.arg("--manifest-path");
    cmd.arg(format!(
        "{}/Cargo.toml",
        &test_crate_path().to_string_lossy()
    ));
    cmd.arg("--color=never");
    cmd.arg("--diff-git-checkouts");
    cmd.arg("v0.0.4");
    cmd.arg("v0.0.5");
    cmd.assert()
        .stdout(include_str!(
            "./expected-output/test_crate_diff_v0.0.4_to_v0.0.5.txt"
        ))
        .success();
}

#[test]
fn diff_public_items_without_git_root() {
    let mut cmd = Command::cargo_bin("cargo-public-api").unwrap();
    cmd.arg("--manifest-path");
    cmd.arg("/does/not/exist/Cargo.toml");
    cmd.arg("--color=never");
    cmd.arg("--diff-git-checkouts");
    cmd.arg("v0.0.4");
    cmd.arg("v0.0.5");
    cmd.assert()
        .stderr(predicates::str::starts_with(
            "Error: No `.git` dir when starting from `",
        ))
        .failure();
}

#[serial]
#[test]
fn diff_public_items_with_color() {
    ensure_test_crate_is_cloned();

    let mut cmd = Command::cargo_bin("cargo-public-api").unwrap();
    cmd.current_dir(test_crate_path());
    cmd.arg("--color=always");
    cmd.arg("--diff-git-checkouts");
    cmd.arg("v0.6.0");
    cmd.arg("v0.7.1");
    cmd.assert()
        .stdout(include_str!(
            "./expected-output/test_crate_diff_v0.6.0_to_v0.7.1_colored.txt"
        ))
        .success();
}

#[serial]
#[test]
fn list_public_items_with_color() {
    let mut cmd = Command::cargo_bin("cargo-public-api").unwrap();
    cmd.arg("--color=always");
    cmd.assert()
        .stdout(include_str!(
            "./expected-output/list_self_test_lib_items_colored.txt"
        ))
        .success();
}

#[serial]
#[test]
fn diff_public_items_from_files() {
    let mut cmd = Command::cargo_bin("cargo-public-api").unwrap();
    cmd.arg("--diff-rustdoc-json");
    cmd.arg("../public-api/tests/rustdoc-json/example_api-v0.1.0.json");
    cmd.arg("../public-api/tests/rustdoc-json/example_api-v0.2.0.json");
    cmd.assert()
        .stdout(
            "Removed items from the public API
=================================
(none)

Changed items in the public API
===============================
-pub fn example_api::function(v1_param: Struct)
+pub fn example_api::function(v1_param: Struct, v2_param: usize)
-pub struct example_api::Struct
+#[non_exhaustive] pub struct example_api::Struct

Added items to the public API
=============================
+pub struct example_api::StructV2
+pub struct field example_api::Struct::v2_field: usize
+pub struct field example_api::StructV2::field: usize

",
        )
        .success();
}

#[serial]
#[test]
fn diff_public_items_missing_one_arg() {
    let mut cmd = Command::cargo_bin("cargo-public-api").unwrap();
    cmd.current_dir(test_crate_path());
    cmd.arg("--diff-git-checkouts");
    cmd.arg("v0.2.0");
    cmd.assert()
        .stderr(predicates::str::contains(
            "requires at least 2 values but only 1 was provided",
        ))
        .failure();
}

#[serial]
#[test]
fn verbose() {
    let mut cmd = Command::cargo_bin("cargo-public-api").unwrap();
    cmd.arg("--verbose");
    cmd.assert()
        .stdout(predicates::str::contains("Processing \""))
        .success();
}

#[test]
fn long_help() {
    let mut cmd = Command::cargo_bin("cargo-public-api").unwrap();
    cmd.arg("--help");
    assert_presence_of_args_in_help(cmd);
}

#[test]
fn short_help() {
    let mut cmd = Command::cargo_bin("cargo-public-api").unwrap();
    cmd.arg("-h");
    assert_presence_of_args_in_help(cmd);
}

fn assert_presence_of_own_library_items(mut cmd: Command) {
    cmd.assert()
        .stdout(
            "pub fn cargo_public_api::for_self_testing_purposes_please_ignore()\n\
             pub mod cargo_public_api\n\
             ",
        )
        .success();
}

fn assert_presence_of_args_in_help(mut cmd: Command) {
    cmd.assert()
        .stdout(predicates::str::contains("--with-blanket-implementations"))
        .stdout(predicates::str::contains("--manifest-path"))
        .stdout(predicates::str::contains("--diff-git-checkouts"))
        .success();
}

fn ensure_test_crate_is_cloned() {
    let path = test_crate_path();
    if path.exists() {
        print!("INFO: using ");
    } else {
        print!("INFO: cloning into ");
        clone_test_crate(&path);
    }
    // Print info about repo when running like this: cargo test -- --nocapture
    println!("'{}'", &path.to_string_lossy());
}

/// Helper to get the absolute path to a given path, relative to the current
/// path
fn current_dir_and<P: AsRef<Path>>(path: P) -> PathBuf {
    let mut cur_dir = std::env::current_dir().unwrap();
    cur_dir.push(path);
    cur_dir
}

/// Helper to clone the test crate git repo to the proper place
fn clone_test_crate(dest: &Path) {
    let mut git = std::process::Command::new("git");
    git.arg("clone");
    git.arg("https://github.com/Enselic/public-api.git"); // Tests still use this old git and the old name `public_items`
    git.arg("-b");
    git.arg("v0.7.1");
    git.arg("--single-branch");
    git.arg(dest);
    assert!(git.spawn().unwrap().wait().unwrap().success());
}

/// Path to the git cloned test crate we use to test the diffing functionality
fn test_crate_path() -> PathBuf {
    let mut path = get_cache_dir();
    path.push("cargo-public-api-test-repo");
    path
}

/// Where to put things that survives across test runs. For example a git cloned
/// test crate. We don't want to clone it every time we run tests.
fn get_cache_dir() -> PathBuf {
    // See https://doc.rust-lang.org/cargo/reference/environment-variables.html#environment-variables-cargo-sets-for-crates
    option_env!("CARGO_TARGET_TMPDIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
}
