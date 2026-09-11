//! A program of several files (Part I, 9.1).
//!
//! Every test here builds a real directory of `.nika` files and drives the
//! compiler over it, because what is being tested is *resolution* - which files
//! take part, what they may see of each other, and what one ledger over all of
//! them says. None of that can be checked on a string.

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use nikaia::contracts::Ledger;
use nikaia::emit::Profile;
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

/// Lower, compile and run - the only proof that the modules really resolved.
fn run(entry: &Path, profile: Profile) -> String {
    let program = Program::read(entry).expect("the program reads");
    let lowered = program.emit(profile).expect("the program lowers");

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
        "the program did not compile:\n{}\n--- emitted ---\n{}",
        String::from_utf8_lossy(&compiled.stderr),
        lowered.rust
    );

    let run = Command::new(&binary).output().expect("run it");
    assert!(
        run.status.success(),
        "it failed: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    String::from_utf8_lossy(&run.stdout).into_owned()
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

/// Two files, one program - and it runs.
#[test]
fn a_program_of_two_files_compiles_and_runs() {
    let (dir, entry) = project(
        "two-files",
        &[
            ("utils.nika", UTILS),
            (
                "main.nika",
                "use utils\n\
                 \n\
                 fn main() {\n\
                 \x20   println(\"{utils::double(21)} {utils::answer()}\")\n\
                 }\n",
            ),
        ],
    );

    for profile in [Profile::Lite, Profile::Advanced] {
        assert_eq!(run(&entry, profile).trim(), "42 42", "under {profile:?}");
    }
    let _ = std::fs::remove_dir_all(dir);
}

/// Part I 9.2, in Nikaia's words: `NK1110` before `rustc` gets a chance.
///
/// The language below enforces it too - the test below this one proves that -
/// but a reader should not meet the rule as a message about a file they did not
/// write (Part III, C.1).
#[test]
fn reaching_a_private_item_across_files_is_reported() {
    let (dir, entry) = project(
        "private-call",
        &[
            ("utils.nika", UTILS),
            (
                "main.nika",
                "use utils

fn main() { let n = utils::secret() }
",
            ),
        ],
    );

    let program = Program::read(&entry).expect("the program reads");
    let library = Ledger::parse(nikaia::contracts::STD).expect("std's ledger");
    let modules = program.module_names();
    let entry_unit = &program.units[0];
    let findings =
        nikaia::check::check_program(&entry_unit.parsed, &program.contracts, &library, &modules)
            .findings;

    assert_eq!(findings.len(), 1, "{:#?}", findings);
    assert_eq!(findings[0].code, "NK1110");
    assert_eq!(findings[0].message, "`secret` is private to `utils.nika`");

    // …and `utils.nika` itself calls `secret()` unqualified, which is not a
    // call across a boundary and must not be reported.
    let utils = &program.units[1];
    assert!(
        nikaia::check::check_program(&utils.parsed, &program.contracts, &library, &modules)
            .findings
            .is_empty(),
        "a file was reported for calling its own private function"
    );

    let _ = std::fs::remove_dir_all(dir);
}

/// Part I 9.2: a private item is private, and the language below enforces it
/// as well - `pub` becomes `pub`, so a `mod` keeps what it was not given.
#[test]
fn a_private_item_cannot_be_reached_from_another_file() {
    let (dir, entry) = project(
        "privacy",
        &[
            ("utils.nika", UTILS),
            (
                "main.nika",
                "use utils\n\
                 \n\
                 fn main() {\n\
                 \x20   println(\"{utils::secret()}\")\n\
                 }\n",
            ),
        ],
    );

    let program = Program::read(&entry).expect("the program reads");
    let lowered = program.emit(Profile::Advanced).expect("it lowers");
    assert!(
        lowered.rust.contains("fn secret()") && !lowered.rust.contains("pub fn secret()"),
        "a private function was emitted public:\n{}",
        lowered.rust
    );

    let rust = dir.join("program.rs");
    std::fs::write(&rust, &lowered.rust).expect("write the Rust");
    let compiled = common::compile(&rust, &["--crate-type", "bin", "-o", "/dev/null"]);
    assert!(
        !compiled.status.success(),
        "reaching a private item across files compiled:\n{}",
        lowered.rust
    );

    let _ = std::fs::remove_dir_all(dir);
}

/// One ledger for the project (Part III, 13.5), keys qualified the way a caller
/// writes them - which is the shape `std.contracts` has always had.
#[test]
fn the_ledger_is_one_file_with_every_module_in_it() {
    let (dir, entry) = project(
        "ledger",
        &[
            ("utils.nika", UTILS),
            ("main.nika", "use utils\n\nfn main() { }\n"),
        ],
    );

    let program = Program::read(&entry).expect("the program reads");
    let rendered = program.contracts.render();

    assert!(rendered.contains("[fn.\"utils::double\"]"), "{rendered}");
    assert!(rendered.contains("[fn.\"utils::answer\"]"), "{rendered}");
    assert!(rendered.contains("[fn.\"utils::secret\"]"), "{rendered}");
    // The entry's own items keep their bare names, because that is what a call
    // to them writes - the entry is the crate root.
    assert!(rendered.contains("[fn.\"main\"]"), "{rendered}");

    // …and it reads back, which is what `--locked` rests on.
    let read = Ledger::parse(&rendered).expect("its own output parses");
    assert!(read.functions["utils::double"].public);
    assert!(!read.functions["utils::secret"].public);

    let _ = std::fs::remove_dir_all(dir);
}

/// The determinism of 13.5 is about the tree, not about a file: the same
/// sources produce the same ledger however many times they are read.
#[test]
fn the_ledger_of_a_program_is_deterministic() {
    let (dir, entry) = project(
        "deterministic",
        &[
            ("a.nika", "pub fn one() -> i32 { return 1 }\n"),
            ("b.nika", "pub fn two() -> i32 { return 2 }\n"),
            (
                "main.nika",
                "use b\nuse a\n\nfn main() { println(\"{a::one()}{b::two()}\") }\n",
            ),
        ],
    );

    let first = Program::read(&entry).expect("reads").contracts.render();
    let second = Program::read(&entry).expect("reads").contracts.render();
    assert_eq!(first, second);
    // Imported in the order `b`, `a`; emitted and recorded in name order, so
    // the file does not depend on which `use` came first.
    assert!(
        first.find("[fn.\"a::one\"]") < first.find("[fn.\"b::two\"]"),
        "{first}"
    );

    let _ = std::fs::remove_dir_all(dir);
}

/// Two files that import each other are two `mod` blocks in one crate, which
/// the language below allows - so nothing here has to break the cycle, only
/// stop walking it.
#[test]
fn two_modules_may_import_each_other() {
    let (dir, entry) = project(
        "cycle",
        &[
            ("a.nika", "use b\n\npub fn one() -> i32 { return 1 }\n"),
            (
                "b.nika",
                "use a\n\npub fn two() -> i32 { return a::one() + 1 }\n",
            ),
            (
                "main.nika",
                "use a\nuse b\n\nfn main() { println(\"{b::two()}\") }\n",
            ),
        ],
    );

    assert_eq!(run(&entry, Profile::Advanced).trim(), "2");
    let _ = std::fs::remove_dir_all(dir);
}

/// A `use` that names no file says which file it wanted.
#[test]
fn an_import_with_no_file_says_which_file_it_wanted() {
    let (dir, entry) = project(
        "missing",
        &[("main.nika", "use helpers\n\nfn main() { }\n")],
    );

    let Err(error) = Program::read(&entry) else {
        panic!("there is no helpers.nika");
    };
    let message = format!("{error:#}");
    assert!(message.contains("helpers.nika"), "{message}");
    assert!(message.contains("Part I, 9.1"), "{message}");

    let _ = std::fs::remove_dir_all(dir);
}

/// A module of this program is one name. `std::…` has a path and names the
/// library; anything else with a path is refused rather than guessed at.
#[test]
fn a_nested_module_path_is_refused_and_std_is_not() {
    let (dir, entry) = project(
        "nested",
        &[("main.nika", "use net::http\n\nfn main() { }\n")],
    );
    let Err(error) = Program::read(&entry) else {
        panic!("nested paths are not resolved");
    };
    assert!(format!("{error:#}").contains("one name"), "{error:#}");
    let _ = std::fs::remove_dir_all(dir);

    // `use std::fs` is the library and never a file, so a program that only
    // imports `std` is still one unit.
    let (dir, entry) = project(
        "std-only",
        &[("main.nika", "use std::fs\nuse std::cli\n\nfn main() { }\n")],
    );
    let program = Program::read(&entry).expect("std is not a file");
    assert!(program.is_single_file());
    let _ = std::fs::remove_dir_all(dir);
}
