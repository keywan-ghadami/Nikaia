//! `examples/1brc.nika`, compiled and run.
//!
//! Everything else in this suite checks a piece of the compiler. This checks
//! the claim: the flagship example is lowered to Rust, that Rust is compiled by
//! the same `rustc` that built this test, the binary is run over a small input,
//! and its output is the one the benchmark asks for.
//!
//! The example is the real file, not a copy. It cannot drift.

mod common;

use std::path::PathBuf;
use std::process::Command;

use nikaia::emit::{emit_program, Profile};
use nikaia::parser::parse_to_ast;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn the_1brc_example_compiles_and_prints_what_the_benchmark_asks_for() {
    let source_path = repo_root().join("examples/1brc.nika");
    let source = std::fs::read_to_string(&source_path).expect("examples/1brc.nika");

    // 1. Lower it.
    let parsed = parse_to_ast(&source).expect("the example parses");
    let lowered = emit_program(&parsed, Profile::Advanced).expect("the example lowers");

    let dir = common::scratch_dir("1brc");
    let rust = dir.join("brc.rs");
    std::fs::write(&rust, &lowered.rust).expect("write the emitted Rust");

    // 2. Compile it.
    let binary = dir.join("brc");
    let compile = common::compile(
        &rust,
        &[
            "--crate-type",
            "bin",
            "-o",
            binary.to_str().expect("utf-8 path"),
        ],
    );

    assert!(
        compile.status.success(),
        "the emitted Rust did not compile:\n{}\n--- emitted ---\n{}",
        String::from_utf8_lossy(&compile.stderr),
        lowered.rust
    );

    // 3. Run it over an input small enough to check by hand.
    let measurements = dir.join("measurements.txt");
    std::fs::write(
        &measurements,
        "Hamburg;12.0\nBulawayo;8.9\nHamburg;-3.4\nPalembang;38.8\nBulawayo;22.1\nHamburg;0.0\n",
    )
    .expect("write the input");

    let run = Command::new(&binary)
        .arg(&measurements)
        .output()
        .expect("run the compiled example");

    assert!(
        run.status.success(),
        "the example failed: {}",
        String::from_utf8_lossy(&run.stderr)
    );

    // Sorted by station, min/mean/max to one decimal, the whole line in braces.
    // Hamburg: -3.4, 0.0 and 12.0, so the mean is 8.6/3 = 2.8666… -> 2.9.
    assert_eq!(
        String::from_utf8_lossy(&run.stdout).trim(),
        "{Bulawayo=8.9/15.5/22.1, Hamburg=-3.4/2.9/12.0, Palembang=38.8/38.8/38.8}"
    );

    // 4. And it says so when the file is not there, rather than panicking.
    let missing = Command::new(&binary)
        .arg(dir.join("not-a-file.txt"))
        .output()
        .expect("run the compiled example");
    assert!(missing.status.success());
    assert!(
        String::from_utf8_lossy(&missing.stderr).starts_with("cannot open "),
        "{}",
        String::from_utf8_lossy(&missing.stderr)
    );

    let _ = std::fs::remove_dir_all(&dir);
}
