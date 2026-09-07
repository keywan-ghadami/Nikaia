//! The lowering, checked against its own output - and by running it.
//!
//! `fixtures/measurements_expected.rs` is what the emitter produces for
//! `fixtures/measurements.nika`. This file does two things with it that a
//! snapshot alone cannot: it `include!`s it, so the parser backend has to
//! accept every line of it, and it drives the `par_fold` driver the lowering
//! generated to check that the answer does not depend on the number of pieces.

use nikaia::emit::{emit_program, Profile};
use nikaia::parser::parse_to_ast;
use winnow_grammar::rt::Parallelism;
use winnow_grammar::ParseContext;

// The generated code. `grammar!` expands it here, so a lowering that produces
// something the backend rejects fails to compile rather than to compare.
include!("fixtures/measurements_expected.rs");

// Names the .nika file uses without declaring: the grammar module is generated
// with `use super::*`, so they resolve where the grammar is placed.

fn digit_value(c: char) -> i32 {
    c as i32 - '0' as i32
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Summary {
    pub count: i64,
    pub tenths: i64,
}

impl Summary {
    fn new() -> Summary {
        Summary::default()
    }

    fn record(self, m: Reading<'_>) -> Summary {
        Summary {
            count: self.count + 1,
            tenths: self.tenths + i64::from(m.temp),
        }
    }

    fn merge(a: Summary, b: Summary) -> Summary {
        Summary {
            count: a.count + b.count,
            tenths: a.tenths + b.tenths,
        }
    }
}

const FIXTURE: &str = include_str!("fixtures/measurements.nika");
const EXPECTED: &str = include_str!("fixtures/measurements_expected.rs");

fn emit(source: &str, profile: Profile) -> String {
    let parsed = parse_to_ast(source).expect("the fixture parses");
    emit_program(&parsed, profile).expect("the fixture lowers")
}

#[test]
fn the_emitted_rust_is_the_file_this_test_compiles() {
    assert_eq!(
        emit(FIXTURE, Profile::Advanced),
        EXPECTED,
        "the emitter and fixtures/measurements_expected.rs have drifted apart; \
         regenerate it with `cargo run -p nikaia --example dump -- \
         crates/nikaia/tests/fixtures/measurements.nika`"
    );
}

#[test]
fn a_frame_becomes_the_backend_attribute() {
    let emitted = emit(FIXTURE, Profile::Advanced);
    // ADR-009 D1: keyed, and the boundary keeps its escape - the backend is
    // handed the same "\n" the source wrote.
    assert!(
        emitted.contains(r#"#[frame(boundary = "\n")]"#),
        "no frame attribute in:\n{emitted}"
    );
}

#[test]
fn a_view_takes_the_input_lifetime_and_the_struct_with_it() {
    let emitted = emit(FIXTURE, Profile::Advanced);
    // ADR-008: `&str` is a view marker. The lifetime is the emitter's, never
    // the source's - and a struct that holds one is tied to the input too.
    assert!(emitted.contains("rule NAME -> &'a str ="), "{emitted}");
    assert!(
        emitted.contains("rule MEASUREMENT -> Reading<'a> ="),
        "{emitted}"
    );
    assert!(emitted.contains("pub struct Reading<'a> {"), "{emitted}");
    assert!(emitted.contains("pub name: &'a str,"), "{emitted}");
}

#[test]
fn a_bare_par_fold_gets_the_binding_its_rule_needs() {
    let emitted = emit(FIXTURE, Profile::Advanced);
    // A `par_fold` is the whole body of its rule (ADR-009 D2), so there is
    // nothing for an action to add - and the emitter supplies the one the
    // backend wants rather than making the user write it.
    assert!(
        emitted.contains(
            "folded:par_fold(MEASUREMENT, Summary::new, |acc, m| { acc.record(m) }, Summary::merge)"
        ),
        "{emitted}"
    );
    assert!(emitted.contains("-> { folded }"), "{emitted}");
}

// --- The driver ---

const WITH_DSL: &str = r#"
grammar Measurements {
    @frame(boundary: "\n")
    rule MEASUREMENT -> i32 = t:i32 frame_end -> { t }

    pub rule file -> i64 =
        par_fold(MEASUREMENT, zero, fn(acc, m) { acc + m }, add)
}

fn summarize() {
    let totals = dsl Measurements from data
}
"#;

#[test]
fn the_profile_chooses_the_parallelism_and_nothing_else() {
    // ADR-009: parallelism is not a dialect. The same source compiles under
    // both profiles; under Lite the driver is told not to cut, which is what
    // "degrades to a sequential fold" means in code.
    let advanced = emit(WITH_DSL, Profile::Advanced);
    let lite = emit(WITH_DSL, Profile::Lite);

    assert!(
        advanced.contains(
            "Measurements::parse_file_pieces(data, ParseContext::<()>::default, Parallelism::Auto)?"
        ),
        "{advanced}"
    );
    assert!(
        lite.contains(
            "Measurements::parse_file_pieces(data, ParseContext::<()>::default, Parallelism::Off)?"
        ),
        "{lite}"
    );

    // Only the executor differs.
    assert_eq!(
        advanced.replace("Parallelism::Auto", "Parallelism::Off"),
        lite
    );
}

#[test]
fn a_sequential_entry_rule_gets_no_piece_driver() {
    // Without a `par_fold` there is nothing to cut and nothing to merge, so
    // the lowering drives the rule's own parser over the whole input.
    let source = r#"
grammar Digits {
    pub rule value -> i32 = v:i32 -> { v }
}

fn read() {
    let n = dsl Digits from data
}
"#;
    let emitted = emit(source, Profile::Advanced);
    assert!(!emitted.contains("_pieces("), "{emitted}");
    assert!(
        emitted.contains("Digits::parse_value().parse_next(&mut stream)?"),
        "{emitted}"
    );
}

// --- Running what was generated ---

const MEASUREMENTS: &str = "Hamburg;12.0\nAbha;-23.0\nSaint-Pierre;9.1\nHamburg;-0.4\n";

#[test]
fn the_generated_parser_gives_the_same_answer_however_it_is_cut() {
    let expected = Summary {
        count: 4,
        tenths: 120 - 230 + 91 - 4,
    };

    for how in [
        Parallelism::Off,
        Parallelism::Pieces(1),
        Parallelism::Pieces(3),
        Parallelism::Pieces(64),
        Parallelism::Auto,
    ] {
        let got = Measurements::parse_file_pieces(MEASUREMENTS, ParseContext::<()>::default, how)
            .unwrap_or_else(|e| panic!("{how:?}: {}", e.render(MEASUREMENTS)));
        assert_eq!(got, expected, "{how:?}");
    }
}

#[test]
fn a_frame_that_does_not_parse_is_rejected_whatever_the_cut() {
    let broken = "Hamburg;12.0\nAbha;not-a-temperature\n";

    for how in [Parallelism::Off, Parallelism::Pieces(2), Parallelism::Auto] {
        assert!(
            Measurements::parse_file_pieces(broken, ParseContext::<()>::default, how).is_err(),
            "{how:?} accepted a broken frame"
        );
    }
}
