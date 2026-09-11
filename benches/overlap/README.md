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
