//! **Text that is a view or text of its own, per value**
//! ([ADR-222](../../../docs/specification/adr/adr-222.md) D3).
//!
//! What a `String` field or result is below when both kinds of text flow into
//! it: a view is borrowed where it is put in, and text of its own is owned
//! where it is put in, so neither line pays for the other's kind. Read, it is
//! text either way - it derefs to `str`, prints, compares and hashes as its
//! text - and `to_owned` hands back a `String`, which is what a program's
//! `.clone()` of text asks for.

use std::borrow::Cow;

/// A view of text, or text of its own.
#[derive(Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EitherText<'a>(Cow<'a, str>);

impl<'a> EitherText<'a> {
    /// The text, as a view of it.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// A copy: text of its own, whichever this is.
    #[allow(
        clippy::inherent_to_string_shadow_display,
        clippy::wrong_self_convention
    )]
    pub fn to_owned(&self) -> String {
        String::from(&*self.0)
    }

    /// The same, under the name a reader from another language types.
    #[allow(clippy::inherent_to_string_shadow_display)]
    pub fn to_string(&self) -> String {
        self.to_owned()
    }

    /// Whether this is a view - for `--tethers` and for tests.
    pub fn is_view(&self) -> bool {
        matches!(self.0, Cow::Borrowed(_))
    }
}

impl std::ops::Deref for EitherText<'_> {
    type Target = str;
    fn deref(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for EitherText<'_> {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::borrow::Borrow<str> for EitherText<'_> {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl<'a> From<&'a str> for EitherText<'a> {
    fn from(view: &'a str) -> Self {
        EitherText(Cow::Borrowed(view))
    }
}

impl<'a> From<&'a String> for EitherText<'a> {
    fn from(view: &'a String) -> Self {
        EitherText(Cow::Borrowed(view))
    }
}

impl From<String> for EitherText<'_> {
    fn from(own: String) -> Self {
        EitherText(Cow::Owned(own))
    }
}

impl<'a> From<&'a EitherText<'_>> for EitherText<'a> {
    fn from(other: &'a EitherText<'_>) -> Self {
        EitherText(Cow::Borrowed(other))
    }
}

impl PartialEq<str> for EitherText<'_> {
    fn eq(&self, other: &str) -> bool {
        *self.0 == *other
    }
}

impl PartialEq<&str> for EitherText<'_> {
    fn eq(&self, other: &&str) -> bool {
        *self.0 == **other
    }
}

impl PartialEq<String> for EitherText<'_> {
    fn eq(&self, other: &String) -> bool {
        *self.0 == **other
    }
}

impl PartialEq<EitherText<'_>> for &str {
    fn eq(&self, other: &EitherText<'_>) -> bool {
        **self == *other.0
    }
}

impl std::fmt::Display for EitherText<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&*self.0, f)
    }
}

impl std::fmt::Debug for EitherText<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(&*self.0, f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_view_is_borrowed_and_text_of_its_own_is_owned() {
        let buffer = String::from("  oslo  ");
        let view: EitherText<'_> = buffer.trim().into();
        let own: EitherText<'_> = format!("{}!", "bergen").into();
        assert!(view.is_view());
        assert!(!own.is_view());
        assert_eq!(view, "oslo");
        assert_eq!(own.len(), 7);
        let copy: String = view.to_owned();
        assert_eq!(copy, "oslo");
        assert_eq!(format!("{view} {own}"), "oslo bergen!");
    }
}
