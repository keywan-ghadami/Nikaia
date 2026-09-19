//! What a C library hands back that this language has to copy
//! ([ADR-147](../../../docs/specification/adr/adr-147.md) D4).
//!
//! `getenv` hands back a C string: memory the caller does not own, whose
//! lifetime is the library's, and which ends at a zero byte rather than
//! carrying a length. None of those three is something a `String` can be made
//! of without reading it, so the reading is here — **one function, written
//! once**, where every program would otherwise write the same loop inside its
//! own `unsafe` block.
//!
//! **The handle is opaque and has no `cleanup`**, which is what tells it from
//! [ADR-147](../../../docs/specification/adr/adr-147.md) D3's: a `FILE` is ours
//! to close and a C string is not ours at all. Nothing here frees anything.

/// **Text a C library owns** (D4): an address, and a zero byte somewhere after
/// it.
///
/// `#[repr(transparent)]` for D3's reason one type over: the handle *is* the
/// address, so a declaration that hands one back is handed the pointer C
/// returns and nothing is wrapped on the way.
#[repr(transparent)]
#[derive(Debug, Clone, Copy)]
pub struct CStr(*const core::ffi::c_char);

impl CStr {
    /// A copy of the text, owned by whoever asked for it.
    ///
    /// **It fails rather than guessing**, in the two ways this can: the address
    /// may be null, which is what every C function that finds nothing hands
    /// back, and the bytes may not be UTF-8, which text in this language is.
    /// Both are `throws` rather than an abort, because a caller can do
    /// something about either — `getenv` finding nothing is an ordinary
    /// Tuesday, and Part III A.2's aborts are for what a program's own
    /// arithmetic got wrong.
    ///
    /// **The `unsafe` is here and nowhere else**, which is the whole of D4: the
    /// walk to the zero byte is the one thing a program must not have to write,
    /// and it is written once.
    pub fn to_string(self) -> Result<String, Box<dyn std::error::Error>> {
        if self.0.is_null() {
            return Err("this C string is a null address, so there is no text to copy".into());
        }
        // SAFETY: the address is not null, and D4's contract with the caller is
        // that what a C function handed back is a C string - an address with a
        // zero byte after it. Nothing else in this language can make one.
        let bytes = unsafe { core::ffi::CStr::from_ptr(self.0) };
        match bytes.to_str() {
            Ok(text) => Ok(text.to_string()),
            Err(_) => Err("this C string is not UTF-8, and text in this language is".into()),
        }
    }
}
