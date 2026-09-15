//! Part I 4.7's `trait` declaration, and the bound it makes possible
//! ([ADR-078](../../../docs/specification/adr/adr-078.md)).
//!
//! **The section's own example did not parse.** `trait Summarize { … }` was a
//! parse error at the keyword, so a trait could be implemented and not declared
//! — and that is what [ADR-074](../../../docs/specification/adr/adr-074.md) §4
//! named as the prerequisite for a bound: `[T: Summarize]` has to name
//! something. Until it did, a generic body could move and pass its value and
//! nothing else, which `NK1126` said to the user in so many words.

mod common;

use nikaia::check;
use nikaia::contracts::{Ledger, Sync, STD};
use nikaia::emit::{emit_program, Build};
use nikaia::parser::parse_to_ast;

/// **`check_program` and not `check`**, because the trait rules read the
/// ledger's *finished* `sync` column and only the program-level entry point runs
/// after `sync::infer` has filled it in
/// ([ADR-080](../../../docs/specification/adr/adr-080.md) §2). A test on the
/// inner entry point would have found `NK1129` silent and concluded it was not
/// built.
fn findings(source: &str) -> Vec<check::Finding> {
    let parsed = parse_to_ast(source).expect("the source parses");
    let own = Ledger::infer(&parsed);
    let library = Ledger::parse(STD).expect("std's shipped ledger parses");
    check::check_program(&parsed, &own, &library, &std::collections::BTreeSet::new()).findings
}

fn lowered(purpose: &str, source: &str) -> String {
    let found = findings(source);
    assert!(
        found.is_empty(),
        "{purpose} is a correct program and the checker says otherwise: {found:#?}"
    );
    let parsed = parse_to_ast(source).expect("the source parses");
    emit_program(&parsed, Build::default())
        .expect("the source lowers")
        .rust
}

/// Compile the lowering as a binary, run it, hand back what it printed.
fn ran(purpose: &str, source: &str) -> String {
    let rust = lowered(purpose, source);
    let dir = common::scratch_dir(purpose);
    let file = dir.join("main.rs");
    std::fs::write(&file, &rust).expect("write the Rust");
    let binary = dir.join("program");
    let out = common::compile(&file, &["-o", binary.to_str().expect("utf-8 path")]);
    assert!(
        out.status.success(),
        "the lowering of {purpose} does not compile:\n{}\n--- the Rust ---\n{rust}",
        String::from_utf8_lossy(&out.stderr),
    );
    let ran = std::process::Command::new(&binary)
        .output()
        .expect("the program runs");
    let printed = String::from_utf8_lossy(&ran.stdout).into_owned();
    let _ = std::fs::remove_dir_all(&dir);
    printed
}

/// Part I 4.7's own program, declared and bound and run.
///
/// The body interpolates where the page writes `"User: " + self.username`, and
/// that is **not** a tidy-up: `&str + String` is accepted here and refused by
/// the language below (`E0369`), which `open-work.md` now carries with its
/// reproduction. Found by writing this fixture, which is the third time this
/// session that running one of the specification's own programs turned one up.
const SUMMARIZE: &str = r#"
trait Summarize {
    fn summary(&self) -> String
}

struct User {
    name: String,
}

impl Summarize for User {
    fn summary(&self) -> String {
        return f"User: {self.name}"
    }
}

fn shout[T: Summarize](x: T) -> String {
    return x.summary()
}

fn main() {
    let u = User { name: "Ada".to_string() }
    println(shout(u))
}
"#;

/// D1 and D3 together: the declaration parses, the bound resolves the call, and
/// the program runs.
#[test]
fn a_trait_is_declared_bound_and_run() {
    assert_eq!(ran("a trait and a bound", SUMMARIZE).trim(), "User: Ada");
}

/// D1: the method is a **signature**, so the lowering writes a `;` where an
/// `impl`'s writes a body.
#[test]
fn a_trait_method_is_a_signature() {
    let rust = lowered("a trait declaration", SUMMARIZE);
    assert!(
        rust.contains("trait Summarize {"),
        "the trait reaches the generated file:\n{rust}"
    );
    assert!(
        rust.contains("fn summary(&self) -> String;"),
        "its method is a signature and not a body:\n{rust}"
    );
}

/// D2: the bound is written where Rust writes one.
#[test]
fn a_bound_reaches_the_generated_file() {
    let rust = lowered("a bound", SUMMARIZE);
    assert!(
        rust.contains("fn shout<T: Summarize>(x: T)"),
        "the parameter carries its bound:\n{rust}"
    );
}

/// Several bounds, `[T: A + B]`, which is why the AST holds a list.
#[test]
fn several_bounds_are_written_with_a_plus() {
    let rust = lowered(
        "two bounds",
        r#"
trait Named {
    fn name_of(&self) -> String
}

trait Aged {
    fn age_of(&self) -> i64
}

struct User {
    name: String,
    age: i64,
}

impl Named for User {
    fn name_of(&self) -> String {
        // `.clone()` and not `return self.name`, which `NK1131` refuses
        // ([ADR-083](../../../docs/specification/adr/adr-083.md)) - and it
        // caught this fixture, which had been writing a program that never
        // compiled. A test that rests on a shape the language does not have is
        // a test that measures the wrong thing.
        return self.name.clone()
    }
}

impl Aged for User {
    fn age_of(&self) -> i64 {
        return self.age
    }
}

fn both[T: Named + Aged](x: T) -> String {
    return x.name_of()
}

fn main() {
    let u = User { name: "Ada".to_string(), age: 36 }
    println(both(u))
}
"#,
    );
    assert!(
        rust.contains("fn both<T: Named + Aged>(x: T)"),
        "both bounds, in the order written:\n{rust}"
    );
}

/// D3: the bound answers the **whole** call and not only whether it exists. An
/// argument the trait's signature does not take is `NK1101`, exactly as it would
/// be on a receiver whose type was written down.
#[test]
fn a_bound_answers_the_arity_too() {
    let found = findings(
        r#"
trait Summarize {
    fn summary(&self) -> String
}

fn shout[T: Summarize](x: T) -> String {
    return x.summary(1)
}

fn main() {
    println("x")
}
"#,
    );
    assert!(
        found.iter().any(|f| f.code == "NK1101"),
        "the declaration says how many arguments it takes: {found:#?}"
    );
}

/// And the result type, which is what makes a bound worth more than a permission
/// slip: `summary` hands back a `String`, so a `let` that says `i64` is refused.
#[test]
fn a_bound_answers_the_result_type() {
    let found = findings(
        r#"
trait Summarize {
    fn summary(&self) -> String
}

fn shout[T: Summarize](x: T) -> i64 {
    return x.summary()
}

fn main() {
    println("x")
}
"#,
    );
    assert!(
        found.iter().any(|f| f.code == "NK1104"),
        "a `String` is not an `i64`, whatever the caller picks: {found:#?}"
    );
}

/// `NK1126` is unchanged where the bound does **not** declare the member, which
/// is the half that keeps the refusal honest: a bound is not a blanket licence.
#[test]
fn a_member_no_bound_declares_is_still_refused() {
    let found = findings(
        r#"
trait Summarize {
    fn summary(&self) -> String
}

fn shout[T: Summarize](x: T) -> String {
    return x.to_uppercase()
}

fn main() {
    println("x")
}
"#,
    );
    assert!(
        found.iter().any(|f| f.code == "NK1126"),
        "`Summarize` says nothing about `to_uppercase`: {found:#?}"
    );
}

/// D4: a trait's method is `sync` in the ledger, because a declaration has no
/// body for `sync::infer` to read and a plain `fn` is the only thing the emitter
/// can write in a trait.
///
/// **This is what stopped `fn shout` from being `async`.** Before it, the callee
/// was simply absent from the fixpoint's map and *absent* read as *pauses*, so
/// the lowering awaited a `String`.
#[test]
fn a_trait_method_is_sync_and_so_is_a_body_that_reaches_one() {
    let parsed = parse_to_ast(SUMMARIZE).expect("the source parses");
    let own = Ledger::infer(&parsed);
    assert_eq!(
        own.functions
            .get("Summarize::summary")
            .expect("the declaration is in the ledger")
            .sync,
        Sync::Asserted,
    );
    assert!(
        own.functions
            .get("shout")
            .expect("the bounded function is in the ledger")
            .sync
            .is_sync(),
        "a body that only reaches a trait's method cannot pause",
    );
    let rust = lowered("a sync body", SUMMARIZE);
    assert!(
        !rust.contains("async fn shout"),
        "and it is not lowered as pausing:\n{rust}"
    );
}

/// `trait` is a reserved word now, which it was not: it was found as a *name*
/// by [ADR-076](../../../docs/specification/adr/adr-076.md)'s sweep.
#[test]
fn trait_is_a_reserved_word() {
    assert!(
        nikaia::parser::RESERVED_WORDS.contains(&"trait"),
        "the construct exists, so the word is not a name"
    );
    assert!(
        parse_to_ast("fn main() {\n    let trait = 3\n}\n").is_err(),
        "`let trait = 3` does not parse"
    );
}

/// `NK1129` ([ADR-080](../../../docs/specification/adr/adr-080.md) D1): the
/// implementation pauses and the declaration has no way to say so.
///
/// *Reproduced before it was refused:* the trait lowered to
/// `fn load(&self) -> Result<…>;` and the `impl` to `async fn load(&self) ->
/// Result<…>`, and the language below answered `E0053` about a file nobody
/// wrote.
#[test]
fn an_implementation_that_pauses_is_refused_where_the_trait_cannot_say_so() {
    let found = findings(
        r#"
trait Loader {
    fn load(&self) -> String throws
}

struct File {
    path: String,
}

impl Loader for File {
    fn load(&self) -> String throws {
        return fs::read_to_string(self.path)
    }
}

fn main() {
    println("x")
}
"#,
    );
    let refusal = found
        .iter()
        .find(|f| f.code == "NK1129")
        .unwrap_or_else(|| panic!("a pausing implementation is refused: {found:#?}"));
    assert!(
        refusal.message.contains("File::load") && refusal.message.contains("Loader"),
        "it names the method and the trait: {}",
        refusal.message
    );
}

/// And a body that does **not** pause is not refused, which is every trait
/// anybody has written so far.
#[test]
fn an_implementation_that_cannot_pause_is_left_alone() {
    assert!(
        findings(SUMMARIZE).is_empty(),
        "the ordinary case stays ordinary"
    );
}

/// `NK1130`, the direction that was `rustc`'s `E0046`: the `impl` leaves out a
/// method the trait declares.
#[test]
fn an_impl_that_leaves_a_method_out_is_refused() {
    let found = findings(
        r#"
trait Summarize {
    fn summary(&self) -> String
    fn title(&self) -> String
}

struct User {
    name: String,
}

impl Summarize for User {
    fn summary(&self) -> String {
        return f"User: {self.name}"
    }
}

fn main() {
    println("x")
}
"#,
    );
    let refusal = found
        .iter()
        .find(|f| f.code == "NK1130")
        .unwrap_or_else(|| panic!("an incomplete impl is refused: {found:#?}"));
    assert!(
        refusal.message.contains("title"),
        "it names what is missing: {}",
        refusal.message
    );
}

/// The other direction, which was `E0407`: a method the trait does not declare.
#[test]
fn a_method_the_trait_does_not_declare_is_refused() {
    let found = findings(
        r#"
trait Summarize {
    fn summary(&self) -> String
}

struct User {
    name: String,
}

impl Summarize for User {
    fn summary(&self) -> String {
        return f"User: {self.name}"
    }

    fn shout(&self) -> String {
        return f"USER"
    }
}

fn main() {
    println("x")
}
"#,
    );
    let refusal = found
        .iter()
        .find(|f| f.code == "NK1130")
        .unwrap_or_else(|| panic!("a method outside the trait is refused: {found:#?}"));
    assert!(
        refusal.message.contains("shout"),
        "it names the method: {}",
        refusal.message
    );
    assert!(
        refusal
            .help
            .as_deref()
            .is_some_and(|h| h.contains("impl User")),
        "and says where it belongs: {:?}",
        refusal.help
    );
}

/// **A trait this unit does not declare is not checked against**, and that is
/// the rule rather than a gap: `impl Error for ConfigError` names the one trait
/// the compiler reads rather than one a `.nika` file wrote
/// ([ADR-023](../../../docs/specification/adr/adr-023.md) D3), and a trait a
/// package publishes cannot be reached at all yet. Silence is the only correct
/// answer about a declaration that is not here.
#[test]
fn an_impl_of_a_trait_declared_elsewhere_is_left_alone() {
    let found = findings(
        r#"
struct ConfigError {
    path: String,
}

impl Error for ConfigError {
    fn message(&self) -> String {
        return f"no config at {self.path}"
    }
}

fn main() {
    println("x")
}
"#,
    );
    assert!(
        !found.iter().any(|f| f.code == "NK1130"),
        "nothing here declares `Error`, so nothing here can check against it: {found:#?}"
    );
}
