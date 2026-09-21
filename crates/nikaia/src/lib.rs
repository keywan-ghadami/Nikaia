// crates/nikaia/src/lib.rs
//
// The compiler front-end as a library, so integration tests (and later other
// tools) can drive it. The binary in `main.rs` is the CLI on top of this.

pub mod ast;
pub mod build_time;
pub mod check;
pub mod contracts;
pub mod describe;
pub mod diagnostics;
pub mod dsl;
pub mod emit;
pub mod fixed;
pub mod fold;
pub mod foreign;
pub mod interpreter;
pub mod manifest;
pub mod modules;
pub mod parser;
pub mod project;
pub mod specbook;
pub mod sysroot;
pub mod traits;
pub mod types;
pub mod views;
