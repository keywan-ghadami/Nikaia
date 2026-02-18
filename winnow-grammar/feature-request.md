# Feature Request: Direct Binding of String Literals

## Problem
Currently, `winnow-grammar` does not support binding string literals directly to variables in a rule. For example, trying to capture an optional keyword like `mut` or `sync` results in a compilation error:

```rust
rule let_stmt -> Stmt =
    "let"
    mutable:"mut"?  // Error: Literals cannot be bound directly
    name:ident
    ...
```

The error message is explicit: `Literals cannot be bound directly (wrap in a rule or group if needed).`

## Workaround
The current workaround requires defining a separate rule for every keyword that needs to be captured:

```rust
rule kw_mut -> () = "mut" -> { () }

rule let_stmt -> Stmt =
    "let"
    mutable:kw_mut?
    name:ident
    ...
```

This adds unnecessary verbosity to the grammar definition, especially when dealing with many optional keywords.

## Proposed Solution
Allow direct binding of string literals, especially when used with the optional operator `?`. 

It would be intuitive if `label:"literal"?` could automatically resolve to an `Option<()>` (or similar) where `Some(())` indicates the literal was present, and `None` indicates it was not.

Alternatively, `label:"literal"` (without `?`) could bind the string slice itself (e.g., `&str` or `String`) or just be ignored if the type doesn't matter, but the `Option` case is the most critical for ergonomics.

## Example Usage

```rust
rule fn_item -> Item =
    "fn"
    name:ident
    is_sync:"sync"? // Should result in Option<()> or bool
    ...
    -> {
        Item::Fn {
            ...
            is_sync: is_sync.is_some()
        }
    }
```
