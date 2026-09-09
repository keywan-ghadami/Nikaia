//! What the compiler says about input that is wrong.
//!
//! `tests/errors/` holds one broken `.nika` file per case, and
//! `tests/errors/EXPECTED.txt` holds what each produces today - wrong messages
//! included. That is the point. A message is only wrong against a claim about
//! what a reader needed; those claims are in `docs/error-corpus.md`, and this
//! file is what turns a change to them into a diff rather than a memory.
//!
//! It exists because three typos measured by hand were not enough: an attempt
//! at the expectation ranking made one case much worse - seventeen
//! expectations in the headline, the position on a token that was correct -
//! and nothing caught it.

use nikaia::parser::parse_to_ast;
use std::path::{Path, PathBuf};

fn errors_dir() -> PathBuf {
    // CARGO_MANIFEST_DIR is crates/nikaia; the corpus lives at the repo root,
    // beside tests/samples, which `samples.rs` walks the same way.
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/errors")
        .canonicalize()
        .expect("errors directory")
}

/// The walk `examples/errors.rs` prints. Kept in both because an example is a
/// binary a test cannot call into - the same split `dump.rs` already has with
/// `grammar_lowering.rs`.
fn render_corpus(dir: &Path) -> String {
    let mut cases: Vec<PathBuf> = std::fs::read_dir(dir)
        .expect("read errors dir")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("nika"))
        .collect();
    cases.sort();

    let mut out = String::new();
    for path in cases {
        let name = path.file_stem().expect("stem").to_string_lossy();
        let source = std::fs::read_to_string(&path).expect("read case");
        out.push_str(&format!("=== {name}\n"));
        match parse_to_ast(&source) {
            Ok(_) => out.push_str("(parses)\n"),
            Err(e) => {
                out.push_str(&e.to_string());
                out.push('\n');
            }
        }
        out.push('\n');
    }
    out
}

#[test]
fn the_messages_are_the_ones_in_expected_txt() {
    let dir = errors_dir();
    let expected = std::fs::read_to_string(dir.join("EXPECTED.txt")).expect("EXPECTED.txt");

    assert_eq!(
        render_corpus(&dir),
        expected,
        "the corpus and tests/errors/EXPECTED.txt have drifted apart. If that \
         is the change you meant, regenerate it with `cargo run -p nikaia \
         --example errors > tests/errors/EXPECTED.txt` and read the diff - \
         every row of docs/error-corpus.md is in it."
    );
}

#[test]
fn the_corpus_is_not_empty() {
    // A walker over a directory that has become empty passes every assertion
    // above without noticing.
    let dir = errors_dir();
    let n = std::fs::read_dir(&dir)
        .expect("read errors dir")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("nika"))
        .count();
    assert!(n >= 20, "only {n} cases in {}", dir.display());
}
