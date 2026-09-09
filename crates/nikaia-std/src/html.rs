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
use std::fmt;

/// Text that is already markup, and may be placed in a template without being
/// escaped.
///
/// [ADR-017](../../../docs/specification/adr/adr-017.md) D2: the one way to say
/// "this is already markup" is a **type**, not a flag at the hole. A flag is a
/// property of the call site and the same value reaches many call sites; a type
/// travels with the value and is decided where the value is built.
///
/// `new` is the only constructor, so "every place this program declared
/// something already-escaped" is one grep rather than a type search. There is
/// deliberately no `From<String>`, no `Deref<Target = str>` that would let it
/// pass as text by accident, and no way back out that hides where it came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Raw(String);

impl Raw {
    /// Promise that `markup` is already HTML.
    ///
    /// The one line to grep for, and the one to review: everything downstream
    /// of it trusts this call.
    pub fn new(markup: impl Into<String>) -> Self {
        Raw(markup.into())
    }

    /// The markup, for whoever writes it out.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// `text`, escaped - the safe way to build markup out of something that is
    /// not markup yet.
    pub fn escaped(text: &str) -> Self {
        Raw(escape(text).into_owned())
    }
}

impl fmt::Display for Raw {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

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

#[cfg(test)]
mod raw_tests {
    use super::*;

    /// The type is the promise, and it is the only way to make it.
    #[test]
    fn raw_is_written_out_unchanged() {
        let markup = Raw::new("<b>bold</b>");
        assert_eq!(markup.as_str(), "<b>bold</b>");
        assert_eq!(markup.to_string(), "<b>bold</b>");
    }

    /// The safe way in: escape first, and the result is markup by construction.
    #[test]
    fn escaped_text_becomes_markup() {
        let markup = Raw::escaped("a < b & c");
        assert_eq!(markup.as_str(), "a &lt; b &amp; c");

        // …and it is not escaped twice, because it is no longer text.
        assert_eq!(Raw::new(markup.as_str()).as_str(), "a &lt; b &amp; c");
    }
}

/// What may be placed in a template hole, and how it is written.
///
/// [ADR-017](../../../docs/specification/adr/adr-017.md) D2 says the one way to
/// say "this is already markup" is a **type**. This is that decision expressed
/// where the compiler can act on it: a [`Raw`] renders itself, text renders
/// escaped, and a type with no impl cannot be put in a template at all.
///
/// The template compiler emits a call to this for every hole, unconditionally.
/// There is no flag at the hole and no way to ask for the other behaviour -
/// choosing is what the type does.
pub trait Render {
    /// The markup this value becomes.
    fn render(&self) -> Cow<'_, str>;
}

impl Render for Raw {
    /// Already markup, by the promise `Raw::new` made.
    fn render(&self) -> Cow<'_, str> {
        Cow::Borrowed(self.as_str())
    }
}

impl Render for str {
    fn render(&self) -> Cow<'_, str> {
        escape(self)
    }
}

impl Render for String {
    fn render(&self) -> Cow<'_, str> {
        escape(self)
    }
}

impl<T: Render + ?Sized> Render for &T {
    fn render(&self) -> Cow<'_, str> {
        (**self).render()
    }
}

/// Numbers and `bool` write themselves: their `Display` cannot produce a
/// character that changes what HTML means, so escaping them would be a scan
/// over text that can never contain anything to escape.
macro_rules! render_by_display {
    ($($t:ty),*) => {$(
        impl Render for $t {
            fn render(&self) -> Cow<'_, str> {
                Cow::Owned(self.to_string())
            }
        }
    )*};
}

render_by_display!(bool, i8, i16, i32, i64, i128, isize, u8, u16, u32, u64, u128, usize, f32, f64);

#[cfg(test)]
mod render_tests {
    use super::*;

    /// The type decides, which is the whole of ADR-017 D2.
    #[test]
    fn text_is_escaped_and_markup_is_not() {
        assert_eq!("a<b".render(), "a&lt;b");
        assert_eq!(String::from("a<b").render(), "a&lt;b");
        assert_eq!(Raw::new("<b>x</b>").render(), "<b>x</b>");
    }

    /// A reference renders as what it points at, so a hole may hold a borrowed
    /// value without the template knowing.
    #[test]
    fn a_reference_renders_as_its_target() {
        let owned = String::from("a&b");
        let view: &str = &owned;
        assert_eq!(view.render(), "a&amp;b");
        let markup = Raw::new("<i>");
        let borrowed: &Raw = &markup;
        assert_eq!(borrowed.render(), "<i>");
    }

    /// A number cannot contain markup, so it is written as it is.
    #[test]
    fn a_number_writes_itself() {
        assert_eq!(42i32.render(), "42");
        assert_eq!(true.render(), "true");
        assert_eq!((-1.5f64).render(), "-1.5");
    }
}
