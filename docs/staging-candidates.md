# Tier-1 Staging: Candidates, Non-Candidates, and How to Measure One

**Date:** September 10, 2026
**Status:** findings, not decisions
**Related:** [ADR-022](specification/adr/adr-022.md) §3 (the two tiers),
[ADR-010](specification/adr/adr-010.md) (the shipped precedent),
[upstream findings](upstream/winnow-grammar-findings.md)

ADR-022 §3 says Tier 1 — compiler-side staging — waits on nothing and is where the interesting
applications live. This file is the survey that follows from it: where the opportunities actually
are in today's code, which ones are already closed, and what it costs to check one. Recorded here so
it is not re-discovered, in the manner of `docs/upstream/winnow-grammar-findings.md`.

Nothing here is implemented.

---

## 1. What is *not* a candidate

Written first, because each of these looks like one.

**Perfect hashing for a route table** — *as a first change*. ADR-022 §3 names it, and it has no
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

Before attempting it, read [ADR-022](specification/adr/adr-022.md) §3.1: it pre-registers the two ways
this claim goes wrong ("table-free is not automatically faster"; "no branch mispredictions is
backwards for the state machine itself") and states the defensible version — *the win is where the
analysis removes states*.

---

## 3. Two candidates inside this repository

**`String::with_capacity` for `dsl html` templates.** `crates/nikaia/src/emit/mod.rs:999` emits
`String::new()` and then a run of `push_str` (`:1018`). The emitter knows the exact byte length of
every literal segment (`crates/nikaia/src/emit/template.rs:104`, `Segment::Text`) and discards it.
About fifteen lines including recursion into `Segment::For` bodies, no semantic risk, exercised by
`examples/escaping.nika` through `crates/nikaia/tests/examples.rs`. **The honest part: this is a small
win, and saying so afterwards would look like an excuse.**

**`html::escape` scanning a five-element array per character** (`crates/nikaia-std/src/html.rs:89`,
table at `:60-66`). The emitter *does* know each hole's `Position` (`emit/template.rs:32-48`) and
discards it at `emit/mod.rs:1024-1028`, emitting the same `Render::render` everywhere.

> **Do not take the staging route here.** [ADR-010](specification/adr/adr-010.md) D8 requires escaping
> at a hole to stay **unconditional**, and ADR-017 D1 is enforced at `emit/mod.rs:988-996`. A
> position-specialised escape reads as weakening that, whatever its measured gain. Make `escape`
> table-driven *inside* `std` and leave the emitter alone: same speed, no argument.

---

## 4. The size template

The only existing "compiler chooses between two implementations" in the codebase is
`TrustedMap`/`TrustedSet` against `HashMap`/`HashSet`: `Emitter::map_name`
(`crates/nikaia/src/emit/mod.rs:1074-1079`) plus `path()` (`:1053-1058`), fed by `trusted_input`
(`:239`, set at `:344`) from `crates/nikaia/src/contracts/trust.rs:48-96`.

**Around 150 lines in total.** That is the scale of a Tier-1 feature, and a useful check against any
proposal that claims to be one.

---

## 5. Measuring: the programs exist, the harness does not

Both halves matter and they are easy to confuse.

**Benchmark programs exist** — `examples/1brc.nika`, `n-body.nika`, `k-nucleotide.nika`, `json.nika`,
`fortunes.nika`, and the rest, drawn from 1BRC, the Computer Language Benchmarks Game and
TechEmpower.

**No benchmark harness exists.** No `[[bench]]`, no criterion, no `Instant::now`, no timing script
anywhere in `crates/`, `scripts/` or `tests/`. Nor is there a generator for large inputs.

**And a harness is not a prerequisite,** because the project's own headline result was measured
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

1. `String::with_capacity` — smallest, in-repo, no risk, small reward. Good for establishing the
   measurement loop on something whose outcome does not matter much.
2. `html::escape` table inside `std` — real gain, no ADR argument, but it is a `std` optimisation and
   does **not** demonstrate the Tier-1 thesis. Do not present it as one.
3. First-byte or prefix dispatch for literal alternations — the actual prize, in the dependency,
   needing a `[patch]` to test and §3.1's caveats to survive.

And the thing worth saying plainly: **Tier 1 does not need Tier 2, and the applications people cite
as the reason for Tier 2 are all in this list.**
