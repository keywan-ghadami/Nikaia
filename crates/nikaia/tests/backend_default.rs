//! What a bare `nikaia --input x.nika` does, and what it must keep doing
//! (ADR-004 D4).
//!
//! The default backend is `rust`, the Stage 0 transpiler. It is the backend
//! that carries the corpus, and the only one every build contains — `bridge` is
//! what links `rustc_private`, so a build that has it is a build tied to one
//! nightly at one path.
//!
//! This file has **no `#![cfg]`**, on purpose: the default is the same word in
//! both builds, and a test of it that only ran in one of them would be a test of
//! the feature rather than of the default. The half that can only be checked
//! where the bridge is absent — that the refusal names the feature and the
//! component — is `backend_absent.rs`; the half that needs the bridge present is
//! `bridge_backend.rs`.

mod common;

use std::path::PathBuf;
use std::process::{Command, Output};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn sample() -> PathBuf {
    repo_root().join("tests/samples/hello_world.nika")
}

fn nikaia(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_nikaia"))
        .args(args)
        .output()
        .expect("the nikaia binary runs")
}

/// The whole of ADR-004 D4, as a command line: no `--backend`, and Rust comes
/// out. Before D4 this invocation went to the bridge backend, which could carry
/// one program in the corpus and needed the pinned nightly to exist at all.
#[test]
fn a_bare_invocation_lowers_to_rust() {
    let dir = common::scratch_dir("backend-default-bare");
    let out = dir.join("hello.rs");

    let run = nikaia(&[
        "--input",
        sample().to_str().expect("sample path"),
        "--output",
        out.to_str().expect("output path"),
    ]);

    assert!(
        run.status.success(),
        "a bare invocation must compile:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    let emitted = std::fs::read_to_string(&out).expect("the lowered Rust");
    assert!(emitted.contains("fn main()"), "not a program:\n{emitted}");

    std::fs::remove_dir_all(&dir).ok();
}

/// And it is the *same* backend, not a second route to a similar answer: the
/// default and the explicit flag produce identical bytes, emitted Rust and
/// ledger alike.
#[test]
fn the_default_is_the_rust_backend_and_not_a_lookalike() {
    let dir = common::scratch_dir("backend-default-agrees");
    let implied = dir.join("implied/out.rs");
    let explicit = dir.join("explicit/out.rs");
    std::fs::create_dir_all(implied.parent().expect("dir")).expect("dirs");
    std::fs::create_dir_all(explicit.parent().expect("dir")).expect("dirs");

    let path = sample();
    let run = |output: &PathBuf, extra: &[&str]| {
        let mut args = vec![
            "--input",
            path.to_str().expect("sample path"),
            "--output",
            output.to_str().expect("output path"),
        ];
        args.extend_from_slice(extra);
        let out = nikaia(&args);
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    };

    run(&implied, &[]);
    run(&explicit, &["--backend", "rust"]);

    assert_eq!(
        std::fs::read_to_string(&implied).expect("implied Rust"),
        std::fs::read_to_string(&explicit).expect("explicit Rust"),
        "the default must be the `rust` backend itself, byte for byte"
    );
    assert_eq!(
        std::fs::read_to_string(implied.with_file_name("nikaia.contracts"))
            .expect("implied ledger"),
        std::fs::read_to_string(explicit.with_file_name("nikaia.contracts"))
            .expect("explicit ledger"),
        "and the ledger it writes beside it (ADR-020) comes from the same lowering"
    );

    std::fs::remove_dir_all(&dir).ok();
}

/// ADR-021 D5 makes the backend a dimension of the cache key, so the question
/// D4 raises is whether the default change forks the store: two spellings of one
/// lowering filling two entries, or worse, one entry answering for the other
/// backend. It is one entry, because the cache records the lowering that made the
/// artifact and the bridge backend consults no cache at all.
#[test]
fn the_default_and_the_explicit_flag_share_one_cache_entry() {
    let dir = common::scratch_dir("backend-default-cache");
    let src = dir.join("src");
    let home = dir.join("cache");
    std::fs::create_dir_all(&src).expect("dirs");
    let input = src.join("hello.nika");
    std::fs::copy(sample(), &input).expect("sample");

    let run = |extra: &[&str]| {
        let mut args = vec!["--input", input.to_str().expect("input path")];
        args.extend_from_slice(extra);
        let out = Command::new(env!("CARGO_BIN_EXE_nikaia"))
            .args(&args)
            .env("NIKAIA_CACHE_DIR", &home)
            .output()
            .expect("the nikaia binary runs");
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).into_owned()
    };

    assert!(
        !run(&[]).contains("from cache"),
        "the first run, with no --backend, has to lower"
    );
    assert!(
        run(&["--backend", "rust"]).contains("from cache"),
        "and naming the backend the default already chose must hit that entry, \
         not open a second one"
    );

    std::fs::remove_dir_all(&dir).ok();
}

/// The flags that explain a decision rather than changing one live in the `rust`
/// backend, so before D4 a bare `nikaia --input x.nika --overlaps` went to the
/// bridge and the question was dropped on the floor. Part I 8.1.1 writes that
/// command line without a `--backend`; this is what makes the specification's
/// own line true.
#[test]
fn the_explanations_answer_without_being_told_which_backend() {
    let dir = common::scratch_dir("backend-default-explains");
    let out = dir.join("hello.rs");
    let path = sample();

    let overlaps = nikaia(&[
        "--input",
        path.to_str().expect("sample path"),
        "--output",
        out.to_str().expect("output path"),
        "--overlaps",
    ]);
    assert!(
        overlaps.status.success(),
        "{}",
        String::from_utf8_lossy(&overlaps.stderr)
    );
    let said = String::from_utf8_lossy(&overlaps.stdout);
    assert!(
        said.contains("adjacent"),
        "`--overlaps` must report on the pairs, not be silently dropped: {said}"
    );

    let trust = nikaia(&[
        "--input",
        path.to_str().expect("sample path"),
        "--output",
        out.to_str().expect("output path"),
        "--trust",
    ]);
    assert!(
        trust.status.success(),
        "{}",
        String::from_utf8_lossy(&trust.stderr)
    );
    assert!(
        String::from_utf8_lossy(&trust.stdout).contains("provenance"),
        "`--trust` must answer too: {}",
        String::from_utf8_lossy(&trust.stdout)
    );

    std::fs::remove_dir_all(&dir).ok();
}

/// ADR-021 D14: the two refusals are different refusals, and a reader has to be
/// able to tell which one they got from the message alone.
///
/// `cranelift` was never written, so there is nothing to install and the message
/// offers nothing — naming a feature or a toolchain component here would send a
/// reader after a build that does not exist. The other half, the backend that
/// *does* exist and was compiled out, is asserted in `backend_absent.rs`, which
/// is the only build where it can be reached.
#[test]
fn a_backend_that_was_never_written_refuses_differently() {
    for backend in ["cranelift", "llvm"] {
        let run = nikaia(&[
            "--input",
            sample().to_str().expect("sample path"),
            "--backend",
            backend,
        ]);

        assert!(
            !run.status.success(),
            "`--backend {backend}` must not report success"
        );
        let said = String::from_utf8_lossy(&run.stderr);
        assert!(said.contains(backend), "the message must name it: {said}");
        assert!(
            said.contains("not implemented"),
            "and say that nothing implements it: {said}"
        );
        assert!(
            !said.contains("rustc-dev") && !said.contains("rustc-backend"),
            "it must offer no remedy, because there is none - that wording \
             belongs to the backend that exists and was compiled out: {said}"
        );
    }
}
