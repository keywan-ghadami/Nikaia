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
