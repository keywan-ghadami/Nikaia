//! Running Nikaia while the program is built
//! ([ADR-073](../../../docs/specification/adr/adr-073.md) D5's second stage,
//! bounded by [ADR-075](../../../docs/specification/adr/adr-075.md)).
//!
//! `comptime` has had an evaluator since the word existed and what it knew was
//! an integer literal, a name whose value already folded, a negation and
//! `+ - * / %` — `fold.rs`, 124 lines with no call in it. D5 called that the
//! first stage and wrote the second one as *when Q4 is answered*: a call, and
//! with it the file reading [ADR-072](../../../docs/specification/adr/adr-072.md)
//! waits behind. [ADR-075](../../../docs/specification/adr/adr-075.md) answered
//! Q4, and three records have been waiting on this since
//! (`docs/open-work.md` §2.9).

use nikaia::contracts::{Ledger, STD};
use nikaia::emit::{emit_program, Build};
use nikaia::parser::parse_to_ast;

fn findings(source: &str) -> Vec<nikaia::check::Finding> {
    let parsed = parse_to_ast(source).expect("the source parses");
    let own = Ledger::infer(&parsed);
    let library = Ledger::parse(STD).expect("std's ledger");
    nikaia::check::check(&parsed, &own, &library).findings
}

fn lowered(source: &str) -> String {
    let parsed = parse_to_ast(source).expect("the source parses");
    emit_program(&parsed, Build::default())
        .expect("it lowers")
        .rust
}

/// **A call in an initialiser**, which is the whole of D5's second stage.
#[test]
fn a_call_is_evaluated_while_the_program_is_built() {
    let source = "fn double(n: i64) -> i64 { return n * 2 }\n\
                  comptime ANSWER = double(21)\n\
                  fn main() { println(f\"{ANSWER}\") }\n";
    assert!(findings(source).is_empty(), "{:#?}", findings(source));
    assert!(
        lowered(source).contains("const ANSWER: i32 = 42;"),
        "{}",
        lowered(source)
    );
}

/// **A recursion with a base case**, which needs the `if` that returns out of
/// one arm and falls through the other — the shape a body is written in.
#[test]
fn a_recursion_with_a_base_case_is_evaluated() {
    let source = "fn factorial(n: i64) -> i64 {\n\
                  \x20   if n < 2 { return 1 }\n\
                  \x20   return n * factorial(n - 1)\n\
                  }\n\
                  comptime SIX = factorial(3)\n\
                  fn main() { println(f\"{SIX}\") }\n";
    assert!(findings(source).is_empty(), "{:#?}", findings(source));
    assert!(lowered(source).contains("const SIX: i32 = 6;"));
}

/// **What the name is worth is what was evaluated, not what folded**, which
/// they were the same thing until there was a call. Binding the fold left
/// `comptime ANSWER = double(21)` visible as a name with no value, so the next
/// constant that read it was `NK1127` although the one before it had just been
/// computed.
#[test]
fn a_constant_can_read_the_one_above_it() {
    let source = "fn double(n: i64) -> i64 { return n * 2 }\n\
                  comptime ANSWER = double(21)\n\
                  comptime BIG = ANSWER > 40\n\
                  fn main() { println(f\"{BIG}\") }\n";
    assert!(findings(source).is_empty(), "{:#?}", findings(source));
    assert!(lowered(source).contains("const BIG: bool = true;"));
}

/// **A callee that can pause may not be called** (D1), and the message says
/// which rule and why.
#[test]
fn a_pausing_callee_is_refused() {
    let found: Vec<_> = findings(
        "use std::io\n\nfn slow() -> i64 throws {\n\
         \x20   let t = io::read_to_string()\n\
         \x20   return t.len()\n\
         }\n\
         comptime N = slow()\n",
    )
    .into_iter()
    .filter(|f| f.code == "NK1152")
    .collect();
    assert_eq!(found.len(), 1, "{found:#?}");
    assert!(
        found[0].notes.join(" ").contains("it can pause"),
        "{found:#?}"
    );
}

/// **And one that reaches the world** (D2). `println` is the case the record
/// names as a reasonable thing to want and a later decision with its own
/// reasons — refused here as a consequence of the boundary rather than a
/// judgement about printing.
#[test]
fn a_callee_that_touches_the_world_is_refused() {
    let found: Vec<_> = findings(
        "fn noisy(n: i64) -> i64 {\n\
         \x20   println(\"building\")\n\
         \x20   return n\n\
         }\n\
         comptime N = noisy(1)\n",
    )
    .into_iter()
    .filter(|f| f.code == "NK1152")
    .collect();
    assert_eq!(found.len(), 1, "{found:#?}");
    assert!(
        found[0].notes.join(" ").contains("reaches the world"),
        "{found:#?}"
    );
}

/// **A recursion with no base case says so rather than taking the stack.**
///
/// [ADR-075](../../../docs/specification/adr/adr-075.md) D4 deliberately has no
/// step budget and wrote down what that costs: a body that does not terminate
/// hangs the build. This is not that — a recursion without a base case would
/// overflow *this compiler's* stack, and a compiler that falls over is not the
/// hang D4 accepted.
#[test]
fn an_unbounded_recursion_is_refused_rather_than_crashing() {
    let found: Vec<_> = findings(
        "fn forever(n: i64) -> i64 { return forever(n + 1) }\n\
         comptime N = forever(0)\n",
    )
    .into_iter()
    .filter(|f| f.code == "NK1152")
    .collect();
    assert_eq!(found.len(), 1, "{found:#?}");
    assert!(found[0].message.contains("too deeply"), "{found:#?}");
}

/// **A callee this unit does not declare is unevaluable, not forbidden** —
/// there is no body here to run, and saying *you may not* about a function
/// whose body is somewhere else would be a claim this cannot make. `NK1127`,
/// which is the refusal that has always been there.
#[test]
fn a_callee_from_elsewhere_is_unevaluable() {
    let found: Vec<_> = findings("comptime N = io::read_to_string()\n")
        .into_iter()
        .filter(|f| f.code == "NK1127" || f.code == "NK1152")
        .collect();
    assert_eq!(found.len(), 1, "{found:#?}");
    assert_eq!(found[0].code, "NK1127", "{found:#?}");
}

/// **A `for` over a range**, which is `open-work.md` §2.9's second step: the
/// loop the evaluator used to refuse.
#[test]
fn a_for_over_a_range_is_evaluated() {
    let source = "fn total(n: i64) -> i64 {\n\
                  \x20   let mut t = 0\n\
                  \x20   for i in 0..<n { t += i }\n\
                  \x20   return t\n\
                  }\n\
                  comptime SIX = total(4)\n\
                  fn main() { println(f\"{SIX}\") }\n";
    assert!(findings(source).is_empty(), "{:#?}", findings(source));
    assert!(
        lowered(source).contains("const SIX: i32 = 6;"),
        "{}",
        lowered(source)
    );
}

/// **`..` and `..<` are different loops**
/// ([ADR-137](../../../docs/specification/adr/adr-137.md) D3, D4), and the
/// evaluator reads the end the same way the emitter does rather than assuming
/// one.
#[test]
fn an_inclusive_range_counts_one_further() {
    let source = "fn total(n: i64) -> i64 {\n\
                  \x20   let mut t = 0\n\
                  \x20   for i in 0..n { t += i }\n\
                  \x20   return t\n\
                  }\n\
                  comptime TEN = total(4)\n\
                  fn main() { println(f\"{TEN}\") }\n";
    assert!(findings(source).is_empty(), "{:#?}", findings(source));
    assert!(
        lowered(source).contains("const TEN: i32 = 10;"),
        "{}",
        lowered(source)
    );
}

/// **A `while`**, which is the loop
/// [ADR-075](../../../docs/specification/adr/adr-075.md) D4 said would get no
/// step budget: this one ends because its body ends it, and nothing here
/// counted the turns.
#[test]
fn a_while_is_evaluated_and_nothing_counts_its_turns() {
    let source = "fn halvings(n: i64) -> i64 {\n\
                  \x20   let mut left = n\n\
                  \x20   let mut steps = 0\n\
                  \x20   while left > 1 {\n\
                  \x20       left = left / 2\n\
                  \x20       steps += 1\n\
                  \x20   }\n\
                  \x20   return steps\n\
                  }\n\
                  comptime STEPS = halvings(64)\n\
                  fn main() { println(f\"{STEPS}\") }\n";
    assert!(findings(source).is_empty(), "{:#?}", findings(source));
    assert!(
        lowered(source).contains("const STEPS: i32 = 6;"),
        "{}",
        lowered(source)
    );
}

/// **`break` leaves the loop and `continue` starts its next turn**, and the two
/// are separate statements here for the reason the tree keeps them separate:
/// one ends a loop and one does not.
#[test]
fn break_and_continue_are_both_read() {
    let source = "fn first_over(limit: i64) -> i64 {\n\
                  \x20   let mut found = 0\n\
                  \x20   for i in 0..<100 {\n\
                  \x20       if i % 3 != 0 { continue }\n\
                  \x20       if i > limit {\n\
                  \x20           found = i\n\
                  \x20           break\n\
                  \x20       }\n\
                  \x20   }\n\
                  \x20   return found\n\
                  }\n\
                  comptime FOUND = first_over(10)\n\
                  fn main() { println(f\"{FOUND}\") }\n";
    assert!(findings(source).is_empty(), "{:#?}", findings(source));
    assert!(
        lowered(source).contains("const FOUND: i32 = 12;"),
        "{}",
        lowered(source)
    );
}

/// **A `return` out of a loop leaves the function**, not the loop — which is
/// the one way a loop's body hands a value back at all, since
/// [ADR-151](../../../docs/specification/adr/adr-151.md) D1 says `break`
/// carries none.
#[test]
fn a_return_inside_a_loop_leaves_the_function() {
    let source = "fn first_square_over(limit: i64) -> i64 {\n\
                  \x20   for i in 0..<100 {\n\
                  \x20       if i * i > limit { return i }\n\
                  \x20   }\n\
                  \x20   return 0\n\
                  }\n\
                  comptime ROOT = first_square_over(50)\n\
                  fn main() { println(f\"{ROOT}\") }\n";
    assert!(findings(source).is_empty(), "{:#?}", findings(source));
    assert!(
        lowered(source).contains("const ROOT: i32 = 8;"),
        "{}",
        lowered(source)
    );
}

/// **The loop's name does not outlive the loop**, which is Part I 3.3's rule
/// and matters here because the evaluator's frame is shared between a loop and
/// the body around it.
#[test]
fn the_loops_binding_does_not_outlive_it() {
    let found: Vec<_> = findings(
        "fn leaks() -> i64 {\n\
         \x20   for i in 0..<3 { }\n\
         \x20   return i\n\
         }\n\
         comptime N = leaks()\n",
    )
    .into_iter()
    .filter(|f| f.code == "NK1127" || f.code == "NK1152")
    .collect();
    assert_eq!(found.len(), 1, "{found:#?}");
}
