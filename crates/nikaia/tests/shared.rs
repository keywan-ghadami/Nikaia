//! `Shared[T]`, compiled and run.
//!
//! Part I 6.2 says how the first handle on a shared value is made - by writing
//! the type - and [ADR-042](../../../docs/specification/adr/adr-042.md) D2 says
//! a function that only *uses* the value takes an ordinary view of it. Neither
//! claim can be checked by reading the emitted Rust: `Rc` and `Arc` differ in
//! nothing a program can observe except speed and `Send`-ness
//! (`docs/rc-or-arc.md` §2), so a test that compared text would pass on a
//! lowering that does not run. These compile the emitted Rust and run the
//! binary.
//!
//! At **both settings of `user_parallelism`**, because
//! [ADR-037](../../../docs/specification/adr/adr-037.md) D6 is the claim that
//! one count serves both: a program that printed differently under the two
//! would be the failure that decision exists to rule out.

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

/// A `Shared[Connection]` made, lent to a function that takes `&Connection`,
/// and printed.
///
/// `serve` never mentions sharing, which is Part I 6.2's own example: whether
/// the value is shared is the caller's decision, and `serve` has no business
/// knowing. The borrow reaches the inner value through `Shared::deref` and
/// ADR-042 D2, with nothing else added.
const BORROWED: &str = "\
struct Connection {
    host: String
}

fn connect(host: String) -> Connection {
    Connection { host: host }
}

fn serve(db: &Connection) {
    println(f\"serving {db.host}\")
}

fn main() {
    let db: Shared[Connection] = connect(\"localhost\".to_string())
    serve(&db)
    serve(&db)
}
";

/// Lower `source`, and hand back the emitted Rust.
fn lower(dir: &Path, source: &str, flags: &[&str]) -> String {
    let input = dir.join("main.nika");
    std::fs::write(&input, source).expect("the source");
    let output = dir.join("main.rs");
    let run = Command::new(env!("CARGO_BIN_EXE_nikaia"))
        .args(["--input", input.to_str().expect("utf-8 path")])
        .args(["--output", output.to_str().expect("utf-8 path")])
        .args(["--no-cache"])
        .args(flags)
        .env("NIKAIA_CACHE_DIR", dir.join("cache"))
        .output()
        .expect("the nikaia binary runs");
    assert!(
        run.status.success(),
        "lowering failed:\n{}{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    std::fs::read_to_string(&output).expect("the emitted Rust")
}

/// Lower, compile, and hand back the binary and the Rust it was built from.
fn build(dir: &Path, source: &str, flags: &[&str]) -> (PathBuf, String) {
    let rust = lower(dir, source, flags);
    let binary = dir.join("program");
    let compiled = common::compile(
        &dir.join("main.rs"),
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
    (binary, rust)
}

/// Lower, compile, run, and hand back what it printed and the Rust behind it.
fn run(purpose: &str, source: &str, flags: &[&str]) -> (String, String) {
    let dir = common::scratch_dir(purpose);
    let (binary, rust) = build(&dir, source, flags);
    let ran = Command::new(&binary).output().expect("run the program");
    assert!(
        ran.status.success(),
        "the program failed: {}",
        String::from_utf8_lossy(&ran.stderr)
    );
    let printed = String::from_utf8_lossy(&ran.stdout).into_owned();
    let _ = std::fs::remove_dir_all(&dir);
    (printed, rust)
}

/// The whole of Part I 6.2's worked example, end to end.
#[test]
fn a_shared_value_is_made_by_writing_the_type_and_lent_out_by_a_view() {
    let (printed, rust) = run("shared-borrowed", BORROWED, &[]);
    assert_eq!(printed.trim(), "serving localhost\nserving localhost");
    // Nothing crosses a thread with it, so D7's optimisation applies: the plain
    // count. That is the base case ADR-037 §5 asks for - an analysis answering
    // `atomic` about everything would pass a test that only checked it ran.
    assert!(
        rust.contains("std::rc::Rc<Connection>") && rust.contains("std::rc::Rc::new(connect("),
        "the annotation is the constructor:\n{rust}"
    );
    // A borrow duplicates nothing (ADR-040 D1's correction). Two of them, and
    // not one step of the count.
    assert!(
        !rust.contains(".clone()"),
        "lending the inner value out must not touch the count:\n{rust}"
    );
}

/// One count at both settings of `user_parallelism`, and the same program.
///
/// ADR-037 D6's whole claim: the representation came off the switch, so a
/// library built at one setting keeps working at the other. The *count* a value
/// gets is per value and never reads the switch either (D4's note), so the two
/// lowerings are byte for byte the same here - which is a stronger statement
/// than "they print the same" and is the one D4 makes.
#[test]
fn the_switches_agree_about_a_shared_value() {
    let (sequential, one) = run("shared-sequential", BORROWED, &["--user-parallelism", "no"]);
    let (parallel, two) = run("shared-parallel", BORROWED, &["--user-parallelism", "yes"]);
    assert_eq!(sequential, parallel, "the two builds print differently");
    assert_eq!(
        one.contains("std::rc::Rc<Connection>"),
        two.contains("std::rc::Rc<Connection>"),
        "the count may not move with the switch (ADR-037 D6)\n--- no ---\n{one}\n--- yes ---\n{two}"
    );
}

/// A value the analysis cannot prove stays put gets the atomic floor, and the
/// program still runs and still prints the same thing.
///
/// `Rc` and `Arc` differ in nothing observable (`docs/rc-or-arc.md` §2), which
/// is what lets the count be inferred at all - so the two lowerings of the same
/// program have to agree about what it prints. Here the public signature is what
/// forces the floor (ADR-037 D8's last row).
#[test]
fn the_atomic_floor_runs_the_same_program() {
    let source = "\
struct Connection {
    host: String
}

fn connect(host: String) -> Connection {
    Connection { host: host }
}

pub fn serve(db: Shared[Connection]) {
    println(f\"serving {db.host}\")
}

fn main() {
    let db: Shared[Connection] = connect(\"localhost\".to_string())
    serve(db)
}
";
    let (printed, rust) = run("shared-atomic", source, &[]);
    assert_eq!(printed.trim(), "serving localhost");
    assert!(
        rust.contains("std::sync::Arc<Connection>"),
        "a public signature is the one fallback no contract can lift:\n{rust}"
    );
}

/// A field whose declared type says the value is shared is the other place the
/// first handle is made (Part I 6.2).
#[test]
fn a_field_whose_declared_type_says_so_makes_a_handle() {
    let source = "\
struct Connection {
    host: String
}

struct Pool {
    db: Shared[Connection],
    size: i64
}

fn connect(host: String) -> Connection {
    Connection { host: host }
}

fn main() {
    let pool = Pool { db: connect(\"localhost\".to_string()), size: 4 }
    println(f\"{pool.size} to {pool.db.host}\")
}
";
    let (printed, rust) = run("shared-field", source, &[]);
    assert_eq!(printed.trim(), "4 to localhost");
    assert!(
        rust.contains("::new(connect("),
        "the field's declared type is what makes the handle:\n{rust}"
    );
}

/// `--sharing` on a program that has a `Shared` value.
///
/// It has never had a real input before: until the type existed, the report was
/// exercised only by the analysis's own tests. This is the flag doing what
/// ADR-037 D7 asks of it, on a program that compiles.
#[test]
fn the_report_explains_a_real_program() {
    let printed = report(BORROWED);
    assert!(printed.starts_with("main:\n"), "{printed}");
    assert!(
        printed.contains("plain   `db` (Shared[Connection])"),
        "{printed}"
    );
    assert!(
        printed.contains("1 `Shared` value(s): 1 plain, 0 atomic"),
        "{printed}"
    );
}

// --- the handle is duplicated, and the cost is readable (ADR-040) ------------

/// A handle **handed on by value** is duplicated, and a **borrow** duplicates
/// nothing. Both directions in one program, because the pair is the rule.
///
/// [ADR-040](../../../docs/specification/adr/adr-040.md) D1: there is no method
/// to call, so the step is written where the handle is handed on - and D2 makes
/// it unconditional rather than conditional on a later use, because a line
/// further down may not decide what a line further up does to a cleanup point.
#[test]
fn a_handle_handed_on_by_value_is_duplicated_and_a_borrow_is_not() {
    let source = "\
struct Conn { host: String }

struct Pool { db: Shared[Conn] }

fn connect(host: String) -> Conn {
    Conn { host: host }
}

fn serve(db: &Conn) {
    println(f\"serving {db.host}\")
}

fn peek(db: &Shared[Conn]) {
    println(f\"peeking {db.host}\")
}

fn keep(db: Shared[Conn]) -> Pool {
    return Pool { db: db }
}

fn main() {
    let db: Shared[Conn] = connect(\"localhost\".to_string())
    serve(&db)
    peek(&db)
    let pool = keep(db)
    println(f\"kept {pool.db.host}\")
    serve(&db)
}
";
    let (printed, rust) = run("shared-duplicated", source, &[]);
    // `db` is still ours after `keep` took one of its own, which is the whole
    // point of the rule: each handle dies at the end of its own block (D3).
    assert_eq!(
        printed.trim(),
        "serving localhost\npeeking localhost\nkept localhost\nserving localhost"
    );
    assert!(
        rust.contains("keep(db.clone())"),
        "a handle handed on by value is duplicated:\n{rust}"
    );
    // Two borrows and one lent handle, and not one step of the count between
    // them. D1's correction is exactly this line.
    assert_eq!(
        rust.matches(".clone()").count(),
        1,
        "only the by-value handover duplicates:\n{rust}"
    );
    assert!(
        rust.contains("serve(&db)") && rust.contains("peek(&db)"),
        "a borrow is handed on as written:\n{rust}"
    );
}

/// A handle read out of a **field** and handed on by value is duplicated too:
/// `keep(pool.db)` gives the callee an owner as surely as `keep(db)` does.
#[test]
fn a_handle_read_out_of_a_field_is_duplicated_when_it_is_handed_on() {
    let source = "\
struct Conn { host: String }

struct Pool { db: Shared[Conn] }

fn connect(host: String) -> Conn {
    Conn { host: host }
}

fn keep(db: Shared[Conn]) -> Pool {
    return Pool { db: db }
}

fn main() {
    let db: Shared[Conn] = connect(\"localhost\".to_string())
    let pool = keep(db)
    let again = keep(pool.db)
    println(f\"{pool.db.host} {again.db.host}\")
}
";
    let (printed, rust) = run("shared-field-handover", source, &[]);
    assert_eq!(printed.trim(), "localhost localhost");
    assert!(
        rust.contains("keep(pool.db.clone())"),
        "a handle taken out of a field is handed on by value:\n{rust}"
    );
}

/// **`--sharing` names each duplication site beside the count it printed**
/// ([ADR-040](../../../docs/specification/adr/adr-040.md) D5).
///
/// D1 makes the place one step of the count is paid unwritten in the source, so
/// it has to be readable somewhere, and the count is already printed here.
#[test]
fn the_report_names_each_duplication_site() {
    let source = "\
struct Conn { host: String }

struct Pool { db: Shared[Conn] }

fn connect(host: String) -> Conn {
    Conn { host: host }
}

fn serve(db: &Conn) { }

fn keep(db: Shared[Conn]) -> Pool {
    return Pool { db: db }
}

fn main() {
    let db: Shared[Conn] = connect(\"localhost\".to_string())
    serve(&db)
    let pool = keep(db)
    println(f\"{pool.db.host}\")
}
";
    let printed = report(source);
    assert!(
        printed.contains("duplicated: handed to `keep` as `db`"),
        "the site the step is paid at has to be in the output: {printed}"
    );
    // And a value that was only ever borrowed says so, rather than saying
    // nothing - the absence of a step is the thing a reader came to look up.
    assert!(
        printed.contains("one handle, so the count is never stepped"),
        "{printed}"
    );
}

/// A borrow contributes no duplication site, which is the other half of D1's
/// correction: a report that listed `serve(&db)` would be charging for an
/// instruction nothing pays.
#[test]
fn a_borrow_is_not_a_duplication_site() {
    let source = "\
struct Conn { host: String }

fn connect(host: String) -> Conn {
    Conn { host: host }
}

fn serve(db: &Conn) { }

fn peek(db: &Shared[Conn]) { }

fn main() {
    let db: Shared[Conn] = connect(\"localhost\".to_string())
    serve(&db)
    peek(&db)
}
";
    let printed = report(source);
    assert!(
        !printed.contains("duplicated:"),
        "lending the inner value out duplicates nothing: {printed}"
    );
    assert!(
        printed.contains("one handle, so the count is never stepped"),
        "{printed}"
    );
}

/// `--sharing` on a source, and what it printed.
fn report(source: &str) -> String {
    let dir = common::scratch_dir("shared-report");
    let input = dir.join("main.nika");
    std::fs::write(&input, source).expect("the source");
    let run = Command::new(env!("CARGO_BIN_EXE_nikaia"))
        .args(["--input", input.to_str().expect("utf-8 path")])
        .args([
            "--output",
            dir.join("main.rs").to_str().expect("utf-8 path"),
        ])
        .args(["--sharing", "--no-cache"])
        .env("NIKAIA_CACHE_DIR", dir.join("cache"))
        .output()
        .expect("the nikaia binary runs");
    let printed = String::from_utf8_lossy(&run.stdout).into_owned();
    assert!(run.status.success(), "{printed}");
    let _ = std::fs::remove_dir_all(&dir);
    printed
}

/// A function that returns a shared value wraps on a line of its own, and the
/// `return` then hands on what is already shared.
///
/// Part I 6.2's rule is that sharing starts on a line where the type was written;
/// a signature line is not one. This is not a dead end and the message says why:
/// wrap inside on a line of its own, or return a plain value and let the caller
/// write the type. Both are ordinary, and this runs the first.
#[test]
fn a_function_returning_a_shared_value_wraps_on_a_line_of_its_own() {
    let (printed, rust) = run(
        "shared-returned",
        "struct Connection {\n    \
             host: String\n\
         }\n\
         \n\
         fn connect(host: String) -> Shared[Connection] {\n    \
             let c: Shared[Connection] = Connection { host: host }\n    \
             return c\n\
         }\n\
         \n\
         fn main() {\n    \
             let db = connect(\"localhost\".to_string())\n    \
             println(f\"connected to {db.host}\")\n\
         }\n",
        &[],
    );
    assert_eq!(printed.trim(), "connected to localhost");
    // The annotated `let` is the one constructor, and the `return` adds nothing.
    assert_eq!(
        rust.matches("Rc::new").count() + rust.matches("Arc::new").count(),
        1,
        "the `return` must not wrap a second time:\n{rust}"
    );
}
