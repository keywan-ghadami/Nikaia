//! A `for` lends, and a `let` over a place is a view of it
//! ([ADR-094](../../../docs/specification/adr/adr-094.md) D4) — the second of
//! that record's five steps.
//!
//! **This one changes what existing programs mean**, which the first step
//! deliberately did not. `for e in entries { … }` used to take `entries` away;
//! it leaves it where it was now, so `entries.len()` on the next line is a
//! program rather than `rustc`'s *use of moved value* about a file nobody
//! wrote. Iteration that takes the elements away is **written** —
//! `for x in xs.drain()` — because removing a name from scope is the rare case
//! and the one worth a word.
//!
//! **Off the shape of the expression and not off a column**, which is why this
//! step needs no ledger: a *place* is lent, and a call, a range or a literal
//! owns what it made. The one question the shape cannot answer is whether a
//! place's value would **move**, and that one the checker answers — `let mi =
//! self.bodies[i].mass` over an `f64` is a copy, and a `&` there is a borrow
//! held across the loop that writes the same field.

mod common;

use std::process::Command;

use nikaia::emit::{emit_program, Build};
use nikaia::parser::parse_to_ast;

/// The Rust this source lowers to.
fn lowered(source: &str) -> String {
    let parsed = parse_to_ast(source).expect("the source parses");
    emit_program(&parsed, Build::default())
        .expect("the source lowers")
        .rust
}

/// Compile the lowering and run it, which is the only proof that the borrow it
/// now writes is one the language below accepts.
fn ran(purpose: &str, source: &str) -> String {
    let rust = lowered(source);
    let dir = common::scratch_dir(&format!("lending-{purpose}"));
    let path = dir.join("program.rs");
    std::fs::write(&path, &rust).expect("write the Rust");
    let binary = dir.join("program");
    let compiled = common::compile(
        &path,
        &[
            "--crate-type",
            "bin",
            "-o",
            binary.to_str().expect("utf-8 path"),
        ],
    );
    assert!(
        compiled.status.success(),
        "{purpose} did not compile:\n{}\n--- emitted ---\n{rust}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    let out = Command::new(&binary).output().expect("run it");
    assert!(
        out.status.success(),
        "{purpose} failed:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let printed = String::from_utf8_lossy(&out.stdout).to_string();
    std::fs::remove_dir_all(&dir).ok();
    printed
}

/// Every finding the checker has about a source.
fn findings(source: &str) -> Vec<nikaia::check::Finding> {
    let parsed = parse_to_ast(source).expect("the source parses");
    let own = nikaia::contracts::Ledger::infer(&parsed);
    let library =
        nikaia::contracts::Ledger::parse(nikaia::contracts::STD).expect("std ships a ledger");
    nikaia::check::check_program(&parsed, &own, &library, &std::collections::BTreeSet::new())
        .findings
}

/// **The line the record was written for.** `entries.len()` after the loop used
/// to be `rustc`'s *use of moved value*; it is a program.
#[test]
fn a_for_leaves_the_collection_where_it_was() {
    let printed = ran(
        "for-lends",
        "fn main() {\n\
         \x20   let mut xs = Vec()\n\
         \x20   xs.push(1)\n\
         \x20   xs.push(2)\n\
         \x20   let mut sum = 0\n\
         \x20   for x in xs { sum += x }\n\
         \x20   println(f\"{sum} of {xs.len()}\")\n\
         }\n",
    );
    assert_eq!(printed.trim(), "3 of 2");
}

/// **And a call, a range or a method call owns what it made.** There is nothing
/// to lend: the value did not exist before the expression that names it.
#[test]
fn a_for_over_something_that_is_not_a_place_owns_it() {
    let rust = lowered(
        "fn main() {\n\
         \x20   let mut sum = 0\n\
         \x20   for i in 0..3 { sum += i }\n\
         \x20   let text = \"abc\".to_string()\n\
         \x20   for c in text.chars() { sum += 1 }\n\
         \x20   println(f\"{sum}\")\n\
         }\n",
    );
    assert!(rust.contains("for i in 0..3"), "{rust}");
    assert!(!rust.contains("0..3.iter()"), "{rust}");
    assert!(rust.contains(".chars()"), "{rust}");
    assert!(!rust.contains(".chars().iter()"), "{rust}");
}

/// **`.iter()` and not `&`**, which is a measurement rather than a preference.
///
/// The iterated name may already *be* a view — a parameter declared
/// `&Vec[Entry]` — and `&entries` is then a `&&Vec<Entry>`, which Rust does not
/// iterate. `.iter()` reads the same through any number of references, and this
/// emitter has no types to tell the two apart with
/// ([ADR-028](../../../docs/specification/adr/adr-028.md)). Found by
/// `examples/inventory/stock.nika`, whose `total` takes exactly that parameter.
#[test]
fn a_for_over_a_parameter_that_is_already_a_view_still_iterates() {
    let printed = ran(
        "for-over-a-view",
        "fn total(xs: &Vec[i64]) -> i64 {\n\
         \x20   let mut sum = 0\n\
         \x20   for x in xs { sum += x }\n\
         \x20   return sum\n\
         }\n\
         \n\
         fn main() {\n\
         \x20   let mut xs = Vec()\n\
         \x20   xs.push(4)\n\
         \x20   xs.push(5)\n\
         \x20   println(f\"{total(&xs)}\")\n\
         }\n",
    );
    assert_eq!(printed.trim(), "9");
}

/// **Taking the elements away is written**, and below it is `into_iter`: D4's
/// words are *"removing a name from scope"*, which is consuming the container
/// rather than emptying one somebody still holds.
///
/// It is what a body that hands an element to a callee which **keeps** it has
/// to say — `all.push(v)` in `examples/json.nika`, and `or_insert(counts)` in
/// `access-log.nika`, both of which this step rewrote.
#[test]
fn a_drain_takes_the_elements_away() {
    let printed = ran(
        "for-drains",
        "fn main() {\n\
         \x20   let mut xs = Vec()\n\
         \x20   xs.push(\"a\".to_string())\n\
         \x20   xs.push(\"b\".to_string())\n\
         \x20   let mut all = Vec()\n\
         \x20   for x in xs.drain() { all.push(x) }\n\
         \x20   println(f\"{all.len()}\")\n\
         }\n",
    );
    assert_eq!(printed.trim(), "2");

    let rust = lowered(
        "fn main() {\n\
         \x20   let mut xs = Vec()\n\
         \x20   xs.push(1)\n\
         \x20   for x in xs.drain() { println(f\"{x}\") }\n\
         }\n",
    );
    assert!(rust.contains(".into_iter()"), "{rust}");
    assert!(!rust.contains(".drain()"), "{rust}");
}

/// **`NK1137`: the `&` is the compiler's to write.** A `for` lends whatever
/// place it is given, so one written in front says the line twice — and left
/// alone it is a reference to a reference, which Rust does not iterate.
#[test]
fn a_written_ampersand_in_a_for_head_is_refused() {
    let found = findings(
        "fn main() {\n\
         \x20   let mut xs = Vec()\n\
         \x20   xs.push(1)\n\
         \x20   for x in &xs { println(f\"{x}\") }\n\
         }\n",
    );
    assert!(found.iter().any(|f| f.code == "NK1137"), "{found:#?}");

    // And the same loop without it is not refused, which is the half that says
    // the rule is about the `&` and not about the loop.
    let clean = findings(
        "fn main() {\n\
         \x20   let mut xs = Vec()\n\
         \x20   xs.push(1)\n\
         \x20   for x in xs { println(f\"{x}\") }\n\
         }\n",
    );
    assert!(!clean.iter().any(|f| f.code == "NK1137"), "{clean:#?}");
}

/// **A `let` over a place is a view of it**: the language below refuses to move
/// a value out of a container, so a move there was never what the line meant.
#[test]
fn a_let_over_a_place_is_a_view() {
    let rust = lowered(
        "struct Row { name: String }\n\
         struct Store { rows: Vec[Row] }\n\
         fn first(store: &Store) -> i64 {\n\
         \x20   let row = store.rows[0]\n\
         \x20   return row.name.len() as i64\n\
         }\n\
         fn main() { }\n",
    );
    assert!(rust.contains("let row = &store.rows["), "{rust}");
}

/// **And over a value that copies, it is not.**
///
/// `let mi = self.bodies[i].mass` over an `f64` is a copy, and a `&` there is a
/// borrow held across the loop that writes the same field — `E0502` about a
/// file nobody wrote. `examples/n-body.nika` is where that was met, and
/// [`moves_away`] is the same answer `NK2101` reads.
#[test]
fn a_let_over_a_place_that_copies_is_not_a_view() {
    let rust = lowered(
        "struct Body { mass: f64 }\n\
         struct World { bodies: Vec[Body] }\n\
         fn heavy(world: &World) -> f64 {\n\
         \x20   let m = world.bodies[0].mass\n\
         \x20   return m\n\
         }\n\
         fn main() { }\n",
    );
    assert!(!rust.contains("let m = &"), "{rust}");
}

/// **`let y = x` over a whole variable stays a move** — it is a rename, and the
/// one shape that separates this rule from `for`'s.
#[test]
fn a_let_over_a_whole_name_stays_a_move() {
    let rust = lowered(
        "fn main() {\n\
         \x20   let a = \"one\".to_string()\n\
         \x20   let b = a\n\
         \x20   println(f\"{b}\")\n\
         }\n",
    );
    assert!(rust.contains("let b = a;"), "{rust}");
}
