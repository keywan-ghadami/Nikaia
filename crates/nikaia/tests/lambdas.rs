//! The trailing lambda, with its arguments named.
//!
//! Part I 5.3 offers a lambda two spellings of the same form: implicit
//! arguments (`fn { a.id }`) or named ones (`fn(user) { user.id }`). In
//! expression position the parser took both. In *trailing* position - the
//! lambda outside the call's parentheses - it took only the implicit one, so
//! every named example in the specification was read as two expressions: the
//! call without its last argument, and a closure nobody passed anywhere. That
//! type-checks in Rust often enough to be a different program rather than an
//! error, which is why these are compiled and not only compared as text.
//!
//! **The stand-ins.** A trailing lambda's callee is usually something `std`
//! does not have yet - a lock's `access`, `access_all`, `task::scope`. ADR-028
//! D5 keeps an entry out of `std` until a program asks for it, so each test
//! that needs one prepends the Rust it stands for. What is under test is the
//! grammar and the lowering: that the emitted call is a well-formed Rust call
//! whose last argument is the closure.

mod common;

use nikaia::emit::{emit_program, Build};
use nikaia::parser::parse_to_ast;

fn lowered(source: &str) -> String {
    let parsed = parse_to_ast(source).expect("the source parses");
    emit_program(&parsed, Build::default())
        .expect("the source lowers")
        .rust
}

/// Lower, compile the result as a Rust library, and hand the emitted text back.
///
/// `--emit=metadata`: whether the call is well formed is settled by type
/// checking, and there is no reason to pay for code generation to hear it.
fn compiled(purpose: &str, support: &str, source: &str) -> String {
    let rust = lowered(source);
    let dir = common::scratch_dir(purpose);
    let file = dir.join("lowered.rs");
    std::fs::write(&file, format!("{support}\n{rust}")).expect("write the Rust");

    let compiled = common::compile(
        &file,
        &[
            "--crate-type",
            "lib",
            "--emit=metadata",
            "-o",
            dir.join("lowered.rmeta").to_str().expect("utf-8 path"),
        ],
    );
    assert!(
        compiled.status.success(),
        "the lowering of {purpose} does not compile:\n{}\n--- the Rust ---\n{support}\n{rust}",
        String::from_utf8_lossy(&compiled.stderr),
    );

    let _ = std::fs::remove_dir_all(&dir);
    rust
}

/// Part I 5.3's own example: `users.map fn(user) { … }`.
#[test]
fn a_method_takes_a_named_trailing_lambda() {
    let rust = compiled(
        "lambda-named",
        "",
        "use std::io\n\
         \n\
         struct User { id: i64, name: String }\n\
         \n\
         fn ids(users: Vec[User]) -> Vec[i64] {\n\
         \x20   return users.map fn(user) { user.id }\n\
         }\n",
    );
    assert!(rust.contains("users.map(|user| { user.id })"), "{rust}");
}

/// Two named parameters, and a receiver that is not the lambda's only
/// argument: `numbers.reduce(0) fn(acc, item) { … }`.
#[test]
fn a_method_takes_arguments_and_a_named_trailing_lambda() {
    let rust = lowered(
        "fn total(numbers: Vec[i64]) -> i64 { return numbers.reduce(0) fn(acc, item) { acc + item } }",
    );
    assert!(
        rust.contains("numbers.reduce(0, |acc, item| { acc + item })"),
        "{rust}"
    );
}

/// Part II 12.3's single `access`: `account.access fn(to) { … }`.
///
/// The stand-in is what `Locked[T]`'s `access` will be - it hands the body a
/// mutable view of the locked value and gives the lock back at the `}`.
#[test]
fn a_lock_is_accessed_through_a_named_trailing_lambda() {
    let rust = compiled(
        "lambda-access",
        "impl Account {\n\
         \x20   fn access<R>(mut self, body: impl FnOnce(&mut Account) -> R) -> R {\n\
         \x20       body(&mut self)\n\
         \x20   }\n\
         }\n",
        "struct Account { balance: i64 }\n\
         \n\
         fn deposit(account: Account) {\n\
         \x20   account.access fn(to) { to.balance += 100 }\n\
         }\n",
    );
    assert!(
        rust.contains("account.access(|to| { to.balance += 100; })"),
        "{rust}"
    );
}

/// Part II 12.3's `access_all`: a free call with arguments **and** a trailing
/// lambda that names two parameters.
///
/// The free-call equivalent of the `"." name args lambda` rule, and the one
/// that needed the lambda to reach a path as well as a method: `access_all` is
/// a plain function, so nothing before the lambda is a method call at all.
#[test]
fn a_free_call_takes_arguments_and_a_named_trailing_lambda() {
    let rust = compiled(
        "lambda-access-all",
        "fn access_all<R>(\n\
         \x20   mut a: Account,\n\
         \x20   mut b: Account,\n\
         \x20   body: impl FnOnce(&mut Account, &mut Account) -> R,\n\
         ) -> R {\n\
         \x20   body(&mut a, &mut b)\n\
         }\n",
        "struct Account { balance: i64 }\n\
         \n\
         fn transfer(account_a: Account, account_b: Account) {\n\
         \x20   access_all(account_a, account_b) fn(a, b) {\n\
         \x20       let amount = 100\n\
         \x20       a.balance -= amount\n\
         \x20       b.balance += amount\n\
         \x20   }\n\
         }\n",
    );
    assert!(
        rust.contains("access_all(account_a, account_b, |a, b| {"),
        "{rust}"
    );
}

/// Part II 12.7's `task::scope fn(s) { … }`: a trailing lambda on a **path**,
/// with the scope's own `s.spawn fn { … }` inside it.
///
/// Both spellings in one program: the outer lambda names its argument because
/// the scope handle needs a name, the inner one does not need any.
#[test]
fn a_path_takes_a_named_trailing_lambda() {
    let rust = compiled(
        "lambda-scope",
        "mod task {\n\
         \x20   pub struct Scope;\n\
         \x20   impl Scope {\n\
         \x20       pub fn spawn<R>(&self, body: impl FnOnce() -> R) {\n\
         \x20           let _ = body();\n\
         \x20       }\n\
         \x20   }\n\
         \x20   pub fn scope<R>(body: impl FnOnce(&Scope) -> R) -> R {\n\
         \x20       body(&Scope)\n\
         \x20   }\n\
         }\n",
        "fn work(total: i64) {\n\
         \x20   task::scope fn(s) {\n\
         \x20       s.spawn fn { total + 1 }\n\
         \x20   }\n\
         }\n",
    );
    assert!(rust.contains("task::scope(|s| {"), "{rust}");
    assert!(rust.contains("s.spawn(|| { total + 1 })"), "{rust}");
}

/// The implicit form is untouched, in every position that had it.
#[test]
fn the_implicit_trailing_lambda_is_unchanged() {
    let rust = lowered(
        "fn main() {\n\
         \x20   let n = xs.map fn { a.id } .len()\n\
         \x20   let s = Server::new().route(\"/x\") fn { handler(db) }.listen(\":8080\")\n\
         }",
    );
    assert!(rust.contains(".map(|a| { a.id }).len()"), "{rust}");
    assert!(
        rust.contains(r#".route("/x", || { handler(db) }).listen(":8080")"#),
        "{rust}"
    );
}

/// ADR-022's removed `fn: …` still gets its sentence rather than a parse error
/// about a parameter list - from a method and from a path alike.
///
/// The named arm is tried first and starts with the same `fn`, so this is the
/// arm order under test: neither lambda arm can match a colon, and the `fn ":"`
/// arm behind them is what a reader reaches.
#[test]
fn the_expression_lambda_still_says_it_was_removed() {
    for source in [
        "fn main() { let ids = users.map fn: a.id }",
        "fn main() { task::scope fn: a.id }",
    ] {
        let message = format!("{:#}", parse_to_ast(source).expect_err("refused"));
        assert!(message.contains("ADR-022"), "{source}: {message}");
        assert!(message.contains("`fn { … }`"), "{source}: {message}");
    }
}

/// **Not built, and this is where it stands.** Part I 7.2 and ADR-006 D6 write
/// the panic hook as `panic::on_panic fn(info) sync { … }`: the lambda carries
/// an effect annotation, and nothing in the grammar gives a lambda one - a
/// closure's parameter list is followed by its block and by nothing else.
///
/// So the `sync` is not part of the lambda, the lambda is not part of the call,
/// and the line is three statements. Recorded as a test rather than left to be
/// rediscovered: the trailing lambda now takes named arguments everywhere
/// *except* where an effect annotation follows them.
#[test]
fn an_effect_annotation_on_a_lambda_is_still_three_statements() {
    let rust = lowered("fn main() { panic::on_panic fn(info) sync { info } }");
    assert!(rust.contains("panic::on_panic;"), "{rust}");
    assert!(!rust.contains("panic::on_panic(|info|"), "{rust}");
}

/// **`spawn` is a call now, and still does not resolve.** Part I 8.2 writes
/// `spawn fn { … }`; the parser's own `spawn` rule is `spawn "(" expr ")"`, so
/// the keyword form never reached it and was read as the variable `spawn`
/// followed by a closure - `cannot find value 'spawn' in this scope`.
///
/// With a trailing lambda reaching a path, it is one expression: a call to a
/// function named `spawn`, with the lambda as its argument. That is what the
/// source says and what `std` has no entry for, so rustc now says *function*
/// where it said *value*. Nothing here redesigns `spawn`; the rule that would
/// make it a task rather than a call is still `spawn "(" expr ")"`.
#[test]
fn spawn_with_a_named_lambda_is_read_as_a_call() {
    let rust = lowered("fn main() { spawn fn(x) { x } }");
    assert!(rust.contains("spawn(|x| { x })"), "{rust}");
}
