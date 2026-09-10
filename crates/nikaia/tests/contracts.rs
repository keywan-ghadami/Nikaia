//! The Borrow Contract Ledger (Part III, 13.5; ADR-020).
//!
//! Two things are checked here. That the ledger says what the source says -
//! `sync`, `throws`, and what a result may point into - and that **`std`'s
//! shipped ledger has not drifted from `std`'s Nikaia sources**, which is the
//! half of "a package ships its ledger" that a reviewer cannot do by eye.

use std::path::PathBuf;

use nikaia::contracts::{Ledger, Sync, STD};
use nikaia::parser::parse_to_ast;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn ledger(source: &str) -> Ledger {
    Ledger::infer(&parse_to_ast(source).expect("the source parses"))
}

/// `throws` is declared, so it is recorded exactly; `sync` is declared *or*
/// earned, and the ledger says which (ADR-027).
#[test]
fn a_declaration_is_recorded_as_it_was_written() {
    let l = ledger(
        "pub fn pure(a: i32) -> i32 sync { return a }\n\
         fn risky() throws { }\n\
         fn plain() { }",
    );

    let pure = &l.functions["pure"];
    assert!(pure.public && !pure.throws);
    assert_eq!(pure.sync, Sync::Asserted, "the source wrote the word");

    // Neither of these says `sync`, and neither of them calls anything, so
    // neither of them can pause. Before ADR-027 the ledger recorded that as
    // "not `sync`" - a claim about a body it had already read and knew better
    // about. `throws` and `sync` are orthogonal: a function may fail without
    // pausing, and `risky` is one.
    let risky = &l.functions["risky"];
    assert!(!risky.public && risky.throws);
    assert_eq!(risky.sync, Sync::Inferred);

    let plain = &l.functions["plain"];
    assert!(!plain.public && !plain.throws);
    assert_eq!(plain.sync, Sync::Inferred);
}

/// The inference claims `sync` only where it can prove it, and a call it cannot
/// resolve is not a proof.
///
/// This is the polarity that matters: the entry is **shipped**, and a consumer
/// reads it and puts the function inside `access`. So an unresolvable call
/// costs the claim rather than being waved through - the opposite of what the
/// *check* does with the same call, and for the opposite reason.
#[test]
fn what_cannot_be_resolved_is_not_inferred_sync() {
    let l = ledger(
        "use std::io\n\
         fn pure(a: i32) -> i32 { return a + 1 }\n\
         fn reads() -> String throws { return io::read_to_string()? }\n\
         fn calls_pure(a: i32) -> i32 { return pure(a) }\n\
         fn calls_reader() -> String throws { return reads()? }",
    );

    assert_eq!(l.functions["pure"].sync, Sync::Inferred);
    // `std`'s ledger says `io::read_to_string` can pause.
    assert_eq!(l.functions["reads"].sync, Sync::No);
    // Transitive, in both directions.
    assert_eq!(l.functions["calls_pure"].sync, Sync::Inferred);
    assert_eq!(l.functions["calls_reader"].sync, Sync::No);
}

/// A method call is resolved by the **type checker**, and the inference uses
/// its answer (ADR-028).
///
/// Before this, every method call was an unknown and cost the function its
/// claim - which is what made the restrictive polarity above so expensive, and
/// why `Summary::record` in `1brc.nika` could not earn a `sync` it deserved.
/// The receiver's type is what was missing, and the checker had it all along.
#[test]
fn a_method_call_is_resolved_through_the_receiver() {
    let l = ledger(
        "fn counted(s: String) -> usize { return s.len() }\n\
         fn shouty(s: String) -> String { return s.to_uppercase() }",
    );

    // `String::len` is in `std`'s ledger and says `sync`.
    assert_eq!(l.functions["counted"].sync, Sync::Inferred);
    // `String::to_uppercase` is not, and an absent entry is the absence of an
    // answer rather than permission to assume one.
    assert_eq!(l.functions["shouty"].sync, Sync::No);
}

/// A method that runs a lambda carries no `sync`, so a caller of one does not
/// get the promise either.
///
/// `Entry::and_modify` does whatever its lambda does. Until a ledger can say
/// `sync = "from(f)"` (ADR-027 §7), the honest entry is one with no `sync` on
/// it, and this is what that costs: the type resolves, and the claim still does
/// not follow.
#[test]
fn a_higher_order_method_does_not_hand_on_a_promise() {
    let l = ledger(
        "use std::collections::HashMap\n\
         fn bump(m: HashMap[&str, i64]) { m.entry(\"x\").or_insert(0) }\n\
         fn tweak(m: HashMap[&str, i64]) { m.entry(\"x\").and_modify fn { a + 1 } }",
    );

    // `entry` and `or_insert` are both plain computation.
    assert_eq!(l.functions["bump"].sync, Sync::Inferred);
    // `and_modify` runs what it is given, and says so by saying nothing.
    assert_eq!(l.functions["tweak"].sync, Sync::No);
}

/// The type checker does not depend on the `sync` it helps infer.
///
/// This is the invariant the whole of ADR-028 rests on. `Ledger::infer` reads
/// the declarations, runs the checker against *that* ledger to resolve method
/// calls, and then infers `sync` using the answers. That is sound only because
/// the checker reads `signature`, `fields` and `iterates` and never `sync` - so
/// resolving a method against the half-finished ledger gives what resolving it
/// against the finished one would.
///
/// Were it ever to read `sync`, this ordering would become a guess about a
/// fixpoint and the ledger would stop being a pure function of its source.
/// Checking it by hand means reading the module; this checks it by running.
#[test]
fn the_checker_does_not_depend_on_the_sync_it_helps_infer() {
    let source = "use std::io\n\
                  fn pure(a: i32) -> i32 { return a + 1 }\n\
                  fn counted(s: String) -> usize { return s.len() }\n\
                  fn reads() -> String throws { return io::read_to_string()? }\n\
                  pub struct S { n: i64 }\n\
                  impl S {\n\
                      pub fn(n: i64) -> S { return S(n: n) }\n\
                      fn use_it(&self) -> i64 sync { return self.n }\n\
                  }";
    let parsed = parse_to_ast(source).expect("the source parses");
    let library = Ledger::parse(STD).expect("std's ledger parses");

    let (finished, from_declarations) = Ledger::infer_checked(&parsed);

    // The ledger really did gain something in the second pass - otherwise this
    // test would pass by proving nothing.
    assert!(
        finished
            .functions
            .values()
            .any(|c| c.sync == Sync::Inferred),
        "nothing was inferred, so there is no difference to be insensitive to"
    );

    // Run the checker again, this time against the *finished* ledger.
    let from_finished = nikaia::check::check(&parsed, &finished, &library);

    assert_eq!(
        from_declarations.methods, from_finished.methods,
        "method resolution changed once `sync` was filled in"
    );
    assert_eq!(
        from_declarations.fallible_loops, from_finished.fallible_loops,
        "which loops can fail changed once `sync` was filled in"
    );
    assert_eq!(
        from_declarations.findings.len(),
        from_finished.findings.len(),
        "the findings changed once `sync` was filled in"
    );
}

/// A pure helper nobody annotated is callable from a `sync` function.
///
/// This is what ADR-027 is *for*. `access`, `access_all` and `par_iter` all
/// demand a `sync` lambda (Part II, 12.2), and while `sync` was opt-in the set
/// of things such a lambda could call was "whatever someone remembered to
/// annotate". Now it is "whatever provably cannot pause".
#[test]
fn a_sync_function_may_call_a_helper_that_never_said_sync() {
    let found = violations(
        "fn helper(a: i32) -> i32 { return a + 1 }\n\
         fn locked(a: i32) -> i32 sync { return helper(a) }",
    );
    assert!(found.is_empty(), "{found:?}");
}

/// Mutual recursion between pure functions keeps the claim.
///
/// The fixpoint is a greatest one - start from "everything is `sync`" and take
/// the claim away - which is what gets this right. A least fixpoint would never
/// give either of them the promise, because each is waiting on the other.
#[test]
fn mutual_recursion_between_pure_functions_stays_sync() {
    let l = ledger(
        "fn even(n: i32) -> bool { if n == 0 { return true } return odd(n - 1) }\n\
         fn odd(n: i32) -> bool { if n == 0 { return false } return even(n - 1) }",
    );

    assert_eq!(l.functions["even"].sync, Sync::Inferred);
    assert_eq!(l.functions["odd"].sync, Sync::Inferred);
}

/// … and mutual recursion that reaches I/O loses it, on both sides.
#[test]
fn mutual_recursion_that_reaches_io_is_not_sync() {
    let l = ledger(
        "use std::io\n\
         fn ping(n: i32) -> i32 throws { if n == 0 { return io::read_to_string()?.len() } return pong(n - 1)? }\n\
         fn pong(n: i32) -> i32 throws { return ping(n - 1)? }",
    );

    assert_eq!(l.functions["ping"].sync, Sync::No);
    assert_eq!(l.functions["pong"].sync, Sync::No);
}

/// The answer does not depend on the order the source declared things in.
///
/// 13.5 makes the ledger a pure function of (source, toolchain) and `--locked`
/// compares it byte for byte, so an inference that walked the call graph in
/// declaration order would be a determinism bug waiting for someone to move a
/// function.
#[test]
fn the_inference_does_not_depend_on_declaration_order() {
    let forwards = "fn a(n: i32) -> i32 { return b(n) }\n\
                    fn b(n: i32) -> i32 { return c(n) }\n\
                    fn c(n: i32) -> i32 { return n }";
    let backwards = "fn c(n: i32) -> i32 { return n }\n\
                     fn b(n: i32) -> i32 { return c(n) }\n\
                     fn a(n: i32) -> i32 { return b(n) }";

    for source in [forwards, backwards] {
        let l = ledger(source);
        for name in ["a", "b", "c"] {
            assert_eq!(l.functions[name].sync, Sync::Inferred, "{name} in {source}");
        }
    }
}

/// A task's body runs later and elsewhere, so a function that starts one is not
/// the pure CPU task 12.1 describes - whatever the task turns out to do.
#[test]
fn starting_a_task_is_not_pure_computation() {
    // Part I 8.2's form as the parser takes it today: `spawn(fn { … })`.
    let l = ledger("fn go(n: i32) { spawn(fn { n + 1 }) }");
    assert_eq!(l.functions["go"].sync, Sync::No);
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
    assert_eq!(l.functions["Stats::add"].sync, Sync::Asserted);
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

// --- Part II 12.1, checked ---------------------------------------------------

use nikaia::contracts::sync;

fn violations(source: &str) -> Vec<sync::Violation> {
    let parsed = parse_to_ast(source).expect("the source parses");
    let own = Ledger::infer(&parsed);
    let library = Ledger::parse(STD).expect("std's ledger parses");
    sync::check(&parsed, &own, &library)
}

/// The rule ADR-019 D2 rests on: a `sync` function may not reach standard
/// input, and the answer comes from the library's ledger rather than from
/// anything this program says.
#[test]
fn a_sync_function_may_not_call_into_std_io() {
    let found = violations(
        "use std::io\n\
         fn tally() -> i64 sync { let t = io::read_to_string() catch { return 0 } return 1 }",
    );

    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].caller, "tally");
    assert_eq!(found[0].callee, "io::read_to_string");
    assert!(found[0].from_library);
}

/// … and may call what the ledger says is `sync`.
#[test]
fn a_sync_function_may_call_a_sync_one() {
    let found = violations(
        "fn helper(a: i32) -> i32 sync { return a + 1 }\n\
         fn outer(a: i32) -> i32 sync { return helper(a) }",
    );
    assert!(found.is_empty(), "{found:?}");
}

/// A function that is not `sync` promises nothing and is checked for nothing.
#[test]
fn a_function_that_is_not_sync_may_call_anything() {
    let found = violations(
        "use std::io\n\
         fn read() -> i64 { let t = io::read_to_string() catch { return 0 } return 1 }",
    );
    assert!(found.is_empty(), "{found:?}");
}

/// The anonymous constructor is a function like any other, and reached under
/// the name the lowering gives it.
///
/// This is what the check found in `1brc.nika` and `access-log.nika` on its
/// first run: a `sync` method building a value through a constructor that never
/// said it was `sync` either.
///
/// **ADR-027 changed the answer here, and it is the change worth looking at.**
/// The constructor builds a struct literal and calls nothing, so it cannot
/// pause, so it is `sync` whether or not anyone wrote the word - and the caller
/// that used to be rejected is now accepted. What the check was reporting was
/// never a program that could pause; it was a missing annotation. The rule it
/// enforces has not moved an inch: what a `sync` function may call is what
/// cannot pause. Only the ledger's answer to "can this pause" got better.
#[test]
fn a_constructor_makes_the_same_promise_or_does_not() {
    let pure = "pub struct S { n: i64 }\n\
                impl S {\n\
                    pub fn(n: i64) -> S { return S(n: n) }\n\
                    fn use_it(&self) sync { let x = S(1) }\n\
                }";
    assert!(violations(pure).is_empty(), "{:?}", violations(pure));

    // The guarantee itself is unchanged, and this is it: a constructor that
    // really can pause still takes the caller's promise down with it.
    let pausing = "use std::io\n\
                   pub struct S { n: i64 }\n\
                   impl S {\n\
                       pub fn(n: i64) -> S throws { let t = io::read_to_string()? return S(n: n) }\n\
                       fn use_it(&self) sync { let x = S(1) }\n\
                   }";
    let found = violations(pausing);
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].callee, "S::new");
    assert!(!found[0].from_library);
}

/// A trailing lambda runs during the call it is given to, so what it calls, the
/// function around it calls.
#[test]
fn a_lambda_is_part_of_the_function_that_writes_it() {
    let found = violations(
        "use std::io\n\
         fn f(xs: Vec[i32]) -> i32 sync { xs.map fn { io::read() catch { 0 } } return 1 }",
    );
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].callee, "io::read");
}

/// What the check cannot resolve it does not reject.
///
/// With no type checker there is no receiver type, so a method call cannot be
/// looked up. The check is therefore conservative in the permissive direction:
/// it never rejects a program the rule allows, and it does not yet catch every
/// program the rule forbids. Worth having and worth saying - an unchecked
/// promise catches nothing at all.
#[test]
fn a_call_that_cannot_be_resolved_is_not_a_violation() {
    let found = violations("fn f(s: String) -> usize sync { return s.len() }");
    assert!(found.is_empty(), "{found:?}");
}

// --- ADR-010: where the bytes came from --------------------------------------

use nikaia::contracts::{trust, Provenance};

fn provenance(source: &str) -> trust::Trust {
    let parsed = parse_to_ast(source).expect("the source parses");
    let library = Ledger::parse(STD).expect("std's ledger parses");
    trust::analyse(&parsed, &library)
}

/// Every source `std` has today is the operator's own, so a program that reads
/// a file it was pointed at is trusted - and its maps may have the fast hash.
#[test]
fn a_program_that_reads_a_file_is_trusted() {
    let t = provenance(
        "use std::fs\n\
         fn main() throws { let data = fs::map(\"x\")? }",
    );
    assert_eq!(t.provenance, Provenance::Trusted);
    assert_eq!(t.reasons.len(), 1);
    assert_eq!(t.reasons[0].source, "fs::map");
}

/// A program that reads nothing has no input to distrust: its maps are keyed by
/// what it wrote itself, which is the compiled-in case of ADR-010 D2.
#[test]
fn a_program_that_reads_nothing_is_trusted_and_says_why() {
    let t = provenance("fn main() { println(\"hello\") }");
    assert_eq!(t.provenance, Provenance::Trusted);
    assert!(t.reasons.is_empty());
    assert!(
        trust::render(&t).contains("no source is read"),
        "{}",
        trust::render(&t)
    );
}

/// The join is over every source, so one untrusted input decides the answer -
/// conservative in the safe direction, always (ADR-010 D1).
///
/// `std` has no untrusted source yet: `http`, `net` and `db` arrive with the
/// modules that have them. This checks the lattice on a ledger that does, so
/// that the join is tested rather than assumed on the day one appears.
#[test]
fn one_untrusted_source_decides() {
    let library = Ledger::parse(
        "version = 1\n\
         toolchain = \"t\"\n\
         inference = \"stage0-signatures\"\n\
         [fn.\"fs::map\"]\n\
         provenance = \"trusted\"\n\
         [fn.\"http::body\"]\n\
         provenance = \"untrusted\"\n",
    )
    .expect("the test ledger parses");

    let parsed = parse_to_ast(
        "use std::fs\n\
         fn main() throws { let a = fs::map(\"x\")? let b = http::body() }",
    )
    .expect("parses");

    let t = trust::analyse(&parsed, &library);
    assert_eq!(t.provenance, Provenance::Untrusted);
    assert_eq!(t.reasons.len(), 2);

    let rendered = trust::render(&t);
    assert!(rendered.contains("http::body is untrusted"), "{rendered}");
    assert!(
        rendered.contains("keyed, per-process random seed"),
        "{rendered}"
    );
}

/// The lattice, on its own terms.
#[test]
fn the_join_is_conservative() {
    use Provenance::{Trusted, Untrusted};
    assert_eq!(Trusted.join(Trusted), Trusted);
    assert_eq!(Trusted.join(Untrusted), Untrusted);
    assert_eq!(Untrusted.join(Trusted), Untrusted);
    assert_eq!(Untrusted.join(Untrusted), Untrusted);
}

/// A source is a source wherever it is called from, including inside a nested
/// block - the walk is the same one the `sync` check uses.
#[test]
fn a_source_is_found_inside_a_nested_block() {
    let t = provenance(
        "use std::io\n\
         fn main() { for i in 0..2 { let t = io::read_to_string() catch { return } } }",
    );
    assert_eq!(t.reasons.len(), 1);
    assert_eq!(t.reasons[0].source, "io::read_to_string");
}

/// The provenance decides the map, which is the only thing it decides
/// (ADR-010 D5): same table, same API, a different hash.
///
/// It is not in the build cache's key and does not need to be: it is a function
/// of the source and of what `std`'s ledger says about the sources that source
/// calls, and the compiler's fingerprint covers `std.contracts` by name
/// (`crates/nikaia/build.rs`).
#[test]
fn the_provenance_chooses_the_map() {
    use nikaia::emit::{emit_program_with_trust, Profile};

    let source = "fn main() { let m: HashMap[&str, i64] = HashMap::new() }";
    let parsed = parse_to_ast(source).expect("parses");

    let trusted = emit_program_with_trust(&parsed, Profile::Advanced, Provenance::Trusted)
        .expect("lowers")
        .rust;
    assert!(trusted.contains("TrustedMap<&str, i64>"), "{trusted}");
    assert!(trusted.contains("TrustedMap::default()"), "{trusted}");

    let untrusted = emit_program_with_trust(&parsed, Profile::Advanced, Provenance::Untrusted)
        .expect("lowers")
        .rust;
    assert!(untrusted.contains("HashMap<&str, i64>"), "{untrusted}");
    assert!(untrusted.contains("HashMap::new()"), "{untrusted}");
}
