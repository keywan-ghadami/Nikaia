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

### Does a pausing function-typed parameter still lower to a boxed future, now that `impl AsyncFn` exists?

**What is blocked.** [ADR-122](specification/adr/adr-122.md) D1 — *a parameter
whose type may pause lowers to a closure returning a boxed future, whether the
callee runs it or keeps it*. [ADR-187](specification/adr/adr-187.md) D3 reopens
it, because the reason D1 gave for the shape is false.

**Measured**, in the tree, by `benches/handler` — `cargo run -p handler-bench
--release --bin handler`, best of five, control tying:

| row | ns/call | × |
| :--- | ---: | ---: |
| plain closure (`impl Fn`) — the floor | 0.31 | 1.0 |
| **boxed future** — D1's shape today | 11.99 | 38.3 |
| **async closure** (`impl AsyncFn(A) -> R`) | 1.37 | 4.4 |

The shape chosen *because it did not exist* costs **8.7×** what it does.
`async |x: i32| -> i32 { … }`, `AsyncFn`, `AsyncFnMut`, `AsyncFnOnce` and
`async move |…|` all compile on this repository's toolchain, stable, with no
feature gate.

**Why it is the owner's.** D1 is not only a cost, it is a **coherence** rule:
*one spelling for a run parameter and a kept one, so a reader can tell what a
signature costs by reading it*. `impl AsyncFn` is a **bound** and not a type, so
a *kept* parameter — one the callee stores in a struct — still needs a box or a
type parameter of its own. Taking the cheap shape for the run case splits run
from kept again, which is exactly what D1 joined. That is a trade between what a
signature teaches and 8.7×, and this compiler has no way to pick between those
two.

**The options.**

* **A — split by run-or-kept.** A *run* parameter becomes `impl AsyncFn(A) -> R`
  and a *kept* one keeps the boxed closure. [ADR-102](specification/adr/adr-102.md)
  D3 already infers run-or-kept, so nothing new is derived — what changes is that
  the same written type lowers two ways again.
* **B — leave D1 as it is.** One spelling, 38× on the case that did not need it,
  and the record's own reason corrected to *coherence* rather than *the
  language below cannot*. This is today plus honesty.
* **C — the type says it.** A third word beside `sync` on the parameter's type,
  so the author picks the shape. Costs a word in the surface language for a
  question about a lowering, which is the trade
  [ADR-102](specification/adr/adr-102.md) D2 already made once and may not want
  to make twice.

**What this page recommends: A.** Run-or-kept is already inferred and already
decides things; the box on a *run* parameter is paid by every caller that hands
over a lambda which does not pause, and 38× against a floor is the kind of
number [ADR-009](specification/adr/adr-009.md) D4 exists to act on. D1's
coherence is real but it is about what a **reader** can tell from a signature —
and what a reader is told today is *this costs a box*, which for a run parameter
would simply stop being true.

**What it costs if wrong**: the same written type lowers two ways, so a
signature no longer says what it costs on its own — the exact property D1 was
buying. Against B: 38× on the common case, kept for a sentence that turned out
to be false. Against C: a word in the language for a fact about the machine,
which is the one this tree has been most careful not to add.

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
