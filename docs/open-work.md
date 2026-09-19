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

And **a grammar action that pauses, lowered to invalid Rust** — a rule's action
is arbitrary Nikaia, so it could call something that pauses, and nothing refused
it: `.await` went into the synchronous parser the `grammar!` macro generates and
the backend answered *`await` is only allowed inside `async` functions*. It
waited on a **ruling** rather than on work, and the ruling is
[ADR-142](specification/adr/adr-142.md) D1 — an action may not pause, which is
the demand an `overlap` branch and a `par_iter` lambda already carry, in the
place a parser needs it. `NK2209`.

And **a method call's options, dropped on the way to the language below** —
`q.execute(target_age: 30)` came out as `q.execute()`, because the emitter fills
options from the callee's contract and finding a *method's* entry means resolving
the receiver, which it cannot do ([ADR-028](specification/adr/adr-028.md)).
`rustc` answered about the arity of a file nobody wrote. Older than the spelling
that makes it easy to hit, since the leading-`;` form went down the same path,
and closed by the hand-over `lent_args` and `nullable_args` already use
([ADR-133](specification/adr/adr-133.md) §5). Part III 15.1's
`script.exec(msg: message)` is the block that proves it: the specification's
lowering floor went **up**, 47 to 48, for the first time it has moved that way.

Each is in the CHANGELOG with what it
was and what fixed it; a fixed entry kept here only makes the list longer to
read.

**One entry, below, and it arrived by building something else.** Before it this
section was empty, and the last entry to leave it was wrong on both of its
claims. It said an
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


### 1.1. A grammar's entry claims nothing, and every caller inherits that

**Found by building [ADR-140](specification/adr/adr-140.md) D3**, which is the
only reason it is visible: a grammar used to be entered through a **method**
call, and the analyses answer *nothing* about a method whose receiver they
cannot resolve. `::` makes it a call by name, so they read the entry's contract
instead — and the entry's contract is `..Default::default()` with `throws` and a
signature on it, because nothing derives its columns.

*Reproduction:* `examples/inventory/nikaia.contracts`, in the diff of that
change. `read`, whose body is one `Stock::file(data)`, went from
`sync = "inferred"`, `keeps = ["data"]`, `touches = []` to none of the three and
`locks = "?"` — and its lowering went from `pub fn` to `pub async fn`.

**The direction was right and the answer was poor**, which is why this was an
entry and not a revert. Withholding a promise on doubt is
[ADR-010](specification/adr/adr-010.md) D1's polarity, and the *old* answer was
the analysis failing open: a rule's action is arbitrary Nikaia and could pause,
and the method shape let the caller keep a promise nobody had derived.

**The sharpest column is answered and the other three are not.**
[ADR-142](specification/adr/adr-142.md) D1 says an action may not pause, so an
entry is `sync` by construction and D2 writes the column — `read` is
`sync = "inferred"` and `pub fn` again. What D1 decided nothing about is what an
action *keeps*, *touches* or *locks*, so those three are still `?` and still
want the derivation: a `pub` rule's action blocks are ordinary bodies, and
`keeps`, `touches` and `locks` can be inferred over them and folded into the
entry the way a function's are.

*Every example still runs*, at both settings, which is what said this cost
information rather than correctness — and `sync` has since been paid back in
full.

**And one more left it, filed in the change before this one and closed in this
one:** *a task nobody joined was left unwoken, about two runs in five.*

`nikaia-std`'s `a_future_fed_from_a_worker_finishes_under_block_on` had been red
at that rate through every change of a long session, and it was not a slow test.
It was **four instances of one mistake**, and the mistake has a name: *an
operation that has already answered is invisible to a caller that asks whether
anything is outstanding.*

The count of I/O completions (`rt::io::generation`) is what remembers that an
answer arrived; `pending` is what says one is still coming. A reply lands in its
channel, `pending` drops, and for a moment **neither** says anything — so a
caller that reads `pending` first concludes *nothing can move*. `exec::block_on`
then either panicked about a waker nobody arranged or spun out the whole
`cleanup-deadline` and abandoned a task whose answer was already in its channel,
which is [ADR-055](specification/adr/adr-055.md) D5 — *a task nobody joins still
runs* — quietly false.

*The four:*

| where | what it read first | what it should read first |
| :--- | :--- | :--- |
| [`uring::Ring::park`](../crates/nikaia-std/src/rt/uring.rs) | `unreaped == 0 && !elsewhere` → *nothing to wait for* | the count. The generation check was **already there**, three lines below the early return that made it unreachable |
| [`io::park_for`](../crates/nikaia-std/src/rt/mod.rs), the blocking half | `pending() == 0` → `false` | the same count, for the same reason, on the other mechanism |
| [`worker::run`](../crates/nikaia-std/src/rt/worker.rs) | dropped `pending` and *then* rang the bell | the bell first, so the two are never both silent |
| [`exec::block_on`](../crates/nikaia-std/src/rt/exec.rs) | rang the tasks' alarms only after a park that **waited** | also when the count moved before the round began — an I/O future stores no waker, so the count is the only thing that can say *poll everyone again* |

*Each one is load-bearing*, which the measurements say rather than the reasoning:
closing them one at a time took the failure rate from about four runs in five
(with the test strengthened, below) to three in five, to one in six, to none in
twenty-seven.

*And the test was the other half.* It caught a **race** once in two and a half
runs, which is a test that reports *no defect* three times out of five — so it
runs its body twenty times now. Twenty rounds of a coin that lands red two times
in five come up green by luck once in twenty-five thousand runs.

### The most negative `i64` has no spelling### The most negative `i64` has no spelling

*Found by building* [ADR-136](specification/adr/adr-136.md), and small enough
that it is here rather than in §2: `-9223372036854775808` is refused, because
`-` is a **unary operator** over a positive literal and `9223372036854775808`
does not fit an `i64`. Every other number in the range is writable.

*What it replaced is worse and that is why it shipped*: the parser read the
digits with `parse().unwrap()`, so the same program took this compiler down.
A refusal that names the range is the honest state; a gap in it is still a gap.

*What it needs:* the literal carried as the `i128` the fold already uses, or a
negation folded in the parser where it sits directly in front of one — and the
second is the smaller change, since `Expr::LitInt` is an `i64` everywhere else
and widening it touches every reader. Nothing in the tree writes the number, so
this is a completeness item rather than a blocker.

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

**Answered by [ADR-123](specification/adr/adr-123.md):** `crosses` takes `false`, the describer writes it for a type whose fields hold what cannot be sent, and the refusals fire on it. The entry stays until they do.

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
`contracts::send` gains a second producer.

*The second producer has arrived and cannot speak.*
[ADR-104](specification/adr/adr-104.md)'s described crate is here — the entry
below is built, and `examples/foreign-runtime/` ships a description of
`hyper_shim`, whose `LocalHandle` holds an `Rc<String>` and provably does not
cross. **The file has no way to say it.** `crosses` is a **boolean**:
`crosses = true` answers `May` and its absence answers `Undecided`, so the
column has two answers where the verdict it feeds has three, and *it does not
cross* is the one it cannot spell.

*That is a sharp, small question and it is in*
[`open-decisions.md`](open-decisions.md): whether `crosses` becomes
three-valued, the way `locks` did ([ADR-039](specification/adr/adr-039.md) D3)
and for the same reason — a two-answer column feeding a three-answer verdict
loses exactly the answer that refuses something.

*So this entry is now about one column in `send.rs`* rather than about the
checker, and it stays open for that reason rather than for the one it was filed
under.

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

### 2.2. A lambda that pauses is refused where `std` takes it

[ADR-055](specification/adr/adr-055.md) §6's remainder, and a limit of this
compiler rather than of the language — so it is here and not in §1, where a
defect is the compiler being *wrong*. **The other half of this entry, a
recursive pausing method not being boxed, is built**; what it took is below.

**A lambda whose body calls something that can pause is refused at the build —
where the parameter it is handed to is `std`'s.** Rust has no stable `async`
closure, so the lowering has nothing to write for a `std` entry that takes one.
The refusal is by the lowering and not by the checker on purpose: refusing it in
the type checker would refuse a correct program (Part III, C.4).

**Where the parameter is declared in *this* language it is not refused any
more** ([ADR-122](specification/adr/adr-122.md) D1, D2): the type says the code
may pause, the declaration is a closure returning a boxed future, and the lambda
is `|a| Box::pin(async move { … })` — a body that pauses is an ordinary body
inside it.

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
per group. What was left is that a *callee's* parameter has to say which it
wants, and the ledger had no column for it. Not a new mechanism — a claim to
record.

**This is closed.** [ADR-102](specification/adr/adr-102.md) D1's function type
was the claim it was waiting for, and
[ADR-122](specification/adr/adr-122.md) D1 said which shape a declaration
commits to: the **type** decides, so a parameter that may pause is a closure
returning a boxed future and a lambda handed to one is
`|a| Box::pin(async move { … })`. A body that pauses is an ordinary body inside
it, which is D2 — *"the build-time refusal has no case left"*.

*Except where `std` is*, which is the one thing that keeps the refusal alive:
`std`'s lambda-taking entries describe **Rust** signatures that take a plain
closure, so a pausing lambda handed to `map` still has no shape and is still
refused at the lowering. That is D3's own exemption read from the other side,
and it is what is left of this entry: a `std` entry whose lambda may genuinely
pause would have to be written in Nikaia or described as taking a future.

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

### 2.3. A `for` over a stream has no suspension point

**[ADR-121](specification/adr/adr-121.md) closed the first half of this entry
and renamed what is left.** The defect was the ring's park, which could not hear
a worker's bell; an eventfd on the ring makes every worker operation awaitable,
and the proposal this entry used to carry — wire standard input to readiness —
is withdrawn, because readiness was never the problem.

**`io::read` and `io::read_to_string` suspend now.** The read is the blocking one
standard input always had, performed on an I/O worker and awaited: no second read
shape, and no ring path for a stream that has no size to `stat`. What changed is
the park.

**`io::lines` does not, and the reason is the language.** A step of it is an
`Iterator::next`, and a suspension point inside one is `while let Some(x) =
s.next().await` in the language below — a `Stream` trait Rust has not stabilised
and a `for` over a stream this language has not decided. This entry always said
that was the larger half; it is now the whole of it.

*Evidence: none, and none is possible yet.* A caller sees a step that returns,
which is what it saw before. What is missing is that the thread is **held** for
the duration of `for line in io::lines()` rather than given up — which nothing
can observe until something else wants the thread, and at
`user_parallelism = yes` something can: a `main` blocked in `io::lines()` is a
thread the pool could have had.

*The shape to copy exists.* [ADR-025](specification/adr/adr-025.md) D6's
`iterates_fallibly` is a property of the **type**, recorded in the ledger, that
makes the emitter write the step differently — which is what a pausing step would
need. What it waits on is a ruling about the `for`, not work: it is a question,
and when it becomes one it belongs in [`open-decisions.md`](open-decisions.md)
rather than here.

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

[ADR-062](specification/adr/adr-062.md). Nothing of it is built: `extern "C"` is
a parse error (Part III 15.1), `Target` has two values, and this repository has
no notion of a linkable artifact — no `cdylib`, no `staticlib`, no `.so`.

**The owner has since named it as open**, so it is no longer waiting on scope
([`project_status_and_roadmap.md`](project_status_and_roadmap.md)). **Talking
*to* C came first** and is 15.1's, and it is built:
[ADR-124](specification/adr/adr-124.md) reserved `extern` and `unsafe` *with
their constructs* — the number that allowed two words was zero — and an
`extern "C"` block, an `unsafe { … }` and `NK1143` are all there. What that
record left open is `Pointer[T]`, and what **this** one still waits on is a
**target**: `Target` has two values and there is no linkable artifact anywhere in
the repository.

*What it needs, in the record's order:* the target and its artifact kind (§4
leaves both open), and then the analysis taking the exported entry points as
roots seeded at the floor — which is the same order of work as the crossing
roots it already seeds, and which D3 says needs no change to any check.

*Why it is written down anyway:* it is the one direction that touches
`user_parallelism` at its root. A target added without it answers "who owns the
threads" by accident, and the answer is invisible — a caller's second thread and a
plain reference count under it is a data race with nothing to see at the source.

*What it needs, when something asks for it:* the analysis takes the exported entry
points as roots seeded at the floor, the way it already seeds crossing roots. The
checks need nothing — [ADR-045](specification/adr/adr-045.md) D1 kept every verdict
off the switch, so a library is already checked for the world it would enter.

### 2.9. The build-time evaluator has a call and a loop and no `push`

**The call is built** ([ADR-073](specification/adr/adr-073.md) D5's second
stage), which is what this entry was mostly about: a `comptime` initialiser may
call a function of this program, and what the called body may be made of is
arithmetic, comparisons, an `if`, `let`s, a `return` — and another call,
including a recursion with a base case.
[`crates/nikaia/src/build_time.rs`](../crates/nikaia/src/build_time.rs) is the
interpreter; [`fold.rs`](../crates/nikaia/src/fold.rs) stays in front of it,
because it is what says which integer type a *declaration* pinned.

*What bounds it was already decided and is read rather than re-invented:*
[ADR-075](specification/adr/adr-075.md) D1 and D2 are two **ledger columns** —
`sync`, and a touch set that is empty or exactly the build's own parameters — so
the interpreter decides nothing about safety. `NK1152` is a callee the rule
forbids, and it is deliberately not `NK1127`: one says *not yet*, the other says
*not allowed*.

*And D4 needed one neighbour.* There is no step budget, so a body that does not
terminate hangs the build — that record's own accepted cost. A **recursion**
that does not terminate is a different failure, because it takes this compiler's
stack with it, so the call depth is bounded at a limit no terminating program
meets and the message says that is what it is.

*And the loop is built too.* A `for` over a range, a `while`, `break` and
`continue`, and an assignment — `t += i` — because a loop that cannot change
anything is not one. Reading it took the evaluator's block apart: a block used to
mean *a value or an error*, and a loop's body is a third thing, a block that runs
to its end and produces nothing. That is what the `Flow` in
[`build_time.rs`](../crates/nikaia/src/build_time.rs) names, and it is also what
makes `if n < 2 { return 1 }` work as a statement wherever it stands rather than
only where it is not last.

*What is left is `push`, and after it the field walk.*

*Reproduced (the part that is left):* the interpreter has no `push` and no
aggregate value, so [ADR-079](specification/adr/adr-079.md) §3's table can be
*computed* and has nowhere to arrive: what a `comptime` hands to the language
below is what Rust's `const` can hold, and today that is one integer or one
`bool`. [ADR-135](specification/adr/adr-135.md)'s literal is **built**, so a
list can be *written* now; what is still missing is a type it can cross in, and
that is [ADR-152](specification/adr/adr-152.md)'s `Array[T, N]` — a `Vec`
allocates and Rust's `const` cannot hold one. Text is not in it either.

*Why it is work and not a question:* three records decided what may happen and
none of them can happen.

| record | what it wants of the evaluator |
| :--- | :--- |
| ~~[ADR-073](specification/adr/adr-073.md) D5~~ | ~~a **call** in an initialiser~~ — **built**; what still waits behind it is [ADR-072](specification/adr/adr-072.md)'s file reading, which needs the allowlist rather than the evaluator |
| [ADR-079](specification/adr/adr-079.md) §3 | a **loop and `push`**, to build a table that then crosses as a view — the loop is **built**; `push` needs a value to push onto |
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
reading a file at build time and is the smallest of the three — **done**. Then
the **loop** — **done** — and `push`, which is not the same step: the loop is a
shape the interpreter reads, and `push` is a *value* the whole compiler has to
be able to carry from the build into the program. The field walk last, because it
needs something the other two do not — see the entry below, *running a grammar
while the program is built*.

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
to rebuild. Today it would reach the user in the backend's words about the
generated file, which is [Part III C.1](specification/30-nikaia-tooling.md)'s
class.

**It is not small work waiting to be done; it is work with nothing to fire
on, and the evidence the entry asked for is now here.** The way to get some was
to edit a hash, and editing one says the case does not arise: `drive` lowers
every member **dependencies-first** and writes each one's `nikaia.contracts`
*before* the member that depends on it is lowered (D5), so an edited ledger is
overwritten by that package's own build in the same run. Under `--locked`
nothing is overwritten and the comparison fails with D4's narrated diff, which
is the message that case is owed. So **every ledger a build reads today is
re-derived from sources in the same run**, and there is no believed-and-unchecked
ledger for D6 to be about.

*Evidence, and it is a test now:*
`a_hand_edited_dependency_ledger_is_repaired_before_it_is_read` in
`crates/nikaia/tests/project.rs` takes `sync` off a package's `hello` — the
claim that would make its consumer `.await` an `i64` — leaves the source hashes
alone so D3's belief still stands, and watches the build repair it and print
`1`.

*What it waits on:* a ledger the build **cannot** re-derive — which is *a
package is found by version through Cargo* below, where the sources are not
there to derive from ([ADR-103](specification/adr/adr-103.md)), or *a foreign
crate is described before it is called*, where there is nothing to derive it
*from* ([ADR-104](specification/adr/adr-104.md)). Either makes D6 writable and
testable in the same change, and the test above is what says the day has come.

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
are the type's. **Steps 1 to 3 are built**: the type parses, the ledger writes it and
reads it back — a function type inside another one included — D2's reading is
in the fit, and a **run** parameter lowers to a closure argument,
`impl Fn(A) -> R`, with the `Result` a `throws` function's declaration has
where the type says `throws`. `fn twice(x: i64, f: fn(i64) -> i64)` is a
program that compiles and runs.

*The trailing words are greedy*, which settles the one ambiguity the record
does not name: in `fn make() -> fn(i64) -> i64 sync` the `sync` belongs to the
**result type**, and a function whose own promise is meant writes it before
the arrow.

*Step 2 is built too.* `NK2206` for a lambda that pauses where the type says
`sync`, `NK2606` for one that fails where the type declares none — each the
shape of the refusal one level down, `NK2202` and `NK2605`. `NK2606` is the
whole message: the function *around* the lambda is not the one that has to
answer for a failure the type refuses.

*What it needed first was [ADR-029](specification/adr/adr-029.md)'s own
ordering*, which the **method** path had and the free path did not: a free call
walked its arguments before it resolved its callee, so `hand(fn(n) { … })` left
`n` with no type at all and the promises had nothing to be asked of. A free
call's lambda parameters are typed now.

*Step 3 is built too.* A parameter the body **runs** gives the function
`sync = "from(f)"` — *the lambda decides* — and one it **keeps** is answered
from the type, so a kept `fn() sync` cannot pause and a kept `fn()` takes the
claim away; a body that stores *and* runs gets the kept answer, which is the
record's §4 and the safe one. `twice` used to lower to an `async fn` every call
awaited and is an ordinary function now.

*`Sync::From` is an outcome of **inference** now*, where before only a written
`std` entry produced one — a value arriving in the greatest fixpoint where
every other outcome is a promise being taken away. It is sound for
[ADR-029](specification/adr/adr-029.md) D3's reason and for one fact of this
compiler: `visit_expr_blocks` walks the body of a lambda passed as an
**argument** and not only a trailing one, so every lambda a caller writes is
counted in the caller whichever parameter the column names.

*And the shortcut that was not one:* reading a call to a run parameter as *does
not pause* would have made `twice` `sync = "inferred"`, a promise that fails
open the moment a caller hands it a pausing lambda —
[ADR-010](specification/adr/adr-010.md) D1's polarity exactly.

*One thing the record had not said, and an ordering settled it.* D3 says the
run-or-kept answer is `keeps` asked one level over, and `keeps::infer` runs
**after** `sync::infer`. So it is asked here in the small — the name is
mentioned, and every mention is the **callee** of a call — which fails closed
and agrees with the column on every shape either can see.

*What is left, in the record's order (§5):* D5's lowering of a **kept**
handler, which is *a lambda that pauses is refused* one entry up, with a callee
that can now say which shape it wants.

*Until that last one lands, a function type is a **parameter** and nothing
else.* A field, a result and a `let` are the positions where it can only be
kept, and they are `NK1142` here rather than `impl Fn(…)` in a Rust field,
which is not Rust and would reach the reader as the backend's words about a
file nobody wrote.

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
crate's version, and reviewed. **Step 1 is built, and step 5 by hand.**

*The refusal is `NK2504`*, once per crate and with the command in the
message — and only where the **manifest** declared the crate with
`type = "rust"`, because refusing on a qualified name nobody declared is
[Part III C.4](specification/30-nikaia-tooling.md)'s correct program refused.
A written **type** from such a crate counts as much as a call does.
`hyper-shim` in the manifest is `hyper_shim` in a program, which is the crate
name Cargo makes of the key.

*And `examples/foreign-runtime/` is described rather than the fixture*, which
is the half of step 5 that could be done without the command: the three
projects carry a `contracts/hyper_shim.contracts` written by hand from the
crate's `pub` signatures, which is what D5 says to expect. A fixture of the
refusal lives in `crates/nikaia/tests/described.rs` instead, where it needs no
network.

*What it needs, in the record's order (§5):* the draft from the sources; the
file's header and the hash rule — the description records the crate source's
SHA-256 today and **nothing compares it**; the rustdoc-JSON reader behind a
toolchain check.

*And something else waits on **one line** of it.* The entry above about the
crossing refusals being built and unreachable is unreachable **because** no
type answers `MayNot` into our own code, and a described foreign type is the
first thing that could. The description ships **no** `crosses` on
`hyper_shim::LocalHandle`, and that is the honest answer rather than a gap: the
type is not `Send` because of its **fields**, and a reader of signatures does
not have them — D3's table says `Send` *on a type* means it may cross, and the
absence of the word is not the claim that it may not. So the crossing stays
`Undecided`, and the day the describer reads fields is the day `NK2501` and
`NK2502` have something to say.

### 2.18. `par_iter` has no entry to demand `sync` of

[ADR-105](specification/adr/adr-105.md). **Steps 1 and 2 are built.** `Seq[T]` and
`Par[T]` are words of the ledger's type language: they parse with the two
trailing words, write themselves back, bind through their item, answer as a
**receiver** — `Seq::collect` is found exactly as `Vec::push` is, and a `Par`
falls back to `Seq`'s entries for D3's *otherwise `Par[T]` has `Seq[T]`'s
surface* — and a `for` over one binds its item. A program cannot write either
(D4), and `NK1135` says so by the rule that refuses `Widgit`.

The entries: `HashMap::keys`, `HashMap::values`, `String::chars`, `Vec::drain`,
`HashMap::drain`, `io::lines`, and six consumers under the receiver's own word.

*`io::lines` was the one that mattered.* It said `-> Lines`, a named type whose
`iterates = "throws"` carried the failing step — which worked for the `for` and
for nothing else: the binding had no type, so `line.len()` was a method on `?`.
`Seq[String] throws` says both things in one place, and the fallible-step rule
reads either.

*And §1's own attribution did not survive the measurement.* *Every one of the 35
downstream of a `?` that is a sequence* is not what the corpus shows: the harness
in `crates/nikaia/tests/sequences.rs` reads **38** before these entries and **33**
after, and the rest are downstream of a receiver with no type for other reasons —
`tail.drain()` in `json.nika` where `tail` is a field nothing types, `map` on the
result of a `catch` in `access-log.nika`. **Those are a separate entry**, and the
number is kept rather than remembered (`MethodCalls::unanswered`).

*Step 3 is built too.* D2's once-only rule is `NK2702`: a name whose sequence a
walk consumed is refused where it is read again, and what counts as a walk is read
**off the signature** — every `Seq` entry writes its receiver `(Seq[$T], …)` and a
container's writes `(&Vec[$T], …)`. It is `NK2101`'s analysis with one word
changed, so an assignment revives the name and a temporary is not asked about. The
fit gained its arm with it, which the first build had missed: a
`Seq[String] throws` did not fit a `Seq[String] throws`.

*What is left is step 4 alone,* and it waits on a program rather than on work:
D3's `sync` demand on a `Par[T]`'s lambda needs `par_iter` to have an entry to
demand it of, and nothing in `examples/`, in `tests/` or in `std` calls it —
`par_fold` is a grammar driver and not this. D4's *it waits for a program* applies:
the **word** is in the type language and binds, so the entry is one line the day
something asks for it, and the demand is the line after.

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

### 2.23. A cleanup the deadline cut off names the resource

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

### 2.24. `?.` reaches through a view

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

### 2.25. Reading a map through the brackets is a `T?`

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

### 2.26. An `overlap` keeps every failure

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

### 2.27. `from` is a name, and a file a build reads is `asset("…")`

[ADR-116](specification/adr/adr-116.md). `from` leaves the reserved list, so
`fs::rename(from:, to:)` parses as Part III writes it; the build-time read is
`asset("…")`, a call the compiler recognises in a `comptime` initialiser under
every rule the allowlist record already states. **Nothing of it is built**:
`from` is in `parser::RESERVED_WORDS`, and the read is unbuilt in either
spelling.

*Evidence:* Part II 10.6's `Json::value(asset("config.json"))` parses and is
refused as `NK1117` and `NK1127` — the page ahead of the compiler, in this
language's words, where the old spelling was a parse fragment.

*What it needs, in the record's order (§5):* the word out of the parser's
table with the `dsl X from e` message matching the bare word; `asset("…")`
when the second stage of `comptime` lands; the two `fs` entries.

### 2.28. `with` is a copy of a value with named fields changed

[ADR-118](specification/adr/adr-118.md). `p with { x: p.x + 1 }` is a new
value of the same type; the braces are the literal's, only the top level, the
unnamed fields are moved, and across a package only `pub` fields may be
named. **Nothing of it is built**: `with` is a reserved word without a rule.

*What it needs, in the record's order (§5):* the grammar rule and AST node;
the checker's field resolution and refusals, the enum operand refused; the
lowering to a struct expression with a base; `examples/1brc.nika`'s `Stats`
and a test.

### 2.29. A target without an operating system

[ADR-119](specification/adr/adr-119.md). A bare-metal target with
`user_parallelism` pinned to `no`; `no_std` emission with a target prelude
and abort; the target's executor over described crates with interrupts as
wakers and `irq::on(vector, fn() sync)`; a heap by default and an
`allocation = "startup"` profile over a derived `allocates` column; locks as
critical sections; runtime settings baked at build time. **Nothing of it is
built**: the compiler knows `x86_64-linux` and `wasm32-unknown`, and `std`
has one Rust half. Scheduled after the HTTP server.

*What it needs, in the record's order (§5):* the target and the pin;
`no_std` emission; the `std` half over described crates; `allocates` and the
profile; build-time settings and the deadline; the availability rows and a
first program.

### 2.30. A grammar's action is the block after the pattern, and two borrowed names go

[ADR-120](specification/adr/adr-120.md). Part II 10.8 is the normative page
of everything a grammar may write. A rule's action is `{ … }` after its
pattern with no second arrow; `tag("x")` is `"x"` and `digit1` is `digit+`,
both refused with the spelling. **Nothing of it is built**: grammars write
`-> { … }`, and the engine's names pass through.

*Evidence:* 53 action arrows in `examples/` (`json.nika` 18, `calc.nika` 11,
`config.nika` 8, `k-nucleotide.nika` 5, `access-log.nika` 4, `report.nika`
4, `1brc.nika` 3), and the grammar blocks of Part II 10.1, 10.6 and 10.7,
which now write the new form and are fragments until the parser takes it.

*What it needs, in the record's order (§5):* the parser's block-after-pattern
with the arrow form refused; the emitter writing the engine's arrow; the two
names refused; the examples rewritten.

### 2.31. The ring's park hears the bell

[ADR-121](specification/adr/adr-121.md). **Built, except `io::lines`.** The ring
carries an eventfd with a poll always armed and its own user data; the bell
writes to it as well as bumping the fallback's count; the bell's completion is
answered by draining the descriptor and arming the next poll and is not counted
among the ring's outstanding jobs, so a park with only the bell armed still
sleeps. `rt::io::wait` has a future beside it, and `io::read` and
`io::read_to_string` are awaited reads on a worker.

*What the building found that the record had not named:* a **poll** and not a
read, which keeps `uring`'s soundness rule out of the bell entirely; the
descriptor beside the lock rather than inside it, because a worker that had to
take that lock could not wake the thread holding it; the bell **coalescing**, so
the park checks the fallback's own generation inside that lock; and a park with a
limit **bounded** by `IORING_ENTER_EXT_ARG`, because after D1 an unheard bell
would be a hang where there had been a panic — which is D3 read strictly.

*What is left is `io::lines`*, and it is §2.3 above rather than work here: a step
of it is an `Iterator::next`, and the `for` over a stream that would give it a
suspension point is undecided.

### 2.32. A function-typed parameter lowers by its type

[ADR-122](specification/adr/adr-122.md). Without `sync` the future shape, run
or kept; with `sync` a plain closure; the refusal of a pausing lambda at a run
parameter goes; the box on the common case is measured before the record is
closed. **D1, D2 and D3 are built.** A parameter whose type may pause is
`impl Fn(A) -> Pin<Box<dyn Future<Output = R>>>`, a call to one carries an
`.await`, and a lambda handed to one is `|a| Box::pin(async move { … })`.

*The number is §3's:* **15.1 ns per call against 0.33 ns**, about ×45, with the
control tying (`benches/handler`). Large as a ratio and small as a number, and
which of the two matters is D1's whole argument: a handler answering a request
spends microseconds, and a lambda run a million times over a list is what
`sync` is the door out of.

*One thing the record had not said, and the corpus said it in one line.* D3's
*`std`'s own entries are untouched* is a **condition on the check** rather than
a remark: `HashMap::and_modify` describes a *Rust* signature, which takes a
plain closure whatever the ledger's `sync` says, and writing the future shape
for it produced *expected `()`, found `Pin<Box<…>>`* against
`examples/access-log.nika`. The shape is written only where the signature was
declared in this language.

*What is left is step 4*, `fortunes.nika` as the corpus program: the handler is
writable now, and what it waits on is `examples/http/` declaring `route` — which
is ADR-102's consequence and needs the package rewritten rather than the
compiler changed.

### 2.33. `nikaia describe` does not write `crosses`

[ADR-123](specification/adr/adr-123.md). **Built, except the command.** The
column has three values, the ledger writes and reads both claims and stays
silent for the third, a third spelling is refused rather than guessed at, the
crossing verdict answers *may not* off `crosses = false`, and
`examples/foreign-runtime/`'s three descriptions say it for the handle over an
`Rc<String>`. `NK2501` and `NK2502` have their first end-to-end tests — the
first either code has ever had, because no type the records named could answer
*may not* until now.

**What is left is D2's *who* rather than its *what*:** `nikaia describe` writing
the column from a foreign type's fields, which is §2.17's step 2 and waits with
it. Meanwhile the line is hand-written like the rest of the description, and the
file says which part of the crate was read for it —
`inference = "described-from-signatures+fields"`, because `crosses = false` is
the one claim there that no signature could give.

*And one hole the claim exposed rather than made, which is not this record's.*
`NK2502` asks its question of a call **nothing** describes, which is ADR-038 D7's
own wording, so a *described* foreign call is not asked — and no column says
whether a described foreign function puts what it is given on a thread.
`examples/foreign-runtime/crossing` is that shape exactly: its handle says
`crosses = false` and it is handed to `hyper_shim::across_a_thread`, which the
description names, so the program is still refused by `rustc`'s `Send` bound
against the `.nika` line. `a_described_foreign_call_is_not_asked_about_crossing`
asserts the silence so nobody rediscovers it. A column for it is a **question**
and belongs in [`open-decisions.md`](open-decisions.md) when somebody asks it.

### 2.34. A C declaration cannot name a pointer

[ADR-124](specification/adr/adr-124.md) §4, and the only part of that record
left. `extern` and `unsafe` are reserved words with constructs, an
`extern "C"` block is a program, `unsafe { … }` is how a call to one is
written, `NK1143` is what a call outside one meets, and the whole thing
compiles and runs — `extern "C" { fn getpid() -> i32 }` prints a process id.

*What is left is the example on the page.* Part III 15.1 writes
`fn malloc(size: usize) -> Pointer[u8]`, and `Pointer[T]` is a type nothing
declares. **A pointer that outlives what it points at is the one thing this
language is built not to allow**, so a type for one wants a record with a
lifetime story rather than a name — which is why that record leaves it open
rather than adding it.

*What is writable meanwhile* is every C function whose signature names types
this language already has: most of `libc`'s arithmetic and process surface, and
none of its memory surface.

### 2.35. A library for other languages

[ADR-125](specification/adr/adr-125.md), all of it. A `pub extern "C" fn`
with a body is an entry point of a library, and `artifact = "c-library"` in
`[build]` makes the package one. What a C caller gets is deliberately narrow:
numbers by value, text and bytes in as pointer and length, text and bytes out
into a buffer **the caller owns** (size, capacity, written; a `NULL` buffer
asks for the size), a struct as an opaque handle with `_new` and `_free`, an
enum as numbered constants in declaration order, an optional as `NULL` or the
package's `NONE`. Every entry point returns an `int` status and puts its
values in out-parameters; a `throws` variant is a positive code, the library's
own failures are the seven negative ones, and `<pkg>_last_error` carries the
site and the secondary list. A caller may hand the library its allocator
before `init`. A panic is caught at the boundary and poisons the library until
`shutdown` and `init`. A pausing function is exported blocking and as
`_async`. Every handle carries a lock, and a re-entrant call on the same
thread is a status, not a deadlock. The header is generated from the ledger
and carries its hash.

*What it needs, in the record's order (§5):* the parser and the four refusals;
the artifact and the safe shape at entry points; the wrapper per entry point;
the runtime surface (`set_allocator`, `init`, `shutdown`, `last_error`,
`free`, getters); the blocking and async forms and the handle lock; the
header generator and the naming; a library called from a C program in
`examples/`, and the test that links it.

### 2.36. A struct crosses the boundary by value

[ADR-127](specification/adr/adr-127.md), all of it. `pub extern "C" struct`
has C's layout (declaration order, C padding — `#[repr(C)]` below) and crosses
by value, in, out and as a field; a `Vec` of them is an array in and the
caller's buffer out, counted in elements; `T?` of one is the `NONE` status.
Fields are numbers, `bool`, `char`, payload-free enums and other such structs,
every field `pub`; anything else is `NK1145` naming the handle as the shape.
No lock around its methods — it is the caller's memory. The layout is in the
ledger, so `--locked` catches a change. The same type serves a declared C
function.

*What it needs, in the record's order (§5):* the parser; the field check and
`NK1145`; the emitter's `repr(C)`, passing, array and buffer; the header's
`typedef struct` and the ledger's field record; an example beside the
library's.

### 2.37. The symbol prefix is one line in the build

[ADR-128](specification/adr/adr-128.md), all of it. `symbol-prefix = "hc"` in
`[build]`, default the package name with `-` written `_`; a C identifier or
refused. No declaration renames its own symbol.

*What it needs:* the manifest key with its check; the header generator and
the emitter reading it.

### 2.38. An async call can be cancelled, and a stream is a callback

[ADR-129](specification/adr/adr-129.md), all of it. The `_async` form ends with
`<package>_op** op` (or `NULL`); `<package>_cancel` cancels the task at its
next pause point with `cleanup` run; `done` is called exactly once,
`E_CANCELLED` (`-7`) when the cancellation came first; `<package>_op_free`
after `done`. A function producing many results takes `fn(item) -> bool sync`,
whose `false` stops it, and the next item is produced only after the callback
returned; a returned list of text or handles is refused naming that shape.

*What it needs, in the record's order (§5):* the ticket, `cancel`, `op_free`
and the code; the `bool` callback row and the refusal message; a streamed file
and a cancelled fetch in the C example.

### 2.39. A WebAssembly library is the same entry point on another target

[ADR-130](specification/adr/adr-130.md), all of it. `target = "wasm32-unknown"`
with `artifact = "c-library"` makes `<package>.wasm`, `<package>.js` and
`<package>.d.ts` from the same declarations; `extern "wasm"` is refused. The
host takes buffers from `<package>_alloc`/`_free`; no `set_allocator`; a
handle is an offset wrapped in a class with `free()`; a pausing entry point
has only the callback form and the `.js` makes it a Promise with an
`AbortSignal` for cancel, the module's executor driven from the host's event
loop.

*What it needs, in the record's order (§5):* the target check with the two
exports and the absent forms; the `.js`/`.d.ts` generator; the executor
bridge and the Promise form; the library on a page, and the test in Node.

### 2.40. A binding is a generated file over the C library

[ADR-131](specification/adr/adr-131.md), all of it. `nikaia bind python`
writes a `ctypes` binding from the ledger (exceptions per variant, `str` and
`bytes` for buffers, classes with `close()` for handles, `IntEnum`, `None`,
generators for streams, an awaitable for `_async`); `nikaia bind js` is the
WebAssembly build's `.js`. No second artifact, no native Node add-on.

*What it needs, in the record's order (§5):* the Python generator over the
example library; the streamed and async forms; the `js` name; a test that
imports the binding.

### 2.41. `nikaia fmt` does not exist

[ADR-132](specification/adr/adr-132.md). **Built, except the formatter's line.**
After `else`, an `if` may stand where the block would: the `else` rule has a
second alternative and it makes that `if` the block's one statement, so the chain
reaches the rest of the compiler as the `if` inside an `if` it is and every rule
of `if` holds at every link with nothing asked of any of them. The emitter writes
Rust's own `else if` where the `else` block holds one `if` and nothing else, and
`examples/http`'s `status_line` is one decision rather than three `if`s in a row.

*Nothing was added to the language, and that is the claim worth a test.* `elseif`
is a **name** — the grammar is scannerless, so a word it has no rule for is read
as one — and `NK1117` says nothing declares it. An `elseif` that quietly became
the keyword would be a second spelling nobody decided on.

**What is left belongs to a tool that is not there.** D2's *`nikaia fmt` writes
`} else if cond {` on one line, and never unfolds a chain into nested blocks or
folds nested blocks into a chain* asks something of a formatter, and there is no
formatter: Part III's tool page names `nikaia fmt` and the CLI has `build`, `run`
and `lower-std`. So this is not a step of that record any more — it is a rule the
formatter is born with, and **the formatter itself is the entry**. Nothing else in
the tree waits on it, which is why it has sat unnamed: `cargo fmt` formats this
compiler's own Rust and no `.nika` file has ever been formatted by a tool.

### 2.42. `throw`, `return`, `break` and `continue` are statements

[ADR-138](specification/adr/adr-138.md). `=> throw NotFound` is a parse error, so
is `?? throw Missing`, and so is an `else` branch that is one `return`. Each has
to be written with braces that hold nothing together and produce no value.

**The type side is already decided**, which is what makes this grammar work
rather than a language question: [ADR-093](specification/adr/adr-093.md) gives
the never type, and `NK1133` — a statement after a `break` in the same block is
refused — is what keeps `break x` from becoming a quietly dropped value one
position over.

*What it needs, in the record's order (§5):* the four as expression alternatives
with the statement forms kept; the never type fitting every expected type in the
checker, so a `match` whose arms are a value and a `throw` is typed by the value;
the specification's error chapter and `examples/` written without the braces, and
a test per position — an arm, a `??` right side, an `else`.

### 2.43. The ledger has nowhere to put a sentence

[ADR-139](specification/adr/adr-139.md). `nikaia.contracts` **ships** with a
package and is the one file a consumer's compiler reads about a dependency — every
signature, every promise, every restriction — and it carries no prose. The prompt
bundle on the roadmap has the same hole from the other side. `///` is an ordinary
comment today and a comment in the source does not travel, because the source of a
published package is not what a consumer reads.

*What it needs, in the record's order (§5):* the lexical rule that **keeps** what
`WS` throws away, for a run of `///` immediately before an item; the field on the
AST's items; the `doc` column in the ledger's parse and render, for a `pub` `fn`
or `type` only; the derivation, which makes it a pure function of the sources like
every other column; and `NK2401` staying silent about prose, because a changed
sentence is not a changed contract.

### 2.44. `use std::…` brings a name in and a package's `use` does not

[ADR-140](specification/adr/adr-140.md) D5.
[ADR-046](specification/adr/adr-046.md)'s rule is *no name is brought in*, and
`std` is the one place it is not followed: `use std::collections::HashMap` gives
the file `HashMap`. What goes is the `use` **acting differently** depending on
what follows it, not the prelude — `Vec`, `String` and `HashMap` need no `use` at
all and that is unchanged.

*What it needs:* the `std` arm of `use` resolving to a prefix like every other,
`collections::HashMap` at each use, and every `std` import in `examples/`, in
`crates/nikaia-std/` and on the three pages. Last of the five, because nothing
waits on it.

### 2.45. The database driver checks the SQL while the program is built

[ADR-143](specification/adr/adr-143.md), all of it. The compiler knows no
SQL: a dialect is a grammar in a driver package. A grammar declares a result
column with `meta::column(name, type)`, the third and last intrinsic of the
hybrid binding, and the compiler derives the statement's row type from the
columns as it derives the parameter type from the holes. A `dsl` block takes
build-time arguments, named, no `;` — `dsl sqlite(schema: app) { … } eod` —
resolved from `comptime` values, and the driver's grammar reads the schema
with its own DDL grammar and refuses a missing column at the query. `std::db`
is the protocol only (traits, statement, row values); `sqlite` and the rest
are packages. No expression capture, no ORM; `raw(text)` for dynamic SQL.

*What it needs, in the record's order (§5):* `meta::column` and the row type;
build-time arguments on a block; `std::db`'s traits; the `sqlite` driver with
both grammars and the schema check; the example with a misspelled column
refused.

### 2.46. `Pointer[T]` is undeclared, and C is `getpid`

[ADR-147](specification/adr/adr-147.md). Every C function whose signature has a
pointer in it is unwritable: all of `libc`'s memory surface, and every library
that hands out a handle — a database connection, an HTTP client, a compressor.
`extern "C" { fn malloc(size: usize) -> Pointer[u8] }` is `NK1135`, because
`Pointer[T]` is a type nothing declares, and
[ADR-124](specification/adr/adr-124.md) §4 left it that way on purpose.

**What the record adds is not a pointer.** A buffer is a **view** that lives
for the call, with a length parameter checked against it at the call site; a
handle is an `opaque type … released by …`, an address the language never
dereferences whose release is a `cleanup`. Both make a dangling dereference
impossible by construction, and `malloc` stays unwritable by design — memory
the language will index arrives with a length the language knows.

*What it needs, in the record's order (§5):* the view forms in an `extern`
declaration; the length check and its refusal; the opaque type, with its
`cleanup`; `CStr` and the `std` function that copies it; `sqlite3` end to end
as the test that the four are enough.

### 2.47. `select` is not a keyword, and nothing cancels a task

[ADR-148](specification/adr/adr-148.md). Part II 12.4's block is a parse error
and carries [ADR-141](specification/adr/adr-141.md) D2's *unspecified* mark —
while the **semantics** have been built since
[ADR-006](specification/adr/adr-006.md) D3: the loser stops at its pause point,
its `cleanup` is adopted, the deadline bounds it. The runtime's race is what
[ADR-129](specification/adr/adr-129.md)'s C `cancel` already leans on.

*And a program cannot say stop to something it started*, because losing a
`select` is the only thing that cancels a task today.

*What it needs, in the record's order (§5):* the keyword, the block and the arm
grammar; the lowering onto the runtime's race; `cancel()` on the handle, over
the same call; the page's example as a test that runs, and the mark taken off.

### 2.48. There is no channel

[ADR-149](specification/adr/adr-149.md). Part II 12.5's
`let (tx, rx) = channel::bounded(100)` names nothing, and carries the
*unspecified* mark. Nothing in it needs syntax — two values, two methods, and
the tuple `let` is built ([ADR-098](specification/adr/adr-098.md)).

*What it needs, in the record's order (§5):* the runtime's bounded queue, with
the pause on a full `send`; `std::channel` and its four entries, `send` carrying
no `sync` and `recv` handing back a `T?`; the page's example as a test that
runs.

### 2.49. There is no duration

[ADR-150](specification/adr/adr-150.md). `5.seconds()` is a method on an
integer that no ledger describes, and Part II 12.4 carries the *unspecified*
mark for it. [ADR-148](specification/adr/adr-148.md)'s `select` needs it for
the timeout arm its example writes.

*What it needs, in the record's order (§5):* `std::time::Duration` and its
entries; the integer extension, five names; `sleep` taking one; the page's line
as a test that runs.

### 2.50. There is no fixed-size array

[ADR-152](specification/adr/adr-152.md), and **after**
[ADR-135](specification/adr/adr-135.md), because the literal is that record's.
Two doors close with one type: [ADR-127](specification/adr/adr-127.md) §4's C
field `[f64; 3]`, and [ADR-135](specification/adr/adr-135.md) §4's container
that does not allocate — which [ADR-119](specification/adr/adr-119.md)'s
`startup` profile needs on its first day, since a `Vec` wants an allocator that
profile has none of.

**The new part is an integer argument in the type language.** Every parameter
so far has been a type, and `Array[T, N]` wants a `comptime` integer.

*What it needs, in the record's order (§5):* the integer argument and
`Array[T, N]` parsing, binding and writing itself back; the literal taking the
array type from its use, and the length refusal; the lowering to `[T; N]`, and
indexing; the C field, laid out as C lays it out.

### 2.51. The prelude is what the compiler happens to know

[ADR-154](specification/adr/adr-154.md).
`crates/nikaia-std/src/lib.rs`'s `prelude` was grown one `pub use` at a time and
carries `fs`, `io`, `cli`, `html`, `task`, `ListExt`, `Full`, `digit_value` and
`HashMap` — every one reachable from a `.nika` file with no `use`. So **a
program can read a file without saying so**, which is the sentence
[ADR-140](specification/adr/adr-140.md) D5 was written to make it say.

*And there is only one list.* The Nikaia-level prelude and the emitter's are the
same module, which is the piece that does not exist rather than the piece that
is wrong: what a program may name and what the generated file needs to compile
are different questions.

*What it needs, in the record's order (§5):* D1's list on Part I's first page,
so the promise is written before it is enforced; the two preludes separated; the
names outside the list refused without a `use`, which is `NK1117` with a help
naming the line to add; the corpus and the pages. **Measure the corpus before
the change**: every `fs::`, `io::`, `cli::`, `html::` and `HashMap` in
`examples/` needs a `use` line it never needed.

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

### 3.2. The ring's `block_on` test flakes under a loaded whole-workspace run, and the bound is not the cause

`rt::tests::a_future_fed_from_a_worker_finishes_under_block_on`
([ADR-121](specification/adr/adr-121.md) D3) passes alone — a hundred times —
and has failed three times this month inside `cargo test --workspace --release`,
each time on a different unrelated change. Two clean whole-workspace runs follow
every failure.

**Its bound has already been raised once, from ten seconds to sixty, and that
was the wrong fix to repeat.** The mechanism is contention rather than slowness:
`io_workers` defaults to **one**, the harness runs the suite in parallel, and
every in-process readiness wait in the workspace queues behind the others on
that one thread — so a wait ahead of this one holds it for as long as *its* own
timeout, and the sum can pass a minute with nothing wrong. Raising the number
again buys a longer wait for the same race.

*What would actually settle it*, in the order of how much it costs: give the
tests that wait on readiness a runtime of their own rather than the process's;
or mark them `#[serial]` so they cannot queue behind one another; or have the
harness run `nikaia-std`'s runtime tests in a single thread.

**A suspicion and not a fact about the runtime**, which is what the head of this
file asks for: no reproduction on demand exists, and every attempt to force one
has passed. What is known is the mechanism above and that no change to the
runtime has been in flight on any of the three occasions.

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
