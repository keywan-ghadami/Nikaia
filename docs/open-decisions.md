# Open decisions — the questions that need the owner

## The shape this page is for
The
entries here are written to be *answerable* — what is blocked, the options, a
recommendation, and what either direction costs if it is wrong.

An answer is an [ADR](specification/adr/), and
the moment a question is answered its entry leaves this file rather than
staying with a note on it. What is merely **unbuilt** is in
[`open-work.md`](open-work.md) — an ADR said what happens and the compiler does
not do it yet, which needs work and not a ruling. Each entry says what the
question is, why it is the owner's, and what this file recommends.

## Open

### What does a Nikaia program have to write to be a microservice, and who decides what is exposed?

**What is blocked.** Nothing yet — this is a direction rather than a repair, and
it is here because the shape has a fork the compiler cannot pick. It sits in
front of [`open-work.md`](open-work.md) §2.6's server: what that entry builds is
a socket layer and an HTTP/1.1 server, and this decides what a person writes on
top of them.

**What is asked for.** The ergonomics of `python -m http.server`: **one command
line, no ceremony, functionality reachable over HTTP in a minute.** Not the
file-serving — the *zero setup*.

**What already exists, which is more than the entry suggests.**

* **The handler is decided.** [ADR-018](specification/adr/adr-018.md) D1 gives it
  the request as an implicit first argument under a rule the language already
  has; D2 says what a return value becomes (a `String` is 200 `text/plain`, an
  `html::Raw` is `text/html`, a `Response` is itself, and a `throws` failure is
  **500 with a generic body and the error in the log** — a decision, not an
  omission); D3 makes a status or a header an ordinary value; D4 makes the
  request's strings views into the connection's bytes.
* **The runtime is ready.** [ADR-121](specification/adr/adr-121.md) is built and
  its §3 names this exact need: ***`rt::io::wait` is awaitable, which is the
  first thing the HTTP server's socket layer needs.***
* **A code parameter is sayable.** `fn route(path: ref String, handler: fn(Request) -> Response)`
  parses and lowers ([ADR-102](specification/adr/adr-102.md) D1), and
  [ADR-192](specification/adr/adr-192.md) D1 decides its shape.
* **The ledger already knows the signature, `throws`, `sync` and `touches` of
  every `pub fn`** — which is what makes the tempting answer tempting.

**The fork: who decides what is on the network?**

* **A — every `pub fn` is a route.** Maximum zero-ceremony, and **this page
  recommends against it.** `pub` is a *package* word
  ([ADR-047](specification/adr/adr-047.md) D2): it means *a consumer of this
  package may call it*. Reading it as *the network may call it* gives one word
  two meanings and makes the dangerous one invisible at the declaration —
  a silent promotion, which is the polarity
  [ADR-010](specification/adr/adr-010.md) D1 exists for. `python -m http.server`
  is already a famous footgun for serving the working directory; this would be
  the same mistake over a call graph.
* **B — a word on the declaration.** `@route("/total") pub fn total(…)`.
  Explicit, greppable, and next to what it exposes — the shape `@borrowed` and
  `@frame` already have. It costs an attribute, and it puts the decision with
  the **author**.
* **C — the program routes, as [ADR-018](specification/adr/adr-018.md) writes
  it.** `.route("/x") fn { … }` in a `main`. No language change at all, and it
  is the ceremony the request is asking to be rid of — but it is what a program
  that wants control writes, and it should stay whatever else is chosen.
* **D — the operator names the routes**, on the command line and in the runtime
  configuration file: `nikaia serve --route /total=total`, or a `[routes]`
  table in `nikaia-runtime.toml`.

**Two of it are answered** (the owner, at 0.0.152), and what is left is
narrower than the options below suggest:

* **Where it binds: localhost.** Anything wider is asked for.
* **Routes: `.route(…)`, option C, for the MVP.** No `@route` yet. So the
  exposure decision is in the **source**, where nothing is implicit — which is
  the safest of the four and needs no language change at all. A and B are not
  the MVP, and D is a deployment concern that arrives with a deployment.

**And the shape is measured to work, end to end, before any of it is built.**
`crates/nikaia/tests/project.rs` runs [ADR-018](specification/adr/adr-018.md)'s
own chain across a package boundary — a type constructed through its package, a
method chain over it, a function-typed parameter
([ADR-102](specification/adr/adr-102.md) D1) and the `async` closure
[ADR-192](specification/adr/adr-192.md) D1 writes for it — with the socket the
only thing missing. Three more things came out of writing it:

* **`tiny::Server()` did not lower**, and that was a defect rather than a gap:
  the constructor rule asked the *library*'s ledger about a qualified name and
  never the package's. Fixed at 0.0.152.
* **[ADR-018](specification/adr/adr-018.md)'s own example is refused today.**
  It writes `http::Server::new()`, and `NK1149` answers *a type is constructed by
  its anonymous constructor* since [ADR-140](specification/adr/adr-140.md) D2.
  The record is not rewritten; whoever builds the server writes `http::Server()`.
* **The builder chain needs nothing new.** `self`, `ref self` and `ref mut self`
  are all receivers, so a consuming `fn route(self, …) -> Server` chains exactly
  as the record prints it.

**What this page recommended, for the day the MVP is not the question: D, with C
kept, and never A.**

The precedent is exact and it is this project's own.
[ADR-038](specification/adr/adr-038.md) D5 **moved `cleanup-deadline` out of the
manifest** into `nikaia-runtime.toml`, *read when the program starts by the
person running it*, on the ground that *how long a program waits at exit is an
operating property, and a build-time key cannot be tuned by the operator — who
is not the person who compiled it.* **What is exposed to a network is an
operating property in exactly that sense**, and more sharply: the person who
compiled a function and the person who decides it may be reached from outside
are routinely not the same person, and only the second one knows what the
network is.

So the command line is the fast path — one line, nothing in the source — and the
runtime file is the same answer written down for a deployment. **B stays
available** as a *default* route a function suggests, which an operator may
accept or override; it is not the thing that grants exposure.

**Three things D leaves to decide, and they are smaller:**

1. **How arguments arrive.** [ADR-018](specification/adr/adr-018.md) D2 decides
   the *result* and says nothing about the parameters. `pub fn total(entries: ref Vec[Entry]) -> i64`
   has to get its `entries` from somewhere — the query, the body as JSON, or
   only functions whose parameters are the request itself. The narrow start is
   the last: a route names a function whose signature is one
   [ADR-018](specification/adr/adr-018.md) D1 already describes, and everything
   else is refused with the signature it would need.
2. **What the ledger should refuse.** This is the thing no other language's
   `http.server` can do: the ledger knows what a function **touches**. A route
   onto a function that touches the filesystem is a different object from one
   that computes, and the command can say so — refuse, warn, or require a word.
   That is its own question and it is worth asking once D is settled.
3. **Where it binds.** `python -m http.server` binds `0.0.0.0` and that is half
   of why it is a footgun. Localhost unless told otherwise is the fail-closed
   default, and it costs one flag to leave.

**What it costs if wrong.** D's cost is a second place to look: a reader of the
source cannot see what is exposed, and has to read the deployment. That is real,
and it is the argument for B — which is why B is kept for the *suggestion* and
not for the grant. A's cost is the one that does not show up in a diff at all:
a function made `pub` for a sibling module becomes reachable from the internet,
and nothing in the file that made it `pub` says so.

**Nothing else is open.** The last question — *does a described foreign function say
whether it puts its argument on a thread?* — was answered at 0.0.149 and left
this file for [ADR-193](specification/adr/adr-193.md), which is what this page
says happens to an answered entry. The work it created is
[`open-work.md`](open-work.md)'s.

*That is a statement about what has been **asked**, not about what is settled.*
Every entry this page has held was put here because something was blocked by it
and somebody noticed; an empty page means nothing is blocked that anyone has
written down. The way to refill it is the head of this file: the moment a piece
of work is blocked by a question, the question comes here in the shape above.
