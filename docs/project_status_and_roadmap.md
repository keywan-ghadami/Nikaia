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
*   ✅ **1BRC below hand-written Rust (ADR-015)**: with the backend's lazy diagnostics the generated parser costs 659 instructions per row, against 688 for the same aggregation hand-tuned in Rust and 840 written naively. Sequentially 0.52 s on 8M rows, 0.14 s on 4 cores, identical output.
*   ✅ **`fs::map`'s UTF-8 check is chunked (ADR-016)**: validated one piece per core at character boundaries, reporting the earliest failing byte, so a rejected file names the same offset however many cores looked at it. 63.7 ms → 16.5 ms on the check; removing it entirely would buy 10 ms more and cost the `&str` guarantee every view rests on.
*   ✅ **Grammar Lowering (Stage 0 transpiler, ADR-011)**: `--backend rust` lowers `grammar` items onto `winnow-grammar`'s `grammar!`: rules, patterns, the commit point, bounded repetition, `@frame` -> `#[frame]`, `fold`/`par_fold`, and `dsl … from …` onto the generated piece driver with the `Parallelism` the `--profile` asks for. Checked by compiling and running the emitter's own output (`crates/nikaia/tests/grammar_lowering.rs`).
    *   *Scope*: syntactic. There is no type checker, so `impl` methods used as a fold's step or merge are emitted as written and rejected by `rustc` rather than adapted (ADR-011 D2, §4).

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
    *   *Open*: expression-level spans (a type error is reported on its statement), and the compiler's own `anyhow` errors, which are still text without a position.
*   [ ] **Type Checking (Frontend)**: Implement a basic type checker in the frontend *before* lowering to Bridge IR to catch errors early.
    *   *Current*: We rely on `rustc` to catch type errors, which gives poor UX for Nikaia users.
*   [ ] **Macro Expansion (JIT)**: Implement the "Phase 2" JIT interpreter mentioned in ADR-003 to handle macros and compile-time execution.
    *   *Status*: Placeholder exists, needs implementation.

### Phase 3: Tooling & Ecosystem (The "Hub")

*   [ ] **Bridge Orchestrator (Cargo Wrapper)**: Implement the logic defined in ADR-003 to wrap `cargo build`.
    *   *Missing*: Setting `RUSTC_WORKSPACE_WRAPPER`, intercepting compiler calls, and delegating to `nikaia` frontend for `.nika` files vs `rustc` for `.rs` files.
    *   *Current*: Orchestrator is a simple CLI argument parser with backend selection scaffolding.
*   [ ] **Incremental Compilation**: Implement hashing and caching in the Orchestrator.
    *   *Goal*: Avoid recompiling unchanged files.
*   [ ] **LSP Server**: Create a Language Server Protocol (LSP) implementation.
    *   *Benefit*: IDE support (syntax highlighting, go-to-definition) in editors like VS Code.
    *   *Reuse*: Reuse the parser and AST for this.
*   [ ] **Standard Library**: Define the Nikaia standard library (wrapper around Rust std or custom).
    *   *Task*: Create `std.nika` files that are implicitly imported.

### Phase 4: Backend Optimization

*   [x] **Direct `rustc_driver` Integration (Code)**: Logic implemented and verified.
*   [ ] **Direct `rustc_driver` Integration (Runtime)**: Fix `std` linkage issues to allow the compiler to run as a standalone binary linking against `rustc_driver` dylibs.
*   [ ] **LLVM / Cranelift Backend**: Investigate alternative backends for faster debug builds (e.g., using Cranelift directly via `bridge-ir`).

---

## Immediate Next Steps

1.  **Fix Linkage**: Configure cargo/rustc flags to allow running the `rustc-executor` with dynamic linking.
2.  **Implement Control Flow**: Add `if/else` support.
3.  **Struct Support**: Allow defining and instantiating simple structs.
