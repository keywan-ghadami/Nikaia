# The error corpus

Twenty-six broken `.nika` files and what the compiler says about each. It exists
to be argued with: a message is only wrong against a claim about what a reader
needed, and this is where those claims are written down before anything is
changed.

It exists for a second reason too. Three typos measured by hand is not a corpus,
and an intermediate attempt at the expectation ranking made one case
much worse — seventeen expectations in the headline, twenty-two in a note, the
position on a token that was correct — which was caught by accident rather than
by a test. Every path taken from here should show its effect on all
twenty-six at once.

**Two columns, because there are two states.** *Before* is `winnow-grammar`
`024e3d3`. *Today* is `e1b0e33`, which ranks an expectation by whether the
grammar required it (winnow-grammar#4) and is what a user gets now. Where they
differ, the difference is what that change bought.

A message today can carry a second line, `note: also possible here: …`, holding
what the grammar would have accepted but did not require. The columns below
quote the headline, which is the part a reader acts on; where the note matters
to a row, the row says so.

`✅` says what a reader needs · `⚠️` does not · `○` does not fail at all

---

## A. A separator or terminator is missing

| # | input | the reader needs | before | today |
| :-- | :--- | :--- | :--- | :--- |
| A1 | `struct S { name: &str` ⏎ `temp: i32 }` | `,` or `}` | ⚠️ `` `//`, whitespace `` | ✅ `` expected one of: `,`, `}` `` |
| A2 | `fn f(a: i32 b: i32) {}` | `,` or `)` | ⚠️ `` `//`, whitespace `` | ✅ `` expected one of: `)`, `,` `` |
| A3 | `fn f() {` ⏎ `let x = 1` | `}` at end of input | ⚠️ 17 tokens | ⚠️ 17 tokens, 22 in the note |
| A4 | `let xs = [1, 2` | `,` or `]` | ⚠️ `` `//`, whitespace `` | ⚠️ unchanged |
| A5 | `struct S { a: i32,, b: i32 }` | a field name | ⚠️ `` `//`, whitespace `` | ✅ `` expected one of: `}`, identifier `` |

A4 is the clearest remaining case of the whitespace skip winning on *progress*:
it is tried at the start of every rule, so at a position no real parser reached
it is trivially the furthest thing that failed.

## B. An operand is missing

| # | input | the reader needs | before | today |
| :-- | :--- | :--- | :--- | :--- |
| B1 | `let y = ` | **`expected expression`** | ⚠️ `` `//`, whitespace `` | ⚠️ unchanged |
| B2 | `let y = 1 + ` | `expected expression` | ⚠️ 8 tokens | ⚠️ 6 tokens |
| B3 | `if { }` | `expected expression` | ○ **parses** | ○ |
| B4 | `f(1, )` | `expected expression` | ⚠️ 8 tokens | ⚠️ 6 tokens |
| B5 | `let x: = 1` | `expected type` | ⚠️ `` `//`, whitespace `` | ✅ `` expected one of: `&`, identifier `` |

B1, B2 and B4 are the group this corpus was built around: no ranking makes a
list of six token spellings into the word *expression*. That needs a label, and
labels have a shape problem — see the note at the end.

B5 comes out as ``&``-or-identifier rather than *type*, which is the same
question one level down: `type_ref` is a single-variant rule and can carry a
label as it stands.

## C. The wrong token where the grammar knows what belongs

| # | input | the reader needs | before | today |
| :-- | :--- | :--- | :--- | :--- |
| C1 | `struct S { name &str }` | `:` | ⚠️ `` `//`, whitespace `` | ✅ ``expected `:` `` |
| C2 | `rule A -> i32 = n:digit1 { n }` | `->` | ⚠️ `` `//`, whitespace `` | ⚠️ `expected digits` |
| C3 | `fn main( {` | `)` or a parameter | ⚠️ `` `//`, whitespace `` | ✅ `` expected one of: `&`, `self` `` |
| C4 | `let 5 = x` | a name | ○ **parses** | ○ |
| C5 | `impl S { struct T {} }` | a method | ⚠️ `` `//`, whitespace `` | ✅ `` expected one of: `fn`, `pub` `` |

C1 is the best row in the corpus and worth keeping as the example: `expected ':'`
is exactly what a reader can act on.

C2 does not improve, and the reason is not the ranking: `digits` comes from the
built-in name table (`digit1 => "digits"`), and the rule really is looking at a
repetition there. It is a grammar question, not a message question.

C3 names `&` and `self` but not `)`, which is honest — an empty parameter list
is one alternative and the message shows the other one it was in the middle of.

## D. Almost the right token

| # | input | the reader needs | before | today |
| :-- | :--- | :--- | :--- | :--- |
| D1 | `/ a broken comment` at top level | `//` named | ⚠️ 4 tokens incl. `//` | ✅ `expected end of input` |
| D2 | `if a = b { }` | open question | ○ **parses** | ○ |
| D3 | `a:B -> C` where `=>` was meant | the cut is `=>` | ⚠️ `` `//`, whitespace `` | ✅ ``expected `{` `` |

D1 is the case where a comment form *is* the answer, and the decision to keep
trivia out of messages entirely gives it up: `expected end of input` is correct
and unhelpful. Recorded so the trade is visible, not to argue it.

## E. Inside a `grammar` block

| # | input | the reader needs | before | today |
| :-- | :--- | :--- | :--- | :--- |
| E1 | `d:digit{1, -> { 1 }` | a number or `}` | ⚠️ incl. `//` | ✅ `` expected one of: `}`, digits `` |
| E2 | `d:nosuchbuiltin` | the backend rejects it | ○ parses, as intended | ○ |
| E3 | `par_fold(M, init)` | `,` — arity is the backend's | ⚠️ incl. `//` | ✅ 6 tokens, no trivia |

## F. The cause is far from the symptom

| # | input | the reader needs | before | today |
| :-- | :--- | :--- | :--- | :--- |
| F1 | `let s = "unterminated` | the **opening quote** | ⚠️ EOF, `` `\`, any character `` | ⚠️ unchanged |
| F2 | a stray `}` at top level | `unexpected '}'` | ⚠️ 4 tokens | ✅ `expected end of input` |
| F3 | one unclosed `fn` | `}`, and where the `{` was | ⚠️ 17 tokens | ⚠️ 17 tokens, 22 in the note |

F1 and F3 are a *position* problem, not a ranking one, and no amount of work on
expectations touches them. They need the opening delimiter remembered so the
message can point back at it. Separate mechanism, separate decision, listed here
so the two are not confused.

## G. Not ASCII

| # | input | the reader needs | before | today |
| :-- | :--- | :--- | :--- | :--- |
| G1 | `let x = “hi”` | the quote named, column right | ⚠️ `` `//`, whitespace `` | ⚠️ unchanged |
| G2 | `let café = 1` | accepted | ○ **parses** | ○ |

The column in G1 is right in both states, which is worth knowing: the offsets
are character-counted, not byte-counted. A byte that is not UTF-8 at all belongs
to `fs::map` ([ADR-016](specification/adr/adr-016.md)) and is tested there.

---

## What the corpus turned up on its own

**Five cases do not fail.** `if { }` (B3), `let 5 = x` (C4) and `if a = b { }`
(D2) are all accepted by the grammar. Those are not message bugs, they are
things the grammar admits that probably should not be — and the corpus found
them by trying to break the compiler on purpose.

**The change moved twelve of twenty-one failing rows** and leaves nine.
Those nine sort into exactly three groups, and each needs a different mechanism:

1. **A list where a word belongs** — A3, B1, B2, B4, F3. A label, and the
   obstacle is documented: `ParseError::labelled` substitutes only when the
   alternative consumed nothing, and the code generator can wrap only a
   *single-variant* rule. `primary_expr` has twelve alternatives, so
   `expected expression` needs them collapsed into an inline group
   (`(a | b | …) # "expression"`), which the backend structurally supports and
   no upstream test covers.
2. **Trivia still winning on progress** — A4, B1, G1. The whitespace skip runs
   at the start of every rule, so it reaches offsets nothing else did, and
   progress is decided before any ranking. The obvious fix was tried and
   reverted: it made B1 far worse.

   A3 and F3 show the second half of the same shape. Their headline no longer
   names trivia, but it names seventeen tokens and the note names twenty-two
   more — at end of input every continuation of every open rule is live at one
   offset, and ranking cannot shorten a list where every entry is genuinely
   possible. Only a label can (group 1), which is why they are counted there
   too.
3. **The position is wrong, not the text** — F1, F3.

**No parse error shows the source line.** Every row above is a one-line headline
plus `in <rule>` lines. A rustc diagnostic routed through `nikaia --explain`
gets a snippet and a caret ([ADR-012](specification/adr/adr-012.md)); a parse
error gets `at line 3, column 5` and no line 3. That asymmetry applies to all
twenty-six rows and may be worth more than any of the three groups above.

## Keeping this honest

The inputs are `tests/errors/*.nika`, beside the existing `tests/samples/`, and
`tests/errors/EXPECTED.txt` holds what each produces **today, against the
backend in `Cargo.lock`** — so the suite is green as checked in and the messages
that are still wrong are on the record rather than in a memory.

```bash
cargo run -p nikaia --example errors > tests/errors/EXPECTED.txt
```

`crates/nikaia/tests/errors.rs` compares the whole file in one assertion, the
shape `grammar_lowering.rs` already uses. A backend bump makes that test fail
whenever it changes a message, which is the point: the diff is the change, in
the reader's terms rather than the parser's. Read it, then regenerate.

The list itself is still meant to be argued with. A row struck or added here
should be a file added or removed there, and the golden regenerated.
