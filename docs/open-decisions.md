# Open decisions — the questions that need the owner

**Six entries, and every one of them is open.** Nothing answered lives here: an
answer is an [ADR](specification/adr/), and the moment a question is answered its
entry leaves this file rather than staying with a note on it. What is merely
**unbuilt** is in [`open-work.md`](open-work.md) — an ADR said what happens and
the compiler does not do it yet, which needs work and not a ruling.

The nine entries this file used to carry are gone that way, eight to their
records and one because it was never a question for the owner at all:
[ADR-046](specification/adr/adr-046.md) (`use` brings nothing in),
[ADR-047](specification/adr/adr-047.md) (a package is a directory),
[ADR-048](specification/adr/adr-048.md) (the numeric surface),
[ADR-049](specification/adr/adr-049.md) (the automatic `a`, `b`, `c` withdrawn),
[ADR-050](specification/adr/adr-050.md) (`overlap { … }`),
[ADR-051](specification/adr/adr-051.md) (keywords are reserved),
[ADR-053](specification/adr/adr-053.md) (a package is its own crate) and
[ADR-055](specification/adr/adr-055.md) (a task is a coroutine). Each record
holds its own reasoning, its alternatives and what they cost; reading the answer
here *and* there was two copies of one thing, and the copy that goes stale is
always the notes page.

**Each entry says what is blocked, what the options are, what I would do, and
what either direction costs** — because a question without a recommendation is
work handed back rather than a decision asked for.

**What "blocked" means is broader than work, and narrower than everything.** §6
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

## 1. What does `access` hand its lambda?

**Blocked by it:** the surface of `Locked[T]` and `SharedMut[T]`.
[ADR-057](specification/adr/adr-057.md) decided what the type **is** and built
both shapes; what a program writes to reach one is this question, and it is the
last thing between the lock and a program somebody can run.

**The state.** [ADR-039](specification/adr/adr-039.md) D10 gives four doors, and
two of them take a lambda:

| door | for | what the lambda gets |
| :--- | :--- | :--- |
| `kasse.update fn(old) { old + 100 }` | new from old, small values | the value; the result replaces it |
| `kasse.access fn(state) { … }` | in place, large values | **undecided** |

`update` is settled by its own row: handed the old value, returns the new one,
and `fn(old) { old + 100 }` is a lambda this language already has. `access` is
not. Part II 12.2's own in-place example writes `to.balance += 100`, which
mutates *through* the parameter — and a lambda's parameter has no spelling that
says it may be mutated, so there is nothing for the compiler to read.

**Three ways out.**

* **(a) `access` hands a mutable reference, and a lambda may name one.** The
  examples compile as written. It costs the language a new spelling in a
  parameter list — `fn(mut state)` or `fn(&mut state)` — which is a change to
  Part I 5.3's one lambda form, and 5.3 is the rule
  [ADR-049](specification/adr/adr-049.md) was written to protect.
* **(b) `access` hands a mutable reference and needs no spelling**, because the
  callee's contract says the parameter is one — the same way
  [ADR-029](specification/adr/adr-029.md) D1 already gives a lambda's parameters
  their *types* from the signature rather than from the source. Nothing changes
  in the grammar; what changes is that a lambda's parameter can be mutable
  without the reader seeing it.
* **(c) `access` goes, and `update` is the only lambda door.** A large value is
  then updated by being handed back, which for a big struct is a move rather
  than a mutation — and the row's own reason for existing ("in place, large
  values") is exactly what that gives up.

**What I would do: (b).** The precedent is already set and it is the one this
language keeps citing — a lambda's parameters are described by the callee, not by
the caller, because only the callee knows
([ADR-029](specification/adr/adr-029.md) D1, [ADR-031](specification/adr/adr-031.md)).
Mutability is the same kind of fact as the type, arrives from the same place, and
adding a spelling for it in (a) buys a reader one word at the cost of a second
lambda form across the whole language.

**What it costs, and it is the real objection to (b):** `to.balance += 100`
inside a lambda mutates something the line does not say is mutable, and this
language's whole position on `mut` is that a reader can see it. The counter-case
is that `access` is *named* `access` — a door whose purpose is mutating in
place — so the mutation is in the construct rather than hidden by it. (a) is the
answer if that is not enough, and it is not irreversible either way: a spelling
can be added later without invalidating a lambda written without one.

---

## 2. Does an un-annotated integer literal have a type?

**Blocked by it:** the one remaining entry in
[`open-work.md`](open-work.md) §1 — an out-of-range literal that nothing
constrains is refused in Rust's words.

```nika
let big = 3000000000
```

is *"literal out of range for `i32`"*: the right line, the backend's words, and a
type the program never wrote, which is the class [Part III
C.1](specification/30-nikaia-tooling.md) calls a bug in this compiler. `NK1116`
does not reach it, and **must not simply be widened to it**, because the same
line is a *correct* program where a use asks for an `i64`:

```nika
let m = 3000000000
println(f"{wide(m)}")   // fn wide(n: i64) -> i64
```

Rust's inference decides that one and this checker has none, so refusing at the
`let` would refuse a correct program — the one thing the checker may never do
(C.4). Part I 2.4 states the rule that makes both lines legal.

**Two ways out.**

* **(a) An un-annotated literal is an `i32`, full stop.** The second program
  above becomes a refusal, and the language gains a rule a reader can apply
  without knowing what inference does. It is what the *first* program's error
  message already assumes, which is why the message exists.
* **(b) Enough inference to know that nothing else constrains the literal.** Both
  programs stay legal and the first is refused in Nikaia's words. It is the
  answer that costs nothing at the surface and the most underneath: a use-site
  walk this checker does not have, for one rule.

**What I would do: (b), and not soon.** (a) is a smaller compiler and a worse
language: `let m = 3000000000` followed by `wide(m)` is what somebody writes, and
Part I 2.4 says it works. The defect is a *message*, not an accepted wrong
program — the literal never reached run time — so the cost of waiting is one
poorly-worded error, and the cost of (a) is a program the page promises being
refused. If the inference turns out not to be worth building, (a) is the
fallback and Part I 2.4 is what has to change with it.

**What it costs either way:** (a) is a paragraph in Part I 2.4 and a check that
already exists; (b) is a use-site pass over a `let`'s scope, which nothing else
in this compiler currently needs.

---

## 3. Is there an unconditional loop?

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

## 4. What may a program do at compile time?

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

## 5. How is a package named by a version?

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

## 6. What **is** `std::http`?

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
