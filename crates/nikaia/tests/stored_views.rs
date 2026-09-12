//! A view kept past the call that was given it (`NK2302`).
//!
//! The defect this exists for: a function taking a naked `&str` and storing it
//! lowered to Rust that `rustc` then refused, naming the generated file - which
//! Part III C.1 calls a bug in this compiler. The program is refused here
//! instead, in Nikaia's own words, and the message shows the shape that works.
//!
//! Three halves now. The first decides whether the check is worth running:
//! **every program in the repository must still produce no finding.** The second
//! is the deliberately stored views, which are the proof that the guard is not
//! passing because the analysis is asleep. The third is the shapes the analysis
//! reaches one step further into: where the destination already names the buffer,
//! the program is *lowered* with that buffer written out, and those tests compile
//! and run the result rather than reading the text.

mod common;

use std::path::PathBuf;

use nikaia::check::{self, Finding};
use nikaia::contracts::{Ledger, STD};
use nikaia::emit::{emit_program, Build};
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

// --- what it refuses, which is where the reach stops ------------------------
//
// Each of these is a destination the analysis can see and cannot name a buffer
// for. They are tests rather than prose so that extending the reach is a visible
// change to this file rather than a silent one.

/// The view is handed back out of the method, and the result does not point into
/// that parameter's buffer. Nothing here names a buffer to write on the
/// parameter, so this is a refusal - and it carries the whole message.
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
    assert_eq!(finding.code, "NK2302");
    assert!(
        finding.message.contains("`name: &str`"),
        "{}",
        finding.message
    );
    assert!(
        finding.notes.iter().any(|n| n.contains("hands back")),
        "{:#?}",
        finding.notes
    );
    // Part III C.2: the way out is shown, not described.
    let help = finding.help.expect("every diagnostic names a way out");
    assert!(help.contains("struct Held { name: &str }"), "{help}");
    assert!(help.contains("examples/1brc.nika"), "{help}");
}

/// A view put into a struct the function builds and hands back, from an `impl`
/// whose own target holds no view. `Reading` carries a buffer; nothing here says
/// which one, and the `impl` has none to lend.
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

/// A task outlives the call, so a view handed to one is refused whatever the
/// subject carries.
#[test]
fn a_naked_view_given_to_a_task_is_refused() {
    let finding = one(r#"
        struct Summary { label: &str }
        impl Summary {
            fn post(&self, name: &str) {
                spawn({ println(name) })
            }
        }
        "#);
    assert!(
        finding.notes.iter().any(|n| n.contains("task")),
        "{:#?}",
        finding.notes
    );
}

// --- one step further: a destination that already names the buffer -----------

/// Lower, compile with the `rustc` that built this test, run, and return stdout.
///
/// Reading the emitted text is not enough here: what the old behaviour produced
/// was text that looked right and did not compile, which is the whole defect.
fn lower_compile_run(source: &str, purpose: &str) -> String {
    let parsed = parse_to_ast(source).expect("the source parses");
    let own = Ledger::infer(&parsed);
    let library = Ledger::parse(STD).expect("std's shipped ledger parses");
    let refusals: Vec<String> = check::check(&parsed, &own, &library)
        .findings
        .into_iter()
        .map(|f| format!("{}: {}", f.code, f.message))
        .collect();
    assert!(
        refusals.is_empty(),
        "the compiler refused a program it should lower: {refusals:#?}"
    );

    let rust = emit_program(&parsed, Build::default())
        .expect("the source lowers")
        .rust;

    let dir = common::scratch_dir(purpose);
    let path = dir.join("prog.rs");
    std::fs::write(&path, &rust).expect("write the emitted Rust");
    let binary = dir.join("prog");
    let compiled = common::compile(
        &path,
        &[
            "--crate-type",
            "bin",
            "-o",
            binary.to_str().expect("utf-8 path"),
        ],
    );
    assert!(
        compiled.status.success(),
        "the emitted Rust did not compile:\n{}\n--- emitted ---\n{rust}",
        String::from_utf8_lossy(&compiled.stderr)
    );

    let run = std::process::Command::new(&binary)
        .output()
        .expect("run the program");
    assert!(
        run.status.success(),
        "the program failed: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    let _ = std::fs::remove_dir_all(&dir);
    String::from_utf8_lossy(&run.stdout).trim().to_string()
}

/// The defect, now lowered instead of refused: the subject already carries the
/// buffer, so the parameter is written as a view of *that* buffer.
///
/// This is the program `rustc` used to refuse with `E0621` about a line nobody
/// wrote. It compiles and prints.
#[test]
fn a_view_stored_in_the_subject_is_lowered_with_the_buffer_named() {
    let source = r#"
        struct Summary { label: &str }
        impl Summary {
            pub fn() -> Summary sync { return Summary(label: "none") }
            fn record(&mut self, name: &str) sync {
                self.label = name
            }
        }
        fn main() {
            let mut s = Summary::new()
            s.record("Hamburg")
            println(s.label)
        }
    "#;
    let rust = emit_program(
        &parse_to_ast(source).expect("the source parses"),
        Build::default(),
    )
    .expect("the source lowers")
    .rust;
    assert!(
        rust.contains("fn record(&mut self, name: &'a str)"),
        "the parameter has to be written as a view of the input buffer:
{rust}"
    );
    assert_eq!(lower_compile_run(source, "stored-view-subject"), "Hamburg");
}

/// The same through a call the compiler cannot see the end of. `insert` may keep
/// what it is given and nothing written down says it does, so this used to be
/// refused in stage A and used to reach `rustc` before that.
#[test]
fn a_view_handed_to_a_call_on_the_subject_is_lowered_too() {
    let source = r#"
        use std::collections::HashMap
        struct Summary { stations: HashMap[&str, i32] }
        impl Summary {
            pub fn() -> Summary sync { return Summary(stations: HashMap::new()) }
            fn record(&mut self, name: &str, temp: i32) sync {
                self.stations.insert(name, temp)
            }
            fn count(&self) -> usize sync { return self.stations.len() }
        }
        fn main() {
            let mut s = Summary::new()
            s.record("Hamburg", 12)
            println(f"{s.count()}")
        }
    "#;
    assert_eq!(lower_compile_run(source, "stored-view-call"), "1");
}

/// A read-only call on a field that holds views. The refusal in stage A covered
/// this, because nothing tells it from `insert`; naming the buffer covers it
/// instead, and costs the program nothing.
#[test]
fn a_read_only_call_on_a_view_field_is_lowered_rather_than_refused() {
    let source = r#"
        use std::collections::HashMap
        struct Summary { stations: HashMap[&str, i32] }
        impl Summary {
            pub fn() -> Summary sync { return Summary(stations: HashMap::new()) }
            fn has(&self, name: &str) -> bool sync {
                return self.stations.contains_key(name)
            }
        }
        fn main() {
            let s = Summary::new()
            println(f"{s.has(\"Hamburg\")}")
        }
    "#;
    assert_eq!(lower_compile_run(source, "stored-view-read"), "false");
}

/// A view that reaches the field through a local. The carrier is followed by
/// name, so the parameter is written with the buffer and the program lowers.
#[test]
fn a_view_that_reaches_the_field_through_a_local_is_lowered_too() {
    let source = r#"
        use std::collections::HashMap
        struct Summary { stations: HashMap[&str, i32] }
        impl Summary {
            pub fn() -> Summary sync { return Summary(stations: HashMap::new()) }
            fn record(&mut self, name: &str) sync {
                let key = name
                self.stations.insert(key, 1)
            }
            fn count(&self) -> usize sync { return self.stations.len() }
        }
        fn main() {
            let mut s = Summary::new()
            s.record("Hamburg")
            println(f"{s.count()}")
        }
    "#;
    let rust = emit_program(
        &parse_to_ast(source).expect("the source parses"),
        Build::default(),
    )
    .expect("the source lowers")
    .rust;
    assert!(
        rust.contains("fn record(&mut self, name: &'a str)"),
        "{rust}"
    );
    assert_eq!(lower_compile_run(source, "stored-view-local"), "1");
}

/// A parameter nothing stores keeps the spelling it had: the buffer is written
/// where the body needs it and nowhere else.
#[test]
fn a_view_that_is_only_read_keeps_the_signature_it_had() {
    let rust = emit_program(
        &parse_to_ast(
            r#"
            struct Summary { label: &str }
            impl Summary {
                fn show(&self, name: &str) sync { println(name) }
            }
            "#,
        )
        .expect("the source parses"),
        Build::default(),
    )
    .expect("the source lowers")
    .rust;
    assert!(rust.contains("fn show(&self, name: &str)"), "{rust}");
}
