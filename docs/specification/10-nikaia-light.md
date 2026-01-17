# Nikaia Language Specification
**Part I: The Language Core & Nikaia Lite**
**Version:** 0.0.4 (Educational Draft)
**Codename:** Vibe Coding Experiment
**Date:** January 16, 2026

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
    * `f64`: Double precision floating-point number.
* **Booleans:** Logic values.
    * `bool`: Can only be `true` or `false`.
* **Text:**
    * `String`: A piece of text that can be modified and owns its memory.
    * `&str`: A "String Slice". A read-only view into an existing string.

### 2.3. Type Inference
Nikaia is **Statically Typed**, meaning the type of every variable is known at compile time. However, you rarely need to write types manually. The compiler uses **Type Inference** to deduce the type based on the value.

```nika
let name = "Nikaia"  // Compiler knows this is a String
let count = 42       // Compiler knows this is an i32
```

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

---

## Chapter 4: Data Structures

### 4.1. Structs (Custom Data Types)
A **Struct** (short for Structure) allows you to group related values together under a single name. Each value inside a struct is called a **Field**.

```nika
struct User {
    username: String,
    email: String,
    active: bool,
}

// Creating an instance of the struct
let user1 = User {
    username: "Alice",
    email: "alice@example.com",
    active: true,
}

// Accessing fields
println(user1.username)
```

### 4.2. Why No Classes? (Data vs. Behavior)
Nikaia does not use **Classes** (a concept from Object-Oriented Programming). Classes often mix data storage with logic, and rely on **Inheritance** (creating hierarchies of objects), which can lead to complex and brittle code.

Instead, Nikaia separates them:
1.  **Structs** define the **Data** (what it is).
2.  **Impl Blocks** define the **Behavior** (what it does).

This approach, known as **Composition**, makes systems easier to understand and test.

```nika
// Defining behavior for the User struct
impl User {
    fn login(&self) {
        println("{self.username} logged in.")
    }
}
```

### 4.3. Enums (Algebraic Data Types)
An **Enum** (Enumeration) is a type that can be one of several distinct variants. Unlike simple lists of constants in other languages, Nikaia Enums can hold data specific to each variant.

```nika
enum Message {
    Quit,
    Move { x: i32, y: i32 },
    Write(String),
}
```

### 4.4. Collections
Nikaia includes built-in types for storing groups of data.

* **List (Vector):** An ordered sequence of elements.
    ```nika
    let numbers = [1, 2, 3, 4]
    ```
* **Map (HashMap):** Stores key-value pairs.
    ```nika
    use std::collections::HashMap
    let mut scores = HashMap::new()
    scores["Player1"] = 100
    ```
### 4.5. Generics (Type Parameters)
To avoid writing the same code for different data types, Nikaia uses **Generics**. You define a type parameter inside square brackets `[...]`.

```nika
// A generic wrapper that can hold any type 'T'
struct Box[T] {
    item: T,
}

// Usage
let int_box = Box { item: 42 }       // T is i32
let str_box = Box { item: "Hello" }  // T is String
```

**Generic Functions**
Functions can also be generic.

```nika
// Returns the item unchanged. Works for any type.
fn identity[T](item: T) -> T {
    return item
}
```
### 4.6. Traits (Defining Behavior)
A **Trait** defines a set of behaviors (methods) that different types can share. It is similar to an "Interface" in other languages.

```nika
// Defining the contract
trait Summarize {
    fn summary(&self) -> String
}

// Implementing the contract for a specific struct
impl Summarize for User {
    fn summary(&self) -> String {
        return "User: " + self.username
    }
}

// Using the trait as a constraint
// This function accepts ANY type that implements Summarize
fn print_summary(item: &Summarize) {
    println(item.summary())
}
```

---

## Chapter 5: Functions and Closures

### 5.1. Functions
A **Function** is a reusable block of code. It is declared with `fn`. Arguments must have types, and the return type is specified after `->`.

```nika
fn add(a: i32, b: i32) -> i32 {
    return a + b
}
```

### 5.2. Closures (Lambdas)
A **Closure** is an anonymous function (a function without a name) that can be passed as data. Nikaia uses modern **Arrow Syntax** (`=>`) to define them.

**A. Expression Lambdas**
Used for short, single-line operations. The result of the expression is automatically returned.
*Syntax:* `argument => expression`

```nika
let numbers = [1, 2, 3]
let doubled = numbers.map(x => x * 2)
```

**B. Standard Closures**
Used for multi-line logic. These use curly braces `{ ... }`.
*Syntax:* `(argument1, argument2) => { statements }`

```nika
numbers.for_each((index, n) => {
    let result = n * 10
    println("Index {index}: {result}")
})
```

**C. Block Lambdas (Syntactic Sugar)**
A special shorthand exists for closures that take **no arguments** (often used for background tasks or simple blocks).
If a closure has no arguments, you can omit the `() =>` entirely and just write the block `{ ... }`. This makes the code look cleaner.

* *Explicit:* `() => { do_work() }`
* *Block Lambda:* `{ do_work() }`

```nika
// These are identical:
spawn(() => { print("Working") }) 
spawn({ print("Working") })       // Cleaner Syntax
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

// Uses a closure to define the safe access area
data.access(guard => {
    guard += 1
})
```
### 6.4. Resource Cleanup (RAII)
Since Nikaia does not use a Garbage Collector, resources must be cleaned up deterministically. Nikaia follows the **RAII** principle (Resource Acquisition Is Initialization).

**Automatic Destruction**
When a variable goes out of scope (usually at the closing brace `}`), Nikaia automatically frees its memory.

**Custom Cleanup (`impl Drop`)**
If your struct manages external resources (like File Handles, Sockets, or C-Pointers), you can implement the `Drop` trait. The `drop` method is called automatically when the object is destroyed.

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

> **Design Note: Why no `defer`?**
> Unlike languages like Go or Zig, Nikaia does not need a `defer` keyword.
> 1.  **Scope-Bound:** Cleanup happens automatically at the end of the block via `Drop`. You cannot forget it.
> 2.  **Safety:** Patterns like `.access()` for locks guarantee that resources are released, replacing manual `lock/defer unlock` sequences.
> 3.  **Unwinding:** If an error occurs (`throws`), the stack unwinds and triggers `drop` for all variables in the scope, ensuring no resource leaks even during failures.

---

## Chapter 7: Error Handling

Failures are a part of software. Nikaia distinguishes between two types of errors.

### 7.1. Recoverable Errors (`throws`)
These are expected problems, like "File not found" or "Network disconnected". Functions that can fail must declare the possible error types in their signature using `throws`.

**Multiple Errors**
A function can define multiple types of errors it might produce.

```nika
// This function might throw an IoError OR a NetworkError
fn fetch_config() throws IoError, NetworkError -> String {
    let file = fs::read("config.txt") // might throw IoError
    return net::send(file)            // might throw NetworkError
}
```

**Automatic Debug Information**
When an error occurs, it "bubbles up" to the caller automatically. Nikaia automatically attaches rich debugging information to this error, including:
* The filename and line number where the error happened.
* The full **Stack Trace** (the history of function calls).
This happens invisibly, so you don't need to manually add context to every error.

**Handling Errors**
To handle an error, use the `?{ ... }` block. Inside this block, the variable `error` is available.

```nika
let content = fetch_config()?{
    println("Failed at {error.file}:{error.line}")
    println("Reason: {error}")
    return // Stop execution
}
```

### 7.2. Unrecoverable Errors (`panic`)
These are logical bugs, like trying to access the 10th item in a list of 5 items. Nikaia stops the execution to prevent incorrect behavior. In **Nikaia Lite**, this aborts the process safely.

---

## Chapter 8: Concurrency (Doing things at the same time)

Even in **Nikaia Lite** (Single-Threaded), you can perform multiple tasks concurrently, such as waiting for a download while responding to user input. This is done using **Asynchronous Programming**.

### 8.1. Async by Default
In Nikaia, functions that perform Input/Output (I/O), like reading a file or downloading a URL, automatically "pause" execution without blocking the whole program. You do not need special keywords like `await`.

### 8.2. Spawning Tasks
To run a new independent task, use `spawn`. It takes a **Block Lambda** (a closure with no arguments) containing the code to run.

```nika
spawn({
    println("I am running in the background!")
})
```

### 8.3. Moving Data (`move`)
By default, closures only "borrow" variables (look at them). If a background task needs to take full **Ownership** of a variable (so it stays alive even after the main function ends), you must use the `move` keyword.

```nika
let message = "Hello"

// 'move' transfers the 'message' variable into the Block Lambda
spawn(move {
    println(message)
})
// 'message' is no longer valid here
```

### 8.4. The Runtime Sidecar Model
While Nikaia Lite enforces a strict single-threaded model for user logic ("The Happy Path"), the Runtime employs a **Hidden Sidecar Pattern** to handle heavy I/O without blocking.

* **Separation of Concerns:** User code runs exclusively on the main thread (Event Loop). Heavy operations (like SQLite queries) are offloaded to a managed Runtime Sidecar (a background thread on Native, or a Web Worker on WASM).
* **Safety Guarantee:** Data exchange occurs via strict message passing (ownership transfer). Since user code never accesses the Sidecar memory directly, **Race Conditions** remain impossible.
* **Non-Blocking:** From the developer's perspective, a database call is simply an async yield point. The Runtime guarantees that the main loop never stalls waiting for disk I/O.

---

## Chapter 9: Project Organization

### 9.1. Modules
Code is organized into files. Each file is a **Module**.
* `main.nika`: The entry point of the program.
* `math.nika`: Can be imported as a module named `math`.

### 9.2. Imports (`use`)
To use code from other modules or libraries, use the `use` keyword.

```nika
use std::http
use my_project::utils

fn main() {
    http::Server::new()
}
```
