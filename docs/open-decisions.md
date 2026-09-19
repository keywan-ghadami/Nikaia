# Open decisions — the questions that need the owner

**Nine entries are open**, below, and the ninth was found by *building* one of
the others rather than by reading. An answer is an [ADR](specification/adr/),
and the moment a question is answered its entry leaves this file rather than
staying with a note on it. What is merely **unbuilt** is in
[`open-work.md`](open-work.md) — an ADR said what happens and the compiler does
not do it yet, which needs work and not a ruling. Each entry says what the
question is, why it is the owner's, and what this file recommends.

## Open

### 1. What a list literal is: `[1, 2, 3]`, and the empty one

The literal is table stakes (`language-review.md` §3.1) and the parser has no
`[` in expression position, so the syntax is free. Two questions are the
owner's: what `[]` is (a `Vec` of the first type a later use gives it, as a
number literal takes its width — [ADR-060](specification/adr/adr-060.md)'s rule
applied to a container — or a refusal asking for the type), and whether a
literal at the start of a statement after an expression is an index or a new
literal (Rust and JavaScript answer this differently). *Recommendation:*
`[]` takes the type from the first use and is refused where none exists;
a `[` at the start of a line begins a literal, because an index across a
line break is a shape nobody writes.

### 2. Number literals: `1_000_000`, `0xFF`, `0b1010`, `0o17`

`NK1117` today (*nothing declares `_000_000`*). The forms are Rust's and the
lowering is verbatim. The one question: does a hexadecimal literal take the
first type that holds it as a decimal does ([ADR-060](specification/adr/adr-060.md)),
or is `0xFF` a `u8` because eight bits were written? *Recommendation:* the
same rule as decimal — a literal is a value, and the digits it was written in
are a spelling — so `0xFF` is an `i32` unless a use says otherwise.

### 3. `match` patterns: tuple, or, range, guard, nested, and `..` in a struct

Six shapes, none built; `calc.nika` matches `step.0` because it cannot match
`step`. Each is Rust's and lowers verbatim. The owner's questions are two:
whether a guard is `if` (Rust) and whether a range pattern is `1..=5`
(Rust) or `1..5` inclusive as the language's own `for` range is not — the
language's `..` is exclusive, and a pattern reader from Rust expects `..=`.
*Recommendation*: if for the guard; .. for the inclusive range pattern, with ..< adopted across the language for exclusive ranges; ..< never in a pattern, so that patterns remain purely inclusive and visually clean.

### 4. A bare `throw` as a `match` arm

`=> throw NotFound` is a parse error; it must be `=> { throw NotFound }`.
The question is whether `throw` (and `return`, `break`, `continue`) are
expressions of the never type ([ADR-093](specification/adr/adr-093.md) has
the type) or statements that need a block. *Recommendation:* expressions of
the never type, as Rust has them, so that an arm, an `??` right side and an
`else` branch may all end in one.

### 5. Doc comments, and a `doc` column in the ledger

`///` is an ordinary comment ([ADR-134](specification/adr/adr-134.md) D3
keeps it so). The ledger ships and the prompt bundle is on the roadmap, and
neither has anywhere to take a sentence about a function from. The question
is whether a doc comment is a language feature (a `doc` column in
`nikaia.contracts`, a `///` the parser keeps, `nikaia doc`) or a convention
the tooling reads. *Recommendation:* a language feature — the ledger is the
one place a consumer reads, and a sentence that is not in it is not read.

### 6. Two spellings for one thing (`language-review.md` §3.3)

Five pairs, each needing a pick: the struct literal `Stats(min: 1)` beside
`Reading { name, temp }`; the anonymous constructor beside `Type::new()`; the
path separator `::` beside the dot of `Json.value(input)`; `throws` before
`->` beside after; `use` bringing in a name for `std` and none for a package.
*Recommendation:* the brace literal, the anonymous constructor with `new`
gone from `std`'s own types, `::` everywhere, `throws` after the type, and
`use` as [ADR-046](specification/adr/adr-046.md) says with `std` brought in
line. Each is a record of its own, and each costs an example migration.

### 7. The specification writes things the language does not have (§3.5)

`neg.is_some()`, a postfix `?`, `List[T]` for `Vec[T]`, a lambda carrying
`sync`, a string where an enum was argued for, an error raised as a
positional constructor, `5.seconds()`, `channel::bounded`, `select { … }`.
Most are corrections a page can take without a ruling; three are not — the
`select` block, the duration literal and the channel — because each is a
construct. *Recommendation:* correct the six; decide the three when their
chapter (Part I 8) is next opened, and mark them *unspecified* until then.

### 8. A roadmap note for the marketing wish list

*Recommendation:*  `std::db` second, the C library
([ADR-125](specification/adr/adr-125.md)) third, the query DSL fourth; the
bare-metal target ([ADR-119](specification/adr/adr-119.md)) after that the server
an. One
paragraph in `project_status_and_roadmap.md` would say it.

### 9. `execute(target_age: 30)` and `Stats(min: first)` are one spelling

**Found by building [ADR-133](specification/adr/adr-133.md), and it blocks that
record's call half.** D3 argues the parser can tell an options-only call apart on
the second token because *nothing in expression position begins with a name
followed by a colon — a struct literal begins `Name {`*. Kap 4.2's other struct
literal does: `Stats(min: first, max: first)` builds the struct, and
`execute(target_age: 30)` is the same five tokens. The parser tries the literal
first, so D1's new call form is read as a struct literal for a struct nothing
declares.

**What is blocked:** ADR-133 D1's and D2's *call* halves, and with them Part III
15.1's `script.exec(msg: message)` — which is why the specification's lowering
floor stays where it is. The **signature** halves are built: `fn execute(target_age:
i64 = 0)` parses and the leading `;` in a signature is refused, because nothing
competes with that shape.

**What is not blocked any more:** the silence. A struct literal naming nothing
this compiler declares used to lower verbatim and come back from `rustc` as
*cannot find struct `execute`* — Part III C.1's class — and is now `NK1135`,
whose message names the call where the name is a function.

*The options.*

1. **Resolution decides, and the parser does not.** A name denotes one thing, so
   `Name(field: value)` is a struct literal where `Name` is a type and a call with
   options where it is a function. The precedent is in the language already:
   `Stats(first)` is Kap 4.2's anonymous constructor and is parsed as a **call**,
   with the checker and the emitter turning it into `Stats::new(first)` by looking
   the name up. This is the same lookup on the other form. *Costs:* the fork in
   `check` and in `emit`, and a rule for a name that is both a type and a function
   — which the grammar allows today and nothing in the corpus writes.
2. **The named constructor goes**, leaving `Stats { min: first }` as the only
   struct literal and `name(opt: v)` unambiguous. *Costs:* a form the
   specification writes in several places and the corpus uses, and a migration
   with nothing to gain but the parser's simplicity.
3. **ADR-133's call half is withdrawn** and an options-only call keeps its
   leading `;`. *Costs:* the shape no reader has seen in any language, kept for a
   collision the compiler could resolve — and D2's *one spelling per shape* then
   applies to the signature only, which is the asymmetry the build is currently in.

*Recommendation:* **option 1.** It is the answer the language already gives for
the other half of the same form, it needs no migration, and it keeps both
constructs exactly as their records describe them. What it asks for is one
sentence — *a name denotes a type or a function, and that is what tells the two
forms apart* — and one refusal for the case where somebody declares both.

*If it is wrong:* option 1 spent on a form that should have gone (option 2) is a
fork in two files that then becomes dead; option 3 spent is the two parser
alternatives already written, reverted. Neither is expensive, which is why the
question is worth asking rather than guessing.

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

**Every question this file held has been answered**, and the last four went in
one round: the ring's park ([ADR-121](specification/adr/adr-121.md)), the retry
that needs no typed `catch` ([ADR-111](specification/adr/adr-111.md) D5,
corrected), the shape a handler's declaration commits to
([ADR-122](specification/adr/adr-122.md)), `crosses` saying *no*
([ADR-123](specification/adr/adr-123.md)) — and what `extern "C"` costs
([ADR-124](specification/adr/adr-124.md)), answered with the number this file
asks for.

**That is the state to write down rather than to enjoy.** A page with nothing
on it means the work in [`open-work.md`](open-work.md) is the kind that needs
building rather than deciding; the next question belongs here the moment
something comes to rest on it, in the shape this page has always asked for and
still describes above: what is blocked, the options, a recommendation, and what
either direction costs if it is wrong.

---

This file is a notes page: nothing here is normative. A decision taken from it is
written down in [`specification/adr/`](specification/adr).

