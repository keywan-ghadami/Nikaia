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

use std::io::{BufRead, Read};

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

/// Standard input, one line at a time.
///
/// Part III 17.1, and the first thing here that reads an unbounded input in
/// **constant memory**: nothing holds the whole stream, so a pipe that never
/// ends is a program that never grows.
///
/// The lines are **owned**, and 17.1 says why: a file's line can be a view
/// because the file is still there to point at, and a stream's bytes are gone
/// once consumed. Keeping them would be `read_to_string` with extra steps.
/// That is also what keeps this expressible at all - an iterator may hand out
/// views into a buffer it does not own, and never into one it does
/// ([ADR-025](../../../docs/specification/adr/adr-025.md), Wall B).
///
/// **A step can fail**, and the item says so rather than pretending a failed
/// read is the end of the stream - which would turn a truncated input into a
/// shorter one, silently, and is the bug class Part I 6.4 refuses by name. What
/// a Nikaia program writes has none of that in it:
///
/// ```nika
/// fn count() -> i64 throws {
///     let mut n = 0
///     for line in io::lines() { n += 1 }
///     return n
/// }
/// ```
///
/// The failure travels out of the loop and out of the function, and the
/// compiler is what made `throws` be there (`NK2701`). `std.contracts` records
/// `iterates = "throws"` on `Lines`, which is how a compiler that cannot see
/// this body knows.
///
/// The trailing newline is not part of a line, and a final line without one is
/// still a line.
pub fn lines() -> Lines {
    Lines {
        inner: std::io::stdin().lock().lines(),
    }
}

/// What [`lines`] hands back.
///
/// A named type rather than `impl Iterator`, because the ledger names types and
/// a caller's compiler looks this one up by name (ADR-020).
pub struct Lines {
    inner: std::io::Lines<std::io::StdinLock<'static>>,
}

impl Iterator for Lines {
    type Item = Result<String, std::io::Error>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

#[cfg(test)]
mod tests {
    use std::io::BufRead;

    /// The three cases a step has, on a reader that is not standard input -
    /// there is one stdin per process and a test may not consume it.
    ///
    /// This is the shape `Lines` wraps, and pinning it here is what says the
    /// wrapper adds nothing: a line, the end, and a failure that arrives *as a
    /// step* rather than as the end.
    #[test]
    fn a_step_is_a_line_an_end_or_a_failure() {
        let text: &[u8] = b"one\ntwo\n";
        let read: Vec<String> = text.lines().map(|l| l.expect("valid")).collect();
        assert_eq!(read, ["one", "two"], "the newline is not part of a line");

        // A last line without a newline is still a line.
        let text: &[u8] = b"one\ntwo";
        assert_eq!(text.lines().count(), 2);

        // And a byte that is not UTF-8 is a *failing step*, not an end: the
        // count is two, and the second one is the failure.
        let text: &[u8] = b"one\n\xFF\n";
        let steps: Vec<_> = text.lines().collect();
        assert_eq!(steps.len(), 2);
        assert!(steps[0].is_ok());
        assert!(
            steps[1].is_err(),
            "a failed read must not be indistinguishable from the end of the stream"
        );
    }
}
