# Nikaia Language Specification
**Part II: Advanced Features & Metaprogramming**
**Version:** 0.0.5 (Draft)
**Date:** January 22, 2026

---

## Chapter 10: Metaprogramming (Code that writes Code)

Metaprogramming allows developers to extend the language itself. In Nikaia, this is not done with text replacements (like in C) but by manipulating the structure of the code (the **AST** or Abstract Syntax Tree) in a safe way.

### 10.1. Parsing with `grammar`
Before you can manipulate code, you often need to read custom data formats. The `grammar` tool allows you to write parsers easily.

```nika
// Defines a parser that turns strings like "#FF0000" into a Color struct
grammar ColorParser {
    option recursion_limit = 50;

    pub rule entry -> Color = {
        "#" r:hex() g:hex() b:hex()
    } -> {
        Color { r, g, b }
    }
    
    rule hex -> u8 = s:regex("[0-9A-Fa-f]{2}") -> { 
        u8::from_str_radix(s, 16)? 
    }
}
```

### 10.2. Dual-Mode Parsing (Static vs. Dynamic)
One of Nikaia's most powerful features is that a grammar defined once can be used in two completely different ways.

**A. Static Embedding (Compile-Time)**
You can use a parser to read files *during the build process*. If the file contains a syntax error, the compilation fails. The result is embedded directly into the binary as a strictly typed object.

```nika
// Validated at compile-time. 'theme' is a Color struct, not a Result.
const theme = ColorParser::parse(include_str("theme.hex")) !
```

**B. Dynamic Parsing (Runtime)**
The same grammar can be used to parse user input at runtime.

```nika
fn parse_input(input: String) -> Result<Color> {
    // Returns a Result because runtime input might be invalid
    return ColorParser::parse(input)
}
```

### 10.3. Code Generation (Quasi-Quoting)
While parsing reads data, **Macros** create new code. Nikaia uses a mechanism called **Quasi-Quoting**. This allows you to write code templates that look like normal Nikaia code, with placeholders for dynamic values.

**Example: Auto-Generating a "Describe" Function**
Imagine you want to automatically print all fields of a struct without writing the boilerplate manually.

```nika
// Definition of the Macro
pub macro Describe(def: StructDef) -> AstExpr {
    
    // We iterate over the struct fields to create a print statement for each.
    // The 'fn:' shorthand creates a lambda where 'a' is the current field.
    let print_statements = def.fields.map fn: quote {
        // We inject 'a.name' (the field name) as a string literal
        // and 'self.a.name' as the property access.
        println("Value of " + a.name + " is: " + self.a.name)
    }

    let name = def.name

    // Return the final implementation block
    quote {
        impl name {
            fn describe(&self) {
                print_statements // Injects the list of println calls here
            }
        }
    }
}

// Usage
struct User with Describe {
    name: String,
    age: i32
}

fn main() {
    // Implicit Anonymous Constructor:
    // Nikaia generates a controlled constructor. 
    // If validation logic exists, it is executed here transparently.
    let u = User("Alice", 30) 
    
    u.describe() 
}
```

### 10.4. Hygienic Macros
Nikaia macros are **Hygienic**. Variables defined inside a macro do not conflict with variables in the user's code, preventing accidental shadowing bugs.

### 10.5. Compile-Time I/O
Macros can access the file system (read-only) during compilation. This enables "Typed Assets."

```nika
// The macro reads the SQL file, checks syntax, and generates types 
// for the query result at compile time.
let query = sql::from_file("users.sql")
```

---

## Chapter 11: Nikaia Advanced Profile (The Compute Engine)

While **Nikaia Lite** is designed for I/O density on a single thread, **Nikaia Advanced** (`--profile=advanced`) is designed for **Parallel CPU Throughput** across multiple cores.

### 11.1. Implicit Async & The Scheduler
In both profiles, the syntax looks identical.
* **Lite:** Functions yield on I/O events (Cooperative Event Loop).
* **Advanced:** The runtime uses a **Work-Stealing Scheduler**. It automatically maps tasks (green threads) onto OS threads (worker pool).

### 11.2. Parallelism via `spawn`
To run tasks in **Parallel**, you use `spawn`.

**Implicit Move (Ownership Transfer)**
In Nikaia, spawning a task **always** implies transferring ownership of captured variables to the new task. There is no explicit `move` keyword required.
* **Safety:** This guarantees thread safety by default. The parent thread loses access to the data, preventing race conditions.
* **Cloning:** If you need to keep the data in the parent thread, you must explicitly call `.clone()` before spawning.

**Fault Isolation (Panic Boundaries)**
If a spawned task crashes (panics), **it does not kill the program**, nor does it kill the worker thread executing it. The panic is isolated to that specific task handle. This ensures server stability even if individual requests fail drastically.

```nika
fn process_image(path: String) -> Image { ... }

fn main() {
    let img_path = "a.jpg"

    // 1. Parallel Execution (Fork)
    // We use the 'fn:' shorthand here.
    // 'img_path' is IMPLICITLY moved into the task.
    // It is no longer accessible in 'main' after this line.
    let handle = spawn fn: process_image(img_path)

    // println(img_path) // <--- Compiler Error: Variable moved to task.

    // 2. Fault Isolation
    // If process_image crashes, 'handle.join()' returns an Error.
    // The main program continues running safely.
    let result = handle.join() catch {
        println("Task crashed, but the system is stable.")
        return
    }
}
```

---

## Chapter 12: Thread Safety and Synchronization

### 12.1. The `sync` Keyword (CPU Constraints)
The `sync` keyword marks a function as a **pure CPU task**, forbidding any I/O operations inside it. This helps the scheduler optimize thread usage and prevents blocking the event loop with hidden I/O.

```nika
// 'sync' guarantees: I will never pause, I will never do I/O.
fn calculate_physics(obj: Object) sync {
    obj.x += obj.velocity 
}

// Usage in Parallel Iterator
// par_iter distributes work across cores. 
// Uses 'fn:' syntax with implicit 'a' (the particle).
particles.par_iter().for_each fn: calculate_physics(a)
```

### 12.2. The Dual Nature of `Locked[T]`
To share mutable data, use `Locked[T]`.
* **Lite:** Protects against Logical Deadlocks (Reentrancy).
* **Advanced:** Uses OS Mutexes to protect against Memory Corruption.

### 12.3. Deadlock Prevention: Atomic Composition
Nesting locks manually is forbidden to prevent deadlocks. Instead, use `access_all` to request multiple resources simultaneously. The runtime sorts locks internally to ensure a consistent locking order.

```nika
let account_a: Shared[Locked[Account]] = ...
let account_b: Shared[Locked[Account]] = ...

// Atomic Locking
// We use explicit arguments 'fn(a, b)' for clarity.
// The runtime locks both accounts safely before entering the block.
access_all(account_a, account_b) fn(a, b) {
    // Both 'a' and 'b' are mutable guards here.
    let amount = 100
    a.balance -= amount
    b.balance += amount
}
```

### 12.4. Racing Tasks (`select`)
`select` allows waiting for the first of multiple tasks to complete. It uses a pattern matching syntax to handle the winner.

```nika
select {
    // Case 1: Computation finishes first
    // Note: Implicit Move applies to variables used in heavy_math
    result = heavy_math() => { return result }
    
    // Case 2: Timeout
    // Subject/Config Protocol: 'sleep' takes the duration as subject.
    _ = sleep(5.seconds()) => { throw TimeoutError("Too slow!") }
}
```

### 12.5. Channels (Message Passing)
Channels allow communication without shared memory. Nikaia encourages this pattern for loose coupling between threads.

```nika
// Subject: 100 (capacity)
// The Subject/Config protocol allows clean initialization.
let (tx, rx) = channel::bounded(100)

// Implicit Move transfers 'tx' into the background task automatically.
spawn fn {
    tx.send("Calculation complete")
}

// Waits for the message
let msg = rx.recv() 
```

### 12.6. Data Parallelism (`par_iter`)
Processing large datasets using all cores is a first-class citizen.

```nika
let pixels = [/* 1 million pixels */]

// 1. .map uses 'fn:' shorthand with implicit 'a'
// 2. .filter uses 'fn:' shorthand
// The compiler ensures that the lambdas passed here are pure (sync).
let bright_pixels = pixels.par_iter()
    .map(fn: a.brightness * 1.5)
    .filter(fn: a > 0.5)
    .collect()
```

### 12.7. Scoped Threads
**Scoped Threads** guarantee that threads finish *before* the current function ends. This allows the child threads to borrow variables from the stack without needing `move` or `clone`.

```nika
let data = [1, 2, 3]

// Syntax: Block Lambda with explicit argument 's' (the scope handle)
thread::scope fn(s) {
    // This thread BORROWS 'data' because the scope guarantees 
    // it won't outlive the function.
    s.spawn fn: println("Reading: {data}")
    
    // The scope blocks here until all inner spawns are done.
}
```

### 12.8. Supervision Trees
In complex systems, threads might crash. A **Supervisor** monitors tasks. 

```nika
// If the child task crashes, the Supervisor policy (Restart) applies.
// This is essential for long-running Advanced Profile applications.
supervisor::start_link(fn {
    server.run()
}, restart_policy: "always")
```
