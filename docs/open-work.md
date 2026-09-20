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

**A closed entry is deleted, not kept.** What it was and what closed it is in
the CHANGELOG, which is the record; this file is the list of what is still
open, and an entry that has been answered only makes it longer to read. The same
holds for the part of an entry that has been answered while the rest stands.

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

**The moment an item is blocked by a question**, the question goes where
questions go: one that needs the **owner** rather than other work has to be
listed in [`open-decisions.md`](open-decisions.md), in the shape that page asks
for — what is blocked, the options, a recommendation, and what either direction
costs if it is wrong. What stays here is what a record already decided and the
compiler does not do yet.

---

## 1. Defects

Two lessons this section has paid for, kept because they are rules and not
records:

* **A defect that needs a decision is not a defect that needs patience.** It
  needs the decision put on [`open-decisions.md`](open-decisions.md), which is
  where the one that held up a false `NK1129` sat until it was taken.
* **A question that can be answered by running the corpus is not a reason to
  leave a defect open.** Measuring one took an afternoon and refused nothing,
  after the question had held its defect open since the record that named it.

**How the entries here are found**, which is a method rather than a habit:
by running the programs the specification prints. `crates/nikaia/tests/specification.rs`
takes every `nika` block in the three pages as far as it goes and hands the ones
that lower to `rustc`, against two recorded baselines. Of 134 blocks, 59 are
programs this compiler takes and 39 of those compile below.

**One entry is open.**

### 1.1. A grammar's entry does not say what it keeps

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

**Three of the four columns are answered, and the fourth is a different
question.** [ADR-142](specification/adr/adr-142.md) D1 says an action may not
pause, so an entry is `sync` by construction and D2 writes the column.
**`touches` and `locks` are now derived over the action blocks** and folded into
the entry the way a function's are — `Stock::file` carries `touches = []`, and
`read` carries it too and has lost its `locks = "?"`, which is this entry's own
reproduction read backwards.

*What that needed first* was a **key**: the checker filed a call's answers under
the enclosing *function*, and an action has none, so a rule's answers landed
nowhere. A `pub` rule is a ledger entry
([ADR-082](specification/adr/adr-082.md) D1) and has its own key now. What the
key must not do is make an action a *function*, and `NK2605` says that where it
belongs instead: an action's failure leaves the parser rather than travelling to
a caller.

*The grammar is the unit rather than the rule*, which is the over-approximation
[ADR-033](specification/adr/adr-033.md) D4 asks for in this column: a rule's
pattern names other rules of the same grammar and their actions run with it, and
which ones is the parser backend's question rather than this walk's.

**`keeps` is what is left, and it is the tether's question rather than this
one's.** A parse hands back views **into its input** — `Stock`'s `Entry` holds
`&str` — so the entry keeps its `input`, and that is
[ADR-008](specification/adr/adr-008.md)'s tether rather than anything an action
block says. Leaving the column absent is the safe reading today: absent means
*nobody said*, so the caller does not lend, while a derived `keeps = []` would
lend the input to a parser that tethers views into it. It is written down here
rather than guessed at, and it waits on the same mechanism
[`open-decisions.md`](open-decisions.md)'s `Bytes` entry waits on.

*Every example still runs*, at both settings, which is what said this cost
information rather than correctness.

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

1. **The refusals around tasks** ([ADR-055](specification/adr/adr-055.md)). The
   mechanism is built at both settings — the executor, `async`/`.await` off the
   ledger's `sync` column, `std`'s own pausing entries, `spawn` with
   `TaskHandle` and `.join()`, and `overlap { … }` — so nothing in this list
   waits on a thread any more. What is left is one **refusal** rather than one
   mechanism: §2 D6's `Send` is asked for by the pool's starter, so a task
   holding something that may not cross is refused by `rustc` about the
   generated file rather than by this compiler about the program. *A task that
   may not cross a thread*, below.
2. **The rules around the lock.** The type, its constructor, its single spelling
   and all four doors are built, and the transfer has its door
   ([ADR-065](specification/adr/adr-065.md)), so nothing here needs deciding:
   what is left is every **refusal** the section states and nothing raises.
   Independent of the sequence above, so it can be taken beside it.
3. **A server to bind to, and the `postgres` block.** Its own project rather than a
   step of this one.
4. **Supervision.** Last because nothing else waits on it.

**And one that is out of the sequence because three entries rest on it**: the
**tether** ([ADR-008](specification/adr/adr-008.md)), last below and first under
*text is one type*, under §1's `keeps` column, and under
[`open-decisions.md`](open-decisions.md)'s `Bytes`. It is not one change
package, and the entry says what each of its four parts is.

### 2.1. A lambda that pauses is refused where `std` takes it

[ADR-055](specification/adr/adr-055.md) §6's remainder, and a limit of this
compiler rather than of the language — so it is here and not in §1, where a
defect is the compiler being *wrong*.

**A lambda whose body calls something that can pause is refused at the build —
where the parameter it is handed to is `std`'s.** Rust has no stable `async`
closure, so the lowering has nothing to write for a `std` entry that takes one.
The refusal is by the lowering and not by the checker on purpose: refusing it in
the type checker would refuse a correct program (Part III, C.4).

**Where the parameter is declared in *this* language it is not refused any
more** ([ADR-122](specification/adr/adr-122.md) D1, D2): the type says the code
may pause, the declaration is a closure returning a boxed future, and the lambda
is `|a| Box::pin(async move { … })` — a body that pauses is an ordinary body
inside it. So what keeps the refusal alive is `std` alone: its lambda-taking
entries describe **Rust** signatures that take a plain closure, so a pausing
lambda handed to `map` still has no shape. That is D3's own exemption read from
the other side.

*Evidence:* `examples/fortunes.nika:120` — `.route("/fortunes") fn { fortunes(db) }`,
a route handler that queries a database. It type-checks clean and no build
reaches it, because the server it binds to does not exist yet — the
`fortunes.nika` entry below. So nothing in the repository meets the refusal
today, and the first program that does will be the one that binds a handler.

*What it needs:* a `std` entry whose lambda may genuinely pause, written in
Nikaia or described as taking a future — `|| async move { … }`, which is stable
Rust and is how a handler is taken in practice. Not a new mechanism: a claim to
record.

### 2.2. A `for` over a stream has no suspension point

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

### 2.3. The lock is built and every rule around it is not

[ADR-057](specification/adr/adr-057.md) decided what the lock **is**,
[ADR-059](specification/adr/adr-059.md) what a program writes to reach one, and
[ADR-064](specification/adr/adr-064.md) gave the shared mutable type its name, its
constructor and its single spelling. Part II 12.2's counter compiles and runs at
both settings, so the type is not what anything here waits on — what is left is
the section's own rules, and they are refusals nothing raises:

* **D7's *stored* lambda**, the one part of [ADR-039](specification/adr/adr-039.md)
  D3 the `locks` column does not answer for. The column itself is built
  (`contracts::locks`, a least fixpoint over the call graph `sync` uses, with a
  `spawn`'s body excluded and a trailing lambda's counted).

  *It has three values and not two, and the corpus is what bought the third.*
  D3 says fail-closed, and it says it about `sync`, where the cost of doubt is
  a caller writing `.await`. Taken literally here it gave the property to **16
  of 59** functions in `examples/` — almost all of them `main`, and **not one
  of those programs opens a lock**. A refusal reading that column would have
  refused correct programs. So `Undecided` is its own answer: not permission,
  and not a refusal either. The corpus reads **0 hold, 24 undecided, 35
  clear**, and what shrinks the middle is entries existing for the methods it
  calls — *a foreign crate is described before it is called*, below — rather
  than a change here. That number is what any refusal reading the column has to
  be read against, which is why it is kept.
* the **re-entrancy check as a build switch** ([ADR-039](specification/adr/adr-039.md)
  D8), which the cache key already accounts for — and which
  [ADR-057](specification/adr/adr-057.md) D2 makes free at one thread and D3
  charges only on the values that actually cross;
* **`NK2201` and `NK2503`**, catalogued and not emitted.

  *`NK2503` needs the reachability walk* [ADR-039](specification/adr/adr-039.md)
  D6 describes as `NK2502`'s generalised.

  *`NK2201` is not obviously anything.* The catalogue calls it *no I/O while
  holding locked data*, and [ADR-067](specification/adr/adr-067.md) D1 split
  that sentence in two: what **pauses** is `NK2202`'s and what **takes a lock**
  is `NK2203`'s, and a `println` is the second. What a third code would add is
  I/O that neither pauses nor takes a lock, and whether any exists is a question
  to answer before writing one.

### 2.4. Part II 12.8's supervision syntax

`supervisor::start_link(fn { … }; restart_policy: …)` is specified and there is no
supervisor. Listed so it is not mistaken for something the `spawn` work includes —
it is not.

### 2.5. `fortunes.nika` waits on two runtime pieces and one language question

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

### 2.6. There is no HTTP server, and three records now wait on it

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

### 2.7. There is no target that lets foreign code call in, and the record for one is written

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

### 2.8. The build-time evaluator has no `push` and no aggregate value

The **call** and the **loop** are built
([ADR-073](specification/adr/adr-073.md) D5's second stage): a `comptime`
initialiser may call a function of this program, and a body may be arithmetic,
comparisons, an `if`, `let`s, a `return`, a `for` over a range, a `while`,
`break`, `continue` and an assignment.
[`crates/nikaia/src/build_time.rs`](../crates/nikaia/src/build_time.rs) is the
interpreter; [`fold.rs`](../crates/nikaia/src/fold.rs) stays in front of it,
because it is what says which integer type a *declaration* pinned.

*What is left is `push`, and after it the field walk.*

*Reproduced:* the interpreter has no `push` and no aggregate value, so
[ADR-079](specification/adr/adr-079.md) §3's table can be *computed* and has
nowhere to arrive. What a `comptime` hands to the language below is what Rust's
`const` can hold, and today that is one integer or one `bool`. The **types** are
no longer the obstacle: [ADR-135](specification/adr/adr-135.md)'s literal is
built and so is [ADR-152](specification/adr/adr-152.md)'s `Array[T, N]`, so a
table can now be written **and** has a type it can cross in — a `Vec` allocates
and a `const` cannot hold one, where `[T; N]` is exactly what a `const` holds.
Text is not in it either.

*Why it is work and not a question:* two records decided what may happen and
neither can happen.

| record | what it wants of the evaluator |
| :--- | :--- |
| [ADR-079](specification/adr/adr-079.md) §3 | a **loop and `push`**, to build a table that then crosses as a view — the loop is built; `push` needs a value to push onto |
| [ADR-088](specification/adr/adr-088.md) D1 | a **loop over a type's fields**, which is the whole of 10.3 |

[ADR-079](specification/adr/adr-079.md) §3 says it plainly — *"This is the real
work behind the feature, and this record does not shorten it"* — and until this
entry existed, that sentence was the only place in the repository where the work
was named. Three records waiting on something the work list does not mention is
how a thing stays unstarted.

*What bounds it, and it is already decided:*
[ADR-075](specification/adr/adr-075.md) D1 and D2 — a body it may evaluate is
`sync` and touches at most the build's own parameters, which are two **ledger
columns**, so the interpreter decides nothing about safety. `NK1152` is a callee
the rule forbids, and it is deliberately not `NK1127`: one says *not yet*, the
other says *not allowed*. There is deliberately **no step budget** (D4), so a
body that does not terminate hangs the build: worth knowing before starting, and
not a reason to add one on the way past. A **recursion** that does not terminate
is bounded, because that one takes this compiler's stack with it.

*The order the records imply:* `push` next — which is not the loop's step, since
the loop is a shape the interpreter reads and `push` is a *value* the whole
compiler has to carry from the build into the program. The field walk last,
because it needs something the other two do not — see the entry below, *running
a grammar while the program is built*.

*What it does **not** include:* running a **grammar**. That looks like the same
job and is not; it is the next entry's, and the reason is there.

### 2.9. Running a grammar while the program is built is not interpretation

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
[ADR-072](specification/adr/adr-072.md) built the permission for — **and the
database driver**, below, whose whole first step is a grammar declaring what
columns a query returns. That entry says what the narrow half costs, which is
less than this one's general case: a flat list of declarations rather than an
arbitrary value.

### 2.10. The cleanup point the ledger should narrate

[ADR-094](specification/adr/adr-094.md) D5, and the only part of that record
left. Steps 1 to 4 are built: the `keeps` column is inferred, a `for` lends, a
`let` over a place is a view where the value would move, `xs.drain()` takes the
elements away, a parameter the callee only reads is declared `&T` and given its
`&` at every call (`NK1137`), and `mut out: Vec[i64]` is the third state and
lowers to `&mut T` (`NK1138`).

*Evidence:* `keep(file)` says nothing about the file being flushed inside the
callee, and nothing tells a caller when a callee *starts* keeping a value whose
teardown has an effect. §3 of the record names that as the one semantic cost and
D5 as where it is paid: a change to a parameter's `keeps`, on a type with a
`Drop` or a `Cleanup`, is a ledger diff that names the callers whose cleanup
moved — the `NK2401` shape, one more thing it narrates.

*One limit the built half carries, and it is worth knowing before D5 is
written.* The `let` that lends and the refusal at a call both act only where the
checker could **type** the value; where it could not — a place inside a lambda,
a value a `catch` handed back — the `let` does not lend, the refusal stays
quiet, and a written `&` is the program's own. The *writing* has no such limit
and must not: the declaration is written off `lends` alone, so anything the
checker skips before recording the argument would be a callee taking a `&T` and
a caller not passing one. Closing it is a better answer for those types, not a
change to either rule.

*Why it is here and not in §1:* nothing is miscompiled. `fill(xs)` reads as
though it does not change `xs`, which is a message in the wrong words rather
than a wrong program.

### 2.11. The boundary translation for a hand-edited hash

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

### 2.12. An error that newly reaches a `catch` is named once

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

### 2.13. An error type is lowered as the `enum` it is

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

### 2.14. A parameter may be a function, and a kept one has no lowering

[ADR-102](specification/adr/adr-102.md) D5, and the only part of that record
left. Steps 1 to 3 are built: the type parses and round-trips through the
ledger, D2's reading is in the fit (`NK2206` for a lambda that pauses where the
type says `sync`, `NK2606` for one that fails where it declares none), a **run**
parameter lowers to a closure argument `impl Fn(A) -> R`, and the run-or-kept
answer feeds the `sync` column — a run parameter gives its function
`sync = "from(f)"` and a kept one is answered from the type.

*What is left is D5's lowering of a **kept** handler*, which is *a lambda that
pauses is refused where `std` takes it* one entry up, with a callee that can now
say which shape it wants.

*Until that lands, a function type is a **parameter** and nothing else.* A
field, a result and a `let` are the positions where it can only be kept, and
they are `NK1142` here rather than `impl Fn(…)` in a Rust field — which is not
Rust, and would reach the reader as the backend's words about a file nobody
wrote.

### 2.15. A package is found by version through Cargo, under `nikaia_<name>`

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

### 2.16. A foreign crate is described before it is called

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

### 2.17. `par_iter` has no entry to demand `sync` of

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

### 2.18. A bound takes a path, and the ledger records traits and `impl`s

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

### 2.19. Text is one type, and `&str` is the assertion

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

### 2.20. A path names its root at the call

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

### 2.21. An `update` block says `mut`, may run more than once, and the compiler picks the lock

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

### 2.22. A cleanup the deadline cut off names the resource

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

### 2.23. `?.` reaches through a view

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

### 2.24. Reading a map through the brackets is a `T?`

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

### 2.25. An `overlap` keeps every failure

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

### 2.26. `from` is a name, and a file a build reads is `asset("…")`

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

### 2.27. `with` is a copy of a value with named fields changed

[ADR-118](specification/adr/adr-118.md). `p with { x: p.x + 1 }` is a new
value of the same type; the braces are the literal's, only the top level, the
unnamed fields are moved, and across a package only `pub` fields may be
named. **Nothing of it is built**: `with` is a reserved word without a rule.

*What it needs, in the record's order (§5):* the grammar rule and AST node;
the checker's field resolution and refusals, the enum operand refused; the
lowering to a struct expression with a base; `examples/1brc.nika`'s `Stats`
and a test.

### 2.28. A target without an operating system

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

### 2.29. A grammar's action is the block after the pattern, and two borrowed names go

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

### 2.30. A function-typed parameter lowers by its type

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

### 2.31. `nikaia describe` does not write `crosses`

[ADR-123](specification/adr/adr-123.md). **Built, except the command.** The
column has three values, the ledger writes and reads both claims and stays
silent for the third, a third spelling is refused rather than guessed at, the
crossing verdict answers *may not* off `crosses = false`, and
`examples/foreign-runtime/`'s three descriptions say it for the handle over an
`Rc<String>`. `NK2501` and `NK2502` have their first end-to-end tests — the
first either code has ever had, because no type the records named could answer
*may not* until now.

**What is left is D2's *who* rather than its *what*:** `nikaia describe` writing
the column from a foreign type's fields, which is *a foreign crate is described
before it is called*'s step 2 and waits with it. Meanwhile the line is hand-written like the rest of the description, and the
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

### 2.32. A library for other languages

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

### 2.33. A struct crosses the boundary by value

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
library's. **And §4's `[f64; 3]` field**, which is
[ADR-152](specification/adr/adr-152.md)'s step 4: `Array[T, N]` is built and
lowers to `[T; N]`, so what is left is a `repr(C)` struct to put one in and the
test that it is laid out as C lays it out.

### 2.34. The symbol prefix is one line in the build

[ADR-128](specification/adr/adr-128.md), all of it. `symbol-prefix = "hc"` in
`[build]`, default the package name with `-` written `_`; a C identifier or
refused. No declaration renames its own symbol.

*What it needs:* the manifest key with its check; the header generator and
the emitter reading it.

### 2.35. An async call can be cancelled, and a stream is a callback

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

### 2.36. A WebAssembly library is the same entry point on another target

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

### 2.37. A binding is a generated file over the C library

[ADR-131](specification/adr/adr-131.md), all of it. `nikaia bind python`
writes a `ctypes` binding from the ledger (exceptions per variant, `str` and
`bytes` for buffers, classes with `close()` for handles, `IntEnum`, `None`,
generators for streams, an awaitable for `_async`); `nikaia bind js` is the
WebAssembly build's `.js`. No second artifact, no native Node add-on.

*What it needs, in the record's order (§5):* the Python generator over the
example library; the streamed and async forms; the `js` name; a test that
imports the binding.

### 2.38. `nikaia fmt` does not exist

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

### 2.39. A field's prose and a variant's go nowhere

[ADR-139](specification/adr/adr-139.md) D1 gives a `///` run in front of a
**field** and a **variant** the same meaning it gives one in front of an item,
and the parser keeps neither. Everything else of that record is built: the nine
item positions, the ledger's `doc` column, and `std`'s hundred and eight
hand-written entries, which carry prose held there by a test.

*Why it sits still:* D2 gives the ledger a column for a `fn` and a `type` and
none for a field or a variant, so nothing would **read** what the parser kept.
What would is `nikaia doc`, which is that record's §4 and wants a record of its
own — so this is one piece of work with that one rather than a job waiting on
nobody.

*What it needs:* `doc_here` at a field's and a variant's first byte, which is
the same hand-written parser the items already use; a place on `FieldDef` and on
`EnumVariant` to keep it; and the reader that makes it travel.

### 2.40. The database driver checks the SQL while the program is built

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

**Step 1 is blocked, and the record does not say by what.** For the compiler to
know which columns a grammar declares, the grammar has to **run while the
program is built** — and *running a grammar while the program is built is not
interpretation*, above, says that may only be done by compiling the **generated**
parser and running it. So `meta::column` waits on that entry, and so does
`meta::parameter`: today a `dsl` block's holes come from a **scan of the body
text** (`crates/nikaia/src/dsl.rs`), which that file's own note calls an
approximation, and `dsl html { … }` is the one block this compiler runs at all —
by [ADR-017](specification/adr/adr-017.md), because `html` is the target it
compiles itself.

*Measured, and the measurement is the point:* the emitter already writes a
complete `grammar! { … }` for a grammar item, and `sysroot.rs` already knows
where `winnow_grammar` and `winnow` are. What is missing is the **harness** — a
crate holding one grammar and a `main` that parses the block's bytes, compiled
and run during the build, keyed in the cache on the grammar's source — and a way
for what it found to come **back**: for this record that is narrow (a flat list
of declared columns and parameters), where the general case of §2.9's own
example (`comptime CONFIG = Config.value(from "config.toml")`) has to carry an
arbitrary value.

*So the order inside this entry is not the record's:* the harness first, then
step 1. Neither is small, and the harness delivers nothing a reader of a program
would notice — which is worth knowing before it is started rather than after.

### 2.41. Two names the prelude promises and `std` does not have

[ADR-154](specification/adr/adr-154.md). The rule is built for a function and
for a type; what is left is D1's own list naming things that do not exist.

**`assert`** and **`panic`** are on the list on Part I 1.3 and are not in `std`.
Neither is a piece of work on its own: both belong with the **testing chapter**
(Part III 14), which is not built either — there is no `nikaia test`, `assert c`
parses as two statements and is refused with `NK1117`, and `assert(c)` is a call
to a function of that name. Whatever `assert` is, it is decided there rather
than here.

*The third name went to [`open-decisions.md`](open-decisions.md).* **`Bytes`**
is on the list too, and it is not a missing name: it is the container Part I
6.6's **tether** needs, and the tether is that section's unbuilt half. Where it
lives — `std`'s or the language's — is what
[ADR-154](specification/adr/adr-154.md) §4 left open, and a question with a
recommendation belongs on that page rather than this one.

*And one the list does not promise and `std` has.* `eprint` is keyed bare, so it
needs no `use`; D1's text writes `eprintln` and not `eprint`. It rides with the
`Bytes` entry, because both are the same sentence being revisited.

*A name on the list that does not exist* is the one direction a prelude can be
wrong in without anybody noticing — nothing refuses it, because nothing reaches
it. That is why all three are written down rather than left to be found.

*What stays bare on purpose:* `Shared`, `SharedMut` and `Locked` are the
language's ([ADR-064](specification/adr/adr-064.md)) rather than a module's, and
a `TaskHandle` is what a `spawn` hands back — a program has no reason to write
the name, so moving it would cost a migration and buy nothing.

### 2.42. The tether: a view that outlives its buffer is refused, not tethered

[ADR-008](specification/adr/adr-008.md), and it is **last in this list and first
under three of its entries** — which is why it is written down rather than left
as a phrase three other places lean on. Part I 6.6's three states are Borrowed,
Tethered and Owned; **two of them are built and the middle one is not.**

*What is built.* **Borrowed** is Rust's own lifetime and costs nothing:
`examples/1brc.nika` and `examples/inventory` are the shape D2 calls free, and
they run. **Owned** is `.to_owned()`, written by the program and never by the
compiler (D5). **D9** is built, and now for a result that *carries* a view as
well as one that *is* one. What stands where Tethered would is a **refusal**:
`NK2302` for a naked view parameter that is stored, and `rustc` on the Nikaia
line for a view of a local that escapes — which is D5's residual hard error
doing the job of the state that is missing.

*What Tethered needs, measured rather than estimated.*

1. **A buffer to tether to.** `Bytes` — refcounted, offset and length. Small in
   Rust; the question is where it lives, and that is
   [`open-decisions.md`](open-decisions.md)'s.
2. **The escape analysis** (D2), which is the load-bearing piece and is a kind
   of analysis this compiler has never had: per **construction site**, a least
   fixpoint over a three-element lattice, deciding whether a value outlives its
   buffer's scope. `views.rs` is 919 lines and answers a much cruder question —
   *does a naked view parameter get stored* — over a fixed list of destinations.
   Both directions of being wrong are costly: too permissive is a use-after-free
   the backend catches, which is Part III C.1; too restrictive is a correct
   program refused, which is C.4.
3. **Three layouts per struct** (D3, D5), chosen per construction site. A
   tethered `Entry` is not a `&str`: the **container** holds the handle (D4) and
   the element holds `(offset, len)`, so every read of `entry.name` becomes a
   slice of the container's buffer. That is a whole representation, emitted
   three ways and picked by the analysis's answer — handed over the way
   [ADR-028](specification/adr/adr-028.md) hands every other answer the emitter
   has no types for.
4. **The buffer table** (D4): one handle per distinct source buffer on the
   container, keys as `(index, offset, len)` with the index elided where the
   compiler proves one buffer. `Eq`/`Hash` content-based and never identity, or
   a parallel run splits `Hamburg` across workers.
5. **The ledger** (D7): the state per view in a signature and the buffer-table
   shape per struct, so the answer crosses a package boundary — plus the barrier
   rule, where a `dyn` or a published non-generic API widens to Tethered.
6. **The lint and the cleanup** (D8): a small extract pinning a large buffer,
   and a `Cleanup` that runs at the last tether rather than at the end of the
   mapping's scope.
7. **`@borrowed`** (D6), which is **vacuous until the rest exists**: it forbids
   a state transition, and today there is no transition to forbid. The emitter
   writes a comment saying so on every `@borrowed` struct.

*So it is not one change package.* It is at least four — the type, the analysis,
the representation, the ledger — and the second is the one that decides whether
the rest is worth starting. **What rests on it:** *text is one type*, above,
whose `String` state comes from this analysis; the `keeps` column of a grammar's
entry, in §1; and `Bytes` itself, which is the same question read from the other
end.

*And a cheaper thing is true meanwhile*, which is why nothing is broken today:
the refusal is the honest answer for a program that would tether, and it names
`.to_owned()`. What it costs is the programs D2 describes as free-and-escaping —
a parser handing its rows past the buffer's scope — and no program in the tree
writes one.

## 3. Upkeep

A stale **Status** note is a defect in its own right
([`README.md`](README.md) §1), because a reader cannot tell a plan from a
promise - so this section being **empty** is a state to try to keep rather than
a milestone.

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

### 3.2. Two examples write a postfix `??` the language does not have

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

*It needs a decision before any work*, and the decision is now where decisions
go: [`open-decisions.md`](open-decisions.md) carries *does the language have a
postfix unwrap?*, with both options, a recommendation — **no**, because the
three things one is reached for already have spellings — and the replacement
measured as a program rather than sketched.

*Why it stays upkeep and not a defect:* no program in the tree writes a postfix
`??`, so nothing is wrong today except the page.

The Part III 17.1 example was rewritten while this was found; ADR-018's stands,
because an ADR is written once and the correction belongs to whatever answers
the question.

### 3.3. Three corpus files cannot be compiled with `--input`, and none of them is broken

A sweep that runs `nikaia --input` over every `.nika` file in the tree reports
three failures, and **all three are the sweep's method rather than the corpus**.
They are written down here so that the next reader does not measure them again.

* **`examples/hello-http/src/main.nika`** and **`examples/fortunes.nika`** write
  `use http`. A package reached by name is declared in `[dependencies]`, which
  lives in `nikaia.toml`, which `--input` does not read — so the refusal is
  correct and says so. `nikaia build` inside `examples/hello-http` compiles it
  and its `http` dependency and finishes clean; `crates/nikaia/tests/project.rs`
  is where that is a gate.
* **`examples/inventory/page.nika`** is one file of a package whose `Entry` is
  declared in `stock.nika` beside it. Compiled alone it is a file referring to a
  type nothing in it declares, which is `NK1135` doing its job.

*What is actually open about `fortunes.nika`* is neither of these: it is written
at specification level and `examples/README.md` lists its gaps — **G6**, the
runtime binding that lets a handler see the request
([ADR-018](specification/adr/adr-018.md)), and the `postgres` block, which is the
database driver above and is itself blocked. Its `render` lowers and runs today.

*So the corpus check to trust is the test suite*, which builds each project the
way a project is built, and not a loop over every file.

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
