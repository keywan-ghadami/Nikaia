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
///
/// **The checker runs too**, because a build runs it: without that a program
/// this compiler would have refused reaches `rustc` and fails there, and the
/// test then reports the wrong thing. (Written after exactly that: a `?.` on a
/// plain value came back as *"`User` is not an iterator"*.)
fn compiled(purpose: &str, source: &str) -> String {
    let parsed = parse_to_ast(source).expect("the source parses");
    let own = Ledger::infer(&parsed);
    let library = Ledger::parse(STD).expect("std's shipped ledger parses");
    let findings = check::check(&parsed, &own, &library).findings;
    assert!(
        findings.is_empty(),
        "{purpose} is a correct program and the checker says otherwise: {findings:#?}"
    );

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

// --- Part I 3.5's `?.` ------------------------------------------------------

/// **The section's own shape**, chained, and it compiles.
///
/// `map` over a plain field and `and_then` over one that is itself a `T?`: the
/// second is the whole difficulty, because `map` there would give an
/// `Option<Option<T>>` and `a?.b?.c` would come out holding a nullable of a
/// nullable. Which of the two is right is a question about the declared type,
/// so the checker decides and this emitter writes the word
/// ([ADR-052](../../../docs/specification/adr/adr-052.md) §4,
/// [ADR-028](../../../docs/specification/adr/adr-028.md)).
#[test]
fn a_safe_reach_maps_over_a_plain_field_and_flattens_a_nullable_one() {
    let rust = compiled(
        "safe-navigation",
        "\
struct Address { city: String, zip: String? }
struct User { name: String, home: Address? }

fn find(id: i64) -> User? {
    if id > 0 {
        let home = Address(city: \"Bletchley\".to_string(), zip: null)
        return User(name: \"Ada\".to_string(), home: home)
    }
    return null
}

fn main() {
    let name = find(1)?.name ?? \"nobody\".to_string()
    let city = find(1)?.home?.city ?? \"nowhere\".to_string()
    let zip = find(1)?.home?.zip ?? \"none\".to_string()
    println(f\"{name} {city} {zip}\")
}
",
    );
    // `name` is a plain `String`, so `map`.
    assert!(
        rust.contains(".map(|__nikaia_it| __nikaia_it.name)"),
        "{rust}"
    );
    // `home` is an `Address?`, so `and_then` — otherwise `?.home?.city` would
    // be reaching through a nullable of a nullable.
    assert!(
        rust.contains(".and_then(|__nikaia_it| __nikaia_it.home)"),
        "{rust}"
    );
    // And `zip` is a `String?`, so `and_then` again.
    assert!(
        rust.contains(".and_then(|__nikaia_it| __nikaia_it.zip)"),
        "{rust}"
    );
}

/// **A struct-literal field is Part I 2.3's fourth position for the wrap.**
///
/// `Address(city: …, zip: null)` needs nothing, and `User(name: …, home: home)`
/// where `home` is an `Address` and the field is an `Address?` needs the
/// `Some(…)`. Keyed by the field's own name, because a struct literal has one of
/// these per field and a statement has only one span.
#[test]
fn a_plain_value_in_a_nullable_field_is_wrapped() {
    let rust = compiled(
        "nullable-field",
        "\
struct Address { city: String }
struct User { name: String, home: Address? }

fn main() {
    let home = Address(city: \"Bletchley\".to_string())
    let u = User(name: \"Ada\".to_string(), home: home)
    let city = u.home?.city ?? \"nowhere\".to_string()
    println(f\"{city}\")
}
",
    );
    assert!(rust.contains("home: Some(home)"), "{rust}");
}

/// **`?.` through something that cannot be absent is `NK1121`.**
///
/// The language below has no `map` on a plain struct, so this was `rustc`'s
/// refusal about the generated file. The way out is the plain `.`, which is
/// what the program meant.
#[test]
fn reaching_through_a_plain_value_is_refused() {
    let source = "\
struct User { name: String }
fn main() {
    let u = User(name: \"Ada\".to_string())
    let n = u?.name
    println(f\"{n}\")
}
";
    let parsed = parse_to_ast(source).expect("parses");
    let own = Ledger::infer(&parsed);
    let library = Ledger::parse(STD).expect("std's shipped ledger parses");
    let findings = check::check(&parsed, &own, &library).findings;
    let it = findings
        .iter()
        .find(|f| f.code == "NK1121")
        .unwrap_or_else(|| panic!("no NK1121: {findings:#?}"));
    assert!(it.message.contains("`User`"), "{it:#?}");
    assert!(
        it.help.as_deref() == Some("write `.name`"),
        "every error names a way out (Part III C.2): {it:#?}"
    );
}

/// And a receiver this checker could not work out says nothing: refusing there
/// would refuse a correct program (Part III, C.4).
#[test]
fn reaching_through_an_unknown_receiver_is_not_refused() {
    let source = "\
fn main() {
    let whatever = cli::args().nth(1)
    let n = whatever?.something
}
";
    let parsed = parse_to_ast(source).expect("parses");
    let own = Ledger::infer(&parsed);
    let library = Ledger::parse(STD).expect("std's shipped ledger parses");
    let findings = check::check(&parsed, &own, &library).findings;
    assert!(
        !findings.iter().any(|f| f.code == "NK1121"),
        "{findings:#?}"
    );
}

/// **`a ?? b` is not `a?.b`**, and the grammar keeps them apart by spelling
/// `?.` as one token: the generator cannot insert the implicit whitespace
/// inside a literal, so `a ? . b` is not safe navigation either.
#[test]
fn the_coalescing_operator_is_not_a_safe_reach() {
    let rust = lowered("fn main() { let a: i64? = null\nlet b = a ?? 1 }");
    assert!(rust.contains("unwrap_or_else"), "{rust}");
    assert!(!rust.contains("map("), "{rust}");

    assert!(
        parse_to_ast("fn main() { let a: i64? = null\nlet b = a ? . x }").is_err(),
        "`?` and `.` apart is not `?.`"
    );
}
