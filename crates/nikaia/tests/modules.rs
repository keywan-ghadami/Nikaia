//! A package of several files (Part I, 9.1).
//!
//! Every test here builds a real directory of `.nika` files and drives the
//! compiler over it, because what is being tested is *resolution* - which files
//! take part, what they may see of each other, and what one ledger over all of
//! them says. None of that can be checked on a string.
//!
//! **A package is a directory** ([ADR-047](../../../docs/specification/adr/adr-047.md)
//! D1): the files in it share one namespace and need no `use` between them. What
//! used to be here - `use utils`, `utils::double(21)`, a private item refused
//! across a file boundary - was the file = module model that decision replaced.

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use nikaia::contracts::Ledger;
use nikaia::emit::Build;
use nikaia::modules::Program;

/// Write a set of files into a scratch directory and hand back the entry.
fn project(name: &str, files: &[(&str, &str)]) -> (PathBuf, PathBuf) {
    let dir = common::scratch_dir(&format!("modules-{name}"));
    for (file, contents) in files {
        let path = dir.join(file);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create directories");
        }
        std::fs::write(&path, contents).expect("write the file");
    }
    (dir.clone(), dir.join("main.nika"))
}

/// Lower, compile and run - the only proof that the files really became one
/// program.
fn run(entry: &Path, build: Build) -> String {
    let program = Program::read(entry).expect("the program reads");
    let lowered = program.emit(build).expect("the program lowers");

    let dir = entry.parent().expect("a directory");
    let rust = dir.join("program.rs");
    std::fs::write(&rust, &lowered.rust).expect("write the Rust");

    let binary = dir.join("program");
    let compiled = common::compile(
        &rust,
        &[
            "--crate-type",
            "bin",
            "-o",
            binary.to_str().expect("utf-8 path"),
        ],
    );
    assert!(
        compiled.status.success(),
        "the emitted Rust did not compile:\n{}\n--- emitted ---\n{}",
        String::from_utf8_lossy(&compiled.stderr),
        lowered.rust
    );
    let ran = Command::new(&binary).output().expect("run it");
    assert!(
        ran.status.success(),
        "the program failed:\n{}",
        String::from_utf8_lossy(&ran.stderr)
    );
    String::from_utf8_lossy(&ran.stdout).to_string()
}

const UTILS: &str = "\
pub fn double(n: i32) -> i32 {
    return n * 2
}

fn secret() -> i32 {
    return 41
}

pub fn answer() -> i32 {
    return secret() + 1
}
";

/// Two files, one package - and it runs, with no `use` between them.
#[test]
fn two_files_of_a_package_are_one_program() {
    let (dir, entry) = project(
        "two-files",
        &[
            ("utils.nika", UTILS),
            (
                "main.nika",
                "fn main() {\n\
                 \x20   println(f\"{double(21)} {answer()}\")\n\
                 }\n",
            ),
        ],
    );

    assert_eq!(run(&entry, Build::default()).trim(), "42 42");
    let _ = std::fs::remove_dir_all(dir);
}

/// **Every `.nika` beside the entry takes part, whether or not anything names
/// it** (ADR-047 D1).
///
/// The directory decides and the `use` lines do not, which is what makes moving
/// a declaration from one file to another housekeeping rather than a change to
/// the package's surface. Without this the file below would simply not be read.
#[test]
fn a_file_nothing_names_is_still_part_of_the_package() {
    let (dir, entry) = project(
        "unnamed-file",
        &[
            ("extra.nika", "pub fn seven() -> i64 {\n    return 7\n}\n"),
            (
                "main.nika",
                "fn main() {\n\x20   println(f\"{seven()}\")\n}\n",
            ),
        ],
    );
    assert_eq!(run(&entry, Build::default()).trim(), "7");
    let _ = std::fs::remove_dir_all(dir);
}

/// **A name that is not `pub` is still visible inside its package** — which is
/// the half of Part I 9.2 that moved.
///
/// It used to be `NK1110` across a file boundary. The boundary is the package
/// now, so a sibling file reaching `secret()` is ordinary code, and the check has
/// nothing to say about it.
#[test]
fn a_private_name_is_visible_across_the_files_of_its_package() {
    let (dir, entry) = project(
        "package-privacy",
        &[
            ("utils.nika", UTILS),
            (
                "main.nika",
                "fn main() {\n\x20   println(f\"{secret()}\")\n}\n",
            ),
        ],
    );

    let program = Program::read(&entry).expect("the program reads");
    let library = Ledger::parse(nikaia::contracts::STD).expect("std's ledger");
    let packages = program.package_names();
    for unit in &program.units {
        let findings =
            nikaia::check::check_program(&unit.parsed, &program.contracts, &library, &packages)
                .findings;
        assert!(findings.is_empty(), "{:#?}", findings);
    }

    assert_eq!(run(&entry, Build::default()).trim(), "41");
    let _ = std::fs::remove_dir_all(dir);
}

/// **Two files of one package may not declare the same name** (ADR-047 D1).
///
/// One namespace, so this is an error rather than a rule about which of the two a
/// line means - and it is refused here, because `rustc` would report a duplicate
/// definition against a file nobody wrote (Part III, C.1).
#[test]
fn two_files_may_not_declare_the_same_name() {
    let (dir, entry) = project(
        "collision",
        &[
            ("a.nika", "pub struct Row { pub id: i64 }\n"),
            ("main.nika", "struct Row { id: i64 }\n\nfn main() { }\n"),
        ],
    );
    let Err(error) = Program::read(&entry) else {
        panic!("two `Row`s in one namespace");
    };
    let message = format!("{error:#}");
    assert!(message.contains("`Row` is declared twice"), "{message}");
    assert!(
        message.contains("a.nika") && message.contains("main.nika"),
        "{message}"
    );
    let _ = std::fs::remove_dir_all(dir);
}

/// One ledger for the package (Part III, 13.5), and **one namespace in it**.
///
/// The keys used to be `utils::double`, because a file was a unit of naming. It
/// is not one any more (ADR-047 D1), so a name belongs to the package and the
/// ledger says so - which is also what a consumer of this package will read,
/// under the package's own name rather than under the file it happens to sit in.
#[test]
fn the_ledger_is_one_file_with_one_namespace_in_it() {
    let (dir, entry) = project(
        "ledger",
        &[("utils.nika", UTILS), ("main.nika", "fn main() { }\n")],
    );

    let program = Program::read(&entry).expect("the program reads");
    let rendered = program.contracts.render();

    for key in ["double", "answer", "secret", "main"] {
        assert!(rendered.contains(&format!("[fn.\"{key}\"]")), "{rendered}");
    }
    assert!(
        !rendered.contains("utils::"),
        "no file is a prefix:\n{rendered}"
    );

    // …and it reads back, which is what `--locked` rests on. `pub` still says
    // what leaves the *package*, which is the boundary that is left.
    let read = Ledger::parse(&rendered).expect("its own output parses");
    assert!(read.functions["double"].public);
    assert!(!read.functions["secret"].public);

    let _ = std::fs::remove_dir_all(dir);
}

/// The determinism of 13.5 is about the tree, not about a file: the same sources
/// produce the same ledger however many times they are read, and the order is the
/// directory sorted rather than the order anything was discovered in.
#[test]
fn the_ledger_of_a_program_is_deterministic() {
    let (dir, entry) = project(
        "deterministic",
        &[
            ("z.nika", "pub fn two() -> i32 { return 2 }\n"),
            ("a.nika", "pub fn one() -> i32 { return 1 }\n"),
            ("main.nika", "fn main() { println(f\"{one()}{two()}\") }\n"),
        ],
    );

    let first = Program::read(&entry).expect("reads").contracts.render();
    let second = Program::read(&entry).expect("reads").contracts.render();
    assert_eq!(first, second);
    assert_eq!(run(&entry, Build::default()).trim(), "12");

    let _ = std::fs::remove_dir_all(dir);
}

/// Two files that reach into each other are one namespace, so there is no cycle
/// to break - only two files.
#[test]
fn two_files_may_reach_into_each_other() {
    let (dir, entry) = project(
        "cycle",
        &[
            ("a.nika", "pub fn one() -> i32 { return 1 }\n"),
            ("b.nika", "pub fn two() -> i32 { return one() + 1 }\n"),
            ("main.nika", "fn main() { println(f\"{two()}\") }\n"),
        ],
    );

    assert_eq!(run(&entry, Build::default()).trim(), "2");
    let _ = std::fs::remove_dir_all(dir);
}

/// **A `use` that names a file beside this one says the rule that replaced it**
/// (ADR-047 D1).
///
/// The shape that used to work is the one most likely to be written, so the
/// message is about the package rather than about a name it could not find.
#[test]
fn a_use_naming_a_sibling_file_says_to_remove_it() {
    let (dir, entry) = project(
        "sibling-use",
        &[
            ("utils.nika", UTILS),
            ("main.nika", "use utils\n\nfn main() { }\n"),
        ],
    );

    let Err(error) = Program::read(&entry) else {
        panic!("`use utils` names a file of this package");
    };
    let message = format!("{error:#}");
    assert!(message.contains("already see one another"), "{message}");
    assert!(message.contains("remove the line"), "{message}");
    let _ = std::fs::remove_dir_all(dir);
}

/// …and a `use` naming something that is not there says depending on a package
/// is not built, rather than looking for a file.
#[test]
fn a_use_naming_another_package_says_it_is_not_built() {
    for source in [
        "use helpers\n\nfn main() { }\n",
        "use net::http\n\nfn main() { }\n",
    ] {
        let (dir, entry) = project("foreign-use", &[("main.nika", source)]);
        let Err(error) = Program::read(&entry) else {
            panic!("{source}");
        };
        let message = format!("{error:#}");
        assert!(message.contains("another package"), "{message}");
        assert!(message.contains("13.2"), "{message}");
        let _ = std::fs::remove_dir_all(dir);
    }

    // `use std::fs` names the library and never a package of yours, so a program
    // that only imports `std` is unaffected.
    let (dir, entry) = project(
        "std-only",
        &[("main.nika", "use std::fs\nuse std::cli\n\nfn main() { }\n")],
    );
    let program = Program::read(&entry).expect("std is not a package of yours");
    assert!(program.is_single_file());
    let _ = std::fs::remove_dir_all(dir);
}

/// **No Rust warning about the generated file reaches the user** (Part III, C.1).
///
/// Measured before this: every multi-file build printed
/// `warning: …/gen:7: unused import: `super::*` (no Nikaia source maps to this)`
/// and told the reader to remove a `use` item this compiler had written itself.
///
/// The import is gone entirely now - one namespace needs no `mod` and nothing to
/// bring the siblings in (ADR-047 D1) - so this asserts the emitted shape as well
/// as the silence. The second half is the one that keeps holding when the next
/// machine-written construct arrives: a **warning** that maps to no Nikaia line is
/// not reported at all.
#[test]
fn a_warning_about_the_generated_file_does_not_reach_the_user() {
    let (dir, entry) = project(
        "silent-preamble",
        &[
            ("plain.nika", "pub fn seven() -> i64 {\n    return 7\n}\n"),
            (
                "main.nika",
                "fn main() {\n\x20   println(f\"{seven()}\")\n}\n",
            ),
        ],
    );

    let program = Program::read(&entry).expect("the program reads");
    let lowered = program.emit(Build::default()).expect("it lowers");
    assert!(
        !lowered.rust.contains("use super::*") && !lowered.rust.contains("pub mod "),
        "one namespace is one crate root:\n{}",
        lowered.rust
    );

    // And the whole way through the CLI, which is where the user was told to
    // remove it.
    let ran = Command::new(env!("CARGO_BIN_EXE_nikaia"))
        .args(["--input", entry.to_str().expect("utf-8 path")])
        .args([
            "--output",
            dir.join("program.rs").to_str().expect("utf-8 path"),
        ])
        .args(["--no-cache"])
        .env("NIKAIA_CACHE_DIR", dir.join("cache"))
        .output()
        .expect("the nikaia binary runs");
    let said = format!(
        "{}{}",
        String::from_utf8_lossy(&ran.stdout),
        String::from_utf8_lossy(&ran.stderr)
    );
    assert!(ran.status.success(), "{said}");
    assert!(
        !said.contains("unused import") && !said.contains("no Nikaia source maps to this"),
        "a warning about the generated file reached the user:\n{said}"
    );
    let _ = std::fs::remove_dir_all(dir);
}

/// **A type another file declares can be named and built**
/// (`docs/open-work.md` §1.1).
///
/// Three things were broken and only the first worked: the *call* was fine, the
/// **type name** resolved to a different type from the one the call handed back,
/// and a struct literal of a type from another file was a parse error. A file
/// could hand out behaviour and not data.
///
/// The package decision (ADR-047 D1) removed the prefix from this case rather
/// than the problem: one namespace, so the names below are bare. What the repair
/// is still for is the cross-*package* boundary, where the prefix comes back.
#[test]
fn a_type_another_file_declares_can_be_named_and_built() {
    let (dir, entry) = project(
        "foreign-type",
        &[
            (
                "pool.nika",
                "pub struct Conn { pub id: i64 }\n\
                 \n\
                 pub fn make() -> Conn {\n\
                 \x20   return Conn(id: 7)\n\
                 }\n\
                 \n\
                 pub fn label(c: Conn) -> i64 {\n\
                 \x20   return c.id\n\
                 }\n",
            ),
            (
                "main.nika",
                "fn main() {\n\
                 \x20   let made: Conn = make()\n\
                 \x20   let built = Conn(id: 35)\n\
                 \x20   println(f\"{label(made)} {label(built)}\")\n\
                 }\n",
            ),
        ],
    );
    assert_eq!(run(&entry, Build::default()).trim(), "7 35");
    let _ = std::fs::remove_dir_all(dir);
}

/// …and the checker still says no where it should. Three shapes, each answered
/// by `rustc` about a generated file before a foreign struct's fields could be
/// read at all.
#[test]
fn a_foreign_type_is_still_checked() {
    for (line, code, says) in [
        ("let c: i64 = make()", "NK1103", "Conn"),
        ("let c = Conn(nmae: 1)", "NK1107", "nmae"),
        ("let c = Conn(id: \"seven\")", "NK1106", "Conn.id"),
    ] {
        let (dir, entry) = project(
            "foreign-type-refused",
            &[
                (
                    "pool.nika",
                    "pub struct Conn { pub id: i64 }\n\
                     \n\
                     pub fn make() -> Conn {\n\
                     \x20   return Conn(id: 7)\n\
                     }\n",
                ),
                ("main.nika", &format!("fn main() {{\n    {line}\n}}\n")),
            ],
        );
        let program = Program::read(&entry).expect("the program reads");
        let library = Ledger::parse(nikaia::contracts::STD).expect("std's ledger parses");
        let packages = program.package_names();
        let found: Vec<_> = program
            .units
            .iter()
            .flat_map(|unit| {
                nikaia::check::check_program(&unit.parsed, &program.contracts, &library, &packages)
                    .findings
            })
            .collect();
        assert_eq!(found.len(), 1, "{line}: {found:#?}");
        assert_eq!(found[0].code, code, "{line}");
        assert!(
            found[0].message.contains(says),
            "{line}: {}",
            found[0].message
        );
        let _ = std::fs::remove_dir_all(dir);
    }
}

/// **A `Shared` in a field another file declares keeps the atomic count**
/// (`docs/open-work.md` §1.6).
///
/// The sharing analysis runs once per **file**, and a package of several files is
/// still several runs of it - so neither run sees the whole of such a field: the
/// file declaring it sees the field and not this value, this one the value and not
/// what the field was decided to be. Measured before the fix, both in one
/// generated file:
///
/// ```text
/// pub db: std::sync::Arc<Conn>,      // decided in pool.nika
/// let c: std::rc::Rc<Conn> = …       // decided in main.nika
/// ```
///
/// which `rustc` refused, about a file nobody wrote. The answer is the polarity
/// this analysis already runs on: where it cannot prove that nothing crosses, it
/// does not lower.
#[test]
fn a_shared_in_a_foreign_field_keeps_the_atomic_count() {
    let (dir, entry) = project(
        "foreign-field",
        &[
            (
                "pool.nika",
                "pub struct Conn { pub id: i64 }\n\
                 \n\
                 pub struct Pool { pub db: Shared[Conn] }\n",
            ),
            (
                "main.nika",
                "fn main() {\n\
                 \x20   let c: Shared[Conn] = Conn(id: 1)\n\
                 \x20   let p = Pool(db: c)\n\
                 \x20   println(f\"{p.db.id}\")\n\
                 }\n",
            ),
        ],
    );

    let program = Program::read(&entry).expect("the program reads");
    let lowered = program.emit(Build::default()).expect("it lowers");
    assert!(
        !lowered.rust.contains("std::rc::Rc"),
        "both sides of the field are the same count, and it is the atomic one:\n{}",
        lowered.rust
    );
    // And it runs, which is the only proof the two agreed.
    assert_eq!(run(&entry, Build::default()).trim(), "1");
    let _ = std::fs::remove_dir_all(dir);
}
