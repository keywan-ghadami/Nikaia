//! A project build, end to end (ADR-002 D1).
//!
//! `nikaia.toml` becomes a `Cargo.toml`, `cargo` builds it, and what comes out
//! runs. The parts that can be checked without compiling anything - the
//! translation itself - are checked through the library; the parts that cannot
//! are driven through the real binary, because "produces a running binary" is
//! not a claim a unit test can make.
//!
//! The cargo-driven tests share one `NIKAIA_CACHE_DIR` under the workspace's
//! own `target/`. That is the **compiled-`std` cache** of ADR-002 D4 and not a
//! test convenience: each project would otherwise build `std` and everything
//! under it again from nothing, and the sharing is the decision being tested.
//! `CARGO_TARGET_DIR` is deliberately *not* set, because setting it is the one
//! way to switch that cache off.

mod common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use nikaia::project::Project;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// One cache, shared by every test here that actually runs `cargo` - and not the
/// developer's real one, which a test has no business writing into.
fn shared_cache_dir() -> PathBuf {
    repo_root().join("target").join("nikaia-project-tests")
}

/// A project directory with a manifest and an entry point (Part III 13.1).
fn a_project(purpose: &str, manifest: &str, entry: &str) -> PathBuf {
    let dir = common::scratch_dir(purpose);
    std::fs::create_dir_all(dir.join("src")).expect("src");
    std::fs::write(dir.join("nikaia.toml"), manifest).expect("manifest");
    std::fs::write(dir.join("src/main.nika"), entry).expect("entry");
    dir
}

const HELLO: &str = "fn main() {\n    println(\"Hallo Welt, Nikaia!\")\n}\n";

fn nikaia(args: &[&str], dir: &Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_nikaia"))
        .args(args)
        .arg("--project")
        .arg(dir)
        .env("NIKAIA_CACHE_DIR", shared_cache_dir())
        .output()
        .expect("the nikaia binary runs")
}

fn said(output: &std::process::Output) -> String {
    format!(
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

/// The whole of D1 in one test: a manifest is translated, `cargo` builds what
/// comes out, and the binary runs and prints what the program says.
///
/// Then the source is edited and the *new* output has to appear. That second
/// half is not a formality - Cargo decides freshness from the dependency file
/// `rustc` wrote, and `rustc` only ever saw the generated Rust, so without the
/// sources being put back into it an edited program would leave the package
/// fresh and the binary stale.
#[test]
fn a_project_builds_runs_and_notices_an_edit() {
    let dir = a_project(
        "project-run",
        "[package]\nname = \"greeter\"\nversion = \"0.2.0\"\n",
        HELLO,
    );

    let run = nikaia(&["run"], &dir);
    assert!(run.status.success(), "{}", said(&run));
    assert!(
        String::from_utf8_lossy(&run.stdout).contains("Hallo Welt, Nikaia!"),
        "the built binary runs and prints what the program says: {}",
        said(&run)
    );

    // The manifests Cargo was handed, and the ledger a build owes (13.5). The
    // root is the workspace and the package is a member of it, even where it is
    // the only one (ADR-053 D1).
    let root = std::fs::read_to_string(dir.join("target/nikaia/build/Cargo.toml"))
        .expect("the workspace root is written");
    assert!(root.contains("members = [\"greeter\"]"), "{root}");

    let generated = std::fs::read_to_string(dir.join("target/nikaia/build/greeter/Cargo.toml"))
        .expect("the translated manifest is written");
    assert!(generated.contains("name = \"greeter\""), "{generated}");
    assert!(
        generated.contains("src/main.nika"),
        "Cargo is told the `.nika` file is the crate root, which is what gives \
         the wrapper something to intercept:\n{generated}"
    );
    assert!(
        dir.join("nikaia.contracts").is_file(),
        "the ledger is written in the project root"
    );

    std::fs::write(
        dir.join("src/main.nika"),
        "fn main() {\n    println(\"zweiter Versuch\")\n}\n",
    )
    .expect("edit");

    let again = nikaia(&["run"], &dir);
    assert!(again.status.success(), "{}", said(&again));
    assert!(
        String::from_utf8_lossy(&again.stdout).contains("zweiter Versuch"),
        "an edited source reaches the binary: {}",
        said(&again)
    );

    std::fs::remove_dir_all(&dir).ok();
}

/// A program is every file of its package (Part I 9.1), and all of them have to
/// reach Cargo's dependency info - not just the one Cargo was told about.
/// Editing a file that is **not** the entry is the case that separates "the
/// sources go back in" from "the entry does".
#[test]
fn editing_a_second_file_rather_than_the_entry_reaches_the_binary() {
    let dir = a_project(
        "project-modules",
        "[package]\nname = \"multi\"\nversion = \"0.1.0\"\n",
        "fn main() {\n    println(doubled(21))\n}\n",
    );
    std::fs::write(
        dir.join("src/utils.nika"),
        "pub fn doubled(n: i32) -> i32 {\n    return n * 2\n}\n",
    )
    .expect("module");

    let run = nikaia(&["run"], &dir);
    assert!(run.status.success(), "{}", said(&run));
    assert_eq!(String::from_utf8_lossy(&run.stdout).trim(), "42");

    std::fs::write(
        dir.join("src/utils.nika"),
        "pub fn doubled(n: i32) -> i32 {\n    return n * 3\n}\n",
    )
    .expect("edit the second file");

    let again = nikaia(&["run"], &dir);
    assert!(again.status.success(), "{}", said(&again));
    assert_eq!(
        String::from_utf8_lossy(&again.stdout).trim(),
        "63",
        "a file Cargo was never told about still invalidates the build: {}",
        said(&again)
    );

    std::fs::remove_dir_all(&dir).ok();
}

/// A backend diagnostic reaches the user against the `.nika` line that caused
/// it, and not against the generated Rust (ADR-005 D7, Part III C.1).
///
/// **The project path had no interception at all.** `nikaia build` shelled out
/// to `cargo` and handed its stderr to the terminal, so everything `rustc` said
/// arrived as Rust about `target/nikaia/gen/….rs` - a file the author has never
/// read - which C.1's Iron Rule calls a bug in this compiler. The machinery to
/// place it existed and was only reachable through `--explain`; what was missing
/// was the channel, and `cargo --message-format=json` is it.
///
/// The program here is one the frontend says nothing about and `rustc` refuses:
/// `Ty::Unknown` means the checker does not guess at what a method on an integer
/// is, so this is exactly the shape that used to escape to the terminal.
#[test]
fn a_backend_error_is_reported_against_the_nika_line() {
    let dir = a_project(
        "project-backend-error",
        "[package]\nname = \"refused\"\nversion = \"0.1.0\"\n",
        "fn main() {\n    let n = 1\n    println(f\"{n.frobnicate()}\")\n}\n",
    );

    let built = nikaia(&["build"], &dir);
    assert!(!built.status.success(), "{}", said(&built));

    let complaint = said(&built);
    assert!(
        complaint.contains("src/main.nika:3:"),
        "the message names the `.nika` line: {complaint}"
    );
    assert!(
        complaint.contains("n.frobnicate()"),
        "and shows it, with a caret under it: {complaint}"
    );
    assert!(
        !complaint.contains("target/nikaia/gen/refused.rs"),
        "and no longer names the generated Rust: {complaint}"
    );

    std::fs::remove_dir_all(&dir).ok();
}

/// A machine the toolchain cannot build for is refused before `cargo` is
/// started, with what is missing (ADR-037 D1). It never emits code for a
/// different machine than the one named.
#[test]
fn a_machine_the_toolchain_cannot_build_for_is_refused() {
    let dir = a_project(
        "project-wasm",
        "[package]\nname = \"wasmish\"\nversion = \"0.1.0\"\n\n\
         [build]\ntarget = \"wasm32-unknown\"\n",
        HELLO,
    );

    let error = Project::open(&dir, None, None).expect_err("wasm is not buildable yet");
    let text = format!("{error:#}");
    assert!(text.contains("wasm32-unknown"), "{text}");
    assert!(
        dir.join("target").try_exists().is_ok() && !dir.join("target/nikaia/build").exists(),
        "and nothing was generated on the way to failing"
    );

    std::fs::remove_dir_all(&dir).ok();
}

/// `RUSTC_WORKSPACE_WRAPPER` is applied to workspace members only, and that is
/// the reason it was chosen over `RUSTC_WRAPPER`: a crate from crates.io is
/// compiled by the real `rustc` with nothing in between.
///
/// The claim is invisible in the output - a dependency passed *through* the
/// wrapper and one that never reached it produce the same rlib - so the wrapper
/// is asked to write down what Cargo handed it, and the trace is the evidence.
///
/// Needs the network the first time, like any Cargo build of a project with a
/// dependency.
#[test]
fn a_crates_io_dependency_never_reaches_the_wrapper() {
    let dir = a_project(
        "project-crates-io",
        "[package]\nname = \"fetcher\"\nversion = \"0.1.0\"\n\n\
         [dependencies]\nregex = { type = \"rust\", version = \"1.5\" }\n",
        HELLO,
    );
    let trace = dir.join("wrapper.log");

    let built = Command::new(env!("CARGO_BIN_EXE_nikaia"))
        .args(["build", "--project"])
        .arg(&dir)
        .env("NIKAIA_CACHE_DIR", shared_cache_dir())
        .env("NIKAIA_WRAPPER_TRACE", &trace)
        .output()
        .expect("the nikaia binary runs");
    assert!(built.status.success(), "{}", said(&built));

    // Cargo resolved the dependency from crates.io, which is the half of D1
    // this toolchain deliberately does not implement itself.
    let lock = std::fs::read_to_string(dir.join("target/nikaia/build/Cargo.lock"))
        .expect("Cargo wrote a lockfile for the generated package");
    assert!(
        lock.contains("name = \"regex\""),
        "the dependency was resolved by Cargo:\n{lock}"
    );

    let trace = std::fs::read_to_string(&trace).expect("the wrapper wrote a trace");
    assert!(
        trace.lines().any(|line| line == "fetcher lowered"),
        "the project's own crate went through the Nikaia pipeline:\n{trace}"
    );
    assert!(
        !trace.contains("regex"),
        "a dependency from crates.io is never handed to this compiler at all - \
         that is what `RUSTC_WORKSPACE_WRAPPER` buys:\n{trace}"
    );
    assert_eq!(
        trace.lines().filter(|l| l.ends_with(" lowered")).count(),
        1,
        "exactly one crate in the build is Nikaia's:\n{trace}"
    );

    std::fs::remove_dir_all(&dir).ok();
}

/// `nikaia.lock` records the versions Cargo resolved (ADR-021 D2), and the same
/// project resolved twice is the same bytes.
///
/// D2 has asked for this since it was written and it was not there. Since
/// ADR-002 D1 the versions exist - a project build generates a `Cargo.toml` and
/// Cargo resolves a real `Cargo.lock` beside it - so the record was the only
/// missing part, and without it the file whose job is to answer *does this build
/// the same thing for you as for me?* answered only about the Nikaia half.
///
/// Three claims, and the middle one is the reason it is worth a test over a real
/// dependency rather than a fixture:
///
/// 1. the **resolved** version is recorded, not the constraint the author wrote
///    - a measurement, not a claim (D4);
/// 2. the **whole graph**, including a crate nobody wrote down: `regex-syntax`
///    is not in `nikaia.toml` and a transitive crate that resolved differently
///    on two machines is a different build;
/// 3. the file is **byte-identical** for two independent resolutions in two
///    processes, with nothing absolute in it (ADR-005 D8). Two processes is the
///    load-bearing half: a hash map's iteration order is randomised per
///    process, so a same-process comparison would pass over exactly the slip
///    that is easiest to make.
///
/// Needs the network the first time, like the wrapper-trace test above.
#[test]
fn the_lockfile_records_what_cargo_resolved_and_does_so_identically_twice() {
    const MANIFEST: &str = "[package]\nname = \"locker\"\nversion = \"0.1.0\"\n\n\
                            [dependencies]\nregex = { type = \"rust\", version = \"1.5\" }\n";

    let mut locks = Vec::new();
    let mut dirs = Vec::new();
    for _ in 0..2 {
        let dir = a_project("project-lockfile", MANIFEST, HELLO);
        let built = nikaia(&["build"], &dir);
        assert!(built.status.success(), "{}", said(&built));

        let text = std::fs::read_to_string(dir.join("nikaia.lock"))
            .expect("a project build writes nikaia.lock in the root");
        locks.push(text);
        dirs.push(dir);
    }

    let lock = &locks[0];
    let parsed: toml::Value = toml::from_str(lock).expect("the lockfile is valid TOML");
    let dependencies = parsed
        .get("dependencies")
        .and_then(toml::Value::as_table)
        .unwrap_or_else(|| panic!("nikaia.lock records no resolved dependencies:\n{lock}"));

    let regex = dependencies["regex"]
        .as_array()
        .expect("a list of versions, because one name can resolve twice");
    let resolved = regex[0].as_str().expect("a version string");
    assert_eq!(regex.len(), 1, "one `regex` in this graph: {regex:?}");
    assert_ne!(
        resolved, "1.5",
        "`1.5` is what the manifest asked for; the lock records what that \
         turned into (ADR-021 D4)"
    );
    assert_eq!(
        resolved.split('.').count(),
        3,
        "an exact version, not a requirement: {resolved}"
    );
    assert!(
        dependencies.contains_key("regex-syntax"),
        "the whole resolved graph, not the two lines the author wrote - a \
         transitive crate is as much a part of what was built:\n{lock}"
    );

    // D8: nothing absolute, and nothing that moves with the checkout.
    for dir in &dirs {
        assert!(
            !lock.contains(&dir.display().to_string()),
            "an absolute path reached the committed record:\n{lock}"
        );
    }

    assert_eq!(
        locks[0], locks[1],
        "two independent resolutions of one project, in two processes, must be \
         the same bytes (ADR-005 D8)"
    );

    for dir in dirs {
        std::fs::remove_dir_all(dir).ok();
    }
}

/// The translation, without building anything: what `[dependencies]` and
/// `[build.<target>]` become.
#[test]
fn the_manifest_becomes_a_cargo_manifest() {
    let dir = a_project(
        "project-translate",
        "[package]\nname = \"hyper-core\"\nversion = \"0.1.0\"\n\n\
         [dependencies]\nregex = { type = \"rust\", version = \"1.5\" }\n\n\
         [build.x86_64-linux]\nopt-level = 3\nlto = true\n\n\
         [build.wasm32-unknown]\nopt-level = \"z\"\n",
        HELLO,
    );

    let project = Project::open(&dir, None, None).expect("the project opens");
    let members = project.members().expect("the packages resolve");
    let workspace = project
        .cargo_workspace(&members, &["fn main() {}".to_string()])
        .expect("the manifest translates");

    // The program's own crate: the package, and what it depends on.
    let rendered = workspace.members[0].1.render();
    let parsed: toml::Value = toml::from_str(&rendered).expect("and is valid TOML");

    assert_eq!(parsed["package"]["name"].as_str(), Some("hyper-core"));
    assert_eq!(
        parsed["dependencies"]["regex"]["version"].as_str(),
        Some("1.5"),
        "a native Rust dependency passes through unchanged:\n{rendered}"
    );
    assert!(
        parsed["dependencies"]["regex"].get("type").is_none(),
        "`type = \"rust\"` is Nikaia's word and Cargo would refuse it:\n{rendered}"
    );

    // The codegen table of the *chosen* machine, and only that one - on the
    // workspace root, which is where a profile Cargo reads has to be
    // (ADR-053 D1).
    let root = workspace.render();
    let parsed: toml::Value = toml::from_str(&root).expect("the root is valid TOML");
    assert_eq!(parsed["profile"]["dev"]["opt-level"].as_integer(), Some(3));
    assert_eq!(parsed["profile"]["dev"]["lto"].as_bool(), Some(true));
    assert_eq!(
        parsed["profile"]["dev"]["panic"].as_str(),
        Some("unwind"),
        "x86-64 unwinds; the machine decides, not the author (ADR-037 D1)"
    );

    std::fs::remove_dir_all(&dir).ok();
}

/// Part III 13.3 shows `http-server = "1.2"` beside the Rust crate, and nothing
/// anywhere decides what resolving it would mean. Refused with the reason,
/// rather than silently handed to Cargo - which would look for it on crates.io
/// and fail somewhere that explains nothing.
///
/// **A version, specifically.** A path dependency is decided and built
/// ([ADR-047](../../../docs/specification/adr/adr-047.md) D2); what a version
/// would need is a registry, a version grammar and a distribution format, which
/// are the three things [ADR-002](../../../docs/specification/adr/adr-002.md)
/// D1 §5 refuses to guess at. So the message names the form that works.
#[test]
fn a_nikaia_package_is_refused_and_says_why() {
    let dir = a_project(
        "project-nikaia-dep",
        "[package]\nname = \"consumer\"\nversion = \"0.1.0\"\n\n\
         [dependencies]\nhttp-server = \"1.2\"\n",
        HELLO,
    );

    let project = Project::open(&dir, None, None).expect("the project opens");
    let members = project.members().expect("the packages resolve");
    let error = project
        .cargo_workspace(&members, &["fn main() {}".to_string()])
        .expect_err("a Nikaia package has nowhere to come from");
    let text = format!("{error:#}");
    assert!(text.contains("http-server"), "{text}");
    // A **version** is what is not decided: no registry, no version grammar, no
    // distribution format (ADR-002 D1 §5). A path is decided and built
    // (ADR-047 D2), so the message names both ways that work.
    assert!(text.contains("a version is not how one is found"), "{text}");
    assert!(text.contains("path = \"../http-server\""), "{text}");
    assert!(
        text.contains("type = \"rust\""),
        "and the crates.io form too: {text}"
    );

    std::fs::remove_dir_all(&dir).ok();
}

/// A directory with no manifest is not a project, and saying so beats whatever
/// Cargo would have said about a file it was never given.
#[test]
fn a_directory_without_a_manifest_is_not_a_project() {
    let dir = common::scratch_dir("project-none");
    let error = Project::open(&dir, None, None).expect_err("no manifest, no project");
    let text = format!("{error:#}");
    assert!(text.contains("nikaia.toml"), "{text}");
    assert!(
        text.contains("--input"),
        "and it points at the thing that does work for a single file: {text}"
    );

    std::fs::remove_dir_all(&dir).ok();
}

/// The single-file path is unchanged by all of this, and many tests drive it.
/// Adding a subcommand to a CLI that had none is exactly the change that breaks
/// it, so it is asserted here next to what was added.
#[test]
fn the_single_file_path_still_works_without_a_subcommand() {
    let dir = common::scratch_dir("project-single-file");
    let input = dir.join("hello.nika");
    std::fs::write(&input, HELLO).expect("source");
    let output = dir.join("hello.rs");

    let run = Command::new(env!("CARGO_BIN_EXE_nikaia"))
        .args(["--input", input.to_str().unwrap()])
        .args(["--backend", "rust"])
        .args(["--output", output.to_str().unwrap()])
        .env("NIKAIA_CACHE_DIR", dir.join("cache"))
        .output()
        .expect("the nikaia binary runs");
    assert!(run.status.success(), "{}", said(&run));
    assert!(
        std::fs::read_to_string(&output)
            .expect("the Rust is written")
            .contains("fn main()"),
        "the single-file `rust` backend still emits"
    );

    let interpreted = Command::new(env!("CARGO_BIN_EXE_nikaia"))
        .args(["--input", input.to_str().unwrap()])
        .args(["--backend", "interpreter"])
        .env("NIKAIA_CACHE_DIR", dir.join("cache"))
        .output()
        .expect("the nikaia binary runs");
    assert!(interpreted.status.success(), "{}", said(&interpreted));
    assert!(
        String::from_utf8_lossy(&interpreted.stdout).contains("Hallo Welt, Nikaia!"),
        "and the interpreter still runs: {}",
        said(&interpreted)
    );

    std::fs::remove_dir_all(&dir).ok();
}

/// **The compiled `std` is built once per machine, not once per project**
/// (ADR-002 D4).
///
/// The old shape was worse than "slow": `nikaia-std` had the compiler as a build
/// dependency, so Cargo compiled the compiler library again inside *every*
/// project's `target/` - while the installed compiler was running and doing that
/// very lowering. Two projects paid for it twice.
///
/// The claim is checked by the rlib's own modification time, because that is the
/// only thing that can carry it. A "Compiling nikaia-std" line missing from a log
/// says Cargo was quiet; an unmoved mtime says the file was not written.
#[test]
fn a_second_project_links_the_std_the_first_one_built() {
    let first = a_project(
        "project-std-cache-first",
        "[package]\nname = \"first\"\nversion = \"0.1.0\"\n",
        HELLO,
    );
    let built = nikaia(&["build"], &first);
    assert!(built.status.success(), "{}", said(&built));

    // Not "this build created it" - the tests in this file run in parallel and
    // share the cache on purpose, so by now any of them may have been the one to
    // compile it. That is the decision working, and what is left to check is that
    // the *next* build does not write over it.
    let after_first = compiled_stds(&shared_cache_dir());
    assert!(
        !after_first.is_empty(),
        "a project build leaves a compiled std in the cache, under {}",
        shared_cache_dir().display()
    );

    let second = a_project(
        "project-std-cache-second",
        "[package]\nname = \"second\"\nversion = \"0.1.0\"\n",
        HELLO,
    );
    let built = nikaia(&["build"], &second);
    assert!(built.status.success(), "{}", said(&built));

    assert_eq!(
        compiled_stds(&shared_cache_dir()),
        after_first,
        "the second project linked the compiled std the first one left and wrote \
         no new one - neither a second copy nor a fresh write of this one"
    );

    // The defect this decision removes, stated as an absence: nothing of the
    // compiler is in a project's build graph any more.
    let lock = std::fs::read_to_string(second.join("target/nikaia/build/Cargo.lock"))
        .expect("Cargo resolved the generated package");
    for gone in ["name = \"nikaia\"", "name = \"clap\"", "name = \"sha2\""] {
        assert!(
            !lock.contains(gone),
            "`{gone}` was one of the 58 packages that existed only to build std:\n{lock}"
        );
    }

    std::fs::remove_dir_all(&first).ok();
    std::fs::remove_dir_all(&second).ok();
}

/// Every compiled `std` under a cache directory, and when it was written.
///
/// Found on disk rather than derived from the key, so the test asserts what is
/// there instead of repeating the code that put it there. A map and not one path
/// because a cache carried over from an earlier toolchain holds an entry under an
/// older key, and the question is whether *this* build wrote anything.
fn compiled_stds(cache: &Path) -> BTreeMap<PathBuf, Option<std::time::SystemTime>> {
    fn walk(dir: &Path, found: &mut BTreeMap<PathBuf, Option<std::time::SystemTime>>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, found);
            } else if path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("libnikaia_std-") && n.ends_with(".rlib"))
            {
                let when = std::fs::metadata(&path)
                    .ok()
                    .and_then(|m| m.modified().ok());
                found.insert(path, when);
            }
        }
    }

    let mut found = BTreeMap::new();
    walk(cache, &mut found);
    found
}

/// **The explain modes reach a project build** (`docs/open-work.md`, since closed).
///
/// `--sharing`'s own help says why they exist: there is no way to *ask* for the
/// cheaper reference count, every fallback is enumerated instead, and *"that is
/// only fair if the fallbacks can be asked about. This is the asking."* They were
/// on the single-file path only — `nikaia build --sharing` answered `unexpected
/// argument` — so the asking was unavailable exactly where a person with a real
/// program would do it.
///
/// Over **every file** of the package, and against the package's own ledger
/// rather than each file's: `sync`, the touch sets and the sharing classes are
/// whole-program facts, so a report built from one file's inferences would answer
/// a different question from the one the build answers.
#[test]
fn the_explain_modes_reach_a_project_build() {
    let dir = a_project(
        "project-explain",
        "[package]\nname = \"explained\"\nversion = \"0.1.0\"\n",
        "fn main() {\n    \
             let c = Shared(Conn(id: 1))\n    \
             println(f\"{hold(c)}\")\n\
         }\n",
    );
    std::fs::write(
        dir.join("src/pool.nika"),
        "pub struct Conn { pub id: i64 }\n\
         \n\
         pub fn hold(c: Shared[Conn]) -> i64 {\n    \
             return c.id\n\
         }\n",
    )
    .expect("the second file");

    let ran = nikaia(&["build", "--sharing"], &dir);
    let out = String::from_utf8_lossy(&ran.stdout);
    assert!(
        out.contains("`c` (Shared[Conn])"),
        "the value is named: {}",
        said(&ran)
    );
    // Both files are reported, each under its own name - a slot key does not say
    // which file it is in.
    assert!(
        out.contains("main.nika") && out.contains("pool.nika"),
        "{out}"
    );

    // And the other two are accepted on `build` as well.
    for flag in ["--trust", "--overlaps"] {
        let ran = nikaia(&["build", flag], &dir);
        assert!(
            ran.status.success(),
            "`nikaia build {flag}`:\n{}",
            said(&ran)
        );
    }

    std::fs::remove_dir_all(&dir).ok();
}

/// **A package reached through two parents is one package**, and so its types
/// are one type ([ADR-053](../../../docs/specification/adr/adr-053.md) D2).
///
/// The diamond: the program depends on `deep` directly, and on a library that
/// depends on **the same directory** under its own key `c`. Cargo has always
/// made this one crate — D2 rests on that — but the checker in front of it named
/// a type by the manifest key it was reached *through*, so the program was told
/// its `deep::Id` was not the `c::Id` the library takes. One type, refused for
/// having two spellings, with a help line asking for what was already written.
///
/// The library's key is deliberately a word the program never writes, because
/// that is the case that has to work: whose word wins cannot be "whoever was
/// read first". It is the program's, here, and the package's `[package] name`
/// where the program does not name it either.
///
/// Run and not only built: the value is made in one package and read in another,
/// so if the two types were ever really two, the generated Rust would say so.
#[test]
fn a_package_reached_through_two_parents_is_one_package() {
    let dir = common::scratch_dir("project-diamond");
    for at in ["app/src", "lib/src", "deep/src"] {
        std::fs::create_dir_all(dir.join(at)).expect("the packages");
    }
    std::fs::write(
        dir.join("app/nikaia.toml"),
        "[package]\nname = \"app\"\nversion = \"0.1.0\"\n\n\
         [dependencies]\ndeep = { path = \"../deep\" }\nlib = { path = \"../lib\" }\n",
    )
    .expect("the program's manifest");
    std::fs::write(
        dir.join("lib/nikaia.toml"),
        "[package]\nname = \"lib\"\nversion = \"0.1.0\"\n\n\
         [dependencies]\nc = { path = \"../deep\" }\n",
    )
    .expect("the library's manifest, with its own word for the same directory");
    std::fs::write(
        dir.join("deep/nikaia.toml"),
        "[package]\nname = \"deep\"\nversion = \"0.1.0\"\n",
    )
    .expect("the shared package's manifest");

    std::fs::write(
        dir.join("deep/src/main.nika"),
        "pub struct Id {\n    pub value: i64\n}\n\n\
         pub fn make(v: i64) -> Id {\n    return Id(value: v)\n}\n",
    )
    .expect("the shared package");
    std::fs::write(
        dir.join("lib/src/main.nika"),
        "use c\n\npub fn show(id: c::Id) -> i64 {\n    return id.value\n}\n",
    )
    .expect("the library");
    std::fs::write(
        dir.join("app/src/main.nika"),
        "use deep\nuse lib\n\nfn main() {\n\
         \x20   let id = deep::make(7)\n\
         \x20   println(f\"{lib::show(id)}\")\n}\n",
    )
    .expect("the program");

    let ran = nikaia(&["run"], &dir.join("app"));
    assert!(
        ran.status.success(),
        "a package reached through two parents is one package (ADR-053 D2): {}",
        said(&ran)
    );
    assert_eq!(String::from_utf8_lossy(&ran.stdout).trim(), "7");

    std::fs::remove_dir_all(&dir).ok();
}

/// A project and a package it depends on by path.
fn a_program_and_a_package(purpose: &str, dependency: &str, files: &[(&str, &str)]) -> PathBuf {
    let dir = common::scratch_dir(purpose);
    std::fs::create_dir_all(dir.join("app/src")).expect("the program");
    std::fs::create_dir_all(dir.join(format!("{dependency}/src"))).expect("the package");
    std::fs::write(
        dir.join("app/nikaia.toml"),
        format!(
            "[package]\nname = \"app\"\nversion = \"0.1.0\"\n\n\
             [dependencies]\n{dependency} = {{ path = \"../{dependency}\" }}\n"
        ),
    )
    .expect("the program's manifest");
    std::fs::write(
        dir.join(format!("{dependency}/nikaia.toml")),
        format!("[package]\nname = \"{dependency}\"\nversion = \"0.1.0\"\n"),
    )
    .expect("the package's manifest");
    for (file, contents) in files {
        std::fs::write(dir.join(file), contents).expect("a source");
    }
    dir
}

/// **A Nikaia package is depended on by path, and its public surface is what a
/// program may write** ([ADR-047](../../../docs/specification/adr/adr-047.md)
/// D2).
///
/// The first thing in this repository that is two packages. Three claims in one
/// program, because each is a different piece of machinery: a **function** of the
/// package resolves and runs, a **type** it declares can be named and built, and
/// a name it declares in *another of its own files* is visible to it — one
/// namespace per package, one level up from D1.
#[test]
fn a_package_is_depended_on_by_path() {
    let dir = a_program_and_a_package(
        "package-path",
        "http",
        &[
            (
                "http/src/main.nika",
                "pub struct Request { pub path: &str }\n\
                 \n\
                 pub fn ok(body: &str) -> String {\n    \
                     return f\"200 {body}\"\n\
                 }\n",
            ),
            (
                "http/src/routes.nika",
                "pub fn route(r: Request) -> String {\n    \
                     return ok(r.path)\n\
                 }\n",
            ),
            (
                "app/src/main.nika",
                "use http\n\
                 \n\
                 fn main() {\n    \
                     let r = http::Request(path: \"/index\")\n    \
                     println(http::route(r))\n\
                 }\n",
            ),
        ],
    );

    let ran = nikaia(&["run"], &dir.join("app"));
    assert!(ran.status.success(), "{}", said(&ran));
    assert_eq!(
        String::from_utf8_lossy(&ran.stdout).trim(),
        "200 /index",
        "{}",
        said(&ran)
    );
    std::fs::remove_dir_all(&dir).ok();
}

/// **And `NK1110` fires for the first time**: a name a package does not publish
/// cannot be reached from outside it (Part I, 9.2).
///
/// Until a second package existed this check had nothing that could trigger it —
/// the files of one package share a namespace, so a qualified name was a name
/// from a package nobody could depend on ([ADR-047](../../../docs/specification/adr/adr-047.md)
/// §5). This is the day `pub` starts to mean something a program can observe.
#[test]
fn a_name_a_package_does_not_publish_is_refused() {
    let dir = a_program_and_a_package(
        "package-privacy",
        "http",
        &[
            (
                "http/src/main.nika",
                "fn secret() -> i64 {\n    return 41\n}\n\
                 \n\
                 pub fn answer() -> i64 {\n    return secret() + 1\n}\n",
            ),
            (
                "app/src/main.nika",
                "use http\n\nfn main() {\n    println(f\"{http::secret()}\")\n}\n",
            ),
        ],
    );

    let ran = nikaia(&["build"], &dir.join("app"));
    assert!(!ran.status.success(), "{}", said(&ran));
    let out = said(&ran);
    assert!(out.contains("NK1110"), "{out}");
    assert!(out.contains("`secret` is private to `http`"), "{out}");
    assert!(
        out.contains("private to the package that declares it"),
        "{out}"
    );

    // …and what it does publish is reachable, so the rule is a rule and not a
    // blanket refusal.
    std::fs::write(
        dir.join("app/src/main.nika"),
        "use http\n\nfn main() {\n    println(f\"{http::answer()}\")\n}\n",
    )
    .expect("the program");
    let ran = nikaia(&["run"], &dir.join("app"));
    assert!(ran.status.success(), "{}", said(&ran));
    assert_eq!(String::from_utf8_lossy(&ran.stdout).trim(), "42");
    std::fs::remove_dir_all(&dir).ok();
}

/// **A Nikaia dependency is part of the program, not a foreign package**
/// ([ADR-047](../../../docs/specification/adr/adr-047.md) D2 rule 5).
///
/// [ADR-043](../../../docs/specification/adr/adr-043.md) D6 turns the overflow
/// check on for the program and off for every foreign package. A dependency
/// written in this language carries this language's promise, and without the rule
/// stated it would land on the foreign side by accident — that is where a
/// dependency mechanically appears — and a library would compute silently wrong
/// numbers while the program calling it aborted.
///
/// So the test is that the **library's** arithmetic aborts.
#[test]
fn arithmetic_in_a_package_aborts_like_the_programs_own() {
    let dir = a_program_and_a_package(
        "package-overflow",
        "maths",
        &[
            (
                "maths/src/main.nika",
                "pub fn grow(n: i32) -> i32 {\n    return n * 2\n}\n",
            ),
            (
                "app/src/main.nika",
                "use maths\n\nfn main() {\n    println(f\"{maths::grow(2000000000)}\")\n}\n",
            ),
        ],
    );

    let ran = nikaia(&["run"], &dir.join("app"));
    assert!(
        !ran.status.success(),
        "a library's overflow is the program's overflow: {}",
        said(&ran)
    );
    assert!(
        said(&ran).contains("attempt to multiply with overflow"),
        "{}",
        said(&ran)
    );
    std::fs::remove_dir_all(&dir).ok();
}

/// **A trait a package publishes, implemented by a body that calls back into
/// that package** ([ADR-100](../../../docs/specification/adr/adr-100.md) D1,
/// D5) — the shape `open-work.md` carried as a defect for as long as the entry
/// existed.
///
/// It was `NK1129`: *"`Fixed::greet` can pause, and `lib::Greeter` declares it
/// as a method that cannot"*, for a body whose only call is a `String` two
/// files away in another package. The consumer's inference could not see into
/// `lib` at all, so `reach_of` set `blocked` — and `Sync::No` means *"can pause
/// **or** could not be vouched for"*, which every reader takes as the first.
///
/// **Driven through the binary and run**, because what closed it is an ordering
/// between two lowerings: `lib` is lowered first, its ledger is written in its
/// own root, and `app`'s inference reads that answer rather than deriving a
/// worse one. None of that exists inside one `Program`.
#[test]
fn a_package_trait_is_implemented_by_a_body_that_calls_that_package() {
    let dir = a_program_and_a_package(
        "package-trait-call",
        "lib",
        &[
            (
                "lib/src/main.nika",
                "pub trait Greeter {\n\
                 \x20   fn greet(&self) -> String\n\
                 }\n\
                 \n\
                 pub fn hello() -> String {\n\
                 \x20   return \"hello\".to_string()\n\
                 }\n",
            ),
            (
                "app/src/main.nika",
                "use lib\n\
                 \n\
                 struct Fixed { n: i64 }\n\
                 \n\
                 impl lib::Greeter for Fixed {\n\
                 \x20   fn greet(&self) -> String {\n\
                 \x20       return lib::hello()\n\
                 \x20   }\n\
                 }\n\
                 \n\
                 fn main() {\n\
                 \x20   let f = Fixed { n: 1 }\n\
                 \x20   println(f\"{f.greet()}\")\n\
                 }\n",
            ),
        ],
    );
    let app = dir.join("app");

    let ran = nikaia(&["run"], &app);
    assert!(
        ran.status.success(),
        "a body that calls the package it implements against is not a pausing body: {}",
        said(&ran)
    );
    assert_eq!(String::from_utf8_lossy(&ran.stdout).trim(), "hello");

    // **And the dependency's own ledger is in the dependency's own root** (D5),
    // which is what the consumer read: a package's answers are written by the
    // package's build and committed with it, the way `std` has always worked.
    let shipped = std::fs::read_to_string(dir.join("lib/nikaia.contracts"))
        .expect("a package's build writes its own ledger");
    assert!(shipped.contains("[sources]"), "{shipped}");
    assert!(shipped.contains("[fn.\"hello\"]"), "{shipped}");

    std::fs::remove_dir_all(&dir).ok();
}

/// The three rules of [ADR-047](../../../docs/specification/adr/adr-047.md) D2
/// that are about the **set** of dependencies rather than any one of them.
///
/// Each is a message and not a silence, because each is a thing somebody will do:
/// name one package twice, depend on a package that depends on packages, or hand
/// a dependency build settings of its own.
#[test]
fn the_rules_that_come_with_a_path_dependency() {
    let dir = a_program_and_a_package(
        "package-rules",
        "http",
        &[
            (
                "http/src/main.nika",
                "pub fn ok() -> i64 {\n    return 1\n}\n",
            ),
            (
                "app/src/main.nika",
                "use http\n\nfn main() {\n    println(f\"{http::ok()}\")\n}\n",
            ),
        ],
    );
    let app = dir.join("app");
    let manifest = app.join("nikaia.toml");
    let both = std::fs::read_to_string(&manifest).expect("the manifest");

    // Rule 3: the same path is the same package, so two names for it are not two
    // packages - they would be two copies of every type it declares.
    std::fs::write(
        &manifest,
        format!("{both}http2 = {{ path = \"../http\" }}\n"),
    )
    .expect("two names");
    let ran = nikaia(&["build"], &app);
    assert!(!ran.status.success(), "{}", said(&ran));
    assert!(
        said(&ran).contains("are the same package"),
        "{}",
        said(&ran)
    );
    std::fs::write(&manifest, &both).expect("back to one");

    // Rule 2: transitive dependencies are not visible - and since
    // [ADR-053](../../../docs/specification/adr/adr-053.md) D3 that is
    // structural rather than a refusal. A package that depends on a package
    // **builds**, because it is generated as its own crate naming its own
    // dependencies; what the program cannot do is reach past `http` to what
    // `http` depends on, and it cannot because that crate is not one of its own.
    std::fs::create_dir_all(dir.join("deeper/src")).expect("the third package");
    std::fs::write(
        dir.join("deeper/nikaia.toml"),
        "[package]\nname = \"deeper\"\nversion = \"0.1.0\"\n",
    )
    .expect("its manifest");
    std::fs::write(
        dir.join("deeper/src/main.nika"),
        "pub fn two() -> i64 {\n    return 2\n}\n",
    )
    .expect("its source");
    std::fs::write(
        dir.join("http/nikaia.toml"),
        "[package]\nname = \"http\"\nversion = \"0.1.0\"\n\n\
         [dependencies]\ndeeper = { path = \"../deeper\" }\n",
    )
    .expect("a transitive dependency");
    std::fs::write(
        dir.join("http/src/main.nika"),
        "use deeper\n\npub fn ok() -> i64 {\n    return deeper::two()\n}\n",
    )
    .expect("the library uses it");
    let ran = nikaia(&["run"], &app);
    assert!(
        ran.status.success(),
        "a library that uses a library is a library: {}",
        said(&ran)
    );
    assert_eq!(String::from_utf8_lossy(&ran.stdout).trim(), "2");

    // …and the program still cannot name `deeper` itself.
    let program = std::fs::read_to_string(app.join("src/main.nika")).expect("the program");
    std::fs::write(
        app.join("src/main.nika"),
        "use http\n\nfn main() {\n    println(f\"{deeper::two()}\")\n}\n",
    )
    .expect("reaching past http");
    let ran = nikaia(&["build"], &app);
    assert!(!ran.status.success(), "{}", said(&ran));
    let out = said(&ran);
    // The words are `rustc`'s and change between its versions; what this test is
    // about is that the refusal names `deeper` and sits on the line of the
    // program that wrote it.
    assert!(
        out.contains("`deeper`") && out.contains("main.nika:4:5"),
        "a transitive package is not a dependency of this crate, so naming it \
         does not resolve: {out}"
    );
    std::fs::write(app.join("src/main.nika"), &program).expect("back to the program");
    std::fs::write(
        dir.join("http/src/main.nika"),
        "pub fn ok() -> i64 {\n    return 1\n}\n",
    )
    .expect("back to the library");

    // Rule 4: a dependency's own `[build]` is ignored, and the compiler says so -
    // a package is built with the settings of the program that uses it.
    std::fs::write(
        dir.join("http/nikaia.toml"),
        "[package]\nname = \"http\"\nversion = \"0.1.0\"\n\n\
         [build]\nuser-parallelism = \"yes\"\n",
    )
    .expect("its own build section");
    let ran = nikaia(&["run"], &app);
    assert!(ran.status.success(), "{}", said(&ran));
    let out = said(&ran);
    assert_eq!(
        out.matches("is ignored").count(),
        1,
        "said once, not once per pass: {out}"
    );
    assert!(
        out.contains("settings of the program that uses it"),
        "{out}"
    );

    std::fs::remove_dir_all(&dir).ok();
}

/// **A field a package does not publish cannot be reached or written from
/// outside it** (Part I 9.2, [ADR-047](../../../docs/specification/adr/adr-047.md) D2).
///
/// The hole this closes had the shortest fuse in the list: the ledger recorded no
/// per-field `pub`, so a type whose fields were private could be **built by
/// name** from another package with nothing saying no — and the language below
/// cannot help here the way it does for an item, because the emitted struct is in
/// the same crate.
///
/// Two shapes, because they are two pieces of machinery: reading a field, and
/// giving one a value in a struct literal.
#[test]
fn a_field_a_package_does_not_publish_is_refused() {
    let dir = a_program_and_a_package(
        "package-fields",
        "http",
        &[
            (
                "http/src/main.nika",
                "pub struct Request { pub id: i64, method: i64 }\n\
                 \n\
                 pub fn get(id: i64) -> Request {\n    \
                     return Request(id: id, method: 1)\n\
                 }\n\
                 \n\
                 pub fn method_of(r: &Request) -> i64 {\n    \
                     return r.method\n\
                 }\n",
            ),
            (
                "app/src/main.nika",
                "use http\n\nfn main() {\n    \
                     let r = http::get(7)\n    \
                     println(f\"{r.method}\")\n\
                 }\n",
            ),
        ],
    );
    let app = dir.join("app");

    let ran = nikaia(&["build"], &app);
    assert!(!ran.status.success(), "{}", said(&ran));
    let out = said(&ran);
    assert!(out.contains("NK1110"), "{out}");
    assert!(
        out.contains("`http::Request.method` is private to `http`"),
        "{out}"
    );

    // Building it from outside is the same rule, and the one that was silent.
    std::fs::write(
        app.join("src/main.nika"),
        "use http\n\nfn main() {\n    \
             let r = http::Request(id: 7, method: 2)\n    \
             println(f\"{r.id}\")\n\
         }\n",
    )
    .expect("the program");
    let ran = nikaia(&["build"], &app);
    assert!(!ran.status.success(), "{}", said(&ran));
    assert!(
        said(&ran).contains("`http::Request.method` is private"),
        "{}",
        said(&ran)
    );

    // **And the public field, and the method that reaches the private one, are
    // fine** - the rule is a rule and not a blanket refusal. A type may keep its
    // fields private and offer functions, which is Part I 9.3's whole point.
    std::fs::write(
        app.join("src/main.nika"),
        "use http\n\nfn main() {\n    \
             let r = http::get(7)\n    \
             println(f\"{r.id} {http::method_of(&r)}\")\n\
         }\n",
    )
    .expect("the program");
    let ran = nikaia(&["run"], &app);
    assert!(ran.status.success(), "{}", said(&ran));
    assert_eq!(String::from_utf8_lossy(&ran.stdout).trim(), "7 1");

    std::fs::remove_dir_all(&dir).ok();
}

/// **Every abort points at the Nikaia line**
/// ([ADR-044](../../../docs/specification/adr/adr-044.md)).
///
/// [ADR-012](../../../docs/specification/adr/adr-012.md) decides that a
/// diagnostic names the `.nika` file the user wrote, and the compiler kept that
/// promise everywhere it *reported* something. An abort at run time was the one
/// path where it could not: there is no compiler left to translate anything, so
/// Rust's own message named the generated file and line.
///
/// **Four paths in one test, because D2's whole argument is that they are one
/// fix.** An overflow, a conversion that does not fit, a negative index and an
/// index past the end all arrive at the same hook, and a per-case fix would have
/// been four fixes and a fifth the day a fifth abort path arrives.
#[test]
fn every_abort_names_the_nikaia_line() {
    let dir = a_project(
        "project-aborts",
        "[package]\nname = \"aborts\"\nversion = \"0.1.0\"\n",
        HELLO,
    );

    for (what, program, says) in [
        (
            "an overflow",
            "fn double(n: i32) -> i32 {\n    return n * 2\n}\n\
             \n\
             fn main() {\n    \
                 let start = 2000000000\n    \
                 println(f\"{double(start)}\")\n\
             }\n",
            "attempt to multiply with overflow",
        ),
        (
            "a conversion",
            "fn narrow(n: i64) -> i32 {\n    return n as i32\n}\n\
             \n\
             fn main() {\n    \
                 let big = 5000000000\n    \
                 println(f\"{narrow(big)}\")\n\
             }\n",
            "does not fit in an `i32`",
        ),
        (
            "a negative index",
            "fn pick(xs: &Vec[i64], at: i64) -> i64 {\n    return xs[at]\n}\n\
             \n\
             fn main() {\n    \
                 let mut xs = Vec::new()\n    \
                 xs.push(1)\n    \
                 let zero = 0\n    \
                 println(f\"{pick(&xs, zero - 1)}\")\n\
             }\n",
            "index out of bounds: the index is -1",
        ),
        (
            "an index past the end",
            "fn pick(xs: &Vec[i64], at: i64) -> i64 {\n    return xs[at]\n}\n\
             \n\
             fn main() {\n    \
                 let mut xs = Vec::new()\n    \
                 xs.push(1)\n    \
                 let far = 5\n    \
                 println(f\"{pick(&xs, far)}\")\n\
             }\n",
            "index out of bounds: the len is 1",
        ),
    ] {
        std::fs::write(dir.join("src/main.nika"), program).expect("the program");
        let ran = nikaia(&["run"], &dir);
        assert!(!ran.status.success(), "{what} must abort: {}", said(&ran));
        let out = said(&ran);

        // The line the abort happened on - line 2 in every one of these, which is
        // the function's body and not the `main` that called it.
        assert!(
            out.contains("src/main.nika:2: the program stopped:"),
            "{what}: {out}"
        );
        assert!(out.contains(says), "{what}: {out}");
        // And nothing about the file nobody wrote.
        assert!(!out.contains("/gen/"), "{what}: {out}");
        assert!(
            !out.contains("nikaia-std/src/"),
            "{what}: not a line of `std`'s own Rust either: {out}"
        );
    }

    std::fs::remove_dir_all(&dir).ok();
}

/// …and a panic with no Nikaia line behind it is left in the words of whoever
/// wrote it, which is the half that keeps a program from being worse off.
///
/// The table covers the lines the emitter wrote *from* a Nikaia line. A panic
/// inside `std`'s own Rust, or inside a foreign crate, has no Nikaia line to
/// name — and the hook installed before ours, which is Rust's, is what says so.
#[test]
fn an_abort_with_no_nikaia_line_keeps_rusts_own_report() {
    let dir = a_project(
        "project-foreign-abort",
        "[package]\nname = \"foreign\"\nversion = \"0.1.0\"\n",
        HELLO,
    );
    // `expect` on a missing environment variable aborts inside `std`'s Rust: the
    // panic's location is `Option::expect`'s caller, which is a line of the
    // generated file that no `.nika` line maps to, because the emitter wrote it
    // for the f-string rather than from a statement.
    std::fs::write(
        dir.join("src/main.nika"),
        "use std::cli\n\nfn main() {\n    \
             println(f\"{cli::args().nth(40)}\")\n\
         }\n",
    )
    .expect("the program");

    let ran = nikaia(&["run"], &dir);
    let out = said(&ran);
    // Either it runs or it aborts - what must not happen is a line this table
    // invented. This asserts the shape of the claim rather than a particular
    // failure, because which of the two it is belongs to `std`'s ledger and not
    // to this record.
    if !ran.status.success() {
        assert!(
            !out.contains("src/main.nika:1:"),
            "a line the table does not know must not be answered: {out}"
        );
    }
    std::fs::remove_dir_all(&dir).ok();
}

/// **`use http as h` shortens the prefix, once, in one place**
/// ([ADR-046](../../../docs/specification/adr/adr-046.md) D3).
///
/// The one thing that record *adds* rather than refuses, and what makes the
/// qualified-only rule affordable. It also settles a collision: two libraries
/// that both want to be `http` are the consumer's to name apart, which is
/// authority the consumer needs anyway.
///
/// **The alias is resolved away rather than emitted.** A Rust `use … as …` would
/// not do: every file's items are in one crate root (ADR-047 D1), so an alias
/// there would be the whole program's name for the package rather than this
/// file's. `Parsed::unaliased` is the one place it happens, on the way to every
/// name — which is why a call, a type, and a struct literal all work without
/// three separate rules.
#[test]
fn a_package_may_be_given_another_name() {
    let dir = a_program_and_a_package(
        "package-alias",
        "http",
        &[
            (
                "http/src/main.nika",
                "pub struct Request { pub id: i64 }\n\
                 \n\
                 pub fn get(id: i64) -> Request {\n    \
                     return Request(id: id)\n\
                 }\n\
                 \n\
                 pub fn id_of(r: &Request) -> i64 {\n    \
                     return r.id\n\
                 }\n",
            ),
            (
                "app/src/main.nika",
                "use http as h\n\
                 \n\
                 fn main() {\n    \
                     let made: h::Request = h::get(7)\n    \
                     let built = h::Request(id: 35)\n    \
                     println(f\"{h::id_of(&made)} {built.id}\")\n\
                 }\n",
            ),
        ],
    );
    let app = dir.join("app");

    let ran = nikaia(&["run"], &app);
    assert!(ran.status.success(), "{}", said(&ran));
    assert_eq!(String::from_utf8_lossy(&ran.stdout).trim(), "7 35");

    // **The refusals reach through it**, and they name the *package* rather than
    // the alias: the type is `http::Request` whatever this file calls the package,
    // and it is the package that keeps a name private.
    std::fs::write(
        app.join("src/main.nika"),
        "use http as h\n\nfn main() {\n    let n: i64 = h::get(1)\n}\n",
    )
    .expect("the program");
    let ran = nikaia(&["build"], &app);
    assert!(!ran.status.success(), "{}", said(&ran));
    assert!(
        said(&ran).contains("this is `http::Request`, and the `let` says `i64`"),
        "{}",
        said(&ran)
    );

    // **And one name per file counts the alias** (D5): the alias is the name this
    // file introduces, so two of them under one name is the collision D3 exists to
    // let a consumer fix — and therefore the one D5 has to catch.
    std::fs::write(
        app.join("src/main.nika"),
        "use http as h\nuse http as h\n\nfn main() { }\n",
    )
    .expect("the program");
    let ran = nikaia(&["build"], &app);
    assert!(!ran.status.success(), "{}", said(&ran));
    let out = said(&ran);
    assert!(out.contains("`h`") && out.contains("twice"), "{out}");
    assert!(
        out.contains("`use … as …` is how one of them gets another name"),
        "{out}"
    );

    std::fs::remove_dir_all(&dir).ok();
}

/// **A warning reaches the user once, and in this language's words.**
///
/// `nikaia run` used to print one twice: the translated one, and then `rustc`'s
/// own spanned against `target/nikaia/gen/….rs`. The cause was the second Cargo
/// invocation — a `run` cannot use `--message-format=json`, because the
/// program's own output is on that stdout, so Cargo rendered its **cached**
/// diagnostics to stderr while checking freshness and nothing intercepted them.
///
/// The build now runs the binary itself, at the path its own JSON named. Part III
/// C.1 is what this is about: a message about the generated Rust is not a message
/// the user gets.
#[test]
fn a_warning_reaches_the_user_once_and_in_nikaia_terms() {
    let dir = a_project(
        "project-warning-once",
        "[package]\nname = \"noisy\"\nversion = \"0.1.0\"\n",
        // `unused_assignments`: the value `null` is written and never read,
        // which is exactly Part I 2.3's own example.
        "fn main() {\n    \
         let mut maybe: String? = null\n    \
         maybe = \"World\".to_string()\n    \
         let shown = maybe ?? \"x\".to_string()\n    \
         println(f\"{shown}\")\n\
         }\n",
    );

    let run = nikaia(&["run"], &dir);
    assert!(run.status.success(), "{}", said(&run));
    assert!(
        String::from_utf8_lossy(&run.stdout).contains("World"),
        "the program still runs: {}",
        said(&run)
    );

    let stderr = String::from_utf8_lossy(&run.stderr);
    let warnings = stderr.matches("is never read").count();
    assert_eq!(
        warnings,
        1,
        "the warning is said once, not once translated and once raw: {}",
        said(&run)
    );
    assert!(
        stderr.contains("main.nika:"),
        "and against the `.nika` line: {}",
        said(&run)
    );
    assert!(
        !stderr.contains("target/nikaia/gen"),
        "nothing may point at the generated Rust (Part III C.1): {}",
        said(&run)
    );
}

/// **The committed `http` package, built and run by its committed consumer.**
///
/// The diamond test above proves path dependencies work; it builds its packages
/// in a scratch directory, which means the mechanism is covered and no example
/// of it was. [ADR-069](../../../docs/specification/adr/adr-069.md) made `http` a
/// package and §4 said in as many words that the package did not exist — so
/// `use http` answered *"no dependency is called `http`"* and a reader had
/// nothing to copy.
///
/// This runs the real directories rather than a copy, for the reason
/// `examples.rs` gives about its own: the example is the file, so it cannot
/// drift.
///
/// **It also covers the trap that cost the first attempt.** The two dependency
/// arms resolve their paths against different things: a Nikaia path is read by
/// `project::packages_of` and is relative to the manifest's own directory, while
/// a `type = "rust"` one is handed to Cargo verbatim (ADR-002 D1) and is
/// therefore relative to the *generated* manifest under `target/nikaia/build/`.
/// `examples/foreign-runtime/serve` climbs four levels; this climbs one.
#[test]
fn the_http_package_serves_its_example() {
    let consumer = repo_root().join("examples/hello-http");
    let ran = nikaia(&["run"], &consumer);
    assert!(
        ran.status.success(),
        "examples/hello-http builds against examples/http: {}",
        said(&ran)
    );

    let out = String::from_utf8_lossy(&ran.stdout);
    assert!(
        out.starts_with("HTTP/1.1 200 OK\r\n"),
        "the status line ends with CRLF, which is what HTTP asks for: {out:?}"
    );
    assert!(
        out.contains("content-length: 20\r\n\r\nHello from a package"),
        "the headers are separated from the body by a blank line: {out:?}"
    );
}

/// **A trait a package publishes is implemented and then called**
/// ([ADR-095](../../../docs/specification/adr/adr-095.md)).
///
/// The emitter wrote the `impl` and never brought the trait into scope, so the
/// method could not be called and the language below said so in its own words:
/// *"trait `Handler` which provides `handle` is implemented but not in scope;
/// perhaps you want to import it"* — [Part III C.1](../../../docs/specification/30-nikaia-tooling.md)'s
/// class, naming a rule the program has no way to satisfy, because
/// [ADR-046](../../../docs/specification/adr/adr-046.md) D2 gives Nikaia no
/// import to write.
///
/// **It has to be two packages.** The same shape in one file compiles, because
/// there the trait is in the same module and needs no import — which is what
/// said the gap was one emitted line rather than a question. `tests/traits.rs`
/// builds one unit and cannot see this.
///
/// The body is a struct literal rather than a call to `handler::plain()`, and
/// that is not tidiness: a call out of the unit is `NK1129` today
/// (`open-work.md`, *a `sync` body is refused as pausing when the call leaves
/// the unit*), so a fixture that made one would be testing two things and
/// failing for the other.
#[test]
fn a_trait_a_package_publishes_can_be_implemented_and_called() {
    let dir = a_program_and_a_package(
        "foreign-trait",
        "handler",
        &[
            (
                "handler/src/main.nika",
                "pub struct Answer {\n\
                 \x20   pub text: String,\n\
                 }\n\
                 \n\
                 pub trait Handler {\n\
                 \x20   fn handle(&self) -> Answer\n\
                 }\n\
                 \n\
                 pub fn render(answer: &Answer) -> String {\n\
                 \x20   return answer.text.clone()\n\
                 }\n",
            ),
            (
                "app/src/main.nika",
                "use handler\n\n\
                 struct Fixed {\n\
                 \x20   n: i64,\n\
                 }\n\
                 \n\
                 impl handler::Handler for Fixed {\n\
                 \x20   fn handle(&self) -> handler::Answer {\n\
                 \x20       return handler::Answer(text: \"handled\".to_string())\n\
                 \x20   }\n\
                 }\n\
                 \n\
                 fn main() {\n\
                 \x20   let f = Fixed(n: 1)\n\
                 \x20   println(f\"{handler::render(&f.handle())}\")\n\
                 }\n",
            ),
        ],
    );
    let app = dir.join("app");
    let ran = nikaia(&["run"], &app);
    assert!(
        ran.status.success(),
        "a trait reached across a package is callable: {}",
        said(&ran)
    );
    assert_eq!(String::from_utf8_lossy(&ran.stdout).trim(), "handled");

    // **Not asserted against the generated file**, and the reason is worth a
    // line: a package's crate names the `.nika` source as its `path`, so there
    // is no `.rs` on disk to read. What this test has instead is the stronger
    // thing - the program runs, which it cannot do without the import.
}
