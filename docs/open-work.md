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
lower to `rustc`, against two recorded baselines. Of 127 blocks, 50 are programs
this compiler takes and 32 of those compile below — the newest of them being
Part II 12.2's `set(neu; after: stand)`
([ADR-111](specification/adr/adr-111.md) D5), which lowers and compiles.

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

And from §2 rather than §1, because it was an explanation rather than a
miscompilation: **`examples/foreign-runtime/` explained the boundary with a rule
that is gone.** Four programs, three manifests, a shim's doc comment and
[`foreign-runtime.md`](foreign-runtime.md) itself all said `Shared` lowers to an
`Rc` at `user_parallelism = no`
([ADR-037](specification/adr/adr-037.md) D3) and that `Shared` was unbuilt.
D6 narrowed D3 — `Shared` is an **atomic** count at both settings — and
[ADR-064](specification/adr/adr-064.md) built it. The notes page had also
*predicted* a failure mode that D6 then made impossible, and the correction is
kept **beside** its reasoning rather than written over it, because what that
page is for is the finding and the finding includes how it was reached.

**What the four still demonstrate is sharper than what they claimed.** A Nikaia
`Shared` cannot be written into this shape at all now:
[ADR-061](specification/adr/adr-061.md) D1 refuses one handed to code nothing
describes, since D7's per-value inference makes a `Shared[Conn]` an `Rc` for one
value and an `Arc` for another in the same program. So the foreign value stopped
being a stand-in for something unbuilt and became the only way to build the
shape.

And **a trait a package publishes, implemented and then not callable**
([ADR-095](specification/adr/adr-095.md)). Rust wants a trait in scope before
its methods can be called and `impl http::Handler for Fixed` does not put it
there, so the message named a remedy the program **cannot write** —
[ADR-046](specification/adr/adr-046.md) D2 gives this language no import at all.
The same shape in one file compiles, which is what said it was one emitted line
rather than a question about the language.

**This section is empty again.** The most recent thing out of it was found
rather than filed: **a binding that is changed and does not say `mut`**. Part I 2.1
writes `// x = 20  <-- This would cause a Compiler Error` and this compiler was
not the one giving it — `let xs = Vec::new()` then `xs.push(1)` lowered to a
Rust binding with no `mut` on it, and the answer came back about the generated
file. It had been there since `mut` existed, and no example wrote it, which is
why nothing had noticed.

**It came out of the neighbouring work rather than out of a search.**
[ADR-094](specification/adr/adr-094.md) D3 gave a *parameter* the same rule and
the same machinery (`NK1138`), and the `let` beside it was then plainly the same
sentence with the word in a different place (`NK1139`). The answer lives on the
binding in scope rather than in a set of its own, so the scope is the one the
checker already keeps: an inner block's `xs` stops being the answer when the
block closes. A set pushed and popped by hand at twenty-eight places would have
been a *correct program refused* waiting to happen, which is the thing
`docs/README.md` ranks second-worst.

*And it found one in the tests:* `concatenation.rs` wrote `let xs` and then
`xs.push(1)`, a program that had never compiled.

**Before that**, and worth naming because of how long it took and why. **A `sync` body was refused as pausing when
the call left the unit** — `NK1129` about a trait method whose only call is
`n + 1` in the file next door, and about a handler whose body calls the package
it implements against. It was a *false refusal*, the second-worst thing on
`docs/README.md`'s list, and it stood for several rounds behind a question
rather than behind work: the obvious repair was built, made both programs
compile, and was **reverted**, because with three packages it made two builds of
one library disagree about whether a function pauses.

**What closed it was the question being answered rather than the work being
found.** [ADR-100](specification/adr/adr-100.md) said what a consumer does with
a dependency's contracts — reads them, never derives them — and once that was
decided the two halves were a week apart: the inference graph became the package
(D2), and then the build became ordered so that a package's ledger exists before
its consumer is checked (D1, D5). The reverted pass is still the wrong shape and
is still reverted. **A defect that needs a decision is not a defect that needs
patience** — it needs the decision put on
[`open-decisions.md`](open-decisions.md), which is where that one sat until it
was taken.

The two before it came out of **running programs** — the specification's, the
corpus's, and the one somebody wrote while building a fixture for something
else. Both of those came from the third kind: a probe of whether a package could
publish a `trait`, written to answer a question and not to find anything.

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
   compiler about the program. *A task that may not cross a thread*, below.
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

### 2.1. The crossing refusals are built and nothing can reach them

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

**The diagnostic is built now, in both halves.** The structural check
[ADR-005](specification/adr/adr-005.md) §1 Group B ran on what a task
**captures** ([`contracts::send`](../crates/nikaia/src/contracts/send.rs),
`NK2501` and `NK2502`); it is now also asked of what a body **binds** — a value
bound inside the task and still live at a suspension point further down, which a
task's `async` block holds *inside the future*, where the pool's starter asks
for `Send` of the whole thing. The rule is
`send::held_across_a_pause`: **bound before a pause, and named after it.**

*What remains is not a missing diagnostic but a verdict nothing produces.*
`Crossing::MayNot` has exactly one producer — a lock at a **foreign**
destination ([ADR-045](specification/adr/adr-045.md) D3) — so at
`Destination::Ours`, which is where a task goes, **no type answers it**.
`NK2501` is therefore silent in both its halves, and will be until
`contracts::send` gains a second producer. What reaches it first is a type from
outside this language, which is
[ADR-104](specification/adr/adr-104.md)'s `nikaia describe` — the entry here
about a foreign crate being described before it is called.

*So this entry is now about `send.rs` rather than about the checker*, and it
stays open for that reason rather than for the one it was filed under.

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
and **that was the reason to build the refusal rather than a reason not to**:
a refusal costs nothing before there are programs it would reject, and the same
refusal added afterwards breaks them.

*What a silent refusal costs is that nothing can check it*, and the liveness
half is a **computation** rather than a lookup — it can be wrong where the
verdict cannot. So the answer is written down (`Checked::held_across_a_pause`)
and `crates/nikaia/tests/held_across_a_pause.rs` holds it on real programs: a
value used after a pause is held, one finished with before it is not, a body
that never pauses holds nothing, and a call nothing describes counts as a pause
(fail-closed, [ADR-010](specification/adr/adr-010.md) D1). One test holds the
*silence* itself — that no type answers `MayNot` into our own code — because the
day that stops being true is the day these programs start being refused, and
that should be a test going red rather than a discovery.

*And the liveness answer is deliberately generous.* **Bound before a pause and
named after it** is wider than Rust's own liveness, which would be a correct
program refused if the refusal stood on it alone. It does not: what refuses is
`MayNot`, a claim about a **type** and never `Undecided`'s silence, so a name
this walk is too generous about is refused only where a value of its type could
not have crossed from anywhere.

**And [ADR-040](specification/adr/adr-040.md) D1's task half is closed rather
than waiting:** the analysis names a `spawn` body's handle as a duplication site,
`NK2101` is raised, and the exemption is held — a `Shared[T]` handed into a task
and used again afterwards is not refused, which `tasks.rs` says about a program
that does it.

### 2.2. A lambda that pauses is refused at the build

[ADR-055](specification/adr/adr-055.md) §6's remainder, and a limit of this
compiler rather than of the language — so it is here and not in §1, where a
defect is the compiler being *wrong*. **The other half of this entry, a
recursive pausing method not being boxed, is built**; what it took is below.

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

**A recursive pausing *method* is boxed now**, and what it took was asking a
question the emitter already had the answer to. §6 step 2 boxed a call that
closes a cycle of pausing functions, resolving a callee's name the way the
emitter resolves anything — which is not at all for a method, since
`stats.add(5)` names `add` and only the type checker knows what it goes to
([ADR-028](specification/adr/adr-028.md)). So a cycle through a method reached
`rustc` as *"recursion in an async fn requires boxing"*, about the generated
file.

*The fix needed no new set and no new answer from the checker*, which the plan
here had said it would. `pausing_reach` already draws an edge to **every**
pausing method of a given name — that is the over-approximation its own note
describes — so the graph had the method cycles in it all along and only the call
site was not asking. The same widening answers it at the call: a box nobody
needed costs one allocation, and a box that was needed and is missing is a
program that does not compile, so where the two readings differ it takes the
wider. `crates/nikaia/tests/recursive_methods.rs` holds the two cycles and the
three shapes that must *not* box.

### 2.3. Standard input is `async` and does not suspend

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

*What it needs is not what this entry used to say.* It proposed
`Op::Readiness` against standard input's descriptor. **That would not have
worked**, and the reason is the park hook rather than readiness: the bell a
worker rings is the *fallback* path's, and on the completion path the executor
parks on the **ring**, whose `park` answers off a count of ring jobs. A worker
operation is not one, so a worker's reply cannot wake the executor and no future
may be fed from one — `exec::block_on` spins or panics with its own *a future
returned `Pending` without arranging for its waker to be called*.
`a_worker_operation_does_not_wake_the_completion_park` in
`crates/nikaia-std/src/rt/mod.rs` holds the finding and goes red the day it
stops being true.

**So the first half is a decision and it is in
[`open-decisions.md`](open-decisions.md)**: put standard input on the ring, or
make the ring park hear the bell. The second is the larger one and stays a
question of its own: a `for` over a **stream** is `while let Some(x) =
s.next().await` in the language below, and Rust has no stable trait for one. The
parallel is [ADR-025](specification/adr/adr-025.md) D6's `iterates_fallibly` — a
property of the *type*, recorded in the ledger, that makes the emitter write the
step differently — so the shape to copy exists.

*And something larger rests on the same answer.* `rt::io::wait` is D3's
readiness half, built for sockets, and it **cannot be awaited** either — only
blocked on, for exactly this reason. That is what an HTTP server needs, so the
entry below about there being no HTTP server waits on this decision and not only
on its own.

### 2.4. The lock is built and every rule around it is not

[ADR-057](specification/adr/adr-057.md) decided what the lock **is**,
[ADR-059](specification/adr/adr-059.md) what a program writes to reach one, and
[ADR-064](specification/adr/adr-064.md) gave the shared mutable type its name, its
constructor and its single spelling. **Part II 12.2's counter compiles and runs at
both settings.** So the type is no longer what anything here waits on — what is
left is the section's own rules. **The first of them is built**; the rest are
refusals nothing raises:

* the **lock-touching** derived property (ADR-039 D3) is **built** —
  `contracts::locks`, a least fixpoint over the call graph `sync` uses, with a
  `spawn`'s body excluded and a trailing lambda's counted, recorded as `locks`
  beside `keeps`. D7's *stored* lambda is not, and is the one part of D3 left.

  *It has three values and not two, and the corpus is what bought the third.*
  D3 says fail-closed, and it says it about `sync`, where the cost of doubt is
  a caller writing `.await`. Taken literally here it gave the property to **16
  of 59** functions in `examples/` — almost all of them `main`, and **not one
  of those programs opens a lock**. A refusal reading that column would have
  refused correct programs. So `Undecided` is its own answer, exactly as it is
  in [`contracts::send`](../crates/nikaia/src/contracts/send.rs): not
  permission, and not a refusal either. The corpus now reads **0 hold, 24
  undecided, 35 clear**, and what shrinks the middle is entries existing for
  the methods it calls — the entry below about describing a foreign crate —
  rather than a change here.

  *Nothing reads the column yet*, which is why it landed alone: the number
  above is what a refusal has to be read against, and finding it out afterwards
  would have meant finding it out from a program somebody wrote.

  *One thing the record had not said*, and a test caught it: the checker's
  record of a method call did not know which side of a `spawn` the call was on,
  and D3 turns on exactly that. `check::MethodCalls` carries the task half
  separately now.
* the **re-entrancy check as a build switch** (ADR-039 D8), which the cache key
  already accounts for — and which ADR-057 D2 makes free at one thread and D3
  charges only on the values that actually cross;
* `NK2201` and `NK2503`, catalogued and not emitted. **Three of the five have
  left this list** — `NK2204` and `NK2205`, the two that come with the doors
  ([ADR-099](specification/adr/adr-099.md)), and now `NK2203`. The first two
  went early because they are **local**: each is one statement. `NK2203` needed
  the column above and one more thing the record had not named — *what is
  inside a door* — which turned out to be a flag on the walk rather than an
  analysis, because the checker already knows which method it is in.
  `NK2503` still needs the reachability walk
  [ADR-039](specification/adr/adr-039.md) D6 describes as `NK2502`'s
  generalised.

  *What `NK2203` refuses, and what it does not.* A second lock written inside
  the block, one reached through a chain of calls, and a **`println`** — which
  is [ADR-067](specification/adr/adr-067.md) D1's case: it never pauses, so
  `sync` says nothing about it, and it takes standard output's own lock while
  yours is open. Not refused: a `spawn` started inside the block, because its
  body runs later and elsewhere; `get` and `set`, which hold nothing open
  (D10); and anything the column answers `Undecided` about, because doubt is
  not permission and not a refusal either.

  *It refuses nothing in `examples/`*, since nothing there opens a lock — and
  one test in `crates/nikaia/tests/shared.rs` had to move its `println` out of
  an `access_all` block, which is the first migration this rule has asked for.

  *`NK2201` is the one left, and it is not obvious what is left of it.* The
  catalogue calls it *no I/O while holding locked data*, and ADR-067 D1 split
  that sentence in two: what **pauses** is `NK2202`'s and what **takes a lock**
  is `NK2203`'s. A `println` is the second. What a third code would add is I/O
  that neither pauses nor takes a lock, and whether any exists is a question to
  answer before writing one.
  **`NK2202` is a second exception and is raised** — `sync::check` finds the
  calls that contradict an assertion and `diagnostics` renders them — which is
  worth the extra words, because the bullet used to name the whole range and a
  reader would have gone looking for work that is done.

  *`NK2201` and `NK2203` are now the next step here*, and the reason they are
  is that what a chain of calls reaches is the column above: what is left for
  each is knowing what is **inside a door**, which is one walk over a body
  rather than an analysis.

### 2.5. Part II 12.8's supervision syntax

`supervisor::start_link(fn { … }; restart_policy: …)` is specified and there is no
supervisor. Listed so it is not mistaken for something the `spawn` work includes —
it is not.

### 2.6. `fortunes.nika` waits on two runtime pieces and one language question

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
* **And a handler cannot be received at all**, which is the language question
  this entry used to say it did not have.
  `.route("/fortunes") fn { fortunes(db) }` needs `route` to declare a parameter
  that is code, and neither door is open: a function type is not sayable
  (`fn apply(f: fn() -> String)` is a parse error) and a bound cannot name a
  trait another package publishes. That is
  *how does a package receive a handler* on
  [`open-decisions.md`](open-decisions.md), and it is independent of the
  server — a socket layer would leave it exactly where it is.

**Measured, so the order is known.** Given a manifest that depends on
`examples/http/`, the file stops before any of the three: `dsl postgres { … }`
has no hole, and `postgres` is not a grammar this compiler has. So the first
thing fortunes needs is the driver question above, and the handler question is
what it meets after that.

Moved here from [`handoff.md`](handoff.md), which is a guide to the parser backend
and was also carrying open work. One list.

### 2.7. There is no HTTP server, and three records now wait on it

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

**The one piece that looked as though it did not wait is answered elsewhere.**
A name the request chose reaching the filesystem is
[ADR-108](specification/adr/adr-108.md): the root is an argument of the call,
`http::File(path, root)` exactly as `fs::map(path, root)`, and there is no
provenance on a path and no refusal to build before the server. The `fs` half
is the entry below about a path naming its root at the call, and `http::File`
inherits it the day it exists.

Nothing of [ADR-058](specification/adr/adr-058.md) is built. What is built is the
bench that decided it (`benches/sendfile/`) and the write-up
([`zero-copy-send.md`](zero-copy-send.md)); `send_file` beside the ring could have
been built ahead of the server and deliberately was not, because D3's measurement
makes it the mechanism that loses at the sizes a server sends most.

**And there is a runtime piece underneath all of it.** A server waits on
sockets, and `rt::io::wait` — the readiness half this would rest on —
**cannot be awaited**, only blocked on: on the completion path the executor
parks on the ring, and a worker's reply does not reach it. That is the entry in
[`open-decisions.md`](open-decisions.md), and the entry above about standard
input is the small end of the same question. Nothing here can be an
`async fn` that actually pauses until it is answered.

### 2.8. There is no target that lets foreign code call in, and the record for one is written

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

### 2.9. Nothing runs Nikaia code while the program is built

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
see the entry below, *running a grammar while the program is built*.

*What it does **not** include:* running a **grammar**. That looks like the same
job and is not; it is the next entry's, *running a grammar while the program is
built*, and the reason is there.

### 2.10. Running a grammar while the program is built is not interpretation

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

### 2.11. The cleanup point the ledger should narrate

[ADR-094](specification/adr/adr-094.md). A parameter is a view unless its body
keeps the value, a `keeps` column records which, the emitter writes the
reference at the call, a `for` lends, a `let` over a place is a view, and
`mut` on a parameter is where in-place change is written. **Steps 1–4 are
built**, which is D1, D2, D3 and D4: the column is inferred, a `for` lends, a
`let` over a place is a view where the value would move, `xs.drain()` takes the
elements away, a parameter the callee only reads is declared `&T` and given its
`&` at every call, and `mut out: Vec[i64]` is the third state and lowers to
`&mut T`. **What is left is D5** — the ledger diff that narrates a kept value's
moved cleanup point.

*Evidence:* `keep(file)` says nothing about the file being flushed inside the
callee, and nothing tells a caller when a callee *starts* keeping a value whose
teardown has an effect. §3 of the record names that as the one semantic cost and
D5 as where it is paid: a change to a parameter's `keeps`, on a type with a
`Drop` or a `Cleanup`, is a ledger diff that names the callers whose cleanup
moved — the `NK2401` shape, one more thing it narrates.

**Step 1 is built.** `contracts::keeps` infers the column and the ledger
records it beside `returns`; nothing reads it yet, which is what that step is
for — the answer was diffed against the corpus before a call site changed. Of
39 functions in `examples/`, **six keep a parameter** and 33 read what they are
given; `report.nika`'s `page(entries, total)` is among the 33, and that file
carries a comment explaining that `count` must be read first because `page`
*"consumes"* the entries. It does not. Of `std`'s 94 entries, six keep — an
absent `keeps` on a present entry means it keeps nothing, which is that file's
own convention for `sync` said once more.

**Step 2 is built too, and it changed what programs mean** — the first step
deliberately did not. A `for` over a place lends it, so `xs.len()` after the
loop is a program; `xs.drain()` is the written form of taking the elements
away, lowered as `into_iter`; and `NK1137` refuses a `&` written in front of a
`for`'s list. Four loops and two `&` moved in `examples/`, which is the whole
of what the corpus had to say.

*Three things that step met and the record had not said*, each on the corpus
rather than in argument: `.iter()` rather than `&`, because an iterated name
may already be a view and `&&Vec<T>` does not iterate; `drain()` had to be
**built**, because D4 names it as the written form and `std` had no such entry;
and a `let` over a place that *copies* must not lend, because a borrow held
across a loop that writes the same field is `E0502`.

**Step 3 is built, and it is the one that rewrote every example.** A parameter
the callee only reads is declared `&T` and given its `&` at every call off the
one column — `contracts::keeps::lends`, called by the emitter for the
declaration and by the checker for the argument, because the two disagreeing is
a `&&T` or a moved value in the language below. `NK1137` now covers both
positions. Not lent, each of them the polarity being spent: a kept parameter, a
value that copies, an argument already a view, and a method's argument, which
is `touches`' reason for asking a weaker question — the emitter cannot resolve
which entry `acc.record(m)` goes to.

*One hole that step made and closed in the same change*, because it is the
reason a new column exists: the ledger's type language spells a view `&T` and
has no second spelling for a mutable one, so `Vec::push` and `Vec::len` wrote
the same receiver type. Once the `&` was written off `keeps`,
`fn fill(out: Vec[i64]) { out.push(1) }` lent `out` and `rustc` answered
*cannot borrow as mutable*. **`mutates`** is the claim now — seven of `std`'s
ninety-four entries, and for a Nikaia function the declaration `&mut self`
rather than an inference. It leaves such a parameter taken **by value**, and D3
is what gives it its `mut`.

**Step 4 is D3, and it is the only one of the three states an author writes.**
`fn fill(mut out: Vec[i64])` lowers to `&mut Vec<i64>` and `fill(xs)` gains its
`&mut`; the word rides in the signature, because that is the one key a caller
across a package boundary reads a parameter's kind off. It closes a hole older
than the record: a body that changed an owned parameter lowered to a Rust
declaration with no `mut` on it, and the answer came from `rustc`. **`NK1138`**
is that answer in this compiler's words, raised only where the change is
certain — an assignment into the parameter, or a method every candidate entry
marks `mutates`. One block in the specification itself was written the old way
and says `mut` now; `examples/` had none.

*One limit two of those steps share, and it is a limit of the same kind.* Both
the `let` half and the refusal at a call act only where the checker could
**type** the value. Where it could not — a place inside a lambda, a value a
`catch` handed back — the `let` does not lend, the refusal stays quiet, and the
written `&` is the program's own. The *writing* has no such limit, and must
not: the declaration is written off `lends` alone, so anything the checker
skips before recording the argument is a callee taking a `&T` and a caller not
passing one. Closing it is a better answer for those types, not a change to
either rule.

*Why it is here and not in §1:* nothing is miscompiled. `fill(xs)` reads as
though it does not change `xs`, which is a message in the wrong words rather
than a wrong program.

### 2.12. The boundary translation for a hand-edited hash

[ADR-100](specification/adr/adr-100.md) D6, and the only part of that record
left. **D1, D2, D3, D4 and D5 are built**: a package's units are inferred as one
graph, its ledger is written in its own root by its own build, the header
records a SHA-256 per unit, a consumer believes that ledger while the hashes
match and derives that package again where they do not, the build is ordered so
a dependency's ledger exists before its consumer is checked, and `--locked`
compares each of them byte for byte. §1's defect closed with them.

*What is left:* if `rustc` reports, at a call across a package boundary, that a
value *is not a future* or that a `Result` was not expected, the driver should
say *the ledger of `<package>` does not match its sources* and name the package
to rebuild. Today it reaches the user in the backend's words about the generated
file, which is [Part III C.1](specification/30-nikaia-tooling.md)'s class.

*Why it is small and still worth doing:* it is a **safety net and not the
mechanism**. D3 establishes the agreement, and the one way past D3 is a hash
edited by hand — so this is the message for a case nothing in a normal build
reaches. That is also why it is last: the same sentence says it is the least
urgent thing in this record and the only one that can still surprise somebody.

*Evidence: none, and the way to get some is to edit a hash.* No program in the
repository can produce it, which is the state
[ADR-055](specification/adr/adr-055.md) §2 D6's refusal was in before it was
built and the reason it was built anyway: a message costs nothing before there
is something to say it about.

### 2.13. An error that newly reaches a `catch` is named once

[ADR-101](specification/adr/adr-101.md). When a callee's `throws` set gains a
member, every `catch` over it is named in the build output, and `--locked`
fails until the ledger is committed. **Nothing of it is built**, and nothing
can be yet: `std.contracts` writes `throws = ["?"]` on every entry, so there is
no set to diff.

*Evidence:* 22 `catch {` handlers in `examples/` and `tests/samples/`, none
matching on `error`; a new failure in any callee reaches all of them in
silence today.

*What it needs:* error types lowered as enums, which [ADR-023](specification/adr/adr-023.md)
D1's set already waits on; then the set written and diffed; then the note and
the `--locked` failure, which are the `NK2401` machinery over one more column.
The first of those is the entry below — *an error type is lowered as the `enum`
it is* — and is the thing this entry waits on.

### 2.14. An error type is lowered as the `enum` it is

[ADR-023](specification/adr/adr-023.md) D1 records `throws` as a **set** of
error types; [ADR-013](specification/adr/adr-013.md) D3 lowers every `throws`
to one boxed error, so the set has no members to name — `std.contracts` writes
`throws = ["?"]` on every entry, and Stage 0 writes `true` for a program's own.
Part I 7.1's `enum ConfigError` with `impl Error for ConfigError` parses and
lowers as a type; what does not exist is the lowering that makes it *the*
error a function fails with, so that a `throw ConfigError::NotFound(path)`
reaches a `catch` as that variant and the ledger can write the name down.

*What waits on it:* the `throws` set (ADR-023 D1), the note over `catch` sites
and the `--locked` failure (*an error that newly reaches a `catch` is named
once*, [ADR-101](specification/adr/adr-101.md)),
the reserved `NK2401` case for a `catch` that stops covering its arrivals, and
`match error { … }` over a variant from a callee in another package.

*What it needs:* a function's error type as the sum of what its body throws
and what its callees throw — one generated `enum` per function where the set
has more than one member, with the conversions the propagation needs — and the
ledger writing the members. The whole-program inference is ADR-023 D1's; what
is unbuilt is the emitter's half.

*Evidence: none yet beyond the `["?"]` in every entry.* No program in the tree
matches on an error variant that crossed a function boundary, because none
can.

### 2.15. A parameter may be a function, and the type says what it may do

[ADR-102](specification/adr/adr-102.md). `fn(Request) -> Response`, with
`sync` and `throws` after the result as a declaration writes them; a lambda
that does less fits a type that allows more, the other direction is refused;
whether the parameter is run or kept is inferred, and a kept one's promises
are the type's. **Nothing of it is built**: the type is a parse error, and the
eight `std` entries that take a lambda bypass the grammar.

*Evidence:* `fn twice(x: i64, f: fn(i64) -> i64)` — *expected `&`; found `fn`*
— and `examples/fortunes.nika`'s `main`, whose `route` cannot be declared by
`examples/http/`.

*What it needs, in the record's order (§5):* the type in the grammar, using
the spelling the ledger's type language already has; the two refusals; the
run-or-kept inference, which is ADR-094's `keeps` asked of a code parameter;
and the lowering of a kept pausing handler, which is *a lambda that pauses is
refused* one entry up, with a callee that can now say which shape it wants.

### 2.16. A package is found by version through Cargo, under `nikaia_<name>`

[ADR-103](specification/adr/adr-103.md). `http = "1.2"` becomes
`http = { package = "nikaia_http", version = "1.2" }` in the generated
`Cargo.toml`; Cargo finds and fetches, the compiler reads `src/*.nika` and
`nikaia.contracts` from where the crate landed and emits it as a workspace
member as it does a path dependency; a crate under the prefix without both is
refused by name. **Nothing of it is built**: a bare version is refused with
the old sentence, and a package's own package dependencies are not followed.

*What it needs, in the record's order (§5):* the manifest arm and the rename;
resolution through `cargo metadata` and the not-a-Nikaia-package refusal;
following a dependency's own manifest in both arms; the stub `Cargo.toml`
written by the build so that `cargo publish` is the whole of publishing.

*Evidence:* Part III 13.3's own example, which was a refusal and is now a
dependency; no package in the tree is published yet, so the first one is the
test.

### 2.17. A foreign crate is described before it is called

[ADR-104](specification/adr/adr-104.md). A call into a crate no ledger
describes is refused with the command in the message; `nikaia describe
<crate>` writes `contracts/<crate>.contracts` for the functions the program
calls, from rustdoc-JSON where the toolchain has it and from the crate's
sources where it does not, translated by Part III 15.2's table with
`touches` and `locks` fail-closed; the file is committed, hashed against the
crate's version, and reviewed. **Nothing of it is built.**

*Evidence:* `examples/foreign-runtime/` — four programs calling `hyper_shim`
with no entry, silent today; they become described or the fixture for the
refusal.

*What it needs, in the record's order (§5):* the refusal; the draft from the
sources; the file, its header and the hash rule; the rustdoc-JSON reader
behind a toolchain check; the four examples.

*And something else waits on it.* The entry above about the crossing refusals
being built and unreachable is unreachable **because** no type answers
`MayNot` into our own code, and a described foreign type is the first thing
that could. `NK2501` and `NK2502` are written and tested and wait on this
file to have something to say.

### 2.18. The ledger says `Seq[T]` and `Par[T]`

[ADR-105](specification/adr/adr-105.md). Two words join the ledger's type
language: `Seq[T]` for what is produced step by step, with `sync`/`throws`
after it for the step, and `Par[T]` for what `par_iter()` hands back, whose
lambdas must be `sync`. A `Seq` is consumed by walking; a container is not.
**Nothing of it is built**: nine `std` entries say `-> ?`, and every method
called on their results is unfound.

*Evidence:* `open-decisions.md`'s measurement, kept here — 51 unanswered
method calls across the corpus, 16 closed by filling entries, the remaining
**35** all downstream of a `?` that is a sequence; `HashMap::keys` as
`(&HashMap[?, ?]) -> ?`, which costs `keys().collect()`, `names`, and
`names.sort()` in one line.

*What it needs, in the record's order (§5):* the two words in the type
language's parser; the `std` entries rewritten (`keys`, `values`, `chars`,
`lines` for file and pipe, `args`, `map`, `filter`, `collect`, `join`,
`count`, `nth`, `par_iter`); the once-only refusal, which is `keeps` asked
of a `Seq`; the `sync` demand on a `Par[T]`'s lambda.

### 2.19. A bound takes a path, and the ledger records traits and `impl`s

[ADR-106](specification/adr/adr-106.md). `[H: http::Handler]` parses; `use`
is unchanged; the ledger gains a `trait` table whose methods are ordinary
`fn` entries, and an `impl` table written where the `impl` stands, so that
*does `T` implement `A`* is the union over every ledger a program reads. A
call through a bound resolves to the implementing type's own entry.
**Nothing of it is built.**

*Evidence:* the three refusals a cross-package `Handler` bound met against
`examples/http/` — a parse error at the path, `NK1126` after `use http`, and
the `NK1129` that ADR-100 D2 has since removed.

*What it needs, in the record's order (§5):* the path in the bound's grammar;
the `trait` table and method entries; the `impl` table and the union in the
bound check; the resolution at a call through a bound.

### 2.20. Text is one type, and `&str` is the assertion

[ADR-107](specification/adr/adr-107.md). `String` is the one text type and
its state — borrowed, tethered, owned — is the compiler's per use; `&str` is
the promise that a value is a borrowed view, held to at the line that would
break it; a copy is `.to_owned()` or a refusal, never inserted. **Nothing of
it is built**: two types in the checker, a literal in a `String` slot is
`NK1106`, and no text carries a handle.

*Evidence:* 13 `.to_string()` in `examples/`, each a literal or a view put
where a `String` was declared; `Response(content_type:
"text/plain".to_string(), …)` in `examples/http/`.

*What it needs, in the record's order (§5):* the checker's acceptance in both
directions with D2's refusal; text represented as `Bytes` is, with the state
from the tether analysis; `NK1106`'s help and the thirteen sites; the
foreign-boundary copy once crates are described; `--tethers` over text.

### 2.21. A path names its root at the call

[ADR-108](specification/adr/adr-108.md). Every `std` function that takes a
path takes its root right after it, with no default: an `fs::Root`, which is
`Dir(store)` — the name is resolved under it and `fs::Outside` where it would
leave it — or `Anywhere`, the one way around the check, recorded per site and
listed by `nikaia --trust`. No exception for a literal. **Nothing of it is
built**: `fs::map(path)` takes one argument, `fs::Root` does not exist, and
the ledger describes the path functions without a root.

*Evidence:* 15 calls in `examples/` — 8 `fs::map`, 6 `fs::read_to_string`,
3 `fs::write` (one of them in `examples/README.md`), 1 `fs::read`, 1
`fs::exists` — every one of them a command-line program whose path the
operator typed, so every one of them writes `fs::Root::Anywhere`.

*What it needs, in the record's order (§5):* `fs::Root` and the root in every
path-taking entry's `signature`; the check in the Rust half of `fs`; the
sites in `examples/`, its README and Part III 17.1; `--trust` listing
`Anywhere` and a literal `"/"` root; `http::File` when it is built.

### 2.22. An `update` block says `mut`, may run more than once, and the compiler picks the lock

[ADR-110](specification/adr/adr-110.md). `update fn(mut v) { … }` is the one
form; `v` is a copy where the value fits a machine word — run the block on the
copy, compare-and-swap, retry on a collision — and the address in the lock
otherwise, where the block runs once; nothing is moved out of the lock and no
slot is ever empty. `update_all` takes one `mut` per lock. `access` reads.
**D1, D4 and D6 are built, and D2's address row with them**: `fn(mut v)`
parses, both doors are handed `&mut T`, the `Option` is gone and with it the
empty slot — and the `emptied()` panic that named that state, because it cannot
occur. **What is left is speed**: a copy and a compare-and-swap where the value
fits a machine word. The block runs exactly once today, which D3 licenses
outright (*may* run more than once is a permission, not a requirement).

*And Part II 12.2's counter runs again.* `counter.update fn(mut n) { n += 1 }`
had stopped lowering the day the page was rewritten to this form;
`tests/specification/COMPILES.txt` carries it as a program rather than a
fragment now.

*One thing the record had not said, and the corpus settled it.* `NK1138` at a
door is D1's, and the first build asked it of **every** lambda parameter, on the
reasoning that one changed without `mut` is already broken Rust. It is not:
`par_fold(…, fn(acc, m) { acc.record(m) })` in `examples/1brc.nika` changes
`acc`, has no `mut`, and compiles — the **emitter** writes the word itself where
it recognises a fold's accumulator, and `and_modify fn(tally) { tally.bump() }`
in `k-nucleotide.nika` is the same shape one library over. So the question is
asked at a door and nowhere else. **Widening it is work of its own**: it means
taking that `mut` out of the emitter and making every such lambda say the word,
which is a corpus migration rather than a rule change.

*What it needs, in the record's order (§5):* the compare-and-swap loop for
word-sized values and the second lowering, with `explain` naming which row a
value fell in; `--sharing` on a large `get`.

### 2.23. `catch` takes everything, and one record writes a `catch` that does not

[ADR-111](specification/adr/adr-111.md) D5 is **built**: `kasse.set(neu; after:
stand)` is `Locked::set(after)` in the ledger, the witness is an argument of
it, `throws = ["Overtaken"]` carries the failure the way every other one
travels, and `NK2208` keeps the lowering from being a second door. What is left
of that record is one **sentence** of it: it writes `catch Overtaken
{ continue }`, and this language has one `catch` and it takes everything.

*It is not work so much as a question*, which is why it is in
[`open-decisions.md`](open-decisions.md) with a recommendation rather than
here with steps. Whichever way it goes, the line in D5 §2 is what changes
first: either a typed handler exists and the line is right, or it does not and
the line should say `catch { continue }`.

*What is not built and belongs to another record:*
[ADR-110](specification/adr/adr-110.md) D2's compare-and-swap, which would make
the door one instruction for a word-sized value instead of one lock
acquisition. It is that record's speed row and §2.22 carries it.

### 2.24. A cleanup the deadline cut off names the resource

[ADR-112](specification/adr/adr-112.md). **Steps 2 and 3 are built**: an
expired `cleanup-deadline` ends the program with exit status 70, and the
message goes the panic path — standard error and the program's panic hook,
never standard output — with a test that runs a second process, expires its
deadline and reads both. `cleanup-deadline = "0"` does not drain and
therefore never expires, which is D3 and needed no code.

*What is left is step 1, and it is [ADR-006](specification/adr/adr-006.md)
D3's.* The parked-cleanup queue does not exist, so what expires today is the
drain of pending **I/O operations**: the message counts them rather than
naming the resources whose cleanup was cut off, which is what D2 asks for.
When the queue lands the names go in the same message on the same path, and
nothing about the status changes.

*One limit of the build, named rather than left to be found:* under
`panic = "abort"` the process is gone with the abort's own status before the
exit code can be set. The hook has already run and said what happened, so
that profile loses the status and not the message.

### 2.25. `?.` reaches through a view

[ADR-113](specification/adr/adr-113.md). `?.` takes nothing: it reaches
through a view of its receiver, and the result is a copy where the member
copies and a view of the receiver otherwise, kept alive as any view of a
place is. **Nothing of it is built**: `x?.a` lowers to `x.map(|it| it.a)`, a
method reach to a `match` over `x` by value, and a receiver used again is
refused below with the note ADR-052 D8 used to translate.

*Evidence:* `let name = user?.name` then `println(user)` is *"use of moved
value"* today.

*What it needs, in the record's order (§5):* the two lowerings over
`as_ref()`; the tether analysis reading the result as a view; the translation
removed and a test that uses the receiver again.

### 2.26. Reading a map through the brackets is a `T?`

[ADR-114](specification/adr/adr-114.md). `m[k]` on a map answers a `T?`,
`get` says the same, `m[k] = v` still inserts, `m[k] += 1` is written
`m[k] = (m[k] ?? 0) + 1`, and a list's `xs[i]` keeps its abort. **Nothing of
it is built**: a map read lowers to Rust's `Index` and panics on an absent
key.

*Evidence:* `&report.paths[path]` in `examples/access-log.nika` and
`&totals.stations[name]` in `examples/1brc.nika`, both safe only because the
key came from the same map a line earlier.

*What it needs, in the record's order (§5):* `nikaia_std::index::get` with
an output type per container; the checker typing the read as `T?` and
refusing `+=`; the two example lines, Part I 4.5 and a test.

### 2.27. An `overlap` keeps every failure

[ADR-115](specification/adr/adr-115.md). Every error carries a `secondary`
list; an `overlap`'s later failing branches join the winner's list in written
order, a cleanup error while unwinding joins the same list, and a log or
`nikaia explain` prints the list indented. `catch` is unchanged. **Nothing of
it is built**: the block's join drops every failure but the first, and a
cleanup error attached below cannot be read by a program.

*What it needs, in the record's order (§5):* the list on the error carrier
and the printer; the `overlap` join appending; the cleanup attachment
through the list; Part I 8.1.2's example and a test with two failing
branches.

### 2.29. `from` is a name, and a file a build reads is `asset("…")`

[ADR-116](specification/adr/adr-116.md). `from` leaves the reserved list, so
`fs::rename(from:, to:)` parses as Part III writes it; the build-time read is
`asset("…")`, a call the compiler recognises in a `comptime` initialiser under
every rule the allowlist record already states. **Nothing of it is built**:
`from` is in `parser::RESERVED_WORDS`, and the read is unbuilt in either
spelling.

*Evidence:* Part II 10.6's `Json.value(asset("config.json"))` parses and is
refused as `NK1117` and `NK1127` — the page ahead of the compiler, in this
language's words, where the old spelling was a parse fragment.

*What it needs, in the record's order (§5):* the word out of the parser's
table with the `dsl X from e` message matching the bare word; `asset("…")`
when the second stage of `comptime` lands; the two `fs` entries.

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

### 3.2. `_` is a name, and reaches the language below as its wildcard

*Reproduced:* `let _ = f()` compiles today and lowers to Rust's `let _ = f();`.
So does `let (a, _) = pair()`.

`_` is not a reserved word here and not a construct; it parses as an ordinary
name. **What it becomes below is not an ordinary binding**, though — Rust's `_`
discards the value rather than binding it, and the two differ where it matters:
a value bound to a name is dropped at the end of its scope, and a value bound to
Rust's `_` is dropped **immediately**. For a lock guard or a file handle that is
a different program.

*Why it is upkeep and not a defect:* nothing in `examples/`, `tests/samples/` or
`crates/nikaia-std/src/` writes one, so no program is wrong today — only the
language is undecided about a spelling it already accepts. That is the same
class as the postfix `??` below.

*Found by* [ADR-098](specification/adr/adr-098.md): the tuple form made `_` look
like a pattern feature, and checking whether to refuse it there turned up that
the single-name form had been accepting it all along.

*What it needs is a decision before any work:* whether `_` is a wildcard in this
language. If it is, Part I 2.1 gains it and the drop timing is stated; if it is
not, it is refused as a name, which is free today and breaking later — the
polarity [ADR-051](specification/adr/adr-051.md) D1 states. Either way the
compiler's answer stops being an accident of what the parser happens to accept.

*Evidence:* the two lines above, through the release binary.

### 3.3. Eight citations named an entry by its number and meant another one

Found by reading, in the round that closed the `catch` binding and again in the
one after it. The page's own head says to cite by **subject** and not by number;
eight live sentences did not, and six of them had gone stale because entries
closed or were added and the ones below moved:

| where | said | meant |
| :--- | :--- | :--- |
| `contracts/mod.rs` | *"a trait whose method genuinely pauses, which is §1.1"* | that is `NK1129` now ([ADR-080](specification/adr/adr-080.md)) |
| `tests/nullable.rs` | *"until a generic function lowers with its `<T>` (§1.1)"* | it does ([ADR-074](specification/adr/adr-074.md)) |
| `tests/typecheck.rs` | the same | the same |
| `tests/common/mod.rs` | the same | and the road is **closed**, not waiting: a member on an unbounded parameter is `NK1126` |
| `spec-promises.md` | *"the qualified path is what fails (§1.1)"* | `http::Response(status: 400)` parses and lowers |
| `spec-promises.md` | *"see §3.7"* | there is no §3.7; §3 runs 3.1, 3.5 |
| `adr-077.md` | *"the item form (§2.14)"* | §2.14 was the build-time evaluator; the item form is a different entry |
| `adr/README.md` | *"a `const` could not use a map (§2.13)"* | §2.13 is `comptime` at item level; what a `const` waits on is the evaluator |

All eight are fixed and now name a record or a subject. The last two were found
by *adding* an entry rather than closing one, which is the half that is easy to
forget: inserting §2.14 moved everything below it, and the two sentences that
pointed into that range had **already** been wrong before the insertion — the
renumber is what made anybody look. What is worth keeping is the shape:
**two of them were not merely misnumbered but false**, and a reader
had no way to tell, because a citation into a notes page is the one kind that
cannot be checked mechanically — the entry it points at exists, it is simply a
different entry. `check-adr-refs.py` covers ADR numbers and has nothing to say
here.

*A guard was measured and not built*, and the reasoning is in
[the index](specification/adr/README.md#reserved-numbers): the one mechanical
shape available produces a fifth false alarm, and a gate people learn to ignore
is worse than none. So the remedy is the rule at the head of this page, and this
entry is the evidence that it has to be applied rather than merely written.

*Evidence:* the eight sentences above, each read against the page as it stands.

### 3.4. Two examples write a postfix `??` the language does not have

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
to use what Part I 3.5 has.

*Two sentences that were here and are false, corrected rather than deleted.* It
said the examples should be rewritten *"to use what 3.5 has"* and cited its own
number for the section, which moves; and it said **Part I 2.3's nullable types
are themselves a parse error** — `let a: i64? = 3` lowers and runs, measured
through the release binary, and has for long enough that nobody noticed the
sentence. That claim was the reason given for this being upkeep rather than a
defect, and the reason holds for a different one: no program in the tree writes
a postfix `??`, so nothing is wrong today except the page.

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
