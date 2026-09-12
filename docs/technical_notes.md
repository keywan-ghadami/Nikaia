# Technical Notes & Knowledge Base

> **The path these notes are about is withdrawn.** Nothing in the toolchain links
> `rustc_private` or builds a `rustc_ast::Crate` any more: a `.nika` file becomes
> Rust source text and `rustc` compiles it like any other Rust
> ([ADR-004](specification/adr/adr-004.md) D1). This page is kept as the notebook
> page it is — these were real findings about a real compiler, and a notebook is
> not edited when the experiment ends. `../CHANGELOG.md` and
> [`withdrawn-one-way-down.md`](withdrawn-one-way-down.md) say what happened.

## Rust Compiler Internals (Nightly 2026-01-01)

Integration with `rustc_private` revealed specific details about the internal AST structure which differs from older versions.

### 1. Structure of `rustc_ast::Item`
The `Item` struct does **not** store the identifier directly.
```rust
pub struct Item<K = ItemKind> {
    pub attrs: AttrVec,
    pub id: NodeId,
    pub span: Span,
    pub vis: Visibility,
    pub kind: K, // Identifier is stored inside specific K variants (e.g. Fn)
    pub tokens: Option<LazyAttrTokenStream>,
}
```

### 2. Structure of `rustc_ast::Fn`
The `Fn` struct **does** store the identifier.
```rust
pub struct Fn {
    pub defaultness: Defaultness,
    pub ident: Ident, // <--- Name is here
    pub generics: Generics,
    pub sig: FnSig,
    // ...
}
```

### 3. Smart Pointers
*   `P<T>` is widely used in the AST (Pointer to immutable data).
*   In this version, `P<T>` seems to be an alias for `Box<T>` or compatible with it, as `Box::new()` is used for construction in place of `P(...)`.
*   Importing `rustc_ast::ptr::P` failed, suggesting refactoring in the compiler source. `Box` works.

### 4. Literals (`Lit`)
*   `rustc_ast::Lit` is not the same as `rustc_ast::token::Lit`.
*   `ExprKind::Lit` takes `rustc_ast::token::Lit`.
*   To construct an integer literal:
    ```rust
    LitKind::Int(u128_val.into(), LitIntType::Signed(IntTy::I64))
    ```

### 5. Macro Arguments
*   `MacArgs` is replaced/used in conjunction with `DelimArgs`.
*   `MacCall` uses `Box<DelimArgs>`.

### 6. Linkage Issues
*   Linking against `rustc_driver` requires `feature(rustc_private)`.
*   Running a binary that depends on `rustc_driver` often fails with "cannot satisfy dependencies so `std` only shows up once" unless compiled with `RUSTFLAGS="-C prefer-dynamic"`.
