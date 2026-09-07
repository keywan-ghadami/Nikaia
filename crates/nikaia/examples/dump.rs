//! Lower a .nika file to Rust and print it.
//!
//! The library-only path to the emitter: the `nikaia` binary links the rustc
//! backend, which this does not need. It is also what regenerates the checked-in
//! output in `tests/fixtures/`:
//!
//! ```text
//! cargo run -p nikaia --example dump -- crates/nikaia/tests/fixtures/measurements.nika \
//!     > crates/nikaia/tests/fixtures/measurements_expected.rs
//! ```

use nikaia::emit::{emit_program, Profile};
use nikaia::parser::parse_to_ast;

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let path = args
        .next()
        .ok_or_else(|| anyhow::anyhow!("usage: dump <file.nika> [lite|advanced]"))?;
    let profile = match args.next() {
        Some(name) => Profile::parse(&name)?,
        None => Profile::default(),
    };

    let source = std::fs::read_to_string(&path)?;
    let parsed = parse_to_ast(&source)?;
    print!("{}", emit_program(&parsed, profile)?);
    Ok(())
}
