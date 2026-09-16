# Open decisions — the questions that need the owner

**One entry, and it is open.** Nothing answered lives here: an
answer is an [ADR](specification/adr/), and the moment a question is answered its
entry leaves this file rather than staying with a note on it. What is merely
**unbuilt** is in [`open-work.md`](open-work.md) — an ADR said what happens and
the compiler does not do it yet, which needs work and not a ruling.

The seventeen entries this file used to carry are gone that way, fifteen to
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
`.to_string()` in the corpus, and the door for the hot loop). Each record
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

This file is a notes page: nothing here is normative. A decision taken from it is
written down in [`specification/adr/`](specification/adr).

---

## 1. Is an untrusted path a **refusal**, or is the taint analysis a feature this language should not have?

**Blocked by it:** [ADR-058](specification/adr/adr-058.md) D7, which
[`open-work.md`](open-work.md) has been carrying as *"the one piece that does
not wait"* and *"the only entry that closes a security hole rather than an
ergonomic one"*. And, behind it,
[ADR-010](specification/adr/adr-010.md) D8's argument for building the trust
lattice **generally** rather than as a hash-function special case — D7 is the
second consumer that argument was staked on.

**The entry is put back as a question because the owner questions the feature
itself**, and because two of the sentences that made it look ready are false.
Both were measured while picking the work up:

* ***"It costs no new analysis."*** It costs exactly one.
  `contracts::trust` is a **whole-program join** — it walks every body, collects
  the `std` sources the program calls, and folds their provenance into one
  answer for the program ([ADR-010](specification/adr/adr-010.md) D7's report,
  and the hasher choice D8 built it for). D7 needs a **per-value** answer:
  *which value* reaching `fs::map`'s path parameter came from where. That is a
  dataflow analysis over locals, through `let`, concatenation, `f"…"` and calls
  — a different thing that happens to read the same lattice. So D8's claim that
  the lattice generalised is not yet tested by this; it is *contradicted* to the
  extent that its second consumer needs a second analysis.
* ***"Testable against `fs::map` today."*** There is nothing to taint.
  `std.contracts` carries **no** `provenance = "untrusted"` entry — every source
  in it is `trusted`, because a file the operator named, a pipe they connected
  and the arguments they typed are all their own choice
  ([ADR-010](specification/adr/adr-010.md) D2). The one untrusted source that
  needs no server is `fs::map(path; trusted: false)`
  ([ADR-010](specification/adr/adr-010.md) D3), and **that is not built either**:
  the ledger parses the word, and no call site may write it.

**So the free-now-breaking-later asymmetry does not hold here, and that is the
finding.** [`open-work.md`](open-work.md) §2's rule — *a refusal is free before
programs exist and breaking afterwards* — is the reason every unbuilt refusal in
this project is urgent. It does not apply to this one: a refusal about untrusted
values cannot reject any program until some value **is** untrusted, and the
thing that makes a value untrusted is the request — which is the server, which
is its own project. The same day the refusal could break a program is the day it
becomes possible to write one. There is no window being lost.

**Four ways out.**

* **(a) Withdraw D7.** The language gets no compile-time rule about paths;
  `fs::within(root, name)` is an ordinary `std` function a server calls, and
  path traversal is a check the program writes, like every other language. The
  trust lattice keeps its one consumer — the hash-function choice
  ([ADR-010](specification/adr/adr-010.md) D8) — and stops claiming to be
  general. This is the option the owner's doubt points at, and
  [`README.md`](README.md) §2's rule says a **withdrawal** is a rewrite and not a
  supersession: the records would read as though the rule had never been there.
* **(b) Build it with the server, as part of it.** D7 lands the day
  `request.path()` exists, with its real source in view, and is designed against
  the shape it is actually for rather than against a stand-in.
* **(c) Build it now, with `trusted: false` as the source.** Two unbuilt things
  (the call-site marker and the per-value analysis) built for a consumer that
  does not exist, to guard a third that does not either.
* **(d) Keep the rule and narrow it to what a type can carry.** Rather than a
  flow analysis, an `Untrusted[T]` — a type a program has to unwrap through
  `fs::within` — which is the same guarantee paid for in the type system instead
  of in a pass. Costs a type constructor and every signature that touches one;
  it is a language change and not an analysis.

**What I would do: (b), and say so in the record rather than leaving D7 looking
ready.** The security property is real — a `../../etc/shadow` answered with a
200 is not a failure a status code fixes afterwards, and that sentence in D7 is
right. But it guards a door that has no building yet, and an analysis designed
without its real source is an analysis designed against a guess: the same
mistake [`open-work.md`](open-work.md) §1 recorded when a defect was filed with
a narrow characterisation and *"a probe two lines longer would have found"* the
real condition. **(a) is defensible and cheap to take**, and the honest thing to
say about it is that it costs the one property that would distinguish this
language's file handling from Go's or Rust's. **(d) is the interesting one** and
deserves a reading before (b) is taken as settled, because a type that cannot be
passed to `fs::map` needs no pass at all and cannot silently stop working — but
it changes the surface of every function that handles a name, which is the cost
a flow analysis exists to avoid.

**What either direction costs, in the numbers that were asked for.** **At
runtime, nothing** — every option but the runtime check inside `fs::within` is a
compile-time refusal, so a program that passes pays zero, and `fs::within`
itself is one path resolution plus a prefix comparison, once per name, which is
the check a server has to perform anyway. **At build time**, (b) and (c) cost
one more walk over the bodies the four derived columns already walk — a fifth
pass where there are four, so on the order of a fifth of the inference phase and
milliseconds on this corpus. (d) costs nothing at build time and something at
every signature. **(a) costs no time at all and one property.**
