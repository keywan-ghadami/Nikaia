# c-text

Text a C library owns: an address with a zero byte somewhere after it. `CText`
is `#[repr(transparent)]` over a non-null pointer, so an `extern` declaration
can return `Option<CText>` where C returns a `char *` that may be null. The
declaration is where *this is a C string* is promised, and calling an
`extern` function is already `unsafe`. Copying the text out is then safe.
No dependencies.

```rust,no_run
use c_text::CText;

extern "C" {
    fn getenv(name: *const std::ffi::c_char) -> Option<CText>;
}
let path = unsafe { getenv(c"PATH".as_ptr()) };
println!("{:?}", path.map(|p| p.to_string()));
```

## Every `unsafe`, and why it is sound

| where | what | why it holds |
| :--- | :--- | :--- |
| `CText::from_ptr` (an `unsafe fn`) | makes a `CText` | the caller's contract: readable up to a zero byte, for as long as the value is used. There is no safe constructor. |
| `to_string` | `CStr::from_ptr` | the contract every `CText` was made under, from an `extern` declaration or `from_ptr`, holds for the length of this call |

## How it is checked

Its own workspace, so its lock file and its checks are its own:

```sh
cargo test && cargo test --all-features
cargo clippy --all-features --all-targets -- -D warnings
cargo +nightly miri test --all-features                       # Stacked Borrows
MIRIFLAGS=-Zmiri-tree-borrows cargo +nightly miri test --all-features
```

`scripts/check-unsafe-crates.sh` runs these for every crate under
`crates/unsafe/`, and CI runs that script.

Miri runs every test except the one that calls the C library's `getenv`.
