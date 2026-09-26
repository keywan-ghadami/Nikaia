//! Views that outlive the scope that read their buffer.
//!
//! A view of text is a plain `&str`. When views have to outlive the local that
//! read the buffer, the buffer moves somewhere that lives as long as they do:
//!
//! * [`Keep`] — an append-only home for buffers; every buffer in it stays at
//!   its address until the `Keep` is dropped.
//! * [`forever`] — a `Keep` behind an `Arc`, borrowed for as long as a clone of
//!   the `Arc` travels beside what is derived from it.
//! * [`Held`] / [`hold`] — one view of text carrying its own handle on the
//!   buffer, for a container that drops entries while it keeps reading.
//! * [`Holding`] — a whole struct of views carrying one handle on the buffer
//!   it points into, with a **safe** constructor: the struct is built from the
//!   buffer inside a closure, so every view in it provably points there.
//!
//! Every `unsafe` in this crate, and the argument for it, is listed in
//! `README.md`.

#![deny(unsafe_op_in_unsafe_fn)]

use std::any::Any;
use std::sync::{Arc, Mutex};

/// **A value whose references may outlive what they point into, as far as the
/// compiler is told.**
///
/// A struct holding a `&'static str` promises the compiler that the text is
/// valid wherever the struct is: passed by value into a function, the
/// reference is taken to be valid for the whole call. A value here is dropped
/// *before* the buffer it points into, inside such a call - so the promise has
/// to be withdrawn. A `MaybeUninit` makes none, which is the whole of what this
/// wrapper is for (the same device as `yoke`'s `KindaSortaDangling`).
///
/// Safe in itself: it stores a value and hands it back by reference. It is
/// what a value holding stretched views has to be stored in, beside the handle
/// that keeps their buffer - as [`Held`] and [`Holding`] do, and as code that
/// uses [`forever`] must.
pub struct Dangling<T>(std::mem::MaybeUninit<T>);

impl<T> Dangling<T> {
    pub fn new(value: T) -> Self {
        Dangling(std::mem::MaybeUninit::new(value))
    }

    pub fn get(&self) -> &T {
        // SAFETY: initialised in `new` and only taken apart in `drop`.
        unsafe { self.0.assume_init_ref() }
    }
}

impl<T> Drop for Dangling<T> {
    fn drop(&mut self) {
        // SAFETY: initialised in `new`, dropped exactly once, here.
        unsafe { self.0.assume_init_drop() }
    }
}

impl<T: Clone> Clone for Dangling<T> {
    fn clone(&self) -> Self {
        Dangling::new(self.get().clone())
    }
}

/// Buffers whose views outlive the scope that made them.
///
/// **Append-only, and nothing in it ever moves**: a buffer is boxed when it
/// arrives and the box is freed only when the `Keep` is, so a view of an
/// earlier buffer stays valid while later ones arrive. That is what lets one
/// `Keep` serve a loop over many files or a recursive include , in
/// place of ADR-008 D4's buffer table.
///
/// The lock is for [`Keep::put`] through `&self` from two tasks at once, and it
/// is taken once per **buffer** - per file read, never per view.
pub struct Keep {
    // An `Arc` and not a `Box`: moving a `Box` asserts it is the only way to
    // its contents, which would invalidate every view already handed out -
    // moving the `Arc` touches nothing it points at.
    buffers: Mutex<Vec<Arc<dyn Any + Send + Sync>>>,
}

impl Keep {
    pub fn new() -> Keep {
        Keep {
            buffers: Mutex::new(Vec::new()),
        }
    }

    /// Move a buffer in, and hand back the place it now lives, for as long as
    /// this `Keep` is borrowed.
    pub fn put<B: Any + Send + Sync>(&self, buffer: B) -> &B {
        let shared: Arc<B> = Arc::new(buffer);
        let at: *const B = Arc::as_ptr(&shared);
        self.buffers
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(shared);
        // SAFETY: the `Arc` was just pushed and is never removed or handed out
        // until `self` is dropped, so the buffer it points at stays alive and
        // in place; nothing writes to it. So the reference is valid for as
        // long as `self` is borrowed, which is the lifetime it is given.
        unsafe { &*at }
    }

    /// How many buffers are kept, for `--tethers` and for tests.
    pub fn len(&self) -> usize {
        self.buffers
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for Keep {
    fn default() -> Keep {
        Keep::new()
    }
}

/// The `Keep` behind a handle, borrowed for as long as the program runs.
///
/// # Safety
///
/// Everything derived from the reference must be dropped before the last
/// clone of `keep` is, which generated code guarantees in exactly two shapes:
/// the handle is a local declared first in the function, so every other local
/// is dropped before it; or what is derived is packed into a value that holds a
/// clone of the handle beside it, declared *after* it so it is dropped first
/// .
pub unsafe fn forever(keep: &Arc<Keep>) -> &'static Keep {
    // SAFETY: the `Keep` lives on the heap behind the `Arc`, so its address is
    // stable; the caller keeps a clone alive for as long as the reference is.
    unsafe { &*Arc::as_ptr(keep) }
}

/// One view of text that carries its own handle on the buffer it points into
/// for a container that drops entries while the loop around it
/// keeps reading, where one `Keep` for the whole loop would keep every buffer
/// the loop ever read.
///
/// **Compared and hashed by content**, never by where it points: two files
/// that both say `Hamburg` are one key.
#[derive(Clone)]
pub struct Held {
    // Declared first, so it is dropped before the handle on its buffer.
    text: Dangling<&'static str>,
    _keep: Arc<Keep>,
}

impl Held {
    fn text(&self) -> &str {
        self.text.get()
    }
}

/// A view of `keep`'s text that keeps `keep` alive.
///
/// # Safety
///
/// `text` must point into a buffer `keep` holds.
pub unsafe fn hold(keep: &Arc<Keep>, text: &str) -> Held {
    Held {
        // SAFETY: `text` points into `keep` (the caller's contract), and the
        // clone stored beside it keeps `keep` alive as long as this value.
        text: Dangling::new(unsafe { std::mem::transmute::<&str, &'static str>(text) }),
        _keep: Arc::clone(keep),
    }
}

impl std::ops::Deref for Held {
    type Target = str;
    fn deref(&self) -> &str {
        self.text()
    }
}

impl std::borrow::Borrow<str> for Held {
    fn borrow(&self) -> &str {
        self.text()
    }
}

impl AsRef<str> for Held {
    fn as_ref(&self) -> &str {
        self.text()
    }
}

impl PartialEq for Held {
    fn eq(&self, other: &Held) -> bool {
        self.text() == other.text()
    }
}

impl Eq for Held {}

impl PartialEq<str> for Held {
    fn eq(&self, other: &str) -> bool {
        self.text() == other
    }
}

impl PartialEq<&str> for Held {
    fn eq(&self, other: &&str) -> bool {
        self.text() == *other
    }
}

impl PartialOrd for Held {
    fn partial_cmp(&self, other: &Held) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Held {
    fn cmp(&self, other: &Held) -> std::cmp::Ordering {
        self.text().cmp(other.text())
    }
}

impl std::hash::Hash for Held {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.text().hash(state)
    }
}

impl std::fmt::Display for Held {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(self.text(), f)
    }
}

impl std::fmt::Debug for Held {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(self.text(), f)
    }
}

/// **A struct of views, over any lifetime** — what [`Holding`] holds.
///
/// Implemented once per struct, always the same way:
///
/// ```
/// struct Record<'a> { name: &'a str }
/// enum RecordViews {}
/// impl tether::Views for RecordViews {
///     type Of<'a> = Record<'a>;
///     fn shorten<'long: 's, 's>(x: &'s Record<'long>) -> &'s Record<'s> { x }
/// }
/// ```
///
/// The body `x` compiles only where the struct is **covariant** in its
/// lifetime, which is the one property [`Holding`] relies on: a struct that
/// could be written through (a `Cell<&'a str>`, a `&'a mut`) is refused by
/// `rustc` right here.
pub trait Views {
    type Of<'a>;
    fn shorten<'long: 's, 's>(x: &'s Self::Of<'long>) -> &'s Self::Of<'s>;
}

/// **A struct of views held beside the buffer it points into.**
///
/// One handle per value, no allocation of its own: the text lives in the
/// `Arc`'s heap block and does not move when this value does. A read goes
/// through [`Holding::get`], which hands the struct out over a lifetime no
/// longer than the read, so nothing derived from it can outlive the buffer.
pub struct Holding<V: Views> {
    // Declared first, so it is dropped before the buffer it points into.
    value: Dangling<V::Of<'static>>,
    buffer: Arc<str>,
}

impl<V: Views> Holding<V> {
    /// Build the struct from the buffer. The closure is handed the buffer's
    /// text and nothing else with a lifetime it could keep, and it has to work
    /// for **every** lifetime, so what it returns can point into that text or
    /// at `'static` data and nowhere else.
    pub fn new(buffer: Arc<str>, build: impl for<'a> FnOnce(&'a str) -> V::Of<'a>) -> Self {
        let value = build(&buffer);
        let value = std::mem::ManuallyDrop::new(value);
        // SAFETY: `V::Of<'_>` and `V::Of<'static>` are one type up to a
        // lifetime, so the bits are the same value; `ManuallyDrop` keeps the
        // original from being dropped twice. The longer lifetime is never
        // observed: `value` is private, dropped before `buffer` (field order),
        // and handed out only through `get`, which shortens it back to a
        // borrow of `self` - and `self` holds the `Arc` that keeps the text
        // where it is.
        let value = unsafe { std::mem::transmute_copy::<V::Of<'_>, V::Of<'static>>(&value) };
        Holding {
            value: Dangling::new(value),
            buffer,
        }
    }

    /// The struct, over a lifetime no longer than this borrow.
    pub fn get(&self) -> &V::Of<'_> {
        V::shorten(self.value.get())
    }

    /// The buffer the struct points into.
    pub fn buffer(&self) -> &Arc<str> {
        &self.buffer
    }
}

impl<V: Views> Clone for Holding<V>
where
    for<'a> V::Of<'a>: Clone,
{
    fn clone(&self) -> Self {
        let copy = self.get().clone();
        let copy = std::mem::ManuallyDrop::new(copy);
        // SAFETY: as in `new` - the copy points where the original does, into
        // `self.buffer`, and the clone of the `Arc` stored beside it keeps
        // that text alive and in place.
        let value = unsafe { std::mem::transmute_copy::<V::Of<'_>, V::Of<'static>>(&copy) };
        Holding {
            value: Dangling::new(value),
            buffer: Arc::clone(&self.buffer),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Record<'a> {
        name: &'a str,
        parts: Vec<&'a str>,
    }
    enum RecordViews {}
    impl Views for RecordViews {
        type Of<'a> = Record<'a>;
        fn shorten<'long: 's, 's>(x: &'s Record<'long>) -> &'s Record<'s> {
            x
        }
    }

    #[test]
    fn a_struct_of_views_outlives_the_scope_that_read_its_buffer() {
        let mut kept: std::collections::VecDeque<Holding<RecordViews>> = Default::default();
        for i in 0..20 {
            let buffer: Arc<str> = Arc::from(format!("  a{i} b{i} c{i}  "));
            kept.push_back(Holding::new(Arc::clone(&buffer), |text| Record {
                name: text.trim(),
                parts: text.split_whitespace().collect(),
            }));
            // The element is now the buffer's only owner.
            drop(buffer);
            if kept.len() > 3 {
                kept.pop_front();
            }
        }
        // Every element moves.
        let moved: Vec<Holding<RecordViews>> = kept.into_iter().collect();
        let names: Vec<&str> = moved.iter().map(|h| h.get().name).collect();
        assert_eq!(names, ["a17 b17 c17", "a18 b18 c18", "a19 b19 c19"]);
        assert_eq!(moved[2].get().parts, ["a19", "b19", "c19"]);
    }

    /// The case `Dangling` exists for: the last owner of a buffer handed by
    /// value into a function that drops it there - for `Held`, `Holding`, and
    /// a value derived through `forever` packed beside its handle.
    #[test]
    fn the_last_owner_is_dropped_inside_a_call() {
        fn consume_held(held: Held) -> usize {
            let n = held.len();
            drop(held);
            n
        }
        fn consume_holding(holding: Holding<RecordViews>) -> usize {
            let n = holding.get().parts.len();
            drop(holding);
            n
        }
        let keep = Arc::new(Keep::new());
        let text = keep.put("one two".to_string());
        // SAFETY: `text` points into `keep`.
        let held = unsafe { hold(&keep, &text[..3]) };
        drop(keep);
        assert_eq!(consume_held(held), 3);
        let holding: Holding<RecordViews> = Holding::new(Arc::from("a b c"), |text| Record {
            name: text,
            parts: text.split(' ').collect(),
        });
        assert_eq!(consume_holding(holding), 3);
        struct Packed {
            value: Dangling<&'static str>,
            _keep: Arc<Keep>,
        }
        fn consume_packed(packed: Packed) -> usize {
            let n = packed.value.get().len();
            drop(packed);
            n
        }
        let keep = Arc::new(Keep::new());
        // SAFETY: `packed` holds a clone of `keep` beside what is derived.
        let text: &'static String = unsafe { forever(&keep) }.put("a=b".to_string());
        let packed = Packed {
            value: Dangling::new(&text[2..]),
            _keep: Arc::clone(&keep),
        };
        drop(keep);
        assert_eq!(consume_packed(packed), 1);
    }

    #[test]
    fn a_held_struct_crosses_a_thread() {
        let held: Holding<RecordViews> = Holding::new(Arc::from("x y"), |text| Record {
            name: text,
            parts: text.split(' ').collect(),
        });
        let got = std::thread::spawn(move || held.get().parts.len())
            .join()
            .unwrap();
        assert_eq!(got, 2);
    }

    #[test]
    fn a_later_buffer_does_not_move_an_earlier_one() {
        let keep = Keep::new();
        let first: &String = keep.put("first".to_string());
        let view: &str = &first[..3];
        for n in 0..1000 {
            keep.put(format!("buffer {n}"));
        }
        assert_eq!(view, "fir");
        assert_eq!(keep.len(), 1001);
    }

    // `Held` holds a handle on a `Keep`, whose lock is interior mutability -
    // but a key hashes and compares by its text alone, which never changes.
    #[allow(clippy::mutable_key_type)]
    #[test]
    fn a_held_view_outlives_the_scope_that_read_it_and_compares_by_content() {
        let mut seen: std::collections::HashSet<Held> = Default::default();
        for text in ["Hamburg;1", "Hamburg;2"] {
            let keep = Arc::new(Keep::new());
            let buffer = keep.put(text.to_string());
            let name = buffer.split(';').next().unwrap_or("");
            seen.insert(unsafe { hold(&keep, name) });
        }
        assert_eq!(seen.len(), 1);
        assert!(seen.contains("Hamburg"));
    }

    #[test]
    fn a_handle_crosses_a_thread() {
        let keep = Arc::new(Keep::new());
        let text: &'static String = unsafe { forever(&keep) }.put("a=b".to_string());
        struct Packed {
            value: &'static str,
            _keep: Arc<Keep>,
        }
        let packed = Packed {
            value: &text[2..],
            _keep: Arc::clone(&keep),
        };
        drop(keep);
        let got = std::thread::spawn(move || packed.value.to_string())
            .join()
            .unwrap();
        assert_eq!(got, "b");
    }
}

/// The README's example, run as a test.
#[doc = include_str!("../README.md")]
#[cfg(doctest)]
pub struct ReadmeDoctests;
