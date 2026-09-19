//! A `match` covers every case
//! ([ADR-146](../../../docs/specification/adr/adr-146.md)).
//!
//! [ADR-137](../../../docs/specification/adr/adr-137.md) §4 left exhaustiveness
//! open — *a question about types this compiler does not yet answer*. It was
//! never open **below**: Rust refuses a non-exhaustive `match`, so the reader
//! got the backend's words on a Nikaia line, which is
//! [Part III C.1](../../../docs/specification/30-nikaia-tooling.md)'s class. All
//! this decides is whose message it is — and Part I 3.4's own first sentence had
//! been promising the rule the whole time.

use nikaia::contracts::{Ledger, STD};
use nikaia::parser::parse_to_ast;

fn refusals(source: &str) -> Vec<nikaia::check::Finding> {
    let parsed = parse_to_ast(source).expect("the source parses");
    let own = Ledger::infer(&parsed);
    let library = Ledger::parse(STD).expect("std's ledger");
    nikaia::check::check(&parsed, &own, &library)
        .findings
        .into_iter()
        .filter(|f| f.code == "NK1151")
        .collect()
}

/// **An enum is complete when every variant is named**, and needs no `else`.
#[test]
fn every_variant_named_is_complete() {
    let found = refusals(
        "enum Op { Plus, Times }\n\
         fn f(o: Op) -> i64 { return match o { Op::Plus => 1, Op::Times => 2 } }\n",
    );
    assert!(found.is_empty(), "{found:#?}");
}

/// **And the message names what is missing** rather than saying *non-exhaustive
/// patterns* about a generated file.
#[test]
fn a_missing_variant_is_named() {
    let found = refusals(
        "enum Op { Plus, Times, Minus }\n\
         fn f(o: Op) -> i64 { return match o { Op::Plus => 1 } }\n",
    );
    assert_eq!(found.len(), 1, "{found:#?}");
    assert!(
        found[0].message.contains("Op::Times"),
        "{}",
        found[0].message
    );
    assert!(
        found[0].message.contains("Op::Minus"),
        "{}",
        found[0].message
    );
}

/// **Anything else needs `else`** (D1), because its set is not enumerable in
/// arms — and the message says that rather than listing one.
#[test]
fn an_open_ended_type_is_told_to_write_else() {
    for scrutinee in ["n: i64", "c: char", "s: String"] {
        let found = refusals(&format!(
            "fn f({scrutinee}) -> i64 {{ return match {} {{ 1 => 1 }} }}\n",
            scrutinee.split(':').next().unwrap().trim()
        ));
        assert_eq!(found.len(), 1, "{scrutinee}: {found:#?}");
        assert!(
            found[0].message.contains("`else`"),
            "{scrutinee}: {}",
            found[0].message
        );
    }
}

/// **`bool` is the case that is neither** an enum nor open-ended: `true` and
/// `false` are two arms and a complete `match`, which no enum map knows. Without
/// this, a program Rust accepts would be refused here — C.4 exactly.
#[test]
fn both_booleans_are_complete_and_one_is_not() {
    let complete = refusals("fn f(b: bool) -> i64 { return match b { true => 1, false => 0 } }\n");
    assert!(complete.is_empty(), "{complete:#?}");

    let half = refusals("fn f(b: bool) -> i64 { return match b { true => 1 } }\n");
    assert_eq!(half.len(), 1, "{half:#?}");
    assert!(half[0].message.contains("false"), "{}", half[0].message);
}

/// **A bare name is a catch-all, and covers** (D2) — which is what the arm
/// already means, read by the same rule `else` is.
#[test]
fn a_bare_name_covers() {
    let found = refusals(
        "enum Op { Plus, Times }\n\
         fn f(o: Op) -> i64 { return match o { Op::Plus => 1, other => 2 } }\n",
    );
    assert!(found.is_empty(), "{found:#?}");
}

/// **And `else` does** ([ADR-145](../../../docs/specification/adr/adr-145.md)),
/// which is how the rest is said everywhere the set cannot be written out.
#[test]
fn else_covers() {
    let found = refusals("fn f(n: i64) -> i64 { return match n { 1 => 1, else => 0 } }\n");
    assert!(found.is_empty(), "{found:#?}");
}

/// **Where the scrutinee's type is not known, nothing is claimed** (D3), which
/// is [Part III C.4](../../../docs/specification/30-nikaia-tooling.md): a
/// refusal on a guess is a correct program refused, and it is the worse of the
/// two mistakes. The backend still answers that one.
#[test]
fn an_untyped_scrutinee_is_left_alone() {
    let found = refusals(
        "fn f(v: Vec[i64]) -> i64 {\n\
         \x20   return match v.first() { 1 => 1 }\n\
         }\n",
    );
    assert!(found.is_empty(), "{found:#?}");
}
