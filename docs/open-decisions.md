# Open decisions — the questions that need the owner

**Eight entries are open**, below — the doors the records of the last rounds
left open on purpose, gathered here so that each has a recommendation and an
owner. The order of the five big pieces is answered on the roadmap page (the
HTTP server last). A question found by building rarely stays long: four of the
five this file held yesterday are records already —
[ADR-142](specification/adr/adr-142.md), *a grammar's action may not pause*,
[ADR-143](specification/adr/adr-143.md), *the driver checks the SQL at build
time*, [ADR-144](specification/adr/adr-144.md), *a name denotes one thing*, and
[ADR-146](specification/adr/adr-146.md), *a `match` covers every case* — which
arrived and left the same day, and whose recommendation had written `_` for the
rest one record before [ADR-145](specification/adr/adr-145.md) made it `else`.
An answer is an [ADR](specification/adr/), and
the moment a question is answered its entry leaves this file rather than
staying with a note on it. What is merely **unbuilt** is in
[`open-work.md`](open-work.md) — an ADR said what happens and the compiler does
not do it yet, which needs work and not a ruling. Each entry says what the
question is, why it is the owner's, and what this file recommends.

## Open

### 1. A pointer for the C direction

**Left open by [ADR-124](specification/adr/adr-124.md) §4**, and it is the
only part of that record not built: `extern "C" { fn malloc(size: usize) ->
Pointer[u8] }` is refused with `NK1135`, because `Pointer[T]` is a type nothing
declares. *A pointer that outlives what it points at is the one thing this
language is built not to allow.*

**What is blocked:** every C function whose signature has a pointer in it —
all of `libc`'s memory surface, and every C library that hands out a handle
(a database connection, an HTTP client, a compressor), which is every C library
worth calling. `getpid` compiles; nothing with a buffer or a handle does.

*The options.*

1. **`Pointer[T]`, a raw pointer** — Rust's `*mut T`, dereferenced inside
   `unsafe`. *Costs:* the language admits a value that can dangle, and every
   dereference is a hole in the borrow story that the compiler cannot close.
2. **Two shapes, and no raw pointer.** *A view for the call:* `&T`, `&mut T`,
   `&[u8]`, `&mut [u8]` in a declaration lower to the pointer and live for the
   call, as every view does; a length is a separate named `usize` parameter,
   and the compiler checks it against the view at the call site
   (`read(fd, buf, count)` with `count <= buf.len()` or a refusal). *An opaque
   handle:* declared in the block — `opaque type sqlite3 released by
   sqlite3_close` — an address the language never dereferences, movable and
   storable, whose release is a `cleanup` the compiler runs at the end of its
   scope, so a handle cannot outlive what it names unless a C function says so.
   Returned text (`getenv`) is an opaque `CStr` handle a `std` function copies
   inside `unsafe`. `malloc` stays unwritable **by design**: memory the language
   would index has to arrive with a length the language knows. *Costs:* the two
   declaration forms, the length check, the handle's cleanup, and a `std`
   function for C strings.
3. **Leave it**, and C is `getpid`.

*Recommendation:* **option 2.** The thing to prevent is the dangling
*dereference*, and both shapes make one impossible by construction; together
they cover what real C libraries look like — handles and buffers — and they
are the language's own words (a view, a `cleanup`) said at the boundary, which
is how [ADR-125](specification/adr/adr-125.md) answered the other direction.

*If it is wrong:* option 2 is the safe subset of option 1, so nothing spent is
lost the day a raw pointer is admitted; option 1 first cannot be narrowed later.

### 2. `select`, and the one word that cancels a task

Part II 12.4 writes `select { result = heavy_math() => { … }  _ =
sleep(5.seconds()) => { throw Timeout::TooSlow } }` and marks it *unspecified*
([ADR-141](specification/adr/adr-141.md) D2). The **semantics** exist — the
loser stops at its pause point, its `cleanup` is adopted, the deadline bounds
it — and the runtime's race is what [ADR-129](specification/adr/adr-129.md)'s
C `cancel` already leans on. What is missing is the construct, and a word:
today nothing but `select` cancels a task, so a program cannot say *stop* to
something it started.

*The questions:* whether `select` is syntax or a function (syntax — a branch
binds a name and runs a block, which no function parameter can); the arm's
form; and whether the handle `spawn` returns has a `cancel()`.

*The options.*

1. **`select` as a block construct**, arms `pattern = expr => { … }` with the
   ignore pattern for a branch whose value is not wanted; cancellation only
   through losing a `select`. *Costs:* a keyword — the most expensive thing a
   language adds ([ADR-084](specification/adr/adr-084.md)) — the arm grammar,
   and the lowering onto the runtime's race.
2. **Option 1, and `cancel()` on the handle `spawn` returns**, with exactly the
   semantics of losing a `select`, so that a program's own cancel and
   [ADR-129](specification/adr/adr-129.md)'s C `cancel` are one mechanism.
   *Costs:* option 1's and one method.
3. **`select` as a library over a `Task` type.** *Costs:* the winner's value
   cannot be bound to a name without syntax, so the example on the page cannot
   be written.

*Recommendation:* **option 2.** `overlap { … }` set the precedent for this
family — a block that runs its parts at once — and `select` is its sibling that
keeps one; a language with `overlap` and no `select` has half a pair. The
`cancel()` costs nothing new because the runtime already does it for a loser.

*If it is wrong:* `cancel()` alone is not enough, because it cannot bind the
winner's value; the keyword is the part that cannot be taken back.

### 3. A channel: `std`'s, and only bounded

Part II 12.5 writes `let (tx, rx) = channel::bounded(100)`, `tx.send(…)`,
`rx.recv()`, and marks it *unspecified*. Nothing in it needs syntax: two
values, two methods, and the tuple `let` is built
([ADR-098](specification/adr/adr-098.md)). The questions are where it lives and
what it promises.

*The options.*

1. **`std::channel` with `bounded(n)` only.** `send` **pauses** when the
   channel is full — so a `sync` body cannot send on one, which the ledger says
   by itself; `recv` returns `T?`, `null` once every sender is gone; the value
   type must `cross` ([ADR-123](specification/adr/adr-123.md)), checked where
   `tx` moves into a `spawn` as any move is. *Costs:* one module in `std` over
   the runtime's queue.
2. **Option 1 plus `unbounded()`.** *Costs:* a memory leak with a name, and a
   `send` that never pauses, which is the one thing a program cannot reason
   about under load.
3. **A language construct.** *Costs:* syntax for what two methods say.

*Recommendation:* **option 1.** A capacity is a promise about memory, and the
language makes programs say their promises; a program that wants "unbounded"
writes a large number and has said it.

*If it is wrong:* `unbounded()` is additive; a bounded channel cannot be
taken away once written.

### 4. The duration: `5.seconds()`

Part II 12.4's `sleep(5.seconds())` writes a value of a type nothing declares.
The questions are the type (`std::time::Duration`, no dispute) and the spelling.

*The options.*

1. **Methods on integers in `std`** — `5.seconds()`, `250.millis()` — as an
   extension `std::time` provides, plus the constructors
   (`Duration::seconds(5)`). *Costs:* nothing in the language; a trait impl in
   `std`.
2. **A suffix literal**, `5s`, `250ms`. *Costs:* a literal form
   [ADR-136](specification/adr/adr-136.md) §4 already declined for numbers
   (*the digits are a spelling; the use gives the type*), reopened for one type.
3. **Constructors only.** *Costs:* `sleep(Duration::seconds(5))` where every
   example on the page reads `5.seconds()`.

*Recommendation:* **option 1.** It is what the pages already write, it is
`std`'s and not the language's, and it keeps the literal rule whole.

*If it is wrong:* a suffix is additive, and the method form loses nothing to it.

### 5. `break` with a value

[ADR-138](specification/adr/adr-138.md) §4: Rust's `break x` hands a value out
of a `loop`; this language has no unconditional loop
([ADR-070](specification/adr/adr-070.md)), so `while true { … break x }` would
be the only home for it, and `NK1133` refuses it today.

*Recommendation:* **keep the refusal**, and make its message name the two
shapes that carry a value out of a loop: a `let` before it that the loop
assigns, or a function whose loop `return`s. A `while` is a statement, its
value is unit, and one construct that is sometimes a value and sometimes not
is the ambiguity `if` was spared by being an expression always.

*If it is wrong:* additive later; the refusal costs nothing but a message.

### 6. A fixed-size array

Two records point here: [ADR-127](specification/adr/adr-127.md) §4 (a field
`[f64; 3]` of a C value struct) and [ADR-135](specification/adr/adr-135.md) §4
(a container that does not allocate, which the bare-metal target's `startup`
profile will ask for on its first day).

*The questions:* the spelling and the literal. Rust's `[T; N]` is a new type
form; the language's type grammar already writes every parameterised type with
brackets, `Vec[T]`, `Shared[T]`.

*Recommendation:* **`Array[T, N]`**, the bracket generic the type language has,
with `N` a `comptime` integer; by value; indexed as a list is, with the abort
of [ADR-114](specification/adr/adr-114.md) out of range; a literal `[1.0, 2.0,
3.0]` takes the array type where the use asks for one, which is
[ADR-135](specification/adr/adr-135.md)'s rule for the empty list extended to a
full one. No new syntax, and both records' doors close.

*If it is wrong:* the type's name is the only thing to change.

### 7. A native Node add-on

[ADR-130](specification/adr/adr-130.md) and [ADR-131](specification/adr/adr-131.md)
give Node the WebAssembly build, and what a WebAssembly module cannot reach on
Node — the filesystem, a socket of its own — is left open. A host marketing
named runs Node.

*Recommendation:* **not until a program asks**, and when one does, `nikaia bind
node` generates an N-API shim **in C over the C library** — generated source
the host's own toolchain compiles, as the Python binding is generated source
the host's own `ctypes` loads — and not a Rust-side `napi` dependency in the
emitted crate. That keeps [ADR-131](specification/adr/adr-131.md) D1 whole: one
artifact, and every binding a file over it.

*If it is wrong:* nothing is spent until the generator is written.

### 8. What needs no `use`: the prelude

[ADR-140](specification/adr/adr-140.md) D5 decided how `use` works and left
what needs none — `Vec`, `String`, `println` — undecided, so the line is drawn
by what the compiler happens to know.

*Recommendation:* **a written list on Part I's first page, small and closed**:
the containers a program cannot do without (`Vec`, `String`, `Bytes`), the
printing functions, `assert`, `panic`, and the numeric conversions — nothing
that does I/O but printing, nothing that pauses. Everything else, `HashMap`
first, is `use std::…`. A name joins the list by a record.

*If it is wrong:* adding to a prelude is additive; removing from one breaks
programs, which is the argument for starting small.


## Answered

The entries this file used to carry are gone that way, most to
their records and two because they were never questions for the owner at all —
the second being whether a word-sized shared value drops its lock, which is a
**performance idea** and is documented as one in
[`lock-free.md`](lock-free.md) §6: the measurement is done, nothing is
half-built, and no program in the tree contends a lock:
[ADR-046](specification/adr/adr-046.md) (`use` brings nothing in),
[ADR-047](specification/adr/adr-047.md) (a package is a directory),
[ADR-048](specification/adr/adr-048.md) (the numeric surface),
[ADR-049](specification/adr/adr-049.md) (the automatic `a`, `b`, `c` withdrawn),
[ADR-050](specification/adr/adr-050.md) (`overlap { … }`),
[ADR-051](specification/adr/adr-051.md) (keywords are reserved),
[ADR-053](specification/adr/adr-053.md) (a package is its own crate),
[ADR-055](specification/adr/adr-055.md) (a task is a coroutine) and
[ADR-059](specification/adr/adr-059.md) (`access` reads, `update` writes — the
question that stopped being one rather than getting an answer) and
[ADR-060](specification/adr/adr-060.md) (a literal no use constrains takes the
first type that holds it, which needed none of the inference it seemed to) and
[ADR-075](specification/adr/adr-075.md) (what a program may do at compile
time: a build-time body calls what is `sync` and touches at most the build's own
parameters — two columns that already exist, so the restriction the question asked
somebody to invent turned out to be maintained by the compiler) and
[ADR-070](specification/adr/adr-070.md) (there is no unconditional loop
keyword — `while true` is it, and Go is the precedent read carefully: it has no
`while` at all, so what it shows is that *one* keyword is enough rather than that
the form is unnecessary) and
[ADR-084](specification/adr/adr-084.md) (`break` and `continue` join the
language — the entry that was **built before it was answered**, because a keyword
is the most expensive thing a language adds and *"what does it cost"* deserved
numbers rather than an estimate; what decided it was not a percentage but a
capability, since a `for` could not be stopped at all) and
[ADR-069](specification/adr/adr-069.md) (`http` leaves `std` and becomes a
package — the entry that asked what `std::http` should contain, when the question
underneath was whether it is in `std` at all, which the specification had been
answering both ways) and
[ADR-063](specification/adr/adr-063.md) (and so does a sum of them — the option
this file recommended, once it turned out that *leaving it* was not one, because
what it left was the backend's own message on a Nikaia line) and
[ADR-100](specification/adr/adr-100.md) (a consumer reads a dependency's
ledger and derives it again only where the sources it came from changed — the
question of whether a consumer derives a dependency's contracts, answered the
way `std`'s already were) and
[ADR-101](specification/adr/adr-101.md) (an error that newly reaches a
`catch` is named once in the build, and the ledger commit is the
acknowledgement — the syntax of `catch` is unchanged) and
[ADR-082](specification/adr/adr-082.md) §5 (the old `dsl … from …` form is
removed in the one change that migrates what writes it — never a question,
only work whose size had been miscounted) and
[ADR-103](specification/adr/adr-103.md) (a package is found by version through
Cargo, under the crate name `nikaia_<name>` — the registry, the version
grammar and the lockfile were already in the tool every build runs) and
[ADR-105](specification/adr/adr-105.md) (the ledger says `Seq[T]` for what is
produced step by step and `Par[T]` where the steps run at once — the word the
type language lacked, and the 35 silent calls behind it) and
[ADR-106](specification/adr/adr-106.md) (a bound takes a path, `use` stays as it
is, and the ledger records a trait and each `impl` where they were written —
the one position that named a type without allowing a path) and
[ADR-107](specification/adr/adr-107.md) (text is one type whose state the
compiler picks, and `&str` is the assertion that it is a view — the 13
`.to_string()` in the corpus, and the door for the hot loop) and
[ADR-108](specification/adr/adr-108.md) (a path names its root at the call —
the taint analysis the question doubted, and the answer is that the check goes
where the name and the directory are both in hand, so there is nothing to
follow) and
[ADR-124](specification/adr/adr-124.md) (what `extern "C"` costs the language —
answered the way this file asks a question to be answered, with a **number**:
nothing in the corpus, the tests or the three pages writes `extern` or `unsafe`
as a name, and nothing is released, so two reserved words cost nothing and the
owner said to take them. The entry had recommended going *around* them, and the
measurement is what made the direct answer the cheap one) and
[ADR-121](specification/adr/adr-121.md) (the ring's park hears the bell — the
entry about what wakes the executor, answered by making every worker
operation awaitable rather than by a special case for standard input) and
[ADR-111](specification/adr/adr-111.md) D5 corrected (the retry is the untyped
`catch`, which is exact where `Overtaken` is the only failure — the entry
about a `catch` that names its error, answered by not needing one yet) and
[ADR-122](specification/adr/adr-122.md) (a function-typed parameter lowers by
its type, the future shape unless it says `sync` — the entry about which
shape a declaration commits to, with the box measured before it is closed)
and
[ADR-123](specification/adr/adr-123.md) (`crosses` says *no* as well as *yes*
— the entry about the two refusals that could not fire, answered the way
`locks` was) and
[ADR-135](specification/adr/adr-135.md) (a list literal is `[1, 2, 3]`, `[]`
takes its element type from the first use and is refused where none says one,
and a `[` at the start of a line begins a literal) and
[ADR-136](specification/adr/adr-136.md) (`1_000_000`, `0xFF`, `0b1010`, `0o17`
— and the radix is a **spelling**, so `0xFF` is `255` and takes the first type
that holds it, because otherwise `0x0FF` would be a wider type than `0xFF`) and
[ADR-137](specification/adr/adr-137.md) (six more `match` patterns, a guard is
`if`, and `..` in a pattern is inclusive — so `..<` is the exclusive range
everywhere and `..=` goes, which is the answer this file's recommendation asked
for and one form more than it knew about) and
[ADR-138](specification/adr/adr-138.md) (`throw`, `return`, `break` and
`continue` are expressions of the never type, so an arm, a `??` right side and
an `else` may each end in one) and
[ADR-139](specification/adr/adr-139.md) (a doc comment is a language feature
and the ledger carries it in a derived `doc` column, because the ledger is the
one file a consumer reads and a sentence that is not in it is not read) and
[ADR-140](specification/adr/adr-140.md) (the five places one thing had two
spellings, picked: the brace literal, the anonymous constructor with `new` gone
from `std`, `::` everywhere, `throws` after the result type, and `use`
bringing nothing in — which also freed
[ADR-133](specification/adr/adr-133.md)'s call half, since the named literal
was the construct it collided with) and
[ADR-141](specification/adr/adr-141.md) (six of the specification's nine slips
corrected on the page and the three that are **constructs** marked *unspecified*
in place, with the mark given a definition beside the **Status** note's) and
[ADR-142](specification/adr/adr-142.md) (a grammar's action may not pause — the
entry that arrived and left in one round, because the answer was the demand an
`overlap` branch and a `par_iter` lambda already carry and the corpus wrote
nothing that would have to change) and
[ADR-144](specification/adr/adr-144.md) (a name denotes one thing, and the
second declaration is refused — the entry that turned out to be **three**
holes rather than the one it asked about: a `trait` and a `grammar` were not
counted at all, and everything else was counted only when the build had a
manifest) and
[ADR-146](specification/adr/adr-146.md) (a `match` covers every case, and the
refusal is this compiler's — the entry that only ever decided *whose message*,
since the backend has been enforcing it all along on a Nikaia line) and
[ADR-143](specification/adr/adr-143.md) (the database driver checks the SQL at
build time against the schema — the "LINQ" the question was really about, and
the answer is that this language already had the better half of it: the SQL is
written as SQL and checked before it runs, so what was missing was the schema
and a typed row rather than an expression tree). Each record
holds its own reasoning, its alternatives and what they cost; reading the answer
here *and* there was two copies of one thing, and the copy that goes stale is
always the notes page.

**Each entry says what is blocked, what the options are, what I would do, and
what either direction costs** — because a question without a recommendation is
work handed back rather than a decision asked for.

**What "blocked" means is broader than work, and narrower than everything.** An
entry may block no work at all and belong here anyway: `std::http` blocked
*reading* an accepted record whose surface half could not be evaluated, which is
a cost that grew silently until [ADR-069](specification/adr/adr-069.md) stopped
it. What does *not* belong is a question nothing rests on — which SQLite
binding `std::db` would use is undecided and blocks nothing, because `std::db`
does not exist in any form. That is **scope**, and
[`project_status_and_roadmap.md`](project_status_and_roadmap.md) holds it; scope
becomes a decision by something coming to rest on it.

**Every question this file held has been answered**, and the last nine went in
one round, each the way this page had recommended: the list literal
([ADR-135](specification/adr/adr-135.md)), the number literal's four forms
([ADR-136](specification/adr/adr-136.md)), the six `match` patterns and the
range spelling ([ADR-137](specification/adr/adr-137.md)), the four jumps as
expressions ([ADR-138](specification/adr/adr-138.md)), the doc comment
([ADR-139](specification/adr/adr-139.md)), the five double spellings
([ADR-140](specification/adr/adr-140.md)), the specification's own slips
([ADR-141](specification/adr/adr-141.md)), the roadmap's ordering — which is a
paragraph in [`project_status_and_roadmap.md`](project_status_and_roadmap.md)
and names no partner, because naming one is theirs to agree to — and the
collision between an options-only call and a named struct literal, which needed
no ruling of its own in the end: ADR-140 D1 takes the named literal out of the
language, so the spelling has one owner.

**Two of the nine came back with something the question did not contain**,
which is the argument for writing a recommendation down rather than deciding in
one's head. `..` inclusive everywhere meant `..=` had to go, and the entry had
not noticed the language already has `..=` in an expression
([ADR-137](specification/adr/adr-137.md) D5). And the marketing entry's
recommendation was edited to garbled text on its way here; what survived it
unambiguously was that the partners are not named, which is what the roadmap
paragraph carries — the ordering itself is the last complete version of the
sentence, and [ADR-119](specification/adr/adr-119.md)'s own scheduling is what
pins the bare-metal target after the server.

**And it did not stay that way for one round.** The page was empty for exactly
as long as it took to *build* three of the records it had just answered: a
grammar action that pauses, a name that is both a type and a function, and a
roadmap sentence that was reconstructed rather than read. None of the three was
findable by reading — the first two are what the compiler does when a program
does something nobody had written, and the third is what a truncated edit leaves
behind.

**That is the state to write down rather than to enjoy.** A page with nothing on
it never means there are no questions; it means none has been *found* yet, and
the way they are found is by building. The next one belongs here the moment
something comes to rest on it, in the shape this page has always asked for and
still describes above: what is blocked, the options, a recommendation, and what
either direction costs if it is wrong.

---

This file is a notes page: nothing here is normative. A decision taken from it is
written down in [`specification/adr/`](specification/adr).

