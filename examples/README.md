# Nikaia Examples

Programs here are **specification-level**: they show what Nikaia 0.0.7 is meant to look like.
The bootstrap parser (`crates/nikaia`) cannot compile them yet — it currently handles
functions, `let`, calls, literals, `spawn` and blocks, and nothing else.

That is deliberate, and it is what these files are *for*. Writing a real program against the
spec is the cheapest way to find out which parts of the spec are underspecified. The gaps each
example exposed are listed below; they read as a roadmap.

Programs that the bootstrap parser *can* handle live in `tests/samples/` and are checked by
`cargo test` (`crates/nikaia/tests/samples.rs`). When the parser catches up with an example,
move it there so it stops being a wish and starts being a test.

---

## Language benchmarks worth targeting

There are three established suites plus one modern challenge that fit a systems language.
They test different things, and only one of them matches what Nikaia currently claims to be
good at.

### 1. One Billion Row Challenge (1BRC) — **recommended first target**

Read a 13 GB text file of `station;temperature` lines, aggregate min/mean/max per station,
print them sorted. One input file, no dependencies, a one-paragraph spec.

It is the best fit because it stresses exactly the three things 0.0.7 asserts:

* **Scannerless parsing** (ADR-007 D1) — a billion lines of a tiny custom format.
* **Zero-copy / tethered slices** (Part II, 10.6) — station names must point into the input
  buffer; allocating a billion strings loses by an order of magnitude.
* **`par_iter` and the `sync` rule** (12.1, 12.6) — the aggregation is pure computation, so
  the Advanced profile can use every core, and the compiler can prove no task pauses.

If Nikaia is slow here, the grammar protocol's performance argument is wrong. That makes it a
useful benchmark rather than a demo. See `1brc.nika`.

### 2. Computer Language Benchmarks Game (CLBG) — **best for comparable numbers**

Ten programs with exact output specifications and reference implementations in ~30 languages,
so results are directly comparable against Rust, C and Go.

| Program | What it exercises in Nikaia |
| :--- | :--- |
| `n-body`, `spectral-norm`, `mandelbrot` | `sync` functions, `par_iter`, Advanced profile |
| `binary-trees` | allocation and deterministic teardown (`Drop` / `Cleanup`, ADR-006) |
| `fannkuch-redux` | scoped tasks (12.7) |
| `reverse-complement`, `k-nucleotide` | **stdin/stdout IO** plus hashing |
| `regex-redux` | the `grammar` construct against a real regex workload |
| `pidigits` | bignum FFI (Chapter 15) |

Small, self-contained, no external services. `n-body` is the usual first one because it is
~100 lines and purely numeric.

### 3. TechEmpower Web Framework Benchmarks — **the Lite profile's actual thesis**

`plaintext`, `json`, `db`, `queries`, `fortunes`, `updates`, `cached-queries`.

This is the benchmark that matches Nikaia's pitch: implicit async, IO density, share-nothing,
WASM-compatible Lite profile. `fortunes` in particular exercises the 0.0.7 DSL protocol
end-to-end — a SQL DSL with deferred parameters *and* an HTML template DSL with capture holes,
in one request handler. See `fortunes.nika`.

The cost is real: it needs an HTTP server, a Postgres driver and the Docker harness. This is a
target for after self-hosting, not before.

### 4. Are We Fast Yet? — **for judging codegen later**

Marr et al.'s cross-language suite (DeltaBlue, Richards, Havlak, CD, Json). Deliberately
written using only constructs every language shares, so it measures the *compiler* rather than
library tricks. No IO. Worth revisiting once there is a real backend to judge.

**Not applicable:** DaCapo and Renaissance (JVM-only), SPEC CPU (licensed, C/Fortran),
CoreMark (embedded C microbenchmark).

### Suggested order

1. `1brc` — validates the 0.0.7 claims, needs only file IO.
2. CLBG `n-body` — first comparable number against other languages.
3. CLBG `reverse-complement` and `k-nucleotide` — stdin/stdout IO with exact expected output.
4. TechEmpower `fortunes` — once an HTTP stack and a DB driver exist.

---

## Gaps these examples exposed

Writing the two programs below surfaced spec questions that are genuinely open. None of them
are invented for the sake of the example; each is something a real implementation must answer.

| # | Gap | Where it belongs |
| :-- | :--- | :--- |
| G1 | `std::fs` is described as "looks blocking, is async" but names no functions. `read`, `open`, `lines` and a memory-mapped variant all need signatures. A 13 GB input makes the mmap question unavoidable. | Part III, 17.1 |
| G2 | No syntax for how a `grammar` is applied to *many* inputs in a loop. `dsl G from x` is specified for one value; a per-line parser needs a cheaper form that does not re-enter the DSL protocol per row. | Part II, 10.2 |
| G3 | Tethered slices are specified for tokens the parser yields (10.6), but not for a slice a user stores in their *own* struct. 1BRC needs exactly that — `Stats` keyed by a name pointing into the input buffer. | Part I, 6.6 |
| G4 | `par_iter` is specified over a collection. Chunking a buffer at line boundaries and distributing the chunks has no spelled-out form. | Part II, 12.6 |
| G5 | Sorted iteration over a `HashMap` is required for 1BRC's output; no ordered map or `sort_by` on map entries is specified. | Part III, 17.1 |
| G6 | `std::http`'s handler signature (17.1) shows `.route("/") fn: "Hello World"` with no request argument, so there is no way to read a query parameter or set a status code. | Part III, 17.1 |
| G7 | The template DSL in `fortunes.nika` needs HTML escaping to be part of the *grammar's* contract, not the caller's discipline. That is a security property and should be stated. | ADR-007, D4/D5 |
