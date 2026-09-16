# Open decisions — the questions that need the owner

**Three entries are open**, below. Nothing answered lives here: an
answer is an [ADR](specification/adr/), and the moment a question is answered its
entry leaves this file rather than staying with a note on it. What is merely
**unbuilt** is in [`open-work.md`](open-work.md) — an ADR said what happens and
the compiler does not do it yet, which needs work and not a ruling.

The eighteen entries this file used to carry are gone that way, sixteen to
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
follow). Each record
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

## 1. What wakes the executor when the work is on a worker thread

**What is blocked.** Standard input does not suspend: `io::read`,
`io::read_to_string` and `io::lines` are `async fn` whose bodies are the
blocking read they always were
([`open-work.md`](open-work.md)'s entry on it). Behind it, and larger:
`rt::io::wait` — the readiness half of
[ADR-038](specification/adr/adr-038.md) D3, built for sockets — **cannot be
awaited**, only blocked on. That is what an HTTP server needs, so the entry
about there being no HTTP server rests on this too.

**The finding, and it is not what the work entry had recorded.** That entry
proposed wiring standard input to `Op::Readiness`. It would not have worked, and
the reason is the park hook rather than readiness:

* The **bell** (`rt::FINISHED` + `BELL`) exists for the *fallback* path. Its own
  comment says why: one private reply channel per operation and no way to wait
  for whichever finishes first, so a worker bumps a count and the hook watches
  it.
* On the **completion** path the hook waits on the ring instead, and
  `Ring::park` answers off `unreaped`, which counts ring jobs. **A worker
  operation is not one.**

So while the ring is the park, a worker's reply cannot wake the executor, and no
future may be fed from one: `exec::block_on` either spins (`main` alone) or
panics with its own *a future returned `Pending` without arranging for its waker
to be called*. `a_worker_operation_does_not_wake_the_completion_park` in
`crates/nikaia-std/src/rt/mod.rs` holds this, and is written to go red the day
it stops being true.

**The options.**

1. **Put standard input on the ring** (Linux), with the worker as the fallback —
   the two paths files already have. *Cost:* a second read shape, because stdin
   has no size to `stat` and the read is a loop until zero rather than one
   transfer; and it fixes standard input only. `rt::io::wait` stays
   un-awaitable, so the HTTP server still has nothing.
2. **Make the ring park hear the bell**, by registering an eventfd on the ring
   that `ring_the_bell` writes to. *Cost:* one fd and one always-armed
   submission, plus the care that a read of the eventfd is not mistaken for a
   completion. *What it buys:* **every** worker operation becomes awaitable at
   once — standard input, socket readiness, and whatever a later `Op` carries —
   and the two paths stop differing in what they can feed.
3. **Leave it**, and correct the work entry to say that standard input holds its
   thread by design until something needs otherwise.

**What I would do: option 2.** The asymmetry is the defect, not standard input:
the runtime has two ways to finish an operation and only one of them can wake
the thing that waits. Option 1 pays a special case for one caller and leaves
`rt::io::wait` where it is, which is the piece the next thing in the file wants.
Option 3 is honest and costs nothing today, and it is the right answer only if
the HTTP server is further off than it looks.

**What either direction costs if it is wrong.** Option 2's risk is a hang, which
is the worst failure this runtime can have — so it wants the same treatment
`park`'s own `false` got: a loud panic rather than a wait, and a test that a
worker reply wakes a ring park. Option 1's risk is that the second read shape is
written twice when option 2 lands anyway.

## 2. Whether a `catch` may name the error it takes

**What is blocked.** Nothing is blocked *today*, and the entry is here because
an accepted record writes a program the language cannot parse.
[ADR-111](specification/adr/adr-111.md) D5 says `Overtaken` is *"an error like
any other: caught or declared, retried in a loop with `catch Overtaken
{ continue }`"* — and there is one `catch` in this language, which takes
everything. So the retry loop that record writes is not writable: a program can
catch the overtaking, and it cannot catch the overtaking *and let a disk error
past*.

**It is not an oversight of D5.** The word appears once in the whole
specification, in that one line, and every other `catch` in the three pages and
in the corpus takes what comes. What D5 needed was for the failure to be an
ordinary error, which it is; the spelling beside it was written as if a typed
`catch` existed.

**The options.**

1. **`catch Overtaken { … }`, a typed handler**, with an untyped `catch` still
   meaning *everything*. *Cost:* the type has to be matched at run time, so the
   failure channel stops being a `Box<dyn Error>` a handler never looks inside
   and becomes one that is downcast — which is the mechanism, not a design
   change. And it raises the question the handler chain always raises: what an
   error that matches no arm does (propagate, presumably, which is what a `?`
   would have done).
2. **Leave `catch` as it is and correct D5's line**, so that the retry is
   written with the untyped handler: `catch { continue }` in a loop, which
   retries a disk error too, or a `match` inside the handler once errors carry
   something to match on.
3. **A `match` on the error rather than a second `catch` form**: `catch { match
   error { … } }` — one keyword, and the language already has the other half.
   *Cost:* `error` is a `Box<dyn Error>` and there is nothing to match it
   against, so this is option 1's downcast wearing a different syntax.

**What I would do: option 2 now, and option 1 when something needs it.** No
program in the corpus catches one error and lets another past, and the door
D5 exists for is complete without it — a `set(…; after:)` whose only failure is
`Overtaken` is served by `catch { continue }` exactly. What the record should
carry meanwhile is the honest spelling, and `docs/open-work.md` has the entry.

**What either direction costs if it is wrong.** Option 1 built early is a
handler form nobody writes, and a downcast in the failure path of every
program. Option 2 left too long is a language where a program that wants to
retry *one* failure has to retry all of them — and that is a silent wrong
behaviour rather than a refusal, which is the worse kind.

## 3. Which shape a declaration commits to for a handler that may pause

**What is blocked.** Two entries in [`open-work.md`](open-work.md) that turned
out to be one: *a lambda that pauses is refused at the build*, and
[ADR-102](specification/adr/adr-102.md)'s remaining step. A lambda whose body
pauses has no lowering — Rust has no stable `async` closure — and the way out
has been known all along: a closure that **returns** a future,
`|| async move { … }`. What was missing was a *callee's* parameter being able to
say it takes one, and D1's function type is now that claim. What is not settled
is which shape the **declaration** commits to.

**It is a question because D5 answers half of it.** A **kept** parameter that
may pause lowers to a boxed closure over a boxed future — that is written down.
A **run** parameter lowers *"as `std`'s do today, a closure argument"* — which
is `impl Fn(A) -> R`, and a lambda that pauses cannot be written into one. So
the case the whole entry exists for, a run parameter handed a pausing lambda,
falls between the two sentences.

**The options.**

1. **The type decides.** A parameter whose type allows pausing lowers to the
   future shape always; one that says `sync` lowers to a plain closure. *Cost:*
   a run parameter handed a lambda that does not pause pays a `Box::pin` and a
   dynamic call it does not need — and `fn(A) -> R` without `sync` is the
   *default*, so that is the common case paying for the rare one. *What it
   buys:* one rule, readable off the signature, with nothing inferred behind it.
2. **The run-or-kept inference decides**, which step 3 already computes: a run
   parameter stays a plain closure and a kept one gets the future shape, and a
   lambda that pauses handed to a *run* parameter is still refused. *Cost:* the
   refusal the entry was written about does not go away, and the emitter has to
   read a ledger column it does not read today. *What it buys:* nobody pays for
   a box they do not use.
3. **Both, keyed by the type**: `fn(A) -> R sync` is a plain closure, and
   everything else is the future shape — which is option 1 — *plus* `sync` on a
   run parameter becoming the thing a library author writes for speed. *Cost:*
   it makes `sync` a performance word as well as a promise, which is a meaning
   it does not have anywhere else in this language.

**What I would do: option 1.** The cost is a box on a call that already pays for
a closure, and the thing it buys is that a reader can tell what a signature
costs by reading it. Option 2 keeps the refusal this entry exists to remove,
which makes it the wrong answer to the question being asked; and a measurement
would settle the cost, which is what [ADR-009](specification/adr/adr-009.md)
D4's own standard asks for before a shape is chosen on a guess.

**What either direction costs if it is wrong.** Option 1 wrong is a box in
every `map`-shaped call a *user's* library writes — `std`'s own entries are
unaffected, since they are Rust and their signatures are hand-written. Option 2
wrong is that `examples/fortunes.nika`'s route handler still cannot be written,
which is the program this has been waiting on for three records.

---

This file is a notes page: nothing here is normative. A decision taken from it is
written down in [`specification/adr/`](specification/adr).

