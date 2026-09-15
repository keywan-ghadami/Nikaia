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
([ADR-056](specification/adr/adr-056.md)), and **a generic function that lowered
without its type parameters** - which turned out to be three defects behind one
reproduction, because writing the `<T>` closes only the first of them
([ADR-074](specification/adr/adr-074.md)), **a trait whose method pauses** and
**a map whose key type the language below could not work out** - the second being
Part I 4.5's own three-line example, which failed in a message naming this
compiler's internal word for a map ([ADR-080](specification/adr/adr-080.md)),
and **three of the four string concatenations** - which also took back a
*false* refusal, since `"a" + s` was typed as a `&str` and made `-> String` an
error ([ADR-081](specification/adr/adr-081.md)), and **a field of a borrowed
subject handed out by value** - which Part I 6.8 had already decided in its own
words, by promising that an ownership rule rejects in plain language and that a
raw internal error reaching the user is a bug
([ADR-083](specification/adr/adr-083.md)), and **a `rustc` warning on a `while
true`**, which arrived the day [ADR-084](specification/adr/adr-084.md) made the
shape writable and left the same day: the fix named for it was a line in the
emitted preamble, and the right one was to stop emitting the shape `rustc` was
right about ([ADR-085](specification/adr/adr-085.md)), and **a statement that
wrapped one call's argument with another call's answer** - `let r = pick(1) +
pick(null)` came out `pick(Some(1)) + pick(Some(None))`, because Part I 2.3's
wrap was keyed by the statement, the callee and the position, which name a
*parameter* rather than a *call*; the same design and the same defect in a
struct literal, where a statement may build two. **Filed here with the wrong
characterisation**, which is worth keeping visible:
[ADR-087](specification/adr/adr-087.md) §3 reported it as *"a `null` in the
second hole of an `f"…"`"*, because that is where it was met - and the string
had nothing to do with it. The entry said the condition was *"two holes with a
non-`null` in the first"*; the condition was two calls to one callee in one
statement, which a probe two lines longer would have found. **A narrow
characterisation is a guess about the cause wearing the clothes of a
reproduction** - and **`&&` and `||` missing from the head of an `if`, a
`while` and a `for`** - found by writing the loop a language without `break`
has to write, and closed by the rule that a head parses the same language as a
body minus the forms a `{` begins ([ADR-086](specification/adr/adr-086.md)).
That one also corrected a number: the shape it made writable turns out to cost
exactly what a `break` costs, so [ADR-084](specification/adr/adr-084.md) §4's
`while` row had been measuring this gap rather than the jump.

and **a `??` whose fallback reached rightwards across every
operator**, which was filed as a suspicion with the exact question that would
decide it and turned out to be a silent wrong value
([ADR-089](specification/adr/adr-089.md)). And **a `catch` that ignored the error
warning about a name the emitter wrote** — `catch { 1000 }` got
*"unused variable: `error`"* about a binding that exists nowhere in the program,
and the walk that answers *does this handler mention the name* was already
there, doing the ordering ([ADR-090](specification/adr/adr-090.md)). Its
over-approximation is what made it usable: the two failure modes are not
symmetric, so a walk that only has to **lean** did work a precise analysis would
have had to be **right** for. That round's own fixture found the next
one and closed it too: **a `catch` over an expression that cannot fail** lowered
to a `match` over something that is not a `Result`, and `NK1134` now says so
([ADR-091](specification/adr/adr-091.md)). Its polarity is the same shape as the
one above and had to be split into two answers rather than one — *nothing here
can fail* and *nothing here could be looked up* — and the **corpus** is what
proved that: six examples write `dsl … from … catch { … }`, which can fail by
[ADR-023](specification/adr/adr-023.md) D9 and carries no contract to say so,
and all six were refused the first time the refusal ran over `examples/`.

Each is in the CHANGELOG with what it
was and what fixed it; a fixed entry kept here only makes the list longer to
read.

**Empty, and the entry that was here was wrong on both of its claims.** It said an
accessor cannot hand back a **view** of a field and that the lowering names no
lifetime. Neither is true, and both were checkable in a minute:

```nika
fn name_of(&self) -> &str { return &self.name }   // compiles and runs
fn tags(&self) -> &Vec[i64] { return &self.tags } // so does this
```

The spelling is the `&` [Part I 6.5](specification/10-nikaia-light.md) writes in
its own example (`let name = &config.name`), and `NK1104`'s help had been saying
*"write `&` to take a view of it"* the whole time. What I had actually found was
`return self.name` — without the `&` — declared `-> &str`, which is refused
because a `String` is not a `&str`, everywhere and not only in that position: the
same program is `NK1102` at a parameter and `NK1103` at a `let`. The rule is
uniform and the way out is written.

The `E0106` the entry also claimed was an artefact of my own scratch directory: a
refused program writes **no file**, so `rustc` was reading a stale one from an
earlier probe.

What was real and is fixed: `NK1131` named two ways out and not the free one.
Its help now leads with the view — *declare the result `&str` and write
`return &self.name`* — and names the type the field actually has, so it is right
for a `Vec` field as well as for text.

Everything this section has held was found the same way — by running the programs the specification prints,
which is `crates/nikaia/tests/specification.rs` now rather than a habit: it takes
every `nika` block in the three pages as far as it goes and hands the ones that
lower to `rustc`, against two recorded baselines. Of 127 blocks, 53 are programs
this compiler takes and 31 of those compile below.

What left: a trait whose method **pauses** is `NK1129`, an `impl` that disagrees
with its trait about which methods exist is `NK1130`, Part I 4.5's map example
**compiles** ([ADR-080](specification/adr/adr-080.md)), and three of the four
string concatenations are programs rather than errors from the language below
([ADR-081](specification/adr/adr-081.md)). The last of those closed the way its
entry said it could **not**: the entry conceded an allocation to `format!` and
measurement said the trait is free — 42 ms against 250 — so the rule turned out
to be the table of four cases rather than a replacement for it.

The sweep also turned up two things that were on the **page** rather than in the
compiler, both fixed: Part I 4.7's body was refused twice over, and three plain
strings in Part I 7 held holes that
[ADR-035](specification/adr/adr-035.md) D5 made into text. A page can be wrong in
a way nothing notices, and `docs/README.md` §1's rule about a stale **Status**
note turns out to apply to the code beside it just as much.

**This section is empty**, for the first time, and that is a statement about
what to do next rather than a victory lap: the list below is decided-and-unbuilt
and upkeep, and neither outranks a defect — so the next defect anybody finds
goes here and goes first. The head of this file says what a defect is; the way
they have been found, every round, is by **running programs** — the
specification's, the corpus's, and the one somebody wrote while building a
fixture for something else.

The last one out was **a grammar fold's `init`, `step` and `merge`, which
nothing checked at all** ([ADR-092](specification/adr/adr-092.md)). It had been
sitting behind a question rather than behind work:
[ADR-084](specification/adr/adr-084.md) D4 closed the half a **jump** can reach
and stopped, because walking those bodies with the whole checker *might newly
refuse programs for reasons that have nothing to do with the construct*. That is
a thing to measure, and measuring it took one afternoon and refused nothing — so
the walk D4 wrote could be **deleted** rather than kept beside the new one, and
the jump's message got better for it. **A question that can be answered by
running the corpus is not a reason to leave a defect open**, and this one had
been open since the record that named it.


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
* `NK2201`, `NK2203`, `NK2204`, `NK2205` and `NK2503`, catalogued and not
  emitted. **`NK2202` is the exception and is raised** — `sync::check` finds the
  calls that contradict an assertion and `diagnostics` renders them — which is
  worth the extra words, because the bullet used to name the whole range and a
  reader would have gone looking for work that is done.

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

### 2.12. A loop that cannot end still has to be followed by a `return`

*Reproduced:*

```nika
fn forever() -> i32 {
    while true {
        let x = 1
    }
}
```

```
error[NK1104]: this function hands back `()`, and it declares `i32`
```

so a function that genuinely never returns — an accept loop, an event loop, a
supervisor — has to end with a `return 0` that cannot be reached, and a reader of
that line cannot tell dead code from a mistake.

*Why it is work and not a question:*
[ADR-070](specification/adr/adr-070.md) D3 decided it. It is also the one real
cost of D1's *no second keyword*, which is why the two were settled together.

*What it needs, and what it did not need before
[ADR-084](specification/adr/adr-084.md):* a `while` whose condition is the
literal `true` can be left in exactly two ways — a `return`, and a `break` bound
to **this** loop. So the test is *"the condition is the literal `true` **and** no
`break` in the body is bound to this loop"*, which is a walk of the body rather
than a look at the head.

It is still small, and smaller than it sounds: the checker already keeps the loop
count that answers it ([ADR-073](specification/adr/adr-073.md) D4's boundaries),
so what this needs is to record, per loop, whether a jump reached it — not a new
analysis.

*And the half that was not the checker's is now in place.* This could not have
been built at all while the lowering emitted `while true`, whatever the checker
did: `while true { }` is `()` in the language below and `loop { }` is `!`, so a
function whose body is an unconditional loop and whose declared type is `i32` was
an `E0308` there. The checker would have stopped refusing and `rustc` would have
refused instead, about a file nobody wrote.
[ADR-074](specification/adr/adr-074.md) D1 emits `loop`, so what is left here is
the checker's half alone.

*What this entry said before, and why the correction is written rather than
edited away:* the version before ADR-073 argued the work was small **because
`break` and `continue` do not exist**, so *"the analysis other languages need for
this question is, here, one test on the condition."* That was true and is not,
and it is the kind of sentence that stays quoted long after its premise leaves.

The polarity is unchanged and is the usual one: say *"cannot be reached"* only
where the condition is the literal and no jump leaves it, never where the
condition is a name that happens to be true.

### 2.13. A `comptime` binding at item level has nowhere to stand

*Reproduced:* `comptime MAX = 1000` at the top of a file is a parse error; the same
line inside a function body parses, folds and runs.

*Why it is work and not a question:*
[ADR-073](specification/adr/adr-073.md) D2 decided both places, and Part I 9.2
already lists **Constants** among the items `pub` applies to - so the rule for a
constant another package may read is written and the syntax for one is not.

*What it needs, and why it is more than a grammar rule:* a name at item level is
in scope for **every** function in the package, and this checker's scope is a
stack pushed per function. So the item form needs a frame under all of them,
filled before any body is walked - which is also where `pub` would be checked,
since a constant reached from another package is [ADR-047](specification/adr/adr-047.md)
D2's question rather than a new one. The emitter needs the same table it already
reads for the body form, keyed the same way.

*What it does **not** need:* any decision. The evaluator, the refusal and the
type spelling are the body form's and are built, and what a result may *be* when
it crosses is [ADR-079](specification/adr/adr-079.md)'s.

*How it came back:* it was written with the body form, dropped from this file by
a restructure that removed the entry above it, and restored from its own commit.
Nothing about it changed in between.

### 2.14. Nothing runs Nikaia code while the program is built

*Reproduced:* `comptime` is built and its evaluator is
[`crates/nikaia/src/fold.rs`](../crates/nikaia/src/fold.rs) — **124 lines**, and
what it knows is an integer literal, a name whose value already folded, a
negation, and `+ - * / %`. No call, no loop, no text, no aggregate.

*Why it is work and not a question:* three records decided what may happen and
none of them can happen.

| record | what it wants of the evaluator |
| :--- | :--- |
| [ADR-073](specification/adr/adr-073.md) D5 | a **call** in an initialiser, which is what [ADR-072](specification/adr/adr-072.md)'s file reading waits behind |
| [ADR-079](specification/adr/adr-079.md) §3 | a **loop and `push`**, to build a table that then crosses as a view |
| [ADR-088](specification/adr/adr-088.md) D1 | a **loop over a type's fields**, which is the whole of 10.3 |

[ADR-079](specification/adr/adr-079.md) §3 says it plainly — *"This is the real
work behind the feature, and this record does not shorten it"* — and until this
entry existed, that sentence was the only place in the repository where the work
was named. Three records waiting on something the work list does not mention is
how a thing stays unstarted.

*What bounds it, and it is already decided:*
[ADR-075](specification/adr/adr-075.md) D1 and D2 — a body it may evaluate is
`sync` and touches at most the build's own parameters. So this is an interpreter
for a **restricted** language, not for all of Nikaia, and what it must refuse is
written down rather than invented here. There is deliberately **no step budget**
(D4), so a body that does not terminate hangs the build: worth knowing before
starting, and not a reason to add one on the way past.

*The order the records imply:* the **call** first, because it alone unlocks
reading a file at build time and is the smallest of the three. Then **loop and
`push`**. The field walk last, because it needs something the other two do not —
see 2.15.

*What it does **not** include:* running a **grammar**. That looks like the same
job and is not; it is 2.15's, and the reason is there.

### 2.15. Running a grammar while the program is built is not interpretation

*The distinction, because it is the whole entry.* A grammar could be run at build
time by interpreting the grammar tree the compiler already holds. **It must not
be**, and the reason is not the size of the work.

`winnow-grammar` is a code **generator**: its model crate parses the grammar
language, validates and analyses it, and hands the result to a macro that writes
a parser. Its own summary says so — *"intended to be used by procedural macros
that generate parsers"*. There is no interpreter in it to borrow.

So interpreting a grammar here would be a **second implementation of the same
semantics**, and the two would have to agree exactly. Part II 10.2 promises that
one grammar means the same thing at both stages; with two implementations that
stops being a property and becomes a hope. The disagreements would land in the
corners — implicit whitespace, repetition bounds, the commit point, frames and
resynchronisation, interning, spans — and would present as *"this file parsed
while the program was built and fails while it runs"*, for the same file and the
same grammar.

*What to do instead:* compile the **generated** parser during the build and run
it. Then there is one implementation and the agreement is a tautology rather than
a claim. The cost is a second compilation, which
[ADR-026](specification/adr/adr-026.md) Q4 named — and which the build cache
turns from *every build* into *when the grammar changes*, since
[ADR-021](specification/adr/adr-021.md) keys on the source. The compiler already
emits Rust and already drives Cargo, so the machinery is not new.

*Why this needs no new security model:* a grammar's action blocks are Nikaia, and
[ADR-075](specification/adr/adr-075.md) already says what a build-time body may
do. Running a generated parser is covered by the same rule as any other
build-time call.

*What it unblocks:* `comptime CONFIG = Config.value(from "config.toml")` — the
case [ADR-082](specification/adr/adr-082.md) rewrote the syntax for and
[ADR-072](specification/adr/adr-072.md) built the permission for.

---

## 3. Upkeep

### 3.1. A whole-workspace test run sometimes fails the project tests, and the wrapper's stdin is the suspect

```text
error: failed to run `rustc` to learn about target-specific information
  process didn't exit successfully: `target/release/nikaia …/rustc - --crate-name ___
  --print=file-names … --print=cfg` (exit status: 1)
```

Sixteen of the twenty-two tests in `crates/nikaia/tests/project.rs`, every one of
them at the point where a nested `cargo` probes `rustc` **through this compiler's
wrapper** (`project::wrapper_main`, where the invocation names no `.nika` source
and is passed straight through). The stdout that reaches the error is the probe's
own output cut off partway down the `--print` list.

**It still does not reproduce on demand, so this stays a suspicion** — the head
of this file makes the difference load-bearing. What it now has is a mechanism
worth testing, which it did not before. What
it looked like at the time: `cargo test --workspace --release` failing 16 of 22
on `origin/main` with nothing applied, three runs out of three, while `cargo test
-p nikaia --test project` on the same commit passed 22 of 22 four times out of
four. Under the same command since: **seven consecutive clean whole-workspace
runs**, two of them with a full rebuild immediately before in the same
invocation, which was the best hypothesis and is now ruled out.

**Measured again, and the hypothesis this entry carried is refuted.** It said
the wrapper's inherited **stdin** was shared between concurrent probes. Running
the whole test with `cargo test … < /dev/null` fails identically, three runs out
of three, so nothing about the caller's stdin is what decides it.

**What the failing runs look like now**, and it is sharper than before: the
probe's `--print` output arrives **complete**, and what fails is the compile of
the source on stdin, because the source is this:

```text
error: unknown start of token: `
 --> <anon>:7:55
7 |   = help: only literals are allowed as values for the `message`, `note`
  |           and `label` options. These options must be separated by a comma
```

**It is line 7 of the input**, and lines 1 to 6 are the rest of a rendered
diagnostic — an `error:` line, a `-->` line, a caret line. So what is on stdin is
not a truncated source or a stray fragment: it is **another process's rendered
stderr**, whole. That names the mechanism as two streams crossing rather than one
being cut short, which is what the first two sightings looked like.

The diagnostic itself is `rustc`'s own, about a malformed
`#[diagnostic::on_unimplemented]`. Nothing in this repository writes that
attribute — searched, and the only hits are this file and the CHANGELOG quoting
it — so it is not something this compiler wrote or relayed. The producing crate
is **not** identified: `serde` carries the attribute in the dependency graph, but
in the form this toolchain accepts (`message = "…"` with a literal), so it is not
the one complaining.

**And it clusters in time rather than in the code.** On one commit, seven
consecutive whole-suite runs passed — two of them immediately after a full
rebuild — and then eight consecutive runs failed. In the failing window a clean
`origin/main`, built from scratch, fails identically two runs out of two; in the
passing window it passes. Whatever decides it is a property of the machine at
that moment and not of the tree, which is why this is here and not in §1.

*What was ruled out, in order:* the tests (they pass alone), the wrapper's own
code path (the probe run by hand exits 0), a stale binary, a rebuild immediately
before, and the caller's stdin. What is left is to find **which** invocation's
stderr is crossing, which means capturing the streams of a failing run rather
than reading one victim's message — and nothing in this repository decides
whether that run fails.

*Why it is kept at all:* if it comes back, this says what was already ruled out —
it is not the tests, not the wrapper's own code path, and not a stale binary. It
is worth one look and not a re-run, which is what an unnamed failing gate
otherwise teaches people to do.

**Otherwise empty**, which it has not been before, so what was here is worth
naming: Part
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

### 3.2. Six citations named an entry by its number and meant another one

Found by reading, in the round that closed the `catch` binding. The page's own
head says to cite by **subject** and not by number; six live sentences did not,
and four of them had gone stale because entries closed and the ones below moved
up:

| where | said | meant |
| :--- | :--- | :--- |
| `contracts/mod.rs` | *"a trait whose method genuinely pauses, which is §1.1"* | that is `NK1129` now ([ADR-080](specification/adr/adr-080.md)) |
| `tests/nullable.rs` | *"until a generic function lowers with its `<T>` (§1.1)"* | it does ([ADR-074](specification/adr/adr-074.md)) |
| `tests/typecheck.rs` | the same | the same |
| `tests/common/mod.rs` | the same | and the road is **closed**, not waiting: a member on an unbounded parameter is `NK1126` |
| `spec-promises.md` | *"the qualified path is what fails (§1.1)"* | `http::Response(status: 400)` parses and lowers |
| `spec-promises.md` | *"see §3.7"* | there is no §3.7; §3 runs 3.1, 3.5 |

All six are fixed and now name a record or a subject. What is worth keeping is
the shape: **two of them were not merely misnumbered but false**, and a reader
had no way to tell, because a citation into a notes page is the one kind that
cannot be checked mechanically — the entry it points at exists, it is simply a
different entry. `check-adr-refs.py` covers ADR numbers and has nothing to say
here.

*A guard was measured and not built*, and the reasoning is in
[the index](specification/adr/README.md#reserved-numbers): the one mechanical
shape available produces a fifth false alarm, and a gate people learn to ignore
is worse than none. So the remedy is the rule at the head of this page, and this
entry is the evidence that it has to be applied rather than merely written.

*Evidence:* the six sentences above, each read against the page as it stands.

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
