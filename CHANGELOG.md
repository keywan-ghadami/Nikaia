# Changelog

## [Unreleased]

### Added
- **Spec/ADR**: ADR-005 "Borrow Model & Lifetime Strategy" — normative four-group taxonomy (solved by rustc / solved by the driver / needs a language construct / genuine user bug), decisions D1–D7, rejected alternatives (implicit clone, in-source borrow annotations, struct lifetime parameters).
- **Spec Part I (6.5–6.8)**: References & Borrowing guarantees ("no lifetime annotations, ever"), Tethered Slices rule ("transient = borrow, stored = tether"), Borrow Contract Ledger overview, diagnostics promise with worked `NK2301` example.
- **Spec Part III (13.5)**: The Borrow Contract Ledger (`nikaia.contracts`) — generated committed file serving as incremental cache and as the diff basis for narrated "what changed and what broke" errors (`NK2401`).
- **Spec Part III (Appendix C)**: The Diagnostics Contract — untranslated rustc errors are compiler bugs; NK error-code catalogue with testable requirements.
- **Spec/ADR (D8)**: Determinism requirement for contract inference — the ledger is a byte-deterministic pure function of (source tree, toolchain); parallel solving allowed, cross-toolchain stability explicitly not required, one profile-neutral ledger per project, violations are compiler bugs; includes implementer ban list and CI double-build/cross-OS test definitions.
- **Spec Part III (13.5)**: Determinism guarantee and `--locked` verification mode for `nikaia.contracts`.
- **Spec/ADR**: ADR-006 "Resource Cleanup under Implicit Async" — the `Cleanup` trait (pausable `cleanup() throws` + synchronous `drop` fallback, compiler-inserted at scope exit); three-death-paths taxonomy; cleanup errors throw normally (signatures tell the truth, secondary-error attachment during unwinding, explicit `close()` opt-in); cancellation parks cleanups with the runtime; shutdown drain phase with `cleanup-deadline` and a termination argument (cannot hang, no deadlock via ADR-005 D6); rejected alternatives (blocking drop, fire-and-forget, explicit-close-only, linear types, rustc `async_drop`).
- **Spec Part I (6.4)**: `Drop` vs. `Cleanup` distinction with worked `NK2601` diagnostic, `sync`-context restriction (`NK2602`), parked-cleanup and honest Lite-panic notes.
- **Spec Part II (12.4)**: defined semantics for "cancelled and cleaned up" (parked cleanup).
- **Spec Part III**: `cleanup-deadline` manifest key (13.3); `NK26xx` resource-cleanup codes in Appendix C.
- **Spec/ADR (ADR-006 D6)**: The Panic Hook (`std::panic::on_panic`) — global per application, `sync`, runs on every panic path including Lite's abort and the WASM trap (rides on the backend invoking the hook before abort under `panic = abort`); diagnosis-not-cleanup rule, brief-blocking allowance with Advanced nuance, recursion guard; `NK2604`; stackable hook chains rejected. Spec: Part I 7.2 (worked example), 6.4 note updated, Part III Appendix A.

### Changed
- **Spec Part II (10.6)**: Zero-copy parsing respecified on Tethered Slices (`bytes::Bytes` model over `Shared`); buffer provably outlives tokens instead of borrow-checker rejection. Unsafe self-referential codegen recorded as future optimization note.
- **Spec Part II (12.2)**: "No I/O while holding a lock" is now a compile-time rule — `access`/`access_all` require a `sync` lambda in both profiles (`NK2201`); runtime reentrancy check/poisoning demoted to backstop.
- **Spec Part II (12.7)**: Scoped tasks respecified for soundness — Lite: runtime-owned scopes, any child task allowed; Advanced: child tasks must be `sync` (`NK2102`), async children use `spawn` + implicit move. Beginner-oriented rewrite with worked error messages.
- **Spec Part I (8.3)**: Resolved contradiction with 5.4/11.2 — `spawn` uses implicit move (no `move` keyword); rewritten with `.clone()` guidance and worked `NK2101` error.
- **Spec Parts I–III**: Version bumped to 0.0.6 (aligns with README badge); fixed a `nila` code-fence typo (Part I, 5.4) and an unclosed code fence (Part III, Appendix B).

### Fixed
- **Grammar**: Fixed `expr` rule to include `block`, enabling parsing of blocks in expression positions (e.g., `spawn({ ... })`).
- **Grammar**: Added specific `spawn_expr` rule to correctly parse `spawn` statements as `Expr::Spawn` instead of generic function calls.
- **Grammar**: Added `skip_ws` rule to consume trailing whitespace at the end of the program, preventing `ParseError` at EOF.
- **Grammar**: Renamed whitespace skipper rule from `ws` to `skip_ws` to avoid infinite recursion bug in `winnow-grammar`.
- **Tests**: Fixed syntax error (missing comma) in `tests/hello_world.rs`.
- **Build**: Updated `src/main.rs` and `tests/hello_world.rs` to wrap input source in `LocatingSlice` to satisfy `winnow::stream::Location` trait bounds required by the generated parser.
