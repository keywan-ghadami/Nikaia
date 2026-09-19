# Nikaia Language Specification
**Part II: Advanced Features & Metaprogramming**
**Version:** 0.0.7 (Draft)
**Date:** September 5, 2026

---

## Chapter 10: Metaprogramming (Code that writes Code)

Metaprogramming allows developers to extend the language itself. In Nikaia it is not done with text replacements (like in C), and — since [ADR-088](adr/adr-088.md) — not by writing code as data either. There are two mechanisms and they are kept apart: a **`grammar`** reads a language the compiler does not know (10.1, 10.5), and **`comptime`** reads what the compiler already knows, a type's shape (10.3). Nothing is generated behind the reader's back, which is what lets a piece of code be understood by reading it.

### 10.1. Parsing with `grammar` (Scannerless)
Before you can manipulate code, you often need to read custom data formats. The `grammar` tool lets you write parsers declaratively.

Nikaia grammars are **scannerless**: there is no separate tokenizer stage. A grammar consumes the raw character stream directly. This is what makes it possible to embed foreign languages (10.5) — a global lexer could never tokenize SQL, assembly and Nikaia at once, because they disagree about what a `}` or a `'` means.

**Key features:**
*   **Commit Points (`=>`):** control backtracking. Once the parser passes a commit point, it stays in this branch — a later failure is an *error*, not a reason to silently try the next alternative.
*   **Lexical vs. syntactic rules:** rule names starting with an uppercase letter are **lexical** (no whitespace between parts); lowercase rules are **syntactic** (whitespace allowed). This replaces the lexer/parser split.
*   **Typed actions (`{ … }` after the pattern):** each rule builds your own types directly ([ADR-120](adr/adr-120.md) D2).

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

> **Note:** Auto-generated AST types (structs for labelled sequences, enums for alternatives, when no `{ … }` is given) are a planned convenience, not current behaviour. Action blocks are required today. See [ADR-007](adr/adr-007.md), D7.

### 10.2. Dual-Mode Parsing (Static vs. Dynamic)
A grammar defined once can be used at compile time and at runtime — **with the same
syntax and the same meaning**. A grammar is entered by an **ordinary call**, and
every `pub` rule in it is an entry named after the rule
([ADR-082](adr/adr-082.md) D1, D2): `Json::value(x)` runs the rule `value` of the
grammar `Json`. What decides *when* it runs is the word in front of the binding,
not the shape of the line.

**The separator is `::`, as it is everywhere else** ([ADR-140](adr/adr-140.md)
D3). A grammar's name is a name and a rule of it is reached the way every other
qualified name is; the dot is for a **value's** members, and a namespace behind
one was the single place this language asked a reader to tell two things apart
by what the left side happens to be.

> **Status:** **built for a grammar** ([ADR-140](adr/adr-140.md) §5).
> `Json::value(input)` is the entry and the dot is refused with `NK1147`, which
> is the checker's and not the parser's: `Json.value(x)` and `text.value(x)` are
> the same five tokens, and only the side that knows `Json` names a grammar can
> tell them apart. 10.3's `T::fields` is the same decision on a page whose
> construct is itself unbuilt — a `comptime` loop over a type's shape does not
> exist yet, so the spelling is all there is to fix.

**A. Static Embedding (Compile-Time)**
In a `comptime` binding the parser runs *during the build*. If the input is
invalid, compilation fails. The result is embedded in the binary with zero runtime
cost.

Three visible steps, each decided on its own: `comptime` says **when**,
`asset("…")` says **where the bytes come from** ([ADR-072](adr/adr-072.md),
[ADR-116](adr/adr-116.md)), and the call says **what is done with them**. The older spelling put all three in one line whose
meaning depended on where it stood — at run time `dsl Json from "config.json"`
parsed the eleven characters of the name, in a binding it was meant to parse the
file.

```nika
// The compiler runs the Json grammar at build time.
// If "config.json" is malformed, the build stops.
comptime CONFIG: Json::Value = Json::value(asset("config.json"))
```

**B. Dynamic Parsing (Runtime)**
The exact same grammar processes user input or network data while the program runs.

```nika
fn parse_input(input: String) throws {
    let data = Json::value(input)
    println(f"Parsed: {data}")
}
```

> **Status:** **B is built and A is half.** `comptime` is a **declaration inside a
> function body** ([ADR-073](adr/adr-073.md) D2, and [ADR-077](adr/adr-077.md) for
> the word — `const` would have named immutability, which 2.1 already gives every
> binding): it parses, its initialiser is evaluated while the program is built,
> and what reaches the language below is the value rather than the expression —
> `comptime LIMIT = 4 * 1024` arrives as `const LIMIT: i32 = 4096;`, in Rust's
> word for the same slot. At **item level** it is still a parse error, so the
> example above has nowhere to stand yet.
>
> What the initialiser may hold is D5's first stage: an integer — a literal,
> arithmetic over literals and over other constants — and `true` or `false`. **A
> call is not in it**, so the `Json::value(asset("…"))` above still runs at runtime
> wherever it is written. A `comptime` binding this compiler cannot evaluate is
> `NK1127` rather than a value computed later, which is D3's demand doing its one
> job.
>
> **And what crosses is decided** ([ADR-079](adr/adr-079.md)): a result arrives in
> its **view** form — `Vec[T]` as `&[T]`, `String` as `&str` — so what was built
> with `push` is fixed once it has crossed, and a value that owns memory is
> refused by its *type* rather than by its parse. A map that crosses is a **fixed**
> map, because a closed key set is what a perfect hash needs and the crossing is
> where it closes; how it is looked up is then the compiler's, the way a map's
> hasher already is. None of that is built — what is missing is an evaluator that
> can loop and `push`.
>
> **The call form above is built** ([ADR-082](adr/adr-082.md) D1, D2): a grammar
> name stands where a callee stands, every `pub` rule is an entry, and the old
> `dsl X from e` is refused with a message naming `X.rule(e)`. What the
> distinguished entry cost is what went with it — the emitter used to take the
> **first** `pub` rule, a `par_fold` one ahead of an earlier one, silently. What
> is still not built is the half this note is really about: the evaluator that
> could run one while the program is built.
>
> **What the declaration will mean is decided** ([ADR-073](adr/adr-073.md)): it
> stands where an item stands and inside a body, its type may be written and does
> not have to be, and — the part the keyword exists for — **a `let` may fold and a
> `const` must**, saying so where it cannot, rather than quietly running at
> program time instead. What may stand in the initialiser is staged: literals and
> arithmetic over them first, a call and therefore the `from` above only once
> [ADR-026](adr/adr-026.md) Q4 says what a build-time body may do.
>
> **And when it does have one, the file will have to be named three times**
> ([ADR-072](adr/adr-072.md) D3): in the code, as the literal above; in an
> allowlist file, one path per line; and in the invocation that puts that list in
> effect, `--allow-read-from-list=…`. Two of the three are committed, so adding a
> read is a diff a reviewer sees; the third lets a build be run with compile-time
> reading switched off without editing anything. A build given no list reads
> nothing (D1), the path may not be computed (D4), and there are no patterns
> (D5).

### 10.3. Generating Code from a Type's Shape
While parsing reads data, this reads **types**. The job is the familiar one — write
`describe` once and have it work for every struct, rather than writing the print
statements again per type — and Nikaia does it with the mechanism it already has.

**There is no macro system** ([ADR-088](adr/adr-088.md) D1): no `macro`, no code
written as data, nothing attached to a declaration. What is needed instead is a
type's shape as **ordinary data** and a loop that runs while the program is built.

```nika
fn describe[T: Struct](value: T) {
    for field in T::fields {
        println(f"{field.name} = {field.of(value)}")
    }
}
```

Every piece of that is something the language has elsewhere.

**The bound is what makes `T::fields` exist.** `T: Struct` is an ordinary bound
(4.7), answered from a declaration the way every other one is — and it is what
says a shape may be asked for at all. A `T: Enum` would have `T.variants`. There is
no builtin and no special syntax: **reflection is reached as a member**, and
`field.of(value)` is a method on the reflected field rather than a piece of
grammar.

That is also what keeps the error messages where they belong. A parameter with no
bound could be handed anything, and a caller would learn that its type does not
fit only when something deep in this body failed — with the message pointing into
somebody else's code. The bound answers *you passed something that is not a
struct* **once, at the call**, and leaves inside only what is genuinely per-field.

**The loop needs no second word.** `T::fields` is known while the program is built,
so a loop over it cannot be anything but unrolled — there is no run-time reading to
rule out. Other languages spell this with a second keyword; here the absence of an
alternative does the work.

It is worth saying what does *not* change: the loop means what a loop always
means, *walk these elements*. Only the **stage** is earlier. That is different from
a construct whose operand changes meaning with its position, which
[ADR-082](adr/adr-082.md) took out of the language for exactly that reason.

**And the body is checked once per unrolled turn, not once.** `field.of(value)` is
a `String` for one field and an `i32` for the next, so there is no single type to
check it against. This costs **nothing at run time** — what is left is the two
`println` calls somebody would have written by hand, with no loop and no dispatch
— and at build time it costs fields × the types actually used. An ordinary
`for x in xs` over a run-time list is checked once, as it always was.

The consequence is in the messages, and it is the part this language may not get
wrong. A body that is wrong for one field is wrong at one unrolled copy, on a line
that is correct for the others, so the diagnostic says which:

```text
error: `println` cannot format a `Vec[u8]`
  --> describe.nika:3:9
     = unrolling `T::fields` for `User`, at field `avatar`
```

**What cannot be read is asked** (D6). No build-time system lets you see what a
function becomes for a given type without unrolling it in your head, and the usual
answer is to invent syntax for it. This one has the other answer, the same one
`--overlaps`, `--sharing` and `--trust` already give: `nikaia --comptime` prints
what was unrolled, for the types actually used.

> **Status.** **None of this is built**, and the compiler says so one step earlier
> than the page does. Run against the program above it answers
>
> ```text
> error[NK1117]: nothing declares `T`
>   --> describe.nika:2:5
>    2 |     for field in T::fields {
> ```
>
> because **a type is not a value here**: `T::fields` reads `T` in a place where a
> value stands, and nothing declares one. That is the first of three things
> missing, and the deepest — it is what Zig means when it says types are values at
> build time. The second is the shape itself: `Struct` is not a trait anything
> declares, so `[T: Struct]` names a bound that does not exist, and `T::fields`
> would be a member nothing provides. The third is a loop that runs while the
> program is built, which is the same missing piece [ADR-079](adr/adr-079.md) §3
> and [ADR-073](adr/adr-073.md) D5 wait on.
>
> What *is* built is the bound **mechanism** ([ADR-078](adr/adr-078.md)) — bounds
> work, this particular one does not exist — and `comptime` for literals and
> arithmetic over them ([ADR-073](adr/adr-073.md)). `--comptime` does not exist.
>
> `macro`, `quote` and `with` are **reserved words and not constructs**
> ([ADR-088](adr/adr-088.md) D7), so a program may not use them as names. The
> earlier design of this section, which built a macro system out of the three, is
> withdrawn.
>
> **One thing is named and not decided:** capturing an *expression* — `u.age > 18`
> as a tree for a provider to translate, which is what a LINQ-shaped query needs.
> Reflection describes types and cannot do it. It is the only capability left on
> this page that would be genuinely new (§3 of that record).

### 10.4. Hygiene, and Why the Question Dissolved
A macro system has to answer this: if generated code binds `temp`, does it collide
with the `temp` the caller already had? Every language with macros needs a rule —
hygiene, and then a way to break it deliberately when a macro is *meant* to hand a
name back.

**Nikaia has no such rule, because it has no such question**
([ADR-088](adr/adr-088.md) D1). Nothing is injected from elsewhere. 10.3's loop
body is written by the program, in place, by the person reading it; there is no
second author whose names could arrive unannounced. A name bound inside that loop
is bound where it is written, the way a name bound in any other block is.

This is the readability argument Zig makes for having no macros, and it is worth
naming as a thing gained rather than a thing missing: **you can tell what a piece
of code does by reading it**, because nothing was generated behind it.

What used to be here — hygiene by default, and an `@`-prefixed escape to inject a
name into the caller's scope — is withdrawn with the construct it applied to.

### 10.5. Using DSLs (The `dsl` Keyword)
While 10.3 reads *Nikaia*'s own types, the `dsl` keyword embeds **foreign syntax** directly into a Nikaia file — SQL, HTML, regex, assembly. That is the line between the two: 10.3 walks a shape the compiler already knows, and this hands a byte slice to something that knows a language the compiler does not.

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
| `meta::column(name, type)` | build time, declared by the grammar | the statement's **result**: one field per column, so a row is a type ([ADR-143](adr/adr-143.md) D2) |

Both are needed. Capturing a variable is exactly right for assembly. It is exactly wrong for a SQL prepared statement: baking the values into the statement definition destroys the reuse that makes preparing it worthwhile.

The third intrinsic is what makes a query's **result** typed without the compiler knowing any SQL: a database driver's grammar reads the statement and the schema — a build-time argument of the block, `dsl sqlite(schema: app) { … } eod`, resolved from a `comptime` value — and declares the columns; the compiler builds the row type from them as it builds the parameter type from the holes. A column the schema lacks is the grammar's error at the query, while the program is built ([ADR-143](adr/adr-143.md)). The compiler itself knows no dialect and never will; a vendor's database is a package.

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
    // Subject: none (method on self), so no `;` - only the option (Part I 5.1)
    // Omitting 'target_age' is a compile error - the compiler knows the
    // statement needs it, because it parsed the statement.
    let users = query.execute(target_age: min_age)
}
```

Library authors accept those parameters with the **typed spread**:

```nika
impl SqlParser {
    // Subject: self (the parsed statement) ; Config: the DSL's parameters
    pub fn execute(self; ...args: Self::dsl) -> Vec[Row] throws {
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
      c:closure_expr { c }
    | e:catch_expr   { e }
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
rule NAME -> &str = s:until(";" | frame_end) { s }

@frame(boundary: "\n")
rule MEASUREMENT -> Reading =
    name:NAME ";" => temp:TENTHS frame_end { Reading { name, temp } }
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
let totals = Measurements::file(data)
```

**Pieces and the whole agree — always.** A `par_fold` rule's parser is the per-piece parser, so it skips no whitespace at its entry, unlike every other rule: whitespace skipped there would be skipped at every cut rather than once. A frame that begins with a space keeps it; whitespace-only text between two frames is an error, in pieces and in one go alike. That is what makes the number of cores unable to change the answer — on inputs the grammar accepts and on inputs it rejects.

**Why you have to ask for it.** The compiler will not turn a `fold` into a `par_fold` on its own, even when it looks associative. Adding `f64` is not associative, so the number of cores would quietly change the answer. Writing `par_fold` is you saying that a different chunk count is the same result to you — for an aggregation over integers, as above, it is.

At `user_parallelism = no`, `par_fold` runs as an ordinary sequential `fold`: same accumulator, same merge, same result, no threads. A grammar written this way compiles unchanged for `wasm32`.

**What you get for free.** Because the grammar states the format, the generated parser is allowed to exploit it: scanning for a separator or a frame boundary works a machine word at a time rather than byte by byte, on every target and with no `unsafe` in sight — and a terminator with up to three alternatives, like `until(";" | frame_end)`, is still one scan. See [ADR-009](adr/adr-009.md).

### 10.8. The Grammar's Vocabulary

Everything a grammar may write, one line each. An element not on this page is
not in the language ([ADR-120](adr/adr-120.md) D1). Where a line says *text*,
the value is a view of the input and nothing is copied (10.6).

**A rule.** `rule name -> Type = pattern { action }`. The `->` names the
result type; the block after the pattern is the action, one per alternative,
and it may read every binding of that alternative. `pub` before `rule` makes
the rule an entry a call can reach (`Json::value(input)`, [ADR-082](adr/adr-082.md)).
A rule may take arguments and be used as `list(pair, ",")` is.

**An action may not pause** ([ADR-142](adr/adr-142.md) D1). A call that can is
refused where it stands, `NK2209`, and a fold's `init`, `step` and `merge` are
action code for this rule as for every other. It is the demand 12.4's `overlap`
branch and 12.6's `par_iter` lambda already carry, in the place a parser needs
it: a parse that can be cut into pieces and run on several cores at once (10.7)
is one whose steps do not wait on the world — an action that reads a file is a
second pass wearing a grammar. An action may still **fail**, which is what a
`pub` rule's `throws` is about, and a grammar may be entered from a body that
pauses; what is refused is pausing *inside* the parse.

**So an entry is `sync`**, and `nikaia.contracts` says so (D2). A function whose
body is one `Json::value(text)` keeps its own promise.

**Lexical and syntactic.** A rule whose name starts with an **uppercase**
letter is lexical: nothing is skipped between its elements. A lowercase name
is syntactic: the `WS` rule is matched between elements. `WS` defaults to any
run of whitespace; a grammar that declares `rule WS = …` says what is skipped,
and `rule WS = "" { }` skips nothing. (ANTLR's lexer rules are uppercase for
the same reason; pest steers skipping by a rule named `WHITESPACE`.)

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
([ADR-120](adr/adr-120.md) D3); a grammar that writes either is refused with
the spelling to use. The engine underneath has further built-ins that no
program has needed; one joins this page the day one does.

> **Status:** the vocabulary above is what the examples use and the engine
> provides. **Not built**: the action block without the arrow, and the two
> refusals ([ADR-120](adr/adr-120.md) §5) — today a grammar writes `-> { … }`.

## Chapter 11: Running Your Code at Once

Setting `user_parallelism` to `yes` (1.2) turns the same source into a program that uses more than one core. Nothing in the source changes.

### 11.1. Implicit Async & The Scheduler
The syntax is identical either way. You do **not** use `async` keywords on function definitions.
* **At `no`:** functions yield on I/O events, cooperatively, on one thread.
* **At `yes`:** the runtime uses a **Work-Stealing Scheduler** and distributes tasks across the cores it is allowed.

The code remains "Direct Style". You write code as if it were synchronous, and the compiler handles the suspension points.

> **Status:** the `no` column is built and the `yes` column is not
> ([ADR-055](adr/adr-055.md) §6 steps 1–4).
>
> **This section had no status note until the lowering caught up with it**, and
> it needed one: *"functions yield on I/O events, cooperatively, on one thread"*
> was not true. The emitted Rust contained the word `async` **zero** times, so a
> function did not yield on an I/O event — it blocked the thread — and nothing
> said so. A function the ledger's `sync` column says can pause is an `async fn`
> now, a call to one carries an `.await` the source never writes, a file read
> suspends on the kernel's completion queue or an I/O worker's reply, and the
> executor is the only place a program parks. So the sentence is a claim about
> the program again.
>
> **The work-stealing scheduler at `yes` is not built.** `rayon`'s pool is a
> work-stealing pool for *closures*, not an executor for futures, so it is a
> second executor over the same worker count rather than a use of that one — and
> it is where a spawned future has to be `Send`. Until it exists, `yes` gets the
> one-thread executor: a program means the same thing, and it uses one core.
>
> **`async` is not a keyword you write** and never will be, which is the first
> line above and is held rather than promised: a test reads every `.nika` file in
> this repository and fails on the words `async`, `await` and the runtime's type
> names ([ADR-038](adr/adr-038.md) D3).

### 11.2. Parallelism via spawn
Since code is implicitly async, "calling a function" usually means "running it now". To run tasks concurrently or in parallel, you strictly use `spawn`.

#### The @detached Contract
The `spawn` function is defined with the `@detached` attribute. This triggers **Implicit Move Semantics**.
* **Why?** This guarantees thread safety where code runs in parallel, and prevents logic races or "Use-After-Free" where it does not. The parent scope cannot access the captured data while the detached task owns it.
* **Copying, for ordinary data:** If you need to keep **data** — a string, a number, a struct or collection of those — in the parent thread, you must explicitly call `.clone()` before spawning, and the copy is what the task takes (Part I, 8.3).
* **Nothing to copy, for a handle:** A handle on a `Shared[T]` is the other case. It is **duplicated** where it is handed to the task, so the name in the parent thread keeps working and there is nothing to call — for a handle there is no method, because copying a handle copies none of the data (Part I, 6.2, [ADR-040](adr/adr-040.md) D1).

> **Status:** both bullets are built ([ADR-055](adr/adr-055.md) §6 step 4).
> There is a task for them to be about: `spawn` lowers, the body is an `async`
> block the captures move into, and the capture the contract decides is reported
> as `NK2101` (Part I, 8.3).
>
> **The first bullet is narrow, and the narrowness is the rule rather than a
> gap.** A move is only refused where the type is **known** and a move takes the
> value away: a number, a `bool`, a `char` and a view are *copied*, so the parent
> keeps them and there is nothing to clone — which is why `let message = "Hello"`
> is not this bullet's case and `"Hello".to_string()` is. A type nothing
> describes is not claimed about at all, because refusing on a guess is the one
> thing the compiler may never do (Part III, C.4). And an assignment between the
> task and the later use clears it: giving the name a value again is a correct
> program.
>
> **The second bullet is built and held**: a handle handed to a task is
> duplicated, so using the name again afterwards is not refused — which is what
> `--sharing` had been naming as a duplication site with no emitted program
> reaching it ([ADR-040](adr/adr-040.md) §4), and now one does.

#### What a task may take with it
A task runs on a thread of its own at `yes`, so **everything it uses has to be able to cross a thread**. The compiler checks that structurally, with no syntax to write and no annotation to forget ([ADR-005](adr/adr-005.md) §1 Group B): a type built out of plain data, and a struct or collection of those, may cross. So does a value that counts its owners: whether a `Shared[T]` may cross is answered by **what it holds**, never by which count it got — the count follows the answer ([ADR-037](adr/adr-037.md) D7), and where the value reaches a task it is the one a second thread may safely touch. **And so does a lock**, into a task of your own: at `yes` a real operating-system lock is underneath and several threads are what it is for, at `no` the task is interleaved on the same thread and nothing crosses at all — two reasons, one answer ([ADR-045](adr/adr-045.md) D2). A `SharedMut[T]` may therefore be used by a task, and a struct holding one is no worse than the field. What a lock may **not** go to is code nothing written down describes ([ADR-045](adr/adr-045.md) D3, Part III 15.2): there the caller opens the lock and hands over the value inside it, so the called code sees an ordinary value and no lock at all.

The answer is **the same at both settings**, so that a library written at one cannot turn out un-compilable where it is used. The question is asked of a value **and a destination**, and each destination gets one switch-independent answer; what the setting changes is only whether this build performs the crossing: at `no` nothing you wrote runs concurrently, so the task does not run and the refusal is a lint rather than an error. The diagnostic is `NK2501`, worked through in Part III C.5.

> **Status.** The check runs, it is asked about the destination, and **no type reaches `NK2501`'s refusing half** — which is now a statement about this destination rather than about the types: plain data, a `Shared[T]` and a lock all go into a task of your own. What does not is the foreign destination, where the lock is refused as `NK2502`. `spawn` **lowers** now ([ADR-055](adr/adr-055.md) §6 step 4) and so does the lock ([ADR-064](adr/adr-064.md)), so both halves of this are programs that run: a task that takes plain data, and a task that takes a `SharedMut[T]`.

**The nesting rule does not reach into a spawned task, and a lock may be handed to one.** Two different rules, and only the first is about nesting. A `spawn`'s body runs later and elsewhere rather than during the call, so it is not part of whatever its writer was holding at the time, and the rule that refuses a lock taken while a lock is held (12.3) does not reach into it ([ADR-039](adr/adr-039.md) D3). A scope is the other case, because it waits for its tasks — see 12.7.

And the paragraph above decides whether a lock gets in there at all: it does. A task of your own is a destination the crossing verdict answers *may* at both settings, so a `SharedMut[T]` can be captured by a `spawn` and the counter of 12.2 is a program you can write ([ADR-045](adr/adr-045.md) D2). Two handles then mean two places at one lock, which is what a lock is for ([ADR-040](adr/adr-040.md) D1). A task may of course also build a lock of its own, and that shares nothing with anybody.

> **Status:** the **types are built** ([ADR-064](adr/adr-064.md)) — the counter below compiles and runs at both settings, which it never did before. What is not built is the lock-touching property of 12.3: no function carries it, so nothing tells a spawned body from a scope's.

#### Return Values & Handles
`spawn` always returns a `TaskHandle`. At `yes` it represents a running thread; at `no` a scheduled event. Calling `.join()` on it works identically either way — which is what the *uniform API* below means.

**`.join()` is the one spelling, and there is no `.await`.** This section offered both until [ADR-055](adr/adr-055.md) D5, which is the record that settles it: a Nikaia program contains the word `await` nowhere (Part I, 8.1 — and a test reads the corpus to hold it), because what `.await` marks is where a function *pauses*, and in this language every function may. `.join()` is where two **tasks** meet again, which is a different thing and is worth a word. The emitted Rust is where the `.await` goes, and the compiler writes it.

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

> **Status:** built ([ADR-055](adr/adr-055.md) §6 step 4). `spawn` lowers to a
> future the executor owns, `.join()` is a suspension point rather than a wait,
> and the implicit move is refused as `NK2101` where the program wanted the
> value back — which is what the commented-out line above is about. The
> `catch` is gone from the example because `join` does not fail: what it hands
> back is whatever the body's last statement was, and a body that can fail
> hands back a value that says so, exactly as any other function does.
>
> **The `yes` column is built too**, so *"at `yes` it represents a running
> thread"* is a sentence about programs: a task goes to a pool of futures over
> the `user-pool` worker count, and four tasks of the same size take 1.58 s at
> `no` against 0.65 s at `yes` on four cores. `.join()` is the same two lines at
> either setting, which is what the *uniform API* claims.
>
> What is not built is D6's `Send` as a refusal of **this** compiler's: the
> pool's starter asks for it, so a task holding something that may not cross is
> refused in the backend's words about the generated file (Part III, C.1). The
> structural check of the section above runs on what a task *captures*; what it
> has never been asked about is what a body holds **across a pause**.

---

## Chapter 12: Thread Safety and Synchronization

Because code at `user_parallelism = yes` runs on several physical CPU cores at once, strict safety rules apply to prevent data corruption.

### 12.1. The `sync` Keyword (CPU Constraints)
Since everything in Nikaia is "Async by Default" (interruptible), we need a way to define code that **must not be interrupted** or moved between threads mid-execution.

The `sync` keyword marks a function that **never pauses**.
* **Constraint:** A `sync` function can **only** call other `sync` functions.
* **What it is not:** it is not a promise that the function has no *effect*.
  `println` is `sync` — it writes and returns, it never suspends — and so is a
  panic hook, which must be `sync` and may block
  ([ADR-006](adr/adr-006.md) D6). One word, one promise
  ([ADR-067](adr/adr-067.md) D1): what a function **reaches** is a second
  column and is answered by `touches` (13.5).

This effectively creates two worlds: the flexible **Async World** (Default) and the strict **Sync World** (Computation).

```nika
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
concurrency safe does it by demanding a `sync` lambda — `access`, `update`,
`access_all`, `par_iter`, a scope's parallel tasks, the panic hook. If `sync` were
something you had to *enter*, the set of things those lambdas could call would
be "whatever somebody remembered to annotate", and the safe path would be the
narrow one. It is the other way round: the safe path is open by default, and it
closes only where something really can pause.

**A library function that runs your lambda does what your lambda does.** `xs.map`,
`xs.filter`, `xs.sort_by_key` and the rest cannot commit to one answer — the
same `map` cannot pause over `fn(n) { n + 1 }` and can over a lambda that reads a
file — so their contracts say *the lambda decides* ([ADR-029](adr/adr-029.md)).
That is why this is fine:

```nika
counter.access fn(n) { n + xs.sort_by_key fn(x) { x } }  // pure lambda, pure call
```

and this is still refused:

```nika
counter.access fn(n) { xs.map fn(x) { fs::read("log") } } // the lambda does I/O
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

> **Status:** built, and **`sync` now decides what a function is compiled to**
> ([ADR-055](adr/adr-055.md) D1). Until that record it was a claim the compiler
> checked and nothing acted on; a function that could pause and one that could
> not were the same shape in the output. Now the column that says *"cannot
> pause"* is the column that says *plain `fn`*, and everything else is an
> `async fn`.
>
> Three things follow, and they are why the paragraph above about the compiler
> not guessing in your favour matters more than it used to. **The conservatism is
> the safe direction**: an unresolved call costs a function its claim, which
> makes it `async` — and an `async fn` that never awaits finishes on its first
> poll, where a plain `fn` that needed to pause does not compile at all. **The
> inference carries real information rather than being technically true**:
> measured across the ten programs in `examples/`, 22 functions are `sync`
> against 17 that can pause. And **a regression in the inference is now a
> regression in the output** rather than in a note.
>
> What the keyword itself does is unchanged: `NK2202` holds a body to a promise
> its author wrote down.

### 12.2. The Dual Nature of the Lock
To share data that changes, you use **`SharedMut[T]`**: several owners, one value, and the lock inside the type rather than in a second wrapper you write around it (Part I, 6.2). You make one by calling it — `let counter = SharedMut(0)` — and there is **no second spelling**: `Shared[Locked[T]]` is refused and the message names this one ([ADR-064](adr/adr-064.md) D2, D3). `Locked[T]` is the same lock on its own, for individually locked fields inside a shared structure ([ADR-039](adr/adr-039.md) D9). Everything in this section is about the lock, so it holds for both.

**The name says what you get, not what is underneath.** `SharedMut[T]` means *several own it and any of them may change it*; which shapes carry that is the compiler's, decided per value the way the owner count is ([ADR-064](adr/adr-064.md) D1). Below, it is a count around a lock, and the pair belongs to the setting.

> **The counter below may go into a task of your own, and not into foreign code.** Neither half is about the owner count: which count a value gets is decided per value and follows this answer rather than making it ([ADR-037](adr/adr-037.md) D7, Part I 6.2). It is about the **lock**, which is what this section is about, and the answer depends on where the value is going ([ADR-045](adr/adr-045.md) D1). Into a task of your own: yes, at both settings — at `yes` because a real operating-system lock is underneath and several threads are what it is for, at `no` because the task is interleaved on the same thread and nothing crosses. Into code nothing written down describes: no, at both settings, and deliberately the worse answer — at `yes` it would in fact be safe, and it is refused anyway so that a library written at one setting stays usable at the other ([ADR-045](adr/adr-045.md) D3, Part III C.5).

**At `user_parallelism = no`:**
* **Implementation:** Similar to a `RefCell` with a reentrancy check.
* **Cost:** Extremely cheap (integer increment).
* **Purpose:** It is where a **Logical Deadlock** would be caught (e.g., Task A locks data, waits for network, Task B tries to lock same data -> Panic!) — a case 12.3 now refuses when you compile, which is why this check is self-control rather than error handling (below). It does not use OS primitives.

**At `user_parallelism = yes`:**
* **Implementation:** A real OS-level **Mutex** (Mutual Exclusion).
* **Cost:** Higher (Atomic operations).
* **Purpose:** It protects against **Memory Corruption**. It ensures that two physical threads cannot write to the memory address at the same time.

**Four Doors, Not One**
A change to locked data has four shapes, and each has its own door. Only two of them run code of yours while the lock is open, and only those two carry the rules further down this section ([ADR-039](adr/adr-039.md) D10):

| form | for | the block may wait |
| :--- | :--- | :--- |
| `kasse.get()` | taking a copy out | — |
| `kasse.set(value)` | replacing; the value is computed outside | yes, outside |
| `kasse.set(neu; after: stand)` | the same, **if nothing moved** since `stand` was seen | yes, outside |
| `kasse.update fn(mut v) { v += 100 }` | changing it, under the lock | no |
| `kasse.access fn(state) { … }` | reading in place, large values | no |

Each is shaped by what it is for:

* **`set` needs no block** because arguments are evaluated before the call: whatever producing the new value costs, including waiting for I/O, is paid outside, and the lock is open for the duration of one store. `get` is the same in the other direction — one load.
* **`update` is handed the value as `mut v`, changes it, and returns nothing.** It is **where locked data changes**, small or large: `v += 100` on a counter, `v.push(entry)` on a list. What `v` is, is the compiler's by type — a **copy** where the value fits a machine word, so the block can run on the copy and be swapped in, and run again if another task got there first; the **address** in the lock otherwise, where the block runs once. Nothing is moved out of the lock in either case, and a ten-thousand-entry list is not copied to append one ([ADR-110](adr/adr-110.md) D1–D3). A block that hands a value back is refused (`NK1141`).
* **`set` takes a witness where the new value came out of the lock.** `after:` is not an option that tunes the store — it changes what the store *is*: compare, and store only if the lock still holds what was seen. It is a definition and not a new rule, because `kasse.set(neu; after: stand)` **is** `kasse.update fn(mut v) { if v == stand { v = neu } else { throw Overtaken } }` ([ADR-111](adr/adr-111.md) D5), and like every `update` it takes the lock once. The comparison is of the whole value, so a program holding something large writes the `update` block with a version field of its own instead.
* **`access` is for reading in place** — it is the one door that hands your block the value where it lies, and it **may not change it**. Without it, asking that ten-thousand-entry list for its length would copy the list; with it, nothing is copied to answer a question about a large value (D1).

**So exactly one lambda in this language is handed something it may change**, and it carries the word every changed parameter carries: `mut`. A change to locked data is written in `update`, where the word is on the line that makes it ([ADR-059](adr/adr-059.md) D3).

Two mistakes are refused at the doors. **Assigning to a `SharedMut` directly** is refused, and the message names `set`: the value lives behind a lock, so replacing it is a call and not an assignment. And **a `set` given a value that was seen in a lock, or standing under a condition that was** is refused, and the message names `update`, the door for a new value computed from the old one, and `after:`, the door for a value computed outside. The check reads the **stamp**: what a lock hands out is a `Seen[T]`, the stamp travels with the value through `let`, arithmetic, calls to lock-free functions, declared fields and time, and it never comes off — so the pair spread over two lines, two functions or two requests is refused like the one written inline ([ADR-111](adr/adr-111.md)). A third refusal closes the back door: an `update` block that assigns to `v` without reading it (`NK2207`). And `set_after`, which is how `after:` is written in the language below, is not a second way in (`NK2208`).

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

Retrying is a loop and a `catch`, and handing the failure to the caller is what a server does with it — in HTTP that is the honest 409. What `Overtaken` carries is nothing: the value in the lock *now* is not in it, because reading it would be a second acquisition, and a caller that wants it takes the door again and gets a fresh stamp.

**The `sync` Rule (No Pausing While Holding a Lock)**
Holding a lock while the program pauses is dangerous *either way*: with threads it can block a whole CPU core; without them it can freeze other tasks that need the same data. Nikaia rules this out **at compile time**, using a keyword the language already has:

> **`update`, `access` and `access_all` require a lambda that is `sync` and touches no lock — at every setting.**

A `sync` lambda (see 12.1) can never pause, and a lambda that touches no lock cannot take a second one (12.3). **Those are two conditions and not one** ([ADR-067](adr/adr-067.md) D1): `sync` answers the first, and what a body **reaches** — `touches`, the fourth derived column (13.5) — answers the second. A `println` inside a door fails the second and not the first: it never pauses, and it takes standard output's own lock while yours is open, which is a lock inside a lock. So while locked data is open, the program runs straight through: lock, compute, unlock. **`get` and `set` need no condition at all, and for a stronger reason: while the lock is open in either of them, no code of yours runs, so there is nothing that could fall due** ([ADR-039](adr/adr-039.md) D10). If you try to do I/O inside `access`, the compiler stops you with a plain explanation:

```nika
let counter = SharedMut(0)

// OK: pure computation
counter.update fn(mut n) { n += 1 }

// Compiler Error (NK2203): a lock inside a lock. `println` takes standard
// output's own, and `fs::write` can pause besides (NK2202).
// counter.access fn(n) { println(f"{n}") }
```

```text
error[NK2201]: cannot wait for I/O while holding locked data
  --> main.nika:7
   |
 7 | counter.access fn(n) { fs::write("log", "{n}") }
   |                        ^^^^^^^^^^^^^^^^^^^^^^ this writes to a file,
   |                                               which makes the program pause
   |
  note: while you hold locked data, every other task that needs it must wait.
        Pausing here could freeze them for a long time (or forever).
  help: copy the value out first, then do the I/O without holding the lock:
        let snapshot = counter.get()
        fs::write("log", "{snapshot}")
```

> **Status.** **The types and the doors are built** ([ADR-057](adr/adr-057.md),
> [ADR-059](adr/adr-059.md), [ADR-064](adr/adr-064.md)): `SharedMut[T]` and `Locked[T]` are types the
> compiler knows, both implementations are in `std`, all four doors exist and `std`'s ledger describes
> them. What is **not** built is the refusal on this page: nothing raises `NK2201`, so I/O inside a
> door is not stopped when you compile — it is specified ahead of the check, which needs the
> lock-touching property of 12.3 that no function carries yet.

This turns the old advice "don't sleep while holding a lock" from a best practice into a guarantee. Re-entering the *same* lock through a chain of calls is not an edge case left to the runtime either — 12.3 refuses that when you compile, and at every setting. The runtime checks described above keep their place for a different reason: the reentrancy check is now **self-control of that refusal rather than error handling.** No input can make it fire; if it ever fires, the compiler has a hole rather than the program having a bug. That is why it is a switch you can decline (Part I, 1.2) and why poisoning on several threads is left as it is ([ADR-039](adr/adr-039.md) D2, D8).

**And a lock is a resource, so two doors onto the same lock keep their order — where both of them write.** Two operations whose touch sets are disjoint have no order between them *inside an `overlap { … }`* (Part I 8.1.2), and a lock is one of the things a touch set can name. **`get` and `access` are reads; `set` and `update` are writes** ([ADR-059](adr/adr-059.md) D3). Two reads of one resource are unordered ([ADR-033](adr/adr-033.md) D2), so two `get`s — or two `access`es — on one lock may run in either order, while any two of the two writing forms are ordered by the same rule that orders two `println`s rather than by a rule of their own ([ADR-033](adr/adr-033.md) §3, [ADR-039](adr/adr-039.md) D10). Two doors onto *different* locks meet on nothing and need not wait for each other.

> **Status.** The compiler does not yet know a lock as a named resource — no entry in any ledger
> claims one, because nothing in the corpus has asked for one
> ([ADR-028](adr/adr-028.md) D5: an entry exists because a program asked for it, never
> speculatively). Until one does, a door onto a lock is an operation whose touches cannot be
> determined, so it counts as touching everything and keeps its place. For the three writing forms
> that is the same answer this section promises, reached by ignorance rather than by a contract; for
> two `get`s it is a stricter one, because ignorance cannot tell a read from a write. Right today,
> and not something to rely on: the moment anything else in the pair is described, the contract is
> what has to say it.

Where two doors would meet on something the touch sets cannot see, the answer is to write them as ordinary statements rather than as branches of an `overlap`: statement order is the written order (Part I 8.1.1), so nothing has to be said at all.

### 12.3. Deadlock Prevention: Atomic Composition
The classic cause of deadlocks is inconsistent locking order (Thread 1 locks A then B; Thread 2 locks B then A).
In Nikaia, taking a lock while a lock is held is **always** a compile-time error ([ADR-039](adr/adr-039.md) D2). What is refused is the nesting of lock *acquisitions* — one door (12.2) opened while another is still open — and not any way of spelling a type: one door on its own is one acquisition, whatever it opens.

**The Solution: `access_all`**
Instead of nesting `access` calls, Nikaia provides `access_all` to request multiple resources simultaneously.

* **Mechanism:** The runtime sorts the resources by their memory address (or internal ID) before locking. This guarantees a globally consistent locking order across all threads.
* **Safety:** It is mathematically impossible to create a deadlock cycle between A and B if everyone uses `access_all(A, B)`, because everyone will implicitly lock "Lower Address first, Higher Address second".

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
lock, nothing returned, nothing sees either lock between the two writes — and
whether the block runs on copies and is retried or on the addresses under the
locks is the compiler's by the pair of types, as it is for one.

> **Status: built** ([ADR-065](adr/adr-065.md)). Both doors run at both settings,
> and two tasks transferring in opposite directions neither deadlock nor lose
> anything. **Two locks at a time**: two are put in order by one comparison, and
> three need a different mechanism — a door handed one lock, or a block that does
> not name one value per lock, is refused as `NK1124`. Neither name is a keyword:
> the grammar has taken this shape since the trailing lambda, and what was missing
> was somebody to type it.

> **Status: the transfer has a door, and it is `update_all`**
> ([ADR-065](adr/adr-065.md)). `access_all` reads several locks at once, because
> [ADR-059](adr/adr-059.md) D1 made `access` a read and the same reading applies
> to it; `update` writes one; `update_all` writes several, one `mut` each. 12.3
> writes both.

**How the compiler sees a chain.** A nesting written one line inside the other is visible where it stands; a chain is not — your block calls a function of yours, which calls another, and the third one opens a lock. So every function carries a second derived property beside `sync` (12.1): **does it touch a lock.** It is inferred over the same call graph, never written by hand, and inside a blocking door (`update`, `access`, `access_all`) a call to anything that carries it is refused. That is what catches chains and self-calls, and it is what makes the rule above complete rather than only local ([ADR-039](adr/adr-039.md) D3).

**The property is coarse, and deliberately so.** It says *"a lock"*, never *which* lock. A precise version would have to prove two handles distinct, and then whether a program compiles would depend on whether that proof happens to succeed — an unrelated line elsewhere could make unchanged code stop compiling. A refusal a programmer can read is the better failure ([ADR-039](adr/adr-039.md) D4).

> **It must never be confused with 12.2's ordering rule.** That two writing doors onto the *same* lock keep their order needs to know *which* lock, and it comes from what the operation touches ([ADR-033](adr/adr-033.md) D2) — never from this property, which cannot tell two locks apart.

**A function handed outward is judged by its body, never by its type.** Where one of these rules has to ask whether a lambda touches a lock and the lambda is not run on the spot — it is handed to foreign code, or kept by the callee — the answer is computed from the body: a lambda is its captures, and nothing writes those down, so no type could answer. Today that reaches a lambda at a call site and the standard library's own detached entries, because Nikaia's type grammar has no function type at all (Part I, 5.4); it widens the day that grammar gains one ([ADR-039](adr/adr-039.md) D7).

> **Status:** the syntax of all three is built. A trailing lambda may name its
> arguments, and it may follow a plain call as well as a method call (Part I,
> 5.3), so `counter.access fn { … }`, `account.access fn(to) { … }` and
> `access_all(account_a, account_b) fn(a, b) { … }` are each one call whose last
> argument is the lambda. **What they call is built too** now: `SharedMut[T]`,
> `Locked[T]` and all four doors are in `std` and in its ledger
> ([ADR-064](adr/adr-064.md)). What is **not** built is every refusal this section
> states — nothing refuses the nesting, no function carries the lock-touching
> property, so nothing refuses a chain or a function handed outward, and nothing
> demands the lambda 12.2 does. The doors work and the rules around them do not.

### 12.4. Racing Tasks (`select`)
Sometimes you want to run multiple tasks, but only care about the one that finishes *first*.

```nika
select {
    // Case 1: Computation finishes first
    result = heavy_math() => { return result }
    
    // Case 2: Timeout happens first
    _ = sleep(5.seconds()) => { throw Timeout::TooSlow }
}
```
*Note: When one branch wins, the other task is automatically cancelled and cleaned up.*

> **Unspecified.** This writes a construct the language does not have and no
> record decides — `select { … }` itself, and the duration literal
> `5.seconds()` inside it. It is here for the shape of the example; the
> decision is taken when Part I chapter 8 is next opened
> ([ADR-141](adr/adr-141.md) D2).

> **Status:** not built. `select` is not a keyword in the parser and the block
> above is a parse error, so nothing races two tasks today and the teardown rule
> below is stated ahead of the construct it governs. The error it throws is an
> **enum variant** and not `TimeoutError("Too slow!")`, because an error type is
> an `enum` ([ADR-023](adr/adr-023.md) D1, [ADR-141](adr/adr-141.md) D1).

**What "cleaned up" means precisely:** the losing task stops at its current pause point and its values are torn down. Resources with a pausable `cleanup` (Part I, 6.4) cannot be awaited by the *winner* — you should not pay for the loser's teardown — so the runtime **adopts** their `cleanup` runs and finishes them in the background ("parked cleanup"). The program will not exit before parked cleanups are done, bounded by the `cleanup-deadline` (Part III, 13.3). Errors from a parked cleanup have no caller to bubble to; they are reported through the runtime's error hook. See [ADR-006](adr/adr-006.md), D3.

### 12.5. Channels (Message Passing)
Instead of locking shared memory, Nikaia encourages **Message Passing**.

> **Unspecified.** This writes a construct the language does not have and no
> record decides: whether a channel is `std`'s or the language's, and what it
> is called, is taken when Part I chapter 8 is next opened
> ([ADR-141](adr/adr-141.md) D2).

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
    .map fn(pixel) { pixel.brightness * 1.5 }
    .filter fn(value) { value > 0.5 }
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

**A scope may not be opened while a lock is open.** Because the scope waits for its tasks, their bodies belong to the function that writes the scope, exactly as a trailing lambda's body does — so whatever a task touches, the surrounding function touches, including a lock (12.3). One consequence is worth stating on its own: opening a scope inside an open lock is refused. A task that wants the same lock waits for whoever holds it, and the holder waits for the end of the scope; that is a deadlock, not a precaution against one. A task started with `spawn` is the other case, because it runs later and elsewhere (11.2) ([ADR-039](adr/adr-039.md) D3).

> **Status:** not built, though the syntax now is: `task::scope fn(s) { … }`
> parses as one call with the lambda as its argument, and so does the
> `s.spawn fn { … }` inside it (Part I, 5.3). What is missing is everything
> underneath — `task::scope` has no entry in `std`, nothing waits the inner
> tasks out, nothing propagates what a scope's tasks touch up to the surrounding
> function or refuses a scope inside an open lock, and `NK2102` is not reported.
>
> **What it would be built on does exist now** ([ADR-055](adr/adr-055.md) §6
> steps 1–4): an executor that owns its tasks, and a `spawn` whose body is a
> future it drives. The `no` bullet below — *"the runtime owns every task, and
> when a scope ends it collects them before your function's variables
> disappear"* — is a description of that executor rather than of something
> hypothetical, so what a scope adds is the **waiting** and the touch
> propagation, not a mechanism of its own.

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
           of its variables, so clone what you still need first:
           let target = url.clone()
           let handle = spawn fn { fetch_url(target) }
           let result = handle.join()
```

> **Design Note (Soundness):** Borrowing across *parallel, pausable* tasks is a known unsoundness trap — a cancelled scope cannot instantly stop a task mid-execution on another core, yet the borrowed variables are about to disappear. General-purpose async runtimes cannot offer a safe async scope for exactly this reason. Nikaia avoids the trap structurally: at `no` the one thread your code runs on owns all task state and tears scopes down synchronously (which additionally requires that task futures are exclusively runtime-owned and that the language exposes no way to leak a live scope — both are language-level guarantees); above `0` the `sync` restriction makes waiting deterministic. See [ADR-005](adr/adr-005.md), D5.

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
}; restart_policy: RestartPolicy::Always)
```

The policy is an **enum** and not a string ([ADR-141](adr/adr-141.md) D1). A
string naming one of a few things is the shape Part I 4.4 argues against on its
own page: a misspelling is caught at the call rather than at the restart, and
the few things are listed where they are declared.
