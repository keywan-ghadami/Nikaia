//! What a Nikaia `List` can do that a Rust `Vec` spells differently.

/// `xs.map fn { … }` (Kap 5.3) is a method on the list itself, not a step in an
/// iterator pipeline, so it is one here too.
pub trait ListExt<T> {
    /// Every element, transformed. Takes the list, because a mapped list
    /// replaces the one it was built from.
    fn map<U>(self, f: impl FnMut(T) -> U) -> Vec<U>;
}

impl<T> ListExt<T> for Vec<T> {
    fn map<U>(self, f: impl FnMut(T) -> U) -> Vec<U> {
        self.into_iter().map(f).collect()
    }
}
