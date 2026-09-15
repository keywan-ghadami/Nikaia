# Open decisions — the questions that need the owner

**Five entries, and all of them are open.** Nothing answered lives here: an
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
only work whose size had been miscounted). Each record
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

## 1. How is a package named by a version?

**Blocked by it:** every dependency that is not a path.
`nikaia.toml` refuses `http-server = "1.2"` and says why — no record names a
registry, a name space, or a distribution format — and
[ADR-002](specification/adr/adr-002.md) D1 §5 declines to answer it deliberately,
so the refusal is correct rather than missing.
[ADR-053](specification/adr/adr-053.md) was built to keep it deferred: a
manifest key is only ever read by one crate, so nothing in the language needs a
global name yet.

**Three ways out, and they are not equally big.**

* **(a) Nothing yet.** A path dependency is what a package gets, and a Nikaia
  library is distributed the way `std` already is
  ([ADR-002](specification/adr/adr-002.md) D4). Costs nothing and stays honest
  as long as there is no second author.
* **(b) Lean on Cargo's registry.** A Nikaia package is published as a crate,
  and a version means what Cargo means by one. The resolver, the lockfile and
  the name space all already exist and this compiler already generates a
  workspace into them.
* **(c) A registry of this language's own.** A name space, a distribution
  format, an index, and the operational commitment that comes with all three.

**What I would do: (a) now, and (b) when there is a second author.** The
question is not ripe: there is one author, one repository, and no package
anybody outside it would fetch — and a distribution format decided before
anything is distributed is decided on a guess. (b) is the answer when it becomes
ripe, for the reason ADR-053 took its own answer: machinery this project already
leans on beats machinery it would then own. (c) needs a reason nothing has given
yet.

**What it costs:** (a) costs a refusal a future consumer meets, which is where
it belongs. (b) costs a record and a mapping from a Nikaia package to a crate
name — and pins this language's distribution to Cargo's, which is a real thing
to give up and the reason it deserves a decision rather than a drift. (c) costs
a project of its own.

---

## 2. Does the ledger's type language grow, so fewer values are `?`?

**Blocked by it:** nothing is half-built. What it blocks is how often this
compiler can answer at all — a cost that is paid everywhere and shows up nowhere
as a failure, which is the kind this file exists to make visible (§4's own
reason).

**Measured first, and then measured again after the cheap half was taken.**
Across the fifteen `.nika` files in `examples/`, `tests/samples/` and
`crates/nikaia-std/src/`, **51** method calls went unanswered. By immediate
cause:

| | calls | |
| ---: | :--- | :--- |
| 16 | the receiver's type **is** known and no entry describes the method | `Args::nth` (10), `String::push` (5), `Tally::map_or` (1) |
| 35 | the receiver was **already** `?` | the cascade |

And the cascade's roots: 24 a local or parameter, 6 a method call whose own
result was `?`, 4 a field, 1 a free call.

**The third option below has since been taken**, and it bought exactly what it
was predicted to: **51 → 35**, with the *no-entry* bucket at **0** — every call
whose receiver type is known now resolves — and **not one** of the 35 gone. So
what is left is one thing, and it is this question.

**The finding that makes this a question rather than a work item.** The roots are
mostly **not** missing entries. `HashMap::keys` has one — and it says `-> ?`:

```toml
[fn."HashMap::keys"]
signature = "(&HashMap[?, ?]) -> ?"
```

so one line costs a whole chain:

```nika
let mut names = totals.stations.keys().collect()   // keys → ?, so collect → ?
names.sort()                                        // so names → ?, so sort → ?
```

Four unanswerable calls from one `?`. Of the ledger's **86** entries: 53 are
fully written, **9** have a result of `?`, 22 carry a `?` among the parameters,
and 2 have no signature at all.

**Why those nine are `?`, and it is not an oversight.** `keys` hands back an
**iterator**, and the ledger's type language has no word for one. Its variables
bind from the receiver ([ADR-031](specification/adr/adr-031.md)), so `-> $K` is
sayable and *"a sequence of `$K`, lazily"* is not. `collect` is the other half of
the same gap: what it builds depends on the **target**, which no signature
written against the receiver can name.

**The options.**

* **Leave it.** `?` is the absence of a claim and not a wrong answer
  ([ADR-024](specification/adr/adr-024.md)), and everything downstream is built
  to be conservative about it — which is why the corpus compiles and runs today.
  The cost is silent: fewer refusals this compiler can make in its own words, and
  positions like [ADR-068](specification/adr/adr-068.md)'s wrap having to work
  without an answer.
* **Give the ledger an iterator type**, so `keys`, `values` and `chars` can say
  what they hand back and `collect` can be written against it. It is the change
  with the leverage — six of the roots are exactly this — and it is a change to
  the **type language**, which nothing else in the compiler has needed yet.
* **Fill only what is sayable today**: `Args::nth`, `String::push`, and the
  entries whose result is a concrete type. That closes 16 of the 51 and none of
  the cascade, and it is worth doing either way. **Done** — and one of the three
  turned out not to be a missing entry at all: `HashMap::get` claimed `-> $V`
  where a key may not be there, which refused `counts.get(k)?.n` and accepted
  `counts.get(k).map_or(…)`. A wrong entry costs more than a thin one.

**The third is done; the second is the decision.** The third was ordinary ledger
work with a measured payoff and no new machinery, and it is finished. The second
is where **all 35** of what is left now are, and it is a question about what a
contract can *say* — which is the owner's, not a thing to start building on a
guess.

**What it is not.** It is not [ADR-028](specification/adr/adr-028.md) D5 being
wrong. An entry exists because a program asked for it, and every entry named here
was asked for; what is at issue is whether the entries that exist may say more
than they do.

---

## 3. Can a bound name a trait in another package?

**Blocked by it:** a generic function in one package constrained by a trait
declared in another. `fn dispatch[H: http::Handler](…)` is a parse error — a
bound is one name — and `fn dispatch[H: Handler](…)` after `use http` is
`NK1126`, because `use` brings no name in and a bound has no way to say where
the trait lives. The `impl` side already crosses the boundary
([ADR-095](specification/adr/adr-095.md)); the bound side does not.

Handing a package a *handler* is not this question any more:
[ADR-102](specification/adr/adr-102.md) gives a parameter a function type,
and `examples/http/`'s `route` is written with one. What is left is the trait
door for its own sake — a `Repository` bound, a `Render` bound — across a
package.

**Two ways out.**

* **A qualified bound**: `[H: http::Handler]`, resolved the way a qualified
  type in a parameter already is, and the ledger carrying a package's `traits`
  so the lookup has something to read. The smaller change, and the one the
  `impl` side already made for its half.
* **Leave it until a program needs it.** No program in the tree writes a
  cross-package bound; the handler that motivated it is answered elsewhere.

**What I would do: the first, when the first such program arrives.** It is a
lookup and a ledger column, not a language question — but it is also §1's
neighbour (what a package publishes), and that is the reason not to decide it
on nothing.

**What it costs:** the first costs the `traits` column and the qualified
bound's parse; the second costs a refusal a library author meets, which is
where it belongs until then.

---


## 4. Is text one type whose state the compiler picks, or two the program picks between?

**Blocked by it:** nothing half-built. What it blocks is 13 `.to_string()` in
`examples/`, every one a literal or a view being put where a `String` is
declared, and `NK1106` telling the author to allocate.
[ADR-094](specification/adr/adr-094.md) closes the `&`-at-the-call row of the
same count and names this as the axis it deliberately does not touch: that one
is *mode* (lent or kept), this one is *representation* (a view or an owned
buffer), and Part I 6.6 already lets the compiler choose among three states
for a view. Text is the one type where the choice is still the program's, at
the type.

**Two ways out.**

* **Leave it.** `String` owns, `&str` views, `.to_string()` says an allocation
  happens here. Consistent with [ADR-005](specification/adr/adr-005.md) §3's
  *no copy the user did not write* — and it costs 13 spellings in 913 lines,
  and `Response(content_type: "text/plain".to_string())` in a language whose
  README promises scripting-language readability.
* **One text type, three states.** `String` (or a new name) is what `Bytes`
  already is — a buffer that may be borrowed, tethered or owned — and a literal
  stored in a field is a *tethered view of static text* that allocates nothing.
  `.to_owned()` stays for a copy the program wants. `&str` leaves the writable
  surface the way `usize` did ([ADR-048](specification/adr/adr-048.md) D1). The
  cost: a text value is a pointer, a length and a handle rather than a pointer,
  a length and a capacity; a text that views into a mapped file pins the
  mapping, which is 6.6's documented cost applied to text; and `std`'s ledger
  entries that say `&str` today say the one type instead.

**What I would do: the second, after ADR-094's step 3 has rewritten the
examples**, so that the two rewrites of the corpus are one. The argument that
decides it is the one ADR-094 rests on: the compiler already chooses a view's
state per use, and a program that has to say it for text and not for a slice
is being asked the same question twice with two answers.

**What it costs either way:** leaving it costs the 13 and every one after them;
taking it costs a representation change in `std`, a record, and the one
measurement this project would want first — what the handle costs on a text
that is compared and hashed a billion times, which `benches/refcount` can be
pointed at.

---

## 5. Is an untrusted path a **refusal**, or is the taint analysis a feature this language should not have?

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
