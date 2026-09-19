//! An argument list of options alone
//! ([ADR-133](../../../docs/specification/adr/adr-133.md)).
//!
//! Part I 5.1 separates a call's **subjects** from its **options** with a `;`,
//! and a function whose only parameters are options was declared
//! `fn execute(; target_age: i64 = 0)` — a semicolon with nothing before it, a
//! shape no reader has seen in any language. The `;` earns its place where it
//! separates two zones; where there is only one zone it separates nothing.
//!
//! **The signature half is built and the call half is not**, and the reason is
//! not effort. D3 says *nothing in expression position begins with a name
//! followed by a colon*, and Kap 4.2's struct literal with named fields —
//! `Stats(min: first, max: first)` — is exactly that shape. The two constructs
//! share a spelling and a name denotes one of them, so what tells them apart is
//! resolution rather than the parser, and whether this language lets two
//! constructs share a spelling is a question: `docs/open-decisions.md` carries
//! it. What this file pins meanwhile is that the silence is gone — a literal for
//! a struct nothing declares is `NK1135` and its message names the call.

use nikaia::parser::parse_to_ast;

fn lowered(source: &str) -> Result<String, String> {
    let parsed = parse_to_ast(source).map_err(|e| format!("{e:#}"))?;
    nikaia::emit::emit_program(&parsed, nikaia::emit::Build::default())
        .map(|it| it.rust)
        .map_err(|e| format!("{e:#}"))
}

fn findings(source: &str) -> Vec<nikaia::check::Finding> {
    let parsed = parse_to_ast(source).expect("the source parses");
    let own = nikaia::contracts::Ledger::infer(&parsed);
    let library = nikaia::contracts::Ledger::parse(nikaia::contracts::STD).expect("std's ledger");
    nikaia::check::check(&parsed, &own, &library).findings
}

/// **A signature whose parameters are all options writes no `;`** (D1).
#[test]
fn a_signature_of_options_alone_needs_no_semicolon() {
    let rust = lowered(
        "fn execute(target_age: i64 = 0) -> i64 { return target_age }\n\
         fn main() { println(f\"{execute()}\") }",
    )
    .expect("it parses and lowers");
    // The language below has no defaults, so the option becomes positional and
    // the default is the value (Kap 5.1).
    assert!(rust.contains("execute(0)"), "{rust}");
}

/// **A mixed signature keeps its `;`, and keeps it required** (D1, D2).
///
/// The half a rule tried first could have broken: `bare_config_params` reads
/// `path: String` and then wants the `=` that makes a parameter an option, so it
/// has to fail at the `;` and let the positional list take it.
#[test]
fn a_mixed_signature_keeps_its_semicolon() {
    let rust = lowered(
        "fn read(path: String; trusted: bool = false) -> String {\n\
             if trusted { return path }\n\
             return \"no\".to_string()\n\
         }\n\
         fn main() { println(read(\"in\".to_string(); trusted: true)) }",
    )
    .expect("it parses and lowers");
    assert!(
        rust.contains("fn read(path: String, trusted: bool)"),
        "{rust}"
    );
    assert!(rust.contains("read(\"in\".to_string(), true)"), "{rust}");
}

/// **The leading `;` in a signature is refused, and the message names the new
/// form** (D2).
#[test]
fn a_leading_semicolon_in_a_signature_is_refused() {
    let said = lowered("fn execute(; target_age: i64 = 0) -> i64 { return target_age }")
        .expect_err("the leading form is a parse error");
    assert!(
        said.contains("writes its options without the `;`"),
        "{said}"
    );
    assert!(
        said.contains("fn execute(target_age: i64 = 0)"),
        "the message names the one spelling: {said}"
    );
}

/// **A plain signature is untouched**, which is what a rule tried first has to
/// leave alone.
#[test]
fn a_signature_of_subjects_alone_is_untouched() {
    let rust = lowered(
        "fn add(a: i64, b: i64) -> i64 { return a + b }\n\
         fn main() { println(f\"{add(1, 2)}\") }",
    )
    .expect("it lowers");
    assert!(rust.contains("fn add(a: i64, b: i64) -> i64"), "{rust}");
    // And the message a missing default gets stays the one that belongs after a
    // `;`, where a parameter can only be an option - reusing `config_param` here
    // would have put *a configuration parameter needs a default* on this.
    assert!(!rust.contains("needs a default"), "{rust}");
}

/// **A call whose arguments are all options lowers to valid Rust**, which is a
/// defect older than the record that names the shape.
///
/// `execute(; target_age: 30)` came out as `execute(, 30)` — the comma the
/// emitter writes *between* arguments, written before the first one because a
/// call of options alone has nothing in front of it. Invalid Rust, reported by
/// `rustc` about a file nobody wrote (Part III C.1), for the only spelling such a
/// call had. Nothing in the corpus declares such a function, which is why it
/// stood.
#[test]
fn a_call_of_options_alone_has_no_leading_comma() {
    let rust = lowered(
        "fn execute(target_age: i64 = 0) -> i64 { return target_age }\n\
         fn main() {\n\
             let a = execute(; target_age: 30)\n\
             let b = execute()\n\
             println(f\"{a} {b}\")\n\
         }",
    )
    .expect("it lowers");
    assert!(rust.contains("execute(30)"), "{rust}");
    assert!(rust.contains("execute(0)"), "{rust}");
    assert!(!rust.contains("execute(,"), "no leading comma: {rust}");
}

/// **A struct literal naming nothing this compiler declares is `NK1135`**, and
/// where the name is a function the message names the call.
///
/// The silence that let the collision through: `execute(target_age: 30)` is
/// Kap 4.2's literal shape, it was read as one, the arm that checks fields
/// `continue`d over a type with none, and the program lowered
/// `execute { target_age: 30 }` verbatim. `rustc` answered *cannot find struct
/// `execute`* about a file nobody wrote.
#[test]
fn a_struct_literal_naming_a_function_says_so() {
    let found: Vec<_> = findings(
        "fn execute(target_age: i64 = 0) -> i64 { return target_age }\n\
         fn main() { let a = execute(target_age: 30) println(f\"{a}\") }",
    )
    .into_iter()
    .filter(|f| f.code == "NK1135")
    .collect();
    assert_eq!(found.len(), 1, "one refusal: {found:#?}");
    let notes = found[0].notes.join(" ");
    assert!(
        notes.contains("Kap 4.2's struct literal"),
        "it says what the line reads as: {notes}"
    );
    assert!(
        notes.contains("is a function"),
        "and what the name is: {notes}"
    );
    let help = found[0].help.as_deref().expect("a way out");
    assert!(help.contains("execute(; option: value)"), "{help}");
}

/// **And a name that is no function either gets the plain sentence** — a `struct`
/// nobody declared, which is `NK1135`'s own claim.
#[test]
fn a_struct_literal_naming_nothing_says_that_instead() {
    let found: Vec<_> = findings("fn main() { let w = Widgit(size: 3) println(f\"{w.size}\") }")
        .into_iter()
        .filter(|f| f.code == "NK1135")
        .collect();
    assert_eq!(found.len(), 1, "one refusal: {found:#?}");
    assert!(
        found[0]
            .notes
            .join(" ")
            .contains("a struct literal names a type"),
        "{:#?}",
        found[0].notes
    );
}

/// **A struct literal for a struct that is declared is untouched**, which is the
/// half that matters: the refusal above must not reach Kap 4.2's own form.
#[test]
fn a_declared_structs_literal_is_not_refused() {
    let found: Vec<_> = findings(
        "struct Stats { min: i64, max: i64 }\n\
         fn main() { let s = Stats(min: 1, max: 2) println(f\"{s.min}\") }",
    )
    .into_iter()
    .filter(|f| f.code == "NK1135")
    .collect();
    assert!(found.is_empty(), "{found:#?}");
}

/// **A qualified name is left alone**, which is `NK1135`'s own convention:
/// whether this build can see that package is a question with its own message.
#[test]
fn a_qualified_literal_is_left_alone() {
    let found: Vec<_> = findings("fn main() { let c = pool::Conn(id: 1) println(f\"{c.id}\") }")
        .into_iter()
        .filter(|f| f.code == "NK1135")
        .collect();
    assert!(found.is_empty(), "{found:#?}");
}
