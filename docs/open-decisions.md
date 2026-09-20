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

### `Bytes`: `std`'s, the language's, or not yet?

**What is blocked.** [ADR-154](specification/adr/adr-154.md) D1 puts **`Bytes`**
on the prelude's list, beside `Vec` and `String`, as *the containers a program
cannot do without*. §4 of the same record leaves open whether it is `std`'s or a
language type — *it is in the list because a program cannot do without it,
wherever it lives*. The list is now **enforced** (§5), and enforcement is what
turned that door into a question: what needs no `use` is what `std`'s ledger
keys **bare**, so the two answers are no longer interchangeable. One of them
puts `Bytes` in a module and behind a `use`, which is the opposite of being on
the list.

**What is measured, and it is more than the name.**

* **`Bytes` does not exist anywhere** — not in `std`, not in the compiler's
  built-in names, not in any `.nika` file. Part I 1.3 lists it and nothing
  reaches it.
* **It is already specified**, and not as a `Vec[u8]` by another name. Part III
  17.2: *`read` returns `Bytes`, not a `Vec[u8]`: it is one shared buffer, and
  slices that outlive its scope are tethered to it (Part I 6.6)*. And `Mapped`
  derefs to it, *so a mapped file is a tethered buffer like any other, and a
  parser cannot tell the difference*.
* **So `Bytes` is the tether's container**, and the tether is the part of
  Part I 6.6 that is **not built**: the three states a view can be in —
  borrowed, tethered, owned — are
  [ADR-008](specification/adr/adr-008.md)'s, and what exists today is the
  *refusals* (`NK2302`) rather than the reference-counted buffer. `Bytes` is not
  a missing name; it is the surface of an unbuilt mechanism.
* **And the pages had already said two things that are not true**, which is
  what the open door cost: Part III 17.2 said `fs::read` hands back `Bytes`
  where the compiler hands back a `Vec[u8]`, and a **Status** note said
  *`fs::map` and `Bytes` exist*. Both are corrected in the change that files
  this entry, because a stale **Status** note is a defect in its own right
  ([`README.md`](README.md) §1) whatever the answer here turns out to be.

**The options.**

* **A — `std`'s, in a module.** A reference-counted byte buffer is a library
  type; the language learns nothing. Under ADR-154's own rule it is then
  `bytes::Bytes` behind a `use`, and **it leaves D1's list**, because the list
  is exactly what needs no `use`.
* **B — the language's**, like `Vec` and `String`: a name the compiler knows,
  written bare, on the list where D1 already put it. This is what D1's text
  implies, since being on that list and being keyed in a module are now mutually
  exclusive.
* **C — not yet.** Take `Bytes` off the list until the tether is built, and
  leave `Vec[u8]` as what `fs::read` hands back. The question then returns with
  ADR-008's own work, where it can be answered against a mechanism rather than
  against a name.

**What this page recommends: C now, B when the tether is built.**

C, because a name on a prelude's list that does not exist is the one direction a
prelude can be wrong in without anybody noticing — nothing refuses it, because
nothing reaches it — and because D4 is that record's own rule: *a name joins the
list by a record, and never by being needed once.* `Bytes` joined by being
**listed** in a record that did not build it, which is the same failure one step
earlier.

B when the time comes, because `Bytes` is what a tethered buffer **is**: Part I
6.6's states are the compiler's to pick, `Mapped` derefs to it, and a type whose
representation the compiler chooses is not a library type. Putting it behind a
`use` would also make `fs::read`'s result a name a program must import before it
can write down what it already holds.

**What either direction costs if it is wrong.**

* **C wrong** — `Bytes` should have stayed promised: nothing breaks, because
  nothing could use it. What it costs is that a program written between now and
  the tether writes `Vec[u8]` and gets a **second spelling** for one thing when
  `Bytes` arrives. That is the problem
  [ADR-140](specification/adr/adr-140.md) spent a whole record removing, and it
  is the real cost here.
* **B wrong** — `Bytes` should have been `std`'s: a name in the prelude, which
  is very hard to take back. Every name in it is one some program writes without
  importing, and removing one breaks that program.
* **A wrong**: `fs::read`'s result needs a `use` to be written down, and Part III
  17.2's `Mapped` deref names a type the reader has to import before they can
  name what they already hold.

**And a smaller question rides with it**, which the owner may answer in the same
breath: `eprint` is in the compiler and **not** on D1's list, which writes only
`eprintln`. It is keyed bare, so it needs no `use` today. It joined by being
needed, which is the direction D4 was written against. Either the list gains it
in a sentence or the entry leaves `std`; the first costs one more name that is
hard to take back, the second one corpus edit.

## A page with nothing on it
only means no question has been *found* yet, and
the way they are found is by building. The next one belongs here the moment
something comes to rest on it, in the shape this page asks for above: what is
blocked, the options, a recommendation, and what either direction costs if it is
wrong.

---

This file is a notes page: nothing here is normative. A decision taken from it is
written down in [`specification/adr/`](specification/adr).

