# Open decisions — the questions that need the owner

**Nothing is open.** An answer is an [ADR](specification/adr/), and the moment
a question is answered its entry leaves this file rather than staying with a
note on it. What is merely **unbuilt** is in [`open-work.md`](open-work.md) —
an ADR said what happens and the compiler does not do it yet, which needs work
and not a ruling. Each entry says what the question is, why it is the owner's,
and what this file recommends.

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
in place, with the mark given a definition beside the **Status** note's). Each record
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

**That is the state to write down rather than to enjoy.** A page with nothing
on it means the work in [`open-work.md`](open-work.md) is the kind that needs
building rather than deciding; the next question belongs here the moment
something comes to rest on it, in the shape this page has always asked for and
still describes above: what is blocked, the options, a recommendation, and what
either direction costs if it is wrong.

---

This file is a notes page: nothing here is normative. A decision taken from it is
written down in [`specification/adr/`](specification/adr).

