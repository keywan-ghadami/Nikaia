# Findings regarding winnow-grammar

## Usage Observations

1.  **Rule Visibility**: The `grammar!` macro generates a structure or module. Accessing the rules from outside might require specific visibility settings or path usage.
    - *Issue*: `use of undeclared type CompilerGrammar`.
    - *Hypothesis*: The macro might generate a module `CompilerGrammar`. If defined inside `parser/mod.rs`, it should be accessible via `CompilerGrammar` or `self::CompilerGrammar`.

2.  **Helper Rules**: Standard helpers like `bracket`, `paren`, `brace` are not automatically available or imported.
    - *Action*: These need to be defined manually in the grammar or imported if they exist in a library.
    - *Workaround*: Define `rule bracket<T> = "[" t:T "]" -> { t }` if generics are supported, or specific rules like `rule bracket_generics = "[" ... "]"`.

3.  **Imports in Grammar**: External parsers like `digit1` need to be imported within the grammar block or wrapper rules must be defined.

## Errors Encountered
- `Undefined rule: 'bracket'`
- `failed to resolve: use of undeclared type CompilerGrammar`
