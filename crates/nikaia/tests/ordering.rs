//! Order is kept where it can be seen (ADR-033, Part I 8.1.1).
//!
//! The first increment: two adjacent `let`s whose calls reach different files
//! are lowered to run at the same time. Everything here is either that working,
//! or one of the reasons it must not - and the second list is the longer one on
//! purpose, because the decision is only safe if every "no" is reliable.
//!
//! The lowering is not taken on trust: the emitted Rust is compiled and run,
//! and the program prints what the sequential one would have printed.

mod common;

use std::path::PathBuf;

use nikaia::emit::{self, Ordering, Profile};
use nikaia::parser::parse_to_ast;

fn lowered(source: &str, ordering: Ordering) -> String {
    let parsed = parse_to_ast(source).expect("the source parses");
    emit::emit_program_ordered(&parsed, Profile::Advanced, ordering)
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

/// The Lite profile has no threads, so it does not get the overlap either.
///
/// Part I 11.6 calls Lite *"a strict single-threaded model for user logic"*,
/// and Part III 19.4 promises a `wasm32-unknown` build with no OS-level
/// mutexes or atomics - a target where `std::thread::scope` does not even
/// link. `--ordering effects` is a question about the program; whether a
/// thread exists to answer it with is a question about the build, and Lite
/// answers that one no. So this degrades exactly as `par_fold` degrades to a
/// sequential `fold` under Lite (ADR-009), rather than quietly contradicting
/// the profile in the same file that emits `Parallelism::Off`.
#[test]
fn the_lite_profile_never_spawns_a_thread() {
    let parsed = parse_to_ast(TWO_READS).expect("the source parses");
    let lite = emit::emit_program_ordered(&parsed, Profile::Lite, Ordering::Effects)
        .expect("the source lowers")
        .rust;
    assert!(
        !lite.contains("task::both"),
        "Lite overlapped under `--ordering effects`:\n{lite}"
    );

    // …and it is the sequential program, not merely a different one.
    let strict = emit::emit_program_ordered(&parsed, Profile::Lite, Ordering::Strict)
        .expect("the source lowers")
        .rust;
    assert_eq!(lite, strict, "Lite's two orderings differ");

    // The guard has to be the profile and not the analysis: Advanced still
    // overlaps the same program, or this test would pass for the wrong reason.
    assert!(overlaps(TWO_READS));
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
            let one = emit::emit_program_ordered(&parsed, Profile::Advanced, Ordering::Effects);
            let other = emit::emit_program_ordered(&parsed, Profile::Advanced, Ordering::Strict);
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
    // Today: none of them. `--overlaps` says why, and the answer is not the
    // one this comment used to give: of 127 refused pairs, 101 fall out on
    // "not a `let` of a single call" - before any `touches` set is consulted.
    // So the zero measures how narrow the analysis is, not how sequential the
    // corpus is (ADR-033 §8.1). Worth having written down rather than
    // rediscovered: widening the shapes comes before any conclusion about
    // whether `ordering = "effects"` earns being the default.
    assert!(
        overlapped.is_empty(),
        "these examples now lower differently: {overlapped:?}"
    );
}

// --- why a pair did not overlap (ADR-033 D9) ---------------------------------

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
