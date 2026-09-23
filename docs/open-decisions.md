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

**Three questions are open.** Two arrived at 0.0.174 by reading
[`open-work.md`](open-work.md) against its own rule — *citing a question is not
asking it* — which is a failure that file has now had **three** times, and the
third is the entry that named this page while writing nothing on it. The page was
empty until 0.0.166. Three questions
were answered in one day and each left this file for its record, which is what
this page says happens to an answered entry: *does a described foreign function say whether it puts its argument on a
thread?* → [ADR-193](specification/adr/adr-193.md), and *what does a Nikaia
program have to write to be a microservice?* →
[ADR-194](specification/adr/adr-194.md), and *is `nikaia describe` written in
Nikaia?* → [ADR-195](specification/adr/adr-195.md). The work they created is
[`open-work.md`](open-work.md)'s.

**And the last of them corrected how this page had framed it.** The entry
recommended writing the grammar *first, as a measurement that would settle the
question* — and [ADR-009](specification/adr/adr-009.md) D4's *measure before
choosing a shape* does not reach a choice that rests on a project principle.
[ADR-195](specification/adr/adr-195.md) D5 is the rule that came out of it:
**a measurement decides between options, and where the choice is already made a
number is a diagnosis.** A page whose whole job is to be answerable can make
that mistake, and this is the shape of it.

**The one that is open is the one this page held back on purpose.** It was
deferred with a note — *the owner has asked to be asked again, with a fuller
write-up, when the work reaches it* — and the work has reached it: the server is
built at 0.0.166, so there is something for a refusal to be about.

### May a route be refused by what its handler `touches`?

**What is blocked.** Nothing, yet — which is why this is a question and not an
entry in [`open-work.md`](open-work.md). What it decides is whether the next
thing built on top of the server is a *check* or another feature.

**What makes it askable at all.** Every function in this language carries a
`touches` column, inferred from its body and written to the ledger: `[]` for one
that only computes, `["file(path) read"]`, `["socket write"]`, `["lock write"]`,
`["stdout write"]` and so on. So at the moment a program writes a handler, the
compiler already knows whether that handler reaches the filesystem, and it knows
it **transitively** — the column is the closure over what the body calls. No
other language's `http.server` has that, because no other one has the column.

**The shape a refusal would take.** Some annotation at the route saying what the
handler may reach, and `NK`-something where the ledger says it reaches more:

```nika
// Not syntax that exists. This is the question.
http::listen(at; reaching: []) fn(request) { … }      // pure handlers only
```

**Three options.**

1. **No.** The column stays a fact a reader may look up and `--trust` may list,
   and the server refuses nothing on it. A handler that reads a file is a handler
   that reads a file.
2. **Declared, and refused where it exceeds what was declared.** A route names
   what it may reach and the compiler holds it to it. This is the shape
   [ADR-010](specification/adr/adr-010.md) D1's polarity asks for — the dangerous
   thing is visible at the declaration — and it is the one that could be wrong in
   an expensive way: a check nobody can satisfy is worked around, and the
   workaround is `reaching: everything` written at every route.
3. **Listed and not refused.** `nikaia --trust` grows a section: *these routes
   are reachable from the network and this is what each of them touches*. The
   shape [ADR-108](specification/adr/adr-108.md) already set for a way around a
   check, applied to a thing that is not a check.

**What this page recommends: 3, and not 2.**

Two reasons, and the second is the one that matters.

*The first is that 2 needs a granularity nobody has.* `touches` today says
`file(path) read` — the *path* is a variable's name, not a value — so a
declaration could say *this handler may read files* and could not say *this
handler may read files under `/srv/www`*. The useful version of 2 is the second
one, and it needs [ADR-108](specification/adr/adr-108.md)'s root-at-the-call to
be built first. Declaring the coarse version would put a word in the language
that is answered by a question the language cannot yet ask.

*And the half this rested on is built at 0.0.178.* `fs::Root` exists, so a
handler that reads a file names the directory it may not leave, and
`nikaia --trust` lists every site that names none. **That is option 3 arriving
for the filesystem before this question is answered for a route** — which is the
recommendation below reaching the same place from the `fs` side, and it is worth
reading against the question rather than as an answer to it: what `--trust`
lists is every `Anywhere` in the program, and what this asks about is a *route*.
The `touches` granularity is unchanged, so the first reason above stands as it
was written.

*The second is that a refusal here has no failure it prevents that the ledger
does not already show.* A handler that touches the filesystem is not a bug; it
is most handlers. What is a bug is a handler touching the filesystem **when
nobody realised it did**, and that is a review problem, which is what a listing
is for. A check earns its keep by refusing programs that would otherwise be
wrong, and this one would mostly refuse programs that are right — which is [Part
III C.4](specification/30-nikaia-tooling.md)'s cost paid for a property nobody
was surprised by.

**What either direction costs if it is wrong.** Choosing 3 and being wrong is
cheap: the listing is the data a later refusal would read, so 2 remains
available and nothing has to be undone. Choosing 2 and being wrong is expensive
in the one way this project cares about — a keyword
([ADR-084](specification/adr/adr-084.md): a keyword is the most expensive thing a
language adds), written at every route, answering a question its own granularity
cannot make precise.

*The first part of this page's statement — that nothing else is open — is a
statement about what has been **asked**, not about what is settled.*
Every entry this page has held was put here because something was blocked by it
and somebody noticed; an empty page means nothing is blocked that anyone has
written down. The way to refill it is the head of this file: the moment a piece
of work is blocked by a question, the question comes here in the shape above.

### How does a bound reach a caller across a package boundary?

**What is blocked.** A call into a **package's** generic function is not checked
against that function's bound, and what a reader gets is `rustc`'s words on their
own line ([`open-work.md`](open-work.md) §1.10):

```text
error: app/src/main.nika:6:5: the trait bound `Bare: Handler` is not satisfied
     = the trait `Handler` is not implemented for `Bare`
     = the trait `Handler` is implemented for `Static`
```

`Handler` is written without the path the program must write, and `Static` is a
name the program never mentions. The position is right — that is
[ADR-005](specification/adr/adr-005.md) D7's translation — and the words are the
backend's, which is [Part III C.1](specification/30-nikaia-tooling.md).

**Why it is the owner's and not work.** The check itself is four lines:
`Checker::declared_bounds` already holds *which bound each parameter carries*,
under the key a call resolves to, and the only thing missing is that key for a
function in another package. It cannot be read from the ledger, because **the
ledger has no column for a bound** — a signature writes `(h: $H) -> String` and
the `: Handler` is nowhere in it. Adding one is
[ADR-106](specification/adr/adr-106.md) D3's table of ledger entries, extended,
and that is a format decision: every reader of a `.contracts` file parses it, and
`--locked` compares it byte for byte.

**Nothing is blocked today.** Nothing in the tree publishes a generic function
with a bound; the shape became writable at 0.0.171 and the first package to use
it is the one that meets this.

**Three options.**

1. **A key of its own.** `bounds = ["H: handler::Handler"]` beside `signature`.
   Additive: an older reader ignores a key it does not know, which is what
   `version` is for.
2. **Widen the `signature` language.** `signature = "[H: handler::Handler](h: $H) -> String"`,
   the bound where the declaration writes it. No new key, and the one string a
   caller already parses says the whole of what the call means.
3. **Neither.** A cross-package generic call stays the backend's to refuse, with
   the position translated and the words Rust's.

**What this page recommends: 2.**

The signature **already** carries the type parameter as `$H`
([ADR-074](specification/adr/adr-074.md) D2: *a generic parameter is recorded as
a variable, so a caller binds it from what it passes and reads the result off the
same signature*). A bound is the rest of that sentence, and a second key that has
to agree with the first is a second source of truth for one fact — the argument
[ADR-106](specification/adr/adr-106.md) D3 already makes about a trait's methods,
which are the `fn` entries beside it rather than a list inside the `trait` table.

**What either direction costs if it is wrong.** Option 2 changes a grammar every
ledger reader parses, and every signature in every `.contracts` file is compared
byte for byte by `--locked` — so getting the spelling wrong is a churn across the
tree rather than a bug. Option 1 costs nothing to add and one more place for the
two columns to disagree, forever. Option 3 costs a [Part III
C.1](specification/30-nikaia-tooling.md) hole that stays open, and it is the only
one of the three that cannot be undone cheaply: a package published under it
would have callers relying on `rustc` to say what the compiler should have.

### Does the language have a type for a list of errors?

**What is blocked.** [ADR-115](specification/adr/adr-115.md) D4's own written
example ([`open-work.md`](open-work.md)'s `overlap` entry):

```nika
} catch {
    throw LoadFailed(error, error.secondary)
}
```

The list of joined failures is what a log and an operator see today
([ADR-170](specification/adr/adr-170.md) D1, built). Handing it to a
**constructor** — a program reading it as a value — needs a Nikaia type for *a
list of errors*, and there is none. In Rust it is a `Vec<Thrown<E>>`; in this
language a trait is never the type of a value and there is no `dyn`
([ADR-078](specification/adr/adr-078.md) §4, by decision).

**Three options.**

1. **An opaque `Failures` in `std`**, with what a program actually needs of it —
   a count, a walk, a rendering — and no element type at all. The precedent is
   [ADR-147](specification/adr/adr-147.md) D3's handle: an address the language
   never dereferences, and `NK1160` where a program tries.
2. **`dyn` as a type**, which would give `Vec[Error]` a meaning and is a language
   feature of its own — the one [ADR-078](specification/adr/adr-078.md) §4
   deliberately left out.
3. **Neither.** `error.secondary` stays a thing a log prints and D4's example
   stays unwritable, said out loud rather than left as an unbuilt line.

**What this page recommends: 1.**

It is the smallest thing that makes the record's example writable, it needs no
language feature, and what a program does with a list of failures is count them,
walk them and print them — none of which wants an element type. Option 2 is a
large decision to take for one example, and taking it here would decide `dyn` by
the back door.

**What either direction costs if it is wrong.** Option 1 costs a `std` type that
may turn out to want an element type later, which is the cheap direction: an
opaque type can gain a way to look inside and cannot lose one. Option 2 costs
`dyn` — the trait-as-a-type question, decided under the pressure of an example
rather than on its own merits. Option 3 costs the record staying half-written,
which is the thing [`docs/README.md`](README.md) §1 calls a defect: a reader
cannot tell a plan from a promise.
