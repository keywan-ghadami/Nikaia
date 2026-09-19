//! `std`'s own types are constructed by the anonymous constructor
//! ([ADR-140](../../../docs/specification/adr/adr-140.md) D2).
//!
//! `pub fn(first: i32)` is what a `.nika` file writes (Part I 4.2) and `new` was
//! Rust's convention reaching through a hand-written ledger — two conventions
//! for one thing, which is `language-review.md` §3.3's second row. One stays,
//! and it is this language's own.
//!
//! **The ledger's key does not move.** `Type::new` is the name the *lowering*
//! writes, and the lowering is name for name
//! ([ADR-011](../../../docs/specification/adr/adr-011.md) D2); what changed is
//! that the resolution reaching for it now reaches into the library too, where
//! it used to stop at this unit.

use nikaia::contracts::{Ledger, STD};
use nikaia::emit::{emit_program, Build};
use nikaia::parser::parse_to_ast;

fn lowered(source: &str) -> String {
    let parsed = parse_to_ast(source).expect("the source parses");
    emit_program(&parsed, Build::default())
        .expect("it lowers")
        .rust
}

fn refusals(source: &str) -> Vec<nikaia::check::Finding> {
    let parsed = parse_to_ast(source).expect("the source parses");
    let own = Ledger::infer(&parsed);
    let library = Ledger::parse(STD).expect("std's ledger");
    nikaia::check::check(&parsed, &own, &library)
        .findings
        .into_iter()
        .filter(|f| f.code == "NK1149")
        .collect()
}

/// **`Vec()`, `String()` and `HashMap()`**, and each lowers to the name the
/// language below has.
#[test]
fn stds_types_are_called_like_a_constructor() {
    let rust = lowered(
        "use std::collections\n\nfn main() {\n\
         \x20   let mut xs = Vec()\n\
         \x20   let s = String()\n\
         \x20   let m = collections::HashMap()\n\
         \x20   xs.push(1)\n\
         \x20   println(f\"{xs.len()} {s.len()} {m.len()}\")\n\
         }\n",
    );
    assert!(rust.contains("Vec::new()"), "{rust}");
    assert!(rust.contains("String::new()"), "{rust}");
    // **And the map goes through `path`**, which is where one name below
    // depends on more than the name: at the default provenance the map is the
    // trusted one, and `new` exists only for the default hasher
    // ([ADR-010](../../../docs/specification/adr/adr-010.md) D5). Writing
    // `HashMap::new(` straight out of the constructor arm would have taken that
    // back for every `HashMap()` in a trusted program, which is what this line
    // is here to catch.
    assert!(rust.contains("TrustedMap::default()"), "{rust}");
}

/// **And it keeps its type arguments**, which is the half that had to be
/// corrected rather than added: the anonymous-constructor rule handed back
/// `Ty::named(ty)` — right for a `.nika` type, which has no parameters, and
/// wrong for `Vec::new`, whose entry declares `-> Vec[?]`. `let xs = Vec()`
/// came out as a plain `Vec` and `NK1106` refused it against every `Vec[T]` it
/// was given to.
#[test]
fn a_constructed_vec_keeps_its_element_type() {
    let parsed = parse_to_ast(
        "struct S { xs: Vec[i64] }\n\
         fn main() {\n\
         \x20   let xs = Vec()\n\
         \x20   let s = S { xs: xs }\n\
         \x20   println(f\"{s.xs.len()}\")\n\
         }\n",
    )
    .expect("it parses");
    let own = Ledger::infer(&parsed);
    let library = Ledger::parse(STD).expect("std's ledger");
    let found: Vec<_> = nikaia::check::check(&parsed, &own, &library)
        .findings
        .into_iter()
        .filter(|f| f.code == "NK1106")
        .collect();
    assert!(found.is_empty(), "{found:#?}");
}

/// **A written `Type::new` is refused**, in `std` as in a `.nika` file, which is
/// the whole of what D2 evens out.
#[test]
fn new_is_refused_at_a_call() {
    let found = refusals("fn main() { let xs = Vec::new() println(f\"{xs.len()}\") }");
    assert_eq!(found.len(), 1, "{found:#?}");
    let help = found[0].help.as_deref().expect("a way out");
    assert!(help.contains("`Vec(…)`"), "{help}");
}

/// **And in value position**, which is the case the record names:
/// `par_fold(M, Summary::new, …)` is how `1brc.nika` wrote it, against a type
/// declaring an anonymous constructor and no `new`.
#[test]
fn new_is_refused_as_a_value_and_the_bare_name_lowers() {
    let source = |ctor: &str| {
        format!(
            "struct S {{ n: i64 }}\n\
             impl S {{ pub fn(n: i64) -> S {{ return S {{ n: n }} }} }}\n\
             fn take(f: fn(i64) -> S) -> i64 {{ return f(1).n }}\n\
             fn main() {{ println(f\"{{take({ctor})}}\") }}\n"
        )
    };
    assert_eq!(refusals(&source("S::new")).len(), 1);
    assert!(refusals(&source("S")).is_empty());
    // The bare name is the constructor, and the language below wants its key.
    assert!(lowered(&source("S")).contains("take(S::new)"));
}

/// **A qualified name is left alone**, which is `NK1135`'s convention one
/// refusal over: `http::Server::new()` names a package this build cannot see,
/// and whether it should be `http::Server()` is that package's ledger to say.
#[test]
fn a_package_that_nothing_describes_is_not_refused() {
    let found = refusals("fn main() { let s = http::Server::new() }");
    assert!(found.is_empty(), "{found:#?}");
}
