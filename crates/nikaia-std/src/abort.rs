//! Every abort points at the Nikaia line.
//!
//! [ADR-044](../../../docs/specification/adr/adr-044.md).
//! [ADR-012](../../../docs/specification/adr/adr-012.md) decides that a
//! diagnostic names the `.nika` file the user wrote, and the compiler keeps that
//! promise everywhere it *reports* something. An abort at run time was the one
//! path where it could not: there is no compiler left to translate anything, so
//! Rust's own message named the generated file and line.
//!
//! **One table and one hook** (D1, D2). The emitter knows both ends of every line
//! it writes and keeps that knowledge in the program rather than throwing it
//! away; the hook runs on every abort path, so the translation goes in one place
//! and reaches an overflow, an index, a division by zero and a written `panic()`
//! at once. A per-case fix would be four fixes and a fifth the day a fifth abort
//! path arrives.
//!
//! **The hook stays pure** ([ADR-006](../../../docs/specification/adr/adr-006.md)
//! D6 requires it to be `sync`): a binary search in a slice reads no file and
//! takes no lock.

/// One row: a line of the generated file, and the `.nika` file and line it came
/// from.
///
/// Sorted by the generated line, because the lookup is a binary search and the
/// emitter can sort once at compile time instead of the program sorting at every
/// start.
pub type Site = (u32, &'static str, u32);

/// Install the hook that translates an abort's location.
///
/// **What it does not do is the important half.** A location the table does not
/// know is handed to the hook that was installed before this one - which is
/// Rust's own - so a program is never worse off than it was. The table covers
/// the lines the emitter wrote *from* a Nikaia line; a panic inside `std`'s own
/// Rust, or inside a foreign crate, has no Nikaia line to name and should say so
/// in the words of whoever wrote it.
pub fn report_in_nikaia_terms(table: &'static [Site]) {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let site = info.location().and_then(|at| lookup(table, at.line()));
        match site {
            Some((file, line)) => {
                eprintln!("{file}:{line}: the program stopped: {}", said(info));
            }
            None => previous(info),
        }
    }));
}

/// The `.nika` file and line for a line of the generated file.
fn lookup(table: &[Site], generated: u32) -> Option<(&'static str, u32)> {
    table
        .binary_search_by_key(&generated, |(line, _, _)| *line)
        .ok()
        .map(|at| (table[at].1, table[at].2))
}

/// What the panic said, in the two shapes a payload comes in.
///
/// `panic!("…")` with no arguments is a `&str`; one with arguments is a `String`.
/// Anything else is something no Nikaia program can produce, and "it stopped" is
/// then the honest whole of what is known.
fn said(info: &std::panic::PanicHookInfo<'_>) -> String {
    let payload = info.payload();
    if let Some(text) = payload.downcast_ref::<&str>() {
        return (*text).to_string();
    }
    if let Some(text) = payload.downcast_ref::<String>() {
        return text.clone();
    }
    "aborted".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    const TABLE: &[Site] = &[
        (10, "src/main.nika", 3),
        (24, "src/parse.nika", 41),
        (99, "src/main.nika", 7),
    ];

    #[test]
    fn a_known_line_is_translated() {
        assert_eq!(lookup(TABLE, 24), Some(("src/parse.nika", 41)));
        assert_eq!(lookup(TABLE, 10), Some(("src/main.nika", 3)));
    }

    /// The half that keeps a program from being worse off: a line the table does
    /// not know is not answered wrongly.
    #[test]
    fn an_unknown_line_is_not_guessed_at() {
        assert_eq!(lookup(TABLE, 11), None);
        assert_eq!(lookup(TABLE, 1), None);
        assert_eq!(lookup(TABLE, 1000), None);
        assert_eq!(lookup(&[], 10), None);
    }
}
