//! A block that joins on the executor pauses, and its branches travel in the
//! function's own channel
//! ([ADR-163](../../../docs/specification/adr/adr-163.md)).
//!
//! `overlap { … }` and `select { … }` are the two constructs that hand several
//! branches to the executor and wait there. Both were built before
//! [ADR-055](../../../docs/specification/adr/adr-055.md) made the lowering
//! `async` and before [ADR-157](../../../docs/specification/adr/adr-157.md)
//! gave the failure channel a type, and neither was asked again afterwards. So
//! each of them had a correct program that `rustc` refused, in a file the
//! author never wrote — [Part III C.1](../../../docs/specification/30-nikaia-tooling.md):
//!
//! * a block whose branches cannot pause left the enclosing function `sync`,
//!   and the vehicle's `.await` landed in a `fn` that is not `async` (`E0728`);
//! * a fallible branch was wrapped in `Ok::<_, Box<dyn Error>>` whatever the
//!   function's channel was, so the `?` under it could not convert (`E0277`).
//!
//! **These are run rather than read.** Both defects are about what the language
//! below accepts, and comparing the emitted string would say no more than that
//! this compiler agrees with itself.

mod common;

use nikaia::check;
use nikaia::contracts::{Ledger, STD};
use nikaia::emit::{emit_program, Build};
use nikaia::parser::parse_to_ast;

fn lowered(source: &str) -> String {
    let parsed = parse_to_ast(source).expect("the source parses");
    emit_program(&parsed, Build::default())
        .expect("the source lowers")
        .rust
}

/// The `sync` violations a source has, as the check reports them.
fn violations(source: &str) -> Vec<nikaia::contracts::sync::Violation> {
    let parsed = parse_to_ast(source).expect("the source parses");
    let own = Ledger::infer(&parsed);
    let library = Ledger::parse(STD).expect("std ships a ledger");
    nikaia::contracts::sync::check(&parsed, &own, &library)
}

/// What the ledger derives for one function's `sync` column.
fn is_sync(source: &str, of: &str) -> bool {
    let parsed = parse_to_ast(source).expect("the source parses");
    Ledger::infer(&parsed).functions[of].sync.is_sync()
}

/// Lower it, compile it, run it, hand back what it printed — which is the only
/// shape that holds C.1 closed.
fn output(purpose: &str, source: &str) -> String {
    let parsed = parse_to_ast(source).expect("the source parses");
    let own = Ledger::infer(&parsed);
    let library = Ledger::parse(STD).expect("std ships a ledger");
    let found =
        check::check_program(&parsed, &own, &library, &std::collections::BTreeSet::new()).findings;
    assert!(
        found.is_empty(),
        "{purpose} is a correct program: {found:#?}"
    );

    let rust = lowered(source);
    let dir = common::scratch_dir(purpose);
    let file = dir.join("main.rs");
    std::fs::write(&file, &rust).expect("write the Rust");
    let binary = dir.join("program");
    let out = common::compile(&file, &["-o", binary.to_str().expect("utf-8 path")]);
    assert!(
        out.status.success(),
        "the lowering of {purpose} compiles:\n{}\n--- the Rust ---\n{rust}",
        String::from_utf8_lossy(&out.stderr)
    );
    let ran = std::process::Command::new(&binary)
        .current_dir(&dir)
        .output()
        .expect("the program runs");
    let printed = String::from_utf8_lossy(&ran.stdout).trim().to_string();
    let _ = std::fs::remove_dir_all(&dir);
    printed
}

/// Two branches that cannot pause, which is the ten-line program that used to
/// be refused.
const SYNC_OVERLAP: &str = "fn twice(n: i64) -> i64 {\n\
                            \x20   return n * 2\n\
                            }\n\
                            \n\
                            fn main() {\n\
                            \x20   let pair = overlap {\n\
                            \x20       twice(3)\n\
                            \x20       twice(4)\n\
                            \x20   }\n\
                            \x20   println(f\"{pair.0} {pair.1}\")\n\
                            }\n";

/// The same one construct over.
const SYNC_SELECT: &str = "fn a() -> i64 { return 1 }\n\
                           fn b() -> i64 { return 2 }\n\
                           \n\
                           fn main() {\n\
                           \x20   select {\n\
                           \x20       x = a() => { println(f\"{x}\") }\n\
                           \x20       y = b() => { println(f\"{y}\") }\n\
                           \x20   }\n\
                           }\n";

/// An error type of this program's own, so the channel is a `Thrown<E>`.
const OWN_ERROR: &str = "enum LoadError {\n\
                         \x20   Missing(String),\n\
                         \x20   Broken(String),\n\
                         }\n\
                         \n\
                         impl Error for LoadError {\n\
                         \x20   fn message(&self) -> String {\n\
                         \x20       match self {\n\
                         \x20           LoadError::Missing(w) => { return f\"{w} is missing\" }\n\
                         \x20           LoadError::Broken(w) => { return f\"{w} is broken\" }\n\
                         \x20       }\n\
                         \x20   }\n\
                         }\n\
                         \n\
                         fn load(which: String) -> String throws {\n\
                         \x20   if which == \"a\" {\n\
                         \x20       throw LoadError::Missing(which)\n\
                         \x20   }\n\
                         \x20   if which == \"b\" {\n\
                         \x20       throw LoadError::Broken(which)\n\
                         \x20   }\n\
                         \x20   return which\n\
                         }\n";

/// **D1: an `overlap` of branches that cannot pause is still a pause.**
///
/// `task::overlap<n>` is an `async fn` and the block lowers to `.await` on it
/// whatever the branches do — so a function holding one parks on the executor,
/// and the `sync` column has to say so. It did not, and the ten lines below are
/// what that cost: `await is only allowed inside async functions`, about a file
/// the author never wrote.
#[test]
fn an_overlap_of_branches_that_cannot_pause_is_a_pause() {
    assert!(
        !is_sync(SYNC_OVERLAP, "main"),
        "the block is a suspension point, so `main` cannot be `sync`"
    );
    // And the lowering agrees with the column, which is the half `rustc` used
    // to have to point out.
    let rust = lowered(SYNC_OVERLAP);
    assert!(rust.contains("async fn __nikaia_main"), "{rust}");
    assert_eq!(output("joining-overlap-sync", SYNC_OVERLAP), "6 8");
}

/// The same for a `select`, because it is the same mechanism: `task::race<n>`
/// is awaited too.
#[test]
fn a_select_of_arms_that_cannot_pause_is_a_pause() {
    assert!(!is_sync(SYNC_SELECT, "main"), "a `select` parks too");
    let rust = lowered(SYNC_SELECT);
    assert!(rust.contains("async fn __nikaia_main"), "{rust}");
    // One of the two arms wins and the other is dropped; which one is the
    // executor's to say, so this asserts that *an* arm answered.
    let printed = output("joining-select-sync", SYNC_SELECT);
    assert!(printed == "1" || printed == "2", "{printed}");
}

/// **A branch that can pause still makes the block one**, which is the case
/// that always worked and is asserted here so the new answer is not the only
/// answer.
#[test]
fn a_branch_that_pauses_still_makes_the_block_pause() {
    let source = "use std::fs\n\
                  \n\
                  fn main() throws {\n\
                  \x20   let r = overlap {\n\
                  \x20       fs::read_to_string(\"eins.txt\")\n\
                  \x20       fs::read_to_string(\"zwei.txt\")\n\
                  \x20   }\n\
                  \x20   println(f\"{r.0} {r.1}\")\n\
                  }\n";
    assert!(!is_sync(source, "main"));
}

/// **D1's other half: an assertion is contradicted too** (`NK2202`).
///
/// [ADR-027](../../../docs/specification/adr/adr-027.md) D4 says the inference
/// never overwrites a written `sync`, so fixing the derivation alone would have
/// left a hand-written one on a body that pauses — and `rustc` would say so
/// about the generated file, which is the defect this record is about.
#[test]
fn a_written_sync_over_a_joining_block_is_refused() {
    let source = "fn twice(n: i64) -> i64 { return n * 2 }\n\
                  \n\
                  fn both() -> i64 sync {\n\
                  \x20   let pair = overlap {\n\
                  \x20       twice(3)\n\
                  \x20       twice(4)\n\
                  \x20   }\n\
                  \x20   return pair.0 + pair.1\n\
                  }\n";
    let found = violations(source);
    assert_eq!(found.len(), 1, "{found:#?}");
    assert_eq!(found[0].caller, "both");
    assert_eq!(found[0].callee, "overlap");
    // **A construct has no ledger entry**, so the diagnostic says where the
    // pause is instead of naming one that does not exist.
    assert!(found[0].construct, "{found:#?}");
    let rendered = nikaia::diagnostics::render_sync_violation(&found[0], "both.nika", source);
    assert!(rendered.contains("error[NK2202]"), "{rendered}");
    assert!(
        rendered.contains("hands its branches to the executor"),
        "{rendered}"
    );
    assert!(
        !rendered.contains("carries no `sync`"),
        "a construct is not a ledger entry:\n{rendered}"
    );
}

/// **D2: a branch travels in the function's own channel.**
///
/// The vehicle needs the error type written out — an `async` block with a `?`
/// in it and nothing to infer from is *type annotations needed* about a
/// generated file. Naming the box was right while every channel was one; since
/// [ADR-157](../../../docs/specification/adr/adr-157.md) it can be a
/// `Thrown<E>`, and `?` does not convert one into the other.
#[test]
fn a_branch_travels_in_the_functions_own_channel() {
    let source = format!(
        "{OWN_ERROR}\n\
         fn main() throws {{\n\
         \x20   let pair = overlap {{\n\
         \x20       load(\"c\".to_string())\n\
         \x20       load(\"d\".to_string())\n\
         \x20   }}\n\
         \x20   println(f\"{{pair.0}} {{pair.1}}\")\n\
         }}\n"
    );
    let rust = lowered(&source);
    assert!(
        rust.contains("Ok::<_, nikaia_std::error::Thrown<LoadError>>("),
        "{rust}"
    );
    assert!(
        !rust.contains("Ok::<_, Box<dyn std::error::Error>>("),
        "the box is not this function's channel:\n{rust}"
    );
    assert_eq!(output("joining-own-channel", &source), "c d");
}

/// **And a library's channel, which travels bare**
/// ([ADR-159](../../../docs/specification/adr/adr-159.md) D2).
///
/// The same defect from the other side: the `?` on a branch had to convert a
/// box into an `io::IoError`, which it cannot.
#[test]
fn a_branch_travels_in_a_librarys_channel_too() {
    let source = "use std::fs\n\
                  \n\
                  fn main() throws {\n\
                  \x20   let r = overlap {\n\
                  \x20       fs::read_to_string(\"eins.txt\")\n\
                  \x20       fs::read_to_string(\"zwei.txt\")\n\
                  \x20   }\n\
                  \x20   println(f\"{r.0} {r.1}\")\n\
                  }\n";
    let rust = lowered(source);
    assert!(rust.contains("Ok::<_, io::IoError>("), "{rust}");
}

/// **D5 still holds, and this is the failure ADR-115 is about.**
///
/// Two branches fail; the first in **written** order is the block's error and
/// the second is dropped. That is
/// [ADR-050](../../../docs/specification/adr/adr-050.md) D5 and it is correct —
/// what [ADR-115](../../../docs/specification/adr/adr-115.md) adds is keeping
/// the other one, which is not built. The assertion is here so that the day it
/// is, this is the line that has to change.
///
/// Written with the failure leaving `main`, because
/// `overlap { … } catch { … }` — ADR-115 D4's own form — does not lower:
/// `docs/open-work.md` §1 carries it with this program as its reproducer.
#[test]
fn the_first_failure_in_written_order_is_still_the_blocks() {
    let source = format!(
        "{OWN_ERROR}\n\
         fn main() throws {{\n\
         \x20   let pair = overlap {{\n\
         \x20       load(\"a\".to_string())\n\
         \x20       load(\"b\".to_string())\n\
         \x20   }}\n\
         \x20   println(f\"{{pair.0}} {{pair.1}}\")\n\
         }}\n"
    );
    let rust = lowered(&source);
    let dir = common::scratch_dir("joining-first-failure");
    let file = dir.join("main.rs");
    std::fs::write(&file, &rust).expect("write the Rust");
    let binary = dir.join("program");
    let out = common::compile(&file, &["-o", binary.to_str().expect("utf-8 path")]);
    assert!(
        out.status.success(),
        "it compiles:\n{}\n--- the Rust ---\n{rust}",
        String::from_utf8_lossy(&out.stderr)
    );
    let ran = std::process::Command::new(&binary)
        .output()
        .expect("the program runs");
    let said = String::from_utf8_lossy(&ran.stderr).into_owned();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(!ran.status.success(), "a failing block fails the program");
    assert!(
        said.contains("a is missing"),
        "the first branch in written order is the block's failure:\n{said}"
    );
    // **And the second failure is nowhere**, which is the whole of ADR-115.
    assert!(!said.contains("is broken"), "{said}");
}
