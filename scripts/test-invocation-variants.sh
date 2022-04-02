#!/usr/bin/env bash
set -o errexit -o nounset -o pipefail -o xtrace

# Make sure we can conveniently run the tool from the source dir
cargo run

# Make sure we can conveniently run the tool from the source dir on an external crate
cargo run -- --manifest-path "$(pwd)"/Cargo.toml

# Install the tool
cargo install --debug --path .

# Make sure we can run the tool on the current directory stand-alone
cargo-public-items

# Make sure we can run the tool on an external directory stand-alone
cargo-public-items --manifest-path "$(pwd)"/Cargo.toml

# Make sure we can run the tool on the current directory as a cargo sub-command
cargo public-items

# Make sure we can run the tool on an external directory as a cargo sub-command
cargo public-items --manifest-path "$(pwd)"/Cargo.toml

# Make sure diffing works
if [ -d "${HOME}/src/public_items" ]; then
    cd ~/src/public_items

    original_branch=$(git branch --show-current)

    git stash
    cargo public-items --diff-git-checkouts v0.0.4 v0.0.5

    git stash
    cargo public-items --diff-git-checkouts v0.2.0 v0.3.0

    git stash
    cargo public-items --diff-git-checkouts v0.3.0 v0.4.0

    if [ -n "${original_branch}" ]; then
        git checkout "${original_branch}"
    fi
fi
