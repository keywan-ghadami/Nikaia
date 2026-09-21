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

### Is a view spelled `ref X`, with `Array[T]` the run and `str` gone?

**What is blocked.** Nothing is broken — [ADR-179](specification/adr/adr-179.md)
built `&[T]` at 0.0.127 and it works. This is the owner asking whether the
*spelling* is the one to keep before more programs are written against it, which
is the cheapest moment to ask and the one question on this page where **later is
strictly more expensive than now**.

**The proposal, which is one and not two.** `ref X` is a view of an `X`, and an
`Array[T]` with no `N` is the run itself:

| today | proposed | what it is |
| :--- | :--- | :--- |
| `String` | `String` | owned text |
| **`&str`** | **`ref String`** | a view of text |
| `Array[T, N]` | `Array[T, N]` | owned, length in the type, laid out inline |
| **`&[T]`** | **`ref Array[T]`** | a view of a run |
| `Vec[T]` | `Vec[T]` | owned, growable |

**The strongest argument is not the keyword — it is that `str` disappears.**
Today `&str` is a view of a `String` spelled with a *different noun*, and `str`
is not a type a program can otherwise write.
[ADR-107](specification/adr/adr-107.md) had to explain that away in prose: text
is one type, `String`, and `&str` is "the assertion that it is a view". Under
`ref String` the assertion is the word `ref` and the type stays the type it was.
One rule replaces two nouns and a punctuation mark.

**And `Array[T]` needs no `?`.** The earlier draft of this entry said reusing
`Array` would mean inventing `Array[T, ?]`; that was wrong. Dropping the `N` is
the spelling, and what makes it hold together is that it composes with `ref` —
`Array[T]` is the run, `ref Array[T]` is a view of one, exactly as `[T]` and
`&[T]` are in the language below.

**The one rule that has to come with it.** A run whose length the type does not
carry **cannot be owned inline** — it has no size, in this machine model or any
other — so `Array[T]` exists only under `ref`. `let xs: Array[i64] = …` must be
a **refusal**, with a way out that names both alternatives: `ref Array[i64]` to
view one, `Vec[i64]` to own one. Without that rule the change trades one
[Part III C.1](specification/30-nikaia-tooling.md) hole for another, because
`[T]` in the generated file is *the size for values of type `[i64]` cannot be
known at compilation time* — the sentence
[ADR-182](specification/adr/adr-182.md) D2 has just finished removing.

**Measured — what the change costs**, by counting every site:

| Where | Sites | Notes |
| :--- | ---: | :--- |
| `.nika` corpus | 53 `&str`, 3 `&[`, 15 `&mut`, 16 expression `&` | mechanical |
| The three specification pages | 43 | mechanical |
| Rust source — diagnostics and the tests that assert on a printed type | 120 | mechanical, and the tests are what catch a miss |
| `.contracts` on disk | 21, of which **18** are in the hand-maintained `std.contracts` | 10 files |
| The word `ref` used as a name today | **0** | the reserved word costs nothing now |

Plus the parser, the type printer, the emitter's view spelling, and the new
refusal for a bare `Array[T]`. **The ADRs are not rewritten**: a record says what
was decided when it was decided, so only the living pages move — which is what
keeps the documentation cost 45 and not 270.

**What this page recommends: take it, with the `Array[T]`-only-under-`ref` rule
attached.** The earlier recommendation here was *keep `&`*, argued from how often
`&` is written. That argument was about **cost** and the owner's is about
**coherence**, and coherence is the right axis for a spelling: `&str` is 54 % of
all `&`s in the corpus precisely because it is the one a reader meets most, and
it is the one that teaches the wrong thing — that text has a second type.

**Two things to settle before the work starts, not after.**

* **`&self`.** `fn mean(ref self)` reads badly. But
  [ADR-094](specification/adr/adr-094.md) D1 already makes a parameter the body
  only reads a view **without the word**, so `fn mean(self)` may already say it
  and `&self` may be a spelling this language can simply drop. That is a
  measurement over the corpus's 4 sites and not a guess.
* **`ref` carries baggage.** In Rust it is a *binding mode* in patterns, which
  means close to the opposite thing. Nikaia has no `ref` in patterns so nothing
  clashes inside the language, but a reader arriving from Rust may misread it —
  the same objection `Seq` would have had, named here so the answer is on the
  record rather than discovered later.

**What it costs if wrong**: a reserved word cannot be given back, and every
`.nika` file, every living page and every diagnostic that prints a type change
together. Done now that is one package; done after the language has users it is
a migration.
