#!/bin/sh
# Run a command on the Bridge-IR backend's pinned nightly (ADR-001 D5).
#
#   scripts/bridge-toolchain.sh                     install it, print its name
#   scripts/bridge-toolchain.sh cargo build --workspace
#   scripts/bridge-toolchain.sh cargo test --workspace --release
#
# The channel and its components live in `bridge-toolchain/rust-toolchain.toml`
# and are never repeated here: `rustup show` inside that directory installs
# exactly what the file names, the same mechanism CI uses for the stable
# toolchain at the root. The channel is read back out of the same file so that
# the command can run at the workspace root, where the manifest is.
#
# The build gets a target directory of its own, and that is not tidiness. Two
# toolchains sharing one `target/deps` put two rustcs' rlibs in one directory,
# and `crates/nikaia/tests/common/mod.rs` finds the crates a generated program
# links by *reading that directory* - it cannot tell a stable `winnow_grammar`
# from a nightly one, so the test compiles against the wrong copy and rustc says
# "compiled by an incompatible version of rustc" about a file nobody chose.
# `CARGO_TARGET_DIR` from the environment still wins, for a caller who knows.
set -eu

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
dir="$root/bridge-toolchain"
file="$dir/rust-toolchain.toml"

# Installs the channel and every component the file names, and is a no-op once
# they are there. Its report is kept rather than discarded so that a toolchain
# that cannot be installed says why.
if ! report=$(cd "$dir" && rustup show 2>&1); then
    printf '%s\n' "$report" >&2
    echo "bridge-toolchain.sh: could not install the toolchain $file names" >&2
    exit 1
fi

channel=$(sed -n 's/^[[:space:]]*channel[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' "$file")
if [ -z "$channel" ]; then
    echo "bridge-toolchain.sh: $file names no channel" >&2
    exit 1
fi

if [ "$#" -eq 0 ]; then
    echo "$channel"
    exit 0
fi

cd "$root"
CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-$root/target/bridge}
export CARGO_TARGET_DIR
# `rustup run` puts the toolchain in the environment, which outranks the
# repository root's `rust-toolchain.toml`.
exec rustup run "$channel" "$@"
