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

/// The printed crate is compiled at the edition it was printed for.
///
/// `execute` passed no `--edition`, so `rustc` compiled at **2015** while the
/// crate had been interned and printed for 2021 - and 2021 is what the emitter
/// writes (`crates/nikaia/src/emit/mod.rs`) and what every test of the `rust`
/// backend compiles (`crates/nikaia/tests/common/mod.rs` passes
/// `--edition 2021`). A mismatch rather than a choice, found while measuring
/// ADR-004 D3 (`docs/subprocess-cost.md` §5) and closed here.
///
/// What this asserts is the two halves of that claim at once. The program the
/// bridge carries still runs and still prints what it printed: the change moved
/// nothing observable, which is what made it safe to make. And the *reason* it
/// moved nothing is that nothing Bridge-IR can express tells 2015 and 2021
/// apart - so the same printed text is compiled at both editions and the two
/// binaries are compared. The day that assertion fails is the day the bridge
/// grew something edition-sensitive, and the edition stops being a formality;
/// failing here is the notice.
#[test]
fn the_printed_crate_is_compiled_at_the_edition_it_was_printed_for() {
    let dir = scratch("bridge-edition");
    let source = repo_root().join("tests/samples/hello_world.nika");

    let run = Command::new(env!("CARGO_BIN_EXE_nikaia"))
        .args(["--input", source.to_str().expect("utf-8 path")])
        .args(["--backend", "bridge"])
        .current_dir(&dir)
        .output()
        .expect("the nikaia binary runs");
    assert!(
        run.status.success(),
        "`--backend bridge` failed\nstderr: {}",
        String::from_utf8_lossy(&run.stderr)
    );

    let printed = dir.join("hello_world.rs");
    let out = Command::new(dir.join("hello_world"))
        .output()
        .expect("the compiled binary runs");
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).trim(),
        "Hallo Welt, Nikaia!",
        "the program the bridge carries still prints what it printed"
    );

    // The same text, compiled twice by the pinned `rustc`, once at each
    // edition. Both must build and both must print the same thing.
    let mut binaries = Vec::new();
    for edition in ["2015", "2021"] {
        let binary = dir.join(format!("hello_world-{edition}"));
        let built = Command::new(env!("NIKAIA_RUSTC"))
            .args(["--edition", edition])
            .arg(&printed)
            .arg("-o")
            .arg(&binary)
            .output()
            .expect("run rustc");
        assert!(
            built.status.success(),
            "the printed crate does not compile at edition {edition}:\n{}",
            String::from_utf8_lossy(&built.stderr)
        );
        let ran = Command::new(&binary).output().expect("the binary runs");
        binaries.push(String::from_utf8_lossy(&ran.stdout).trim().to_string());
    }

    assert_eq!(
        binaries[0], binaries[1],
        "the bridge's output is edition-sensitive now, so `--edition` in \
         `rustc-executor` is no longer a formality - check what changed before \
         relaxing this"
    );
    assert_eq!(binaries[1], "Hallo Welt, Nikaia!");
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
