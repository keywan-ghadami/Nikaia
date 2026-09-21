//! **Generating code from a type's shape** — Part II 10.3,
//! [ADR-088](../../../docs/specification/adr/adr-088.md) D2, D4 and D5, built
//! by [ADR-181](../../../docs/specification/adr/adr-181.md).
//!
//! That section was specified in full and built by halves: the **bound** landed
//! at 0.0.120 and what it reaches was `NK1171` — *this is specified and this
//! compiler does not have it* — until 0.0.129.
//!
//! **These tests run programs.** What an unrolling produces is a question the
//! language below answers: a test that compared the emitted Rust against a
//! string would pass for a copy that read the wrong field, and one that asked
//! the checker would pass for a loop that was never unrolled at all.

mod common;

use nikaia::check;
use nikaia::contracts::{Ledger, STD};
use nikaia::emit::{emit_program, Build};
use nikaia::parser::parse_to_ast;

fn findings(source: &str) -> Vec<check::Finding> {
    let parsed = parse_to_ast(source).expect("the source parses");
    let own = Ledger::infer(&parsed);
    let library = Ledger::parse(STD).expect("std's shipped ledger parses");
    check::check_program(&parsed, &own, &library, &std::collections::BTreeSet::new()).findings
}

fn lowered(source: &str) -> String {
    let found = findings(source);
    assert!(found.is_empty(), "a correct program: {found:#?}");
    let parsed = parse_to_ast(source).expect("the source parses");
    emit_program(&parsed, Build::default())
        .expect("the source lowers")
        .rust
}

fn ran(purpose: &str, source: &str) -> String {
    let rust = lowered(source);
    let dir = common::scratch_dir(purpose);
    let file = dir.join("main.rs");
    std::fs::write(&file, &rust).expect("write the Rust");
    let binary = dir.join("program");
    let out = common::compile(&file, &["-o", binary.to_str().expect("utf-8 path")]);
    assert!(
        out.status.success(),
        "the lowering of {purpose} does not compile:\n{}\n--- the Rust ---\n{rust}",
        String::from_utf8_lossy(&out.stderr),
    );
    let ran = std::process::Command::new(&binary)
        .output()
        .expect("the program runs");
    let printed = String::from_utf8_lossy(&ran.stdout).into_owned();
    let _ = std::fs::remove_dir_all(&dir);
    printed
}

/// **Part II 10.3's own example, compiled and run** — the first time anything
/// in that section was.
///
/// Two types, so the unrolling is per **instantiation** and not per function:
/// `describe__User` prints two fields and `describe__Point` prints two others,
/// from one `describe` the program wrote once.
#[test]
fn the_specifications_own_example_runs() {
    let source = "struct User { name: &str, age: i64 }\n\
                  struct Point { x: i64, y: i64 }\n\
                  \n\
                  fn describe[T: Struct](value: T) {\n\
                  \x20   for field in T::fields {\n\
                  \x20       println(f\"{field.name} = {field.of(value)}\")\n\
                  \x20   }\n\
                  }\n\
                  \n\
                  fn main() {\n\
                  \x20   describe(User { name: \"ada\", age: 36 })\n\
                  \x20   describe(Point { x: 1, y: 2 })\n\
                  }\n";
    let rust = lowered(source);
    // **Nothing is left at run time** (ADR-088 D5): no loop, no descriptor, no
    // dispatch — the field reads a program would have written by hand.
    assert!(!rust.contains("T::fields"), "{rust}");
    assert!(
        !rust.contains("fn describe<T>"),
        "the generic original: {rust}"
    );
    assert!(rust.contains("fn describe__User("), "{rust}");
    assert!(rust.contains("fn describe__Point("), "{rust}");
    assert!(rust.contains("\"name\", value.name"), "{rust}");

    assert_eq!(
        ran("the describe example", source),
        "name = ada\nage = 36\nx = 1\ny = 2\n"
    );
}

/// **A body wrong for one field is right for the others, and the message says
/// which turn it came from** ([ADR-088](../../../docs/specification/adr/adr-088.md)
/// D5).
///
/// `total + field.of(value)` is arithmetic for `age` and not for `name`, on one
/// line. Without the note this is the error class C++ templates carried for
/// twenty years: a message about a line that is correct for every other turn.
#[test]
fn a_body_wrong_for_one_field_names_the_turn() {
    let found = findings(
        "struct User { name: &str, age: i64 }\n\
         \n\
         fn sum[T: Struct](value: T) -> i64 {\n\
         \x20   let mut total: i64 = 0\n\
         \x20   for field in T::fields {\n\
         \x20       total = total + field.of(value)\n\
         \x20   }\n\
         \x20   return total\n\
         }\n\
         \n\
         fn main() { println(f\"{sum(User { name: \\\"ada\\\", age: 36 })}\") }\n",
    );
    assert_eq!(
        found.len(),
        1,
        "one turn is wrong and one is right: {found:#?}"
    );
    assert!(
        found[0]
            .notes
            .iter()
            .any(|n| n == "unrolling `T::fields` for `User`, at field `name`"),
        "{:#?}",
        found[0]
    );
}

/// **A reflected field answers two members**
/// ([ADR-088](../../../docs/specification/adr/adr-088.md) D2), and `NK1180`
/// prints the list, which is short enough to be a misspelling rather than
/// something nobody told the compiler about.
///
/// **Said once**, not once per field: the generic walk owns this message
/// ([ADR-181](../../../docs/specification/adr/adr-181.md) D3), because it is
/// about the function rather than about a turn.
#[test]
fn a_member_a_reflected_field_does_not_have_is_said_once() {
    let found = findings(
        "struct User { name: &str, age: i64 }\n\
         \n\
         fn d[T: Struct](value: T) {\n\
         \x20   for field in T::fields { println(field.label) }\n\
         }\n\
         \n\
         fn main() { d(User { name: \"a\", age: 1 }) }\n",
    );
    let refused: Vec<_> = found.iter().filter(|f| f.code == "NK1180").collect();
    assert_eq!(refused.len(), 1, "{found:#?}");
    assert!(
        refused[0].message.contains("has no `label`"),
        "{:#?}",
        refused[0]
    );
}

/// **A `[T: Struct]` function that never asks for the shape stays generic**
/// ([ADR-181](../../../docs/specification/adr/adr-181.md) D1).
///
/// The bound says a shape *may* be asked for; the body says whether it is. One
/// function below, generic in the language below, and no copies.
#[test]
fn a_bound_that_is_never_asked_is_an_ordinary_generic() {
    let rust = lowered(
        "struct User { name: &str }\n\
         \n\
         fn tell[T: Struct](value: T) { println(\"told\") }\n\
         \n\
         fn main() { tell(User { name: \"a\" }) }\n",
    );
    assert!(rust.contains("fn tell<T>("), "{rust}");
    assert!(!rust.contains("tell__User"), "{rust}");
}

/// **`variants` is the half that is not built**
/// ([ADR-181](../../../docs/specification/adr/adr-181.md) D4), and the refusal
/// says so rather than listing what has since arrived.
#[test]
fn variants_says_which_half_is_built() {
    let found = findings(
        "enum Shade { Odd, Even }\n\
         \n\
         fn tell[T: Enum](value: T) {\n\
         \x20   for v in T::variants { println(\"x\") }\n\
         }\n\
         \n\
         fn main() { tell(Shade::Odd) }\n",
    );
    let refused: Vec<_> = found.iter().filter(|f| f.code == "NK1171").collect();
    assert_eq!(refused.len(), 1, "{found:#?}");
    assert!(
        refused[0].notes[0].contains("`T::fields` are built"),
        "{:#?}",
        refused[0].notes
    );
    assert!(
        refused[0]
            .help
            .as_deref()
            .unwrap_or_default()
            .contains("`match`"),
        "{:#?}",
        refused[0]
    );
}
