//! A project build, end to end (ADR-002 D1).
//!
//! `nikaia.toml` becomes a `Cargo.toml`, `cargo` builds it, and what comes out
//! runs. The parts that can be checked without compiling anything - the
//! translation itself - are checked through the library; the parts that cannot
//! are driven through the real binary, because "produces a running binary" is
//! not a claim a unit test can make.
//!
//! The cargo-driven tests share one `CARGO_TARGET_DIR` under the workspace's
//! own `target/`. Each project would otherwise build the compiler's runtime
//! again from nothing, and four projects would pay for it four times.

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use nikaia::project::Project;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// One directory, shared by every test here that actually runs `cargo`.
fn shared_target_dir() -> PathBuf {
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
        .env("CARGO_TARGET_DIR", shared_target_dir())
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

    // The manifest Cargo was handed, and the ledger a build owes (13.5).
    let generated = std::fs::read_to_string(dir.join("target/nikaia/build/Cargo.toml"))
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

/// A program is however many files its entry reaches (Part I 9.1), and all of
/// them have to reach Cargo's dependency info - not just the one Cargo was told
/// about. Editing a *module* is the case that separates "the sources go back in"
/// from "the entry does".
#[test]
fn editing_a_module_rather_than_the_entry_reaches_the_binary() {
    let dir = a_project(
        "project-modules",
        "[package]\nname = \"multi\"\nversion = \"0.1.0\"\n",
        "use utils\n\nfn main() {\n    println(utils::doubled(21))\n}\n",
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
    .expect("edit the module");

    let again = nikaia(&["run"], &dir);
    assert!(again.status.success(), "{}", said(&again));
    assert_eq!(
        String::from_utf8_lossy(&again.stdout).trim(),
        "63",
        "a module Cargo was never told about still invalidates the build: {}",
        said(&again)
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

    let error = Project::open(&dir, None, None, None).expect_err("wasm is not buildable yet");
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
        .env("CARGO_TARGET_DIR", shared_target_dir())
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

    let project = Project::open(&dir, None, None, None).expect("the project opens");
    let rendered = project
        .cargo_project("fn main() {}")
        .expect("the manifest translates")
        .render();
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

    // The codegen table of the *chosen* machine, and only that one.
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
#[test]
fn a_nikaia_package_is_refused_and_says_why() {
    let dir = a_project(
        "project-nikaia-dep",
        "[package]\nname = \"consumer\"\nversion = \"0.1.0\"\n\n\
         [dependencies]\nhttp-server = \"1.2\"\n",
        HELLO,
    );

    let project = Project::open(&dir, None, None, None).expect("the project opens");
    let error = project
        .cargo_project("fn main() {}")
        .expect_err("a Nikaia package has nowhere to come from");
    let text = format!("{error:#}");
    assert!(text.contains("http-server"), "{text}");
    assert!(text.contains("not decided"), "{text}");
    assert!(
        text.contains("type = \\\"rust\\\"") || text.contains("type = \"rust\""),
        "and it says what does work today: {text}"
    );

    std::fs::remove_dir_all(&dir).ok();
}

/// A directory with no manifest is not a project, and saying so beats whatever
/// Cargo would have said about a file it was never given.
#[test]
fn a_directory_without_a_manifest_is_not_a_project() {
    let dir = common::scratch_dir("project-none");
    let error = Project::open(&dir, None, None, None).expect_err("no manifest, no project");
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
