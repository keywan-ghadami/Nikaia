# tether

Views that outlive the scope that read their buffer, without copying the text.

| type | what it is |
| :--- | :--- |
| `Keep` | an append-only home for buffers; a buffer stays at its address until the `Keep` is dropped |
| `forever` | a `Keep` behind an `Arc`, borrowed as long as the program runs — for a handle that travels beside what is derived from it |
| `Held` / `hold` | one view of text carrying its own handle on its buffer; compared, hashed and printed as the text |
| `Holding<V>` / `Views` | a struct of views beside the one buffer it points into, with a **safe** constructor |
| `Dangling<T>` | storage for a value whose views are stretched, beside the handle that keeps their buffer — what anything derived through `forever` must be kept in |

```rust
use std::sync::Arc;
use tether::{Holding, Views};

struct Record<'a> { name: &'a str, size: usize }
enum RecordViews {}
impl Views for RecordViews {
    type Of<'a> = Record<'a>;
    fn shorten<'long: 's, 's>(x: &'s Record<'long>) -> &'s Record<'s> { x }
}

let buffer: Arc<str> = Arc::from("  oslo  ");
let held: Holding<RecordViews> =
    Holding::new(buffer, |text| Record { name: text.trim(), size: text.len() });
assert_eq!(held.get().name, "oslo");
```

No dependencies. Written for the Nikaia compiler's generated code, usable by
anything that needs the same.

## Every `unsafe`, and why it is sound

| where | what | why it holds |
| :--- | :--- | :--- |
| `Dangling::get`, `Dangling::drop` | `MaybeUninit::assume_init_ref` / `assume_init_drop` | the value is written in `new` and taken apart only in `drop`, once |
| `Keep::put` | `&*at` from `Arc::as_ptr` | the `Arc` is pushed and never removed until the `Keep` is dropped, so the buffer stays alive and in place; the reference is tied to the borrow of the `Keep` |
| `forever` (an `unsafe fn`) | `&*Arc::as_ptr(keep)` as `'static` | the caller's contract: everything derived is dropped before the last clone of the `Arc` |
| `hold` (an `unsafe fn`) | `&str` → `&'static str` | the caller's contract: the text points into `keep`; the clone of `keep` stored beside it keeps it alive |
| `Holding::new`, `Holding::clone` | `transmute_copy` of `V::Of<'_>` to `V::Of<'static>` | the closure must work for every lifetime, so the struct can point only into the buffer or at `'static` data; it is private, dropped before the buffer, and handed out only through `get`, which shortens it back |

**Two things the compiler checks for the caller**, so neither is a comment:

* `Views::shorten`'s body is `x`, which compiles only where the struct is
  **covariant** in its lifetime. A struct that could be written through
  (`Cell<&'a str>`, `&'a mut`) is refused there.
* `Holding::new`'s closure is `for<'a> FnOnce(&'a str) -> V::Of<'a>`, so a view
  of anything but the buffer (a local, a captured borrow) is refused.

**Why `Dangling`.** A struct holding a `&'static str` promises that the text is
valid wherever the struct goes; passed by value into a function, the reference
is taken to be valid for the whole call. `Held`, `Holding` and a value packed
beside a `forever` handle drop their value *before* the buffer, possibly inside
such a call, so the promise must not be made: the value lives in a
`MaybeUninit`, which makes none. Miri found both this and the `Box` in
`Keep::put` (moving a `Box` invalidates references into it; an `Arc` does not).

## How it is checked

It is its own workspace, so its lock file, its checks and the toolchain for
them are its own:

```sh
cd crates/unsafe/tether
cargo test                                                   # stable
cargo clippy --all-targets -- -D warnings
cargo +nightly miri test                                     # Stacked Borrows
MIRIFLAGS=-Zmiri-tree-borrows cargo +nightly miri test       # Tree Borrows
```

`scripts/check-unsafe-crates.sh` runs all four for every crate under
`crates/unsafe/`, and CI runs that script.
