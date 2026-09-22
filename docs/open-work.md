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

**A closed entry leaves its number behind.** Deleting the entry is right — what
it was and what closed it is in the CHANGELOG — and **renumbering the rest is
not**: this file is cited by number from records and from
[`open-decisions.md`](open-decisions.md), and a shift of two turns every one of
those into a sentence pointing at somebody else's entry. That is the failure
the paragraph below is about, met from the other side. So a gap in the numbers
is a closed entry, and it is cheaper to read than a citation that lies.

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

**No entry is open.** §1.1 closed at 0.0.137, §1.2 at 0.0.132, §1.3 and §1.4 at 0.0.131, and §1.5 and §1.6 at 0.0.136. The closed numbers stay where they were, because this file is cited by number.

**What that means is that the list is empty, not that the compiler is right.**
Every entry this section has ever held was found by *running* something — the
specification's own programs, the corpus at both settings, a two-file project —
and never by reading the code. So an empty §1 is a statement about what has been
run, and the way to refill it is the method above.

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

**And the order is the thing this file most easily gets wrong**, because it is
the part that goes stale without any entry changing: what sat first here sat
first because of what stood in front of it, and both of those were built out
from under it. It used to open with *the refusals around tasks*, pointing at an
entry called *a task that may not cross a thread* — which **did not exist**, and
whose work has since been done from the other end. Read against the code before
it is followed.

So, in order, and each says below why it sits where it does:

1. **A server to bind to, and the `postgres` block.** Its own project rather
   than a step of this one, and **first now** rather than third: it is what
   several entries here are waiting for. A `std` entry whose lambda may
   genuinely pause is a route handler (§2.1); a `par_iter` with an entry to
   demand `sync` of is a program that calls one (*`par_iter` has no entry*); a
   pausing sequence's lazy walk is the same shape (§2.2). Each of those says
   *it waits on a program rather than on work*, and this is the program.
2. **The rules around the lock.** The type, its constructor, its single spelling
   and all four doors are built, and the transfer has its door
   ([ADR-065](specification/adr/adr-065.md)), so nothing here needs deciding:
   what is left is **D7's stored lambda** and nothing else — and the corpus says
   a refusal reading the `locks` column today would refuse correct programs, so
   what it needs first is entries for what those functions call.
3. **Supervision.** Last because nothing else waits on it.

**And what left the head of this list, measured rather than assumed.** The
refusals around tasks ([ADR-055](specification/adr/adr-055.md) §2 D6) were
first, and `NK2501` fires end to end now: a Rust crate with an `Rc` field,
`nikaia describe` writing `crosses = false` from that field
([ADR-123](specification/adr/adr-123.md) D2), the description merged into the
ledger the analyses read ([ADR-104](specification/adr/adr-104.md) D1), and a
`spawn` refused in **this compiler's** words on the `.nika` line —
`crates/nikaia/tests/describing.rs` runs that chain from a program. What is left
is the case this compiler **cannot decide**, where `rustc`'s own `Send` bound
refuses against the right `.nika` line through
[ADR-005](specification/adr/adr-005.md) D7's translation: the position is kept
and the words are `rustc`'s, which that record carries as its own open half and
is not work in this section.

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
where the parameter it is handed to is `std`'s.** Those entries describe
**Rust** signatures that take a **synchronous** closure, so the lowering has
nothing to write for one. The refusal is by the lowering and not by the checker
on purpose: refusing it in the type checker would refuse a correct program
(Part III, C.4).

**This entry used to give a different reason**, and it was false: *Rust has no
stable `async` closure*. It has one — `async |x| { … }`, `AsyncFn`, `AsyncFnMut`
and `AsyncFnOnce`, measured on this tree's toolchain
([ADR-187](specification/adr/adr-187.md) D1). What is true is narrower and is
about those `std` entries: `Iterator::map` takes `FnMut`, and an `async` closure
handed to it yields an iterator **of futures**, which is a different program.

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

*And no `std` entry it could meet is one where a pausing lambda would be a
**correct** program*, counted at 0.0.107 — which is what says the refusal is
narrow rather than merely unmet. `std` takes a lambda in ten places.
`list::ListExt::map`, `Seq::map`, `Seq::filter`, `Vec::sort_by_key`,
`Entry::and_modify` and `Entry::or_insert_with` lower to Rust's own
synchronous closures, where a suspension point is not a shape that exists. The
other four — `Locked::access`, `Locked::update`, `SharedMut::access`,
`SharedMut::update` — run their lambda **while a lock is held**, where pausing
is refused on its own merits (Part II 12.3). So what is left here is an entry
that does not exist, and the lowering that would serve it is built and waiting:
[ADR-122](specification/adr/adr-122.md) D1's `|p| Box::pin(async move { … })`,
measured on a parameter declared `fn(ref String) -> String throws` in this language.

*What it needs:* a `std` entry whose lambda may genuinely pause, written in
Nikaia or described as taking one — either a future, `|| async move { … }`, or
an `impl AsyncFn(…) -> …`, which is how a handler is taken in practice and costs
about a ninth of the boxed form ([ADR-187](specification/adr/adr-187.md) D1).
Which of the two a *declared* parameter lowers to is the question that record
left on [`open-decisions.md`](open-decisions.md); which one a **described**
entry says is the describer's, because the signature is hand-written. Not a new
mechanism: a claim to record.

### 2.2. A lazy walk of a pausing sequence has no shape

**[ADR-172](specification/adr/adr-172.md) closed all but one corner of this
entry.** A `for` over `io::lines()` gives its thread up (D1), and the **eager**
walks of the same sequence — `collect`, `count`, `nth`, `join` — pause and
propagate with it (D5): each is a loop around the step, and both of the
receiver's words reach them.

**What is left is the two lazy ones.** `map` and `filter` hand back another
*sequence*, whose steps would pause, and a sequence like that is the trait D3
defers until a second producer needs one. They are refused from the lowering
meanwhile, with the loop as the way out and the line under it.

*Evidence: a refusal, which is the good kind.* `io::lines().map fn { … }` says
what is missing and what to write instead, in this compiler's words and on the
`.nika` line.

*What it needs:* the trait, and a type to put behind `map` that holds a lambda
and a pausing source. [ADR-172](specification/adr/adr-172.md) D3 says whose it
will be and why it is not `futures_core::Stream`; what it does not say is what
it looks like, and that is written the day a program asks for it — D4's *it
waits for a program*, which is [ADR-105](specification/adr/adr-105.md)'s rule
for the same file.

### 2.3. The lock is built and every rule around it is not

[ADR-057](specification/adr/adr-057.md) decided what the lock **is**,
[ADR-059](specification/adr/adr-059.md) what a program writes to reach one, and
[ADR-064](specification/adr/adr-064.md) gave the shared mutable type its name, its
constructor and its single spelling. Part II 12.2's counter compiles and runs at
both settings, so the type is not what anything here waits on — what is left is
the section's own rules, and they are refusals nothing raises. **The
re-entrancy check is off this list** ([ADR-168](specification/adr/adr-168.md)):
`reentrancy-check` is a `[build]` key now, a dimension of the build cache and
of the compiled `std`'s own tree, and Part I 1.2's note — which said the check
was not emitted and the nesting not refused, both false — says what is true.
**And so is `NK2503`**: a lock reachable through a call's arguments is refused
under its own code, with Part III C.6's sentences and its way out.

*That one is worth a line, because of what stood in front of it.* The walk had
found the lock all along and printed `NK2502`'s message about it, and the last
thing needed was telling a lock from a count in the verdict. Looking for the
count turned up something else: [ADR-061](specification/adr/adr-061.md) D1 — *a
`Shared` may not go into code nothing describes* — was **decided and not
built**. `Shared` sat in `contracts::send`'s `CHOSEN` row and in its
`CONTAINERS` row, the container row answered first, and the row that would have
refused was reached by nothing. The test that should have caught it asserted the
silence instead and passed. Both are built now, and the corpus is unmoved.

What is left:

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

### 2.10. The cleanup point the ledger should narrate

[ADR-094](specification/adr/adr-094.md) D5, and the only part of that record
left. Steps 1 to 4 are built: the `keeps` column is inferred, a `for` lends, a
`let` over a place is a view where the value would move, `xs.drain()` takes the
elements away, a parameter the callee only reads is declared `&T` and given its
`&` at every call (`NK1137`), and `mut out: Vec[i64]` is the third state and
lowers to `&mut T` in the language below (`NK1138`).

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
crate's version, and reviewed. **Steps 1, 2 and 3 are built, and step 5 by
hand.**

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

*Step 2 is built too, for a `path` dependency.* `nikaia describe <crate>` reads
the crate's `pub` signatures out of its sources, translates them by D3's table
and writes the draft under a header that says it is to be **reviewed**. It is a
**signature scraper** and not a Rust parser — an item a macro generates is not
in the text, a `pub` item inside a `mod` is read as the crate's own, and a type
it cannot account for is `?`. The command names what it could not answer rather
than leaving a draft that looks complete. A version dependency is refused with
its reason: those sources are in Cargo's registry cache, and guessing at that
cache's shape would be resolving a version, which
[ADR-002](specification/adr/adr-002.md) D1 hands to Cargo.

*The measurement is the experiment's own file.* The draft for each of the three
projects is asserted against the reviewed file, entry for entry and hash for
hash. Two of the reviewed lines are not in the draft and both are D5:
`crosses = false`, which is read from a **field**, and the comments.

*And the entries reach the analyses now*, which was D1's **first sentence** and
not an addition to it: *every analysis reaches to the boundary and reads an
entry there*. For two records the file was read to see whether it **parsed**,
the ledger was dropped, and its only effect was silencing the refusal that had
asked for it — while `NK2504`'s own message promised a reader four answers in
return for writing it. Measured on a three-line project: a description saying
`(a: i64, b: i64) -> i64` left a one-argument call unremarked. Three of the four
are in (the signature, `throws`, `sync`); the fourth is `crosses` and is not a
gap here — see the paragraph below.

*And the hash rule holds* (step 3, D5). A description is believed while the
files it was derived from hash as recorded, and `NK2505` says so where one does
not — the same rule as a stale ledger's with the one difference that matters:
[ADR-100](specification/adr/adr-100.md) D3 derives a ledger again, and a
description is **reviewed** again instead, because what it says is a person's
judgement. Only where there is a hash to disagree with: a crate declared by
version has its sources in Cargo's registry cache, and refusing on that absence
would refuse every crate that comes from a registry.

*What it needs, in the record's order (§5):* the rustdoc-JSON reader behind a
toolchain check; and a way to read a version dependency's sources, which is
what would make the hash rule reach every crate rather than the ones with a
`path`.

*And one line of the merge is decided in the cheapest direction rather than
decided.* A description's names carry the crate word in front of them and
`std`'s carry a module's, so a manifest declaring a crate whose word is one of
`std`'s modules would have two answers for one name. `std` wins today, silently,
which is a silence and not an answer — no manifest in this repository reaches
it, and the day one does it is a refusal to write. A **suspicion** rather than a
defect: nothing reproduces it.

*And the line something else was waiting on is written.* The describer reads a
`pub struct`'s **fields** now ([ADR-123](specification/adr/adr-123.md) D2), so
`crosses = false` on `hyper_shim::LocalHandle` is the command's answer rather
than a hand's — which is what the entry above about the crossing refusals being
unreachable into **our own code** was waiting for: a described foreign type is
the first thing that can answer `MayNot` there, and `NK2501` has something to
say the day a program `spawn`s one. `NK2502` and `NK2503` need more than that —
the call itself has to be one nothing describes — which is
[`open-decisions.md`](open-decisions.md)'s question about a described foreign
function and a thread.

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

*Evidence:* the three refusals a cross-package `Handler` bound met against
`examples/http/` — a parse error at the path, `NK1126` after `use http`, and
the `NK1129` that ADR-100 D2 has since removed.

*What it needs, in the record's order (§5), and where each stands* — measured
at 0.0.105 and built at 0.0.106, which is why *nothing of it is built* no
longer stands here:

1. **The path in the bound's grammar** — open, and it is the only step left.
   `fn tell[T: greet::Speaks](…)` is a parse error at the `:`,
   `expected one of: +, ,`. It waits on a trait being reachable across a
   **package**, which [ADR-078](specification/adr/adr-078.md) §4 calls a
   question about modules rather than about traits — and which nobody has
   decided. Until then a bound names a trait the unit declares, which is what
   makes (3) below complete for every bound the language can write.
2. **The `trait` table and its method entries** — **built**
   ([ADR-078](specification/adr/adr-078.md)). `Ledger::traits` names every
   declared trait and its methods, the signatures live in `functions` under
   `Summarize::summary`, and the keys are module-qualified when a program of
   several files absorbs them. `traits.rs` checks an `impl` against the trait
   it names.
3. **The `impl` table and the union in the bound check** — **built**
   ([ADR-174](specification/adr/adr-174.md)). `Ledger::implementations` is
   written where the `impl` stands and merged when a program's ledger absorbs
   its units', so the answer is the union over the files; `NK1164` refuses a
   call whose type answers for nothing, with a second sentence for a parameter
   of the caller's own, which cannot be told to write an `impl`.
4. **The resolution at a call through a bound** — partly, and nothing needs the
   rest. `x.say()` on a `[T: Speaks]` resolves through the **trait's** entry,
   which is the right answer for its signature; the implementing type's own
   entry is what (3) has now made reachable, and no program has asked for it.

### 2.19. Text is one type, and `ref String` is the assertion

[ADR-107](specification/adr/adr-107.md). `String` is the one text type and
its state — borrowed, tethered, owned — is the compiler's per use; `ref String` is
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
parses, both doors are handed a view they may write through, the `Option` is gone and with it the
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

### 2.23. `?.` over a member that is not copied still takes its receiver

[ADR-113](specification/adr/adr-113.md), **two thirds built**
([ADR-189](specification/adr/adr-189.md)). `?.` takes nothing: it reaches
through a view of its receiver, and the result is a copy where the member
copies and a view of the receiver otherwise, kept alive as any view of a place
is.

*What is built.* A reach over a field the compiler knows to **copy** lends its
receiver — `x.as_ref().map(|it| it.a)` — and a reach over a **method** that
changes nothing lends its scrutinee. Both shapes leave the receiver usable on
the next line, and both **run** with it used twice in
`crates/nikaia/tests/nullable.rs`.

*What is left is one third, and it is **not** the tether*
([ADR-190](specification/adr/adr-190.md) D1). A member that does not copy comes
out of the reach as a **view of the receiver**
([ADR-113](specification/adr/adr-113.md) D2) — and a view is not a *state*: the
lattice has three and the cheap one is the default.
[ADR-008](specification/adr/adr-008.md) D2 reaches Tethered only where a value
**escapes** the buffer's owning scope, and three of the four shapes a `?.` has
do not:

| shape | what happens | state |
| :--- | :--- | :--- |
| the receiver is a **local**, the view stays in its scope | compiles, runs | Borrowed |
| the receiver is a **temporary**, the view is consumed in the same statement | compiles, runs | Borrowed |
| view **`??` view** — `user?.name ?? "nobody"`, and a literal *is* a view | compiles, runs, zero cost | Borrowed |
| the view is **bound past a temporary** — `let n = find(1)?.name` | *temporary value dropped while borrowed* | an escape |

The fourth is not Tethered either: it is `NK2303`'s own case — a view of a
buffer that does not outlive it, refused in this language's words and naming
`.to_owned()` ([ADR-156](specification/adr/adr-156.md) D4), which this compiler
already builds twice.

*What it actually waits on is one question about `??`*, and it is the owner's,
on [`open-decisions.md`](open-decisions.md): a view on the left and an **owned**
value on the right — `user?.name ?? "nobody".to_owned()` — cannot be joined
without the copy [ADR-008](specification/adr/adr-008.md) D5 forbids.

*And it is not refused meanwhile*, which is
[ADR-189](specification/adr/adr-189.md) D3 and stands: refusing
`find(1)?.name ?? "nobody".to_owned()` would refuse a program that compiles and
runs today ([Part III C.4](specification/30-nikaia-tooling.md)).

### 2.25. An `overlap` keeps every failure — the cleanup half

[ADR-115](specification/adr/adr-115.md) D1 and D2 are **built**
([ADR-170](specification/adr/adr-170.md)): every error carries the failures
that joined it, an `overlap`'s later failing branches join the winner's list in
written order, and an uncaught failure prints them indented under it. The
question that blocked it — where the list lives when the channel has no
envelope — was answered **A**: a body that joins puts one on.

*What is left, in the record's order:*

* **D3's cleanup attachment.** The shape is there — a secondary with
  secondaries of its own is the tree that record wants — and the attachment is
  not: a cleanup that fails while a branch is already failing should join
  **that branch's** error, and today it is attached below in the language
  below's own way with nothing a program can read.
* **D4's `error.secondary` as a value a program reads.**
  `throw LoadFailed(error, error.secondary)` is that record's own written
  example. The list is what a log and an operator see today; handing it to a
  constructor needs a Nikaia type for *a list of errors*, which nothing writes
  down yet — so this is a question about the type language before it is work.
* **Whether the list survives a hop to a caller with a bare channel of its
  own.** [ADR-170](specification/adr/adr-170.md) D1 covers the body the block
  is written in and says so: the block, its `catch` and the function around
  them, which is where a handler is written. A caller that propagates such a
  failure has a channel of its own, and making the envelope travel is the
  transitive version — a derived column like `locks`, and worth its own
  measurement rather than a guess.

*And one case the list cannot cover, named rather than discovered:* a failure
that joins an error from **below** Nikaia, which has no envelope at all, is
dropped. That is the boxed channel's downcast finding nothing, and it is the
price of the box.

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

*The shape is reopened, and D1's reason for it was false*
([ADR-187](specification/adr/adr-187.md) D3): *Rust has no stable `async`
closure* is what chose the box, and `impl AsyncFn(A) -> R` costs 1.37 ns/call
against the box's 11.99 on a 0.31 floor. What holds D1 up now is its coherence
half alone, and whether that is worth 8.7× is on
[`open-decisions.md`](open-decisions.md).

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

### 2.31. A described foreign call is not asked whether it threads

[ADR-123](specification/adr/adr-123.md) is **built**, command and all: the
column has three values, `nikaia describe` writes it from a foreign type's
fields since [ADR-104](specification/adr/adr-104.md)'s step 2 landed, and
`NK2501` refuses a `spawn` over a described `crosses = false` type from a
program end to end. What is left of this entry is the hole that claim exposed
rather than made.

*And one hole the claim exposed rather than made, which is not this record's.*
`NK2502` asks its question of a call **nothing** describes, which is ADR-038 D7's
own wording, so a *described* foreign call is not asked — and no column says
whether a described foreign function puts what it is given on a thread.
`examples/foreign-runtime/crossing` is that shape exactly: its handle says
`crosses = false` and it is handed to `hyper_shim::across_a_thread`, which the
description names, so the program is still refused by `rustc`'s `Send` bound
against the `.nika` line. `a_described_foreign_call_is_not_asked_about_crossing`
asserts the silence so nobody rediscovers it.

***The question is asked.*** [`open-decisions.md`](open-decisions.md) carries
it: whether a described foreign function says that it puts what it is given on
a thread. The place is already right — C.1's rule is kept and the refusal names
the `.nika` line — and what is open is the **words**, which
[ADR-005](specification/adr/adr-005.md) D7 recorded as its own half.

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

*And the harness this waited on is built* ([ADR-177](specification/adr/adr-177.md)):
a crate holding the grammar and a `main` that parses the bytes, compiled and run
during the build, keyed on what went into it. What comes **back** is a
build-time value, and the shape this record needs — a flat list of declared
columns and parameters — is one of the five that cross. So what is left here is
the driver's own work rather than the machinery under it.

### 2.41. One name the prelude promises and `std` does not have

[ADR-154](specification/adr/adr-154.md). The rule is built for a function and
for a type; what is left is D1's own list naming **one** thing that does not
exist.

**`assert`** is on the list on Part I 1.3 and is not in `std`. It is not a piece
of work on its own: it belongs with the **testing chapter** (Part III 14), which
is not built either — there is no `nikaia test`, `assert c` parses as two
statements and is refused with `NK1117`, and `assert(c)` is a call to a function
of that name. Whatever `assert` is, it is decided there rather than here.

*The other three are answered.* **`Bytes`** is the language's and it exists
([ADR-156](specification/adr/adr-156.md) D1). **`panic`** exists
([ADR-161](specification/adr/adr-161.md) D3), ends the program with the
program's own words at the Nikaia line, and was built because
[ADR-114](specification/adr/adr-114.md) D1 writes it as the way a program says
it knows a key is present. And **`eprint`** is on the list
([ADR-162](specification/adr/adr-162.md) D1) rather than being a question:
diagnosis is the language's, and what it *does* is a thing only the compiler
knows, because only the compiler knows the target.

*A name on the list that does not exist* is the one direction a prelude can be
wrong in without anybody noticing — nothing refuses it, because nothing reaches
it. **The other direction is now watched too**
([ADR-162](specification/adr/adr-162.md) D3): a test reads the list off the page
and holds it against what `std` keys bare, which is what found `access_all` and
`update_all` sitting there unlisted.

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
`NK2302` for a naked view parameter that is stored, and `NK2303` for a view of
a buffer the body owns handed back through the result
([ADR-156](specification/adr/adr-156.md) D4) — which is D5's residual hard error
doing the job of the state that is missing. The second used to be `rustc` on the
Nikaia line, which is the one thing Part III C.1 says may not happen.

*What Tethered needs, measured rather than estimated.*

1. ~~**A buffer to tether to.**~~ **Built.** `Bytes` is the language's
   ([ADR-156](specification/adr/adr-156.md) D1, D2): a reference-counted,
   immutable run of bytes, and `fs::read` hands one back. What it does **not**
   carry yet is the offset and length a tethered *slice* is — a `Bytes` is the
   buffer, and the position beside it is item 3's layout. `Mapped` does not
   deref to it either (D6), which Part III 17.2 promises and the tether is what
   makes true.
2. ~~**The escape analysis** (D2).~~ **Built**, and built *alone*: every view in
   a signature carries its state in the ledger under `views`, `--tethers` prints
   it, and **nothing reads it**. What it found is that the whole corpus is the
   free case — every view in `examples/` and `benches/` solves to Borrowed,
   which is §3's worked check read off the analysis rather than asserted, and a
   test holds it as a ceiling. So the piece that was going to decide whether the
   rest is worth starting has answered: **nothing in the tree needs Tethered**,
   and what would reach it is a parser handing its rows past the buffer's scope.
3. **Three layouts per struct** (D3, D5), chosen per construction site. A
   tethered `Entry` is not a `ref String`: the **container** holds the handle (D4) and
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

*So it is not one change package.* It was at least four — the type, the
analysis, the representation, the ledger — and **the analysis is done**. What is
left is the representation and the type, and the representation is the expensive
half: three layouts per struct, chosen per site, with the container holding the
handle. **What rests on it:** *text is one type*, above, whose `String` state
comes from this analysis, and `Bytes` itself, which is the same question read
from the other end.

*Two things this list named do not rest on it*, and both were found by reading
it against the code rather than following it — which is the rule this file's own
head states. **The `keeps` column of a grammar's entry** closed at 0.0.137
([ADR-186](specification/adr/adr-186.md)): a parse keeps its `input` exactly
when its declared **result type** may hold a view into it, which is not a state.
**The third of `?.`** closed out of this list at 0.0.141
([ADR-190](specification/adr/adr-190.md)): three of its four shapes are
Borrowed and the fourth is `NK2303`'s, so none of it is an escape.

*Which is worth saying twice, because it is how this entry grows wrong:* a value
that is a **view** is not a value that is **Tethered**. The lattice has three
states and the cheap one is the default, so *a view of X* says nothing about
which — only **escaping the buffer's owning scope** does
([ADR-008](specification/adr/adr-008.md) D2).

*And the analysis has a limit worth knowing before it is trusted further.* It
errs **towards Tethered**, which is D7's own polarity, and the one shape it does
not decide is a buffer built element by element into a list whose element type
the ledger does not name. No program in the tree writes one.

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

### 3.1. A whole-workspace test run fails the project tests, and the cause is now Cargo's package cache rather than the wrapper's stdin

**Half of this is answered** ([ADR-166](specification/adr/adr-166.md)). Cargo
asks every `rustc` wrapper what the target looks like by running
`rustc - --print=… ` — `-` meaning *the program is on standard input* — and
writes nothing there. The wrapper passed that invocation through with the
standard input it had, which is **whoever started the build**, and anything
sitting in it was read as a Rust program:

```text
error: failed to run `rustc` to learn about target-specific information
  --- stderr
  error: unknown start of token: `
   --> <anon>:1:16
  1 | warning: trait `Foo` is never used
```

That is fixed, and `crates/nikaia/tests/project.rs` holds it with the real
probe through the real wrapper and a rendered diagnostic written onto its
standard input — a test that reproduces the original error without the fix,
which is what this entry never had.

**The earlier hypothesis was refuted for the right reason and the wrong one.**
It said the wrapper's inherited stdin was the suspect and then recorded that
refuted, because `cargo test … < /dev/null` failed identically three runs out
of three. The suspicion was correct; the measurement only showed that closing
the **outer** command's standard input does not close the **test binary's**,
and the nested build inherits from the harness.

**What is left is not the probe, and the claim this entry made in
[ADR-166](specification/adr/adr-166.md) was too strong.** That record said
something in a parallel sweep reaches a child's standard input *past an
explicit `Stdio::null()`*. Traced since — the wrapper's own trace says which
invocation is the probe (`NIKAIA_WRAPPER_TRACE`) — and in a **clean** sweep
both probes are recognised and the whole workspace passes, 124 binaries and no
failures, twice over. What the failing sweeps have in common is that they are
the **first run after a rebuild**; the next one passes with nothing changed.
So the binary the first run spawns is the suspect rather than the stream, and
the end-to-end test is `#[ignore]`d for that reason — a test that flakes in CI
is worth less than a red build costs. `cargo test -p nikaia --test project --
--ignored` is how to see it, and it passes every time on its own.

**And a second thing shows in a parallel sweep**, with the reproducer down from
the whole workspace to two binaries:

```text
cargo test --release --test project --test one_name
```

Two of the twenty-seven fail, and neither says anything about a probe:

```text
Blocking waiting for file lock on package cache
Blocking waiting for file lock on artifact directory
```

— one build's output arriving at the other test's assertion, and one that never
finishes behind the lock. The project tests share one Cargo package cache
(`NIKAIA_CACHE_DIR`, `shared_cache_dir()`), which is deliberate — it is what
`a_second_project_links_the_std_the_first_one_built` is about — and what has
not been decided is what that sharing costs when two test binaries want it at
once.

*Why it stays upkeep:* nothing a user does fails. `cargo test -p nikaia --test
project` passes on its own, repeatedly, and CI has been green on every release
through this one.

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
