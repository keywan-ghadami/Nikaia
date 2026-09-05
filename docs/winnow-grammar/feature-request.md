# Feature Requests & Bug Reports

> **Note:** `winnow-grammar` lives in its own repository
> (<https://github.com/keywan-ghadami/winnow-grammar>). The files in this
> directory are Nikaia-side notes; `README.md` here is a copy and is older than
> upstream `main`. Report items from this file there.

## Bugs

### `Symbol` round-trip is off by one (blocks interning)

`InternerContext::resolve` returns the *next* symbol's text, or panics with
`Key out of bounds` on the most recently interned one.

`lasso::Spur` stores `index + 1` internally:

* `Spur::into_inner()` yields `index + 1`
* `Spur::try_from_usize(n)` builds a key from an **index**, storing `n + 1`

`Symbol` combines the two without compensating (`src/interner.rs`):

```rust
pub fn from_spur(spur: Spur) -> Self { Self(spur.into_inner()) }          // stores index + 1
pub fn into_spur(self) -> Spur { Spur::try_from_usize(self.0.get() as usize) } // makes index + 2
```

Reproduction — parse `fn foo() {} fn main() {}` with a grammar binding
`name:ident`, then resolve the first function's symbol: it yields `"main"`.
With a single function it panics instead.

Suggested fix: `Spur::try_from_usize(self.0.get() as usize - 1)`, plus a test
that resolves the *last* interned symbol (the existing `tests/interning_test.rs`
only compares symbols to each other, so it passes either way).

**Impact on Nikaia:** the grammar uses the non-interning `raw_ident` terminal and
the AST stores `String`. Spec Part II 10.6 / ADR-007 §4 call for interned
identifiers; that switch waits on this fix.

### Infinite Recursion with rule named `ws`
Defining a rule named `ws` that uses `multispace0` (or likely any external parser) causes a stack overflow/infinite recursion.
```rust
// Causes stack overflow
rule ws -> () = multispace0 -> { () }
```
It seems `winnow-grammar` might be using `ws` internally or generating a recursive call when this name is used. Renaming the rule to `skip_ws` resolves the issue.

## Feature Requests

### Automatic Trailing Whitespace Consumption
`winnow-grammar` handles whitespace between tokens automatically, but strict parsing (like `Parser::parse`) fails if there is trailing whitespace at the end of the input.
It would be helpful if the generated entry point parser automatically consumed trailing whitespace or if there was a configuration to enable this.
**Workaround:** Added a manual `skip_ws` rule at the end of the entry rule.

### Documentation Improvements
The current documentation or implicit knowledge is lacking in several areas which caused implementation delays:

1.  **Function Naming Convention**: It is not explicitly documented that `rule name` generates `fn parse_name`. This was discovered through trial and error (compiler errors).
    *   **Request**: Clearly document the naming convention for generated functions.

2.  **Input Type Requirement**: The generated parser functions require `LocatingSlice` (trait bound `winnow::stream::Location`), failing with raw `&str`.
    *   **Request**: Document the required input traits or types.

3.  **Bracket Syntax**: The support for `[...]` syntax (without strings) for grouping/brackets is valuable but undocumented.
    *   **Request**: Document the supported syntax for grouping and sequences (e.g., `[]`, `()`, etc.).
