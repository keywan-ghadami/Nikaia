//! A field of a borrowed subject, handed out by value
//! ([ADR-083](../../../docs/specification/adr/adr-083.md)).
//!
//! **Part I 6.8 is what decides this**, and it decides it in the language's own
//! words rather than in a judgement of mine:
//!
//! > Ownership rules occasionally reject code […] every such error explains
//! > itself in plain language and tells you what to do next. You never need Rust
//! > knowledge to read a Nikaia error; **if a raw internal (Rust) error ever
//! > reaches you, that is a Nikaia bug.**
//!
//! `return self.username` out of a `&self` method was exactly that: an
//! ownership rule rejecting code, with `E0507` reaching the user about a file
//! nobody wrote.
//!
//! **The polarity is what these tests are mostly about.** A refusal that fires
//! where it should not is worse than the leak it replaces —
//! [Part III C.4](../../../docs/specification/30-nikaia-tooling.md) says this
//! compiler never refuses a correct program — so every shape that worked before
//! is here, run, beside every shape that leaked.

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

fn ran(purpose: &str, source: &str) -> String {
    let found = findings(source);
    assert!(
        found.is_empty(),
        "{purpose} is a correct program and the checker says otherwise: {found:#?}"
    );
    let parsed = parse_to_ast(source).expect("the source parses");
    let rust = emit_program(&parsed, Build::default())
        .expect("the source lowers")
        .rust;
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

/// A struct with one field that moves and one that copies, for both directions.
fn around(body: &str, call: &str) -> String {
    format!(
        r#"
fn takes(s: String) -> i64 {{
    return 1
}}

struct Row {{
    name: String,
    count: i64,
}}

impl Row {{
{body}
}}

fn main() {{
    let r = Row {{ name: "a".to_string(), count: 1 }}
    println(f"{{{call}}}")
}}
"#
    )
}

/// **The three positions that leaked**, each refused by name.
///
/// Measured as a class before anything was built: a field read off a borrowed
/// subject leaks by being handed back, by being bound, and by being passed —
/// and nowhere else.
#[test]
fn every_by_value_position_is_refused() {
    let positions = [
        (
            "handed back",
            "    fn m(&self) -> String {\n        return self.name\n    }",
        ),
        (
            "bound",
            "    fn m(&self) -> i64 {\n        let x = self.name\n        return 1\n    }",
        ),
        (
            "passed",
            "    fn m(&self) -> i64 {\n        return takes(self.name)\n    }",
        ),
    ];
    for (what, body) in positions {
        let found = findings(&around(body, "r.m()"));
        let refusal = found
            .iter()
            .find(|f| f.code == "NK1131")
            .unwrap_or_else(|| panic!("a field {what} by value is refused: {found:#?}"));
        assert!(
            refusal.message.contains("name") && refusal.message.contains(what),
            "it names the field and what was done with it: {}",
            refusal.message
        );
        assert!(
            refusal
                .help
                .as_deref()
                .is_some_and(|h| h.contains("&self.name")
                    && h.contains(".clone()")
                    && h.contains("(self)")),
            "and names all three ways out, the free one first: {:?}",
            refusal.help
        );
    }
}

/// **A field that copies is not refused**, because handing one out takes
/// nothing away. This is half the polarity and it is why the rule reads the
/// field's type rather than counting `&self`s.
#[test]
fn a_field_that_copies_is_handed_out_freely() {
    let printed = ran(
        "a copying field",
        &around(
            "    fn m(&self) -> i64 {\n        return self.count\n    }",
            "r.m()",
        ),
    );
    assert_eq!(printed.trim(), "1");
}

/// The way out the message names first, run.
#[test]
fn a_written_clone_is_the_way_out() {
    let printed = ran(
        "a written clone",
        &around(
            "    fn m(&self) -> String {\n        return self.name.clone()\n    }",
            "r.m()",
        ),
    );
    assert_eq!(printed.trim(), "a");
}

/// And the other: a method that means to consume its subject says so.
#[test]
fn a_method_that_owns_its_subject_may_hand_the_field_out() {
    let printed = ran(
        "an owning receiver",
        r#"
struct Row {
    name: String,
}

impl Row {
    fn into_name(self) -> String {
        return self.name
    }
}

fn main() {
    let r = Row { name: "a".to_string() }
    println(r.into_name())
}
"#,
    );
    assert_eq!(printed.trim(), "a");
}

/// **Reading a field is not taking it**, which is the case that would have made
/// this refusal useless: a method call on the field, its length, and its use in
/// an interpolation all borrow, and all still work.
#[test]
fn reading_a_field_is_not_taking_it() {
    let printed = ran(
        "reading rather than taking",
        r#"
struct Row {
    name: String,
    count: i64,
}

impl Row {
    fn shout(&self) -> String {
        return self.name.to_uppercase()
    }

    fn size(&self) -> i64 {
        return self.name.len()
    }

    fn label(&self) -> String {
        return f"{self.name}: {self.count}"
    }
}

fn main() {
    let r = Row { name: "a".to_string(), count: 1 }
    println(f"{r.shout()} {r.size()} {r.label()}")
}
"#,
    );
    assert_eq!(printed.trim(), "A 1 a: 1");
}

/// A **free function** is not a method, so nothing here is borrowed and nothing
/// is refused — including a local that happens to be called `self`… which it
/// cannot be (`NK1119`), so this is about the ordinary case.
#[test]
fn a_free_function_is_not_touched() {
    let found = findings(
        r#"
struct Row {
    name: String,
}

fn name_of(r: Row) -> String {
    return r.name
}

fn main() {
    let r = Row { name: "a".to_string() }
    println(name_of(r))
}
"#,
    );
    assert!(
        found.is_empty(),
        "nothing is borrowed in a free function: {found:#?}"
    );
}

/// **The way out the message leads with**, run — and it is the one the entry
/// that filed this said did not exist.
///
/// `&self.name` is [Part I 6.5](../../../docs/specification/10-nikaia-light.md)'s
/// own spelling (`let name = &config.name`), and `NK1104`'s help had been saying
/// *"write `&` to take a view of it"* the whole time. What is refused is
/// `return self.name` **without** the `&` against a `-> &str`, which is a
/// `String` in a `&str` slot — the same refusal a parameter gets (`NK1102`) and
/// a `let` gets (`NK1103`), so the rule is uniform rather than special here.
#[test]
fn a_view_of_the_field_is_the_free_way_out() {
    let printed = ran(
        "a view of the field",
        r#"
struct Row {
    name: String,
    tags: Vec[i64],
}

impl Row {
    fn name(&self) -> &str {
        return &self.name
    }

    fn tags(&self) -> &Vec[i64] {
        return &self.tags
    }
}

fn main() {
    let mut v = Vec::new()
    v.push(7)
    let r = Row { name: "a".to_string(), tags: v }
    println(f"{r.name()} {r.tags()[0]}")
}
"#,
    );
    assert_eq!(printed.trim(), "a 7");
}

/// And the help names the **field's own** view type, not always `&str`.
#[test]
fn the_help_names_the_view_the_field_would_have() {
    let found = findings(
        r#"
struct Row {
    tags: Vec[i64],
}

impl Row {
    fn t(&self) -> Vec[i64] {
        return self.tags
    }
}

fn main() {
    println("x")
}
"#,
    );
    let refusal = found
        .iter()
        .find(|f| f.code == "NK1131")
        .expect("a Vec field is moved too");
    assert!(
        refusal
            .help
            .as_deref()
            .is_some_and(|h| h.contains("`&Vec[i64]`")),
        "a `Vec` field's view is not `&str`: {:?}",
        refusal.help
    );
}
