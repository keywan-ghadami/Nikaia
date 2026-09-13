# Nikaia Language Specification
**Part I: The Language Core**
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

### 1.2. One Language, and the Switches
There is one Nikaia. The same source compiles for every machine and at every
setting, and prints the same bytes. What you choose when you build is **how**,
never **what**.

These are the things to choose. They are named rather than counted: the count
has been wrong twice — once by leaving `ordering` out and once by leaving the
re-entrancy check out — and a number nobody maintains reads as a promise.

#### `target` — which machine
A 64-core server has threads and unwinds a stack when something goes wrong;
WebAssembly has neither. So the machine decides what the standard library can
offer — whether a file can be memory-mapped at all — and what happens on a
`panic`: an orderly unwind where the machine unwinds, an immediate trap where it
traps.

The default is `x86_64-linux`.

#### `user_parallelism` — may *your* code run concurrently at all?
* `no` (the default) — nothing you wrote ever runs concurrently. A web service
  that wants one event loop is built this way, and **data races are
  impossible**: two pieces of your code are never in flight together, so there
  is nothing to collide.
* `yes` — it may. This is what an image filter or a scientific calculation
  wants, and the compiler enforces the rules that keep shared data intact.

It is a permission and not a count. *How many* threads or cores serve a `yes`
belongs to the machine and the moment, so the runtime decides it; a number here
would be a promise the language cannot keep on hardware it has not seen.

**The word *your* is the whole of it.** This bounds your program, not the
compiler. Reading a file may still validate its text on four cores at
`user_parallelism = no`, and the runtime may still hand a blocking call to a
helper thread — neither runs code you wrote, and neither changes a single byte
of what your program prints. The rule is:

> The compiler may use as many threads as the machine has, for as long as no
> code **you** wrote runs concurrently.

#### `ordering` — how strictly is the written order taken?
Two operations that touch nothing in common may overlap, because the order
between them is not something a program can observe (Chapter 8.1.1).

* `effects` (the default) — the order is kept wherever it is observable, and
  only there.
* `strict` — the written order, always; the analysis is not applied.

`strict` is not a development aid to be removed later. It is the escape for a
program whose author does not want this, and for ruling out a bug in the
analysis in the field. A semantic default that cannot be switched off is a
decision imposed rather than offered ([ADR-033](adr/adr-033.md) D8).

#### the re-entrancy check — should a broken rule be noticed?
Taking a lock while a lock is held is refused when you compile (Part II, 12.3).
This switch decides whether a program *also* carries the run-time check
that notices such a nesting if one ever gets through. It is on by default, and
it can be declined.

This is the switch's own guarantee of the rule above it: **for every program
that obeys the nesting rule, both builds behave identically.** The check cannot
fire in a correct compiler, so what it controls is not error handling but
self-control — if it ever fires, the compiler has a hole, and without the check
that hole would show up as a silent hang instead. It is not a development aid to
be removed later; it is a guarantee that may be declined
([ADR-039](adr/adr-039.md) D8, D2).

> **Status:** not built. Nothing refuses the nesting of Part II 12.3, no
> re-entrancy check is emitted, and `nikaia.toml` has no key for this one — it
> is the one switch here specified ahead of the manifest that would carry it.

Each of them lives in `nikaia.toml`, and `--target` and `--user-parallelism`
override those two for a single build ([ADR-037](adr/adr-037.md),
[ADR-033](adr/adr-033.md) D8, [ADR-039](adr/adr-039.md) D8).

---

## Chapter 2: Variables and Data Types

A **comment** begins with `//` and runs to the end of the line. That is the only
form. There is no block comment — `/* … */` is not one, and does not parse — and
no doc comment: a `///` is an ordinary comment whose first character happens to
be a slash.

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

**A number is written in digits, and the exponent above is the only other thing
in one.** There are no digit separators, no radix prefixes and no type suffixes,
so `1_000`, `0xFF` and `1i64` are each not a number but a number beside a name.

**An integer that does not fit aborts, at every build.** An `i32` holds what an
`i32` holds; an arithmetic result that does not is an inconsistent program state,
like an index past the end of a list or a division by zero (Part III, A.2), and
the program stops rather than carrying a number nobody computed. It is the same
at both settings of `user_parallelism` and in every build, so the same program
computes the same thing wherever it is built — which is 1.2's rule, applied to
arithmetic ([ADR-043](adr/adr-043.md) D1).

Three of these are easy to miss, because the code looks harmless:

* `-x` and `x.abs()` on the **smallest** value of a signed type, because there is
  no matching positive one;
* the smallest value divided by `-1`, for the same reason;
* shifting by more places than the type is wide.

**Where a program means to wrap or to stop at the limit, it says so by name.**

```nika
let h = h.wrapping_mul(31).wrapping_add(c)        // a hash, wrapping on purpose
let volume = level.saturating_add(increase)       // stops at the maximum
let n = a + b                                     // aborts if it does not fit
```

`wrapping_add`, `wrapping_sub`, `wrapping_mul`, `wrapping_div`, `wrapping_neg`,
`wrapping_abs`, `wrapping_shl`, `wrapping_shr`, and the same names with
`saturating_`. There is no operator for either: both are rare, both are meant to
be visible in the line that does them, and a sign for one would have to be spelt
with `&`, which is borrowing here ([ADR-043](adr/adr-043.md) D2, D3).

**A conversion is written `as`, and one that may not fit aborts too.**

```nika
let average = (total as f64) / (count as f64)     // widening, always fits
let small = big as i32                            // aborts if `big` does not fit
```

Otherwise the question would be answered at the front door and let in at the
back: whoever wants the digits thrown away says so, the same way wrapping is said
([ADR-043](adr/adr-043.md) D4).

**A literal that does not fit its type is a compile error, not an abort.**
`let x: i32 = 3000000000` is decidable where it is written, and so is a sum of
literals that cannot fit, so neither waits for the program to run
([ADR-043](adr/adr-043.md) D5).

> **Status:** the abort is built — the generated project carries the check for
> your program and turns it off for every Rust dependency, whose own arithmetic
> is not this compiler's to be right about
> ([ADR-043](adr/adr-043.md) D6), and
> `crates/nikaia/tests/overflow.rs` compiles an overflowing program and runs it.
> The `wrapping_` and `saturating_` names are built for `i32` and `i64` — the two
> integer types named above — and `crates/nikaia/tests/overflow.rs` runs them
> beside the same arithmetic with `*`, which aborts. `saturating_shl` and
> `saturating_shr` do not exist, here or in the language below. The out-of-range literal is
> refused here now, as `NK1116`, wherever a type stands beside it: an annotated
> `let`, a `return` against a declared result, or an argument whose parameter says
> what it takes. It prevented no abort — one never happened — and what it takes
> back is the message, which was the backend's, in Rust's words, about a file
> nobody wrote.
>
> **Not built:** the narrowing check, so `5000000000 as i32` still prints
> `705032704`; and a sum of constants that cannot fit, which is still refused the
> other way (Part III, C.1).

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

> **Status:** not built. A trailing `?` on a type is a parse error, and `null` is
> read as an ordinary name rather than as a value — so neither line of the
> example above is accepted as written, while `??` (3.5) is.

### 2.4. Type Inference
Nikaia is **Statically Typed**, meaning the type of every variable is known at compile time. However, you rarely need to write types manually. The compiler uses **Type Inference** to deduce the type based on the value.

```nika
let name = "Nikaia"  // Compiler knows this is a &str - a view of static text
let count = 42       // Compiler knows this is an i32
```

### 2.5. Strings, Plain and Interpolated
There are two string literals, and the difference is one character at the front.

```nika
let name = "Nikaia"

println(f"hello, {name} - {name.len()} characters")   // f"…" - the braces are code
println("hello, {name}")                              // "…"  - the braces are braces
```

**`"…"` is text.** A `{` is a brace and nothing else, so a program that writes JSON, CSS or a
regular expression says what it means:

```nika
print("{}")               // prints {}
print("\\d{3}")            // a regular expression, written as one
print("{ margin: 0 }")    // a rule, not a hole
```

**`f"…"` has code in it.** Between `{` and `}` stands an expression — not just a name, but a
field, a call, an index. What follows a `:` inside one says *how* to write the value rather than
which value: the first colon that is not inside a call or an index separates the two, so
`Point(x: 1)` in a hole keeps its own.

Inside an `f"…"`, two braces stand for one: `{{` is a literal `{` and `}}` a literal `}`. An
escape is not a hole — the `{` in `"\u{0041}"` belongs to the escape — and a `}` on its own is an
error rather than a guess. **A plain string needs none of that**: it has no holes to be told
apart from, so `"{"` is a brace and `"{{"` is two.

The `f` and the quote are **one token**. `f"x"` interpolates; `f "x"` is a variable named `f`
beside a string, and whitespace never decides what a program means.

**A newline is an ordinary character in a literal.** Nothing ends one but the
closing `"`, so a literal left unterminated runs on to the next `"` in the file
or, if there is none, to the end of it.

The type follows the syntax rather than the contents. `"…"` is a view of static text and `f"…"`
builds a `String`, whether or not anyone put a hole in it — so adding a brace to a piece of text
cannot quietly change its type.

**A template's holes need no `f`.** `dsl html { <p>{name}</p> } eod` (Part II) is already marked
as a place where code appears, and the mark belongs on the construct rather than on every brace
inside it. That is the same rule read twice: a literal that holds code says so.

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

A block's last line is the **block's** value; `return` is the **function's**. So
a `return` written at the end of a block that is itself a value — a `match` arm
(3.4), an `if` branch whose value is taken (3.2), a `catch` handler (7.1), a
`seq` block (8.1.1) — leaves the enclosing function rather than handing that
block a value. The one block that is its own function is a lambda, whose
`return` leaves the lambda (5.3).

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
    println(f"Count is {count}")
    count += 1
}
```

**The `for` Loop**
Iterates over a sequence (like a range of numbers or a list).

```nika
// Iterates from 0 to 4 (5 is excluded)
for i in 0..5 {
    println(f"Index: {i}")
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

> **Status:** `??` is built. `?.` is not: there is no safe-navigation operator in
> the parser, and a `?` after an expression is a parse error.

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
        println(f"{self.username} logged in.")
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

> **Status:** the **list literal is not built** — `[1, 2, 3]` is a parse error at
> the `[`, and so is a list *type* written `[User]`. Built: the type `Vec[T]`,
> the tuple, indexing (`xs[0]`), indexed assignment, and `HashMap::new()`.

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

> **Status:** `impl Summarize for User` is built. **`trait Summarize { … }` is
> not** — a trait declaration is a parse error at the keyword, so a trait can be
> implemented but not declared, and the traits that can be implemented today are
> the ones the standard library and the Rust side already name.

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
A lambda is written `fn { … }`, and 5.3 is the whole of it. There is no second, shorter form for
single-line bodies.

Naming the arguments — `fn(user) { … }`, 5.3 Option A — is that one form spelled out rather than a
second one: the body is a block either way, and both spellings go in all the same places.

**Naming them is the normal way to write one.** Leaving the arguments to be named
automatically still compiles and is still the same form, but it is carried as
experimental and the compiler warns where one of the automatic names is actually
used (5.3, `NK1114`). The reason is that the automatic rule cannot be changed
while every lambda depends on it: [ADR-041](adr/adr-041.md).

The short form `fn: expression` is **not** part of the language. It saved four characters and its
body ran to the end of the expression, so a `.method()` chained after it landed *inside* the
lambda — silently, and a different program. Writing it is a compile error that names the block
form, rather than a parse failure, because the form appeared in earlier versions of this
specification and readers will have it in their fingers. Full reasoning:
[ADR-022](adr/adr-022.md).

### 5.3. Lambdas (`fn { ... }`)
When logic requires multiple steps, use a Block Lambda. Name its arguments; the
automatic names are still accepted and are being phased out.

**Option A: Named Arguments — the normal form**
* **Syntax:** `fn(name) { ... }`, and `fn(first, second) { ... }` for more than one
* **Args:** as many as the list says, and the list is the only thing that says so

```nika
let complex = users.map fn(user) {
    let bonus = calculate_bonus(user)
    // Implicit return of the last line
    user.score + bonus
}
```

**Option B: Automatic Names — experimental, and warned about**
* **Syntax:** `fn { ... }`
* **Args:** `a` (1st), `b` (2nd), `c` (3rd)
* **How many arguments it takes is read off the body**, not written anywhere: the
  lambda takes as many of the three names as its body *mentions*.
* **So the three names belong to the lambda and a body must not bind them.** A
  body that writes `let c = …` is asking for a third argument rather than
  shadowing anything. That is the trap Option A removes, and it is why naming
  them is the normal form.
* **Using one is a warning** — `NK1114`, which names the mechanical rewrite: the
  same letters in a parameter list. The program still compiles. A `fn { … }` that
  reaches for none of the three is not warned about, because the form is not
  being withdrawn; the automatic *naming* is
  ([ADR-041](adr/adr-041.md) D2).

```nika
// Accepted, and warned about: `a` is the argument.
let ids = users.map fn { a.id }
```

**Trailing Syntax**
A lambda that is the last argument may go *outside* the parentheses, and where there are no other
arguments the parentheses go away with it:

```nika
let ids = users.map fn(user) { user.id }
let sum = numbers.reduce(0) fn(acc, n) { acc + n }
```

A block ends at its `}`, so a chain continues after it and means what it reads as:

```nika
Server::new()
    .route("/x") fn { handler(db) }
    .listen(":8080")
```

```nika
// The count comes from the list, so a local may be called anything
users.map fn(user) {
    if user.is_guest() {
        return "Guest"
    }
    return user.name
}
```

> **Status:** built, in both spellings and in both positions, and the warning on
> Option B is built too. A trailing lambda may name its arguments or leave them
> automatic, and it may follow a method call with or without other arguments, a
> plain call, or a path: `users.map fn(user) { … }`, `users.map fn { a.id }`
> (with `NK1114`), `numbers.reduce(0) fn(acc, n) { … }`,
> `access_all(a, b) fn(x, y) { … }` (Part II, 12.3), `task::scope fn(s) { … }`
> (Part II, 12.7), and the chain above. `NK1114` fires once per lambda that
> reaches for one of the three names, positioned at the statement around it,
> because an expression carries no span of its own. **Not built:** an effect
> marker on a lambda. A parameter list is followed by the body and by nothing
> else, so `fn(info) sync { … }` (7.2) ends the lambda at the `sync` and the line
> is read as three expressions rather than one.

### 5.4. Contextual Capture (The Lifecycle Rule)
Nikaia simplifies memory management in closures by automatically inferring whether to Borrow or Move variables based on the context in which the lambda is used. This behavior is the same at either `user_parallelism`.

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
* **Behavior:** Implicit Move (Ownership Transfer) for ordinary data. A handle on a
  shared value is **duplicated** rather than moved, so the name outside stays usable
  ([ADR-040](adr/adr-040.md) D1, and 6.2 for the rule).
* **Examples:** `spawn`, `defer`, `set_timeout`, `channel.on_receive`.

```nika
let prefix = "Log: "

// 'spawn' is @detached. The lambda might outlive the current function.
// 'prefix' is implicitly moved into the background task to ensure safety.
spawn fn { println(prefix + "System started") }

// Compiler Error: 'prefix' has been moved!
// println(prefix)
```

#### C. Where the Distinction Lives

Which of the two a lambda is in belongs to the **function that takes it**, not to
the call: `map` is immediate for every caller and `spawn` is detached for every
caller, and that is what lets the capture be decided where the lambda is
written.

**It is not something your own function can declare.** Nikaia's type grammar has
no function type — a parameter's type is a name with type arguments, optionally
a view, or a tuple — so no `.nika` source takes a lambda and passes it on, and
`@detached` is a property of the standard library's own entries rather than a
word a program writes (Part III, Appendix B). What that closes off is **effect
polymorphism**: a wrapper whose own immediacy would have to follow its
parameter's ([ADR-029](adr/adr-029.md) D1). A lambda that must be moved is
therefore handed to the detached function where it is written, not through a
wrapper of your own.

> **Status:** not built, and not writable — a parameter of function type is a
> parse error, so `@detached` on one has nowhere to appear. The
> immediate/detached rule itself is `std`'s, held by a check over `std`'s ledger
> entries rather than by anything in a source file ([ADR-029](adr/adr-029.md)
> D4); the capture it decides (`NK2101`, 8.3) is not reported yet.

---

## Chapter 6: Memory and Ownership

Memory management is usually either manual (hard) or automatic via Garbage Collection (slow). Nikaia uses a third way: **Ownership and Borrowing**, handled by the compiler.

### 6.1. The Concept of Scope
When a variable goes out of **Scope** (usually at the end of the block `{}` where it was created), Nikaia automatically cleans up the memory. You do not need to free memory manually.

### 6.2. Unified Types
To make coding easier, Nikaia provides smart types that handle memory logic for you.

You write `Shared[T]` yourself — it is not inferred, because sharing changes *when* a value is cleaned up (6.4), and that is something your program can observe. What the compiler decides is the machinery underneath: the count of owners is one a second thread may safely touch, at **every** setting of `user_parallelism` ([ADR-037](adr/adr-037.md) D6). That is what makes a `Shared` something you can hand to a task (Part II, 11.2) — the machinery cannot be safe in one build and unsafe in another, because a library you wrote at one setting has to keep working at the other.

**Where the compiler can prove that a particular value never leaves the thread that made it, it uses a cheaper count instead** ([ADR-037](adr/adr-037.md) D7). That is an optimisation and never a change of meaning: the program does the same thing either way, and the only difference is about 9 ns each time a handle is made and dropped — nothing at all for a handle you only read. **Where nothing proves it, the safe count is what you get**, and there is no way to ask for the other one: every case where the proof fails is a place something has not been written down, and writing it down is the way to the cheaper count. `nikaia --input x.nika --sharing` prints which count each of your values got, why, and what would have changed it. Like `--overlaps` and `--trust`, it explains a decision rather than changing one.

* **`Shared[T]`**: for a value several parts of the program own at once and nobody changes. The memory is only cleaned up when the *last* owner is finished.
* **`SharedMut[T]`**: for a value several parts own at once and any of them may change. The lock that keeps the changes apart is part of the type — there is no second wrapper to write around it — and the changing is done through the four doors of 6.3. This is the common case, which is why it has the short name ([ADR-039](adr/adr-039.md) D9).
* **`Locked[T]`**: for individually locked fields inside a shared structure — one lock per field rather than one lock around the whole of it. It is the same lock as the one inside `SharedMut[T]`, opened by the same four doors.

**How the second owner comes about.** You do not write the step that produces
one. Where a handle on a `Shared[T]` or a `SharedMut[T]` is handed on **by
value** — passed to a function that keeps it, or used by a task (Part II, 11.2) —
the handle is **duplicated**, and each handle is
cleaned up at the end of its own block (6.1). There is no method to call, because
there would be nothing for it to do: a duplicated handle copies none of the data
and produces no second value — one value, one more owner — so there is nothing to
name ([ADR-040](adr/adr-040.md) D1). Both are shared values, so the rule is the
same for both — though a `SharedMut[T]` has nowhere to be handed *to* across a
thread, because it may not cross one (Part II, 11.2).

**Lending the inner value out duplicates nothing.** `&` on a shared value is a
view of the value *inside* it, so a function that only uses the value takes an
ordinary view and never mentions sharing:

```nika
fn serve(db: &Connection) { … }

let db: Shared[Connection] = postgres::connect("…")
serve(&db)          // a view; no handle is made, and the count is untouched
```

Whether the value is shared is the caller's decision, and `serve` has no business
knowing. A signature names the shared type only where the function **keeps** the
value past the call — puts it in a structure, gives it to a task, hangs it on
something that outlives the call — because only then does it need a handle of its
own ([ADR-042](adr/adr-042.md) D1, D2).

**With `SharedMut[T]` you get in by opening it, and then it is an ordinary view
again.** There is no `&` straight through a lock: the caller opens it with one of
the four doors of 6.3 and passes the borrowed value in, and the called function
sees a plain value and must obey the rule for a lock that is open — it may not
pause, and it may not touch a lock of its own (Part II, 12.2).

```nika
let db: SharedMut[Connection] = postgres::connect("…")
db.access fn(open) { serve(open) }    // `serve` must be `sync` and lock-free
```

**How the first handle is made: by writing the type.** There is no constructor
and no method.

```nika
let db: Shared[Connection] = postgres::connect("…")
```

The language asks for sharing to be visible in the source because it changes when
the value is cleaned up — and here it is, in the line where it starts.
`Shared::new(…)` would write the same thing twice.

> **Sharing always starts on a line where you wrote the type yourself** — an
> assignment with a type annotation, or a field with a declared type. Everywhere
> else you receive what is already shared.

That sentence is the whole rule, and the two places it names are the only two. Not at a
call site merely because a signature wants one. A call in which the word does not
appear would otherwise move the cleanup point silently, and at such a place there
would be no saying whether the value was handed on or duplicated — it would be
both at once. So this is refused, and the message points at the line where the
sharing belongs:

```text
error[NK1115]: `serve` takes a shared value, and `db` is not one
  help: write the sharing where it starts:
        let db: Shared[Connection] = postgres::connect("…")
```

**A function that returns a shared value is the same rule, not an exception to
it.** A signature line is not a line you wrote the type on, so the `return` is
not where sharing may begin — which leaves two ways, and both are ordinary:

```nika
// Wrap inside, on a line of its own. The `return` then hands on a value that
// is already shared, which is a plain return.
fn connect(url: String) -> Shared[Connection] {
    let c: Shared[Connection] = Connection(url: url)
    return c
}

// Or - usually better - return a plain value and let the caller decide.
fn connect(url: String) -> Connection { … }
let db: Shared[Connection] = connect(url)
```

The second imposes sharing on nobody who does not want it. The first costs one
line, and that line is where a reader sees the sharing begin.

**This is the handle and nothing else.** Ordinary data — a string, a number, a
struct of those — is still **moved** where it is handed on (8.3). An automatic
duplication there would copy the whole of the data, which is a different thing at
a different cost, so for data the `.clone()` stays something you write yourself
([ADR-040](adr/adr-040.md) D1).

**Each handle lives to the end of its own block whether you use it again or
not.** The duplication is not conditional on a later use
([ADR-040](adr/adr-040.md) D2), so the value is cleaned up where *your* handle
ends, which may be later than the task that holds the other one. That is the
block rule of 6.1 applied unchanged rather than a special case to learn: whoever
wants the cleanup earlier ends the block earlier ([ADR-040](adr/adr-040.md) D3).
Where a handle is duplicated is not something your source shows, so `--sharing`
names each duplication site beside the count it printed for that value — the cost
of an extra handle stays something you can look up
([ADR-040](adr/adr-040.md) D5).

> **Status:** `Shared[T]` is built. It is a type the compiler knows — `std`'s
> ledger carries the entry, the backend lowers it to one of the two owner counts,
> and the annotated `let` above really does make the first handle, as does a field
> whose declared type says so. `serve(&db)` works through the `deref` entry and
> nothing else ([ADR-042](adr/adr-042.md) D2), and the call that wants a shared
> value and is given a plain one is refused by `NK1115` rather than wrapped
> silently (Part III, C.3).
>
> **`SharedMut[T]` and `Locked[T]` are not.** Writing either names a type that
> does not exist, the backend has no lowering for one, and the four doors of 6.3
> wait on the same thing ([ADR-039](adr/adr-039.md) §4).
>
> **The duplication is built for a handle handed to a function**, and not for one
> used by a task. A handle handed on by value — to a call whose parameter takes one
> — is duplicated, so the name outside stays usable; lending the inner value out
> duplicates nothing; and `--sharing` names each duplication site beside the count
> it printed ([ADR-040](adr/adr-040.md) D1, D5). The task half waits on `spawn`,
> which does not lower yet (Part II, 11.2) — the analysis sees it and names it, and
> no emitted program reaches it. What is built above all of it is the reasoning:
> which owner count a `Shared` value gets is inferred per value, and `--sharing`
> prints it ([ADR-037](adr/adr-037.md) D7).

### 6.3. Changing Shared Data: The Four Doors
A value behind a lock is not changed by assignment — the lock has to be opened
first, and the shape of the change decides which door you use:

```nika
let kasse: SharedMut[i32] = ...

let stand = kasse.get()              // take a copy out
kasse.set(hole_neuen_stand())        // replace it; the new value comes from outside
kasse.update fn(old) { old + 100 }   // new value from the old one

// Where the value is large, change it in place rather than copying it out and back.
let protokoll: SharedMut[Log] = ...
protokoll.access fn(log) { log.add("gebucht") }
```

Each door is shaped for what it is for:

* **`get` and `set` take no block at all.** `get` copies the value out. `set`'s argument is computed *before* the call, so everything slow about producing the new value — including waiting for I/O — happens outside, and the lock is open for one store.
* **`update` is handed a copy of the old value and returns the new one.** Nothing that points inside the lock ever exists, so there is nothing that could outlive the block.
* **`access` is for where copying is too expensive** — a list of ten thousand entries is not copied to append one. It hands your block the value itself.

Because `update` and `access` run your code while the lock is open, that code
must be able to run straight through: no I/O, and no second lock. The compiler
checks both (Part II, 12.2 and 12.3).

Two mistakes are refused by name. Assigning to a `SharedMut` directly —
`kasse = 0` — is refused, and the message names `set`. And a `set` whose
argument reads the same container, `kasse.set(kasse.get() + 100)`, is refused,
and the message names `update`, which is the door for a new value computed from
the old one ([ADR-039](adr/adr-039.md) D10).

> **Status:** not built. None of the four doors exists: `get`, `set`, `update`,
> `access` and `access_all` have no entry in `std`, nothing lowers them, and
> neither of the two refusals above is reported.

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
    fn cleanup(&mut self) throws {
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

**Cleanup errors are real errors.** If closing a resource can fail, the function that owns it can fail — Nikaia does not hide this (silently losing data at close time is a decades-old bug class in other languages). If `cleanup` declares `throws`, the surrounding function needs `throws` too, and the compiler tells you precisely why:

```text
error[NK2601]: this function can fail because closing `f` can fail
  --> report.nika:2
   |
 2 |     let f = fs::create("report.txt")
   |         ^ `f` is a buffered file; writing its remaining data
   |           to disk at the end of this function can fail
   |
  help: declare the error:  fn save_report(text: String) throws
  help: or handle it precisely by closing explicitly:
        f.close() catch { ... }
```

Two refinements:
* If a value dies **while an error is already bubbling up**, the cleanup error does not replace it — it is attached to the original error as a *secondary error* (see 7.1's automatic debug information).
* If you want to react to the close error specifically, call **`close()`** yourself — it consumes the resource, returns the error normally, and no implicit cleanup runs afterwards.

**One restriction, told straight:** a `sync` function can never pause — so a resource with a pausable `cleanup` must not go out of scope inside one. The compiler catches this (`NK2602`) and names the ways out: return the resource to your caller, close it before the `sync` part, or use a non-buffering variant.

**When `cleanup` cannot run.** If a task is *cancelled* (it lost a `select` race, or a supervisor restarts it), nobody can wait for its I/O. The runtime then adopts the pending `cleanup` runs and finishes them in the background before the program exits ("parked cleanup" — bounded by the `cleanup-deadline`, Part III 13.3). Only a **panic** gets no pausable cleanup: during panic teardown only the synchronous `drop` fallback runs — and **on a target that traps rather than unwinds, a panic ends the process immediately, so no destructors run at all** (Part III, Appendix A). What *does* still run on every panic is the **Panic Hook** (7.2) — your registered last-moment handler for dumps and crash reports. Panics are for unrecoverable bugs; recoverable failures use `throws`, where full cleanup is guaranteed.

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
        println(f"{name}: {data}")     // ...and the borrow is still valid.
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

**A parameter written `&str` may not be kept past its call.** A view inside a struct carries the buffer it points into, because the struct's own declaration says it holds a view; a parameter written `&str` on its own says only that the call may look at one. So a function that stores such a parameter — into a field of its subject, into a struct it hands back, into a task — is refused as `NK2302` (Part III, C.3), and the way to write it is to put the view in a struct and take the struct:

```nika
@borrowed
struct Reading { name: &str, temp: i32 }

impl Summary {
    fn record(&mut self, m: Reading) { … }     // and not `name: &str`
}
```

Handing a view back out of the buffer it came from is **not** this rule: `fn count(seq: &str, k: usize) -> HashMap[&str, Tally]` returns views of `seq`, and the result points into `seq` and nothing else. Why a parameter is not given a buffer of its own to name is [ADR-005](adr/adr-005.md) D1 — the language has no syntax for one — and what a struct carries instead is [ADR-008](adr/adr-008.md) D1.

Where the thing it is stored into **already carries a buffer**, there is nothing to refuse: a method of a struct that holds a view has that struct's buffer in hand, so storing the parameter into one of its fields is accepted and the parameter is a view of *that* buffer. `fn note(&mut self, name: &str)` on a `Summary` holding `label: &str` compiles, and `name` is a view of the same buffer `label` points into — which narrows what a caller may pass and is why it is the signature rather than the body that changes.

> **Status:** built for a **method of a struct that holds a view**, which is where a buffer is already named. Three cases are still refused: a function or method whose own subject holds no view (there is no buffer to name, even when the destination is a field of a struct that carries one); a view handed back through the result; and a view given to a task. Of the accepted cases, one is accepted without being decided: where the view is handed to a call on the subject, the compiler cannot see whether the callee keeps it, so it is treated as kept and the parameter is written as a view of the subject's buffer either way.

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

**Because nothing marks it, the compiler insists on the declaration.** A call that can fail, in a
function that does not say `throws`, is **refused** — `NK2605` — rather than lowered:

```text
error[NK2605]: this function can fail because `liest` can fail
  --> app.nika:2:23
   2 | fn ruft() -> String { return liest() }
                             ^
     = `liest` carries `throws = ["?"]` in the contracts this program is built against (Part III, 13.5)
     = nothing marks a failing call, so a failure leaves at a call exactly as it leaves at a block's closing brace or a loop's step (ADR-023 D8, ADR-025 D1)
     help: declare the error: add `throws` to `ruft` - or handle it at the call, `… catch { … }` (Part I, 7.1)
```

on `fn liest() -> String throws { return fs::read_to_string("x.txt") }` one line above. The shape
is Appendix C.4's: the caret is on the statement, **the note is the contract** quoted from the
ledger the call was resolved against, and the help is one of the two things a reader can type.

Two things rest on that refusal. The signature is the only place the source says a function can
fail, so a caller that does not say it has said something false; and `nikaia.contracts` records
`throws` per function and is **committed and read by other programs** (Part III, 13.5), so
accepting the program would publish that `ruft` cannot fail.

> **Status:** built for a call **by name** — a function of this program, of another of its
> modules, or of `std`. A **method** call that can fail is refused by `NK2605` all the same, but
> the lowering does not yet propagate one: on a method, `catch` at the call is the shape that
> compiles today. A call nothing describes is neither refused nor propagated, and reaches the
> backend as before.

**Handling: `catch`.** The block supplies the replacement value — or it leaves the function.

```nika
let config = load() catch {
    eprintln(f"{error}")
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
            eprintln(f"config broken at line {line}")
            return
        }
        _ => throw error
    }
}
```

> **Status:** the patterns of 3.4 are built. Two things in the example above are
> not: **`..` in a named pattern**, so a `match` cannot yet bind some fields and
> ignore the rest, and a bare `throw` as an arm's body — write the arm as a
> block, `_ => { throw error }`.

Two sets, and they are not the same one. The **variants of an error type** are closed, a `match`
over them is exhaustive, and adding one is a breaking change — correctly. The **set of error types**
arriving at a `catch` is open, and it grows when a callee gains a failure. Where that changes a
`catch`, the compiler narrates the chain: what changed, which contract moved, which caller broke
(`NK2401`).

**What an error brings without anyone attaching it.** The **site** it was raised from, and the chain
beneath it where another error joined on the way — a cleanup that failed while the stack was
unwinding is attached to the original as a *secondary* error rather than replacing it (6.4). The
site costs nothing at run time: the compiler knew it and wrote it into the binary as text.

**A stack trace is not among them.** Errors here are the *expected* kind — a missing file, a line
that does not parse — and capturing a trace for each one costs far more than raising it, so a
program would pay that per rejected line for a value almost nothing reads. `NIKAIA_TRACE=1` asks
for one; without it there is none, and the long form **says so** rather than leaving you to wonder
whether one was lost. What that costs, and why the cost decided it, is
[ADR-036](adr/adr-036.md).

**Printing it: short is the default.**

```nika
eprintln(f"{error}")           // the message, and nothing else
eprintln(f"{error.full()}")    // the site, the chain, and a trace if one was captured
```

`{error}` is the message the author wrote. An error message is for the operator, not for the visitor
of a web page — which is why a failed HTTP handler answers with a generic 500 and logs the rest
([ADR-018](adr/adr-018.md)). The form you type without thinking is the one you may show a stranger.

`full()` is an ordinary call, not a spelling the language had to invent: a hole holds an expression
(2.5), and a method call is one.

So that a generic answer stays findable anyway, every error knows the **site that raised it** and
has a short form of it a person can read out. A screenshot carrying `(NK-2C7)` leads through
`nikaia explain NK-2C7` to the line that threw it — with no log file, and also when your working
tree is two features further along ([ADR-023](adr/adr-023.md) D6).

**Failure nobody writes down.** Two places perform a call you did not type, and both can fail: the
end of a block, where a resource is cleaned up (6.4, `NK2601`), and a loop's step over a fallible
stream ([ADR-025](adr/adr-025.md), `NK2701`). In both the enclosing function gains a `throws`, and
the compiler says which resource or which loop it was.

**It is one rule with three sites**, stated generally by [ADR-025](adr/adr-025.md) D1: where a
call can fail — written by you or performed by the language — the failure fails the enclosing
function, the function must declare `throws`, and the compiler names the call that is the reason.
`NK2605` is the written call, `NK2601` the closing brace, `NK2701` the loop's step; three codes
because three messages have three different things to point at, and one rule behind them.

### 7.2. Unrecoverable Errors (`panic`)
These are logical bugs, like trying to access the 10th item in a list of 5 items. Nikaia stops the execution to prevent incorrect behavior. Where the machine cannot unwind, this ends the process safely.

**The Panic Hook (`std::panic::on_panic`)**
"Abort" does not mean *no* code runs anymore — it means no normal cleanup runs. Before the process dies, or the crashed task is isolated where the program survives, Nikaia calls one last, registered function: the **Panic Hook**. This is the place for a crash dump, a crash report, or flushing a diagnostics log — so a crash in production never has to be a mystery.

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
* **It runs on every panic, on every target** — right before the trap where the machine traps, before the task is poisoned where it unwinds. Supervisors (Part II, 12.8) receive their crash information from the same `info`.
* **Blocking is allowed here — briefly.** Where the process is ending anyway it costs nothing. Where the program keeps running, keep the hook short and hand heavy reporting to something you started earlier.
* **Diagnosis, not cleanup.** Do not try to flush buffered files or finish transactions from the hook — those objects may be broken in exactly the way that caused the panic. That is why panics skip destructors, and the hook does not reopen that door. If the hook itself panics, the process aborts immediately.

> **Status:** not built. There is no `std::panic`, and the line above does not
> parse as one construct either: a lambda's parameter list is followed by its
> body and by nothing else (5.3), so the `sync` marker ends the lambda and
> `panic::on_panic fn(info) sync { … }` is read as three expressions. The
> trailing lambda itself is built, named arguments included — it is the marker
> between the parameters and the block that has no grammar.

---

## Chapter 8: Concurrency (Doing things at the same time)

Even at `user_parallelism = no`, you can perform multiple tasks concurrently, such as waiting for a download while responding to user input. This is done using **Asynchronous Programming**.

### 8.1. Async by Default
In Nikaia, functions that perform Input/Output (I/O), like reading a file or downloading a URL, automatically "pause" execution without blocking the whole program. You do not need special keywords like `await`.

Within one task the order is exactly the order you wrote: `let a = fs::read("x")` pauses, and the line after it does not run until `a` is there. What runs meanwhile is some *other* task — a pause point never forks one. Tasks exist only where you put them (`spawn`, `par_iter`, `task::scope`), and `.await` on a handle is where two of them meet again. Nikaia did not remove the marker for waiting; it kept it exactly where something branches, and left it off where nothing does.

### 8.1.1. Order Is Kept Where It Can Be Seen

> **Status.** This section is specified ahead of the compiler, because it changes what a program
> *means* and a decision of that kind belongs here before it belongs in the implementation
> ([ADR-033](adr/adr-033.md), still marked provisional). Built today: a **run of adjacent
> statements of any length** whose calls reach different resources overlaps — a `let` or a bare
> expression statement, and a value built out of literals and calls rather than being one call;
> `seq { … }`; the resources a file, standard output, standard error and the program's arguments;
> `--overlaps` explains every pair; the `ordering` key is read from `nikaia.toml`, and
> `--ordering strict` restores the written order everywhere. Not built: a **method call**, whose
> contract depends on the type of its receiver; an argument that is not a literal; a `let` with a
> written type; a call with options; standard input, a socket and a lock as resources — nothing
> asks for one yet, so all three of them count as touching everything; and nothing infers what a
> function of your own touches, so every one of those counts as touching everything too.
>
> **And which of them your build performs depends on `user_parallelism` (1.2).** A pair of **file
> reads** overlaps at either setting: the operations are the standard library's, nothing you wrote
> is in flight twice, and no promise of `no` is touched ([ADR-033](adr/adr-033.md) D10). Everything
> else above needs `user-parallelism = "yes"`, because running it means running two pieces of *your*
> code at once — at `no` the analysis still answers, and `--overlaps` marks such a pair `would`
> rather than `together`. Two writes in flight, and a run of three reads rather than two, are not
> built at `no` yet: a run of three overlaps its first two and leaves the third where you wrote it.

Two lines that never meet have no reason to wait for one another:

```nika
let a = fs::read("x")       // 40 ms
let b = fs::read("y")       // 40 ms
return a.len() + b.len()    // 40 ms, not 80
```

Every operation is known by **what it touches** — not merely "it does I/O", but *which file, which socket, which lock*. From that comes one rule:

> **Two operations whose touch sets are disjoint have no order between them. Everything else keeps the order it was written in.**

You write nothing for this. The default is the fast path, and the cases below are what keep it from being a surprise:

* **The obvious stays obvious.** `println("a")` then `println("b")` prints `a` before `b` — both touch standard output, so they are ordered. Not a special case; the rule already says it. A `println` and an `eprintln` are ordered too: two handles, and one destination as soon as anybody runs the program with `2>&1`, so the compiler does not treat them as separate things.
* **It is a run, not a pair.** Three lines that all meet on nothing run as one group, not as two and then one. Every one of them has to meet *every* other on nothing, and not merely the line next to it — `read("a")`, `read("b")`, `write("a")` is a group of the first two, because the third meets the first.
* **What is not known is ordered.** An operation whose touches the compiler cannot determine counts as touching everything, and stays exactly where you put it. A program built against libraries that say nothing behaves precisely as it does today, and gets faster only as contracts get written. A resource named in a word this compiler does not know — a library built for a newer toolchain — reads the same way: the operation touches everything, and nothing is assumed.
* **Nothing is run on speculation.** `if x { lies(a) } else { lies(b) }` starts one of them. Overlapping only ever applies to work that was certainly going to happen.
* **Errors keep their order.** If two overlapped operations both fail, the failure you see is the one written first — never the one that lost a race.

**When you need an order the compiler cannot see** — two calls to different addresses of the same service, say — say so:

```nika
seq {
    benachrichtige(kunde)
    protokolliere(vorgang)
}
```

Inside the block the statements run in the order you wrote them, whatever their touch sets say, and that holds for anything written inside it — a nested block, an `if`, a loop. The block itself keeps its own place as well: you have just said the compiler cannot see what the order is for, so it does not move the block either. It changes nothing else: a `seq` block is a block, so it is an expression and has a value like any other (3.1).

> **The keyword is `seq`, and it is settled** ([ADR-033](adr/adr-033.md) D7). It was written down
> as a placeholder — it has to read as *"in this order, whatever you think"* — and kept, because a
> second round of naming a block almost nobody writes buys nothing. A program written today does
> not have to be renamed.

That is the trade the rule is built on: the common path is the fast and safe one and costs nothing to write, and the exception costs a line and is visible where it matters.

**There is no way to say "run these together anyway", and that is deliberate.** The one refusal where you might want to — a `catch` that leaves the function, which makes everything after it conditional — has a clearer form already:

```nika
// Kept in order: if the first fails, the second would never have run.
let k = lade_kunde(id) catch { return Seite::leer() }
let e = hole_empfehlungen()  catch { … }

// Runs together, and reads better: the failure and the decision to stop are
// two different things.
let k = lade_kunde(id) catch { Kunde::unbekannt() }
let e = hole_empfehlungen()  catch { Empfehlungen::leer() }
if k.ist_unbekannt() { return Seite::leer() }
```

**When you want to know why two things did not run together, ask:** `nikaia --input x.nika --overlaps` prints every adjacent pair, which of them run together, and for the rest the reason and — where there is one — what to write instead. A pair marked `would` is one your program allows and this build has no way to run: the line names the switch, because "they did not run together" without a reason is exactly what this flag exists to prevent. It changes nothing about the program; it explains a decision, the way `--trust` does for where a program's bytes came from.

The whole thing can be turned off for a project with `ordering = "strict"` in `nikaia.toml` (Part III, 13.3), which restores the written order everywhere.

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

**Two cases, and the type of the value decides which.** What the rule above is
about is **ordinary data** — a string, a number, a struct or collection of those —
and for data a move stays a move. A handle on a `Shared[T]` is the other case:
the handle is **duplicated** rather than moved (6.2), so the name outside the task
keeps working and there is nothing for you to write
([ADR-040](adr/adr-040.md) D1). `message` here is a string, so everything in the
rest of this section is the **data** case.

If you still need the value afterwards, clone it **before** the task is built and
give the task the copy. A `.clone()` written *inside* the body does not help: the
body runs after `message` has already moved into the task, so it would clone the
task's own copy and leave nothing behind for the parent.

```nika
let message = "Hello"
let copy = message.clone()   // made here, while `message` is still ours
spawn fn { println(copy) }   // the copy is what moves into the task
println(message)             // OK: `message` never left
```

The compiler error for this situation — data moved into a task and used again
afterwards — explains exactly that:

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
  help: clone before the task is built, and give the task the copy:
        let copy = message.clone()
        spawn fn { println(copy) }
```

**`NK2101` belongs to the data case only.** For a handle on a `Shared[T]` there is
nothing to refuse: using the value again after the task is built is the very thing
the duplication of 6.2 serves, so there is no error and no `.clone()` to write
([ADR-040](adr/adr-040.md) D5).

> **Status:** not built, in either case. `spawn` does not lower yet — the runtime
> integration it needs is the next step (Part II, 11.2) — and `NK2101` is
> catalogued but never raised (Part III, Appendix C.3). The move rule is
> therefore written ahead of both. `Shared[T]` is a type the compiler knows now
> (6.2) and a handle handed on by value is duplicated, so the rule the other case
> states is built everywhere a handle is handed to a **function**; into a task it
> is not, because there is no task to hand one to, and there is no raised `NK2101`
> for the exemption above to apply to either
> ([ADR-040](adr/adr-040.md) §4). The form above is the one
> spelling of a `spawn`, the trailing lambda of 5.3; the parser still insists on
> parentheses around the body instead, which is a bug in the parser and not a
> second form.

### 8.4. The Runtime Sidecar Model
While `user_parallelism = no` keeps your own logic on one thread ("The Happy Path"), the Runtime employs a **Hidden Sidecar Pattern** to handle heavy I/O without blocking.

* **Separation of Concerns:** User code runs exclusively on the main thread (Event Loop). Heavy operations (like SQLite queries) are offloaded to a managed Runtime Sidecar (a background thread on Native, or a Web Worker on WASM).
* **Safety Guarantee:** Data exchange occurs via strict message passing (ownership transfer). Since user code never accesses the Sidecar memory directly, **Race Conditions** remain impossible.
* **Non-Blocking:** From the developer's perspective, a database call is simply an async yield point. The Runtime guarantees that the main loop never stalls waiting for disk I/O.

> **Status:** the sidecar exists; the yield point does not.
>
> The runtime starts **before your first statement**, with one I/O thread
> always and a pool for your own code only at `user_parallelism = yes`
> ([ADR-038](adr/adr-038.md) D4). What runs on that thread is `std`'s own code
> and nothing else — the boundary is a closed list of operations rather than a
> queue of closures, which is what makes "user code never accesses the Sidecar
> memory" a property of the compiler rather than a promise
> ([ADR-037](adr/adr-037.md) D2, ADR-038 §4.2). A file read is handed to the
> **kernel** where the machine can complete it, so the commonest heavy
> operation involves no sidecar thread at all (ADR-038 D3).
>
> What is not built is the last bullet. Stage 0 emits a direct call, and the
> call blocks the calling thread while the kernel works: there is no state
> machine, so there is no other task for the main loop to run in the meantime.
> The two halves this section promises therefore arrive in order — the thread
> that never stalls is here, and the yield point that would use it is not.

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
    println(f"{utils::double(21)}")
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
    * Functions, Structs, Enums, and Constants are only visible inside the file they are defined in. (A `const` declaration has no syntax yet — Part II, 10.2.)
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

