//! The runtime, from the outside: one program, both mechanisms, same answer.
//!
//! [ADR-038](../../../docs/specification/adr/adr-038.md) D3 says a file may be
//! served by the kernel's completion queue or by the blocking path, and that
//! **which one is chosen is a run-time decision**. The claim that carries is
//! not that either mechanism works - the unit tests in `nikaia-std` say that -
//! but that *one compiled binary* runs on both and prints the same thing. A
//! binary built on a machine with `io_uring` has to run on one without it, and
//! the only way to know is to force each route through the same executable.
//!
//! D4's half is checked here too, on the emitted Rust: `fn main` is the
//! runtime's, the program's own `main` is called from inside it, and which
//! pool starts is `user_parallelism`'s answer rather than the operator's.

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

/// Two reads and a write, which is every half of D3 a `.nika` file can reach
/// today - and not one word of `async`, `await` or a runtime type in it
/// (Part I 8.1).
const READS_AND_WRITES: &str = "use std::fs\n\
     \n\
     fn main() throws {\n\
     \x20   let a = fs::read_to_string(\"eins.txt\") catch { \"\".to_string() }\n\
     \x20   let b = fs::read_to_string(\"zwei.txt\") catch { \"\".to_string() }\n\
     \x20   println(f\"{a.len()} {b.len()}\")\n\
     \x20   fs::write(\"drei.txt\", f\"{a}{b}\") catch { return }\n\
     \x20   let back = fs::read_to_string(\"drei.txt\") catch { \"\".to_string() }\n\
     \x20   println(f\"{back.len()}\")\n\
     }";

/// Lower `source` and hand back the emitted Rust.
fn lower(dir: &Path, source: &str, flags: &[&str]) -> String {
    let input = dir.join("main.nika");
    std::fs::write(&input, source).expect("the source");
    let output = dir.join("main.rs");
    let run = Command::new(env!("CARGO_BIN_EXE_nikaia"))
        .args(["--input", input.to_str().unwrap()])
        .args(["--backend", "rust"])
        .args(["--output", output.to_str().unwrap()])
        .args(["--no-cache"])
        .args(flags)
        .env("NIKAIA_CACHE_DIR", dir.join("cache"))
        .output()
        .expect("the nikaia binary runs");
    assert!(
        run.status.success(),
        "lowering failed:\n{}",
        String::from_utf8_lossy(&run.stderr)
    );
    std::fs::read_to_string(&output).expect("the emitted Rust")
}

/// Lower, compile, and hand back the binary.
fn build(dir: &Path, source: &str, flags: &[&str]) -> PathBuf {
    let rust = lower(dir, source, flags);
    let binary = dir.join("program");
    let compiled = common::compile(
        &dir.join("main.rs"),
        &["--crate-type", "bin", "-o", binary.to_str().unwrap()],
    );
    assert!(
        compiled.status.success(),
        "the emitted Rust did not compile:\n{}\n--- emitted ---\n{rust}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    binary
}

/// Run `binary` in a directory of its own, with a runtime configuration file
/// if one is asked for, and hand back what it printed.
fn run(binary: &Path, at: &Path, config: Option<&str>) -> (String, String) {
    std::fs::create_dir_all(at).expect("a directory to run in");
    std::fs::write(at.join("eins.txt"), "Hamburg;12.0\n".repeat(500)).expect("eins");
    std::fs::write(at.join("zwei.txt"), "Bremen;9.5\n".repeat(700)).expect("zwei");

    let mut command = Command::new(binary);
    command.current_dir(at);
    match config {
        Some(text) => {
            let path = at.join("nikaia-runtime.toml");
            std::fs::write(&path, text).expect("the runtime configuration");
            command.env("NIKAIA_RUNTIME_CONFIG", &path);
        }
        None => {
            command.env_remove("NIKAIA_RUNTIME_CONFIG");
        }
    }
    let out = command.output().expect("run the compiled program");
    assert!(
        out.status.success(),
        "the program failed under {config:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    (
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

/// **The fallback actually runs.** One binary, three routes through D3's `std`
/// surface, and the same output from each - which is what "feature-detect at
/// run time, never at compile time" has to mean to be worth anything.
#[test]
fn one_binary_runs_on_both_mechanisms_and_prints_the_same_thing() {
    let dir = common::scratch_dir("runtime-mechanisms");
    let binary = build(&dir, READS_AND_WRITES, &[]);

    // What the machine chose for itself, with no configuration file at all.
    let (detected, _) = run(&binary, &dir.join("detected"), None);
    assert_eq!(
        detected.trim(),
        "6500 7700\n14200",
        "the detected mechanism printed something else"
    );

    // The fallback, pinned: the route an older kernel or a sandbox that
    // forbids the syscalls would take, forced on a machine that may well have
    // the ring. Two workers, so a pair has something to overlap with.
    let (fallback, _) = run(
        &binary,
        &dir.join("fallback"),
        Some("io-method = \"blocking\"\nio-workers = 2\n"),
    );
    assert_eq!(
        fallback, detected,
        "the fallback and the chosen mechanism must print the same thing"
    );
    assert_eq!(
        std::fs::read_to_string(dir.join("fallback").join("drei.txt")).expect("the written file"),
        std::fs::read_to_string(dir.join("detected").join("drei.txt")).expect("the written file"),
        "…and must write the same file"
    );

    // One I/O worker is the default and has to work too: a pair of reads on
    // the fallback then runs one after the other, which is the honest answer
    // for a machine with neither a completion queue nor a thread to spare.
    let (single, _) = run(
        &binary,
        &dir.join("one-worker"),
        Some("io-method = \"blocking\"\nio-workers = 1\n"),
    );
    assert_eq!(single, detected);

    // The completion path, pinned. On a machine that has it, the same answer;
    // on one that does not, a **refusal to start**, because the whole point of
    // pinning a mechanism is to find out whether it is there (D5). Both are
    // allowed here and neither is a silent fallback, which is what is being
    // asserted.
    let at = dir.join("uring");
    std::fs::create_dir_all(&at).expect("a directory");
    std::fs::write(at.join("eins.txt"), "Hamburg;12.0\n".repeat(500)).expect("eins");
    std::fs::write(at.join("zwei.txt"), "Bremen;9.5\n".repeat(700)).expect("zwei");
    let path = at.join("nikaia-runtime.toml");
    std::fs::write(&path, "io-method = \"uring\"\n").expect("the configuration");
    let pinned = Command::new(&binary)
        .current_dir(&at)
        .env("NIKAIA_RUNTIME_CONFIG", &path)
        .output()
        .expect("run the compiled program");
    if pinned.status.success() {
        assert_eq!(
            String::from_utf8_lossy(&pinned.stdout),
            detected,
            "the pinned completion path is the same one"
        );
    } else {
        let why = String::from_utf8_lossy(&pinned.stderr);
        assert!(
            why.contains("io_uring"),
            "a pinned mechanism that is not there must say so: {why}"
        );
    }

    std::fs::remove_dir_all(&dir).ok();
}

/// **ADR-033 D10, end to end.** The overlapped pair at
/// `user_parallelism = no` prints what the sequential program printed and
/// writes the same file - on whichever mechanism this machine has.
///
/// The claim D10 rests on is that putting two reads in flight changes what a
/// pair *costs* and never what it *means*, and the only way to know is to run
/// both programs. `READS_AND_WRITES` is the right shape for it: the two reads
/// are a pair, and everything after them depends on both, so a difference would
/// show in the output and in `drei.txt` rather than only in a timing.
#[test]
fn the_overlapped_pair_prints_what_the_sequential_program_printed() {
    let effects = common::scratch_dir("runtime-overlap-effects");
    let overlapped = lower(&effects, READS_AND_WRITES, &[]);
    assert!(
        overlapped.contains("task::read_pair("),
        "the pair must overlap at `user_parallelism = no`:\n{overlapped}"
    );
    assert!(
        !overlapped.contains("task::both"),
        "…and not on a thread carrying the program's own code:\n{overlapped}"
    );

    let strict = common::scratch_dir("runtime-overlap-strict");
    let sequential = lower(&strict, READS_AND_WRITES, &["--ordering", "strict"]);
    assert!(
        !sequential.contains("task::read_pair("),
        "`--ordering strict` is still the escape:\n{sequential}"
    );

    let overlapped = build(&effects, READS_AND_WRITES, &[]);
    let sequential = build(&strict, READS_AND_WRITES, &["--ordering", "strict"]);
    for config in [
        None,
        Some("io-method = \"blocking\"\nio-workers = 2\n"),
        Some("io-method = \"blocking\"\nio-workers = 1\n"),
    ] {
        let at = effects.join("run");
        let (one, _) = run(&overlapped, &at, config);
        let (other, _) = run(&sequential, &strict.join("run"), config);
        assert_eq!(one, other, "the overlapped program printed something else");
        assert_eq!(
            std::fs::read_to_string(at.join("drei.txt")).expect("the written file"),
            std::fs::read_to_string(strict.join("run").join("drei.txt")).expect("the written file"),
            "…or wrote a different file"
        );
    }

    std::fs::remove_dir_all(&effects).ok();
    std::fs::remove_dir_all(&strict).ok();
}

/// D4, on the emitted Rust: `fn main` starts the runtime, the program's own
/// `main` is called from inside it, and the drain happens after.
#[test]
fn the_emitted_main_starts_the_runtime_before_the_program() {
    let dir = common::scratch_dir("runtime-entry");
    let rust = lower(&dir, READS_AND_WRITES, &[]);

    let start = rust.find("fn main()").expect("a generated entry point");
    let program = rust
        .find("fn __nikaia_main")
        .expect("the program's own main");
    assert!(
        program < start,
        "the program's `main` must be emitted before the entry point that calls it"
    );

    let entry = &rust[start..];
    assert!(
        entry.contains("nikaia_std::rt::start"),
        "the entry point does not start the runtime: {entry}"
    );
    assert!(
        entry.contains("__nikaia_main()"),
        "the entry point does not call the program: {entry}"
    );
    assert!(
        entry.find("rt::start").unwrap() < entry.find("__nikaia_main()").unwrap(),
        "the runtime has to be up before the program's first statement (D4)"
    );
    assert!(
        entry.find("__nikaia_main()").unwrap() < entry.find("finish()").unwrap(),
        "the drain has to be after the program's last statement (ADR-006 D5)"
    );

    std::fs::remove_dir_all(&dir).ok();
}

/// What starts is what `user_parallelism` allows, and the *compiler* is what
/// says so - it is a build switch, not one of D5's four operating settings
/// (ADR-037 D2).
#[test]
fn user_parallelism_decides_which_pool_starts() {
    let dir = common::scratch_dir("runtime-parallelism");

    let sequential = lower(&dir, READS_AND_WRITES, &["--user-parallelism", "no"]);
    assert!(
        sequential.contains("UserCode::Sequential"),
        "`no` must start no pool for user code"
    );
    assert!(!sequential.contains("UserCode::Concurrent"));

    let concurrent = lower(&dir, READS_AND_WRITES, &["--user-parallelism", "yes"]);
    assert!(
        concurrent.contains("UserCode::Concurrent"),
        "`yes` must start one"
    );
    assert!(!concurrent.contains("UserCode::Sequential"));

    std::fs::remove_dir_all(&dir).ok();
}

/// A program with a `main` the wrapper does not recognise keeps its own, and a
/// file with no `main` gains nothing - so a module of a project does not grow
/// a second entry point.
#[test]
fn only_a_plain_main_is_wrapped() {
    let dir = common::scratch_dir("runtime-shapes");

    let no_main = lower(&dir, "fn double(n: i64) -> i64 { return n * 2 }", &[]);
    assert!(
        !no_main.contains("rt::start"),
        "a file with no entry point must not gain one: {no_main}"
    );

    // A `main` that hands back a value is not the shape the wrapper writes, so
    // it is emitted exactly as written rather than guessed at.
    let returns = lower(&dir, "fn main() -> i64 { return 0 }", &[]);
    assert!(
        !returns.contains("__nikaia_main"),
        "a `main` the wrapper cannot type must keep its own name: {returns}"
    );

    std::fs::remove_dir_all(&dir).ok();
}

/// The operator's file is read at startup, and a setting they wrote reaches the
/// runtime (D5). Checked through the program's own report rather than by
/// reading the file back, so it is the runtime that is being asked.
#[test]
fn the_runtime_configuration_file_reaches_the_runtime() {
    let dir = common::scratch_dir("runtime-config");
    // A program that prints what its runtime started on. Not something a
    // `.nika` file can say - which is the point of D3's surface - so it is
    // written in Rust against `std`'s own API, as a `--runtime` flag would be.
    let source = dir.join("report.rs");
    std::fs::write(
        &source,
        "fn main() { println!(\"{}\", nikaia_std::rt::handle().describe()); }\n",
    )
    .expect("the source");
    let binary = dir.join("report");
    let compiled = common::compile(
        &source,
        &["--crate-type", "bin", "-o", binary.to_str().unwrap()],
    );
    assert!(
        compiled.status.success(),
        "{}",
        String::from_utf8_lossy(&compiled.stderr)
    );

    let said = |config: Option<&str>| {
        let at = dir.join(format!("run-{}", config.unwrap_or("none").len()));
        std::fs::create_dir_all(&at).expect("a directory");
        let mut command = Command::new(&binary);
        command.current_dir(&at);
        match config {
            Some(text) => {
                let path = at.join("nikaia-runtime.toml");
                std::fs::write(&path, text).expect("the configuration");
                command.env("NIKAIA_RUNTIME_CONFIG", &path);
            }
            None => {
                command.env_remove("NIKAIA_RUNTIME_CONFIG");
            }
        }
        let out = command.output().expect("run");
        (
            String::from_utf8_lossy(&out.stdout).into_owned(),
            String::from_utf8_lossy(&out.stderr).into_owned(),
        )
    };

    let (default, _) = said(None);
    assert!(default.contains("io-workers=1"), "{default}");
    assert!(default.contains("io-method=auto"), "{default}");
    assert!(default.contains("cleanup-deadline=30s"), "{default}");

    let (tuned, _) = said(Some(
        "io-workers = 3\nio-method = \"blocking\"\ncleanup-deadline = \"2s\"\n",
    ));
    assert!(tuned.contains("io-workers=3"), "{tuned}");
    assert!(tuned.contains("chose blocking"), "{tuned}");
    assert!(tuned.contains("cleanup-deadline=2s"), "{tuned}");

    // A file that does not parse is the operator's to fix, and the program
    // still runs - on the defaults, with the reason said once.
    let (still_ran, complained) = said(Some("io-workers = plenty\n"));
    assert!(still_ran.contains("io-workers=1"), "{still_ran}");
    assert!(complained.contains("io-workers"), "{complained}");

    std::fs::remove_dir_all(&dir).ok();
}

/// `cleanup-deadline` in `nikaia.toml` still compiles, and says where it went
/// (D5). Failing a manifest somebody wrote to the specification that
/// documented it would punish them for the move.
#[test]
fn a_manifest_cleanup_deadline_compiles_and_says_where_it_went() {
    let dir = common::scratch_dir("runtime-moved-key");
    std::fs::write(
        dir.join("nikaia.toml"),
        "[package]\nname = \"moved\"\nversion = \"0.1.0\"\n\n\
         [build]\ncleanup-deadline = \"30s\"\n",
    )
    .expect("the manifest");
    let input = dir.join("main.nika");
    std::fs::write(&input, "fn main() { println(\"ran\") }").expect("the source");

    let run = Command::new(env!("CARGO_BIN_EXE_nikaia"))
        .args(["--input", input.to_str().unwrap()])
        .args(["--backend", "rust"])
        .args(["--output", dir.join("main.rs").to_str().unwrap()])
        .args(["--no-cache"])
        .env("NIKAIA_CACHE_DIR", dir.join("cache"))
        .output()
        .expect("the nikaia binary runs");
    assert!(
        run.status.success(),
        "a manifest with the moved key must still compile:\n{}",
        String::from_utf8_lossy(&run.stderr)
    );
    let said = String::from_utf8_lossy(&run.stderr);
    assert!(said.contains("cleanup-deadline"), "{said}");
    assert!(said.contains("nikaia-runtime.toml"), "{said}");

    std::fs::remove_dir_all(&dir).ok();
}

/// Nothing about the runtime reaches a `.nika` file.
///
/// Not a promise in a comment: the corpus is read and the words are looked for
/// (Part I 8.1, Part III 15.x). Comments are stripped first, because several
/// examples *talk* about there being no `async` and no `await` - which is the
/// claim, and the reason they may say the words.
#[test]
fn no_nika_file_says_async_or_names_a_mechanism() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut checked = 0;
    for dir in ["examples", "benches", "crates/nikaia-std/src"] {
        let Ok(entries) = std::fs::read_dir(root.join(dir)) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("nika") {
                continue;
            }
            let text = std::fs::read_to_string(&path).expect("a .nika file");
            let code: String = text
                .lines()
                .map(|line| match line.find("//") {
                    Some(at) => &line[..at],
                    None => line,
                })
                .collect::<Vec<_>>()
                .join("\n");
            checked += 1;
            for forbidden in [
                "async",
                "await",
                "io_uring",
                "epoll",
                "nikaia_std",
                "Runtime",
            ] {
                assert!(
                    !code.contains(forbidden),
                    "{} says `{forbidden}` in code, and the runtime is invisible \
                     from a Nikaia program (ADR-038 D3)",
                    path.display()
                );
            }
        }
    }
    assert!(checked > 5, "only {checked} .nika files were read");
}
