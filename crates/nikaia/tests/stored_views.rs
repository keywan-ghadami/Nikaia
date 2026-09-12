//! A view kept past the call that was given it (`NK2302`).
//!
//! The defect this exists for: a function taking a naked `&str` and storing it
//! lowered to Rust that `rustc` then refused, naming the generated file - which
//! Part III C.1 calls a bug in this compiler. The program is refused here
//! instead, in Nikaia's own words, and the message shows the shape that works.
//!
//! Two halves, and the first decides whether the check is worth running: **every
//! program in the repository must still produce no finding.** The deliberately
//! stored views below are the proof that the guard is not passing because the
//! analysis is asleep.

use std::path::PathBuf;

use nikaia::check::{self, Finding};
use nikaia::contracts::{Ledger, STD};
use nikaia::parser::parse_to_ast;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn findings(source: &str) -> Vec<Finding> {
    let parsed = parse_to_ast(source).expect("the source parses");
    let own = Ledger::infer(&parsed);
    let library = Ledger::parse(STD).expect("std's shipped ledger parses");
    check::check(&parsed, &own, &library)
        .findings
        .into_iter()
        .filter(|f| f.code == "NK2302")
        .collect()
}

fn one(source: &str) -> Finding {
    let found = findings(source);
    assert_eq!(
        found.len(),
        1,
        "expected exactly one NK2302, got {:#?}",
        found.iter().map(|f| &f.message).collect::<Vec<_>>()
    );
    found.into_iter().next().expect("checked above")
}

// --- the guard ---------------------------------------------------------------

/// No program in the repository keeps a naked view.
///
/// The cost of the check, measured rather than assumed. `examples/` is the
/// corpus that has to keep lowering and keep running; `crates/nikaia-std/src` is
/// the part of `std` written in Nikaia.
#[test]
fn no_program_in_the_repository_keeps_a_naked_view() {
    let mut checked = 0;
    let mut reported = String::new();

    for dir in ["examples", "crates/nikaia-std/src", "benches"] {
        let dir = repo_root().join(dir);
        let mut paths: Vec<PathBuf> = std::fs::read_dir(&dir)
            .unwrap_or_else(|e| panic!("read {}: {e}", dir.display()))
            .map(|entry| entry.expect("dir entry").path())
            .filter(|path| path.extension().and_then(|e| e.to_str()) == Some("nika"))
            .collect();
        paths.sort();

        for path in paths {
            let source = std::fs::read_to_string(&path).expect("read the program");
            // `fortunes.nika` is the one program in the corpus that does not
            // lower (`dsl postgres` has no lowering); it parses, so it is still
            // asked this question.
            let parsed = parse_to_ast(&source)
                .unwrap_or_else(|e| panic!("{} does not parse: {e}", path.display()));
            let name = path.display().to_string();
            for finding in nikaia::views::check(&parsed) {
                reported.push_str(&nikaia::diagnostics::render_finding(
                    &finding, &name, &source,
                ));
            }
            checked += 1;
        }
    }

    assert!(checked >= 12, "only {checked} programs were checked");
    assert!(reported.is_empty(), "the corpus is not clean:\n{reported}");
}

/// The idiomatic form, which is what the message points at: the view arrives
/// inside a struct whose own declaration says which buffer it points into.
///
/// This is `examples/1brc.nika`'s `Summary::record` with the names shortened,
/// and it has to stay silent - it is the answer the refusal offers.
#[test]
fn a_view_inside_a_struct_is_not_a_finding() {
    assert!(findings(
        r#"
        @borrowed
        struct Reading { name: &str, temp: i32 }
        struct Summary { stations: HashMap[&str, i32] }
        impl Summary {
            fn record(&mut self, m: Reading) sync {
                self.stations.insert(m.name, m.temp)
            }
        }
        "#,
    )
    .is_empty());
}

/// `examples/k-nucleotide.nika`'s shape: views of the parameter are written into
/// a map the function hands back. Correct, because the result points into that
/// parameter's buffer and nothing else's - and the one thing this check may not
/// do is refuse it.
#[test]
fn a_view_handed_back_in_the_result_it_came_from_is_not_a_finding() {
    assert!(findings(
        r#"
        struct Tally { n: i64 }
        fn count(seq: &str, k: usize) -> HashMap[&str, Tally] {
            let mut counts: HashMap[&str, Tally] = HashMap::new()
            for i in 0..seq.len() {
                let fragment = &seq[i..i + k]
                counts.entry(fragment).or_insert_with fn { Tally(1) }
            }
            return counts
        }
        "#,
    )
    .is_empty());
}

/// A field that holds no view cannot be where a view went, so storing text into
/// one is not a finding.
#[test]
fn a_field_that_holds_no_view_is_not_a_destination() {
    assert!(findings(
        r#"
        struct Log { lines: Vec[String], n: i64 }
        impl Log {
            fn add(&mut self, line: &str) sync {
                self.lines.push(line.to_owned())
                self.n += 1
            }
        }
        "#,
    )
    .is_empty());
}

// --- what it catches ---------------------------------------------------------

/// The defect, exactly: the naked form of `examples/1brc.nika`'s `record`.
#[test]
fn a_naked_view_put_into_a_field_of_the_subject_is_refused() {
    let finding = one(r#"
        struct Summary { stations: HashMap[&str, i32] }
        impl Summary {
            fn record(&mut self, name: &str, temp: i32) sync {
                self.stations.insert(name, temp)
            }
        }
        "#);
    assert_eq!(finding.code, "NK2302");
    assert!(
        finding.message.contains("`name: &str`"),
        "{}",
        finding.message
    );
    assert!(
        finding
            .notes
            .iter()
            .any(|n| n.contains("`Summary.stations`")),
        "{:#?}",
        finding.notes
    );
    // Part III C.2: the way out is shown, not described.
    let help = finding.help.expect("every diagnostic names a way out");
    assert!(help.contains("struct Held { name: &str }"), "{help}");
    assert!(help.contains("examples/1brc.nika"), "{help}");
}

/// Assignment straight into a view field, which is the same rule with no call in
/// the way.
#[test]
fn a_naked_view_assigned_to_a_view_field_is_refused() {
    let finding = one(r#"
        struct Summary { label: &str }
        impl Summary {
            fn note(&mut self, name: &str) sync {
                self.label = name
            }
        }
        "#);
    assert!(
        finding.notes.iter().any(|n| n.contains("`Summary.label`")),
        "{:#?}",
        finding.notes
    );
}

/// A view handed back out of a method, where the result does not point into that
/// parameter's buffer.
#[test]
fn a_naked_view_handed_back_from_a_method_is_refused() {
    let finding = one(r#"
        struct Summary { label: &str }
        impl Summary {
            fn pick(&self, name: &str) -> &str sync {
                return name
            }
        }
        "#);
    assert!(
        finding.notes.iter().any(|n| n.contains("hands back")),
        "{:#?}",
        finding.notes
    );
}

/// A view put into a struct the function builds and hands back. The struct's own
/// declaration carries a buffer; the parameter does not say it is the same one.
#[test]
fn a_naked_view_put_into_a_struct_that_is_handed_back_is_refused() {
    let finding = one(r#"
        @borrowed
        struct Reading { name: &str, temp: i32 }
        struct Factory { n: i32 }
        impl Factory {
            fn make(&self, name: &str) -> Reading sync {
                return Reading(name: name, temp: self.n)
            }
        }
        "#);
    assert!(
        finding.notes.iter().any(|n| n.contains("`Reading.name`")),
        "{:#?}",
        finding.notes
    );
}

/// A view that reaches the field through a local is still a finding: a name that
/// carries the view keeps carrying it.
#[test]
fn a_view_that_reaches_the_field_through_a_local_is_refused() {
    let finding = one(r#"
        struct Summary { stations: HashMap[&str, i32] }
        impl Summary {
            fn record(&mut self, name: &str) sync {
                let key = name
                self.stations.insert(key, 1)
            }
        }
        "#);
    assert_eq!(finding.code, "NK2302");
}
