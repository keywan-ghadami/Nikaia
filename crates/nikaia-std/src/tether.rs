//! Where a buffer lives once views of it outlive the scope that made it
//! ([ADR-209](../../../docs/specification/adr/adr-209.md), Part I 6.6).
//!
//! **A view stays a plain `&str`.** What changes when it tethers is only where
//! its buffer lives: not in the local that read it, but in a [`Keep`] owned by
//! whatever keeps the views - the caller's frame (D2), a handle that travels
//! with the value into a task (D3), or one handle per element of a container
//! that drops entries while the loop around it goes on reading (D4). The
//! compiler picks which, per place, from facts it already has; a program never
//! names any of this.
//!
//! **Two `unsafe` functions and the reason for each.** A `Keep` hands out
//! references tied to its own borrow and is safe. [`forever`] and [`hold`] are
//! the two places a reference is given a longer type than the borrow it came
//! from, because the thing that keeps its buffer alive is a handle the
//! reference travels *with* rather than a scope `rustc` can see. Both are
//! called only from generated code, and only with a handle that is packed next
//! to what is derived from it (ADR-209 D3, D4).

use std::any::Any;
use std::sync::{Arc, Mutex};

/// Buffers whose views outlive the scope that made them.
///
/// **Append-only, and nothing in it ever moves**: a buffer is boxed when it
/// arrives and the box is freed only when the `Keep` is, so a view of an
/// earlier buffer stays valid while later ones arrive. That is what lets one
/// `Keep` serve a loop over many files or a recursive include (ADR-209 D2), in
/// place of ADR-008 D4's buffer table.
///
/// The lock is for [`Keep::put`] through `&self` from two tasks at once, and it
/// is taken once per **buffer** - per file read, never per view.
pub struct Keep {
    buffers: Mutex<Vec<Box<dyn Any + Send + Sync>>>,
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
        let boxed: Box<B> = Box::new(buffer);
        let at: *const B = &*boxed;
        self.buffers
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(boxed);
        // SAFETY: the box was just pushed and is never removed, moved out or
        // mutated until `self` is dropped; a `Box`'s contents do not move when
        // the `Vec` holding the box reallocates. So the reference is valid for
        // as long as `self` is borrowed, which is the lifetime it is given.
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
/// (ADR-209 D3).
pub unsafe fn forever(keep: &Arc<Keep>) -> &'static Keep {
    // SAFETY: the `Keep` lives on the heap behind the `Arc`, so its address is
    // stable; the caller keeps a clone alive for as long as the reference is.
    unsafe { &*Arc::as_ptr(keep) }
}

/// One view of text that carries its own handle on the buffer it points into
/// (ADR-209 D4): for a container that drops entries while the loop around it
/// keeps reading, where one `Keep` for the whole loop would keep every buffer
/// the loop ever read.
///
/// **Compared and hashed by content**, never by where it points
/// ([ADR-008](../../../docs/specification/adr/adr-008.md) D4): two files that
/// both say `Hamburg` are one key.
#[derive(Clone)]
pub struct Held {
    text: &'static str,
    _keep: Arc<Keep>,
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
        text: unsafe { std::mem::transmute::<&str, &'static str>(text) },
        _keep: Arc::clone(keep),
    }
}

impl std::ops::Deref for Held {
    type Target = str;
    fn deref(&self) -> &str {
        self.text
    }
}

impl std::borrow::Borrow<str> for Held {
    fn borrow(&self) -> &str {
        self.text
    }
}

impl AsRef<str> for Held {
    fn as_ref(&self) -> &str {
        self.text
    }
}

impl PartialEq for Held {
    fn eq(&self, other: &Held) -> bool {
        self.text == other.text
    }
}

impl Eq for Held {}

impl PartialEq<str> for Held {
    fn eq(&self, other: &str) -> bool {
        self.text == other
    }
}

impl PartialEq<&str> for Held {
    fn eq(&self, other: &&str) -> bool {
        self.text == *other
    }
}

impl PartialOrd for Held {
    fn partial_cmp(&self, other: &Held) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Held {
    fn cmp(&self, other: &Held) -> std::cmp::Ordering {
        self.text.cmp(other.text)
    }
}

impl std::hash::Hash for Held {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.text.hash(state)
    }
}

impl std::fmt::Display for Held {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(self.text, f)
    }
}

impl std::fmt::Debug for Held {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(self.text, f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
