//! An error about the emitted Rust, reported on the Nikaia line that caused it.
//!
//! This is the whole pipeline, not a simulation of it: the fixture is lowered,
//! the emitted Rust is handed to the same `rustc` that built this test, and the
//! JSON diagnostics that come back - including the parser backend's frame
//! check, which runs inside a proc macro - are mapped through the emitter's
//! source map onto the `.nika` file.

use std::path::PathBuf;
use std::process::Command;

use nikaia::diagnostics::{self, Diagnostic};
use nikaia::emit::{emit_program, Lowered, Profile};
use nikaia::parser::parse_to_ast;

const BROKEN: &str = include_str!("fixtures/broken_frame.nika");
/// Self-contained on purpose: it declares every name it uses, so what it
/// compiles to has nothing to complain about.
const GOOD: &str = include_str!("fixtures/digits.nika");

fn lower(source: &str) -> Lowered {
    let parsed = parse_to_ast(source).expect("the fixture parses");
    emit_program(&parsed, Profile::Advanced).expect("the fixture lowers")
}

/// Where cargo put the crates this test links against - which is also where the
/// emitted Rust has to find `winnow_grammar`.
fn deps_dir() -> PathBuf {
    std::env::current_exe()
        .expect("test binary path")
        .parent()
        .expect("deps directory")
        .to_path_buf()
}

fn rlib(crate_name: &str) -> PathBuf {
    let prefix = format!("lib{crate_name}-");
    let mut candidates: Vec<_> = std::fs::read_dir(deps_dir())
        .expect("read deps directory")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default();
            name.starts_with(&prefix) && name.ends_with(".rlib")
        })
        .collect();

    // Stale builds leave older hashes behind; the newest is the one this test
    // was linked against.
    candidates.sort_by_key(|path| std::fs::metadata(path).and_then(|m| m.modified()).ok());
    candidates
        .pop()
        .unwrap_or_else(|| panic!("no {crate_name} rlib in {}", deps_dir().display()))
}

/// Compile emitted Rust and return rustc's JSON diagnostics.
///
/// `--emit=metadata` is enough: the frame check runs during macro expansion, so
/// there is no reason to pay for code generation to hear about it.
fn compile(rust: &str) -> String {
    // One directory per call: the tests run in parallel threads of one process,
    // and two of them sharing a file name means one compiles the other's code.
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let id = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("nikaia-diagnostics-{}-{id}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("scratch directory");
    let source = dir.join("lowered.rs");
    std::fs::write(&source, rust).expect("write emitted Rust");

    let output = Command::new(env!("NIKAIA_RUSTC"))
        .args([
            "--edition",
            "2021",
            "--crate-type",
            "lib",
            "--emit=metadata",
        ])
        .args(["--error-format", "json"])
        .arg("-L")
        .arg(format!("dependency={}", deps_dir().display()))
        // `grammar!` expands to `::winnow::…` as well as `::winnow_grammar::…`,
        // so both have to be named for a file compiled outside cargo.
        .arg("--extern")
        .arg(format!(
            "winnow_grammar={}",
            rlib("winnow_grammar").display()
        ))
        .arg("--extern")
        .arg(format!("winnow={}", rlib("winnow").display()))
        .arg(&source)
        .arg("-o")
        .arg(dir.join("lowered.rmeta"))
        .output()
        .expect("run rustc");

    let _ = std::fs::remove_dir_all(&dir);
    String::from_utf8(output.stderr).expect("rustc writes utf-8")
}

fn errors_for(source: &str) -> Vec<Diagnostic> {
    let lowered = lower(source);
    let json = compile(&lowered.rust);
    let all = diagnostics::translate(&json, &lowered.map, source);

    assert!(
        !json.trim().is_empty() || all.is_empty(),
        "rustc said nothing at all"
    );
    all.into_iter().filter(|d| d.level == "error").collect()
}

/// The 1-based line a piece of the fixture is on, so the assertions below name
/// the code rather than a number that shifts when a comment is edited.
fn line_of(source: &str, needle: &str) -> usize {
    source
        .lines()
        .position(|line| line.contains(needle))
        .unwrap_or_else(|| panic!("`{needle}` is not in the fixture"))
        + 1
}

#[test]
fn a_rejected_frame_is_reported_on_the_nika_line_that_caused_it() {
    let errors = errors_for(BROKEN);

    assert_eq!(
        errors.len(),
        1,
        "expected exactly the frame check to fail: {errors:#?}"
    );
    let error = &errors[0];

    // The backend's message is already about the Nikaia source, because the
    // lowering is name for name (ADR-011 D2) - `NAME` and `MEASUREMENT` are
    // the user's own rule names. Only the position had to be corrected.
    assert!(
        error.message.contains("until"),
        "unexpected message: {}",
        error.message
    );
    assert!(
        error.message.contains("NAME") && error.message.contains("MEASUREMENT"),
        "the message does not name the rules: {}",
        error.message
    );

    let location = error
        .location
        .as_ref()
        .expect("the frame error maps back to the source");

    // `s:until(";")` - the pattern that cannot be cut at, not the rule, not the
    // grammar, and above all not a line of generated Rust.
    assert_eq!(location.line, line_of(BROKEN, "s:until(\";\")"));
    assert_eq!(
        &BROKEN[location.span.clone()],
        "until(\";\")",
        "the span should cover the offending pattern"
    );

    // It really was reported somewhere else first.
    assert!(
        error
            .generated_line
            .is_some_and(|line| line != location.line),
        "the emitted file should have blamed a different line"
    );
}

#[test]
fn the_rendered_message_shows_the_nika_line() {
    let errors = errors_for(BROKEN);
    let rendered = diagnostics::render(&errors[0], "broken_frame.nika", BROKEN, "lowered.rs");

    assert!(
        rendered.starts_with("error: broken_frame.nika:15:"),
        "{rendered}"
    );
    assert!(rendered.contains("s:until(\";\")"), "{rendered}");
    assert!(rendered.contains("^^^^^"), "{rendered}");
}

#[test]
fn a_grammar_the_backend_accepts_reports_nothing() {
    // The counterpart, and the one that would catch a source map that maps
    // everything to line 1: a fixture that compiles has nothing to translate.
    let errors = errors_for(GOOD);
    assert!(errors.is_empty(), "{errors:#?}");
}

#[test]
fn the_map_leads_from_emitted_code_back_to_what_wrote_it() {
    // The map is what everything above rests on, so it is worth checking
    // directly rather than only through a diagnostic.
    let lowered = lower(BROKEN);
    assert!(!lowered.map.is_empty());

    let attribute = lowered.rust.find("#[frame(").expect("the frame attribute");
    let span = lowered
        .map
        .source_span(attribute)
        .expect("the attribute maps back");
    assert!(
        BROKEN[span].starts_with("@frame(boundary: \"\\n\")"),
        "the attribute should lead back to the rule that carries it"
    );
}

#[test]
fn an_unmapped_diagnostic_says_so_rather_than_guessing() {
    // Nothing in the preamble comes from the source, so an error reported
    // against it has no Nikaia line - and inventing one would be worse than
    // admitting it.
    let lowered = lower(GOOD);
    let json = r#"{"$message_type":"diagnostic","message":"something about the preamble","level":"error","spans":[{"file_name":"lowered.rs","byte_start":0,"byte_end":4,"line_start":1,"column_start":1,"is_primary":true}],"children":[]}"#;

    let translated = diagnostics::translate(json, &lowered.map, GOOD);
    assert_eq!(translated.len(), 1);
    assert!(translated[0].location.is_none());

    let rendered = diagnostics::render(&translated[0], "digits.nika", GOOD, "lowered.rs");
    assert!(rendered.contains("lowered.rs:1"), "{rendered}");
    assert!(
        rendered.contains("no Nikaia source maps to this"),
        "{rendered}"
    );
}
