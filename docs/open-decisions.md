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

### May a build-time value cross as a **view into what the build allocated**?

**What is blocked.** Three shapes that a `comptime` computes perfectly well and
cannot hand to the program, all three for one reason:

* A **`Vec[T]` inside a crossed value** — a field of a `struct`
  (`examples/config.nika`'s `Section { settings: Vec[Setting] }`) or the payload
  of a variant (`examples/json.nika`'s `Json::Array(Vec[Json])`), which is
  **what both corpus grammars produce**. `NK1167` refuses it, and correctly:
  `Array[Setting, N]` cannot type a field whose `N` differs per value, and a
  declaration is one type.
* A **binary asset**. [ADR-072](specification/adr/adr-072.md)'s `asset("…")`
  hands back a `&str` and refuses a file that is not UTF-8 by name
  (`Denied::NotText`). There is no `asset_bytes`, so a build cannot read a
  `.png`, a lookup table or a serialised index at all.
* **`Bytes`** ([ADR-156](specification/adr/adr-156.md) D1), which is
  reference-counted and owns its run, so it has no `const` form — the same
  sentence `String` gets.

**Measured, at 0.0.125.** The rest of the aggregate crossed in this package:
`Array[u8, N]` is a byte buffer a `const` holds today (`comptime MAGIC:
Array[u8, 4] = [0x7F, 0x45, 0x4C, 0x46]` compiles and runs), a nested array is
`[[T; M]; N]`, a `struct` may hold an `Array[T, N]` field, and an `enum` crosses
by the variant the value is. So what is left is **exactly** the case where the
length is not one number: a field whose list is a different length per value.

**Why it is the owner's.** The answer is a **type**, and this language does not
have one to write. [ADR-079](specification/adr/adr-079.md) D1 says a result
crosses as `&[T]`, and §5 records that what was built is `Array[T, N]` instead,
*because the array is what the type language can already spell*. So the record
already names the missing type and calls the absence somebody else's question.
Whether to close it is a decision about Part I 2.2's surface rather than about
this evaluator.

**The options.**

* **A — spell the view**: `&[T]`, the missing half of D1's view form, with
  `&[u8]` falling out of it and a `Vec[T]` field crossing as `&[T]` the way text
  crosses as `&str`. One type, and it answers all three bullets — `asset_bytes`
  then has a result type to have.
* **B — a `std` type**, `Slice[T]`, wrapping a `&'static [T]`. Cheaper below and
  a **different type from what the program declared**, so `Section.settings`
  would have to be written as `Slice[Setting]` and stop being a `Vec` for the
  code that builds one at run time. That is the objection: a field's type would
  depend on who fills it.
* **C — leave it.** A `Vec` field stays out of a `comptime`, a binary asset
  stays unreadable, and a grammar whose result nests a list is refused by name —
  which is where 0.0.125 leaves it, with `NK1167` saying so rather than `rustc`
  about the generated file.

**What this page recommends: A, and not in this package.** `&[T]` is the type
this language keeps almost-having: the tether work (`open-work.md`'s Tethered
entry) wants it, `asset_bytes` wants it, and a `Vec` field crossing wants it. B
buys the same three at the cost of a type the program did not write, which is
the thing [ADR-107](specification/adr/adr-107.md) D3 refuses one construct over.
But A is a **language surface change** and belongs in a record of its own, with
the borrow rules written down — not folded into a package about what an
evaluator computes.

**What it costs if wrong**: C is free today and gets more expensive the more
grammars are written, because the shape it refuses is the shape a parser's
result naturally has. A costs a type in Part I 2.2 that every later analysis has
to answer for.

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
