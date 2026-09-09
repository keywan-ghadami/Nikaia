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
    /// Fed to the program on **standard input**, for the examples that read it
    /// (ADR-019). A pipe is not a path, so this is not `input` with a flag.
    stdin: Option<&'static str>,
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
        stdin: None,
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
        stdin: None,
        args: &["{input}"],
        // Sorted by path; /index.html was hit twice, and the 404 is the one
        // failure. The counts are what makes the merge visible: under Advanced
        // the five lines are parsed by several accumulators and added up.
        // By hits, descending; ties keep the name order the first sort put
        // them in - which is the compound order two stable sorts state.
        expected: "\
5 requests, 1 failed
/index.html 2 10240
/api/order 1 512
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
        stdin: None,
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
    Example {
        file: "json.nika",
        input: Some(Input {
            name: "data.json",
            contents: "{\"name\": \"a\\nb\", \"tags\": [1, 2.5, true, null], \"empty\": {}}\n",
        }),
        stdin: None,
        args: &["{input}"],
        // The document back, two spaces a level, with the string bodies
        // printed raw - and one line that had to decode one: `a\nb` is four
        // characters in the file and three in the value.
        expected: "\
{
  \"name\": \"a\\nb\",
  \"tags\": [
    1,
    2.5,
    true,
    null
  ],
  \"empty\": {}
}
longest string: 3 characters",
    },
    Example {
        file: "n-body.nika",
        input: None,
        stdin: None,
        args: &["1000"],
        // The Computer Language Benchmarks Game's published output for
        // n = 1000, to the digit. That is what makes this example worth
        // having: the number is not ours to choose, so the arithmetic either
        // agrees with thirty other languages or it does not.
        expected: "\
-0.169075164
-0.169087605",
    },
    Example {
        file: "k-nucleotide.nika",
        input: None,
        // On a pipe, which is the point of the example (ADR-019). Three
        // sections so that picking the third is a real choice, and the second
        // in lower case so that the uppercasing is doing something.
        stdin: Some(
            ">ONE Homo sapiens alu\n\
             GGCCGGGCGCGGTGGCTCACGCCTGTAATCCCAGCACTTTGG\n\
             GAGGCCGAGGCGGGCGGATCACCTGAGGTCAGGAGTTCGAGA\n\
             >TWO IUB ambiguity codes\n\
             cttBtatcatatgctaKggNcataaaSatgtaaaDcDRtBggDtctttataattcBgtcg\n\
             >THREE Homo sapiens frequency\n\
             aacacttcaccaggtatcgtgaaggctcaagattacccagagaacctttgcaatataaga\n\
             atatgtatgcagcattaccctaagtaattatattctttttctgactcaaagtgacaagcc\n\
             ctagtgtatattaaatcggtatatttgggaaattcctcaaactatcctaatcaggtagcc\n",
        ),
        args: &[],
        // The benchmark's own report, on 180 characters instead of 25 MB:
        // every single character and every pair as a percentage, most frequent
        // first with ties in alphabetical order, then five named fragments.
        expected: "\
A 33.333
T 30.000
C 20.556
G 16.111

TA 11.173
AA 10.615
AT 10.615
TT 8.380
AG 7.263
CA 6.704
CC 6.145
CT 6.145
TC 6.145
AC 5.028
GT 5.028
GA 4.469
TG 4.469
GC 3.352
GG 3.352
CG 1.117

3\tGGT
3\tGGTA
0\tGGTATT
0\tGGTATTTTAATT
0\tGGTATTTTAATTTATAGT",
    },
    Example {
        file: "escaping.nika",
        input: None,
        stdin: None,
        args: &[],
        // Every character that changes what HTML means is gone from the three
        // rows that hold text: `<` and `&` in a name, the quotes around a note,
        // an apostrophe, and a `</td>` that is not a tag here. The fourth is
        // the one the program built as markup and said so with a type - it
        // keeps its `<em>`, and the name inside it is escaped once, by the call
        // that made the promise.
        expected: "\
<table>
        <tr class=\"odd\"><td>Ada</td><td>fine</td></tr>\
<tr class=\"even\"><td>a&lt;b &amp; c</td><td>&quot;quoted&quot;</td></tr>\
<tr class=\"odd\"><td>O&#39;Hara</td><td>&lt;/td&gt; is not a tag here</td></tr>
        </table>
<tr class=\"odd\"><td>Ada</td><td><em>Ada</em></td></tr>",
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

    // …and it shows the line, with a caret under the character it stopped at.
    // A position a reader has to go and look up is half a diagnostic, and the
    // program prints this where a person will see it.
    assert!(
        reported.contains("   2 | 198.51.100.4 GET /oops 20 512"),
        "the message should show the line: {reported}"
    );
    assert!(
        reported.lines().any(|l| l.trim_start().starts_with('^')),
        "the message should point at the character: {reported}"
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

    let mut command = Command::new(&binary);
    command.args(&args);

    let run = match example.stdin {
        None => command.output().expect("run the compiled example"),
        Some(text) => {
            // A pipe, not a redirected file: what the program sees is what
            // ADR-019 is about.
            let mut child = command
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .expect("spawn the compiled example");
            use std::io::Write;
            child
                .stdin
                .take()
                .expect("the child's stdin")
                .write_all(text.as_bytes())
                .expect("write to the child's stdin");
            child.wait_with_output().expect("run the compiled example")
        }
    };
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
