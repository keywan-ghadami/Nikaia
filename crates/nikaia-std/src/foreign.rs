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

/// **A C function said it hands back a handle, and handed back nothing**
/// ([ADR-155](../../../docs/specification/adr/adr-155.md) D3).
///
/// A declaration that does not say `?` is the author's **claim** that this
/// never happens — the arrangement `sync` on a declaration already has
/// ([ADR-124](../../../docs/specification/adr/adr-124.md) D2). A declaration is
/// written from a header by somebody reading it, and reading is where it can go
/// wrong, so the claim is checked: what a program gets is an abort in this
/// language's words rather than a handle that is secretly null.
///
/// `#[track_caller]` so that the location the panic hook is handed is the
/// **caller's** — the line of the generated file that made the call — and not a
/// line of this file, which
/// [ADR-044](../../../docs/specification/adr/adr-044.md) D1's table would have
/// nothing to look up for.
#[cold]
#[inline(never)]
#[track_caller]
pub fn nothing_came_back(declaration: &str) -> ! {
    panic!(
        "`{declaration}` handed back nothing, and its declaration does not say it can \
         - write `?` on the result to say that it may"
    )
}

/// **Text a C library owns** (D4): an address, and a zero byte somewhere after
/// it.
///
/// `#[repr(transparent)]` for D3's reason one type over: the handle *is* the
/// address, so a declaration that hands one back is handed the pointer C
/// returns and nothing is wrapped on the way.
#[repr(transparent)]
#[derive(Debug, Clone, Copy)]
pub struct CStr(core::ptr::NonNull<core::ffi::c_char>);

impl CStr {
    /// What a declaration that says `-> CStr` hands back
    /// ([ADR-155](../../../docs/specification/adr/adr-155.md) D3): the address,
    /// or an abort naming the declaration that claimed it would be one.
    #[track_caller]
    pub fn from_c(declaration: &str, address: *mut core::ffi::c_char) -> CStr {
        match core::ptr::NonNull::new(address) {
            Some(address) => CStr(address),
            None => nothing_came_back(declaration),
        }
    }

    /// The same, where the declaration **does** say `?` (D1). `None` is C's
    /// `NULL`, which is the whole of D2: the two are one machine word.
    pub fn maybe(address: *mut core::ffi::c_char) -> Option<CStr> {
        core::ptr::NonNull::new(address).map(CStr)
    }

    /// A copy of the text, owned by whoever asked for it.
    ///
    /// **It fails in one way**, and only one: the bytes may not be UTF-8, which
    /// text in this language is — the same failure `fs::read_to_string` has, for
    /// the same reason. *There is no text* used to be the other one and is a
    /// **value** now ([ADR-155](../../../docs/specification/adr/adr-155.md) D4):
    /// a declaration says `-> CStr?` where the library may find nothing, and the
    /// program writes `?? ""`.
    ///
    /// **The `unsafe` is here and nowhere else**, which is the whole of D4: the
    /// walk to the zero byte is the one thing a program must not have to write,
    /// and it is written once.
    pub fn to_string(self) -> Result<String, Box<dyn std::error::Error>> {
        // SAFETY: the address is not null by construction (D2's hull), and D4's
        // contract with the caller is that what a C function handed back is a C
        // string - an address with a zero byte after it. Nothing else in this
        // language can make one.
        let bytes = unsafe { core::ffi::CStr::from_ptr(self.0.as_ptr()) };
        match bytes.to_str() {
            Ok(text) => Ok(text.to_string()),
            Err(_) => Err("this C string is not UTF-8, and text in this language is".into()),
        }
    }
}
