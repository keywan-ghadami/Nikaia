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
        cargo test --quiet
        cargo clippy --quiet --all-targets -- -D warnings
        cargo +nightly miri test --quiet
        MIRIFLAGS=-Zmiri-tree-borrows cargo +nightly miri test --quiet
    )
done
