//! The lowering, checked against its own output - and by running it.
//!
//! The two files under `fixtures/` ending in `_expected.rs` are what the
//! emitter produces for the `.nika` files beside them. This test does two
//! things with them that a snapshot alone cannot: it `include!`s them, so the
//! parser backend has to accept every line, and it drives the code they
//! generate.
//!
//! Each generated file goes in a module of its own, because each is a whole
//! compilation unit and brings its own imports.

use nikaia::emit::{emit_program, Profile};
use nikaia::parser::parse_to_ast;

const FIXTURE: &str = include_str!("fixtures/measurements.nika");
const EXPECTED: &str = include_str!("fixtures/measurements_expected.rs");
const DIGITS: &str = include_str!("fixtures/digits.nika");

fn emit(source: &str, profile: Profile) -> String {
    let parsed = parse_to_ast(source).expect("the fixture parses");
    emit_program(&parsed, profile)
        .expect("the fixture lowers")
        .rust
}

/// Compare emitted code by its tokens rather than its indentation, for the one
/// place where the same code is written at two different depths.
fn squashed(code: &str) -> String {
    code.chars().filter(|c| !c.is_whitespace()).collect()
}

// --- What the emitter produced, compiled ---

mod measurements {
    //! The names `measurements.nika` uses without declaring live here: the
    //! grammar module is generated with `use super::*`, so they resolve where
    //! the grammar is placed.

    include!("fixtures/measurements_expected.rs");

    pub fn digit_value(c: char) -> i32 {
        c as i32 - '0' as i32
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq)]
    pub struct Summary {
        pub count: i64,
        pub tenths: i64,
        /// Only here to read the view: `NAME` is a slice of the input, and a
        /// summary that never touched it would not notice if it were wrong.
        pub name_bytes: i64,
    }

    impl Summary {
        fn new() -> Summary {
            Summary::default()
        }

        fn record(self, m: Reading<'_>) -> Summary {
            Summary {
                count: self.count + 1,
                tenths: self.tenths + i64::from(m.temp),
                name_bytes: self.name_bytes + m.name.len() as i64,
            }
        }

        fn merge(a: Summary, b: Summary) -> Summary {
            Summary {
                count: a.count + b.count,
                tenths: a.tenths + b.tenths,
                name_bytes: a.name_bytes + b.name_bytes,
            }
        }
    }
}

mod digits {
    include!("fixtures/digits_expected.rs");

    /// What `dsl Digits from data` lowers to, written out so that the compiler
    /// has to accept it and a test can run it.
    /// `a_sequential_entry_rule_gets_no_piece_driver` is what keeps this copy
    /// and the emitter's output the same code.
    // `&*` is redundant for a `&str` and necessary for everything else the
    // emitter has to accept there - a mapping, an owned string - so the copy
    // keeps it and clippy is told why.
    #[allow(clippy::borrow_deref_ref)]
    pub fn sequential_driver(data: &str) -> Result<Pair, String> {
        use winnow::Parser;
        let _source = &*data;
        let mut stream = winnow_grammar::ParseInput::<()> {
            state: winnow_grammar::ParseContext::<()>::default(),
            input: winnow::stream::LocatingSlice::new(_source),
        };
        // The emitted form is this block followed by `?`, in a `throws`
        // function; here it is the return value, which is the same code with
        // one fewer wrapper - clippy objects to `Ok(…?)` and is right.
        Digits::parse_pair()
            .parse_next(&mut stream)
            .map_err(|error| error.render(_source))
    }
}

// --- The emitted text ---

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

#[test]
fn operators_keep_their_meaning_and_lose_their_noise() {
    // Nikaia and Rust bind these the same way, so the emitter parenthesises
    // only where the source's own grouping demands it.
    let source = r#"
fn arithmetic() {
    let flat = a * 10 + b
    let grouped = (a + b) * c
    let right = a - (b - c)
    let mixed = a + b < c && d
}
"#;
    let emitted = emit(source, Profile::Advanced);

    assert!(emitted.contains("let flat = a * 10 + b;"), "{emitted}");
    assert!(emitted.contains("let grouped = (a + b) * c;"), "{emitted}");
    assert!(emitted.contains("let right = a - (b - c);"), "{emitted}");
    assert!(emitted.contains("let mixed = a + b < c && d;"), "{emitted}");
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
            "Measurements::parse_file_pieces(_source, &ParseContext::<()>::default(), Parallelism::Auto)"
        ),
        "{advanced}"
    );
    assert!(
        lite.contains(
            "Measurements::parse_file_pieces(_source, &ParseContext::<()>::default(), Parallelism::Off)"
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
    // Without a `par_fold` there is nothing to cut and nothing to merge, so the
    // lowering drives the rule's own parser over the whole input (ADR-011 D4).
    let source = format!("{DIGITS}\n\nfn read() {{\n    let p = dsl Digits from data\n}}\n");
    let emitted = emit(&source, Profile::Advanced);

    assert!(!emitted.contains("_pieces("), "{emitted}");
    assert!(
        squashed(&emitted).contains(&squashed(
            r#"{
                use winnow::Parser;
                let _source = &*data;
                let mut stream = winnow_grammar::ParseInput::<()> {
                    state: winnow_grammar::ParseContext::<()>::default(),
                    input: winnow::stream::LocatingSlice::new(_source),
                };
                Digits::parse_pair()
                    .parse_next(&mut stream)
                    .map_err(|error| error.render(_source))
            }?"#
        )),
        "the emitted driver and the one `digits::sequential_driver` compiles \
         have drifted:\n{emitted}"
    );
}

/// `dsl … from …` propagates its failure on its own, and a `catch` beside it
/// is the one place that wants the `Result` instead. Emitting the `?` there
/// too produced `match <value> { Ok(..) => .., Err(..) => .. }` - a handler
/// matching on something already unwrapped, which is not a message about a
/// program but a compiler bug (Kap 7.1).
#[test]
fn a_dsl_with_a_catch_is_handed_the_result_and_not_the_value() {
    let source = format!(
        "{DIGITS}\n\nfn read() throws {{\n    \
         let p = dsl Digits from data catch {{\n        return\n    }}\n}}\n"
    );
    let emitted = emit(&source, Profile::Advanced);

    assert!(
        emitted.contains("Ok(value) => value"),
        "the catch should match on the parse:\n{emitted}"
    );
    assert!(
        !emitted.contains("}? {"),
        "the catch is matching on an unwrapped value:\n{emitted}"
    );
}

/// A `ParseError` knows an offset and not the text it came from, so only the
/// driver can turn one into a line and a column - and it is the only place
/// that has both. Without this a program that rejects a file says *what* was
/// expected and never *where* (ADR-009 D3, `docs/error-corpus.md`).
#[test]
fn a_rejected_parse_is_rendered_against_the_input_it_parsed() {
    let emitted = emit(WITH_DSL, Profile::Advanced);
    assert!(
        emitted.contains(".map_err(|error| error.render(_source))"),
        "{emitted}"
    );
}

// --- Running what was generated ---

const MEASUREMENTS: &str = "Hamburg;12.0\nAbha;-23.0\nSaint-Pierre;9.1\nHamburg;-0.4\n";

#[test]
fn the_generated_parser_gives_the_same_answer_however_it_is_cut() {
    use measurements::{Measurements, Summary};
    use winnow_grammar::rt::Parallelism;
    use winnow_grammar::ParseContext;

    let expected = Summary {
        count: 4,
        tenths: 120 - 230 + 91 - 4,
        // "Hamburg", "Abha", "Saint-Pierre", "Hamburg" - the station names are
        // slices of the input, and a cut in the wrong place would show here.
        name_bytes: 7 + 4 + 12 + 7,
    };

    for how in [
        Parallelism::Off,
        Parallelism::Pieces(1),
        Parallelism::Pieces(3),
        Parallelism::Pieces(64),
        Parallelism::Auto,
    ] {
        let got =
            Measurements::parse_file_pieces(MEASUREMENTS, &ParseContext::<()>::default(), how)
                .unwrap_or_else(|e| panic!("{how:?}: {}", e.render(MEASUREMENTS)));
        assert_eq!(got, expected, "{how:?}");
    }
}

#[test]
fn a_frame_that_does_not_parse_is_rejected_whatever_the_cut() {
    use measurements::Measurements;
    use winnow_grammar::rt::Parallelism;
    use winnow_grammar::ParseContext;

    let broken = "Hamburg;12.0\nAbha;not-a-temperature\n";

    for how in [Parallelism::Off, Parallelism::Pieces(2), Parallelism::Auto] {
        assert!(
            Measurements::parse_file_pieces(broken, &ParseContext::<()>::default(), how).is_err(),
            "{how:?} accepted a broken frame"
        );
    }
}

#[test]
fn the_sequential_driver_runs() {
    let pair = digits::sequential_driver("12,30").expect("parses");
    assert_eq!((pair.left, pair.right), (12, 30));
    assert!(digits::sequential_driver("12;30").is_err());
}

// --- The example this exists for ---

/// The `grammar` item and the struct it builds, taken out of the real
/// `examples/1brc.nika` rather than copied - a copy would drift, and the point
/// of this test is that the example itself lowers.
fn one_brc_grammar_half() -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/1brc.nika");
    let source = std::fs::read_to_string(&path).expect("examples/1brc.nika");

    let mut out = String::new();
    let mut inside = false;
    for line in source.lines() {
        if line.starts_with("grammar Measurements {") || line.starts_with("@borrowed") {
            inside = true;
        }
        if inside {
            out.push_str(line);
            out.push('\n');
        }
        if inside && line == "}" {
            inside = false;
        }
    }

    assert!(
        out.contains("par_fold(MEASUREMENT"),
        "the extraction missed the grammar:\n{out}"
    );
    out
}

#[test]
fn the_grammar_half_of_the_1brc_example_lowers() {
    // ADR-011 §3: the file as a whole does not compile yet - its `impl` blocks,
    // `throws`/`catch` and string interpolation are not lowered. Its grammar
    // does, and that is the half this work was about.
    let emitted = emit(&one_brc_grammar_half(), Profile::Advanced);

    assert!(
        emitted.contains(r#"#[frame(boundary = "\n")]"#),
        "{emitted}"
    );
    assert!(
        emitted.contains("rule MEASUREMENT -> Reading<'a> ="),
        "{emitted}"
    );
    assert!(
        emitted.contains(
            "par_fold(MEASUREMENT, Summary::new, |acc, m| { acc.record(m) }, Summary::merge)"
        ),
        "{emitted}"
    );
    assert!(emitted.contains("s:until(\";\" | frame_end)"), "{emitted}");
    assert!(emitted.contains("whole:digit{1,2}"), "{emitted}");
}

// --- Type arguments to a built-in ---

#[test]
fn a_type_argument_is_written_with_brackets_and_emitted_with_angles() {
    // Kap 4.3: Nikaia spells a type argument with brackets everywhere, which
    // is what keeps `<` free elsewhere in the language. The backend's
    // built-ins take theirs between angles, and the emitter is the one place
    // the two meet - the source never writes the backend's spelling.
    let source = r#"
grammar Ids {
    rule N -> i32 = n:dec[i32](digit{1,2}) -> { n }
    rule T -> &str = t:text(alpha1 digit*) -> { t }
    pub rule entry -> i32 = n:N -> { n }
}
"#;
    let emitted = emit(source, Profile::Advanced);

    assert!(emitted.contains("n:dec<i32>(digit{1,2})"), "{emitted}");
    // A built-in without type arguments keeps its call exactly as written.
    assert!(emitted.contains("t:text(alpha1 digit*)"), "{emitted}");
}
