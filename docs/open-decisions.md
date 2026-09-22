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

### Does the body's **run-or-kept** answer decide a function-typed parameter's lowering, as the body's answer already decides three other representations?

**What is blocked.** [ADR-122](specification/adr/adr-122.md) D1 — *a parameter
whose type may pause lowers to a closure returning a boxed future, whether the
callee runs it or keeps it*. [ADR-187](specification/adr/adr-187.md) D3
reopened it, because the reason D1 gave for the shape turned out to be false.

**This entry was first written as a backend question** — *does it still lower to
a boxed future now that `impl AsyncFn` exists?* — and that was the wrong
altitude. The choice of Rust shape is downstream. What is actually being decided
is whether **one written type may take two representations, chosen by an
analysis of the callee's body**, and that is a question about this language
rather than about its backend.

**Because the answer elsewhere is already yes, three times over**, which is what
the first draft of this entry missed and what makes D1's *coherence* argument
weaker than it looked:

* **`Shared[T]` is `Rc<T>` or `Arc<T>`, and which one is decided *per value*** —
  the emitter's own words, on [ADR-037](specification/adr/adr-037.md) D7. Not
  per build: two values of one written type in one program get two
  representations, and the analysis picks.
* **[ADR-008](specification/adr/adr-008.md) D2 is titled *solved per
  construction site, not per type***, and D3 gives a struct up to three layouts
  for that reason.
* **[Part I 5.4](specification/10-nikaia-light.md) C says it about this very
  construct**: *the context of such a parameter is **inferred**, not written. A
  parameter the body only calls is immediate and borrows. A parameter the body
  keeps … is detached and moves.*

And a fourth, built and shipped: **[ADR-094](specification/adr/adr-094.md) D1** —
the *callee's* body decides whether the caller's argument gains a `&`, and the
caller writes nothing. *The body decides the caller's lowering* is not a rule
this language would be breaking; it is a rule it already keeps.

**Measured.** `benches/handler`, best of five, control tying:

| row | ns/call | × |
| :--- | ---: | ---: |
| plain closure (`impl Fn`) — the floor | 0.31 | 1.0 |
| **boxed future** — D1's shape today | 11.99 | 38.3 |
| **async closure** (`impl AsyncFn(A) -> R`) | 1.37 | 4.4 |

And what the compiler does today, read off the lowering rather than off the
record: a parameter written `fn() -> String` becomes
`impl Fn() -> Pin<Box<dyn Future<…>>>` and one written `fn() -> String sync`
becomes `impl Fn() -> String`. **So the distinction that exists today is
*declared*, not inferred** — [ADR-102](specification/adr/adr-102.md) D3's
run-or-kept answer feeds the `sync` column and not this shape. This question
asks for it to decide one thing more, not to be believed for the first time.

**Why it is the owner's.** What D1 actually buys is not *truth* — the signature
is accurate today, since the box is always paid — but **predictability**: a
library author who changes a body from *run* to *kept* would, under the change,
silently move every caller's cost. That is action at a distance, which
[ADR-005](specification/adr/adr-005.md) §3 rejected in-source annotations to
avoid.

**And this language already has its answer to that**, which is the piece both
the first draft of this entry and its critique were missing: the **ledger**. A
`keeps` change is a `nikaia.contracts` diff in review
([ADR-094](specification/adr/adr-094.md)), a tether state is one
([ADR-008](specification/adr/adr-008.md) D6 — *the inverse tool is inspection,
not assertion*), and `--locked` fails a build whose contracts moved unrecorded.
The instrument for "the body decided something the caller pays for" exists and
is used twice. Whether it is trusted a third time is the decision.

**The options.**

* **A — the inference decides.** A *run* parameter becomes
  `impl AsyncFn(A) -> R` and a *kept* one keeps the boxed closure, with the
  state in the ledger as `keeps` and the tether state already are.
* **B — leave D1 as it is.** One spelling, 38× on the case that did not need
  it, and the record's reason corrected from *the language below cannot* to
  *predictability*. This is today plus honesty.
* **C — the type says it.** A third word beside `sync`. This is the weakest of
  the three: it puts a word in the surface language for a fact about the
  machine, which is what [ADR-102](specification/adr/adr-102.md) D2 and
  [ADR-008](specification/adr/adr-008.md) D1 both weighed and what
  [Part I 5.4](specification/10-nikaia-light.md) C explicitly refuses —
  *there is no `@detached` to write*.

**What this page recommends: A**, and on the precedents rather than on the
number. 38× is what makes it worth doing; *`Shared[T]` is decided per value* is
what makes it consistent. The ledger is what answers the objection that decided
it the other way in the first place.

**What it costs if wrong**: a reader can no longer tell a run parameter's cost
from its signature alone, and has to read the ledger instead — which is the same
trade `keeps` made, and the same one `--tethers` exists for. Against B: 38× kept
for a property three other constructs do not have. Against C: a keyword for
something the compiler already knows.

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
