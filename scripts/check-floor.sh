#!/bin/sh
# The oldest Rust Nikaia claims to work with is measured, not assumed
# (ADR-219): this builds the compiler and `std` with exactly that version and
# runs the whole test suite on it, which compiles every program the tests emit
# with it too. The version is read from the one place that holds it, the
# emitter's `RUST_FLOOR`.
set -e
floor=$(sed -n 's/^pub const RUST_FLOOR: &str = "\(.*\)";$/\1/p' crates/nikaia/src/emit/mod.rs)
[ -n "$floor" ] || { echo "RUST_FLOOR not found"; exit 1; }
echo "floor: Rust $floor"
rustup toolchain install "$floor" --profile minimal
CARGO_TARGET_DIR=target/floor cargo +"$floor" test --release -p nikaia -p nikaia-std
# Every crate under crates/unsafe/ says the same floor, and builds on it.
for dir in crates/unsafe/*/; do
    grep -q "^rust-version = \"$floor\"" "$dir/Cargo.toml" || {
        echo "$dir does not say rust-version = \"$floor\""
        exit 1
    }
    (cd "$dir" && CARGO_TARGET_DIR=../../../target/floor-unsafe cargo +"$floor" test --all-features --quiet)
done
for manifest in crates/nikaia/Cargo.toml crates/nikaia-std/Cargo.toml crates/orchestrator/Cargo.toml; do
    grep -q "^rust-version = \"$floor\"" "$manifest" || {
        echo "$manifest does not say rust-version = \"$floor\""
        exit 1
    }
done
