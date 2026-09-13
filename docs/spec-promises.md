# What the specification promised, and what the compiler answers

The laboratory notebook page behind the **Status** notes added to Parts I–III.
Nothing here is normative: the rule is in the specification, and this is the
evidence that each note says the truth.

## Method

Every row below is a `.nika` file of one or two lines, written to use exactly one
construct and handed to the compiler as

```
nikaia --input <probe>.nika -o <probe>.rs
```

on the release build of the branch's `origin/main` base. The **Result** column
records what happened, in three kinds:

* **parse error** — the file is refused, and the message is quoted.
* **parses, means something else** — the file is *accepted*, and the emitted Rust
  is not what the source says. This is the dangerous kind: no construct in the
  parser is keyword-protected, so a bare keyword becomes an ordinary name and the
  brace-led thing after it becomes a statement of its own.
* **built** — the construct exists and lowers.

Four rows were re-run after the run below, because the parser changed under
them: the three trailing-lambda rows are now **built**, and the row they were
re-run beside — `fn(a) sync { … }` — is unchanged. Those four say "re-run" in
the Result column; every other row is the original run.

## The constructs

| Construct | Specified in | Result |
| :--- | :--- | :--- |
| `trait Name { … }` | I 4.7 | parse error: *expected end of input; found `trait`* |
| `pub trait Name { … }` | I 4.7 | parse error: *expected one of `enum`, `struct`; found `trait`* |
| `impl Name for T { … }` | I 4.7 | **built** |
| `[1, 2, 3]` | I 4.5, II 12.6, III 14.4 | parse error: *expected expression; found `[`* |
| `[]` | II 12.6 | parse error: *expected expression; found `[`* |
| `-> [User]` (list type) | III 14.3 | parse error: *expected type; found `[`* |
| `Vec[i64]`, `xs[0]`, `xs[k] = v` | I 4.5 | **built** |
| `macro Name(…) -> AstExpr { … }` | II 10.3 | parse error: *expected end of input; found `macro`* |
| `quote { … }` | II 10.3, 10.4 | parses, means something else: `let q = quote;` then `{ 1 + 1 }` — **re-run, and still silent.** `NK1117` reaches a statement that is one name, and this one is the *value of a `let`*, which is `open-work.md` §1.1 |
| `struct User with Describe { … }` | II 10.3 | parse error: *expected `{`; found `with`* |
| `const NAME: T = …` (item) | II 10.2, I 9.2 | parse error: *expected end of input; found `const`* |
| `const NAME = …` (in a body) | II 10.2 | parses as `const; LIMIT = 10;` — **re-run**, and refused now: `NK1117` on the name |
| `select { … }` | II 12.4 | parse error: *expected expression; found `>`* (at the arm's `=>`) |
| `test "name" { … }` | III 14.1 | parse error: *expected end of input; found `test`* |
| `test "name" (u: User) { … }` | III 14.3 | parse error: *expected end of input; found `test`* |
| `bench "name" { … }` | III 14.4 | parse error: *expected end of input; found `bench`* |
| `assert cond` | III 14.1, 14.2 | parses as `assert; b != 0;` — **re-run**, and refused now: `NK1117`, *"nothing declares `assert`, and this statement is just that name"*. The assertion still does not exist; it no longer disappears in silence |
| `assert cond, "message"` | III 14.2 | parse error: *expected `}`; found `,`* |
| `assert(cond)` | — | parses as a call to a function named `assert` |
| `extern "C" { … }` | III 15.1 | parse error: *expected end of input; found `extern`* |
| `unsafe { … }` | III 15.1, 16.3 | parses as `unsafe; { puts("hi") }` — **re-run**, and refused now: `NK1117` on the name |
| `null` | I 2.3, 3.5 | **built** ([ADR-052](specification/adr/adr-052.md)): a reserved word, lowering to `None` |
| `String?`, `&str?` | I 2.3 | **built**: `Option<String>`, `Option<&str>` — and the `Some(…)` where a plain value stands in a nullable slot is the compiler's to write |
| `a?.b` | I 3.5 | **built** ([ADR-052](specification/adr/adr-052.md) D6): `map` over a plain field, `and_then` over one that is itself a `T?`. `a?.m()` is not — it needs the method's return type to pick between them (`open-work.md` §2.4) |
| `a ?? 1` | I 3.5 | **built**: `a.unwrap_or_else(\|\| 1.into())` |
| `use a::{b, c}` | II 10.5 | **re-run**, and a sentence now: *"names are not brought in; a package is reached through its name. Write `use http`, and `http::Request` where you need it — and `use http as h` if the prefix is long"*, with the caret on the brace ([ADR-046](specification/adr/adr-046.md) D2) |
| `use std::fs`, `use utils` | I 9.1 | **built** |
| `Point { line, .. }` (pattern) | I 7.1 | parse error: *expected `}`; found `.`* |
| `Point { line }` (pattern) | I 3.4 | **built** |
| `_ => throw error` (arm body) | I 7.1 | parse error: *expected one of `(`, `{`* |
| `_ => { throw error }` | — | **built** |
| `fn f(g: fn())` (function type) | I 5.4 C, III App. B.2 | parse error: *expected `)`; found `(`* |
| `fn f(g: @detached fn())` | I 5.4 C | parse error: *expected type; found `@`* |
| `x.m fn { … }` | I 5.3 | **built** |
| `x.m(a) fn { … }` | I 5.3 | **built** |
| `x.m fn(a) { … }` | I 5.3, II 12.3 | **built** (re-run after the trailing lambda took a parameter head): `users.map(\|user\| { … })` |
| `f(a, b) fn(x, y) { … }` | II 12.3 | **built** (re-run): `access_all(a, b, \|x, y\| { … })` |
| `p::q fn(s) { … }` / `p::q fn { … }` | II 12.7 | **built** (re-run): `task::scope(\|s\| { … })` — the callee has no `std` entry, so the *call* is what is built |
| `fn(a) sync { … }` | I 7.2 | parses, means something else: `fn(a); sync { a };` — **re-run, and refused** since `sync` became a reserved word ([ADR-051](specification/adr/adr-051.md)): *expected `{`; found unexpected token `sync`*, at the annotation. What is unbuilt is unchanged — a parameter list is still followed by the body and by nothing else — and the three statements are gone |
| `1_000` | I 2.2 (silent) | parses, means something else: `let x = 1; _000;` |
| `0xFF` | I 2.2 (silent) | parses, means something else: `let x = 0; xFF;` |
| `1i64` | I 2.2 (silent) | parses, means something else: `let x = 1; i64;` |
| `/* … */` | I ch. 2 (silent) | parse error: *expected `}`; found `/`* |
| `/// doc` | I ch. 2 (silent) | parses as an ordinary `//` comment |
| unterminated `"…` | I 2.5 (silent) | the newline is inside the literal; the literal runs to the next `"` or to end of file |

## The commands

`nikaia --help` on the same build offers `build`, `run`, `lower-std` and `help`.

| Command | Specified in | Result |
| :--- | :--- | :--- |
| `nikaia build`, `nikaia run` | III 13.2 | **built** |
| `nikaia lower-std` | III 13.2b | **built** |
| `nikaia new` | III 13.1 | `error: unrecognized subcommand 'new'` |
| `nikaia test`, `nikaia bench`, `nikaia fmt` | III 13.2 | not present |
| `nikaia explain <code>`, `nikaia explain --tethers` | I 6.6, 7.1 | `error: unrecognized subcommand 'explain'` |
| `nikaia build --with-asserts` | III 14.2 | no such flag |
| `nikaia bench --history` | III 14.4 | no such command |
| `--locked`, `--overlaps`, `--trust`, `--explain`, `--ordering`, `--no-cache`, `--target`, `--user-parallelism`, `--backend` | III 13.2, 13.5, I 8.1.1 | **built** |

Note that `--explain` (read `rustc` JSON on stdin) and the `nikaia explain`
*subcommand* of Part I are two different things; only the flag exists.

## The diagnostics

Codes the compiler emits: `NK1101`–`NK1113`, `NK2202`, `NK2501`, `NK2502`,
`NK2605`, `NK2701`. Codes Part III C.3 catalogues and nothing raises: `NK2101`,
`NK2102`, `NK2201`, `NK2301`, `NK2401`, `NK2601`, `NK2602`, `NK2603`, `NK2604`.

`NK2605` was added after this page was written, for the shape
[`from-for-throws-and-touches.md`](from-for-throws-and-touches.md) §6 found: a
written call that can fail, in a function that declares nothing.

## Left for the owner

Three things were **not** decided here and the specification was unchanged at
every one of their sites. The second has since been decided; the note is kept
with what it was, because a page that quietly loses a finding cannot be read
against the day it was written.

1. **`spawn`.** The specification writes `spawn fn { … }` twelve times
   — six in Part I (5.4 B, 8.2, and four in 8.3, two of them inside `NK2101`'s
   own text and `help:`) and six in Part II (11.2, 12.5, two in 12.7, and two
   inside `NK2102`'s text and `help:`) — and the parser is `spawn "(" expr ")"`.
   `spawn fn { … }` parsed as `spawn; || { … };`, two statements.

   **Re-run after the trailing lambda reached a path.** It is now *one*
   expression, `spawn(|| { … })`, and `spawn fn(x) { … }` likewise — a call to a
   function named `spawn`, which is what the source says and what nothing
   provides. `rustc` moved from `cannot find value 'spawn' in this scope` to
   `cannot find function 'spawn' in this scope`: the same `E0425`, one noun
   closer to the truth, and still an error. Whether `spawn` is a keyword form
   that becomes a task or a `std` function that takes a lambda is the decision
   left here; nothing in the parser's `spawn` rule changed.

   **Re-run again, after the reserved-word list.** `spawn` is a reserved word
   now ([ADR-051](specification/adr/adr-051.md)), so there is no variable and no
   function of that name for the line to be read as: `spawn fn { … }` is
   *expected `(`; found unexpected token `fn`*, this compiler's refusal at the
   `fn` rather than `rustc`'s about the generated file. Both readings above are
   gone and the decision left here is untouched — what `spawn` **means** is
   still `open-work.md` §2.1, and the parser's `spawn "(" expr ")"` rule is still
   what it was.

   **The spelling is settled; the rest of this entry is not.** `spawn fn { … }`
   is the one form, in all three Parts — the parenthesised `spawn({ … })` that
   Part III Appendix C.5's `NK2501` example wrote is the thirteenth site this
   page did not count, and it is now `spawn fn { … }` like the other twelve.
   That closes the *disagreement between two normative documents* and nothing
   else: `spawn` is a call whose last argument is a lambda, which is the trailing
   lambda Part I 5.3 already has, and the parser's `spawn "(" expr ")"` rule is
   still what it was. So the compiler-against-specification half of this entry
   stands exactly as written above.
2. ~~**`throws` with a type.**~~ **Settled, and fixed.** Part I 7.1 states that
   `throws` names no types and the parser agrees; the four sites that
   contradicted it — Part I 6.4 once in code, twice in prose and once inside
   `NK2601`'s own `help:`, and Part II 10.2 B's `throws ParseError` — are
   corrected to Part I 7.1's form. `throws IoError` still does not parse, and
   the refusal is now a sentence rather than a parse error at the type name:
   `kw_throws` carries a `fail(…)` citing [ADR-023](specification/adr/adr-023.md)
   D1 and pointing at `nikaia.contracts`, on the precedent of ADR-022's removed
   `fn: …` form in the same file. The `help:` line was the worst of the four,
   because the compiler's own suggested fix told the user to write what the
   compiler refuses.
3. **Where the honest fix is a removal.** Part II 10.3–10.4 is two pages of
   `macro` and `quote` with nothing underneath them, and Part III Chapter 14 is
   a whole chapter. Each now carries a Status note; whether a chapter specified
   that far ahead should stay is a decision, not a documentation defect.
