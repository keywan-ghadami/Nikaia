//! **Text a C library owns**: an address, and a zero byte somewhere after it.
//!
//! [`CText`] is `#[repr(transparent)]` over a non-null pointer, so an `extern`
//! declaration can return `Option<CText>` where C returns a `char *` that may
//! be null - the declaration is where the promise *this is a C string* is
//! made, and calling an `extern` function is already `unsafe`. From then on,
//! copying the text out is safe.
//!
//! Every `unsafe` in this crate, and the argument for it, is listed in
//! `README.md`.

#![deny(unsafe_op_in_unsafe_fn)]

use core::ffi::c_char;
use core::ptr::NonNull;

/// A C string somebody else owns.
///
/// **No safe constructor.** A value comes from an `extern` declaration that
/// returns one, or from [`CText::from_ptr`], and both say the same thing: the
/// address is readable up to and including a zero byte, for as long as the
/// value is used.
#[repr(transparent)]
#[derive(Debug, Clone, Copy)]
pub struct CText(NonNull<c_char>);

impl CText {
    /// A C string at `address`.
    ///
    /// # Safety
    ///
    /// `address` must be readable up to and including a zero byte, and stay so
    /// for as long as the value is used.
    pub unsafe fn from_ptr(address: NonNull<c_char>) -> CText {
        CText(address)
    }

    /// The address.
    pub fn as_ptr(self) -> *const c_char {
        self.0.as_ptr()
    }

    /// A copy of the text, owned by whoever asked for it. Fails only where the
    /// bytes are not UTF-8.
    pub fn to_string(self) -> Result<String, core::str::Utf8Error> {
        // SAFETY: the contract every `CText` was made under - an `extern`
        // declaration returning one, or `from_ptr` - is that the address is
        // readable up to a zero byte for as long as the value is used, and it
        // is used here, for the length of this call.
        let bytes = unsafe { core::ffi::CStr::from_ptr(self.0.as_ptr()) };
        bytes.to_str().map(str::to_string)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_c_string_is_copied_out() {
        let owned = std::ffi::CString::new("HOME=/root").expect("no zero inside");
        let address = NonNull::new(owned.as_ptr() as *mut c_char).expect("not null");
        // SAFETY: `owned` is a zero-terminated string alive for this test.
        let text = unsafe { CText::from_ptr(address) };
        assert_eq!(text.to_string().as_deref(), Ok("HOME=/root"));
    }

    #[test]
    fn bytes_that_are_not_utf8_are_refused() {
        let owned = std::ffi::CString::new(vec![b'o', b'k', 0xFF]).expect("no zero inside");
        let address = NonNull::new(owned.as_ptr() as *mut c_char).expect("not null");
        // SAFETY: as above.
        let text = unsafe { CText::from_ptr(address) };
        assert!(text.to_string().is_err());
    }

    #[test]
    fn an_absent_one_is_none_and_one_word() {
        assert_eq!(
            std::mem::size_of::<Option<CText>>(),
            std::mem::size_of::<*const c_char>()
        );
    }

    #[cfg(not(miri))]
    #[test]
    fn an_extern_declaration_returns_one() {
        unsafe extern "C" {
            fn getenv(name: *const c_char) -> Option<CText>;
        }
        // SAFETY: `getenv` takes a zero-terminated name and returns a C string
        // or null; nothing sets the environment while this reads it.
        let path = unsafe { getenv(c"PATH".as_ptr()) };
        let missing = unsafe { getenv(c"C_TEXT_NOT_SET_ANYWHERE".as_ptr()) };
        assert!(path.is_some_and(|p| p.to_string().is_ok()));
        assert!(missing.is_none());
    }
}

/// The README's example, run as a test.
#[doc = include_str!("../README.md")]
#[cfg(doctest)]
pub struct ReadmeDoctests;
