//! `std::text` - characters and strings.

/// The value of a decimal digit: `'7'` is 7.
///
/// A free function rather than a method on the character, because Rust's own
/// `char::to_digit` has the same name and a different shape (it takes a radix
/// and yields an option). An inherent method wins over a trait method, so a
/// Nikaia `to_digit` on a character could never be reached - and a name that
/// resolves to the wrong thing is worse than a name that reads differently.
pub fn digit_value(c: char) -> i32 {
    debug_assert!(c.is_ascii_digit(), "not a digit: {c:?}");
    c as i32 - '0' as i32
}
