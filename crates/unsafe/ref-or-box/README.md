# ref-or-box

One word that holds either a `&'static A` or an owned `Box<B>`, with the
null niche beside it, so `Option<RefOrBox<A, B>>` is one word too. The common
case is the reference and costs no allocation; the box is for the rare case
that needs more. No dependencies.

```rust
use ref_or_box::{Either, RefOrBox};

static SITE: &str = "load";
let site: RefOrBox<&'static str, Vec<String>> = RefOrBox::from_ref(&SITE);
assert!(matches!(site.get(), Either::Ref(s) if *s == "load"));
```

The low bit of the word tells the two apart, so both `A` and `B` must be
aligned to at least two bytes. That is checked while the code is compiled.

## Every `unsafe`, and why it is sound

| where | what | why it holds |
| :--- | :--- | :--- |
| `get` (reference) | `as_ref` on the word with the tag cleared | a tagged word came from `from_ref`, whose reference is `'static`; clearing the tag gives its address back |
| `get`, `boxed_mut` (box) | `as_ref` / `as_mut` on the word | an untagged word came from `Box::leak` in `from_box`; this value owns that box and lends it through `&self` / `&mut self` |
| `Drop` | `Box::from_raw` | an untagged word came from `Box::leak`, and this value is the only thing that holds it |
| `Send`, `Sync` | `unsafe impl` | the value holds a `&'static A` or owns a `Box<B>`, and the bounds are the ones those two would have |

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
