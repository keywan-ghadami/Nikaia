# Open decisions — the questions that need the owner

Six entries. Three are answered, and their records now exist —
[ADR-046](specification/adr/adr-046.md), [ADR-047](specification/adr/adr-047.md)
and [ADR-048](specification/adr/adr-048.md); §2 and §6's language half are built,
and each entry says what is left. They stay here until the owner drops them. The
rest are questions that work cannot settle. Each one says what is blocked, what the
options are, **what I would do**, and what either direction costs — because a
question without a recommendation is work handed back rather than a decision
asked for.

Everything that is merely unbuilt is in [`open-work.md`](open-work.md). An item
moves from here to there the moment it is answered, and the answer becomes an ADR
if it changes what a program means.

This file is a notes page: nothing here is normative. A decision taken from it is
written down in [`specification/adr/`](specification/adr).

---

## 1. What `use pool` does — **answered** ([ADR-046](specification/adr/adr-046.md); the qualified form is built, the rest waits on a package that can be depended on)

**The answer.** `use pool` makes a module reachable and does nothing else. Every
name from it is written with its prefix, at every use. `use pool as p` shortens
that prefix. Nothing is brought in — not by a glob, not by a braced list, not one
name at a time.

```nika
use pool
use request_handling as rh

fn handle(c: pool::Conn) -> rh::Response {        // in a type
    let d: pool::Conn = pool::make()              // in a type and in a call
    let e = pool::Conn(id: 1)                     // in a struct literal
    let s: Shared[pool::Conn] = pool::make()      // nested, like any other type
    return rh::ok(d.id + e.id)
}
```

**The prefix falls where a name is written, not where a value is used.** `d.id`
and `d.respond(200)` carry none: there is a value in hand, and nothing to resolve.
In a body that works with values the prefix barely appears, which is what makes the
answer bearable without an import form.

**A module of the program is one name.** No path, no directories — `use a::b` is
not a form for a file of your own. The one `use` with a path in it is `std`'s,
which names the library rather than a file (ADR-030 D1), and that is unchanged.

**Refused, with the way out in the diagnostic:**

```
error: names are not brought in; a module is reached through its name
   1 | use pool::{Conn, Pool}
       ^^^^^^^^^^^^^^^^^^^^^^
     = write `use pool`, and `pool::Conn` where you need it
     = if the prefix is long, `use pool as p` shortens it once, in one place
```

**Two rules that come with it.** A prefix must be introduced: `pool::Conn` without
`use pool` is refused, so that a file still lists what it depends on at the top.
And one name per file: two modules that end up under the same name — by alias or
by collision with another module — is an error rather than a rule about which
wins.

### Why the selected form is refused rather than left open

Both shapes are proven, and the evidence says so plainly.

* **The blanket form is regretted everywhere it exists.** Rust lints it, Python's
  style guide forbids it, Elm discourages it for libraries. That is the clearest
  signal in the whole space, and it settles `use pool::*` on its own.
* **The selected form is everywhere and regretted nowhere.** Java, Python, Rust,
  Haskell, Elm and OCaml all have it and none has withdrawn it. It is not a
  mistake, and refusing it is not a claim that it is one.
* **Qualified-only is sustained.** Go has had no import selection for fifteen
  years, with no serious movement to add it.
* **And one community that had both converged away from the convenient form.**
  Haskell's practice moved toward explicit lists and qualified names, for the
  reason this decision is about: knowing where a name came from.

So general experience does not decide it — both work. What decides it is a rule
this language has already applied to itself: **one form, not two spellings of it.**
[ADR-041](specification/adr/adr-041.md) withdrew the automatic lambda argument
names on exactly that ground. A selected import creates the same situation for
every name in the language — the same type spelled `pool::Conn` here and `Conn`
there, both correct. Having just paid to remove one such pair, the language should
not introduce another across its whole surface.

**And the refusal is not irreversible, which is why it can be firm.** Adding a
selected form later breaks no program: everything written with a prefix stays
valid. If a program of real size shows the prefix to be a genuine cost rather than
a predicted one, the form can be added then, with evidence instead of a guess. What
is not deferred is the decision — a question left open is one that every file
written in the meantime has to live with unanswered.

### What this does not answer

**Re-export.** There is no way for a module to offer a name that another module
declares, and this decision does not create one. It is recorded in §6, where
libraries are, because that is where its cost lands.

### What it costs, and what it does not block

Every use of a foreign name is longer by a prefix, and the prefix lands where this
language already asks for a type — a signature, and the annotated `let` that is
the only place sharing begins. What is bought is that a reader of any line knows
where every name in it comes from without consulting the top of the file, and that
no edit elsewhere changes what an already-written line means.

[`open-work.md`](open-work.md) §1.1 was never blocked by this. Both answers needed
the same two pieces — a qualified type name that resolves to the same type, and a
struct literal that tolerates a prefix. The alias is the one thing this answer adds
to that repair.

---

## 2. The numeric surface, and what `len()` hands back — **answered and built** ([ADR-048](specification/adr/adr-048.md))

The heading asked which numeric types the language offers; the body only argued
about `len`. Both halves are answered here, because answering the second alone
leaves the first standing.

### A length is an `i64`

`len` and its three siblings hand back an `i64`. The machine-width type leaves
the surface a program can write, and `usize::truncating_i32` and
`usize::truncating_i64` go with it — nothing narrows out of a type no program can
hold.

**The scope is four signatures**, not a neighbourhood: `Vec::len`, `String::len`,
`str::len` and `HashMap::len` are every entry in `std.contracts` that returns one.
On the compiler side the writable surface is one list — `check`'s `NUMERIC`, which
carries `usize` and `isize` — and `contracts::send`'s plain-type list, which only
answers whether such a type may cross and is unaffected.

**Why this way round, and what the two conversions really cost.** They are not
symmetric, and the entry treated them as if they were.

*Out of a length* — `t.len() as i64` — cannot fail. A `usize` exceeds an `i64`
only above 9,223,372,036,854,775,807 elements; at one byte each that is eight
exabytes of memory. Every one of those conversions is ceremony with nothing behind
it, written by the user, in every loop and sum and comparison that meets a length.

*Into an index* — `v[i]` where `i` is an `i64` — can fail, for a negative number,
which `pos - 1` at `pos == 0` produces. But a negative index **is** an access out
of bounds, and Part III A.2 already aborts on those. It is not a new failure mode;
it is the same one, reached a step earlier. And the conversion is emitted rather
than written: the user writes no conversion at all, in either direction.

So the trade is a great deal of visible ceremony that cannot fail, against one
invisible abort that already exists.

**One requirement comes with it.** The emitted conversion at an index must report
as an access out of bounds, not as a failed conversion. Otherwise a user gets a
diagnostic about a conversion they never wrote, for a mistake they understand as a
bad index.

**The field agrees, with one dissenter.** Go, Java, C# and Swift all hand back a
signed integer for a length and index with it — Swift explicitly, on the ground
that unsigned arithmetic causes more bugs than it prevents. Rust is the outlier,
and the conversions its `usize` demands are among the most commonly named
papercuts in it.

### The numeric surface is stated, and `u8` is named

Part I 2.2 offers `i32`, `i64` and `f64`. More than three are reachable:
`fs::read` hands back a `Vec[u8]`, `std.contracts` says in as many words that *"the
compiler accepts `u32` and the rest, but the specification does not offer them"*,
and `usize` was writable until the paragraph above.

**`u8` is named in 2.2**, with the same conversion and arithmetic names every other
numeric type now has. Not the rest of the unsigned family: an entry exists because
a program asked for it ([ADR-028](specification/adr/adr-028.md) D5), and a program
that reads a file asks for a byte. `u32`, `u64` and `isize` stay out until
something does.

**Why not signed bytes instead.** Java has no unsigned type and pays for it in
every byte-handling program, where a masking dance undoes the sign at each step.
Go, Rust, C# and Swift all carry a byte type. The choice here is not whether a
program meets one — reading a file already hands it one — but whether the
specification admits it.

**What it costs:** four numeric types where the front page promised three, and a
beginner meets an unsigned one the first time they read a file. What it buys is
that the list of types is the list of types.

### Checked before deciding

* `x as i64` where `x` is already an `i64` lowers without complaint, so the
  four redundant conversions in `examples/1brc.nika` do not have to change with
  this — they become unnecessary rather than wrong.
* The machine-width type reaches user code through the one `NUMERIC` list named
  above and nowhere else in `check`.

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

## 6. How a Nikaia library is offered, and what consumes one — **answered** ([ADR-047](specification/adr/adr-047.md); the package half is built, the path dependency is not)

Two questions were hiding in one, and only the first of them shapes what a program
means.

### The language half: a package is a directory, and its files share one namespace

**A package is a directory.** The files in it see one another with no `use` at
all. What the package offers outward is whatever is marked public, in whichever
file it is declared. A consumer writes `use http` and names the **package**, never
a file inside it.

**What this changes.** Part I 9.2's privacy boundary moves from the file to the
package: a declaration is private to its package unless it says `pub`, and two
files of one package may not declare the same name. That sentence has to be
rewritten rather than extended.

**Why this rather than a re-export form.** It removes the problem instead of
patching it. There is nothing to re-offer, because the names inside a package
already share a space — so no new form is needed at all. And a library's internal
file layout stops being its public surface by construction: moving a declaration
from one file to another is housekeeping, and breaks no consumer.

The field points the same way. Java, Go and Rust all put the boundary of privacy
and publication **above** the single file. Python is the one that puts it at the
file, and `__init__.py` — a front door maintained by hand, forgotten names and
all — is the consequence, not an accident of it.

**The alternative, considered and refused:** keep file = module and add a
re-export. It is not wrong, and it would leave 9.2 untouched. It costs a new form
in a language that has just withdrawn two, and it moves the upkeep onto every
library author forever.

**A side effect worth stating rather than discovering.**
[`open-work.md`](open-work.md) §1.1 is about naming a type across a file boundary.
Inside a package that boundary no longer exists, so half of what it describes
stops being reachable; what remains is the cross-*package* case, which is the one
that was always the point.

### The distribution half: a path dependency, and nothing else yet

**A package may be depended on by path**, the way a Rust one already can be.
`std` arrives that way today ([ADR-002](specification/adr/adr-002.md) D4), so this
generalises a mechanism rather than adding one.

**No registry, no name space, no distribution format.** The three things ADR-002
D1 §5 refuses to guess at stay open, deliberately. Go went years with the fetch
location as the identity; Rust and JavaScript built their registries early and
have carried decisions made when there were three packages. A registry is built
when there are libraries, not before.

**Five rules come with it, or they get decided by accident:**

1. **The manifest key is the name.** `use` writes the key, and the path says where
   it comes from. Two libraries that both want to be `http` are therefore the
   consumer's to name apart, which is the same authority `use pool as p` already
   gives them.
2. **Transitive dependencies are not visible.** If A depends on B and B on C, A
   does not see C. Otherwise a library's surface is everything it happens to use,
   and swapping an internal dependency breaks consumers.
3. **Same path is the same package; a different path is a different package**,
   identical contents included. With no versions there is nothing to unify, and
   two paths to two copies would be two types of one name.
4. **A dependency's `[build]` section is ignored, and the compiler says so.** A
   package is built with the settings of the program that uses it. Anything else
   would put two answers to the parallelism question in one build.
5. **A Nikaia dependency is part of the program, not a foreign package.**
   [ADR-043](specification/adr/adr-043.md) turns overflow checks on for the
   program and off for every foreign package; a Nikaia dependency is written in
   this language, carries this language's promise, and an overflow in it is the
   same broken state. Without this stated it lands on the foreign side by
   accident, because that is where a dependency mechanically appears — and then a
   library computes silently wrong numbers while the program that calls it aborts.

**What this makes real.** The rule that shaped
[ADR-045](specification/adr/adr-045.md)'s crossing verdict — a library built at one
setting stays usable at the other — has until now protected a situation nothing
could construct. A path dependency is the smallest change that makes it testable,
and the first Nikaia library is the first real test of something that has so far
only been argued.
