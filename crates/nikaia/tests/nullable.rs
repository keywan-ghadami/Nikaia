//! Part I 2.3's nullable types, and Part I 3.5's `??` over one.
//!
//! **The section's own example did not parse.** A trailing `?` on a type was a
//! parse error and `null` was read as an ordinary name, so neither line of it
//! was accepted — which is what `docs/open-work.md` carried. These are the
//! programs that page names, compiled rather than only compared as text:
//! whether the `Some(…)` lands in the right places is settled by the language
//! below, and reading the emitted string would only say that this compiler
//! agrees with itself.

mod common;

use nikaia::check;
use nikaia::contracts::{ty::Ty, Ledger, STD};
use nikaia::emit::{emit_program, Build};
use nikaia::parser::parse_to_ast;

fn lowered(source: &str) -> String {
    let parsed = parse_to_ast(source).expect("the source parses");
    emit_program(&parsed, Build::default())
        .expect("the source lowers")
        .rust
}

/// Lower, compile the result as a Rust library, and hand the emitted text back.
fn compiled(purpose: &str, source: &str) -> String {
    let rust = lowered(source);
    let dir = common::scratch_dir(purpose);
    let file = dir.join("lowered.rs");
    std::fs::write(&file, &rust).expect("write the Rust");

    let out = common::compile(
        &file,
        &[
            "--crate-type",
            "lib",
            "--emit=metadata",
            "-o",
            dir.join("lowered.rmeta").to_str().expect("utf-8 path"),
        ],
    );
    assert!(
        out.status.success(),
        "the lowering of {purpose} does not compile:\n{}\n--- the Rust ---\n{rust}",
        String::from_utf8_lossy(&out.stderr),
    );

    let _ = std::fs::remove_dir_all(&dir);
    rust
}

/// **Part I 2.3's example, both lines.**
///
/// `&str?` is an `Option<&str>` and `null` is `None`. The `?` is peeled before
/// the rest of the type is rendered, which is what keeps the view a view: the
/// wrapper is applied to the type and not to its name.
#[test]
fn the_sections_own_example_lowers() {
    let rust = compiled(
        "nullable-example",
        "\
fn main() {
    let strictly_string: String = \"Hello\".to_string()
    let mut maybe_string: String? = null
    maybe_string = \"World\".to_string()
    let shown = maybe_string ?? \"nothing\".to_string()
    println(f\"{strictly_string} {shown}\")
}
",
    );
    assert!(
        rust.contains("let mut maybe_string: Option<String> = None;"),
        "{rust}"
    );
    // The second line is the one that needs the constructor written: a
    // `String` standing where a `String?` is wanted.
    assert!(
        rust.contains("maybe_string = Some(\"World\".to_string());"),
        "{rust}"
    );
}

/// **A nullable result, and the `Some(…)` at a `return`.**
///
/// `return 42` against a declared `i64?` is the case a type alone cannot answer:
/// a number has no type of its own on purpose (Part I 2.4), so the checker reads
/// the *value* as well and a literal is never a `T?`.
#[test]
fn a_function_may_hand_back_a_nullable() {
    let rust = compiled(
        "nullable-return",
        "\
fn score(id: i64) -> i64? {
    if id > 0 {
        return 42
    }
    return null
}

fn main() {
    let hit = score(1) ?? 0
    let miss = score(0) ?? 0
    println(f\"{hit} {miss}\")
}
",
    );
    assert!(rust.contains("fn score(id: i64) -> Option<i64>"), "{rust}");
    assert!(rust.contains("return Some(42);"), "{rust}");
    // `null` needs no wrap, being a `T?` itself — and as the body's tail it
    // keeps no `return` either (Part I 3.1).
    assert!(rust.contains("None"), "{rust}");
    assert!(!rust.contains("Some(None)"), "{rust}");
}

/// **A nullable is not wrapped twice.** Handing a `T?` where a `T?` is wanted
/// needs nothing, and a value whose type this checker could not work out is
/// left alone rather than guessed at — wrapping one that is already an
/// `Option<T>` would make an `Option<Option<T>>`.
#[test]
fn a_value_that_is_already_nullable_is_not_wrapped() {
    let rust = compiled(
        "nullable-no-double-wrap",
        "\
fn score(id: i64) -> i64? {
    return null
}

fn main() {
    let mut a: i64? = null
    a = score(1)
    let shown = a ?? 0
    println(f\"{shown}\")
}
",
    );
    assert!(rust.contains("a = score(1);"), "{rust}");
    assert!(!rust.contains("Some(score(1))"), "{rust}");
}

/// **A `T?` round-trips through the ledger as `T?`**, and never as
/// `Option[T]` — which is a name no program can write (Part III, C.1). The
/// specification's own mapping is Rust `Option<T>` to Nikaia `T?`
/// (Part III, 15.2).
#[test]
fn a_nullable_type_round_trips_through_its_text() {
    for text in ["i64?", "&str?", "String?", "Vec[i64]?", "Vec[i64?]"] {
        let parsed = Ty::parse(text);
        assert_eq!(parsed.text(), text, "{text} does not come back as itself");
    }

    // `?` alone is `Unknown`, which is the one spelling this shares a character
    // with, and the two stay apart.
    assert!(Ty::parse("?").is_unknown());
    assert!(!Ty::parse("i64?").is_unknown());
}

/// **The one widening this language has**, and only in that direction.
///
/// A plain `T` fits a `T?`, because Part I 2.3 writes it: `let mut m: &str? =
/// null` and then `m = "World"`. A `T?` does not fit a `T` — that is the whole
/// point of the type being separate, and `??` (3.5) is how a program gets from
/// one to the other.
#[test]
fn a_plain_value_fits_a_nullable_slot_and_not_the_other_way() {
    let plain = Ty::parse("i64");
    let nullable = Ty::parse("i64?");
    assert!(plain.fits(&nullable), "a `T` stands where a `T?` is wanted");
    assert!(!nullable.fits(&plain), "a `T?` does not stand for a `T`");

    // And two nullables fit when what they may hold fits.
    assert!(nullable.fits(&Ty::parse("i64?")));
    assert!(!nullable.fits(&Ty::parse("String?")));
}

/// A `T?` where a `T` is wanted is refused, with both types in this language's
/// words.
#[test]
fn handing_a_nullable_where_a_plain_value_is_wanted_is_refused() {
    let source = "\
fn plain(n: i64) -> i64 { return n }
fn main() {
    let maybe: i64? = null
    println(f\"{plain(maybe)}\")
}
";
    let parsed = parse_to_ast(source).expect("parses");
    let own = Ledger::infer(&parsed);
    let library = Ledger::parse(STD).expect("std's shipped ledger parses");
    let findings = check::check(&parsed, &own, &library).findings;
    let it = findings
        .iter()
        .find(|f| f.code == "NK1102")
        .unwrap_or_else(|| panic!("no NK1102: {findings:#?}"));
    assert!(it.message.contains("i64?"), "{it:#?}");
    assert!(
        !it.message.contains("Option"),
        "a message may not name a type the program cannot write (C.1): {it:#?}"
    );
}
