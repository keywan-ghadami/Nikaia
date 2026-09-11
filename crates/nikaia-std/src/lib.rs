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
pub mod error;
pub mod fs;
pub mod hash;
pub mod html;
pub mod io;
pub mod list;
pub mod rt;
pub mod task;

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
    pub use crate::error::Full;
    pub use crate::fs;
    pub use crate::hash::{TrustedMap, TrustedSet};
    pub use crate::html;
    pub use crate::io;
    pub use crate::list::ListExt;
    pub use crate::task;
    // `rt` is in the prelude so that the `fn main` the emitter writes can name
    // `rt::start` without a `use` the program did not ask for. Nothing in a
    // `.nika` file reaches it: ADR-038 D3's whole point is that a program says
    // `fs::read_to_string(p)` and the runtime is invisible.
    pub use crate::rt;
    pub use crate::text::digit_value;
    pub use std::collections::HashMap;
}
