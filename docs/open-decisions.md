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

**One question is open**, and the page was empty until 0.0.166. Three questions
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
