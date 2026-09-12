//! Wrapping Cargo: the generated `Cargo.toml`, and the `rustc` interception.
//!
//! [ADR-002](../../../docs/specification/adr/adr-002.md) D1 decides this and
//! [ADR-003](../../../docs/specification/adr/adr-003.md) D2 decides where it
//! lives: the CLI wrapping and the Cargo instrumentation are the *generic*
//! half, so nothing below knows what a `.nika` file is. A frontend hands over a
//! package description and the extension its own sources carry; this renders
//! the manifest, runs `cargo`, and rewrites one argument on the way to `rustc`.
//!
//! Three properties are what the whole thing rests on, and each was measured
//! against a real `cargo` rather than assumed:
//!
//! * **`RUSTC_WORKSPACE_WRAPPER` reaches workspace members only.** That is the
//!   point of choosing it over `RUSTC_WRAPPER`: a dependency from crates.io is
//!   compiled by the real `rustc`, untouched, and the language's pipeline never
//!   sees a crate it did not write.
//! * **A generated file is only written when it changed** ([`write_if_changed`]).
//!   Cargo decides freshness by comparing mtimes against the fingerprint it
//!   wrote last time, so a generated source rewritten unconditionally is a
//!   package that is dirty on every build, for ever.
//! * **The source `cargo` thinks it compiled is not the source the user
//!   edited**, so the dependency file `rustc` emits names the wrong input.
//!   [`record_extra_dependencies`] puts the real ones back, and without it a
//!   changed source would not rebuild.

use anyhow::{Context, Result};
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// `[package]` of the generated manifest.
#[derive(Debug, Clone)]
pub struct Package {
    pub name: String,
    pub version: String,
    pub edition: String,
}

/// The Cargo profile the build switches and the codegen table decide.
///
/// ADR-002 D1: the build switches "set `opt-level` and the panic strategy from
/// there". `opt-level` and `lto` are a project's own choice about output size
/// and speed (Part III 13.3); `panic` is not a choice at all - it follows from
/// the machine, which is why it arrives here already decided.
#[derive(Debug, Clone, Default)]
pub struct Profile {
    pub opt_level: Option<toml::Value>,
    pub lto: Option<toml::Value>,
    pub panic: Option<String>,
}

impl Profile {
    fn is_empty(&self) -> bool {
        self.opt_level.is_none() && self.lto.is_none() && self.panic.is_none()
    }
}

/// A `Cargo.toml` to be written, and the one binary it builds.
#[derive(Debug, Clone)]
pub struct CargoProject {
    pub package: Package,
    /// The binary's name, and the file Cargo is told is its crate root - which
    /// may carry any extension at all. Cargo does not require `.rs` there, and
    /// that is what lets the wrapper below get a look at it.
    pub bin_name: String,
    pub bin_path: PathBuf,
    /// Rendered verbatim. A native Rust dependency passes through unchanged
    /// (D1), which means this map holds exactly what the author wrote.
    pub dependencies: BTreeMap<String, toml::Value>,
    /// Which Cargo profile [`Profile`] is written under - `dev` for a build
    /// that is not asked to be otherwise.
    pub profile_name: String,
    pub profile: Profile,
}

impl CargoProject {
    /// The manifest text. Generated, so it says so: a file a build wrote is one
    /// a person will find in a diff and wonder about.
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str(
            "# GENERATED from `nikaia.toml`. Do not edit - it is rewritten by every build.\n\
             # Native Rust dependencies pass through unchanged (ADR-002 D1); the profile\n\
             # below carries the codegen table and the machine's panic strategy.\n\n",
        );
        out.push_str("[package]\n");
        out.push_str(&format!("name = {}\n", string(&self.package.name)));
        out.push_str(&format!("version = {}\n", string(&self.package.version)));
        out.push_str(&format!("edition = {}\n", string(&self.package.edition)));

        out.push_str("\n[[bin]]\n");
        out.push_str(&format!("name = {}\n", string(&self.bin_name)));
        out.push_str(&format!(
            "path = {}\n",
            string(&self.bin_path.to_string_lossy())
        ));

        out.push_str("\n[dependencies]\n");
        for (name, value) in &self.dependencies {
            out.push_str(&format!("{} = {value}\n", key(name)));
        }

        if !self.profile.is_empty() {
            out.push_str(&format!("\n[profile.{}]\n", self.profile_name));
            if let Some(opt) = &self.profile.opt_level {
                out.push_str(&format!("opt-level = {opt}\n"));
            }
            if let Some(lto) = &self.profile.lto {
                out.push_str(&format!("lto = {lto}\n"));
            }
            if let Some(panic) = &self.profile.panic {
                out.push_str(&format!("panic = {}\n", string(panic)));
            }
        }

        // The generated package is its own workspace. Without this, a project
        // that happens to sit under someone else's `Cargo.toml` would be read
        // as a member of it and fail for a reason that has nothing to do with
        // the program being built.
        out.push_str("\n[workspace]\n");
        out
    }

    /// Writes the manifest into `dir`, and returns its path.
    pub fn write_to(&self, dir: &Path) -> Result<PathBuf> {
        std::fs::create_dir_all(dir)
            .with_context(|| format!("creating the build directory {}", dir.display()))?;
        let path = dir.join("Cargo.toml");
        write_if_changed(&path, &self.render())?;
        Ok(path)
    }
}

/// A TOML basic string. Paths and names both go through it, because a build
/// directory with a quote in its name is not a reason to emit a broken file.
fn string(value: &str) -> String {
    toml::Value::String(value.to_string()).to_string()
}

/// A bare key where TOML allows one, a quoted key otherwise.
fn key(name: &str) -> String {
    if !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        name.to_string()
    } else {
        string(name)
    }
}

/// Write `contents` to `path`, but **only if that changes the file**.
///
/// Returns whether anything was written. The condition is not an optimisation:
/// Cargo decides a target is fresh by comparing the mtimes of the files its
/// dependency info names against the fingerprint it wrote, so a generated
/// source rewritten with identical bytes is newer than the fingerprint and the
/// package rebuilds - every time, for ever. Measured before it was believed:
/// with an unconditional write, `cargo build` twice in a row recompiles twice.
pub fn write_if_changed(path: &Path, contents: &str) -> Result<bool> {
    if let Ok(existing) = std::fs::read_to_string(path) {
        if existing == contents {
            return Ok(false);
        }
    }
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating {}", parent.display()))?;
        }
    }
    std::fs::write(path, contents).with_context(|| format!("writing {}", path.display()))?;
    Ok(true)
}

/// How `cargo` is invoked for a generated project.
#[derive(Debug, Clone)]
pub struct Cargo {
    pub manifest: PathBuf,
    /// Where Cargo puts its own artifacts. `None` leaves Cargo to its default,
    /// which is what lets `CARGO_TARGET_DIR` still be honoured.
    pub target_dir: Option<PathBuf>,
    /// The program Cargo runs in place of `rustc`, **for workspace members
    /// only** (ADR-002 D1).
    pub wrapper: PathBuf,
    /// Handed to the wrapper, since Cargo owns its argument list and nothing
    /// else can reach it.
    pub env: Vec<(String, OsString)>,
}

impl Cargo {
    /// Runs `cargo <subcommand>`, inheriting stdio so the user sees Cargo's own
    /// progress and `rustc`'s diagnostics as they happen.
    ///
    /// `after` is passed through following `--`, which is how `cargo run` takes
    /// a program's arguments.
    pub fn run(&self, subcommand: &str, args: &[String], after: &[String]) -> Result<i32> {
        let mut command = Command::new(cargo_binary());
        command.arg(subcommand);
        command.arg("--manifest-path").arg(&self.manifest);
        if let Some(dir) = &self.target_dir {
            command.arg("--target-dir").arg(dir);
        }
        command.args(args);
        if !after.is_empty() {
            command.arg("--").args(after);
        }
        command.env("RUSTC_WORKSPACE_WRAPPER", &self.wrapper);
        for (name, value) in &self.env {
            command.env(name, value);
        }

        let status = command
            .status()
            .with_context(|| format!("running `cargo {subcommand}`"))?;
        Ok(status.code().unwrap_or(1))
    }

    /// The same, with `--message-format=json`, handing back what the backend
    /// said instead of letting it reach the terminal.
    ///
    /// **The one channel a frontend can read a backend diagnostic on.** Cargo's
    /// human-readable rendering goes to stderr already formatted and spanned
    /// against a generated file, which is the thing a language frontend must not
    /// let a user see; `--message-format=json` puts every `rustc` diagnostic on
    /// *stdout*, one JSON object per line, where something can trade its byte
    /// offsets for places in the source the user wrote.
    ///
    /// Only stdout is captured. Cargo's progress ("Compiling", "Finished") and
    /// anything it says about resolution stay on stderr and still reach the
    /// user as they happen, because none of that is about the program's text.
    ///
    /// Never used for `run`: the program's own output is on stdout too, and a
    /// compiler that swallowed it to read diagnostics would be reading the wrong
    /// thing at the worst moment.
    pub fn messages(&self, subcommand: &str, args: &[String]) -> Result<(i32, String)> {
        let mut command = Command::new(cargo_binary());
        command.arg(subcommand);
        command.arg("--manifest-path").arg(&self.manifest);
        if let Some(dir) = &self.target_dir {
            command.arg("--target-dir").arg(dir);
        }
        command.arg("--message-format=json");
        command.args(args);
        command.env("RUSTC_WORKSPACE_WRAPPER", &self.wrapper);
        for (name, value) in &self.env {
            command.env(name, value);
        }
        // **stdout piped, stderr inherited, and spelling both out is the point.**
        // `Command::output` pipes *both*, which would swallow Cargo's progress
        // and - worse - anything it says that is not a `rustc` diagnostic at
        // all: a manifest it cannot parse, a dependency it cannot resolve. Those
        // are not about the program's text, nobody translates them, and a build
        // that failed in silence is the one outcome worse than an untranslated
        // error.
        command.stdout(Stdio::piped());
        command.stderr(Stdio::inherit());

        let mut child = command
            .spawn()
            .with_context(|| format!("running `cargo {subcommand}`"))?;
        let mut messages = String::new();
        if let Some(stdout) = child.stdout.as_mut() {
            stdout
                .read_to_string(&mut messages)
                .context("reading what the backend said")?;
        }
        let status = child
            .wait()
            .with_context(|| format!("waiting for `cargo {subcommand}`"))?;
        Ok((status.code().unwrap_or(1), messages))
    }
}

/// The `cargo` to drive. `CARGO` is set by Cargo itself when the compiler is
/// run from a build script or a test, and naming the same one avoids driving a
/// second toolchain by accident.
fn cargo_binary() -> OsString {
    std::env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"))
}

/// One `rustc` invocation, as Cargo handed it to the wrapper.
///
/// Cargo calls the wrapper as `wrapper <rustc> <args…>`, and does so for its
/// own probes (`rustc -vV`) as well as for real compiles. `source` is `None`
/// for everything that is not a compile of a file with the frontend's
/// extension, and such an invocation is passed straight through.
#[derive(Debug, Clone)]
pub struct Invocation {
    pub rustc: OsString,
    pub args: Vec<OsString>,
    /// The index in `args` of the crate root carrying the frontend's extension.
    pub source: Option<usize>,
    pub crate_name: Option<String>,
    pub out_dir: Option<PathBuf>,
    pub extra_filename: Option<String>,
}

impl Invocation {
    /// Splits what Cargo passed. `argv` is the wrapper's own arguments, the
    /// program name already removed.
    pub fn parse(argv: &[OsString], extension: &str) -> Option<Invocation> {
        let (rustc, args) = argv.split_first()?;
        let args: Vec<OsString> = args.to_vec();

        let mut invocation = Invocation {
            rustc: rustc.clone(),
            source: None,
            crate_name: None,
            out_dir: None,
            extra_filename: None,
            args,
        };

        let suffix = format!(".{extension}");
        for index in 0..invocation.args.len() {
            let arg = invocation.args[index].to_string_lossy().into_owned();
            match arg.as_str() {
                "--crate-name" => {
                    invocation.crate_name = invocation
                        .args
                        .get(index + 1)
                        .map(|v| v.to_string_lossy().into_owned());
                }
                "--out-dir" => {
                    invocation.out_dir = invocation.args.get(index + 1).map(PathBuf::from);
                }
                "-C" => {
                    if let Some(value) = invocation.args.get(index + 1) {
                        let value = value.to_string_lossy();
                        if let Some(rest) = value.strip_prefix("extra-filename=") {
                            invocation.extra_filename = Some(rest.to_string());
                        }
                    }
                }
                _ => {
                    if let Some(rest) = arg.strip_prefix("--out-dir=") {
                        invocation.out_dir = Some(PathBuf::from(rest));
                    } else if let Some(rest) = arg.strip_prefix("-Cextra-filename=") {
                        invocation.extra_filename = Some(rest.to_string());
                    } else if arg.ends_with(&suffix) && invocation.source.is_none() {
                        invocation.source = Some(index);
                    }
                }
            }
        }

        Some(invocation)
    }

    /// The crate root Cargo named, if it is one this frontend owns.
    pub fn source_path(&self) -> Option<PathBuf> {
        self.source.map(|index| PathBuf::from(&self.args[index]))
    }

    /// Puts a different file in the crate root's place.
    pub fn replace_source(&mut self, path: &Path) {
        if let Some(index) = self.source {
            self.args[index] = path.as_os_str().to_os_string();
        }
    }

    /// Where `rustc` will write its dependency info, if Cargo asked for any.
    ///
    /// Cargo names the file after the crate and the metadata hash it chose, so
    /// this is derived rather than searched for: a directory scan would pick up
    /// the previous build's file when `--emit` did not include `dep-info`.
    pub fn dep_info(&self) -> Option<PathBuf> {
        let dir = self.out_dir.as_ref()?;
        let name = self.crate_name.as_ref()?;
        let extra = self.extra_filename.clone().unwrap_or_default();
        Some(dir.join(format!("{name}{extra}.d")))
    }

    /// Runs the real `rustc`, and returns its exit code.
    pub fn run(&self) -> Result<i32> {
        let status = Command::new(&self.rustc)
            .args(&self.args)
            .status()
            .with_context(|| format!("running {}", self.rustc.to_string_lossy()))?;
        Ok(status.code().unwrap_or(1))
    }
}

/// What Cargo *resolved*, read out of the `Cargo.lock` it wrote beside the
/// generated manifest.
///
/// The toolchain does not resolve versions and must not start
/// ([ADR-002](../../../docs/specification/adr/adr-002.md) D1) - but
/// [ADR-021](../../../docs/specification/adr/adr-021.md) D2 asks the lockfile to
/// record them, and until this existed they lived only in a generated file under
/// `target/` that nobody commits. So this reads Cargo's answer rather than
/// computing one: a measurement, in D4's sense, of what the declaration in
/// `nikaia.toml` turned into.
///
/// **The whole graph, not the direct dependencies.** The question the record
/// exists to answer is *does this build the same thing for you as for me*, and a
/// transitive crate that resolved differently on the two machines is a different
/// build. That is also why the root package is left out: the generated package is
/// the project, not something it depends on.
///
/// **A list of versions per name, because one name can resolve twice.** Two
/// majors of one crate in a graph is ordinary, and a map from name to a single
/// version would silently drop one of them - a record that quietly stops being
/// the whole answer, which is the failure D2 is about.
///
/// A `Cargo.lock` that is not there is not an error: a `cargo` subcommand that
/// resolves nothing writes none, and there is then nothing to record.
pub fn resolved_versions(
    cargo_lock: &Path,
    root_package: &str,
) -> Result<BTreeMap<String, BTreeSet<String>>> {
    let mut out: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let Ok(text) = std::fs::read_to_string(cargo_lock) else {
        return Ok(out);
    };
    let document: toml::Value =
        toml::from_str(&text).with_context(|| format!("parsing {}", cargo_lock.display()))?;

    let packages = document
        .get("package")
        .and_then(toml::Value::as_array)
        .map(|a| a.as_slice())
        .unwrap_or_default();

    for package in packages {
        let (Some(name), Some(version)) = (
            package.get("name").and_then(toml::Value::as_str),
            package.get("version").and_then(toml::Value::as_str),
        ) else {
            continue;
        };
        if name == root_package {
            continue;
        }
        out.entry(name.to_string())
            .or_default()
            .insert(version.to_string());
    }

    Ok(out)
}

/// Adds `sources` to a `rustc` dependency file as prerequisites of everything
/// it lists as a target.
///
/// This is what keeps `cargo build` honest. `rustc` compiled a *generated*
/// file, so its dependency info names that and nothing else - and Cargo
/// believes it, which means editing the source the user actually wrote would
/// leave the package fresh and the binary stale. The sources go back in here,
/// in Make's own syntax, because that is the file Cargo reads.
///
/// A dependency file that is not there is not an error: `--emit` may simply not
/// have asked for one, and the build has already succeeded by this point.
pub fn record_extra_dependencies(dep_info: &Path, sources: &[PathBuf]) -> Result<()> {
    let Ok(text) = std::fs::read_to_string(dep_info) else {
        return Ok(());
    };
    if sources.is_empty() {
        return Ok(());
    }

    let extra: Vec<String> = sources
        .iter()
        .map(|path| escape_make(&path.to_string_lossy()))
        .collect();

    let mut out = String::new();
    for line in text.lines() {
        // `target: prerequisites` is a rule, and the sources join its
        // prerequisites. A line that starts with whitespace is a continuation,
        // and a bare `file:` line is the empty target `rustc` writes so Make
        // tolerates a deleted input - both are copied through untouched.
        let is_rule = !line.starts_with(char::is_whitespace) && line.contains(": ");
        out.push_str(line);
        if is_rule {
            for source in &extra {
                out.push(' ');
                out.push_str(source);
            }
        }
        out.push('\n');
    }
    for source in &extra {
        out.push_str(source);
        out.push_str(":\n");
    }

    std::fs::write(dep_info, out)
        .with_context(|| format!("recording sources in {}", dep_info.display()))
}

/// Make treats a space as a separator, so a path containing one is escaped.
fn escape_make(path: &str) -> String {
    path.replace(' ', "\\ ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project() -> CargoProject {
        let mut dependencies = BTreeMap::new();
        dependencies.insert(
            "regex".to_string(),
            toml::Value::try_from(BTreeMap::from([("version", "1.5")])).expect("a table"),
        );
        CargoProject {
            package: Package {
                name: "hyper-core".into(),
                version: "0.1.0".into(),
                edition: "2021".into(),
            },
            bin_name: "hyper-core".into(),
            bin_path: PathBuf::from("/p/src/main.nika"),
            dependencies,
            profile_name: "dev".into(),
            profile: Profile {
                opt_level: Some(toml::Value::Integer(3)),
                lto: Some(toml::Value::Boolean(true)),
                panic: Some("unwind".into()),
            },
        }
    }

    /// The whole of D1's first clause: what comes out has to be a manifest
    /// Cargo accepts, with the Rust dependency exactly as it was written.
    #[test]
    fn the_generated_manifest_is_valid_toml_and_keeps_the_dependency() {
        let text = project().render();
        let parsed: toml::Value = toml::from_str(&text).expect("the generated manifest parses");

        assert_eq!(parsed["package"]["name"].as_str(), Some("hyper-core"));
        assert_eq!(
            parsed["dependencies"]["regex"]["version"].as_str(),
            Some("1.5")
        );
        assert_eq!(parsed["bin"][0]["path"].as_str(), Some("/p/src/main.nika"));
        assert_eq!(parsed["profile"]["dev"]["opt-level"].as_integer(), Some(3));
        assert_eq!(parsed["profile"]["dev"]["lto"].as_bool(), Some(true));
        assert_eq!(parsed["profile"]["dev"]["panic"].as_str(), Some("unwind"));
        assert!(
            parsed.get("workspace").is_some(),
            "the generated package owns its own workspace:\n{text}"
        );
    }

    /// Cargo's freshness is mtime against mtime, so an unconditional write is a
    /// package that never goes fresh.
    #[test]
    fn an_unchanged_file_is_not_rewritten() {
        let dir = std::env::temp_dir().join(format!("nikaia-write-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("scratch");
        let path = dir.join("Cargo.toml");

        assert!(write_if_changed(&path, "a").expect("write"), "first write");
        assert!(
            !write_if_changed(&path, "a").expect("write"),
            "identical contents are not written again"
        );
        assert!(
            write_if_changed(&path, "b").expect("write"),
            "changed contents are"
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    fn argv(parts: &[&str]) -> Vec<OsString> {
        parts.iter().map(OsString::from).collect()
    }

    #[test]
    fn a_compile_of_a_frontend_source_is_recognised_and_rewritten() {
        let mut invocation = Invocation::parse(
            &argv(&[
                "/bin/rustc",
                "--crate-name",
                "demo",
                "--edition=2021",
                "src/main.nika",
                "--out-dir",
                "/t/deps",
                "-C",
                "extra-filename=-abc123",
            ]),
            "nika",
        )
        .expect("an invocation");

        assert_eq!(
            invocation.source_path(),
            Some(PathBuf::from("src/main.nika"))
        );
        assert_eq!(
            invocation.dep_info(),
            Some(PathBuf::from("/t/deps/demo-abc123.d"))
        );

        invocation.replace_source(Path::new("/t/gen/demo.rs"));
        assert_eq!(
            invocation.source_path(),
            Some(PathBuf::from("/t/gen/demo.rs"))
        );
    }

    /// Cargo probes the compiler through the wrapper too (`rustc -vV`), and an
    /// invocation with nothing of ours in it must be passed through untouched.
    #[test]
    fn a_probe_carries_no_source_and_is_left_alone() {
        let invocation = Invocation::parse(&argv(&["/bin/rustc", "-vV"]), "nika").expect("parses");
        assert_eq!(invocation.source, None);
        assert_eq!(invocation.dep_info(), None);
    }

    /// Without this the build is silently stale: Cargo reads the file `rustc`
    /// wrote, which names the generated Rust and never the source.
    #[test]
    fn the_real_sources_go_back_into_the_dependency_file() {
        let dir = std::env::temp_dir().join(format!("nikaia-depinfo-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("scratch");
        let path = dir.join("demo.d");
        std::fs::write(&path, "/t/demo: /t/gen/demo.rs\n\n/t/gen/demo.rs:\n").expect("write");

        record_extra_dependencies(&path, &[PathBuf::from("/p/src/main.nika")]).expect("record");
        let text = std::fs::read_to_string(&path).expect("read");

        assert!(
            text.lines()
                .next()
                .expect("a rule line")
                .ends_with("/p/src/main.nika"),
            "the source is a prerequisite of the binary:\n{text}"
        );
        assert!(
            text.contains("\n/p/src/main.nika:\n"),
            "and has a target line of its own, so a deleted file is not a Make error:\n{text}"
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    /// ADR-021 D2's resolved versions, read from Cargo's own answer.
    ///
    /// Two properties that are easy to get wrong and invisible afterwards: the
    /// root package is the project rather than a dependency of it, and **two
    /// versions of one name are both kept**. A graph holding two majors of one
    /// crate is ordinary, and a map from name to a single version would drop
    /// one of them - a record that silently answers less than it claims to.
    #[test]
    fn what_cargo_resolved_is_read_whole() {
        let dir = std::env::temp_dir().join(format!("nikaia-cargolock-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("scratch");
        let path = dir.join("Cargo.lock");
        std::fs::write(
            &path,
            "version = 4\n\n\
             [[package]]\nname = \"greeter\"\nversion = \"0.1.0\"\n\n\
             [[package]]\nname = \"regex\"\nversion = \"1.13.1\"\n\
             source = \"registry+https://github.com/rust-lang/crates.io-index\"\n\n\
             [[package]]\nname = \"windows-sys\"\nversion = \"0.52.0\"\n\n\
             [[package]]\nname = \"windows-sys\"\nversion = \"0.48.0\"\n",
        )
        .expect("write");

        let resolved = resolved_versions(&path, "greeter").expect("reads");
        assert!(
            !resolved.contains_key("greeter"),
            "the root package is the project, not something it depends on: {resolved:?}"
        );
        assert_eq!(
            resolved["regex"],
            BTreeSet::from(["1.13.1".to_string()]),
            "the version Cargo chose, not the constraint the author wrote"
        );
        assert_eq!(
            resolved["windows-sys"],
            BTreeSet::from(["0.48.0".to_string(), "0.52.0".to_string()]),
            "one name can resolve twice, and both versions determine the build"
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    /// A `cargo` subcommand that resolved nothing wrote no lockfile, and there
    /// is then nothing to record - not an error to fail a finished build with
    /// (ADR-021 D12).
    #[test]
    fn a_missing_cargo_lock_records_nothing() {
        let resolved =
            resolved_versions(Path::new("/nonexistent/Cargo.lock"), "greeter").expect("no error");
        assert!(resolved.is_empty());
    }
}
