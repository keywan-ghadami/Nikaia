# Nikaia Language Specification
**Part I: The Language Core & Nikaia Lite**
**Version:** 0.0.7 (Draft)
**Date:** September 5, 2026

---

## Chapter 1: Introduction and Philosophy

### 1.1. What is Nikaia?
Nikaia is a programming language designed to solve a specific problem in software development: the trade-off between ease of use and technical performance.

In many languages, developers must choose between:
1.  **Scripting Languages** (like Python): Easy to read and write, but often slow and prone to errors that only appear when the program is running (**Runtime Errors**).
2.  **Systems Languages** (like C++ or Rust): Extremely fast and reliable, but difficult to learn and require writing complex code to manage computer memory.

Nikaia aims to combine the readability of a scripting language with the performance and safety of a systems language. The developer writes simple code that focuses on the logic (the "Happy Path"). The **Compiler** (the program that translates your code into machine-readable instructions) automatically handles the complex technical details in the background.

### 1.2. One Language, Two Profiles
Nikaia uses a unique concept called **"Unified Core Architecture."** The same code can be compiled in two different ways, depending on what the software needs to do. These modes are called **Profiles**.

#### A. Nikaia Lite (The Default)
* **Purpose:** Building network services (like web servers) or programs that move files and data.
* **Behavior:** It uses a **Single-Threaded** architecture. This means the program performs tasks one after another extremely quickly, without the complexity of managing multiple parallel processes manually.
* **Safety:** Because it runs on a single thread, **Race Conditions** (errors where two processes try to modify the same data at the same time) are impossible by design.

#### B. Nikaia Advanced (formerly "Standard")
* **Purpose:** Heavy computations (like image processing, scientific calculations, or game engines).
* **Behavior:** It uses a **Multi-Threaded** architecture. The program splits tasks across all available processor cores to run them simultaneously.
* **Safety:** The compiler enforces strict mathematical rules to ensure data is not corrupted when accessed by multiple threads.

*Note: This document (Part I) focuses on the core language features available in **Nikaia Lite**.*

---

## Chapter 2: Variables and Data Types

### 2.1. Variables and Assignment
A **Variable** is a named storage location in memory that holds a value. In Nikaia, variables are declared using the `let` keyword.

**Immutability**
By default, variables are **Immutable**. This means once a value is assigned to a name, it cannot be changed. This prevents accidental modification of data.

```nika
let x = 10
// x = 20  <-- This would cause a Compiler Error
```

**Mutability**
To allow a variable to change, you must explicitly mark it as **Mutable** using the keyword `mut`.

```nika
let mut y = 10
y = 20     // This is allowed
```

### 2.2. Primitive Data Types
Nikaia provides basic types to represent simple values.

* **Integers:** Whole numbers without fractions.
    * `i32`: A standard integer (32-bit). Used for most numbers.
    * `i64`: A large integer (64-bit). Used for very large numbers.
* **Floats:** Numbers with decimal points.
    * `f64`: Double precision floating-point number. A literal may carry an
      **exponent** — `1.5e-4`, `2e3`, `9.54791938424326609e-04` — which is how a
      program about physical quantities is written; the same number spelled out
      in zeroes is how a digit gets lost.
* **Booleans:** Logic values.
    * `bool`: Can only be `true` or `false`.
* **Text:**
    * `String`: A piece of text that can be modified and owns its memory.
    * `&str`: A "String Slice". A read-only view into an existing string.
    * `char`: One character — a Unicode scalar value, not a byte. Written
      between single quotes, with the escapes a string uses: `'a'`, `'\n'`,
      `'\''`. It is what iterating a text yields (`for c in name.chars()`) and
      what a `match` over one compares against (3.4); a text is not a list of
      `char`, and turning one into the other is a decision a program makes
      rather than something that happens to it.

### 2.3. Nullable Types (Null Safety)
In Nikaia, types are **non-nullable** by default. A variable of type `String` must always contain a string and cannot be `null`. To allow the absence of a value, the type must be explicitly marked with a trailing question mark `?`.

```nika
let strictly_string: String = "Hello".to_string()
// strictly_string = null // Error!

let mut maybe_string: &str? = null // Valid
maybe_string = "World"             // Valid (`mut`, as in 2.1)
```

A literal is a **view** of text the program was compiled with, not a `String` — see 6.6, where
an allocation happens only where you wrote that you wanted one, and [ADR-024](adr/adr-024.md) D5.
`.to_string()` is how you say you want one.

### 2.4. Type Inference
Nikaia is **Statically Typed**, meaning the type of every variable is known at compile time. However, you rarely need to write types manually. The compiler uses **Type Inference** to deduce the type based on the value.

```nika
let name = "Nikaia"  // Compiler knows this is a &str - a view of static text
let count = 42       // Compiler knows this is an i32
```

### 2.5. String Interpolation
A string may contain **holes**: `{` and `}` around an expression, whose value is written where
the hole is.

```nika
let name = "Nikaia"
println("hello, {name} - {name.len()} characters")
```

A hole holds an expression, not just a name: a field, a call, an index. What follows a `:` inside
one says *how* to write the value rather than which value — the first colon that is not inside a
call or an index separates the two, so `Point(x: 1)` in a hole keeps its own.

Two braces stand for one: `{{` is a literal `{` and `}}` a literal `}`. An escape is not a hole —
the `{` in `"\u{0041}"` belongs to the escape — and a `}` on its own is an error rather than a
guess.

---

## Chapter 3: Control Flow

Control flow determines the order in which individual statements, instructions, or function calls are executed.

### 3.1. Expressions and Blocks
Nikaia is an **Expression-Oriented Language**. This means almost every construct returns a value.
A **Block** is a group of statements surrounded by curly braces `{ ... }`. The last line in a block (without a semicolon) is the return value of that block.

```nika
let result = {
    let a = 5
    let b = 10
    a + b  // Returns 15
}
```

### 3.2. Conditional Logic (if / else)
The `if` expression checks a condition (a `bool`). If true, it executes the first block; otherwise, it executes the `else` block. Since `if` is an expression, it can be assigned to a variable.

```nika
let age = 18

let status = if age >= 18 {
    "Adult"
} else {
    "Minor"
}
```

### 3.3. Loops
Loops allow code to be repeated.

**The `while` Loop**
Repeats code as long as a condition is true.

```nika
let mut count = 0
while count < 5 {
    println("Count is {count}")
    count += 1
}
```

**The `for` Loop**
Iterates over a sequence (like a range of numbers or a list).

```nika
// Iterates from 0 to 4 (5 is excluded)
for i in 0..5 {
    println("Index: {i}")
}
```

`a..b` excludes its end and `a..=b` includes it. A range is an ordinary
expression — it may be given a name, passed, or indexed with — and it binds
**looser than every operator in it**, so `0..n - 1` is a range ending at
`n - 1` rather than a range with something subtracted from it. That is the
reading a loop head wants and the only one that is ever useful.

**A loop can fail.** Some things a `for` walks are read *as it goes* — standard
input's lines are the one `std` has today. Getting the next one is real work,
and real work can fail. When it does, the loop stops and **the failure leaves
the function**, exactly as a failing call would (Chapter 7), and the compiler
makes you declare it:

```nika
fn tally() -> i64 throws {           // without `throws`: error[NK2701]
    let mut n = 0
    for line in io::lines() { n += 1 }
    return n
}
```

Nothing marks the loop, for the reason nothing marks a call that can fail
([ADR-023](adr/adr-023.md) D8) — and this is the same rule 6.4 already applies
to the *end* of a block, where a resource's cleanup can fail and the function
that owns it has to say so. One rule, two places the language calls something
you did not write ([ADR-025](adr/adr-025.md) D1).

What this rule exists to prevent is the alternative: a failed read that looks
like the end of the input, so a truncated stream becomes a shorter one and the
count is quietly wrong. 6.4 calls that "a decades-old bug class in other
languages", and it is the same bug at the other end of the block.

Most loops cannot fail. A range, a list, a map: nothing is read, so nothing
about them changes.

### 3.4. Pattern Matching (`match`)
The `match` expression compares a value against a series of patterns. It is similar to a "switch" statement in other languages but ensures that every possible case is handled.

```nika
let value = 2

match value {
    1 => println("One"),
    2 => println("Two"),
    _ => println("Something else"), // '_' catches all other values
}
```

A pattern is one of six things, and each is read the way it is written:

| pattern | matches |
| :--- | :--- |
| `_` | anything, and binds nothing |
| `1`, `"text"`, `true`, `'n'` | that value |
| `Op::Times` | that variant |
| `Message::Write(text)` | that variant, binding what it carries |
| `Message::Move { x, y }` | that variant, binding its fields by name |
| `other` | anything, and **binds it** to that name |

The last two lines are one rule: a path with `::` in it names a variant, and a
bare name binds. That is the same line the enum's own syntax draws — `Quit` is
a variant *of* `Message`, never on its own — so a pattern never has to be read
twice to see which of the two it is.

An arm's body is an expression or a block:

```nika
match step.0 {
    Op::Times  => { value = value * step.1 }
    Op::Divide => { value = value / step.1 }
}
```

### 3.5. Null Safety Operators
Accessing members of a Nullable Type requires handling the potential `null` case.

* **Safe Navigation (`?.`):** Accesses a member only if the receiver is not null. If it is null, the expression short-circuits to `null`.
* **Null Coalescing (`??`):** Provides a fallback value when an expression evaluates to `null`.

```nika
// If find_user returns null, 'name' becomes null.
let name = repo.find_user(id)?.full_name

// If 'name' is null, "Guest" is assigned.
let display_name = name ?? "Guest"
```

---

## Chapter 4: Data Structures

### 4.1. Structs (Custom Data Types)
A **Struct** allows you to group related values together under a single name.

**Visibility and Encapsulation**
In Nikaia, **everything is Private by Default**. This includes Structs and their Fields.
* To make a Struct usable by other modules, you must mark it `pub`.
* Even if a Struct is public, its fields remain private unless explicitly marked `pub`.

```nika
// file: users.nika

// The Struct is public, but fields are private
pub struct User {
    username: String,
    email: String,
    is_active: bool,
}
```

### 4.2. Constructors and Instantiation
Because fields are private by default, you often cannot initialize a struct directly from another module using the standard `Type(field: value)` syntax. You must provide a public **Constructor**.

**The Anonymous Constructor (`pub fn`)**
Nikaia allows you to define a special function inside an `impl` block that has no name. This function is automatically called when you invoke the Type name like a function `User(...)`.

* **Internal Access:** Inside the `users.nika` module, code can access private fields to build the object using the standard Struct Literal syntax.
* **External Access:** Outside modules utilize the public anonymous constructor.

```nika
// file: users.nika
impl User {
    // The Constructor
    // It accepts positional arguments (Subject Zone)
    pub fn(username: String, email: String) -> User {
        // We can access private fields here because we are inside the module
        return User(
            username: username,
            email: email,
            is_active: true // Default logic handled internally
        )
    }
}
```

**Usage Example**
External modules see `User` as a factory function.

```nika
// file: main.nika
use users::User

fn main() {
    // ERROR: Private Fields
    // Direct struct initialization is forbidden because fields are private.
    // let u = User(username: "A", email: "a@b.com", is_active: true)

    // OK: Public Factory Constructor
    // Calls the 'pub fn' defined in 'impl User'.
    // Note: Uses positional arguments as per Function Syntax.
    let u = User("Alice", "alice@example.com")
}
```

### 4.3. Why No Classes? (Data vs. Behavior)
Nikaia does not use **Classes** (a concept from Object-Oriented Programming). Instead, Nikaia separates them:
1.  **Structs** define the **Data** (what it is).
2.  **Impl Blocks** define the **Behavior** (what it does).

```nika
// Defining behavior for the User struct
impl User {
    fn login(&self) {
        println("{self.username} logged in.")
    }
}
```

### 4.4. Enums (Algebraic Data Types)
An **Enum** (Enumeration) is a type that can be one of several distinct variants.

```nika
enum Message {
    Quit,                        // carries nothing
    Move { x: i32, y: i32 },     // named fields, read by name
    Write(String),               // positional, read by position
}
```

Use one where a value is **one of a fixed set of things**: two operators, four
directions, the three states a connection can be in. A string would say the
same and check nothing — `"tiems"` is a value a string type accepts and an enum
does not. Reading one back is `match` (3.4), which is where the fixed set pays:
an arm per variant and no arm for a case that cannot happen.

### 4.5. Collections
Nikaia includes built-in types for storing groups of data.

* **List (Vector):** An ordered sequence of elements.
    ```nika
    let numbers = [1, 2, 3, 4]
    ```
    A list keeps the order it was given, and that order can be changed:
    `xs.sort()` puts the elements in their natural order, `xs.sort_by_key fn { … }`
    in the order of whatever the closure returns. **Both are stable** — elements
    the key does not separate keep the order they had — which is what makes two
    passes say a compound order without a comparator:
    ```nika
    names.sort()                                  // by name
    names.sort_by_key fn { -report[a].hits }         // then by hits, descending
    ```
    That matters because of the line below: a map has no order to borrow, so a
    program that prints one says which.
* **Tuple:** A fixed number of values of *different* types, with no name for
    the group and no names for the parts. Written and read by position:
    ```nika
    let pair = ("*", 3)          // (&str, i64)
    let op = pair.0
    ```
    A tuple is the answer when a pair of values belongs together for one step of
    a computation and naming it would be a struct pretending to be a type — the
    element of a grammar rule that yields an operator and its operand, the two
    halves of a map entry in `for (name, value) in map`. Where the group has a
    meaning that outlives the step, it wants a struct (4.1) and its fields want
    names.
* **Map (HashMap):** Stores key-value pairs.
    ```nika
    use std::collections::HashMap
    let mut scores = HashMap::new()
    scores["Player1"] = 100
    ```
    A map is hashed according to where its keys came from: keys derived from data a remote peer supplied are hashed with a random per-run key, so nobody can pick keys that make your program crawl, and keys from data you supplied are hashed with the fast function. You do not configure this and, in the ordinary case, you do not think about it — see Part III, 17.1, for the cases where you want the last word. One consequence is worth remembering here: **the order you get when iterating a map is not guaranteed** and may differ between runs.
### 4.6. Generics (Type Parameters)
To avoid writing the same code for different data types, Nikaia uses **Generics**. You define a type parameter inside square brackets `[...]`.

```nika
struct Box[T] {
    item: T,
}
```

### 4.7. Traits (Defining Behavior)
A **Trait** defines a set of behaviors (methods) that different types can share.

```nika
trait Summarize {
    fn summary(&self) -> String
}

impl Summarize for User {
    fn summary(&self) -> String {
        return "User: " + self.username
    }
}
```

---

## Chapter 5: Functions & Argument Architecture

### 5.1. The "Subject ; Config" Protocol
Nikaia enforces a strict separation between data (subjects) and configuration options to maximize readability. This is achieved via a dedicated **Semicolon Separator (`;`)** in function signatures.

**Zone 1: Subject (Positional)**
Arguments *before* the semicolon are the data the function operates on.
* **Syntactic Rule:** Positional arguments are allowed here.

**Zone 2: Configuration (Named Only)**
Arguments *after* the semicolon are options, flags, or modifiers.
* **Syntactic Rule:** Arguments here *must* be named. Positional usage is forbidden.

```nika
// Definition
fn request(url: &str; timeout: i32 = 30, method: &str = "GET") { ... }

// Valid calls
request("https://api.com")                              // both options defaulted
request("https://api.com"; timeout: 60)                 // one of them named
request("https://api.com"; method: "POST", timeout: 5)  // in any order

// Invalid calls
// request("https://api.com", 60)      // a positional argument in the named zone
// request("https://api.com"; timout: 5)  // error[NK1109]: no option `timout`
```

**Every configuration parameter has a default**, and that is what makes it an
*option*: a caller may leave it out, and leaving it out is never a question
about what the value is. A parameter that has to be passed belongs before the
`;`. Writing one without a default is a parse error that says so.

**A default is a literal.** An option's default is a constant in every program
anyone writes, and an arbitrary expression would raise a question with no
obvious answer — whether it is evaluated where the function is declared or where
it is called. That is worth deciding when something needs it.

**Order is the declaration's**, not the call's: `method` written first above is
still passed second, because only the declaration knows what the order is. This
is also why the ledger records an option's name, type *and* default (Part III,
13.5) — a call that leaves an option out still passes a value, and a consumer
compiling against a library cannot work out which.

**Optional Parentheses**
For functions defined without configuration or arguments, parentheses may be omitted to match the block lambda style.

```nika
fn init { 
    // No args, no parentheses required
}
```

### 5.2. There Is One Lambda Form
Earlier drafts had a second one, `fn: expression`, for single-line logic. It is **removed**
([ADR-022](adr/adr-022.md)), and 5.3 is the whole of what a lambda looks like.

It was four characters shorter than the block and cost three things. It did not grow: a body that
gained a second line had to change *form* rather than gain a line, which the flagship example had
already had to do. It was a second way to write the same thing, so every reader had to know when
to use which. And its body ran to the end of the expression, so a `.method()` chained after it
landed **inside** the lambda — silently, with no error and a different program.

Writing it today is an error that says so, because the form was in this specification and someone
will have it in their fingers.

### 5.3. Lambdas (`fn { ... }`)
When logic requires multiple steps, use a Block Lambda. You can choose between implicit arguments (for speed) or explicit arguments (for clarity).

**Option A: Implicit Arguments (The Default)**
Use this for short blocks where context is obvious.
* **Syntax:** `fn { ... }`
* **Args:** `a` (1st), `b` (2nd), `c` (3rd)
* **The three names belong to the lambda.** A body that binds one of them —
  `let c = …` — is asking for a third argument rather than shadowing anything,
  because how many arguments the lambda takes is read off which of the names
  its body mentions. Where a local wants one of those names, use **Option B**
  and name the arguments.

```nika
let complex = users.map fn {
    let bonus = calculate_bonus(a)
    // Implicit return of the last line
    a.score + bonus
}
```

**Option B: Explicit Arguments**
Use this when you need specific names (e.g., nested closures) or types.
* **Syntax:** `fn(name) { ... }`
* **Note:** This disables the implicit `a` and `b`.

**Trailing Syntax**
A lambda that is the last argument may go *outside* the parentheses, and where there are no other
arguments the parentheses go away with it:

```nika
let ids = users.map fn { a.id }
let sum = numbers.reduce(0) fn { a + b }
```

A block ends at its `}`, so a chain continues after it and means what it reads as:

```nika
Server::new()
    .route("/x") fn { handler(db) }
    .listen(":8080")
```

```nika
// Explicit naming for better readability
users.map fn(user) {
    if user.is_guest() {
        return "Guest"
    }
    return user.name
}
```
### 5.4. Contextual Capture (The Lifecycle Rule)
Nikaia simplifies memory management in closures by automatically inferring whether to Borrow or Move variables based on the context in which the lambda is used. This behavior is consistent across both Lite (Event Loop) and Advanced (Multi-Threaded) profiles.

#### A. Immediate Context (`@immediate`)
If a function guarantees that the callback will be executed and finished before the function itself returns, it is an **Immediate Context**.
* **Behavior:** Implicit Borrow (`&T`).
* **Examples:** `map`, `filter`, `for_each`, `sort_by`.

```nika
let prefix = "User: "
let names = ["Alice", "Bob"]

// 'map' is @immediate. It executes completely within this stack frame.
// 'prefix' is implicitly borrowed.
let formatted = names.map fn { prefix + a } 

// 'prefix' is still valid here because it was only borrowed.
println(prefix)
```

#### B. Detached Context (`@detached`)
If a function stores the callback, executes it later, or sends it to another thread/task, it is a **Detached Context**.
* **Behavior:** Implicit Move (Ownership Transfer).
* **Examples:** `spawn`, `defer`, `set_timeout`, `channel.on_receive`.

```nika
let prefix = "Log: "

// 'spawn' is @detached. The lambda might outlive the current function.
// 'prefix' is implicitly moved into the background task to ensure safety.
spawn fn { println(prefix + "System started") }

// Compiler Error: 'prefix' has been moved!
// println(prefix)
```

#### C. Constraint Propagation (The Viral Rule)

The distinction between immediate and detached is part of the function's type signature.
* By default, function parameters accepting lambdas `fn()` are Immediate.
* To accept a lambda that will be stored or spawned, you must explicitly mark the parameter as `@detached`.
* **Safety Rule:** You cannot pass an immediate lambda to a detached parameter.

```nika
// Custom function wrapper for spawning
fn launch_task(task: @detached fn()) {
    // Valid: 'task' is marked detached, so we can pass it to 'spawn'
    spawn(task)
}
```

---

## Chapter 6: Memory and Ownership

Memory management is usually either manual (hard) or automatic via Garbage Collection (slow). Nikaia uses a third way: **Ownership and Borrowing**, handled by the compiler.

### 6.1. The Concept of Scope
When a variable goes out of **Scope** (usually at the end of the block `{}` where it was created), Nikaia automatically cleans up the memory. You do not need to free memory manually.

### 6.2. Unified Types
To make coding easier, Nikaia provides smart types that handle memory logic for you, adapting to whether you are in Lite or Advanced mode.

* **`Shared[T]`**: Allows data to be owned by multiple parts of the program. The memory is only cleaned up when the *last* owner is finished.
* **`Locked[T]`**: Allows data inside a `Shared` container to be modified (mutated). It acts as a gatekeeper to ensure safety.

### 6.3. The Access Pattern
To modify data inside a `Locked` container, you must use the `.access()` method. This ensures that you have exclusive permission to change the data.

```nika
let data: Shared[Locked[i32]] = ...

// Uses short syntax where 'a' is the locked value
data.access fn { a += 1 }
```

### 6.4. Resource Cleanup (RAII)
Since Nikaia does not use a Garbage Collector, resources must be cleaned up deterministically. Nikaia follows the **RAII** principle (Resource Acquisition Is Initialization).

**Automatic Destruction**
When a variable goes out of scope (usually at the closing brace `}`), Nikaia automatically frees its memory.

**Custom Cleanup (`impl Drop`)**
If your struct manages external resources (like File Handles, Sockets, or C-Pointers), you can implement the `Drop` trait. The `drop` method is called automatically when the object is destroyed. `drop` is **synchronous**: it runs straight through and can never pause — use it for teardown that is pure memory work or a cheap native call.

```nika
struct FileHandle {
    fd: i32
}

impl Drop for FileHandle {
    fn drop(&mut self) {
        println("Closing file descriptor...")
        // Native close call would go here
    }
}
```

**Cleanup That Needs I/O (`impl Cleanup`)**
Some resources cannot be torn down without doing real work: a buffered file must *flush* its remaining data to disk, a database transaction must *roll back*, a TLS connection wants to say goodbye over the network. All of that is I/O — and in Nikaia, I/O means the function may pause (Chapter 8). A synchronous `drop` cannot pause. For these resources, implement `Cleanup` instead:

```nika
impl Cleanup for BufferedFile {
    // Pausable teardown. May pause, may fail.
    // The compiler calls it automatically at the end of the scope —
    // on normal exit AND while an error is bubbling up.
    fn cleanup(&mut self) throws IoError {
        self.flush()
    }

    // Synchronous last resort. Must not pause, must not fail.
    // Runs after cleanup(), or alone if cleanup() cannot run
    // (see "When cleanup cannot run" below).
    fn drop(&mut self) {
        // release the handle — nothing that waits
    }
}
```

You never call `cleanup` yourself, and you cannot forget it — the compiler inserts the call at the end of the block, exactly like `drop`. The only visible difference: the end of the block becomes a place where the function may briefly pause (like any other I/O), and the truth about errors surfaces (next paragraph).

**Cleanup errors are real errors.** If closing a resource can fail, the function that owns it can fail — Nikaia does not hide this (silently losing data at close time is a decades-old bug class in other languages). If `cleanup` declares `throws IoError`, the surrounding function needs `throws IoError` too, and the compiler tells you precisely why:

```text
error[NK2601]: this function can fail because closing `f` can fail
  --> report.nika:2
   |
 2 |     let f = fs::create("report.txt")
   |         ^ `f` is a buffered file; writing its remaining data
   |           to disk at the end of this function can fail
   |
  help: declare the error:  fn save_report(text: String) throws IoError
  help: or handle it precisely by closing explicitly:
        f.close() catch { ... }
```

Two refinements:
* If a value dies **while an error is already bubbling up**, the cleanup error does not replace it — it is attached to the original error as a *secondary error* (see 7.1's automatic debug information).
* If you want to react to the close error specifically, call **`close()`** yourself — it consumes the resource, returns the error normally, and no implicit cleanup runs afterwards.

**One restriction, told straight:** a `sync` function can never pause — so a resource with a pausable `cleanup` must not go out of scope inside one. The compiler catches this (`NK2602`) and names the ways out: return the resource to your caller, close it before the `sync` part, or use a non-buffering variant.

**When `cleanup` cannot run.** If a task is *cancelled* (it lost a `select` race, or a supervisor restarts it), nobody can wait for its I/O. The runtime then adopts the pending `cleanup` runs and finishes them in the background before the program exits ("parked cleanup" — bounded by the `cleanup-deadline`, Part III 13.3). Only a **panic** gets no pausable cleanup: during panic teardown only the synchronous `drop` fallback runs — and in the **Lite profile, a panic aborts the process immediately, so no destructors run at all** (Part III, Appendix A). What *does* still run on every panic is the **Panic Hook** (7.2) — your registered last-moment handler for dumps and crash reports. Panics are for unrecoverable bugs; recoverable failures use `throws`, where full cleanup is guaranteed.

> **Design Note: Why no `defer`?**
> Unlike languages like Go or Zig, Nikaia does not need a `defer` keyword.
> 1.  **Scope-Bound:** Cleanup happens automatically at the end of the block via `Drop`. You cannot forget it.
> 2.  **Safety:** Patterns like `.access()` for locks guarantee that resources are released, replacing manual `lock/defer unlock` sequences.
> 3.  **Unwinding:** If an error occurs (`throws`), the stack unwinds and triggers `drop` for all variables in the scope, ensuring no resource leaks even during failures.

### 6.5. References and Borrowing

Sometimes a function only needs to *look at* a value, not own it. For this, Nikaia has **References** (written `&T`, or `&str` for text). Think of it as lending a book: the owner keeps the book, the borrower may read it, and the loan ends automatically.

**The headline guarantee:**

> **Nikaia source code contains no lifetime annotations. Ever.**
> There is no syntax for them, and there never will be. Everything described below happens inside the compiler. (See [ADR-005](adr/adr-005.md).)

Two things are guaranteed to *just work*:

1.  **Borrowing inside a function**, even across I/O. Because Nikaia is async by default, a function may pause at any I/O call — and a borrowed value simply stays valid across that pause:

    ```nika
    fn report(config: &Config) {
        let name = &config.name       // borrow
        let data = fs::read("log")    // the function pauses here (I/O)...
        println("{name}: {data}")     // ...and the borrow is still valid.
    }
    ```

2.  **Returning borrowed values from functions.** A function like `fn first_word(s: &str) -> &str` needs no annotations. Even when the result could come from *several* inputs, the compiler figures out the connection on its own — across function boundaries, through your whole program (see 6.7).

**The one rule you need to know:** a borrow may not outlive its owner. You will rarely be able to break this rule by accident, because of the next section.

### 6.6. Escaping References Are Tethered

What happens when a borrowed value is not just used, but **escapes** — returned past the scope that owns the buffer, captured by a `@detached` lambda, or put into a collection that lives longer than the buffer? In most systems languages this is where the pain starts, because the compiler must prove the owner lives long enough.

Nikaia takes a different route. The rule is:

> **Transient = borrow. Escaping = tether. Copying = yours to ask for.**

Every view (`&str`, `&[u8]`) is in one of three states, and the compiler picks the cheapest one that works. You never write these states:

| State | What it is | Cost |
| :--- | :--- | :--- |
| **Borrowed** | a plain reference into the buffer | nothing at all |
| **Tethered** | a handle on the buffer plus a position | one shared handle per *container* — no copy, no allocation |
| **Owned** | a `String` of its own | one allocation — **only** where you wrote `.to_owned()` |

The effect: **the buffer cannot die while anything still points into it.** Instead of an error telling you "you may not use this after the buffer is gone," the language guarantees the buffer stays alive — deterministically, by reference counting, with no garbage collector involved.

Two things about this are worth knowing, because they are what make it usable on large data:

* **Storing a slice in a struct is not, by itself, an escape.** A struct that is built and consumed inside the scope that owns the buffer keeps plain references. A parser loop that builds a hundred million small records pays *nothing* for them.
* **The handle sits on the container, not on every slice.** When a map full of slices outlives its buffer, the *map* holds one handle; its keys stay positions. Filling it costs no handle traffic at all.

```nika
struct Token {
    text: &str,   // a view into someone else's buffer
}

fn tokenize(source: String) -> List[Token] {
    // The returned tokens outlive `source`'s scope, so the list is tethered:
    // it keeps `source` alive. No annotations, no copies of the text,
    // no dangling references — and one handle for the whole list.
    ...
}
```

Because of this, you will never see a "struct with lifetime parameters" in Nikaia — that concept does not exist in the language.

**When you want the guarantee in writing.** In a hot loop you may want to be *told* if a value ever starts tethering rather than borrowing. Mark the struct `@borrowed`:

```nika
@borrowed
struct Reading { name: &str, temp: i32 }
```

Nothing about the program changes — except that an escape is now a compile error that names the place where it happens. `nikaia explain --tethers` prints the state of every view without changing anything at all.

**The one case that is an error.** If a slice escapes and its buffer cannot be shared — it lives on the stack, or came from a foreign library — no tether is possible. The compiler says so and offers the ways out, `.to_owned()` among them. It never inserts that copy on your behalf: an invisible copy in a loop over a billion rows is exactly the kind of surprise Nikaia refuses to produce. (Details: [ADR-008](adr/adr-008.md).)

**One honest cost.** A tether keeps the *whole* buffer alive, not just the part you pointed at. Keeping one short name out of a 13 GB memory-mapped file pins all 13 GB. Where that looks like a mistake the compiler warns and suggests `.to_owned()`.

### 6.7. The Borrow Contract Ledger

For every function whose signature involves borrows, the compiler infers a **Borrow Contract**: a note such as "the result of `longest(a, b)` borrows from `a` or `b`." You never write these contracts. They are stored in a generated file, **`nikaia.contracts`**, which is committed alongside `nikaia.lock` (details in Part III, Chapter 13.5).

The ledger has two jobs:

1.  **Cache:** if a contract did not change, none of the function's callers need to be re-checked. Builds stay fast.
2.  **Explanation:** if you edit a function *body* and that changes its contract, the compiler compares old and new contract. If a caller elsewhere breaks, the error does not point at some mysterious distant line — it tells the whole story: *what you changed, how the contract changed, and which caller is affected* — plus what to do about it.

### 6.8. When the Compiler Says No

Ownership rules occasionally reject code — usually because the code contains a genuine bug. Nikaia's promise ([ADR-005](adr/adr-005.md), D7): **every such error explains itself in plain language and tells you what to do next.** You never need Rust knowledge to read a Nikaia error; if a raw internal (Rust) error ever reaches you, that is a Nikaia bug — please report it.

The most common case is changing a collection while looping over it:

```nika
let mut users = load_users()
for u in users {
    if u.is_duplicate() {
        users.remove(u)   // bug: pulling the rug out from under the loop
    }
}
```

```text
error[NK2301]: cannot change `users` while looping over it
  --> main.nika:4
   |
 4 |         users.remove(u)
   |         ^^^^^^^^^^^^^^^ the loop is still reading `users`
   |
  note: removing items mid-loop would invalidate the loop's position
        (this is a crash or silent bug in most languages)
  help: use the built-in method that does this safely:
        users.retain fn { !a.is_duplicate() }
```

For every known pattern of this kind, the standard library provides a safe, named method (`retain`, `drain`, `entry`, `swap(i, j)`, …) and the error message points directly at it.

---

## Chapter 7: Error Handling

Failures are a part of software. Nikaia distinguishes between two types of errors.

### 7.1. Recoverable Errors (`throws`)

These are expected problems: a file is missing, a connection drops, an input does not fit the
format. A function that can fail says so with `throws`.

```nika
fn fetch_config() throws -> String {
    let file = fs::read("config.txt")   // can fail
    return net::send(file)              // can fail too
}
```

**`throws` names no types.** What a function can fail *with* follows from its body, so the compiler
infers it whole-program and writes it to `nikaia.contracts` (Part III, 13.5). Writing it into the
signature would mean maintaining derived truth by hand — the same reason a borrow relationship is
not written in the source ([ADR-005](adr/adr-005.md) D3, [ADR-023](adr/adr-023.md) D1).

**An error type is an `enum`.**

```nika
enum ConfigError {
    NotFound(Path),
    Unreadable(Path),
    BadSyntax { line: i64, expected: &str },
}

impl Error for ConfigError {
    fn message(&self) -> String {
        match self {
            ConfigError::NotFound(p)   => "no config at {p}"
            ConfigError::Unreadable(p) => "cannot read {p}"
            ConfigError::BadSyntax { line, expected } => "line {line}: expected {expected}"
        }
    }
}
```

An error **carries what belongs to it** — "not found" without the path is a message that costs a
question before it helps. An `enum` is what the language already has for one of a fixed set of
things (4.4), and a `match` over one is checked for completeness. No marker on the type is needed:
what is thrown must implement `Error`, and the `impl` line is where that is said.

**Raising: `throw`.**

```nika
if !fs::exists(path) {
    throw ConfigError::NotFound(path)
}
```

**Propagation happens on its own, and nothing marks it.** A call that can fail, inside a function
that declares `throws` — that is all of it. No operator, no sigil:

```nika
fn load() throws -> Config {
    let text = fetch_config()   // if it fails, `load` fails
    return parse(text)
}
```

That is deliberate, and it is the same decision as in three other places. Four things can leave the
control flow of a Nikaia program without the line showing it: a call may **pause** (8.1), a block's
end may **pause and fail** (6.4), a call may **fail** (here), and a loop's step may fail
([ADR-025](adr/adr-025.md)). Marking one of them would claim the other three were absent.

**Handling: `catch`.** The block supplies the replacement value — or it leaves the function.

```nika
let config = load() catch {
    eprintln("{error}")
    return                                // leaves the function
}

let port = read_port() catch { 8080 }     // replacement value
```

Inside the block the error is called **`error`**. To pass it on rather than handle it, throw it:
`throw error`.

**Telling failures apart.** `error` is the sum of the errors that can arrive at this point — the
compiler knows them because it inferred them. Match with the patterns of 3.4, where a path with
`::` names a variant:

```nika
let config = load() catch {
    match error {
        ConfigError::NotFound(p) => Config::default()
        ConfigError::BadSyntax { line, .. } => {
            eprintln("config broken at line {line}")
            return
        }
        _ => throw error
    }
}
```

Two sets, and they are not the same one. The **variants of an error type** are closed, a `match`
over them is exhaustive, and adding one is a breaking change — correctly. The **set of error types**
arriving at a `catch` is open, and it grows when a callee gains a failure. Where that changes a
`catch`, the compiler narrates the chain: what changed, which contract moved, which caller broke
(`NK2401`).

**What an error brings without anyone attaching it.** The place it was raised, the stack trace, and
the chain beneath it where another error joined on the way — a cleanup that failed while the stack
was unwinding is attached to the original as a *secondary* error rather than replacing it (6.4).

**Printing it: short is the default.**

```nika
eprintln("{error}")         // the message, and the mark if the application shows one
eprintln("{error:full}")    // plus the chain and the stack trace
```

`{error}` **never** prints the stack trace. An error message is written for the operator, not for
the visitor of a web page — which is why a failed HTTP handler answers with a generic 500 and logs
the rest ([ADR-018](adr/adr-018.md)). The form you type without thinking is the one you may show a
stranger.

So that a generic answer stays findable anyway, every error knows the **site that raised it** and
has a short form of it a person can read out. A screenshot carrying `(NK-2C7)` leads through
`nikaia explain NK-2C7` to the line that threw it — with no log file, and also when your working
tree is two features further along ([ADR-023](adr/adr-023.md) D6).

**Failure nobody writes down.** Two places perform a call you did not type, and both can fail: the
end of a block, where a resource is cleaned up (6.4, `NK2601`), and a loop's step over a fallible
stream ([ADR-025](adr/adr-025.md), `NK2701`). In both the enclosing function gains a `throws`, and
the compiler says which resource or which loop it was.

### 7.2. Unrecoverable Errors (`panic`)
These are logical bugs, like trying to access the 10th item in a list of 5 items. Nikaia stops the execution to prevent incorrect behavior. In **Nikaia Lite**, this aborts the process safely.

**The Panic Hook (`std::panic::on_panic`)**
"Abort" does not mean *no* code runs anymore — it means no normal cleanup runs. Before the process dies (Lite) or the crashed task is isolated (Advanced), Nikaia calls one last, registered function: the **Panic Hook**. This is the place for a crash dump, a crash report, or flushing a diagnostics log — so a crash in production never has to be a mystery.

```nika
use std::panic

fn main() {
    // Global, one per application. Set it early.
    // The hook must be 'sync' — mid-panic there is nothing to pause on.
    panic::on_panic fn(info) sync {
        // 'info' carries: message, file/line, and the stack trace.
        // Pattern: open crash resources at startup, only WRITE here.
        crash_log.write_report(info)
    }

    run_app()
}
```

The rules, told straight:
* **Global, application-only.** Exactly one hook per program, set by the application — a library calling `on_panic` is a compile error (`NK2604`). A crash-reporting library instead exports a function that your hook calls.
* **It runs on every panic, in both profiles** — in Lite right before the abort (before the trap on WASM), in Advanced before the task is poisoned. Supervisors (Part II, 12.8) receive their crash information from the same `info`.
* **Blocking is allowed here — briefly.** In Lite the process is ending anyway. In Advanced the program keeps running, so keep the hook short and hand heavy reporting to something you started earlier.
* **Diagnosis, not cleanup.** Do not try to flush buffered files or finish transactions from the hook — those objects may be broken in exactly the way that caused the panic. That is why panics skip destructors, and the hook does not reopen that door. If the hook itself panics, the process aborts immediately.

---

## Chapter 8: Concurrency (Doing things at the same time)

Even in **Nikaia Lite** (Single-Threaded), you can perform multiple tasks concurrently, such as waiting for a download while responding to user input. This is done using **Asynchronous Programming**.

### 8.1. Async by Default
In Nikaia, functions that perform Input/Output (I/O), like reading a file or downloading a URL, automatically "pause" execution without blocking the whole program. You do not need special keywords like `await`.

### 8.2. Spawning Tasks
To run a new independent task, use `spawn`. It takes a lambda containing the code to run — the
one lambda form (5.3), whose body is a block whether it holds one line or several.

```nika
spawn fn { println("I am running in the background!") }
```

### 8.3. Data Ownership in Tasks (Implicit Move)
A background task may keep running after the function that started it has already finished. It therefore cannot merely *borrow* variables — they might be gone by the time it runs. Following the Contextual Capture rules (Chapter 5.4), `spawn` is a **Detached Context**: variables used inside the task are **moved** into it automatically. There is no `move` keyword — the compiler applies the rule for you.

```nika
let message = "Hello"

// 'message' is implicitly moved into the task (spawn is @detached)
spawn fn { println(message) }

// Compiler Error: 'message' now belongs to the task.
// println(message)
```

If you still need the value afterwards, clone it first:

```nika
let message = "Hello"
spawn fn { println(message.clone()) }
println(message)   // OK: the task owns a copy
```

The compiler error for this situation explains exactly that:

```text
error[NK2101]: this background task takes ownership of `message`
  --> main.nika:3
   |
 3 | spawn fn { println(message) }
   |                   ^^^^^^^ moved into the task here
 4 | println(message)
   | --------------- but `message` is used again afterwards
   |
  note: a task started with `spawn` may outlive this function,
        so it cannot merely borrow your variables — it takes them with it
  help: keep using `message` here by giving the task its own copy:
        spawn fn { println(message.clone()) }
```

### 8.4. The Runtime Sidecar Model
While Nikaia Lite enforces a strict single-threaded model for user logic ("The Happy Path"), the Runtime employs a **Hidden Sidecar Pattern** to handle heavy I/O without blocking.

* **Separation of Concerns:** User code runs exclusively on the main thread (Event Loop). Heavy operations (like SQLite queries) are offloaded to a managed Runtime Sidecar (a background thread on Native, or a Web Worker on WASM).
* **Safety Guarantee:** Data exchange occurs via strict message passing (ownership transfer). Since user code never accesses the Sidecar memory directly, **Race Conditions** remain impossible.
* **Non-Blocking:** From the developer's perspective, a database call is simply an async yield point. The Runtime guarantees that the main loop never stalls waiting for disk I/O.

---

## Chapter 9: Project Organization and Visibility

### 9.1. Modules and Files
Every file in Nikaia (e.g., `utils.nika`) is implicitly a **Module**.

`use utils` brings one in, and it names the file `utils.nika` beside the
program's entry:

```nika
// file: main.nika
use utils

fn main() {
    println("{utils::double(21)}")
}
```

A module of your program is **one name**. `use std::fs` is the one `use` with a
path in it, and it names the library rather than a file
([ADR-030](adr/adr-030.md) D1).

Two files may import each other. Nothing about a module is an order of
declaration: the compiler reads the files the entry reaches, and what they say
about each other is the same however it got there.

### 9.2. Visibility Rules (Privacy)
Nikaia enforces strict encapsulation to prevent tight coupling between parts of your code.

1.  **Private by Default:**
    * Functions, Structs, Enums, and Constants are only visible inside the file they are defined in.
    * Struct Fields are only visible inside the file where the struct is defined.

2.  **The `pub` Keyword:**
    * To allow other modules to use an item, prefix it with `pub`.
    * To allow other modules to access a specific field of a struct, prefix the field with `pub`.
    * A **public type may keep its fields private**, and 9.3 is about why that
      is the ordinary case rather than an inconvenience.

Reaching a private item from another file is `NK1110`, and says which file
keeps it:

```text
error[NK1110]: `secret` is private to `utils.nika`
   4 |     let n = utils::secret()
           ^
     = an item is private to the file that declares it unless it says `pub` (Part I, 9.2)
     help: write `pub fn secret` in `utils.nika`, or reach it through something that is public
```

### 9.3. Granular Control
While `pub` makes an item available generally, strict privacy forces developers to create safe interfaces (Constructors and Methods) rather than exposing raw data.

```nika
// file: network.nika

// Private: Only usable inside network.nika
struct Config {
    port: i32
}

// Public: Usable by anyone
pub struct Server {
    // Private field: Can only be changed by Server methods
    config: Config,
    
    // Public field: Can be read/written by anyone
    pub name: String 
}
```

