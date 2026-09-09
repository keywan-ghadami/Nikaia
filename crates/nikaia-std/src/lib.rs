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
//!
//! Some of it is written in Nikaia. `build.rs` compiles every `src/*.nika`
//! with the Stage 0 compiler when this crate is built, and the modules below
//! include the result (ADR-014).

pub mod cli;
pub mod fs;
pub mod html;
pub mod io;
pub mod list;

/// The parser backend a generated program's grammars run on.
pub use winnow_grammar;

/// `std::text`, and the first module here that is Nikaia rather than Rust:
/// `src/text.nika`, compiled by the Stage 0 compiler in `build.rs`.
pub mod text {
    include!(concat!(env!("OUT_DIR"), "/text.rs"));
}

/// What a `use std::…` in a Nikaia program brings into scope.
pub mod prelude {
    pub use crate::cli;
    pub use crate::fs;
    pub use crate::html;
    pub use crate::io;
    pub use crate::list::ListExt;
    pub use crate::text::digit_value;
    pub use std::collections::HashMap;
}
