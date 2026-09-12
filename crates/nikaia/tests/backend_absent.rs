//! A compiler built without the bridge backend says so, and compiles nothing.
//!
//! `rustc-backend` is the one feature that links `rustc_private`, and so the one
//! feature that needs ADR-001 D1's pinned nightly with its `rustc-dev`
//! component. Turning it off is what puts the build on a stable toolchain
//! (`docs/nightly-cost.md`), and since ADR-004 D4 such a build is a whole
//! installation rather than a reduced one: the default backend is `rust` here
//! and everywhere.
//!
//! What must not happen is the one ADR-021 D9 names about `cranelift`: a backend
//! flag accepted and quietly served by a different backend. D14 extends that to
//! a backend that was **compiled out**, and requires the two refusals to be
//! distinguishable from the message alone - which is what
//! `the_refusal_says_compiled_out_and_not_unimplemented` holds.
//!
//! The positive side - the bridge actually compiling and running a program - is
//! `bridge_backend.rs`, which compiles only with the feature. The default being
//! `rust` in *both* builds is `backend_default.rs`, which is gated on neither.
#![cfg(not(feature = "rustc-backend"))]

use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("the workspace root")
        .to_path_buf()
}

fn nikaia(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_nikaia"))
        .args(args)
        .current_dir(std::env::temp_dir())
        .output()
        .expect("the nikaia binary runs")
}

/// The flag fails, and the message names the backend and the way to get it.
#[test]
fn the_bridge_backend_says_so_when_it_is_absent() {
    let source = repo_root().join("tests/samples/hello_world.nika");
    let out = nikaia(&[
        "--input",
        source.to_str().expect("sample path"),
        "--backend",
        "bridge",
    ]);

    assert!(
        !out.status.success(),
        "a backend this build does not contain must not report success:\n{}",
        String::from_utf8_lossy(&out.stdout)
    );

    let said = String::from_utf8_lossy(&out.stderr);
    assert!(said.contains("bridge"), "the message must name it: {said}");
    assert!(
        said.contains("rustc-backend"),
        "the message must say what to switch on: {said}"
    );
    assert!(
        said.contains("rustc-dev"),
        "the message must say what to install: {said}"
    );
}

/// ADR-021 D14: *compiled out* and *never implemented* are two refusals, and the
/// message alone has to say which one this is.
///
/// The reader's next move differs - install a component and rebuild, against
/// nothing at all - so a message that read the same for both would send half its
/// readers after a build that does not exist. `backend_default.rs` holds the
/// other side: `cranelift` says nothing implements it and offers no remedy.
#[test]
fn the_refusal_says_compiled_out_and_not_unimplemented() {
    let source = repo_root().join("tests/samples/hello_world.nika");
    let out = nikaia(&[
        "--input",
        source.to_str().expect("sample path"),
        "--backend",
        "bridge",
    ]);

    let said = String::from_utf8_lossy(&out.stderr);
    assert!(
        said.contains("not compiled into"),
        "this refusal is about a backend that exists: {said}"
    );
    assert!(
        !said.contains("not implemented"),
        "and it must not read like the one about a backend nobody wrote: {said}"
    );
    assert!(
        said.contains("rust"),
        "and it must say what this build does have, since `rust` is the default \
         here exactly as it is with the bridge (ADR-004 D4): {said}"
    );
}

/// The backend that does not need the nightly still works here. Without this
/// the test above would pass on a compiler that refused everything.
#[test]
fn the_rust_backend_still_lowers() {
    let source = repo_root().join("tests/samples/hello_world.nika");
    let out = std::env::temp_dir().join("nikaia-backend-absent-hello.rs");
    let _ = std::fs::remove_file(&out);

    let result = nikaia(&[
        "--input",
        source.to_str().expect("sample path"),
        "--backend",
        "rust",
        "-o",
        out.to_str().expect("output path"),
    ]);

    assert!(
        result.status.success(),
        "the `rust` backend must be unaffected:\n{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let emitted = std::fs::read_to_string(&out).expect("the lowered Rust");
    assert!(emitted.contains("fn main()"), "not a program:\n{emitted}");
}
