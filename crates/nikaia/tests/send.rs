//! The structural `Send` check - `NK2501` and `NK2502`.
//!
//! [ADR-005](../../../docs/specification/adr/adr-005.md) §1 Group B decides it:
//! "`Send`-ness checked **structurally in the frontend** at *every* setting of
//! `user_parallelism`, so a library written at [`no`] cannot turn out
//! un-compilable at [`yes`]".
//! [ADR-037](../../../docs/specification/adr/adr-037.md) §3 and
//! [ADR-038](../../../docs/specification/adr/adr-038.md) D7 both rely on it, and
//! the experiment in `docs/foreign-runtime.md` is what found it missing.
//!
//! **Two halves, and the first decides whether the tool is worth running.** A
//! check that refuses a correct program is worse than no check, so the corpus is
//! the guard and the deliberately-wrong programs are the proof that the guard is
//! not passing because the check is asleep. The type checker's own corpus guard
//! (`typecheck.rs`) covers these codes too, because they are its findings; what
//! is here is the cases a corpus cannot contain, because nothing in the
//! repository writes `Shared` - `Shared` is unbuilt, and this check is what has
//! to exist before it lands.
//!
//! **Since [ADR-037](../../../docs/specification/adr/adr-037.md) D6 the second
//! half has no input, and that is stated rather than worked around.** `Shared`
//! was the one type `contracts::send` answered `may not` about; D6 gives it one
//! representation at both settings, so it is answered by what it holds and no
//! type in the language is refused a crossing. So the programs below that used
//! to produce `NK2501` and `NK2502` now assert **silence**, the walk's own tests
//! in `contracts::send` keep the refusal arm and its sentences whole, and the
//! two codes stay built for the next type whose expansion moves with the switch.
//! What is *not* here any more is an end-to-end rendering of either code, and
//! there is no honest way to write one: a refusal can only come from a type the
//! records name, and they name none.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use nikaia::check::{self, Finding};
use nikaia::contracts::order;
use nikaia::contracts::send::{self, Crossing};
use nikaia::contracts::{ty::Ty, Ledger, STD};
use nikaia::parser::parse_to_ast;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn findings(source: &str) -> Vec<Finding> {
    let parsed = parse_to_ast(source).expect("the source parses");
    let own = Ledger::infer(&parsed);
    let library = Ledger::parse(STD).expect("std's shipped ledger parses");
    check::check(&parsed, &own, &library).findings
}

/// Only what this check says, so that an unrelated finding cannot make a test
/// pass for the wrong reason.
fn crossings(source: &str) -> Vec<Finding> {
    findings(source)
        .into_iter()
        .filter(|f| f.code.starts_with("NK25"))
        .collect()
}

/// What `project::check` does with a program, which is where the build switch
/// meets the severity and nothing else.
fn refused_at(source: &str, user_parallelism: &str) -> Option<String> {
    let parsed = parse_to_ast(source).expect("the source parses");
    let own = Ledger::infer(&parsed);
    nikaia::project::check(
        &parsed,
        &own,
        &BTreeSet::new(),
        Path::new("app.nika"),
        source,
        user_parallelism,
    )
    .err()
    .map(|error| format!("{error:#}"))
}

// --- the guard ---------------------------------------------------------------

/// No program in the repository has a crossing refused.
///
/// Part III C.4's property, for this check: it never rejects a program that is
/// correct. Every `.nika` there is, including the ones in subdirectories that
/// the type checker's own guard does not walk - `examples/inventory`, and the
/// three projects of the ADR-038 D7 experiment, which are the programs that
/// actually hand values to a foreign runtime.
#[test]
fn no_program_in_the_repository_has_a_crossing_refused() {
    let library = Ledger::parse(STD).expect("std's shipped ledger parses");
    let mut checked = 0;
    let mut reported = String::new();

    for path in every_program(&repo_root()) {
        let source = std::fs::read_to_string(&path).expect("read the program");
        let Ok(parsed) = parse_to_ast(&source) else {
            // A probe that is not meant to parse is not this test's business.
            continue;
        };
        let own = Ledger::infer(&parsed);
        let name = path.display().to_string();
        for finding in check::check(&parsed, &own, &library)
            .findings
            .iter()
            .filter(|f| f.code.starts_with("NK25"))
        {
            reported.push_str(&nikaia::diagnostics::render_finding(
                finding, &name, &source,
            ));
        }
        checked += 1;
    }

    assert!(checked >= 12, "only {checked} programs were checked");
    assert!(
        reported.is_empty(),
        "a program in the repository was refused a crossing:\n{reported}"
    );
}

fn every_program(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut pending = vec![root.join("examples"), root.join("crates/nikaia-std/src")];
    while let Some(dir) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().and_then(|e| e.to_str()) == Some("nika") {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

/// An ordinary value crosses into a task, and nothing is said about it.
///
/// The other half of the guard, in the smallest program that shows it: the
/// corpus has no `spawn` at all, because Part II 11.2's runtime integration is
/// the next roadmap line - so without this, "the corpus is clean" would also be
/// true of a check that never looked at a task.
#[test]
fn an_ordinary_value_crosses_into_a_task() {
    let clean = crossings(
        "struct Reading { name: String, temp: f64 }\n\
         fn report(text: String, counts: Vec[i64], r: Reading) {\n\
             spawn({ println(f\"{text} {counts.len()} {r.temp}\") })\n\
         }",
    );
    assert!(clean.is_empty(), "{clean:#?}");
}

/// A value whose type nothing says is **not refused**, which is the one thing
/// this check may never get wrong.
///
/// Stage 0 knows the type of rather less than half of what a program writes, so
/// a check that refused what it could not decide would refuse most correct
/// programs. `Undecided` is not accepted either - `rustc` still type-checks the
/// emitted crate and ADR-005 D7's `E0277` translation reports that refusal
/// against the `.nika` line - but it is not refused *here*.
#[test]
fn a_value_whose_type_is_not_written_down_is_not_refused() {
    let clean = crossings(
        "fn report(path: String) throws {\n\
             let handle = fs::map(path) catch { return }\n\
             spawn({ println(f\"{handle.len()}\") })\n\
         }",
    );
    assert!(clean.is_empty(), "{clean:#?}");
}

// --- what it catches --------------------------------------------------------

/// A `Shared` crosses into a task, and nothing is said about it - **ADR-037 D6**.
///
/// This program was `NK2501`'s own example until the count stopped moving with
/// the switch. `Shared[Vec[i64]]` is answered by `Vec[i64]` now, which may
/// cross, so the task may take it. The verdict is still a property of the type
/// and still never reads `user_parallelism`; what moved is one row of
/// `contracts::send`'s table.
#[test]
fn a_shared_crosses_into_a_task() {
    let clean = crossings(
        "fn zaehle(counts: Shared[Vec[i64]]) {\n\
             spawn({ println(f\"{counts.len()}\") })\n\
         }",
    );
    assert!(clean.is_empty(), "{clean:#?}");
}

/// The rule is still **structural and transitive**, which is what Group B asks
/// for: a struct is exactly as crossable as its fields, in both directions.
#[test]
fn a_struct_that_holds_a_shared_crosses_into_a_task() {
    let clean = crossings(
        "struct Tally { hits: Shared[i64] }\n\
         fn zaehle(t: Tally) {\n\
             spawn({ println(f\"{t.hits}\") })\n\
         }",
    );
    assert!(clean.is_empty(), "{clean:#?}");
}

/// …and a `Shared` of something nothing describes is **undecided**, which is not
/// permission and is also not a refusal.
///
/// Part II 12.2's counter is this program. `Locked` is what decides it - its
/// representation is what ADR-037 D3's second half is about, and D6 does not
/// touch it - so nothing here is refused and nothing here is waved through:
/// `rustc` type-checks the emitted crate and ADR-005 D7's translation reports
/// its answer against this `.nika` line.
#[test]
fn a_shared_of_something_undescribed_is_not_refused_and_not_permitted() {
    let clean = crossings(
        "fn zaehle(counter: Shared[Locked[i32]]) {\n\
             spawn({ println(f\"{counter}\") })\n\
         }",
    );
    assert!(clean.is_empty(), "not refused: {clean:#?}");

    let parsed =
        parse_to_ast("fn zaehle(counter: Shared[Locked[i32]]) { }").expect("the source parses");
    let own = Ledger::infer(&parsed);
    let library = Ledger::parse(STD).expect("std's shipped ledger parses");
    let answer = send::crossing(&Ty::parse("Shared[Locked[i32]]"), &own, &library);
    assert!(
        matches!(answer, Crossing::Undecided { .. }),
        "and not permitted either: {answer:?}"
    );
}

/// ADR-038 D7's first rule still has no `Shared` to refuse, so the foreign
/// crossing of the `examples/foreign-runtime/crossing` shape is silent too.
///
/// The rule is untouched - a call this compiler cannot see the end of may put
/// what it is given on a thread of its own - and `contracts::sharing` is where
/// that now costs something: the value's count stays atomic rather than the
/// program being refused. `docs/rc-or-arc.md` §5.3 is the polarity, and
/// `tests/sharing.rs` is where it is asserted.
#[test]
fn a_shared_crosses_into_a_call_this_compiler_cannot_see() {
    let clean = crossings(
        "fn ueber(handle: Shared[String]) {\n\
             fremd::auf_einen_thread(handle)\n\
         }",
    );
    assert!(clean.is_empty(), "{clean:#?}");
}

/// A call into a *described* function is not this, however unpleasant its
/// arguments.
///
/// The rule is about a body this compiler cannot see, so a `std` function and a
/// function of this program are both outside it - their bodies are accounted
/// for, and what they do with what they are given is the same question asked
/// inside them.
#[test]
fn a_call_this_compiler_can_see_is_not_a_crossing() {
    let clean = crossings(
        "fn halte(s: Shared[String]) { }\n\
         fn ueber(handle: Shared[String]) { halte(handle) }",
    );
    assert!(clean.is_empty(), "{clean:#?}");
}

// --- the asymmetry Group B exists to prevent --------------------------------

/// **One verdict at both settings**, which is the property ADR-037 §5 asks this
/// file to assert - and it is asserted by comparing verdicts rather than exit
/// codes, because that is the only comparison that catches the failure.
///
/// Accepted at `yes` and refused at `no` - or silent at `no` and refused at
/// `yes` - is what Group B was written to prevent. Since ADR-037 D6 the
/// `Shared` programs are accepted at both, so the property holds from the other
/// side: the same source, the same verdict, and the build goes on either way.
///
/// **The severity split is what has no input now.** `lint_where_nothing_crosses`
/// downgrades `NK2501` at `no` and leaves `NK2502` alone, and both arms are
/// still there for the next type whose expansion moves with the switch - which
/// is exactly why D6 removed the cause and not the mechanism.
#[test]
fn the_verdict_is_the_same_at_both_settings() {
    for source in [
        "fn zaehle(counts: Shared[Vec[i64]]) {\n\
             spawn({ println(f\"{counts.len()}\") })\n\
         }",
        "fn ueber(handle: Shared[String]) {\n\
             fremd::auf_einen_thread(handle)\n\
         }",
        "fn zaehle(counter: Shared[Locked[i32]]) {\n\
             spawn({ println(f\"{counter}\") })\n\
         }",
    ] {
        // The analysis never sees a switch, so there is one verdict to compare.
        let found = crossings(source);
        assert!(found.is_empty(), "{found:#?}");
        assert_eq!(
            refused_at(source, "no"),
            refused_at(source, "yes"),
            "{source}"
        );
        assert_eq!(refused_at(source, "no"), None, "{source}");
    }
}

/// Part II 12.2's counter, at both settings, which is the program D3's open
/// question was wanted for.
///
/// `Shared` no longer stands in front of it: the count is atomic at both
/// settings, so the handle may cross. What is left is the **lock**, and this
/// test says exactly what the compiler says about it today - nothing, because
/// nothing written down describes `Locked`. Which of `RefCell` and `Mutex` it
/// expands to is ADR-037 D3's second half and is not decided here, so this
/// asserts the silence rather than a verdict about the lock.
#[test]
fn the_shared_half_of_part_ii_12_2s_counter_no_longer_refuses() {
    let source = "fn zaehle(counter: Shared[Locked[i32]]) {\n\
                      counter.access(fn(a) { a })\n\
                      spawn({ println(f\"{counter}\") })\n\
                  }";
    for setting in ["no", "yes"] {
        assert_eq!(refused_at(source, setting), None, "at `{setting}`");
    }
    assert!(crossings(source).is_empty());
}

// --- the crossing the compiler chooses for itself ---------------------------

/// A ledger with one crossable result and one the compiler cannot decide about,
/// so that the overlapping analysis can be asked about both.
///
/// Hand-written rather than `std`'s, because nothing in `std` returns a value
/// that may not cross a thread - which is the whole reason this check could be
/// absent for so long without anything being unsound (`docs/foreign-runtime.md`
/// §3.2).
///
/// **`probe::held` hands back `Shared[Locked[i64]]` rather than `Shared[i64]`
/// since ADR-037 D6.** The overlapping analysis refuses anything but `May`, so
/// what it needs here is a non-`May` answer and not specifically a refusal - and
/// since D6 a `Shared` of plain data *is* `May`, which is a change to what this
/// analysis lets through and is asserted below.
fn probe_library() -> Ledger {
    Ledger::parse(
        "version = 2\n\
         toolchain = \"probe\"\n\
         inference = \"probe\"\n\
         \n\
         [fn.\"probe::plain\"]\n\
         pub = true\n\
         sync = true\n\
         touches = []\n\
         signature = \"() -> i64\"\n\
         \n\
         [fn.\"probe::held\"]\n\
         pub = true\n\
         sync = true\n\
         touches = []\n\
         signature = \"() -> Shared[Locked[i64]]\"\n\
         \n\
         [fn.\"probe::counted\"]\n\
         pub = true\n\
         sync = true\n\
         touches = []\n\
         signature = \"() -> Shared[i64]\"\n",
    )
    .expect("the probe ledger parses")
}

fn report_against_probe(source: &str) -> String {
    let parsed = parse_to_ast(source).expect("the source parses");
    let own = Ledger::infer(&parsed);
    // Every vehicle available: this file asks what the *program* allows, and a
    // build caveat per pair would be noise in a test about crossing a thread.
    order::report(&parsed, &own, &probe_library(), &|_| None)
}

/// Two operations that meet on nothing overlap - the control, without which the
/// test below would also pass against an analysis that refuses everything.
#[test]
fn two_crossable_operations_still_overlap() {
    let report = report_against_probe(
        "fn main() {\n\
             let a = probe::plain()\n\
             let b = probe::plain()\n\
             println(f\"{a} {b}\")\n\
         }",
    );
    assert!(
        report.contains("together  probe::plain / probe::plain"),
        "{report}"
    );
}

/// A pair whose result is a `Shared` of plain data **does** overlap since
/// ADR-037 D6 - and that is the one thing step 1 changes about the overlapping.
///
/// It is also where the floor in `contracts::sharing` earns its keep. An
/// overlapped operation runs in a closure somewhere else and hands its value
/// back, so this `Shared`'s count is touched on two threads - which is why the
/// result of a call is a seed there and may never be lowered to a plain count.
#[test]
fn a_pair_whose_result_is_a_shared_of_plain_data_overlaps() {
    let report = report_against_probe(
        "fn main() {\n\
             let a = probe::counted()\n\
             let b = probe::counted()\n\
             println(f\"{a} {b}\")\n\
         }",
    );
    assert!(
        report.contains("together  probe::counted / probe::counted"),
        "{report}"
    );
}

/// … and a pair whose result may not cross a thread, or nothing says may, keeps
/// the order it was written in, with no diagnostic at all.
///
/// This is the crossing the **compiler** chose: overlapping puts each statement
/// in a closure that runs elsewhere and hands its value back, so the result
/// crosses a thread. Not overlapping is a step the compiler was never obliged to
/// take, so fail-closed here costs speed and never a refusal - which is ADR-033
/// D4's own polarity, applied to the crossing question.
#[test]
fn an_operation_whose_result_may_not_cross_keeps_its_place() {
    let source = "fn main() {\n\
                      let a = probe::held()\n\
                      let b = probe::held()\n\
                      println(f\"{a} {b}\")\n\
                  }";
    let report = report_against_probe(source);
    assert!(!report.contains("together"), "{report}");
    assert!(
        report.contains("would have to cross a thread"),
        "and the report says which refusal it was (ADR-033 D9): {report}"
    );

    // And it is not a refusal: the program compiles, it simply does not overlap.
    assert!(crossings(source).is_empty(), "{:#?}", crossings(source));
}

/// A result nothing is written down about keeps its place too, and the way to
/// buy the overlap back is the one `touches` already has: write it down.
///
/// `cli::args` is the measured case. Its result is a Rust type whose parts this
/// compiler cannot walk, so the answer was "nobody said" - which is not
/// permission (ADR-010 D1) - and the ten pairs ADR-033 §8.1 bought with a
/// `touches` line would have gone again. The `crosses = true` on
/// `[type."cli::Args"]` is what keeps them.
#[test]
fn a_library_type_says_it_may_cross_and_the_overlap_stays() {
    let library = Ledger::parse(STD).expect("std's shipped ledger parses");
    let args = library
        .types
        .get("cli::Args")
        .expect("`cli::Args` is described");
    assert!(
        args.crosses,
        "the claim is what keeps `cli::args` overlapping"
    );
    assert_eq!(
        send::crossing(&Ty::named("Args"), &Ledger::empty(), &library),
        Crossing::May
    );

    // And without the claim it would be undecided rather than permitted.
    let silent = Ledger::parse(
        "version = 2\n\
         toolchain = \"probe\"\n\
         inference = \"probe\"\n\
         \n\
         [type.\"cli::Args\"]\n\
         pub = true\n",
    )
    .expect("the probe ledger parses");
    assert!(matches!(
        send::crossing(&Ty::named("Args"), &Ledger::empty(), &silent),
        Crossing::Undecided { .. }
    ));
}
