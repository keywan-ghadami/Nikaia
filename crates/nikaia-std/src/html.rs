//! `std::html` - the escaping a template's contract rests on.
//!
//! [ADR-017](../../../docs/specification/adr/adr-017.md) makes escaping a
//! property of the template grammar rather than of the caller's discipline: a
//! hole is escaped, every hole, every time, and the one way to say "this is
//! already markup" is a type. Both halves need the same function, and it lives
//! here rather than inside the code generator so that there is one
//! implementation, testable on its own, and a hand-written renderer gets
//! exactly what a template gets.

use std::borrow::Cow;

/// The five characters that change what HTML *means* in text and in a quoted
/// attribute value.
///
/// `&` first, and that is not a style choice: escaping it after the others
/// would escape the ampersands the others just produced.
const ESCAPES: [(char, &str); 5] = [
    ('&', "&amp;"),
    ('<', "&lt;"),
    ('>', "&gt;"),
    ('"', "&quot;"),
    ('\'', "&#39;"),
];

/// `text`, safe to place in an HTML text node or a quoted attribute value.
///
/// Returns the input unchanged - and unallocated - when nothing needs
/// escaping, which is the common case for the column of a database table. The
/// scan is one pass either way.
///
/// It does **not** make text safe for every position: a `<script>` body, a CSS
/// block, an unquoted attribute and a URL each need something else, which is
/// why ADR-017 D3 makes a hole in those positions a compile error rather than a
/// call to this function.
pub fn escape(text: &str) -> Cow<'_, str> {
    let Some(first) = text.find(|c| ESCAPES.iter().any(|(from, _)| *from == c)) else {
        return Cow::Borrowed(text);
    };

    // Room for the prefix that needs nothing plus a little: a message with one
    // apostrophe is the ordinary case, not a message that is all apostrophes.
    let mut escaped = String::with_capacity(text.len() + 16);
    escaped.push_str(&text[..first]);
    for c in text[first..].chars() {
        match ESCAPES.iter().find(|(from, _)| *from == c) {
            Some((_, to)) => escaped.push_str(to),
            None => escaped.push(c),
        }
    }
    Cow::Owned(escaped)
}

#[cfg(test)]
mod tests {
    use super::{escape, ESCAPES};
    use std::borrow::Cow;

    #[test]
    fn every_character_that_changes_the_meaning_is_escaped() {
        for (from, to) in ESCAPES {
            assert_eq!(escape(&from.to_string()), to, "{from}");
        }
    }

    #[test]
    fn text_that_needs_nothing_is_returned_unchanged_and_unallocated() {
        let plain = "Additional fortune added at request time.";
        assert!(matches!(escape(plain), Cow::Borrowed(_)));
        assert_eq!(escape(plain), plain);
    }

    /// The order matters: escaping `&` last would escape the ampersands the
    /// other four just produced.
    #[test]
    fn the_ampersand_is_escaped_first() {
        assert_eq!(escape("<b>"), "&lt;b&gt;");
        assert_eq!(escape("&lt;"), "&amp;lt;");
    }

    #[test]
    fn what_a_template_is_for() {
        assert_eq!(
            escape(r#"<script>alert("x")</script>"#),
            "&lt;script&gt;alert(&quot;x&quot;)&lt;/script&gt;"
        );
    }

    /// Escaping is **not** idempotent, and a test says so rather than an
    /// invariant that is not true: escaping twice is a bug at the call site,
    /// which is exactly why ADR-017 D2 gives an already-escaped value a type
    /// instead of leaving it to be recognised.
    #[test]
    fn escaping_twice_is_visible_rather_than_harmless() {
        assert_eq!(escape(&escape("<b>")), "&amp;lt;b&amp;gt;");
    }
}
