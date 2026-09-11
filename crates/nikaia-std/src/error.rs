//! Kap 7.1: what an error carries besides its message.
//!
//! An error raised by `throw` arrives here on its way into the failure channel.
//! What this module adds is the part the author did not write and should not
//! have to: where it was raised, what joined it on the way, and — only where a
//! program asked for it — a stack trace.
//!
//! **Why the trace is not simply always there.** Capturing one costs about
//! 28 300 instructions per error - roughly sixteen times what the rest of the
//! program does - for a value almost nothing reads. So it is off unless
//! `NIKAIA_TRACE` asks for it. The measurement behind that number, and the
//! reasoning that made it the deciding one, are ADR-023 §4.2.
//!
//! **`NIKAIA_TRACE` is the only switch.** `Backtrace::capture()` consults
//! `RUST_BACKTRACE`/`RUST_LIB_BACKTRACE` and is `Disabled` unless one of them
//! is set, so a program told to trace would silently not have traced. This
//! module uses `force_capture` behind its own flag: one variable decides, and
//! it is the one the message names.

use std::backtrace::{Backtrace, BacktraceStatus};
use std::error::Error;
use std::fmt;
use std::sync::OnceLock;

/// Whether this process captures a trace when an error is raised.
///
/// Read once. A program that never asks pays this check once rather than per
/// error, and pays nothing for the capture itself.
fn tracing() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| match std::env::var("NIKAIA_TRACE") {
        Ok(v) => v != "0" && !v.is_empty(),
        Err(_) => false,
    })
}

/// An error on its way out of the function that raised it.
///
/// It is the author's error plus what the language attaches: the site, and a
/// trace where one was asked for. `Display` is the author's message and nothing
/// else, because that is what a `{error}` hole prints and what may be shown to
/// a stranger (Kap 7.1, ADR-018).
pub struct Raised {
    inner: Box<dyn Error>,
    /// `file:line` of the `throw`. Costs nothing at run time: the compiler knew
    /// it and wrote it into the binary as text.
    origin: &'static str,
    trace: Option<Backtrace>,
}

impl Raised {
    /// Everything, for an operator: the message, where it was raised, and the
    /// trace if this process captured one.
    ///
    /// This is what `error.full()` reaches. It is deliberately not what
    /// `{error}` prints - the short form is the one you get without thinking,
    /// and it is the one that is safe in front of a stranger.
    pub fn full(&self) -> String {
        let mut out = format!("{}\n  raised at {}", self.inner, self.origin);
        let mut source = self.inner.source();
        while let Some(cause) = source {
            out.push_str(&format!("\n  caused by {cause}"));
            source = cause.source();
        }
        match &self.trace {
            Some(t) if t.status() == BacktraceStatus::Captured => {
                out.push_str(&format!("\n{t}"));
            }
            _ => out.push_str("\n  (no trace; set NIKAIA_TRACE=1 to capture one)"),
        }
        out
    }

    /// Where the `throw` was, as the compiler wrote it.
    pub fn origin(&self) -> &'static str {
        self.origin
    }
}

impl fmt::Display for Raised {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(f)
    }
}

impl fmt::Debug for Raised {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} (raised at {})", self.inner, self.origin)
    }
}

impl Error for Raised {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.inner.source()
    }
}

/// What `throw` lowers to: put the value in the failure channel, with the site
/// it came from.
pub fn raise<E>(error: E, origin: &'static str) -> Box<dyn Error>
where
    E: Error + 'static,
{
    Box::new(Raised {
        inner: Box::new(error),
        origin,
        // `force_capture`, not `capture`: `capture` additionally requires
        // `RUST_BACKTRACE`, so `NIKAIA_TRACE=1` alone would capture nothing.
        trace: tracing().then(Backtrace::force_capture),
    })
}

/// `error.full()` on whatever a `catch` bound.
///
/// A `catch` binds the failure channel's type, which is a box. An error that
/// came through `raise` can say where it was raised; one that came from a
/// library below can only say what it is, and says so rather than pretending.
pub trait Full {
    fn full(&self) -> String;
}

impl Full for Box<dyn Error> {
    fn full(&self) -> String {
        match self.downcast_ref::<Raised>() {
            Some(raised) => raised.full(),
            None => format!("{self}\n  (raised below Nikaia; no site recorded)"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct Boom;
    impl fmt::Display for Boom {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("boom")
        }
    }
    impl Error for Boom {}

    #[test]
    fn the_short_form_is_the_message_and_nothing_else() {
        let e = raise(Boom, "conf.nika:12");
        assert_eq!(e.to_string(), "boom");
    }

    #[test]
    fn the_full_form_names_the_site() {
        let e = raise(Boom, "conf.nika:12");
        let full = e.full();
        assert!(full.contains("boom"), "{full}");
        assert!(full.contains("raised at conf.nika:12"), "{full}");
    }

    /// Without `NIKAIA_TRACE` there is no trace, and the full form says so
    /// rather than leaving a reader wondering whether it lost one.
    #[test]
    fn without_the_switch_the_absence_is_stated() {
        let e = raise(Boom, "conf.nika:12");
        assert!(e.full().contains("NIKAIA_TRACE=1"), "{}", e.full());
    }

    /// An error from below Nikaia has no site, and says that instead of
    /// inventing one.
    #[test]
    fn an_error_from_below_says_it_has_no_site() {
        let e: Box<dyn Error> = Box::new(Boom);
        assert!(e.full().contains("no site recorded"), "{}", e.full());
    }
}
