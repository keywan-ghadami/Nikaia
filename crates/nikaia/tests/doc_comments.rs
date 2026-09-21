//! `///` before an item is its documentation, and the ledger carries it
//! ([ADR-139](../../../docs/specification/adr/adr-139.md)).
//!
//! `nikaia.contracts` **ships** with a package and is the one file a
//! consumer's compiler reads about a dependency — every signature, every
//! promise, every restriction — and it had nowhere to put a sentence. A
//! comment in the source does not travel, because the source of a published
//! package is not what a consumer reads.

use nikaia::contracts::Ledger;
use nikaia::parser::parse_to_ast;

fn items(source: &str) -> Vec<(String, Option<String>)> {
    let parsed = parse_to_ast(source).expect("the source parses");
    parsed
        .program
        .items
        .iter()
        .map(|item| {
            (
                format!("{:?}", std::mem::discriminant(&item.node)),
                item.doc.clone(),
            )
        })
        .collect()
}

fn ledger(source: &str) -> Ledger {
    Ledger::infer(&parse_to_ast(source).expect("the source parses"))
}

/// **A run of `///` before an item is its documentation** (D1), joined by its
/// own line breaks with the slashes and one space taken off.
#[test]
fn a_run_of_slashes_is_the_items_documentation() {
    let found = items(
        "/// The status line for a response code.\n\
         ///\n\
         /// An unknown code is `500`.\n\
         pub fn status_line(code: i32) -> ref String { return \"200 OK\" }\n",
    );
    assert_eq!(
        found[0].1.as_deref(),
        Some("The status line for a response code.\n\nAn unknown code is `500`.")
    );
}

/// **Anywhere else `///` is an ordinary comment** (D1), which is what
/// [ADR-134](../../../docs/specification/adr/adr-134.md) D3 said of it and what
/// this leaves true: a run with a statement between it and the next item
/// belongs to nothing.
#[test]
fn prose_a_token_stands_between_belongs_to_nothing() {
    let found = items(
        "fn first() -> i64 {\n\
         \x20   /// not an item's\n\
         \x20   return 0\n\
         }\n\
         fn second() -> i64 { return 0 }\n",
    );
    assert_eq!(found[1].1, None, "the run is inside the body above it");
}

/// **An ordinary comment between the prose and the item does not end it.** It
/// is trivia, so no token has been consumed — and the shape it allows is the
/// one this repository is written in: a sentence for whoever reaches the item,
/// then a note for whoever reads the source.
#[test]
fn an_ordinary_comment_between_them_is_a_note() {
    let found = items(
        "/// What it is for.\n\
         // How it works, which is the source's business.\n\
         pub fn f() -> i64 { return 0 }\n",
    );
    assert_eq!(found[0].1.as_deref(), Some("What it is for."));
}

/// **`////` is an ordinary comment**, as it is in every language that has
/// both.
#[test]
fn four_slashes_are_a_comment() {
    let found = items("//// a rule\npub fn f() -> i64 { return 0 }\n");
    assert_eq!(found[0].1, None);
}

/// **An item does not inherit the one above it.**
#[test]
fn the_next_item_gets_nothing() {
    let found = items(
        "/// About the first.\n\
         pub fn first() -> i64 { return 0 }\n\
         pub fn second() -> i64 { return 0 }\n",
    );
    assert_eq!(found[0].1.as_deref(), Some("About the first."));
    assert_eq!(found[1].1, None);
}

/// **The ledger carries it, on a `fn` and on a `type`** (D2).
#[test]
fn the_ledger_carries_it() {
    let ledger = ledger(
        "/// What the function is for.\n\
         pub fn f() -> i64 { return 0 }\n\
         /// What the type is for.\n\
         pub struct Row { name: ref String }\n",
    );
    assert_eq!(
        ledger.functions["f"].doc.as_deref(),
        Some("What the function is for.")
    );
    assert_eq!(
        ledger.types["Row"].doc.as_deref(),
        Some("What the type is for.")
    );
}

/// **Only for a `pub` item** (D2): the ledger records what a consumer may
/// reach, and a private item's prose is the source's.
#[test]
fn a_private_items_prose_stays_in_the_source() {
    let source = "/// Kept here and nowhere else.\n\
                  fn hidden() -> i64 { return 0 }\n";
    assert_eq!(
        items(source)[0].1.as_deref(),
        Some("Kept here and nowhere else."),
        "the source keeps it"
    );
    assert_eq!(ledger(source).functions["hidden"].doc, None);
}

/// **A method's prose is its own**, which is the shape a package's surface is
/// mostly made of.
#[test]
fn a_method_carries_its_own() {
    let ledger = ledger(
        "pub struct Row { name: ref String }\n\
         impl Row {\n\
         \x20   /// What this one is for.\n\
         \x20   pub fn name(ref self) -> ref String { return ref self.name }\n\
         }\n",
    );
    assert_eq!(
        ledger.functions["Row::name"].doc.as_deref(),
        Some("What this one is for.")
    );
}

/// **And it survives the round trip**, line breaks and all — which is the one
/// demand this makes of a format that is read a line at a time.
#[test]
fn it_round_trips_through_the_file() {
    let ledger = ledger(
        "/// One sentence.\n\
         ///\n\
         /// And another, with a \"quote\" and a \\\\ in it.\n\
         pub fn f() -> i64 { return 0 }\n",
    );
    let text = ledger.render();
    assert!(text.contains("doc = \""), "{text}");
    assert!(
        !text.contains("doc = \"One sentence.\n"),
        "a raw newline: {text}"
    );
    let read = Ledger::parse(&text).expect("it reads back");
    assert_eq!(read.functions["f"].doc, ledger.functions["f"].doc);
}

/// **`std`'s own entry is the first corpus of it**, and it is *derived*: the
/// regeneration test is what keeps the shipped line and the source in step.
#[test]
fn stds_nikaia_entry_carries_its_prose() {
    let shipped = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../crates/nikaia-std/std.contracts"),
    )
    .expect("std ships a ledger");
    let shipped = Ledger::parse(&shipped).expect("std's ledger parses");
    let doc = shipped.functions["text::digit_value"]
        .doc
        .as_deref()
        .expect("the one Nikaia entry has prose");
    assert!(doc.starts_with("The value of a decimal digit"), "{doc}");
    assert!(doc.contains('\n'), "its line breaks survive: {doc}");
}
