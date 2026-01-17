# Nikaia Language Specification
**Part III: Tooling, Ecosystem & Interoperability**
**Version:** 0.0.4 (Educational Draft)
**Date:** January 17, 2026

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

---

## Chapter 16: Inline Assembly

For low-level control (kernels, drivers, SIMD), Nikaia provides `unsafe asm`.
To ensure robust parsing and clear separation of concerns, the assembly construct is divided into two distinct blocks: the **Binding Header** and the **Assembly Body**.

### 16.1. Syntax Structure
```nika
unsafe asm {
    // [Block 1] The Binding Header
    // Maps Nikaia variables to internal assembly aliases.
    // Syntax: $alias = direction(location) variable
    $lhs = in(reg) a,
    $rhs = in(mem) b,
    $dst = out(reg) result
} {
    // [Block 2] The Assembly Body
    // Contains raw assembly instructions.
    // The compiler treats this as a template string and only replaces aliases ($name).
    mov $dst, $lhs
    add $dst, $rhs
}
```

### 16.2. Directions and Modifiers
The first part of the constraint defines how data flows between Nikaia and the CPU.

* `in(...)`: Read-only input. The variable is copied into the location before execution.
* `out(...)`: Write-only output. The result in the location is copied to the variable after execution.
* `inout(...)`: Read-write. Initialized with the variable's value, and the result is written back.
* `lateout(...)`: Optimization hint. Defines an output that is written *after* all inputs are consumed. Allows the compiler to reuse an input register for this output (saving registers).

### 16.3. Location Constraints
The second part defines where the value must be placed (Register vs. Memory).

| Constraint | Description | Example Architecture Mapping |
| :--- | :--- | :--- |
| `reg` | Any general-purpose integer register | x86: `rax`, `rbx`, ... |
| `freg` | Floating-point / SIMD register | x86: `xmm0` - `xmm15` |
| `mem` | A memory operand (address) | Passed as `[ptr]` or specific syntax |
| `imm` | An immediate constant value | Used for instructions expecting literals |
| `reg_or_mem` | Flexible: Compiler chooses best fit | Useful for CISC (x86) instructions like `add` |

### 16.4. Example: x86_64 Arithmetic
This example demonstrates mixing memory and register operands safely.

```nika
fn fast_add(val: i64, ptr: &i64) -> i64 {
    let result: i64
    
    unsafe asm {
        // We read 'val' into a register
        $v = in(reg) val,
        // We can read 'ptr' directly from memory (efficient on x86)
        $p = in(mem) ptr,
        // We write the result to a register
        $r = out(reg) result
    } {
        // AT&T Syntax example
        mov $v, $r
        add $p, $r
    }
    
    return result
}
```

### 16.5. Clobbering (Side Effects)
If your assembly modifies registers that are not defined as outputs (e.g., flags or specific hardcoded registers), you must declare them in the header using the `clobber` keyword.

```nika
unsafe asm {
    $src = in(reg) input,
    clobber("cc") // "cc" tells the compiler: Condition Codes (Flags) are modified
} {
    test $src, $src
}
```
