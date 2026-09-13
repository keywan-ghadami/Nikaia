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
missing from a project build, and a cache that filled the disk. Each is in the
CHANGELOG with what it was and what fixed it; a fixed entry kept here only makes
the list longer to read.

### 1.1. A `dsl` block missing its `} eod` is reported as an undeclared name

`dsl postgres { SELECT 1 }` without the closing `} eod` is not a `dsl` block at
all, so what is left parses as statements and `NK1117` says *"nothing declares
`postgres`"*. True, and not what the writer got wrong.

**The obvious fix does not work, and this is why.** A `fail("…")` arm on
`dsl_block_expr` — the shape ADR-022's `fn: …` refusal uses — is *not fatal* in
this parser: its own documentation says **"an error that got further still wins
(progress before priority)"**. So where the rest of the file has a reading that
parses, as it does here, the `fail` is discarded along with everything else about
that attempt. Tried and reverted this session.

Two answers, and both are bigger than the entry looks:

* **A cut.** Once `dsl NAME {` is matched, forbid backtracking out of the rule.
  The grammar library offers `not`, `peek`, `until`, `recover` and `fail`, and no
  cut, so this is a change to that library.
* **Reserve the word.** `dsl` is a keyword (Part II, 10.5) and `NAME` matches it
  anyway, which is what gives the bad reading its alternative. Reserving it is one
  word in the grammar and a decision about whether this language's keywords are
  reserved words — today they are contextual, and some of them (`rule`,
  `boundary`, `fold`) are words a program may well want.

Where a `dsl` block opens and nothing else parses, the message is already good:
the parser names `} eod` among what it expected.

### 1.2. An undeclared name is refused as a **statement** and nowhere else

`NK1117` fires where a statement is one name. A name used inside an expression —
`let n = q + 1`, `f(q)`, `q.len()` — resolves to nothing and is passed over in
silence, and `rustc` then refuses the generated file about a name the user did
write but never declared. That is the Part III C.1 class, one step milder than the
cases already fixed: the *line* is the user's even though the message is Rust's.

Two of the three letters the withdrawn lambda form used are covered specially
([ADR-049](specification/adr/adr-049.md) §5): a free `a`, `b` or `c` is refused
with a message about the withdrawal, because that is the mistake a reader of the
old specification will actually make. Every other name is not.

*What it needs:* a decision, not just work. "This expression names something
nothing declares" is a much wider claim than the statement rule makes, and the
checker's polarity is that it never refuses a correct program (C.4) — so the list
of what counts as declaring a name has to be complete before the rule can be
widened, and today it is not: a name from a package this build cannot see would be
refused.

### 1.3. A `std` function whose Rust parameter is a `usize` still needs a written conversion

[ADR-048](specification/adr/adr-048.md) D1 made a length an `i64` and emits both
conversions, and its scope is deliberately what a length *returns*. The other
direction is left: `"  ".repeat(indent as usize)` in `examples/json.nika` is the
one site in this repository, and `as usize` is now a conversion to a type the
specification does not offer (§3.1 of that record says so).

*What it needs:* the same shape of answer one level over — a parameter a ledger
describes as `i64` whose Rust counterpart takes a `usize`. No program has yet made
the answer obvious, which is why the record leaves it open rather than guessing.

### 1.4. A relayed `rustc` message may still name a type the program did not write

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

### 2.1. `spawn` has no runtime binding — and four records wait on it

`Expr::Spawn` refuses in the emitter: *"`spawn` needs the runtime integration; not
emitted yet"*. What is checked but cannot run:

* Part II 11.2's whole section;
* [ADR-040](specification/adr/adr-040.md) D1's task half — the analysis names a
  `spawn` body's handle as a duplication site, and no emitted program reaches it,
  so there is still no `NK2101` to exempt;
* [ADR-045](specification/adr/adr-045.md) D2 — a lock may go into a task, which is
  now checked and cannot yet be run;
* Part II 12.2's counter, the program `user_parallelism = yes` exists to serve.

This is the largest single unblocking in the file.

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

### 2.3. Every abort should name the Nikaia line

[ADR-044](specification/adr/adr-044.md). One location table beside the program and
a lookup in the panic hook. Nothing is built: the emitter keeps no line table and
the panic hook is itself unbuilt. Every abort this project added — the overflow,
the two conversions — names the generated file today, which is the reason that
record exists.

### 2.4. A sum of constants that cannot fit is still `rustc`'s refusal

[ADR-043](specification/adr/adr-043.md) §3 and §4. `NK1116` refuses an out-of-range
**literal** where a type stands beside it; `let b = a + 1` where both are constants
is still refused by rustc, with *"this arithmetic operation will overflow"* about
the generated file. The same Part III C.1 class as §1.2 and §1.4 above.

### 2.5. Part I 2.3's nullable types are a parse error

A trailing `?` on a type does not parse and `null` is read as an ordinary name, so
neither line of that section's own example is accepted. `??` (Part I 3.5) is built.

### 2.6. Part II 12.8's supervision syntax

`supervisor::start_link(fn { … }; restart_policy: …)` is specified and there is no
supervisor. Listed so it is not mistaken for something the `spawn` work includes —
it is not.

### 2.7. A package's own package dependencies are not resolved

[ADR-047](specification/adr/adr-047.md) D2 is built one level deep: a program
depends on a package by path, and a package that declares Nikaia dependencies **of
its own** is refused rather than resolved.

What is missing is the **resolution**, not the visibility rule: transitive
dependencies are not visible either way (D2 rule 2), so the refusal states the
rule correctly and declines the graph. A second level needs a dependency graph,
a cycle rule and an order — none of which any program has asked for yet.

### 2.8. `use http as h`, and the braced and glob forms in this language's words

[ADR-046](specification/adr/adr-046.md) §5. D1, D4 and D5 are built now that a
package can be depended on; two pieces are left.

**D3's alias** — `use http as h` — does not parse. It is the one thing that record
*adds* rather than refuses, and it is what makes the qualified-only rule
affordable, so it is the next piece of it to build.

**D2's braced and glob forms** are parse errors at the brace and the star rather
than the sentence D2 writes. *"Names are not brought in; a package is reached
through its name"* belongs in the grammar, beside the `fn: …` refusal
[ADR-022](specification/adr/adr-022.md) already put there.

---

## 3. Upkeep

### 3.1. Part III 15.2 promises crate metadata the compiler does not read

[ADR-045](specification/adr/adr-045.md) §5, found and deliberately not fixed
there. The *"Thread Safety (Send/Sync)"* block says *"The compiler reads the
metadata of the Rust Crate"*, that a `Send` Rust type is allowed in a `spawn`
task, and quotes an error about `Rc<i32>`. **No crate metadata is read**: a foreign
call is one no ledger describes, and the verdict is taken on the argument's Nikaia
type. The section's `Status` note covers the lock rule and not this claim, so the
claim stands unmarked.

### 3.2. …and the same section's type mapping has no shared entry

It maps `i32`, `String` and `Option<T>` and stops, so nothing written down says
which Rust type a `Shared[T]` is at a boundary — which is the type
[ADR-045](specification/adr/adr-045.md) §3's whole argument turns on.

### 3.3. Part III 15.3 says a single-threaded build generates no atomic operations

*"the compiler does not generate OS-level mutexes or atomic operations in this
mode"* has been false since [ADR-037](specification/adr/adr-037.md) D6 made the
owner count atomic at both settings, and ADR-045 §3's measurement shows a foreign
call forcing the atomic count whatever the setting.

### 3.4. Two notes pages carry claims a later decision displaced

* [`foreign-runtime.md`](foreign-runtime.md) and [`std-sysroot.md`](std-sysroot.md)
  predate [ADR-037](specification/adr/adr-037.md) D6 in the places where they talk
  about what follows `user_parallelism`.
* [`from-for-throws-and-touches.md`](from-for-throws-and-touches.md),
  [`mutex-floor.md`](mutex-floor.md) and
  [`rc-or-arc.md`](rc-or-arc.md) write their lambdas in the form
  [ADR-049](specification/adr/adr-049.md) withdrew — `sort_by_key fn { a }`, which
  no longer compiles. The analysis each records is unaffected; the samples are not
  copyable.

A notes page is a laboratory record and is allowed to be a snapshot — what it is
not allowed to do is read as current. A dated header on each is enough.

---

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
