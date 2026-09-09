//! The Borrow Contract Ledger (Part III, 13.5; ADR-020).
//!
//! Two things are checked here. That the ledger says what the source says -
//! `sync`, `throws`, and what a result may point into - and that **`std`'s
//! shipped ledger has not drifted from `std`'s Nikaia sources**, which is the
//! half of "a package ships its ledger" that a reviewer cannot do by eye.

use std::path::PathBuf;

use nikaia::contracts::Ledger;
use nikaia::parser::parse_to_ast;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn ledger(source: &str) -> Ledger {
    Ledger::infer(&parse_to_ast(source).expect("the source parses"))
}

/// `sync` and `throws` are declared, so they are recorded exactly.
#[test]
fn a_declaration_is_recorded_as_it_was_written() {
    let l = ledger(
        "pub fn pure(a: i32) -> i32 sync { return a }\n\
         fn risky() throws { }\n\
         fn plain() { }",
    );

    let pure = &l.functions["pure"];
    assert!(pure.public && pure.sync && !pure.throws);

    let risky = &l.functions["risky"];
    assert!(!risky.public && !risky.sync && risky.throws);

    let plain = &l.functions["plain"];
    assert!(!plain.public && !plain.sync && !plain.throws);
}

/// The borrow contract, in the spec's own spelling: a result that is a view may
/// point into any view it was given.
///
/// Stage 0 has one input lifetime, so `borrows(a | b)` is the widest contract
/// the signature supports and the honest one to record - narrowing it needs the
/// whole-program analysis of ADR-005 D3, which is what the ledger's `inference`
/// header exists to distinguish.
#[test]
fn a_result_that_is_a_view_records_what_it_may_point_into() {
    let l = ledger(
        "fn longest(a: &str, b: &str) -> &str { return a }\n\
         fn owned(a: &str) -> String { return \"x\" }\n\
         fn counted(a: &str, n: i32) -> &str { return a }",
    );

    assert_eq!(l.functions["longest"].borrows, ["a", "b"]);
    assert!(l.functions["owned"].borrows.is_empty());
    // `n` is not a view, so the result cannot point into it.
    assert_eq!(l.functions["counted"].borrows, ["a"]);
}

/// A type is tethered by the fields that hold a view - including through
/// another type that does, which is the transitive half of Part II 10.6.
#[test]
fn a_type_records_what_ties_it_to_the_input() {
    let l = ledger(
        "@borrowed\n\
         pub struct Hit { path: &str, bytes: i64 }\n\
         pub struct Report { hits: Vec[Hit], total: i64 }\n\
         pub struct Counts { n: i64 }",
    );

    let hit = &l.types["Hit"];
    assert!(hit.borrowed);
    assert_eq!(hit.tethered, ["path"]);

    // `Report` says no `&` anywhere and is tied to the input all the same.
    assert_eq!(l.types["Report"].tethered, ["hits"]);
    assert!(l.types["Counts"].tethered.is_empty());
}

/// A method is named as a caller reaches it, and the anonymous constructor of
/// Kap 4.2 is `Type::new` because that is what the lowering calls it.
#[test]
fn a_method_is_named_the_way_it_is_called() {
    let l = ledger(
        "pub struct Stats { n: i64 }\n\
         impl Stats {\n\
             pub fn(first: i64) -> Stats { return Stats(n: first) }\n\
             fn add(&mut self, x: i64) sync { self.n += x }\n\
         }",
    );

    assert!(l.functions.contains_key("Stats::new"), "{:?}", l.functions);
    assert!(l.functions["Stats::add"].sync);
}

/// The file is a pure function of source and toolchain, which is what lets
/// `--locked` compare bytes.
#[test]
fn the_ledger_is_deterministic_and_reads_back() {
    let source = "pub fn f(a: &str) -> &str sync { return a }\n\
                  pub struct S { t: &str }";
    let first = ledger(source).render();
    let second = ledger(source).render();
    assert_eq!(first, second);

    let read = Ledger::parse(&first).expect("its own output parses");
    assert_eq!(read.render(), first);
    assert_eq!(read.inference, nikaia::contracts::INFERENCE);
    assert_eq!(read.functions["f"].borrows, ["a"]);
    assert_eq!(read.types["S"].tethered, ["t"]);
}

/// `std` ships its ledger, and the entries it says are inferred really are.
///
/// The modules still written in Rust are written into `std.contracts` by hand -
/// a compiler that does not read Rust cannot infer them - but the modules
/// written in Nikaia can be, and a hand-maintained copy of a derived truth goes
/// stale. This is what stops it.
#[test]
fn the_shipped_std_ledger_agrees_with_its_nikaia_sources() {
    let shipped = std::fs::read_to_string(repo_root().join("crates/nikaia-std/std.contracts"))
        .expect("std ships a ledger");
    let shipped = Ledger::parse(&shipped).expect("std's ledger parses");

    let sources = repo_root().join("crates/nikaia-std/src");
    let mut checked = 0;

    for entry in std::fs::read_dir(&sources).expect("read std's sources") {
        let path = entry.expect("dir entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("nika") {
            continue;
        }
        let module = path
            .file_stem()
            .expect("stem")
            .to_string_lossy()
            .to_string();
        let source = std::fs::read_to_string(&path).expect("read the module");
        let inferred = Ledger::infer(&parse_to_ast(&source).expect("the module parses"));

        for (name, contract) in &inferred.functions {
            // In the shipped ledger an item is named as a caller writes it,
            // which includes the module it lives in.
            let key = format!("{module}::{name}");
            let shipped_contract = shipped.functions.get(&key).unwrap_or_else(|| {
                panic!("std.contracts has no entry for `{key}`, which `{module}.nika` declares")
            });
            assert_eq!(
                shipped_contract, contract,
                "std.contracts and {module}.nika disagree about `{key}`"
            );
            checked += 1;
        }
    }

    assert!(checked > 0, "no Nikaia module in std was checked");
}

/// Every module `std` exposes has an entry, so a caller never has to guess.
///
/// The list is written here rather than derived, because deriving it from the
/// Rust sources is the thing this cannot do - and a module added to the prelude
/// without a contract is exactly the omission worth failing on.
#[test]
fn every_std_module_is_in_the_shipped_ledger() {
    let shipped = std::fs::read_to_string(repo_root().join("crates/nikaia-std/std.contracts"))
        .expect("std ships a ledger");
    let shipped = Ledger::parse(&shipped).expect("std's ledger parses");

    for module in ["cli", "fs", "html", "io", "list", "text"] {
        assert!(
            shipped
                .functions
                .keys()
                .any(|k| k.starts_with(&format!("{module}::"))),
            "std.contracts says nothing about `{module}`"
        );
    }
}
