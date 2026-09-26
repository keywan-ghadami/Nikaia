//! **What a `String` field or result is below is decided by what flows into
//! it** ([ADR-222](../../../docs/specification/adr/adr-222.md)), and each
//! tier is compiled and **run** here at both settings of `user_parallelism`:
//!
//! | what flows in | below |
//! | :--- | :--- |
//! | text of its own only | `String`, unchanged |
//! | views (and literals) only | a view, its buffer placed by ADR-209 |
//! | both | `EitherText`: each value borrowed or owned where it is put in |
//!
//! Not one of these programs writes `ref`, `.clone()` or anything about
//! where text lives.

mod common;

use std::process::Command;

use nikaia::emit::{Build, emit_program};
use nikaia::parser::parse_to_ast;

const APP_CONF: &str = "# settings\nhost = example.org\nport = 8080\ninclude = extra.conf\n";
const EXTRA_CONF: &str = "mode = fast\nhost = override.org\n";

fn lowered(source: &str, how: Build) -> String {
    let parsed = parse_to_ast(source).expect("the source parses");
    emit_program(&parsed, how).expect("the source lowers").rust
}

/// Compile the lowering and run it in a directory holding the two files.
fn ran(purpose: &str, source: &str, how: Build) -> String {
    let rust = lowered(source, how);
    let dir = common::scratch_dir(&format!("text-tiers-{purpose}"));
    std::fs::write(dir.join("app.conf"), APP_CONF).expect("write app.conf");
    std::fs::write(dir.join("extra.conf"), EXTRA_CONF).expect("write extra.conf");
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
    let out = Command::new(&binary)
        .current_dir(&dir)
        .output()
        .expect("run it");
    assert!(
        out.status.success(),
        "{purpose} failed:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let printed = String::from_utf8_lossy(&out.stdout).trim().to_string();
    std::fs::remove_dir_all(&dir).ok();
    printed
}

fn findings(source: &str) -> Vec<nikaia::check::Finding> {
    let parsed = parse_to_ast(source).expect("the source parses");
    let own = nikaia::contracts::Ledger::infer(&parsed);
    let library =
        nikaia::contracts::Ledger::parse(nikaia::contracts::STD).expect("std ships a ledger");
    nikaia::check::check_program(&parsed, &own, &library, &std::collections::BTreeSet::new())
        .findings
        .into_iter()
        .filter(|f| f.severity == nikaia::check::Severity::Error)
        .collect()
}

/// Both settings, the same output, and no refusal on the way.
fn runs(purpose: &str, source: &str, expected: &str) {
    assert!(
        findings(source).is_empty(),
        "{purpose}: {:#?}",
        findings(source)
    );
    for how in [Build::default(), Build::parallel()] {
        assert_eq!(ran(purpose, source, how), expected, "{purpose} at {how:?}");
    }
}

/// **The four shapes the README listed as a wall**, each now a program:
/// a `ref String` parameter, a slice of a buffer the function read, a name
/// bound to a literal - all kept in a `String` field - and a view handed back
/// from a function declared `-> String`.
const WALL: &str = r##"use std::fs

struct Person {
    name: String,
}

struct Config {
    host: String,
}

fn make(n: ref String) -> Person {
    return Person { name: n }
}

fn load(path: String) -> Config throws {
    let text = fs::read_to_string(path, fs::Root::Anywhere)
    return Config { host: text.trim() }
}

fn first_word(line: ref String) -> String {
    return line.trim()
}

fn main() throws {
    let ada = make("ada")
    let config = load("app.conf")
    let n = "grace"
    let grace = Person { name: n }
    let word = first_word("  hi  ")
    println(f"{ada.name} {config.host.len()} {grace.name} [{word}]")
}
"##;

#[test]
fn a_view_kept_where_a_string_is_declared_is_a_view() {
    runs("wall", WALL, "ada 62 grace [hi]");
    let rust = lowered(WALL, Build::default());
    // `Person.name` only ever receives views: it is one, and costs nothing.
    assert!(rust.contains("struct Person<'a>"), "{rust}");
    // The slice of the file: the buffer lives in the caller's keep (ADR-209).
    assert!(
        rust.contains("__keep: &'k nikaia_std::tether::Keep"),
        "{rust}"
    );
    // The result is a view of the parameter.
    assert!(rust.contains("fn first_word(line: &str) -> &str"), "{rust}");
    // `Config.host` too, and nothing copies.
    assert!(!rust.contains("to_owned"), "{rust}");
}

/// **Both kinds of text flow into one field**: text of its own where one line
/// builds it, a view where another line cuts it from a buffer. Neither pays
/// for the other - the owned text is moved in, the view is borrowed.
const MIXED: &str = r##"use std::fs

struct Entry {
    key: String,
    note: String,
}

fn parse(path: String) -> Vec[Entry] throws {
    let text = fs::read_to_string(path, fs::Root::Anywhere)
    let mut out: Vec[Entry] = Vec()
    for line in text.lines() {
        let parts: Vec[ref String] = line.split("=").collect()
        if parts.len() == 2 {
            out.push(Entry { key: parts[0].trim(), note: f"from {parts[1].trim()}" })
        } else {
            out.push(Entry { key: f"line {out.len()}", note: line })
        }
    }
    return out
}

fn describe(line: ref String) -> String {
    if line.starts_with("#") {
        return f"comment of {line.len()}"
    }
    return line.trim()
}

fn main() throws {
    let entries = parse("app.conf")
    for e in entries {
        println(f"{e.key}|{e.note}")
    }
    println(f"{entries.len()} {entries[1].key.clone().len()}")
    let first = describe("# settings")
    let second = describe("  port = 8080 ")
    println(f"{first}|{second}")
}
"##;

#[test]
fn both_kinds_of_text_in_one_field_each_as_it_is() {
    runs(
        "mixed",
        MIXED,
        "line 0|# settings\nhost|from example.org\nport|from 8080\ninclude|from extra.conf\n4 4\n\
         comment of 10|port = 8080",
    );
    let rust = lowered(MIXED, Build::default());
    assert!(
        rust.contains("key: nikaia_std::either_text::EitherText<'a>"),
        "{rust}"
    );
    assert!(
        rust.contains("note: nikaia_std::either_text::EitherText<'a>"),
        "{rust}"
    );
    // A result both kinds flow into: each `return` as it is.
    assert!(
        rust.contains("fn describe(line: &str) -> nikaia_std::either_text::EitherText<'_>"),
        "{rust}"
    );
}

/// **Text of its own only: nothing changes.** The ordinary field is a
/// `String` below, as before this record, and a literal in it is built where
/// it stands (ADR-207 D2).
#[test]
fn a_field_only_text_of_its_own_flows_into_stays_a_string() {
    let source = r##"struct Greeting {
    text: String,
}

fn main() {
    let who = "world"
    let a = Greeting { text: f"hello {who}" }
    let b = Greeting { text: "hi" }
    println(f"{a.text} {b.text}")
}
"##;
    runs("owned", source, "hello world hi");
    let rust = lowered(source, Build::default());
    assert!(
        rust.contains("struct Greeting {\n    text: String,"),
        "{rust}"
    );
}

/// **What the compiler chose is a report** (ADR-107 D4, ADR-222 D5):
/// `--tethers` names each `String` field or result that is not text of its
/// own below, and why.
#[test]
fn the_tiers_are_reported() {
    let parsed = parse_to_ast(MIXED).expect("the source parses");
    let ledger = nikaia::contracts::Ledger::infer(&parsed);
    let report = nikaia::contracts::tether::report(&parsed, &ledger);
    assert!(
        report.contains("`Entry.key` is a view or text of its own, per value"),
        "{report}"
    );
    assert!(
        report.contains("what `describe` hands back is a view or text of its own"),
        "{report}"
    );
    let parsed = parse_to_ast(WALL).expect("the source parses");
    let ledger = nikaia::contracts::Ledger::infer(&parsed);
    let report = nikaia::contracts::tether::report(&parsed, &ledger);
    assert!(
        report.contains("`Person.name` is a view: only views and literals flow into it"),
        "{report}"
    );
}

/// **A published field is never both kinds** (ADR-222 D2): its
/// representation leaves with the package, before the programs that build it
/// exist. The view is refused as before, saying why.
#[test]
fn a_published_field_both_kinds_would_flow_into_stays_text_of_its_own() {
    let source = r##"pub struct Person {
    pub name: String,
}

pub fn named(n: ref String) -> Person {
    return Person { name: n }
}

pub fn built(n: i64) -> Person {
    return Person { name: f"person {n}" }
}

fn main() {
    println(named("ada").name)
}
"##;
    let found = findings(source);
    assert!(
        found
            .iter()
            .any(|f| f.code == "NK1106" && f.message.contains("`Person.name` is `String`")),
        "{found:#?}"
    );
}

// ---------------------------------------------------------------------------
// Every other place a program declares `String`
// ([ADR-223](../../../docs/specification/adr/adr-223.md))
// ---------------------------------------------------------------------------

/// **A parameter, an annotated `let`, and the elements of a list and a map**
/// are positions too. Every line here puts a view where the program declared
/// `String`, none writes a copy, and nothing is copied.
const EVERYWHERE: &str = r##"use std::fs
use std::collections

struct Person {
    name: String,
}

fn keep(s: String) -> Person {
    return Person { name: s }
}

fn words(text: ref String) -> Vec[String] {
    let mut out: Vec[String] = Vec()
    for w in text.split(" ") {
        out.push(w)
    }
    return out
}

fn main() throws {
    let text = fs::read_to_string("app.conf", fs::Root::Anywhere)
    let first: String = text.trim()
    let p = keep(first)
    let mut names: Vec[String] = Vec()
    let mut counts: collections::HashMap[String, i64] = collections::HashMap()
    for line in text.lines() {
        names.push(line.trim())
        for w in line.split(" ") {
            counts[w] = (counts[w] ?? 0) + 1
        }
    }
    let ws = words("a b c")
    println(f"{p.name.len()} {names.len()} {counts[\"=\"] ?? 0} {ws.len()} {ws[2]}")
}
"##;

#[test]
fn a_view_in_a_parameter_a_let_a_list_or_a_map_is_a_view() {
    runs("everywhere", EVERYWHERE, "62 4 3 3 c");
    let rust = lowered(EVERYWHERE, Build::default());
    assert!(rust.contains("fn keep(s: &str)"), "{rust}");
    assert!(rust.contains("-> Vec<&str>"), "{rust}");
    assert!(!rust.contains("to_owned"), "{rust}");
}

/// **Both kinds into one parameter, one `let` and one list**: each value is
/// handed over as it is - borrowed where it is a view, moved where it is text
/// of its own.
const MIXED_EVERYWHERE: &str = r##"use std::fs

fn shout(s: String) -> String {
    return f"{s}!"
}

fn main() throws {
    let text = fs::read_to_string("extra.conf", fs::Root::Anywhere)
    let mut last: String = f"nothing"
    let mut all: Vec[String] = Vec()
    for line in text.lines() {
        last = line.trim()
        all.push(line)
        all.push(f"#{all.len()}")
    }
    let loud = shout(last)
    let quiet = shout(f"x")
    println(f"{loud} {quiet} {all.len()} {all[1]}")
}
"##;

#[test]
fn both_kinds_into_a_parameter_a_let_and_a_list_each_as_it_is() {
    runs(
        "mixed-everywhere",
        MIXED_EVERYWHERE,
        "host = override.org! x! 4 #1",
    );
    let rust = lowered(MIXED_EVERYWHERE, Build::default());
    assert!(
        rust.contains("Vec<nikaia_std::either_text::EitherText<'_>>"),
        "{rust}"
    );
    assert!(rust.contains(".into()"), "{rust}");
}
