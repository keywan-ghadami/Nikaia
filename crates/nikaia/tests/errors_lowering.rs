//! Kap 7.1: how a program declares an error, raises one, and catches it.
//!
//! Until this, `throws` and `catch` existed and nothing could *produce* an
//! error: `throw` was in the specification exactly once, defined nowhere, and
//! absent from the parser ([ADR-023](../../../docs/specification/adr/adr-023.md)
//! §1). A program could propagate what `std` handed it and never raise one of
//! its own.

mod common;

use nikaia::emit::{emit_program, Profile};
use nikaia::parser::parse_to_ast;

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
        rust.contains("return Err(Box::new(ConfigError::NotFound))"),
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
                    ConfigError::NotFound(p) => { return "no config at {p}" }
                }
            }
        }

        fn load(path: String) throws -> String {
            throw ConfigError::NotFound(path)
        }

        fn main() {
            let text = load("app.conf".to_string()) catch {
                println("{error}")
                return
            }
            println("{text}")
        }
    "#;
    let rust = emit(source);
    assert!(rust.contains("return Err(Box::new("), "{rust}");
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
