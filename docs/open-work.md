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

### 1.1. A module can hand out functions, but not types — **fixed for what remains of it**

Three findings, and the first of them always worked: the qualified **call**,
`pool::make()`. The other two are fixed.

**The qualified type name resolved to a different type.** `pool::Conn` and the
`Conn` that `pool::make()` hands back were two types, so `NK1103`'s help asked for
what was already written and could not be followed. The cause was one spelling
short: the ledger keys a type `pool::Conn` and a module's own signature says
`-> Conn`, because that is how the file declaring it writes the name. `absorb` now
qualifies the types **inside** an entry as well as the key — it is the only place
that knows both the module and what it declares — so the two spellings are one
name before anything compares them.

**And a struct from another file could not be built.** `pool::Conn(id: 1)` was a
parse error, because the literal's name was `NAME` where the type's was
`type_name`. Both are `type_name` now, and `fields_of` reads a foreign struct's
fields from the program's own ledger by the **exact** key — never by suffix, which
is how the library is matched: two modules may each declare a `Conn`, and a suffix
match would answer with whichever came first.

So a wrong field name, a wrong field type and a wrong annotation are all refused
by their qualified names now, in this language's words.

**What is left of this entry is the cross-*package* case.** The answer to
[`open-decisions.md`](open-decisions.md) §6 makes a package a directory whose
files share one namespace, so the file boundary this entry is about stops
existing — and the same repair is what a package boundary will need, one level
up. Two things are also not built, and neither is affected by that: **visibility**
(a struct whose fields are private to its declaring file can still be built from
another, because the ledger records no per-field `pub`), and the **alias**
`use pool as p`.

### 1.2. A bare word that is no construct becomes a different program — **fixed**

Two causes, and both are closed.

**A word keyword matched the beginning of a longer word**, because the grammar is
scannerless and nothing said where a word ends. Measured:

```text
true asfoo      ->  `true as foo`
returnx         ->  `return x`
forx in 0..3    ->  `for x in 0..3`
assert c        ->  `let c = true as sert; c;`
```

`forx` is the worst of them: a **valid program with a different meaning**, because
it binds `x` where the source says `forx`. Every word keyword now carries a
boundary (`parser::KW_*`), which is one rule per keyword because the generator has
no parameters — and UPPERCASE, which is not style: a lowercase rule is syntactic,
so the implicit whitespace would be inserted between the word and its boundary and
`as i32` would be refused for having a space in it.

**And a statement that is one undeclared name was accepted**, so what was left of
the class reached `rustc` about a file nobody wrote. It is `NK1117` now, in this
language's words. Four things count as declaring a name — a local or parameter in
scope, a function either ledger describes, a type declared here, and a module of
this program — and anything the compiler cannot see is a name it does not refuse,
because refusing a correct program is the one thing it may never do (Part III,
C.4).

One thing that had to be fixed to make the refusal safe: a `fn { … }` declares no
parameter list, so the checker walked its body with `a` in scope nowhere. It now
binds the automatic names from `emit::implicit_params` — the same answer the
emitter writes the parameter list from, so the two cannot disagree.

### 1.3. A type error exits through `anyhow`, and carries a backtrace — **fixed**

`Error: 1 type error` used to follow a clean diagnostic, and with
`RUST_BACKTRACE=1` in the environment — a normal thing for a developer to have set
— ten frames of `nikaia::project::check` and below came with it.
`diagnostics::Refused` marks a statement about the *program*; `main` prints one
without the `Error:` envelope and without the trace, and the tally stays. A parse
error travels the same path, which needed one more fix: `modules.rs` formatted the
error into a string to prefix the file name, losing the type that says it is a
refusal.

**A failure of this compiler keeps its trace**, deliberately, and a test asserts
that half too — otherwise the change would be indistinguishable from one that
swallowed everything.

### 1.4. A Rust warning about the generated file reaches the user — **fixed**

Both halves, because either alone leaves the hole. The emitted `use super::*`
carries `#[allow(unused_imports)]`, so the warning is not produced; and a
**warning** that maps to no Nikaia line is not reported at all, so the next
machine-written construct cannot do the same thing again. The import cannot simply
be left out where it looks unused — it also brings in the sibling modules, which
is how `utils::helper()` resolves.

An **error** with nowhere to put it is still reported. A warning suppressed costs
nothing; an error suppressed leaves a build that failed with no reason given
anywhere. Such an error is a defect in this compiler, and it should be visible.

### 1.5. A struct holding a view, across files — suspicion, no reproduction

Recorded earlier in this session as an `E0106` (a missing lifetime in the
generated Rust) from a struct that holds a `&str`. **The reproduction is not
re-established**: a struct with a view field, a struct holding such a struct, and
a public one declared in another module all lower with the lifetime inserted
correctly when tried again.

*What it needs:* find the shape or strike the entry. It stays only because a
lifetime defect in generated code is the class Part III C.1 is about, and
`docs/stored-views.md` is where the surrounding analysis is written down.

### 1.6. A `Shared` field's count is decided per file — **fixed, fail-closed**

Confirmed exactly as predicted, the moment §1.1 stopped hiding it: `Arc` in the
field and `Rc` in the value, in one generated file, refused by `rustc`.

The fix is the polarity this analysis already runs on, not a new answer: **where
it cannot prove that nothing crosses, it does not lower.** A field slot whose
struct this file does not declare is such a case, and it keeps the atomic floor as
`Fallback::ForeignField` — a seventh row in the enumeration `--sharing` prints,
because a fallback that is not named is one nobody can ask about.

It is forced in **one place**, after the walk and before the classes are read off,
rather than at each of the three sites that create a field slot: a rule that has
to be remembered at three sites is one that will be forgotten at the fourth.

The declaring file forces the same field itself where it is public
(`Fallback::PublicField`), so the two runs now agree by both refusing to lower.
Where the declaring file does *not* force it — a private type, a private field —
the other file cannot name the type either, so the caution costs nothing.

*What is still open:* where a field's count is **agreed** rather than
independently refused. That wants either a ledger column or an analysis that runs
over the whole program at once — and the package decision points at the second,
since the files of a package will be one namespace anyway.

### 1.7. The explain modes cannot be reached from a project build

`--sharing`, `--overlaps` and `--trust` exist on the single-file path only.
`nikaia build --sharing` answers `unexpected argument`.

`--sharing`'s own help says why it exists: there is no way to *ask* for the
cheaper count, every fallback is enumerated instead, and *"that is only fair if
the fallbacks can be asked about. This is the asking, and it is what a person
reads when they want the 9 ns back."* A person with a real program builds it with
`nikaia build`, so the asking is unavailable exactly where it would be done.

*What it needs:* the three flags on `build`, reporting over the project's files
rather than over one. Nothing about the analyses changes; they already run in that
path.

### 1.8. The project tests' cache accumulates and fills the disk

`target/nikaia-project-tests/` reached **13 GB** in one session, and
`~/.cache/nikaia` a further 3.7 GB, with the filesystem full and a build failing
on "no space left on device". Both are caches a test run creates and nothing
collects — the same class as the scratch roots `tests/common/mod.rs` now collects
by process liveness, and the fix is likely the same shape.

*What it needs:* a collection rule for the project tests' cache root. Deleting
both by hand freed 17 GB and nothing broke, so nothing in a later run depends on
what an earlier one left.

---

## 2. Decided and unbuilt

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
the generated file. The same Part III C.1 class as 1.2 and 1.4 above.

### 2.5. Part I 2.3's nullable types are a parse error

A trailing `?` on a type does not parse and `null` is read as an ordinary name, so
neither line of that section's own example is accepted. `??` (Part I 3.5) is built.

### 2.6. Part II 12.8's supervision syntax

`supervisor::start_link(fn { … }; restart_policy: …)` is specified and there is no
supervisor. Listed so it is not mistaken for something the `spawn` work includes —
it is not.

### 2.7. `isize` is checked and has no `truncating_` name

[ADR-043](specification/adr/adr-043.md) §4 records this on purpose: nothing in the
ledger hands back an `isize`, so no program can reach a conversion out of one, and
it is in the checked list anyway rather than left to truncate. The day something
produces one, the name comes with it. **No work until then** — the entry exists so
the asymmetry is not read as an oversight.

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

A notes page is a laboratory record and is allowed to be a snapshot — what it is
not allowed to do is read as current. A dated header on each is enough.

---

### 3.5. Part I 9.2 makes the file the boundary of privacy

[`open-decisions.md`](open-decisions.md) §6 moves it to the package: files of one
package see one another with no `use`, and `pub` publishes out of the package
rather than out of the file. 9.2's two bullets — *"only visible inside the file
they are defined in"* and *"only visible inside the file where the struct is
defined"* — are false under that answer, and 9.1's *"every file in Nikaia is
implicitly a Module"* needs the package above it. **Rewritten rather than
extended**, and the record that takes the decision is where it happens; the entry
exists so the sentences are not left standing while it is written.

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
