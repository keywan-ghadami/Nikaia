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

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use nikaia::check::{self, Finding, Severity};
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

/// The one crossing a source is written to produce, rendered the way a user
/// sees it.
fn one(source: &str) -> (String, String) {
    let found = crossings(source);
    assert_eq!(
        found.len(),
        1,
        "expected exactly one crossing, got {:#?}",
        found.iter().map(|f| &f.message).collect::<Vec<_>>()
    );
    (
        found[0].code.to_string(),
        nikaia::diagnostics::render_finding(&found[0], "app.nika", source),
    )
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

/// `Shared` may not cross into a task, and the message says so in Nikaia words.
///
/// `Shared[T]` is a count of the value's owners, and at `user_parallelism = no`
/// it is a count only one thread may touch ([ADR-037](adr-037.md) D3). Part III
/// C.2 asks for a headline, the reason, and one paste-ready way out; all three
/// are asserted here, because the catalogue *is* the test suite (C.1).
#[test]
fn a_shared_may_not_cross_into_a_task() {
    let (code, rendered) = one("fn zaehle(counts: Shared[Vec[i64]]) {\n\
             spawn({ println(f\"{counts.len()}\") })\n\
         }");
    assert_eq!(code, "NK2501");
    assert!(
        rendered.starts_with(
            "error[NK2501]: `counts` may not cross into a task, and this task uses it\n"
        ),
        "{rendered}"
    );
    assert!(rendered.contains("app.nika:2:"), "{rendered}");
    assert!(rendered.contains('^'), "a caret, as C.2 asks: {rendered}");
    assert!(
        rendered.contains("`Shared[Vec[i64]]` counts its owners"),
        "{rendered}"
    );
    assert!(
        rendered.contains("help: write `Vec[i64]` where the value crosses"),
        "the help is paste-ready: {rendered}"
    );
    // C.2's first requirement: no Rust vocabulary anywhere in it.
    for word in ["Rc", "Arc", "Send", "E0277", "lifetime", "borrow"] {
        assert!(!rendered.contains(word), "`{word}` in: {rendered}");
    }
}

/// The rule is **structural and transitive**, which is what Group B asks for: a
/// struct with one `Shared` field is no more crossable than the field.
#[test]
fn a_struct_that_holds_a_shared_may_not_cross_into_a_task() {
    let (code, rendered) = one("struct Tally { hits: Shared[i64] }\n\
         fn zaehle(t: Tally) {\n\
             spawn({ println(f\"{t.hits}\") })\n\
         }");
    assert_eq!(code, "NK2501");
    assert!(
        rendered.contains("its field `hits`"),
        "the note names the field that decided it: {rendered}"
    );
}

/// ADR-038 D7's first rule: a value handed to a call this compiler cannot see
/// the end of may reach a thread that call owns.
///
/// This is the `examples/foreign-runtime/crossing` shape, which the experiment
/// measured as a raw `E0277` against a generated file.
#[test]
fn a_shared_may_not_cross_into_a_call_this_compiler_cannot_see() {
    let (code, rendered) = one("fn ueber(handle: Shared[String]) {\n\
             fremd::auf_einen_thread(handle)\n\
         }");
    assert_eq!(code, "NK2502");
    assert!(
        rendered.starts_with(
            "error[NK2502]: `handle` may not cross a thread, and \
             `fremd::auf_einen_thread` may put it on one\n"
        ),
        "{rendered}"
    );
    assert!(
        rendered.contains("nothing written down describes `fremd::auf_einen_thread`"),
        "{rendered}"
    );
    assert!(
        rendered.contains("Part III, 15.2"),
        "and points at the rule it is about: {rendered}"
    );
    assert!(rendered.contains("app.nika:2:"), "{rendered}");
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

/// **One verdict, two severities.** The check runs at both settings of
/// `user_parallelism` and says the same thing; what the switch decides is
/// whether *this* build performs the crossing.
///
/// At `no` nothing the program wrote runs concurrently (ADR-037 D2), so the
/// emitter writes no task and the crossing does not happen - refusing would
/// refuse a program that compiles. At `yes` it does happen. Part III C.3 asked
/// for exactly this: "reported at `user_parallelism = no` as a lint, so a
/// library built there stays usable at `yes`".
///
/// Accepted at `yes` and refused at `no` - or silent at `no` and refused at
/// `yes` - is the failure Group B was written to prevent, and this is the test
/// that neither has happened.
#[test]
fn a_task_crossing_is_a_lint_at_no_and_an_error_at_yes() {
    let source = "fn zaehle(counts: Shared[Vec[i64]]) {\n\
                      spawn({ println(f\"{counts.len()}\") })\n\
                  }";

    // The verdict itself does not move: the analysis never sees a switch.
    let found = crossings(source);
    assert_eq!(found.len(), 1, "{found:#?}");
    assert_eq!(found[0].severity, Severity::Error);

    assert_eq!(
        refused_at(source, "no"),
        None,
        "at `no` the crossing does not happen, so it is a lint and the build goes on"
    );
    let refused = refused_at(source, "yes").expect("at `yes` it happens, so it is refused");
    assert_eq!(refused, "1 value that may not cross a thread");
}

/// A crossing into a **foreign** thread is refused at both settings, and that
/// is the decision rather than an oversight.
///
/// `user_parallelism` bounds what the *program* runs at once - ADR-037 D2's
/// load-bearing "user". A Rust dependency's own runtime is not the program's, so
/// the crossing is real at `no` too, and so is the refusal (ADR-038 D7).
#[test]
fn a_foreign_crossing_is_refused_at_both_settings() {
    let source = "fn ueber(handle: Shared[String]) {\n\
                      fremd::auf_einen_thread(handle)\n\
                  }";
    for setting in ["no", "yes"] {
        let refused = refused_at(source, setting)
            .unwrap_or_else(|| panic!("a foreign crossing is refused at `{setting}` too"));
        assert_eq!(refused, "1 value that may not cross a thread");
    }
}

// --- the crossing the compiler chooses for itself ---------------------------

/// A ledger with one crossable result and one that may not cross, so that the
/// overlapping analysis can be asked about both.
///
/// Hand-written rather than `std`'s, because nothing in `std` returns a value
/// that may not cross a thread - which is the whole reason this check could be
/// absent for so long without anything being unsound (`docs/foreign-runtime.md`
/// §3.2).
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
         signature = \"() -> Shared[i64]\"\n",
    )
    .expect("the probe ledger parses")
}

fn report_against_probe(source: &str) -> String {
    let parsed = parse_to_ast(source).expect("the source parses");
    let own = Ledger::infer(&parsed);
    order::report(&parsed, &own, &probe_library())
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

/// … and a pair whose result may not cross a thread keeps the order it was
/// written in, with no diagnostic at all.
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
