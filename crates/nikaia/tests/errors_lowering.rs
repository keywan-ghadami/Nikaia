//! Kap 7.1: how a program declares an error, raises one, and catches it.
//!
//! Until this, `throws` and `catch` existed and nothing could *produce* an
//! error: `throw` was in the specification exactly once, defined nowhere, and
//! absent from the parser ([ADR-023](../../../docs/specification/adr/adr-023.md)
//! §1). A program could propagate what `std` handed it and never raise one of
//! its own.

mod common;

use nikaia::contracts::Ledger;
use nikaia::emit::{emit_program, Profile};
use nikaia::parser::parse_to_ast;

/// The ledger this source produces, rendered the way it is committed.
fn ledger_for(source: &str) -> String {
    Ledger::infer(&parse_to_ast(source).expect("the source parses")).render()
}

fn emit(source: &str) -> String {
    let parsed = parse_to_ast(source).expect("the source parses");
    emit_program(&parsed, Profile::Advanced)
        .expect("the source lowers")
        .rust
}

/// Kap 4.7: a trait impl names the trait, and the `for` is what tells it from
/// the inherent form. Before this the AST had no room for the name at all.
#[test]
fn an_impl_may_name_a_trait() {
    let rust = emit(
        r#"
        struct User { name: String }
        impl Summarize for User {
            fn summary(&self) -> String { return self.name }
        }
        "#,
    );
    assert!(
        rust.contains("impl Summarize for User {"),
        "expected a trait impl, got:\n{rust}"
    );
}

/// The inherent form still lowers the way it did - `impl User`, no `for`.
#[test]
fn an_inherent_impl_is_unchanged() {
    let rust = emit(
        r#"
        struct User { name: String }
        impl User {
            fn shout(&self) -> String { return self.name }
        }
        "#,
    );
    assert!(rust.contains("impl User {"), "{rust}");
    assert!(!rust.contains(" for User"), "{rust}");
}

/// Kap 7.1: `Error` is the one trait the compiler reads rather than relays. The
/// author writes `message`; being `Display`, and being an error at all, are what
/// the failure channel needs and neither is a decision they make.
#[test]
fn an_error_impl_brings_what_the_failure_channel_needs() {
    let rust = emit(
        r#"
        enum ConfigError { NotFound }
        impl Error for ConfigError {
            fn message(&self) -> String { return "no config" }
        }
        "#,
    );
    assert!(rust.contains("impl ConfigError {"), "{rust}");
    assert!(
        rust.contains("impl std::fmt::Display for ConfigError"),
        "the message has to reach `Display`:\n{rust}"
    );
    assert!(
        rust.contains("f.write_str(&self.message())"),
        "`Display` is written from `message`, not beside it:\n{rust}"
    );
    assert!(
        rust.contains("impl std::error::Error for ConfigError {}"),
        "a value gets into `Box<dyn Error>` by being one:\n{rust}"
    );
}

/// ADR-023 D2: `throw` leaves the function with the value in the failure
/// channel. `Box::new` is the emitter's to write - the language has one way to
/// raise an error, not one way plus a conversion.
#[test]
fn throw_leaves_the_function() {
    let rust = emit(
        r#"
        enum ConfigError { NotFound }
        fn load() throws -> i64 {
            throw ConfigError::NotFound
        }
        "#,
    );
    assert!(
        rust.contains(r#"return Err(nikaia_std::error::raise(ConfigError::NotFound, "load"))"#),
        "{rust}"
    );
}

/// ADR-023 D8: there is no postfix `?`. It parsed until now, which meant a
/// program could still write the operator the decision removed.
#[test]
fn there_is_no_postfix_question_mark() {
    let parsed = parse_to_ast("fn main() throws { let x = load()? }");
    assert!(
        parsed.is_err(),
        "`?` should no longer parse, but it did: {parsed:?}"
    );
}

/// `??` is the null-coalescing operator of Kap 3.5 and keeps working - removing
/// the postfix `?` must not take its first character with it.
#[test]
fn coalescing_still_parses() {
    let rust = emit(r#"fn main() { let p = cli::args().nth(1) ?? "x" }"#);
    assert!(rust.contains("unwrap_or_else"), "{rust}");
}

/// The whole chapter in one program, compiled and run: an error type with a
/// payload, raised, caught, and its message printed.
#[test]
fn an_error_is_declared_raised_caught_and_printed() {
    let source = r#"
        enum ConfigError { NotFound(String) }

        impl Error for ConfigError {
            fn message(&self) -> String {
                match self {
                    ConfigError::NotFound(p) => { return f"no config at {p}" }
                }
            }
        }

        fn load(path: String) throws -> String {
            throw ConfigError::NotFound(path)
        }

        fn main() {
            let text = load("app.conf".to_string()) catch {
                println(f"{error}")
                return
            }
            println(f"{text}")
        }
    "#;
    let rust = emit(source);
    assert!(
        rust.contains("return Err(nikaia_std::error::raise("),
        "{rust}"
    );
    assert!(
        rust.contains("impl std::error::Error for ConfigError"),
        "{rust}"
    );

    let dir = common::scratch_dir("errors");
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
    let out = String::from_utf8_lossy(&run.stdout);
    assert_eq!(out.trim(), "no config at app.conf", "stdout was: {out:?}");
}

// --- the ledger ------------------------------------------------------------

/// ADR-023 D1: `throws` in the source says *that* a function fails; the ledger
/// says with what, inferred over the call graph.
#[test]
fn the_ledger_names_the_errors_a_function_throws() {
    let ledger = ledger_for(
        r#"
        enum ConfigError { NotFound }
        fn load() throws -> i64 { throw ConfigError::NotFound }
        "#,
    );
    assert!(
        ledger.contains(r#"throws = ["ConfigError"]"#),
        "expected the error named, got:\n{ledger}"
    );
}

/// Nothing marks a failing call (D8), so propagation is what a call does - and
/// the set grows along the call graph without anyone writing it down.
#[test]
fn an_error_set_grows_through_a_caller() {
    let ledger = ledger_for(
        r#"
        enum ConfigError { NotFound }
        enum NetError { Timeout }
        fn load() throws -> i64 { throw ConfigError::NotFound }
        fn fetch() throws -> i64 { throw NetError::Timeout }
        fn both() throws -> i64 { let a = load() return fetch() }
        "#,
    );
    let both = ledger
        .split("[fn.\"both\"]")
        .nth(1)
        .expect("an entry for `both`");
    assert!(
        both.contains(r#"throws = ["ConfigError", "NetError"]"#),
        "both callees' errors should reach it:\n{ledger}"
    );
}

/// Mutual recursion terminates because the sets only grow and the names are
/// finite - the same reason `sync`'s greatest fixpoint terminates going the
/// other way.
#[test]
fn mutual_recursion_settles() {
    let ledger = ledger_for(
        r#"
        enum E { X }
        fn ping(n: i32) throws -> i32 { if n == 0 { throw E::X } return pong(n - 1) }
        fn pong(n: i32) throws -> i32 { return ping(n - 1) }
        "#,
    );
    let pong = ledger
        .split("[fn.\"pong\"]")
        .nth(1)
        .expect("an entry for `pong`");
    assert!(
        pong.contains(r#"throws = ["E"]"#),
        "`pong` reaches `ping`'s error:\n{ledger}"
    );
}

/// A failure the compiler cannot name is `?`, ADR-024 D1's absence of a claim -
/// never silence, because reading "I cannot see it" as "it does not fail" is
/// the direction ADR-010 D1 calls a vulnerability generator.
#[test]
fn what_cannot_be_named_is_a_question_mark() {
    let ledger = ledger_for(r#"fn reads() throws -> String { return io::read_to_string() }"#);
    assert!(
        ledger.contains(r#"throws = ["?"]"#),
        "`std`'s failures have no Nikaia name yet:\n{ledger}"
    );
}

/// A declared `throws` never loses its entry. The declaration is a promise a
/// caller already relies on, and inference is here to say more than it.
#[test]
fn a_declared_throws_keeps_its_entry() {
    let ledger = ledger_for(r#"fn maybe() throws -> i64 { return 1 }"#);
    assert!(ledger.contains(r#"throws = ["?"]"#), "{ledger}");
}

/// A function that cannot fail says nothing, because the file says only what is
/// true (13.5).
#[test]
fn a_function_that_cannot_fail_has_no_entry() {
    let ledger = ledger_for(r#"fn pure(n: i64) -> i64 { return n }"#);
    // The header explains the key, so look for the key being *set*.
    assert!(!ledger.contains("throws = "), "{ledger}");
}

// --- the site, and the trace ------------------------------------------------

/// ADR-023 D6: an error knows where it was raised, and the compiler wrote that
/// down rather than the author. `raise` is what carries it.
#[test]
fn a_throw_carries_the_site_it_came_from() {
    let rust = emit(
        r#"
        enum E { X }
        fn load() throws -> i64 { throw E::X }
        "#,
    );
    assert!(
        rust.contains(r#"nikaia_std::error::raise(E::X, "load")"#),
        "the raise site should be in the call:\n{rust}"
    );
}

/// The whole of Kap 7.1's reporting rule, compiled and run: `{error}` is the
/// message and nothing else, `error.full()` adds the site, and the trace is
/// absent unless the program asked - with its absence stated rather than left
/// to be guessed at.
#[test]
fn short_is_safe_and_full_is_asked_for() {
    let source = r#"
        enum ConfigError { NotFound(String) }

        impl Error for ConfigError {
            fn message(&self) -> String {
                match self {
                    ConfigError::NotFound(p) => { return f"no config at {p}" }
                }
            }
        }

        fn load(path: String) throws -> String {
            throw ConfigError::NotFound(path)
        }

        fn main() {
            let text = load("app.conf".to_string()) catch {
                println(f"short: {error}")
                println(f"full: {error.full()}")
                return
            }
            println(text)
        }
    "#;
    let rust = emit(source);
    let dir = common::scratch_dir("trace");
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
        .env_remove("NIKAIA_TRACE")
        .output()
        .expect("run the program");
    let out = String::from_utf8_lossy(&run.stdout);

    // The short form is the message the author wrote, and nothing else. It is
    // what a generic 500 may carry (ADR-018).
    assert!(
        out.contains("short: no config at app.conf"),
        "stdout was:\n{out}"
    );
    // The full form adds where it came from...
    assert!(out.contains("raised at load"), "stdout was:\n{out}");
    // ...and says a trace was not captured, rather than leaving a reader to
    // wonder whether one was lost.
    assert!(out.contains("NIKAIA_TRACE=1"), "stdout was:\n{out}");

    // Asked for, there is one - and `NIKAIA_TRACE` alone has to be enough.
    // `RUST_BACKTRACE` is cleared deliberately: leaving it to the environment
    // is what once let a `Backtrace::capture()` that needs it pass locally and
    // fail in CI.
    let traced = std::process::Command::new(&binary)
        .env("NIKAIA_TRACE", "1")
        .env_remove("RUST_BACKTRACE")
        .env_remove("RUST_LIB_BACKTRACE")
        .output()
        .expect("run the program with tracing on");
    let traced = String::from_utf8_lossy(&traced.stdout);
    assert!(
        !traced.contains("NIKAIA_TRACE=1"),
        "with the switch on there should be a trace:\n{traced}"
    );
}
