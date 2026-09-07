//! Nikaia's `std`, as far as Stage 0 needs it.
//!
//! A transpiler has to answer "what is `fs::map`" somewhere, and the two places
//! it could are a table inside the emitter or a crate the generated program
//! links against. This is the second, because it makes the answer readable: the
//! semantics of `std::fs` are code with tests, not string substitutions in a
//! printer (ADR-013).
//!
//! What is here is exactly what `examples/1brc.nika` reaches for, and nothing
//! is stubbed: every function does what the specification says it does, or it
//! is not here at all.

pub mod cli;
pub mod fs;
pub mod list;
pub mod text;

/// What a `use std::…` in a Nikaia program brings into scope.
pub mod prelude {
    pub use crate::cli;
    pub use crate::fs;
    pub use crate::list::ListExt;
    pub use crate::text::digit_value;
    pub use std::collections::HashMap;
}
