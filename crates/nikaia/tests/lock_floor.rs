//! What an always-`Mutex` floor for `Locked[T]` would do to `sync`.
//!
//! **A probe, not a feature.** `Locked[T]` is unbuilt — the type name and
//! `access` are accepted by the front end and reach `rustc` as
//! `Shared<Locked<i32>>`, where they do not exist — so nothing here decides
//! its representation and nothing here emits anything. What it does is put
//! Part II 12.2's open question to the *real* inference
//! ([`contracts::sync`](../src/contracts/sync.rs), ADR-027) instead of arguing
//! it: the one thing an always-`Mutex` floor could break is whether
//! `Locked::access` is still `sync`, and that turns entirely on one line in a
//! ledger.
//!
//! The type checker cannot resolve `counter.access` today (there is no
//! `Locked` type for it to resolve *through*), so its answer is handed in —
//! which is what `sync::infer` takes it as anyway (ADR-028). The library
//! ledger is `std`'s with one entry appended, and the two spellings of that
//! entry are the two answers the floor could take:
//!
//! | the entry | what the floor is claiming | what the inference then says |
//! | :--- | :--- | :--- |
//! | `sync = "from(f)"` | an acquisition adds no pausing of its own | 12.2's idiom is `sync` |
//! | no `sync` line | an acquisition can pause | 12.2's idiom is not, and every caller of it loses the claim too |
//!
//! The finding is in [`docs/mutex-floor.md`](../../../docs/mutex-floor.md).

use std::collections::BTreeMap;

use nikaia::check::MethodCalls;
use nikaia::contracts::{sync, Ledger, Sync, STD};
use nikaia::parser::parse_to_ast;

/// Part II 12.2's idiom, with a helper below it so the propagation is visible.
const IDIOM: &str = "fn tally(counter: Shared[Locked[i32]]) {\n\
                     \x20   counter.access fn { a += 1 }\n\
                     }\n\
                     fn outer(counter: Shared[Locked[i32]]) {\n\
                     \x20   tally(counter)\n\
                     }";

/// `std`'s ledger with one `Locked::access` entry appended.
fn with_access(sync_line: &str) -> Ledger {
    let text = format!(
        "{STD}\n[fn.\"Locked::access\"]\npub = true\n{sync_line}signature = \"(&Locked[$T], f: fn(&$T))\"\n"
    );
    Ledger::parse(&text).expect("std's ledger plus one entry still parses")
}

/// What the compiler concludes about `tally`, given a library ledger and the
/// type checker's answer for `counter.access`.
fn sync_of(source: &str, library: &Ledger, resolved: bool) -> BTreeMap<String, Sync> {
    let parsed = parse_to_ast(source).expect("the source parses");
    let mut ledger = Ledger::infer(&parsed);

    let mut methods = BTreeMap::new();
    if resolved {
        for name in ["tally", "outer"] {
            let mut calls = MethodCalls::default();
            if name == "tally" {
                calls.resolved.insert("Locked::access".to_string());
            }
            methods.insert(name.to_string(), calls);
        }
    }
    sync::infer(&mut ledger, &parsed, library, &methods);

    ledger
        .functions
        .iter()
        .map(|(name, contract)| (name.clone(), contract.sync.clone()))
        .collect()
}

/// Today, and for a reason that has nothing to do with the representation:
/// `counter.access` is a method call on a type no ledger describes, so
/// ADR-027 D2's polarity takes the claim away.
///
/// This is the baseline every row below is read against. **Part II 12.2's own
/// idiom is not `sync` today**, whatever `Locked` turns out to be.
#[test]
fn the_idiom_is_not_sync_today_because_nothing_can_resolve_access() {
    let l = Ledger::infer(&parse_to_ast(IDIOM).expect("parses"));
    assert_eq!(l.functions["tally"].sync, Sync::No);
    assert_eq!(l.functions["outer"].sync, Sync::No);
}

/// With an entry that says an acquisition adds no pausing of its own, the
/// idiom is `sync` — and so is everything above it.
///
/// This is the `sync = "from(f)"` shape ADR-029 D3 built for `and_modify`, and
/// it is the shape an acquisition fits: the lambda runs *during* the call, so
/// its body is already counted in the function that writes it (D4's bound).
/// **An always-`Mutex` floor changes nothing here** — a `Mutex` acquisition is
/// not a suspension point, which is the property `sync` is about (ADR-005 D6
/// names it "no *suspension point* while a lock is held").
#[test]
fn a_non_pausing_acquisition_leaves_the_idiom_sync() {
    let answers = sync_of(IDIOM, &with_access("sync = \"from(f)\"\n"), true);
    assert_eq!(answers["tally"], Sync::Inferred);
    assert_eq!(answers["outer"], Sync::Inferred);
}

/// And the lambda still decides, which is what keeps `from` from being a hole:
/// the same entry over a lambda that does I/O leaves the function not `sync`.
#[test]
fn the_lambda_still_decides_under_from() {
    let source = "use std::fs\n\
                  fn tally(counter: Shared[Locked[i32]]) {\n\
                      counter.access fn { fs::write(\"log\", \"x\") }\n\
                  }\n\
                  fn outer(counter: Shared[Locked[i32]]) { tally(counter) }";
    let answers = sync_of(source, &with_access("sync = \"from(f)\"\n"), true);
    assert_eq!(answers["tally"], Sync::No);
    assert_eq!(answers["outer"], Sync::No);
}

/// The other answer, and the one that would cost the language Chapter 12.
///
/// If a blocking acquisition is classified as a pause — no `sync` line, the
/// way `fs::write` carries none — then `access` is not `sync`, so a function
/// that uses a lock cannot appear in a `par_iter` body, in `access` itself, in
/// a scope's tasks or in the panic hook. Part II 12.2's own idiom stops
/// compiling, and the greatest fixpoint (ADR-027 D1) carries the loss up every
/// caller.
///
/// That is the load-bearing question the floor asks, and this test is what
/// makes the consequence a run rather than a claim.
#[test]
fn a_pausing_acquisition_costs_the_idiom_and_everything_above_it() {
    let answers = sync_of(IDIOM, &with_access(""), true);
    assert_eq!(answers["tally"], Sync::No);
    assert_eq!(
        answers["outer"],
        Sync::No,
        "the loss propagates: ADR-027 D1's fixpoint takes the claim from every caller"
    );
}
