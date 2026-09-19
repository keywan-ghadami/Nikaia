# Open decisions — the questions that need the owner

**Nothing is open.** The nine doors the records of the last rounds had left
open were gathered here so that each would have a recommendation and an owner,
and the owner took all nine — each the way the entry recommended. They are
[ADR-147](specification/adr/adr-147.md) to
[ADR-155](specification/adr/adr-155.md).

**That is the shape this page is for**, and it is worth saying once: the
entries were written to be *answerable* — what is blocked, the options, a
recommendation, and what either direction costs if it is wrong — and eight
answers arrived in one sitting because there was nothing left to work out at
the moment of deciding — and the ninth was written while the thing it blocked
was being built and answered in the same round. A question without a
recommendation is work handed back.
An answer is an [ADR](specification/adr/), and
the moment a question is answered its entry leaves this file rather than
staying with a note on it. What is merely **unbuilt** is in
[`open-work.md`](open-work.md) — an ADR said what happens and the compiler does
not do it yet, which needs work and not a ruling. Each entry says what the
question is, why it is the owner's, and what this file recommends.

## Open

Nothing.

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
[ADR-155](specification/adr/adr-155.md) (a handle may be absent, and `T?` is how
it says so — the entry that was written **while the thing it blocked was being
built**, because [ADR-147](specification/adr/adr-147.md) D3's own example turned
out not to be a program: three ways out, what each cost, and the one that was
already a type this language has) and
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
[ADR-147](specification/adr/adr-147.md) (the C boundary has a view and an
opaque handle, and no raw pointer — the thing to prevent is the dangling
*dereference*, and both shapes make one impossible by construction) and
[ADR-148](specification/adr/adr-148.md) (`select { … }` keeps the first branch
to finish, and a task handle has `cancel()` — `overlap`'s missing half, which
is the argument for the keyword) and
[ADR-149](specification/adr/adr-149.md) (a channel is `std`'s and only bounded
— a capacity is a promise about memory, and this language makes programs say
their promises) and
[ADR-150](specification/adr/adr-150.md) (a duration is `std::time::Duration`
written `5.seconds()`, with no suffix literal, so
[ADR-136](specification/adr/adr-136.md)'s rule keeps no exception) and
[ADR-151](specification/adr/adr-151.md) (`break` carries no value, and the
refusal names the two shapes that do — the one of the eight that was already
built and needed only its reason written down) and
[ADR-152](specification/adr/adr-152.md) (a fixed-size array is `Array[T, N]`,
which closes two records' doors with one type and adds no type form) and
[ADR-153](specification/adr/adr-153.md) (a native Node add-on waits for a
program, and is generated C over the C library — the entry whose answer is
*not yet*, written down so the shape is decided before the pressure is) and
[ADR-154](specification/adr/adr-154.md) (the prelude is a written list, small
and closed — nothing that does I/O but printing, nothing that pauses, and
`HashMap` is the first thing outside it) and
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

**Then it filled up a second way, and emptied a second time.** Eight doors that
accepted records had left open *on purpose* were gathered here — a pointer for
C, `select`, a channel, a duration, `break x`, a fixed array, a Node add-on, the
prelude — and all eight were answered in one sitting. That is not the page
working differently; it is the same page reading its own instruction: each entry
had already done the deciding work, so what was left was the owner saying yes.
A door left open by a record is a question whether or not anyone has tripped
over it yet, and writing it down here is what turns *we did not decide that* into
*that is decided*.

**That is the state to write down rather than to enjoy.** A page with nothing on
it never means there are no questions; it means none has been *found* yet, and
the way they are found is by building. The next one belongs here the moment
something comes to rest on it, in the shape this page has always asked for and
still describes above: what is blocked, the options, a recommendation, and what
either direction costs if it is wrong.

---

This file is a notes page: nothing here is normative. A decision taken from it is
written down in [`specification/adr/`](specification/adr).

