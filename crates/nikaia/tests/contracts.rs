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
    assert!(pure.public && pure.throws.is_empty());
    assert_eq!(pure.sync, Sync::Asserted, "the source wrote the word");

    // Neither of these says `sync`, and neither of them calls anything, so
    // neither of them can pause. Before ADR-027 the ledger recorded that as
    // "not `sync`" - a claim about a body it had already read and knew better
    // about. `throws` and `sync` are orthogonal: a function may fail without
    // pausing, and `risky` is one.
    let risky = &l.functions["risky"];
    assert!(!risky.public && !risky.throws.is_empty());
    assert_eq!(risky.sync, Sync::Inferred);

    let plain = &l.functions["plain"];
    assert!(!plain.public && plain.throws.is_empty());
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
         fn reads() -> String throws { return io::read_to_string() }\n\
         fn calls_pure(a: i32) -> i32 { return pure(a) }\n\
         fn calls_reader() -> String throws { return reads() }",
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

/// A higher-order method hands on whatever its lambda does (ADR-029).
///
/// This is the whole of `sync = "from(f)"`. `and_modify` cannot commit to one
/// answer for every caller, because `fn { a + 1 }` and `fn { io::read()… }` are
/// the same `and_modify` and only one of them can pause. Before this it had to
/// commit, and the honest commitment was the pessimistic one - so nothing
/// containing a `map`, a `filter` or an `and_modify` could be `sync`, whatever
/// its lambda did, and nothing containing one could go inside `access`.
#[test]
fn a_higher_order_method_hands_on_what_its_lambda_does() {
    let l = ledger(
        "use std::collections::HashMap\n\
         use std::io\n\
         fn pure(m: HashMap[&str, i64]) { m.entry(\"x\").and_modify fn { a + 1 } }\n\
         fn pausing(m: HashMap[&str, i64]) { m.entry(\"x\").and_modify fn { io::read() catch { } } }",
    );

    assert_eq!(l.functions["pure"].sync, Sync::Inferred);
    assert_eq!(l.functions["pausing"].sync, Sync::No);
}

/// … and the check agrees, on a function that wrote `sync` by hand.
///
/// The two analyses read the same contract, so a `map` over a pure lambda is
/// not a violation and a `map` over a pausing one still is. The second half is
/// the one that matters: `from(f)` must not become a way to smuggle I/O into a
/// lock.
#[test]
fn from_does_not_let_io_into_a_sync_function() {
    let clean =
        violations("fn scale(xs: Vec[i64]) -> i64 sync { xs.sort_by_key fn { a } return 1 }");
    assert!(clean.is_empty(), "{clean:?}");

    let dirty = violations(
        "use std::io\n\
         fn scale(xs: Vec[i64]) -> i64 sync { xs.sort_by_key fn { io::read() catch { } } return 1 }",
    );
    assert_eq!(dirty.len(), 1, "{dirty:?}");
    assert_eq!(dirty[0].callee, "io::read");
}

/// ADR-029 D3's reason carries to `throws`, so `from` needs nothing new for it.
///
/// D3 reads `sync = "from(f)"` as "this call adds no pausing of its own", sound
/// because the lambda runs *during* the call and its calls are already counted in
/// the function that writes it. `throws` is the same walk with the lattice turned
/// around (`contracts::throws`), over the same `visit_expr_blocks` - so a lambda
/// that fails puts its error in the **enclosing** function's set, which is where
/// a caller of *that* function reads it. Nothing is left for a `throws` key on
/// `sort_by_key` to add.
///
/// **The `"?"` that used to stand beside `LeereZeile` is gone**, and that is
/// the second thing asserted here. `contracts::throws` answered every method
/// call with `"?"` rather than asking the type checker, so `sortiere` read
/// "fails with `LeereZeile`, or with something I cannot name" about a call the
/// compiler had already named. Since ADR-028's resolution is handed to this
/// walk too, `xs.sort_by_key` resolves to `Vec::sort_by_key`, whose entry in
/// `std.contracts` carries no `throws` - and an absent `throws` there is a
/// written-down "it cannot fail", reviewed like code, which is the opposite of
/// how an absent `touches` reads and is deliberate (that file's own header).
///
/// So the set is narrower and nothing is assumed: the pessimism that went was
/// pessimism about a **named** call, and every call the compiler still cannot
/// name - a receiver of unknown type, a method no ledger describes - arrives
/// as `"?"` exactly as before. `ohne_lambda` below is that floor held to.
#[test]
fn a_throwing_lambda_is_already_in_the_enclosing_functions_error_set() {
    let l = ledger(
        "enum LeereZeile { Leer }\n\
         fn pruefe(n: &i64) -> i64 throws {\n\
             if n == 0 { throw LeereZeile::Leer }\n\
             return 1\n\
         }\n\
         fn sortiere(xs: Vec[i64]) -> i64 throws {\n\
             xs.sort_by_key fn { pruefe(a) }\n\
             return 1\n\
         }\n\
         fn ohne_lambda(xs: Vec[i64]) -> i64 throws {\n\
             xs.sort_by_key fn { a }\n\
             return 1\n\
         }",
    );

    assert_eq!(l.functions["pruefe"].throws, ["LeereZeile"]);
    assert_eq!(l.functions["sortiere"].throws, ["LeereZeile"]);
    // The same call over a lambda that cannot fail contributes nothing at all,
    // so `LeereZeile` above came from the lambda's body and from nowhere else.
    // The entry stays `["?"]` because the *declaration* is a promise a caller
    // already relies on and the inference is here to say more than it, never
    // less (ADR-027 D4's polarity, in `throws`).
    assert_eq!(l.functions["ohne_lambda"].throws, ["?"]);
}

/// A `while` body is part of the function around it.
///
/// Here because `while` arrived from another branch while this analysis was
/// being built, and a statement the walk does not descend into is a silent
/// hole: a loop body doing I/O would leave its function looking pure. Cheap to
/// check, and the kind of thing a merge is exactly where it goes wrong.
#[test]
fn a_while_body_is_walked_like_any_other() {
    let l = ledger(
        "use std::io\n\
         fn reads() -> i64 throws { let mut n = 0 while n < 3 { let t = io::read_to_string() n += 1 } return n }\n\
         fn counts(n: i64) -> i64 { let mut i = 0 while i < n { i += 1 } return i }",
    );

    assert_eq!(l.functions["reads"].sync, Sync::No);
    assert_eq!(l.functions["counts"].sync, Sync::Inferred);
}

/// An asserted `sync` may not survive a call nothing can resolve.
///
/// [ADR-027](../../../docs/specification/adr/adr-027.md) D2 makes the
/// *inference* conservative in the restrictive direction, and D4 says an
/// **assertion** is never overwritten by it. Those two together meant a source
/// that wrote `sync` and called something no ledger knows kept `sync = true` -
/// in a file that ships ([ADR-020](../../../docs/specification/adr/adr-020.md)),
/// so a consumer's `par_iter` body would believe it.
///
/// The check was permissive here for a diagnostic reason: `NK2202` wants a
/// caret on the call. But the caret is available - Part III C.2 reports this
/// checker at *statement* granularity already - so the permissiveness was
/// buying nothing and costing the polarity
/// [ADR-010](../../../docs/specification/adr/adr-010.md) D1 exists to protect.
///
/// Found by the ADR-038 D7 experiment: a `sync` function calling into a foreign
/// crate is exactly this shape, and a foreign crate has no contract by
/// definition.
#[test]
fn an_asserted_sync_does_not_survive_an_unresolvable_call() {
    let source = "fn versprochen(n: i64) -> i64 sync { fremd::macht_irgendwas(n) return n + 1 }";

    let found = violations(source);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(
        found[0].callee.contains("fremd::macht_irgendwas"),
        "and it must name the call rather than the function: {found:?}"
    );
}

/// A method call stays permissive here, and that is not the same hole.
///
/// `stats.add(5)` names `add` and says nothing about what `stats` is. The
/// **type checker** resolves it (ADR-028) and the inference merges that answer
/// in per function, so the claim is still taken away where it has to be - the
/// check simply is not the place that does it. The test above must not be read
/// as making every unresolved thing an error.
#[test]
fn a_method_call_is_asked_elsewhere_rather_than_refused_here() {
    let source = "fn ordne(xs: Vec[i64]) -> i64 { xs.sort_by_key fn { a } return 1 }\n\
                  fn im_lock(xs: Vec[i64]) -> i64 sync { return ordne(xs) }";
    assert!(violations(source).is_empty());
}

/// The crossover, in the smallest program that shows it.
///
/// A helper nobody annotated uses an iterator method over a pure lambda, and a
/// `sync` function calls it. Before `from(f)`, `sort_by_key` had to be recorded
/// as able to pause - it cannot say "it depends" - so the helper could not be
/// `sync`, and the call was reported:
///
/// ```text
/// error[NK2202]: `im_lock` is `sync`, and `ordne` can pause
/// ```
///
/// That was a **correct program being rejected**, which is the one thing the
/// compiler is not allowed to do. Nothing about the helper could pause; the
/// ledger simply had no way to say so.
#[test]
fn a_helper_that_uses_an_iterator_method_may_be_called_from_a_lock() {
    let source = "fn ordne(xs: Vec[i64]) -> i64 { xs.sort_by_key fn { a } return 1 }\n\
                  fn im_lock(xs: Vec[i64]) -> i64 sync { return ordne(xs) }";

    assert_eq!(ledger(source).functions["ordne"].sync, Sync::Inferred);
    let found = violations(source);
    assert!(found.is_empty(), "{found:?}");
}

/// The chain, end to end, on the shape that held the last three ADRs up
/// (ADR-031).
///
/// `HashMap[&str, Stats]` binds `$V` to `Stats`, so `entry` hands back an
/// `Entry[Stats]`, so `and_modify`'s `fn(&$V)` is a `fn(&Stats)`, so the `a` in
/// the lambda is a `&Stats`, so `a.add(v)` resolves to `Stats::add`, which is
/// pure - and `record` is `sync` without anyone writing the word.
///
/// Every one of those links had to exist. This is the test that they do.
#[test]
fn a_map_of_structs_types_its_lambda_all_the_way_down() {
    let l = ledger(
        "use std::collections::HashMap\n\
         pub struct Stats { n: i64 }\n\
         impl Stats {\n\
             pub fn(first: i64) -> Stats { return Stats(n: first) }\n\
             fn add(&mut self, x: i64) { self.n += x }\n\
         }\n\
         pub struct Summary { stations: HashMap[&str, Stats] }\n\
         impl Summary {\n\
             fn record(&mut self, name: &str, v: i64) {\n\
                 self.stations.entry(name).and_modify fn { a.add(v) }.or_insert_with fn { Stats(v) }\n\
             }\n\
         }",
    );

    assert_eq!(l.functions["Summary::record"].sync, Sync::Inferred);
}

/// A variable in an **argument** would reject correct programs, so there is
/// none (ADR-031 D3).
///
/// Rust's `HashMap::get` takes anything the key borrows as, so a map with
/// `String` keys is correctly asked with a `&str` - `k-nucleotide.nika` does
/// exactly this. Writing `key: $K` would have made the checker reject it, which
/// is the one thing the checker may not do. The rule that prevents it is that a
/// variable says what flows *out*; `?` stays for what flows *in*.
#[test]
fn a_key_may_be_given_as_something_it_borrows_as() {
    let source = "use std::collections::HashMap\n\
                  fn find(m: HashMap[String, i64]) -> i64 { let hit = m.get(\"x\") return 1 }";
    let parsed = parse_to_ast(source).expect("the source parses");
    let library = Ledger::parse(STD).expect("std's ledger parses");
    let own = Ledger::infer(&parsed);

    let findings = nikaia::check::check(&parsed, &own, &library).findings;
    assert!(findings.is_empty(), "{findings:?}");
}

/// A receiver that says nothing degrades to `?` rather than guessing.
///
/// `HashMap::new()` gives a map whose type arguments are unknown, so `$V` binds
/// to nothing and the lambda's parameter is `?`. The chain above simply stops
/// being able to help, which is the correct failure: an unbound variable is the
/// absence of a claim, never a claim about a type called `$V`.
#[test]
fn an_unknown_element_type_does_not_become_a_claim() {
    let l = ledger(
        "use std::collections::HashMap\n\
         fn build() { let m = HashMap::new() m.entry(\"x\").and_modify fn { a.whatever() } }",
    );

    // `a.whatever()` cannot be resolved, so the claim is refused - and refused
    // for the right reason rather than by an error about `$V`.
    assert_eq!(l.functions["build"].sync, Sync::No);
}

/// **`from` is only sound for a lambda that runs before the call returns.**
///
/// A caller reads `from(f)` as "this call adds no pausing of its own", and that
/// holds because the lambda's body is part of the function that writes it -
/// Part I 5.4's `@immediate` - so its calls are already counted there. A
/// parameter the callee **stored or spawned** (`@detached`) would break it: the
/// lambda's calls would belong to nobody the caller is counting, and a pausing
/// body would slip into a lock with the ledger saying it could not.
///
/// The ledger cannot spell `@detached` yet, so this is the guard: every `std`
/// entry that says `from` must name a parameter that is a lambda, and `std` has
/// no detached one. The day it does, `from` needs a companion and this test is
/// where that is noticed.
#[test]
fn a_detached_lambda_may_not_use_from() {
    let shipped = std::fs::read_to_string(repo_root().join("crates/nikaia-std/std.contracts"))
        .expect("std ships a ledger");
    let shipped = Ledger::parse(&shipped).expect("std's ledger parses");

    let mut checked = 0;
    for (name, contract) in &shipped.functions {
        let Some(parameter) = contract.sync.from() else {
            continue;
        };
        let signature = contract
            .signature
            .as_ref()
            .unwrap_or_else(|| panic!("`{name}` says `from({parameter})` and has no signature"));
        let found = signature
            .arguments()
            .iter()
            .find(|(argument, _)| argument == parameter)
            .unwrap_or_else(|| {
                panic!("`{name}` says `from({parameter})` and has no parameter `{parameter}`")
            });
        assert!(
            matches!(found.1, nikaia::contracts::ty::Ty::Fn { .. }),
            "`{name}` says `from({parameter})`, and `{parameter}` is `{}` rather than a lambda",
            found.1.text()
        );
        checked += 1;
    }
    assert!(
        checked > 0,
        "no `from` entry was checked, so nothing was proved"
    );
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
                  fn reads() -> String throws { return io::read_to_string() }\n\
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
        from_declarations.fallible_methods, from_finished.fallible_methods,
        "which method calls can fail changed once `sync` was filled in"
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
         fn ping(n: i32) -> i32 throws { if n == 0 { return io::read_to_string().len() } return pong(n - 1) }\n\
         fn pong(n: i32) -> i32 throws { return ping(n - 1) }",
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
                       pub fn(n: i64) -> S throws { let t = io::read_to_string() return S(n: n) }\n\
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
         fn main() throws { let data = fs::map(\"x\") }",
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
         fn main() throws { let a = fs::map(\"x\") let b = http::body() }",
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
    use nikaia::emit::{emit_program_with_trust, Build};

    let source = "fn main() { let m: HashMap[&str, i64] = HashMap::new() }";
    let parsed = parse_to_ast(source).expect("parses");

    let trusted = emit_program_with_trust(&parsed, Build::default(), Provenance::Trusted)
        .expect("lowers")
        .rust;
    assert!(trusted.contains("TrustedMap<&str, i64>"), "{trusted}");
    assert!(trusted.contains("TrustedMap::default()"), "{trusted}");

    let untrusted = emit_program_with_trust(&parsed, Build::default(), Provenance::Untrusted)
        .expect("lowers")
        .rust;
    assert!(untrusted.contains("HashMap<&str, i64>"), "{untrusted}");
    assert!(untrusted.contains("HashMap::new()"), "{untrusted}");
}

/// A call inside a hole is a call, and the **inference** has to see it.
///
/// This is the half of the blind spot that was more than a missed diagnostic.
/// `sync` is inferred from what a body calls (ADR-027), the hole's expression
/// was parsed on the way to the emitter, and so a function whose only pausing
/// call sat inside `"{…}"` was recorded `sync = "inferred"` - a claim, in a
/// file a library ships, that it cannot pause. ADR-027 D2 and ADR-010 D1 both
/// name that direction the dangerous one: an analysis that fails open is a
/// vulnerability generator.
#[test]
fn a_pausing_call_inside_a_hole_costs_the_sync_claim() {
    let l = ledger(
        "use std::io\n\
         pub fn hidden() -> String { return f\"hello {io::read_to_string()}\" }\n\
         pub fn plain() -> String { let who = io::read_to_string() return f\"hello {who}\" }\n\
         pub fn pure() -> String { let n = 1 return f\"hello {n}\" }",
    );

    assert!(
        !l.functions["hidden"].sync.is_sync(),
        "a call inside a hole was invisible to the inference"
    );
    // The same call written outside a hole, for comparison - the two spellings
    // have to give the same answer, which is the whole point.
    assert!(!l.functions["plain"].sync.is_sync());
    // …and a hole with nothing in it that can pause still earns the claim, so
    // this is not a blanket refusal of anything holding a `{`.
    assert!(l.functions["pure"].sync.is_sync());
}

/// The **check** sees into a hole too, so an asserted `sync` is held to it.
#[test]
fn a_sync_function_may_not_pause_inside_a_hole_either() {
    let found = violations(
        "use std::io\n\
         pub fn greet() -> String sync { return f\"hello {io::read_to_string()}\" }",
    );
    assert_eq!(found.len(), 1, "{found:#?}");
    assert_eq!(found[0].caller, "greet");
    assert_eq!(found[0].callee, "io::read_to_string");
}
