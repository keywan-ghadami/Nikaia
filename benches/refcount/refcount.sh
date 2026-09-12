#!/usr/bin/env bash
# What an atomic reference count costs, against a plain one — the number
# ADR-037 D3's open question turns on.
#
# The method, the machine and the spread are in `docs/rc-or-arc.md`; this script
# prints the machine and the load so a table can never be read without them
# (`docs/runtime-cost.md` §6.3: absolutes on this box move by 1.4–1.9× from one
# day to the next, and only the ratios travel).
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
n=${1:-50000000}
repeats=${2:-9}
threads=${3:-$(nproc)}

cargo build --manifest-path "$root/benches/refcount/Cargo.toml" --release --quiet

echo "machine: $(nproc) cores, $(uname -sr)"
echo "rustc:   $(rustc --version)"
echo "loadavg before: $(cut -d' ' -f1-3 /proc/loadavg)"
echo
"$root/target/release/refcount" "$n" "$repeats" "$threads"
echo "loadavg after:  $(cut -d' ' -f1-3 /proc/loadavg)"
