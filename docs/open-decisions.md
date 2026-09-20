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

**Nothing is open.** The last question — where an error's `secondary` list can
live when the channel has no envelope — was answered **A** and is
[ADR-170](specification/adr/adr-170.md): a body that joins puts an envelope on
a bare channel, which was always the plan and is now written down.
