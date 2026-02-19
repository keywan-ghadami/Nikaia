# Findings regarding winnow-grammar

## Verified Behavior

1.  **Module Generation**: 
    The `grammar Name { ... }` block generates a Rust **module** with the name `Name`.
    *   *Example*: `grammar CompilerGrammar` creates `mod CompilerGrammar`.

2.  **Function Naming Convention**: 
    For every rule defined as `rule name`, the macro generates a public function named **`parse_name`**.
    *   *Observation*: `pub rule program` generated `pub fn parse_program`.
    *   *Usage*: Must be called as `CompilerGrammar::parse_program`.

3.  **Input Type Requirements**: 
    The generated parser functions require the input type to implement `winnow::stream::Location`.
    *   *Issue*: passing a raw `&str` fails with `E0277` (`&str: Location` is not satisfied).
    *   *Solution*: Wrap the input in `winnow::stream::LocatingSlice`.
    *   *Code*: `let input = LocatingSlice::new(input_str);`

4.  **Scope & Imports**: 
    The grammar block acts as a closed scope.
    *   External types (e.g., `crate::ast::*`) must be imported inside the `grammar! { ... }` block.
    *   Winnow combinators (e.g., `winnow::ascii::digit1`) must be explicitly imported to be used as terminal rules.

## Syntax Notes

*   **Brackets**: The syntax `[ ... ]` is supported directly within the grammar to define sequences enclosed in brackets or for grouping.
    *   *Example*: `rule generic_list -> Vec<T> = [ params:generic_params? ] -> { params }`
*   **Return Types**: Defined after the arrow `->` (e.g., `rule foo -> MyType`).
*   **Rule Visibility**: `pub rule` makes the generated `parse_` function public.
