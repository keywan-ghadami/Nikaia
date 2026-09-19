# Open decisions — the questions that need the owner

**Three entries are open**, below, and all three were found by *building* or by
being *asked for* rather than by reading — which is the only way this file fills
up once its reading-questions are answered. A fourth left it the day it arrived:
[ADR-142](specification/adr/adr-142.md), *a grammar's action may not pause*. An answer is an [ADR](specification/adr/), and
the moment a question is answered its entry leaves this file rather than
staying with a note on it. What is merely **unbuilt** is in
[`open-work.md`](open-work.md) — an ADR said what happens and the compiler does
not do it yet, which needs work and not a ruling. Each entry says what the
question is, why it is the owner's, and what this file recommends.

## Open

### 1. A name that is both a type and a function

**Found by building [ADR-140](specification/adr/adr-140.md) D1**, and
[ADR-133](specification/adr/adr-133.md)'s own open question had named the shape:
*a rule for a name that is both a type and a function — which the grammar allows
today and nothing in the corpus writes*. D1 removed the collision between the
two constructs and left this one untouched, and the build picked an answer in
silence:

```nika
struct Foo { n: i64 }
fn Foo(n: i64 = 0) -> i64 { return n }

let a = Foo(n: 1)   // error[NK1146]: … `Foo` is a type
                    // help: write `Foo { n: … }`
```

The type wins, so the **function is uncallable** through the only spelling
[ADR-133](specification/adr/adr-133.md) D1 gives it — and the help sends the
reader to a line that builds the struct, which is a *different program*. A wrong
help is worse than none.

**What is blocked:** nothing anybody has written; no `.nika` file in the tree
declares both. What is at stake is a message that is currently misleading, and a
rule that exists by accident rather than by decision.

*The options.*

1. **A name denotes one thing**, and declaring both is refused at the second
   declaration. *Costs:* one check over the item list and one message; the
   language loses nothing it uses.
2. **Both may be declared**, and the *call* form belongs to the function while
   the *brace* form belongs to the type — which is what the two spellings
   already mean everywhere else. *Costs:* `NK1146` has to ask whether the name
   is also a function before it fires, and a reader has to hold a rule that
   `Foo(n: 1)` and `Foo { n: 1 }` are two different programs.
3. **Leave it**, and fix only the help. *Costs:* the rule stays an accident, and
   the next record that touches either construct meets it again.

*Recommendation:* **option 1.** It is the same sentence the language already
says about two files declaring one name and about two packages under one alias —
[ADR-046](specification/adr/adr-046.md) D5's *one name per file*, applied one
namespace over — and it is the only option under which `NK1146`'s help is
always right.

*If it is wrong:* option 1 spent is a refusal that has to be lifted before
option 2 could be taken, and nothing in the corpus would notice either way.

### 2. The order of the five big unchecked boxes

**Written down from a truncated sentence, which is why it is back here.** This
file's old §8 asked for the order among the HTTP server, `std::db`, the C
library ([ADR-125](specification/adr/adr-125.md)), the query DSL and the
bare-metal target ([ADR-119](specification/adr/adr-119.md)). The edit that
answered it removed the partner names — deliberately, and that part is carried —
and left the recommendation cut off mid-sentence: *"`std::db` second, the C
library third, the query DSL fourth; the bare-metal target
([ADR-119](specification/adr/adr-119.md)) after that the server an."*

**What is blocked:** nothing, and that is the honest answer — this is **scope**
rather than work, and it is here only because
[`project_status_and_roadmap.md`](project_status_and_roadmap.md) now states an
order that was reconstructed rather than read. A paragraph that says something
the owner did not is worse than one that says nothing.

*What the paragraph says today*, and what it is reconstructed from: the HTTP
server first because every demo stands on it, then `std::db`, the C library, the
query DSL, and the bare-metal target after the server and the C library. The
first clause is the one the edit deleted; the last is
[ADR-119](specification/adr/adr-119.md)'s own scheduling, which the roadmap page
already cites, so any order putting bare metal before the server would
contradict a record.

*Recommendation:* **confirm or correct the first clause.** If the HTTP server is
no longer first, the sentence to replace it is one line and everything after it
holds.

*If it is wrong:* nothing is built on it — the cost is a roadmap paragraph a
reader takes for the owner's and is not.

### 3. The answer to "LINQ": the SQL DSL with rows typed from the schema, and no expression capture

**Asked by marketing** (SAP and DATEV want "LINQ and an ORM"), and the language
already has most of an answer that is better than the one asked for — which is
why this needs the owner to say so rather than a record to invent something.

*What LINQ is, read carefully.* Two things under one name. **LINQ to objects**
is `where`, `select`, `orderBy` over an in-memory collection: this language has
that as `filter`, `map`, `fold` over a `Vec` and a `Seq`
([ADR-105](specification/adr/adr-105.md)), with lambdas, and nothing is missing
but the keyword spelling nobody needs. **LINQ to SQL** is the interesting half:
the compiler keeps `u.age > 18` as an **expression tree**, and a *provider*
translates it into SQL at runtime. That is the part every user learns to
distrust — the expression the provider cannot translate fails at runtime,
the SQL it emits is nobody's and reads that way, and the N+1 query is invisible
in the source. [ADR-088](specification/adr/adr-088.md) §3 names expression
capture as *the one capability that would be genuinely new*, needed by exactly
this use case, and left it undecided.

*What this language does instead*, today, in Part II 10.5: the SQL is written
**as SQL**, in the dialect the database runs, inside `dsl mysql { … } eod`; the
compiler parses it at build time with the dialect's grammar, so a typo is a
compile error; every `:hole` is a **typed parameter** the call must pass by
name, so a missing or misspelled one is `NK1112`/`NK1113`; and the statement is
prepared once and reused. No translation layer, no provider, no tree: what
the reader sees is what the database receives. That is better than LINQ to SQL
at the thing LINQ to SQL was for — catching the query's mistakes before it runs.

*What it lacks, and what the question is.* Two things LINQ has and 10.5 does
not: the **result** is untyped (a row is a row; `name` and `email` are not
fields of anything until the program says so), and the query is not checked
against the **schema** (a column that does not exist is found by the
database). Both are one piece: the schema read at build time.

*The options.*

1. **Expression capture**, and a LINQ-shaped provider over it. *Costs:* the
   one construct ADR-088 kept off the list, a second query language beside SQL,
   a translation layer whose failures are runtime, and the readability argument
   of Part II 10.4 given up for one use case.
2. **The SQL DSL as it is, plus the schema at build time.** The grammar's driver
   takes the schema as a build-time input — `asset("schema.sql")` in a
   `comptime` initialiser is the mechanism [ADR-116](specification/adr/adr-116.md)
   already has — and the statement's **result type is derived**: a struct with
   one field per selected column, named and typed from the schema, so
   `for u in users { println(u.email) }` is checked and `u.emial` is `NK1117`.
   A column the schema does not have is a compile error at the query. The
   parameters are already typed. *Costs:* a schema grammar per dialect (DDL,
   the small subset that declares tables and columns), the driver reading a
   build-time asset, and the derived row type — no new construct, no
   evaluator beyond what `dsl` already runs at build time. An "ORM" is not
   added: the row type **is** the mapping, and a migration is a SQL file.
3. **Leave 10.5 as it is** and answer "no LINQ". *Costs:* the honest answer to
   the first half of the ask and none to the second; the untyped row is the
   thing a demo would be asked about first.

*Recommendation:* **option 2**, and this sentence to marketing: *Nikaia does
not translate your code into SQL; you write the SQL your database runs, and
the compiler checks it — the syntax, every parameter, every column against
your schema — before it runs, and gives you a typed row back. What LINQ
promised, without the provider.* For in-memory queries the answer is the
`Seq` combinators, and no keyword. It also says what the roadmap's "query
DSL fourth" **is**: this, which is why `std::db` stands before it — the
driver the schema is read for has to exist first.

*If it is wrong:* option 2 spent is a schema grammar and a derived type, both
of which stay useful under option 1; option 1 spent first is a construct that
cannot be taken back.

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
in place, with the mark given a definition beside the **Status** note's) and
[ADR-142](specification/adr/adr-142.md) (a grammar's action may not pause — the
entry that arrived and left in one round, because the answer was the demand an
`overlap` branch and a `par_iter` lambda already carry and the corpus wrote
nothing that would have to change). Each record
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

**That is the state to write down rather than to enjoy.** A page with nothing on
it never means there are no questions; it means none has been *found* yet, and
the way they are found is by building. The next one belongs here the moment
something comes to rest on it, in the shape this page has always asked for and
still describes above: what is blocked, the options, a recommendation, and what
either direction costs if it is wrong.

---

This file is a notes page: nothing here is normative. A decision taken from it is
written down in [`specification/adr/`](specification/adr).

