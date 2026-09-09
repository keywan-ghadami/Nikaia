# Nikaia Examples

Three of the four programs here compile, run, and are checked by `cargo test`. The fourth is
written at specification level — it shows what Nikaia 0.0.7 is meant to look like, and what it
needs is listed under *Gaps* below.

| | what it is | runs |
| :--- | :--- | :--- |
| [`1brc.nika`](1brc.nika) | the One Billion Row Challenge: a frame, a parallel fold, a billion rows | ✅ `crates/nikaia/tests/one_brc.rs` |
| [`calc.nika`](calc.nika) | a four-function calculator: the grammar protocol at its smallest | ✅ `crates/nikaia/tests/examples.rs` |
| [`access-log.nika`](access-log.nika) | a web log summarised: several fields per line, a report at the end | ✅ `crates/nikaia/tests/examples.rs` |
| [`fortunes.nika`](fortunes.nika) | the TechEmpower benchmark: a SQL DSL and an HTML template DSL in one handler | ❌ needs G6 and G7 |

Each of the three is compiled and run **under both profiles**, and their output must be
identical — that is the claim the profiles rest on, and a test is where it belongs rather than
in a paragraph. Each is the real file: the tests read `examples/*.nika` rather than a copy, so
an example cannot drift from what is checked.

They are deliberately different shapes. `1brc.nika` is the protocol at scale — `@frame`,
`par_fold`, a memory-mapped file, one accumulator per core
([ADR-014](../docs/specification/adr/adr-014.md): 8 million lines, 4 cores, 0.52 s → 0.14 s,
identical output), and one function it calls for every digit is written in Nikaia and compiled
into `std` by the compiler itself. `calc.nika` is the same protocol with none of that: no
frame, no fold, no I/O, one `pub rule` that returns a number — recursion and precedence, which
is what a grammar can say and a chain of combinators cannot. `access-log.nika` is the shape
most real work has: a line with several fields of different kinds, a record built from them,
and a report at the end — including what a *rejected* line looks like.

An example that is added has to be declared: either it runs and says what it prints, or it is
specification-level and its gaps are here. `crates/nikaia/tests/examples.rs` fails on a file
that is neither.

What the bootstrap compiler handles: functions and methods, `impl` blocks, `struct` and `use`
items, `let`, assignment, `for`, `if`, `return`, calls, field access, indexing, casts, struct
literals and constructors, lambdas, operators, `throws`/`catch`/`??`, string interpolation — and,
since [ADR-011](../docs/specification/adr/adr-011.md), the whole `grammar` construct: rules,
patterns, `@frame`, `fold`/`par_fold`, and `dsl … from …` with the driver the profile asks for.
Errors are reported on the `.nika` line that caused them
([ADR-012](../docs/specification/adr/adr-012.md)). There is no type checker.

That is deliberate, and it is what these files are *for*. Writing a real program against the
spec is the cheapest way to find out which parts of the spec are underspecified. The gaps each
example exposed are listed below; they read as a roadmap.

`tests/samples/` holds smaller programs that only have to *parse* — one construct each, checked
by `crates/nikaia/tests/samples.rs`. An example graduates the other way: when the compiler
catches up with one, it stays here and gains a row in `RUNNABLE`, because what makes it worth
having is that it is a whole program.

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

**G4 — chunking a buffer for `par_iter`.** `par_iter` is specified over a collection.
Splitting a buffer at line boundaries into one chunk per core, with each chunk a slice of the
original, had no spelled-out form.

[ADR-009](../docs/specification/adr/adr-009.md) closes it by **dissolving** it rather than
specifying `split_aligned`. A chunking helper asks the user to restate as an argument what the
grammar already says — that a measurement ends at `"\n"` — and then to hand-write the
split/parallel/merge pipeline that follows from it. Instead the grammar carries both halves:
`@frame` marks a rule as a resynchronization unit — and the compiler *verifies* it, and never
rewrites the grammar to make it so: what a frame can reach is either safe or rejected with the
rule and pattern named (a literal containing the boundary — CSV with quoted newlines — `any`,
`multispace0`, a syntactic rule whose implicit whitespace eats newlines, an `until` that does not
cover the boundary, `recover`). The flagship `NAME` therefore *says* `until(";" | frame_end)`,
where `frame_end` names the boundary of the enclosing frame; an intermediate design that silently
bounded `until(";")` was rejected in review because the same rule text then parsed differently
depending on what reached it. `par_fold(rule, init, step, merge)` supplies the monoid, and its
parser skips no whitespace at its entry, so pieces and the sequential parse agree on every input,
rejections included. All of it is in `winnow-grammar` `main` (`#[frame(boundary = …)]`,
`frame_end`, `par_fold`, `unchecked` for the formats a byte-string boundary cannot cut — its
ADR 16 names them), with `frames_<RULE>`, `merge_<RULE>` and the driver
`parse_<RULE>_pieces(input, ctx, Parallelism)` generated; Nikaia chooses the `Parallelism` from
the profile and the executor. The blind split, the
seam repair, the per-core accumulators and the reduce are then generated; `1brc.nika`'s `main`
is down to `let totals = dsl Measurements from data`.

The same ADR settles what the compiler may then do with the format the grammar states:
word-at-a-time scanning as a specified complexity rather than a hoped-for optimization
(portable SWAR baseline, SIMD only as a target-gated layer, so Lite and `wasm32` keep the same
story), hashing a view from its first bytes while equality stays full-content, and skipping the
unmap of a large read-only mapping at process exit. Parallelism stays opt-in: a plain `fold` is
never parallelised behind your back, because a merge over floats would make the answer depend on
the core count.

**G8 — the default hasher.** Raised by ADR-009, which framed it as a profile question and was
wrong to. The profile answers "which runtime", never "who supplied these bytes": a single-threaded
Lite server hashing attacker-supplied header names is exactly as vulnerable as an Advanced one,
and a compute job over operator-chosen data has no adversary in either.

[ADR-010](../docs/specification/adr/adr-010.md) decides it on the axis that matters, **provenance**,
and at the level where the user actually knows the answer — the place the input enters. Sources are
classified by `std` (network, IPC and database rows untrusted; files, argv, env and compile-time data
trusted), the state travels the edges ADR-008 already tracks and lands in the same Ledger, joins
conservatively, and fails safe at `dyn`/FFI barriers. The user overrides it at the source
(`fs::map(path; trusted: false)`) and a DSL for a wire format can pin an `@untrusted` floor its
callers cannot lower. Only then does the compiler pick an implementation: keyed hash with a random
seed for untrusted keys, fast hash for trusted ones — the profile enters as *how*, never as
*whether*. 1BRC keeps the fast path without a word about hashing; `fortunes` gets hardened without
anyone remembering to ask.

**G9 — bounded repetition (`digit{1,2}`) in the grammar protocol.** ADR-009 D5: fixed-width
numeric parsing is only sound where the grammar states the width bound. Delivered upstream —
`p{n}`, `p{n,}`, `p{n,m}`, together with a single-digit `digit` terminal that turned out to be
missing too (only the greedy `digit1` existed, which would have swallowed the run before a bound
could count anything). `1brc.nika`'s `TENTHS` now states its width and rejects a three-digit
temperature.

**G11 — a rejected parse said what was expected and never where.** Found by writing
`access-log.nika`, whose `catch` prints the failure: the message read
``expected a digit; found unexpected token ` ` `` with the rule stack under it, and no
position at all. A `ParseError` carries an offset and not the text it came from, so only the
code that has both can turn 1042 into *line 2, column 26* — and that is the driver
`dsl … from …` lowers to, which is the one place holding the input and the error together. It
now binds the input and renders against it, so the same message reads
``expected a digit; found unexpected token ` ` at line 2, column 26``.

Worth keeping apart from the two things it resembles. `docs/error-corpus.md` is about the
compiler's messages for `.nika` source, which have carried a line and a column all along; its
closing finding is that none of them shows the *line itself*, with a caret, the way a rustc
diagnostic routed through `--explain` does. This was smaller and worse: a program Nikaia
generates had no position at all.

The same example found the bug beside it: `dsl … from …` emitted its own `?`, so a `catch`
next to it was handed the value it was meant to inspect and nothing compiled. Both are pinned
in `crates/nikaia/tests/grammar_lowering.rs`.

### Open

**G10 — no tuple, and no sum type.** `calc.nika` needs to carry an operator alongside the
operand it applies to (`3 * 4`, where the `*` must survive until the fold). A tuple would say
it — `(&str, i64)` — and an enum would say it better, since there are exactly two operators.
Stage 0 has neither: a type is a name with optional generic arguments, so the example declares
a two-field `struct Step` to say what one line of a tail rule should have said. It costs
nothing at runtime and it is the first thing in these examples that is *worse* written in
Nikaia than in the language it lowers to, which is why it is written down here rather than
worked around quietly. Subtraction avoids it by being addition of a negation; division cannot.

**G5 — ordered iteration over a map.** 1BRC's output must be sorted by station name; neither
an ordered map nor `sort_by` over map entries is specified. ADR-010 D6 raises the stakes: an
untrusted map is seeded randomly, so its iteration order differs between runs, and the spec now
says so — which makes the explicit ordered form the only correct answer rather than merely the
tidy one.

**G6 — the HTTP handler cannot see the request.** `std::http`'s signature (17.1) is
`.route("/") fn: "Hello World"` with no request argument, so a handler cannot read a query
parameter or set a status code. `fortunes` happens not to need either, which is why this went
unnoticed — the other six TechEmpower tests do need it.

**G7 — HTML escaping belongs in the template grammar's contract.** A template DSL that lets an
un-escaped value through a hole is an XSS hole with extra steps, and the compiler is the only
place that can enforce it for every hole, every time. That makes it a property of the grammar
(ADR-007, D4/D5), not of the caller's discipline. ADR-010 D8 adds the boundary condition for when
this is specified: provenance may supply evidence and lints, but escaping at a hole must stay
**unconditional** — an analysis that skips escaping on "trusted" data turns one wrong
`trusted: true` into an XSS hole.


