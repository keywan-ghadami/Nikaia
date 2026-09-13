# Open decisions — the questions that need the owner

Eight entries. **Six are answered and one is dropped**, and every answer has its
record: [ADR-046](specification/adr/adr-046.md),
[ADR-047](specification/adr/adr-047.md), [ADR-048](specification/adr/adr-048.md),
[ADR-049](specification/adr/adr-049.md) and
[ADR-050](specification/adr/adr-050.md). The seventh answer still owes its
record. §2 and §3 are built, §6's language
half is, and each entry says what is left. They stay here until the owner drops
them. **§8 is open.** Each entry says what is blocked, what the options are, **what I would do**, and what either direction costs — because a
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

## 3. `fn { … }`'s automatic `a`, `b`, `c` — **withdrawn, and built** ([ADR-049](specification/adr/adr-049.md))

Refused, not warned about, and not announced for a later release.

**What goes with it.** The form, the rule underneath it, and the warning that made
the rule visible. How many arguments such a lambda takes is read off *which of the
three names its body mentions*, so a local called `a` inside one is not a local but
an argument. [ADR-041](specification/adr/adr-041.md) made that visible with
`NK1114`; visible is not the same as good, and the rule stays true for exactly as
long as the form exists.

**Why refused rather than announced.** Announcing a withdrawal a release ahead is
the procedure for a language with code in the world. There is no release and no
Nikaia outside this repository, so the deprecation window protects nobody. What it
would do is keep the rule alive for the length of the window, and keep the warning
code alive with it.

**Scope.** Seven sites still reach for one of the three — `examples/1brc.nika`,
`examples/access-log.nika` and `examples/k-nucleotide.nika`, in two idioms: folding
into a map entry, and a sort key. They are rewritten with named arguments as part
of this, since after it they no longer compile. `NK1114` and the arity-from-body
mechanism are removed rather than left unreachable; `emit::implicit_params`, which
the warning shared with the emitter, goes with them.

**What it costs.** A named argument where a letter used to do. That is the whole
of it.

---

## 4. What is built next? — **dropped, not answered**

This was never a question for the owner. Its own first line said so: *"Blocked by
it: my next piece of work."* What is built next is chosen by whoever is building,
from [`open-work.md`](open-work.md), and the one real dependency among the
candidates — `SharedMut[T]` needs `spawn`, since its whole point is a second task
— orders itself without anybody ruling on it.

The heading stays so the numbering below it does not move. The two pieces of
reasoning that were worth keeping are now in `open-work.md` §2's opening, where
they apply to every entry rather than to one week's choice.

---

## 5. Statement order, and how a program asks for overlap — **answered** ([ADR-050](specification/adr/adr-050.md); not built — it needs the runtime binding `spawn` waits on)

The entry asked when `ordering` gets measured and whether it stays on. Both halves
are answered, and the first is answered by dropping it.

### The measurement requirement is struck

It could not settle what it was there for. The only corpus is this repository's own
examples, written by the people who designed the language, and a number off them
says something about those programs rather than about the language. "Measure it on
the flagship before v1.0" moves a decision onto an event that will not decide it —
which is why [ADR-033](specification/adr/adr-033.md) has read *provisional* for
months.

What decides it is a principle, which is how every other entry here was settled.

### The explicit form: `overlap { … }`

The language has no way to say *"run these and wait for all of them"*. `spawn`
starts something that outlives the call, `select` takes the first to finish,
`par_iter` runs one operation over many elements, `seq` forces an order. The one
shape every other language provides first is missing.

```nika
let (user, rights, prefs) = overlap {
    db::load_user(id)
    db::load_rights(id)
    cache::load_prefs(id)
}
```

Each statement in the block is one branch. The block starts them all, waits for
all, and its value is the tuple of their results in written order.

**It is not a task.** The block ends before the function continues, so nothing
outlives it: nothing is moved, borrowing works, and the crossing check has nothing
to do. That is what makes it lighter than two `spawn`s, and why it earns a form of
its own rather than being a pattern.

**The branches must meet on nothing, and the compiler checks it.** The same
analysis that decides today whether two statements *may* overlap, used the other
way round — not *"may I?"* but *"you said so; is it true?"*. A branch pair that
meets on a resource is refused, and the diagnostic names the resource and points at
`seq { … }` or plain statements for the case where an order was meant. No other
language checks this; they take the programmer's word.

**Failures behave as they do anywhere else.** A branch that fails makes the block
fail. If two fail, the first **in written order** wins, so the result is
reproducible — written order is the only order the source has. Per-branch handling
is `catch` inside the branch, which already works.

**A branch is an expression.** Several steps in one branch are a block expression
inside it. Where two branches would both be multi-line blocks doing the same shape
of work, the answer is a function called twice, and the code is better for it.

**The meaning is the same at both settings; only how much overlaps differs.** At
`no` a branch that computes still overlaps with another branch's *waiting*, because
the waiting is not user code — computation beside I/O overlaps at every setting,
and only computation beside computation needs `yes`. Same results, different
duration: `how`, not `what`.

### The automatic reordering goes, and `seq` and the switch go with it

Three packages were coherent: automatic reordering with `seq` and `overlap`;
`overlap` alone; or today's state, which is the automatic half with no way to ask
for the rest. The third is the worst of them — unpredictable wins, and no recourse
where they do not arrive.

**Taken: `overlap` alone.** Statements run in the order they are written, without
qualification. Whoever wants overlap writes it, and has it checked.

`seq { … }` exists only because statements can be reordered unasked; with nothing
reordering them it has nothing to do, and goes. So does `ordering` in `[build]`,
which returns Part I 1.2's count of build switches to the two it promises.

**Why on principle rather than on a number.** The design ships two escapes: `seq`,
to force an order the analysis cannot see, and a switch to turn the whole thing
off. Two escapes are an admission — that the analysis can be incomplete, and that
the correction is by hand. A feature that changes what a program means, resting on
an analysis admitted to be incomplete, corrected manually after the fact, is the
shape of a defect source rather than of a guarantee. Against that stands an
optimisation whose reach nobody has been able to state.

### What is built, and what this needs

**The automatic half is built as a special case, not as a primitive.** A pair of
adjacent file reads with their failures caught lowers to `task::read_pair(…)` — a
purpose-built "read two files", with a comment naming the record. `task::both`
exists in `std`'s Rust and the emitter does not reach it for this.

**So `overlap` is not a small piece of work, and it is not unblocked.** Branches
that are arbitrary expressions need the general concurrent path — the same runtime
binding [`open-work.md`](open-work.md) §2.1 says `spawn` is waiting on. What could
be built ahead of it is more special cases, which is how the automatic half got
narrow in the first place.

*Order that follows from this:* the runtime binding, then `overlap` on it, then the
removal of the automatic half, `seq` and the switch — removing them before there is
a way to ask would leave the language with neither.

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

---

## 7. Are this language's keywords reserved words? — **answered: yes**

**The answer.** Nikaia has a list of reserved words, and a name may not be one of
them. The list is what the grammar treats as a keyword **at the Nikaia level** —
not the vocabulary of the grammar sublanguage (`rule`, `boundary`, `fold`), which
is reserved inside a grammar block and nowhere else.

**What the state was, and it was not a decision.** `rule NAME = not(digit) n:ident`
— a name is any identifier that does not start with a digit, and nothing is
excluded. There is no reserved-word list in the parser at all. So the keywords were
not *chosen* to be contextual; the question had never been put.

**Why reserved rather than contextual.** The reason languages make a keyword
contextual is backwards compatibility: a new reserved word breaks programs that
used it as a name, so it is introduced in a position where it cannot be mistaken.
`var`, `async`, `await` and `nameof` are contextual in C# for exactly that reason,
and Go, which never had to add one, reserves all of them. **Nikaia has no programs
outside this repository**, so the reason does not apply, and the choice is free
today in a way it will never be again.

**Why this rather than a cut in the parser.** `open-work.md` §1.1 offered both: a
cut, so that once `dsl NAME {` matches the parser may not back out of the rule, or
reserving the word. A cut repairs **one diagnostic for one construct**. The
reserved list repairs **the class** — the `dsl` block, the silent miscompilation
below, and every keyword anybody adds later, without a new cut each time.

*(The grammar machinery does have a cut, written `=>`, for user-written grammars.
Whether the compiler's own grammar can reach it was not established, and does not
change the order: a local repair against a categorical one.)*

**What it costs.** A program may not name a variable `fn`, `spawn`, `seq`,
`overlap`, `dsl`. That is what every reserved word costs in every language, and it
is the price of a word meaning one thing wherever it appears. The list has to be
written down in Part I rather than left implicit in the grammar, so that a reader
can see it.

**What it fixes beyond §1.1.** The entry restored as a defect below: `assert c`
lowers to `let c = true as sert;` with no diagnostic at all — the word swallowed
into a cast, `sert` taken for a type name. A program that means something other
than what is written is the worst class this project names, and it has no fix short
of this one.

---

## 8. How does a package name the packages it depends on?

**Blocked by it:** [`open-work.md`](open-work.md) §2.7 — a package that declares
Nikaia dependencies of its own is refused rather than resolved, so a library that
uses a library does not exist. That is the second step of every real library.

[ADR-047](adr-047.md) D2 gave a package a path dependency, and it is built one level
deep. What stops the next level is not the visibility rule — rule 2 already says a
transitive dependency is invisible — but the fact that **every package becomes a
`mod` at one crate root, named by the manifest key of whoever depends on it**. Two
things follow, and both need answering rather than working around:

* If A names B as `b`, and B names C as `c`, the root carries `mod b` **and**
  `mod c`. A can then write `c::thing()` with no dependency on C at all, and the
  ledger answers — rule 2 broken silently.
* If A also depends on a *different* package it calls `c`, two packages want one
  `mod c`. Refusing the clash tells A about B's internals, which rule 2 says A may
  not see; allowing it gives two types one name.

Three answers:

* **(a) Name the module by package identity.** The root module name comes from
  something stable about the package rather than from the consumer's word, and the
  manifest key becomes a local alias onto it. The clash disappears. The leak does
  not: the module is still at the root, so [ADR-046](adr-046.md) D4's check has to
  widen from `use` lines to **qualified names**. And "package identity" is the
  registry-shaped question ADR-002 D1 §5 declines to answer, arriving early.
* **(b) Nest the modules.** B's dependencies live inside B's module: `mod b`, and
  `mod c` inside it. A has no path to C, so the leak is structural rather than
  checked. But the same package reached through two parents becomes two modules,
  which contradicts ADR-047 D2 rule 3 — same path, same package.
* **(c) One Rust crate per Nikaia package.** Each package is generated as its own
  crate with its own `Cargo.toml`, listing its own dependencies under its own
  manifest keys.

**I would take (c).** It answers both halves without inventing anything: a name
resolves per crate, so A has no way to name C and rule 2 costs nothing to enforce;
two packages under one key are in two different crates and never meet; and a
package reached twice is one crate, so D2 rule 3 holds because Cargo already works
that way. It also keeps the registry question deferred, because nothing here needs
a global name — the manifest key is enough when it is only ever read by one crate.

And it is the same move [ADR-002](adr-002.md) D1 already made once. Rust
dependencies are not resolved by this compiler; the manifest is translated and
Cargo does it. Package structure is the same kind of problem with the same tool
sitting under it, and the current shape — everything flattened into one crate — is
what creates a question Cargo does not have.

**What it costs, and it is not nothing:** a build produces several crates instead
of one, so the generated project grows a layout and the manifest generation grows
with it. Every `[build]` setting has to reach every crate, and
[ADR-043](adr-043.md)'s overflow checks have to name each Nikaia crate on the
program's side rather than the dependency's — that rule exists
([ADR-047](adr-047.md) D2 rule 5) and currently has one crate to apply to.
Incremental builds probably improve, which is a guess and not a claim.

**What the other two cost:** (a) pulls the registry question forward and widens a
check that was written for `use` lines; (b) is the cheapest to emit and breaks a
rule that was decided this week. Neither removes the question — they answer it with
machinery this project would then own, where (c) answers it with machinery it
already leans on.
