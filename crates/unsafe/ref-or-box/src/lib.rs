//! **One word that holds either a `&'static A` or a `Box<B>`**, and the null
//! niche beside it, so `Option<RefOrBox<A, B>>` is one word too.
//!
//! The common case is the reference, and it costs no allocation; the box is for
//! the rare case that needs more. The low bit of the word tells the two apart,
//! which is why both `A` and `B` must be aligned to at least two bytes - a
//! requirement checked when the code is compiled, not at run time.
//!
//! Every `unsafe` in this crate, and the argument for it, is listed in
//! `README.md`.

#![deny(unsafe_op_in_unsafe_fn)]

use std::marker::PhantomData;
use std::ptr::NonNull;

/// What [`RefOrBox::get`] hands back.
#[derive(Debug)]
pub enum Either<'s, A: 'static, B> {
    Ref(&'static A),
    Boxed(&'s B),
}

/// Either a `&'static A` or an owned `Box<B>`, in one word.
pub struct RefOrBox<A: 'static, B> {
    word: NonNull<()>,
    _held: PhantomData<(&'static A, Box<B>)>,
}

/// The low bit set: the word is a reference.
const REF: usize = 1;

/// Both alignments leave the low bit free - asked while compiling.
struct Aligned<A, B>(PhantomData<(A, B)>);
impl<A, B> Aligned<A, B> {
    const OK: () = assert!(
        std::mem::align_of::<A>() >= 2 && std::mem::align_of::<B>() >= 2,
        "RefOrBox needs both types aligned to at least two bytes"
    );
}

impl<A: 'static, B> RefOrBox<A, B> {
    /// The reference, in the word itself.
    pub fn from_ref(at: &'static A) -> Self {
        #[allow(clippy::let_unit_value)]
        let () = Aligned::<A, B>::OK;
        RefOrBox {
            word: NonNull::from(at).cast::<()>().map_addr(|a| a | REF),
            _held: PhantomData,
        }
    }

    /// The box, with its address as the word.
    pub fn from_box(boxed: Box<B>) -> Self {
        #[allow(clippy::let_unit_value)]
        let () = Aligned::<A, B>::OK;
        RefOrBox {
            word: NonNull::from(Box::leak(boxed)).cast::<()>(),
            _held: PhantomData,
        }
    }

    fn is_ref(&self) -> bool {
        self.word.addr().get() & REF != 0
    }

    /// Which of the two it holds.
    pub fn get(&self) -> Either<'_, A, B> {
        match self.is_ref() {
            true => {
                let at = self.word.map_addr(|a| {
                    // The tag cleared from a non-zero address that was at
                    // least two-aligned is that address again, and not zero.
                    std::num::NonZeroUsize::new(a.get() & !REF).unwrap_or(a)
                });
                // SAFETY: a tagged word came from `from_ref`, whose reference
                // is `'static`; clearing the tag gives its address back.
                Either::Ref(unsafe { at.cast::<A>().as_ref() })
            }
            // SAFETY: an untagged word came from `Box::leak` in `from_box`,
            // and this value owns that box; borrowed through `&self`.
            false => Either::Boxed(unsafe { self.word.cast::<B>().as_ref() }),
        }
    }

    /// The box, where it holds one.
    pub fn boxed_mut(&mut self) -> Option<&mut B> {
        match self.is_ref() {
            true => None,
            // SAFETY: as in `get`, borrowed through `&mut self`.
            false => Some(unsafe { self.word.cast::<B>().as_mut() }),
        }
    }
}

impl<A: 'static, B> Drop for RefOrBox<A, B> {
    fn drop(&mut self) {
        if !self.is_ref() {
            // SAFETY: an untagged word came from `Box::leak` in `from_box`,
            // and this value is the only thing that holds it.
            drop(unsafe { Box::from_raw(self.word.cast::<B>().as_ptr()) });
        }
    }
}

// SAFETY: the value holds a `&'static A` (shareable across threads when `A`
// is `Sync`) or owns a `Box<B>` (sendable when `B` is `Send`), which is what the
// `PhantomData` says; the word is only their address.
unsafe impl<A: Sync + 'static, B: Send> Send for RefOrBox<A, B> {}
// SAFETY: shared access hands out `&A` and `&B` and nothing else.
unsafe impl<A: Sync + 'static, B: Sync> Sync for RefOrBox<A, B> {}

impl<A: std::fmt::Debug + 'static, B: std::fmt::Debug> std::fmt::Debug for RefOrBox<A, B> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.get().fmt(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static SITE: &str = "load";

    #[test]
    fn one_word_and_a_niche() {
        assert_eq!(
            std::mem::size_of::<RefOrBox<&str, Vec<u8>>>(),
            std::mem::size_of::<usize>()
        );
        assert_eq!(
            std::mem::size_of::<Option<RefOrBox<&str, Vec<u8>>>>(),
            std::mem::size_of::<usize>()
        );
    }

    #[test]
    fn a_reference_and_a_box_come_back_as_they_went_in() {
        let r: RefOrBox<&'static str, (String, Vec<u32>)> = RefOrBox::from_ref(&SITE);
        assert!(matches!(r.get(), Either::Ref(s) if *s == "load"));
        let mut b: RefOrBox<&'static str, (String, Vec<u32>)> =
            RefOrBox::from_box(Box::new(("cold".to_string(), vec![1, 2])));
        b.boxed_mut().unwrap().1.push(3);
        assert!(matches!(b.get(), Either::Boxed((s, v)) if s == "cold" && v == &[1, 2, 3]));
        let mut r = r;
        assert!(r.boxed_mut().is_none());
        // Replacing one with the other drops the box exactly once.
        r = b;
        assert!(matches!(r.get(), Either::Boxed(_)));
    }

    #[test]
    fn it_crosses_a_thread() {
        let b: RefOrBox<&'static str, String> = RefOrBox::from_box(Box::new("x".to_string()));
        let got = std::thread::spawn(move || matches!(b.get(), Either::Boxed(s) if s == "x"))
            .join()
            .unwrap();
        assert!(got);
    }
}

/// The README's example, run as a test.
#[doc = include_str!("../README.md")]
#[cfg(doctest)]
pub struct ReadmeDoctests;
