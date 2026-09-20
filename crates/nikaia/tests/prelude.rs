//! Part I 1.3's list and what `std` keys **bare** are the same set
//! ([ADR-162](../../../docs/specification/adr/adr-162.md)).
//!
//! [ADR-154](../../../docs/specification/adr/adr-154.md) D1 names the list and
//! §5 enforced it: what needs no `use` is what `std`'s ledger keys without a
//! module. Enforcing it made the rule true for what a **program** writes — and
//! left the other direction unwatched, so the page and the compiler drifted
//! apart in silence.
//!
//! They had. `eprint`, `access_all` and `update_all` were reachable with no
//! `use` and named nowhere on the page, and Part III's own sentence about the
//! printing functions said *they are in the prelude* while Part I's list wrote
//! three of the four. This is the test that would have said so.

use nikaia::contracts::{Ledger, STD};

/// The names Part I 1.3's list writes, read off the page.
fn the_list() -> Vec<String> {
    let page = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/specification/10-nikaia-light.md");
    let text = std::fs::read_to_string(page).expect("Part I");
    let start = text.find("### 1.3. The prelude").expect("the section");
    let rest = &text[start..];
    let end = rest
        .find("\nEverything else in the standard library")
        .expect("its end");
    // Each name is written `**`name`**`, which is the only bold-and-code the
    // section uses for one.
    let mut names = Vec::new();
    for piece in rest[..end].split("**`").skip(1) {
        if let Some(name) = piece.split("`**").next() {
            names.push(name.to_string());
        }
    }
    names
}

/// **Every function `std` keys bare is on the list** — the direction that was
/// not watched, and the one the drift happened in.
///
/// The other direction cannot be asserted yet: the list names `assert`, which
/// does not exist (`docs/open-work.md`). A name on the list that is missing is
/// the one a program meets; a name that is reachable and unlisted is the one
/// **nobody** meets, which is why it needs a test rather than a program.
#[test]
fn every_bare_name_in_std_is_on_the_list() {
    let library = Ledger::parse(STD).expect("std ships a ledger");
    let listed = the_list();
    let unlisted: Vec<_> = library
        .functions
        .keys()
        .filter(|key| !key.contains("::"))
        .filter(|key| !listed.contains(key))
        .cloned()
        .collect();
    assert!(
        unlisted.is_empty(),
        "reachable with no `use` and named nowhere on Part I 1.3's list: {unlisted:?}\n\
         Either the list gains it in a record (ADR-154 D4) or the entry gains a module."
    );
}

/// **And the list is read, not guessed**: this holds the reader above honest,
/// because a parse that found nothing would make the test above pass for the
/// wrong reason.
#[test]
fn the_list_is_the_one_on_the_page() {
    let listed = the_list();
    for name in ["Vec", "String", "Bytes", "println", "eprint", "panic"] {
        assert!(listed.iter().any(|n| n == name), "{name} in {listed:?}");
    }
    assert!(
        !listed.iter().any(|n| n == "HashMap"),
        "`HashMap` is the first thing outside it (D3): {listed:?}"
    );
}

/// **The four printing functions are four**, which is the sentence Part III
/// already wrote and Part I's list did not: *`eprintln` / `eprint` are the same
/// two on standard error. They are in the prelude rather than in a module.*
#[test]
fn the_two_pages_agree_about_printing() {
    let listed = the_list();
    for name in ["println", "print", "eprintln", "eprint"] {
        assert!(listed.iter().any(|n| n == name), "{name} in {listed:?}");
    }
    let tooling = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/specification/30-nikaia-tooling.md");
    let text = std::fs::read_to_string(tooling).expect("Part III");
    assert!(
        text.contains("They are in the prelude rather than in\na module."),
        "Part III still says the four are in the prelude"
    );
}
