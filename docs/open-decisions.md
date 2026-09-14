# Open decisions — the questions that need the owner

**Four entries, and every one of them is open.** Nothing answered lives here: an
answer is an [ADR](specification/adr/), and the moment a question is answered its
entry leaves this file rather than staying with a note on it. What is merely
**unbuilt** is in [`open-work.md`](open-work.md) — an ADR said what happens and
the compiler does not do it yet, which needs work and not a ruling.

The fourteen entries this file used to carry are gone that way, twelve to
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
[ADR-069](specification/adr/adr-069.md) (`http` leaves `std` and becomes a
package — the entry that asked what `std::http` should contain, when the question
underneath was whether it is in `std` at all, which the specification had been
answering both ways) and
[ADR-063](specification/adr/adr-063.md) (and so does a sum of them — the option
this file recommended, once it turned out that *leaving it* was not one, because
what it left was the backend's own message on a Nikaia line). Each record
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

## 1. Is there an unconditional loop?

**Blocked by it:** nothing is half-built, and that is why it is here rather than
in `open-work.md` — there is nothing to build until this is answered.

Part I 3.3 has `while` and `for` and no third form. `loop` has been on the
roadmap since it was written and is **not in the specification**, so the
compiler having no rule for it is correct rather than a gap. What a program
writes today is `while true { … }`, which works.

**Two ways out.**

* **(a) Nothing. `while true` is the unconditional loop.** One fewer keyword,
  and a reader of Part I 3.3 has the whole of control flow on one page.
* **(b) `loop { … }`.** It says *"this does not end on its own"* at the top
  rather than leaving a reader to notice that the condition is a constant, and
  in a language that will eventually want a `break` with a value it is the form
  that carries one.

**What I would do: (a), until something asks for (b).** No program in the corpus
writes `while true`, so the form nobody uses does not need a second spelling —
and a keyword is the most expensive thing to add and the hardest to remove
([ADR-051](specification/adr/adr-051.md) made every one of them a reserved
word). (b) is the right answer the day a `break` hands back a value, because
`while true { … }` with a value-carrying `break` reads as a lie.

**What it costs:** (a) costs a line in Part I 3.3 saying so, so that the absence
is a decision rather than an omission — which is the whole point of asking. (b)
costs a keyword, a reserved word, and a grammar rule.

---

## 2. What may a program do at compile time?

**Blocked by it:** compile-time I/O, and with it the **asset dimension of the
build cache**, which is carried through `Key::build` and exercised by tests with
no real producer behind it ([ADR-021](specification/adr/adr-021.md) D13).

**This one is written down where it belongs.**
[ADR-026](specification/adr/adr-026.md) is the record, its status is **Open**,
and it holds the whole design space: two things already decided (I/O belongs to
the compiler rather than to a sandbox; a path stays in the project root and `..`
is refused rather than resolved), six questions as Q1–Q6, prior art, what was
considered and rejected, and what answering it buys the cache.

**The one that blocks the others is Q4 — what is a program allowed to do in
`const`?** Everything else in that record is downstream of it: whether a sandbox
is needed at all, what it would be, and where the trust line goes all read
differently depending on how much a build-time body may reach.

**What I would do:** answer Q4 alone, narrowly, and leave Q1–Q3 and Q5–Q6 where
they are. A grammar's `action` blocks are arbitrary Nikaia, so evaluating one at
build time means running user code at build time — and the cheap version of that
is a restriction rather than a sandbox: name what a `const` body may call, and
the question of confining it does not arise. ADR-026 §4 makes that case itself.

**What it costs:** a narrow answer is a list in a record and a check in the
compiler. A wide one is a sandbox, which that record's §6 already declines on
the grounds that nothing in the language needs one yet.

---

## 3. How is a package named by a version?

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


---


---

## 4. Does the ledger's type language grow, so fewer values are `?`?

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
