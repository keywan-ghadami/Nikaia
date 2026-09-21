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

### May a `Fixed` table hold a `struct`?

**What is blocked.** `comptime PAIRS: Fixed[&str, Row] = [("x", Row { a: 1, b:
2 }), …]` is `NK1127`. Every part of it crosses on its own — a `Row` is a
`const`, a table of `&str` keys is a `const` — and the combination is not.

**Measured.** [ADR-176](specification/adr/adr-176.md)'s `Fixed<V>::get` hands
back `Option<V> where V: Copy`, which is 0.0.118's own correction: returning
`Option<&V>` made `??` over a table of text ambiguous between two `Or` impls and
`rustc` spoke about the generated file. A Nikaia `struct` derives `Clone` and
`Debug` and **not** `Copy`, so a struct value in a table would be `rustc` about
the generated file again — which is why the checker refuses it before it gets
there, rather than the refusal being a judgement about tables.

**Why it is the owner's.** The answer is what `Copy` means in this language.
Nothing in Part I says a struct is copied, and [ADR-008](specification/adr/adr-008.md)
D5 says a copy is never something a compiler inserts on its own — so deriving
`Copy` where every field is `Copy` would make the compiler's answer to *may this
be copied* depend on a field nobody looked at.

**The options.**

* **A — derive `Copy` where every field allows it**, and let a table hold such a
  struct. The rule is mechanical and the emitter can see it; what it changes is
  that adding a `String` field to a struct silently removes it from every
  `Fixed` that held it, one file away.
* **B — a second read.** `Fixed::get` keeps `V: Copy` and gains a sibling that
  hands back a view, with the checker picking by what `V` is. Two spellings
  below and one in the source.
* **C — leave it**, with a sentence that says *a table's value is a number, a
  `bool` or text* rather than `NK1127`'s *cannot evaluate*, which is the wrong
  claim: the value evaluated fine.

**What this page recommends: C now, B later.** C is one message and closes a
diagnostic that is currently untrue — [Part III C.2](specification/30-nikaia-tooling.md)
asks for the reason and `NK1127` gives the wrong one. B is the right shape when
somebody wants it, because it leaves `Copy` alone; A makes a struct's
copyability a consequence of its fields, which is the inference
[ADR-107](specification/adr/adr-107.md) D3 is written against.

**What it costs if wrong**: C costs nothing and buys a true sentence. A is the
one that is hard to undo — once a struct is `Copy`, every program that relied on
it being copied keeps a `Fixed` alive across a field change nobody meant.
