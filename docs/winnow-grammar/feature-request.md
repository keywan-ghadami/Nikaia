# Feature Requests & Bug Reports

## Bugs

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
