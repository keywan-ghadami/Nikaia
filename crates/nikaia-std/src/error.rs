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
//! reasoning that made it the deciding one, are ADR-023 §3.2.
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

/// The same envelope, over an error type the compiler could **name**
/// ([ADR-157](../../../docs/specification/adr/adr-157.md) D1).
///
/// Where a function's inferred error set is one type this program declares, the
/// failure channel is that type rather than a box, and this is what travels in
/// it: the author's value, plus the site [ADR-023](../../../docs/specification/adr/adr-023.md)
/// D6 says an error knows.
///
/// **A type of its own rather than a generic [`Raised`]**, and the reason is
/// the language below: `Box<dyn Error>` does not implement `Error`, so one type
/// cannot be bounded to cover both the box and a named error without the two
/// impls overlapping. Two envelopes, each honest about what it holds, is the
/// shape that compiles — and the box keeps its own `full()` with the cause
/// chain, which the named case does not need because an author's `enum` has no
/// cause below it.
pub struct Thrown<E> {
    inner: E,
    origin: &'static str,
    trace: Option<Backtrace>,
}

impl<E> Thrown<E> {
    /// The error the program actually threw, and everything the language put
    /// around it, as two values.
    ///
    /// What a `catch` binds is the error: `match error { ConfigError::NotFound(p)
    /// => … }` is about the author's `enum` and not about the envelope (D2). But
    /// the envelope is not thrown away — `error.full()` needs it and so does
    /// `throw error` (D3) — so it comes back beside the error rather than
    /// around it, and the handler holds both.
    pub fn split(self) -> (E, Site) {
        (
            self.inner,
            Site {
                origin: self.origin,
                trace: self.trace,
            },
        )
    }

    /// Where the `throw` was, as the compiler wrote it.
    pub fn origin(&self) -> &'static str {
        self.origin
    }
}

impl<E: fmt::Display> Thrown<E> {
    /// Everything, for an operator — [`Raised::full`]'s answer for a named
    /// error.
    pub fn full(&self) -> String {
        full_form(&self.inner, self.origin, self.trace.as_ref())
    }
}

/// What the language put around a thrown error: where it was raised, and the
/// trace if this process captured one
/// ([ADR-157](../../../docs/specification/adr/adr-157.md) D2).
///
/// It exists because a handler is handed the **error** and still has to be able
/// to answer both of the questions the envelope answers. Carrying it beside the
/// error is what lets `match error { … }` be the plain match the source wrote.
pub struct Site {
    origin: &'static str,
    trace: Option<Backtrace>,
}

impl Site {
    /// `error.full()` in a handler a named channel reached: the message, the
    /// site, and the trace if there is one.
    ///
    /// The same string [`Thrown::full`] builds, from the two halves the handler
    /// holds rather than from one value.
    pub fn full_of<E: fmt::Display>(&self, error: &E) -> String {
        full_form(error, self.origin, self.trace.as_ref())
    }

    /// `throw error`: put the error back in the channel, in the envelope it
    /// arrived in (D3).
    ///
    /// The **original** site and the original trace. A handler that passed an
    /// error on is not where it was raised, and re-capturing here would make it
    /// look like it was ([ADR-023](../../../docs/specification/adr/adr-023.md)
    /// D6).
    pub fn refill<E>(self, error: E) -> Thrown<E> {
        Thrown {
            inner: error,
            origin: self.origin,
            trace: self.trace,
        }
    }
}

/// The long form, from its three parts.
fn full_form<E: fmt::Display>(
    error: &E,
    origin: &'static str,
    trace: Option<&Backtrace>,
) -> String {
    let mut out = format!("{error}\n  raised at {origin}");
    match trace {
        Some(t) if t.status() == BacktraceStatus::Captured => {
            out.push_str(&format!("\n{t}"));
        }
        _ => out.push_str("\n  (no trace; set NIKAIA_TRACE=1 to capture one)"),
    }
    out
}

impl<E: fmt::Display> fmt::Display for Thrown<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(f)
    }
}

impl<E: fmt::Display> fmt::Debug for Thrown<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} (raised at {})", self.inner, self.origin)
    }
}

impl<E: Error> Error for Thrown<E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.inner.source()
    }
}

/// What `throw` lowers to where the channel is **the error type** (D1).
///
/// No `Box` and no `'static`, which is the half a typed channel buys: an error
/// carrying a view of the caller's buffer — `ConfigError::NotFound(path)`,
/// Part I 7.1's own example — has a lifetime, and boxing it into a
/// `Box<dyn Error>` asks it to outlive the program (`E0521`).
pub fn throwing<E>(error: E, origin: &'static str) -> Thrown<E> {
    Thrown {
        inner: error,
        origin,
        trace: tracing().then(Backtrace::force_capture),
    }
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
