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

use nikaia::ast::Item;
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
        "fn route(path: ref String, handler: fn(Request) -> Response) { }\n",
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
        "fn(ref Stats)",
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
        !fits("fn(i64)", "fn(ref String)"),
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
    // With `sync`, the plain closure: the `Result` is the whole of what the
    // word adds.
    let plain = lowered("fn load(reader: fn(Path) -> Bytes sync throws) { }\nfn main() { }\n");
    assert!(
        plain.contains("impl Fn(Path) -> Result<Bytes, Box<dyn std::error::Error>>"),
        "{plain}"
    );
    // Without it, the same `Result` is what the **future** hands back
    // ([ADR-122](../../../docs/specification/adr/adr-122.md) D1).
    let future = lowered("fn load(reader: fn(Path) -> Bytes throws) { }\nfn main() { }\n");
    assert!(
        future.contains(
            "Pin<Box<dyn std::future::Future<Output = Result<Bytes, Box<dyn std::error::Error>>>>>"
        ),
        "{future}"
    );
    let nothing = lowered("fn attempt(step: fn() sync throws) { }\nfn main() { }\n");
    assert!(
        nothing.contains("impl Fn() -> Result<(), Box<dyn std::error::Error>>"),
        "{nothing}"
    );
}

/// **A lambda that pauses is an ordinary program now**
/// ([ADR-122](../../../docs/specification/adr/adr-122.md) D2), where it used to
/// be refused at the build, because the lowering had no shape for one. D1 *is*
/// the shape — a closure returning a boxed future — so there is nothing left to
/// refuse.
///
/// **The reason D1 gave for choosing that shape was false**: Rust's `async`
/// closure is stable, `impl AsyncFn(A) -> R` costs 1.37 ns/call against the
/// box's 11.99, and whether D1's one-spelling coherence is worth the difference
/// is open ([ADR-187](../../../docs/specification/adr/adr-187.md) D1, D3). What
/// this test asserts — that the program lowers rather than being refused — is
/// D2 and does not depend on which shape wins.
#[test]
fn a_lambda_that_pauses_fits_a_parameter_that_allows_pausing() {
    let source = "use std::io\n\nfn run(f: fn() -> String) -> String { return f() }\n\
                  fn main() { let said = run(fn() { return io::read_to_string() })\n\
                  \x20   println(f\"{said}\") }\n";
    // It lowers, which is what "refused at the build" meant: the emitter used
    // to return an error here rather than write anything.
    let rust = lowered(source);
    assert!(
        rust.contains("|| Box::pin(async move {"),
        "the lambda is a closure returning a future: {rust}"
    );
    assert!(
        rust.contains("io::read_to_string().await"),
        "and its body may await inside it: {rust}"
    );
}

/// **`std`'s own entries are untouched** (D3), and this is the corpus case that
/// said so out loud: `and_modify` is a hand-written description of a **Rust**
/// signature, which takes a plain closure whatever the ledger's `sync` column
/// says. Writing the future shape for one produced
/// *expected `()`, found `Pin<Box<…>>`* against `examples/access-log.nika`.
///
/// So the shape is written only where the signature was **declared here**.
#[test]
fn a_std_entrys_lambda_keeps_its_plain_closure() {
    let rust = lowered(
        "fn main() {\n\
         \x20   let xs = Vec()\n\
         \x20   let doubled = xs.map fn(n) { n * 2 }\n\
         \x20   println(f\"{doubled.len()}\")\n\
         }\n",
    );
    assert!(!rust.contains("Box::pin"), "{rust}");
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

/// **`NK2206`: a lambda that pauses, handed to a `fn() sync`** (D2).
///
/// A type that says `sync` is the assertion
/// [ADR-027](../../../docs/specification/adr/adr-027.md) makes about a
/// declaration, made about somebody else's code: a caller who wrote it has
/// promised their own callers something, and a handler that pauses takes the
/// promise away without saying so.
#[test]
fn a_pausing_lambda_handed_to_a_sync_type_is_refused() {
    let refused = findings(
        "use std::io\n\nfn on_tick(handler: fn() sync) { }\n\
         fn main() { on_tick(fn() { let t = io::read_to_string() }) }\n",
    );
    let about = refused
        .iter()
        .find(|f| f.code == "NK2206")
        .unwrap_or_else(|| panic!("{refused:#?}"));
    assert!(about.message.contains("`sync`"), "{}", about.message);

    // …and the direction that is allowed is allowed: a lambda that does less
    // fits a type that allows more.
    let fine = findings(
        "fn on_tick(handler: fn() sync) { }\nfn main() { on_tick(fn() { let n = 1 + 1 }) }\n",
    );
    assert!(fine.is_empty(), "{fine:#?}");
}

/// **`NK2606`: a lambda that fails, handed to a type that declares none** (D2).
///
/// And it is the **whole** message: the function *around* the lambda is not the
/// one that has to answer for a failure the type refuses, so `NK2605` — *this
/// function can fail because …* — does not stand beside it. Where the type
/// **does** say `throws`, the failure travels to the caller by
/// [ADR-029](../../../docs/specification/adr/adr-029.md) D3 and `NK2605` is
/// right again.
#[test]
fn a_failing_lambda_handed_to_a_type_without_throws_is_refused() {
    const ERROR: &str = "enum E { Bad }\n\
                         impl Error for E { fn message(ref self) -> String { return \"bad\".to_string() } }\n\
                         fn risky() throws { throw E::Bad }\n";

    let refused = findings(&format!(
        "{ERROR}fn attempt(step: fn()) {{ }}\nfn main() {{ attempt(fn() {{ risky() }}) }}\n"
    ));
    assert!(refused.iter().any(|f| f.code == "NK2606"), "{refused:#?}");
    assert!(
        !refused.iter().any(|f| f.code == "NK2605"),
        "one mistake, one message: {refused:#?}"
    );

    let allowed = findings(&format!(
        "{ERROR}fn attempt(step: fn() throws) {{ }}\nfn main() {{ attempt(fn() {{ risky() }}) }}\n"
    ));
    assert!(!allowed.iter().any(|f| f.code == "NK2606"), "{allowed:#?}");
    assert!(
        allowed.iter().any(|f| f.code == "NK2605"),
        "the failure does reach `main` here: {allowed:#?}"
    );
}

/// **A free call resolves its callee before it walks its arguments**, which is
/// [ADR-029](../../../docs/specification/adr/adr-029.md)'s ordering — the
/// *method* path was given it when that record landed and this one was not.
///
/// It is what makes the two refusals above possible at all: a lambda's
/// promises are read off the parameter's type, and the type is not in hand
/// until the callee is. It also types the lambda's **parameters**, which is
/// what `hand(fn(n) { … })` had never had.
#[test]
fn a_free_calls_lambda_is_typed_from_the_signature() {
    // Nothing is refused, and that is the claim: the walk reaches the body
    // through the door that carries types rather than the one that does not.
    let source = "fn twice(x: i64, f: fn(i64) -> i64) -> i64 { return f(f(x)) }\n\
                  fn main() { println(f\"{twice(2, fn(n) { return n * 3 })}\") }\n";
    assert!(findings(source).is_empty(), "{:#?}", findings(source));
}

/// **The type decides, and nothing inferred stands behind it**
/// ([ADR-122](../../../docs/specification/adr/adr-122.md) D1).
///
/// This test used to assert the opposite, and the record it asserted is the one
/// ADR-122 answers: [ADR-102](../../../docs/specification/adr/adr-102.md) D3
/// gave a **run** parameter `sync = "from(f)"` — *the lambda decides* — on
/// [ADR-029](../../../docs/specification/adr/adr-029.md) D3's reasoning that
/// the lambda's body is counted in the caller. That reasoning holds while the
/// lambda is a plain closure. It stops holding the moment the parameter's type
/// may pause, because then the lambda is a **future the callee awaits**, and a
/// function that awaits is `async` whatever its caller handed over.
///
/// So the column follows the type: `sync` on the parameter and the callee keeps
/// its claim; nothing on it and the callee pauses, run or kept.
#[test]
fn the_parameters_type_decides_whether_the_callee_pauses() {
    let sync_of = |source: &str, of: &str| {
        let parsed = parse_to_ast(source).expect("the source parses");
        Ledger::infer(&parsed).functions[of].sync.clone()
    };
    assert!(
        sync_of(
            "fn twice(x: i64, f: fn(i64) -> i64 sync) -> i64 { return f(f(x)) }\n",
            "twice"
        )
        .is_sync(),
        "a plain closure's call adds nothing"
    );
    assert!(
        !sync_of(
            "fn twice(x: i64, f: fn(i64) -> i64) -> i64 { return f(f(x)) }\n",
            "twice"
        )
        .is_sync(),
        "a call to a parameter that may pause is an await, and this function does it"
    );
    // …and the two lowerings say the same thing, which is the half a reader
    // sees: one is a function, the other is an `async fn` whose call is awaited.
    let plain = lowered(
        "fn twice(x: i64, f: fn(i64) -> i64 sync) -> i64 { return f(f(x)) }\n\
         fn main() { println(f\"{twice(2, fn(n) { return n * 3 })}\") }\n",
    );
    assert!(plain.contains("fn twice("), "{plain}");
    assert!(!plain.contains("async fn twice("), "{plain}");
    let future = lowered(
        "fn twice(x: i64, f: fn(i64) -> i64) -> i64 { return f(f(x)) }\n\
         fn main() { println(f\"{twice(2, fn(n) { return n * 3 })}\") }\n",
    );
    assert!(future.contains("async fn twice("), "{future}");
    assert!(future.contains("f(f(x).await).await"), "{future}");
}

/// **A kept one is answered from the type** (D3), and a body that stores *and*
/// runs gets the kept answer, which is the safe one and what ADR-102 §4 says it
/// gets.
#[test]
fn a_kept_parameter_is_answered_from_the_type() {
    let sync_of = |source: &str, of: &str| {
        let parsed = parse_to_ast(source).expect("the source parses");
        Ledger::infer(&parsed).functions[of].sync.clone()
    };
    // Stored and run, and the type allows pausing: the claim is off.
    assert!(
        !sync_of(
            "fn keep(g: fn()) { }\nfn both(f: fn()) { keep(f)\n    f() }\n",
            "both"
        )
        .is_sync(),
        "a wrapper that stores and runs gets the kept answer"
    );
    // The same body, with a type that says the code never pauses.
    assert!(
        sync_of(
            "fn keep(g: fn() sync) { }\nfn both(f: fn() sync) { keep(f)\n    f() }\n",
            "both"
        )
        .is_sync(),
        "a kept `fn() sync` cannot pause, so neither does this"
    );
    // And a body that only hands it on keeps its own answer: storing code is
    // not running it.
    assert!(
        sync_of(
            "fn keep(g: fn()) { }\nfn hold(f: fn()) { keep(f) }\n",
            "hold"
        )
        .is_sync(),
        "handing it on is not running it"
    );
}

/// **A promise is still taken away by anything else the body does.** `from` is
/// *this call adds no pausing of its own*, not *this call cannot pause*.
#[test]
fn a_body_that_also_pauses_keeps_no_claim() {
    let parsed = parse_to_ast(
        "use std::io\n\nfn twice(x: i64, f: fn(i64) -> i64) -> i64 { let t = io::read_to_string()\n\
         \x20   return f(x) }\n",
    )
    .expect("the source parses");
    assert!(!Ledger::infer(&parsed).functions["twice"].sync.is_sync());
}

/// **The trailing words are greedy**, which settles the one ambiguity D1 does
/// not name: in `fn make() -> fn(i64) -> i64 sync` the `sync` belongs to the
/// *result type*.
///
/// **And the declaration's own pre-arrow form is gone**
/// ([ADR-140](../../../docs/specification/adr/adr-140.md) D4), which is what
/// D1's note used to point at as the way to mean the other thing. Nothing can
/// write that shape today — a function type in a **result** is `NK1142` until
/// D5's boxed lowering exists — so what it costs is a spelling for a program
/// that does not compile, and D5 is the record that owes one.
#[test]
fn a_trailing_sync_belongs_to_the_type_it_follows() {
    let parsed = parse_to_ast("fn make() -> fn(i64) -> i64 sync { }\n").expect("the source parses");
    // **The AST and not the ledger**, which is what this test used to ask and
    // could not have answered: `sync` is *inferred* from the body, and an empty
    // body pauses at nothing — so the ledger says `sync` whichever of the two
    // the word attached to, and the old assertion held for the wrong reason.
    let Item::Fn {
        is_sync, ret_type, ..
    } = &parsed.program.items[0].node
    else {
        panic!("a function");
    };
    assert!(!is_sync, "the trailing `sync` is not the declaration's");
    let code = ret_type
        .as_ref()
        .and_then(|t| t.code.as_ref())
        .expect("the result is a function type");
    assert!(code.is_sync, "it is the result type's");
}

/// **The pre-arrow form is refused, and the message names the order**
/// ([ADR-140](../../../docs/specification/adr/adr-140.md) D4).
#[test]
fn a_promise_before_the_arrow_is_refused() {
    let said = format!(
        "{:#}",
        parse_to_ast("fn load() throws -> String { return \"a\".to_string() }")
            .expect_err("the pre-arrow form is a parse error")
    );
    assert!(said.contains("stand after the result type"), "{said}");
    assert!(said.contains("fn f() -> String throws"), "{said}");
}

/// **A declaration with no result type writes the word where it always did**,
/// because there is nothing for it to be before or after — which is why the
/// refusal's cut sits after the arrow and not after the word.
#[test]
fn a_promise_with_no_result_type_is_untouched() {
    let parsed = parse_to_ast("fn tick() sync { }\n").expect("it parses");
    assert!(Ledger::infer(&parsed).functions["tick"].sync.is_sync());
}
