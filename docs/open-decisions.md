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

### Is `nikaia describe` written in Nikaia?

**What is blocked.** [ADR-193](specification/adr/adr-193.md) D4's second step —
*a real parser (`syn`) replaces the scraper* — and with it
[`open-work.md`](open-work.md) §2.44's step 3. If the command is a **Nikaia**
program, the parser is a **grammar** and `syn` is not the answer.

**Two things are called a parser here and they have nothing in common but the
word**, which is the distinction the owner drew and it is the whole of why this
is a separate question:

| | the HTTP/1.1 parser | `describe`'s Rust-signature parser |
| :--- | :--- | :--- |
| runs | per connection, in a server's hot path | once per crate, when a person types a command |
| who waits for it | every request | one reader, once |
| the compiler needs it | to serve | **never** — `NK2504` names the command and a person runs it |
| decided | Rust for the MVP ([ADR-194](specification/adr/adr-194.md) D5) | **this question** |

**Why it is a good candidate, measured rather than felt.**

* **It is a separate command, so nothing bootstraps through it.** The compiler
  compiles without it; it writes a file a person then reviews
  ([ADR-104](specification/adr/adr-104.md) D5).
* **The way it would ship already exists.**
  [ADR-002](specification/adr/adr-002.md) D4 pre-lowers `std`'s Nikaia half —
  *the `.nika` kept beside the `.rs`* — so a binary install compiles the `.rs`
  and needs no compiler to do it. `nikaia lower-std` is the release step, and a
  Nikaia `describe` would ride the same one. There is no chicken and egg.
* **The half that would become a grammar is the half that is hand-written char
  scanning today.** `describe.rs` is 843 lines, and `split_top_level`,
  `items_of`, `fields_at`, `line_starts`, `signature_at` and `matching` are a
  scanner written by hand — which is precisely what the module header apologises
  for: *a **signature scraper** and not a Rust parser*.
* **It is the language's headline feature pointed at a real workload nobody
  designed the language around.** `json.nika`, `config.nika` and
  `access-log.nika` are the same shape one domain over.

**What it would cost, against `std`'s actual surface.** `std` has `fs::map`,
`fs::read`, `fs::read_to_string`, `fs::write` and `cli::args`. It has **no
directory walk** — and reading a crate means reading its `.rs` files
(`collect_rust` today) — and **no subprocess**, which
[ADR-193](specification/adr/adr-193.md) D4's first step wants for
`cargo metadata`.

Both are operating-system resources of exactly the kind
[ADR-194](specification/adr/adr-194.md) D1 just put the socket in `std` for, so
[ADR-069](specification/adr/adr-069.md) D2's subtraction covers them: they are
**work, not a boundary problem**. But they are two pieces of new `std` surface
that this question would be the reason for.

**The options.**

* **A — Nikaia, with a grammar.** `describe` becomes a `.nika` program in the
  sysroot, pre-lowered like `std`'s half. `std` gains a directory walk and a
  subprocess. [ADR-193](specification/adr/adr-193.md) D4's `syn` step is
  replaced by a grammar, and D4's other steps are unchanged.
* **B — Rust, with `syn`**, as [ADR-193](specification/adr/adr-193.md) D4 says
  today. No new `std` surface, no self-hosting, and the tool stays in the
  compiler's own tree.
* **C — Nikaia, but later.** Build `syn` now because §2.44 needs a describer
  that works, and rewrite it in Nikaia once `std` has the two pieces. Costs the
  work twice.

**What this page recommends: A, and it recommends asking one thing first.**

The argument for A is not self-hosting for its own sake. It is that **the
hand-written scanner is the thing that made every one of `describe`'s three
named limits**, and a grammar removes the class rather than the instance —
which is what this project prefers everywhere else
([ADR-117](specification/adr/adr-117.md) D2's reserved list over a cut, and
[ADR-048](specification/adr/adr-048.md) D1's trait over an emitter case are the
same move). `syn` would also remove it, and would do it in a tree that is
already Rust — that is B's real strength and it should not be waved away.

**The thing to ask first is whether a Nikaia grammar can do it at all**, and
that is answerable by writing one: a grammar over `pub fn`, `pub struct` and
`impl` headers, run against the crates `examples/foreign-runtime/` already has,
against the scraper's own output. **That is an afternoon and it settles the
question with a measurement instead of a preference** — the same route
[`rc-or-arc.md`](rc-or-arc.md) took for `Rc` against `Arc`, and
[`lock-free.md`](lock-free.md) for the compare-and-swap loop.

**What it costs if wrong.** A's cost is that every bug in the language becomes a
bug in the tool that stands between a program and a foreign crate — and
`describe` is security-adjacent, which is the whole of
[ADR-193](specification/adr/adr-193.md) D3's asymmetry. That its output is
**reviewed by a person** ([ADR-104](specification/adr/adr-104.md) D5) is what
makes the risk bearable, and it is the reason this is a better first
self-hosted tool than one whose output nobody reads. B's cost is that the
scraper's limits are traded for a dependency and the language keeps having no
tool written in it — which is not a technical cost and is the one that
compounds.

**Nothing else is open.** Two questions were answered in one day and both left this
file for their record, which is what this page says happens to an answered
entry: *does a described foreign function say whether it puts its argument on a
thread?* → [ADR-193](specification/adr/adr-193.md), and *what does a Nikaia
program have to write to be a microservice?* →
[ADR-194](specification/adr/adr-194.md). The work they created is
[`open-work.md`](open-work.md)'s.

**One question is deferred rather than answered**, and it is written here so it
is not mistaken for settled: **may a route be refused by what its handler
`touches`?** [ADR-194](specification/adr/adr-194.md) §4 carries it, and the
owner has asked to be asked again — with a fuller write-up than the one sentence
it has — **when the work reaches it**. It is not in the shape this page asks for
yet, and putting it in that shape before there is a server to refuse anything is
the kind of premature question this page is worse for holding.

*That is a statement about what has been **asked**, not about what is settled.*
Every entry this page has held was put here because something was blocked by it
and somebody noticed; an empty page means nothing is blocked that anyone has
written down. The way to refill it is the head of this file: the moment a piece
of work is blocked by a question, the question comes here in the shape above.
