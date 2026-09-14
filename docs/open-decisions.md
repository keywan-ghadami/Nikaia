# Open decisions — the questions that need the owner

**Five entries, and every one of them is open.** Nothing answered lives here: an
answer is an [ADR](specification/adr/), and the moment a question is answered its
entry leaves this file rather than staying with a note on it. What is merely
**unbuilt** is in [`open-work.md`](open-work.md) — an ADR said what happens and
the compiler does not do it yet, which needs work and not a ruling.

The twelve entries this file used to carry are gone that way, eleven to their
records and one because it was never a question for the owner at all:
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
[ADR-063](specification/adr/adr-063.md) (and so does a sum of them — the option
this file recommended, once it turned out that *leaving it* was not one, because
what it left was the backend's own message on a Nikaia line). Each record
holds its own reasoning, its alternatives and what they cost; reading the answer
here *and* there was two copies of one thing, and the copy that goes stale is
always the notes page.

**Each entry says what is blocked, what the options are, what I would do, and
what either direction costs** — because a question without a recommendation is
work handed back rather than a decision asked for.

**What "blocked" means is broader than work, and narrower than everything.** §4
blocks no work at all and belongs here anyway: it blocks *reading* an accepted
record whose surface half cannot be evaluated, which is a cost that grows
silently. What does *not* belong is a question nothing rests on — which SQLite
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

## 4. What **is** `std::http`?

**Why it is here and not in the work list.** [ADR-058](specification/adr/adr-058.md)
is accepted and none of it is built, which is ordinary; what is not ordinary is
that four of its nine decisions describe a **surface nobody has chosen**. `http::File`
is a type in an interface that does not exist. "A body may be a `Bytes` or a
mapping" says what a handler may return. A bounded, invalidated table of mappings
is the inside of a library there is no outside for. Each of those reads as settled
and is not — the question they rest on has never been put.

**What is *not* in question**, so the record is not reopened wholesale. Five of its
decisions constrain whatever `std::http` turns out to be, and hold either way:

* a path out of a request is `Untrusted` and may not reach the filesystem
  unchecked — a statement about the provenance lattice and the filesystem, not
  about a server, and the one piece buildable before a socket exists;
* `std` chooses the mechanism and the program never does, with the operator able
  to overrule — a principle, and the one four production servers arrived at
  independently;
* `splice` is not it; TLS turns the file path off as a property; whatever carries
  the bytes pauses through the executor or is not `std`'s to call;
* the length is settled before the status line — a fact about HTTP.

**The question.** Not how a server is implemented, but what its surface *is*:

* **What answers a request.** A handler returns the answer today
  ([ADR-018](specification/adr/adr-018.md) D2), and the set of things it may
  return is what ADR-058 D1 and D2 quietly extend. Is that set open, closed, or a
  trait the program may implement?
* **Who builds the server.** `http::Server::new().route(…).listen(…)` is the
  README's shape and nothing decides it.
* **Whether `std` ships it at all**, or whether an HTTP server is a package —
  which is now a real alternative, since a package can depend on a package
  ([ADR-053](specification/adr/adr-053.md)). `std` shipping a web server is a
  choice most languages made once and regretted differently.

**What is blocked by it:** nothing today, and that is the point. It blocks
*reading* — an accepted record whose surface half cannot be evaluated, because
what it would constrain has no shape yet. The order is the wrong way round, and
writing that down is cheaper than discovering it when somebody builds to it.

**A note on order rather than a recommendation.** The path check needs no answer
here and closes a security hole; it can be built while this stays open.

---

## 5. Does a word-sized shared value drop its lock?

**Blocked by it:** nothing is half-built. The measurement is done
([`lock-free.md`](lock-free.md)), so what is left is the ruling and then the work.

[ADR-039](specification/adr/adr-039.md) §3 leaves a door open and promises
nothing: `update` takes a pure function and is therefore **repeatable**, which is
the route to an implementation that retries instead of locking — Clojure's `atom`
and Haskell's `TVar` are that route. The numbers are now in, and they make the
question narrow rather than large.

**What the measurement says.** A compare-and-swap loop against the two shapes the
compiler writes, uncontended, two runs:

* against the **cheap** shape: **×6.5 to ×7.5**. It must never replace that one —
  and that one is what every value gets at `user_parallelism = no`;
* against the **crossing** shape: **×0.63 to ×0.67**, about 6 ns a door, and
  several times that under contention where only the sign reproduces.

So the shape of a yes is: **a third representation for a word-sized value that
may cross a thread**, replacing the crossing shape and nothing else. `i32`,
`i64`, `bool`, `char` — not "a small type": a two-number struct is small and has
no atomic instruction.

**The options.**

* **Leave it.** Six nanoseconds a door on the values that cross, and the
  contended case stays the expensive one it is. Nothing to build, nothing to
  explain, and the door stays open because the block's shape keeps it open.
* **Build it for `get` and `set` only.** Those need no repetition at all, so the
  one thing that is missing below is not needed. It is the smaller half of the
  win and all of the safety.
* **Build it for `update` too**, which needs the thing below.

**What it needs, and it is one derivation away now.** A block that may be
**repeated**. `sync` says a block does not *wait*; it does not say it has no
**effect**. Run, not argued:

```nika
let k = SharedMut(0)
k.update fn(alt) {
    println(f"ich laufe: {alt}")     // prints, today
    return alt + 1
}
```

So a retry would print twice. ADR-039 §3 calls the missing property *"an
additional assurance which does not exist yet"*, and **the half that was missing
is there now**: `touches` is inferred over the call graph since
[ADR-067](specification/adr/adr-067.md) D2, so a user-written function answers
what it reaches instead of *"nobody said"*, and *"touches nothing, and it is
known"* is very nearly *"repeating it is unobservable"*.

**What is left is to say that the two are the same thing** — and one asymmetry
that has to be written down first, because it is not obvious and it changes how
carefully a `std` entry has to be read.

**An empty touch set means different things to the two consumers.** To
`contracts::order` it is a **speed**: a function that reaches nothing may overlap
with anything, and an entry that forgets a resource buys an overlap it should not
have. To a repetition it would be a **permission**: repeat me freely, and an entry
that forgets a resource loses an effect. Same mistake, and the second consumer
pays more for it. Whoever takes this on should say so where a `std` entry is
written, not only where it is read.

**And the clock is the example.** A repetition can observe anything the vocabulary
does not name; the vocabulary names `file`, `stdout`, `stderr`, `args` and `lock`.
It does not name a clock — and `std` has no function that reads one, so the rule
that a word waits until a program asks for it keeps it out, exactly as it kept
`lock` out until yesterday. `touch.rs` carries the note for the day it changes,
including what to write: `clock read`, because two calls conflict over nothing and
neither changes anything.

**And the contradiction it uncovered is settled.** Part II said a `sync` function
*"will never do I/O"*; `std.contracts` had `println` as `sync = true`, and the
program above is what that meant in practice.
[ADR-067](specification/adr/adr-067.md) D1 gives the word **one** promise — it
never pauses — and what a body reaches is the second column's to answer.

**And one interaction, cheap to state and expensive to miss:** a value that
appears in a door over several locks ([ADR-065](specification/adr/adr-065.md))
cannot take the atomic shape — `update_all` holds both at once and a retry loop
cannot be held. Such a value keeps a lock.

**What I would do: the second, and not yet.** `get` and `set` are free of the open
assurance and would take the win where it is safest. But nothing in the corpus
contends a lock today, so the honest order is to leave it until a program does —
and the reason to write the question down now is that the measurement is fresh and
will not be repeated cheaply.

**What it explicitly is not:** a switch. There is already a decision that there is
no way to *ask* for the cheaper reference count — the remedy for a fallback is a
contract, not a permission ([ADR-037](specification/adr/adr-037.md) D8). A third
lock shape follows the same rule: the compiler takes it where it can prove the
value qualifies, and `--sharing` says why not.
