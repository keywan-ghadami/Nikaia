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

A spec section states the rule and, where the rule is surprising, *why it is the
rule in one or two sentences* — then links the ADR. It does **not** carry
instruction counts, timings, what an alternative would have cost, what an
earlier draft said, or what was discovered while implementing it. Those are the
next two sections.

A rule specified ahead of the compiler says so in a short **Status** note naming
what is built and what is not. That note is maintained; a stale one is a defect,
because a reader cannot tell a plan from a promise.

## 2. The decisions — [`specification/adr/`](specification/adr)

**Why the language is that way.** One decision per record, numbered `D1…Dn` so
it can be cited, plus the evidence that settled it — including the number, when
a number is what decided it.

Start at the [ADR index](specification/adr/README.md): every record, what it
decides, whether it is built, and what supersedes what.

An ADR is written once. When a later decision changes it, the change is a new
ADR that names the section or `Dn` it displaces — never a silent edit. That is
why records marked *superseded* are still here and still cited: every
supersession in this project is **partial**.

## 3. The notes — this directory

**What was tried, measured and learned.** The laboratory notebook. Nothing here
is normative and nothing may depend on it to know what a program means.

* [`handoff.md`](handoff.md) — work left open at the end of a session, and where
  it stands.
* [`staging-candidates.md`](staging-candidates.md) — the survey of where
  compile-time staging would pay, and what it costs to check one.
* [`error-corpus.md`](error-corpus.md) — twenty-six broken `.nika` files and
  what the compiler says about each, before and after.
* [`subprocess-cost.md`](subprocess-cost.md) — what invoking `rustc` as a child
  process costs, against the codegen it wraps, and why ADR-004 D3's in-memory
  exit is not worth building yet.
* [`runtime-cost.md`](runtime-cost.md) — what a pair of operations costs on a
  runtime that is already running, and the finding that only the completion
  path gets ADR-033 §8.5's zero.
* [`technical_notes.md`](technical_notes.md) — `rustc_private` internals as
  found on the pinned nightly.
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
