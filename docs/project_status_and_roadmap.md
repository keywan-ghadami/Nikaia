# Nikaia Project Status & Roadmap

This document outlines the current status of the Nikaia compiler and the toolchain, and lists the necessary steps to reach a fully functional and stable v1.0 release.

## Current Status (Vertical Slice: Complete & Architecturally Robust)

We have successfully implemented a "Vertical Slice" of the compiler that can compile a simple "Hello World" program, using a robust, future-proof architecture.

*   ✅ **Parser**: Functional `winnow-grammar` parser for `fn`, `block`, `let`, `call`, and primitive literals.
*   ✅ **AST**: Nikaia AST defined and used.
*   ✅ **Lowering**: Transformation from Nikaia AST to Bridge IR implemented.
*   ✅ **Bridge IR**: Stable intermediate representation defined.
*   ✅ **Unified AST Lowering (Phase 4)**: 
    *   Implemented `Bridge -> rustc_ast::Crate` transformation in `rustc-executor` (ADR-004).
    *   Verified against `nightly-2026-01-01` source code.
    *   Correctly maps Nikaia concepts to internal Rust AST (`Fn`, `Item`, `Stmt`, `Expr`, `Lit`).
    *   *Note*: Code compiles (`cargo check`), but running the binary requires `RUSTFLAGS="-C prefer-dynamic"` and correct environment setup due to `rustc_private` dynamic linking requirements.
*   ✅ **Executor**: Generates valid Rust source from the internal AST (Transpilation for Debug) using `rustc_ast_pretty`.
*   ⚠️ **End-to-End Execution**: Currently blocked by `std` linkage conflicts when running via `cargo run`. Requires environment configuration for dynamic linking of `rustc_driver`.
*   ✅ **Nikaia in `std` (ADR-014)**: `crates/nikaia-std/src/*.nika` are compiled by the Stage 0 compiler when `std` is built. One function so far (`digit_value`); the share grows with what the compiler can lower. `fs::map` is a memory mapping and the `par_fold` driver runs on rayon (8M lines: 0.52 s → 0.14 s on 4 cores).
*   ✅ **Input provenance (ADR-010) chooses the hasher**: `std`'s ledger says which of its calls are sources and who chose their bytes (ADR-020), `contracts::trust` joins them over a program, and a map keyed by the input gets the hash that answer picks - fast and fixed-seed where the operator chose the bytes, keyed and randomly seeded where someone else did. Callgrind on 200 000 rows, the same tree built twice with byte-identical output: **120.0 M instructions hardened against 89.1 M chosen, 26 %** - the largest single item on the flagship, and it was 22 % when it was predicted. `nikaia --trust` prints the choice and what decided it (D7). What Stage 0's analysis is: one buffer, because ADR-008 gives a compilation unit one input lifetime, so the join over a program's sources *is* the per-buffer analysis for the one buffer this representation can express.
*   ✅ **1BRC below hand-written Rust (ADR-015)**: with the backend's lazy diagnostics the generated parser costs 659 instructions per row, against 688 for the same aggregation hand-tuned in Rust and 840 written naively. Sequentially 0.52 s on 8M rows, 0.14 s on 4 cores, identical output.
*   ✅ **`fs::map`'s UTF-8 check is chunked (ADR-016)**: validated one piece per core at character boundaries, reporting the earliest failing byte, so a rejected file names the same offset however many cores looked at it. 63.7 ms → 16.5 ms on the check; removing it entirely would buy 10 ms more and cost the `&str` guarantee every view rests on.
*   ✅ **Grammar Lowering (Stage 0 transpiler, ADR-011)**: `--backend rust` lowers `grammar` items onto `winnow-grammar`'s `grammar!`: rules, patterns, the commit point, bounded repetition, `@frame` -> `#[frame]`, `fold`/`par_fold`, and `dsl … from …` onto the generated piece driver with the `Parallelism` the `--profile` asks for. Checked by compiling and running the emitter's own output (`crates/nikaia/tests/grammar_lowering.rs`).
    *   *Scope*: syntactic. The lowering adapts nothing: an `impl` method used as a fold's step or merge is emitted as written (ADR-011 D2, §4). The type checker (ADR-024) reports what the ledger writes down; a shape mismatch inside a generated parser is not among it, and is still `rustc`'s to find.

---

## Remaining Work to Finalize Nikaia

### Phase 1: Language Completeness (Frontend)

To make Nikaia usable for real-world programming, we need to expand the frontend capabilities.

*   [ ] **Control Flow**: Implement `if/else`, `loop`, `while`, `for`.
    *   *Parser*: `if/else` and `for` parse; `loop` and `while` do not.
    *   *AST*: `Expr::If` and `Stmt::For` exist; `Loop` does not.
    *   *Rust backend*: both are emitted.
    *   *Lowering*: Map to Rust equivalents in Bridge IR - still open, the Bridge path carries neither.
*   [ ] **Data Structures**: Implement `struct` and `enum` definitions.
    *   *Parser*: `struct` fields parse (with `@borrowed`); enum variants do not.
    *   *Rust backend*: emits struct definitions, with the input lifetime where a field is a view (ADR-011 D6).
    *   *Bridge IR*: Add `BridgeStruct` and `BridgeEnum` definitions.
    *   *Executor*: Generate Rust struct/enum definitions.
*   [x] **Methods & Impl Blocks** (ADR-013): `impl` blocks, receivers, the anonymous constructor, and the fold adapter that ADR-011 D2 deferred until the receivers were known.
    *   *Open*: traits, generics on impls, and operators as methods.
*   [ ] **Generics**: Fully support generic type parameters (`<T>`) across functions and structs.
    *   *Status*: Parser has basic support (using `[...]`), but lowering and bridge need full integration.
*   [ ] **Modules & Imports**: Implement `use` and multi-file compilation support.
    *   *Parser*: `use` keyword.
    *   *Orchestrator*: Handle file resolution and dependency graph.

### Phase 2: Compiler Robustness (Middle-end)

*   [ ] **Error Reporting**: Replace generic `anyhow` errors with specific, span-aware error messages using `miette` or `codespan`.
    *   *Done (ADR-012)*: the AST carries spans, the `rust` backend emits a source map, and `nikaia --explain` reports rustc's JSON diagnostics - including the parser backend's frame check - on the `.nika` line that caused them.
    *   *Open*: expression-level spans - both the `sync` check and the type checker report on the enclosing statement (ADR-024 D7), and both get narrower the day expressions carry spans, without either changing - and the compiler's own `anyhow` errors, which are still text without a position.
*   [x] **Kap 5.1's `Subject ; Config` protocol** (G18): a `;` in a signature, positional data before it and named options with defaults after it, on both the declaration and the call. The language below has neither named arguments nor defaults, so an option becomes an ordinary parameter in declaration order and a call fills in what it left out - which needs the *callee's* declaration, and is therefore read from the ledger. `fs::write` has the `append` and `create` its specification names. `NK1109` is an option the callee does not have.
*   [x] **A higher-order function does what its lambda does** ([ADR-029](specification/adr/adr-029.md)): `sync = "from(f)"` names the parameter that decides, and the ledger's type language gains `fn(…)` so a lambda's parameters have types. Before it, no `map`, `filter`, `fold` or `sort_by_key` could appear inside `access` or `par_iter` over *any* lambda - a ledger entry holds for every caller, so a function running somebody else's code had to commit to the pessimistic answer.
    *   *Why it is sound*: a caller reads `from` as "adds no pausing of its own", which holds because the lambda runs *during* the call and its body is already counted in the function that writes it. A **detached** parameter would break that; the ledger cannot spell `@detached`, so a test over `std`'s own entries enforces the restriction instead.
    *   *Measured, honestly*: **nothing changed** on the corpus - 14 of 18, 20 of 43, both unchanged. No program here is unblocked by it. What it removes is a **false rejection**, pinned by a two-line test: a helper using `sort_by_key` over a pure lambda used to be `NK2202` when called from a lock.
    *   *Open, and one question*: the four functions still not recovered are all `.and_modify fn { a.add(…) }`, where `a` would have to be typed `&Stats` - and `Stats` is the map's type argument, which the signature language cannot name. **May a `std.contracts` signature refer to its receiver's type arguments?** It would be the first inference in the ledger's type language rather than more vocabulary.

*   [x] **Method calls resolve, because the type checker already resolved them** ([ADR-028](specification/adr/adr-028.md)): `Ledger::infer` runs the type checker and the `sync` inference reads its answers, rather than treating every `stats.add(5)` as an unknown. The alternative was a second type checker living in `sync.rs`, obliged to agree with the first.
    *   *Soundness*: the checker reads `signature`, `fields` and `iterates` and never `sync`, so resolving a method against the half-built ledger gives what resolving it against the finished one would. Held by a test that runs the checker against both and compares, rather than by a comment.
    *   *Also*: indexing a `Vec[T]`/`List[T]`/`HashMap[K, V]` has a type (one `Unknown` at the bottom of an expression erased everything built on it), and `std.contracts` now describes the collections and text the language below provides - every entry observed being asked for by a program here, because a wrong entry rejects a correct program.
    *   *Measured*: strip every `sync` from the examples and count what comes back - **9 of 18 → 14 of 18**, with `n-body` going 0/3 → 3/3 on the typed index alone.
    *   *Open, and now a single shape*: the remaining four are all a method on a **lambda parameter** (`.and_modify fn { a.add(…) }`). Typing `a` needs the ledger to describe a function-typed parameter - the same vocabulary `sync = "from(f)"` needs, from the other side.

*   [x] **`sync` is earned, not entered** ([ADR-027](specification/adr/adr-027.md)): the ledger records `sync` for every function whose body provably cannot pause, not only for the ones someone annotated. Part II 12.1's rule is unchanged; what changed is that a function that calls nothing which can pause now *says so*, instead of the ledger recording "not `sync`" about a body it had already read.
    *   *Why it is not a nicety*: `access`, `access_all`, `par_iter`, a scope's tasks under Advanced and the panic hook all gate on a `sync` lambda. While `sync` was opt-in, what those lambdas could call was whatever somebody had remembered to annotate, and nothing from another library could be annotated at all - so the safe path was the narrow one.
    *   *The polarity*: the **check** on an assertion stays permissive (it never rejects a correct program; an unresolvable method call is not an error), and the **inference** is restrictive (that same call costs the function its claim). The entry is shipped and a consumer admits the body into `access` on the strength of it - ADR-010 D1's rule, applied to a second question.
    *   *Measured*: strip all seven `sync` declarations from `examples/1brc.nika` and four come back on their own; the three that do not reach `HashMap::new()` and `.entry(…)`. That boundary is ADR-024's open work, not this decision's limit - a method becomes resolvable when its receiver has a signature, and neither analysis changes.
    *   *Open*: a `pub` function that loses an inferred `sync` is a breaking change for its consumers and nothing classifies it as one yet - the same question a `pub` function *gaining* `throws` raises. `--locked` has the mechanism (it now names the entry a changed line belongs to); what is missing is calling such a diff a semver event by name.

*   [x] **A loop can fail, and the compiler says so** ([ADR-025](specification/adr/adr-025.md)): where the language performs a call nobody wrote - a resource's `cleanup` at a block's end, a `for`'s step - a failure of it fails the enclosing function, and `NK2701` is what the compiler says when the function does not declare `throws`. Part I 6.4 was the rule's first case and never stated it generally. `fs::lines` and `fs::bytes` are **removed** from Part III 17.1 rather than deferred: their specified shape would have an iterator hand out views into a buffer it owns, and the shape that works - `fs::map(path)` plus `.lines()` - is the separation Part I 6.6 rests on. `io::lines()` exists, because a pipe cannot move its failure to the call, and `examples/tally.nika` is it running in constant memory.
    *   *Where it shows in the compiler*: the emitter reads the **type checker's** answers for the first time - it asks which `for` iterates a fallible stream, so naming the stream in a `let` first is the same as calling it in the loop head. The ledger carries the fact (`iterates = "throws"` on a type contract), because the type is `std`'s and its body is Rust.
*   [x] **Type Checking (Frontend)**: done, and it runs before a line of Rust is emitted ([ADR-024](specification/adr/adr-024.md)). Eight `NK1xxx` codes - call arity, argument types, `let`, `return` and a body's tail, assignment, struct-literal fields, a field that is not there, a condition that is not a `bool` - each with the source line, a caret and a concrete way out.
    *   *The design*: `?` is part of the type language and means **the absence of a claim**. An error is reported only where **both** sides are written down and disagree, so the checker never rejects a program that is correct - which matters because half of `std` is still Rust and a program calls `push_str`, `entry` and `chars` freely. Its database is the ledger (13.5), so a call into `std` is checked against the contracts `std` ships, and a call into a package will be checked against that package's on the day one arrives.
    *   *What it does not catch*: a method on a receiver whose type is not written down, a collection's element type, what a `?` or a `??` unwraps, what a `match` arm binds. Each becomes checkable when a signature is written, without the checker changing - which is why it was built on the ledger rather than beside it.
    *   *Guard*: every `.nika` in the repository must produce no findings, and a deliberately-wrong program per code proves a clean corpus is not the checker being asleep.
*   [ ] **Macro Expansion (JIT)**: Implement the "Phase 2" JIT interpreter mentioned in ADR-003 to handle macros and compile-time execution.
    *   *Status*: Placeholder exists, needs implementation.

### Phase 3: Tooling & Ecosystem (The "Hub")

*   [ ] **Bridge Orchestrator (Cargo Wrapper)**: Implement the logic defined in ADR-003 to wrap `cargo build`.
    *   *Missing*: Setting `RUSTC_WORKSPACE_WRAPPER`, intercepting compiler calls, and delegating to `nikaia` frontend for `.nika` files vs `rustc` for `.rs` files.
    *   *Current*: Orchestrator is a simple CLI argument parser with backend selection scaffolding.
*   [x] **Incremental Compilation (mechanism)**: `bridge_orchestrator::cache` implements it per [ADR-021](specification/adr/adr-021.md) - SHA256 input hashing, `nikaia.lock` as the committed record, one key per translation unit, and a content-addressed artifact store under `target/nikaia/cache/`. Every dimension of the key is enumerated in one function (`Key::build`); build-time choices reach the key and never the file. No external cache (`sccache` was considered and rejected: it memoizes `rustc` calls, while the repeated work is macro evaluation and lowering, which happen before `rustc` is reached).
    *   *On by default* ([ADR-021](specification/adr/adr-021.md) D11): reusing an unchanged lowering is the difference in feel to a Rust build, and a default nobody types is not that. `--no-cache` opts out. Where the files go follows from that: with a `nikaia.toml` the lock is committed at the root and the store sits in `target/`; without one nothing is written into the source tree at all and both move to the user's cache directory. A cache that cannot be read or written degrades the build and never fails it (D12).
    *   *Covers the whole `rust` build* (D13): on a hit nothing is parsed, `sync`-checked, lowered or inferred, because only a build that passed every check is ever recorded and the key holds everything those checks read. The emitted Rust and ADR-020's ledger travel together in one envelope, so a hit is never half a build. `--locked` still runs on a hit - that question is about the committed file, not the source.
    *   *Missing*: nothing reports **assets** yet - compile-time I/O (`from "schema.sql"`) is specified but unimplemented, so the asset dimension is carried through the key and exercised by tests without a real producer. And the `rustc` invocations in `rustc-executor` and the test harness are still uncached, which is where the measured repetition actually is.
*   [ ] **Tier-1 staging (compiler-side)**: independent of compile-time I/O and blocked by none of its questions ([ADR-026](specification/adr/adr-026.md) §3). Survey of real candidates, closed ones, and how to measure one: [`docs/staging-candidates.md`](staging-candidates.md). The rule it lands on: **a staging decision enters the compiler only with a measured crossover.**
    *   *Done*: a **measurement harness** (`crates/nikaia/tests/measure.rs`, callgrind, ignored by default) with two workloads in `benches/`, and the survey's first two candidates measured and built. Both intuitions were wrong. `html::escape` is **65 % cheaper** - and the *table-driven* version the survey recommended was **16 % worse** than what it replaced, because a 256-entry table of fat pointers is 4 KB to walk where a one-word mask is a register. A template's `String::with_capacity` is worth **3.4 %** on a static page and **0.0 %** on a growing table, and the difference is structural.
    *   *Done, and it was not what the survey said*: **the compiler parses 20 % faster.** One callgrind profile, taken before writing any code, put `parse_WS_inner` at **32.5 %** of the parse and the class scan inside it at **24.1 %**, against under 3 % for the literal-alternation dispatch the survey called "the actual prize". A syntactic rule's leading whitespace skip was emitted inside each alternative, so a sixteen-way rule ran sixteen skips at one position to consume one blank. Hoisting it ([winnow-grammar#14](https://github.com/keywan-ghadami/winnow-grammar/pull/14)) is **1,025.5 M -> 822.2 M instructions, −19.8 %**, branches −19.8 %, mispredicts −9.5 %, cache flat, error corpus byte-identical.
    *   *Left*: what remains of the literal-alternation item is a ≤3 % ceiling in a dependency, which is not where the next measurement should go. Route hashing still has no target - there is no HTTP server.
*   [ ] **Compile-Time I/O**: the producer the cache's asset dimension is waiting for. Not a straightforward feature - a grammar's `action` blocks are arbitrary Nikaia, so evaluating one at build time means running user code at build time. The design space is staked out in [ADR-026](specification/adr/adr-026.md) (**Open**): two things decided (I/O belongs to the compiler, not the sandbox; paths stay in the project root and `..` is refused rather than resolved), six questions listed, and the one that blocks the others named - what a program is allowed to do in `const`.
*   [ ] **LSP Server**: Create a Language Server Protocol (LSP) implementation.
    *   *Benefit*: IDE support (syntax highlighting, go-to-definition) in editors like VS Code.
    *   *Reuse*: Reuse the parser and AST for this.
*   [ ] **Standard Library**: Define the Nikaia standard library (wrapper around Rust std or custom).
    *   *Task*: Create `std.nika` files that are implicitly imported.

### Phase 4: Backend Optimization

*   [x] **Direct `rustc_driver` Integration (Code)**: Logic implemented and verified.
*   [ ] **Direct `rustc_driver` Integration (Runtime)**: Fix `std` linkage issues to allow the compiler to run as a standalone binary linking against `rustc_driver` dylibs.
*   [x] **LLVM / Cranelift Backend (investigation)**: Done - see [ADR-021](specification/adr/adr-021.md) D9 and its measurements. Cranelift is fully compatible with this workspace, `rustc_private` linkage included, and all 51 tests pass under it; it buys ~1s on an incremental rebuild and nothing on a full build. Supported option, not the default.
*   [x] **Backend selection rejects what it cannot do**: `--backend cranelift` and `--backend llvm` used to be accepted and silently behave like `bridge`. They now fail with a message naming the available backends, as does any unknown value ([ADR-021](specification/adr/adr-021.md) D9). Implementing them remains open; pretending to have done so no longer is.

---

## Immediate Next Steps

1.  **Fix Linkage**: Configure cargo/rustc flags to allow running the `rustc-executor` with dynamic linking.
2.  **Implement Control Flow**: Add `if/else` support.
3.  **Struct Support**: Allow defining and instantiating simple structs.
