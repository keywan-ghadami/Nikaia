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

**Nothing is open.** Three questions were answered in one day and both left this
file for their record, which is what this page says happens to an answered
entry: *does a described foreign function say whether it puts its argument on a
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

**One question is deferred rather than answered**, and it is written here so it
is not mistaken for settled: **may a route be refused by what its handler
`touches`?** [ADR-194](specification/adr/adr-194.md) §4 carries it, and the
owner has asked to be asked again — with a fuller write-up than the one sentence
it has — **when the work reaches it**. It is not in the shape this page asks for
yet, and putting it in that shape before there is a server to refuse anything is
the kind of premature question this page is worse for holding.

*That is a statement about what has been **asked**, not about what is settled.*
Every entry this page has held was put here because something was blocked by it
and somebody noticed; an empty page means nothing is blocked that anyone has
written down. The way to refill it is the head of this file: the moment a piece
of work is blocked by a question, the question comes here in the shape above.
