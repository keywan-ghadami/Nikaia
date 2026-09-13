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
that could not fit, and **a keyword that could be a name** - which took the `dsl`
block's diagnostic and three silent misreadings with it
([ADR-051](specification/adr/adr-051.md)). Each is in the CHANGELOG with what it
was and what fixed it; a fixed entry kept here only makes the list longer to
read.

### 1.1. An undeclared name is refused as a **statement** and nowhere else

`NK1117` fires where a statement is one name. A name used inside an expression —
`let n = q + 1`, `f(q)`, `q.len()` — resolves to nothing and is passed over in
silence, and `rustc` then refuses the generated file about a name the user did
write but never declared. That is the Part III C.1 class, one step milder than the
cases already fixed: the *line* is the user's even though the message is Rust's.

Two of the three letters the withdrawn lambda form used are covered specially
([ADR-049](specification/adr/adr-049.md) §5): a free `a`, `b` or `c` is refused
with a message about the withdrawal, because that is the mistake a reader of the
old specification will actually make. Every other name is not.

**The reserved-word list narrowed this and left one case behind**, which is what
makes it worth reading against [`spec-promises.md`](spec-promises.md). Every word
the specification names for a construct the grammar does not have — `assert`,
`const`, `unsafe`, `test`, `bench`, `macro` — is a name in statement position, and
`NK1117` refuses each of them by name. `quote` is the exception, and it is this
entry exactly:

```nika
let q = quote { 1 + 1 }
```

parses as `let q = quote` and then `{ 1 + 1 }`, and **lowers in silence**, because
the undeclared name is the *value of a `let`* and not a statement. So this is not
a hypothetical: it is the one row of that page still marked *"means something
else"*.

*What it needs:* a decision, not just work. "This expression names something
nothing declares" is a much wider claim than the statement rule makes, and the
checker's polarity is that it never refuses a correct program (C.4) — so the list
of what counts as declaring a name has to be complete before the rule can be
widened, and today it is not: a name from a package this build cannot see would be
refused.

### 1.2. A `std` function whose Rust parameter is a `usize` still needs a written conversion

[ADR-048](specification/adr/adr-048.md) D1 made a length an `i64` and emits both
conversions, and its scope is deliberately what a length *returns*. The other
direction is left: `"  ".repeat(indent as usize)` in `examples/json.nika` is the
one site in this repository, and `as usize` is now a conversion to a type the
specification does not offer (§3.1 of that record says so).

*What it needs:* the same shape of answer one level over — a parameter a ledger
describes as `i64` whose Rust counterpart takes a `usize`. No program has yet made
the answer obvious, which is why the record leaves it open rather than guessing.

### 1.3. A relayed `rustc` message may still name a type the program did not write

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

### 1.4. An out-of-range literal that nothing constrains is refused in Rust's words

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

### 1.5. A parameter or a struct field may still be called `self`

[ADR-051](specification/adr/adr-051.md) D4. `self` is a reserved word the grammar
cannot exclude from its name rule - `self.min` refers to it, and one rule serves
both declaring a name and referring to one - so declaring one is `NK1119` from
the checker. It covers a `let`, a `for` binding and a lambda's argument, and not
a parameter or a struct field.

```nika
fn f(self: i32) -> i32 { return self }
```

is accepted here and refused by `rustc` about the generated file - *"expected
identifier, found keyword `self`"* - which is the Part III C.1 class, now down to
two positions from all of them.

*Why it is not simply done:* neither `FnArg` nor `FieldDef` records a source
position, and the nearest span each walk has is the body's first statement, which
is a different line. A caret on the wrong line is worse than no message, so this
waits on the span rather than being approximated. **Small and mechanical**: one
field on each of two AST nodes, set where the parser already has `_span`.

### 1.6. A function that hands back a view emits Rust with no lifetime

```nika
fn name() -> &str { "Ada" }
```

lowers to `fn name() -> &str { "Ada" }`, and `rustc` refuses it: *"missing
lifetime specifier — this function's return type contains a borrowed value, but
there is no value for it to be borrowed from"*, about the generated file. The
Part III C.1 class.

**Why it happens.** A view's lifetime comes from the parser's input
([ADR-008](specification/adr/adr-008.md)), and the emitter elides it where a
reference among the arguments can carry it. A function with **no** reference
argument has nothing to elide from, and `"Ada"` is a `&'static str` that the
signature does not say so about.

*Found* while building [ADR-052](specification/adr/adr-052.md), because `&str?`
made the same signature one step longer and the error easier to read; it is not
about nullability and reproduces without it. *What it needs:* a decision about
where a returned view's lifetime comes from when no argument provides one -
`'static` is right for a literal and wrong for anything else, so this is not a
one-line default.

### 1.7. A `rustc` **warning** about the generated file reaches the user

Part I 2.3's own example, written as the page writes it, prints

```text
warning: value assigned to `maybe_string` is never read
   = help: maybe it is overwritten before being read?
```

The observation is true of the program, and the message is Rust's about a file
nobody wrote. `--explain` ([ADR-012](specification/adr/adr-012.md)) translates an
**error** back to the `.nika` line; a warning goes past it untouched.

*Not a matter of silencing it:* the warning says something the writer of the
Nikaia program should hear. What it needs is the translation path warnings do not
take yet - and a decision about which of `rustc`'s warnings are about the user's
program (this one) and which are about the shape this compiler emitted (the
class `is_rust_internal` already drops).

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

### 2.4. Part I 3.5's `?.`, and the nullable wrap at an argument

[ADR-052](specification/adr/adr-052.md) §4. The type is built — `T?`, `null`, and
the `Some(…)` at an annotated `let`, an assignment and a `return`. Two pieces are
left, and each has its own reason for being left.

**`?.` needs the checker and not the emitter.** The lowering is not the
difficulty: `x?.full_name` is `x.map(|v| v.full_name)`. It is that a field which
is **itself** nullable needs `and_then` instead, or `a?.b?.c` comes out holding a
nullable of a nullable — and which of the two is right is a question about the
field's declared type. So it goes the way D4's wrap does: the checker decides and
the emitter writes. The key can be `(statement, field name)`, which is the shape
`fallible_methods` already uses and documents.

*What is measurable now:* `let name = repo.find_user(id)?.full_name` is a parse
error at the `?`. The position is free — the parser never builds `Expr::Try`,
because failure propagation is implicit
([ADR-025](specification/adr/adr-025.md)) — so there is nothing to disambiguate
against.

**The wrap at an argument** — `takes(42)` where the parameter is an `i64?` — is
not a position this compiler can name: expressions carry no spans, and the three
places that *are* covered are each named by the statement they stand in. So this
waits on the same thing §1.5 does, one field on an AST node, or on a different
key.

### 2.5. Part II 12.8's supervision syntax

`supervisor::start_link(fn { … }; restart_policy: …)` is specified and there is no
supervisor. Listed so it is not mistaken for something the `spawn` work includes —
it is not.

### 2.6. A package's own package dependencies are not resolved

[ADR-047](specification/adr/adr-047.md) D2 is built one level deep: a program
depends on a package by path, and a package that declares Nikaia dependencies **of
its own** is refused rather than resolved.

What is missing is the **resolution**, not the visibility rule: transitive
dependencies are not visible either way (D2 rule 2), so the refusal states the rule
correctly and declines the graph.

**And the graph is a decision, not just work** — which is what looking at it this
session established. Every package becomes a `mod` at the one crate root, named by
the manifest key of whoever depends on it. So if A names B as `b` and B names C as
`c`, the root carries `mod b` and `mod c`, and:

* A could write `c::thing()` with no `use c`, and the ledger would answer — which
  breaks rule 2 silently. Closing that means making
  [ADR-046](specification/adr/adr-046.md) D4 a check on **qualified names** and
  not only on `use` lines.
* If A also depends on a *different* package under the name `c`, two packages want
  one `mod c`. Refusing it names B's internals to A, which rule 2 says A should
  not see; not refusing it is two types of one name.

Either answer needs a naming scheme keyed on package identity rather than on the
consumer's word — which is the registry-shaped question ADR-002 D1 §5 declines.
So this waits on a decision and not on an afternoon.

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
* [`handoff.md`](handoff.md) — a previous session's open work on messages and the
  parser backend, with its own closed/open split. Still accurate about that area.
* [`spec-promises.md`](spec-promises.md) — every construct the specification
  names, probed against the compiler. The evidence behind the **Status** notes, and
  the right place to look before adding an entry to §3 here.
* [`error-corpus.md`](error-corpus.md) — twenty-six broken programs and what the
  compiler says about each.
