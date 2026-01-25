# Nikaia Language Specification
**Part III: Tooling, Ecosystem & Interoperability**
**Version:** 0.0.5 (Draft)
**Date:** January 22, 2026

---

## Chapter 13: The Toolchain (CLI)

A modern programming language is more than just a compiler. It requires a suite of tools to manage dependencies, formatting, and building. Nikaia provides a single command-line interface (CLI) called `nikaia`.

### 13.1. Project Structure
When you create a new project (`nikaia new my_project`), the following structure is generated:

* `nikaia.toml`: The **Manifest**. It describes the project, its authors, and its dependencies.
* `nikaia.lock`: The **Lockfile**. It records the exact versions of dependencies for reproducible builds. Additionally, it serves as a **Cache Key** for Compile-Time I/O.
    * **Asset Hashing:** If a macro or grammar reads an external file (e.g., `from "schema.sql"`), the compiler stores the file's SHA256 hash here.
    * **Instant Builds:** On subsequent builds, if the hash on disk hasn't changed, the compiler skips re-processing the macro.
* `src/`: The folder containing your source code.
    * `main.nika`: The entry point.

### 13.2. Core Commands
* `nikaia build`: Compiles the project.
* `nikaia run`: Compiles and executes.
* `nikaia test`: Runs unit tests and fuzzers.
* `nikaia bench`: Runs performance benchmarks.
* `nikaia fmt`: Automatically formats your code.

### 13.3. Manifest Configuration (`nikaia.toml`)
The manifest allows defining project metadata and configuring Build Profiles (Lite vs. Advanced).

```toml
[package]
name = "hyper-core"
version = "0.1.0"
authors = ["dev@nikaia.org"]

# Defines the default compilation mode
# Options: "lite" (I/O optimized) or "advanced" (CPU optimized)
default-profile = "advanced"

[dependencies]
http-server = "1.2"
# Import native Rust Crates
regex = { type = "rust", version = "1.5" }

[profiles.lite]
opt-level = "z"     # Optimize for binary size
panic = "abort"     # Disable stack unwinding for smaller footprint

[profiles.advanced]
opt-level = 3       # Maximize throughput
lto = true          # Link Time Optimization
```

### 13.4. Build Scripts (`build.nika`)
If a project requires custom build steps (e.g., compiling C-code or generating proto-files), you can place a `build.nika` file in the root. This script is compiled and executed **before** the main build.

It has access to a special `std::build` API to emit instructions to the compiler.

```nika
// build.nika
use std::build

fn main() {
    // Compile a local C library
    build::cc("src/native/mylib.c")
    
    // Link against a system library
    build::rustc_link_lib("z") // links libz
}
```

---

## Chapter 14: Testing and Quality Assurance

Testing and verification are first-class citizens in Nikaia.

### 14.1. Unit Tests (`test`)
Standard tests check specific inputs. These blocks are only compiled during `nikaia test`.

```nika
test "Addition" {
    assert 1 + 1 == 2
}
```

### 14.2. Runtime Assertions (Design by Contract)
You can use `assert` statements inside normal functions to enforce preconditions or invariants.

**Compiler Behavior:**
* **Debug Profile:** Assertions are active. If the condition is false, the program panics with a detailed message.
* **Release Profile:** Assertions are **removed** (optimized out) to ensure maximum performance, unless explicitly enabled via `nikaia build --with-asserts`.

```nika
fn divide(a: i32, b: i32) -> i32 {
    // Precondition: Denominator must not be zero.
    // In Release mode, this check disappears.
    assert b != 0, "Division by zero prohibited"
    
    return a / b
}
```

### 14.3. Property-Based Testing (Fuzzing)
Fuzzing generates random data to find crashes. Nikaia automates this.

**Automatic Data Generation**
If you pass arguments to a test, Nikaia automatically generates inputs.
* **Primitives:** Random integers, strings, bools.
* **Structs:** Nikaia recursively generates data for every field.

```nika
struct User { name: String, age: i32 }

// Nikaia automatically creates random 'User' structs here
test "User Validation" (u: User) {
    assert u.age >= 0 // Might fail if fuzzer generates -1
}
```

**Custom Generators (`impl Generator`)**
Sometimes random data isn't enough (e.g., you need valid email addresses). You can implement the `Generator` trait.

```nika
impl Generator for User {
    // 1. How to generate standard random samples
    fn arbitrary() -> User {
        User { 
            name: String::random_alphanumeric(10), 
            age: i32::random_range(0, 100) 
        }
    }

    // 2. Defining "Edge Cases" (Values likely to break things)
    // The fuzzer will ALWAYS try these values first.
    fn edge_cases() -> [User] {
        [
            User { name: "", age: 0 },         // Empty/Zero
            User { name: "A" * 1000, age: -1 } // Overflow/Negative
        ]
    }
}
```

### 14.4. Benchmarking (`bench`)
Benchmarking measures how fast your code is.

**Regression Detection**
When `nikaia bench` runs, it:
1.  Executes the code block thousands of times.
2.  Calculates the average time and standard deviation.
3.  **Compares** it against the last recorded run.

If the new version is significantly slower (e.g., > 5%), the CLI prints a warning:
> ⚠️ **Performance Regression:** 'Sorting' is 12% slower than commit 8f3a2c.

**Result Storage**
Results are stored in `.nikaia/benchmarks.json`. This file tracks:
* Timestamp
* Git Commit Hash
* Function Name
* Nanoseconds per Operation

```nika
bench "Sorting" {
    let list = [5, 2, 9, 1, 6]
    list.sort()
}
```

**Viewing History**
You can visualize the history using: `nikaia bench --history`.

---

## Chapter 15: Interoperability (FFI)

Nikaia is designed to live in a world dominated by C and Rust.

### 15.1. C Interoperability
Talking to C requires `unsafe` blocks because C is not memory-safe.

```nika
extern "C" {
    fn malloc(size: usize) -> Pointer[u8]
}

fn raw_alloc() {
    unsafe { malloc(1024) }
}
```

### 15.2. Rust Integration (Deep Integration)
Nikaia treats Rust Crates differently than C libraries. Because Rust has a strong type system, Nikaia can verify safety properties.

**Mapping Types**
* Rust `i32` -> Nikaia `i32`
* Rust `String` -> Nikaia `String`
* Rust `Option<T>` -> Nikaia `T?` (Nullable)

**Thread Safety (Send/Sync)**
Nikaia can detect thread safety in Rust code. The compiler reads the metadata of the Rust Crate.

* If a Rust type implements the `Send` trait (safe to move between threads), Nikaia allows using it in `spawn` tasks.
* If a Rust type is `!Send` (e.g., `Rc<T>`), and you try to use it in **Nikaia Advanced** (Multi-Threaded), the Nikaia compiler produces an error:
    > "Error: Cannot move Rust type 'Rc<i32>' to another thread. It is not Thread-Safe."

```nika
// Usage of a Rust crate
[dependencies]
image = { type = "rust", version = "0.24" }

// In code
use crate::image

fn process() {
    // This is safe because the 'image' crate implements proper locking
    let img = image::open("test.png")
}
```

### 15.3. WebAssembly (WASM) Synergy
The **Lite Profile** possesses a natural affinity for WebAssembly. Since WASM (in its basic form) shares a linear memory model and runs in single-threaded host environments, the Lite Profile is the perfect match.

**Zero Overhead**
Compiling with `nikaia build --profile=lite --target=wasm32-unknown` produces extremely compact binaries because the compiler does not generate OS-level mutexes or atomic operations in this mode.

**JavaScript Interoperability (`dsl js`)**
Instead of trying to map the entire DOM to Nikaia structs, Nikaia allows embedding raw JavaScript using the `dsl` keyword.

```nika
// main.nika (Lite Profile)
fn main() {
    let message = "Hello from Nikaia!"

    // The 'js' grammar parses the code.
    // We use ':msg' to define a parameter hole.
    let script = dsl js {
        document.querySelector("#submit").addEventListener("click", () => {
            window.alert(:msg);
        });
    } eod

    // Execute the script, passing 'message' into ':msg'
    script.exec(; msg: message)
}
```

---

## Chapter 16: Inline Assembly (via DSL)

In Nikaia 0.0.6, hardware instructions are no longer part of the core language. Instead, they are provided by library-defined DSLs (e.g., `dsl backend::x86` or `dsl backend::wasm`). This decouples the language core from specific hardware architectures.

### 16.1. Usage
Assembly is written using the standard `dsl` syntax. Unlike SQL (which creates a reusable object), the Assembly DSL uses **Immediate Capture** (`meta::capture`) to bind variables from the current scope and injects the machine code directly at the call site.

```nika
use std::backend::x86

fn fast_add(val: i64, ptr: &i64) -> i64 {
    let mut result: i64 = 0
    
    // The DSL executes immediately in the current scope.
    // The grammar parses the bindings and resolves 'val', 'ptr', and 'result'
    // directly from the environment using meta::capture.
    dsl x86 {
        // 1. Binding Header
        // Syntax defined by x86 grammar: $alias = constraint(variable)
        $v = in(reg) val
        $p = in(mem) ptr
        $r = out(reg) result

        // 2. Instructions
        mov $r, $v
        add $r, $p
    } eod
    
    return result
}
```

### 16.2. Benefits
*   **Portability:** The core compiler is not tied to register-based CPUs, making it fully compatible with Stack Machines like WebAssembly.
*   **Validation:** The DSL parser can validate instruction operands at compile time.
*   **Optimization:** The DSL implementation can generate optimized machine code or SIMD instructions specific to the target.

---

## Chapter 17: The Standard Library ("Batteries Included")

Unlike languages that prefer a minimal core, Nikaia pursues immediate productivity. The standard library consists of universal modules (same API everywhere) and profile-specific capabilities.

### 17.1. Universal Modules
These modules rely on Unified Types and function identically in both Lite and Advanced profiles, though their internal implementation differs significantly to match the runtime model.

**`std::http`**
A production-ready HTTP/1.1 and HTTP/2 server and client.
* **Lite Profile:** Runs on a single-threaded Event Loop.
* **Advanced Profile:** Runs on a multi-threaded Work-Stealing Executor.

```nika
use std::http

fn main() {
    // Starts a server on Port 8080.
    // The code looks the same, but the runtime behavior adapts to the profile.
    // Note: We use the Trailing Lambda syntax (fn: ...) for the handler.
    http::Server::new()
        .route("/") fn: "Hello World"
        .listen(":8080")
}
```

**`std::fs` (Compiler Magic)**
File system access is designed to look **blocking** (synchronous) for ease of use. However, the compiler automatically transforms these calls into **non-blocking** state machines backed by the runtime's reactor. You never block the thread, but you never have to write "callback hell".

**Other Key Modules:**
* **`std::json`**: High-performance serialization using compile-time code generation (zero-allocation parsing where possible).
* **`std::cli`**: Parsers for command-line arguments, environment variables, and ANSI terminal colors.
* **`std::net`**: Low-level TCP/UDP sockets for building custom protocols.

### 17.2. Profile-Specific Availability
Some modules are only available or behave restrictively depending on the compilation target.

* **`std::process`**: Spawning child processes.
* **`std::thread` / `spawn`**:
    * **Advanced:** Supports full concurrency. The primary mechanism is `spawn`.
        * **Strict Implicit Move:** To ensure thread safety without complex lifetime tracking, Nikaia enforces **Implicit Move Semantics** for all tasks spawned this way. Ownership of variables used inside the `spawn` block is automatically transferred to the new thread.
    * **Lite / WASM:** Direct usage of `std::thread` results in a **compile-time error**. The Lite profile enforces a "Share-Nothing" architecture where manual threading is prohibited to ensure compatibility with WASM hosts.

**`std::db` (Universal SQL)**
Nikaia provides a unified SQL interface, starting with SQLite, designed to abstract the underlying platform constraints completely.

* **Zero-Blocking Guarantee:** Database operations are implicitly asynchronous. They never block the Event Loop (Lite) or the Compute Scheduler (Advanced).
* **Architecture Adapter:** The implementation switches automatically based on the compilation target:
    * **Native Targets:** Utilizes a dedicated, hidden I/O thread (powered by `tokio-rusqlite`) to offload blocking filesystem operations.
    * **WASM Targets:** Automatically spawns a **Web Worker** and utilizes the **OPFS** (Origin Private File System). This enables native-grade, persistent SQL performance in the browser without freezing the UI thread.

```nika
use std::db::sqlite

fn query_data() {
    // Transparently starts the required Sidecar (Thread or Worker)
    let db = sqlite::open("app.db")
    
    // The 'sql' macro validates syntax at compile-time.
    // At runtime, it performs an async round-trip to the sidecar.
    let active_users = dsl sql db {
        SELECT * FROM users WHERE last_login > 0
    } eod
}
```

---

# Appendix A: Error Hierarchy

Nikaia strictly distinguishes between errors caused by the environment (recoverable) and bugs in the program logic (unrecoverable).

### A.1. Recoverable Errors (`throws`)
Errors arising from external circumstances (File not found, Network timeout).
* **Mechanism:** Must be declared in the function signature via `throws`.
* **Handling:** Enforced by the compiler via `catch{}` blocks or propagation.

### A.2. Unrecoverable Errors (`panic`)
Errors indicating an inconsistent program state (Index Out of Bounds, Division by Zero, explicit `panic()`). The behavior differs drastically based on the profile:

| Profile | Panic Behavior | Consequence |
| :--- | :--- | :--- |
| **Lite** | **Abort** | The entire process terminates immediately. In WebAssembly, this triggers a "Trap". There is no stack unwinding, resulting in minimal binary size. |
| **Advanced** | **Task Poisoning** | Only the affected Task (Green Thread) is terminated. The worker thread catches the panic (Fault Isolation). Resources (`Locked[T]`) held by the task are marked as "poisoned" to prevent other threads from accessing corrupted state. |

# Appendix B: Compiler Internals & Annotations

To enforce the "Contextual Capture" rules (Chapter 5.4) without hard-coding specific function names into the compiler, Nikaia uses internal attributes. These are primarily used by the Standard Library but are available to library authors.

### B.1. Capture Attributes

| Attribute | Internal Name | Default | Description |
| :--- | :--- | :--- | :--- |
| None | `capture_mode = "immediate"` | Yes | The lambda executes within the caller's stack frame. Captured variables are **Borrowed** (`&T`). Used by `map`, `filter`, `lock.access`. |
| `@detached` | `capture_mode = "detached"` | No | The lambda escapes the current stack frame (stored, spawned, or deferred). Captured variables are **Moved** (Owned). Used by `spawn`, `defer`. |

### B.2. Standard Library Signatures

Here is how common standard library functions are annotated internally to drive the compiler's behavior:

```nika
// std::collections::List
// Standard immediate execution
pub fn map[U](self, op: fn(T) -> U) -> List[U]

// std::task (Global Spawn)
// Detached execution: Must take ownership of environment
pub fn spawn(task: @detached fn() -> T) -> TaskHandle[T]

// std::task
// Scope is immediate because it waits for completion
pub fn scope(f: fn(Scope))
