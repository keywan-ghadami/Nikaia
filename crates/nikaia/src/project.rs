//! A project build: `nikaia.toml` becomes a `Cargo.toml`, and `cargo` runs it.
//!
//! [ADR-002](../../../docs/specification/adr/adr-002.md) D1. Dependency
//! resolution and linking are solved problems, and a language that cannot reach
//! crates.io starts with no ecosystem - so the toolchain does not resolve
//! versions, does not link, and does not own a build graph. It translates a
//! manifest, hands the result to Cargo, and intercepts the one step Cargo
//! cannot do: turning a `.nika` file into something `rustc` accepts.
//!
//! The interception is `RUSTC_WORKSPACE_WRAPPER`, and the choice of *that*
//! variable rather than `RUSTC_WRAPPER` is the load-bearing part. It is applied
//! to **workspace members only**, so a crate from crates.io is compiled by the
//! real `rustc` with nothing in between - which is what makes "native Rust
//! dependencies pass through unchanged" true of the compilation and not only of
//! the manifest.
//!
//! What is generic about all this lives in
//! [`bridge_orchestrator::project`] ([ADR-003](../../../docs/specification/adr/adr-003.md)
//! D2): rendering a manifest, running `cargo`, splitting a `rustc` command
//! line. What is here is Nikaia's half - which file is the entry, which
//! switches were chosen, and what the lowering does.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use bridge_orchestrator::cache::{Artifacts, Cache, Choices, Layout};
use bridge_orchestrator::project::{
    record_extra_dependencies, write_if_changed, Cargo, CargoProject, Invocation, Package, Profile,
};

use crate::contracts::{sync, Ledger, STD};
use crate::emit::{Build, Ordering, Target};
use crate::manifest::{Dependency, Manifest};
use crate::{check, diagnostics, modules};

/// The extension a Nikaia source carries, and therefore the argument the
/// wrapper is looking for in a `rustc` command line.
pub const EXTENSION: &str = "nika";

/// The entry point of a project, relative to its root (Part III 13.1).
pub const ENTRY: &str = "src/main.nika";

/// Set on the wrapper's own process, and the only thing that tells it apart
/// from an ordinary run: Cargo owns the wrapper's argument list, so there is no
/// flag or subcommand available to say which mode this is.
pub const WRAPPER_MARKER: &str = "NIKAIA_RUSTC_WRAPPER";
const TARGET_VAR: &str = "NIKAIA_BUILD_TARGET";
const PARALLELISM_VAR: &str = "NIKAIA_USER_PARALLELISM";
const ORDERING_VAR: &str = "NIKAIA_ORDERING";
const GEN_DIR_VAR: &str = "NIKAIA_GEN_DIR";
const NO_CACHE_VAR: &str = "NIKAIA_NO_CACHE";

/// A file the wrapper appends one line to per `rustc` it is handed: the crate
/// Cargo named, and whether this compiler lowered it or passed it through.
///
/// It exists because D1's central claim is otherwise invisible. A crate from
/// crates.io compiled by the real `rustc` and one passed *through* this wrapper
/// to the real `rustc` produce identical output, so "dependencies are untouched"
/// cannot be checked by looking at what was built - only by asking what was
/// asked for. Off unless the variable is set.
pub const TRACE_VAR: &str = "NIKAIA_WRAPPER_TRACE";

/// Where the compiler's own runtime lives, for the generated `Cargo.toml`.
///
/// `nikaia-std` is not published, so a generated project reaches it by path.
/// The default is the checkout this compiler was built from, which is what
/// makes a build work with no configuration at all inside the repository;
/// `NIKAIA_STD_PATH` moves it for a compiler installed somewhere else.
const STD_PATH_VAR: &str = "NIKAIA_STD_PATH";

/// The build switches, resolved: flag over manifest over built-in default.
///
/// One value threaded through rather than three fields re-read at each use.
/// Every setting is kept both as the word that named it and as the parsed
/// thing, because the cache key and the diagnostics want the word (ADR-021 D5
/// hashes what was asked for) and the emitter wants the value.
#[derive(Debug, Clone)]
pub struct Settings {
    pub build: Build,
    pub ordering: Ordering,
    pub target: String,
    pub user_parallelism: String,
    pub ordering_word: String,
}

impl Settings {
    /// Resolve every switch before anything is read, so a mistyped one fails on
    /// its own account rather than after a compile (ADR-037 D5).
    pub fn resolve(
        manifest: &Manifest,
        target: Option<&str>,
        user_parallelism: Option<&str>,
        ordering: Option<&str>,
    ) -> Result<Settings> {
        let target = manifest
            .setting("target", target, "x86_64-linux")
            .to_string();
        let user_parallelism = manifest
            .setting("user-parallelism", user_parallelism, "no")
            .to_string();
        let ordering_word = manifest
            .setting("ordering", ordering, "effects")
            .to_string();

        Ok(Settings {
            build: Build::parse(&target, &user_parallelism)?,
            ordering: Ordering::parse(&ordering_word)?,
            target,
            user_parallelism,
            ordering_word,
        })
    }

    /// The words, for the wrapper. Cargo owns the wrapper's arguments, so the
    /// environment is the only channel the switches can travel down - and they
    /// travel already resolved, because a second resolution is a second chance
    /// to disagree.
    fn as_env(&self) -> Vec<(String, OsString)> {
        vec![
            (TARGET_VAR.to_string(), OsString::from(&self.target)),
            (
                PARALLELISM_VAR.to_string(),
                OsString::from(&self.user_parallelism),
            ),
            (
                ORDERING_VAR.to_string(),
                OsString::from(&self.ordering_word),
            ),
        ]
    }

    /// What the driver put in the environment. Absent means the wrapper was run
    /// by hand, and the built-in defaults are the honest answer.
    fn from_env() -> Result<Settings> {
        let word =
            |name: &str, default: &str| std::env::var(name).unwrap_or_else(|_| default.to_string());
        let target = word(TARGET_VAR, "x86_64-linux");
        let user_parallelism = word(PARALLELISM_VAR, "no");
        let ordering_word = word(ORDERING_VAR, "effects");
        Ok(Settings {
            build: Build::parse(&target, &user_parallelism)?,
            ordering: Ordering::parse(&ordering_word)?,
            target,
            user_parallelism,
            ordering_word,
        })
    }

    /// The cache's view of this build (ADR-021 D5).
    pub fn choices(&self) -> Choices {
        Choices::with_ordering(
            format!("{}/{}", self.target, self.user_parallelism),
            "rust",
            &self.ordering_word,
        )
    }

    /// What a panic does on this machine (ADR-037 D1), in Cargo's vocabulary.
    ///
    /// Not a choice, which is why it is not in the codegen table: WebAssembly
    /// traps and has no stack to unwind, and a build that asked for `unwind`
    /// there would be asking for something the machine does not have.
    fn panic_strategy(&self) -> &'static str {
        match self.build.target {
            Target::X86_64Linux => "unwind",
            Target::Wasm32Unknown => "abort",
        }
    }
}

/// Everything one lowering produced.
pub struct Lowered {
    pub rust: String,
    pub ledger: String,
    /// Every `.nika` file that took part, entry first. The wrapper puts these
    /// back into Cargo's dependency info, or a changed module would not
    /// rebuild.
    pub sources: Vec<PathBuf>,
    /// Whether the build cache answered instead of the emitter.
    pub reused: bool,
}

/// Stage 0: parse, check, lower, infer - or serve all four from the cache.
///
/// The build cache (ADR-021) covers the whole of it: on a hit nothing is
/// checked, lowered or inferred, because only a build that passed every check
/// was recorded and the key holds everything those checks depend on (D13).
pub fn lower(input: &Path, settings: &Settings, no_cache: bool) -> Result<Lowered> {
    // `Layout` decides where the lock and the store go, and guarantees that
    // outside a `nikaia.toml` project nothing is written into the source tree.
    // It also names the unit relative to its root: an absolute path is D7's
    // first failure direction, a key that moves with the checkout.
    let layout = Layout::resolve(input);
    let unit = layout.unit_name(input);
    let choices = settings.choices();

    // A cache that cannot be opened is a slower build, never a failed one
    // (D12).
    let mut cache = if no_cache {
        None
    } else {
        match Cache::open(
            &layout.lock,
            &layout.store,
            env!("NIKAIA_RUSTC_VERSION"),
            env!("NIKAIA_COMPILER"),
        ) {
            Ok(cache) => Some(cache),
            Err(error) => {
                eprintln!("warning: the build cache is unavailable: {error:#}");
                None
            }
        }
    };

    // Part I 9.1: every file is a module, so a build is however many files the
    // entry reaches - and the cache cannot be asked about a program until the
    // program is known.
    let program = modules::Program::read(input)?;
    let key_source = program.sources().join("\n// --- unit ---\n");
    let sources: Vec<PathBuf> = program.units.iter().map(|unit| unit.path.clone()).collect();

    // An entry that predates an artifact this build needs is a miss, not a
    // gap: adding an output stays a safe change.
    let cached = cache
        .as_ref()
        .and_then(|cache| cache.lookup(&unit, &key_source, &choices, &layout.root))
        .filter(|artifacts| artifacts.has_all(&[RUST, CONTRACTS]));
    let reused = cached.is_some();

    let (rust, ledger) = match &cached {
        Some(artifacts) => (
            artifacts.get(RUST).expect("checked above").to_string(),
            artifacts.get(CONTRACTS).expect("checked above").to_string(),
        ),
        None => {
            // Every unit is checked against the *program's* contracts, not its
            // own: `utils::double` is a name `main.nika` may write, and the
            // type checker resolves it in the one place any name is resolved.
            let modules = program.module_names();
            for unit in &program.units {
                check(
                    &unit.parsed,
                    &program.contracts,
                    &modules,
                    &unit.path,
                    &unit.source,
                )?;
            }

            let lowered = program.emit_ordered(settings.build, settings.ordering)?;
            let ledger = program.contracts.render();

            if let Some(cache) = &mut cache {
                // Nothing reports assets yet: compile-time I/O
                // (`from "schema.sql"`) is specified and not implemented. The
                // dimension travels through the key regardless, so switching it
                // on later does not reshape the key.
                let artifacts = Artifacts::new()
                    .with(RUST, &lowered.rust)
                    .with(CONTRACTS, &ledger);
                let stored = cache
                    .record(&unit, &key_source, BTreeMap::new(), &choices, &artifacts)
                    .and_then(|()| cache.save());
                if let Err(error) = stored {
                    // The outputs are in hand; only the next build is slower.
                    eprintln!("warning: the build cache could not be updated: {error:#}");
                }
            }
            (lowered.rust, ledger)
        }
    };

    Ok(Lowered {
        rust,
        ledger,
        sources,
        reused,
    })
}

/// The names a lowering stores its outputs under.
pub const RUST: &str = "rust";
pub const CONTRACTS: &str = "contracts";

/// Everything the compiler decides for itself, before it emits a line of Rust.
///
/// Two rules today, and one mechanism under both: the ledger (ADR-020) is what
/// makes either possible across the `std` boundary. A call to
/// `io::read_to_string` is a `sync` violation only if something says that
/// function can pause, and it takes the wrong number of arguments only if
/// something says how many it takes - `std.contracts` is where both are said.
///
/// Types are reported before suspension because a call that passes the wrong
/// thing is usually why the rest of the file reads strangely.
pub fn check(
    parsed: &crate::parser::Parsed,
    own: &Ledger,
    modules: &BTreeSet<String>,
    path: &Path,
    source: &str,
) -> Result<()> {
    let library = Ledger::parse(STD).context("std's shipped ledger")?;

    let all = check::check_program(parsed, own, &library, modules).findings;
    let violations = sync::check(parsed, own, &library);
    if all.is_empty() && violations.is_empty() {
        return Ok(());
    }

    // A warning is printed and does not stop anything. There is one, and it is
    // a migration (ADR-035 D5): a string written before `f"…"` existed looks
    // exactly like one that meant its braces, and neither refusing it nor
    // saying nothing would be right.
    let path = path.display().to_string();
    for finding in &all {
        eprint!("{}", diagnostics::render_finding(finding, &path, source));
    }
    let findings: Vec<&check::Finding> = all
        .iter()
        .filter(|f| f.severity == check::Severity::Error)
        .collect();
    if findings.is_empty() && violations.is_empty() {
        return Ok(());
    }
    for violation in &violations {
        eprint!(
            "{}",
            diagnostics::render_sync_violation(violation, &path, source)
        );
    }

    let mut refused = Vec::new();
    // The `NK1xxx` family is types; anything else the checker reports is a rule
    // of its own and should not be summarised as one. Today that is `NK2701`,
    // a loop whose step can fail in a function that does not say so.
    let (types, rules): (Vec<&check::Finding>, Vec<&check::Finding>) = findings
        .iter()
        .copied()
        .partition(|f| f.code.starts_with("NK1"));

    if !types.is_empty() {
        refused.push(format!(
            "{} type error{}",
            types.len(),
            if types.len() == 1 { "" } else { "s" }
        ));
    }
    if !rules.is_empty() {
        refused.push(format!(
            "{} loop{} that can fail without saying so",
            rules.len(),
            if rules.len() == 1 { "" } else { "s" }
        ));
    }
    if !violations.is_empty() {
        refused.push(format!(
            "{} call{} a `sync` function may not make",
            violations.len(),
            if violations.len() == 1 { "" } else { "s" }
        ));
    }
    bail!("{}", refused.join(", "))
}

/// Write the ledger, or - under `--locked` - check that it did not need
/// writing.
///
/// The determinism guarantee (13.5) is what lets this compare bytes rather than
/// meanings: the same sources and the same compiler produce the same file, so a
/// difference is a change in a contract and never in the formatting.
pub fn write_ledger(path: &Path, ledger: &str, locked: bool) -> Result<()> {
    if !locked {
        std::fs::write(path, ledger)?;
        return Ok(());
    }

    let committed = std::fs::read_to_string(path).with_context(|| {
        format!(
            "--locked, but {} is not there; run without --locked to write it",
            path.display()
        )
    })?;

    if committed != *ledger {
        bail!(
            "--locked: the contracts changed and {} does not say so.\n\
             What the build inferred and the ledger does not have:\n{}\n\
             Run without --locked to record it, and read the diff.",
            path.display(),
            changed_lines(ledger, &committed).join("\n")
        );
    }

    Ok(())
}

/// The lines the build produced that the committed ledger does not have, each
/// under the entry it belongs to.
///
/// Naming the entry is the whole point. A contract line is short and repeats -
/// `sync = true` says nothing on its own, and since ADR-027 it is the commonest
/// line in the file - so a bare list of them tells you that *something* changed
/// and leaves you to find out what. 13.5 asks this diff to narrate a cause; it
/// cannot do that without saying whose contract moved.
pub fn changed_lines(ledger: &str, committed: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut entry: Option<&str> = None;
    let mut named: Option<&str> = None;

    for line in ledger.lines() {
        if line.starts_with('[') {
            entry = Some(line);
            named = None;
        }
        if line.is_empty() || line.starts_with('#') || committed.lines().any(|c| c == line) {
            continue;
        }
        match entry.filter(|e| *e != line) {
            // A line inside an entry, under the entry's name - printed once,
            // however many of its lines changed.
            Some(entry) => {
                if named != Some(entry) {
                    out.push(format!("  {entry}"));
                    named = Some(entry);
                }
                out.push(format!("      {line}"));
            }
            // The line *is* the entry - a whole contract that is new - or it is
            // a header line, which belongs to no entry. Either way it names
            // itself, and an entry that has named itself must not be named
            // again by the lines that follow it.
            None => {
                out.push(format!("  {line}"));
                named = entry;
            }
        }
    }

    out
}

/// A project: its root, its manifest, and the switches this build resolved.
#[derive(Debug)]
pub struct Project {
    pub root: PathBuf,
    pub manifest: Manifest,
    pub settings: Settings,
}

impl Project {
    /// Find the project governing `start` and read its manifest.
    ///
    /// The walk to the root is `Layout::resolve`'s - the same one the build
    /// cache uses - so a build and the cache entries it writes can never
    /// disagree about which project a file belongs to.
    pub fn open(
        start: &Path,
        target: Option<&str>,
        user_parallelism: Option<&str>,
        ordering: Option<&str>,
    ) -> Result<Project> {
        // `Layout::resolve` searches from an input file's *parent*, so it is
        // handed the manifest's own path: the search then starts at `start`
        // itself, which is what "build the project I am standing in" means.
        let layout = Layout::resolve(&start.join("nikaia.toml"));
        if !layout.in_project {
            bail!(
                "no `nikaia.toml` in {} or any directory above it.\n\
                 A project build needs a manifest (Part III 13.1); a single file \
                 is compiled with `nikaia --input {}`.",
                start.display(),
                start.join("main.nika").display()
            );
        }

        let manifest = Manifest::read(&layout.root.join("nikaia.toml"))?;
        // ADR-038 D5 moved `cleanup-deadline` out of the manifest. The key is
        // still accepted, and the note is what keeps the move from being
        // silent - a manifest whose setting stopped being read without saying
        // so is the mistake `[build]`'s unknown-key check already exists for.
        for note in manifest.notes() {
            eprintln!("note: {note}");
        }
        let settings = Settings::resolve(&manifest, target, user_parallelism, ordering)?;
        if let Some(missing) = settings.build.target.unbuildable() {
            bail!(
                "cannot build for `{}` yet: {missing}",
                settings.build.target.triple()
            );
        }

        Ok(Project {
            root: layout.root,
            manifest,
            settings,
        })
    }

    /// `src/main.nika` (Part III 13.1).
    pub fn entry(&self) -> PathBuf {
        self.root.join(ENTRY)
    }

    /// Where the generated `Cargo.toml` goes. Under `target/`, because it is
    /// build output: it is regenerated from `nikaia.toml` on every build and
    /// editing it would be editing something the next build overwrites.
    pub fn build_dir(&self) -> PathBuf {
        self.root.join("target").join("nikaia").join("build")
    }

    /// Where the emitted Rust goes - **outside** [`Self::build_dir`], and that
    /// is not tidiness. Cargo lists the files of a path package to decide
    /// whether it is fresh, so a generated source inside the package would make
    /// the package dirty the moment it was written, and every build would
    /// recompile.
    pub fn gen_dir(&self) -> PathBuf {
        self.root.join("target").join("nikaia").join("gen")
    }

    /// The ledger, in the project root (Part III 13.5).
    pub fn ledger_path(&self) -> PathBuf {
        self.root.join("nikaia.contracts")
    }

    /// `nikaia.toml` translated (ADR-002 D1).
    ///
    /// `rust` is the program as this build lowered it, and it decides which of
    /// the compiler's own runtime crates the generated package declares - a
    /// program that names none of them depends on nothing and builds in a
    /// second.
    pub fn cargo_project(&self, rust: &str) -> Result<CargoProject> {
        let name = self.manifest.package_name().ok_or_else(|| {
            anyhow!(
                "{}/nikaia.toml has no `[package] name`, and Cargo needs one to \
                 name the binary (Part III 13.3)",
                self.root.display()
            )
        })?;

        let mut dependencies = runtime_dependencies(rust)?;
        for (dependency, value) in self.manifest.dependencies() {
            match value {
                // The whole point of D1: whatever the author wrote reaches
                // Cargo, and Cargo resolves and links it as it would for any
                // Rust project.
                Dependency::Rust(value) => {
                    dependencies.insert(dependency.clone(), value.clone());
                }
                // Nothing decides what this means yet. A registry, a name
                // space and a distribution format are all undecided, and
                // guessing at one here would be inventing the answer in the
                // place it is hardest to review.
                Dependency::Nikaia(_) => bail!(
                    "`{dependency}` is a Nikaia package, and how a Nikaia package is \
                     resolved is not decided yet: no ADR names a registry, a version \
                     grammar or a distribution format for one.\n\
                     A crate from crates.io works today - write it as \
                     `{dependency} = {{ type = \"rust\", version = \"…\" }}` \
                     (Part III 13.3, ADR-002 D1)."
                ),
            }
        }

        let codegen = self.manifest.codegen_for(&self.settings.target);
        Ok(CargoProject {
            package: Package {
                name: name.to_string(),
                version: self.manifest.package_version().to_string(),
                // What the emitter writes, and what the tests compile it as.
                edition: "2021".to_string(),
            },
            bin_name: name.to_string(),
            bin_path: self.entry(),
            dependencies,
            // One profile, because a Nikaia build has no dev/release division
            // to map onto: `[build.<target>]` is the project's single statement
            // about output size and speed, and `dev` is the profile Cargo uses
            // when nothing asks otherwise.
            profile_name: "dev".to_string(),
            profile: Profile {
                opt_level: codegen.get("opt-level").cloned(),
                lto: codegen.get("lto").cloned(),
                panic: Some(self.settings.panic_strategy().to_string()),
            },
        })
    }

    /// Build the project, or build and run it.
    ///
    /// The lowering happens twice and that is deliberate: **here**, so the
    /// checks and the ledger are the compiler's own output rather than
    /// something buried in a `cargo` subprocess, and again inside the wrapper,
    /// where it is what `rustc` is actually handed. Both go through [`lower`]
    /// and the second is a cache hit, so the work is done once even though the
    /// decision is made in two places.
    pub fn drive(
        &self,
        subcommand: &str,
        program_args: &[String],
        no_cache: bool,
        locked: bool,
    ) -> Result<i32> {
        let entry = self.entry();
        if !entry.is_file() {
            bail!(
                "{} is not there. A project's entry point is `{ENTRY}` (Part III 13.1).",
                entry.display()
            );
        }

        let lowered = lower(&entry, &self.settings, no_cache)?;
        write_ledger(&self.ledger_path(), &lowered.ledger, locked)?;

        let manifest = self
            .cargo_project(&lowered.rust)?
            .write_to(&self.build_dir())?;

        let mut env = self.settings.as_env();
        env.push((WRAPPER_MARKER.to_string(), OsString::from("1")));
        env.push((GEN_DIR_VAR.to_string(), self.gen_dir().into_os_string()));
        if no_cache {
            env.push((NO_CACHE_VAR.to_string(), OsString::from("1")));
        }

        let cargo = Cargo {
            manifest,
            // `CARGO_TARGET_DIR` is honoured where it is set, which is what
            // lets a test share one directory across runs instead of building
            // the runtime again for every project it makes.
            target_dir: match std::env::var_os("CARGO_TARGET_DIR") {
                Some(_) => None,
                None => Some(self.root.join("target").join("nikaia").join("cargo")),
            },
            wrapper: std::env::current_exe().context("finding this compiler's own path")?,
            env,
        };

        cargo.run(subcommand, &[], program_args)
    }
}

/// What this particular program links against, out of the three crates the
/// emitter can name.
///
/// Declared because the *emitted Rust* names them, not because a program might:
/// a program with no grammar in it depends on nothing at all and builds without
/// fetching anything, and a manifest that declared three crates regardless would
/// make every project pay for the ones it does not use.
///
/// The versions are read out of the compiler's own workspace rather than
/// written down a second time. A program that linked a different `winnow` from
/// the one `winnow-grammar` was built against is a type error at every `Stream`
/// bound, so the two must not be able to drift apart.
fn runtime_dependencies(rust: &str) -> Result<BTreeMap<String, toml::Value>> {
    let mut out = BTreeMap::new();
    let std_path = std_path();

    if rust.contains("nikaia_std") {
        let mut table = toml::Table::new();
        table.insert(
            "path".to_string(),
            toml::Value::String(std_path.to_string_lossy().into_owned()),
        );
        out.insert("nikaia-std".to_string(), toml::Value::Table(table));
    }

    // `winnow_grammar` contains `winnow`, so a grammar pulls in both - which is
    // right: the expansion writes `winnow::Parser` as well.
    let wanted: Vec<&str> = ["winnow-grammar", "winnow"]
        .into_iter()
        .filter(|name| rust.contains(&name.replace('-', "_")))
        .collect();
    if wanted.is_empty() {
        return Ok(out);
    }

    let workspace = std_path
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| anyhow!("{} has no workspace above it", std_path.display()))?
        .join("Cargo.toml");

    let text = std::fs::read_to_string(&workspace).with_context(|| {
        format!(
            "reading {} for the runtime's versions; set {STD_PATH_VAR} if the \
             compiler's checkout has moved",
            workspace.display()
        )
    })?;
    let document: toml::Value =
        toml::from_str(&text).with_context(|| format!("parsing {}", workspace.display()))?;

    for crate_name in wanted {
        let value = document
            .get("workspace")
            .and_then(|w| w.get("dependencies"))
            .and_then(|d| d.get(crate_name))
            .ok_or_else(|| {
                anyhow!(
                    "{} does not declare `{crate_name}` in `[workspace.dependencies]`, \
                     and a generated program names it directly",
                    workspace.display()
                )
            })?;
        out.insert(crate_name.to_string(), value.clone());
    }

    Ok(out)
}

/// The `nikaia-std` a generated project depends on.
fn std_path() -> PathBuf {
    match std::env::var_os(STD_PATH_VAR) {
        Some(path) => PathBuf::from(path),
        // The checkout this compiler was built from. `nikaia-std` is not
        // published, so there is nowhere else to point.
        None => {
            let here = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            let path = here.join("..").join("nikaia-std");
            path.canonicalize().unwrap_or(path)
        }
    }
}

/// The wrapper: what Cargo runs in `rustc`'s place, for workspace members.
///
/// Cargo calls this as `nikaia <rustc> <args…>`, for its own probes as well as
/// for real compiles, so the first thing it does is decide whether this
/// invocation has anything to do with Nikaia at all. Everything else - every
/// crate from crates.io - is passed to the real `rustc` untouched, which is
/// exactly what `RUSTC_WORKSPACE_WRAPPER` already guarantees and what this
/// keeps true for a workspace member that happens to be Rust.
pub fn wrapper_main() -> Result<i32> {
    let argv: Vec<OsString> = std::env::args_os().skip(1).collect();
    let mut invocation = Invocation::parse(&argv, EXTENSION)
        .ok_or_else(|| anyhow!("{WRAPPER_MARKER} is set but no `rustc` was named"))?;

    let Some(source) = invocation.source_path() else {
        trace(&invocation, "passed through");
        return invocation.run();
    };
    trace(&invocation, "lowered");

    let settings = Settings::from_env()?;
    let lowered = lower(&source, &settings, std::env::var_os(NO_CACHE_VAR).is_some())?;

    let name = invocation.crate_name.clone().unwrap_or_else(|| {
        source
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "main".to_string())
    });
    let gen_dir = match std::env::var_os(GEN_DIR_VAR) {
        Some(dir) => PathBuf::from(dir),
        None => std::env::temp_dir().join("nikaia-gen"),
    };
    let generated = gen_dir.join(format!("{name}.rs"));
    write_if_changed(&generated, &lowered.rust)?;

    invocation.replace_source(&generated);
    let code = invocation.run()?;

    // Cargo believes the dependency file `rustc` wrote, and `rustc` only saw
    // the generated Rust. Without this an edit to a `.nika` source leaves the
    // package fresh and the binary stale.
    if code == 0 {
        if let Some(dep_info) = invocation.dep_info() {
            record_extra_dependencies(&dep_info, &lowered.sources)?;
        }
    }
    Ok(code)
}

/// Record one invocation, if anybody asked. A trace that cannot be written is
/// silent: this is a debugging facility, and a build must not fail because the
/// place it was told to write a note to is unwritable.
fn trace(invocation: &Invocation, verdict: &str) {
    use std::io::Write;

    let Some(path) = std::env::var_os(TRACE_VAR) else {
        return;
    };
    let crate_name = invocation.crate_name.as_deref().unwrap_or("-");
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        writeln!(file, "{crate_name} {verdict}").ok();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `--locked` names the entry a changed line belongs to.
    ///
    /// A contract line is short and repeats; since ADR-027 `sync` is the
    /// commonest line in the file. Reporting three bare `sync = true` lines
    /// tells a reader that something moved and leaves them to find out what,
    /// which is not the narrated diff 13.5 asks for.
    #[test]
    fn a_locked_diff_says_whose_contract_moved() {
        let committed = "version = 1\n\n[fn.\"a\"]\nsync = true\n\n[fn.\"b\"]\n";
        let built = "version = 1\n\n[fn.\"a\"]\nsync = true\n\n[fn.\"b\"]\nsync = \"inferred\"\n";

        assert_eq!(
            changed_lines(built, committed),
            ["  [fn.\"b\"]", "      sync = \"inferred\""]
        );
    }

    /// An entry is named once, however many of its lines changed.
    #[test]
    fn an_entry_is_named_once_for_all_of_its_changes() {
        let committed = "version = 1\n\n[fn.\"a\"]\n";
        let built = "version = 1\n\n[fn.\"a\"]\nsync = \"inferred\"\nthrows = true\n";

        assert_eq!(
            changed_lines(built, committed),
            [
                "  [fn.\"a\"]",
                "      sync = \"inferred\"",
                "      throws = true"
            ]
        );
    }

    /// A wholly new entry names itself, and is not named twice.
    #[test]
    fn a_new_entry_names_itself_once() {
        let committed = "version = 1\n";
        let built = "version = 1\n\n[fn.\"z\"]\nsync = \"inferred\"\nthrows = true\n";

        assert_eq!(
            changed_lines(built, committed),
            [
                "  [fn.\"z\"]",
                "      sync = \"inferred\"",
                "      throws = true"
            ]
        );
    }

    /// A header line belongs to no entry and is reported on its own - which is
    /// what a toolchain upgrade changing the inference looks like.
    #[test]
    fn a_changed_header_is_reported_without_an_entry() {
        let committed = "version = 1\ninference = \"stage0-signatures\"\n";
        let built = "version = 1\ninference = \"stage0-signatures+sync-bodies\"\n";

        assert_eq!(
            changed_lines(built, committed),
            ["  inference = \"stage0-signatures+sync-bodies\""]
        );
    }

    /// The runtime the emitted Rust names, resolved out of the compiler's own
    /// workspace. Written down a second time, the two copies would drift and a
    /// program would link two `winnow`s.
    #[test]
    fn the_runtime_is_read_from_the_compilers_own_workspace() {
        let runtime = runtime_dependencies("use winnow_grammar::grammar; use nikaia_std::prelude;")
            .expect("the compiler's workspace is readable");
        assert!(runtime["nikaia-std"].get("path").is_some());
        assert!(runtime.contains_key("winnow-grammar"));
        assert!(
            runtime.contains_key("winnow"),
            "a grammar expansion writes `winnow::Parser` too"
        );
    }

    /// A program that names none of them declares none of them, and fetches
    /// nothing at all.
    #[test]
    fn a_program_that_names_no_runtime_depends_on_nothing() {
        let runtime = runtime_dependencies("fn main() { println!(\"hi\"); }").expect("resolves");
        assert!(runtime.is_empty(), "{runtime:?}");
    }

    /// ADR-037 D1: the machine decides what a panic does, and that reaches the
    /// Cargo profile (ADR-002 D1).
    #[test]
    fn the_machine_decides_the_panic_strategy() {
        let native = Settings::resolve(&Manifest::default(), None, None, None).expect("resolves");
        assert_eq!(native.panic_strategy(), "unwind");

        let wasm = Settings::resolve(&Manifest::default(), Some("wasm32-unknown"), None, None)
            .expect("resolves");
        assert_eq!(
            wasm.panic_strategy(),
            "abort",
            "WebAssembly traps and has no stack to unwind"
        );
    }
}
