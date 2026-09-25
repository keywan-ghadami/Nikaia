//! Owned keys, literals where text is kept, and data used after it was handed
//! over ([ADR-213](../../../docs/specification/adr/adr-213.md)).
//!
//! Each was `rustc`'s words about a file nobody wrote: a map keyed by an owned
//! `String` could not be written at all, a map keyed by an `i64` had its key
//! made a `usize`, `m[k] ?? "-"` over a map of text did not compile, a literal
//! assigned to a `String` was refused, and `xs.push(name)` followed by
//! `println(name)` was *borrow of moved value*. ADR-094 D2 decided that last one
//! is refused in this language's words, and nothing did.

mod common;

use std::process::Command;

use nikaia::emit::{emit_program, Build};
use nikaia::parser::parse_to_ast;

fn lowered(source: &str, how: Build) -> String {
    let parsed = parse_to_ast(source).expect("the source parses");
    emit_program(&parsed, how).expect("the source lowers").rust
}

fn ran(purpose: &str, source: &str, how: Build) -> String {
    let rust = lowered(source, how);
    let dir = common::scratch_dir(&format!("handed-{purpose}"));
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
    let printed = String::from_utf8_lossy(&out.stdout).trim().to_string();
    std::fs::remove_dir_all(&dir).ok();
    printed
}

fn findings(source: &str) -> Vec<nikaia::check::Finding> {
    let parsed = parse_to_ast(source).expect("the source parses");
    let own = nikaia::contracts::Ledger::infer(&parsed);
    let library =
        nikaia::contracts::Ledger::parse(nikaia::contracts::STD).expect("std ships a ledger");
    nikaia::check::check_program(&parsed, &own, &library, &std::collections::BTreeSet::new())
        .findings
        .into_iter()
        .filter(|f| f.severity == nikaia::check::Severity::Error)
        .collect()
}

/// Both settings, the same output, and no refusal on the way.
fn runs(purpose: &str, source: &str, expected: &str) {
    assert!(
        findings(source).is_empty(),
        "{purpose}: {:#?}",
        findings(source)
    );
    for how in [Build::default(), Build::parallel()] {
        assert_eq!(ran(purpose, source, how), expected, "{purpose} at {how:?}");
    }
}

// --- D1: a map whose keys are owned ------------------------------------------

/// **The wall this package began with**: a map keyed by an owned `String`,
/// written with a literal and with a name, read with a name that stays usable,
/// and a text fallback after `??` on a map of text.
#[test]
fn a_map_keyed_by_owned_text_is_written_and_read() {
    runs(
        "string-keys",
        "use std::collections\n\n\
         fn main() {\n\
         \x20   let mut m: collections::HashMap[String, i64] = collections::HashMap()\n\
         \x20   m[\"a\"] = 1\n\
         \x20   let name: String = \"b\"\n\
         \x20   m[name] = 2\n\
         \x20   let probe: String = \"a\"\n\
         \x20   let first = m[probe] ?? 0\n\
         \x20   let second = m[\"b\"] ?? 0\n\
         \x20   let third = m[probe.to_uppercase()] ?? 9\n\
         \x20   println(f\"{m.len()} {first} {second} {third} {probe}\")\n\
         }\n",
        "2 1 2 9 a",
    );
}

/// **A key is not a position**: a map keyed by an `i64` had its key turned into
/// a `usize` by the brackets. And a map of text, read with a literal after
/// `??`, is a view of text - nothing copied.
#[test]
fn a_map_keyed_by_numbers_holds_text() {
    runs(
        "int-keys",
        "use std::collections\n\n\
         fn main() {\n\
         \x20   let mut ids: collections::HashMap[i64, String] = collections::HashMap()\n\
         \x20   let k = 7\n\
         \x20   ids[k] = \"seven\"\n\
         \x20   ids[k + 1] = \"eight\"\n\
         \x20   let a = ids[7] ?? \"-\"\n\
         \x20   let b = ids[k + 1] ?? \"-\"\n\
         \x20   let c = ids[3] ?? \"-\"\n\
         \x20   println(f\"{a} {b} {c}\")\n\
         }\n",
        "seven eight -",
    );
}

/// **A view written as a key the map keeps is refused**, with ADR-208's
/// sentence: the map needs text of its own and a copy is the program's to write.
#[test]
fn a_view_written_as_an_owned_key_is_refused() {
    let found = findings(
        "use std::collections\n\n\
         fn put(k: ref String) {\n\
         \x20   let mut m: collections::HashMap[String, i64] = collections::HashMap()\n\
         \x20   m[k] = 1\n\
         }\n\
         fn main() {}\n",
    );
    assert_eq!(codes_of(&found), ["NK1105"], "{found:#?}");
    assert!(found[0].message.contains("key"), "{}", found[0].message);
    assert!(
        found[0]
            .help
            .as_deref()
            .is_some_and(|h| h.contains("to_owned")),
        "{:?}",
        found[0].help
    );
}

// --- D2: a literal assigned to text ----------------------------------------------

/// **An assignment keeps what it is given**, so a literal assigned to a
/// `String` is built into one there, as at every other place that keeps text.
#[test]
fn a_literal_assigned_to_text_is_built_there() {
    runs(
        "assign-literal",
        "fn main() {\n\
         \x20   let mut s: String = \"a\"\n\
         \x20   s = \"b\"\n\
         \x20   println(s)\n\
         }\n",
        "b",
    );
}

// --- D3: used after it was handed over -----------------------------------------

fn codes_of(found: &[nikaia::check::Finding]) -> Vec<&'static str> {
    found.iter().map(|f| f.code).collect()
}

fn one_refusal(source: &str, words: &str) {
    let found = findings(source);
    assert_eq!(codes_of(&found), ["NK2105"], "{found:#?}");
    assert!(found[0].message.contains(words), "{}", found[0].message);
}

const PERSON: &str = "struct Person {\n    name: String,\n}\n\n";

#[test]
fn a_value_pushed_is_given_away() {
    one_refusal(
        "fn main() {\n\
         \x20   let mut xs: Vec[String] = []\n\
         \x20   let name: String = \"b\"\n\
         \x20   xs.push(name)\n\
         \x20   println(name)\n\
         }\n",
        "handed to `push`",
    );
}

#[test]
fn a_key_written_is_given_away() {
    one_refusal(
        "use std::collections\n\n\
         fn main() {\n\
         \x20   let mut m: collections::HashMap[String, i64] = collections::HashMap()\n\
         \x20   let name: String = \"b\"\n\
         \x20   m[name] = 2\n\
         \x20   println(name)\n\
         }\n",
        "as a key",
    );
}

#[test]
fn a_field_holds_what_it_is_given() {
    one_refusal(
        &format!(
            "{PERSON}fn main() {{\n\
             \x20   let name: String = \"b\"\n\
             \x20   let p = Person {{ name: name }}\n\
             \x20   println(name)\n\
             }}\n"
        ),
        "`Person`'s `name`",
    );
    one_refusal(
        &format!(
            "{PERSON}fn main() {{\n\
             \x20   let name: String = \"b\"\n\
             \x20   let p = Person {{ name }}\n\
             \x20   println(name)\n\
             }}\n"
        ),
        "`Person`'s `name`",
    );
}

#[test]
fn a_rename_moves() {
    one_refusal(
        "fn main() {\n\
         \x20   let name: String = \"b\"\n\
         \x20   let t = name\n\
         \x20   println(name)\n\
         }\n",
        "bound to `t`",
    );
}

#[test]
fn a_loop_hands_it_over_again() {
    one_refusal(
        "fn main() {\n\
         \x20   let mut xs: Vec[String] = []\n\
         \x20   let name: String = \"b\"\n\
         \x20   for i in 0..<3 {\n\
         \x20       xs.push(name)\n\
         \x20   }\n\
         }\n",
        "inside a loop",
    );
}

/// **None of these is refused**: a copy handed over, data that is copied
/// anyway, two arms of one choice, a name given a value again, and
/// the `http` example's shape - hand over and leave, several times in a row.
#[test]
fn what_is_not_given_away_is_not_refused() {
    let found = findings(
        "fn stop(s: String) {\n\
         \x20   let mut xs: Vec[String] = []\n\
         \x20   xs.push(s)\n\
         }\n\n\
         fn serve(connection: String, a: i64) {\n\
         \x20   if a == 0 {\n\
         \x20       stop(connection)\n\
         \x20       return\n\
         \x20   }\n\
         \x20   if a == 1 {\n\
         \x20       stop(connection)\n\
         \x20       return\n\
         \x20   }\n\
         \x20   stop(connection)\n\
         }\n\n\
         fn main() {\n\
         \x20   let mut xs: Vec[String] = []\n\
         \x20   let name: String = \"b\"\n\
         \x20   xs.push(name.clone())\n\
         \x20   println(name)\n\
         \x20   let n = 3\n\
         \x20   let mut ys: Vec[i64] = []\n\
         \x20   ys.push(n)\n\
         \x20   println(f\"{n}\")\n\
         \x20   let c: String = \"c\"\n\
         \x20   if n > 2 {\n\
         \x20       xs.push(c)\n\
         \x20   } else {\n\
         \x20       println(c)\n\
         \x20   }\n\
         \x20   let mut again: String = \"d\"\n\
         \x20   xs.push(again)\n\
         \x20   again = \"e\"\n\
         \x20   println(again)\n\
         \x20   serve(\"x\".to_owned(), n)\n\
         }\n",
    );
    assert_eq!(codes_of(&found), Vec::<&str>::new(), "{found:#?}");
}

/// **And the programs that are not refused run**: the refusal is only worth
/// having if what it lets through is what the language below accepts.
#[test]
fn what_is_not_refused_runs() {
    runs(
        "not-refused",
        "fn stop(s: String) -> i64 {\n\
         \x20   let mut xs: Vec[String] = []\n\
         \x20   xs.push(s)\n\
         \x20   return xs.len()\n\
         }\n\n\
         fn serve(connection: String, a: i64) -> i64 {\n\
         \x20   if a == 0 {\n\
         \x20       return stop(connection)\n\
         \x20   }\n\
         \x20   return stop(connection) + 1\n\
         }\n\n\
         fn main() {\n\
         \x20   let mut xs: Vec[String] = []\n\
         \x20   let c: String = \"c\"\n\
         \x20   if xs.len() > 2 {\n\
         \x20       xs.push(c)\n\
         \x20   } else {\n\
         \x20       println(c)\n\
         \x20   }\n\
         \x20   let mut again: String = \"d\"\n\
         \x20   xs.push(again)\n\
         \x20   again = \"e\"\n\
         \x20   println(f\"{again} {xs.len()} {serve(\\\"x\\\".to_owned(), 1)}\")\n\
         }\n",
        "c\ne 1 2",
    );
}

// --- ADR-214: within one statement, parts of a value, and no stray warning ---

/// **Two hand-overs in one statement** are one after the other: the second
/// argument is read after the first was given away (ADR-214 D1).
#[test]
fn a_statement_that_hands_over_twice_is_refused() {
    one_refusal(
        "fn keep(a: String, b: String) -> i64 {\n\
         \x20   let mut xs: Vec[String] = []\n\
         \x20   xs.push(a)\n\
         \x20   xs.push(b)\n\
         \x20   return xs.len()\n\
         }\n\n\
         fn main() {\n\
         \x20   let name: String = \"n\"\n\
         \x20   println(f\"{keep(name, name)}\")\n\
         }\n",
        "handed to `keep`",
    );
}

/// **`name = f(name)` gives back what it took**, in the same statement.
#[test]
fn a_statement_that_takes_and_gives_back_is_a_program() {
    runs(
        "given-back",
        "fn main() {\n\
         \x20   let mut name: String = \"n\"\n\
         \x20   name = name + \"x\"\n\
         \x20   println(name)\n\
         }\n",
        "nx",
    );
}

/// **A part of an owned value is handed over, and the rest stays**: `p.x` is
/// still there after `p.name` went, `p.name` is not, and neither is `p` as a
/// whole (ADR-214 D2).
#[test]
fn a_part_handed_over_leaves_the_rest() {
    let head = "struct P {\n    name: String,\n    x: i64,\n}\n\n";
    runs(
        "part-rest",
        &format!(
            "{head}fn main() {{\n\
             \x20   let mut xs: Vec[String] = []\n\
             \x20   let p = P {{ name: \"a\", x: 1 }}\n\
             \x20   xs.push(p.name)\n\
             \x20   println(f\"{{p.x}} {{xs.len()}}\")\n\
             }}\n"
        ),
        "1 1",
    );
    one_refusal(
        &format!(
            "{head}fn main() {{\n\
             \x20   let mut xs: Vec[String] = []\n\
             \x20   let p = P {{ name: \"a\", x: 1 }}\n\
             \x20   xs.push(p.name)\n\
             \x20   println(p.name)\n\
             }}\n"
        ),
        "`p.name` was handed to `push`",
    );
    one_refusal(
        &format!(
            "{head}fn main() {{\n\
             \x20   let mut xs: Vec[String] = []\n\
             \x20   let p = P {{ name: \"a\", x: 1 }}\n\
             \x20   xs.push(p.name)\n\
             \x20   let q = p\n\
             }}\n"
        ),
        "`p` is used here, and `p.name` was handed",
    );
    // And a part assigned again is there again.
    runs(
        "part-given-back",
        &format!(
            "{head}fn main() {{\n\
             \x20   let mut xs: Vec[String] = []\n\
             \x20   let mut p = P {{ name: \"a\", x: 1 }}\n\
             \x20   xs.push(p.name)\n\
             \x20   p.name = \"b\"\n\
             \x20   println(p.name)\n\
             }}\n"
        ),
        "b",
    );
}

/// **A parameter whose part is handed over is kept**: it was lent (`&P`) and
/// the body took a field out of the loan, which `rustc` refused.
#[test]
fn a_parameter_whose_part_is_handed_over_is_kept() {
    runs(
        "part-of-parameter",
        "struct P {\n    name: String,\n    x: i64,\n}\n\n\
         fn names(p: P) -> i64 {\n\
         \x20   let mut xs: Vec[String] = []\n\
         \x20   xs.push(p.name)\n\
         \x20   return xs.len() + p.x\n\
         }\n\n\
         fn main() {\n\
         \x20   let p = P { name: \"z\", x: 5 }\n\
         \x20   println(f\"{names(p)}\")\n\
         }\n",
        "6",
    );
}

/// **`NK2106`: a part of something lent is not given away** - a `ref`
/// parameter and a `for` over a list alike.
#[test]
fn a_part_of_a_loan_is_not_handed_over() {
    for source in [
        "struct P {\n    name: String,\n}\n\n\
         fn f(p: ref P) {\n\
         \x20   let mut xs: Vec[String] = []\n\
         \x20   xs.push(p.name)\n\
         }\n\
         fn main() {}\n",
        "struct P {\n    name: String,\n}\n\n\
         fn f(ps: Vec[P]) {\n\
         \x20   let mut xs: Vec[String] = []\n\
         \x20   for p in ps {\n\
         \x20       xs.push(p.name)\n\
         \x20   }\n\
         }\n\
         fn main() {}\n",
    ] {
        let found = findings(source);
        assert_eq!(codes_of(&found), ["NK2106"], "{found:#?}");
        assert!(
            found[0]
                .help
                .as_deref()
                .is_some_and(|h| h.contains("p.name.clone()")),
            "{:?}",
            found[0].help
        );
    }
}

/// **A read through the brackets carries no parentheses of its own**: they
/// stood around every read, and `m[k] ?? 0` and `let x = xs[1]` were
/// `rustc`'s *unnecessary parentheses* about a file nobody wrote (ADR-214 D3).
/// Where a postfix follows, they are still there, because there they are
/// needed.
#[test]
fn a_read_through_the_brackets_warns_about_nothing() {
    let source = "use std::collections\n\n\
                  fn main() {\n\
                  \x20   let mut m: collections::HashMap[String, i64] = collections::HashMap()\n\
                  \x20   m[\"a\"] = 3\n\
                  \x20   let a = m[\"a\"] ?? 0\n\
                  \x20   let xs = [1, 2, 3]\n\
                  \x20   let v = xs[1]\n\
                  \x20   let w = xs[2] * 2 + xs[0]\n\
                  \x20   let f = xs[2] as f64\n\
                  \x20   let neg = -xs[0]\n\
                  \x20   let words = [\"ab\", \"c\"]\n\
                  \x20   let n = words[0].len()\n\
                  \x20   println(f\"{a} {v} {w} {f} {neg} {n}\")\n\
                  }\n";
    let rust = lowered(source, Build::default());
    let dir = common::scratch_dir("handed-parentheses");
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
    let said = String::from_utf8_lossy(&compiled.stderr);
    assert!(compiled.status.success(), "{said}\n{rust}");
    assert!(!said.contains("unnecessary parentheses"), "{said}\n{rust}");
    let out = Command::new(&binary).output().expect("run it");
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "3 2 7 3 -1 2");
    std::fs::remove_dir_all(&dir).ok();
}

// --- ADR-215: the rest of ADR-212 §5 --------------------------------------------

/// **`insert` is the write under the language below's name**, handing back
/// what it replaced; a literal key is built into the map's own text.
#[test]
fn insert_writes_and_hands_back_what_it_replaced() {
    runs(
        "insert",
        "use std::collections\n\n\
         fn main() {\n\
         \x20   let mut m: collections::HashMap[String, i64] = collections::HashMap()\n\
         \x20   m.insert(\"a\", 1)\n\
         \x20   let before = m.insert(\"a\", 2) ?? 0\n\
         \x20   let now = m[\"a\"] ?? 0\n\
         \x20   println(f\"{before} {now} {m.len()}\")\n\
         }\n",
        "1 2 1",
    );
    one_refusal(
        "use std::collections\n\n\
         fn main() {\n\
         \x20   let mut m: collections::HashMap[String, i64] = collections::HashMap()\n\
         \x20   let k: String = \"a\"\n\
         \x20   m.insert(k, 1)\n\
         \x20   println(k)\n\
         }\n",
        "handed to `insert`",
    );
}

/// **A copy is a copy of a view too**: a `String` the body only reads is a
/// `&str` below, and `.clone()` of that was the reference.
#[test]
fn a_copy_of_a_view_is_text_of_its_own() {
    runs(
        "clone-view",
        "fn copy(name: String) -> String {\n\
         \x20   return name.clone()\n\
         }\n\n\
         fn main() {\n\
         \x20   let c = copy(\"x\")\n\
         \x20   let ys: Vec[i64] = [1, 2]\n\
         \x20   let zs = ys.clone()\n\
         \x20   println(f\"{c} {zs.len()} {ys.len()}\")\n\
         }\n",
        "x 2 2",
    );
}
