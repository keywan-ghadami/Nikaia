# Open decisions — the questions that need the owner

Six entries. One is answered and kept here until its record exists;
the rest are questions that work cannot settle. Each one says what is blocked, what the
options are, **what I would do**, and what either direction costs — because a
question without a recommendation is work handed back rather than a decision
asked for.

Everything that is merely unbuilt is in [`open-work.md`](open-work.md). An item
moves from here to there the moment it is answered, and the answer becomes an ADR
if it changes what a program means.

This file is a notes page: nothing here is normative. A decision taken from it is
written down in [`specification/adr/`](specification/adr).

---

## 1. What `use pool` does — **answered**

**The answer.** `use pool` makes a module reachable and does nothing else. Every
name from it is written with its prefix, at every use:

```nika
use pool

fn handle(c: pool::Conn) -> i64 {
    let d: pool::Conn = pool::make()
    return c.id + d.id
}
```

Four forms, and the answer for each:

* **`use pool`** — takes the module's file name. Kept, and it is the whole of it.
* **`use pool as p`** — an alias, so `p::Conn` reads at every use site. **Added.**
  It is what makes the decision bearable for a module called
  `request_handling`, and the property that matters survives it: a prefix still
  stands at every use, declared once and visibly at the top.
* **`use pool::*`** — everything the module makes public, without naming it.
  **Refused, and not left open.** A name added to `pool` later changes what an
  unchanged line in another file means, which is the one thing a reader cannot
  defend against.
* **`use pool::{Conn, Pool}`**, with `as` on a single name — **not decided, and
  deliberately not refused.** See below.

**Why the selected form stays open rather than being settled with the rest.** The
argument against importing is about the **blanket** form and does not reach the
selected one: a braced list says by name what comes in, a name added to the module
later changes nothing, and two lists that name the same thing are a compile error
rather than a silent change of meaning. It is a real candidate, and the reason not
to take it today is a different one — **direction**. Adding it later breaks no
program, because everything written with a prefix stays valid. Removing it later
breaks every program that used it. Between a choice that can be reversed and one
that cannot, and with no evidence yet from a program of any size, the reversible
one is taken first.

**What this does not answer, and it is the larger question.** There is no
**re-export**: no way for a module to offer a name that another module declares.
Today that is invisible, because a program is one package and its author knows
their own file layout. It stops being invisible the day a library is published:
without it, a library's *internal file layout is its public surface*, and moving a
declaration from one file to another — housekeeping — breaks every consumer. That
belongs with §6 and is recorded there.

**What it costs, stated rather than implied.** Every use of a foreign name is
longer by a prefix, and the prefix lands in the places this language already asks
for a type: a signature, and the annotated `let` that is the only place sharing
begins — `let db: Shared[pool::Connection] = …`. What is bought is that a reader
of any line knows where every name in it comes from without consulting the top of
the file, and that no edit elsewhere can change what a line already written means.

**And it does not block the repair.** [`open-work.md`](open-work.md) §1.1 needs the
same two pieces under either answer — a qualified type name that resolves to the
same type, and a struct literal that tolerates a prefix. Only the import form
depended on this, and it is not being built. The repair can proceed; the alias is
the one addition this answer brings with it.

---

## 2. Which numeric types does the language offer, and what does `len()` hand back?

**Blocked by it:** a specification gap that a program can already fall into, and
the only place a machine-dependent type reaches user code.

What is true today, measured:

* Part I 2.2 offers `i32`, `i64`, `f64` — and `usize` is writable anyway:
  `let n: usize = t.len()` is accepted.
* `let m: i64 = t.len()` is refused, `NK1103`, with the help *"write `as i64` —
  Nikaia converts where you say so, never quietly"*.
* Since [ADR-043](specification/adr/adr-043.md) D4 that `as i64` is a **checked**
  conversion, because a count is as wide as the machine is. So the commonest line
  a beginner writes now carries a conversion that can abort.

Three answers:

* **(a) A count is an `i64`.** `len` and its neighbours hand back `i64`, `usize`
  leaves the language surface, and the machine-width rule stays in the compiler as
  a backstop that no program reaches.
* **(b) `usize` becomes a written type.** Part I 2.2 names it, explains that its
  width is the machine's, and it gets its own `truncating_` names.
* **(c) Leave it.** Unlisted in the specification, writable in practice.

**I would take (a).** A count is a number, the language offers two integer types,
and a length should be one of them rather than a third type the specification does
not name. It removes a machine-dependent type from every program's surface, and it
removes a conversion from the line `for i in 0..list.len()` is written in.

**What it costs, and this is the part worth seeing before deciding:** Rust indexes
with a `usize`, so an `i64` index has to be converted back at every `v[i]` — and
that conversion can fail, for a negative number. That is an abort the program did
not write. It is the honest abort for the case (a negative index is an index out of
bounds, which Part III A.2 already aborts on), but it means (a) buys readability at
the front door and pays for it at the index. (b) pays in the opposite place: every
program that touches a length carries a type the specification has to teach. (c)
keeps a gap that a user finds before the specification mentions it, which is the
one option I would not take.

---

## 3. Is `fn { … }`'s automatic `a`, `b`, `c` withdrawn, and when?

**Blocked by it:** nothing, which is exactly why it should be answered now rather
than when something is.

[ADR-041](specification/adr/adr-041.md) made named lambda arguments the normal
form and carried the automatic naming as experimental, with `NK1114` naming the
mechanical rewrite where a body reaches for one of the three letters. That record
says explicitly that whether the form is eventually withdrawn **is not decided**.

**I would withdraw it in the next release** and say so in Part I 5.3 now, so that
no new code is written against it. The argument is the one ADR-041 already made:
how many arguments such a lambda takes is read off *which of the three names its
body mentions*, so a local called `a` is not a local but an argument. The warning
makes that visible; it does not make it good. Every site in `examples/` is already
rewritten, so the cost of withdrawal is a deprecation window and nothing else.

**What the other direction costs:** carrying two spellings indefinitely means the
rule above stays true indefinitely, and any future change to the automatic rule
breaks whatever came to rely on it.

---

## 4. What is built next?

**Blocked by it:** my next piece of work.

Three candidates, each already specified:

* **`spawn` and the runtime binding** (Part II 11.2). It unblocks the most: three
  decisions are currently *checked but unrunnable* —
  [ADR-040](specification/adr/adr-040.md) D1's task half,
  [ADR-045](specification/adr/adr-045.md) D2, and Part II 12.2's counter.
* **`SharedMut[T]` and `Locked[T]` as real types** with the four doors
  ([ADR-039](specification/adr/adr-039.md) D10). Large, and it needs a lock
  representation per setting plus the lock-touching property.
* **Part I 2.3's nullable types.** Self-contained, small, and the one item above
  that a beginner meets on the first page they read.

**I would build `spawn` next.** A decision that is checked and cannot run is the
state that rots fastest: the check has no program to be tested against, so it is
the kind of correctness that quietly stops being true. `spawn` turns three such
decisions into running code at once, and `SharedMut` needs it anyway — its whole
point is a second task.

**What the other order costs:** nullable types first is the cheapest real
improvement for somebody learning the language, and if the next weeks are about
the language's front page rather than its concurrency, that is the better answer.
`SharedMut` first costs the most and buys the least on its own, because without
`spawn` there is no second place to hold a lock from.

---

## 5. When does `ordering` get measured, and does it stay on by default?

**Blocked by it:** whether the execution model that everything else is built on is
still provisional at v1.0.

[ADR-033](specification/adr/adr-033.md) changed what a program *means*: two
operations touching disjoint resources have no order between them, and the roadmap
calls the decision **provisional and unmeasured**. `--ordering strict` turns it
off. Two increments are built and tested; what has never been produced is a number
saying what the reordering buys on a real program.

**I would measure it on the flagship before v1.0 and keep it on by default.**
Keeping it on is the whole decision — a default that has to be asked for is not a
language rule — and measuring it is the thing that makes "provisional" a stage
rather than a permanent label. If the number turns out to be small, that is worth
knowing about a feature that changes what a program means.

**What the other direction costs:** shipping it unmeasured means the argument for
it stays an argument. Turning it off by default would be cheaper to defend and
would make every program slower than the model promises, which is the worst of the
three.

---

## 6. How is a Nikaia library distributed, and what consumes one?

**Blocked by it:** whether the rule that shaped
[ADR-045](specification/adr/adr-045.md)'s crossing verdict guards a case this
toolchain can produce.

Part III 13.2's manifest shows a Nikaia package next to a Rust one and refuses the
first with its reason: no record names a registry, a name space or a distribution
format, so the compiler does not guess ([ADR-002](specification/adr/adr-002.md) D1
§5). `type = "rust"` dependencies resolve through Cargo; `std` arrives by path
into a sysroot. Between them there is no way for one Nikaia package to depend on
another.

**Why this is not only a missing convenience.** The reason a crossing verdict may
not consult `user_parallelism` is that *a library built at one setting has to stay
usable at the other*. That sentence decided the shape of ADR-045 D1 — the verdict
takes the destination rather than the switch, so that both answers stay
switch-independent. It is a correct rule and it should stay. But it currently
protects a situation nothing can construct: there are no Nikaia libraries, because
there is no way to depend on one.

**And a second half that §1 handed over.** There is no **re-export**: no way for a
module to offer a name that another module declares. A library is many files, and
without it the consumer must name the file a declaration happens to sit in — so the
library's *internal layout is its public surface*, and moving a declaration between
files breaks every consumer. A library that cannot curate what it offers has no
stable surface to offer. Whatever answer (a), (b) or (c) gets, this comes with it:
it is not an ergonomic form like §1's braced import, it is the difference between a
library having a front door and not having one.

Three answers:

* **(a) A path dependency first.** A Nikaia package may be depended on by path,
  the way a Rust one already can be. No registry, no name space, no distribution
  format — the three things ADR-002 D1 §5 refuses to guess are all still open.
* **(b) The whole question at once** — resolution, versions, a place packages come
  from.
* **(c) Leave it.** A program is one package, and the portability rule stays a
  precaution.

**I would take (a).** It is the smallest change that makes a library a thing that
exists, it needs none of the three decisions ADR-002 declines to make, and it is
the only one of the three that turns the portability rule from a precaution into
something a test can exercise. A rule nothing can reach is a rule that quietly
stops being true, which is the same argument §4 makes for building `spawn`.

**What it costs, and it is worth seeing before deciding:** a second package in one
build is a second analysis boundary, so
[`open-work.md`](open-work.md) §1.6 stops being about files and becomes about
packages — where a field's count is agreed is then a question across a boundary
nobody can see across, rather than across two files of one program. Taking (a)
before §1.6 is answered multiplies it. (b) costs the most and would be decided
without a single Nikaia library existing to learn from. (c) is defensible for as
long as the language has one user, and stops being defensible the day it has two.
