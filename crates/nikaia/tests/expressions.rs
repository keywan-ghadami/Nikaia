//! How an expression is written, and what the lowering owes it. All of these
//! came out of `examples/n-body.nika`, which is arithmetic and nothing else.

use nikaia::ast::{Expr, Item, Stmt};
use nikaia::emit::{emit_program, Profile};
use nikaia::parser::parse_to_ast;

fn emit(source: &str) -> String {
    let parsed = parse_to_ast(source).expect("the source parses");
    emit_program(&parsed, Profile::Advanced)
        .expect("the source lowers")
        .rust
}

/// Kap 3.3: `0..n` is what a `for` counts over.
#[test]
fn a_range_is_an_expression() {
    let parsed = parse_to_ast("fn main() { for i in 0..5 { } }").expect("parses");
    let Item::Fn { body, .. } = &parsed.program.items[0].node else {
        panic!("expected a function");
    };
    let Stmt::For { iter, .. } = &body.stmts[0].node else {
        panic!("expected a `for`");
    };
    assert!(
        matches!(
            iter,
            Expr::Range {
                inclusive: false,
                ..
            }
        ),
        "{iter:?}"
    );

    assert!(emit("fn main() { for i in 0..5 { } }").contains("for i in 0..5 {"));
    assert!(emit("fn main() { for i in 0..=5 { } }").contains("for i in 0..=5 {"));
}

/// A range binds looser than the arithmetic in it, which is the reading a loop
/// head wants: `0..n - 1` ends at `n - 1`.
#[test]
fn a_range_binds_looser_than_its_arithmetic() {
    let emitted = emit("fn f(n: i32) { for i in 1..n - 1 { } }");
    assert!(emitted.contains("for i in 1..n - 1 {"), "{emitted}");
}

/// A range over something that has to be asked for its length - the shape every
/// index loop has, and the one `0.` would eat if a float literal were tried
/// first without backtracking.
#[test]
fn a_range_may_end_in_a_call() {
    let emitted = emit("fn f(xs: Vec[i32]) { for i in 0..xs.len() { } }");
    assert!(emitted.contains("for i in 0..xs.len() {"), "{emitted}");
}

/// A group is not a node - the parser drops it, because that is how the tree
/// was written rather than part of it - so a postfix applied to something that
/// binds looser has to get its parentheses back.
///
/// This was silently wrong and often still compiled: `(a as f64).sqrt()` came
/// out as `a as f64.sqrt()`, which parses as a cast to a type nobody named.
#[test]
fn a_postfix_reparenthesises_what_binds_looser() {
    let emitted = emit(
        "fn f(a: i32, b: i32, xs: Vec[i32]) {\n\
         let c = (a as f64).sqrt()\n\
         let d = (a + b).to_string()\n\
         let e = (a - b).abs()\n\
         let g = (a + b) as f64\n\
         }",
    );
    assert!(emitted.contains("(a as f64).sqrt()"), "{emitted}");
    assert!(emitted.contains("(a + b).to_string()"), "{emitted}");
    assert!(emitted.contains("(a - b).abs()"), "{emitted}");
    assert!(emitted.contains("(a + b) as f64"), "{emitted}");
}

/// … and does not add parentheses where the expression already binds tighter.
#[test]
fn a_postfix_leaves_a_plain_receiver_alone() {
    let emitted = emit("fn f(xs: Vec[i32]) { let n = xs.len() let m = xs[0].abs() }");
    assert!(emitted.contains("xs.len()"), "{emitted}");
    assert!(!emitted.contains("(xs)"), "{emitted}");
    assert!(emitted.contains("xs[0].abs()"), "{emitted}");
}

/// A float may carry an exponent, which is how a program about physical
/// quantities is written: `9.54791938424326609e-04` beside a `1.0`, rather
/// than the same number spelled out in zeroes with one of them lost.
#[test]
fn a_float_may_carry_an_exponent() {
    for literal in ["1.5e5", "1.5e-4", "1.5e+4", "2e3", "2e-3", "1.5E-4"] {
        let emitted = emit(&format!("fn main() {{ let a = {literal} }}"));
        assert!(
            emitted.contains(&format!("let a = {literal};")),
            "{literal}: {emitted}"
        );
    }
}

/// The compiler's identifier does not start with a digit, and a number is
/// therefore a number.
///
/// The backend's `ident` accepts a leading digit - a grammar that wants
/// otherwise says so - and until it did, `1.5` parsed as the field `5` of a
/// variable called `1`. It printed back identically, which is why it survived:
/// the emitted text read the same right up until there was more after it.
#[test]
fn a_number_is_not_an_identifier() {
    let parsed = parse_to_ast("fn main() { let a = 1.5 }").expect("parses");
    let Item::Fn { body, .. } = &parsed.program.items[0].node else {
        panic!("expected a function");
    };
    let Stmt::Let { value, .. } = &body.stmts[0].node else {
        panic!("expected a `let`");
    };
    assert!(
        matches!(value, Expr::LitFloat(f) if f == "1.5"),
        "{value:?}"
    );

    // What it looked like when it was one: the field access printed back the
    // same, and the exponent did not.
    assert!(emit("fn main() { let a = 1.5e-4 }").contains("let a = 1.5e-4;"));
}

/// An `if` in statement position is not a value, so a `return` in one of its
/// branches has to stay a `return`.
///
/// The branches of an `if` were emitted as tails unconditionally, which is
/// right for `let x = if c { a } else { b }` and wrong for a guard: the last
/// statement of a branch was written as the branch's *value*, so
/// `if n < k { return counts }` came out as `if n < k { counts }` and the
/// function carried on. Found by `examples/k-nucleotide.nika`, where the types
/// happened to disagree; in a function returning nothing it would have
/// compiled.
#[test]
fn a_return_inside_an_if_statement_still_returns() {
    let emitted = emit(
        "fn f(n: i32, k: i32) -> i32 {\n\
         if n < k { return 0 }\n\
         return n - k\n\
         }",
    );
    assert!(emitted.contains("return 0;"), "{emitted}");

    // …and the case it was right for keeps working: in value position the
    // branches *are* the value.
    let value = emit("fn f(c: bool) -> i32 { let x = if c { 1 } else { 2 } return x }");
    assert!(value.contains("if c { 1 } else { 2 }"), "{value}");
    assert!(!value.contains("return 1"), "{value}");
}

/// The same for an `if` whose branch ends in a `return` inside a `throws`
/// function, where a bare tail would have been wrapped in `Ok` by the caller
/// and a `return` must wrap itself.
#[test]
fn a_return_inside_an_if_wraps_itself_when_the_function_throws() {
    let emitted = emit(
        "fn f(n: i32) -> i32 throws {\n\
         if n < 0 { return 0 }\n\
         return n\n\
         }",
    );
    assert!(emitted.contains("return Ok(0);"), "{emitted}");
}

// --- ADR-022: one lambda form -----------------------------------------------

/// A method may take arguments **and** a trailing lambda, and the chain
/// continues after it.
///
/// This form had no grammar at all: there was a rule for a method with
/// arguments and one for a method with a trailing lambda, and none for both.
#[test]
fn a_method_takes_arguments_and_a_trailing_lambda() {
    let emitted = emit(
        "fn main() {\n\
         let s = Server::new()\n\
         .route(\"/x\") fn { handler(db) }\n\
         .listen(\":8080\")\n\
         }",
    );
    assert!(
        emitted.contains(r#".route("/x", || { handler(db) }).listen(":8080")"#),
        "{emitted}"
    );
}

/// The trailing lambda without arguments still works, and so does a chain
/// after it - a block ends at its `}`, so there is nothing to swallow.
#[test]
fn a_block_lambda_does_not_swallow_the_chain() {
    let emitted = emit("fn main() { let n = xs.map fn { a.id } .len() }");
    assert!(emitted.contains(".map(|a| { a.id }).len()"), "{emitted}");
}

/// `fn: …` is gone, and says so.
///
/// The form was in the specification and in three examples, so a parse error
/// at the colon would be true and useless. What it did wrong is in the message,
/// because that is the part a reader cannot see from the code in front of them.
#[test]
fn the_expression_lambda_says_it_was_removed() {
    let message = format!(
        "{:#}",
        parse_to_ast("fn main() { let ids = users.map fn: a.id }").expect_err("refused")
    );
    assert!(message.contains("ADR-022"), "{message}");
    assert!(message.contains("`fn { … }`"), "{message}");
    assert!(message.contains("inside* the lambda"), "{message}");
}

/// ADR-025 D1: a loop whose step can fail propagates the failure, and this is
/// the line that does it.
///
/// One line inside the loop, and it is the one a Rust programmer writes by
/// hand - which is what keeps the lowering readable and the rule honest: the
/// failure leaves the function the moment it happens, rather than being asked
/// about afterwards.
#[test]
fn a_loop_whose_step_can_fail_unwraps_the_step() {
    let rust = emit(
        "fn count() -> i64 throws {\n\
         \x20   let mut n = 0\n\
         \x20   for line in io::lines() { n += 1 }\n\
         \x20   return n\n\
         }",
    );
    assert!(
        rust.contains("for line in io::lines() {\n        let line = line?;"),
        "the step is not unwrapped:\n{rust}"
    );

    // …and naming the stream first is the same loop (D7).
    let rust = emit(
        "fn count() -> i64 throws {\n\
         \x20   let stream = io::lines()\n\
         \x20   let mut n = 0\n\
         \x20   for line in stream { n += 1 }\n\
         \x20   return n\n\
         }",
    );
    assert!(
        rust.contains("let line = line?;"),
        "naming the stream first hid it:\n{rust}"
    );
}

/// An ordinary loop keeps the shape it always had. The rule above must not
/// leak into every `for` in the language.
#[test]
fn an_ordinary_loop_is_not_touched() {
    let rust = emit("fn main() { for i in 0..5 { } }");
    assert!(rust.contains("for i in 0..5"), "{rust}");
    assert!(!rust.contains("?;"), "{rust}");
}

/// Kap 5.1: the language below has neither named arguments nor defaults, so an
/// option becomes an ordinary parameter - and a call fills in whatever it left
/// out, in the order the *declaration* gives.
#[test]
fn an_option_becomes_a_positional_argument_in_declaration_order() {
    let rust = emit(
        "fn request(url: &str; timeout: i32 = 30, method: &str = \"GET\") -> i32 {\n\
         \x20   return timeout\n\
         }\n\
         fn main() {\n\
         \x20   request(\"a\")\n\
         \x20   request(\"b\"; timeout: 60)\n\
         \x20   request(\"c\"; method: \"POST\", timeout: 5)\n\
         }",
    );

    assert!(
        rust.contains("fn request(url: &str, timeout: i32, method: &str) -> i32"),
        "the options are not parameters:\n{rust}"
    );
    assert!(
        rust.contains(r#"request("a", 30, "GET")"#),
        "a call that names none does not get the defaults:\n{rust}"
    );
    assert!(
        rust.contains(r#"request("b", 60, "GET")"#),
        "a call that names one does not keep the other's default:\n{rust}"
    );
    // Written `method` first, and it is `timeout` that goes first - because
    // only the declaration knows what the order is.
    assert!(
        rust.contains(r#"request("c", 5, "POST")"#),
        "the caller's order was not put into the callee's:\n{rust}"
    );
}

/// …and `std`'s options come from the ledger `std` ships, by the same path.
#[test]
fn a_library_option_is_filled_in_from_the_shipped_ledger() {
    let rust = emit("fn main() throws { fs::write(\"o\", \"x\"; append: true) }");
    assert!(
        rust.contains(r#"fs::write("o", "x", true, true)"#),
        "{rust}"
    );

    let rust = emit("fn main() throws { fs::write(\"o\", \"x\") }");
    assert!(
        rust.contains(r#"fs::write("o", "x", false, true)"#),
        "{rust}"
    );
}

/// Kap 3.3: `while` is a loop, not an identifier.
///
/// It was neither in the parser nor in the AST, so `while n < 5 { … }` read as
/// **three** statements - a variable called `while`, a comparison thrown away,
/// and a bare block - and the compiler said nothing at all. What a reader got
/// was `rustc` complaining about `while;` in a file they had not written, which
/// Part III C.1 calls a bug in this compiler.
#[test]
fn a_while_is_one_statement_and_not_three() {
    let parsed = parse_to_ast(
        "fn main() {\n\
         \x20   let mut n = 0\n\
         \x20   while n < 5 { n += 1 }\n\
         }",
    )
    .expect("parses");
    let Item::Fn { body, .. } = &parsed.program.items[0].node else {
        panic!("expected a function");
    };
    assert_eq!(
        body.stmts.len(),
        2,
        "the `while` did not stay one statement: {:#?}",
        body.stmts
    );
    let Stmt::While { body: inner, .. } = &body.stmts[1].node else {
        panic!("expected a `while`, found {:#?}", body.stmts[1].node);
    };
    assert_eq!(inner.stmts.len(), 1, "the body is the block after it");
}

/// …and it lowers name for name, because the language below spells it the same
/// way (ADR-011 D2).
#[test]
fn a_while_lowers_to_a_while() {
    let rust = emit(
        "fn main() {\n\
         \x20   let mut n = 0\n\
         \x20   while n < 5 { n += 1 }\n\
         }",
    );
    assert!(rust.contains("while n < 5 { n += 1; }"), "{rust}");
}
