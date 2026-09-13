//! An arithmetic overflow aborts, at every build.
//!
//! [ADR-043](../../../docs/specification/adr/adr-043.md) D1. The specification
//! said nothing about overflow, and what the silence cost was measurable: the
//! generated project named one profile, `dev`, so a user's program aborted while
//! every benchmark in this repository runs `--release`, where it wraps. The
//! measurements were taken on wrapping arithmetic and the shipped program
//! aborted. Nobody chose that.
//!
//! **Why this file has three tests and not one.** The check lives in the
//! generated `Cargo.toml` (D6) rather than in the emitted code, so no single
//! test sees the whole of it:
//!
//! 1. the emitted Rust aborts when the check is on — the semantics;
//! 2. the manifest this compiler writes carries the check, and carries it
//!    *off* for every dependency — the setting;
//! 3. the two meet, end to end, through Cargo — the seam, and the only one
//!    that needs the network.
//!
//! Without the third the setting could be correct and unapplied; without the
//! second the third would not say why it passed. The first is what holds if
//! somebody ever reaches for a checked helper per operation, which D6 refuses.

mod common;

use std::path::Path;
use std::process::Command;

/// A sum that overflows an `i32`, across a call so that it is not constant.
///
/// Across a call on purpose: rustc refuses a *constant-evaluable* overflow at
/// compile time with "this arithmetic operation will overflow", which is a
/// message about the generated file and a different defect (ADR-043 §3). What is
/// under test here is the run time, so the addition has to be one no lint can
/// see through.
const OVERFLOWS: &str = "\
fn add(a: i32, b: i32) -> i32 {
    return a + b
}

fn main() {
    println(f\"{add(2147483647, 1)}\")
}
";

fn lower(dir: &Path, source: &str) -> String {
    let input = dir.join("main.nika");
    std::fs::write(&input, source).expect("the source");
    let output = dir.join("main.rs");
    let run = Command::new(env!("CARGO_BIN_EXE_nikaia"))
        .args(["--input", input.to_str().expect("utf-8 path")])
        .args(["--output", output.to_str().expect("utf-8 path")])
        .args(["--no-cache"])
        .env("NIKAIA_CACHE_DIR", dir.join("cache"))
        .output()
        .expect("the nikaia binary runs");
    assert!(
        run.status.success(),
        "lowering failed:\n{}{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    std::fs::read_to_string(&output).expect("the emitted Rust")
}

/// **1. The semantics.** With the check on, the emitted program aborts.
///
/// And the emitted Rust says `a + b`, which is the other half of D6: the
/// arithmetic is identical whichever way the check is arranged, but anyone who
/// opens the generated Rust during a benchmark comparison and finds a helper
/// call where `+` should be will not believe the numbers.
#[test]
fn an_overflow_aborts_where_the_check_is_on() {
    let dir = common::scratch_dir("overflow-aborts");
    let rust = lower(&dir, OVERFLOWS);
    assert!(
        rust.contains("a + b"),
        "the emitted code must say `+` (ADR-043 D6):\n{rust}"
    );

    let binary = dir.join("program");
    let compiled = common::compile(
        &dir.join("main.rs"),
        &[
            "--crate-type",
            "bin",
            "-C",
            "overflow-checks=on",
            "-o",
            binary.to_str().expect("utf-8 path"),
        ],
    );
    assert!(
        compiled.status.success(),
        "the emitted Rust did not compile:\n{}",
        String::from_utf8_lossy(&compiled.stderr)
    );

    let ran = Command::new(&binary).output().expect("run the program");
    assert!(
        !ran.status.success(),
        "an overflow must abort, and this program exited cleanly printing {:?}",
        String::from_utf8_lossy(&ran.stdout)
    );
    let said = String::from_utf8_lossy(&ran.stderr);
    assert!(
        said.contains("attempt to add with overflow"),
        "it failed, but not of the overflow:\n{said}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// **2. The setting**, in the manifest this compiler writes for a real project.
///
/// Both halves, because one without the other is not the decision: on for the
/// program, and **off for every dependency**. A hash function in a Rust crate
/// wraps on purpose and is not our code to be right or wrong about, so the rule
/// reaches the program this compiler emits and stops at the crate boundary.
#[test]
fn the_generated_manifest_checks_the_program_and_not_its_dependencies() {
    let dir = common::scratch_dir("overflow-manifest");
    let project = dir.join("src");
    std::fs::create_dir_all(&project).expect("the project directory");
    std::fs::write(dir.join("nikaia.toml"), "[package]\nname = \"ovf\"\n").expect("the manifest");
    std::fs::write(project.join("main.nika"), OVERFLOWS).expect("the source");

    // `--emit-manifest` is not a flag; the manifest is written as part of a
    // build. So this asks for the one thing a build does first and reads what it
    // left behind, which is why it tolerates the build itself failing.
    let _ = Command::new(env!("CARGO_BIN_EXE_nikaia"))
        .args(["build"])
        .current_dir(&dir)
        .env("NIKAIA_CACHE_DIR", dir.join("cache"))
        .env("CARGO_NET_OFFLINE", "true")
        .output()
        .expect("the nikaia binary runs");

    let written = find_manifest(&dir).unwrap_or_else(|| {
        panic!(
            "no generated Cargo.toml under {} - the build wrote none",
            dir.display()
        )
    });
    let text = std::fs::read_to_string(&written).expect("the generated manifest");
    let parsed: toml::Value = toml::from_str(&text).expect("it parses");
    let profile = &parsed["profile"];
    let (name, table) = profile
        .as_table()
        .expect("a profile table")
        .iter()
        .next()
        .expect("one profile");

    assert_eq!(
        table["overflow-checks"].as_bool(),
        Some(true),
        "`[profile.{name}]` must check the program:\n{text}"
    );
    assert_eq!(
        table["package"]["*"]["overflow-checks"].as_bool(),
        Some(false),
        "`[profile.{name}.package.\"*\"]` must not check a dependency:\n{text}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// The generated `Cargo.toml`, wherever the build put it.
fn find_manifest(under: &Path) -> Option<std::path::PathBuf> {
    let mut stack = vec![under.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).ok()?.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.file_name().is_some_and(|n| n == "Cargo.toml") {
                return Some(path);
            }
        }
    }
    None
}

/// **3. The seam.** A `nikaia build` of an overflowing program, run, aborts.
///
/// `#[ignore]`d for the reason `tests/foreign_runtime.rs` gives for the same
/// thing: it runs Cargo, which resolves and fetches `std`'s dependencies. The
/// two tests above are what hold in the fast suite, and between them they cover
/// both sides of this one — what is left is only that the setting is applied,
/// which is the thing a reviewer would most reasonably doubt.
#[test]
#[ignore = "runs cargo and resolves std's dependencies from crates.io"]
fn a_built_program_that_overflows_aborts() {
    let dir = common::scratch_dir("overflow-built");
    let project = dir.join("src");
    std::fs::create_dir_all(&project).expect("the project directory");
    std::fs::write(dir.join("nikaia.toml"), "[package]\nname = \"ovf\"\n").expect("the manifest");
    std::fs::write(project.join("main.nika"), OVERFLOWS).expect("the source");

    let ran = Command::new(env!("CARGO_BIN_EXE_nikaia"))
        .args(["run"])
        .current_dir(&dir)
        .env("NIKAIA_CACHE_DIR", dir.join("cache"))
        .output()
        .expect("the nikaia binary runs");

    let said = String::from_utf8_lossy(&ran.stderr);
    assert!(
        said.contains("attempt to add with overflow"),
        "a built program that overflows must abort:\nstdout: {}\nstderr: {said}",
        String::from_utf8_lossy(&ran.stdout)
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// **Wrapping and saturating arithmetic, run.**
///
/// ADR-043 D2 and D3. These exist because D1 aborts: a hash function wraps on
/// purpose and has to be able to say so. Run rather than read, because what is
/// under test is a number and not a spelling - and the pair with the plain `*`
/// below is the whole point, since the same arithmetic two ways must give two
/// answers.
#[test]
fn wrapping_and_saturating_say_what_the_operator_would_refuse() {
    let dir = common::scratch_dir("overflow-named");
    let rust = lower(
        &dir,
        "fn hash(seed: i32, c: i32) -> i32 {\n    \
             return seed.wrapping_mul(31).wrapping_add(c)\n\
         }\n\
         \n\
         fn clamp(level: i32, rise: i32) -> i32 {\n    \
             return level.saturating_add(rise)\n\
         }\n\
         \n\
         fn shifted(n: i64) -> i64 {\n    \
             return n.wrapping_shl(2)\n\
         }\n\
         \n\
         fn main() {\n    \
             println(f\"{hash(2147483647, 7)} {clamp(2147483647, 5)} {shifted(3)}\")\n\
         }\n",
    );

    let binary = dir.join("program");
    let compiled = common::compile(
        &dir.join("main.rs"),
        &[
            "--crate-type",
            "bin",
            "-C",
            "overflow-checks=on",
            "-o",
            binary.to_str().expect("utf-8 path"),
        ],
    );
    assert!(
        compiled.status.success(),
        "the emitted Rust did not compile:\n{}\n--- emitted ---\n{rust}",
        String::from_utf8_lossy(&compiled.stderr)
    );

    let ran = Command::new(&binary).output().expect("run the program");
    assert!(
        ran.status.success(),
        "none of these may abort, and this one did:\n{}",
        String::from_utf8_lossy(&ran.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&ran.stdout).trim(),
        "2147483624 2147483647 12",
        "wrapped, saturated, shifted"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// And the same multiplication with `*` aborts, which is what makes the names
/// above worth having rather than decoration.
#[test]
fn the_same_arithmetic_with_an_operator_aborts() {
    let dir = common::scratch_dir("overflow-contrast");
    lower(
        &dir,
        "fn hash(seed: i32, c: i32) -> i32 {\n    \
             return seed * 31 + c\n\
         }\n\
         \n\
         fn main() {\n    \
             println(f\"{hash(2147483647, 7)}\")\n\
         }\n",
    );
    let binary = dir.join("program");
    let compiled = common::compile(
        &dir.join("main.rs"),
        &[
            "--crate-type",
            "bin",
            "-C",
            "overflow-checks=on",
            "-o",
            binary.to_str().expect("utf-8 path"),
        ],
    );
    assert!(compiled.status.success());

    let ran = Command::new(&binary).output().expect("run the program");
    let said = String::from_utf8_lossy(&ran.stderr);
    assert!(
        said.contains("attempt to multiply with overflow"),
        "`*` must abort where `wrapping_mul` wraps:\n{said}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
