//! A write through the brackets, and why it is not an index
//! ([ADR-080](../../../docs/specification/adr/adr-080.md) D2).
//!
//! **Part I 4.5's own three-line map example did not compile.** `scores[k] = v`
//! lowered to an indexed assignment, and Rust's `Index` for a map is over
//! whatever the key *borrows* as — so indexing a `HashMap<K, V>` with a `&str`
//! leaves `K` unpinned:
//!
//! ```text
//! error[E0282]: type annotations needed for
//!               `HashMap<_, i32, BuildHasherDefault<FxHasher>>`
//! help: consider giving `scores` an explicit type, where the type for type
//!       parameter `K` is specified
//! ```
//!
//! `TrustedMap`, `BuildHasherDefault<FxHasher>` and a type parameter `K`: three
//! spellings the program never wrote, in a message about a file nobody wrote.
//! [Part III C.1](../../../docs/specification/30-nikaia-tooling.md)'s class at
//! its worst, because it names this compiler's own internal word for a map.
//!
//! These are run rather than read: whether the key type is pinned is a question
//! only the language below can answer, and comparing the emitted string would
//! say no more than that this compiler agrees with itself.

mod common;

use nikaia::check;
use nikaia::contracts::{Ledger, STD};
use nikaia::emit::{emit_program, Build};
use nikaia::parser::parse_to_ast;

fn lowered(purpose: &str, source: &str) -> String {
    let parsed = parse_to_ast(source).expect("the source parses");
    let own = Ledger::infer(&parsed);
    let library = Ledger::parse(STD).expect("std's shipped ledger parses");
    let found =
        check::check_program(&parsed, &own, &library, &std::collections::BTreeSet::new()).findings;
    assert!(
        found.is_empty(),
        "{purpose} is a correct program and the checker says otherwise: {found:#?}"
    );
    emit_program(&parsed, Build::default())
        .expect("the source lowers")
        .rust
}

/// Compile and run, hand back what it printed.
fn ran(purpose: &str, source: &str) -> String {
    let rust = lowered(purpose, source);
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

/// Part I 4.5's own example, compiled and run.
#[test]
fn a_map_written_through_the_brackets_compiles_and_runs() {
    let printed = ran(
        "a map written through the brackets",
        r#"

fn main() {
    let mut scores = HashMap()
    scores["Player1"] = 100
    scores["Player2"] = 7
    println(f"{scores[\"Player1\"]} {scores[\"Player2\"]}")
}
"#,
    );
    assert_eq!(printed.trim(), "100 7");
}

/// D2: the write is an `insert`, which takes the key **by value** and is what
/// pins it. The read stays an index, because a read is one.
#[test]
fn the_write_is_a_set_and_the_read_is_still_an_index() {
    let rust = lowered(
        "a map's two directions",
        r#"

fn main() {
    let mut scores = HashMap()
    scores["Player1"] = 100
    println(f"{scores[\"Player1\"]}")
}
"#,
    );
    assert!(
        rust.contains(
            "nikaia_std::index::set(&mut scores, nikaia_std::index::at(\"Player1\"), 100)"
        ),
        "the write goes through `set`:\n{rust}"
    );
    assert!(
        rust.contains("scores[nikaia_std::index::at(\"Player1\")]"),
        "and the read is still an index:\n{rust}"
    );
}

/// **A sequence is written the same way and means the same thing**, which is
/// what makes this one rule rather than two: `Set` for a `Vec` *is* an indexed
/// assignment, chosen by the language below on the container's type because this
/// emitter does not know it (ADR-011 D2).
#[test]
fn a_sequence_written_through_the_brackets_still_works() {
    let printed = ran(
        "a sequence written through the brackets",
        r#"
fn main() {
    let mut xs = Vec()
    xs.push(10)
    xs.push(20)
    xs[0] = 99
    println(f"{xs[0]} {xs[1]}")
}
"#,
    );
    assert_eq!(printed.trim(), "99 20");
}

/// **A compound write is left alone**, and that is the decision rather than an
/// omission: `xs[0] += 1` reads the slot as well as writing it, so it is an
/// `Index` either way — and saying what reading an absent key means is a
/// question of its own (ADR-080 §4).
#[test]
fn a_compound_write_is_still_an_indexed_assignment() {
    let rust = lowered(
        "a compound write",
        r#"
fn main() {
    let mut xs = Vec()
    xs.push(10)
    xs[0] += 1
    println(f"{xs[0]}")
}
"#,
    );
    assert!(
        !rust.contains("index::set"),
        "a compound write is not a `set`:\n{rust}"
    );
    // The `at` is absent here and that is a different rule: a **constant**
    // index needs no conversion, because the fold already knows it fits
    // ([ADR-048](../../../docs/specification/adr/adr-048.md) D1). What this
    // asserts is the form, not the spelling of the subscript.
    assert!(
        rust.contains("xs[0] += 1"),
        "it keeps the indexed form:\n{rust}"
    );
}

/// And both together, run: the sequence, its compound write, and the map.
#[test]
fn a_sequence_and_a_map_in_one_program() {
    let printed = ran(
        "both containers",
        r#"

fn main() {
    let mut xs = Vec()
    xs.push(10)
    xs.push(20)
    xs[0] = 99
    xs[1] += 1

    let mut scores = HashMap()
    scores["a"] = 1

    println(f"{xs[0]} {xs[1]} {scores[\"a\"]}")
}
"#,
    );
    assert_eq!(printed.trim(), "99 21 1");
}
