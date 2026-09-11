//! What a literal is, and what survives the lowering.
//!
//! Three of these came out of writing `examples/json.nika`, which is what the
//! examples are for: a character literal was not a thing the language had, a
//! string could not hold a `\u{…}` escape, and output could not be composed
//! without a newline attached to every piece.

use nikaia::ast::{Expr, Item, MatchPattern, Stmt};
use nikaia::emit::{emit_program, Profile};
use nikaia::parser::parse_to_ast;

fn emit(source: &str) -> String {
    let parsed = parse_to_ast(source).expect("the source parses");
    emit_program(&parsed, Profile::Advanced)
        .expect("the source lowers")
        .rust
}

/// Kap 2.2: `'a'` is a character, and its body is kept as written.
///
/// The language below spells a character literal the same way, so the lowering
/// is a transcription: nothing here decides what `\n` means, which is the same
/// bargain `LitStr` makes.
#[test]
fn a_character_literal_is_kept_as_written() {
    let parsed = parse_to_ast("fn main() { let c = '\\n' }").expect("parses");
    let Item::Fn { body, .. } = &parsed.program.items[0].node else {
        panic!("expected a function");
    };
    let Stmt::Let { value, .. } = &body.stmts[0].node else {
        panic!("expected a `let`");
    };
    assert!(matches!(value, Expr::LitChar(c) if c == "\\n"), "{value:?}");

    assert!(emit("fn main() { let c = '\\n' }").contains("let c = '\\n';"));
}

/// A quote and a backslash are characters like any other.
#[test]
fn the_awkward_characters_are_characters() {
    let emitted = emit("fn main() { let q = '\\'' let b = '\\\\' let s = ' ' }");
    assert!(emitted.contains("let q = '\\'';"), "{emitted}");
    assert!(emitted.contains("let b = '\\\\';"), "{emitted}");
    assert!(emitted.contains("let s = ' ';"), "{emitted}");
}

/// Kap 3.4: a character is a pattern, which is what makes a `match` over
/// `chars()` read as one - `examples/json.nika` decodes its escapes that way.
#[test]
fn a_character_literal_is_a_pattern() {
    let source = "fn f(c: char) { match c { 'n' => { println(\"newline\") } _ => { } } }";
    let parsed = parse_to_ast(source).expect("parses");
    let Item::Fn { body, .. } = &parsed.program.items[0].node else {
        panic!("expected a function");
    };
    let Stmt::Expr(Expr::Match { arms, .. }) = &body.stmts[0].node else {
        panic!("expected a `match`");
    };
    assert!(
        matches!(&arms[0].pattern, MatchPattern::Literal(Expr::LitChar(c)) if c == "n"),
        "{:?}",
        arms[0].pattern
    );
    assert!(emit(source).contains("'n' =>"), "{}", emit(source));
}

/// A `{` inside an escape is part of the escape, not a hole.
///
/// A string's body reaches the emitter as it was written - the parser keeps the
/// escapes rather than decoding them - so an interpolation scanner that does
/// not know that reads `"\u{0041}"` as a hole named `0041` and emits a
/// `format!` with an argument nobody wrote.
#[test]
fn a_unicode_escape_is_not_a_hole() {
    let emitted = emit(r#"fn main() { println("\u{0041}") }"#);
    assert!(emitted.contains(r#"println!("\u{0041}")"#), "{emitted}");
    assert!(!emitted.contains("0041)"), "{emitted}");
}

/// … and a hole beside one is still a hole.
#[test]
fn an_escape_does_not_swallow_the_interpolation_after_it() {
    let emitted = emit(r#"fn main() { let x = 1 println(f"\u{0041}{x}\n") }"#);
    assert!(
        emitted.contains(r#"println!("\u{0041}{}\n", x)"#),
        "{emitted}"
    );
}

/// `print` writes without a newline, which is what output composed piece by
/// piece needs - a pretty-printer cannot put one after every fragment.
#[test]
fn print_is_a_macro_like_println() {
    let emitted = emit(r#"fn main() { print("a") eprint("b") }"#);
    assert!(emitted.contains(r#"print!("a")"#), "{emitted}");
    assert!(emitted.contains(r#"eprint!("b")"#), "{emitted}");
}

/// A hole is Nikaia source that was written **inside** a string literal, so the
/// escaping it carries is that literal's and has to be undone before it is
/// parsed.
///
/// Without this the parser is handed `f(\"a\")`, which is not an expression -
/// and a program that wants to print the result of a call taking a string could
/// not say so.
#[test]
fn a_hole_may_hold_a_string_literal() {
    let emitted = emit(r#"fn f(s: &str) -> i32 { return 1 } fn main() { println(f"{f(\"a\")}") }"#);
    assert!(emitted.contains(r#"f("a")"#), "{emitted}");

    // A backslash the inner text wants keeps its meaning: only the two
    // characters the enclosing literal had to escape are undone.
    let escaped =
        emit(r#"fn f(s: &str) -> i32 { return 1 } fn main() { println(f"{f(\"a\\nb\")}") }"#);
    assert!(escaped.contains(r#"f("a\nb")"#), "{escaped}");
}

/// **A brace is a brace** (ADR-035 D1).
///
/// The case this was written for is `examples/json.nika`, where a program
/// whose job is to print `{` had to write `print("{{}}")` - and every string
/// in every program paid for a feature one string in four uses.
#[test]
fn a_plain_string_is_text_and_a_brace_is_part_of_it() {
    let emitted = emit(r#"fn main() { let x = "{ \"a\": 1 }" }"#);
    assert!(emitted.contains(r#"let x = "{ \"a\": 1 }";"#), "{emitted}");
}

/// …and Rust's macro must not read that brace as a hole of its own.
///
/// The one place the two languages disagree about a string literal: `print` is
/// a macro below, so a plain string's braces are doubled on the way down. That
/// is a transcription detail and not a rule anybody writes.
#[test]
fn a_plain_string_printed_keeps_its_braces() {
    let emitted = emit(r#"fn main() { print("{}") }"#);
    assert!(emitted.contains(r#"print!("{{}}")"#), "{emitted}");
}

/// An escape is copied whole, `\u{…}` included - doubling the braces inside one
/// would hand `println!` a `\u` with nothing after it.
#[test]
fn an_escape_is_not_a_brace_to_double() {
    let emitted = emit(r#"fn main() { println("\u{0041}") }"#);
    assert!(emitted.contains(r#"println!("\u{0041}")"#), "{emitted}");
}

/// `f"…"` is what says there is code in here (ADR-035 D1).
#[test]
fn an_f_string_interpolates_and_a_plain_one_does_not() {
    let woven = emit(r#"fn main() { let n = 1 let s = f"n is {n}" }"#);
    assert!(woven.contains(r#"format!("n is {}", n)"#), "{woven}");

    let plain = emit(r#"fn main() { let n = 1 let s = "n is {n}" }"#);
    assert!(plain.contains(r#"let s = "n is {n}";"#), "{plain}");
}

/// **The type comes from the syntax, not from the text** (ADR-035 D3).
///
/// Before the `f`, adding a brace to a string changed its type: `"a"` was a
/// view of static text and `"a {b}"` a `String`, and which one you had written
/// depended on whether the *content* happened to hold a hole. Now the first
/// character says it, so an `f"…"` with nothing in it is still a `String`.
#[test]
fn an_f_string_is_a_string_even_with_no_hole_in_it() {
    let emitted = emit(r#"fn main() { let s = f"no holes here" }"#);
    assert!(
        emitted.contains(r#""no holes here".to_string()"#),
        "{emitted}"
    );
}

/// The `f` and the quote are **one token**, so whitespace cannot change what a
/// program means: `f "x"` is the variable `f` beside a string, and it is not
/// an interpolation that lost its nerve.
#[test]
fn a_space_after_the_f_is_not_an_interpolation() {
    let parsed = parse_to_ast(r#"fn main() { let f = 1 let s = "{f}" }"#).expect("parses");
    let Item::Fn { body, .. } = &parsed.program.items[1 - 1].node else {
        panic!("expected a function");
    };
    let Stmt::Let { value, .. } = &body.stmts[1].node else {
        panic!("expected a `let`");
    };
    assert!(
        matches!(value, Expr::LitStr(_)),
        "a plain string became an interpolation: {value:?}"
    );
}
