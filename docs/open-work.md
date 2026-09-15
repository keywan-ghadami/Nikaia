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

**This section was empty**, for one round, and the two entries below are what
the next program found — which is the sentence that stood here promising exactly
that. The list further down is decided-and-unbuilt and upkeep, and neither
outranks a defect, so these go first. The way they have been found, every round,
is by **running programs** — the specification's, the corpus's, and the one
somebody wrote while building a fixture for something else. These two came out
of the third kind: a probe of whether a package could publish a `trait`, written
to answer a question on [`open-decisions.md`](open-decisions.md) and not to find
anything.

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


### 1.1. A `sync` body is refused as pausing when the call leaves the unit

[ADR-078](specification/adr/adr-078.md) D4 makes a trait's methods `sync`, and
`NK1129` refuses an `impl` whose body pauses. It refuses ones that do not:

```nika
impl http::Handler for Fixed {
    fn handle(&self) -> http::Response {
        return http::Response::not_found()
    }
}
```

```
error[NK1129]: `Fixed::handle` can pause, and `http::Handler` declares it as a method that cannot
```

**The cause is one level below where this entry first put it.** It was filed as
a *package* boundary; it is a **unit** boundary, and the shorter reproduction is
two files of **one** package with no packages in sight:

```nika
// helper.nika
pub fn plain(n: i32) -> i32 { return n + 1 }

// main.nika
impl Simple for Thing {
    fn go(&self) -> i32 { return plain(self.n) }   // NK1129
}
```

`Ledger::infer` is called once per **unit** (`modules::Program::of`), so
`sync::infer`'s graph is that one file's. A callee outside it is in neither
`own` nor `std`, and `reach_of` sets `blocked`. `Sync::No`'s own documentation
says it means *"something it calls can pause, **or** something it calls cannot
be resolved and therefore cannot be vouched for"* — and every reader takes the
first meaning. `NK1129` is simply the first reader where that shows.

*Measured, both ways:* the two-file program above is refused by the compiler as
it stands, and a settling pass — re-running the assembly with the previous
round's ledger as the library, until it stops changing — makes both it and the
`http::Handler` program compile.

**And that settling pass is why this is not the small fix the entry first
claimed.** It was built and reverted, because it breaks
`the_rules_that_come_with_a_path_dependency`:

```
error: app/src/main.nika:4:5: `i64` is not a future
```

With three packages — `app` → `http` → `deeper`, and `http::ok` calling
`deeper::two` — **the two builds of `http` stop agreeing about it.** `http`'s own
build settles and makes `ok` `sync`, so its crate declares a plain `fn`; `app`'s
build cannot see `deeper` at all, so it still reads `ok` as pausing and writes
an `.await`. Today they agree only because both are equally ignorant.

*Why that is not a bug in the settling:* it is
[ADR-053](specification/adr/adr-053.md) D3 working as decided. A transitive
package is deliberately not in this program's ledger — *"nothing below it is in
this program's ledger"* — so a consumer **cannot** reproduce what the dependency
computed about itself. Any pass that improves a package's own answer diverges
from what its consumer can derive, unless the consumer stops deriving it.

*What closes it is [ADR-100](specification/adr/adr-100.md)*: the inference
graph is the package rather than the file (D2), which is the two-file
reproduction and needs no ledger from anybody, and a consumer reads a
dependency's ledger rather than deriving it (D1), which is the three-package
one. The settling pass is not the shape of the fix — it derives on the
consumer's side, which is the divergence — and stays reverted. The work is
§2.13.

*What must not be done meanwhile:* relax `NK1129`. The refusal is right about
what it reads; what it reads conflates two facts. Making it quieter would trade
a false refusal for a silent miscompilation, which is the worse half of
`docs/README.md`'s list.

*Evidence:* the two programs above, the reverted pass, and the three-package
test that fails under it — all run.


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

### 2.1. A task that may not cross a thread is refused by `rustc`, not by this compiler

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

### 2.2. A lambda that pauses is refused, and a recursive pausing method is not boxed

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

*What it needs:* `Op::Readiness` against standard input's descriptor, and a
`Lines` whose step is a future. The second half is the larger one and is a
question of its own: a `for` over a **stream** is `while let Some(x) =
s.next().await` in the language below, and Rust has no stable trait for one. The
parallel is [ADR-025](specification/adr/adr-025.md) D6's `iterates_fallibly` — a
property of the *type*, recorded in the ledger, that makes the emitter write the
step differently — so the shape to copy exists.

### 2.4. The lock is built and every rule around it is not

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
* `NK2201`, `NK2203` and `NK2503`, catalogued and not emitted. **Two of the five
  left this list** — `NK2204` and `NK2205`, the two that come with the doors
  ([ADR-099](specification/adr/adr-099.md)) — and they went first because they
  are **local**: each is one statement, while `NK2201` and `NK2203` need to know
  what is *inside a door* and what a chain of calls reaches, and `NK2503` needs
  the reachability walk [ADR-039](specification/adr/adr-039.md) D6 describes as
  `NK2502`'s generalised.
  **`NK2202` is a second exception and is raised** — `sync::check` finds the
  calls that contradict an assertion and `diagnostics` renders them — which is
  worth the extra words, because the bullet used to name the whole range and a
  reader would have gone looking for work that is done.

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

### 2.9. A grammar is entered by `dsl … from …`, and the record that replaced it is unbuilt

[ADR-082](specification/adr/adr-082.md) D1: a grammar is entered by an ordinary
call, `Json.value(input)`, and D2 makes every `pub` rule an entry. Accepted,
**Built: no** — *"a grammar name is not yet accepted as a callee"*.

*Why it is here rather than in a subordinate clause:* it was in one. The entry
about **running a grammar while the program is built** named it as *"the case
ADR-082 rewrote the syntax for"*, and nothing else on this page did — which is the exact pattern the entry below this one was written about, two
rounds ago: a record waiting on work that the list people read does not mention.

*What it fixes, in the record's words:* the emitter picks the entry rule with

```rust
def.rules.iter().find(|r| r.is_public && par_fold_of(r).is_some())
    .or_else(|| def.rules.iter().find(|r| r.is_public))
```

so a grammar with two `pub` rules gets one of them by source order, silently, and
a `par_fold` one beats an earlier one. **That is a live defect and not merely an
unbuilt decision** — it is filed here rather than in §1 only because the record
that closes it is written and the repair is the migration, not a patch.

*And the free moment has passed.* ADR-082 §5 gave *"no program in the tree writes
it"* as the reason to **remove** the old form rather than deprecate it. Counted
while [ADR-091](specification/adr/adr-091.md) ran a new refusal over the corpus:
**eight programs write it on nine lines** — `1brc`, `access-log`, `calc`,
`config`, `inventory/stock`, `json`, `k-nucleotide`, `report` — plus Part II 10.2
and three test files. The record now carries the count; whether removal is still
right is a decision and is in
[`open-decisions.md`](open-decisions.md).

*One thing the migration has to carry with it:* the entry call needs a `throws`
in its contract. Today `dsl … from …` is told it is fallible by one line in the
checker ([ADR-091](specification/adr/adr-091.md) D4), because it is not a call
and has no contract. As a call it would be answered from one — and a generated
entry rule carrying no `throws` would meet `NK1134` at all eight of those
programs, each of which writes `catch` beside the entry.

*Evidence:* the eight files, listed above, found by `grep` and confirmed by the
refusal that ran over them.

### 2.10. Nothing runs Nikaia code while the program is built

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

### 2.11. Running a grammar while the program is built is not interpretation

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

### 2.12. The caller writes the `&`, and the record says the compiler does

[ADR-094](specification/adr/adr-094.md). A parameter is a view unless its body
keeps the value, a `keeps` column records which, the emitter writes the
reference at the call, a `for` lends, a `let` over a place is a view, and
`mut` on a parameter is where in-place change is written. **None of it is
built**: every `&` in `examples/` is the caller's, `for x in xs` consumes `xs`,
and `xs.len()` on the next line is `rustc`'s *use of moved value* about a file
nobody wrote — reproduced with a nine-line probe, and the review that found it
is [`language-review.md`](language-review.md) §1.1.

*Evidence:* 42 `&` at calls and loop heads in 913 non-comment lines of
`examples/`, each repeating what the callee's signature says;
`examples/report.nika`'s comment explaining that `count` has to be read before
`page(entries, total)` "consumes" them.

*What it needs, in the record's own order (§5):* the `keeps` inference and
column first, because it changes no program and can be diffed against the
corpus; then the `for` and `let` half, which needs no ledger; then the emitter
writing the argument off the column and refusing a written `&`; then `mut`
parameters; then the cleanup-point narration of D5. Step 3 is the one that
rewrites every example, so the examples are the test.

*Why it is here and not in §1:* nothing is miscompiled. It is a message in the
wrong words at every site the caller forgets the `&`, and a tax at every site
they remember it.

### 2.13. A consumer reads a dependency's ledger, and the inference graph is the package

[ADR-100](specification/adr/adr-100.md). A package's ledger is inferred over
all of its units at once and written by its own build; a consumer reads it and
believes it while the per-unit source hashes in its header match, derives it
again where they do not, and compares bytes only under `--locked`. **None of it
is built**: `modules::Program::of` calls `Ledger::infer` once per file, the
header carries no hash, and a path dependency's ledger is neither read nor
written. §1.1 is the defect this closes.

*Evidence:* §1.1's two programs — two files of one package, and
`app → http → deeper` — both run.

*What it needs, in the record's own order (§5):* the package-wide graph first,
because it closes the two-file case with no file format change; then the
`[sources]` table in the header; then reading and writing a path dependency's
ledger in dependency order; then `--locked` over path dependencies; then the
boundary translation of D6.

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
