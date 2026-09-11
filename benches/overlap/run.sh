#!/usr/bin/env bash
# What ADR-033's lowering costs and when it pays.
#
# `strict.rs` and `effects.rs` are the two shapes the emitter produces for the
# same two reads - see `TWO_READS` in crates/nikaia/tests/ordering.rs - reduced
# to plain std so they can be built with a stock `rustc` and timed against each
# other. Keep them in step with the emitter by hand: they exist to be read next
# to the generated Rust, not to be generated.
#
# Usage: benches/overlap/run.sh [output-dir]
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
out=${1:-$(mktemp -d)}
mkdir -p "$out"

rustc -O "$here/strict.rs"  -o "$out/strict"
rustc -O "$here/effects.rs" -o "$out/effects"
cd "$out"

printf '%12s  %12s  %12s  %8s\n' 'bytes/file' 'strict (ms)' 'effects (ms)' 'speedup'
for size in 5 65536 262144 524288 1048576 4194304 8388608; do
    # base64 of urandom, truncated to the exact size: incompressible, and
    # ASCII, so `read_to_string` validates it exactly as the real program
    # would. `truncate` rather than a second `head` - a `head` that closes the
    # pipe early would fail the run under `pipefail`, not shorten the file.
    for name in eins zwei; do
        head -c "$size" /dev/urandom | base64 > "$name.txt"
        truncate -s "$size" "$name.txt"
    done

    # Enough runs that a pair costs more than the clock's resolution, few
    # enough that the large sizes finish.
    if   [ "$size" -lt 100000 ];  then n=20000
    elif [ "$size" -lt 2000000 ]; then n=2000
    else n=200
    fi

    ./strict "$n" >/dev/null; ./effects "$n" >/dev/null   # warm the page cache
    s=$(./strict "$n" | awk '{print $2}')
    e=$(./effects "$n" | awk '{print $2}')
    printf '%12s  %12s  %12s  %8s\n' "$size" "$s" "$e" \
        "$(awk -v s="$s" -v e="$e" 'BEGIN{printf "%.2fx", s/e}')"
done
