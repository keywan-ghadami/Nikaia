# Open decisions — the questions that need the owner

**One entry is open**, below. Nothing answered lives here: an
answer is an [ADR](specification/adr/), and the moment a question is answered its
entry leaves this file rather than staying with a note on it. What is merely
**unbuilt** is in [`open-work.md`](open-work.md) — an ADR said what happens and
the compiler does not do it yet, which needs work and not a ruling.

The twenty-two entries this file used to carry are gone that way, twenty to
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
`locks` was). Each record
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

## 1. What `extern "C"` costs the language, and in which order

**What is blocked.** Talking to C. [Part III 15.1](specification/30-nikaia-tooling.md)
writes the whole thing out — an `extern "C"` block, a call inside `unsafe { … }`
— and it is the direction the roadmap asks for first, ahead of letting C call
*in* ([ADR-062](specification/adr/adr-062.md), which is the other one and is
about threads rather than about syntax).

**What the page writes and the language does not have.** Three things, each
checked rather than recalled:

| written in 15.1 | what happens today |
| :--- | :--- |
| `extern "C" { … }` | *expected end of input; found `extern`* — the word is not reserved (Part I 2.1) and the item is not in the grammar |
| `unsafe { … }` | parses as a name and a block, and is `NK1117`, *nothing declares `unsafe`* |
| `Pointer[u8]` | `NK1135`, *nothing declares the type `Pointer`* |

`usize` is **not** on that list and is fine: it is writable although `as usize`
is `NK1122`, which is [ADR-054](specification/adr/adr-054.md) D1's own
distinction between what `as` may name and what may be written.

**So this is not one decision but three, and two of them are the expensive
kind.** `extern` and `unsafe` are **reserved words**, and
[ADR-084](specification/adr/adr-084.md) calls a keyword the most expensive thing
a language adds — [ADR-117](specification/adr/adr-117.md) has just taken four
*off* the list, and a record that puts two back is going the other way and
should say why in numbers rather than in intent. `Pointer[T]` is a **type**, and
Part I 2.2's surface is a closed set on purpose.

**The options.**

1. **Two words and a type**, as 15.1 writes it. *Cost:* `extern` and `unsafe`
   stop being names — a search of the corpus says how many programs that costs,
   which is the measurement [ADR-084](specification/adr/adr-084.md) took for
   `break` — and `Pointer[T]` joins the surface with a lifetime story of its
   own, because a pointer that outlives what it points at is the one thing this
   language has been built not to allow.
2. **One word.** `extern "C"` becomes an *attribute* on an ordinary
   declaration — `@foreign fn malloc(size: usize) -> Pointer[u8]` — the way
   `@borrowed` already is, so only `unsafe` is reserved. *Cost:* the page is
   rewritten, and an attribute is a weaker signal than a block for something a
   reader must see.
3. **No new word at all.** A foreign declaration is a **ledger** entry rather
   than source — which is exactly what
   [ADR-104](specification/adr/adr-104.md) decides for a Rust crate, one section
   further down the same page: `nikaia describe` writes the entry, the file is
   reviewed like code, and the *program* writes an ordinary call. C is the same
   shape with a narrower translation table. *Cost:* no `unsafe` marker at the
   call site, so the boundary is visible in the ledger and not in the body.

**What I would do: option 3, and measure before option 1.** The machinery ADR-104
builds for Rust is the machinery C needs — a described boundary, hashed, reviewed,
fail-closed on `touches` and `locks` — and C's table is *smaller* than Rust's, not
larger. It also answers the `Pointer[T]` question by not asking it yet: a
described C function's parameter types are whatever the table can name, and what
it cannot name is `?`, which is [ADR-024](specification/adr/adr-024.md) D1's own
answer for an absent claim.

**What either direction costs if it is wrong.** Option 3 wrong is that a reader
cannot see at the call that they are crossing into C, which is the thing `unsafe`
exists to show — and the way back is option 1, unchanged, so nothing is spent.
Option 1 wrong is two reserved words that a later record would have to take off
the list again, which [ADR-117](specification/adr/adr-117.md) has just shown is
possible but is not free: every program that used the name in between is a
program that broke.
