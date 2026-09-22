# Nikaia Language Specification
**Part II: Advanced Features & Metaprogramming**
**Version:** 0.0.148 (Draft)
**Date:** 2026-09-22

---

## Chapter 10: Metaprogramming (Code that writes Code)

Metaprogramming extends the language from user code. Nikaia does not do it with text replacement, and it does not do it by writing code as data ([ADR-088](adr/adr-088.md)). Two mechanisms exist, and they are kept apart. A **`grammar`** reads a language the compiler does not know (10.1, 10.5). **`comptime`** reads what the compiler already knows: a type's shape (10.3). Nothing is generated behind user code.

> A piece of code can be understood by reading it, because nothing was generated behind it.

### 10.1. Parsing with `grammar` (Scannerless)
A `grammar` is a parser written declaratively. It reads a data format or a language the compiler does not know.

Nikaia grammars are **scannerless**: there is no separate tokenizer stage. A grammar consumes the raw character stream directly. A foreign language can therefore be embedded in a Nikaia file (10.5). SQL, assembly and Nikaia disagree about what a `}` or a `'` means, and a scannerless grammar tokenizes only its own body.

**Key features:**
*   **Commit points (`=>`)** control backtracking. Once the parser passes a commit point, it stays in that branch. A later failure is an error, not a reason to try the next alternative.
*   **Lexical and syntactic rules.** A rule whose name starts with an uppercase letter is **lexical**: no whitespace is skipped between its parts. A rule whose name starts with a lowercase letter is **syntactic**: whitespace is skipped. The distinction replaces the lexer/parser split.
*   **Typed actions (`{ … }` after the pattern).** Each rule builds the program's own types directly ([ADR-120](adr/adr-120.md) D2).

```nika
grammar Json {
    pub rule value -> Value =
        o:object { Value::Object(o) }
      | a:array  { Value::Array(a) }
      | s:string { Value::String(s) }

    // Commit point: once '{' matched, members and '}' MUST follow, or we error.
    rule object -> Object
        = "{" => members:list(pair, ",") "}"
        { Object { members } }

    rule pair -> Pair = key:string ":" => val:value { Pair { key, val } }

    // Lexical rule (uppercase): no whitespace inside a hex byte. Two hex digits
    // always fit in a `u8`, so this action cannot fail; where one can, what its
    // failure means is decided by the commit point above it
    // ([ADR-023](adr/adr-023.md) D9).
    rule HEX -> u8 = d:hex_digit{2} { hex_byte(d) }
}
```

> **Implementation status:** Not implemented. Auto-generated AST types (a struct for a labelled sequence, an enum for alternatives, where no `{ … }` is written) are decided and not built. An action block is required today ([ADR-007](adr/adr-007.md) D7).

### 10.2. Dual-Mode Parsing (Static vs. Dynamic)
A grammar defined once runs at compile time and at runtime, **with the same
syntax and the same meaning**. A grammar is entered by an **ordinary call**.
Every `pub` rule in it is an entry named after the rule
([ADR-082](adr/adr-082.md) D1, D2): `Json::value(x)` runs the rule `value` of the
grammar `Json`. The word in front of the binding decides *when* the call runs,
not the shape of the line.

**The separator is `::`, as it is for every other qualified name**
([ADR-140](adr/adr-140.md) D3). The dot is for a **value's** members.
`Json.value(x)` is refused with `NK1147`. `T::fields` in 10.3 follows the same
rule.

*Design rationale:* `Json.value(x)` and `text.value(x)` are the same five
tokens, and a namespace behind a dot would make a reader tell them apart by
what the left side happens to be ([ADR-140](adr/adr-140.md) D3).

> **Implementation status:** Implemented. `Json::value(input)` is the entry, and
> `Json.value(x)` is refused with `NK1147` by the checker, not by the parser
> ([ADR-140](adr/adr-140.md) §5).

**A. Static Embedding (Compile-Time)**
In a `comptime` binding the parser runs *during the build*. Invalid input fails
the build. The result is embedded in the binary at no runtime cost.

Three words each decide one thing. `comptime` says **when**. `asset("…")` says
**where the bytes come from** ([ADR-072](adr/adr-072.md),
[ADR-116](adr/adr-116.md)). The call says **what is done with them**. The form
`dsl Json from "config.json"` is withdrawn ([ADR-082](adr/adr-082.md)).

```nika
// The compiler runs the Json grammar at build time.
// If "config.json" is malformed, the build stops.
comptime CONFIG: Json::Value = Json::value(asset("config.json"))
```

**B. Dynamic Parsing (Runtime)**
The same grammar processes user input or network data while the program runs.

```nika
fn parse_input(input: String) throws {
    let data = Json::value(input)
    println(f"Parsed: {data}")
}
```

**The `comptime` declaration.** `comptime` declares a binding whose initialiser
is evaluated while the program is built ([ADR-073](adr/adr-073.md) D2; the word
is [ADR-077](adr/adr-077.md)). It stands where an item stands and inside a
function body. Its type may be written and may be omitted. What reaches the
emitted program is the value, not the expression: `comptime LIMIT = 4 * 1024`
arrives as `const LIMIT: i32 = 4096;`. A `let` may be folded while the program
is built; a `comptime` must be. A `comptime` binding the compiler cannot
evaluate is refused with `NK1127` ([ADR-073](adr/adr-073.md) D3). It is never
evaluated at run time instead.

**What an initialiser may hold** is staged ([ADR-073](adr/adr-073.md) D5). The
current stage is an integer, a `bool`, **text**, a **list**, a **pair** or a
**struct**: literals, `f"… {n} …"`, arithmetic and comparisons over them and
over other constants, `+`, `==` and `.len()` over text, `xs[i]`, `xs[i] = …`,
`xs.push(…)` and `xs.len()` over a list, a struct literal and a field of one, an
`if`, and a **call** to a function or a **method** this program declares — in
any of its files — whose body is made of those. A called body may recurse with a
base case, and it may loop: a `for` over a range, a `while`, `break` and
`continue`. A called body must be `sync` and must touch nothing but the build's
own parameters ([ADR-075](adr/adr-075.md)). A callee that fails either condition
is refused with `NK1152`. `NK1127` says *not yet*; `NK1152` says *not allowed*.
What a build-time body may do beyond this stage is decided by
[ADR-026](adr/adr-026.md) Q4.

A `comptime` may read one declared below it: a constant is an **item**, and
items are order-independent. A ring of them has no base case to reach and is
refused with `NK1168`.

A loop has no step budget ([ADR-075](adr/adr-075.md) D4). A `while` that does
not end hangs the build. Call depth is bounded, so an unbounded recursion does
not exhaust the compiler's stack.

**What crosses from build time to run time** is decided
([ADR-079](adr/adr-079.md)). A result arrives in its **view** form: `Vec[T]` as
a `ref Array[T]` — or as an `Array[T, N]` where the program writes the length into the
type ([ADR-152](adr/adr-152.md), [ADR-179](adr/adr-179.md) D1) — and `String` as
a `ref String`. A value built with `push` is fixed once
it has crossed. A value that owns memory is refused by what it *is* and never by
its parse — by its **type** for a `struct`, and for an `enum` by the **variant
the value is**, because `Shape::Empty` is a `const` and `Shape::Many([1, 2])` is
not while both are the same `Shape`. A map that crosses is a **fixed** map with
a closed key set. How it is looked up is the compiler's decision, as a map's
hasher is.

**A map the build can see is written as a list of pairs**, and the declared type
says it is a map ([ADR-176](adr/adr-176.md) D1). There is no map literal:

```nika
comptime ROUTES: Fixed[ref String, i64] = [("get", 1), ("post", 2)]
```

`Fixed[ref String, V]` is the fixed map, a type in `std`. `ROUTES.get(k)` is a `V?`.
Keys are text; a key of any other type is refused with `NK1170`, and a key
written twice with `NK1169` — a table has one value per key, and there is no
meaning a compiler may pick between. **A value may be a `struct` or an `enum`
this program declares**, and what the program then reads is a **view** of the
row ([ADR-180](adr/adr-180.md) D1): the row lives in the binary, so there is
nothing to copy it out of. `TABLE.get(k)?.field ?? …` is how a field of one is
reached, which is what a `T?` asks for anywhere (2.3). A list of text crosses as an array of
views, `[ref String; N]`, which is the same rule read over both shapes at once.

**Reading a file while the program is built** ([ADR-072](adr/adr-072.md)). A
build given no allowlist reads nothing (D1). A file the build reads is named
three times: in the source, as the `asset("…")` literal; in an allowlist file,
one path per line; and in the invocation that puts the list in effect,
`--allow-read-from-list=…` (D3). The path may not be computed (D4). There are no
patterns (D5). What `asset("…")` comes to is the file's **text**, which crosses
to the program as the `ref String` a `const` holds
([ADR-079](adr/adr-079.md) D1); bytes that are not UTF-8 are refused rather than
converted. The list binds the whole build, a dependency's read included (D6).

*Design rationale:* two of the three namings are committed, so adding a read is
a diff a reviewer sees; the third lets a build run with compile-time reading
switched off without editing anything ([ADR-072](adr/adr-072.md) D3).

> **Implementation status:** Partially implemented. B is implemented: a grammar
> name stands where a callee stands, every `pub` rule is an entry, and the
> withdrawn `dsl X from e` is refused with a message naming the call form
> ([ADR-082](adr/adr-082.md) §5). `comptime` is implemented at item level and
> inside a function body, for the whole stage named above — calls across the
> files of a program, methods, loops, `push`, text, arrays and the fixed map
> included — and so is the crossing of [ADR-079](adr/adr-079.md)
> ([ADR-175](adr/adr-175.md), [ADR-176](adr/adr-176.md)). **`asset("…")` and
> the allowlist are implemented** ([ADR-072](adr/adr-072.md) §4):
> `comptime CONFIG: ref String = asset("config.txt")` is a `const` holding the file's
> text, `NK1175` says which of the three namings a refused read is missing,
> `NK1176` refuses a path the build worked out, and `NK1177` refuses an `asset`
> written outside a `comptime` with the name of the run-time read.
>
> **And A is implemented** ([ADR-177](adr/adr-177.md)): a grammar runs here by
> **compiling the parser it generates** rather than by interpreting the grammar,
> so there is one implementation of the grammar language and this section's *the
> same syntax and the same meaning* is a tautology. Invalid input fails the build
> in the parser's own words. What A's own example still meets is the
> **crossing**: a build-time value is a whole number, a float, a `bool`, text, a
> list, a `struct` and an `enum` variant, and a rule whose result is one of the
> seven crosses and runs — including into a `ref Array[T]`, which is what a run of
> `struct`s arrives as ([ADR-179](adr/adr-179.md) D1, D3). What a rule may
> **not** hand back is a value it built into a position that views a run, and
> `NK1179` says so on the action's own line.
>
> Not implemented is [ADR-079](adr/adr-079.md) D5's serialised blob, which waits
> for a table large enough to ask for it.

### 10.3. Generating Code from a Type's Shape
Where 10.1 reads data, this section reads **types**. A function such as
`describe` is written once and works for every struct.

**There is no macro system** ([ADR-088](adr/adr-088.md) D1): no `macro`, no code
written as data, nothing attached to a declaration. A type's shape is
**ordinary data**, and a loop over it runs while the program is built.

```nika
fn describe[T: Struct](value: T) {
    for field in T::fields {
        println(f"{field.name} = {field.of(value)}")
    }
}
```

Every piece of that is something the language has elsewhere.

**The bound makes `T::fields` exist.** `T: Struct` is an ordinary bound (4.7),
answered from a declaration as every other bound is. It says that a shape may be
asked for. A `T: Enum` has `T::variants`. There is no builtin and no special
syntax: **reflection is reached as a member**, and `field.of(value)` is a method
on the reflected field. The separator is `::` (10.2, [ADR-140](adr/adr-140.md) D3).

A parameter with no bound could be handed anything, and the caller would learn
that its type does not fit only at a line deep in the body. With the bound, a
caller that passes something that is not a struct is refused **once, at the
call**. What remains inside the body is per-field.

**The loop needs no second keyword.** `T::fields` is known while the program is
built, so a loop over it is unrolled. There is no run-time reading to rule out.
The loop means what a loop always means: *walk these elements*. Only the
**stage** is earlier. A construct whose operand changes meaning with its position
is withdrawn ([ADR-082](adr/adr-082.md)).

**The body is checked once per unrolled turn.** `field.of(value)` is a `String`
for one field and an `i32` for the next, so there is no single type to check the
body against. At run time this costs **nothing**: what remains is the `println`
calls a program would have written by hand, with no loop and no dispatch. At
build time it costs fields × the types actually used. An ordinary `for x in xs`
over a run-time list is checked once.

A body that is wrong for one field is wrong at one unrolled copy, on a line that
is correct for the others. The diagnostic names the field:

```text
error: `println` cannot format a `Vec[u8]`
  --> describe.nika:3:9
     = unrolling `T::fields` for `User`, at field `avatar`
```

**What cannot be read is printed** (D6). `nikaia --comptime` prints what was
unrolled, for the types actually used, as `--overlaps`, `--sharing`,
`--tethers` and `--trust` print their own analyses. There is no syntax for it.
A function that walks a shape and is **never called** has a line of its own,
because no copy is written for it and nothing else says it exists.

`macro`, `quote` and `with` are **reserved words and not constructs**
([ADR-088](adr/adr-088.md) D7). A program may not use them as names. The macro
system built out of the three is withdrawn ([ADR-088](adr/adr-088.md)).

> **Implementation status:** Implemented but for `T::variants` and
> `--comptime`. **The bound is built** ([ADR-088](adr/adr-088.md) D2):
> `[T: Struct]` and `[T: Enum]` are bounds this compiler answers, and what
> answers them is the **declaration** rather than an `impl`. So is the refusal
> the bound exists to make — a caller that passes something which is not a
> struct is `NK1164` **at the call** (D3). Neither bound reaches the language
> below, which has no trait by either name.
>
> **And what the bound reaches is built since 0.0.129**
> ([ADR-181](adr/adr-181.md)): the example above compiles and runs. `T::fields`
> is a list of reflected fields, the loop over it is **unrolled once per type a
> call gave the function**, and what is emitted is one copy per type with no
> generic original and nothing left at run time. A function walks a shape when
> its **body** says so: a `[T: Struct]` function that never writes `T::fields`
> is an ordinary generic one. The body is checked once per turn and every
> message carries the field it came from; a member a reflected field does not
> have is `NK1180`.
>
> **D6's `--comptime` is built since 0.0.130**, printed once for the program
> because an unrolling is a fact about a **call** — and a call may stand in a
> different file from the function it names, which is the same reason the
> unrolling itself reaches across the files of a package (Part I 9.1).
> **`T::variants` is not built** and is
> `NK1171`, whose sentence says which half is which: an `enum`'s shape is a
> different value, because a variant carries a payload where a field carries a
> type. `macro`, `quote` and `with` are reserved
> ([ADR-088](adr/adr-088.md) §5).

Capturing an *expression* as a tree, `u.age > 18` for a provider to translate,
is not part of the language. A query is written in the database's own SQL and
checked by the driver while the program is built (10.5,
[ADR-143](adr/adr-143.md) D5). Reflection describes types and does not capture
expressions ([ADR-088](adr/adr-088.md) §3).

### 10.4. Hygiene, and Why the Question Dissolved
A macro system needs a hygiene rule: whether a `temp` bound by generated code
collides with the `temp` the caller already had, and a way to break the rule
where a macro is meant to hand a name back.

Nikaia has no hygiene rule ([ADR-088](adr/adr-088.md) D1). Nothing is injected
from elsewhere. The loop body of 10.3 is written by the program, in place; there
is no second author whose names could arrive unannounced. A name bound inside
that loop is bound where it is written, as a name bound in any other block is.

> Nikaia has no such rule, because it has no such question.

Hygiene by default, and the `@`-prefixed escape that injected a name into the
caller's scope, are withdrawn with the macro system ([ADR-088](adr/adr-088.md)).

### 10.5. Using DSLs (The `dsl` Keyword)
The `dsl` keyword embeds **foreign syntax** in a Nikaia file: SQL, HTML, regex, assembly. 10.3 walks a shape the compiler already knows; `dsl` hands a byte slice to a grammar that knows a language the compiler does not.

**The protocol:**

1.  **Explicit termination.** A DSL block ends with `} eod` ("end of DSL"). The core parser does not find the end by counting braces, because the body is foreign syntax in which `}` may be a string character or absent. The compiler scans ahead for the marker, skips the body, and **parses the rest of the file in parallel** while the DSL parser works.
2.  **Scannerless delegation.** The core hands the isolated byte slice to the grammar. There is no global lexer.
3.  **Hybrid binding.** A DSL author chooses per hole between a compile-time capture and a runtime parameter (below).
4.  **Subject `;` config.** Runtime parameters are *configuration*. They are passed as named arguments after the `;` (Part I 5.1).

**Hybrid Binding: immediate vs. deferred**

| Intrinsic | When it resolves | Use for |
| :--- | :--- | :--- |
| `meta::capture(id)` | compile time, from the surrounding scope | assembly operands, table names — anything meaning *this variable, here* |
| `meta::parameter(name, type)` | runtime, as a named argument | SQL placeholders — anything the statement should be *reusable* over |
| `meta::column(name, type)` | build time, declared by the grammar | the statement's **result**: one field per column, so a row is a type ([ADR-143](adr/adr-143.md) D2) |

Capture is right for assembly. Capture is wrong for a SQL prepared statement, because baking the values into the statement destroys the reuse that makes preparing it worthwhile. Both intrinsics are therefore needed.

`meta::column(name, type)` gives a grammar control over the statement's result type. The compiler builds the row type from the declared columns, one typed field per column, as it builds the parameter type from the holes. A database driver's grammar therefore reads the statement and the schema and declares one column per result column. The schema is a build-time argument of the block, `dsl sqlite(schema: app) { … } eod`, resolved from a `comptime` value. A column the schema lacks is refused by the grammar at the query, while the program is built. The compiler does not understand SQL; the driver owns the grammar, and a vendor's database is a package ([ADR-143](adr/adr-143.md)).

The immediate/deferred split is the distinction the language draws for lambda capture (`@immediate` borrows, `@detached` moves; Part I 5.4) and for resource teardown (`task::scope` finishes now, a cancelled cleanup is parked; [ADR-006](adr/adr-006.md) D3). The question is the same in each place: *does this resolve here, or later?*

**Example: Embedding SQL**
SQL written as a string carries a typo to run time. SQL written in a `dsl` block is verified while the program is built.

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
    // Subject: none (method on self), so no `;` - only the option (Part I 5.1)
    // Omitting 'target_age' is a compile error - the compiler knows the
    // statement needs it, because it parsed the statement.
    let users = query.execute(target_age: min_age)
}
```

A library accepts those parameters with the **typed spread**:

```nika
impl SqlParser {
    // Subject: self (the parsed statement) ; Config: the DSL's parameters
    pub fn execute(self; ...args: Self::dsl) -> Vec[Row] throws {
        return self.conn.query(self.sql, args.values())
    }
}
```

`Self::dsl` is the constraint that ties these named arguments to this grammar. The compiler monomorphizes `args` per DSL string and allocates it on the stack. There is no heap traffic per query.

**What the call site is checked against.** The statement's parameters are read
out of the body as written. A call that omits one is refused with `NK1112`. A
call that passes a name the statement has no hole for is refused with `NK1113`:

```text
error[NK1112]: `query` needs `:target_age`, and this call does not pass it
  --> users.nika:17:5
  17 |     let users = query.execute(targt_age: min_age)
           ^
     = the statement's parameters are `:target_age`
     help: pass it by name: `target_age: …`
error[NK1113]: `query` has no parameter `:targt_age`
  --> users.nika:17:5
  17 |     let users = query.execute(targt_age: min_age)
           ^
     = the statement's parameters are `:target_age`
     help: did you mean `target_age`?
```

A `:name` is a hole where the body wrote `:` and a name. A path (`a::b`) and a
time (`12:30`) are not holes. The same `:name` twice is one parameter. Which
holes a body has is the **grammar's** answer. Until a grammar can give one, the
compiler reads the holes off the text, so a colon and a name inside the foreign
language's own string literal (`SELECT ':id'`) count as a hole. Such a call has
to pass a parameter that is not one, and the message names it; nothing compiles
to something other than what it says.

> **Implementation status:** Partially implemented. The shadow type, `NK1112`,
> `NK1113` and `...args: Self::dsl` are implemented ([ADR-007](adr/adr-007.md)
> D5). `args.values()` is not, because no `grammar` can declare the type
> `meta::parameter(name, type)` names yet: a driver receives the parameter
> struct, passes it on, and reads values out of it only by name. The target of a
> `dsl … { … } eod` block is not resolved to a grammar: a body with `:name` holes
> lowers whatever the target is called, its text reaches the driver with the
> holes as written, and a body without holes is refused. `meta::column` is not
> implemented ([ADR-143](adr/adr-143.md) §5). The grouped
> `use nikaia_sql::{Database, mysql}` is not implemented: a `use` takes one path
> (Part I 9.1), and a brace after `::` is a parse error.

### 10.6. Advanced Parser Features
Nikaia grammars serve tooling that reads large inputs.

**Zero-Copy Parsing**
A Nikaia parser never copies the source text. Two mechanisms share that job, and a grammar picks per rule:

*Identifiers and keywords are interned symbols.* An identifier is short, repeats constantly, and is compared more often than it is read. The parser **interns** it: each distinct spelling is stored once in a shared table, and the parser yields a small `Symbol` handle. Comparing two identifiers is an integer comparison. A file with a thousand uses of `count` stores the text once.

*Bulk text is a tethered slice.* String literals, comments, doc text and DSL bodies are long, rarely repeated and rarely compared, so they are not interned. The parser yields **slices** into the original buffer. Whether a slice stays a plain reference or becomes a **tether**, a handle on the source buffer plus a position, follows the rule of Part I 6.6: transient is a borrow, escaping is a tether. A token that lives and dies inside the scope holding the source text costs nothing. A token that outlives that scope is tethered automatically. An AST handed back to a caller is the second case, so it keeps its source alive; the handle sits on the AST, not on each of its tokens ([ADR-008](adr/adr-008.md) D4).

**The source text cannot be freed while any token still points into it.** User code does not manage the relationship and meets no error about it; the buffer's lifetime follows the tokens. The cost is one shared handle per *container*, for the whole AST and not per token, and nothing for a token that never escapes.

*Design rationale:* tethering is the architecture zero-copy I/O converged on (`bytes::Bytes` in Rust: a reference-counted buffer plus offsets), and interning is what compiler front-ends converged on for symbol tables. The tether's reference count may later be replaced by compiler-verified self-referential storage without a change in meaning, and a parser compiled to a direct byte-stream consumer may use SIMD matching specialised to its own syntax ([ADR-005](adr/adr-005.md) D4, [ADR-007](adr/adr-007.md) D8).

**What a rule is called, when it fails where it began**

A grammar rule with several alternatives fails by reporting what each of them
could have started with. Past three or four alternatives the list says less than
one word would: `expected one of: "!", "-", "dsl", "false", "spawn", "true"`
where the word *expression* belongs. A rule may name itself, between its return
type and its `=`:

```nika
rule expr -> Expr # "expression" =
      c:closure_expr { c }
    | e:catch_expr   { e }
```

The name replaces the list **only where the rule failed at its own starting
position**: where none of its alternatives consumed anything and there is
nothing more specific to say. A rule that got further was in the middle of
something, and that is the message: `(1` reports the missing `)`, not
`expected expression`.

The same syntax after a single alternative names that alternative.

**Fault Tolerance (Recovery)**
A tool such as a language server does not stop at the first error. It recovers and continues parsing the rest of the file. A grammar declares **synchronization points**: where a rule fails, the parser discards input until it reaches the sync token, then resumes.

```nika
// If parsing fails inside the block, skip ahead to the closing brace
// and carry on - the errors after this point are still worth reporting.
rule block -> Vec[Stmt] =
    "{" => statements:recover(stmt, "}")* "}"
```

The commit point (`=>`) and the recovery work together. Once the opening brace matched, the rule *is* a block, so a failure inside it is reported rather than abandoning the alternative. The commit point decides where an error is reported; the recovery decides where parsing resumes.

### 10.7. Parallel Parsing

A parser that reads a file end to end uses one core. For a multi-gigabyte input that is the whole cost of the program. Nikaia parallelises the *parse itself* when the grammar has declared the two things that make it safe.

**Where the file may be cut.** A rule marked `@frame` declares that it can be found from an arbitrary offset by scanning to the next **boundary**, with no knowledge of what came before:

```nika
// Up to ";" - or to the end of the frame, whichever comes first. `frame_end`
// is the boundary of the frame this rule is reached from: written once, in
// the attribute below, and referenced here.
rule NAME -> ref String = s:until(";" | frame_end) { s }

@frame(boundary: "\n")
rule MEASUREMENT -> Reading =
    name:NAME ";" => temp:TENTHS frame_end { Reading { name, temp } }
```

A bare `@frame` takes the boundary from the rule's trailing literal. A frame that ends in `frame_end` names the boundary in the attribute. A frame must end in its boundary. A frame is a **lexical** (uppercase) rule: the implicit whitespace of a syntactic rule would consume newlines, and a data format with no whitespace between its fields is lexical anyway.

**The declaration is checked, and a pattern is never rewritten.** Cutting at the next boundary is correct only if the boundary cannot appear *inside* a frame. The compiler walks everything the rule can reach, and each element that consumes input is one of two things:

* **Safe:** a literal without the boundary in it; a built-in that cannot produce it (`digit1`, `ident`); lookahead; an `until(…)` whose terminator *covers* the boundary, as `NAME` above does with `frame_end`.
* **Rejected**, with the rule and the pattern named: a literal that contains the boundary (CSV with quoted fields, where a newline inside `"…"` would put the cut in the middle of a record); a built-in that can consume it (`any`, `multispace0`); a syntactic rule; an `until(…)` that does not cover the boundary, where the message says what to add; and `recover(…)`, whose skip cannot be kept inside a frame. A grammar recovers per frame instead: `(item | until(frame_end)) frame_end`.

Nothing here changes what a pattern means. `until(";")` consumes up to the next `;` wherever it is written. Inside a frame that is a mistake, and the compiler refuses it and names what to write. The parser generated for a rule is the same whether or not a frame reaches it. A program meets a compile error rather than a wrong total on some inputs and not others.

**When the check cannot see through the format.** A boundary is a byte string, and some formats cannot be cut by searching for one: CSV with quoted newlines, records recognisable by how they *start*, escaped boundaries. `@frame(boundary: "\n", unchecked)` skips the check. The program asserts the invariant, as with `unsafe`, and the word is there to grep for. The attribute is a keyed list, so each of those formats can get its own key later (`quote`, `start`, `escape`, `scan`) without changing what the existing ones mean.

**How two pieces combine.** The entry rule folds with a **merge**:

```nika
pub rule file -> Summary =
    par_fold(MEASUREMENT, Summary::new, fn(acc, m) { acc.record(m) }, Summary::merge)
```

`par_fold` is `fold` plus that merge, and it must be the whole body of its rule. With both declarations the compiler generates the rest. The file is cut into one piece per core. Each piece repairs its own start to the next boundary, so every frame belongs to exactly one worker. Each worker folds into its own accumulator with nothing shared. The accumulators are merged at the end. A failing piece reports its error at its position in the whole file. Nothing about chunks appears in user code:

```nika
let data = fs::map(path)
let totals = Measurements::file(data)
```

**Pieces and the whole agree.** A `par_fold` rule's parser is the per-piece parser, so it skips no whitespace at its entry, unlike every other rule: whitespace skipped there would be skipped at every cut rather than once. A frame that begins with a space keeps it. Whitespace-only text between two frames is an error, in pieces and in one go alike. The number of cores therefore cannot change the answer, on inputs the grammar accepts and on inputs it rejects.

**`par_fold` is written, never inferred.** The compiler does not turn a `fold` into a `par_fold`, even where the step looks associative. Adding `f64` is not associative, so the number of cores would change the answer. A written `par_fold` states that a different chunk count is the same result; for an aggregation over integers, as above, it is.

At `user_parallelism = no`, `par_fold` runs as an ordinary sequential `fold`: same accumulator, same merge, same result, no threads. A grammar written this way compiles unchanged for `wasm32`.

**What the declared format buys.** Because the grammar states the format, the generated parser may exploit it: scanning for a separator or a frame boundary works a machine word at a time rather than byte by byte, on every target and with no `unsafe`. A terminator with up to three alternatives, such as `until(";" | frame_end)`, is one scan ([ADR-009](adr/adr-009.md)).

### 10.8. The Grammar's Vocabulary

Everything a grammar may write, one line each. An element not on this page is
not in the language ([ADR-120](adr/adr-120.md) D1). Where a line says *text*,
the value is a view of the input and nothing is copied (10.6).

**A rule.** `rule name -> Type = pattern { action }`. The `->` names the
result type; the block after the pattern is the action, one per alternative,
and it may read every binding of that alternative. `pub` before `rule` makes
the rule an entry a call can reach (`Json::value(input)`, [ADR-082](adr/adr-082.md)).
A rule may take arguments and be used as `list(pair, ",")` is.

**An action may not pause** ([ADR-142](adr/adr-142.md) D1). A call that can
pause is refused where it stands with `NK2209`. A fold's `init`, `step` and
`merge` are action code, for this rule as for every other. An action may
**fail**, which is what a `pub` rule's `throws` is about. A grammar may be
entered from a body that pauses; what is refused is a pause *inside* the parse.

*Design rationale:* a parse that can be cut into pieces and run on several cores
at once (10.7) is one whose steps do not wait on the world; the demand is the one
12.4's `overlap` branch and 12.6's `par_iter` lambda carry
([ADR-142](adr/adr-142.md) D1).

**An entry is therefore `sync`**, and `nikaia.contracts` says so (D2). A function
whose body is one `Json::value(text)` keeps its own promise.

**Lexical and syntactic.** A rule whose name starts with an **uppercase**
letter is lexical: nothing is skipped between its elements. A lowercase name
is syntactic: the `WS` rule is matched between elements. `WS` defaults to any
run of whitespace. A grammar that declares `rule WS = …` says what is skipped,
and `rule WS = "" { }` skips nothing.

| element | matches | yields |
| :--- | :--- | :--- |
| `"text"` | that text, exactly | nothing |
| `x:p` | `p`, and binds what it yields to `x` for the action | — |
| `p q` | `p` then `q` (whitespace between them in a syntactic rule) | each binding |
| `p \| q` | `p`, or where `p` fails without a cut, `q` | the alternative's action |
| `p => q` | the **cut**: after `p`, `q` must follow; a failure in `q` is an error and no enclosing alternative is retried | as `p q` |
| `p?` | `p` or nothing | a `T?` |
| `p*`, `p+` | zero or more, one or more `p` | a list of what `p` yields, or the text matched where `p` is a character class |
| `p{n}`, `p{n,}`, `p{n,m}` | exactly, at least, between `n` and `m` times; greedy and never giving one back | as `p*` |
| `( p )` | grouping | as `p` |
| `[ p ]`, `{ p }`, `paren( p )` | the delimiter around `p` in the **input**: `[`, `{`, `(` | as `p` |
| `not(p)` | succeeds where `p` does not match, consuming nothing | nothing |
| `peek(p)` | `p` without consuming it | as `p` |
| `until(p)` | everything up to `p`, not consuming `p`; an error where `p` never comes | text |
| `text(p)` | `p`, handing back the text it covered instead of its value | text |
| `dec[T](p)` | `p`, its text read as the number type `T`; an error where it does not fit | `T` |
| `intern(p)`, `ident` | `p`'s text interned (10.6); `ident` is an identifier, interned | a symbol |
| `raw_ident` | an identifier | text |
| `string` | a quoted string, quotes excluded | text |
| `char` | a character literal such as `'\n'` | a `char` |
| `any` | one character | a `char` |
| `digit` | one decimal digit; `digit+` is a run of them | a `char`; text for the run |
| `alpha1` | a run of letters | text |
| `hex_digit` | one hexadecimal digit | a `char` |
| `multispace0`, `multispace1` | zero or more, one or more whitespace characters, newlines included | text |
| `line_ending` | `\n` or `\r\n` | nothing |
| `empty` | nothing, always succeeding | nothing |
| `eof` | the end of the input | nothing |
| `frame_end` | the boundary of the enclosing `@frame` (10.7) | nothing |
| `fail("message")` | never; fails with the message | — |
| `recover(p, sync)` | `p`, and where `p` fails, skips to `sync` and reports the failure without stopping the parse | `p`'s value or the report |
| `list(item, sep)` | `item`, separated by `sep`, zero or more | a list |

**Names for failures.** `rule expr -> Expr # "expression" = …` names the rule
for the message where it fails at its own start, in place of the list of what
its alternatives could have begun with (10.6). `alt # "label"` after one
alternative names that alternative alone.

**Frames.** `@frame(boundary: "\n")` before a lexical rule says a record can be
found from any offset by scanning to the boundary, which is what lets the input
be cut and parsed in parallel; the compiler checks that no rule reachable from
it consumes the boundary in the middle (10.7, [ADR-009](adr/adr-009.md)).

**What is not written.** `tag("x")` is `"x"`, and `digit1` is `digit+`
([ADR-120](adr/adr-120.md) D3). A grammar that writes either is refused with
the spelling to use. A further built-in of the engine underneath joins this page
when a program needs it, and not before.

> **Implementation status:** Partially implemented. The vocabulary above is what
> the examples use and the engine provides. The action block without the arrow
> and the two refusals of `tag` and `digit1` are not implemented; today a grammar
> writes `-> { … }` ([ADR-120](adr/adr-120.md) §5).

## Chapter 11: Running Your Code at Once

The build option `user_parallelism = yes` (1.2) turns the same source into a program that uses more than one core. Nothing in the source changes.

### 11.1. Implicit Async & The Scheduler
The syntax is identical at both values of the option. A function definition carries no `async` keyword.
* **At `no`:** functions yield on I/O events, cooperatively, on one thread.
* **At `yes`:** the runtime runs tasks on a pool of worker threads; `user-pool` in the runtime configuration sets their number.

User code is written in direct style, as if it were synchronous. The compiler places the suspension points. `async` and `await` are not words of the language and never will be ([ADR-038](adr/adr-038.md) D3).

> **Implementation status:** Partially implemented. The `no` column is
> implemented: a function the ledger's `sync` column says can pause is emitted as
> an `async fn`, a call to one carries an `.await` the source never writes, a
> file read suspends on the kernel's completion queue or an I/O worker's reply,
> and the executor is the only place a program parks. At `yes` a task runs on a
> pool of futures over the `user-pool` worker count
> ([ADR-055](adr/adr-055.md) §6 steps 1–4).

### 11.2. Parallelism via spawn
Since code is implicitly async, calling a function runs it now. A task that runs concurrently or in parallel is started with `spawn`.

#### The @detached Contract
`spawn` is declared with the `@detached` attribute, which gives it **implicit move semantics**: a captured value moves into the task. The parent scope cannot use the captured data while the detached task owns it. Where code runs in parallel, this is what keeps it thread-safe; where it does not, it prevents logic races and use-after-free.
* **Ordinary data is cloned.** A program that keeps **data** in the parent, a string, a number, a struct or a collection of those, calls `.clone()` before spawning, and the copy is what the task takes (Part I 8.3).
* **A handle is duplicated.** A handle on a `Shared[T]` is **duplicated** where it is handed to the task, so the name in the parent keeps working and there is nothing to call. Copying a handle copies none of the data (Part I 6.2, [ADR-040](adr/adr-040.md) D1).

A move is refused only where the type is **known** and a move takes the value away. A number, a `bool`, a `char` and a view are *copied*, so the parent keeps them and there is nothing to clone: `let message = "Hello"` is not this case, and `"Hello".to_string()` is. A value whose type nothing describes is not refused, because the compiler does not refuse on a guess (Part III C.4). An assignment between the spawn and the later use clears the refusal; a name given a value again is a correct program.

> **Implementation status:** Implemented. `spawn` lowers, its body is an `async`
> block the captures move into, and a use after the move is refused with
> `NK2101` (Part I 8.3). A handle handed to a task is duplicated, and `--sharing`
> names the duplication site ([ADR-055](adr/adr-055.md) §6 step 4,
> [ADR-040](adr/adr-040.md) §4).

#### What a task may take with it
A task runs on a thread of its own at `yes`, so **everything it uses must be able to cross a thread**. The compiler checks that structurally, with no syntax to write ([ADR-005](adr/adr-005.md) §1 Group B). A type built out of plain data may cross, and so may a struct or collection of those. A value that counts its owners may cross when **what it holds** may cross: whether a `Shared[T]` crosses is never answered by which count it got, and the count follows the answer ([ADR-037](adr/adr-037.md) D7). **A lock may cross** into a task of the program's own: at `yes` an operating-system lock is underneath and several threads are what it is for; at `no` the task is interleaved on the same thread and nothing crosses ([ADR-045](adr/adr-045.md) D2). A `SharedMut[T]` may therefore be used by a task, and a struct holding one is no worse than the field. A lock may **not** go to code nothing written down describes ([ADR-045](adr/adr-045.md) D3, Part III 15.2): there the caller opens the lock and hands over the value inside it, so the called code sees an ordinary value and no lock.

The answer is **the same at both values of the option**, so that a library written at one compiles where it is used at the other. The question is asked of a value **and a destination**, and each destination gets one answer that does not depend on the option. The option changes only whether the build performs the crossing: at `no` nothing in user code runs concurrently, so the task does not run and the refusal is a lint rather than an error. The diagnostic is `NK2501` (Part III C.5).

> **Implementation status:** Implemented. The check runs and is asked about the destination. Plain data, a `Shared[T]` and a lock go into a task of the program's own, so no type reaches `NK2501`'s refusing half at that destination; at the foreign destination a lock is refused with `NK2503` ([ADR-039](adr/adr-039.md) D6, Part III C.6) and a `Shared[T]` with `NK2502` ([ADR-061](adr/adr-061.md) D1). `spawn` and the lock both lower, so a task that takes plain data and a task that takes a `SharedMut[T]` are programs that run ([ADR-055](adr/adr-055.md) §6 step 4, [ADR-064](adr/adr-064.md) §5).

**The nesting rule does not reach into a spawned task.** A `spawn`'s body runs later and elsewhere, not during the call, so it is not part of what its writer holds at the time. The rule that refuses a lock taken while a lock is held (12.3) does not reach into it ([ADR-039](adr/adr-039.md) D3). A scope is the other case, because it waits for its tasks (12.7).

**A lock may be handed to a spawned task.** A task of the program's own is a destination the crossing verdict answers *may* at both values of the option, so a `SharedMut[T]` may be captured by a `spawn`, and the counter of 12.2 is a program user code can write ([ADR-045](adr/adr-045.md) D2). Two handles are two places at one lock ([ADR-040](adr/adr-040.md) D1). A task may also build a lock of its own, which shares nothing.

> **Implementation status:** Partially implemented. The types are implemented, and the counter of 12.2 compiles and runs at both values of the option ([ADR-064](adr/adr-064.md) §5). The lock-touching property of 12.3 is not implemented: no function carries it, so nothing tells a spawned body from a scope's ([ADR-039](adr/adr-039.md) §4).

#### Return Values & Handles
`spawn` returns a `TaskHandle`. At `yes` it represents a running thread; at `no` a scheduled event. `.join()` on it behaves identically at both, which is what the *uniform API* below means.

**`.join()` is the one spelling, and there is no `.await`** ([ADR-055](adr/adr-055.md) D5). A Nikaia program contains the word `await` nowhere (Part I 8.1). `.await` marks where a function *pauses*, and in this language every function may pause. `.join()` is where two **tasks** meet again. The emitted Rust is where the `.await` goes, and the compiler writes it.

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
    let result = handle.join()
}
```

`join` does not fail. It hands back the body's last value, and a body that can fail hands back a value that says so, as any other function does.

> **Implementation status:** Partially implemented. `spawn` lowers to a future
> the executor owns, `.join()` is a suspension point rather than a wait, and the
> implicit move is refused with `NK2101` where the program uses the value
> afterwards, which is the commented-out line above. At `yes` a task goes to a
> pool of futures over the `user-pool` worker count, and `.join()` is the same
> two lines at both values of the option. D6's `Send` is not a refusal of this
> compiler's: the pool's starter asks for it, so a task holding across a pause
> something that may not cross is refused in the backend's words (Part III
> C.1); the structural check above runs on what a task *captures*, not on what a
> body holds across a pause ([ADR-055](adr/adr-055.md) §6 step 4).

---

## Chapter 12: Thread Safety and Synchronization

At `user_parallelism = yes` user code runs on several CPU cores at once. The rules of this chapter keep shared data intact.

### 12.1. The `sync` Keyword (CPU Constraints)
Every function in Nikaia may pause unless it says otherwise. The `sync` keyword marks a function that **never pauses** and is never moved between threads mid-execution.

* **Constraint:** a `sync` function calls **only** `sync` functions.
* **What it is not:** `sync` is not a promise that the function has no *effect*.
  `println` is `sync`: it writes and returns, and it never suspends. A panic
  hook is `sync` and may block ([ADR-006](adr/adr-006.md) D6). One word, one
  promise ([ADR-067](adr/adr-067.md) D1): what a function **reaches** is a
  second column, answered by `touches` (13.5).

Two worlds follow: the pausable world, which is the default, and the **`sync` world** of computation.

```nika
use std::fs

// 'sync' guarantees one thing: I will never pause.
fn calculate_physics(mut obj: Object) sync {   // `mut`: it changes the caller's value (6.5)
    obj.x += obj.velocity
    // fs::read("log.txt") // Compiler Error: that call can pause, and this cannot
}

// Usage in Parallel Iterator
// par_iter requires a 'sync' closure because it runs purely on CPU cores.
// A trailing lambda goes outside the parentheses.
particles.par_iter().for_each fn (p) { calculate_physics(p) }
```

**The word is not required for the rule to be satisfied.** A function that
calls nothing which can pause cannot pause, and the compiler infers that from
the body ([ADR-027](adr/adr-027.md)). The helper below needs no annotation to be
callable from a `sync` function, from `access` (12.2), or from a `par_iter`
body:

```nika
fn bonus(score: i32) -> i32 { score * 2 + 1 }   // no `sync`, and cannot pause

fn total(p: ref Player) -> i32 sync {
    return p.score + bonus(p.score)             // fine: `bonus` provably cannot pause
}
```

Every construct in this chapter that makes concurrency safe demands a `sync`
lambda: `access`, `update`, `access_all`, `par_iter`, a scope's parallel tasks,
the panic hook. Because `sync` is inferred, the set of functions those lambdas
may call is every function that cannot pause, not every function somebody
annotated. The safe path is open by default, and it closes only where something
can pause.

**A library function that runs a lambda does what the lambda does.** `xs.map`,
`xs.filter`, `xs.sort_by_key` and the rest cannot commit to one answer: the same
`map` cannot pause over `fn(n) { n + 1 }` and can over a lambda that reads a
file. Their contracts say that *the lambda decides* ([ADR-029](adr/adr-029.md)).
This is accepted:

```nika
counter.access fn(n) { n + xs.sort_by_key fn(x) { x } }  // pure lambda, pure call
```

and this is refused:

```nika
counter.access fn(n) { xs.map fn(x) { fs::read("log") } } // the lambda does I/O
```

User code writes nothing for this. Without it no iterator method could appear
inside `access` or `par_iter`, whatever its lambda did.

**What the keyword is for.** A written `sync` does what `@borrowed` does in
Part I 6.6: it states a property so that losing it is an error rather than a
silent change. A body written `sync` is held to it, and a call inside it that
can pause is refused with `NK2202`. A body without the word that qualifies still
gets the benefit; a body that stops qualifying stops at the call that now needs
it.

Where the compiler **cannot tell**, it does not guess in the program's favour. A
call it cannot resolve costs the function its inferred promise, because the
promise is written into the ledger and shipped, and a wrong one would put a
pausing body inside a lock. A written `sync` overrules the inference: it is an
assertion, checked as far as the compiler can see.

`sync` decides what a function is compiled to ([ADR-055](adr/adr-055.md) D1). A
function that cannot pause is a plain `fn`; every other function is an
`async fn`. An unresolved call therefore makes a function `async`, and an
`async fn` that never awaits finishes on its first poll.

> **Implementation status:** Implemented. The inference, the lowering by the
> `sync` column and `NK2202` are built ([ADR-055](adr/adr-055.md) §6,
> [ADR-027](adr/adr-027.md)).

### 12.2. The Dual Nature of the Lock
To share data that changes, a program uses **`SharedMut[T]`**: several owners, one value, and the lock inside the type rather than in a second wrapper around it (Part I 6.2). A program makes one by calling it: `let counter = SharedMut(0)`. There is **no second spelling**: `Shared[Locked[T]]` is refused, and the message names `SharedMut[T]` ([ADR-064](adr/adr-064.md) D2, D3). `Locked[T]` is the same lock on its own, for individually locked fields inside a shared structure ([ADR-039](adr/adr-039.md) D9). Everything in this section is about the lock and holds for both.

**The name says what the program gets, not what is underneath.** `SharedMut[T]` means *several own it and any of them may change it*. Which shapes carry that is the compiler's decision, made per value as the owner count is ([ADR-064](adr/adr-064.md) D1). Below, it is a count around a lock, and the pair belongs to the build option.

**The counter below may go into a task of the program's own, and not into foreign code.** Neither half is about the owner count: which count a value gets is decided per value and follows this answer ([ADR-037](adr/adr-037.md) D7, Part I 6.2). The answer is about the **lock**, and it depends on where the value goes ([ADR-045](adr/adr-045.md) D1). Into a task of the program's own: yes, at both values of the option. At `yes` an operating-system lock is underneath and several threads are what it is for; at `no` the task is interleaved on the same thread and nothing crosses. Into code nothing written down describes: no, at both values of the option ([ADR-045](adr/adr-045.md) D3, Part III C.5).

*Design rationale:* at `yes` the foreign crossing would be safe, and it is refused anyway so that a library written at one value of the option stays usable at the other ([ADR-045](adr/adr-045.md) D3).

**At `user_parallelism = no`:**
* **Implementation:** a cell like a `RefCell`, with a re-entrancy check. No OS primitive is used.
* **Cost:** an integer increment.
* **Purpose:** the check is where a **logical deadlock** would be caught: task A locks data and waits for the network, task B tries to lock the same data. 12.3 refuses that nesting when the program is compiled, so the check is self-control of the refusal rather than error handling (below).

**At `user_parallelism = yes`:**
* **Implementation:** an OS-level **mutex**.
* **Cost:** atomic operations.
* **Purpose:** it protects against **memory corruption**. Two threads cannot write to the same memory address at the same time.

**Four Doors, Not One**
A change to locked data has four shapes, and each has its own door. Only two of them run user code while the lock is open, and only those two carry the rules further down this section ([ADR-039](adr/adr-039.md) D10):

| form | for | the block may wait |
| :--- | :--- | :--- |
| `kasse.get()` | taking a copy out | — |
| `kasse.set(value)` | replacing; the value is computed outside | yes, outside |
| `kasse.set(neu; after: stand)` | the same, **if nothing moved** since `stand` was seen | yes, outside |
| `kasse.update fn(mut v) { v += 100 }` | changing it, under the lock | no |
| `kasse.access fn(state) { … }` | reading in place, large values | no |

Each is shaped by what it is for:

* **`set` needs no block.** Arguments are evaluated before the call, so whatever producing the new value costs, including waiting for I/O, is paid outside, and the lock is open for one store. `get` is the same in the other direction: one load.
* **`update` is handed the value as `mut v`, changes it, and returns nothing.** It is **where locked data changes**, small or large: `v += 100` on a counter, `v.push(entry)` on a list. What `v` is, is the compiler's decision by type. It is a **copy** where the value fits a machine word: the block runs on the copy, the result is swapped in, and the block runs again if another task got there first. It is the **address** in the lock otherwise, and the block runs once. Nothing is moved out of the lock in either case, and a ten-thousand-entry list is not copied to append one ([ADR-110](adr/adr-110.md) D1–D3). A block that hands a value back is refused with `NK1141`.
* **`set` takes a witness where the new value came out of the lock.** `after:` changes what the store *is*: compare, and store only if the lock still holds what was seen. `kasse.set(neu; after: stand)` **is** `kasse.update fn(mut v) { if v == stand { v = neu } else { throw Overtaken } }` ([ADR-111](adr/adr-111.md) D5), and like every `update` it takes the lock once. The comparison is of the whole value. A program holding something large writes the `update` block with a version field of its own instead.
* **`access` is for reading in place.** It is the one door that hands the block the value where it lies, and the block **may not change it**. Asking a ten-thousand-entry list for its length copies nothing (D1).

**Exactly one lambda in this language is handed something it may change**, and it carries the word every changed parameter carries: `mut`. A change to locked data is written in `update`, where the word is on the line that makes it ([ADR-059](adr/adr-059.md) D3).

Two mistakes are refused at the doors. **Assigning to a `SharedMut` directly** is refused, and the message names `set`: the value lives behind a lock, so replacing it is a call and not an assignment. **A `set` given a value that was seen in a lock, or standing under a condition that was,** is refused, and the message names `update`, the door for a new value computed from the old one, and `after:`, the door for a value computed outside. The check reads the **stamp**: what a lock hands out is a `Seen[T]`. The stamp travels with the value through `let`, arithmetic, calls to lock-free functions, declared fields and time, and it never comes off, so the pair spread over two lines, two functions or two requests is refused like the one written inline ([ADR-111](adr/adr-111.md)). An `update` block that assigns to `v` without reading it is refused with `NK2207`. `set_after`, which is how `after:` is written in the language below, is not a second way in and is refused with `NK2208`.

**`after:` is the way through for a stamped value**, and `Overtaken` is an error like any other:

```nika
fn charge(kasse: SharedMut[i64], wieviel: i64) throws {
    let stand = kasse.get()
    // Computed outside the lock, which is what `set` is for - and stored only
    // if nothing moved while it was being computed.
    kasse.set(stand + wieviel; after: stand)
}

fn main() {
    let kasse = SharedMut(100)
    charge(kasse, 23) catch { println("somebody got there first") }
    println(f"{kasse.get()}")
}
```

A retry is a loop and a `catch`. A server hands the failure to its caller; in HTTP that is a 409. `Overtaken` carries nothing: the value in the lock *now* is not in it, because reading it would be a second acquisition. A caller that wants it takes the door again and gets a fresh stamp.

**The `sync` Rule (No Pausing While Holding a Lock)**
A lock held while the program pauses blocks a CPU core where there are threads, and freezes other tasks that need the same data where there are none. Nikaia rules it out **at compile time**:

> **`update`, `access` and `access_all` require a lambda that is `sync` and touches no lock, at both values of `user_parallelism`.**

A `sync` lambda (12.1) never pauses. A lambda that touches no lock cannot take a second one (12.3). **These are two conditions and not one** ([ADR-067](adr/adr-067.md) D1): `sync` answers the first, and what a body **reaches**, `touches`, the fourth derived column (13.5), answers the second. A `println` inside a door fails the second and not the first: it never pauses, and it takes standard output's own lock while the door's lock is open, which is a lock inside a lock. While locked data is open, the program runs straight through: lock, compute, unlock. **`get` and `set` need no condition: while the lock is open in either of them, no user code runs, so nothing can fall due** ([ADR-039](adr/adr-039.md) D10). I/O inside `access` is refused with `NK2201`:

```nika
let counter = SharedMut(0)

// OK: pure computation
counter.update fn(mut n) { n += 1 }

// Compiler Error (NK2203): a lock inside a lock. `println` takes standard
// output's own, and `fs::write` can pause besides (NK2202).
// counter.access fn(n) { println(f"{n}") }
```

**`NK2201` is the third case, and it is the one that is neither of the other
two** ([ADR-169](adr/adr-169.md)). `fs::write` inside a door is `NK2202`'s,
because it pauses; a `println` is `NK2203`'s, because it takes a lock. What is
left is I/O that does **neither**, and there is exactly one thing in `std` that
is: reading a `fs::Mapped`. A mapping is a file held as memory, so touching a
page that is not there yet is a disk read with no call in the source at all —
and inside a door it is a disk read with the lock held.

```text
error[NK2201]: `len` reads a file, and this runs with a lock held
  --> main.nika:7:9
   7 |         n = n + page.len()
               ^
     = `Mapped` is a file held as memory, so reading it is a page fault - a disk
       read, which is what a door's block may not wait for (Part II, 12.2)
     = it is neither a pause nor a second lock, which is why `NK2202` and
       `NK2203` say nothing about it (ADR-067 D1)
     help: read what you need before the door and hand the value in, so the
           block works on memory that is already there
```

The claim is the **type's own**: `fs::Mapped` records `touches = ["file read"]`
in the ledger (13.5), so a second such type is a line there and nothing in the
compiler. A type that records nothing is claimed nothing about, which is the
one place this compiler does not read silence fail-closed — what is built on it
is a refusal, and refusing on doubt refuses correct programs
([Part III C.4](30-nikaia-tooling.md)).

> **Implementation status:** Implemented. `SharedMut[T]` and `Locked[T]` are
> types the compiler knows, both implementations are in `std`, all four doors
> exist and `std`'s ledger describes them ([ADR-057](adr/adr-057.md) §5,
> [ADR-059](adr/adr-059.md) §5, [ADR-064](adr/adr-064.md) §5). All three of what
> 12.2 forbids now have a code: `NK2202` for what pauses, `NK2203` for what
> takes a lock — over the `locks` column, so a lock reached through a chain of
> calls is refused too — and `NK2201` for the page fault, at both shapes of read
> ([ADR-169](adr/adr-169.md) D2).

The rule turns the advice not to sleep while holding a lock into a guarantee. Re-entering the *same* lock through a chain of calls is refused by 12.3 when the program is compiled, at both values of the option. The runtime check described above is **self-control of that refusal rather than error handling**. No input can make it fire; if it fires, the compiler has a hole. It is therefore a build option a program may decline (Part I 1.2), and poisoning on several threads is left as it is ([ADR-039](adr/adr-039.md) D2, D8).

**A lock is a resource, so two doors onto the same lock keep their order where both of them write.** Two operations whose touch sets are disjoint have no order between them *inside an `overlap { … }`* (Part I 8.1.2), and a lock is one of the things a touch set can name. **`get` and `access` are reads; `set` and `update` are writes** ([ADR-059](adr/adr-059.md) D3). Two reads of one resource are unordered ([ADR-033](adr/adr-033.md) D2), so two `get`s, or two `access`es, on one lock may run in either order. Any two of the writing forms are ordered by the rule that orders two `println`s, not by a rule of their own ([ADR-033](adr/adr-033.md) §3, [ADR-039](adr/adr-039.md) D10). Two doors onto *different* locks meet on nothing and need not wait for each other.

> **Implementation status:** Not implemented. No ledger entry claims a lock as a
> named resource, because no program has asked for one
> ([ADR-028](adr/adr-028.md) D5). A door onto a lock is an operation whose
> touches cannot be determined, so it counts as touching everything and keeps
> its place: for the writing forms that is the answer this section states, and
> for two `get`s it is a stricter one.

Where two doors would meet on something the touch sets cannot see, a program writes them as ordinary statements rather than as branches of an `overlap`: statement order is the written order (Part I 8.1.1).

### 12.3. Deadlock Prevention: Atomic Composition
The classic cause of a deadlock is inconsistent locking order: thread 1 locks A then B, thread 2 locks B then A.
In Nikaia, taking a lock while a lock is held is **always** a compile-time error ([ADR-039](adr/adr-039.md) D2). What is refused is the nesting of lock *acquisitions*, one door (12.2) opened while another is still open, and not any way of spelling a type. One door on its own is one acquisition, whatever it opens.

**The Solution: `access_all`**
Instead of nesting `access` calls, a program requests several locks at once with `access_all`.

* **Mechanism:** the runtime sorts the locks by memory address (or internal ID) before locking. The locking order is therefore the same in every thread.
* **Safety:** a deadlock cycle between A and B cannot form when every acquisition goes through `access_all(A, B)`, because every thread takes the lower address first.

```nika
let account_a = SharedMut(Account(…))
let account_b = SharedMut(Account(…))

// ERROR: Manual Nesting is forbidden to prevent Deadlocks.
// Taking one lock inside another is what creates the inconsistent order
// this section is about — a single door (12.2) is of course fine.
// account_a.access fn(from) {
//     account_b.update fn(to) { … }
// }

// Atomic Locking (Deadlock Proof)
// The runtime sorts A and B by address and takes them in that order, so this
// and `access_all(account_b, account_a)` in another task take them alike.
access_all(account_a, account_b) fn(a, b) {
    // Both are read in place, and neither may be changed here (ADR-059 D1).
    a.balance + b.balance
}

// And the transfer, which is a write on both: one `mut` per lock, and the
// block changes them ([ADR-065](adr/adr-065.md) D2, [ADR-110](adr/adr-110.md) D6).
update_all(account_a, account_b) fn(mut von, mut nach) {
    von -= 30
    nach += 30
}
```

**`update_all` is `update`'s rule widened, not a second rule.** One `mut` per
lock, nothing returned, nothing sees either lock between the two writes.
Whether the block runs on copies and is retried, or on the addresses under the
locks, is the compiler's decision by the pair of types, as it is for one lock.
`access_all` reads several locks at once, because `access` is a read
([ADR-059](adr/adr-059.md) D1); `update` writes one; `update_all` writes
several, one `mut` each ([ADR-065](adr/adr-065.md)). A door takes two locks at
a time: a door handed one lock, or a block that does not name one value per
lock, is refused with `NK1124`. Neither name is a keyword.

> **Implementation status:** Implemented. Both doors run at both values of the
> option, and `NK1124` fires ([ADR-065](adr/adr-065.md) §5).

**How the compiler sees a chain.** A nesting written one line inside the other is visible where it stands; a chain is not: a block calls a function, which calls another, and the third opens a lock. Every function therefore carries a second derived property beside `sync` (12.1): **whether it touches a lock.** The property is inferred over the same call graph and never written by hand. Inside a blocking door (`update`, `access`, `access_all`), a call to anything that carries it is refused. That catches chains and self-calls, and it makes the rule above complete rather than local ([ADR-039](adr/adr-039.md) D3).

**The property is coarse.** It says *a lock*, never *which* lock.

*Design rationale:* a precise version would have to prove two handles distinct, and whether a program compiles would then depend on whether that proof succeeds; an unrelated line elsewhere could make unchanged code stop compiling ([ADR-039](adr/adr-039.md) D4).

The property is not 12.2's ordering rule. That two writing doors onto the *same* lock keep their order needs to know *which* lock, and that comes from what the operation touches ([ADR-033](adr/adr-033.md) D2), never from this property.

**A function handed outward is judged by its body, never by its type.** Where a rule asks whether a lambda touches a lock and the lambda is not run on the spot, because it is handed to foreign code or kept by the callee, the answer is computed from the body. A lambda is its captures, and no type writes those down. Today that reaches a lambda at a call site and the standard library's own detached entries, because the type grammar has no function type (Part I 5.4); it widens when that grammar gains one ([ADR-039](adr/adr-039.md) D7).

> **Implementation status:** Partially implemented. The syntax of all three
> doors is implemented: a trailing lambda may name its arguments and may follow
> a plain call as well as a method call (Part I 5.3), so
> `counter.access fn { … }`, `account.access fn(to) { … }` and
> `access_all(account_a, account_b) fn(a, b) { … }` are each one call whose last
> argument is the lambda. `SharedMut[T]`, `Locked[T]` and all four doors are in
> `std` and in its ledger ([ADR-064](adr/adr-064.md) §5). No refusal in this
> section is implemented: nothing refuses the nesting, no function carries the
> lock-touching property, so nothing refuses a chain or a function handed
> outward, and nothing demands the lambda 12.2 does
> ([ADR-039](adr/adr-039.md) §4).

### 12.4. Racing Tasks (`select`)
`select` runs several tasks and keeps the result of the one that finishes *first*.

```nika
use std::time

select {
    // Case 1: Computation finishes first
    result = heavy_math() => { return result }

    // Case 2: Timeout happens first
    _ = time::sleep(5.seconds()) => { throw Timeout::TooSlow }
}
```
`select` is a block whose arms bind ([ADR-148](adr/adr-148.md)). The first branch to finish wins. The losers are cancelled with the teardown [ADR-006](adr/adr-006.md) D3 describes, and the handle `spawn` returns gains `cancel()`, so a program's cancel and a loser's are one mechanism. `5.seconds()` is a `std::time::Duration`, made by a `std` extension on the integers; there is no suffix literal ([ADR-150](adr/adr-150.md)). `sleep` lives in `std::time` and is written with it, because the prelude's rule is that nothing in it pauses (Part I 1.3, [ADR-154](adr/adr-154.md) D2). The error a branch throws is an **enum variant**, because an error type is an `enum` ([ADR-023](adr/adr-023.md) D1, [ADR-141](adr/adr-141.md) D1).

`select` and `overlap` are a **pair** ([ADR-148](adr/adr-148.md) D4): both start every branch at once, and the difference is what they keep — `overlap` keeps every result ([ADR-050](adr/adr-050.md)), `select` keeps the first. A block needs at least two arms, because one arm has nothing to race against, and at most eight, which is what `std` writes a vehicle for.

> **Implementation status:** Implemented. `select` is a keyword, the block above
> is a program, and `5.seconds()` and `sleep` are `std`
> ([ADR-148](adr/adr-148.md) §5, [ADR-150](adr/adr-150.md) §5). The losers are
> cancelled by being dropped, which is [ADR-006](adr/adr-006.md) D3's teardown
> reached from a second place, and `handle.cancel()` is the same mechanism a
> program can ask for. What a **cancelled** task's handle would say afterwards
> is a question no program can ask, because cancelling takes the handle
> ([ADR-148](adr/adr-148.md) §4).

**What "cleaned up" means.** The losing task stops at its current pause point and its values are torn down. A resource with a pausable `cleanup` (Part I 6.4) is not awaited by the *winner*. The runtime **adopts** such `cleanup` runs and finishes them in the background ("parked cleanup"). The program does not exit before parked cleanups are done, bounded by the runtime configuration `cleanup-deadline` (Part III 13.3). An error from a parked cleanup has no caller to reach and is reported through the runtime's error hook ([ADR-006](adr/adr-006.md) D3).

### 12.5. Channels (Message Passing)
Nikaia offers **message passing** beside shared memory. A channel is `std`'s and not the language's: two values and two methods say everything the example below says ([ADR-149](adr/adr-149.md)). A channel is **bounded**. `send` pauses when the channel is full, so a `sync` body cannot send on one, and the ledger's column says so. `recv` hands back a `T?`; `null` means every sender is gone. The value type must `crosses` ([ADR-123](adr/adr-123.md)), checked where `tx` moves into the `spawn` as any move is.

> **Implementation status:** Implemented ([ADR-149](adr/adr-149.md) §5). The
> example below is a program: `std::channel` is a module, `send` pauses where
> the channel is full and `recv` hands back a `T?`. There is no `unbounded()`,
> and a capacity below one is refused.

```nika
use std::channel

// Subject: 100 (capacity)
let (tx, rx) = channel::bounded(100)

// Implicit Move transfers 'tx' into the background task
spawn fn {
    tx.send("Calculation complete")
}

let msg = rx.recv() // Waits (non-blocking yield) for the message
```

### 12.6. Data Parallelism (`par_iter`)
`par_iter` (parallel iterator) processes a large list on all CPU cores.

```nika
let pixels = [/* 1 million pixels */]

// The compiler splits the array into chunks and distributes them
// across all cores. The closure must be 'sync'.
// Methods chained with trailing lambdas. A block ends at its `}`, so the chain
// continues after it and means what it reads as (ADR-022).
let bright_pixels = pixels.par_iter()
    .map fn(pixel) { pixel.brightness * 1.5 }
    .filter fn(value) { value > 0.5 }
    .collect()
```

### 12.7. Scoped Tasks (@immediate Parallelism)

A `spawn` task takes its captured variables *with* it, because it may outlive the function (Part I 8.3). A **scoped** task *borrows* them, because the scope waits until every task inside it has finished.

`task::scope` is an **immediate context** (Part I 5.4): it does not return until every task spawned inside it has completed. A task inside the scope therefore reads the function's variables without moving or cloning them.

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

**A scope may not be opened while a lock is open.** Because the scope waits for its tasks, their bodies belong to the function that writes the scope, as a trailing lambda's body does. Whatever a task touches, the surrounding function touches, including a lock (12.3). Opening a scope inside an open lock is therefore refused: a task that wants the same lock waits for the holder, and the holder waits for the end of the scope, which is a deadlock. A task started with `spawn` is the other case, because it runs later and elsewhere (11.2, [ADR-039](adr/adr-039.md) D3).

> **Implementation status:** Not implemented. `task::scope fn(s) { … }` and the
> `s.spawn fn { … }` inside it parse as calls with the lambda as the argument
> (Part I 5.3). `task::scope` has no entry in `std`, nothing waits the inner
> tasks out, nothing propagates what a scope's tasks touch to the surrounding
> function or refuses a scope inside an open lock, and `NK2102` is not reported.
> The executor that owns its tasks and drives a `spawn`'s future exists
> ([ADR-055](adr/adr-055.md) §6 steps 1–4); a scope adds the waiting and the
> touch propagation.

**One rule differs with `user_parallelism`.** The scope's promise holds only where the runtime can wait the tasks out:

* **At `no`:** everything runs on one thread, and the runtime owns every task. When a scope ends, including when an error tears it down early, the runtime collects its tasks *before* the function's variables disappear. **A scoped task may do anything, including I/O.**
* **At `yes`:** tasks run on other CPU cores *in parallel*. A task in mid-computation on another core cannot be stopped at an arbitrary moment, so the scope keeps its promise only for tasks that finish on their own. **At `yes`, a scoped task must be `sync`**: pure computation, no I/O, no pausing, the same rule as `par_iter` (12.6).

A task that needs I/O is a background task, not a scoped one. A program starts it with `spawn` (the task takes ownership, Part I 8.3) and collects the result through its handle. A scoped task that can pause is refused with `NK2102` where tasks run in parallel:

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
           of its variables, so clone what you still need first:
           let target = url.clone()
           let handle = spawn fn { fetch_url(target) }
           let result = handle.join()
```

*Design rationale:* borrowing across *parallel, pausable* tasks is unsound, because a cancelled scope cannot stop a task in mid-execution on another core while the borrowed variables disappear; general-purpose async runtimes offer no safe async scope for that reason. Nikaia avoids it structurally. At `no` the one thread user code runs on owns all task state and tears a scope down synchronously, which requires that task futures are runtime-owned and that the language exposes no way to leak a live scope. At `yes` the `sync` restriction makes waiting deterministic ([ADR-005](adr/adr-005.md) D5).

### 12.8. Supervision Trees
A task may panic. A **supervisor** monitors tasks. When a child task panics, the supervisor does one of three things, according to its policy:
* **Restart** the task.
* **Crash** the parent (escalate).
* **Ignore** the error.

A supervision tree is how a program heals itself.

```nika
// Subject: The Task (Lambda)
// Config: restart_policy (Protocol: separated by ;)
// Note: 'spawn' syntax (fn {}) is the subject.
supervisor::start_link(fn {
    server.run()
}; restart_policy: RestartPolicy::Always)
```

The policy is an **enum** and not a string ([ADR-141](adr/adr-141.md) D1). A
misspelled policy is refused at the call rather than noticed at the restart, and
the policies are listed where they are declared (Part I 4.4).
