//! The per-value `Rc`/`Arc` prototype — [ADR-037](../../../docs/specification/adr/adr-037.md)
//! D3's open question, asked of a value instead of a build.
//!
//! D3 says it itself: "whether the *choice between `Rc` and `Arc`* could be made
//! per value rather than per build is a real question and is not answered here".
//! `contracts::sharing` is the experiment, not the answer, and
//! [`docs/rc-or-arc.md`](../../../docs/rc-or-arc.md) is the notebook page it
//! belongs to: what the atomic costs, what was built, what was done by hand, and
//! which cases came out atomic because nothing could decide them.
//!
//! **Every source here parses and is analysed for real.** `Shared` has no
//! constructor - there is no `Rc::new` or `Arc::new` in the emitter - so a
//! `Shared` can only arrive as a *declared type*, which is exactly how
//! `tests/send.rs` reaches the same type. Nothing is emitted and no decision
//! here reaches a build.

use nikaia::contracts::sharing::{self, Count};
use nikaia::contracts::{Ledger, TypeContract, STD};
use nikaia::parser::parse_to_ast;

fn decisions(source: &str) -> Vec<sharing::Decision> {
    let parsed = parse_to_ast(source).expect("the source parses");
    let own = Ledger::infer(&parsed);
    let library = Ledger::parse(STD).expect("std ships a ledger");
    sharing::analyse(&parsed, &own, &library)
}

/// One value's answer, by the name the source gives it.
fn one(source: &str, value: &str) -> sharing::Decision {
    let found = decisions(source);
    found
        .iter()
        .find(|d| d.value == value)
        .unwrap_or_else(|| panic!("no decision for `{value}` in {found:#?}"))
        .clone()
}

// --- the base case, which is the only reason the rest is worth anything ------

/// A `Shared` that nothing crosses with gets a **plain** count.
///
/// Without this the analysis could answer `atomic` for everything and pass every
/// other test here, which is the failure mode a fail-closed analysis has.
#[test]
fn a_shared_that_never_crosses_gets_a_plain_count() {
    let decision = one(
        "fn zaehle(counts: Shared[Vec[i64]]) -> i64 {\n\
             let n = counts.len()\n\
             n\n\
         }",
        "counts",
    );
    assert_eq!(decision.count, Count::Plain, "{decision:#?}");
    assert!(decision.why.is_none(), "{decision:#?}");
}

/// Two names for one `Shared` are one allocation and therefore one count.
///
/// The count belongs to the allocation and not to the handle, which is why the
/// analysis is union-find over handles rather than a walk down a lattice - and
/// why one crossing anywhere in a class decides the whole class.
#[test]
fn two_handles_on_one_allocation_share_one_answer() {
    let found = decisions(
        "fn zaehle(counts: Shared[Vec[i64]]) {\n\
             let also = counts\n\
             spawn({ println(f\"{also.len()}\") })\n\
         }",
    );
    assert_eq!(found.len(), 2, "{found:#?}");
    for decision in &found {
        assert_eq!(decision.count, Count::Atomic, "{decision:#?}");
    }
}

// --- the crossings ----------------------------------------------------------

/// A `spawn` body that uses it forces the atomic count (`NK2501`'s crossing).
#[test]
fn a_spawn_forces_the_atomic_count() {
    let decision = one(
        "fn zaehle(counts: Shared[Vec[i64]]) {\n\
             spawn({ println(f\"{counts.len()}\") })\n\
         }",
        "counts",
    );
    assert_eq!(decision.count, Count::Atomic);
    assert!(
        !decision.undecided,
        "a `spawn` is a crossing, not a mystery"
    );
    assert!(
        decision.why.as_deref().unwrap().contains("spawn"),
        "{decision:#?}"
    );
}

/// A lambda handed to a parallel method forces it too.
#[test]
fn a_parallel_lambda_forces_the_atomic_count() {
    let decision = one(
        "fn zaehle(counts: Shared[Vec[i64]], rows: Vec[i64]) -> i64 {\n\
             rows.par_iter(fn(r) { counts.len() + r })\n\
         }",
        "counts",
    );
    assert_eq!(decision.count, Count::Atomic);
    assert!(!decision.undecided, "{decision:#?}");
}

/// **Hard case three.** A call whose body this compiler cannot see comes out
/// atomic, which is the polarity the experiment was set to confirm.
///
/// `NK2502` refuses this crossing. Here it costs an atomic instead, because the
/// cost of being wrong in the safe direction is speed and not a rejected
/// program - and that is the one way this analysis has an easier job than the
/// check beside it.
#[test]
fn a_call_this_compiler_cannot_see_the_end_of_comes_out_atomic() {
    let decision = one(
        "fn zaehle(counts: Shared[Vec[i64]]) {\n\
             hyper_shim::across_a_thread(counts)\n\
         }",
        "counts",
    );
    assert_eq!(decision.count, Count::Atomic);
    assert!(decision.undecided, "nothing decided it: {decision:#?}");
    assert!(
        decision
            .why
            .as_deref()
            .unwrap()
            .contains("hyper_shim::across_a_thread"),
        "{decision:#?}"
    );
}

/// **Hard case one.** A `Shared` in a published signature comes out atomic, and
/// comes out atomic as *undecided*.
///
/// A public function's callers are in a unit this build never sees. The
/// representation is in the artifact by the time one of them crosses, so nothing
/// here can answer - and `docs/rc-or-arc.md` §5.1 is why a ledger column would
/// not answer either.
#[test]
fn a_shared_in_a_published_signature_comes_out_atomic() {
    let decision = one(
        "pub fn zaehle(counts: Shared[Vec[i64]]) -> i64 {\n\
             counts.len()\n\
         }",
        "counts",
    );
    assert_eq!(decision.count, Count::Atomic);
    assert!(decision.undecided, "{decision:#?}");
    assert!(
        decision.why.as_deref().unwrap().contains("public"),
        "{decision:#?}"
    );
}

/// A call *within* the unit decides nothing by itself: the two handles are one
/// allocation, and the answer is whatever the pair of them reaches.
///
/// This is what the whole-program ledger buys. The callee is not public, so its
/// parameter is not a boundary, and a plain count survives a call.
#[test]
fn a_call_inside_the_unit_keeps_a_plain_count() {
    let found = decisions(
        "fn laenge(counts: Shared[Vec[i64]]) -> i64 { counts.len() }\n\
         fn zaehle(counts: Shared[Vec[i64]]) -> i64 { laenge(counts) }",
    );
    assert!(!found.is_empty(), "nothing was analysed");
    for decision in &found {
        assert_eq!(decision.count, Count::Plain, "{decision:#?}");
    }
}

/// …and a crossing on one side of that call reaches the other side, because it
/// is one allocation.
#[test]
fn a_crossing_in_a_callee_reaches_the_callers_handle() {
    let found = decisions(
        "fn laenge(counts: Shared[Vec[i64]]) -> i64 {\n\
             spawn({ println(f\"{counts.len()}\") })\n\
             1\n\
         }\n\
         fn zaehle(counts: Shared[Vec[i64]]) -> i64 { laenge(counts) }",
    );
    assert_eq!(found.len(), 2, "{found:#?}");
    for decision in &found {
        assert_eq!(decision.count, Count::Atomic, "{decision:#?}");
    }
}

/// **Hard case two.** A `Shared` in a struct field, reached by a value of the
/// struct that crosses.
///
/// `send::crossing` walks a struct's `fields` through the ledger (ADR-024, the
/// walk ADR-029 established) to refuse the struct for its field's sake. This is
/// the same walk the other way round: the struct crosses, so the field's count
/// has to be atomic - and the handle that was *put into* the field is the same
/// allocation, so it is atomic too.
#[test]
fn a_shared_in_a_struct_field_follows_the_struct_across() {
    let found = decisions(
        "struct Counter { hits: Shared[i64] }\n\
         fn zaehle(hits: Shared[i64]) {\n\
             let c = Counter { hits: hits }\n\
             spawn({ println(f\"{c.hits}\") })\n\
         }",
    );
    let decision = found
        .iter()
        .find(|d| d.value == "hits")
        .unwrap_or_else(|| panic!("{found:#?}"));
    assert_eq!(decision.count, Count::Atomic, "{found:#?}");
}

/// The same struct, with nothing crossing, keeps the plain count.
#[test]
fn a_shared_in_a_struct_field_that_stays_put_keeps_a_plain_count() {
    let found = decisions(
        "struct Counter { hits: Shared[i64] }\n\
         fn zaehle(hits: Shared[i64]) -> i64 {\n\
             let c = Counter { hits: hits }\n\
             1\n\
         }",
    );
    let decision = found
        .iter()
        .find(|d| d.value == "hits")
        .unwrap_or_else(|| panic!("{found:#?}"));
    assert_eq!(decision.count, Count::Plain, "{found:#?}");
}

/// The struct's field is decided the same way whichever function the walk sees
/// first.
///
/// The crossing is in one function and the handle is put into the field in
/// another, and here the crossing is written **first**. An analysis that only
/// forced slots it had already met would answer `plain` for this program and
/// `atomic` for the same two functions in the other order - a fail-open bug, and
/// the one direction the polarity forbids. It is a real one: the prototype had it
/// (`docs/rc-or-arc.md` §8).
#[test]
fn a_struct_field_is_decided_whatever_order_the_functions_are_written_in() {
    for source in [
        "struct Counter { hits: Shared[i64] }\n\
         fn cross(c: Counter) { spawn({ println(f\"{c.hits}\") }) }\n\
         fn build(hits: Shared[i64]) -> Counter { Counter { hits: hits } }",
        "struct Counter { hits: Shared[i64] }\n\
         fn build(hits: Shared[i64]) -> Counter { Counter { hits: hits } }\n\
         fn cross(c: Counter) { spawn({ println(f\"{c.hits}\") }) }",
    ] {
        let found = decisions(source);
        let decision = found
            .iter()
            .find(|d| d.value == "hits")
            .unwrap_or_else(|| panic!("{found:#?}"));
        assert_eq!(decision.count, Count::Atomic, "{found:#?}");
    }
}

/// A struct whose fields are Rust - an entry with no `fields` - cannot be walked,
/// so a crossing of it says nothing about the `Shared` it may hold.
///
/// This is the gap `docs/rc-or-arc.md` §5.2 names: the transitivity is exactly as
/// good as the ledger's `fields`, and `fields` is empty for every type whose
/// parts are Rust. A value *put into* such a type by a Nikaia program is still
/// caught, because the putting is visible; one built inside the Rust half is not.
#[test]
fn a_type_whose_fields_are_rust_hides_what_it_holds() {
    let mut own = Ledger::empty();
    own.types.insert(
        "Opaque".to_string(),
        TypeContract {
            fields: Vec::new(),
            ..TypeContract::default()
        },
    );
    let source = "fn zaehle(o: Opaque) {\n\
             spawn({ println(f\"{o}\") })\n\
         }";
    let parsed = parse_to_ast(source).expect("the source parses");
    let library = Ledger::parse(STD).expect("std ships a ledger");
    // Nothing to decide, and nothing claimed: the analysis reports no value,
    // which is honest and is also the hole.
    assert!(
        sharing::analyse(&parsed, &own, &library).is_empty(),
        "a type with no recorded fields cannot be walked into"
    );
}

// --- the corpus -------------------------------------------------------------

/// The repository writes **one** `Shared`, and the prototype makes it atomic
/// because it cannot decide - which is the experiment's own finding turned into
/// a test.
///
/// `examples/fortunes.nika` line 73 takes `db: Shared[postgres::Connection]` and
/// hands it to `all.fetch(db)`. Nothing written down describes `fetch` - its body
/// is the driver's Rust - so ADR-038 D7's case applies and the polarity decides:
/// atomic. The file's own comment says the opposite about the build it is written
/// for ("At `user_parallelism = no` ... `Shared` costs a non-atomic refcount"),
/// and both are right: the per-build expansion gives it `Rc`, and a per-value
/// inference that may not fail open cannot. That gap is `docs/rc-or-arc.md` §6's
/// finding, and this is where it is asserted rather than asserted about.
#[test]
fn the_one_shared_in_the_repository_comes_out_atomic_undecided() {
    let library = Ledger::parse(STD).expect("std ships a ledger");
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut checked = 0;
    let mut found = Vec::new();

    let mut pending = vec![root.join("examples"), root.join("benches")];
    while let Some(dir) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
                continue;
            }
            if path.extension().and_then(|e| e.to_str()) != Some("nika") {
                continue;
            }
            let source = std::fs::read_to_string(&path).expect("read the program");
            let Ok(parsed) = parse_to_ast(&source) else {
                continue;
            };
            let own = Ledger::infer(&parsed);
            for decision in sharing::analyse(&parsed, &own, &library) {
                let name = path.file_name().unwrap().to_string_lossy().to_string();
                found.push((name, decision));
            }
            checked += 1;
        }
    }

    assert!(checked >= 12, "only {checked} programs were analysed");
    assert_eq!(
        found.len(),
        1,
        "the corpus writes exactly one `Shared`: {found:#?}"
    );
    let (file, decision) = &found[0];
    assert_eq!(file, "fortunes.nika");
    assert_eq!(decision.value, "db");
    assert_eq!(decision.count, Count::Atomic, "{decision:#?}");
    assert!(
        decision.undecided,
        "and it is atomic because nothing decided it: {decision:#?}"
    );
}

/// `--sharing` says so rather than printing an empty table.
#[test]
fn the_report_says_when_there_is_nothing_to_choose() {
    let source = "fn main() { println(\"hello\") }";
    let parsed = parse_to_ast(source).expect("the source parses");
    let own = Ledger::infer(&parsed);
    let library = Ledger::parse(STD).expect("std ships a ledger");
    let report = sharing::report(&parsed, &own, &library);
    assert!(report.contains("nothing to choose"), "{report}");
}

/// The report names the crossing beside every atomic count, and counts the ones
/// nothing decided - the list ADR-010 D1 asks to be named.
#[test]
fn the_report_names_what_it_could_not_decide() {
    let source = "pub fn zaehle(counts: Shared[Vec[i64]]) -> i64 { counts.len() }\n\
                  fn lokal(hits: Shared[i64]) -> i64 { 1 }";
    let parsed = parse_to_ast(source).expect("the source parses");
    let own = Ledger::infer(&parsed);
    let library = Ledger::parse(STD).expect("std ships a ledger");
    let report = sharing::report(&parsed, &own, &library);
    assert!(report.contains("could not decide:"), "{report}");
    assert!(report.contains("1 plain, 1 atomic"), "{report}");
}
