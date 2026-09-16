//! A function type says what a handler may do
//! ([ADR-102](../../../docs/specification/adr/adr-102.md) D1 and D2).
//!
//! A parameter could not be a function. The type grammar had a name, a view,
//! type arguments and `?`, so `fn route(path: &str, handler: fn(Request) ->
//! Response)` was a parse error, and the eight `std` entries that take a lambda
//! were written straight into the ledger where the grammar never saw them — a
//! second author could not write `route`, `retry`, `sort_by` or a panic hook.
//!
//! **The type carries two promises and no more** (D2), and their defaults are
//! the language's: without `sync` the code may pause, without `throws` it
//! cannot fail. That is the reading a *declaration* already has, applied to a
//! type — which is why it needs no words of its own.

mod common;

use std::process::Command;

use nikaia::contracts::ty::Ty;
use nikaia::contracts::{Ledger, STD};
use nikaia::emit::{emit_program, Build};
use nikaia::parser::parse_to_ast;

fn findings(source: &str) -> Vec<nikaia::check::Finding> {
    let parsed = parse_to_ast(source).expect("the source parses");
    let own = Ledger::infer(&parsed);
    let library = Ledger::parse(STD).expect("std ships a ledger");
    nikaia::check::check_program(&parsed, &own, &library, &std::collections::BTreeSet::new())
        .findings
}

fn lowered(source: &str) -> String {
    let parsed = parse_to_ast(source).expect("the source parses");
    emit_program(&parsed, Build::default())
        .expect("the source lowers")
        .rust
}

fn signature(source: &str, of: &str) -> String {
    let parsed = parse_to_ast(source).expect("the source parses");
    Ledger::infer(&parsed).functions[of]
        .signature
        .as_ref()
        .expect("a signature")
        .text()
}

/// Compile the lowering and run it, because "a closure argument" is a claim
/// about the language below.
fn ran(purpose: &str, source: &str) -> String {
    let rust = lowered(source);
    let dir = common::scratch_dir(&format!("function-type-{purpose}"));
    let path = dir.join("program.rs");
    std::fs::write(&path, &rust).expect("write the Rust");
    let binary = dir.join("program");
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
        "{purpose} did not compile:\n{}\n--- emitted ---\n{rust}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    let out = Command::new(&binary).output().expect("run it");
    assert!(
        out.status.success(),
        "{purpose} failed:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let printed = String::from_utf8_lossy(&out.stdout).to_string();
    std::fs::remove_dir_all(&dir).ok();
    printed
}

/// **D1's three forms**, exactly as the record writes them: parameters in
/// parentheses, `-> R` where there is a result, and `sync` and `throws` after
/// it, in the positions a declaration puts them.
#[test]
fn the_records_three_forms_parse() {
    for source in [
        "fn route(path: &str, handler: fn(Request) -> Response) { }\n",
        "fn on_tick(handler: fn() sync) { }\n",
        "fn load(reader: fn(Path) -> Bytes throws) { }\n",
    ] {
        parse_to_ast(source).unwrap_or_else(|e| panic!("{source}\n{e}"));
    }
}

/// **`fn` is a shape and not a name**, which is what a tuple already is: its
/// parameters go where a named type's arguments go, so nothing that walks a
/// type has to know about it. Reading `name` there reported that nothing
/// declares a type called `fn`.
#[test]
fn a_function_type_is_not_an_undeclared_type_name() {
    let refused = findings("fn twice(x: i64, f: fn(i64) -> i64) -> i64 { return f(f(x)) }\n");
    assert!(!refused.iter().any(|f| f.code == "NK1135"), "{refused:#?}");
}

/// **The ledger writes it and reads it back** — which is the half that makes a
/// package able to publish `route`, since a consumer builds against contracts
/// rather than guesses (Part III 13.5).
#[test]
fn the_ledger_carries_the_type_and_its_two_promises() {
    assert_eq!(
        signature(
            "fn twice(x: i64, f: fn(i64) -> i64 sync) -> i64 { return f(f(x)) }\n",
            "twice"
        ),
        "(x: i64, f: fn(i64) -> i64 sync) -> i64"
    );
    for written in [
        "fn()",
        "fn(&Stats)",
        "fn(Request) -> Response",
        "fn() sync",
        "fn(Path) -> Bytes throws",
        "fn(i64) -> i64 sync throws",
        // A function type may stand inside another one, which is why the
        // closing parenthesis is counted rather than looked for at the end.
        "fn(fn(i64)) -> i64",
    ] {
        assert_eq!(Ty::parse(written).text(), written);
    }
}

/// **A lambda that does less fits a type that allows more** (D2), and the other
/// direction does not: a type that says `sync` is the same assertion
/// [ADR-027](../../../docs/specification/adr/adr-027.md) makes about a
/// declaration, made about somebody else's code.
#[test]
fn a_lambda_that_does_less_fits_a_type_that_allows_more() {
    let fits = |a: &str, b: &str| Ty::parse(a).fits(&Ty::parse(b));
    assert!(
        fits("fn() sync", "fn()"),
        "never pausing goes where pausing is allowed"
    );
    assert!(
        !fits("fn()", "fn() sync"),
        "the assertion is not given away"
    );
    assert!(
        fits("fn()", "fn() throws"),
        "cannot fail goes where failing is allowed"
    );
    assert!(!fits("fn() throws", "fn()"), "and not the other way");
    assert!(fits("fn(i64) -> i64", "fn(i64) -> i64"));
    assert!(
        !fits("fn(i64)", "fn(&str)"),
        "the parameters still have to match"
    );
}

/// **D5's run case**: a closure argument, which is what `std`'s own `map` and
/// `access` take and what costs nothing.
#[test]
fn a_run_parameter_lowers_to_a_closure_argument() {
    let printed = ran(
        "twice",
        "fn twice(x: i64, f: fn(i64) -> i64) -> i64 { return f(f(x)) }\n\
         fn main() { println(f\"{twice(2, fn(n) { return n * 3 })}\") }\n",
    );
    assert_eq!(printed.trim(), "18");
    let rust = lowered("fn on_tick(handler: fn() sync) { }\nfn main() { }\n");
    assert!(rust.contains("fn on_tick(handler: impl Fn())"), "{rust}");
}

/// **And `throws` puts the same `Result` on the closure's result** that a
/// `throws` function's own declaration puts on its (Part I 7.1), which is what
/// makes a lambda that fails fit it.
#[test]
fn throws_on_the_type_is_the_result_a_throws_function_has() {
    let rust = lowered("fn load(reader: fn(Path) -> Bytes throws) { }\nfn main() { }\n");
    assert!(
        rust.contains("impl Fn(Path) -> Result<Bytes, Box<dyn std::error::Error>>"),
        "{rust}"
    );
    let nothing = lowered("fn attempt(step: fn() throws) { }\nfn main() { }\n");
    assert!(
        nothing.contains("impl Fn() -> Result<(), Box<dyn std::error::Error>>"),
        "{nothing}"
    );
}

/// **`NK1142`: only a parameter yet.** D1 says a function type may stand
/// wherever a type may and D5 says the two cases lower differently — a run
/// parameter is a closure argument, a **kept** one is a boxed closure over a
/// boxed future, and only the first is built. A field written `impl Fn(…)` is
/// not Rust, so the reader would meet the backend's words about a file nobody
/// wrote ([Part III C.1](../../../docs/specification/30-nikaia-tooling.md)).
#[test]
fn a_function_type_outside_a_parameter_is_refused_here() {
    for source in [
        "struct Router { handler: fn(Request) -> Response }\nfn main() { }\n",
        "fn make() -> fn(i64) -> i64 { }\nfn main() { }\n",
        "fn main() { let f: fn(i64) -> i64 = 1 }\n",
    ] {
        let refused = findings(source);
        let about = refused
            .iter()
            .find(|f| f.code == "NK1142")
            .unwrap_or_else(|| panic!("{source}\n{refused:#?}"));
        assert!(
            about
                .help
                .as_deref()
                .is_some_and(|h| h.contains("parameter")),
            "the help names the way through: {:?}",
            about.help
        );
    }
    // …and a parameter is not refused, which is the whole of what is built.
    let fine = findings("fn twice(x: i64, f: fn(i64) -> i64) -> i64 { return f(f(x)) }\n");
    assert!(!fine.iter().any(|f| f.code == "NK1142"), "{fine:#?}");
}

/// **The trailing words are greedy**, which settles the one ambiguity D1 does
/// not name: in `fn make() -> fn(i64) -> i64 sync` the `sync` belongs to the
/// *result type*. A function whose own promise is meant writes it before the
/// arrow, which the declaration grammar accepts already.
#[test]
fn a_trailing_sync_belongs_to_the_type_it_follows() {
    let source = "fn make() sync -> fn(i64) -> i64 { }\n";
    let parsed = parse_to_ast(source).expect("the source parses");
    let ledger = Ledger::infer(&parsed);
    assert!(
        ledger.functions["make"].sync.is_sync(),
        "the `sync` before the arrow is the function's own"
    );
}
