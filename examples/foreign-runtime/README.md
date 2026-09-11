# A Rust crate that brings its own runtime — the ADR-038 D7 experiment

[ADR-038](../../docs/specification/adr/adr-038.md) D7 says a Nikaia program may
depend on a Rust crate that starts threads and an event loop of its own, and
names two rules that are supposed to keep that sound. This directory is the
program that tests it: a Nikaia project depending on `hyper` and `tokio` through
[ADR-002](../../docs/specification/adr/adr-002.md) D1's crates.io passthrough.

**What it found is in [`docs/foreign-runtime.md`](../../docs/foreign-runtime.md).**
That file is the laboratory notebook — the method, the machine, and the two
false starts. This one says only what is here and how to run it.

## What is here

| | |
| :--- | :--- |
| `shim/` | A plain Rust crate over `hyper`, `hyper-util` and `tokio`. Not a member of the Nikaia workspace it sits inside, so `cargo test --workspace` never builds it. |
| `serve/` | A Nikaia project that starts the server, serves one request, and sends a `String` to a thread the foreign runtime owns. **Builds and runs.** |
| `crossing/` | The same, with a value that may not cross a thread. **Must not build**, and who refuses it is the finding. |
| `smuggled/` | The same crossing through a foreign API that says `unsafe impl Send` about a type that is not. **Builds and runs**, and nothing anywhere complains. |
| `overlaps.nika` | Four pairs of statements, three of them with a foreign call in, for `--overlaps`. |

Each project's `nikaia.toml` reaches the shim as
`hyper-shim = { type = "rust", path = "../../../../shim" }`. The path is
relative to the *generated* `Cargo.toml`, which a build writes into
`target/nikaia/build/`, which is why it climbs four levels and not one. A
version from crates.io would need no such care; a path dependency is used here
only so the experiment is self-contained.

## Running it

Everything below needs the network the first time, and `cargo`.

```sh
# The server, and a legal crossing. Prints two lines.
nikaia run --project examples/foreign-runtime/serve

# The illegal crossing. Expected to fail.
nikaia build --project examples/foreign-runtime/crossing

# The crossing a foreign `unsafe impl Send` lets through. Expected to succeed.
nikaia run --project examples/foreign-runtime/smuggled

# What the ordering analysis says about a call it knows nothing about.
nikaia --input examples/foreign-runtime/overlaps.nika \
       --overlaps --user-parallelism yes --backend rust --output /tmp/o.rs
```

`serve` prints:

```text
served: GET /hello on tokio-rt-worker
crossed: a String is Send on tokio-rt-worker, called from main
```

## In `cargo test`

`crates/nikaia/tests/foreign_runtime.rs`. One test runs on every build — the
one that needs neither Cargo nor the network, and which checks a *guarantee*
rather than a finding: a foreign call has no ledger entry, so
[ADR-033](../../docs/specification/adr/adr-033.md) D4 makes it order against
everything. The three that drive `cargo` are `#[ignore]`d, because the first run
fetches twenty-six crates:

```sh
cargo test --test foreign_runtime -- --ignored --nocapture
```

**Not wired into CI**, and that is deliberate: those three need crates.io.
