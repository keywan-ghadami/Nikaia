//! What goes between the brackets, converted where it has to be.
//!
//! **A length is an `i64`** ([ADR-048](../../../docs/specification/adr/adr-048.md)
//! D1), so `for i in 0..xs.len()` gives an `i64` and `xs[i]` is an `i64` index.
//! Rust indexes a sequence by `usize`, and this is the conversion - emitted, never
//! written. D1's trade is that the ceremony a user would write cannot fail while
//! the conversion the compiler writes can, in one direction only: a **negative**
//! index.
//!
//! **A negative index reports as an access out of bounds**, which D1 requires: it
//! *is* one, and a diagnostic about a failed conversion would be about something
//! the user did not write. `as usize` would not do - `-3 as usize` is
//! 18446744073709551613, and the message a reader gets is then about a number
//! nothing in their program contains.
//!
//! **Why a trait and not a function.** The emitter puts this around *every* index
//! it writes, because it does not know types (ADR-011 D2) and a map is indexed
//! too: `counts[path]` takes a `&str`. So the conversion has to be chosen by the
//! type of what is in the brackets, and choosing on a type is what a trait is. For
//! anything that is not an integer this is the identity, which is why a uniform
//! rule is safe here - and a uniform rule is the one that cannot be applied to the
//! wrong index.

/// What a value in brackets becomes.
pub trait At {
    type Out;
    fn at(self) -> Self::Out;
}

/// The one message, so a negative index reads the same however it arrived.
#[cold]
#[inline(never)]
fn out_of_bounds(index: i64) -> ! {
    panic!("index out of bounds: the index is {index}")
}

macro_rules! signed {
    ($($t:ty),*) => {
        $(
            impl At for $t {
                type Out = usize;
                fn at(self) -> usize {
                    match usize::try_from(self) {
                        Ok(index) => index,
                        Err(_) => out_of_bounds(self as i64),
                    }
                }
            }

            impl At for std::ops::Range<$t> {
                type Out = std::ops::Range<usize>;
                fn at(self) -> std::ops::Range<usize> {
                    std::ops::Range { start: self.start.at(), end: self.end.at() }
                }
            }

            impl At for std::ops::RangeInclusive<$t> {
                type Out = std::ops::RangeInclusive<usize>;
                fn at(self) -> std::ops::RangeInclusive<usize> {
                    let (start, end) = self.into_inner();
                    start.at()..=end.at()
                }
            }
        )*
    };
}

signed!(i8, i16, i32, i64, isize);

macro_rules! unsigned {
    ($($t:ty),*) => {
        $(
            impl At for $t {
                type Out = usize;
                fn at(self) -> usize {
                    self as usize
                }
            }

            impl At for std::ops::Range<$t> {
                type Out = std::ops::Range<usize>;
                fn at(self) -> std::ops::Range<usize> {
                    std::ops::Range { start: self.start.at(), end: self.end.at() }
                }
            }
        )*
    };
}

unsigned!(u8, u16, u32, u64, usize);

/// **Anything that is not a number goes through untouched** - a map's key, most
/// of all. `&T` covers `&str`, `&String` and a reference to any key type, which
/// is the shape Rust's `Index` for a map wants anyway.
impl<'a, T: ?Sized> At for &'a T {
    type Out = &'a T;
    fn at(self) -> &'a T {
        self
    }
}

/// The emitted spelling: `xs[nikaia_std::index::at(i)]`.
pub fn at<I: At>(index: I) -> I::Out {
    index.at()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_index_becomes_a_usize() {
        assert_eq!(at(3_i64), 3_usize);
        assert_eq!(at(0_i32), 0_usize);
        let xs = [10, 20, 30];
        assert_eq!(xs[at(2_i64)], 30);
    }

    /// D1's one requirement: the message is about the **index**, not about a
    /// conversion the user never wrote.
    #[test]
    fn a_negative_index_reports_as_an_access_out_of_bounds() {
        let failed = std::panic::catch_unwind(|| at(-1_i64)).expect_err("it aborts");
        let said = failed
            .downcast_ref::<String>()
            .expect("the message is a String");
        assert!(
            said.contains("index out of bounds") && said.contains("-1"),
            "the message must name the index: {said}"
        );
        // And not the wrapped number `as usize` would have produced.
        assert!(!said.contains("18446744073709551615"), "{said}");
    }

    /// A key is not a number, and a uniform rule at every index is only safe
    /// because of this.
    #[test]
    fn a_key_goes_through_untouched() {
        use std::collections::HashMap;
        let mut counts: HashMap<&str, i64> = HashMap::new();
        counts.insert("host", 7);
        assert_eq!(counts[at("host")], 7);
    }

    #[test]
    fn a_slice_range_is_converted_at_both_ends() {
        let text = "abcdef";
        assert_eq!(&text[at(1_i64..3_i64)], "bc");
        assert_eq!(&text[at(1_i64..=3_i64)], "bcd");
    }
}
