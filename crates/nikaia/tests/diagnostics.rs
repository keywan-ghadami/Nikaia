//! An error about the emitted Rust, reported on the Nikaia line that caused it.
//!
//! This is the whole pipeline, not a simulation of it: the fixture is lowered,
//! the emitted Rust is handed to the same `rustc` that built this test, and the
//! JSON diagnostics that come back - including the parser backend's frame
//! check, which runs inside a proc macro - are mapped through the emitter's
//! source map onto the `.nika` file.

mod common;

use nikaia::diagnostics::{self, Diagnostic};
use nikaia::emit::{emit_program, Build, Lowered};
use nikaia::parser::parse_to_ast;

const BROKEN: &str = include_str!("fixtures/broken_frame.nika");
/// Self-contained on purpose: it declares every name it uses, so what it
/// compiles to has nothing to complain about.
const GOOD: &str = include_str!("fixtures/digits.nika");

fn lower(source: &str) -> Lowered {
    let parsed = parse_to_ast(source).expect("the fixture parses");
    emit_program(&parsed, Build::default()).expect("the fixture lowers")
}

/// Compile emitted Rust and return rustc's JSON diagnostics.
///
/// `--emit=metadata` is enough: the frame check runs during macro expansion, so
/// there is no reason to pay for code generation to hear about it.
fn compile(rust: &str) -> String {
    let dir = common::scratch_dir("diagnostics");
    let source = dir.join("lowered.rs");
    std::fs::write(&source, rust).expect("write emitted Rust");

    let output = common::compile(
        &source,
        &[
            "--crate-type",
            "lib",
            "--emit=metadata",
            "--error-format",
            "json",
            "-o",
            dir.join("lowered.rmeta").to_str().expect("utf-8 path"),
        ],
    );

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

/// What **`cargo`** writes is read by the same function, and it is the project
/// build's only channel (ADR-005 D7, Part III C.1).
///
/// `cargo --message-format=json` wraps each `rustc` diagnostic in a line of its
/// own and mixes in lines of its own that are not diagnostics at all. Both have
/// to be handled here, because until they were, `nikaia build` handed Cargo's
/// stderr to the terminal and every backend error reached the user as Rust about
/// a generated file.
///
/// The diagnostic below is an `E0277`, which is the class ADR-005 D7 was short
/// of: `Send`, `sync` and every future marker rule surfaces as a trait bound,
/// and D7's seven were all borrow, ownership or lifetime errors.
#[test]
fn a_trait_bound_error_from_cargo_is_placed_in_the_nika_file() {
    let lowered = lower(GOOD);
    // A byte the map knows. Which byte is the emitter's business, so it is
    // found rather than guessed: the first one the map covers at all.
    let offset = (0..lowered.rust.len())
        .find(|at| lowered.map.source_span(*at).is_some())
        .expect("the map covers some of what was emitted");

    let end = offset + 7;
    let message = format!(
        r#"{{"reason":"compiler-message","message":{{"message":"`Rc<String>` cannot be sent between threads safely","code":{{"code":"E0277"}},"level":"error","spans":[{{"file_name":"digits.rs","byte_start":{offset},"byte_end":{end},"line_start":7,"column_start":5,"is_primary":true}}],"children":[{{"level":"help","message":"within `LocalHandle`, the trait `Send` is not implemented for `Rc<String>`"}}]}}}}"#
    );
    let json = format!(
        "{}\n{message}\n{}\n",
        r#"{"reason":"compiler-artifact","target":{"name":"digits"}}"#,
        r#"{"reason":"build-finished","success":false}"#,
    );

    let translated = diagnostics::translate_units(&json, &lowered.map, &[GOOD]);
    assert_eq!(
        translated.len(),
        1,
        "Cargo's own lines carry no diagnostic: {translated:#?}"
    );
    let placed = translated[0]
        .location
        .as_ref()
        .expect("the map knows where the entry came from");
    assert_eq!(placed.unit, 0);

    let rendered = diagnostics::render(&translated[0], "digits.nika", GOOD, "digits.rs");
    assert!(rendered.starts_with("error: digits.nika:"), "{rendered}");
    assert!(
        !rendered.contains("digits.rs"),
        "and not against the generated Rust: {rendered}"
    );
    assert!(
        rendered.contains("the trait `Send` is not implemented"),
        "the note comes through - the **text** is still Rust's, which ADR-005 \
         D7 records as the open half: {rendered}"
    );
}
