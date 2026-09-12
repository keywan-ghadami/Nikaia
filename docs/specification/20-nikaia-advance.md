# Nikaia Language Specification
**Part II: Advanced Features & Metaprogramming**
**Version:** 0.0.7 (Draft)
**Date:** September 5, 2026

---

## Chapter 10: Metaprogramming (Code that writes Code)

Metaprogramming allows developers to extend the language itself. In Nikaia, this is not done with text replacements (like in C) but by manipulating the structure of the code (the **AST** or Abstract Syntax Tree) in a safe way.

### 10.1. Parsing with `grammar` (Scannerless)
Before you can manipulate code, you often need to read custom data formats. The `grammar` tool lets you write parsers declaratively.

Nikaia grammars are **scannerless**: there is no separate tokenizer stage. A grammar consumes the raw character stream directly. This is what makes it possible to embed foreign languages (10.5) — a global lexer could never tokenize SQL, assembly and Nikaia at once, because they disagree about what a `}` or a `'` means.

**Key features:**
*   **Commit Points (`=>`):** control backtracking. Once the parser passes a commit point, it stays in this branch — a later failure is an *error*, not a reason to silently try the next alternative.
*   **Lexical vs. syntactic rules:** rule names starting with an uppercase letter are **lexical** (no whitespace between parts); lowercase rules are **syntactic** (whitespace allowed). This replaces the lexer/parser split.
*   **Typed actions (`-> { … }`):** each rule builds your own types directly.

```nika
grammar Json {
    pub rule value -> Value =
        o:object -> { Value::Object(o) }
      | a:array  -> { Value::Array(a) }
      | s:string -> { Value::String(s) }

    // Commit point: once '{' matched, members and '}' MUST follow, or we error.
    rule object -> Object
        = "{" => members:list(pair, ",") "}"
        -> { Object { members } }

    rule pair -> Pair = key:string ":" => val:value -> { Pair { key, val } }

    // Lexical rule (uppercase): no whitespace inside a hex byte. Two hex digits
    // always fit in a `u8`, so this action cannot fail; where one can, what its
    // failure means is decided by the commit point above it
    // ([ADR-023](adr/adr-023.md) D9).
    rule HEX -> u8 = d:hex_digit{2} -> { hex_byte(d) }
}
```

> **Note:** Auto-generated AST types (structs for labelled sequences, enums for alternatives, when no `-> { … }` is given) are a planned convenience, not current behaviour. Action blocks are required today. See [ADR-007](adr/adr-007.md), D7.

### 10.2. Dual-Mode Parsing (Static vs. Dynamic)
A grammar defined once can be used at compile time and at runtime — **with the same syntax**. Whether parsing happens during the build or while the program runs follows from the context, not from a different spelling.

**A. Static Embedding (Compile-Time)**
Used in a `const`, the parser runs *during the build*. If the input is invalid, compilation fails. The result is embedded in the binary with zero runtime cost.

```nika
// The compiler runs the Json grammar at build time.
// If "config.json" is malformed, the build stops.
const CONFIG: Json::Value = dsl Json from "config.json"
```

**B. Dynamic Parsing (Runtime)**
The exact same grammar processes user input or network data while the program runs.

```nika
fn parse_input(input: String) throws {
    let data = dsl Json from input
    println(f"Parsed: {data}")
}
```

> **Status:** **B is built and A is not.** There is no `const` in the parser at
> all — at item level it is a parse error, and inside a function body `const X =
> 1` is read as two statements — so the compile-time half of dual-mode parsing
> has no syntax to be written in, and a `dsl … from …` runs at runtime wherever
> it stands.

### 10.3. Code Generation (Quasi-Quoting)
While parsing reads data, **Macros** create new code. Nikaia uses a mechanism called **Quasi-Quoting**. The `quote` block allows you to write Nikaia code as data templates and fill in the blanks with variables.

**Example: Auto-Generating a "Describe" Function**
Imagine you want to automatically print all fields of a struct without writing the print statements manually every time.

```nika
// Definition of the Macro
pub macro Describe(def: StructDef) -> AstExpr {
    
    // Logic: Create a print statement for every field in the struct.
    // .map() iterates over the fields and creates a list of code blocks.
    // Syntax: a trailing lambda, outside the parentheses - and where there are
    // no other arguments, the parentheses go with them.
    let print_statements = def.fields.map fn { quote {
        // 'quote' creates a piece of code. 
        // We inject 'a.name' and 'self.a.name' into this code.
        println("Value of " + a.name + " is: " + self.a.name)
    } }

    let name = def.name

    // Return the final code block
    // We inject the list of print_statements into the function body.
    quote {
        impl name {
            fn describe(&self) {
                print_statements // Automatic insertion of the list
            }
        }
    }
}

// Usage
// The 'with' keyword triggers the macro.
struct User with Describe {
    name: String,
    age: i32
}

fn main() {
    // Implicit Anonymous Constructor via strict Subject/Config protocol.
    let u = User("Alice", 30)
    u.describe() // This function was generated by the macro!
}
```

### 10.4. Hygiene and Scope Injection
By default, Nikaia macros are **Hygienic**. This means a variable defined inside a macro (like `let temp = 0`) is invisible to your main code. This prevents accidental conflicts where a macro overwrites your variables.

**Breaking Hygiene (Injection)**
Sometimes, you *want* a macro to create a variable that the user can use (e.g., a loop macro providing an `index` variable). To do this, you must mark the variable name as "injected" using the `@` prefix inside the quote.

```nika
pub macro CreateVar(name_str: String) -> AstExpr {
    let var_name = Ident::new(name_str)
    
    quote {
        // The '@' tells the compiler: "Inject this name into the user's scope"
        let @var_name = 100
    }
}

fn main() {
    CreateVar("score")
    println(score) // Works! Prints 100.
}
```

> **Status for 10.3 and 10.4:** **not built.** `macro` and `struct … with …` are
> parse errors, and `quote { … }` is read as a name followed by a block rather
> than as one construct — so nothing on these two pages is accepted as written,
> and hygiene and injection have no construct to apply to. Nikaia's built
> metaprogramming is `grammar` (10.1) and `dsl` (10.5).

### 10.5. Using DSLs (The `dsl` Keyword)
While `quote` generates *Nikaia* code, the `dsl` keyword embeds **foreign syntax** directly into a Nikaia file — SQL, HTML, regex, assembly.

**The protocol:**

1.  **Explicit termination.** A DSL block ends with `} eod` ("end of DSL"). The core parser cannot find the end by counting braces — the body is foreign syntax where `}` may be a string character or absent entirely. Having scanned ahead for the marker, the compiler can skip the body and **parse the rest of the file in parallel** while the DSL parser works.
2.  **Scannerless delegation.** The core hands the isolated byte slice to the grammar. There is no global lexer to disagree with.
3.  **Hybrid binding.** DSL authors choose per hole between a compile-time capture and a runtime parameter (see below).
4.  **Subject `;` Config.** Runtime parameters are *configuration*, so they are passed as named arguments after the `;` — the rule from Part I, 5.1, without exception.

**Hybrid Binding: immediate vs. deferred**

| Intrinsic | When it resolves | Use for |
| :--- | :--- | :--- |
| `meta::capture(id)` | compile time, from the surrounding scope | assembly operands, table names — anything meaning *this variable, here* |
| `meta::parameter(name, type)` | runtime, as a named argument | SQL placeholders — anything the statement should be *reusable* over |

Both are needed. Capturing a variable is exactly right for assembly. It is exactly wrong for a SQL prepared statement: baking the values into the statement definition destroys the reuse that makes preparing it worthwhile.

> **A recurring idea.** This immediate/deferred split is the same distinction the language already draws for lambda capture (`@immediate` borrows, `@detached` moves — Part I, 5.4) and for resource teardown (`task::scope` finishes now, a cancelled cleanup is parked — [ADR-006](adr/adr-006.md), D3). When you meet it a fourth time, it will mean the same thing: *does this resolve here, or later?*

**Example: Embedding SQL**
Instead of writing SQL as a string (where a typo survives until runtime), it is verified at compile time.

```nika
use nikaia_sql::{Database, mysql}

fn query_users(db: Shared[Database], min_age: i32) {
    // 1. Define the statement.
    // The compiler parses this SQL, sees ':target_age' (a parameter hole
    // declared by the grammar) and generates a specialized shadow type.
    let query = dsl mysql {
        SELECT name, email
        FROM users
        WHERE age >= :target_age
    } eod

    // 2. Execute with typed parameters.
    // Subject: none (method on self) ; Config: target_age
    // Omitting 'target_age' is a compile error - the compiler knows the
    // statement needs it, because it parsed the statement.
    let users = query.execute(; target_age: min_age)
}
```

Library authors accept those parameters with the **typed spread**:

```nika
impl SqlParser {
    // Subject: self (the parsed statement) ; Config: the DSL's parameters
    pub fn execute(self; ...args: Self::dsl) -> List[Row] throws {
        return self.conn.query(self.sql, args.values())
    }
}
```

`Self::dsl` is the constraint proving these named arguments belong to this grammar. The compiler monomorphizes `args` per DSL string and allocates it on the stack — no heap traffic per query.

**What the call site is checked against.** The statement's parameters are read
out of the body as written, so the two ways of getting a call wrong are both
errors in the `.nika` file:

```text
error[NK1112]: `query` needs `:target_age`, and this call does not pass it
  --> users.nika:17:5
  17 |     let users = query.execute(; targt_age: min_age)
           ^
     = the statement's parameters are `:target_age`
     help: pass it after the `;`: `target_age: …`
error[NK1113]: `query` has no parameter `:targt_age`
  --> users.nika:17:5
  17 |     let users = query.execute(; targt_age: min_age)
           ^
     = the statement's parameters are `:target_age`
     help: did you mean `target_age`?
```

A `:name` is a hole because the body wrote `:` and a name: a path (`a::b`) and a
time (`12:30`) are not holes, and the same `:name` twice is one parameter. Which
holes a body has is the **grammar's** answer, and until a grammar can give one
the compiler reads them off the text - so a colon and a name inside the foreign
language's own string literal (`SELECT ':id'`) counts as a hole. The cost is a
call that has to pass a parameter which is not really one, and the message names
it; nothing compiles to something other than what it says.

> **Status.** The shadow type, the check above and `...args: Self::dsl` are
> built ([ADR-007](adr/adr-007.md) D5). Two things in the code on this page are
> not. **`args.values()`** needs the parameters to have a *type*, which is what
> `meta::parameter(name, type)` declares — and no `grammar` can declare one yet,
> so a driver receives the parameter struct and can pass it on but not read the
> values out of it by anything but their names. And the target of a `dsl … { … }
> eod` block is not resolved to a grammar at all: a body with `:name` holes
> lowers whatever the target is called, its text reaching the driver with the
> holes as written, and a body **without** holes is refused rather than given a
> meaning this compiler would have to invent. And the grouped `use
> nikaia_sql::{Database, mysql}` above is not built: a `use` takes one path
> (Part I, 9.1), so a brace after `::` is a parse error.

### 10.6. Advanced Parser Features
Nikaia grammars are designed for high-performance tooling.

**Zero-Copy Parsing**
Traditional parsers copy text into a new `String` for every identifier they find. Nikaia never copies the source text. Two mechanisms share that job, and the grammar picks per rule:

*Identifiers and keywords → interned symbols.* Identifiers are short, repeat constantly, and are compared far more often than they are read. The parser **interns** them: each distinct spelling is stored once in a shared table and the parser yields a small `Symbol` handle. Comparing two identifiers becomes an integer comparison, and a file with a thousand uses of `count` stores the text once.

*Bulk text (string literals, comments, doc text, DSL bodies) → tethered slices.* This text is long, rarely repeated, and rarely compared, so interning would only waste table space. The parser yields **slices** into the original buffer. Whether that slice stays a plain reference or becomes a **tether** — a handle on the source buffer plus a position — follows the "transient = borrow, escaping = tether" rule (Part I, Chapter 6.6): a token that lives and dies inside the scope holding the source text costs nothing at all, and one that outlives it is tethered automatically. An AST handed back to a caller is the second case, so it keeps its source alive; the handle sits on the AST, not on each of its tokens ([ADR-008](adr/adr-008.md), D4).

For tethered slices the guarantee is stated positively: **the source text cannot be freed while any token still points into it.** You never manage this relationship, and you never see an error about it — the buffer's lifetime simply follows the tokens. The cost is one shared handle per *container* — for the whole AST, not per token, and nothing per token that never escapes — which is negligible next to the avoided string copies.

> **Design Note (implementation):** Tethering is the architecture the async ecosystem converged on for zero-copy I/O (`bytes::Bytes` in Rust: a reference-counted buffer plus offsets); interning is what compiler front-ends converged on for symbol tables. A future optimization may replace the tether's reference count with compiler-verified self-referential storage — the driver controls all access patterns and could prove the invariants itself — but that is an internal optimization avenue, not a semantic change. Because grammars compile to direct byte-stream consumers, a parser may also use SIMD matching specialized to its own syntax. See [ADR-005](adr/adr-005.md) D4 and [ADR-007](adr/adr-007.md) D8.

**What a rule is called, when it fails where it began**

A grammar rule with several alternatives fails by reporting what each of them
could have started with. That is true and, past three or four alternatives,
useless: `expected one of: "!", "-", "dsl", "false", "spawn", "true"` where
the word *expression* belongs. A rule may therefore name itself, between its
return type and its `=`:

```nika
rule expr -> Expr # "expression" =
      c:closure_expr -> { c }
    | e:catch_expr   -> { e }
```

The name replaces the list **only where the rule failed at its own starting
position** — where none of its alternatives got anywhere and there is nothing
more specific to say. A rule that got further was in the middle of something,
and what it was in the middle of is the better message: `(1` reports the
missing `)`, not `expected expression`.

The same syntax after a single alternative names that alternative, which is
the narrower case of the same thing.

**Fault Tolerance (Recovery)**
When building tools like Language Servers (LSP), the parser must not crash on the first error. It needs to recover and continue parsing the rest of the file. A grammar declares **synchronization points**: if a rule fails, the parser discards input until it reaches the sync token, then resumes.

```nika
// If parsing fails inside the block, skip ahead to the closing brace
// and carry on - the errors after this point are still worth reporting.
rule block -> Vec[Stmt] =
    "{" => statements:recover(stmt, "}")* "}"
```

Note the commit point (`=>`): once the opening brace matched, this *is* a block, so a failure inside it is reported rather than causing the whole alternative to be abandoned. Commit points and recovery work together — the first decides where errors are worth reporting, the second decides where to resume.

### 10.7. Parallel Parsing

A parser that reads a file end to end uses one core. For a multi-gigabyte input that is the whole cost of the program. Nikaia parallelises the *parse itself* — but only when the grammar has said the two things that make it safe.

**First: where may the file be cut?** A rule marked `@frame` declares that it can be found from an arbitrary offset by scanning to the next **boundary**, with no knowledge of what came before:

```nika
// Up to ";" - or to the end of the frame, whichever comes first. `frame_end`
// is the boundary of the frame this rule is reached from: written once, in
// the attribute below, and referenced here.
rule NAME -> &str = s:until(";" | frame_end) -> { s }

@frame(boundary: "\n")
rule MEASUREMENT -> Reading =
    name:NAME ";" => temp:TENTHS frame_end -> { Reading { name, temp } }
```

A bare `@frame` takes the boundary from the rule's trailing literal; a frame that ends in `frame_end` names it in the attribute. A frame must end in its boundary, and it is written **lexical** (uppercase) — the implicit whitespace of a syntactic rule would eat newlines, and a data format with no whitespace between its fields is lexical anyway.

**This is checked, not believed — and never rewritten.** Cutting at the next boundary is only correct if the boundary cannot appear *inside* a frame. The compiler walks everything the rule can reach, and what consumes input is one of two things:

* **Safe** — a literal without the boundary in it; a built-in that cannot produce it (`digit1`, `ident`); lookahead; an `until(…)` whose terminator *covers* the boundary, as `NAME` above does with `frame_end`.
* **Rejected**, with the rule and the pattern named — a literal that contains the boundary (the classic case is CSV with quoted fields, where a newline inside `"…"` would put the cut in the middle of a record); a built-in that can consume it (`any`, `multispace0`); a syntactic rule; an `until(…)` that does not cover the boundary, where the error says what to add; and `recover(…)`, whose skip cannot be kept inside a frame — recover per frame instead: `(item | until(frame_end)) frame_end`.

Nothing here changes what a pattern means. `until(";")` consumes up to the next `;` wherever it is written; inside a frame that is a mistake, and the compiler says so and says what to write. The parser generated for a rule is the same whether or not a frame reaches it. You get a compile error, rather than a wrong total on some inputs and not others.

**When the check cannot see through the format.** A boundary is a byte string, and some formats cannot be cut by searching for one — CSV with quoted newlines, records recognisable by how they *start*, escaped boundaries. `@frame(boundary: "\n", unchecked)` skips the check: you assert the invariant, as with `unsafe`, and the word is there to grep for. The attribute is a keyed list so that each of those formats can get its own key later (`quote`, `start`, `escape`, `scan`) without changing what the existing ones mean.

**Second: how do two pieces combine?** The entry rule folds with a **merge**:

```nika
pub rule file -> Summary =
    par_fold(MEASUREMENT, Summary::new, fn(acc, m) { acc.record(m) }, Summary::merge)
```

`par_fold` is `fold` plus that merge, and it must be the whole body of its rule. With both declarations in hand the compiler generates the rest: the file is cut into one piece per core, each piece repairs its own start to the next boundary so that every frame belongs to exactly one worker, each worker folds into its own accumulator with nothing shared, and the accumulators are merged at the end. A failing piece reports its error at its position in the whole file. Nothing about chunks appears in your program:

```nika
let data = fs::map(path)
let totals = dsl Measurements from data
```

**Pieces and the whole agree — always.** A `par_fold` rule's parser is the per-piece parser, so it skips no whitespace at its entry, unlike every other rule: whitespace skipped there would be skipped at every cut rather than once. A frame that begins with a space keeps it; whitespace-only text between two frames is an error, in pieces and in one go alike. That is what makes the number of cores unable to change the answer — on inputs the grammar accepts and on inputs it rejects.

**Why you have to ask for it.** The compiler will not turn a `fold` into a `par_fold` on its own, even when it looks associative. Adding `f64` is not associative, so the number of cores would quietly change the answer. Writing `par_fold` is you saying that a different chunk count is the same result to you — for an aggregation over integers, as above, it is.

At `user_parallelism = no`, `par_fold` runs as an ordinary sequential `fold`: same accumulator, same merge, same result, no threads. A grammar written this way compiles unchanged for `wasm32`.

**What you get for free.** Because the grammar states the format, the generated parser is allowed to exploit it: scanning for a separator or a frame boundary works a machine word at a time rather than byte by byte, on every target and with no `unsafe` in sight — and a terminator with up to three alternatives, like `until(";" | frame_end)`, is still one scan. See [ADR-009](adr/adr-009.md).

## Chapter 11: Running Your Code at Once

Setting `user_parallelism` to `yes` (1.2) turns the same source into a program that uses more than one core. Nothing in the source changes.

### 11.1. Implicit Async & The Scheduler
The syntax is identical either way. You do **not** use `async` keywords on function definitions.
* **At `no`:** functions yield on I/O events, cooperatively, on one thread.
* **At `yes`:** the runtime uses a **Work-Stealing Scheduler** and distributes tasks across the cores it is allowed.

The code remains "Direct Style". You write code as if it were synchronous, and the compiler handles the suspension points.

### 11.2. Parallelism via spawn
Since code is implicitly async, "calling a function" usually means "running it now". To run tasks concurrently or in parallel, you strictly use `spawn`.

#### The @detached Contract
The `spawn` function is defined with the `@detached` attribute. This triggers **Implicit Move Semantics**.
* **Why?** This guarantees thread safety where code runs in parallel, and prevents logic races or "Use-After-Free" where it does not. The parent scope cannot access the captured data while the detached task owns it.
* **Copying:** If you need to keep data in the parent thread, you must explicitly call `.clone()` before spawning.

#### What a task may take with it
A task runs on a thread of its own, so **everything it uses has to be able to cross a thread**. The compiler checks that structurally, with no syntax to write and no annotation to forget ([ADR-005](adr/adr-005.md) §1 Group B): a type built out of plain data, and a struct or collection of those, may cross; a value that counts its owners may not, because at `user_parallelism = no` that count is one only a single thread may touch (Part I, 6.2). A struct with one such field is no better than the field.

The answer is **the same at both settings**, so that a library written at one cannot turn out un-compilable where it is used. What the setting changes is only whether this build performs the crossing: at `no` nothing you wrote runs concurrently, so the task does not run and the refusal is a lint rather than an error. The diagnostic is `NK2501`, worked through in Part III C.5.

> **Status.** The check runs; `spawn` itself does not yet lower, because the runtime integration it needs is the next step. So `NK2501` is reported ahead of the construct it is about — which is the right order, since the check is what has to exist before a value that may not cross one does.

#### Return Values & Handles
`spawn` always returns a `TaskHandle`. At `yes` it represents a running thread; at `no` a scheduled event. Calling `.await` or `.join()` on it works identically either way.

```nika
fn process_image(path: String) -> Image { ... }

fn main() {
    let img_path = "a.jpg"

    // 'spawn' is a @detached context.
    // 'img_path' is implicitly moved into the task.
    let handle = spawn fn { process_image(img_path) }

    // Compiler Error: img_path is gone.
    // println("Processing: " + img_path) 

    // Uniform API: the same at either `user_parallelism`
    let result = handle.await catch { return }
}
```

---

## Chapter 12: Thread Safety and Synchronization

Because code at `user_parallelism = yes` runs on several physical CPU cores at once, strict safety rules apply to prevent data corruption.

### 12.1. The `sync` Keyword (CPU Constraints)
Since everything in Nikaia is "Async by Default" (interruptible), we need a way to define code that **must not be interrupted** or moved between threads mid-execution.

The `sync` keyword allows you to mark a function as a **pure CPU task**.
* **Constraint:** A `sync` function can **only** call other `sync` functions.
* **Prohibition:** It cannot call standard functions (which might perform I/O).

This effectively creates two worlds: the flexible **Async World** (Default) and the strict **Sync World** (Computation).

```nika
// 'sync' guarantees: I will never pause, I will never do I/O.
fn calculate_physics(obj: Object) sync {
    obj.x += obj.velocity 
    // fs::read("log.txt") // Compiler Error: Forbidden I/O in sync context
}

// Usage in Parallel Iterator
// par_iter requires a 'sync' closure because it runs purely on CPU cores.
// A trailing lambda goes outside the parentheses.
particles.par_iter().for_each fn { calculate_physics(a) }
```

**You do not have to write the word for the rule to be satisfied.** A function
that calls nothing which can pause *is* a pure CPU task, and the compiler works
that out from the body rather than waiting to be told ([ADR-027](adr/adr-027.md)).
So the helper below needs no annotation to be callable from a `sync` one, from
`access` (12.2), or from a `par_iter` body:

```nika
fn bonus(score: i32) -> i32 { score * 2 + 1 }   // no `sync`, and cannot pause

fn total(p: &Player) -> i32 sync {
    return p.score + bonus(p.score)             // fine: `bonus` provably cannot pause
}
```

This matters more than it looks. Every construct in this chapter that makes
concurrency safe does it by demanding a `sync` lambda — `access`, `access_all`,
`par_iter`, a scope's parallel tasks, the panic hook. If `sync` were
something you had to *enter*, the set of things those lambdas could call would
be "whatever somebody remembered to annotate", and the safe path would be the
narrow one. It is the other way round: the safe path is open by default, and it
closes only where something really can pause.

**A library function that runs your lambda does what your lambda does.** `xs.map`,
`xs.filter`, `xs.sort_by_key` and the rest cannot commit to one answer — the
same `map` cannot pause over `fn { a + 1 }` and can over a lambda that reads a
file — so their contracts say *the lambda decides* ([ADR-029](adr/adr-029.md)).
That is why this is fine:

```nika
counter.access fn { a + xs.sort_by_key fn { b } }   // pure lambda, pure call
```

and this is still refused:

```nika
counter.access fn { xs.map fn { fs::read("log") } } // the lambda does I/O
```

You never write anything for this. It matters because without it *no* iterator
method could appear inside `access` or `par_iter`, whatever its lambda did.

**So what is the keyword for?** The same thing `@borrowed` is for in Part I,
6.6: saying a property out loud so that losing it becomes an error rather than a
silent change. Write `sync` where the promise matters to you — the inner loop
that must not acquire an I/O dependency, the API you publish as pure — and the
compiler holds the body to it (`NK2202`). Write nothing, and a body that
qualifies still gets the benefit; a body that stops qualifying just stops, at
the call that now needs it.

Where the compiler **cannot tell**, it does not guess in your favour: a call it
cannot resolve costs the function its inferred promise, because that promise is
written into the ledger and shipped, and a wrong one puts a pausing body inside
somebody's lock. Writing `sync` yourself is how you overrule that — an
assertion, checked as far as the compiler can see, and yours where it cannot.

### 12.2. The Dual Nature of `Locked[T]`
To share mutable data, you use the `Locked[T]` type. Its implementation follows `user_parallelism`, providing "Zero Cost Abstraction" relative to the requirements.

**At `user_parallelism = no`:**
* **Implementation:** Similar to a `RefCell` with a reentrancy check.
* **Cost:** Extremely cheap (integer increment).
* **Purpose:** It protects against **Logical Deadlocks** (e.g., Task A locks data, waits for network, Task B tries to lock same data -> Panic!). It does not use OS primitives.

**At `user_parallelism = yes`:**
* **Implementation:** A real OS-level **Mutex** (Mutual Exclusion).
* **Cost:** Higher (Atomic operations).
* **Purpose:** It protects against **Memory Corruption**. It ensures that two physical threads cannot write to the memory address at the same time.

**The `sync` Rule (No Pausing While Holding a Lock)**
Holding a lock while the program pauses is dangerous *either way*: with threads it can block a whole CPU core; without them it can freeze other tasks that need the same data. Nikaia rules this out **at compile time**, using a keyword the language already has:

> **`access` and `access_all` require a `sync` lambda — at every setting.**

A `sync` lambda (see 12.1) can never perform I/O and can never pause. Therefore, while you hold locked data, the program provably runs straight through: lock, compute, unlock. There is nothing to remember — if you try to do I/O inside `access`, the compiler stops you with a plain explanation:

```nika
let counter: Shared[Locked[i32]] = ...

// OK: pure computation
counter.access fn { a += 1 }

// Compiler Error: I/O inside a lock
// counter.access fn { fs::write("log", "{a}") }
```

```text
error[NK2201]: cannot wait for I/O while holding locked data
  --> main.nika:7
   |
 7 | counter.access fn { fs::write("log", "{a}") }
   |                    ^^^^^^^^^^^^^^^^^^^^^^ this writes to a file,
   |                                            which makes the program pause
   |
  note: while you hold locked data, every other task that needs it must wait.
        Pausing here could freeze them for a long time (or forever).
  help: copy the value out first, then do the I/O without holding the lock:
        let snapshot = counter.access fn { a }
        fs::write("log", "{snapshot}")
```

This turns the old advice "don't sleep while holding a lock" from a best practice into a guarantee. The runtime checks described above (a reentrancy check on one thread, poisoning on several) remain as a safety net for the remaining edge cases — e.g. accidentally re-entering the *same* lock through a chain of `sync` calls — but well-formed code never triggers them.

**And a lock is a resource, so two `access` blocks on the same lock keep their order.** Part I 8.1.1 says that two operations whose touch sets are disjoint have no order between them, and a lock is one of the things a touch set can name: both `access` blocks reach the lock and both change it, so they are ordered by the same rule that orders two `println`s, rather than by a rule of their own ([ADR-033](adr/adr-033.md) §3). Two `access` blocks on *different* locks meet on nothing and need not wait for each other.

> **Status.** The compiler does not yet know a lock as a named resource — no entry in any ledger
> claims one, because nothing in the corpus has asked for one
> ([ADR-028](adr/adr-028.md) D5: an entry exists because a program asked for it, never
> speculatively). Until one does, an `access` block is an operation whose touches cannot be
> determined, so it counts as touching everything and keeps its place. That is the same answer this
> section promises, reached by ignorance rather than by a contract — right today, and not something
> to rely on: the moment anything else in the pair is described, the contract is what has to say
> it.

Where you need an order between two locks, or between a lock and something else, that the touch sets cannot see, `seq { … }` (Part I 8.1.1) is how a program says so.

### 12.3. Deadlock Prevention: Atomic Composition
The classic cause of deadlocks is inconsistent locking order (Thread 1 locks A then B; Thread 2 locks B then A).
In Nikaia, trying to nest locks manually is considered an anti-pattern and often a compile-time error.

**The Solution: `access_all`**
Instead of nesting `access` calls, Nikaia provides `access_all` to request multiple resources simultaneously.

* **Mechanism:** The runtime sorts the resources by their memory address (or internal ID) before locking. This guarantees a globally consistent locking order across all threads.
* **Safety:** It is mathematically impossible to create a deadlock cycle between A and B if everyone uses `access_all(A, B)`, because everyone will implicitly lock "Lower Address first, Higher Address second".

```nika
let account_a: Shared[Locked[Account]] = ...
let account_b: Shared[Locked[Account]] = ...

// ERROR: Manual Nesting is forbidden to prevent Deadlocks.
// Taking one lock inside another is what creates the inconsistent order
// this section is about — a single `access` (12.2) is of course fine.
// account_a.access fn(from) {
//     account_b.access fn(to) { to.balance += 100 }
// }

// Atomic Locking (Deadlock Proof)
// The runtime sorts A and B internally and locks them safely.
// We use a trailing lambda block explicitly here.
access_all(account_a, account_b) fn(a, b) {
    // Both 'a' and 'b' are mutable guards here.
    let amount = 100
    a.balance -= amount
    b.balance += amount
}
```

> **Status:** `counter.access fn { … }` is built as a method call with a trailing
> lambda. The two forms above are not: a trailing lambda attaches only to a
> method call and only with implicit arguments (Part I, 5.3), so `account.access
> fn(to) { … }` and `access_all(…) fn(a, b) { … }` are each read as two
> expressions rather than one.

### 12.4. Racing Tasks (`select`)
Sometimes you want to run multiple tasks, but only care about the one that finishes *first*.

```nika
select {
    // Case 1: Computation finishes first
    result = heavy_math() => { return result }
    
    // Case 2: Timeout happens first
    _ = sleep(5.seconds()) => { throw TimeoutError("Too slow!") }
}
```
*Note: When one branch wins, the other task is automatically cancelled and cleaned up.*

> **Status:** not built. `select` is not a keyword in the parser and the block
> above is a parse error, so nothing races two tasks today and the teardown rule
> below is stated ahead of the construct it governs.

**What "cleaned up" means precisely:** the losing task stops at its current pause point and its values are torn down. Resources with a pausable `cleanup` (Part I, 6.4) cannot be awaited by the *winner* — you should not pay for the loser's teardown — so the runtime **adopts** their `cleanup` runs and finishes them in the background ("parked cleanup"). The program will not exit before parked cleanups are done, bounded by the `cleanup-deadline` (Part III, 13.3). Errors from a parked cleanup have no caller to bubble to; they are reported through the runtime's error hook. See [ADR-006](adr/adr-006.md), D3.

### 12.5. Channels (Message Passing)
Instead of locking shared memory, Nikaia encourages **Message Passing**.

```nika
// Subject: 100 (capacity)
let (tx, rx) = channel::bounded(100)

// Implicit Move transfers 'tx' into the background task
spawn fn {
    tx.send("Calculation complete")
}

let msg = rx.recv() // Waits (non-blocking yield) for the message
```

### 12.6. Data Parallelism (`par_iter`)
If you have a large list of data and want to process it using all CPU cores, use `par_iter` (Parallel Iterator).

```nika
let pixels = [/* 1 million pixels */]

// The compiler splits the array into chunks and distributes them
// across all cores. The closure must be 'sync'.
// Methods chained with trailing lambdas. A block ends at its `}`, so the chain
// continues after it and means what it reads as (ADR-022).
let bright_pixels = pixels.par_iter()
    .map fn { a.brightness * 1.5 }
    .filter fn { a > 0.5 }
    .collect()
```

### 12.7. Scoped Tasks (@immediate Parallelism)

**The idea in one sentence:** a normal `spawn` task takes your variables *with* it (because it might outlive you, Chapter 8.3) — a **scoped** task only *borrows* them, because the scope promises to wait until every task inside it is finished.

Think of it like this: lending your notebook to a colleague is fine *if they stay in the room and give it back before you leave*. That is what `task::scope` enforces: it is an **Immediate Context** (Chapter 5.4) — it does not return until all tasks spawned inside it have completed. Because of that promise, tasks inside the scope may simply read your variables: no moving, no cloning.

```nika
let data = [1, 2, 3]

// 'task::scope' waits for all inner tasks before it returns.
task::scope fn(s) {
    // Note: s.spawn is tied to the scope, unlike global spawn.
    s.spawn fn { println(f"Reading: {data}") } // Safe Borrow
    s.spawn fn { println(f"Reading: {data}") } // Safe Borrow
}
// 'data' is still valid here
```

> **Status:** not built. `task::scope fn(s) { … }` does not parse as one
> construct — a trailing lambda attaches only to a method call and only with
> implicit arguments (Part I, 5.3) — and `NK2102` is not reported.

**One rule differs with `user_parallelism`.** The promise "everybody gives the notebook back before you leave" is only enforceable if the runtime can actually wait the tasks out:

* **At `no`:** everything runs on one thread, and the runtime owns every task. When a scope ends (even when it is torn down early by an error), the runtime collects its tasks *before* your function's variables disappear. **Scoped tasks may do anything, including I/O.**
* **At `yes`:** tasks run on other CPU cores *in parallel*. A task that is mid-computation on another core cannot be stopped at an arbitrary moment — so the scope can only keep its promise for tasks that finish on their own, deterministically. Therefore: **above `0`, scoped tasks must be `sync`** (pure computation — no I/O, no pausing; the same rule as `par_iter`, 12.6). This is the natural fit anyway: scoped parallelism exists exactly for "split this computation across all cores".

If a task needs to do I/O there, it does not belong in a scope — it is a background task. Use a normal `spawn` (the task takes ownership, Chapter 8.3) and collect the result through its handle. The compiler explains this when you hit the rule:

```text
error[NK2102]: tasks inside `task::scope` must be `sync` where they run in parallel
  --> worker.nika:12
   |
12 |     s.spawn fn { fetch_url(url) }
   |                 ^^^^^^^^^^^^^^ `fetch_url` performs network I/O
   |
  note: a scope promises to wait for its tasks. On multiple CPU cores this
        promise only holds for pure computations (`sync` functions), which
        finish on their own — a task waiting on the network might not.
  help: choose one of the following:
        1. keep it in the scope, but make the work pure:
           mark the function `sync` (and do the I/O before the scope)
        2. run it as a background task instead — it will take ownership
           of its variables (clone what you still need):
           let handle = spawn fn { fetch_url(url.clone()) }
           let result = handle.await
```

> **Design Note (Soundness):** Borrowing across *parallel, pausable* tasks is a known unsoundness trap — a cancelled scope cannot instantly stop a task mid-execution on another core, yet the borrowed variables are about to disappear. General-purpose async runtimes cannot offer a safe async scope for exactly this reason. Nikaia avoids the trap structurally: at `no` the single-threaded runtime owns all task state and tears scopes down synchronously (which additionally requires that task futures are exclusively runtime-owned and that the language exposes no way to leak a live scope — both are language-level guarantees); above `0` the `sync` restriction makes waiting deterministic. See [ADR-005](adr/adr-005.md), D5.

### 12.8. Supervision Trees
In complex systems, threads might crash (panic). A **Supervisor** monitors tasks. If a child task crashes, the supervisor can decide to:
* **Restart** the task.
* **Crash** the parent (escalate).
* **Ignore** the error.

This allows building self-healing systems.

```nika
// Subject: The Task (Lambda)
// Config: restart_policy (Protocol: separated by ;)
// Note: 'spawn' syntax (fn {}) is the subject.
supervisor::start_link(fn {
    server.run()
}; restart_policy: "always")
```
