# Nikaia Language Specification
**Part I: The Language Core & Nikaia Lite**
**Version:** 0.0.5 (Draft)
**Date:** January 22, 2026

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

### 2.3. Nullable Types (Null Safety)
In Nikaia, types are **non-nullable** by default. A variable of type `String` must always contain a string and cannot be `null`. To allow the absence of a value, the type must be explicitly marked with a trailing question mark `?`.

```nika
let strictly_string: String = "Hello"
// strictly_string = null // Error!

let maybe_string: String? = null // Valid
maybe_string = "World"           // Valid
```

### 2.4. Type Inference
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
    Quit,
    Move { x: i32, y: i32 },
    Write(String),
}
```

### 4.5. Collections
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
fn request(url: String; timeout: i32 = 30, method: String = "GET") { ... }

// Valid Calls
request("[https://api.com](https://api.com)"; timeout: 60)

// Invalid Calls (Compiler Errors)
// request("[https://api.com](https://api.com)", 60)         // Error: Positional arg in named zone
```

**Optional Parentheses**
For functions defined without configuration or arguments, parentheses may be omitted to match the block lambda style.

```nika
fn init { 
    // No args, no parentheses required
}
```

### 5.2. Expression Lambdas (The `fn:` Shorthand)
For concise, single-line logic, use the `fn:` syntax.
* **Implicit Arguments:** `a`, `b`, `c` are automatically available.
* **Implicit Return:** The result of the expression is returned.

**Trailing Syntax**
If a `fn:` expression is the last argument, parentheses can be omitted.

```nika
// Cleanest Syntax: No parentheses required
let ids = users.map fn: a.id

// With other arguments
let sum = numbers.reduce(0) fn: a + b
```

### 5.3. Block Lambdas (`fn { ... }`)
When logic requires multiple steps, use a Block Lambda. You can choose between implicit arguments (for speed) or explicit arguments (for clarity).

**Option A: Implicit Arguments (The Default)**
Use this for short blocks where context is obvious.
* **Syntax:** `fn { ... }`
* **Args:** `a` (1st), `b` (2nd)...

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

```nika
// Explicit naming for better readability
users.map fn(user) {
    if user.is_guest() {
        return "Guest"
    }
    return user.name
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
data.access fn: a += 1
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

**Handling Errors (`catch`)**
To handle an error, use the `catch` keyword. Inside the catch block, the error is available for inspection.

```nika
let content = fetch_config() catch {
    println("Failed to fetch")
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
To run a new independent task, use `spawn`. It takes an **Explicit Block Lambda** containing the code to run.

```nika
spawn fn: println("I am running in the background!")
```

### 8.3. Moving Data (`move`)
By default, closures only "borrow" variables (look at them). If a background task needs to take full **Ownership** of a variable (so it stays alive even after the main function ends), you must use the `move` keyword.

```nika
let message = "Hello"

// 'move' transfers the 'message' variable into the Lambda
spawn move fn: println(message)

// 'message' is no longer valid here
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

### 9.2. Visibility Rules (Privacy)
Nikaia enforces strict encapsulation to prevent tight coupling between parts of your code.

1.  **Private by Default:**
    * Functions, Structs, Enums, and Constants are only visible inside the file they are defined in.
    * Struct Fields are only visible inside the file where the struct is defined.

2.  **The `pub` Keyword:**
    * To allow other modules to use an item, prefix it with `pub`.
    * To allow other modules to access a specific field of a struct, prefix the field with `pub`.

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

