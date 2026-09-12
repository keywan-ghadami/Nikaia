//! A compiler built without the bridge backend says so, and compiles nothing.
//!
//! `rustc-backend` is the one feature that links `rustc_private`, and so the one
//! feature that needs ADR-001 D1's pinned nightly with its `rustc-dev`
//! component. Turning it off is what puts the build on a stable toolchain
//! (`docs/nightly-cost.md`), and the thing that must not happen then is the one
//! ADR-021 D9 names about `cranelift`: a backend flag accepted and quietly
//! served by a different backend. A backend that was configured out is refused
//! exactly like one that was never written.
//!
//! The positive side - the bridge actually compiling and running a program - is
//! `bridge_backend.rs`, which compiles only with the feature.
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
