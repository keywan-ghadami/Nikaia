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
way `std`'s already were). Each record
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


---


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

## 3. Is `dsl … from …` removed when the call form is built, or deprecated first?

**Blocked by it:** nothing yet, and that is the point — it decides how the
migration is shaped, and the migration has not started.

[ADR-082](specification/adr/adr-082.md) D1 replaces `dsl Json from input` with
`Json.value(input)`, and its §5 named the order: D1 and D2 together, *"then the
old form is removed rather than deprecated"*. The reason it gave was

> no program in the tree writes it and the free moment to take a form back is
> while that is true

**That fact has stopped being true, and the record now says so.** Eight programs
write the old form on nine lines — `1brc`, `access-log`, `calc`, `config`,
`inventory/stock`, `json`, `k-nucleotide`, `report` — plus a line in Part II 10.2
and three test files. Counted while
[ADR-091](specification/adr/adr-091.md) ran a new refusal across the corpus,
which is the only reason anybody looked.

**Why it is a question for the owner and not work.** The two answers cost
different things and neither is obviously right:

* **Remove, and migrate the eight in the same change.** One form in the language
  at every moment, and nothing to take back later. The cost is that the change is
  not separable: a mistake in the migration and a mistake in the new form arrive
  together, and every one of those eight is a program somebody reads to learn
  the language.
* **Deprecate, migrate, then remove.** Each half can be reviewed and reverted on
  its own, and the examples move one at a time under a compiler that still
  accepts both. The cost is a window in which the language has two spellings for
  one thing — which is exactly the state
  [ADR-082](specification/adr/adr-082.md) §1 calls the defect: *"one keyword, two
  namespaces"*, and a reader cannot tell which one this repository means.

**What does not change either way**, so it is not part of the question: the
entry call needs a `throws` in its contract. Today the checker is told in one
line that a `dsl … from …` can fail
([ADR-091](specification/adr/adr-091.md) D4), because it is not a call and has
no contract to say so. As a call it is answered from one, and a generated entry
rule carrying no `throws` meets `NK1134` at all eight of those programs — each
of which writes `catch` beside the entry. That is work, and it is on
[`open-work.md`](open-work.md) with the entry.

**What this is not.** It is not D1 being reopened. A grammar is entered by a
call; the question is only whether the old spelling stops working on the same
day the new one starts.

---

## 4. How does a package receive a handler?

**Blocked by it:** every library written *in Nikaia* that is handed a piece of
the caller's code — a route handler, a callback, a comparator. The first one is
already in the tree. [`examples/http/`](../examples/http/) is a package with a
request, a response and HTTP/1.1's text half, and **no server**, because the
line a server exists for cannot be written against it:

```nika
http::Server::new()
    .route("/fortunes") fn { fortunes(db) }
    .listen(":8080")
```

That is [`examples/fortunes.nika`](../examples/fortunes.nika)'s `main`, and what
stops it there is not the socket. **A socket layer would leave this exactly where
it is** — *there is no HTTP server* on [`open-work.md`](open-work.md) is the
other half and a separate thing. What is missing here is a way to *say* what
`route` takes.

### Two doors, and neither one opens from a package

**The lambda door is closed to everyone but `std`.** A parameter cannot be
declared as a function — the type rule has a reference, a name, generic
arguments and a `?`, and nothing else:

```
fn apply(f: fn() -> String) -> String { … }

Parse error: expected `&`; found unexpected token `fn` at line 1, column 13
in type_name / in type_ref / in fn_arg_def
```

Meanwhile lambdas are passed all over the corpus — `rows.sort_by_key fn(row) {
-row.1 }`, `names.map fn(path) { … }`, `par_fold(…, fn(acc, m) { … }, …)`. Those
work because a `std` signature is written in the ledger and never goes through
that rule: eight entries spell a parameter `f: fn(…)`. **So the door exists and
only `std` may walk through it.**

**The trait door opens — and then closes at the package boundary.**
[ADR-078](specification/adr/adr-078.md) built a `trait` and a bound, and a
handler stored in a struct works today, measured in one file:

```nika
trait Handler { fn handle(&self) -> Response }
struct Router[H: Handler] { only: H }
fn dispatch[H: Handler](r: &Router[H]) -> String { return r.only.handle() }
```

That compiles and runs. Put the trait in a package and three things stop it,
each reproduced against `examples/http/` with a `Handler` added:

| | what happens |
| :--- | :--- |
| `fn dispatch[H: http::Handler](…)` | parse error — a bound is one name, so a bound cannot reach into a package at all |
| `fn dispatch[H: Handler](…)` after `use http` | `NK1126`: *nothing says it has a method `handle`* — `use` brings nothing in ([ADR-046](specification/adr/adr-046.md)) and a bound has no way to say where the trait is |
| `impl http::Handler for Fixed` whose body calls the package | `NK1129`: *`Fixed::handle` can pause* — for a body the package's own ledger records as `sync = "inferred"`, and the identical shape in one file compiles |

The third is a defect and is on [`open-work.md`](open-work.md) with its
reproduction, as is the untranslated `rustc` message a fourth attempt produced
when the pausing refusal was stepped around. **The first two are this question.**

### The part that is a decision and not a defect

A parameter that is code makes all four of a caller's derived answers depend on
what is *passed in*. The ledger has met that once, unevenly:

| | today | |
| :--- | :--- | :--- |
| `sync` | `sync = "from(f)"` — the lambda decides | [ADR-029](specification/adr/adr-029.md) D3 |
| `touches` | deliberately **no** `from(f)`; a higher-order entry must *add* what the lambda reaches rather than assume it away | same record, and the ledger's own comment says why |
| `throws` | never asked — no entry has a `from` form | — |
| `sharing` | never asked | — |

**And the one mechanism that exists does not reach a server.** `from` is for an
**immediate** lambda, one the callee runs before it returns, and
[ADR-029](specification/adr/adr-029.md) D4 says so with a test that enforces it.
A router **stores** the handler and calls it later from a request loop — the
detached case that record names as `from`'s limit, because the lambda's calls
belong to nobody the caller is counting. A server is therefore not `sort_by_key`
one level up; it is precisely the case the existing answer excluded. The trait
door has the mirror-image version of the same gap:
[ADR-078](specification/adr/adr-078.md) D4 makes every trait method `sync`
because a declaration has no body to read, so a handler that genuinely pauses —
which is what a handler that reads a database *is* — cannot be declared at all.

**So whichever door is opened, the same thing has to be decided: what a
signature says about code it is handed.** That is why this is one question and
not two.

### The options

* **(a) Open the trait door across a package.** A bound gets a qualified name,
  and a package publishes its traits. Smaller than it sounds: the `impl` side
  already resolves `http::Handler` across the boundary — that is how `NK1129`
  found it — so what is missing is the bound's lookup and the ledger carrying
  `traits`, which [ADR-078](specification/adr/adr-078.md) §4 already names as
  undecided. Does **not** answer the pausing handler.
* **(b) Open the lambda door: a function type that carries the columns.**
  `fn() -> String` becomes sayable, and what it says includes whether the
  handler may pause, may throw, and what it may touch. The only option under
  which `route` can write down what it demands and a caller be checked against
  it.
* **(c) A function type that carries nothing.** The grammar grows, the columns
  do not, and anything holding a handler is treated as though it may do
  anything — the fail-closed direction the compiler already uses when nobody
  said. Cheap, and it makes every higher-order function in Nikaia as
  unconstrained as the worst handler anybody might pass.
* **(d) Register instead of pass.** A handler is a top-level function with a
  marking above it and the compiler collects the table while the program is
  built. No function type, no bound. Loses the capture: `fortunes(db)` closes
  over a connection, and a free function cannot reach one — though a struct
  field can, which is why this is weaker than (a) rather than different from it.
* **(e) The server goes back into `std`.** Works immediately, because `std` may
  already spell the parameter. Takes back [ADR-069](specification/adr/adr-069.md),
  which moved `http` out so it could ripen at its own speed.

**What I would do: (a) first, then (b).** (a) is the cheaper half of a door that
is already built and already half-open, and it makes `examples/http/` able to
hold a `Handler` that somebody outside it can implement — which is the smallest
thing that turns the package from a data type into a library. It is not enough:
a stored handler that pauses still cannot be declared, under either door. (b) is
where that gets answered, and it is worth doing second rather than first because
(a) will show what the columns actually need to say. (c) spends the grammar and
keeps none of what the grammar was for. (d) buys less than (a) for comparable
work. (e) trades a decision already taken for a shortcut.

**What it costs.** (a) costs a name space question this project has been
deferring — what a package publishes, and how a name reaches into one — which is
§1's neighbour and should be read beside it. (b) costs a type language where a
type says more than a name, the first time the surface language has needed that,
and a second answer to the detached question. Both cost a refusal a user will
meet: a handler that pauses, passed where the signature said it may not — and
that refusal is the whole point, because today the same program is refused by
`rustc` instead, about a file nobody wrote.

**What it is not.** It is not a missing feature of the ledger. Eight `std`
entries take a lambda and the corpus calls them; nothing about `std` is blocked.
What is blocked is a second author writing the same kind of function, which is
exactly what a package is for.

---

## 5. Is text one type whose state the compiler picks, or two the program picks between?

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
