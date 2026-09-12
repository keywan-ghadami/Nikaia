# One way down: the nightly, Bridge-IR and the `rustc_ast` path, and why they went

**Date:** September 12, 2026
**Status:** history. Nothing here is normative and nothing depends on it to know
what a program means. The decisions that stand are
[ADR-001](specification/adr/adr-001.md) D1, [ADR-003](specification/adr/adr-003.md)
D1 and D2, and [ADR-004](specification/adr/adr-004.md) D1; the release entry is
[`../CHANGELOG.md`](../CHANGELOG.md).
**The measurements are elsewhere and are unrevised:**
[`nightly-cost.md`](nightly-cost.md), [`subprocess-cost.md`](subprocess-cost.md),
[`std-sysroot.md`](std-sysroot.md), [`technical_notes.md`](technical_notes.md).

The specification and the ADRs are written as though none of this existed, which
is what `docs/README.md` §2 requires of a **withdrawal** as against a
supersession. This page is where the route went. It is here so that a person who
finds `bridge` in a two-year-old commit message, or wonders why a language that
lowers to text ever needed a nightly, gets an answer instead of an archaeology
project.

---

## 1. What the three things were

They were one design in three parts, and they only make sense together.

**Bridge-IR** — `crates/bridge-ir`, 85 lines. A pure-data intermediate
representation: `BridgeModule`, `BridgeFunction`, `BridgeStruct`, `BridgeLetStmt`,
`BridgeExpr` with `Literal`, `Variable` and `Call`. No `NodeId`, no interned
strings, spans as `Range<usize>` byte offsets, `serde` on everything. It was the
**narrow waist**: a language frontend would target Bridge-IR and never a compiler
data structure, so a `rustc` change reached the executor and stopped there, and a
frontend could be tested by asserting on the IR it produced with no compiler in
the loop. A construct with no direct Rust spelling was to be lowered in the
*frontend* into ones that had, rather than widening the protocol.

**The `rustc_ast` path** — `crates/rustc-executor`. It took a `BridgeModule` and
built a `rustc_ast::Crate` in one pass, so that the mapping for a construct
existed in exactly one place and a syntactically invalid program was
unrepresentable — the types refuse it, where a string builder emits it and fails
later somewhere that does not name the cause. Text was then a *print* of that
crate (`rustc_ast_pretty::pprust`) rather than a second code generator, and a
`rustc` subprocess compiled the printed text. The exit that was kept open: hand
the same crate to `rustc_interface::run_compiler` and skip rustc's parser
altogether.

**The pinned nightly** — `bridge-toolchain/rust-toolchain.toml`, channel
`nightly-2026-01-01`, components `rustc-dev`, `llvm-tools-preview`, `rustfmt`,
`clippy`, installed and used through `scripts/bridge-toolchain.sh`. It existed
for one reason: `rustc_ast` lives behind `#![feature(rustc_private)]`, which is
nightly-only and carries no stability promise, so the crate that linked it had to
be tested against exactly one snapshot and shipped as a unit with it. A second CI
leg built the whole workspace on that channel, because a backend that is not
built against its pin is a backend that rots.

## 2. What was measured, when, and by whom it was measured for

Every number below was taken for a *different* question and is written up in the
notes named beside it. None was taken to justify the withdrawal.

| what | number | where |
| :--- | :--- | :--- |
| what Bridge-IR could carry | **1 of 15** corpus programs | [`subprocess-cost.md`](subprocess-cost.md) §5 |
| the printed-Rust round trip | **0.161 %** of a full compile (2.77 M of 1 719 M Ir) | [`subprocess-cost.md`](subprocess-cost.md) §4 |
| starting the `rustc` process | **0.48 %** (8.3 M Ir), two thirds of it the rustup shim | [`subprocess-cost.md`](subprocess-cost.md) §3–§4 |
| the two together — what the in-memory exit would buy | **0.64 %** | [`subprocess-cost.md`](subprocess-cost.md) §4 |
| `-Zpolonius=next` over the corpus | **0 verdicts changed** at **+7.11 %** of a build | [`nightly-cost.md`](nightly-cost.md) §3 |
| the pinned toolchain against a stable one | 1 420 MiB / 24.7 s against 602 MiB / 12.3 s | [`nightly-cost.md`](nightly-cost.md) §4 |
| what the nightly cost a *user* who only runs programs | **2 MiB** — and a non-relocatable binary | [`nightly-cost.md`](nightly-cost.md) §4 |
| the compiler in `std`'s build graph | 58 of 97 packages, 29 s for a hello-world | [`std-sysroot.md`](std-sysroot.md) |

Two of those deserve their own sentence.

**One of fifteen.** Bridge-IR had no control flow — no `if`, no `while`, no
`match`, not even a binary operator. `hello_world.nika` reached the backend;
`let_assignment.nika` was refused by name, because `println` with a variable
argument had nowhere to go in `BridgeExpr`; and every `examples/*.nika` lowers to
a `grammar! { }` macro invocation, which Bridge-IR had no node for. So the waist
was narrow in the wrong dimension. A narrow waist is a discipline — *this much,
and constructs without a spelling get desugared before they arrive*. An empty one
is not a discipline; it is an absence with a rule written on it.

**Not relocatable.** A binary built with the backend linked
`librustc_driver-<per-build hash>.so` by absolute RUNPATH into the build
machine's rustup home, and the hash in the soname differed per build, so a
different install had a differently named file and therefore the wrong one. The
same compiler built without the feature linked `libc`/`libgcc` and went wherever
it was put. This is the number that did the most work, and it is not a percentage.

## 3. The decision, and that no further measurement was taken

The owner withdrew all three **without a further measurement, knowingly.** That
is worth stating plainly, because this project's habit is the opposite — a
decision here usually waits for a number, and several of the records above exist
only because somebody refused to decide without one.

It was the right call anyway, for a reason the numbers themselves show: there was
no measurement left that could have changed the answer. Each of the four findings
answers a *different* question, and they agree.

* If the concern was **speed**, the exit was worth 0.64 %, and three quarters of
  even that was process start rather than the text round trip.
* If the concern was **reach**, the path compiled one program in fifteen.
* If the concern was the **borrow checker** — the one language capability the
  nightly could have bought — the flag changed zero verdicts and cost 7 % of
  every build.
* If the concern was **what a user installs**, the pin cost two megabytes and
  cost them a binary that only runs beside the toolchain that built it.

A measurement is worth taking when its outcome would change what you do. Nobody
could name one here. What a further round would have bought is confidence about a
decision already unanimous across four independent axes, at the price of the
weeks it takes to grow Bridge-IR far enough to be worth re-measuring — which is
itself the work being declined.

**What it costs to have decided this way, stated rather than hidden.** The
argument the withdrawn design made was never disproved: a narrow waist between
many languages and one backend is a good shape, and if Nikaia ever hosts a second
language and finds text a bad interface, this page is the record of a design that
was half-built and what it would take to build again. That question is open in
the ordinary sense that any unbuilt thing is open. It is not open in the records,
because the records owe a reader the reason the current answer is the answer.

## 4. Things that were true and are now only history

Kept because each cost somebody a day to find out.

* **A default nobody measures is a default nobody runs.** While `bridge` was the
  default backend it panicked on *every* input — `Ident::from_str` interning a
  symbol with no `rustc_span` session installed, a `scoped-tls` panic before it
  printed a line — and this survived undetected because `cargo check` cannot tell
  a lowering that works from one that aborts on its first symbol. It was found
  while measuring something else.
* **The AST was print-only.** Every node carried `NodeId::from_u32(0)` and
  `DUMMY_SP`, where rustc's parser writes `DUMMY_NODE_ID` and expansion asserts
  it before assigning a real id. So the "free to keep open" exit was not free: it
  needed a node-id discipline, a `SourceMap` in the lowering, and a second route
  for diagnostics, whose whole mechanism is translating
  `rustc --error-format=json` about a generated file that an in-memory compile
  does not write.
* **The subprocess compiled at the wrong edition** — no `--edition` was passed,
  so the printed crate compiled at 2015 while it had been interned and printed
  for 2021. Nothing Bridge-IR could express told the two apart, which is why it
  went unnoticed and why fixing it changed no byte of output.
* **Two toolchains cannot share one `target/deps`.** Two rustcs' rlibs in one
  directory, and a test harness that reads that directory to find the crates a
  generated program links cannot tell a stable `winnow_grammar` from a nightly
  one. `scripts/bridge-toolchain.sh` gave the nightly build a target directory of
  its own for that reason, not for tidiness.
* **The test harness held the workspace's only `-Z`.** `rustc -Zls=root` read an
  rlib's dependency list, so the suite needed the nightly even where nothing it
  compiled did. The same question is answered without a flag by asking which
  candidate rlib's filename hash occurs in the anchor's metadata — which is what
  `crates/nikaia/tests/common/mod.rs` does now, and it is the reason a
  stable-only run of the corpus became possible at all.
* **`rustc_private` internals as found on that nightly** — `Item` not holding its
  own identifier, `Fn` holding it, `P<T>` being `Box<T>`, `rustc_ast::Lit` not
  being `rustc_ast::token::Lit` — are in
  [`technical_notes.md`](technical_notes.md), unchanged.

## 5. What replaced it, in one sentence each

* **The interface** is Rust source text ([ADR-003](specification/adr/adr-003.md)
  D1): a frontend emits bytes, and nothing else crosses the line.
* **The lowering** is `crates/nikaia/src/emit`, one pass from the Nikaia AST to
  Rust ([ADR-004](specification/adr/adr-004.md) D1) — which is what
  `--backend rust` always was, and is now simply what the compiler does.
* **The toolchain** is stable, named in `rust-toolchain.toml`, and there is one
  CI leg ([ADR-001](specification/adr/adr-001.md) D1).
* **The orchestrator** stayed, because it was never Bridge-IR's: it is
  `crates/orchestrator` now, and it owns the cache, the lockfile and the Cargo
  wrapping ([ADR-003](specification/adr/adr-003.md) D2).
* **Group B.2** is answered by desugaring in the frontend onto entry-style APIs
  ([ADR-005](specification/adr/adr-005.md) D2), which is what the corpus already
  writes unprompted, and not by a nightly-only flag.
