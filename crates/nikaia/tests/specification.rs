//! Every `nika` block in the specification, taken as far as it goes.
//!
//! **A page can be wrong in a way nothing notices.** A program printed as an
//! example, never run, stale from the day a decision changed under it.
//! `docs/README.md` §1 already makes a stale **Status** note a defect in its own
//! right; this is the same rule applied to the code beside it.
//!
//! It exists because running the blocks by hand once turned up four things, and
//! two of them were on the page rather than in the compiler: Part I 4.7's
//! `return "User: " + self.username` was refused twice over — the checker said
//! it hands back a `&str` where `String` is declared, and the lowering would not
//! have compiled either — and three plain strings in Part I 7 held `{p}`,
//! `{line}` and `{expected}`, written before
//! [ADR-035](../../../docs/specification/adr/adr-035.md) made only `f"…"`
//! interpolate. Both are fixed; this is what stops them coming back.
//!
//! **The baseline records fragments and refusals too**, and that is deliberate.
//! A chapter shows signature lists, bodies without their function, and sketches
//! with `…` where code would be, so "every block compiles" is not the property —
//! *nothing changed without somebody reading the diff* is. A block that stops
//! lowering shows up as a changed line, and so does one that starts.

mod common;

use nikaia::specbook::{self, Stage};
use std::path::{Path, PathBuf};

fn baseline() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/specification/EXPECTED.txt")
        .canonicalize()
        .expect("the baseline")
}

#[test]
fn the_specifications_programs_are_the_ones_in_expected_txt() {
    let expected = std::fs::read_to_string(baseline()).expect("EXPECTED.txt");
    assert_eq!(
        specbook::report(&specbook::specification_dir()),
        expected,
        "the specification and tests/specification/EXPECTED.txt have drifted \
         apart. If that is the change you meant, regenerate it with `cargo run \
         -p nikaia --example specification > tests/specification/EXPECTED.txt` \
         and read the diff - a block that stopped lowering is a defect in the \
         page or in the compiler, and a block that started is progress worth \
         seeing"
    );
}

/// **No page holds a plain string with a hole in it**, except where the page is
/// about exactly that.
///
/// `NK1111` is the one-release migration warning for `"{name}"` after
/// [ADR-035](../../../docs/specification/adr/adr-035.md) D5, and a page that
/// trips it is a page written before that decision. Part I 2.5 trips it on
/// purpose — it is the section that explains the difference — so the allowance
/// is that one block, named by where it is rather than by a marker somebody has
/// to remember.
#[test]
fn only_the_section_about_interpolation_writes_a_plain_string_with_a_hole() {
    let tripped: Vec<_> = specbook::verdicts(&specbook::specification_dir())
        .into_iter()
        .filter(|v| v.codes.contains("NK1111"))
        .collect();
    assert_eq!(
        tripped.len(),
        1,
        "a plain string holding a hole is a page written before ADR-035 D5; the \
         one allowed is Part I 2.5, which is about the difference: {:?}",
        tripped
            .iter()
            .map(|v| format!("{} line {}", v.block.file, v.block.line))
            .collect::<Vec<_>>()
    );
    // Recognised by what the block **says** rather than by where it is: a line
    // number moves with every paragraph above it, which is the churn the report
    // above was rebuilt to stop having.
    assert!(
        tripped[0].block.code.contains("the braces are braces"),
        "and the one allowed is the block that explains the difference, not \
         whichever one happens to trip it: {}",
        tripped[0].block.code
    );
}

/// **How much of the specification is a program this compiler takes**, as a
/// floor rather than as a number to admire.
///
/// It is the page's own progress measure and the one figure the baseline above
/// does not state in one place: a chapter shows signature lists, bodies without
/// their function, and sketches with an elision where code would be, so "all of
/// them" is not the target and never will be. What this guards against is the
/// direction that matters — a change that quietly turns programs back into
/// fragments, which the baseline would also show and which a count says out
/// loud.
///
/// Raise it when it is beaten. That is the point of a floor.
#[test]
fn most_of_a_third_of_the_specifications_blocks_are_programs() {
    let verdicts = specbook::verdicts(&specbook::specification_dir());
    let lowered = verdicts
        .iter()
        .filter(|v| v.stage == Stage::Lowered)
        .count();
    assert!(
        lowered >= 52,
        "{lowered} of {} blocks lower, and 52 did when this floor was set - \
         raise it if it is beaten, and read the diff if it is not",
        verdicts.len()
    );
}

/// **And the ones that lower are handed to `rustc`.**
///
/// This is the half that found the defects. A block can pass every stage of this
/// compiler and be rejected by the language below, which
/// [Part III C.1](../../../docs/specification/30-nikaia-tooling.md) calls a bug
/// here rather than there — and two of the specification's own programs were
/// exactly that: Part I 4.5's three-line map example (`E0282`, in a message
/// naming `TrustedMap`, `BuildHasherDefault<FxHasher>` and a type parameter `K`,
/// none of which the program wrote) and every string concatenation except
/// `String + &str` (`E0369`). Both are in `docs/open-work.md`.
///
/// **Most of the failures here are not defects and the baseline says which.** A
/// chapter's block names `User`, `Config`, `postgres` or `http` and leaves them
/// to the chapter around it, so `rustc` cannot resolve them and neither could
/// anything else; `NK1117` deliberately does not refuse a name this compiler
/// cannot see ([Part III C.4](../../../docs/specification/30-nikaia-tooling.md)).
/// What the baseline is for is the same thing the one above is for: a block that
/// stops compiling is a line in a diff.
#[test]
fn what_lowers_is_handed_to_rustc_and_the_verdicts_are_the_recorded_ones() {
    let dir = common::scratch_dir("specification-compiles");
    let mut report = String::new();
    let mut nth: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for v in specbook::verdicts(&specbook::specification_dir()) {
        // Counted over **every** block and not only the ones that lower, so the
        // ordinal means the same thing here as in the other baseline.
        let at = nth.entry(v.block.file.clone()).or_insert(0);
        *at += 1;
        let ordinal = *at;
        if v.stage != Stage::Lowered {
            continue;
        }
        let source = specbook::reading(&v.block.code, v.reading)
            .expect("the reading that got furthest is one of the readings");
        let parsed = nikaia::parser::parse_to_ast(&source).expect("it parsed once already");
        let rust = nikaia::emit::emit_program(&parsed, nikaia::emit::Build::default())
            .expect("it lowered once already")
            .rust;
        let file = dir.join("block.rs");
        std::fs::write(&file, &rust).expect("write the Rust");
        let out = common::compile(
            &file,
            &[
                "--crate-type",
                "lib",
                "--emit=metadata",
                "-o",
                dir.join("block.rmeta").to_str().expect("utf-8 path"),
                "-A",
                "warnings",
            ],
        );
        let verdict = match out.status.success() {
            true => "compiles".to_string(),
            false => {
                let stderr = String::from_utf8_lossy(&out.stderr);
                let line = stderr
                    .lines()
                    .find(|l| l.starts_with("error"))
                    .unwrap_or("error (no line)");
                // The code only, never the message: a `rustc` upgrade rewords
                // its diagnostics and that must not be a failing test here.
                line.split(':').next().unwrap_or(line).to_string()
            }
        };
        report.push_str(&format!("{} #{ordinal} {verdict}\n", v.block.file));
    }
    let _ = std::fs::remove_dir_all(&dir);

    let baseline = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/specification/COMPILES.txt")
        .canonicalize()
        .expect("the compile baseline");
    let expected = std::fs::read_to_string(baseline).expect("COMPILES.txt");
    assert_eq!(
        report, expected,
        "what the language below says about the specification's programs has \
         changed. Regenerate with `cargo run -p nikaia --example specification \
         --features regenerate` is not a thing - this baseline is written by \
         this test, so copy the left-hand side in and read the diff: a block \
         that stopped compiling is Part III C.1's class"
    );
}
