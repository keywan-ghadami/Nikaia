//! Every example that claims to run, compiled and run.
//!
//! `examples/README.md` divides the directory in two: files that the bootstrap
//! compiler handles today, and files written at specification level to find out
//! what the specification forgot. Only the first kind can be checked, and this
//! is where that check lives - lower the real file, compile the emitted Rust
//! with the `rustc` that built this test, run the binary, compare what it
//! printed.
//!
//! Under **both profiles**, and the outputs must be equal. That is the claim
//! the profiles rest on (Part I): the same source compiles under Lite and
//! Advanced and means the same thing, only the runtime underneath differs. A
//! test that only ran one of them would leave the interesting half unchecked.
//!
//! `1brc.nika` has a test of its own (`one_brc.rs`), because it checks more
//! than its output. The last test here makes sure no example escapes both.

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use nikaia::emit::{emit_program, Profile};
use nikaia::parser::parse_to_ast;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// An example, and what running it must print.
struct Example {
    /// The file under `examples/`.
    file: &'static str,
    /// A file written into the scratch directory first. `{input}` in `args`
    /// stands for its path.
    input: Option<Input>,
    args: &'static [&'static str],
    /// Compared after trimming, so a trailing newline is not a test.
    expected: &'static str,
}

struct Input {
    name: &'static str,
    contents: &'static str,
}

/// The examples that run. An entry here is a promise that `cargo test` keeps.
const RUNNABLE: &[Example] = &[
    Example {
        file: "calc.nika",
        input: None,
        args: &["2 + 3 * (10 - 4) / 2"],
        expected: "2 + 3 * (10 - 4) / 2 = 11",
    },
    Example {
        file: "access-log.nika",
        input: Some(Input {
            name: "access.log",
            contents: "\
203.0.113.7 GET /index.html 200 5120
203.0.113.7 GET /style.css 200 1024
198.51.100.4 POST /api/order 201 512
198.51.100.4 GET /missing 404 0
203.0.113.7 GET /index.html 200 5120
",
        }),
        args: &["{input}"],
        // Sorted by path; /index.html was hit twice, and the 404 is the one
        // failure. The counts are what makes the merge visible: under Advanced
        // the five lines are parsed by several accumulators and added up.
        expected: "\
5 requests, 1 failed
/api/order 1 512
/index.html 2 10240
/missing 1 0
/style.css 1 1024",
    },
    Example {
        file: "config.nika",
        input: Some(Input {
            name: "app.conf",
            // A comment on its own line, one after a value, and a blank line:
            // all three are `WS` here, which is what defining `WS` buys.
            contents: "\
# app.conf - the front door
[server]
host = 0.0.0.0
port = 8080          # the usual one

# what a request may cost
[limits]
max_body = 1048576
timeout = 30s
",
        }),
        args: &["{input}"],
        expected: "\
2 sections
[server]
  host = 0.0.0.0
  port = 8080
[limits]
  max_body = 1048576
  timeout = 30s",
    },
];

/// Written at specification level: they say what the language is meant to look
/// like and the bootstrap compiler does not handle them yet. Each one's gaps
/// are listed in `examples/README.md`.
const SPECIFICATION_LEVEL: &[&str] = &["fortunes.nika"];

/// Runnable, but with a test of its own that checks more than the output.
const COVERED_ELSEWHERE: &[&str] = &["1brc.nika"];

#[test]
fn every_runnable_example_prints_what_it_promises() {
    for example in RUNNABLE {
        for profile in [Profile::Lite, Profile::Advanced] {
            let printed = build_and_run(example, profile);
            assert_eq!(
                printed.trim(),
                example.expected,
                "{} under {profile:?}",
                example.file
            );
        }
    }
}

/// The two profiles are one language, not two dialects: same source, same
/// output. Checked separately from the value above so a failure says which of
/// the two claims broke.
#[test]
fn the_profiles_agree_on_every_example() {
    for example in RUNNABLE {
        let lite = build_and_run(example, Profile::Lite);
        let advanced = build_and_run(example, Profile::Advanced);
        assert_eq!(lite, advanced, "{} differs between profiles", example.file);
    }
}

/// An example is either checked or declared unfinished. Adding a file to
/// `examples/` without deciding which is what this catches.
#[test]
fn no_example_is_neither_run_nor_declared() {
    let dir = repo_root().join("examples");
    let mut seen = 0;

    for entry in std::fs::read_dir(&dir).expect("read examples directory") {
        let path = entry.expect("dir entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("nika") {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .expect("utf-8 file name");
        seen += 1;

        let known = RUNNABLE.iter().any(|e| e.file == name)
            || SPECIFICATION_LEVEL.contains(&name)
            || COVERED_ELSEWHERE.contains(&name);

        assert!(
            known,
            "examples/{name} is in neither list in tests/examples.rs: add it to \
             RUNNABLE with what it prints, or to SPECIFICATION_LEVEL with its gaps \
             in examples/README.md"
        );
    }

    assert!(seen > 0, "no examples found in {}", dir.display());
}

/// Every file the specification-level list names must actually be there, so a
/// renamed or deleted example cannot leave a stale excuse behind.
#[test]
fn the_lists_name_files_that_exist() {
    for name in SPECIFICATION_LEVEL.iter().chain(COVERED_ELSEWHERE) {
        let path = repo_root().join("examples").join(name);
        assert!(path.exists(), "{} is listed but not there", path.display());
    }
    for example in RUNNABLE {
        let path = repo_root().join("examples").join(example.file);
        assert!(path.exists(), "{} is listed but not there", path.display());
    }
}

/// A malformed line is a value the program can print, not a panic and not a
/// wrong summary. The example's `catch` is what turns it into one, and this is
/// the claim its comment makes.
#[test]
fn a_malformed_line_is_reported_and_no_summary_is_printed() {
    let (dir, binary) = build("access-log.nika", Profile::Advanced);

    // The status is two digits where the format fixes three.
    let log = dir.join("broken.log");
    std::fs::write(
        &log,
        "203.0.113.7 GET /index.html 200 5120\n198.51.100.4 GET /oops 20 512\n",
    )
    .expect("write the input");

    let run = Command::new(&binary)
        .arg(&log)
        .output()
        .expect("run the compiled example");

    assert!(run.status.success(), "it should report, not fail");
    assert!(
        String::from_utf8_lossy(&run.stdout).is_empty(),
        "a rejected file must not produce a summary: {}",
        String::from_utf8_lossy(&run.stdout)
    );

    // Line 2, at the third character of the status - the offset is in the
    // whole file, not in whichever piece a core was holding, and it is a line
    // and a column rather than a byte because `dsl … from …` renders the error
    // against the input it parsed.
    let reported = String::from_utf8_lossy(&run.stderr).into_owned();
    assert!(
        reported.contains("line 2"),
        "the message should say where: {reported}"
    );
    assert!(
        reported.contains("expected a digit"),
        "the message should say what was expected: {reported}"
    );
    assert!(
        reported.contains("in STATUS"),
        "the message should say which rule wanted it: {reported}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// The label in `calc.nika`'s `factor`, end to end.
///
/// A Nikaia grammar says what a rule is called (`# "expression"`), the emitter
/// hands that to the backend in the backend's own spelling, the backend
/// reports the word instead of the three ways an operand can start, and the
/// driver renders it against the input the program parsed. Four pieces, one
/// message, and this is the only place all four are exercised together.
#[test]
fn a_missing_operand_is_reported_as_an_expression() {
    let (dir, binary) = build("calc.nika", Profile::Advanced);

    let run = Command::new(&binary)
        .arg("2 +")
        .output()
        .expect("run the compiled example");

    assert!(run.status.success(), "it should report, not fail");
    assert!(
        String::from_utf8_lossy(&run.stdout).is_empty(),
        "a rejected expression must not print a result: {}",
        String::from_utf8_lossy(&run.stdout)
    );

    let reported = String::from_utf8_lossy(&run.stderr).into_owned();
    assert!(
        reported.contains("expected expression"),
        "the label should be the expectation: {reported}"
    );
    assert!(
        reported.contains("column 4"),
        "and the position should be where the operand belongs: {reported}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// Lower and compile. The example is the real file: it cannot drift.
fn build(file: &str, profile: Profile) -> (PathBuf, PathBuf) {
    let source_path = repo_root().join("examples").join(file);
    let source = std::fs::read_to_string(&source_path)
        .unwrap_or_else(|e| panic!("{}: {e}", source_path.display()));

    let parsed = parse_to_ast(&source).unwrap_or_else(|e| panic!("{file} does not parse:\n{e}"));
    let lowered =
        emit_program(&parsed, profile).unwrap_or_else(|e| panic!("{file} does not lower:\n{e}"));

    let dir = common::scratch_dir(&format!("example-{}", file.replace('.', "-")));
    let rust = dir.join("example.rs");
    std::fs::write(&rust, &lowered.rust).expect("write the emitted Rust");

    let binary = dir.join("example");
    let compiled = common::compile(
        &rust,
        &[
            "--crate-type",
            "bin",
            "-o",
            binary.to_str().expect("utf-8 path"),
        ],
    );
    assert!(
        compiled.status.success(),
        "{file} did not compile under {profile:?}:\n{}\n--- emitted ---\n{}",
        String::from_utf8_lossy(&compiled.stderr),
        lowered.rust
    );

    (dir, binary)
}

/// Lower, compile, run, and hand back what it printed.
fn build_and_run(example: &Example, profile: Profile) -> String {
    let (dir, binary) = build(example.file, profile);

    let input_path = example.input.as_ref().map(|input| {
        let path = dir.join(input.name);
        std::fs::write(&path, input.contents).expect("write the example's input");
        path
    });

    let args: Vec<String> = example
        .args
        .iter()
        .map(|arg| substitute(arg, input_path.as_deref()))
        .collect();

    let run = Command::new(&binary)
        .args(&args)
        .output()
        .expect("run the compiled example");
    assert!(
        run.status.success(),
        "{} failed under {profile:?}: {}",
        example.file,
        String::from_utf8_lossy(&run.stderr)
    );

    let printed = String::from_utf8_lossy(&run.stdout).into_owned();
    let _ = std::fs::remove_dir_all(&dir);
    printed
}

/// `{input}` stands for the path the example's input was written to.
fn substitute(arg: &str, input: Option<&Path>) -> String {
    match input {
        Some(path) => arg.replace("{input}", &path.display().to_string()),
        None => {
            assert!(
                !arg.contains("{input}"),
                "an argument names {{input}} but the example declares none"
            );
            arg.to_string()
        }
    }
}
