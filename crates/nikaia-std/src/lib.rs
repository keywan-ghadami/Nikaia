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
//! Some of it is written in Nikaia (ADR-014 D1). The `.nika` source is the one
//! to edit; the `.rs` beside it is what the Stage 0 compiler lowered it to, is
//! committed, and is what the modules below include. **This crate has no build
//! dependency on the compiler** and must not grow one: building `std` needs
//! nothing but `rustc` (ADR-002 D4). `nikaia lower-std` regenerates the `.rs`,
//! and `crates/nikaia/tests/sysroot.rs` fails if what is committed has drifted
//! from what the compiler produces.

pub mod abort;
pub mod channel;
pub mod cli;
pub mod concat;
pub mod count;
pub mod error;
pub mod foreign;
pub mod fs;
pub mod hash;
pub mod html;
pub mod index;
pub mod io;
pub mod list;
pub mod lock;
pub mod num;
pub mod rt;
pub mod task;
pub mod time;

/// The parser backend a generated program's grammars run on.
pub use winnow_grammar;

/// `std::text`, and the first module here that is Nikaia rather than Rust:
/// `src/text.nika`, lowered to `src/text.rs` by the Stage 0 compiler.
///
/// `include!` rather than `mod text;` so that the file keeps reading as what it
/// is - the compiler's output, committed - rather than as something written by
/// hand here.
pub mod text {
    include!("text.rs");
}

/// What a `use std::…` in a Nikaia program brings into scope.
pub mod prelude {
    pub use crate::channel;
    pub use crate::channel::{Receiver, Sender};
    pub use crate::cli;
    pub use crate::error::Full;
    // **The C boundary's one `std` type**
    // ([ADR-147](../../../docs/specification/adr/adr-147.md) D4): a program
    // that declares `fn getenv(name: &[u8]) -> CStr` has to be able to name
    // it, and until [ADR-154](../../../docs/specification/adr/adr-154.md)
    // decides what the prelude is, this is how a name reaches a program.
    pub use crate::foreign::CStr;
    pub use crate::fs;
    pub use crate::hash::{TrustedMap, TrustedSet};
    pub use crate::html;
    pub use crate::io;
    pub use crate::list::ListExt;
    pub use crate::task;
    // **A duration and the call that waits one out**
    // ([ADR-150](../../../docs/specification/adr/adr-150.md) D1, D2). The type
    // so that `let deadline: Duration = 2.minutes()` can be written, the
    // extension because `5.seconds()` is a method call and a trait has to be
    // in scope for one, and `sleep` because Part II 12.4 writes it bare — the
    // same way `digit_value` is written bare.
    pub use crate::time;
    pub use crate::time::{sleep, Duration, DurationExt};
    // `rt` is in the prelude so that the `fn main` the emitter writes can name
    // `rt::start` without a `use` the program did not ask for. Nothing in a
    // `.nika` file reaches it: ADR-038 D3's whole point is that a program says
    // `fs::read_to_string(p)` and the runtime is invisible.
    pub use crate::rt;
    pub use crate::text::digit_value;
    pub use std::collections::HashMap;
}
