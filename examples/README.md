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

Writing the two programs surfaced spec questions that a real implementation must answer.

### Resolved

**G1 — `std::fs` named no functions.** The module was described ("looks blocking, is async")
but had no surface. Now specified in Part III, 17.1: whole-file (`read`, `read_to_string`,
`write`), streaming (`lines`, `bytes`), handles (`open` → `File`, which implements `Cleanup`
so a failed flush surfaces as an error instead of being swallowed), memory mapping (`map`,
a **compile error under Lite** because WASM has no mmap and degrading it to a full read would
turn a constant-memory program into one that allocates its whole input), metadata and
directory calls, and a per-profile availability table.

**G2 — a grammar rule that folds instead of collecting.** An earlier draft of
this note claimed a gap around "applying a grammar per line". That was wrong,
and it misread what the language offers: you do not run a grammar per row. You
write down what the *file* looks like and get a parser for it, which drives
itself over the bytes — paying the DSL protocol once instead of a billion times.

The real gap was narrower and only appeared at the entry rule:
`rule file -> Vec[Reading] = measurement*` collects, and a billion `Reading`s do
not fit in memory. `fold` now exists in `winnow-grammar` (upstream, with
`SYNTAX.md` documentation and tests), so the entry rule threads an accumulator
and never builds a collection:

```nika
pub rule file -> Summary =
    fold(measurement, Summary::new, fn(acc, m) { acc.record(m) })
```

**G3 — tethered slices in user structs.** Specified for tokens a parser yields (Part II,
10.6), but not for a slice stored in a struct of one's own. `Reading.name` and the `HashMap`
key in `1brc.nika` are exactly that, and it is the difference between one allocation and a
billion.

Writing the example did more than expose the gap — it showed the existing rule was stated over
the wrong event. "Stored in a struct ⇒ tethered" (Part I 6.6, as of 0.0.7) puts a shared handle
on `Reading`, which this program builds a billion times; under the Advanced profile that is a
billion atomic increment/decrement pairs on one refcount word shared by every worker. Nothing
in the program actually escapes, so the right answer costs nothing at all.

[ADR-008](../docs/specification/adr/adr-008.md) settles it: view types stay (`&str` is a view
marker, not a lifetime), the rule is restated over **escape** rather than storage, a view has
three inferred states (Borrowed ⊑ Tethered ⊑ Owned), the shared handle sits on the *container*
rather than on each slice, and `.to_owned()` is never inserted for you. `@borrowed` turns "this
stays a plain reference" into a compile-time assertion for hot structs — used in `1brc.nika` on
`Reading`. Under those rules the program allocates nothing per row and does no refcount work in
the parallel section.

### Open

**G4 — chunking a buffer for `par_iter`.** `par_iter` is specified over a collection.
Splitting a buffer at line boundaries into one chunk per core, with each chunk a slice of the
original, has no spelled-out form.

**G5 — ordered iteration over a map.** 1BRC's output must be sorted by station name; neither
an ordered map nor `sort_by` over map entries is specified.

**G6 — the HTTP handler cannot see the request.** `std::http`'s signature (17.1) is
`.route("/") fn: "Hello World"` with no request argument, so a handler cannot read a query
parameter or set a status code. `fortunes` happens not to need either, which is why this went
unnoticed — the other six TechEmpower tests do need it.

**G7 — HTML escaping belongs in the template grammar's contract.** A template DSL that lets an
un-escaped value through a hole is an XSS hole with extra steps, and the compiler is the only
place that can enforce it for every hole, every time. That makes it a property of the grammar
(ADR-007, D4/D5), not of the caller's discipline.
