//! The measurement loop.
//!
//! `docs/staging-candidates.md` §5 records that the benchmark *programs* exist
//! and the harness does not, and §6 ends on the rule this file serves: **a
//! staging decision enters the compiler only together with a measured
//! crossover.** This is where a crossover is measured.
//!
//! Instructions retired, under callgrind, on the same tree - which is how
//! ADR-010's hasher was measured (ADR-011 §4) and is the only kind of number
//! this repository has ever accepted. Wall-clock on a shared machine is not a
//! measurement; an instruction count is deterministic and diffable.
//!
//! **Ignored by default**, because a `cargo test` that shells out to valgrind
//! is not a test suite. Run them on purpose:
//!
//! ```text
//! cargo test -p nikaia --test measure -- --ignored --nocapture
//! ```
//!
//! Each test prints a table and asserts only what it is *sure* of - usually
//! that the change did not make things worse. A number that has to be argued
//! about belongs in the output, not in an assertion.

mod common;

use std::path::PathBuf;
use std::process::Command;

use nikaia::emit::{emit_program, Build};
use nikaia::parser::parse_to_ast;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// One variant of a program, and what it cost.
struct Run {
    label: &'static str,
    instructions: u64,
}

/// Lower a `.nika` benchmark to Rust.
fn lower(file: &str) -> String {
    let path = repo_root().join("benches").join(file);
    let source =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let parsed = parse_to_ast(&source).unwrap_or_else(|e| panic!("{file} does not parse:\n{e}"));
    emit_program(&parsed, Build::default())
        .unwrap_or_else(|e| panic!("{file} does not lower:\n{e}"))
        .rust
}

/// Compile a Rust program and count the instructions one run of it retires.
///
/// `-O`, because an unoptimised binary measures the optimiser's absence rather
/// than the change: every one of these candidates is about work `rustc` is
/// already allowed to remove, and a number taken without it can only mislead.
fn instructions(label: &'static str, rust: &str, args: &[&str]) -> Run {
    let dir = common::scratch_dir(&format!("measure-{}", label.replace([' ', ':'], "-")));
    let source = dir.join("bench.rs");
    std::fs::write(&source, rust).expect("write the Rust");

    let binary = dir.join("bench");
    let compiled = common::compile(
        &source,
        &[
            "--crate-type",
            "bin",
            "-O",
            "-o",
            binary.to_str().expect("utf-8 path"),
        ],
    );
    assert!(
        compiled.status.success(),
        "{label} did not compile:\n{}",
        String::from_utf8_lossy(&compiled.stderr)
    );

    let run = Command::new("valgrind")
        .args([
            "--tool=callgrind",
            "--callgrind-out-file=/dev/null",
            binary.to_str().expect("utf-8 path"),
        ])
        .args(args)
        .output()
        .expect("run under callgrind - is valgrind installed?");
    assert!(run.status.success(), "{label} failed under callgrind");

    // callgrind writes `==pid== I   refs:      1,234,567` to stderr.
    let stderr = String::from_utf8_lossy(&run.stderr);
    let instructions = stderr
        .lines()
        .find_map(|line| line.split("I   refs:").nth(1))
        .map(|n| n.trim().replace(',', ""))
        .and_then(|n| n.parse::<u64>().ok())
        .unwrap_or_else(|| panic!("no instruction count in callgrind's output:\n{stderr}"));

    let _ = std::fs::remove_dir_all(&dir);
    Run {
        label,
        instructions,
    }
}

/// Print what was measured, in the shape the ADRs quote.
fn report(what: &str, workload: &str, runs: &[Run]) {
    println!("\n{what} - {workload}");
    let baseline = runs.first().expect("at least one run").instructions;
    for run in runs {
        let delta = if run.instructions == baseline {
            "baseline".to_string()
        } else {
            let change = run.instructions as f64 / baseline as f64 - 1.0;
            format!("{:+.1}%", change * 100.0)
        };
        println!(
            "  {:<28} {:>14} {}",
            run.label,
            thousands(run.instructions),
            delta
        );
    }
}

fn thousands(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// The template's `String::new()` against a `String::with_capacity(…)`.
///
/// `docs/staging-candidates.md` §3 names this as the smallest candidate and
/// says the honest part out loud: it is a small win, and saying so afterwards
/// would look like an excuse. So it is said here, before the number.
///
/// The A/B is one tree and one emitter: the "before" variant is the emitted
/// Rust with the capacity taken back out, which is exactly what the emitter
/// used to write.
#[test]
#[ignore = "shells out to valgrind; run with --ignored"]
fn a_template_reserving_its_length_against_one_that_does_not() {
    let with_capacity = lower("template.nika");
    assert!(
        with_capacity.contains("String::with_capacity("),
        "the emitter does not reserve; there is nothing to compare:\n{with_capacity}"
    );

    // The emitter before the change: `String::new()`, and the same code after.
    let plain = strip_capacity(&with_capacity);

    for rows in ["200", "2000", "20000"] {
        let runs = [
            instructions("String::new()", &plain, &[rows]),
            instructions("String::with_capacity", &with_capacity, &[rows]),
        ];
        report("one growing table", &format!("{rows} rows"), &runs);
    }

    // The other shape, and the one the reservation was proposed for: mostly
    // static markup, rendered many times. Here the compiler knows almost the
    // whole answer rather than a few dozen bytes of it.
    let with_capacity = lower("page.nika");
    let plain = strip_capacity(&with_capacity);
    for pages in ["200", "2000", "20000"] {
        let runs = [
            instructions("String::new()", &plain, &[pages]),
            instructions("String::with_capacity", &with_capacity, &[pages]),
        ];
        report("a static page, rendered", &format!("{pages} times"), &runs);
    }
}

/// `String::with_capacity(N)` back to `String::new()`, and nothing else.
fn strip_capacity(rust: &str) -> String {
    let mut out = String::with_capacity(rust.len());
    let mut rest = rust;
    while let Some(at) = rest.find("String::with_capacity(") {
        out.push_str(&rest[..at]);
        out.push_str("String::new()");
        let after = &rest[at + "String::with_capacity(".len()..];
        let close = after.find(')').expect("a call has a closing paren");
        rest = &after[close + 1..];
    }
    out.push_str(rest);
    out
}

/// What escaping costs, on the workload a template puts through it.
///
/// A `std` change cannot be A/B'd by rewriting the emitted Rust the way the
/// template's reservation can - the code being measured is behind a crate
/// boundary. So this prints one number and the comparison is made the way
/// ADR-010's hasher was: the same tree, built twice, once with the patch. The
/// numbers that comparison produced are in `docs/staging-candidates.md` §3.
///
/// `benches/template.nika` is the right workload for it: two holes per row,
/// one that needs escaping and one that does not, so both paths through
/// `html::escape` are on the build.
#[test]
#[ignore = "shells out to valgrind; run with --ignored"]
fn what_escaping_costs() {
    let rust = lower("template.nika");
    let runs = [instructions("html::escape", &rust, &["20000"])];
    report("escaping", "20000 rows, 40000 holes", &runs);
}
