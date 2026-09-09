//! `std::io` - standard input.
//!
//! Part III 17.1 and [ADR-019](../../../docs/specification/adr/adr-019.md). A
//! stream is not a file, and the surface says so: no map, no seek, no length,
//! and no second read of the same bytes. `std::fs`'s zero-copy story rests on
//! pages that exist before the program asks for them; stdin's bytes do not
//! exist until they are read, so a program that wants views into its input owns
//! the buffer first.
//!
//! Neither function is `sync`: a read from a pipe is a suspension point, which
//! is exactly what the `sync` rule (Part II, 12.1) exists to keep out of a
//! `par_iter` body. Stage 0 emits ordinary Rust, so the suspension is the
//! blocking read the runtime binding will replace - what the source says does
//! not change when it does.

use std::io::Read;

/// All of standard input, as text.
///
/// UTF-8 validated, for the reason `fs::read_to_string` validates: a parser
/// handed bytes that are not text would find that out one view at a time, and a
/// frame boundary is a string.
///
/// Reading it a second time yields what the operating system says, which is
/// nothing.
pub fn read_to_string() -> Result<String, std::io::Error> {
    let mut text = String::new();
    std::io::stdin().lock().read_to_string(&mut text)?;
    Ok(text)
}

/// All of standard input, as bytes.
///
/// The half for input that is not text - and the one that says what
/// `read_to_string` is doing, since the only difference is the check.
pub fn read() -> Result<Vec<u8>, std::io::Error> {
    let mut bytes = Vec::new();
    std::io::stdin().lock().read_to_end(&mut bytes)?;
    Ok(bytes)
}
