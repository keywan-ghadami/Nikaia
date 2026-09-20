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
//! `par_iter` body.
//!
//! **Whole-of-input reads suspend; a line at a time does not yet**
//! ([ADR-121](../../../docs/specification/adr/adr-121.md) D4).
//!
//! Since [ADR-055](../../../docs/specification/adr/adr-055.md) §6 step 3 these
//! signatures have said a read from here may pause, which is what a caller's
//! compiler reads and what makes the enclosing function `async`. The bodies did
//! not: the blocking read ran on the program's own thread, because a worker's
//! reply could not wake an executor parked on the ring and a future fed from one
//! had nowhere to come back to.
//!
//! ADR-121 D1 put the bell on the ring, so [`read`] and [`read_to_string`] are
//! now the read they always were, performed on an I/O worker and **awaited**.
//! There is still no second read shape and no ring path for a stream with no
//! size to `stat`, which is D4's own sentence: what changed is the park.
//!
//! [`lines`] is the one that has not moved, and the reason is the language and
//! not the runtime. A step of it is an `Iterator::next`, and a suspension point
//! inside one would have to be `while let Some(x) = s.next().await` in the
//! language below - a `Stream` trait Rust has not stabilised, and a `for` over a
//! stream this language has not decided. `docs/open-work.md` carries that as the
//! half it always said was the larger one.

use std::io::BufRead;

/// What a `std` call fails with ([ADR-158](../../../docs/specification/adr/adr-158.md) D1).
///
/// Part I 7.1's error type, for the library: an `enum` with payload, `impl
/// Error`, and variants a `catch` can tell apart. Until this existed every
/// `std` entry wrote `throws = ["?"]` — *something this compiler cannot name* —
/// which is the absence of an answer standing in for one, and it made
/// [ADR-023](../../../docs/specification/adr/adr-023.md) D1's set unusable for
/// every program that reads a file.
///
/// **It lives in `io` and is written out.** `use std::io` and
/// `io::IoError::NotFound(…)` is what a program that matches on it writes, which
/// is [ADR-154](../../../docs/specification/adr/adr-154.md) D3's rule applied
/// straight: what needs no prefix is Part I 1.3's list, and this is not on it.
/// `fs::read` throwing an `io` type is the cost, accepted: a program that only
/// *propagates* the failure names nothing, and one that takes it apart writes
/// the second `use`.
///
/// **Why these four.** D4 of [ADR-023](../../../docs/specification/adr/adr-023.md)
/// makes the variants of one error type **closed**, so each is a lasting
/// commitment and the list is short on purpose:
///
/// * `NotFound` is the specification's own — Appendix A.1 calls *a missing
///   file* the example of a recoverable error, and D6's worked output writes
///   `IoError::NotFound`;
/// * `PermissionDenied` is the other environmental failure a program acts on
///   **differently** rather than reports;
/// * `NotText` is `std`'s own rather than the operating system's: three places
///   here check UTF-8 and report `InvalidData`, and Part III 17.1 specifies the
///   check;
/// * `Other` is D4's **named residue** — *exhaustiveness degrades to "and
///   anything else"* — carrying what the operating system said.
///
/// **The payload is owned text and not a view.**
/// [ADR-023](../../../docs/specification/adr/adr-023.md) D10 says a path that
/// came from a buffer should travel as a view; that is the tether
/// ([ADR-008](../../../docs/specification/adr/adr-008.md)), which is not built,
/// and a lifetime on this type would reach every `std` signature. One
/// allocation on a path that is already failing is the honest price until then.
/// What an [`IoError`] from this module was about: a stream has no path, and
/// the name is what a message can say instead.
const STDIN: &str = "standard input";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IoError {
    /// It is not there. The payload is what was looked for.
    NotFound(String),
    /// It is there and this program may not have it.
    PermissionDenied(String),
    /// The bytes are not text, and text in this language is UTF-8.
    NotText(String),
    /// Everything else, as the operating system said it.
    Other(String),
}

impl IoError {
    /// One of the language below's failures, told apart and given what it was
    /// about.
    ///
    /// `what` is the path, or the name of the stream: an operating system's
    /// error does not carry it, and *not found* without the thing that was not
    /// found is the message D3 calls a round trip to the user.
    pub fn of(error: std::io::Error, what: &str) -> IoError {
        match error.kind() {
            std::io::ErrorKind::NotFound => IoError::NotFound(what.to_string()),
            std::io::ErrorKind::PermissionDenied => IoError::PermissionDenied(what.to_string()),
            std::io::ErrorKind::InvalidData => IoError::NotText(what.to_string()),
            _ => IoError::Other(format!("{what}: {error}")),
        }
    }

    /// What was looked for, where the variant names one.
    pub fn what(&self) -> &str {
        match self {
            IoError::NotFound(what)
            | IoError::PermissionDenied(what)
            | IoError::NotText(what)
            | IoError::Other(what) => what,
        }
    }
}

impl std::fmt::Display for IoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IoError::NotFound(what) => write!(f, "no such file or directory: {what}"),
            IoError::PermissionDenied(what) => write!(f, "permission denied: {what}"),
            IoError::NotText(what) => write!(f, "not valid UTF-8: {what}"),
            IoError::Other(what) => f.write_str(what),
        }
    }
}

impl std::error::Error for IoError {}

/// All of standard input, as text.
///
/// UTF-8 validated, for the reason `fs::read_to_string` validates: a parser
/// handed bytes that are not text would find that out one view at a time, and a
/// frame boundary is a string.
///
/// Reading it a second time yields what the operating system says, which is
/// nothing.
pub async fn read_to_string() -> Result<String, IoError> {
    let bytes = crate::rt::io::stdin_whole()
        .await
        .map_err(|e| IoError::of(e, STDIN))?;
    // The same failure `std`'s own `read_to_string` reports, under the name
    // this library gives it: a stream that is not text is `NotText` and not a
    // lossy string.
    String::from_utf8(bytes).map_err(|_| IoError::NotText(STDIN.to_string()))
}

/// All of standard input, as bytes.
///
/// The half for input that is not text - and the one that says what
/// `read_to_string` is doing, since the only difference is the check.
pub async fn read() -> Result<Vec<u8>, IoError> {
    crate::rt::io::stdin_whole()
        .await
        .map_err(|e| IoError::of(e, STDIN))
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
///
/// **This one does not suspend**, and the module comment says why: a step is an
/// `Iterator::next`, and a suspension point inside one needs a `for` over a
/// stream that neither this language nor the one below has decided.
/// [`read_to_string`] is the entry that does, for an input a program is willing
/// to hold whole.
pub async fn lines() -> Lines {
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
    /// **`IoError` and not the language below's own**
    /// ([ADR-158](../../../docs/specification/adr/adr-158.md) D1): a step of
    /// this fails, so the function around the `for` says `throws` — and what it
    /// throws is what `std` says it throws, here as everywhere else. This was
    /// the one surface that record missed, and it showed the day a program
    /// whose whole set was `io::IoError` got a **named** channel
    /// ([ADR-159](../../../docs/specification/adr/adr-159.md) D1): the `?` the
    /// loop's step takes had a raw `std::io::Error` on the left of it and a
    /// named one on the right.
    type Item = Result<String, IoError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner
            .next()
            .map(|line| line.map_err(|e| IoError::of(e, STDIN)))
    }
}

#[cfg(test)]
mod tests {
    use std::io::BufRead;

    /// **Standard input is read on a worker, and the read is awaited**
    /// ([ADR-121](../../../docs/specification/adr/adr-121.md) D4).
    ///
    /// It has to be a *process*. There is one standard input per process, a test
    /// may not consume it, and what is being asserted is that the executor
    /// **parks** for the read and is woken by the worker's reply - which needs a
    /// program whose standard input is a pipe somebody else is writing to.
    ///
    /// **The child bounds itself**, which is D3's *a hang is the failure this
    /// may not have*. `exec::block_on`'s park is unbounded while `main` is
    /// running, so a bell that is not heard would be a child that never exits
    /// and a test that never finishes; the watchdog turns that into a status the
    /// parent reads and a sentence that says what happened.
    #[test]
    fn standard_input_is_read_on_a_worker_and_awaited() {
        use std::io::Write;

        const NAME: &str = "standard_input_is_read_on_a_worker_and_awaited";
        const CHILD: &str = "NIKAIA_STDIN_CHILD";
        /// What the watchdog exits with. Not 70 and not 101, so that a hang is
        /// told apart from an expired cleanup and from a panic.
        const DEAF: i32 = 99;

        if std::env::var_os(CHILD).is_some() {
            std::thread::spawn(|| {
                std::thread::sleep(std::time::Duration::from_secs(10));
                eprintln!(
                    "the child never got its input: the executor parked for a worker's \
                     reply and was not woken (ADR-121 D1)"
                );
                std::process::exit(DEAF);
            });
            let text = crate::rt::exec::block_on(super::read_to_string()).expect("the input");
            print!("READ[{text}]");
            std::io::stdout().flush().expect("flushed");
            return;
        }

        let mut child = std::process::Command::new(std::env::current_exe().expect("this binary"))
            .args([NAME, "--nocapture"])
            .env(CHILD, "1")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("the child started");

        // Written after a pause, so the child reaches its park with nothing to
        // read - which is the whole of what this is about. The pipe is then
        // closed, because a whole-of-input read ends at the end of the input.
        let mut input = child.stdin.take().expect("the child's standard input");
        let writing = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(150));
            input.write_all(b"eins\nzwei\n").expect("the parent writes");
        });

        let ran = child.wait_with_output().expect("the child ran");
        let _ = writing.join();
        let said = String::from_utf8_lossy(&ran.stderr).into_owned();
        let out = String::from_utf8_lossy(&ran.stdout).into_owned();
        assert_ne!(
            ran.status.code(),
            Some(DEAF),
            "the read never came back.\nstderr:\n{said}"
        );
        assert!(
            out.contains("READ[eins\nzwei\n]"),
            "the whole of the input came back through the worker.\nstdout:\n{out}\nstderr:\n{said}"
        );
    }

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
