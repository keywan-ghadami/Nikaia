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
    /// **The failures that joined this one**
    /// ([ADR-115](../../../docs/specification/adr/adr-115.md) D1), in the order
    /// they joined.
    ///
    /// Empty until something joins, which costs nothing on the path where
    /// nothing fails. What fills it is an `overlap` whose later branches failed
    /// too (D2): the block waits for every branch, so when it ends every
    /// outcome is known and the list is a **fact** rather than a race.
    secondary: Vec<Box<dyn Error>>,
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
        for later in &self.secondary {
            out.push_str(&indented(&later.full()));
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

/// The same for the boxed channel, and for the same reason.
impl fmt::Debug for Raised {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.full())
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
        secondary: Vec::new(),
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
    /// The failures that joined this one
    /// ([ADR-115](../../../docs/specification/adr/adr-115.md) D1).
    ///
    /// **Of this channel's own type**, which is what makes the list possible at
    /// all: a joining block hands every branch the same channel
    /// ([ADR-164](../../../docs/specification/adr/adr-164.md) D2), so the
    /// failures that meet here are the same kind of thing as the one they meet.
    /// Each keeps its own envelope, so each keeps the site
    /// [ADR-023](../../../docs/specification/adr/adr-023.md) D6 gives it — and
    /// a secondary with secondaries of its own is D3's tree.
    secondary: Vec<Thrown<E>>,
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
    pub fn split(self) -> (E, Site<E>) {
        (
            self.inner,
            Site {
                origin: self.origin,
                trace: self.trace,
                secondary: self.secondary,
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
    ///
    /// **With what joined it, indented under it**
    /// ([ADR-115](../../../docs/specification/adr/adr-115.md) D1). An outage
    /// that took two of three loads down reads as two failures, the second
    /// under the first, rather than as one with the rest gone.
    pub fn full(&self) -> String {
        self.long(true)
    }

    /// The long form, told whether to say anything about the trace.
    ///
    /// **A secondary says nothing about it**: the note is about how this
    /// *process* was started, so repeating it under every joined failure says
    /// the same thing three times and buries the failures.
    fn long(&self, note_trace: bool) -> String {
        let mut out = full_form_with(&self.inner, self.origin, self.trace.as_ref(), note_trace);
        for later in &self.secondary {
            out.push_str(&indented(&later.long(false)));
        }
        out
    }
}

/// One joined failure, under the one it joined
/// ([ADR-115](../../../docs/specification/adr/adr-115.md) D1).
///
/// Every line of it moves right, not only the first, so a secondary that has
/// secondaries of its own reads as the tree D3 makes it: the depth on the page
/// is the depth in the list.
fn indented(full: &str) -> String {
    let mut out = String::from("\n  and then:");
    for line in full.lines() {
        out.push_str("\n  ");
        out.push_str(line);
    }
    out
}

/// What the language put around a thrown error: where it was raised, and the
/// trace if this process captured one
/// ([ADR-157](../../../docs/specification/adr/adr-157.md) D2).
///
/// It exists because a handler is handed the **error** and still has to be able
/// to answer both of the questions the envelope answers. Carrying it beside the
/// error is what lets `match error { … }` be the plain match the source wrote.
pub struct Site<E> {
    origin: &'static str,
    trace: Option<Backtrace>,
    /// What joined the error this site belongs to
    /// ([ADR-115](../../../docs/specification/adr/adr-115.md) D1), kept beside
    /// the error for the same reason the site is: a handler is handed the
    /// **error**, and both of the things the envelope holds have to stay
    /// reachable from where it was opened.
    secondary: Vec<Thrown<E>>,
}

impl<E> Site<E> {
    /// `error.full()` in a handler a named channel reached: the message, the
    /// site, and the trace if there is one.
    ///
    /// The same string [`Thrown::full`] builds, from the two halves the handler
    /// holds rather than from one value.
    pub fn full_of(&self, error: &E) -> String
    where
        E: fmt::Display,
    {
        full_form(error, self.origin, self.trace.as_ref())
    }

    /// `throw error`: put the error back in the channel, in the envelope it
    /// arrived in (D3).
    ///
    /// The **original** site and the original trace. A handler that passed an
    /// error on is not where it was raised, and re-capturing here would make it
    /// look like it was ([ADR-023](../../../docs/specification/adr/adr-023.md)
    /// D6).
    pub fn refill(self, error: E) -> Thrown<E> {
        Thrown {
            inner: error,
            origin: self.origin,
            trace: self.trace,
            // **And what joined it travels on with it.** A handler that passes
            // an error along passes what came with it; dropping the list here
            // would make `throw error` the one place a failure quietly loses
            // the others ([ADR-115](../../../docs/specification/adr/adr-115.md) D1).
            secondary: self.secondary,
        }
    }

    /// The failures that joined this one, for a handler that reads them
    /// ([ADR-115](../../../docs/specification/adr/adr-115.md) D1).
    pub fn secondary(&self) -> &[Thrown<E>] {
        &self.secondary
    }
}

/// The long form, from its three parts.
fn full_form<E: fmt::Display>(
    error: &E,
    origin: &'static str,
    trace: Option<&Backtrace>,
) -> String {
    full_form_with(error, origin, trace, true)
}

/// The same, told whether to say anything about the trace
/// ([ADR-115](../../../docs/specification/adr/adr-115.md) D1).
fn full_form_with<E: fmt::Display>(
    error: &E,
    origin: &'static str,
    trace: Option<&Backtrace>,
    note_trace: bool,
) -> String {
    // **An envelope with no site says so in the sentence that already existed**
    // ([ADR-159](../../../docs/specification/adr/adr-159.md) D3). Since
    // [ADR-115](../../../docs/specification/adr/adr-115.md) D1 a library's
    // error may be enveloped for the sake of the **list**, and *raised at
    // (below Nikaia)* would be this compiler inventing a place.
    let mut out = match origin == BELOW_SITE {
        true => format!("{error}\n{BELOW}"),
        false => format!("{error}\n  raised at {origin}"),
    };
    match trace {
        Some(t) if t.status() == BacktraceStatus::Captured => {
            out.push_str(&format!("\n{t}"));
        }
        _ if note_trace => out.push_str("\n  (no trace; set NIKAIA_TRACE=1 to capture one)"),
        _ => {}
    }
    out
}

impl<E: fmt::Display> fmt::Display for Thrown<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(f)
    }
}

/// **What an uncaught failure prints**, which is where the list is read most
/// ([ADR-115](../../../docs/specification/adr/adr-115.md) D1).
///
/// A `main` that hands back an `Err` is printed by the language below through
/// `Debug`, so this is the operator's view of a program that stopped — and a
/// short form there would be the one place the failures that joined are
/// dropped on the floor. It is the long form, with them under it.
impl<E: fmt::Display> fmt::Debug for Thrown<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.full())
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
        secondary: Vec::new(),
    }
}

/// **A library's error, put in an envelope** — [ADR-115](../../../docs/specification/adr/adr-115.md)
/// D1 read against [ADR-159](../../../docs/specification/adr/adr-159.md) D2.
///
/// That record said a library's error travels **bare**, because there is no
/// `throw` in this program to have a site. That holds for the site and not for
/// the list: an `overlap` that combines failures is the language doing
/// something, so there is something to attach even where nothing was raised
/// here. Where a function's body joins, its channel is the envelope and this is
/// what a `?` from a callee with a bare one converts through.
///
/// The site stays absent, which [`Full`] already has words for.
impl<E> From<E> for Thrown<E> {
    fn from(error: E) -> Thrown<E> {
        Thrown {
            inner: error,
            origin: BELOW_SITE,
            trace: None,
            secondary: Vec::new(),
        }
    }
}

/// What the origin says for an error no `throw` of this program raised.
const BELOW_SITE: &str = "(below Nikaia)";

/// **What a joining block does with the failures after the first**
/// ([ADR-115](../../../docs/specification/adr/adr-115.md) D2).
///
/// The first failure in written order is the block's
/// ([ADR-050](../../../docs/specification/adr/adr-050.md) D5) and every later
/// one joins its list, in written order. One trait so that
/// [`crate::task::combine2`] and its arities need to know only that the channel
/// can take one, whichever of the envelopes it is.
pub trait Joined {
    /// Put `later` in this error's list, after whatever is already there.
    fn joined_by(&mut self, later: Self);
}

impl<E> Joined for Thrown<E> {
    fn joined_by(&mut self, later: Thrown<E>) {
        self.secondary.push(later);
    }
}

/// **The boxed channel joins through a downcast**, which is the price of the
/// box rather than a shortcut: what is inside it is a [`Raised`] wherever this
/// program raised it, and an error from below Nikaia has no envelope to hold a
/// list. A failure that joins one of those is dropped, and that is the one case
/// the list cannot cover — named here rather than discovered.
impl Joined for Box<dyn Error> {
    fn joined_by(&mut self, later: Box<dyn Error>) {
        if let Some(raised) = self.downcast_mut::<Raised>() {
            raised.secondary.push(later);
        }
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
            None => format!("{self}\n{BELOW}"),
        }
    }
}

/// What the long form says about a failure **no `throw` in this program
/// raised** ([ADR-159](../../../docs/specification/adr/adr-159.md) D3).
const BELOW: &str = "  (raised below Nikaia; no site recorded)";

/// The long form for a **library's** error type, where one is the channel (D2).
///
/// It has no envelope, because there is no `throw` in this program to have a
/// site — so `full()` says so, in the words the boxed channel has always used
/// for an error that came from below. What it carries instead is **what** the
/// failure was about, which is in the message.
///
/// **Named types and not a blanket impl**: `Box<dyn Error>` does not implement
/// `Error`, so the language below cannot be told that an `impl<E: Error>` would
/// not overlap the one above. One line per library error type that can be a
/// channel, and there are two.
impl Full for crate::io::IoError {
    fn full(&self) -> String {
        format!("{self}\n{BELOW}")
    }
}

impl Full for crate::lock::Overtaken {
    fn full(&self) -> String {
        format!("{self}\n{BELOW}")
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

    /// **A failure that joined is under the one it joined**
    /// ([ADR-115](../../../docs/specification/adr/adr-115.md) D1, D2), in the
    /// order they joined.
    #[test]
    fn what_joined_is_printed_under_it() {
        let mut first = throwing(Boom, "load.nika:3");
        first.joined_by(throwing(Boom, "load.nika:9"));
        let full = first.full();

        assert_eq!(full.matches("boom").count(), 2, "{full}");
        assert!(full.contains("raised at load.nika:3"), "{full}");
        assert!(full.contains("raised at load.nika:9"), "{full}");
        assert!(
            full.find("load.nika:3") < full.find("load.nika:9"),
            "the one that was joined comes first:\n{full}"
        );
        assert!(full.contains("and then:"), "{full}");
    }

    /// **And the trace note is said once**, because it is about how this
    /// *process* was started: repeating it under every joined failure says the
    /// same thing three times and buries them.
    #[test]
    fn the_trace_note_is_not_repeated_under_each() {
        let mut first = throwing(Boom, "load.nika:3");
        first.joined_by(throwing(Boom, "load.nika:9"));
        assert_eq!(first.full().matches("NIKAIA_TRACE").count(), 1);
    }

    /// **An envelope with no site says so**, which is the sentence
    /// [ADR-159](../../../docs/specification/adr/adr-159.md) D3 already had.
    /// Since D1 a library's error may be enveloped for the sake of the list,
    /// and *raised at (below Nikaia)* would be this compiler inventing a place.
    #[test]
    fn an_enveloped_error_from_below_still_has_no_site() {
        let below: Thrown<Boom> = Boom.into();
        let full = below.full();
        assert!(full.contains("no site recorded"), "{full}");
        assert!(!full.contains("raised at"), "{full}");
    }

    /// **The list survives `throw error`** (D1): a handler that passes an error
    /// along passes what came with it, or that would be the one place a
    /// failure quietly loses the others.
    #[test]
    fn passing_an_error_on_keeps_what_joined_it() {
        let mut first = throwing(Boom, "load.nika:3");
        first.joined_by(throwing(Boom, "load.nika:9"));
        let (error, site) = first.split();
        assert_eq!(site.secondary().len(), 1);
        assert!(site.refill(error).full().contains("load.nika:9"));
    }

    /// An error from below Nikaia has no site, and says that instead of
    /// inventing one.
    #[test]
    fn an_error_from_below_says_it_has_no_site() {
        let e: Box<dyn Error> = Box::new(Boom);
        assert!(e.full().contains("no site recorded"), "{}", e.full());
    }
}
