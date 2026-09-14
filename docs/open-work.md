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
twice, an undeclared name refused only where it stood alone, **a cast that could
name any type the language below has** - which is what `repeat(indent as usize)`
turned out to be, rather than the one open parameter it had been filed as
([ADR-054](specification/adr/adr-054.md)) - and **a relayed message naming a type
the program never wrote**, which wanted a rule and not a list
([ADR-056](specification/adr/adr-056.md)).

Each is in the CHANGELOG with what it
was and what fixed it; a fixed entry kept here only makes the list longer to
read.

**This section is empty again.** Two entries arrived while
[ADR-066](specification/adr/adr-066.md) was being built — not caused by it, which
is what writing programs against a construct does — and both left in the same
session, each by being answered rather than ruled on. A member reached off a `T?`
was emitted rather than refused, and what it needed was Part I 2.3 applied to a
position that had escaped it (ADR-066 D6). A value this checker could not type
got no `Some(…)` and no word, and what it needed was a wrap that is right in
both directions rather than a decision about which one it is
([ADR-068](specification/adr/adr-068.md)).

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

1. **The emitted Rust becomes `async`, and an executor to run it**
   ([ADR-055](specification/adr/adr-055.md)). Everything about tasks is inside
   this one item and in the order that record's §6 gives: the executor, then
   `async`/`.await` off the ledger's `sync` column, then `std`'s own pausing
   entries, then `spawn`, then `overlap { … }`. Five records were checked and
   could not run until it was done, which is the first principle above in its
   sharpest form — and for a long time it looked like one step rather than five,
   because a pause was a thread that blocked and nothing said so.

   **All five steps are built, at both settings**: the executor — one thread at
   `no` and a pool of futures at `yes` — `async fn` with `.await` off the
   ledger, `std`'s own pausing entries so a file operation suspends rather than
   blocking its thread, `spawn` itself with `TaskHandle`, `.join()` and
   `NK2101`, and `overlap { … }` with `NK2104`. **This item no longer blocks
   anything**, and nothing in the list waits on a thread any more.

   What is left of it is one refusal rather than one mechanism: §2 D6's `Send`
   is asked for by the pool's starter, so a task holding something that may not
   cross is refused by `rustc` about the generated file rather than by this
   compiler about the program. §2.2 below.
2. **The rules around the lock**, now that the lock itself is finished. The type,
   its constructor, its single spelling and all four doors are built
   ([ADR-057](specification/adr/adr-057.md),
   [ADR-059](specification/adr/adr-059.md),
   [ADR-064](specification/adr/adr-064.md)), and Part II 12.2's counter runs at
   both settings — so a program that spawns has something it may share, and this
   item stopped being about representation. [ADR-065](specification/adr/adr-065.md)
   then gave the transfer its door, so **nothing here needs deciding any more**:
   what is left is every **refusal** the section states and nothing raises.
   Independent of the sequence above, so it can be taken beside it.
3. **A server to bind to, and the `postgres` block.** Its own project rather than a
   step of this one.
4. **Supervision.** Last because nothing else waits on it.

### 2.1. `examples/foreign-runtime/` explains the boundary with a rule that is gone

Four programs about handing values across the foreign boundary — `crossing`,
`serve`, `shim`, `smuggled` — and their comments name the **per-build** expansion:
*"what `Shared` lowers to at `user_parallelism = no`, namely an `Rc`"*. That is
[ADR-037](specification/adr/adr-037.md) D3, which D6 replaced, and it is now wrong
a second way: [ADR-061](specification/adr/adr-061.md) D1 refuses a `Shared` at a
foreign destination outright.

*What it needs:* somebody to read the four and say which still demonstrate
something. This is where a reader goes to learn what crossing means, so a stale
explanation here is worth more than its size — and it is where the bridge D1
reserved would first be missed, if it is missed at all.

### 2.2. A task that may not cross a thread is refused by `rustc`, not by this compiler

[ADR-055](specification/adr/adr-055.md) §2 D6's third sharp edge, and the last
thing that record decided which the compiler does not do.

**The mechanism is built, and so is everything it was waiting on.** `spawn`
lowers at both settings; at `user_parallelism = yes` a task goes to `rt::pool`, a
queue of futures over the same worker count, and four tasks of the same size take
1.58 s at `no` against 0.65 s at `yes` on four cores. Part II 12.2's counter runs
on more than one core, and since [ADR-064](specification/adr/adr-064.md) gave the
lock its type, [ADR-045](specification/adr/adr-045.md) D2's lock in a task is a
program: four tasks bumping one `SharedMut[i64]` on four threads print every
increment (`a_lock_shared_by_four_tasks_counts_every_increment`).

**What is missing is the diagnostic.** The pool's starter asks for `Send`,
because a task may be polled on a thread that did not start it — so a task
holding something that may not cross is refused where it should be, in the
backend's words about the generated file, which [Part III C.1](specification/30-nikaia-tooling.md)
calls a bug in this compiler.

*What it needs:* the structural check [ADR-005](specification/adr/adr-005.md) §1
Group B already runs on what a task **captures**
([`contracts::send`](../crates/nikaia/src/contracts/send.rs), `NK2501` and
`NK2502`). What it has never been asked about is what a body holds **across a
pause** — a value bound inside the task, still live at an `.await` further down.
The checker knows where the suspension points are (`Checked::pausing_methods` and
the ledger's `sync` column), so the input exists and what is owed is the liveness
question between the two.

*Why it is here and not in §1:* nothing is miscompiled and no correct program is
refused. It is a message in the wrong words, which is the same class as every
entry this file has closed by moving a refusal from `rustc` into the compiler.

*Evidence, and it is that the gap is currently **unreachable** rather than
narrow.* The two shapes that could carry a non-`Send` future are chosen per value
by an inference that already sees the crossing, so at `yes` they come out
crossing-safe wherever a task can reach them: a `Shared[Note]` bound inside a
task body and held across a file read lowers to `std::sync::Arc`
([ADR-037](specification/adr/adr-037.md) D7), and a `SharedMut[i64]` to
`Arc<lock::Crossing<…>>` ([ADR-064](specification/adr/adr-064.md)). Both were
compiled and run to check it. Everything else a program can write — a number, a
`bool`, a `char`, a view, a struct of those — crosses anyway.

So no program this compiler can lower produces a task future that is not `Send`,
and **that is the reason to build the refusal now rather than a reason not to**:
a refusal costs nothing before there are programs it would reject, and the same
refusal added afterwards breaks them. What would reach it first is a type from
outside this language — a foreign value bound inside a task body and still live
at a suspension point, where `NK2501`'s capture check never looks.

**And [ADR-040](specification/adr/adr-040.md) D1's task half is closed rather
than waiting:** the analysis names a `spawn` body's handle as a duplication site,
`NK2101` is raised, and the exemption is held — a `Shared[T]` handed into a task
and used again afterwards is not refused, which `tasks.rs` says about a program
that does it.

### 2.3. A lambda that pauses is refused, and a recursive pausing method is not boxed

Both are [ADR-055](specification/adr/adr-055.md) §6's remainder, and both are
limits of this compiler rather than of the language — so they are here and not in
§1, where a defect is the compiler being *wrong*.

**A lambda whose body calls something that can pause is refused at the build.**
Rust has no stable `async` closure, so the lowering has nothing to write. The
refusal is by the lowering and not by the checker on purpose: refusing it in the
type checker would refuse a correct program (Part III, C.4).

*Evidence:* `examples/fortunes.nika:120` — `.route("/fortunes") fn { fortunes(db) }`,
a route handler that queries a database. It type-checks clean and no build reaches
it, because the server it binds to does not exist yet - the `fortunes.nika`
entry below. So today
nothing in the repository meets the refusal, and the first program that does will
be the one that binds a handler.

*What it needs:* a `std` signature that says a parameter takes something which
may pause, so a lambda handed to one can be written as a closure that **returns**
a future — `|| async move { … }`, which is stable Rust and is how a handler is
taken in practice. Step 3 showed the shape works: `task::interleave` takes
futures where `task::both` takes closures, and the emitter chooses between them
per group. What is left is that a *callee's* parameter has to say which it wants,
and the ledger has no column for it. Not a new mechanism — a claim to record.

**A recursive pausing *method* is not boxed.** §6 step 2 boxes a call that closes
a cycle of pausing functions, and it resolves a callee's name the way the emitter
resolves anything — which is not at all for a method, because `stats.add(5)` names
`add` and only the type checker knows what it goes to ([ADR-028](specification/adr/adr-028.md)).
So a cycle through a method reaches `rustc` as *"recursion in an async fn requires
boxing"*, about the generated file.

*Evidence: none.* No program in the repository has one — `examples/json.nika`'s
recursion is through free functions, which is the case that is built. It is
written down because it is the same edge one step further in, not because
something failed.

*What it needs:* the checker already hands the emitter *whether* a method call
pauses, keyed by statement and name (`Checked::pausing_methods`). A third set
keyed the same way, saying whether it also closes a cycle, is the same shape
again — the checker has the resolved call graph that `contracts::sync` builds.

### 2.4. `let` takes one name, and the specification writes it taking several

```nika
let (user, rights, prefs) = overlap { … }          // Part I 8.1.2
let (tx, rx) = channel::bounded(100)               // Part II 12.5
```

Neither parses. `let` takes **one** name, and a destructuring `let` is a form the
specification uses twice, for two different constructs, and defines nowhere —
Part I 2.1 introduces `let` with a name and says nothing about a pattern.

*Evidence:* both lines above, in the specification. `overlap { … }` met this
rather than made it: the construct is built and is reached by its tuple in the
meantime (`let r = overlap { … }`, then `r.0`), which works and reads worse than
what the page promises.

*What it needs:* a flat tuple of names is all either site writes, so that is the
whole of the work — `Stmt::Let`'s single `Ident` becomes several, and the twenty
places that read it are made to look. Nesting and `_` are written nowhere and
should be **refused with a sentence** rather than quietly accepted, which is this
compiler's rule for a form nobody decided.

*What it is not:* a pattern language. `match` has patterns already and this is
not them; what the two sites need is destructuring a tuple whose arity is known,
and a bigger answer would be a decision rather than this repair.

### 2.5. Standard input is `async` and does not suspend

[ADR-055](specification/adr/adr-055.md) §6 step 3 made every pausing `std` entry
an `async fn`, and made **files** actually suspend: a read is a slot on the ring
or a worker's reply, and `exec::block_on` is the only place a program parks. A
read of standard input is not. `io::read`, `io::read_to_string` and `io::lines`
are `async fn` whose bodies are the blocking read they always were, so they
finish on their first poll.

That is [ADR-038](specification/adr/adr-038.md) D3's own split rather than
something the step left half done: its completion mechanism serves files, and a
stream needs the readiness half — which is built (`rt::io::wait`, for sockets)
and not wired to standard input.

*Evidence: none, and none is possible yet.* A caller sees a read that returns,
which is what it saw before, so no program behaves differently. What is missing
is only that the thread is **held** rather than given up for the duration of
`for line in io::lines()` — which nothing can observe until something else wants
the thread. Something can now: at `user_parallelism = yes` a task is on a thread
of its own, so a `main` blocked in `io::lines()` is a thread the pool could have
had.

*What it needs:* `Op::Readiness` against standard input's descriptor, and a
`Lines` whose step is a future. The second half is the larger one and is a
question of its own: a `for` over a **stream** is `while let Some(x) =
s.next().await` in the language below, and Rust has no stable trait for one. The
parallel is [ADR-025](specification/adr/adr-025.md) D6's `iterates_fallibly` — a
property of the *type*, recorded in the ledger, that makes the emitter write the
step differently — so the shape to copy exists.

### 2.6. The lock is built and every rule around it is not

[ADR-057](specification/adr/adr-057.md) decided what the lock **is**,
[ADR-059](specification/adr/adr-059.md) what a program writes to reach one, and
[ADR-064](specification/adr/adr-064.md) gave the shared mutable type its name, its
constructor and its single spelling. **Part II 12.2's counter compiles and runs at
both settings.** So the type is no longer what anything here waits on — what is
left is the section's own rules, every one of which is a refusal nothing raises:

* the **lock-touching** derived property (ADR-039 D3, D7): no function carries it,
  so nothing tells a spawned body from a scope's;
* the **re-entrancy check as a build switch** (ADR-039 D8), which the cache key
  already accounts for — and which ADR-057 D2 makes free at one thread and D3
  charges only on the values that actually cross;
* `NK2201`–`NK2205` and `NK2503`, catalogued and not emitted.

### 2.7. Part II 12.8's supervision syntax

`supervisor::start_link(fn { … }; restart_policy: …)` is specified and there is no
supervisor. Listed so it is not mistaken for something the `spawn` work includes —
it is not.

### 2.8. `fortunes.nika` waits on two runtime pieces, and neither is a language question

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

### 2.9. There is no HTTP server, and three records now wait on it

[ADR-038](specification/adr/adr-038.md) §4.5. Its D3, D4 and D5 are built — the
runtime is running before `main`, files complete on `io_uring`, sockets signal
readiness — and [ADR-055](specification/adr/adr-055.md) has since put an executor
on top of them at `user_parallelism = no`, so a task can pause and another can
run. **D1's server, D2's `rustls` and D6's HTTP/1.1 parser are untouched**, and
the executor does not change that: what is missing is not somewhere for a
handler to run, it is a socket to run it for. The order that record gives is unchanged: a socket layer that keeps
registrations rather than answering one readiness question at a time, then a
minimal HTTP/1.1 server on it, then the parsing moved into Nikaia, then `rustls`,
then HTTP/2. The first step is the blocker; `worker::poll_one` builds a poller per
wait today.

What waits inside it:

* [ADR-018](specification/adr/adr-018.md) entire — what a handler sees and what it
  returns is specified and has nowhere to run;
* [ADR-058](specification/adr/adr-058.md) D1's `Bytes` body row, D2's `http::File`,
  D3's mechanism choice and D8's kept mappings, all of which are things to build
  *on* a server ([#45](https://github.com/keywan-ghadami/Nikaia/pull/45));
* [`project_status_and_roadmap.md`](project_status_and_roadmap.md) Phase 3's route
  hashing, which says in as many words that it has no target because there is no
  server.

**One piece does not wait, and it is the one worth building first.**
[ADR-058](specification/adr/adr-058.md) D7 — a path out of a request is
`Untrusted` and may not reach `fs::map`, `fs::read`, `fs::write` or `http::File`
unchecked — needs no socket. `contracts::trust` exists and `nikaia --trust` prints
what it found ([ADR-010](specification/adr/adr-010.md) D7); what is missing is the
consumer, a diagnostic where an untrusted value reaches a path parameter, and
`fs::within(root, name)` beside it. Testable against `fs::map` today, and every
program that later writes `http::File` inherits it.

**It is also the only entry in this section that closes a security hole rather
than an ergonomic one**, which is worth saying where a reader chooses what to
pick up. [ADR-010](specification/adr/adr-010.md) D8 built a taint lattice and
argued for building it generally rather than as a hasher special case; its
second consumer is the test of whether that generalised, and it costs no new
analysis. Everything else here is a feature nobody can use yet — this one is
absent from every program written before it lands.

Nothing of [ADR-058](specification/adr/adr-058.md) is built. What is built is the
bench that decided it (`benches/sendfile/`) and the write-up
([`zero-copy-send.md`](zero-copy-send.md)); `send_file` beside the ring could have
been built ahead of the server and deliberately was not, because D3's measurement
makes it the mechanism that loses at the sizes a server sends most.

### 2.10. There is no target that lets foreign code call in, and the record for one is written

[ADR-062](specification/adr/adr-062.md). Nothing of it is built and nothing of it
**can** be: `extern "C"` is a parse error (Part III 15.1), `Target` has two values,
and this repository has no notion of a linkable artifact — no `cdylib`, no
`staticlib`, no `.so`. So this entry is not in the ordered list above; it is not
waiting its turn, it is waiting on scope
([`project_status_and_roadmap.md`](project_status_and_roadmap.md)).

*Why it is written down anyway:* it is the one direction that touches
`user_parallelism` at its root. A target added without it answers "who owns the
threads" by accident, and the answer is invisible — a caller's second thread and a
plain reference count under it is a data race with nothing to see at the source.

*What it needs, when something asks for it:* the analysis takes the exported entry
points as roots seeded at the floor, the way it already seeds crossing roots. The
checks need nothing — [ADR-045](specification/adr/adr-045.md) D1 kept every verdict
off the switch, so a library is already checked for the world it would enter.

### 2.11. A type nothing declares goes into the language below untranslated

*Reproduced:* `let counter: SharedMut[i64] = 0` used to emit `SharedMut<i64>` and
come back as `rustc`'s *"cannot find type `SharedMut` in this scope"* — about a
name the program did write and Part I 6.2 does promise.
[ADR-064](specification/adr/adr-064.md) fixed that name by building it; **the
class is untouched.** Every misspelled type takes the same route.

*Why it is a defect and not a gap:* a **value** nothing declares has had `NK1117`
since [ADR-051](specification/adr/adr-051.md) — *"nothing declares `q`"*. A type
has nothing, so Part III C.1's rule holds for one half of the language's names and
not the other.

*What it needs, and the reason it is not free:* the set of names that count as
declared — the writable types of Part I 2.2, the three hulls, `std`'s, this
package's structs and enums, a function's type parameters, `Self`. Getting that
set wrong refuses a **correct** program, which is the one thing the checker may
never do (Part III, C.4), so it wants the same polarity every other check here
has: refuse only what is certainly wrong, and say nothing about a name it cannot
account for.

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

### 3.5. Two examples write a postfix `??` the language does not have

Part I 3.5 defines `??` as **null coalescing** — `a ?? b`, a fallback when the
left side is null — and nothing else. There is no postfix unwrap in that section,
in the parser, or anywhere the specification states a rule. But two examples use
one:

* [ADR-018](specification/adr/adr-018.md) D3: `lookup(a.query("id")??)`
* Part III 17.1, in the same shape, copied from it

Measured, on the form reduced to one line:

```
q.nika: Parse error:
expected expression; found unexpected token `)` at line 3, column 22
   3 |     return lookup(q??)
                            ^
note: also possible here: `"`, `&`, `'`, `(`, `//`, `f"`, `if`, `match`, `seq`, `{`, digits, identifier
```

This is not a rule specified ahead of the compiler — those carry a **Status**
note and this has none. It is an example using a construct the language never
defined, which is worse: a reader who copies it gets a parse error with nothing
to look up, because the section it would be defined in does not mention it.

*What it needs is a decision before any work:* whether the nullable gets a
postfix unwrap at all. If it does, Part I 3.5 gains it and the parser follows —
and it wants a name for what it does when the value **is** null, which for an
abort is Part III A.2's territory. If it does not, the two examples are rewritten
to use what 3.5 has. Part I 2.3's nullable types are themselves a parse error
(§2.5), so nothing can be written either way yet, and that is the reason this is
upkeep rather than a defect: no program is wrong today, only the page is.

The Part III 17.1 example was rewritten while this was found; ADR-018's stands,
because an ADR is written once and the correction belongs to whatever answers
the question above.

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
