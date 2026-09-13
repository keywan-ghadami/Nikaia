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

### 1.1. A module can hand out functions, but not types

Two files, `src/pool.nika` and `src/main.nika`:

```nika
// pool.nika
pub struct Conn { pub id: i64 }
pub fn make() -> Conn { return Conn(id: 7) }
```

**The qualified call works.** This builds and prints `7`:

```nika
use pool
fn main() {
    let c = pool::make()
    println(f"{c.id}")
}
```

**The qualified type name does not resolve to the same type.** Measured:

```
error[NK1103]: this is `Conn`, and the `let` says `pool::Conn`
     help: make it a `pool::Conn`, or change what is declared to `Conn`
```

for `let c: pool::Conn = pool::make()`. `pool::Conn` is treated as a different
type from the one `pool::make()` hands back, so the help line asks for exactly
what is already written — it cannot be followed.

**And a struct from another module cannot be built.** Not even the parser takes
it:

```
Parse error: expected one of: `)`, `,`; found unexpected token `:`
   4 |     let c = pool::Conn(id: 1)
                                ^
```

`pool::Conn(...)` is read as a call rather than as a struct literal. Unqualified
`Conn(id: 1)` does not find the name, which Part I 9.1 is right about: a module is
reached through its name.

**Together:** a module can export behaviour and not data. Whoever wants to split a
program can move functions and not structures — which is most of the reason to
split one.

*What it needs:* name resolution has to strip a module prefix before comparing
two types (ADR-011 D2's name-for-name rule, applied to a type rather than to a
function), and the struct-literal form has to tolerate a qualified name. The
second is parser work; the first is one place in `check`. See
[`open-decisions.md`](open-decisions.md) §1, now answered: `use pool` makes a
module reachable and nothing more, `use pool as p` is added, and every form that
brings a name in — a glob, a braced list, a single name — is refused. The scope of
this repair is therefore the two pieces above plus the alias: no import form is
being built, and the answer changed nothing else about what is needed here.

And the parser half is not the whole of it: once a qualified struct literal
parses, **visibility decides whether it is allowed**. A struct whose fields are
private to the file that declares it may not be built from another one, and the
refusal should name the module's own functions rather than only saying no. Part I
9.2 makes private fields the ordinary case, which is the encapsulation this change
must not spend on its way to making types usable.

### 1.2. A bare word that is no construct becomes a different program

The worst class in this file, because nothing is refused. Source:

```nika
fn main() {
    let c = true
    assert c
    unsafe { println("x") }
}
```

lowers, with no diagnostic, to:

```rust
let c = true as sert;
c;
unsafe;
{ println!("x") }
```

`assert` is swallowed into an `as` cast that takes `sert` for a type name;
`unsafe` becomes a statement of its own and its block an unrelated one. And a
number with a separator:

```nika
let n = 1_000
```

becomes

```rust
let n = 1;
_000;
```

Part I 2.2 is deliberate about the last one — *"`1_000`, `0xFF` and `1i64` are
each not a number but a number beside a name"* — so the **reading** is as
specified. What is not specified is that the program is then accepted: a bare
name that nothing declares should be refused here, by this compiler. Today it
reaches `rustc`, which rejects it about a file nobody wrote (Part III C.1).

*What it needs:* an expression statement that is a name nothing declares is a
refusal, with a `NK1xxx` code. That single rule covers all three shapes above and
every other bare word.

### 1.3. A type error exits through `anyhow`, and carries a backtrace where one is asked for

After a clean diagnostic, the user gets one more line:

```
error[NK1103]: this is `Conn`, and the `let` says `pool::Conn`
     help: make it a `pool::Conn`, or change what is declared to `Conn`
Error: 1 type error
```

`Error: 1 type error` is the compiler's error type reaching the terminal after the
diagnostics have already said it, and with `RUST_BACKTRACE=1` in the environment —
which is a normal thing for a developer to have set — it brings a stack trace of
this compiler's own functions (`nikaia::project::check`, `nikaia::main`, and eight
frames below). Measured: without the variable it is the one redundant line, with
it the trace as well.

*What it needs:* a refused check exits with a status and no second message.
Nothing about the compiler's internals belongs on that path.

### 1.4. A Rust warning about the generated file reaches the user

Every multi-file build prints:

```
warning: …/target/nikaia/gen:7: unused import: `super::*` (no Nikaia source maps to this)
     = remove the whole `use` item
```

Two defects in one line. The `use super::*` is **the emitter's own**, not the
user's, so the warning is about a decision the user did not make — and the
translation says so itself, *"no Nikaia source maps to this"*, and passes it on
anyway. Part III C.1 forbids exactly this.

*What it needs:* a diagnostic that maps to no Nikaia line is dropped rather than
printed with a note saying it maps to none — and separately, the emitter should
not write an import it does not need.

### 1.5. A struct holding a view, across files — suspicion, no reproduction

Recorded earlier in this session as an `E0106` (a missing lifetime in the
generated Rust) from a struct that holds a `&str`. **The reproduction is not
re-established**: a struct with a view field, a struct holding such a struct, and
a public one declared in another module all lower with the lifetime inserted
correctly when tried again.

*What it needs:* find the shape or strike the entry. It stays only because a
lifetime defect in generated code is the class Part III C.1 is about, and
`docs/stored-views.md` is where the surrounding analysis is written down.

### 1.6. A `Shared` field's count is decided per file, and §1.1 is hiding it

Which reference count a value gets is an optimisation over an atomic floor, and
it may only ever lower where it *proves* nothing crosses a thread
([ADR-037](specification/adr/adr-037.md) D7). A public field of a public type is
one of the enumerated fallbacks, so within a file the field forces the atomic
count and the value flowing into it is pulled along with it. Measured, one file:

```nika
pub struct Pool { pub db: Shared[Conn] }
let c: Shared[Conn] = Conn(id: 1)
let p = Pool(db: c)
```

```rust
pub db: std::sync::Arc<Conn>,
let c: std::sync::Arc<Conn> = std::sync::Arc::new(Conn { id: 1 });
```

Both atomic. Correct — the constraint propagates from the field to the value.

**Split over two files, the two answers diverge.** `Pool` in `pool.nika`, the
value in `main.nika`, and the generated Rust carries both:

```rust
pub struct Pool {
    pub db: std::sync::Arc<Conn>,      // decided in pool.nika
}
    let c: std::rc::Rc<Conn> = ...      // decided in main.nika
```

The second run never sees the field, so nothing forces it and it lowers. A field's
emitted type is derived from the values this run watched, and there is no ledger
column in which the two runs could agree — `FnContract::sharing` covers parameters
and `<result>`, and a field has no slot.

**This entry's point is the sequencing.** The build stops at §1.1's name
resolution long before `rustc` sees either type, so the divergence is inert today.
**Fixing §1.1 uncovers it**, and what a user would then get is a type mismatch
reported about a generated file — the class Part III C.1 forbids. The two belong
in one piece of work, not one after the other.

*What it needs:* a decision on where a field's count is agreed, and until then
the polarity this analysis already states — where it cannot prove, it does not
lower. A field whose count no run can see in full is such a case.

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
