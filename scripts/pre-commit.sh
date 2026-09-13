#!/bin/sh
# `rust-toolchain.toml` names `stable`, which is a moving target: a lint that
# arrives in a later stable is invisible to a gate run against an older one, and
# CI resolves `stable` on its own schedule. So a local run says nothing about CI
# until this has happened.
rustup update stable

# Format all code
cargo fmt

# Re-add all staged rust files to include formatting changes
# If there are staged rust files, add them again to capture formatting changes
FILES=$(git diff --name-only --cached | grep '\.rs$')
if [ -n "$FILES" ]; then
    for FILE in $FILES; do
        if [ -f "$FILE" ]; then
            git add "$FILE"
        fi
    done
fi

# Ensure no clippy warnings are present
# cargo clippy -- -D warnings
