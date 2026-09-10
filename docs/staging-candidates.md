# Tier-1 Staging: Candidates, Non-Candidates, and How to Measure One

**Date:** September 10, 2026
**Status:** findings; §3 is measured and built, §2 is open
**Related:** [ADR-026](specification/adr/adr-026.md) §3 (the two tiers),
[ADR-010](specification/adr/adr-010.md) (the shipped precedent),
[upstream findings](upstream/winnow-grammar-findings.md)

ADR-026 §3 says Tier 1 — compiler-side staging — waits on nothing and is where the interesting
applications live. This file is the survey that follows from it: where the opportunities actually
are in today's code, which ones are already closed, and what it costs to check one. Recorded here so
it is not re-discovered, in the manner of `docs/upstream/winnow-grammar-findings.md`.

§3 is implemented and measured. Everything else here is a finding.

---

## 1. What is *not* a candidate

Written first, because each of these looks like one.

**Perfect hashing for a route table** — *as a first change*. ADR-026 §3 names it, and it has no
target today: there is no HTTP server. `crates/nikaia-std/src/` holds `cli`, `fs`, `hash`, `html`, `io`, `list`, `text` and
nothing else; `route` appears only in `examples/fortunes.nika` and `examples/README.md`, at
specification level. ADR-018 marked this "a decision and not a delivery". Building it means building
the server first — a feature, not a first step.

The requirement it stands for is real and outlives the example, and it is worth stating in the
stronger form: **when someone builds a server, their route table should benefit.** That says the
staging has to sit where *user* code reaches it, not inside a server we happen to write. Which is
what §2 is about — the literal-alternation path is exactly such a place, and it serves every grammar
anyone writes.

**`match` over string literals.** Nikaia's `match` lowers one-to-one to a Rust `match`
(`crates/nikaia/src/emit/mod.rs:1365-1379`, `match_pattern` at `:1592`), and rustc already lowers a
`match` on `&str` to length buckets plus `memcmp`. Nothing to add. No example uses it anyway — every
`match` in `examples/*.nika` is over an enum or a `char`.

**Character classes and terminator sets.** Solved upstream and measured. `AsciiClass` in
`winnow-grammar` tests up to three inclusive ranges **eight bytes at a time** (documented 380 MiB/s →
10.7 GiB/s). `until(a | b)` already collects its literal set at macro time and picks
`rest` / `scan_to_line_ending` / `scan_to_literal` / `scan_to_any` (memchr, memchr2, memchr3),
falling back to a per-character parser only above three needles. Every Nikaia grammar uses at most
two. There is no headroom here.

---

## 2. The real opportunity, and why it is not a one-file change

**A rule with N literal alternatives becomes N sequential prefix compares.** Nikaia's emitter has the
whole literal set at compile time and hands it on unchanged: `Pattern::Literal`
(`crates/nikaia/src/emit/mod.rs:735`) is a text copy, `Pattern::Choice` (`:732`) joins with `" | "`,
and `fn grammar` (`:660`) prints a `grammar! { … }` invocation. The golden fixture
`crates/nikaia/tests/fixtures/measurements_expected.rs` is the `.nika` text re-spelled.

Every parser decision is therefore made **one level down, in `winnow-grammar-macros`** — an external
git dependency. There a multi-alternative rule becomes `alt((literal(…), literal(…), …))`, tried in
order, each failure also recording an expectation for diagnostics. No first-byte switch, no length
bucketing, no trie.

Where it costs something today:

| Site | Alternatives | Note |
| :--- | ---: | :--- |
| `examples/json.nika:69-78` `rule value` | 9 (6 literal-led) | a `Json::Object` pays 8 failed compares |
| `crates/nikaia/src/parser/mod.rs:854` `cmp_op` | 6 | a two-byte peek would decide in one step |
| `crates/nikaia/src/parser/mod.rs:747` `assign_op` | 5 | |
| `crates/nikaia/src/parser/mod.rs:268` `receiver` | 3, sharing a `"&"` prefix | factoring a common prefix is a second, distinct win |

The compiler's own grammar has **35** literal-led alternatives, so a win there shows up in every
`cargo test`.

**The obstacle is location, not difficulty.** The change belongs in the dependency. Cargo fingerprints
a git dependency on its *commit*, so editing a vendored checkout has no effect (recorded in
`docs/upstream/winnow-grammar-findings.md`); testing needs a `[patch]` or a path override. The repo
already has the channel for handing such a finding over — that file exists for exactly this.

Before attempting it, read [ADR-026](specification/adr/adr-026.md) §3.1: it pre-registers the two ways
this claim goes wrong ("table-free is not automatically faster"; "no branch mispredictions is
backwards for the state machine itself") and states the defensible version — *the win is where the
analysis removes states*.

---

## 3. Two candidates inside this repository — **both now measured**

Both are done, and the measurements are below because both of them **went against the intuition that
proposed them**. That is the whole reason §6's rule exists, and it earned its keep on the first two
candidates it was applied to.

The harness is `crates/nikaia/tests/measure.rs`, ignored by default:

```text
cargo test -p nikaia --test measure -- --ignored --nocapture
```

Two workloads, in `benches/`, because a template has two shapes and they answer differently:
`template.nika` is **one growing table** (a few dozen literal bytes, a megabyte of result) and
`page.nika` is **a static page rendered many times** (the whole answer known but for two holes).

### 3.1 `String::with_capacity` for `dsl html` templates — kept, and it depends on the shape

The emitter knew the byte length of every literal segment and discarded it. It now reserves the
**floor**: every hole and every turn of a `<for>` adds to the result and none subtracts, so the
reservation is never too large and needs no threshold. A `<for>` body counts *once* — the compiler
knows the body, not the element count.

| workload | `String::new()` | `String::with_capacity` | |
| :--- | ---: | ---: | ---: |
| one growing table, 20 000 rows | 161,548,287 | 161,547,702 | **−0.0 %** |
| a static page, 20 000 renders | 346,633,864 | 334,754,555 | **−3.4 %** |

The zero is the more informative number, and it is structural rather than disappointing: for a
loop-dominated template the floor is a few dozen bytes of a megabyte, so the string doubles its way
up regardless and all that is saved is a constant 585 instructions — the same at 200 rows and at
20 000. Where the compiler knows almost the whole answer it is worth 3.4 %.

This ADR file predicted "a small win" and said saying so afterwards would look like an excuse. The
honest version is finer than that: **it is worth nothing on one shape and 3.4 % on the other**, and
which shape a program has is visible in its source.

### 3.2 `html::escape` — the table was **16 % slower**, and a mask was 65 % faster

The original scanned a five-element array per character, twice: once in `find` to decide whether to
allocate, once per character while copying. This file's advice was "make `escape` table-driven
*inside* `std`". Taken literally, that is a 256-entry `[&str; 256]` — and it is the worse answer:

| variant | Ir | vs. original |
| :--- | ---: | ---: |
| original: char scan, linear find over five, per-character push | 466,227,851 | baseline |
| 256-entry `[&str; 256]` table, copying runs | 541,727,867 | **+16 %** |
| one-word bitmask, copying runs | 180,967,750 | −61 % |
| **one-word bitmask, per-character push** | **161,547,659** | **−65 %** |

*(`benches/template.nika`, 20 000 rows, 40 000 holes.)*

Two findings, and neither would have survived being derived instead of measured.

**A table of fat pointers is 4 KB to walk where a mask is a register.** All five characters are below
64 — `"` 34, `&` 38, `'` 39, `<` 60, `>` 62 — so "is this one of the five" fits in a single `u64` and
touches no memory at all. This is exactly the failure [ADR-026](specification/adr/adr-026.md) §3.1
pre-registers as *"table-free is not automatically faster"*, met from the other side: table-**ful**
was slower, on the first candidate that tried it.

**Copying runs between escapes costs 12 %.** The shape that looks obviously better — `push_str` the
span between two escapes rather than pushing characters — loses on text this size: the index
arithmetic and the bounds check on each slice outweigh what the copies save. Measured against the
same mask, so the two halves are separated rather than credited to one another.

> The advice not to take the *staging* route here stands and is unaffected.
> [ADR-010](specification/adr/adr-010.md) D8 requires escaping at a hole to stay **unconditional**
> and ADR-017 D1 enforces it at `emit/mod.rs:988-996`; a position-specialised escape would read as
> weakening that whatever it measured. Everything above happens inside `std`, and the emitter still
> writes the same `Render::render` for every hole.

---

## 4. The size template

The only existing "compiler chooses between two implementations" in the codebase is
`TrustedMap`/`TrustedSet` against `HashMap`/`HashSet`: `Emitter::map_name`
(`crates/nikaia/src/emit/mod.rs:1074-1079`) plus `path()` (`:1053-1058`), fed by `trusted_input`
(`:239`, set at `:344`) from `crates/nikaia/src/contracts/trust.rs:48-96`.

**Around 150 lines in total.** That is the scale of a Tier-1 feature, and a useful check against any
proposal that claims to be one.

---

## 5. Measuring: the harness, and what it is for

Both halves matter and they are easy to confuse.

**Benchmark programs exist** — `examples/1brc.nika`, `n-body.nika`, `k-nucleotide.nika`, `json.nika`,
`fortunes.nika`, and the rest, drawn from 1BRC, the Computer Language Benchmarks Game and
TechEmpower.

**A harness exists now**: `crates/nikaia/tests/measure.rs`, and the two workloads §3 used are in
`benches/`. It lowers a `.nika` file with the real emitter, compiles it with `-O` through the same
plumbing the example tests use, runs it under callgrind and prints instructions retired. Ignored by
default, because a `cargo test` that shells out to valgrind is not a test suite.

A workload lives in `benches/` rather than `examples/` on purpose: an example is a program someone
reads to learn the language, and these are programs a compiler is weighed with.

**A harness was never the prerequisite,** because the project's own headline result was measured
without one. ADR-011 §193-206 records ADR-010's hasher under callgrind on 200 000 rows, the same tree
built twice with byte-identical output: **120.0 M → 89.1 M instructions**, 600 → 446 per row.
`valgrind` is installed. The reusable plumbing for build-and-run is
`crates/nikaia/tests/common/mod.rs:23-129` (`rustc()`, `deps_dir()`, `externs()`, `scratch_dir()`,
`compile()`).

**Measure more than instructions.** `docs/upstream/winnow-grammar-findings.md:157-166` reports four
columns — instructions, branches, mispredicts, D1 and LL misses — with the warning that
*"instruction counts alone would have been the wrong measurement here"*.

**Pick the benchmark that exercises the change.** This is easy to get wrong: `1brc.nika` is the
flagship for the *hasher* and has exactly one alternation, `until(";" | frame_end)`, which is already
staged to memchr2 with no headroom. It does **not** exercise literal dispatch. `json.nika` does, and
so does the compiler's own grammar.

**The cautionary tale is in-house.** `docs/upstream/winnow-grammar-findings.md:167-196` proposed a
plausible staging idea; the dependency's `TODO.md` §5 retracted it after measurement — the crossover
was at three characters, not the six to eight assumed, and the real win (−17 % on the compiler
parsing 2000 small functions) lay somewhere else entirely. Measurement is not ceremony here; it has
already changed an answer once.

---

## 6. Can the compiler even decide? — the constraint that answers it

The question behind every candidate above: a staging choice has a crossover, and the crossover moves
with the number of alternatives, the input distribution, and the microarchitecture. A memory access
that replaces a branch is not automatically cheaper. So *can* the compiler pick?

**Determinism settles the largest part of it, and rules out the obvious answer.** Autotuning — build
both variants, time them, keep the winner — is what ATLAS and FFTW do, and it is unavailable here:
[ADR-005](specification/adr/adr-005.md) D8 requires byte-identical output, enforced by CI
double-builds. A choice made from local timing or the host CPU produces two different programs on two
machines. **The emitter may therefore decide only from what is inside its pure function: the source
and the toolchain.** That is the same discipline [ADR-021](specification/adr/adr-021.md) D7 imposes on
the cache key, and it is the real answer to "what can the compiler know" — the same list, for the
same reason.

What remains splits four ways:

1. **Do not decide — someone already does.** Much of "table versus code" *is* LLVM's switch lowering:
   jump table, bit test or binary tree, chosen on density and count. Because we emit Rust, we get it
   for free, and a plainly emitted `match` often beats a hand-made choice. The same argument covers
   the input distribution: rustc ships PGO, which knows what we never will.
2. **A threshold on a structural property.** Alternative count, shared prefixes, literal or not — the
   compiler knows these, they are deterministic, and this is what practitioners actually ship. In
   house: `until(a | b)` switches at three needles. Also in house, the counter-example: the crossover
   in `upstream/winnow-grammar-findings.md` §5 sat at three characters, not the six to eight assumed.
   **A threshold must be measured, not derived.**
3. **It depends on the input distribution** — unknowable at build time. But the author has already
   said something: the specification makes alternative *order* the trial order, so priority is
   declared rather than needing a new annotation. Read what is there before inventing syntax.
4. **It depends on the actual CPU** — also not decidable at build time without giving up portability.
   The answer is runtime dispatch inside `std`: the ADR-010 shape again, where the compiler picks
   *which* function and `std` decides *how*.

**The rule that follows, and the reason this file exists:** a staging decision enters the compiler
only together with a measured crossover. Without one, the complexity is certain and the gain is not —
and the complexity is paid twice, once in the language surface and once in the generated code.

---

## 7. If someone picks this up

An order that reflects cost rather than appeal:

1. ~~`String::with_capacity`~~ — **done** (§3.1). It did establish the measurement loop, which was
   the reason for putting it first, and its outcome turned out to be two outcomes.
2. ~~`html::escape` inside `std`~~ — **done** (§3.2), and it is still true that this is a `std`
   optimisation and does **not** demonstrate the Tier-1 thesis. It demonstrated something else worth
   more at this stage: that the intuition in this file was wrong twice, in opposite directions.
3. First-byte or prefix dispatch for literal alternations — the actual prize, in the dependency,
   needing a `[patch]` to test and ADR-026 §3.1's caveats to survive. **Now the one that is left**,
   and the harness above is what it should be measured with.

And the thing worth saying plainly: **Tier 1 does not need Tier 2, and the applications people cite
as the reason for Tier 2 are all in this list.**
