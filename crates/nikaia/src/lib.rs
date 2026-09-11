// crates/nikaia/src/lib.rs
//
// The compiler front-end as a library, so integration tests (and later other
// tools) can drive it. The binary in `main.rs` links the rustc backend on top
// of this; nothing here depends on `rustc_private`.

pub mod ast;
pub mod check;
pub mod contracts;
pub mod diagnostics;
pub mod emit;
pub mod interpreter;
pub mod manifest;
pub mod modules;
pub mod parser;
