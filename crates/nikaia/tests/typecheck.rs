//! The type checker (ADR-024).
//!
//! Two halves, and the first is the one that decides whether the tool is worth
//! running: **every program in the repository must produce no findings.** A
//! checker that reports something about correct code is worse than none, so the
//! corpus is the guard and the deliberately-wrong programs below are the proof
//! that the guard is not passing because the checker is asleep.

use std::path::PathBuf;

use nikaia::check::{self, Finding, Severity};
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
    let (code, message) = one("fn main() throws { let text = io::read_to_string(\"x\") }");
    assert_eq!(code, "NK1101");
    assert_eq!(
        message,
        "`io::read_to_string` takes 0 arguments, and this call passes 1"
    );
    assert_eq!(
        findings("fn main() throws { let text = io::read_to_string(\"x\") }")[0]
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

/// A `while` decides on a `bool` too, and says which loop it is about.
#[test]
fn a_while_condition_that_is_not_a_bool_is_reported() {
    let (code, message) = one("fn main() {\n\
         \x20   let name = \"Hamburg\"\n\
         \x20   while name { }\n\
         }");
    assert_eq!(code, "NK1108");
    assert_eq!(message, "this is `&str`, and a condition is a `bool`");
    assert_eq!(
        findings(
            "fn main() {\n\
             \x20   let name = \"Hamburg\"\n\
             \x20   while name { }\n\
             }"
        )[0]
        .notes[0],
        "a `while` repeats while a `bool` holds"
    );
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

// --- the subject ; config protocol (Kap 5.1) ---------------------------------

/// An option the callee does not have, with the one that was probably meant.
#[test]
fn an_option_that_does_not_exist_is_reported() {
    let (code, message) = one("fn request(url: &str; timeout: i32 = 30) { }\n\
         fn main() { request(\"x\"; timout: 5) }");
    assert_eq!(code, "NK1109");
    assert_eq!(message, "`request` has no option `timout`");
    assert_eq!(
        findings(
            "fn request(url: &str; timeout: i32 = 30) { }\n\
             fn main() { request(\"x\"; timout: 5) }"
        )[0]
        .help
        .as_deref(),
        Some("did you mean `timeout`?")
    );
}

/// …and one passed to a function that has no `;` at all.
#[test]
fn an_option_on_a_function_that_takes_none_is_reported() {
    let (code, _) = one("fn plain(a: i32) { }\n\
         fn main() { plain(1; loud: true) }");
    assert_eq!(code, "NK1109");
}

/// An option is checked by type like anything else.
#[test]
fn an_option_of_the_wrong_type_is_reported() {
    let (code, message) = one("fn request(url: &str; timeout: i32 = 30) { }\n\
         fn main() { request(\"x\"; timeout: \"soon\") }");
    assert_eq!(code, "NK1106");
    assert_eq!(
        message,
        "`request` takes `timeout: i32`, and this passes `&str`"
    );
}

/// Options do not count towards the arity: leaving every one of them out is
/// what a default is for.
#[test]
fn options_are_not_counted_as_arguments() {
    assert!(findings(
        "fn request(url: &str; timeout: i32 = 30, method: &str = \"GET\") { }\n\
         fn main() {\n\
         \x20   request(\"x\")\n\
         \x20   request(\"x\"; timeout: 5)\n\
         \x20   request(\"x\"; method: \"POST\", timeout: 5)\n\
         }"
    )
    .is_empty());
}

/// `std`'s options are read from the ledger it ships, like everything else.
#[test]
fn an_option_of_a_library_function_is_checked_from_its_ledger() {
    assert!(findings("fn main() throws { fs::write(\"o\", \"x\"; append: true) }").is_empty());

    let (code, message) = one("fn main() throws { fs::write(\"o\", \"x\"; apend: true) }");
    assert_eq!(code, "NK1109");
    assert_eq!(message, "`fs::write` has no option `apend`");
}

/// An interpolated string is a `format!` in the emitted Rust, and a `format!`
/// is a `String`. Two spellings in Nikaia, two types below - and the checker
/// follows the lowering rather than the syntax.
#[test]
fn an_interpolated_string_is_a_string_and_a_plain_one_is_a_view() {
    assert!(findings("fn label(n: i32) -> String { return f\"{n} rows\" }").is_empty());

    let (code, message) = one("fn label(n: i32) -> &str { return f\"{n} rows\" }");
    assert_eq!(code, "NK1104");
    assert_eq!(
        message,
        "this returns `String`, and the function declares `&str`"
    );
}

// --- what a literal hides ----------------------------------------------------

/// A hole is Nikaia source, and it is checked like Nikaia source.
///
/// The same mistake was caught outside a hole and passed silently inside one,
/// which is worth a test of its own: `println("{…}")` is the most-written
/// syntax in the language, so the blind spot covered most of what people type.
#[test]
fn a_hole_in_a_string_is_checked_like_anything_else() {
    let outside = one("fn add(a: i32, b: i32) -> i32 { return a + b }\n\
         fn main() { let n = add(1) }");
    let inside = one("fn add(a: i32, b: i32) -> i32 { return a + b }\n\
         fn main() { println(f\"{add(1)}\") }");
    assert_eq!(
        inside, outside,
        "a hole is checked differently from a statement"
    );
    assert_eq!(inside.0, "NK1101");
}

/// …and a template's holes too (ADR-017), which reach the emitter the same way.
#[test]
fn a_hole_in_a_template_is_checked_too() {
    let (code, _) = one("fn add(a: i32, b: i32) -> i32 { return a + b }\n\
         fn page() -> String {\n\
         \x20   return dsl html {\n\
         \x20       <p>{add(1)}</p>\n\
         \x20   } eod\n\
         }");
    assert_eq!(code, "NK1101");
}

/// A hole that does not parse is the emitter's to report, in its own words at
/// the place it happens. The checker says nothing rather than guessing.
#[test]
fn a_hole_that_does_not_parse_is_not_the_checkers_business() {
    assert!(findings("fn main() { println(f\"{let}\") }").is_empty());
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

/// A method `std` does not write down is not an error.
///
/// This is `Unknown` doing its job: `insert_str` is Rust's and no ledger names
/// it, and a checker that had an opinion about it would be guessing.
///
/// `push_str` used to stand here and no longer can, because the ledger now says
/// what `String::new()` hands back (ADR-033 needed its touch set, and a
/// signature came with it) - so the receiver has a type, the entry for
/// `String::push_str` resolves, and three arguments to a method that takes one
/// is caught. That is the ledger growing and the checker getting sharper
/// together, which is what ADR-028 predicted it would look like.
#[test]
fn a_method_nobody_wrote_down_says_nothing() {
    assert!(findings(
        "fn main() {\n\
         \x20   let mut out = String::new()\n\
         \x20   out.insert_str(0, \"a\", \"b\")\n\
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

// --- the migration warning (ADR-035 D5) --------------------------------------

/// A string written before the `f` existed looks exactly like one that meant
/// its braces, so the change of meaning cannot be silent - Part III C.1 calls a
/// silent one a bug in this compiler.
#[test]
fn a_string_that_used_to_interpolate_is_warned_about() {
    let found = findings("fn main() { let name = \"welt\" println(\"hallo {name}\") }");
    assert_eq!(found.len(), 1, "{found:#?}");
    assert_eq!(found[0].code, "NK1111");
    assert_eq!(found[0].severity, Severity::Warning);
    // Paste-ready, as Part III C.2 requires of every diagnostic.
    assert_eq!(
        found[0].help.as_deref(),
        Some("write `f\"hallo {name}\"` if the value was meant to appear (Part I, 2.5)")
    );
}

/// **And it never fires on a program that is right.** This is the whole reason
/// it is a warning and not an error: a stylesheet, a regular expression and a
/// JSON document all hold braces, and `margin` is nobody's variable.
#[test]
fn braces_that_name_nothing_are_just_braces() {
    assert!(findings("fn main() { println(\"{ margin: 0 }\") }").is_empty());
    assert!(findings("fn main() { println(\"\\\\d{3}\") }").is_empty());
    assert!(findings("fn main() { println(\"{}\") }").is_empty());
    assert!(findings("fn main() { println(\"\\u{0041}\") }").is_empty());
}

/// The other half of the change of meaning: `"{{}}"` printed `{}` and now
/// prints itself.
#[test]
fn a_doubled_brace_in_a_plain_string_is_warned_about_too() {
    let found = findings("fn main() { print(\"{{}}\") }");
    assert_eq!(found.len(), 1, "{found:#?}");
    assert_eq!(found[0].code, "NK1111");
    assert_eq!(found[0].severity, Severity::Warning);
    assert!(
        found[0].help.as_deref().unwrap().contains("write `\"{}\"`"),
        "{:?}",
        found[0].help
    );
}

/// An `f"…"` is held to nothing by this - it says what it is.
#[test]
fn an_f_string_is_never_warned_about() {
    assert!(findings("fn main() { let n = 1 println(f\"{n}\") }").is_empty());
    assert!(findings("fn main() { let n = 1 println(f\"{{{n}}}\") }").is_empty());
}

/// **The type comes from the syntax** (ADR-035 D3), so a `&str` return that
/// hands back an `f"…"` is the mistake it was before - and a plain string with
/// a brace in it is a `&str` rather than becoming a `String` by accident.
#[test]
fn the_type_of_a_literal_is_read_off_its_first_character() {
    let (code, _) = one("fn label(n: i32) -> &str { return f\"{n} rows\" }");
    assert_eq!(code, "NK1104");
    assert!(findings("fn label() -> &str { return \"{ a brace }\" }").is_empty());
}

// --- a written call that can fail (ADR-023 D8, ADR-025 D1) -------------------

/// `NK2605`: the function calls something that can fail and declares nothing.
///
/// The same rule as `NK2701` one line earlier in the block, and the case the
/// rule was generalised *from*: ADR-025 D1 says an implicit fallible call
/// "fails the enclosing function, exactly as a written call would" - and the
/// written call had no code. So a caller lowered in silence, and the ledger
/// then published `signature = "() -> String"` with no `throws` at all - a
/// committed file that other programs read (ADR-020) saying a function cannot
/// fail when its body provably can.
#[test]
fn a_written_call_that_can_fail_in_a_function_that_does_not_say_so_is_reported() {
    let (code, message) = one(
        "fn liest() -> String throws { return fs::read_to_string(\"x.txt\") }\n\
         fn ruft() -> String { return liest() }",
    );
    assert_eq!(code, "NK2605");
    assert_eq!(message, "this function can fail because `liest` can fail");
}

/// The note is the contract, quoted from the ledger (Part III, C.4), and the
/// help names both of the language's ways out.
#[test]
fn the_note_quotes_the_callees_contract_and_the_help_is_a_way_out() {
    let found = findings(
        "enum ConfigError { NotFound }\n\
         fn liest() -> String throws { throw ConfigError::NotFound }\n\
         fn ruft() -> String { return liest() }",
    );
    assert_eq!(found.len(), 1, "{found:#?}");
    assert!(
        found[0]
            .notes
            .iter()
            .any(|n| n.contains(r#"`liest` carries `throws = ["ConfigError"]`"#)),
        "{:#?}",
        found[0].notes
    );
    let help = found[0].help.as_deref().unwrap_or_default();
    assert!(help.contains("add `throws` to `ruft`"), "{help}");
    assert!(help.contains("catch"), "{help}");
}

/// …and nothing at all once the function says so, or catches it.
///
/// Both of these are correct programs, and the one thing this checker may
/// never do is refuse one.
#[test]
fn a_declared_throws_or_a_catch_is_the_end_of_it() {
    let declared = "fn liest() -> String throws { return fs::read_to_string(\"x.txt\") }\n\
                    fn ruft() -> String throws { return liest() }";
    assert!(findings(declared).is_empty(), "{:#?}", findings(declared));

    let caught = "fn liest() -> String throws { return fs::read_to_string(\"x.txt\") }\n\
                  fn ruft() -> String { return liest() catch { return \"\".to_string() } }";
    assert!(findings(caught).is_empty(), "{:#?}", findings(caught));
}

/// A `catch` covers what it guards and not what its **handler** does.
///
/// `contracts::order` reached the same conclusion about the same shape: the
/// handler is ordinary code, and a failure raised inside one leaves the
/// function like any other. Reading the handler as covered would be the
/// fail-open direction ADR-010 D1 forbids.
#[test]
fn a_catch_handler_is_not_itself_caught() {
    let (code, _) = one(
        "fn liest() -> String throws { return fs::read_to_string(\"x.txt\") }\n\
         fn ruft() -> String { return liest() catch { return liest() } }",
    );
    assert_eq!(code, "NK2605");
}

/// A method call is a written call too, and the receiver's type is what makes
/// it answerable (ADR-028).
#[test]
fn a_method_that_can_fail_is_the_same_rule() {
    let (code, message) = one("enum ZuVoll { Voll }\n\
         struct Stats { n: i64 }\n\
         impl Stats {\n\
             fn add(&self, v: i64) throws { if self.n > 100 { throw ZuVoll::Voll } }\n\
         }\n\
         fn record(s: Stats) { s.add(1) }");
    assert_eq!(code, "NK2605");
    assert_eq!(
        message,
        "this function can fail because `Stats::add` can fail"
    );
}

/// ...and it still refuses now that the `?` is emitted for it.
///
/// The two halves have to stay the two halves: the emitter writes the `?` only
/// where the function declares `throws`, and this is what guarantees the `?`
/// always has somewhere to go. An inference that gave `record` a `throws` it
/// never wrote would have made the lowering work and the ledger lie (ADR-023
/// D1, ADR-025 D1), so the refusal is load-bearing rather than a leftover.
#[test]
fn the_refusal_does_not_weaken_when_the_method_call_propagates() {
    let body = "enum ZuVoll { Voll }\n\
                struct Stats { n: i64 }\n\
                impl Stats {\n\
                    fn add(&self, v: i64) -> i64 throws {\n\
                        if self.n + v > 100 { throw ZuVoll::Voll }\n\
                        return self.n + v\n\
                    }\n\
                }\n";

    // Undeclared: refused, and the set the emitter reads is not what decides it.
    let (code, _) = one(&format!(
        "{body}fn record(s: Stats) -> i64 {{ return s.add(1) }}"
    ));
    assert_eq!(code, "NK2605");

    // Declared: nothing to say, and this is the program that lowers with a `?`.
    let declared = format!("{body}fn record(s: Stats) -> i64 throws {{ return s.add(1) }}");
    assert!(findings(&declared).is_empty(), "{:#?}", findings(&declared));

    // Caught at the call: nothing to say either, and no `?` is emitted there.
    let caught =
        format!("{body}fn record(s: Stats) -> i64 {{ return s.add(1) catch {{ return 0 }} }}");
    assert!(findings(&caught).is_empty(), "{:#?}", findings(&caught));
}

/// What the emitter is handed, said of the checker's own output.
///
/// ADR-028: there is one type checker, and the emitter is not a second one. So
/// the answer about which method calls can fail is computed here, keyed by the
/// statement it is in and the method's name, and the emitter looks it up. Two
/// calls of the same name in one statement where only one can fail produce no
/// entry at all - the set never claims a call fails that does not.
#[test]
fn the_checker_says_which_method_calls_can_fail() {
    let source = "enum ZuVoll { Voll }\n\
                  struct A { n: i64 }\n\
                  impl A {\n\
                      fn add(&self, v: i64) -> i64 throws {\n\
                          if self.n > 100 { throw ZuVoll::Voll }\n\
                          return self.n + v\n\
                      }\n\
                  }\n\
                  struct B { n: i64 }\n\
                  impl B {\n\
                      fn add(&self, v: i64) -> i64 { return self.n + v }\n\
                      fn plain(&self) -> i64 { return self.n }\n\
                  }\n\
                  fn one_of_them(a: A) -> i64 throws { return a.add(1) }\n\
                  fn neither(b: B) -> i64 { return b.plain() }\n\
                  fn both_names(a: A, b: B) -> i64 throws { return a.add(1) + b.add(2) }";
    let parsed = parse_to_ast(source).expect("the source parses");
    let fallible = nikaia::check::propagation_against(&parsed, &Ledger::infer(&parsed)).methods;

    let names: Vec<&str> = fallible.iter().map(|(_, name)| name.as_str()).collect();
    assert_eq!(
        names,
        vec!["add"],
        "only the resolved fallible call belongs in the set: {fallible:?}"
    );

    // And it is the one in `one_of_them`, not the pair in `both_names`: the
    // statement there writes `add` twice and one of the two cannot fail, so the
    // pair is removed rather than guessed at.
    let at = fallible.iter().next().expect("one entry").0;
    let line = source[..at].lines().count();
    assert_eq!(
        source.lines().nth(line - 1).unwrap_or_default().trim(),
        "fn one_of_them(a: A) -> i64 throws { return a.add(1) }",
        "the entry should be the statement in `one_of_them`"
    );
}

/// A call nothing describes says nothing here.
///
/// The checker answers only where the fact is written down (C.4). A foreign
/// function has no contract by definition, so this is silence and not
/// approval - `rustc` still type-checks the emitted crate, and ADR-005 D7's
/// translation reports its refusal against this same `.nika` line.
#[test]
fn a_callee_no_ledger_describes_is_not_guessed_at() {
    assert!(findings("fn ruft() -> i64 { return fremd::macht_irgendwas(1) }").is_empty());
}
