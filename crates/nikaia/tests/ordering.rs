//! Order is kept where it can be seen (ADR-033, Part I 8.1.1).
//!
//! Two adjacent statements whose calls reach different files are lowered to run
//! at the same time. Everything here is either that working, or one of the
//! reasons it must not - and the second list is the longer one on purpose,
//! because the decision is only safe if every "no" is reliable.
//!
//! The shapes are ADR-033 §8.3's first item: a `let`, a **bare expression
//! statement**, and a value built out of literals and calls rather than being
//! one call. §8.1 measured the narrow version and found 101 of 127 refusals
//! falling out on the shape alone, which measured the analysis and not the
//! corpus - so each widening below has a test for the operation it now sees and
//! a test for the refusal it must still make.
//!
//! The lowering is not taken on trust: the emitted Rust is compiled and run,
//! and the program prints what the sequential one would have printed.

mod common;

use std::path::PathBuf;

use nikaia::emit::{self, Build, Ordering};
use nikaia::parser::parse_to_ast;

/// Overlapping is only reachable with parallelism asked for: at
/// `user_parallelism = no` nothing the user wrote may run concurrently, and
/// the two closures `task::both` takes are code the user wrote (ADR-037 D2).
/// So these tests are about `yes`, and the one below checks the `no` side.
fn lowered(source: &str, ordering: Ordering) -> String {
    let parsed = parse_to_ast(source).expect("the source parses");
    emit::emit_program_ordered(&parsed, Build::parallel(), ordering)
        .expect("the source lowers")
        .rust
}

/// Whether the emitted Rust runs the two calls together.
fn overlaps(source: &str) -> bool {
    lowered(source, Ordering::Effects).contains("task::both")
}

const TWO_READS: &str = "use std::fs\n\
     fn main() throws {\n\
         let a = fs::read_to_string(\"eins.txt\") catch { \"\".to_string() }\n\
         let b = fs::read_to_string(\"zwei.txt\") catch { \"\".to_string() }\n\
         println(f\"{a.len()} {b.len()}\")\n\
     }";

/// Two reads of different files meet on nothing.
#[test]
fn two_reads_of_different_files_overlap() {
    assert!(
        overlaps(TWO_READS),
        "{}",
        lowered(TWO_READS, Ordering::Effects)
    );
}

/// At `user_parallelism = no` there is nothing to overlap with.
///
/// Part I 1.2 promises that nothing **you** wrote ever runs concurrently at
/// `no`, and Part III 15.3 promises a `wasm32-unknown` build with no OS-level
/// mutexes or atomics - a target where `rayon::join` does not even link.
/// `--ordering effects` is a question about the program; whether a vehicle
/// exists to answer it with is a question about the build, and `no` answers
/// that one no. So this degrades exactly as `par_fold` degrades to a
/// sequential `fold` (ADR-009), rather than quietly contradicting the switch
/// in the same file that emits `Parallelism::Off`.
#[test]
fn no_user_parallelism_never_spawns_a_thread() {
    let parsed = parse_to_ast(TWO_READS).expect("the source parses");
    let sequential = emit::emit_program_ordered(&parsed, Build::default(), Ordering::Effects)
        .expect("the source lowers")
        .rust;
    assert!(
        !sequential.contains("task::both"),
        "`user_parallelism = no` overlapped under `--ordering effects`:\n{sequential}"
    );

    // …and it is the sequential program, not merely a different one.
    let strict = emit::emit_program_ordered(&parsed, Build::default(), Ordering::Strict)
        .expect("the source lowers")
        .rust;
    assert_eq!(sequential, strict, "the two orderings differ at `no`");

    // The guard has to be the switch and not the analysis: `yes` still
    // overlaps the same program, or this test would pass for the wrong reason.
    assert!(overlaps(TWO_READS));
}

/// A `catch` handler's own effects are part of what the statement touches.
///
/// The analysis looks *past* a `catch` at the call it guards, so without this
/// the handler below would be invisible: the pair would meet on nothing and
/// overlap, and the overlapped program could print its two lines in either
/// order. A handler is code that runs, so what it reaches counts (D2), and
/// where it cannot be read D4 says the statement reaches everything.
#[test]
fn a_handlers_own_effects_are_part_of_the_statement() {
    const HANDLER_WRITES_STDOUT: &str = "use std::fs\n\
         fn main() {\n\
         \x20   fs::write(\"a.txt\", \"x\") catch { println(\"failed\") }\n\
         \x20   println(\"next\")\n\
         }";

    assert!(
        !overlaps(HANDLER_WRITES_STDOUT),
        "the handler writes stdout and so does the next statement:\n{}",
        report(HANDLER_WRITES_STDOUT)
    );
    let report = report(HANDLER_WRITES_STDOUT);
    assert!(
        report.contains("stdout") && report.contains("println"),
        "and the refusal must name what they meet on: {report}"
    );
}

/// A handler this analysis cannot read makes the statement reach everything.
///
/// `catch { …; … }` is more than one statement, which the walk stops at. The
/// answer then has to be the fail-closed one, or a handler could smuggle an
/// effect past the touch set - which is exactly the hole this pair is here to
/// keep shut (ADR-010 D1's polarity, applied to a third question).
#[test]
fn an_unreadable_handler_is_not_an_empty_one() {
    const HANDLER_DOES_MORE: &str = "use std::fs\n\
         fn main() throws {\n\
         \x20   let a = fs::read_to_string(\"eins.txt\") catch { \
         fs::write(\"zwei.txt\", \"x\") catch { }; \"\".to_string() }\n\
         \x20   let b = fs::read_to_string(\"zwei.txt\") catch { \"\".to_string() }\n\
         \x20   println(f\"{a.len()} {b.len()}\")\n\
         }";

    assert!(
        !overlaps(HANDLER_DOES_MORE),
        "the first handler writes the file the second statement reads:\n{}",
        report(HANDLER_DOES_MORE)
    );
}

/// … and the emitted Rust compiles and prints what the sequential one would.
///
/// The half that cannot be checked by reading the output: a lowering that
/// produces plausible-looking Rust which does not build, or builds and prints
/// something else, is worth nothing at all.
#[test]
fn the_overlapped_program_compiles_and_runs() {
    let dir = common::scratch_dir("ordering");
    let source = dir.join("two_reads.rs");
    std::fs::write(&source, lowered(TWO_READS, Ordering::Effects)).expect("write the Rust");
    std::fs::write(dir.join("eins.txt"), "hallo").expect("write eins");
    std::fs::write(dir.join("zwei.txt"), "welt!!").expect("write zwei");

    let binary = dir.join("two_reads");
    let built = common::compile(&source, &["-o", &binary.to_string_lossy()]);
    assert!(
        built.status.success(),
        "the overlapped lowering does not compile:\n{}",
        String::from_utf8_lossy(&built.stderr)
    );

    let run = std::process::Command::new(&binary)
        .current_dir(&dir)
        .output()
        .expect("run it");
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout).trim(), "5 6");
}

/// `ordering = "strict"` turns it off, and that is the whole of what it does.
///
/// ADR-033 D8: not an aid to be removed later. The same source, both ways, and
/// the strict one is the program this compiler emitted before any of this.
#[test]
fn strict_ordering_leaves_the_program_alone() {
    let strict = lowered(TWO_READS, Ordering::Strict);
    assert!(!strict.contains("std::thread::scope"), "{strict}");
    assert!(strict.contains("let a = match"), "{strict}");
    assert!(strict.contains("let b = match"), "{strict}");
}

// --- the shapes the analysis learned to see (ADR-033 §8.3, first item) -------

/// Two bare expression statements overlap.
///
/// The widening that matters most: `fs::write(…)` binds nothing, and §8.1's 101
/// refusals were mostly statements of exactly this shape falling out before any
/// `touches` set was consulted. Two writes to different files meet on nothing,
/// so neither waits for the other.
const TWO_WRITES: &str = "use std::fs\n\
     fn main() throws {\n\
         fs::write(\"drei.txt\", \"abc\") catch { }\n\
         fs::write(\"vier.txt\", \"defg\") catch { }\n\
         let a = fs::read_to_string(\"drei.txt\") catch { \"\".to_string() }\n\
         let b = fs::read_to_string(\"vier.txt\") catch { \"\".to_string() }\n\
         println(f\"{a.len()} {b.len()}\")\n\
     }";

#[test]
fn two_expression_statements_overlap() {
    assert!(
        overlaps(TWO_WRITES),
        "{}",
        lowered(TWO_WRITES, Ordering::Effects)
    );
}

/// … and a pair that binds nothing is lowered as a statement, not as a binding.
///
/// `let (_, _) = …` would be a pattern that says nothing, and the Rust this
/// emits is read by people.
#[test]
fn a_pair_that_binds_nothing_binds_nothing() {
    let rust = lowered(TWO_WRITES, Ordering::Effects);
    assert!(rust.contains("\n    task::both("), "{rust}");
    assert!(!rust.contains("let (_, _)"), "{rust}");
}

/// … and the program still writes both files and still prints what the
/// sequential one printed.
///
/// The half that cannot be checked by reading the output: an overlap that
/// produces plausible-looking Rust which does not build, or builds and does
/// something else, is worth nothing at all.
#[test]
fn the_overlapped_expression_statements_compile_and_run() {
    let dir = common::scratch_dir("ordering-expression-statements");
    let source = dir.join("two_writes.rs");
    std::fs::write(&source, lowered(TWO_WRITES, Ordering::Effects)).expect("write the Rust");

    let binary = dir.join("two_writes");
    let built = common::compile(&source, &["-o", &binary.to_string_lossy()]);
    assert!(
        built.status.success(),
        "the overlapped lowering does not compile:\n{}",
        String::from_utf8_lossy(&built.stderr)
    );

    let run = std::process::Command::new(&binary)
        .current_dir(&dir)
        .output()
        .expect("run it");
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    // What the sequential program prints, and what it leaves behind.
    assert_eq!(String::from_utf8_lossy(&run.stdout).trim(), "3 4");
    assert_eq!(
        std::fs::read_to_string(dir.join("drei.txt")).expect("drei.txt"),
        "abc"
    );
    assert_eq!(
        std::fs::read_to_string(dir.join("vier.txt")).expect("vier.txt"),
        "defg"
    );
}

/// A `let` whose initialiser is not a bare call is weighed, and its touch set
/// is the union of every call in it.
///
/// The other half of §8.3's first item. Two reads inside one statement reach
/// two files, and the statement next to them is compared against both.
#[test]
fn a_value_that_is_not_a_bare_call_is_weighed() {
    let why = report(
        "use std::fs\n\
         fn main() throws {\n\
             let beide = (fs::read_to_string(\"eins.txt\"), fs::read_to_string(\"zwei.txt\")) catch { (\"\".to_string(), \"\".to_string()) }\n\
             let c = fs::read_to_string(\"drei.txt\") catch { \"\".to_string() }\n\
             println(\"x\")\n\
         }",
    );
    assert!(
        why.contains("fs::read_to_string + fs::read_to_string / fs::read_to_string"),
        "{why}"
    );
    assert!(why.contains("together"), "{why}");

    // … and the union is what is compared: one of the two inner reads meets the
    // write next to it, so the pair keeps its order.
    let why = report(
        "use std::fs\n\
         fn main() throws {\n\
             let beide = (fs::read_to_string(\"eins.txt\"), fs::read_to_string(\"zwei.txt\")) catch { (\"\".to_string(), \"\".to_string()) }\n\
             fs::write(\"zwei.txt\", \"x\") catch { }\n\
             println(\"x\")\n\
         }",
    );
    assert!(why.contains("both reach file `zwei.txt`"), "{why}");
}

// --- and every reason two statements must keep their order --------------------

/// A data dependency: the second uses what the first bound.
#[test]
fn a_data_dependency_keeps_the_order() {
    assert!(!overlaps(
        "use std::fs\n\
         fn main() throws {\n\
             let a = fs::read_to_string(\"eins.txt\") catch { \"\".to_string() }\n\
             let b = fs::read_to_string(a) catch { \"\".to_string() }\n\
             println(f\"{b.len()}\")\n\
         }"
    ));
}

/// The same file, written by one of them.
#[test]
fn a_write_to_the_same_file_keeps_the_order() {
    assert!(!overlaps(
        "use std::fs\n\
         fn main() throws {\n\
             let a = fs::write(\"log.txt\", \"x\") catch { }\n\
             let b = fs::read_to_string(\"log.txt\") catch { \"\".to_string() }\n\
             println(f\"{b.len()}\")\n\
         }"
    ));
}

/// A file this compiler cannot name is every file of its kind (ADR-033 D4).
///
/// `fs::read_to_string(pfad)` where `pfad` is computed could be the file the
/// other one writes. The alternative to keeping the order is a program that is
/// right on some inputs and wrong on others.
#[test]
fn a_file_that_cannot_be_named_keeps_the_order() {
    assert!(!overlaps(
        "use std::fs\n\
         fn main(pfad: &str) throws {\n\
             let a = fs::write(\"log.txt\", \"x\") catch { }\n\
             let b = fs::read_to_string(pfad) catch { \"\".to_string() }\n\
             println(f\"{b.len()}\")\n\
         }"
    ));
}

/// A function nobody described touches everything.
///
/// `println` has no `touches` in `std.contracts`, so it orders against
/// everything - which is also what makes two `println`s keep their order, the
/// case any model like this has to get right without a special rule for it.
#[test]
fn a_function_with_no_contract_keeps_the_order() {
    assert!(!overlaps(
        "use std::fs\n\
         fn main() throws {\n\
             let a = fs::read_to_string(\"eins.txt\") catch { \"\".to_string() }\n\
             let b = etwas_unbekanntes() catch { \"\".to_string() }\n\
             println(f\"{a.len()}\")\n\
         }"
    ));
}

/// A handler that can leave the function makes the next statement conditional.
///
/// This is what building the increment found, and it was not in ADR-033 D5 when
/// it was written (ADR-034). If the first read fails and its handler `return`s,
/// the sequential program never performs the second read at all - so performing
/// it early is speculation, which D5 forbids.
#[test]
fn a_diverting_handler_keeps_the_order() {
    assert!(!overlaps(
        "use std::fs\n\
         fn main() throws {\n\
             let a = fs::read_to_string(\"eins.txt\") catch { return }\n\
             let b = fs::read_to_string(\"zwei.txt\") catch { \"\".to_string() }\n\
             println(f\"{a.len()} {b.len()}\")\n\
         }"
    ));
}

/// A data dependency inside a `catch` handler keeps the order too.
///
/// `f("a") catch { … }` is one expression, so a name its handler mentions is a
/// name the statement mentions. Looking only past the `catch` missed it, and
/// the lowering puts the handler in the same closure - so the second statement
/// would have read what the first had not finished binding.
#[test]
fn a_dependency_in_a_handler_keeps_the_order() {
    assert!(!overlaps(
        "use std::fs\n\
         fn main() throws {\n\
             let a = fs::read_to_string(\"eins.txt\") catch { \"\".to_string() }\n\
             let b = fs::read_to_string(\"zwei.txt\") catch { a }\n\
             println(f\"{a.len()} {b.len()}\")\n\
         }"
    ));

    // … including where the name is inside the handler's interpolated text,
    // whose holes this analysis has not parsed. Every word of the raw text
    // counts, which is over-approximate on purpose.
    assert!(!overlaps(
        "use std::fs\n\
         fn main() throws {\n\
             let a = fs::read_to_string(\"eins.txt\") catch { \"\".to_string() }\n\
             let b = fs::read_to_string(\"zwei.txt\") catch { f\"{a}\" }\n\
             println(f\"{a.len()} {b.len()}\")\n\
         }"
    ));
}

/// An argument that is not a literal is not sent to another thread.
///
/// The first increment's own restriction rather than the rule's: a closure that
/// captures nothing cannot capture something that must not cross a thread, and
/// what may cross one deserves its own decision.
#[test]
fn a_non_literal_argument_keeps_the_order() {
    assert!(!overlaps(
        "use std::fs\n\
         fn main(eins: &str, zwei: &str) throws {\n\
             let a = fs::read_to_string(eins) catch { \"\".to_string() }\n\
             let b = fs::read_to_string(zwei) catch { \"\".to_string() }\n\
             println(f\"{a.len()} {b.len()}\")\n\
         }"
    ));
}

/// Two `println`s keep their order, which is D6's own test of the model.
///
/// ADR-033 D6 asks for the obvious without an exception for it, and this is
/// where a wider analysis could have lost it: a bare `println(x)` is now a
/// shape the analysis reads, so the answer has to come from somewhere. It comes
/// from D4 - nothing in `std.contracts` says what `println` reaches, so it
/// reaches everything and orders against everything, `stdout` included.
#[test]
fn two_printlns_keep_their_order() {
    assert!(!overlaps(
        "fn main() {\n\
             println(\"a\")\n\
             println(\"b\")\n\
         }"
    ));
}

/// A failure nobody catches keeps the order (ADR-034 D2).
///
/// The same rule as a diverting handler, reached from the other side: if the
/// first write fails, the failure leaves the function and the sequential
/// program never performs the second. Starting it early would perform work the
/// program as written might never have performed - which is exactly what D5
/// forbids, and what a bare expression statement makes easy to write.
#[test]
fn an_uncaught_failure_keeps_the_order() {
    assert!(!overlaps(
        "use std::fs\n\
         fn main() throws {\n\
             fs::write(\"eins.txt\", \"a\")\n\
             fs::write(\"zwei.txt\", \"b\")\n\
             println(\"x\")\n\
         }"
    ));

    // … and the same statement with the failure caught does overlap, so the
    // test cannot pass because the shape was not read at all.
    assert!(overlaps(
        "use std::fs\n\
         fn main() throws {\n\
             fs::write(\"eins.txt\", \"a\") catch { }\n\
             fs::write(\"zwei.txt\", \"b\") catch { }\n\
             println(\"x\")\n\
         }"
    ));
}

/// An assignment is never an operation, however plain it looks.
///
/// Not an implementation gap but a rule: `x = 1` changes a name without binding
/// one, so the data-dependency test - does the later statement mention what the
/// earlier one bound - would find nothing to compare. A shape whose
/// dependencies the analysis cannot see is a shape it may not read.
#[test]
fn an_assignment_is_not_an_operation() {
    assert!(!overlaps(
        "use std::fs\n\
         fn main() throws {\n\
             let mut a = \"\".to_string()\n\
             a = fs::read_to_string(\"eins.txt\") catch { \"\".to_string() }\n\
             let b = fs::read_to_string(\"zwei.txt\") catch { \"\".to_string() }\n\
             println(f\"{a.len()} {b.len()}\")\n\
         }"
    ));
}

/// A method call keeps the order: which ledger entry it is depends on the type
/// of its receiver, and that is the type checker's answer (ADR-028).
#[test]
fn a_method_call_keeps_the_order() {
    assert!(!overlaps(
        "use std::fs\n\
         fn main() throws {\n\
             let mut out = Vec::new()\n\
             out.push(\"eins\")\n\
             let b = fs::read_to_string(\"zwei.txt\") catch { \"\".to_string() }\n\
             println(f\"{b.len()}\")\n\
         }"
    ));
}

/// A value-returning function overlaps the same way - its tail is the
/// expression, and the two `let`s before it are an ordinary pair.
///
/// The guard against pairing the tail itself is defensive rather than
/// observable: a block's last statement being a `let` means the block hands
/// back nothing, so a value-returning body cannot end in one. It is in the code
/// because a rule that holds by accident somewhere else is a rule that breaks
/// when the accident does.
#[test]
fn a_value_returning_body_overlaps_before_its_tail() {
    assert!(overlaps(
        "use std::fs\n\
         fn beides() -> i64 throws {\n\
             let a = fs::read_to_string(\"eins.txt\") catch { \"\".to_string() }\n\
             let b = fs::read_to_string(\"zwei.txt\") catch { \"\".to_string() }\n\
             a.len() + b.len()\n\
         }"
    ));
}

/// Every `.nika` in the repository lowers the same under both orderings, or
/// differs only by an overlap.
///
/// The corpus guard, in the form this decision needs: turning the analysis on
/// must not change a program into one that does not compile, and the cheapest
/// check of that is that the two lowerings agree wherever no pair was found.
#[test]
fn the_corpus_lowers_under_both_orderings() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut seen = 0;
    let mut overlapped = Vec::new();

    for dir in ["examples", "benches"] {
        let Ok(entries) = std::fs::read_dir(root.join(dir)) else {
            continue;
        };
        for entry in entries {
            let path = entry.expect("dir entry").path();
            if path.extension().and_then(|e| e.to_str()) != Some("nika") {
                continue;
            }
            let source = std::fs::read_to_string(&path).expect("read it");
            let Ok(parsed) = parse_to_ast(&source) else {
                continue;
            };
            let one = emit::emit_program_ordered(&parsed, Build::parallel(), Ordering::Effects);
            let other = emit::emit_program_ordered(&parsed, Build::parallel(), Ordering::Strict);
            match (one, other) {
                (Ok(one), Ok(other)) => {
                    seen += 1;
                    if one.rust != other.rust {
                        overlapped.push(path.file_name().unwrap().to_string_lossy().to_string());
                    }
                }
                // A file the bootstrap compiler cannot lower at all fails the
                // same way under both, which is not this test's business.
                (one, other) => assert_eq!(one.is_err(), other.is_err(), "{}", path.display()),
            }
        }
    }

    assert!(seen > 0, "no example was lowered");
    // Today: still none of them, and `--overlaps` now says why in terms of the
    // programs rather than of the analysis. The same 127 pairs are weighed; the
    // 101 that used to fall out on "not a `let` of a single call" are gone, and
    // what stands in their place is 30 method calls (whose ledger entry needs
    // the type checker), 29 statements that perform no operation at all, and 28
    // callees nobody has described - D4's fail-closed polarity, which doubled
    // from 14 once the shapes stopped hiding it (ADR-033 §8.1).
    //
    // So the zero is no longer a measurement of the analysis. It is a corpus of
    // microsecond work written in method calls, and §8.4 says the overlap could
    // not pay for it even if every one of them were accounted for.
    assert!(
        overlapped.is_empty(),
        "these examples now lower differently: {overlapped:?}"
    );
}

// --- why a pair did not overlap (ADR-033 D9) ---------------------------------

/// The report answers "may these two overlap", which is a question about the
/// program alone - whether a vehicle then exists to overlap them with is a
/// question about the build, and the CLI says so separately (ADR-033 §8.2b).
/// So no build setting reaches this.
fn report(source: &str) -> String {
    let parsed = parse_to_ast(source).expect("the source parses");
    let library = nikaia::contracts::Ledger::parse(nikaia::contracts::STD).expect("std's ledger");
    let own = nikaia::contracts::Ledger::infer(&parsed);
    nikaia::contracts::order::report(&parsed, &own, &library)
}

/// The refusal that the language decided **not** to give a keyword names its
/// own way out.
///
/// ADR-033 D9: Nikaia has no `allow_parallel`, on the argument that the one
/// case it would serve has a clearer spelling already. That argument only holds
/// if the compiler says so - a silent refusal with no way to ask would be
/// exactly the trap the keyword was supposed to be an escape from.
#[test]
fn a_diverting_handler_is_told_what_to_write_instead() {
    let why = report(
        "use std::fs\n\
         fn main() throws {\n\
             let a = fs::read_to_string(\"eins.txt\") catch { return }\n\
             let b = fs::read_to_string(\"zwei.txt\") catch { \"\".to_string() }\n\
             println(\"x\")\n\
         }",
    );
    assert!(why.contains("can leave the function"), "{why}");
    assert!(why.contains("hands back a value instead"), "{why}");
}

/// The report does not promise an overlap the emitter will not make.
///
/// A value-returning body ends in its value (Kap 3.1), and a pair hands back a
/// tuple - so the last statement is never half of one. Reading only `let`s hid
/// this, because a `let` cannot be a block's value; a bare expression statement
/// can, and a report that said "together" where nothing overlaps would be worse
/// than no report.
#[test]
fn the_value_of_a_function_is_never_half_a_pair() {
    let source = "use std::fs\n\
         fn zwei() -> String throws {\n\
             fs::write(\"eins.txt\", \"1\") catch { }\n\
             fs::read_to_string(\"zwei.txt\") catch { \"\".to_string() }\n\
         }";
    let why = report(source);
    assert!(why.contains("what this function hands back"), "{why}");
    assert!(!why.contains("together"), "{why}");
    assert!(!overlaps(source), "{}", lowered(source, Ordering::Effects));
}

/// A resource collision names the resource.
#[test]
fn a_collision_names_the_file_it_is_about() {
    let why = report(
        "use std::fs\n\
         fn main() throws {\n\
             let a = fs::write(\"log.txt\", \"x\") catch { }\n\
             let b = fs::read_to_string(\"log.txt\") catch { \"\".to_string() }\n\
             println(\"x\")\n\
         }",
    );
    assert!(why.contains("both reach file `log.txt`"), "{why}");
    assert!(why.contains("one writes it"), "{why}");
}

/// A function nobody described says that, rather than "not accounted for".
#[test]
fn an_undescribed_call_is_named() {
    let why = report(
        "fn main() throws {\n\
             let a = unbekannt() catch { \"\".to_string() }\n\
             let b = auch_unbekannt() catch { \"\".to_string() }\n\
             println(\"x\")\n\
         }",
    );
    assert!(
        why.contains("nothing says what `unbekannt` reaches"),
        "{why}"
    );
}

/// A pair that does overlap says so, so the report is readable as a whole and
/// not only as a list of complaints.
#[test]
fn a_pair_that_overlaps_is_reported_too() {
    let why = report(TWO_READS);
    assert!(why.contains("together"), "{why}");
    assert!(why.contains("they meet on nothing"), "{why}");
}

/// The rewrite the refusal recommends actually works.
///
/// The whole argument against `allow_parallel` is that `catch { return }`
/// conflates two things - what to do about the failure, and whether to go on -
/// and that separating them is clearer *and* overlaps. If the rewrite did not
/// overlap, the advice would be wrong and the keyword would be needed.
#[test]
fn the_recommended_rewrite_overlaps() {
    assert!(overlaps(
        "use std::fs\n\
         fn main() throws {\n\
             let a = fs::read_to_string(\"eins.txt\") catch { \"\".to_string() }\n\
             let b = fs::read_to_string(\"zwei.txt\") catch { \"\".to_string() }\n\
             if a.is_empty() { return }\n\
             println(\"{a.len()} {b.len()}\")\n\
         }"
    ));
}
