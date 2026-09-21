# Open decisions — the questions that need the owner

## The shape this page is for
The
entries here are written to be *answerable* — what is blocked, the options, a
recommendation, and what either direction costs if it is wrong.

An answer is an [ADR](specification/adr/), and
the moment a question is answered its entry leaves this file rather than
staying with a note on it. What is merely **unbuilt** is in
[`open-work.md`](open-work.md) — an ADR said what happens and the compiler does
not do it yet, which needs work and not a ruling. Each entry says what the
question is, why it is the owner's, and what this file recommends.

## Open

### Does a **described** foreign function say whether it puts its argument on a thread?

**What is blocked.** [`open-work.md`](open-work.md)'s *a described foreign call
is not asked whether it threads*, whose last paragraph says a column for this
*is a question and belongs here when somebody asks it*.

**Measured.** `NK2502` asks its question of a call **nothing** describes, which
is [ADR-038](specification/adr/adr-038.md) D7's own wording. A *described*
foreign call is not asked, because no column says whether it threads —
`crates/nikaia/tests/send.rs` holds that silence on purpose so nobody
rediscovers it. `examples/foreign-runtime/crossing` is exactly that shape: a
handle whose description says `crosses = false`, handed to
`hyper_shim::across_a_thread`, which the description names. The build fails —
**against the `.nika` line**, which is [Part III C.1](specification/30-nikaia-tooling.md)'s
rule kept — with `rustc`'s words:

```text
`Rc<String>` cannot be sent between threads safely
```

[ADR-005](specification/adr/adr-005.md) D7 records the *text* as the open half,
and this is it.

**Why it is the owner's.** A column is a claim somebody writes by hand and a
reviewer checks, and *does this function put what it is given on a thread* is
**not visible in a signature**. That is the same difficulty `crosses` has, and
[ADR-123](specification/adr/adr-123.md) D2 answered it with *written by hand and
never inferred*. Whether this language wants a second such claim is a question
about how much a description is trusted to say.

**The options.**

* **A — a column**, `threads`, hand-written like `crosses` and reviewed with the
  rest of the description. `NK2502` then asks a described call too, and the
  refusal is in this compiler's words.
* **B — no column.** The answer stays `rustc`'s `Send` bound on the right line,
  and [C.2](specification/30-nikaia-tooling.md)'s *in the compiler's own words*
  is knowingly not met for this one case. This is today.
* **C — the describer derives it.** Out of reach and worth saying so:
  `nikaia describe` reads rustdoc JSON, which carries signatures and not bodies,
  and threading is a fact about a body.

**What this page recommends: A, with the third value.** The precedent is exact —
`crosses = false` is already a hand-written claim about something a signature
cannot show — and the description is reviewed like code
([ADR-104](specification/adr/adr-104.md)). But it must have **three** values and
not two, for the reason `crosses` has three: the absence of the word is *nobody
said*, never *it does not*. A `threads = false` written by a hopeful hand is a
false **silence**, which is the polarity
[ADR-010](specification/adr/adr-010.md) D1 calls a vulnerability generator, and
the refusal must fire on the claim rather than on its absence.

**What it costs if wrong**: one more line a describer has to get right, on a
surface where getting it wrong is silent. That is the argument for B, and it is
a real one — which is why the recommendation carries the three-value shape
rather than the column alone.

### Is tier-1 staging withdrawn, or is it still on the list?

**What is blocked.** The roadmap's *Tier-1 staging (compiler-side)* box, which is
one of the three points the language area is short of, and which is `[ ]` on a
page that reads as a list of things still to build.

**Measured, and by the file behind the box itself.**
[`staging-candidates.md`](staging-candidates.md) was written to find candidates
and it found three. Two are **done** — `String::with_capacity` and `html::escape`
— and both turned out to be `std` optimisations rather than demonstrations of
the thesis. The third, first-byte dispatch for literal alternations, was
profiled and is **not the prize**: under 3 % of the parse, against 56 % for the
whitespace the alternation was re-skipping. The whitespace hoist that came out
of that profile is **−19.8 %** and is merged. What is left of the original item
is a ≤3 % ceiling in a dependency.

That file's own conclusion is a rule: *a staging decision enters the compiler
only together with a measured crossover; without one the complexity is certain
and the gain is not, and the complexity is paid twice — once in the language
surface and once in the generated code.* And its last line: **Tier 1 does not
need Tier 2, and the applications people cite as the reason for Tier 2 are all
in this list.**

**Why it is the owner's.** Every candidate anybody wrote down is closed, and the
rule the survey ended on says the next one may not be opened without a
measurement that nobody has. So the box is not *unbuilt work* — it is a
direction the project examined and did not take. Whether that is a **withdrawal**
is a judgement about what the language is for, which is the one thing this page
never decides on its own. [ADR-070](specification/adr/adr-070.md) D1's `loop` is
the precedent for the shape: *the absence is a decision rather than an
omission*, written down so a reader can tell the two apart.

**The options.**

* **A — withdraw it**, with a record saying what was measured and what would
  reopen it: a candidate with a crossover, measured before the complexity is
  paid. The box leaves the roadmap the way `loop` left Part I, and the language
  area is counted out of 25 rather than 26.
* **B — keep it open.** The box stays `[ ]` and the page keeps saying there is
  work here. Honest only if somebody intends to go looking for a fourth
  candidate, because the three that existed are closed.
* **C — keep it, and make the box say what the file says.** No record, no
  withdrawal: the box is re-written to *examined, no candidate survived
  measurement*, and it stays `[ ]` as a standing invitation.

**What this page recommends: A.** The argument is the one
[ADR-084](specification/adr/adr-084.md) makes about a keyword and
[`staging-candidates.md`](staging-candidates.md) makes about a staging rule —
the cost is certain and the gain is not — and a `[ ]` on a progress page is a
promise to somebody reading it. C is the cheap half of A and is worth taking if
the answer is *not yet rather than no*: it costs one paragraph and no record,
and it keeps the door where it is.

**What it costs if wrong**: a withdrawal has to be undone by a record, which is
the price [ADR-070](specification/adr/adr-070.md) already paid for `loop` and is
small — the word stays reserved there, and here there is not even a word to
reserve. The real cost is the other way: a `[ ]` that nobody intends to build
makes every other `[ ]` on the page worth less.

### What does a program write, for a map the build can see?

**What is blocked.** [ADR-079](specification/adr/adr-079.md) D3's fixed map —
the last unbuilt part of that record, and the thing the build-time evaluator has
no way to be given. §3 of it names *a fixed map type, as the crossed form of a
map*, and says *that is a type in `std` and not a language rule*; nobody has
written it.

**Measured, and the threshold it was waiting on is answered.**
[`fixed-map-lookup.md`](fixed-map-lookup.md) has the crossover D3 left to the
compiler: a `match` wins below sixteen keys — at eight by **2.4×** on a hit and
**4.3×** on a miss — and a perfect hash wins from sixteen up and never gives the
lead back, flat at 12–14 ns where the `match` climbs to 25 at two hundred and
fifty-six. So *how it is looked up is the compiler's* is now a decision the
compiler can make, and this entry is the one thing in front of it.

**What is actually missing.** The build cannot see a map's contents. A program
builds one with `collections::HashMap()` and `insert`, and both are `std`'s,
whose bodies are Rust — so the evaluator has nothing to read. There is **no map
literal** in this language either. A `comptime` map therefore has no shape a
program can write today, whatever the lowering would do with it.

**Why it is the owner's.** Every option below adds surface, and which surface a
language has is not a thing a measurement settles. [ADR-084](specification/adr/adr-084.md)
calls a keyword the most expensive thing a language adds, and the same argument
runs one size down for a literal form.

**The options.**

* **A — a map literal**, `{"get": 1, "post": 2}` or similar. The most direct and
  the most expensive: a new literal form, a place in the grammar, and a question
  about what it means *outside* a `comptime` (an allocated map? the fixed one?).
* **B — a list of pairs, and the type decides.** `comptime ROUTES: Fixed[&str, i64] = [("get", 1), ("post", 2)]`
  adds no literal at all: a list of tuples is a shape the evaluator nearly reads
  already, and the **declared type** is what says it crosses as a map — which is
  the rule [ADR-152](specification/adr/adr-152.md) D4 already uses to make a list
  literal an `Array[T, N]`, and which [ADR-079](specification/adr/adr-079.md)
  D1's crossing is built on. What it needs is a tuple value in the evaluator and
  the `std` type.
* **C — `insert` read as a build-time shape.** A `HashMap()` and a run of
  `insert` calls, recognised by the evaluator. Cheap for a reader to write and
  the worst of the three to justify: it makes two `std` names into language
  forms, which is what [ADR-075](specification/adr/adr-075.md) refused to do
  when it made permission a *ledger column* rather than a list of blessed
  functions.

**What this page recommends: B.** It is the only one that adds no new syntax —
the list literal and the declared type are both built, and the rule that a use
answers a literal is the one this language already makes twice. It also puts the
`std` type where D3 wants it rather than in the grammar. The cost is a tuple
value in the evaluator, which is small and is wanted by the field walk
([ADR-088](specification/adr/adr-088.md) D1) anyway.

**What it costs if wrong**: a literal form added later is additive, so B does not
close A — a map literal could still arrive, and would then have B's crossing
already built under it. The other direction is not true, which is most of the
argument.
