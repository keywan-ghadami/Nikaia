# Rust AST Source Location

The Rust compiler's Abstract Syntax Tree (AST) source code is located at:

`/home/user/.rustup/toolchains/nightly-2026-01-01-x86_64-unknown-linux-gnu/lib/rustlib/rustc-src/rust/compiler/rustc_ast/`

## Troubleshooting

If you cannot find the directory listed above, it is likely because the `rust-src` component is not installed for your toolchain because the environment was rebuilt. This component provides the full source code for the Rust compiler and standard library.

To install it, run the following command:

```bash
rustup component add rust-src
```

After the installation is complete, the directory should be available.
