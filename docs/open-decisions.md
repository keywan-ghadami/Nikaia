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

### Is `&[T]` the right spelling, or should a run be `&Seq[T]`, an `Array`, or a word?

**What is blocked.** Nothing — [ADR-179](specification/adr/adr-179.md) built
`&[T]` at 0.0.127 and it works. This is the owner asking whether the *spelling*
is the one to keep before more programs are written against it, which is the
cheapest moment to ask.

**Measured**, over the 2 847 lines of Nikaia in this repository:

| Spelling | Uses | What it is |
| :--- | ---: | :--- |
| `&str` | **53** | a **type**, and the single commonest use of `&` in the language |
| `&mut` | 15 | [ADR-147](specification/adr/adr-147.md) D1's half |
| `&Name` (`&Vec`, `&Stats`, `&Response`, `&Json`, `&Entry`, `&Counts`) | 6 | a view of a declared type |
| `&self` | 4 | a method's receiver |
| `&[` | 3 | this question's subject |
| **`&` in expression position** (`&path`, `&out`, `&markup`, `&dna[…]`, …) | **16** | a borrow the source writes |

So **82 of 98** `&`s are a *type* spelling, and **53 of those 82 are `&str`**.

**Why it is the owner's.** It is a question about how the language reads, and
nothing in the compiler prefers one answer.

**The options.**

* **A — keep `&[T]`.** It stands beside `&str`, where the `&` means the same
  thing: *a view of a run that is somewhere else*. `&[u8]` and `&str` are then
  one rule and not two, which is what [ADR-179](specification/adr/adr-179.md) §1
  leaned on when it called `&str` "exactly such a run".
* **B — `&Seq[T]`.** This **collides**: [ADR-105](specification/adr/adr-105.md)
  D1 already gives `Seq[T]` a meaning, and it is a different one. A `Seq` is
  produced step by step and **consumed by walking it** — a second walk is
  refused — while a run is walked as often as a program likes, indexed, and
  sliced again. `xs.map fn …` and `&data[1..3]` would become one spelling for
  two things, and the refusal `a_sequence_is_walked` makes would have to stop
  being about the type.
* **C — reuse `Array`.** An `Array[T, N]` **carries its length in the type**
  ([ADR-152](specification/adr/adr-152.md) D4), and the reason `&[T]` was needed
  at all is a run whose length the type does **not** carry: a field whose run
  differs per value has no `Array` to be, and that is both corpus grammars'
  output. Reusing it means inventing `Array[T, ?]`, which is `&[T]` with more
  syntax and one more thing to explain.
* **D — a word, `Slice[T]` or `Run[T]`.** The honest version of the question:
  Nikaia's type language is otherwise words. But then `&str` is the odd one out
  instead, and renaming *it* is a change to 53 lines and to every page of the
  specification.

**What this page recommends: A.** The measurement is the argument — `&` in this
language is overwhelmingly `&str`, and `&[T]` is `&str` with the element named.
B is the one option that is not merely a preference: it takes a word that
already means *walked once* and gives it to a thing that is walked many times.

**What it costs if wrong**: three lines in the corpus and one paragraph of
[ADR-179](specification/adr/adr-179.md). The cost is low **now** and rises with
every program written, which is why the question is worth answering rather than
leaving.

### Should the view be spelled `ref` instead of `&`?

**What is blocked.** Nothing. The same moment-of-asking as the entry above, and
the two are better answered together: a decision to write `ref [T]` is a
decision about `ref str` first.

**Measured.** The table above is this question's measurement too. `&` occurs 98
times in 2 847 lines, and **54 %** of all of them are the four characters
`&str`. Only **16** are a borrow the source writes — because
[ADR-094](specification/adr/adr-094.md) D1 already made a parameter the body
only reads a view **without the word**, which is the decision that took the
`&` out of the place a reader meets it most. [ADR-179](specification/adr/adr-179.md)
D2 says the same thing from the other side: *a parameter is where the `&` is the
compiler's*.

**Why it is the owner's.** How a language looks is not a thing a compiler has a
view about.

**The options.**

* **A — keep `&`.** One character in the spelling this language writes 53 times,
  and the same character the generated file carries — so
  [ADR-011](specification/adr/adr-011.md) D2's *the generated file says what the
  program said* holds at the lowest level there is, the token.
* **B — `ref`.** `ref str`, `ref [T]`, `ref Response`. It costs a **reserved
  word**, and `ref` is an ordinary English noun a program might want — the same
  objection [ADR-088](specification/adr/adr-088.md) D7 weighed for `macro`,
  `quote` and `with`, and lost there only because those three name constructs
  this language refuses to have. It also makes the *commonest* spelling in the
  language three characters longer, in return for clarity at the 16 places a
  borrow is written by hand.
* **C — `ref` in expressions only**, keeping `&str` as a type. Two spellings for
  one idea, which is the option that reads well in isolation and badly in a
  language: a reader then has to know that `ref x` and `&str` are the same
  claim.

**What this page recommends: A.** Not because `&` is better in the abstract, but
because of where this language already spent the change: D1 removed the `&` from
parameters, so what is left is 53 `&str`s and 16 hand-written borrows. Making
the 53 longer to make the 16 clearer is the wrong side of that ratio. **If the
word is wanted anyway**, C is the shape to refuse and B the shape to take, and
it should be taken **before** `&[T]` spreads rather than after.

**What it costs if wrong**: a reserved word cannot be given back, and every
`.nika` file, every page of the specification and every diagnostic that prints a
type changes together. This is the one question on this page where *later* is
strictly more expensive than *now*.
