# Nikaia Toolchain Architecture

This document describes how the various crates in the Nikaia workspace are stacked and interact with each other to form the complete compiler toolchain. The architecture follows the **Hub-and-Spoke model** defined in [ADR-003](specification/adr/adr-003.md).

## High-Level Stack

The toolchain is composed of four distinct layers, separating the language frontend from the compilation backend via a stable protocol.

```mermaid
graph TD
    A[User Source (.nika)] -->|Input| B(Crate: nikaia)
    subgraph Frontend
        B -->|Parses| C{AST}
        C -->|Lowers| D[Bridge IR]
    end
    D -->|Protocol| E(Crate: bridge-ir)
    E -->|Consumed by| F(Crate: rustc-executor)
    subgraph Backend
        F -->|Generates| G[Rust Source (.rs)]
        G -->|Invokes| H[System rustc]
    end
    H -->|Produces| I[Binary Executable]
    
    J(Crate: bridge-orchestrator) -.->|Manages| B
    J -.->|Manages| F
```

## Detailed Component Breakdown

### 1. The Frontend: `crates/nikaia`
*   **Role**: The user-facing entry point (CLI). It understands the Nikaia language syntax and semantics.
*   **Responsibilities**:
    *   **CLI**: Handles arguments via `clap`.
    *   **Parsing**: Uses `winnow-grammar` (and `winnow`) to parse `.nika` source files into a Nikaia-specific Abstract Syntax Tree (AST).
    *   **AST Definition**: Defines the language constructs (Functions, Structs, Enums, Expressions) in `src/ast`.
    *   **Lowering**: Translates the high-level Nikaia AST into the simplified `BridgeModule`. This is where Nikaia-specific sugar is desugared.
*   **Dependencies**: `bridge-ir`, `winnow-grammar`.

### 2. The Protocol: `crates/bridge-ir`
*   **Role**: The "Waist" of the compiler. A stable, pure-data intermediate representation.
*   **Responsibilities**:
    *   Defines the contract between any frontend (Nikaia, or future DSLs) and the backend.
    *   Contains serializable structs (`BridgeModule`, `BridgeFunction`, `BridgeLetStmt`).
    *   **Crucial**: This crate has **no** logic, only definitions. It decouples the frontend from Rust compiler internals.

### 3. The Backend: `crates/rustc-executor`
*   **Role**: The heavy lifter that produces machine code.
*   **Responsibilities**:
    *   Accepts a `BridgeModule`.
    *   **Transpilation**: Converts the Bridge IR into valid, compilable Rust source code (`.rs`).
    *   **Compilation**: Invokes the system `rustc` command to compile the generated source into a binary.
    *   *Note*: In the future, this might link directly against `rustc_driver` for deeper integration, but currently operates via source generation for stability.

### 4. The Manager: `crates/bridge-orchestrator`
*   **Role**: The build system coordinator.
*   **Responsibilities**:
    *   Intended to handle incremental compilation, caching, and workspace management.
    *   *Current Status*: Currently provides trait definitions (`LanguageFrontend`) used by `nikaia`. Its role will expand to manage the build graph.

## Data Flow Example

1.  **Input**: `let x = 5;`
2.  **Nikaia Parser**: Produces `ast::Stmt::Let { name: "x", value: LitInt(5) ... }`
3.  **Nikaia Lowering**: Converts to `bridge_ir::BridgeStmt::Let { name: "x", init: Literal(Int(5)) }`
4.  **Executor**: Generates Rust code `let x = 5;` inside a main function.
5.  **Rustc**: Compiles it to machine code.
