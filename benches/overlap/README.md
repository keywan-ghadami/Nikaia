# What an overlap costs

ADR-033 lets two statements that touch nothing in common run at the same time.
This measures what that is worth, and it is the evidence §8.2 and §8.4 rest on.

Four binaries over the same two reads — the `TWO_READS` program in
`crates/nikaia/tests/ordering.rs`, reduced to plain Rust so there is nothing
between the measurement and the mechanism:

| binary | what it is |
|---|---|
| `strict` | the sequential lowering — the baseline |
| `scope` | the overlap's first lowering, `std::thread::scope` |
| `join` | what the emitter writes today, `nikaia_std::task::both` = `rayon::join` |
| `fixed` | both vehicles with no work in them at all — the floor |

Run it:

```sh
benches/overlap/run.sh            # the table, over seven file sizes
cargo run -p overlap-bench --release --bin fixed -- 20000
```

Keep these in step with the emitter by hand. They exist to be read next to the
generated Rust, not to be generated from it.

## …and what it costs on the runtime instead

[ADR-038](../../docs/specification/adr/adr-038.md) D4 starts the runtime before
`main`, and [ADR-033](../../docs/specification/adr/adr-033.md) §8.5 predicted
that a pair of operations on an I/O thread that is *already running* carries no
per-pair wake-up at all. `runtime` is that measurement, and unlike the four
above it goes through the **real** `nikaia_std` rather than a reduction of it:

| binary | what it is |
|---|---|
| `runtime` | `seq`, `join` and `fs::read_both` over the same two files, on the pre-started runtime |

```sh
benches/overlap/runtime.sh            # the table, both of D3's mechanisms
cargo run -p overlap-bench --release --bin runtime -- 20000 9
```

It measures **both** mechanisms — `io-method = "auto"` and `io-method =
"blocking"` pinned — because the answer differs between them, and that
difference is the finding. The numbers, the method and the confound are in
[`docs/runtime-cost.md`](../../docs/runtime-cost.md).
