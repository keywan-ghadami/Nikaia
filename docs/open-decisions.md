# Open decisions — the questions that need the owner

Five questions that work cannot settle. Each one says what is blocked, what the
options are, **what I would do**, and what either direction costs — because a
question without a recommendation is work handed back rather than a decision
asked for.

Everything that is merely unbuilt is in [`open-work.md`](open-work.md). An item
moves from here to there the moment it is answered, and the answer becomes an ADR
if it changes what a program means.

This file is a notes page: nothing here is normative. A decision taken from it is
written down in [`specification/adr/`](specification/adr).

---

## 1. Does `use pool` bring the names in, or does everything stay qualified?

**Blocked by it:** the shape of the fix for [`open-work.md`](open-work.md) §1.1 —
a module can hand out functions and not types today.

Part I 9.1 says a module is reached through its name, and the call side already
works that way: `pool::make()`. What is missing is the same for a type — a
qualified name that resolves to the same type, and a struct literal that tolerates
a prefix, `pool::Conn(id: 1)`.

The question is whether `use pool` does anything beyond making the module
reachable. Two answers:

* **Qualified only.** `use pool` makes the module visible and nothing else; every
  name from it is written `pool::…` at every use.
* **Qualified plus import.** `use pool` also brings the public names in, so
  `Conn(id: 1)` works, perhaps with a `use pool::{Conn}` form for choosing.

**I would keep it qualified only**, and add the two missing pieces without an
import form. The reason is the one thing qualification buys: in a file that
imports names, a reader has to know every import to know what a name means, and a
second import can change the meaning of a line that did not change. That is the
cost this language avoids in return for four characters. `f"{c.id}"` is short
anyway because the prefix is on the *type*, not on every use of the value.

**What it costs either way:** nothing in the compiler differs much — the
resolution change is the same either way, and an import form is a second lookup
table. The cost is on the reader, which is why it is a decision and not a
preference.

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
