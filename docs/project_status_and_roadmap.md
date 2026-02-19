# Nikaia Project Status & Roadmap

This document outlines the current status of the Nikaia compiler and the toolchain, and lists the necessary steps to reach a fully functional and stable v1.0 release.

## Current Status (Vertical Slice: Complete)

We have successfully implemented a "Vertical Slice" of the compiler that can compile a simple "Hello World" program.

*   ✅ **Parser**: Functional `winnow-grammar` parser for `fn`, `block`, `let`, `call`, and primitive literals.
*   ✅ **AST**: Nikaia AST defined and used.
*   ✅ **Lowering**: Transformation from Nikaia AST to Bridge IR implemented.
*   ✅ **Bridge IR**: Stable intermediate representation defined.
*   ✅ **Executor**: Transpilation to Rust source code and compilation to executable via `rustc`.
*   ✅ **End-to-End**: `hallo.nika` -> `hallo` executable works.

---

## Remaining Work to Finalize Nikaia

### Phase 1: Language Completeness (Frontend)

To make Nikaia usable for real-world programming, we need to expand the frontend capabilities.

*   [ ] **Control Flow**: Implement `if/else`, `loop`, `while`, `for`.
    *   *Parser*: Add grammar rules.
    *   *AST*: Add `If`, `Loop` variants to `Expr`.
    *   *Lowering*: Map to Rust equivalents in Bridge IR.
*   [ ] **Data Structures**: Implement `struct` and `enum` definitions.
    *   *Parser*: Support struct fields and enum variants.
    *   *Bridge IR*: Add `BridgeStruct` and `BridgeEnum` definitions.
    *   *Executor*: Generate Rust struct/enum definitions.
*   [ ] **Methods & Impl Blocks**: Support `impl` blocks and method calls (`x.foo()`).
    *   *Parser*: Handle dot notation and `impl` keyword.
    *   *Lowering*: Desugar method calls to function calls with `self`.
*   [ ] **Generics**: Fully support generic type parameters (`<T>`) across functions and structs.
    *   *Status*: Parser has basic support (using `[...]`), but lowering and bridge need full integration.
*   [ ] **Modules & Imports**: Implement `use` and multi-file compilation support.
    *   *Parser*: `use` keyword.
    *   *Orchestrator*: Handle file resolution and dependency graph.

### Phase 2: Compiler Robustness (Middle-end)

*   [ ] **Error Reporting**: Replace generic `anyhow` errors with specific, span-aware error messages using `miette` or `codespan`.
    *   *Requirement*: Propagate source spans correctly through AST -> Bridge IR.
*   [ ] **Type Checking (Frontend)**: Implement a basic type checker in the frontend *before* lowering to Bridge IR to catch errors early.
    *   *Current*: We rely on `rustc` to catch type errors, which gives poor UX for Nikaia users.
*   [ ] **Macro Expansion (JIT)**: Implement the "Phase 2" JIT interpreter mentioned in ADR-003 to handle macros and compile-time execution.
    *   *Status*: Placeholder exists, needs implementation.

### Phase 3: Tooling & Ecosystem (The "Hub")

*   [ ] **Bridge Orchestrator**: Implement incremental compilation and caching.
    *   *Goal*: Avoid recompiling unchanged files.
    *   *Strategy*: Hash source files and cache Bridge IR artifacts.
*   [ ] **LSP Server**: Create a Language Server Protocol (LSP) implementation.
    *   *Benefit*: IDE support (syntax highlighting, go-to-definition) in editors like VS Code.
    *   *Reuse*: Reuse the parser and AST for this.
*   [ ] **Standard Library**: Define the Nikaia standard library (wrapper around Rust std or custom).
    *   *Task*: Create `std.nika` files that are implicitly imported.

### Phase 4: Backend Optimization

*   [ ] **Direct `rustc_driver` Integration**: Move from source generation (transpilation) to direct AST generation using `rustc_private` (optional, for performance).
    *   *Status*: `rustc-executor` currently shells out to `rustc` CLI.
*   [ ] **LLVM / Cranelift Backend**: Investigate alternative backends for faster debug builds (e.g., using Cranelift directly via `bridge-ir`).

---

## Immediate Next Steps

1.  **Implement Control Flow**: Add `if/else` support to the parser and bridge.
2.  **Struct Support**: Allow defining and instantiating simple structs.
3.  **Error Messages**: Improve parser error reporting to show line/column numbers.
