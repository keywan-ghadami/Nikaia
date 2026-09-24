# Open decisions — the questions that need the owner

## The shape this page is for
The
entries here are written to be *answerable* — what is blocked, the options, a
recommendation, and what either direction costs if it is wrong.

An answer is an [ADR](specification/adr/), and
the moment a question is answered its entry leaves this file rather than
staying with a note on it. What is merely **unbuilt** is in
[`open-work.md`](open-work.md). Each entry says what the
question is, why it is the owner's, and what this file recommends.

**What is here is what has been asked**, which is not the same as what is
unsettled: an entry is here because a piece of work was blocked by it and somebody
noticed, so an empty page means nothing is blocked that anyone has written down.
The way to refill it is the paragraph above — the moment a piece of work is
blocked by a question, the question comes here in that shape.

## Open

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
