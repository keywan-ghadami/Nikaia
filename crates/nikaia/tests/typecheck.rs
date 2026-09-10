//! The type checker (ADR-024).
//!
//! Two halves, and the first is the one that decides whether the tool is worth
//! running: **every program in the repository must produce no findings.** A
//! checker that reports something about correct code is worse than none, so the
//! corpus is the guard and the deliberately-wrong programs below are the proof
//! that the guard is not passing because the checker is asleep.

use std::path::PathBuf;

use nikaia::check::{self, Finding};
use nikaia::contracts::{Ledger, STD};
use nikaia::parser::parse_to_ast;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn findings(source: &str) -> Vec<Finding> {
    let parsed = parse_to_ast(source).expect("the source parses");
    let own = Ledger::infer(&parsed);
    let library = Ledger::parse(STD).expect("std's shipped ledger parses");
    check::check(&parsed, &own, &library).findings
}

/// The one finding a source is written to produce, with its code.
fn one(source: &str) -> (String, String) {
    let found = findings(source);
    assert_eq!(
        found.len(),
        1,
        "expected exactly one finding, got {:#?}",
        found.iter().map(|f| &f.message).collect::<Vec<_>>()
    );
    (found[0].code.to_string(), found[0].message.clone())
}

// --- the guard ---------------------------------------------------------------

/// Every `.nika` in the repository is checked, and none of them has anything
/// wrong with it.
///
/// This is the test that keeps the checker honest. `Ty::Unknown` exists so that
/// the checker says nothing where it knows nothing; the way to find out whether
/// that discipline held is to run it over every program there is.
#[test]
fn no_program_in_the_repository_has_a_type_error() {
    let library = Ledger::parse(STD).expect("std's shipped ledger parses");
    let mut checked = 0;
    let mut reported = String::new();

    for dir in ["examples", "crates/nikaia-std/src"] {
        let dir = repo_root().join(dir);
        let mut paths: Vec<PathBuf> = std::fs::read_dir(&dir)
            .unwrap_or_else(|e| panic!("read {}: {e}", dir.display()))
            .map(|entry| entry.expect("dir entry").path())
            .filter(|path| path.extension().and_then(|e| e.to_str()) == Some("nika"))
            .collect();
        paths.sort();

        for path in paths {
            let source = std::fs::read_to_string(&path).expect("read the program");
            let parsed = parse_to_ast(&source)
                .unwrap_or_else(|e| panic!("{} does not parse: {e}", path.display()));
            let own = Ledger::infer(&parsed);
            let name = path.display().to_string();
            for finding in check::check(&parsed, &own, &library).findings {
                reported.push_str(&nikaia::diagnostics::render_finding(
                    &finding, &name, &source,
                ));
            }
            checked += 1;
        }
    }

    assert!(checked >= 8, "only {checked} programs were checked");
    assert!(reported.is_empty(), "the corpus is not clean:\n{reported}");
}

// --- what it catches ---------------------------------------------------------

/// A call that passes the wrong number of arguments, answered from this
/// program's own contracts.
#[test]
fn a_call_with_too_few_arguments_is_reported() {
    let (code, message) = one("fn add(a: i32, b: i32) -> i32 { return a + b }\n\
         fn main() { let n = add(1) }");
    assert_eq!(code, "NK1101");
    assert_eq!(message, "`add` takes 2 arguments, and this call passes 1");
}

/// …and one answered from `std`'s shipped ledger, which is what makes the
/// ledger worth shipping (Part III, 13.5).
#[test]
fn a_call_into_std_is_checked_against_the_shipped_ledger() {
    let (code, message) = one("fn main() throws { let text = io::read_to_string(\"x\")? }");
    assert_eq!(code, "NK1101");
    assert_eq!(
        message,
        "`io::read_to_string` takes 0 arguments, and this call passes 1"
    );
    assert_eq!(
        findings("fn main() throws { let text = io::read_to_string(\"x\")? }")[0]
            .help
            .as_deref(),
        Some("call it as `io::read_to_string()`")
    );
}

/// An argument of the wrong type, with the name the callee gave it.
#[test]
fn an_argument_of_the_wrong_type_is_reported() {
    let (code, message) = one("fn greet(who: String) { }\n\
         fn main() { greet(\"world\") }");
    assert_eq!(code, "NK1102");
    assert_eq!(
        message,
        "`greet` takes `who: String`, and this call passes `&str`"
    );
}

/// A `let` that says one thing and is given another.
#[test]
fn a_let_annotation_that_disagrees_is_reported() {
    let (code, message) = one("fn main() { let n: i32 = 'x' }");
    assert_eq!(code, "NK1103");
    assert_eq!(message, "this is `char`, and the `let` says `i32`");
}

/// A `return` that does not hand back what the function declared.
#[test]
fn a_return_of_the_wrong_type_is_reported() {
    let (code, message) = one("fn width() -> i32 { return \"wide\" }");
    assert_eq!(code, "NK1104");
    assert_eq!(
        message,
        "this returns `&str`, and the function declares `i32`"
    );
}

/// The last expression of a body is what the function hands back, so it
/// answers to the declared type exactly as a `return` does.
#[test]
fn a_tail_expression_of_the_wrong_type_is_reported() {
    let (code, _) = one("fn label(name: &str) -> i32 {\n\
         \x20   name\n\
         }");
    assert_eq!(code, "NK1104");
}

/// An assignment of the wrong type.
#[test]
fn an_assignment_of_the_wrong_type_is_reported() {
    let (code, message) = one("fn main() {\n\
         \x20   let mut n: i32 = 0\n\
         \x20   n = 'x'\n\
         }");
    assert_eq!(code, "NK1105");
    assert_eq!(
        message,
        "this is `char`, and what it is assigned to is `i32`"
    );
}

/// A struct literal whose field is given the wrong type.
#[test]
fn a_struct_field_of_the_wrong_type_is_reported() {
    let (code, message) = one("struct Reading { name: String, temp: i32 }\n\
         fn main() { let r = Reading { name: \"Hamburg\", temp: 12 } }");
    assert_eq!(code, "NK1106");
    assert_eq!(message, "`Reading.name` is `String`, and this is `&str`");
}

/// A field that is not there, with the one that was probably meant.
#[test]
fn a_field_that_does_not_exist_is_reported() {
    let (code, message) = one("struct Reading { name: &str, temp: i32 }\n\
         fn label(r: &Reading) -> &str { return r.nmae }");
    assert_eq!(code, "NK1107");
    assert_eq!(message, "`Reading` has no field `nmae`");
    assert_eq!(
        findings(
            "struct Reading { name: &str, temp: i32 }\n\
                  fn label(r: &Reading) -> &str { return r.nmae }"
        )[0]
        .help
        .as_deref(),
        Some("did you mean `name`?")
    );
}

/// …and one in a struct literal, which is the same mistake in the other place.
#[test]
fn a_field_that_does_not_exist_in_a_literal_is_reported() {
    let (code, message) = one("struct Reading { name: &str }\n\
         fn main() { let r = Reading { nmae: \"Hamburg\" } }");
    assert_eq!(code, "NK1107");
    assert_eq!(message, "`Reading` has no field `nmae`");
}

/// A condition that is not a `bool`.
#[test]
fn a_condition_that_is_not_a_bool_is_reported() {
    let (code, message) = one("fn main() {\n\
         \x20   let name = \"Hamburg\"\n\
         \x20   if name { }\n\
         }");
    assert_eq!(code, "NK1108");
    assert_eq!(message, "this is `&str`, and a condition is a `bool`");
}

/// A grammar's action builds the rule's value, so it answers to the rule's
/// declared type - and a struct literal in one is checked like any other.
#[test]
fn a_grammar_action_is_checked_against_its_rule() {
    let (code, message) = one("struct Reading { name: &str, temp: i32 }\n\
         grammar Measurements {\n\
         \x20   rule LINE -> Reading = name:until(\";\") \";\" temp:digit\n\
         \x20       -> { Reading { nmae: name, temp: temp } }\n\
         }");
    assert_eq!(code, "NK1107");
    assert_eq!(message, "`Reading` has no field `nmae`");
}

/// …and an action that builds something else entirely.
#[test]
fn a_grammar_action_of_the_wrong_type_is_reported() {
    let (code, message) = one("grammar Measurements {\n\
         \x20   rule COUNT -> i32 = s:until(\";\") -> { \"one\" }\n\
         }");
    assert_eq!(code, "NK1104");
    assert_eq!(
        message,
        "this action builds `&str`, and its rule declares `i32`"
    );
}

/// A `for` over a list whose element type is written down binds that type.
#[test]
fn a_loop_binds_the_element_type_of_a_list() {
    let (code, message) = one("struct Row { id: i32 }\n\
         fn count(rows: Vec[Row]) {\n\
         \x20   for row in rows { let n: i32 = row.idd }\n\
         }");
    assert_eq!(code, "NK1107");
    assert_eq!(message, "`Row` has no field `idd`");
}

// --- a loop whose step can fail (ADR-025) ------------------------------------

/// `NK2701`: a turn of the loop reads, a read can fail, and the function does
/// not say so.
///
/// The rule is Part I 6.4's, stated generally: an implicit call that can fail
/// fails the enclosing function, and the compiler makes you declare it.
#[test]
fn a_loop_that_can_fail_in_a_function_that_does_not_say_so_is_reported() {
    let (code, message) = one("fn count() -> i64 {\n\
         \x20   let mut n = 0\n\
         \x20   for line in io::lines() { n += 1 }\n\
         \x20   return n\n\
         }");
    assert_eq!(code, "NK2701");
    assert_eq!(
        message,
        "this function can fail because a turn of this loop can fail"
    );
}

/// …and nothing at all once it does.
#[test]
fn a_loop_that_can_fail_is_fine_where_the_failure_may_leave() {
    assert!(findings(
        "fn count() -> i64 throws {\n\
         \x20   let mut n = 0\n\
         \x20   for line in io::lines() { n += 1 }\n\
         \x20   return n\n\
         }"
    )
    .is_empty());
}

/// The stream may be named first, and it is the same loop.
///
/// ADR-025 D7: this is why the emitter asks the type checker rather than
/// matching on the name `io::lines` - the name is not there to match.
#[test]
fn naming_the_stream_first_does_not_hide_it() {
    let (code, _) = one("fn count() -> i64 {\n\
         \x20   let stream = io::lines()\n\
         \x20   let mut n = 0\n\
         \x20   for line in stream { n += 1 }\n\
         \x20   return n\n\
         }");
    assert_eq!(code, "NK2701");
}

/// A stream of pairs does not exist in `std`, and unwrapping a failure while
/// also taking a pair apart is a shape to design rather than to guess at.
#[test]
fn a_fallible_loop_binds_one_name() {
    let (code, message) = one("fn count() throws {\n\
         \x20   for (a, b) in io::lines() { }\n\
         }");
    assert_eq!(code, "NK2701");
    assert_eq!(
        message,
        "a `for` over `Lines` binds one name, and this binds 2"
    );
}

/// An ordinary loop is not touched by any of this.
#[test]
fn a_loop_over_something_that_cannot_fail_says_nothing() {
    assert!(findings(
        "fn count(xs: Vec[i32]) -> i64 {\n\
         \x20   let mut n = 0\n\
         \x20   for x in xs { n += 1 }\n\
         \x20   for i in 0..10 { n += 1 }\n\
         \x20   return n\n\
         }"
    )
    .is_empty());
}

// --- what it deliberately does not catch -------------------------------------

/// A method on a receiver `std` does not write down is not an error.
///
/// This is `Unknown` doing its job: `push_str`, `entry` and `chars` are Rust's,
/// and a checker that had an opinion about them would be guessing.
#[test]
fn a_method_on_an_unknown_receiver_says_nothing() {
    assert!(findings(
        "fn main() {\n\
         \x20   let mut out = String::new()\n\
         \x20   out.push_str(\"a\", \"b\", \"c\")\n\
         }"
    )
    .is_empty());
}

/// A bare number fits every numeric type, as it does in the language below.
/// Committing a literal to one would make `add(3)` wrong wherever the parameter
/// is not that one.
#[test]
fn an_integer_literal_fits_any_numeric_parameter() {
    assert!(findings(
        "fn small(a: i32) { }\n\
         fn big(a: u64) { }\n\
         fn main() { small(3) big(3) }"
    )
    .is_empty());
}

/// A generic parameter is a name that stands for a type rather than being one,
/// so nothing is claimed about it.
#[test]
fn a_generic_parameter_is_not_a_type() {
    assert!(findings(
        "fn first[T](xs: T) -> T { return xs }\n\
         fn main() { let n: i32 = first(1) }"
    )
    .is_empty());
}

/// A struct whose fields are not written down anywhere is not checked - and an
/// absent type is never an error.
#[test]
fn a_field_of_an_unknown_type_says_nothing() {
    assert!(findings("fn label(r: &Row) -> &str { return r.nmae }").is_empty());
}

/// `for (k, v) in map` takes apart a pair whose shape Stage 0 has no signature
/// for, so it binds nothing rather than guessing.
#[test]
fn a_loop_over_pairs_binds_nothing() {
    assert!(findings(
        "struct Row { id: i32 }\n\
         fn count(rows: Vec[Row]) {\n\
         \x20   for (a, b) in rows { let n: i32 = a.idd }\n\
         }"
    )
    .is_empty());
}

/// A parameter whose Rust type is a *bound* rather than a type is `?` in the
/// ledger, and this is why: `fs::write` accepts a `String`, a `&str` and a
/// buffer, so a claim of `&str` there would refuse a correct program.
#[test]
fn a_parameter_that_accepts_several_types_claims_none() {
    assert!(findings(
        "fn main() throws {\n\
         \x20   let text = io::read_to_string()\n\
         \x20   let path = \"out.txt\"\n\
         \x20   fs::write(path, text)\n\
         }"
    )
    .is_empty());

    // …and the arity is still checked, which is the half that survives.
    let (code, _) = one("fn main() throws { fs::write(\"out.txt\") }");
    assert_eq!(code, "NK1101");
}

/// Both directions of the rule at once: an unknown on either side fits.
#[test]
fn a_value_from_an_unwritten_signature_fits_anywhere() {
    assert!(findings(
        "fn takes(a: i32) { }\n\
         fn main() { takes(cli::args().nth(1)) }"
    )
    .is_empty());
}
