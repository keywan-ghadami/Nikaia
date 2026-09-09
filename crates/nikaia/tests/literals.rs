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
    let emitted = emit(r#"fn main() { let x = 1 println("\u{0041}{x}\n") }"#);
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
