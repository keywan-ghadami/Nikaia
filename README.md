<div align="center">
  <img src="1768075880760.jpg" alt="Nikaia Logo" width="300" />
  <h1>N I K A I A</h1>
  <p><strong>Good wins.</strong></p>

  <p>
    <a href="#-why-this-exists">Why</a> •
    <a href="#-what-nikaia-does-differently">What's different</a> •
    <a href="#-a-language-for-the-age-of-generated-code">AI-generated code</a> •
    <a href="#-what-its-good-at--and-what-it-isnt">Good for</a> •
    <a href="#-how-it-compares">Comparison</a> •
    <a href="#-two-profiles-one-language">Profiles</a> •
    <a href="#-code-example">Example</a> •
    <a href="#-where-the-project-actually-stands">Status</a> •
    <a href="docs/specification">Specification</a> •
    <a href="https://gemini.google.com/gem/1T8viw7ZHA0TwDZDhr6h1mgRBVnw3aTNP?usp=sharing">Gemini explains Nikaia</a>
  </p>

  ![Version](https://img.shields.io/badge/version-0.0.7-blue.svg)
  ![Status](https://img.shields.io/badge/status-specification_+_bootstrap-orange.svg)
  ![License](https://img.shields.io/badge/license-Apache_2.0-blue.svg)
</div>

---

## ⚡ Why this exists

Writing fast, correct software today means paying a tax that has nothing to do with your
problem.

You want to read a file and answer a request. Instead you decide whether the function is
`async` or not — and that decision infects every caller. You want to keep a name that points
into a buffer you already have. Instead you write lifetime annotations, or you allocate a
copy you didn't need. You want a value two tasks can see. Instead you pick `Rc` or `Arc`
*by hand*, and if you pick wrong the code stops compiling three modules away. You take two
locks and hope everyone else in the codebase takes them in the same order.

None of that is your program. All of it is bookkeeping — and bookkeeping is exactly the kind
of work a compiler is good at.

**Nikaia is a bet that the whole tax is compiler work.** You write straight-line code that
says what should happen. The compiler decides the concurrency model, the pointer types, the
lock order, the lifetimes, and the state machines. Not by guessing, and not by adding a
garbage collector — by inference over a program it can see all of.

### Why now

The bet would have been a harder sell fifteen years ago, because for fifteen years nobody had
to take it. Efficiency in software has gone through three eras:

1. **Constraint.** Small machines and small budgets meant you had to understand the machine —
   memory layout, I/O cycles, what the cache actually does. Every line had physical weight.
2. **Subsidy.** Cheap cloud capacity and cheap capital turned horizontal scaling into the
   answer to every performance question. Hardware quietly paid the bill for inefficient
   software, and the bill compounds: a home computer of the constraint era ran its operating
   system, an editor and a game inside 64 KB, while a chat window today asks for a few hundred
   megabytes — four orders of magnitude to show a list of messages. A CRUD service came to need
   an orchestrator to stay up.
3. **The wall.** Training and inference now compete for the same power, silicon and datacenter
   capacity as everything else, and competition prices things. "Throw more servers at it" is no
   longer the cheap answer to a design problem — and in some regions there are no more servers
   to throw.

Nikaia is a synthesis, not a rollback to era one. You keep the ergonomics the subsidy bought —
no manual thread management, no callback hell, no lifetime bookkeeping — and the compiler pays
for them **once, at build time**, instead of the runtime billing you for them **per request,
forever**. That trade is the entire economic argument for the project.

The same technology is behind both halves of it: AI is why a growing share of code is no longer
typed by a human — and a generator needs *guarantees*, not comfort — and it is also why the
compute that code runs on stopped being cheap. That gets
[its own section](#-a-language-for-the-age-of-generated-code) below.

The language is named after my daughter **Nika**. Her name carries more than one lineage, and
the deeper one is not Greek: in Persian, *nik* means good — virtuous, good the way a person is
good and not the way a product is. The motto is just the name with a verb: **good wins**.

Not over anyone. There is no war here between people and their tools, and least of all one
where the human beats the machine. It says the thing this project refuses to accept: that good
and fast are opposites, and that being decent to the person writing the code has to be paid for
at runtime. Here it is the other way round. Because the language never makes you write `Arc`,
or a lifetime, or a lock order, those decisions belong to the compiler — and only because they
belong to it can it pick `Rc` under Lite and `Arc` under Advanced, order the locks, and infer
borrow contracts across a whole program. A stricter language would have to take your word for
it instead. Good is not the price of fast here; it is the reason for it.

Fittingly, the ancient city the name also points to is remembered for a council, where a schism
ended in consensus rather than in one side defeating the other. The rest of the story is in the
[**Manifesto**](manifesto.md).

---

## 💎 What Nikaia does differently

Two profiles are the *packaging*. These are the actual claims — each one is specified, and
each one links to the decision record that argues it:

**1. Functions have no colour.**
There is no `async` and no `await`. Any function may pause on I/O; the compiler builds the
state machine. This is not "async made easier" — it deletes the split of a language's
ecosystem into a sync half and an async half, where every library has to exist twice.
→ [Spec Part I](docs/specification/10-nikaia-light.md)

**2. Ownership without lifetime annotations.**
Nikaia compiles through the Rust toolchain, so it inherits the borrow checker's guarantees —
but you never write `'a`. Borrow contracts are inferred whole-program and written to an
auditable ledger; the cases that are genuinely inexpressible in safe Rust (a slice stored in
a struct, a task borrowing from its parent) get real language constructs instead of a lecture.
→ [ADR-005](docs/specification/adr/adr-005.md), [ADR-008](docs/specification/adr/adr-008.md)

**3. One source, two runtimes.**
You write `Shared[T]`. Under the **Lite** profile it compiles to `Rc` and a single-threaded
event loop; under **Advanced** it becomes `Arc` and a work-stealing thread pool. The same
holds for `spawn`. Your source file does not encode the deployment decision, so changing it
is a build flag, not a refactor.

**4. Deadlocks removed by construction.**
Multiple resources are requested together — `access_all(a, b)` — and the runtime always takes
them in address order. A deadlock cycle between two `access_all` callers is not unlikely; it
is unconstructible. The lambda must be `sync` (provably non-pausing), so a lock can never be
held across an I/O suspension point.
→ [Spec Part II](docs/specification/20-nikaia-advance.md)

**5. Grammars are part of the language, not a preprocessor.**
`dsl` is an expression. A scannerless grammar can be parsed at compile time *or* at runtime
with the same syntax, embedded DSLs get their own real syntax (SQL, JavaScript, x86) instead
of stringly-typed interpolation, and a grammar rule marked `@frame` can be folded in parallel
with `par_fold` — with the compiler *verifying* the resynchronization property rather than
trusting your word for it. This is why parsing benchmarks are a first-class target, not a demo.
→ [ADR-007](docs/specification/adr/adr-007.md), [ADR-009](docs/specification/adr/adr-009.md)

**6. The compiler tracks where your data came from.**
Trust is a property of the *source*, not of the profile: bytes off a socket are untrusted,
your own config file is not, and that provenance travels with the value. The compiler then
picks a DoS-resistant hasher exactly where it matters and a fast one everywhere else,
instead of making every program pay for the worst case — or, worse, making you remember.
→ [ADR-010](docs/specification/adr/adr-010.md)

**7. No garbage collector.**
Deterministic teardown via ownership and RAII, including under implicit async, where "when
does this file close" is otherwise a genuinely hard question.
→ [ADR-006](docs/specification/adr/adr-006.md)

### How is that even possible?

Nikaia is not a new backend. It lowers to a stable intermediate representation and then drives
`rustc` directly through its internal driver API, pinned to one exact nightly per release.
That is what makes points 1, 2 and 3 tractable: the hard safety machinery already exists and
is battle-tested — Nikaia's job is to stop making humans operate it by hand.
→ [ADR-001](docs/specification/adr/adr-001.md), [Toolchain architecture](docs/toolchain_architecture.md)

---

## 🤖 A language for the age of generated code

There is an obvious objection to launching a systems language in 2026: if a model writes the
`if err != nil` chains and argues with the borrow checker on your behalf, who cares how
ergonomic the syntax is? "Nicer to type" is a shrinking argument.

The objection is correct, and it points straight at the stronger one. When a machine writes the
code, the question stops being *how pleasant is this to write* and becomes **what can the
compiler still prove about code that no human wrote?** Every claim above changes meaning under
that question:

* **Boilerplate costs context, not just keystrokes.** A model has a finite window and loses the
  thread as it fills. Ceremony — `async`/`await` plumbing, lifetime annotations, `Arc::clone`
  dances, error-propagation chains — is budget spent on machinery instead of on your problem.
  Nikaia is dense on purpose. What used to be a comfort argument for humans is now a
  *capability* argument for the generator.
* **Concurrency is where generated code fails silently.** Models are very good at plausible code
  and weak on the memory model. A hallucinated lock order or a shared mutable capture is not a
  compile error in Go or C++; it is a bug that appears in production, under load, once. In
  Nikaia a deadlock between `access_all` callers is unconstructible, a lock cannot be held
  across a suspension point, and data races are rejected at compile time under *both* profiles.
  The compiler is a merciless reviewer for exactly the class of defect human review is worst at.
* **Fewer decisions to get wrong.** `Rc` or `Arc`? The sync or the async variant of this API? Is
  this future `Send`? Each is a coin flip a generator can lose, and losing it surfaces as an
  error three modules away, in code the author has never read. In Nikaia these decisions are not
  in the source at all — the profile settles them at build time.

This is also the honest answer to a new language's chicken-and-egg problem. Nobody has to learn
Nikaia to get something out of it: hand a model the specification and your requirements, and let
a compiler that rejects deadlocks, races and leaks decide whether what comes back is sound. A
generated program that runs fast and refuses to race is a better first contact with a language
than a tutorial is.

**The honest caveat:** no model has Nikaia in its training data, so generating it means putting
the specification in context. That is a hard constraint on this repository, not an afterthought
— the spec is written to be precise and small enough to fit, and a
[ready-made prompt bundle](#roadmap-to-010) is on the roadmap. Until then, `docs/specification/`
plus `examples/` is the bundle.

---

## 🎯 What it's good at — and what it isn't

**Use Nikaia when:**

* You are writing **I/O-dense services** — HTTP APIs, proxies, edge workers, CLI tools that
  talk to the network — and you want Go-like ergonomics with no GC pauses and no `async`
  colouring.
* You are chewing through **data in a custom or semi-structured format** — logs, telemetry,
  columnar dumps, protocol frames. The grammar protocol plus zero-copy tethered slices is
  the part of Nikaia that is most obviously not just "Rust with fewer keystrokes".
* You need **CPU-bound throughput** — simulation, image and signal processing, aggregation
  over huge inputs — and you would like the parallelism to be checked, not hoped for.
* One team owns both halves and is tired of maintaining **a fast core in one language and a
  service layer in another**, with a serialization boundary in the middle.
* You intend to **generate most of the code anyway** and would rather have the guarantees
  checked by a compiler than by a code review — see the [section above](#-a-language-for-the-age-of-generated-code).
* **Your compute bill is a line item somebody looks at** — density and no GC mean the machine
  you already pay for does more, instead of the cluster growing to cover the runtime's habits.

**Don't use Nikaia when:**

* You need something you can ship next quarter. Read the [status](#-where-the-project-actually-stands)
  section: today this is a specification and a bootstrap compiler.
* You want a REPL-first scripting language, a mature package ecosystem, or ten years of
  Stack Overflow answers. Python and Rust respectively are better at being Python and Rust.
* Your bottleneck is a **runtime**, not a library. A C-ABI dependency is fine in principle —
  Chapter 15 specifies `extern "C"` with `unsafe` at the boundary — but PyTorch, Spark or a
  vendor's JVM SDK are not libraries you link against, they are runtimes you would have to host.
  Embedding one means carrying its GIL or its garbage collector inside your process, which is
  usually the thing you came here to avoid. Nikaia does not dissolve that trade; it only changes
  where you make it.

---

## 📊 How it compares

| | Python / TS | Go | Rust | **Nikaia** |
| :--- | :--- | :--- | :--- | :--- |
| Async in the type system | colours functions | invisible (goroutines) | colours functions | **invisible** |
| Memory management | GC | GC | ownership, manual annotations | **ownership, inferred** |
| Pause times | GC pauses | GC pauses | none | **none** |
| Thread-safe vs single-thread types | n/a | n/a | you choose `Rc`/`Arc` | **one type, profile decides** |
| Data-race protection | none / GIL | detector at runtime | compile time | **compile time** |
| Deadlock protection | none | none | none | **`access_all` ordering** |
| Embedded DSLs | strings / metaprogramming | none | proc macros over Rust tokens | **first-class scannerless grammars** |
| Single-core / multi-core target | one runtime | one runtime | you build it | **build-time profile** |
| Maturity | ✅ production | ✅ production | ✅ production | ⚠️ **specification** |

The last row is the honest one, and it is the only one where Nikaia loses on purpose.

---

## 🎛 Two profiles, one language

Nikaia adapts to your problem, not the other way around. The profile is a line in your
`nikaia.toml`, not a rewrite:

### 🟢 Nikaia Lite (The I/O Engine)
* **Alternative to:** Node.js, Go, WASM runtimes.
* **Architecture:** single-threaded event loop.
* **Benefit:** race conditions impossible by design, maximum I/O density, minimal footprint.
* **Use case:** microservices, web servers, CLI tools, edge workers.

### 🔵 Nikaia Advanced (The Compute Engine)
* **Alternative to:** Rust, C++.
* **Architecture:** multi-threaded work-stealing runtime.
* **Benefit:** every core busy, thread safety proven by the borrow checker.
* **Use case:** HPC, game engines, heavy backend systems.

Because the profile difference lives in the compiler and not in your source, a library written
under Lite is checked against Advanced's rules too — you cannot accidentally ship something
that only works single-threaded.

---

## 💻 Code example

A HTTP server. Note what is *absent*: no `async`, no `await`, no `.unwrap()`, no lifetimes,
no `Arc::clone`.

```nika
use std::http
use std::fs

// `throws` replaces Result-Unwrapping. Errors bubble up automatically.
fn handle_request(req: http::Request) throws IoError {
    // Looks synchronous, but is non-blocking I/O (Suspension Point)
    let data = fs::read_string("index.html")
    return req.respond(200, data)
}

fn main() {
    println("Starting Nikaia Server on :8080")

    // `spawn` behaves polymorphically:
    // - Lite: Green Thread on Main Loop
    // - Advanced: Task on Thread Pool
    // Syntax: Uses 'fn' block for lambdas (no '||')
    spawn fn {
        http::Server::new()
            .route("/", handle_request)
            .listen(":8080")
    }

    // No `await` needed. The process stays alive.
}
```

Bigger, more revealing programs live in [`examples/`](examples/): the One Billion Row
Challenge, a four-function calculator, a web access log summarised, and the TechEmpower
`fortunes` benchmark. **The first three compile, run, and are checked by `cargo test` under
both profiles**, with their output required to be identical; `fortunes` is still written at
specification level. They are there because writing a real program against a spec is the
cheapest way to find out what the spec forgot, and
[`examples/README.md`](examples/README.md) lists exactly which gaps each one exposed and
which are still open. `tests/samples/` holds the smaller programs the bootstrap compiler can
already parse.

---

## 🚦 Where the project actually stands

Nikaia is an experiment conducted in the open, and the specification is far ahead of the
compiler. Concretely:

* ✅ **Specification 0.0.7** — syntax, profiles, unified types, borrow model, cleanup
  semantics, grammar protocol. Ten ADRs recording *why*, including the ones that reverse an
  earlier decision.
* 🚧 **0.0.8 (unreleased)** — tethered slices in user structs, parallel parsing, input
  provenance. See the [CHANGELOG](CHANGELOG.md).
* 🚧 **Bootstrap compiler (Stage 0)** — a Rust front-end that lowers to a Bridge IR and drives
  `rustc` to produce a binary. It handles functions and methods, `impl`, `struct` and `use`,
  control flow, `throws`/`catch`/`??`, string interpolation — and the whole `grammar` construct,
  `@frame` and `dsl … from …` included. **`examples/1brc.nika` compiles, runs and is a test.**
  A type checker is what it does not have.
* ❌ **Not yet** — a type checker, the runtime binding to `tokio`, the LSP, self-hosting. The
  standard library exists in the narrow sense the examples need, and one of its files is
  already written in Nikaia.

Full detail: [project status & roadmap](docs/project_status_and_roadmap.md).

### Roadmap to 0.1.0

- [x] **Spec 0.0.5:** syntax, profiles, unified types.
- [x] **Spec 0.0.6:** borrow model without lifetime annotations; cleanup under implicit async.
- [x] **Spec 0.0.7:** scannerless grammar protocol, DSLs as expressions, hardware instructions as libraries.
- [x] **Manifesto:** the soul and philosophy of the project.
- [ ] **Bootstrap compiler:** the transpiler in Rust (Stage 0).
  - [x] The grammar protocol: `grammar` onto `grammar!`, `@frame` onto `#[frame]`, and
    `dsl … from …` onto the parallel piece driver, with the profile choosing the parallelism
    ([ADR-011](docs/specification/adr/adr-011.md)).
  - [x] Diagnostics on the `.nika` line that caused them, for every error class at once
    ([ADR-012](docs/specification/adr/adr-012.md)).
  - [x] `impl` blocks and methods, `throws`/`catch`/`??`, string interpolation, and a `std` for
    what the examples call ([ADR-013](docs/specification/adr/adr-013.md)) — enough that
    **`examples/1brc.nika` compiles and runs**.
  - [x] `fs::map` is a memory mapping and the parallel driver runs on every core
    ([ADR-014](docs/specification/adr/adr-014.md)): 8M lines, 4 cores, 0.52 s → 0.14 s.
  - [x] The parser backend's lazy diagnostics, and the measurement they made possible
    ([ADR-015](docs/specification/adr/adr-015.md)): **659 instructions per row against 688 for
    the same aggregation hand-tuned in Rust**, and 840 for it written naively — a generated
    parser below hand-written code on the workload the spec picked to be judged by.
  - [x] `fs::map`'s UTF-8 check divided across the cores rather than skipped
    ([ADR-016](docs/specification/adr/adr-016.md)): 3.9× on the check, and what is left to gain
    by removing it altogether is 10 ms of a 140 ms program.
  - [ ] A type checker: everything Stage 0 cannot infer, the example has to say (ADR-013 D7).
- [ ] **Runtime integration:** binding `tokio` (current-thread & thread-pool).
- [ ] **Interop:** `extern "C"` in the compiler (Chapter 15 specifies it) and Python bindings —
      so a Nikaia core can be dropped into an existing stack as a hot loop, without anyone
      having to migrate a codebase to find out whether it is worth it.
- [ ] **Prompt bundle:** a single-file specification digest for Claude Projects, Copilot
      instructions and system prompts, so a model can write correct Nikaia from context.
- [ ] **Self-hosting:** the compiler compiles itself.
  - [x] The first `.nika` file the toolchain runs on: `crates/nikaia-std/src/text.nika`,
    compiled into `std` by Stage 0 when `std` is built
    ([ADR-014](docs/specification/adr/adr-014.md) D1). What a `std` file may contain is exactly
    what the compiler can lower; the share grows as it does.

### Where to start reading

| If you want to… | Read |
| :--- | :--- |
| know why the project exists at all | [manifesto.md](manifesto.md) |
| learn the language | [Spec Part I](docs/specification/10-nikaia-light.md) |
| see the concurrency and parallelism model | [Spec Part II](docs/specification/20-nikaia-advance.md) |
| understand a design decision | [the ADRs](docs/specification/adr) |
| see real programs | [examples/](examples/) |
| let a model write Nikaia for you | [the spec](docs/specification) + [examples/](examples/) in one context |
| work on the compiler | [toolchain architecture](docs/toolchain_architecture.md) |

Contributions, objections and "this cannot work because…" arguments are all welcome —
especially the third kind. An unimplemented specification is the cheapest possible moment to
be told you are wrong.

---

## ❤️ Dedication — for Nika

**Nika is my daughter, and I love her more than anything I will ever build.**

Everything in this repository — the specification, the compiler, every argument above — sits
downstream of one simple wish: that the world she grows into has fewer walls in it than the one
I found. The language carries her name because she is the reason it exists at all.

*Because the future belongs to those who build it.*

We are already standing in tomorrow's past: everything that will be ordinary in twenty years is
being decided right now. Everyone decides, and everyone prioritises — and it does not have to be
the whole machine. One gear, turning, is enough to move the ones around it.

That is one half of how I think about Nikaia. It is not mainstream. It takes a different route
and argues with settled consensus in several places. That is the part of me swimming against the
current: one person deciding to turn, and accepting that turning alone is slow.

The other half is knowing I am made of the river. Nikaia was written with the help of AI, which
stands on the accumulated knowledge of everyone who ever wrote a compiler, a paper, a textbook,
a Stack Overflow answer. It rests on Rust's borrow checker, on decades of runtime research, on a
language nobody in this repository invented. It was built with money, electricity and machines
that the flow provided. Not one idea in it is uncaused. Nothing here was taken from nowhere.

It also cost less than it should have. Without AI, this project would have lost the
prioritisation to the people I love — and rightly so. What AI changed is the price: it made
things move fast enough that Nikaia could exist without taking its time out of theirs. Money,
yes. Evenings, far fewer than it would have cost a few years ago.

Both are true at once, and the contradiction is not a flaw in the story — it is the story.
Swimming against the stream is still swimming in it. This is a solo effort that only makes sense
as a shared work: it exists to show what a single person can still put into the world, and in
the same breath to admit how much of that was already lying there, done by others, waiting to be
picked up.

So: to everyone whose work is upstream of this one — the named and the uncounted, the people who
built the tools and the people who wrote down why — thank you. Most of you never heard of this
project. And to whoever is downstream, who will take this further or take it apart and build
something better: this was always meant to end up in your hands.

And to Nika: whatever becomes of it, I would rather hand you a world shaped a little less by
walls and a little more by the freedom to build. ❤️

---

## ⚖️ Intellectual property & governance

Nikaia introduces the **Unified Core Architecture**, a novel approach to compile-time
orchestration, deterministic concurrency (`access_all`), and profile-based runtime
transformation.

**For corporate entities & implementers:**
This repository establishes public **prior art** for these architectural concepts, ensuring
they remain unencumbered for the open-source ecosystem.

While Nikaia is released under the **Apache 2.0 License**, the project is designed with
long-term governance in mind to prevent proprietary fragmentation. We actively invite
organizations interested in adopting, extending, or standardizing these concepts to join us as
**Founding Partners** rather than attempting parallel implementations.

The architecture is complex; let's build the standard together.

---

## 📄 License

This project is licensed under the Apache License, Version 2.0. See the [LICENSE](LICENSE) file for details.

<div align="center">
  <sub>For Nika. For everyone downstream.</sub>
</div>
