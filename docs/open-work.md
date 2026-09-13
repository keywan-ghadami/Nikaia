# Open work — what is found, what is decided and unbuilt, what is stale

The running list. Three kinds of entry, kept apart because they cost different
things to be wrong about:

* **Defects** — the compiler accepts a program it should refuse, produces a
  different program than the source says, or hands the user something in the
  backend's words. A defect outranks everything below it.
* **Decided and unbuilt** — an ADR says what happens and the compiler does not
  do it yet. Each one names its record; the record is the specification of the
  work, and this file only says where it stands.
* **Upkeep** — a specification sentence or a notes page that a later decision
  made false. `docs/README.md` §1 makes a stale **Status** note a defect in its
  own right, because a reader cannot tell a plan from a promise.

**Cite an entry by its subject and not by its number.** The numbers renumber
whenever something closes - measured the hard way, by a round that closed three
entries and left nine citations in the specification, the records and the tests
pointing at whichever entry had moved into the slot. What an entry *is* stays
put; where it sits does not.

**Every entry carries its evidence or says that it has none.** An item with a
reproduction is a fact; an item without one is a suspicion, and it is marked as
such rather than inheriting the authority of the list around it. Questions that
need the owner rather than work are in
[`open-decisions.md`](open-decisions.md).

This file is a notes page: nothing here is normative, and nothing may depend on
it to know what a program means.

---

## 1. Defects

What was here and is gone: a module that could not hand out types, a bare word
that became a different program, a refusal that carried a backtrace, a Rust
warning about the generated file, a `Shared` slot decided twice, the explain modes
missing from a project build, a cache that filled the disk, a sum of constants
that could not fit, **a keyword that could be a name** - which took the `dsl`
block's diagnostic, three silent misreadings, and every position that can
declare one with it ([ADR-051](specification/adr/adr-051.md)) - and a result that
borrowed from nothing and named no lifetime for it, a warning the backend said
twice, and an undeclared name refused only where it stood alone.

Each is in the CHANGELOG with what it
was and what fixed it; a fixed entry kept here only makes the list longer to
read.

### 1.1. A `std` function whose Rust parameter is a `usize` still needs a written conversion

[ADR-048](specification/adr/adr-048.md) D1 made a length an `i64` and emits both
conversions, and its scope is deliberately what a length *returns*. The other
direction is left: `"  ".repeat(indent as usize)` in `examples/json.nika` is the
one site in this repository, and `as usize` is now a conversion to a type the
specification does not offer (§3.1 of that record says so).

*What it needs:* the same shape of answer one level over — a parameter a ledger
describes as `i64` whose Rust counterpart takes a `usize`. No program has yet made
the answer obvious, which is why the record leaves it open rather than guessing.

### 1.2. A relayed `rustc` message may still name a type the program did not write

**The map's hasher is fixed.** The emitter writes a trusted input's map as
`TrustedMap`, so *"type annotations needed for `HashMap<_, _,
BuildHasherDefault<FxHasher>>"* named a hasher this compiler chose
([ADR-010](specification/adr/adr-010.md) D5) in a message about a real defect in
the program, on the right line. A name this compiler substituted on the way out is
put back on the way in, and the criterion is the substitution's own: `map_name`
says *"same table, same API, same full-content equality — so this is a name and
not a translation"*, and only that kind is undone. `Shared[T]` deliberately is
not: `Rc` and `Arc` are different types, and *"expected `Shared[T]`, found
`Shared[T]`"* would hide a defect in this compiler rather than translate one of
Rust's words.

*What is left:* the general rule. Every other type the emitter writes and the
program does not — the shadow struct of a deferred-parameter DSL, a grammar's
generated types, `nikaia_std`'s own names — can appear in a relayed message, and
there is no list saying which of them are names and which are translations. The
two that exist are handled where they are; a third will arrive without anything
noticing.

---

### 1.3. An out-of-range literal that nothing constrains is refused in Rust's words

```nika
let big = 3000000000
```

is *"literal out of range for `i32`"* — the right line, the backend's words, and a
type the program never wrote. `NK1116` does not reach it: it answers where a type
**stands beside** the literal, and here nothing does.

**And it must not simply be widened to this case.** The same line is a *correct*
program where a use asks for an `i64`:

```nika
let m = 3000000000
println(f"{wide(m)}")   // fn wide(n: i64) -> i64
```

Rust's inference decides, and this checker has none — so refusing at the `let`
would refuse that program, which is the one thing the checker may never do
(Part III, C.4). Part I 2.4 now states the rule properly, which it did not.

*What it needs:* enough inference to know that nothing else constrains the
literal — or a decision that an un-annotated literal is an `i32` full stop, which
would make the second program above a refusal and is not what the page says
today.

**The constant fold does not reach it, and that is worth stating** now that the
fold exists ([ADR-043](specification/adr/adr-043.md) §4, which closed the *sum*
of constants this list used to carry). The fold answers *what a constant
expression comes to*; this entry is about *what type it has*, and those are
different questions. So `let b = 3000000000 + 1` is the same one entry as `let big
= 3000000000`: the fold evaluates both and neither has a type to be measured
against, because a literal pins nothing. Only inference closes it.

*Fixed on the way past:* the note `rustc` attaches to it — *"consider using the
type `u32` instead"* — is dropped. It was kept once, checked, because it
compiles; [ADR-048](specification/adr/adr-048.md) D2 is what changed, since the
numeric surface is the one Part I 2.2 names and `u32` is deliberately not on it. A
remedy that works is kept; one that leads out of the language is not.

---

## 2. Decided and unbuilt

Two things hold across this whole section, and they are here rather than argued
again inside each entry.

**Checked and unrunnable is the state that rots fastest.** A check with no program
to be tested against is correctness that quietly stops being true — nothing fails
when it drifts, because nothing exercises it. An entry here that says *"the check
runs and the construct does not"* is more urgent than its size suggests.

**A refusal is free before programs exist and breaking afterwards.** Anything in
this section that adds a refusal — a diagnostic, a narrowed rule — costs nothing
today, because no program can be written that it would reject. The same refusal
added after programs exist breaks them. That asymmetry belongs to the work, not to
the order somebody happens to pick.

**And the order lives here**, because this is the list that knows what each item
costs. It used to live twice — this file and the roadmap's own "next steps" — and
the second copy is the one that went stale, still asking for `if/else` and structs
long after both worked. Two lists of one thing is one list and one liability.

So, in order, and each says below why it sits where it does:

1. **The runtime binding `spawn` needs.** Five records are checked and cannot run
   until it exists, which is the first principle above in its sharpest form. It is
   also what the two entries after it wait on.
2. **`SharedMut[T]` and `Locked[T]` as types the backend can build.** The other
   half of the same story: a program that spawns needs something it may share, and
   today writing one is checked and then fails to emit.
3. **`overlap { … }`, and then the automatic reordering out.** In that order and
   not the other — removing the automatic half first would leave the language with
   no way to ask for overlap at all. The removal is the second principle's case:
   free today, breaking once programs exist.
4. **The diamond in the checker.** Small, self-contained, waits on nothing. It is
   the only entry here that one afternoon closes.
5. **A server to bind to, and the `postgres` block.** Its own project rather than a
   step of this one.
6. **Supervision.** Last because nothing else waits on it.

### 2.1. `spawn` has no runtime binding — and five records wait on it

`Expr::Spawn` refuses in the emitter: *"`spawn` needs the runtime integration; not
emitted yet"*. What is checked but cannot run:

* Part II 11.2's whole section;
* [ADR-040](specification/adr/adr-040.md) D1's task half — the analysis names a
  `spawn` body's handle as a duplication site, and no emitted program reaches it,
  so there is still no `NK2101` to exempt;
* [ADR-045](specification/adr/adr-045.md) D2 — a lock may go into a task, which is
  now checked and cannot yet be run;
* Part II 12.2's counter, the program `user_parallelism = yes` exists to serve;
* [ADR-050](specification/adr/adr-050.md) D2's `overlap { … }` — the one way a
  program asks for overlap, now that the automatic half is on its way out. Its §5
  gives the order and this is step one of it.

This is the largest single unblocking in the file, and it grew by one this
session.

### 2.2. `SharedMut[T]` and `Locked[T]` are not types the backend can build

[ADR-039](specification/adr/adr-039.md) §4. The verdicts about them are built —
the crossing destination ([ADR-045](specification/adr/adr-045.md)), the four doors
and the nesting rule are specified — but a program that writes one as an
annotation is checked and then fails to emit. Also waiting inside this:

* the **lock-touching** derived property (ADR-039 D3, D7): no function carries it,
  so nothing tells a spawned body from a scope's;
* the **re-entrancy check as a build switch** (ADR-039 D8), which the cache key
  already accounts for;
* `NK2201`–`NK2205` and `NK2503`, catalogued and not emitted.

### 2.3. The automatic reordering, `seq` and the `ordering` switch are still here

[ADR-050](specification/adr/adr-050.md) D1 and D7 withdraw all three, and its §5 says
**not yet**: the removal is step three, after the runtime binding and `overlap`.
Removing them before there is a way to *ask* for overlap would leave the language
with neither, which is worse than either end state.

So this entry is not work to pick up — it is the thing that must not be picked up
early. It is here because a reader of [ADR-033](specification/adr/adr-033.md)
should find out from the list that its `seq` and its switch are on their way out.

### 2.4. Part II 12.8's supervision syntax

`supervisor::start_link(fn { … }; restart_policy: …)` is specified and there is no
supervisor. Listed so it is not mistaken for something the `spawn` work includes —
it is not.

### 2.5. A package reached under two names is two types to the checker

[ADR-053](specification/adr/adr-053.md) is built: a package is its own crate, a
library may depend on a library, and a program cannot reach past what it declared.
**One thing it decided is not built.** D2 says a package reached through two
parents is one crate, and Cargo makes that true — the hand-built shape in that
record's §3 passes a value from one to the other and it is accepted.

The checker in front of Cargo does not agree. The ledger names a type by the
manifest key it was reached **through**, so a program that depends on a package as
`deep`, and on a library that depends on the same package as `c`, is told its
`deep::Id` is not the `c::Id` the library's function takes. Both are the same Rust
type; only the name this compiler gave it differs.

It is a refusal and not a wrong answer, and it needs a diamond to meet: one level
of dependencies is named entirely by the program's own words. The fix is for a
type's identity in the ledger to be the package's **canonical path** — which is
what `packages_of` already computes and what D2 means by identity — rather than
the word a consumer happened to write.

### 2.6. `fortunes.nika` waits on two runtime pieces, and neither is a language question

The template half is built — [ADR-017](specification/adr/adr-017.md)'s `dsl html`
compiles where it is written, every hole goes through `html::Render`, and the
whole file parses since [ADR-022](specification/adr/adr-022.md) removed the `fn:`
form. What is left is machinery, not syntax:

* **The `postgres` block**, a deferred-parameter DSL
  ([ADR-007](specification/adr/adr-007.md) D4): the statement has to reach a
  driver intact, which is different machinery from the template that exists.
* **The runtime binding for a handler.** [ADR-018](specification/adr/adr-018.md)
  decided what a handler *is* — the request as its first implicit argument, and
  what each return type answers with — and none of it can be built before there is
  a server to bind to.

Moved here from [`handoff.md`](handoff.md), which is a guide to the parser backend
and was also carrying open work. One list.

---

## 3. Upkeep

**Empty**, which it has not been before, so what was here is worth naming: Part
III 15.2 claimed the compiler *"reads the metadata of the Rust Crate"* and quoted
an error about an `Rc<i32>` that nothing produces; the same section's type mapping
stopped at three rows and had no entry for the type
[ADR-045](specification/adr/adr-045.md) §3's whole argument turns on; Part III 15.3
said a single-threaded build generates no atomic operations, which
[ADR-037](specification/adr/adr-037.md) D6 made false; and five notes pages read as
current while writing lambdas in the form
[ADR-049](specification/adr/adr-049.md) withdrew or reasoning from what
`user_parallelism` used to imply. Each is in the CHANGELOG.

A stale **Status** note is a defect in its own right
([`README.md`](README.md) §1), because a reader cannot tell a plan from a promise -
so this section being empty is a state to try to keep rather than a milestone.

## 4. Where the other lists are

* [`project_status_and_roadmap.md`](project_status_and_roadmap.md) — the phases,
  and what runs today. The long view; this file is the short one.
* [`handoff.md`](handoff.md) — how to work on the **parser backend**: how to test a
  change against Nikaia, what the patch does, and what was tried and must not be
  redone. A guide rather than a list; what was open in it is an entry above.
* [`spec-promises.md`](spec-promises.md) — every construct the specification
  names, probed against the compiler. The evidence behind the **Status** notes, and
  the right place to look before adding an entry to §3 here.
* [`error-corpus.md`](error-corpus.md) — twenty-six broken programs and what the
  compiler says about each.
