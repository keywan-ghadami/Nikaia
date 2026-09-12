//! The ADR-038 D7 experiment, kept re-runnable.
//!
//! D7 says a Rust crate may bring its own runtime and its own threads, and
//! names two rules that keep that sound. `examples/foreign-runtime/` is the
//! program that tests them: a Nikaia project depending on a shim crate over
//! `hyper` and `tokio`, through ADR-002 D1's crates.io passthrough. What the
//! experiment found is written up in `docs/foreign-runtime.md`.
//!
//! **One test here runs on every `cargo test`** - the one that needs neither
//! Cargo nor the network, and which is the one that checks a guarantee rather
//! than a finding: a call into a foreign crate has no ledger entry, so ADR-033
//! D4 makes it order against everything.
//!
//! **The rest are `#[ignore]`d**, because each one runs `cargo`, and the first
//! run fetches `hyper`, `tokio` and twenty-four other crates from crates.io.
//! Run them with:
//!
//! ```text
//! cargo test --test foreign_runtime -- --ignored --nocapture
//! ```

use std::path::{Path, PathBuf};
use std::process::Command;

use nikaia::contracts::{order, Ledger, STD};
use nikaia::emit::{self, Build, Ordering};
use nikaia::parser::parse_to_ast;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn experiment() -> PathBuf {
    repo_root().join("examples/foreign-runtime")
}

/// One `CARGO_TARGET_DIR` for all three projects, so `hyper` is built once.
fn shared_target_dir() -> PathBuf {
    repo_root().join("target").join("nikaia-foreign-runtime")
}

fn nikaia(subcommand: &str, project: &Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_nikaia"))
        .args([subcommand, "--project"])
        .arg(project)
        .env("CARGO_TARGET_DIR", shared_target_dir())
        .output()
        .expect("the nikaia binary runs")
}

fn said(output: &std::process::Output) -> String {
    format!(
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

// ---------------------------------------------------------------------------
// D7's third paragraph: what needs no rule.
// ---------------------------------------------------------------------------

/// A foreign crate has no ledger entry, so `touches` is absent, so ADR-033 D4
/// makes the call reach everything and order against everything.
///
/// This is the one claim in D7 that is a *guarantee* rather than a finding, so
/// it is the one asserted on every build. It needs no network: the ordering
/// analysis is a question about the program, and the answer does not depend on
/// the foreign crate existing.
#[test]
fn a_call_into_a_foreign_crate_orders_against_everything() {
    let path = experiment().join("overlaps.nika");
    let source = std::fs::read_to_string(&path).expect("the probe is in the repository");
    let parsed = parse_to_ast(&source).expect("it parses");

    let library = Ledger::parse(STD).expect("std ships a ledger");
    let own = Ledger::infer(&parsed);
    // A build with every vehicle in it, because the claim under test is about
    // the program: which switch gates which vehicle is ADR-033 D10's business
    // and not this record's.
    let report = order::report(&parsed, &own, &library, &|_| None);

    // The control: two reads of two files meet on nothing and run together.
    // Without it, "everything is in order" would also be true of a build that
    // had overlapping switched off, and the test would prove nothing.
    assert!(
        report.contains("together  fs::read_to_string / fs::read_to_string"),
        "the control pair must overlap, or this test cannot tell D4 from a \
         disabled analysis:\n{report}"
    );

    // Every pair with a foreign call in it, and the reason must be D4's.
    for line in report.lines() {
        if line.contains("hyper_shim::serve_once") {
            assert!(
                line.trim_start().starts_with("in order"),
                "a pair with a foreign call in it overlapped:\n{report}"
            );
            assert!(
                line.contains("reaches everything"),
                "and the reason has to be the absent `touches`, not something \
                 that happens to coincide with it:\n{report}"
            );
        }
    }
    assert!(
        report.matches("hyper_shim::serve_once").count() >= 4,
        "the probe is supposed to put a foreign call in four pairs:\n{report}"
    );

    // And the emitter agrees with the report: exactly the control overlaps. The
    // control is two reads, so the vehicle is the runtime's completion pair and
    // not `task::both` (ADR-033 D10) - which also means this program overlaps at
    // `user_parallelism = no`, where it did not before.
    for build in [Build::parallel(), Build::default()] {
        let rust = emit::emit_program_ordered(&parsed, build, Ordering::Effects)
            .expect("it lowers")
            .rust;
        assert_eq!(
            rust.matches("task::read_pair(").count(),
            1,
            "one overlap in the emitted Rust, and it is the control's ({build:?}):\n{rust}"
        );
        assert!(
            !rust.contains("task::both"),
            "two reads need no thread of the program's ({build:?}):\n{rust}"
        );
        let control = rust
            .split_once("fn control()")
            .expect("the control function is emitted")
            .1;
        let control = control
            .split_once("\nfn ")
            .map_or(control, |(body, _)| body);
        assert!(
            control.contains("task::read_pair("),
            "the one overlap is inside `control` ({build:?}):\n{rust}"
        );
    }
}

// ---------------------------------------------------------------------------
// D7's first paragraph: a crate may bring its own runtime.
// ---------------------------------------------------------------------------

/// Question 1: a Nikaia program starts a Rust HTTP server and serves one
/// request, and a value that may cross any thread crosses into one the foreign
/// runtime owns.
///
/// Needs the network on the first run, and `cargo`.
#[test]
#[ignore = "runs cargo and fetches hyper and tokio from crates.io"]
fn a_nikaia_program_serves_one_request_through_hyper() {
    let run = nikaia("run", &experiment().join("serve"));
    assert!(run.status.success(), "{}", said(&run));

    let out = String::from_utf8_lossy(&run.stdout);
    assert!(
        out.contains("served: GET /hello"),
        "hyper parsed the request and the shim handed the body back: {}",
        said(&run)
    );
    assert!(
        out.contains("crossed: a String is Send on tokio-"),
        "and a Nikaia value really did run on a thread the foreign runtime \
         owns, not on ours: {}",
        said(&run)
    );
}

/// Question 2, first rule: a value that may **not** cross a thread, handed to
/// a thread the foreign runtime owns.
///
/// It is refused, and **by whom depends on who can decide**. The structural
/// `Send` check (ADR-005 §1 Group B, `NK2501`/`NK2502`) decides what is written
/// down: a `Shared` is `MayNot` and is refused in Nikaia words with a caret in
/// the `.nika` file. This program's value is not written down - it is *borrowed
/// from the foreign crate*, so its type has no ledger entry and the verdict is
/// `Undecided`, which is not permission and not a refusal either
/// (`contracts::send`). So what refuses this one is still `rustc`.
///
/// What changed is the half Part III C.1 actually calls a bug: the refusal now
/// arrives **against the `.nika` line**, because ADR-005 D7 enumerates `E0277`
/// and `nikaia build` reads `cargo --message-format=json` through
/// `diagnostics::translate` instead of handing Cargo's stderr to the terminal.
/// The *text* is still `rustc`'s, which D7 records as the open half.
#[test]
#[ignore = "runs cargo and fetches hyper and tokio from crates.io"]
fn a_value_that_may_not_cross_a_thread_is_refused_against_the_nika_line() {
    let built = nikaia("build", &experiment().join("crossing"));
    assert!(
        !built.status.success(),
        "an `Rc` reached a foreign thread and the build succeeded. If this \
         fires, read `docs/foreign-runtime.md` - it is either a real \
         unsoundness or something about the crossing has changed: {}",
        said(&built)
    );

    let complaint = said(&built);
    assert!(
        complaint.contains("cannot be sent between threads safely"),
        "the refusal is a `Send` refusal: {complaint}"
    );

    // The half that is fixed: the place. Part III C.1's Iron Rule.
    assert!(
        complaint.contains("src/main.nika:"),
        "the refusal names the `.nika` line the author wrote: {complaint}"
    );
    assert!(
        complaint.contains("across_a_thread"),
        "and shows the statement it is about: {complaint}"
    );
    assert!(
        !complaint.contains("gen/foreign_runtime_crossing.rs:"),
        "and no longer points into a file the author has never read: {complaint}"
    );

    // The half that is open, asserted so that it cannot quietly stop being
    // true: the *text* is still Rust's, which ADR-005 D7 records rather than
    // solves. A frontend check cannot translate it, because the type it is
    // about was never written in Nikaia.
    assert!(
        !complaint.contains("NK25"),
        "this crossing is `Undecided` for the structural check - the value's \
         type comes from the foreign crate - so there is no `NK25xx` for it. A \
         code appearing here means the check has learned to decide it, and this \
         test is what has to change: {complaint}"
    );
}

/// The same crossing through a foreign API that lies about `Send`.
///
/// It builds, it runs, and it increments an `Rc` refcount on a thread the
/// foreign runtime owns. Nikaia contributes no check to either outcome: the
/// whole of D7's first rule is, today, rustc's `Send` bound on whatever the
/// foreign crate happened to write - which ADR-033 D4 already names as the
/// "reached through `unsafe`" case, and which no analysis in the frontend can
/// see.
#[test]
#[ignore = "runs cargo and fetches hyper and tokio from crates.io"]
fn a_foreign_api_that_lies_about_send_is_not_caught_by_anything() {
    let run = nikaia("run", &experiment().join("smuggled"));
    assert!(
        run.status.success(),
        "this is expected to build and run - the point is that nothing stops \
         it: {}",
        said(&run)
    );
    assert!(
        String::from_utf8_lossy(&run.stdout).contains("crossed: not Send"),
        "an `Rc` was read on a foreign thread: {}",
        said(&run)
    );
}

/// The experiment is three projects and a shim, and a test that drives them by
/// path says nothing useful once one of them has been renamed away.
#[test]
fn the_experiment_is_where_the_tests_say_it_is() {
    for name in [
        "README.md",
        "overlaps.nika",
        "shim/Cargo.toml",
        "shim/src/lib.rs",
        "serve/nikaia.toml",
        "crossing/nikaia.toml",
        "smuggled/nikaia.toml",
    ] {
        assert!(
            experiment().join(name).exists(),
            "examples/foreign-runtime/{name} is missing"
        );
    }
}
