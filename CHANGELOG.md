# Changelog

## [Unreleased]

### Fixed
- **Grammar**: Fixed `expr` rule to include `block`, enabling parsing of blocks in expression positions (e.g., `spawn({ ... })`).
- **Grammar**: Added specific `spawn_expr` rule to correctly parse `spawn` statements as `Expr::Spawn` instead of generic function calls.
- **Grammar**: Added `skip_ws` rule to consume trailing whitespace at the end of the program, preventing `ParseError` at EOF.
- **Grammar**: Renamed whitespace skipper rule from `ws` to `skip_ws` to avoid infinite recursion bug in `winnow-grammar`.
- **Tests**: Fixed syntax error (missing comma) in `tests/hello_world.rs`.
- **Build**: Updated `src/main.rs` and `tests/hello_world.rs` to wrap input source in `LocatingSlice` to satisfy `winnow::stream::Location` trait bounds required by the generated parser.
