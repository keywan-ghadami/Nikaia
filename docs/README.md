# The documentation, and what belongs in each part

Four kinds of writing live here. They are separated on purpose: a reader who
wants to know what a program *means* should never have to read what a benchmark
did on someone's laptop, and an experiment that later turned out wrong should
never be able to quietly change the language.

## 1. The specification — [`specification/`](specification)

**What the language is.** Normative. If the compiler disagrees with it, one of
the two is a bug and the disagreement gets recorded.

* [Part I — The Language Core](specification/10-nikaia-light.md): the language
  a person learns first.
* [Part II — Concurrency and Metaprogramming](specification/20-nikaia-advance.md):
  concurrency, parallelism, the grammar protocol.
* [Part III — Tooling](specification/30-nikaia-tooling.md): the CLI, the
  ledger, `std`, the diagnostics contract.

The three parts follow one language standard,
[`specification/STYLE.md`](specification/STYLE.md): rule, then consequence, then
at most one short *Design rationale* ending in the ADR. A page carries no
history, no argument, no instruction counts and no timings. Those are the next
two sections.

A rule specified ahead of the compiler says so in a status note of one form,
`> **Implementation status:** <value>.`, with a controlled vocabulary of six
values. That note is maintained; a stale one is a defect, because a reader
cannot tell a plan from a promise.

## 2. The decisions — [`specification/adr/`](specification/adr)

**Why the language is that way.** One decision per record, numbered `D1…Dn` so
it can be cited, plus the evidence that settled it — including the number, when
a number is what decided it.

Start at the [ADR index](specification/adr/README.md): every record, what it
decides, whether it is built, and what supersedes what.

An ADR is written once. When a later decision **changes** it, the change is a
new ADR that names the section or `Dn` it displaces — never a silent edit. That
is why records marked *superseded* are still here and still cited: every
supersession in this project is **partial**.

**But a decision that is *withdrawn* is removed, not superseded.** When the
project stops doing something altogether — a dependency it no longer has, a
mechanism it no longer builds, a path nothing takes — the records are rewritten
as though it had never been there, and one sentence says what is done instead.
The ADRs do not owe a reader every loop the project turned; they owe the reason
the current answer is the answer. Where the withdrawn thing went, and why, is
[`../CHANGELOG.md`](../CHANGELOG.md) and these notes — which is what they are
for.

The line between the two: a supersession leaves a **live citation** behind,
because something still rests on the displaced part. A withdrawal leaves none.
If after the rewrite anything still cites it, it was a supersession and the
rewrite was wrong.

## 3. The notes — this directory

**What was tried, measured and learned.** The laboratory notebook. Nothing here
is normative and nothing may depend on it to know what a program means.

* [`open-work.md`](open-work.md) — **the running list**: what is found and
  broken, what an ADR decided and the compiler does not do yet, and which
  specification sentences a later decision made false. Every entry carries its
  reproduction or says it has none.
* [`open-decisions.md`](open-decisions.md) — the questions work cannot settle,
  each with what is blocked by it, the options, and a recommendation. **Only
  what is open**: an answered question leaves this file for its
  [ADR](specification/adr/) rather than staying with a note on it, so the answer
  and its reasoning live in one place.
* [`handoff.md`](handoff.md) — how to work on the **parser backend**: testing a
  change against Nikaia, what the upstream patch does, and what was tried and
  must not be redone. A guide; the open work that was in it is in
  [`open-work.md`](open-work.md).
* [`staging-candidates.md`](staging-candidates.md) — the survey of where
  compile-time staging would pay, and what it costs to check one.
* [`error-corpus.md`](error-corpus.md) — twenty-six broken `.nika` files and
  what the compiler says about each, before and after.
* [`stored-views.md`](stored-views.md) — the `E0621` a stored naked view used to
  produce, the two increments of the analysis that replaced it, and what the full
  tether lattice would have to track that they do not: the seven things it needs,
  the two the compiler has, and the measurement showing that building the
  reachable half first refuses a program in `examples/` that works today.
* [`from-for-throws-and-touches.md`](from-for-throws-and-touches.md) — whether
  ADR-029 D3's `sync = "from(f)"` argument carries to the other two effect
  columns: the smallest program for each, the ledger it produced, and the one
  place the argument breaks.
* [`spec-promises.md`](spec-promises.md) — every construct and command the
  specification names, run against the compiler one probe at a time; the
  evidence behind the **Status** notes in Parts I–III.
* [`language-review.md`](language-review.md) — the surface language read as a
  newcomer would read it: where it contradicts its own promise, where a program
  can be silently wrong, where the syntax will make people curse, and the order
  in which turning it inside out would pay. Every claim reproduced with a probe
  or quoted from the page.
* [`subprocess-cost.md`](subprocess-cost.md) — what invoking `rustc` as a child
  process costs, against the codegen it wraps: 0.64 % of a compile, which is what
  the text interface of ADR-004 D1 is paid for with.
* [`std-sysroot.md`](std-sysroot.md) — what `std`'s build graph cost when `std`
  had the compiler as a build dependency, the 58 packages that existed only for
  that, and the two things that were expected to decide it and did not.
* [`nightly-cost.md`](nightly-cost.md) — what a toolchain weighs, what the
  nightly-only borrow checker would have bought and cost, and the measurements
  that led to the withdrawal recorded in `CHANGELOG.md`.
* [`runtime-cost.md`](runtime-cost.md) — what a pair of operations costs on a
  runtime that is already running, and the finding that only the completion
  path gets ADR-033 §8.5's zero.
* [`break-continue-cost.md`](break-continue-cost.md) — `break` and `continue`
  built and measured *before* they were decided, so that the question
  [ADR-070](specification/adr/adr-070.md) D2 left open could be answered with
  numbers: what a jump costs at run time (nothing) against what the shapes
  standing in for it cost (a quarter of a `while`, and all of a `for` that
  wanted to stop early), what two more rules cost the parser and why the answer
  is per *block* rather than per statement, and the four constructs a jump may
  not leave because each is a function in the language below. §7 says what
  [ADR-084](specification/adr/adr-084.md) took from it and what it did not.
* [`rc-or-arc.md`](rc-or-arc.md) — what an atomic reference count costs, whether
  `Rc` and `Arc` differ in anything a program can observe, and the prototype that
  asked ADR-037 D3's open question of a value instead of a build. The method, the
  machine and the false starts; §11 says which option the owner took, and
  ADR-037 D6–D8 are the decision.
* [`mutex-floor.md`](mutex-floor.md) — what an always-`Mutex` floor for
  `Locked` would cost against Part II 12.2's no-pausing rule: why blocking is
  not pausing in this language, the one ledger line the answer turns on, and
  the 11 ns an uncontended acquisition costs where it can never be contended.
* [`lock-free.md`](lock-free.md) — whether ADR-039 §3's open door is worth
  walking through: what a compare-and-swap loop would buy over the shipped
  crossing door, the **3.7×** defect the measurement uncovered on the way, and
  §6's finding that what decides it is not the saving but whether a real program
  shares a word-sized value across threads at all.
* [`unique-ownership.md`](unique-ownership.md) — **an idea, not a measurement**:
  extending the ledger with a proof that a mutable value is reachable from
  exactly one task at a time, so a door over it would need no lock. Why it is in
  character (ADR-037 D7 already picks `Rc` against `Arc` per value by proving a
  reachability property), the five things the first afternoon would run into,
  and the one that would kill it.
* [`fixed-map-lookup.md`](fixed-map-lookup.md) — a fixed map's lookup, `match`
  against a perfect hash, and the crossover ADR-079 D3 left unnamed.
* [`zero-copy-send.md`](zero-copy-send.md) — what it costs to answer a request
  with a file, and the trap in the obvious reading of *map it, don't read it*.
* [`foreign-runtime.md`](foreign-runtime.md) — a Nikaia program that starts
  `hyper`, and what ADR-038 D7's two rules cost today.
* [`automatic-clone.md`](automatic-clone.md) — why having the compiler insert the
  `.clone()` a detached task makes necessary is not a convenience: it adds an
  owner and so moves a cleanup point, which is the observable change ADR-037 D3
  refused to infer and ADR-005 §3 already rejected outright, so the proposal has
  to answer those records rather than appeal to typing saved.
* [`technical_notes.md`](technical_notes.md) — the compiler-internals findings
  from the path that was withdrawn; kept as the notebook page it is.
* [`withdrawn-one-way-down.md`](withdrawn-one-way-down.md) — what the nightly
  toolchain, the Bridge-IR backend and the `rustc_ast` path were, what was
  measured about them, and why they were withdrawn without a further
  measurement.
* [`toolchain_architecture.md`](toolchain_architecture.md) — how the crates in
  the workspace stack up.
* [`project_status_and_roadmap.md`](project_status_and_roadmap.md) — what runs
  today and what is next.
* [`upstream/`](upstream) — findings and patches handed to dependencies.

This is the right home for the detail the specification must not carry: the
method, the machine, the false starts, the measurement that overturned an
intuition.

## 4. The history — [`../CHANGELOG.md`](../CHANGELOG.md)

**What changed, when, and what it cost.** Per release. A correction to an
earlier claim is recorded here as well as in the record it corrects.

---

## The rule of thumb

> If deleting a sentence would change what a correct Nikaia program is, it
> belongs in the **specification**. If it would only change whether you *agree*
> with the language, it belongs in an **ADR**. If it would only change how much
> you trust the number, it belongs in the **notes**.
