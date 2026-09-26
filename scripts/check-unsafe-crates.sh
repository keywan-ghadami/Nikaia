#!/bin/sh
# Every crate under crates/unsafe/ holds one `unsafe` topic and is checked on
# its own (ADR-218): its tests and lints on stable, and Miri under both of its
# aliasing models on nightly.
set -e
for dir in crates/unsafe/*/; do
    echo "== $dir"
    (
        cd "$dir"
        cargo fmt --check
        # Without features and with all of them, since a feature may carry
        # an `unsafe` of its own.
        cargo test --quiet
        cargo test --quiet --all-features
        cargo clippy --quiet --all-targets -- -D warnings
        cargo clippy --quiet --all-features --all-targets -- -D warnings
        cargo +nightly miri test --quiet --all-features
        MIRIFLAGS=-Zmiri-tree-borrows cargo +nightly miri test --quiet --all-features
    )
done
