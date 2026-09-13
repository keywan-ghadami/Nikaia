//! What an always-`Mutex` floor for `Locked[T]` would do to `sync`.
//!
//! **A probe, not a feature**, and it stays one now that the answer is decided
//! ([ADR-057](../../../docs/specification/adr/adr-057.md)). Nothing here emits
//! anything; what it does is hold on to the *other* answer's cost, so the
//! decision keeps its evidence rather than only its conclusion. `with_access`
//! therefore takes the shipped entry out before putting a variant in. What it does is put
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

/// `std`'s ledger with its **own** `Locked::access` entry replaced by the one
/// this probe wants to ask about.
///
/// The entry is real now ([ADR-057](../../../docs/specification/adr/adr-057.md)
/// §5) and says `sync = "from(f)"`. Appending to the shipped text would leave
/// the shipped answer standing and quietly turn the second row below into the
/// first, so the shipped one is taken out first — which is what keeps this file
/// a probe of both answers rather than a test of the decided one.
fn with_access(sync_line: &str) -> Ledger {
    let text = format!(
        "{}\n[fn.\"Locked::access\"]\npub = true\n{sync_line}signature = \"(&Locked[$T], f: fn(&$T))\"\n",
        without_access(STD)
    );
    Ledger::parse(&text).expect("std's ledger plus one entry still parses")
}

/// The shipped ledger with the `Locked::access` block cut out, comments and all.
fn without_access(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut skipping = false;
    for line in text.lines() {
        if line.starts_with('[') {
            skipping = line.starts_with("[fn.\"Locked::access\"]");
        }
        if !skipping {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
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

// --- and what `Locked[T]` is in the machine ---------------------------------

/// **At one user thread, every lock is the cheap shape**
/// ([ADR-057](../../../docs/specification/adr/adr-057.md) D2).
///
/// Nothing a user writes can cross a thread there, so the safe shape buys
/// nothing and costs 11.3 ns an acquisition — which is what that setting exists
/// to save. It also keeps the one diagnostic reachable at this setting: the
/// cheap shape says so when a program re-enters one lock, and a bare `Mutex`
/// hangs for ever without saying anything.
#[test]
fn at_one_user_thread_a_lock_is_the_cheap_shape() {
    let rust = lowered(
        "fn main() {\n    let n: i64 = 1\n    let c: Shared[Locked[i64]] = n\n}",
        false,
    );
    assert!(
        rust.contains("nikaia_std::lock::Local<i64>"),
        "the cheap shape:\n{rust}"
    );
    assert!(
        !rust.contains("lock::Crossing"),
        "and not the other one:\n{rust}"
    );
}

/// **At several, it is decided per value by the analysis that decides the
/// count** ([ADR-057](../../../docs/specification/adr/adr-057.md) D3).
///
/// A lock is only reachable from two places through a shared handle, so the
/// count that handle was given is the answer for the lock inside it. Here one
/// value is used by a task and the other is not, and the two come out different
/// in one program — which is the whole of what "per value" means.
#[test]
fn at_several_threads_a_lock_follows_the_value() {
    let rust = lowered(
        "fn main() {\n\
        \x20   let n: i64 = 1\n\
        \x20   let crossing: Shared[Locked[i64]] = n\n\
        \x20   let t = spawn fn { crossing.access fn { n } }\n\
         }",
        true,
    );
    assert!(
        rust.contains("std::sync::Arc<nikaia_std::lock::Crossing<i64>>"),
        "a value a task uses takes the shape that can cross:\n{rust}"
    );
}

/// And the annotation is the constructor for **both** hulls, because there is no
/// `Locked::new` in the language any more than there is a `Shared::new`.
///
/// **The two hulls are two decisions**, which this pins as well: the count keeps
/// its own floor ([ADR-037](../../../docs/specification/adr/adr-037.md) D6, atomic
/// at both settings until the analysis lowers it), while the lock is the cheap
/// shape here by D2. `Arc<Local<T>>` is not `Send` and does not need to be at
/// this setting — the answer that matters is that neither hull is guessed from
/// the other.
#[test]
fn the_annotation_allocates_the_lock_as_well_as_the_handle() {
    let rust = lowered(
        "fn main() {\n    let n: i64 = 1\n    let c: Shared[Locked[i64]] = n\n}",
        false,
    );
    assert!(
        rust.contains("::new(nikaia_std::lock::Local::new(n))"),
        "one line, two hulls:\n{rust}"
    );
}

fn lowered(source: &str, parallel: bool) -> String {
    let parsed = parse_to_ast(source).expect("the fixture parses");
    let build = match parallel {
        true => nikaia::emit::Build::parallel(),
        false => nikaia::emit::Build::default(),
    };
    nikaia::emit::emit_program(&parsed, build)
        .expect("the fixture lowers")
        .rust
}
