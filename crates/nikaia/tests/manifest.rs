//! The build switches come from `nikaia.toml`, and a flag overrides them
//! (ADR-037 D5, [ADR-033](../../../docs/specification/adr/adr-033.md) D8).
//!
//! `manifest.rs`'s own unit tests hold the precedence rule. What is checked
//! here is the half those cannot see: that the resolved value actually reaches
//! the emitter, so a committed `ordering = "strict"` is a property of the
//! project and not a comment in a file nothing reads.

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

/// A project directory: a manifest, and a source with two reads that meet on
/// nothing and therefore overlap wherever the analysis is allowed to run.
fn project(purpose: &str, manifest: &str) -> PathBuf {
    let dir = common::scratch_dir(purpose);
    std::fs::write(
        dir.join("nikaia.toml"),
        format!("[package]\nname = \"switches\"\nversion = \"0.1.0\"\n\n{manifest}"),
    )
    .expect("the manifest");
    std::fs::write(
        dir.join("main.nika"),
        "use std::fs\n\
         fn main() throws {\n\
         \x20   let a = fs::read_to_string(\"eins.txt\") catch { \"\".to_string() }\n\
         \x20   let b = fs::read_to_string(\"zwei.txt\") catch { \"\".to_string() }\n\
         \x20   println(f\"{a.len()} {b.len()}\")\n\
         }",
    )
    .expect("the source");
    dir
}

/// Lower the project and hand back the emitted Rust.
fn lower(dir: &Path, flags: &[&str]) -> String {
    let output = dir.join("out.rs");
    let run = Command::new(env!("CARGO_BIN_EXE_nikaia"))
        .args(["--input", dir.join("main.nika").to_str().unwrap()])
        .args(["--backend", "rust"])
        .args(["--output", output.to_str().unwrap()])
        .args(["--no-cache"])
        .args(flags)
        .output()
        .expect("the nikaia binary runs");
    assert!(
        run.status.success(),
        "lowering failed\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    std::fs::read_to_string(&output).expect("the emitted Rust")
}

/// `user-parallelism = "yes"` in the manifest is what makes the pair overlap.
/// Nothing on the command line says so, which is the point: a switch nobody
/// types is a property of the project (ADR-037 D5).
#[test]
fn the_manifest_decides_the_switches() {
    let dir = project("manifest-decides", "[build]\nuser-parallelism = \"yes\"\n");
    assert!(
        lower(&dir, &[]).contains("task::both"),
        "the manifest's `yes` did not reach the emitter"
    );

    // …and the default really is the other way, or the assertion above would
    // hold for a program that overlaps whatever anyone writes.
    let plain = project("manifest-absent", "");
    assert!(
        !lower(&plain, &[]).contains("task::both"),
        "a project with no `[build]` table must stay at `user_parallelism = no`"
    );
}

/// The flag is for one build - a benchmark, a bug hunt - so it wins over the
/// committed value, in both directions.
#[test]
fn a_flag_overrides_a_committed_switch() {
    let parallel = project("flag-over-yes", "[build]\nuser-parallelism = \"yes\"\n");
    assert!(
        !lower(&parallel, &["--user-parallelism", "no"]).contains("task::both"),
        "`--user-parallelism no` did not override the manifest"
    );

    let sequential = project("flag-over-no", "[build]\nuser-parallelism = \"no\"\n");
    assert!(
        lower(&sequential, &["--user-parallelism", "yes"]).contains("task::both"),
        "`--user-parallelism yes` did not override the manifest"
    );
}

/// `ordering` is the other switch that lived only on the CLI. It is the escape
/// for a project that does not want the analysis, so it has to be committable.
#[test]
fn ordering_is_a_project_setting_too() {
    let dir = project(
        "manifest-ordering",
        "[build]\nuser-parallelism = \"yes\"\nordering = \"strict\"\n",
    );
    let strict = lower(&dir, &[]);
    assert!(
        !strict.contains("task::both"),
        "the manifest's `strict` did not reach the emitter"
    );
    assert_eq!(
        strict,
        lower(&dir, &["--ordering", "strict"]),
        "and it is the same program the flag produces"
    );
    assert!(
        lower(&dir, &["--ordering", "effects"]).contains("task::both"),
        "`--ordering effects` did not override the manifest"
    );
}

/// A switch misspelled in the manifest must fail the build, not be ignored.
///
/// The trap it closes: the manifest spells it `user-parallelism` and the flag
/// `--user-parallelism`, so an underscore is the mistake a person actually
/// makes - and a silently ignored key would leave the build at a default the
/// author believed they had changed.
#[test]
fn a_misspelled_key_fails_the_build_and_names_itself() {
    let dir = project("manifest-typo", "[build]\nuser_parallelism = \"yes\"\n");
    let run = Command::new(env!("CARGO_BIN_EXE_nikaia"))
        .args(["--input", dir.join("main.nika").to_str().unwrap()])
        .args(["--backend", "rust"])
        .output()
        .expect("the nikaia binary runs");

    assert!(!run.status.success(), "an unknown `[build]` key must fail");
    let stderr = String::from_utf8_lossy(&run.stderr);
    assert!(
        stderr.contains("user_parallelism") && stderr.contains("nikaia.toml"),
        "the refusal must name the key and the file: {stderr}"
    );
}

/// And a value the switch cannot make sense of is refused by the switch, with
/// the switch's own reason rather than a type error about TOML.
#[test]
fn a_count_in_the_manifest_gets_the_same_answer_as_a_count_on_the_cli() {
    let dir = project("manifest-count", "[build]\nuser-parallelism = 4\n");
    let run = Command::new(env!("CARGO_BIN_EXE_nikaia"))
        .args(["--input", dir.join("main.nika").to_str().unwrap()])
        .args(["--backend", "rust"])
        .output()
        .expect("the nikaia binary runs");

    assert!(!run.status.success(), "a count must fail");
    let stderr = String::from_utf8_lossy(&run.stderr);
    assert!(
        stderr.contains("not a count"),
        "the reason belongs to the switch, not to the parser: {stderr}"
    );
}
