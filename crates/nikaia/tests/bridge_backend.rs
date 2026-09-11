//! The Bridge-IR backend runs, end to end (ADR-004 D2).
//!
//! D2 says the shipped path is "the executor prints the crate and invokes
//! `rustc` as a subprocess". Nothing ever ran it: `execute` reached
//! `Symbol::intern` with no `rustc_span` session globals installed and
//! panicked in `scoped-tls` before it printed a line, so `--backend bridge` -
//! the **default** backend - failed on every input. It compiled, which is why
//! it survived; `cargo check` cannot tell a lowering that works from one that
//! aborts on its first interned symbol.
//!
//! This is the test that would have caught it, and it asserts the whole path
//! rather than any part of it: `.nika` in, a binary out, and the binary prints
//! what the program says. The subprocess is what makes that assertion cheap -
//! there is no `rustc_private` in this file and nothing here links the
//! compiler's internals.

use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("the workspace root")
        .to_path_buf()
}

/// A scratch directory of this test's own. `--backend bridge` names its
/// outputs after the input stem and writes them beside the working directory,
/// so two tests sharing one would compile over each other.
fn scratch(purpose: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("nikaia-{purpose}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("scratch directory");
    dir
}

#[test]
fn the_bridge_backend_compiles_and_runs_hello_world() {
    let dir = scratch("bridge");
    let source = repo_root().join("tests/samples/hello_world.nika");

    let run = Command::new(env!("CARGO_BIN_EXE_nikaia"))
        .args(["--input", source.to_str().expect("utf-8 path")])
        .args(["--backend", "bridge"])
        .current_dir(&dir)
        .output()
        .expect("the nikaia binary runs");
    assert!(
        run.status.success(),
        "`--backend bridge` failed\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );

    // D2: the text is a *print* of the crate that was compiled, so it is left
    // beside the binary and is what a person reads when the binary is wrong.
    let printed =
        std::fs::read_to_string(dir.join("hello_world.rs")).expect("the printed crate was written");
    assert!(
        printed.contains("println!(\"Hallo Welt, Nikaia!\")"),
        "the printed crate is not the program:\n{printed}"
    );

    let binary = dir.join("hello_world");
    let out = Command::new(&binary)
        .output()
        .expect("the compiled binary runs");
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).trim(),
        "Hallo Welt, Nikaia!"
    );
}

/// The narrow waist refuses rather than guesses (ADR-003 D3).
///
/// `println(name)` is a macro call whose argument is a variable, and
/// `BridgeExpr` has no place to put one. The answer has to be an error naming
/// that, not a panic and not a program with the argument dropped - which is
/// the same standard ADR-021 D9 holds an unimplemented backend to.
#[test]
fn a_program_the_bridge_cannot_carry_is_refused_by_name() {
    let dir = scratch("bridge-refusal");
    let source = repo_root().join("tests/samples/let_assignment.nika");

    let run = Command::new(env!("CARGO_BIN_EXE_nikaia"))
        .args(["--input", source.to_str().expect("utf-8 path")])
        .args(["--backend", "bridge"])
        .current_dir(&dir)
        .output()
        .expect("the nikaia binary runs");

    assert!(
        !run.status.success(),
        "a program the bridge cannot carry compiled"
    );
    let stderr = String::from_utf8_lossy(&run.stderr);
    assert!(
        stderr.contains("println"),
        "the refusal does not name what it could not carry:\n{stderr}"
    );
    assert!(
        !stderr.contains("panicked"),
        "the refusal is a panic:\n{stderr}"
    );
}
