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
* **C — the describer derives it.** ~~Out of reach~~ — **the reason this option
  gave was false**, and finding that out is what added D below. It read:
  *`nikaia describe` reads rustdoc JSON, which carries signatures and not
  bodies*. It does not. [ADR-104](specification/adr/adr-104.md) D4 makes
  rustdoc-JSON the **future** better half and says so outright — *a stable-only
  toolchain ([ADR-001](specification/adr/adr-001.md) D1) is not given up for it,
  so the source parser is what runs today*. `nikaia describe` reads the crate's
  own `.rs` **text**. What is out of reach is narrower and different: it is a
  **signature scraper and not a Rust parser**, by its own module header.
* **D — A, and the describer *proposes*.** The column stays hand-written and the
  tool stops making the author hunt for the answer. Where the scraper sees
  evidence, it writes it as a reviewable note beside the absent column —
  *`spawn_worker`: the parameter `f` is bound `Send + 'static`; does this put it
  on a thread?* — and the person writes `threads`, or does not.

**Why D is the shape rather than a fourth answer.** It is
[ADR-123](specification/adr/adr-123.md) D2's own pattern one column over, and
that one is **built**: `nikaia describe` already writes `crosses = false` for a
type whose fields hold an `Rc` or a raw pointer, `true` where every field is
sendable, and **nothing** where it cannot tell.

**But the license for D2 is soundness, not a good hit rate**, and that is the
line D has to stay on the right side of. A field holding an `Rc` *entails* not
sendable — it is a structural read, not a guess. The indicators here do not
entail:

* **`F: FnOnce() + Send + 'static` on a parameter** is a very good sign and not a
  fact: the bound says the callee *may* send it, which is usually `spawn` and is
  sometimes an API keeping a door open. Written as a **claim** it can refuse a
  correct program ([C.4](specification/30-nikaia-tooling.md)); written as a
  **note** it is the most actionable sentence the tool could produce.
* **`std::thread::spawn`, `tokio::spawn`, `rayon::spawn`, `Sender::send` in the
  body** are the same kind of sign one layer deeper, and they cost more to see:
  the text is in the file the scraper already opens, but knowing *which
  function's body* a line sits in is brace tracking — more than a signature
  scraper does and less than a Rust parser. A step, not a rewrite.
* **A leaf function is the one indicator that must not be built as proposed.**
  *Pure computation, no external calls* would be the ground for `threads =
  false`, and that is the dangerous direction: a false silence, which
  [ADR-010](specification/adr/adr-010.md) D1 calls a vulnerability generator.
  Worse, a signature scraper cannot establish leafness at all — it would need
  the body **and** every callee. **Nothing a signature can show entails
  `false`**, because a function may spawn something it built itself.

**So the asymmetry is the design.** The describer may propose `true`, must
propose `false` never, and stays silent wherever it cannot see — which is the
same *absence is nobody said* the column already rests on.

**What this page recommends: A with D, and the third value.** The precedent is exact —
`crosses = false` is already a hand-written claim about something a signature
cannot show — and the description is reviewed like code
([ADR-104](specification/adr/adr-104.md)). But it must have **three** values and
not two, for the reason `crosses` has three: the absence of the word is *nobody
said*, never *it does not*. A `threads = false` written by a hopeful hand is a
false **silence**, which is the polarity
[ADR-010](specification/adr/adr-010.md) D1 calls a vulnerability generator, and
the refusal must fire on the claim rather than on its absence.

**And D is what answers the cost of A**, rather than a second thing to build:
the objection to A is that it is *one more line a describer has to get right, on
a surface where getting it wrong is silent*. A note that says **where to look**
does not make the claim safer to get wrong — it makes it less likely to be
skipped, which is the failure mode a hand-written column actually has. The
describer proposes; the person disposes; `NK2502` fires on the claim.

**What it costs if wrong**: A's cost is the line above, unchanged. D's own cost
is a note that cries wolf — a `Send + 'static` bound on an API that never
spawns, read by a reader who then stops reading the notes. That is a reason to
keep the note **specific** (name the parameter and the bound, never *this might
thread*) and to write none at all where the evidence is weaker than a bound; it
is not a reason to skip D, because the alternative is that nobody looks.
