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
//! [`orchestrator::project`] ([ADR-003](../../../docs/specification/adr/adr-003.md)
//! D2): rendering a manifest, running `cargo`, splitting a `rustc` command
//! line. What is here is Nikaia's half - which file is the entry, which
//! switches were chosen, and what the lowering does.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use orchestrator::cache::{Artifacts, Cache, Choices, Layout, Lockfile};
use orchestrator::project::{
    record_extra_dependencies, resolved_versions, write_if_changed, Cargo, CargoProject, CrateKind,
    Invocation, Package, Profile, Workspace,
};

use crate::contracts::{sync, Ledger, STD};
use crate::emit::{Build, Target};
use crate::manifest::{Dependency, Manifest};
use crate::sysroot::{Codegen, Sysroot};
use crate::{check, diagnostics, modules, refuse, refused};

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

/// The build switches, resolved: flag over manifest over built-in default.
///
/// One value threaded through rather than three fields re-read at each use.
/// Every setting is kept both as the word that named it and as the parsed
/// thing, because the cache key and the diagnostics want the word (ADR-021 D5
/// hashes what was asked for) and the emitter wants the value.
#[derive(Debug, Clone)]
pub struct Settings {
    pub build: Build,
    pub target: String,
    pub user_parallelism: String,
}

impl Settings {
    /// Resolve every switch before anything is read, so a mistyped one fails on
    /// its own account rather than after a compile (ADR-037 D5).
    pub fn resolve(
        manifest: &Manifest,
        target: Option<&str>,
        user_parallelism: Option<&str>,
    ) -> Result<Settings> {
        let target = manifest
            .setting("target", target, "x86_64-linux")
            .to_string();
        let user_parallelism = manifest
            .setting("user-parallelism", user_parallelism, "no")
            .to_string();
        Ok(Settings {
            build: Build::parse(&target, &user_parallelism)?,
            target,
            user_parallelism,
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
        ]
    }

    /// What the driver put in the environment. Absent means the wrapper was run
    /// by hand, and the built-in defaults are the honest answer.
    fn from_env() -> Result<Settings> {
        let word =
            |name: &str, default: &str| std::env::var(name).unwrap_or_else(|_| default.to_string());
        let target = word(TARGET_VAR, "x86_64-linux");
        let user_parallelism = word(PARALLELISM_VAR, "no");
        Ok(Settings {
            build: Build::parse(&target, &user_parallelism)?,
            target,
            user_parallelism,
        })
    }

    /// The cache's view of this build (ADR-021 D5).
    ///
    /// The backend is `"rust"` because it names **the lowering that produced the
    /// artifact**, not the flag the user typed, and a bare invocation *is*
    /// `--backend rust` ([ADR-004](../../docs/specification/adr/adr-004.md) D1),
    /// so the two spellings share one entry - which is correct, because it is
    /// one lowering. The dimension itself stays in
    /// [`orchestrator::cache::Key::build`] (D5, D7) so that the day a second
    /// backend caches, the literal here becomes that backend's name and the two
    /// cannot collide.
    pub fn choices(&self) -> Choices {
        Choices::new(format!("{}/{}", self.target, self.user_parallelism), "rust")
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
/// **The packages a project depends on by path**
/// ([ADR-047](../../docs/specification/adr/adr-047.md) D2), resolved against
/// the manifest's own directory.
///
/// A free function because two callers need the same answer: the project build,
/// and the `rustc` wrapper, which is handed a `.nika` path by Cargo and has to
/// find the manifest above it (`wrapper_main`). Two copies of a resolution rule
/// is two chances to disagree about what a program is.
///
/// Three of D2's five rules are kept here, because they are about the set of
/// dependencies rather than about any one of them:
///
/// * **rule 1**, the manifest key is the name: the map's key is what `use`
///   writes and the path is only where it comes from, so nothing inside a
///   package gets to name it;
/// * **rule 3**, the same path is the same package: two keys resolving to one
///   directory would be two `mod`s of one package and two types of one name,
///   so it is refused rather than unified;
/// * **rule 4**, a dependency's `[build]` is ignored, and the compiler says
///   so — a package is built with the settings of the program that uses it,
///   and anything else puts two answers to the parallelism question in one
///   build.
///
/// **rule 2** — transitive dependencies are not visible — is no longer checked
/// anywhere, because it is structural
/// ([ADR-053](../../docs/specification/adr/adr-053.md) D3): a package is
/// generated as its own crate naming only its own dependencies, so a name from
/// two levels down does not resolve and no rule of ours has to say so.
/// **rule 5** is the generated workspace's profile (D4): every Nikaia crate is
/// named on the program's side of the overflow checks, beside the exception
/// that turns them off for every foreign package.
pub fn packages_of(
    manifest: &Manifest,
    root: &Path,
    announce: bool,
) -> Result<Vec<modules::Dependency>> {
    let mut out: Vec<modules::Dependency> = Vec::new();
    let mut seen: BTreeMap<PathBuf, String> = BTreeMap::new();

    // **What this build calls each package**, before any of them is read, so
    // that a package reached twice is renamed to one name rather than to
    // whichever of the two was resolved first
    // ([ADR-053](../../docs/specification/adr/adr-053.md) D2).
    let mut here: BTreeMap<PathBuf, String> = BTreeMap::new();
    for (name, value) in manifest.dependencies() {
        if let Dependency::Path(path) = value {
            let at = root.join(path);
            here.insert(at.canonicalize().unwrap_or(at), name.clone());
        }
    }

    for (name, value) in manifest.dependencies() {
        let Dependency::Path(path) = value else {
            continue;
        };
        let root = root.join(path);
        let root = root.canonicalize().unwrap_or(root);

        if let Some(first) = seen.get(&root) {
            refuse!(
                "`{name}` and `{first}` are the same package: both are {}.\n\
                 The same path is the same package (ADR-047 D2), so two names for it \
                 would be two copies of every type it declares - name it once.",
                root.display()
            );
        }
        seen.insert(root.clone(), name.clone());

        // **A package's own packages are resolved, not refused**
        // (ADR-053 D1): it is generated as its own crate, so they are its
        // dependencies and not ours. Read here only to fail early where one is
        // not there.
        let dependency = Manifest::read(&root.join("nikaia.toml"))?;
        // `announce`, because this resolution runs more than once per build - the
        // driver, and the `rustc` wrapper Cargo calls for the build and again for
        // a run - and a note a reader meets three times reads as three problems.
        // The driver is the one that reports (ADR-005 D7's shape: the work happens
        // where it happens, the saying happens once).
        if announce && dependency.has_build_section() {
            eprintln!(
                "note: `{}`'s own `[build]` is ignored: a package is built with the \
                 settings of the program that uses it (ADR-047 D2).",
                name
            );
        }

        out.push(modules::Dependency {
            name: name.clone(),
            renames: renames_in(&dependency, &root, &here)?,
            root,
            // **Its own keys**, so its `use` lines are read the way its own
            // build would read them (ADR-053 D3).
            reachable: dependency.dependencies().keys().cloned().collect(),
        });
    }
    Ok(out)
}

/// How a dependency's **own** package names are read in this build
/// ([ADR-053](../../docs/specification/adr/adr-053.md) D2: a package reached
/// through two parents is one package).
///
/// A library writes its own manifest keys in its own signatures, and those keys
/// are its author's choice rather than this program's. So a program that depends
/// on `deep` directly, and on a library that depends on the same directory as
/// `c`, was told its `deep::Id` was not the `c::Id` that library takes - one
/// type, refused for having two spellings, which is the one thing D2 says it is
/// not. The types are identical in the generated Rust; only the name this
/// compiler gave them differed.
///
/// So the identity is the **package**, and this is the translation into the word
/// this build uses for it: the program's own key where the program names it too,
/// and otherwise the package's `[package] name`, which `members_of` has already
/// refused to let two packages share. Either way it is a function of the
/// directory and not of who asked.
///
/// One level deep, because one level is what is read: a dependency's own
/// dependencies are its crate's business (D3), and nothing below it is in this
/// program's ledger to be renamed.
fn renames_in(
    dependency: &Manifest,
    dependency_root: &Path,
    here: &BTreeMap<PathBuf, String>,
) -> Result<BTreeMap<String, String>> {
    let mut out = BTreeMap::new();
    for (key, value) in dependency.dependencies() {
        let Dependency::Path(path) = value else {
            continue;
        };
        let at = dependency_root.join(path);
        let at = at.canonicalize().unwrap_or(at);
        let canonical = match here.get(&at) {
            Some(ours) => ours.clone(),
            None => Manifest::read(&at.join("nikaia.toml"))?
                .package_name()
                .map(str::to_string)
                .unwrap_or_else(|| key.clone()),
        };
        if &canonical != key {
            out.insert(key.clone(), canonical);
        }
    }
    Ok(out)
}

/// One package of a build, and everything needed to generate a crate for it
/// ([ADR-053](../../docs/specification/adr/adr-053.md) D1).
#[derive(Debug, Clone)]
pub struct Member {
    /// The Cargo package name - the `[package] name` of its own `nikaia.toml`.
    /// Its **identity** is `root`; this is what the profile rows and the
    /// renaming form below name it by.
    pub name: String,
    /// The canonical package directory. Two manifest keys resolving here are
    /// one crate, which is [ADR-047](../../docs/specification/adr/adr-047.md)
    /// D2 rule 3 holding because Cargo already works that way (D2).
    pub root: PathBuf,
    pub manifest: Manifest,
    /// The `.nika` file Cargo is told is the crate root. `src/main.nika` where
    /// there is one; a library needs none, so the first source beside it stands
    /// in - the wrapper walks up from whatever it is handed, so any file of the
    /// package finds the same manifest.
    pub entry: PathBuf,
    /// Its **own** packages, under its **own** manifest keys.
    pub dependencies: Vec<modules::Dependency>,
}

/// Every package of this build, the entry first
/// ([ADR-053](../../docs/specification/adr/adr-053.md) D1).
///
/// Breadth-first from the entry, so a diamond is visited once and the order is
/// the same on every machine. A package already seen is the same crate: the key
/// under which it was reached is the *depending* crate's business (D2), and
/// nothing here is named by it.
pub fn members_of(entry_manifest: &Manifest, entry_root: &Path) -> Result<Vec<Member>> {
    let mut out: Vec<Member> = Vec::new();
    let mut seen: BTreeMap<PathBuf, usize> = BTreeMap::new();
    let mut queue: Vec<(Manifest, PathBuf)> = vec![(entry_manifest.clone(), canonical(entry_root))];

    while let Some((manifest, root)) = queue.first().cloned() {
        queue.remove(0);
        if seen.contains_key(&root) {
            continue;
        }

        let name = manifest
            .package_name()
            .ok_or_else(|| {
                refused!(
                    "{}/nikaia.toml has no `[package] name`, and Cargo needs one to \
                     name the crate (Part III 13.3)",
                    root.display()
                )
            })?
            .to_string();

        // A crate name is a package's identity in the generated workspace, and
        // two of them would be two answers to which crate the profile rows and
        // the renaming form below mean. The canonical path is the identity, so
        // this is only ever two *different* packages wanting one name.
        if let Some(first) = out.iter().find(|m| m.name == name) {
            refuse!(
                "two packages are both named `{name}`: {} and {}.\n\
                 Each package is generated as its own crate (ADR-053 D1) and a crate is \
                 named by its `[package] name`, so give one of them a name of its own - \
                 the key a `use` line writes is set by whoever depends on it and does not \
                 have to change.",
                first.root.display(),
                root.display()
            );
        }

        // Announced only for the entry: this walk reads every manifest of the
        // build, and a note about an ignored `[build]` is about the package
        // that carries it, said once.
        let dependencies = packages_of(&manifest, &root, out.is_empty())?;
        for dependency in &dependencies {
            let manifest = Manifest::read(&dependency.root.join("nikaia.toml"))?;
            queue.push((manifest, canonical(&dependency.root)));
        }

        seen.insert(root.clone(), out.len());
        out.push(Member {
            name,
            entry: crate_root_of(&root)?,
            root,
            manifest,
            dependencies,
        });
    }
    Ok(out)
}

/// `path`, resolved where the filesystem lets us. The same fallback
/// `packages_of` uses, so the two agree about when two keys are one package.
fn canonical(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

/// The `.nika` file handed to Cargo as a package's crate root.
///
/// `src/main.nika` where there is one. A library has none - what a package
/// offers is its public surface (`modules::package_at`) - and Cargo still needs
/// a path that exists, so the first source beside it stands in. Which file it
/// is changes nothing: the wrapper walks up from it to the same `nikaia.toml`,
/// and `package_at` reads the whole directory either way.
fn crate_root_of(root: &Path) -> Result<PathBuf> {
    let entry = root.join(ENTRY);
    if entry.is_file() {
        return Ok(entry);
    }

    let src = root.join("src");
    let mut beside: Vec<PathBuf> = std::fs::read_dir(&src)
        .with_context(|| format!("cannot read {}", src.display()))?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|e| e == EXTENSION))
        .collect();
    beside.sort();

    beside.into_iter().next().ok_or_else(|| {
        refused!(
            "{} has no `.nika` sources.\n\
             A package is a directory with a `src/` in it (Part III 13.1), and a package \
             with nothing in it is nothing to build.",
            src.display()
        )
    })
}

pub fn lower(
    input: &Path,
    settings: &Settings,
    no_cache: bool,
    packages: &[modules::Dependency],
) -> Result<Lowered> {
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

    // Part I 9.1: a package is a directory, so a build is every `.nika` beside
    // the entry - and the cache cannot be asked about a program until the
    // program is known.
    //
    // **Outside a project it is the one file.** A package is a directory *of a
    // project*, which is what a `nikaia.toml` declares; a directory of loose
    // examples is a directory of programs, and compiling one of them must not
    // pull in the other ten (ADR-047 D1, `modules::collect_one`).
    let program = match layout.in_project {
        true => modules::Program::read_with(input, packages)?,
        false => modules::Program::read_one(input)?,
    };
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
            let modules = program.package_names();
            for unit in &program.units {
                check(
                    &unit.parsed,
                    &program.contracts,
                    &modules,
                    &unit.path,
                    &unit.source,
                    &settings.user_parallelism,
                )?;
            }

            let lowered = program.emit(settings.build)?;
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
///
/// `user_parallelism` reaches this and reaches **nothing inside the analyses**.
/// The verdict of every rule is a property of the program, so no switch may
/// change it (ADR-005 §1 Group B); what the switch does change is whether a
/// refusal is about *this* build, and that is a question about severity. See
/// [`lint_where_nothing_crosses`].
pub fn check(
    parsed: &crate::parser::Parsed,
    own: &Ledger,
    modules: &BTreeSet<String>,
    path: &Path,
    source: &str,
    user_parallelism: &str,
) -> Result<()> {
    let library = Ledger::parse(STD).context("std's shipped ledger")?;

    let mut all = check::check_program(parsed, own, &library, modules).findings;
    lint_where_nothing_crosses(&mut all, user_parallelism);
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
    // One line per family, because a family is what a reader can act on in one
    // go. The `NK1xxx` codes are types; `NK23xx` is a view kept past its call;
    // `NK25xx` is a value on the wrong thread; `NK2605` and `NK2701` are one
    // rule counted together, because they *are* one rule (ADR-025 D1) - a call
    // that can fail, written or not, in a function that does not declare
    // `throws`.
    let count = |family: &str| {
        findings
            .iter()
            .filter(|f| f.code.starts_with(family))
            .count()
    };
    let plural = |n: usize| if n == 1 { "" } else { "s" };

    let types = count("NK1");
    if types > 0 {
        refused.push(format!("{types} type error{}", plural(types)));
    }
    let crossings = count("NK25");
    if crossings > 0 {
        refused.push(format!(
            "{crossings} value{} that may not cross a thread",
            plural(crossings)
        ));
    }
    // `NK23xx` is aliasing: who else points at the thing this one is about.
    // Counted on its own because the way out is a different one - a shape to
    // change rather than a declaration to add.
    let aliases = count("NK23");
    if aliases > 0 {
        refused.push(format!(
            "{aliases} view{} kept past the call that was given it",
            plural(aliases)
        ));
    }
    // `NK21xx` is running things at once, and its two rules want two lines: what
    // a `spawn` took with it is a value to clone, and an `overlap` whose
    // branches meet is a block to take apart. Counted apart from the line below
    // for the reason they are counted at all - the tally has to say what it
    // counted, and "a place that can fail without saying so" is neither of them.
    let moved = count("NK2101");
    if moved > 0 {
        refused.push(format!(
            "{moved} value{} a task took with it",
            plural(moved)
        ));
    }
    let together = count("NK2104");
    if together > 0 {
        refused.push(format!(
            "{together} branch{} that cannot run beside the others",
            if together == 1 { "" } else { "es" }
        ));
    }
    let tasks = count("NK21");
    let rules = findings.len() - types - crossings - aliases - tasks;
    if rules > 0 {
        refused.push(format!(
            "{rules} place{} that can fail without saying so",
            plural(rules)
        ));
    }
    if !violations.is_empty() {
        refused.push(format!(
            "{} call{} a `sync` function may not make",
            violations.len(),
            if violations.len() == 1 { "" } else { "s" }
        ));
    }
    // A refusal and not a failure of this compiler, so it leaves without a
    // backtrace: the diagnostics above have already said everything, and this is
    // the tally (`diagnostics::Refused`).
    Err(diagnostics::refuse(refused.join(", ")))
}

/// The `NK25xx` codes whose crossing this build does not perform, reported as a
/// lint rather than as an error.
///
/// Part III C.3 says the `Send` rules are "reported at `user_parallelism = no`
/// as a lint, so a library built there stays usable at `yes`", and the reason is
/// exact: at `no` nothing the program wrote runs concurrently (ADR-037 D2), so
/// the emitter writes no `task::both` and no task - the crossing `NK2501` is
/// about does not happen in *this* build. Refusing it would be refusing a
/// program that compiles, which is the one thing the compiler may never do; and
/// saying nothing would be ADR-005 §1 Group B's failure exactly, a library built
/// at `no` that turns out un-compilable at `yes`. A lint is the third answer, and
/// it is the one that record asked for.
///
/// **`NK2502` is not downgraded, and that is the decision.** A Rust dependency
/// may bring its own runtime (ADR-038 D7), and its threads are not the
/// program's: `user_parallelism` bounds what *you* wrote, which is ADR-037 D2's
/// load-bearing "user". So that crossing is real at both settings and so is the
/// refusal.
///
/// This is the only place a build switch meets a `NK25xx`, and it meets the
/// **severity** rather than the verdict. The verdict is a property of the
/// program and the analyses never see a switch.
fn lint_where_nothing_crosses(findings: &mut [check::Finding], user_parallelism: &str) {
    if user_parallelism != "no" {
        return;
    }
    for finding in findings.iter_mut().filter(|f| f.code == "NK2501") {
        finding.severity = check::Severity::Warning;
        finding.notes.push(
            "`user_parallelism = no` runs nothing you wrote concurrently, so this build does \
             not perform the crossing - it is reported so that the same source still builds at \
             `yes` (Part III, C.3)"
                .to_string(),
        );
    }
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
        refuse!(
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

/// Which of the three explanations a run was asked for
/// ([ADR-033](../../docs/specification/adr/adr-033.md) D9,
/// [ADR-037](../../docs/specification/adr/adr-037.md) D8,
/// [ADR-010](../../docs/specification/adr/adr-010.md) D7).
///
/// They exist because these analyses **cannot be asked for** - there is no way to
/// request the cheaper reference count, the overlap or the faster hash; every
/// fallback is enumerated instead. `--sharing`'s own help says why that is only
/// fair if the fallbacks can be asked about, *"and this is the asking"*.
///
/// So they have to reach a real program, and a person with a real program builds
/// it with `nikaia build`. They were on the single-file path only, which is
/// exactly where the asking is not done.
#[derive(Debug, Clone, Copy, Default)]
pub struct Explain {
    pub overlaps: bool,
    pub sharing: bool,
    pub trust: bool,
}

impl Explain {
    pub fn asked(&self) -> bool {
        self.overlaps || self.sharing || self.trust
    }
}

/// Print the explanations a run asked for, over every file of the package.
///
/// **Against the package's own ledger and not each file's**, which is the
/// difference from the single-file path doing this three times: `sync`, the touch
/// sets and the sharing classes are whole-program facts (ADR-027, ADR-033), and a
/// report built from one file's inferences would answer a different question from
/// the one the build answers.
///
/// The file name is printed where there is more than one, because a slot key
/// (`zaehle::counts`) does not say which file it is in - one namespace or not.
pub fn explain(program: &modules::Program, settings: &Settings, want: Explain) -> Result<()> {
    if !want.asked() {
        return Ok(());
    }
    let library = Ledger::parse(STD).context("std's shipped ledger")?;
    let several = program.units.len() > 1;

    for unit in &program.units {
        if several {
            println!("--- {}", unit.path.display());
        }
        if want.overlaps {
            print!(
                "{}",
                crate::contracts::order::overlap_report(
                    &unit.parsed,
                    &program.contracts,
                    &library,
                    &crate::emit::branch_starts_first(
                        &unit.parsed,
                        settings.build,
                        &program.contracts,
                    ),
                )
            );
        }
        if want.sharing {
            print!(
                "{}",
                crate::contracts::sharing::report(
                    &unit.parsed,
                    &program.contracts,
                    &library,
                    settings.build.user_parallelism == crate::emit::UserParallelism::Yes,
                )
            );
        }
        if want.trust {
            print!(
                "{}",
                crate::contracts::trust::render(&crate::contracts::trust::analyse(
                    &unit.parsed,
                    &library
                ))
            );
        }
    }
    Ok(())
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
    ) -> Result<Project> {
        // `Layout::resolve` searches from an input file's *parent*, so it is
        // handed the manifest's own path: the search then starts at `start`
        // itself, which is what "build the project I am standing in" means.
        let layout = Layout::resolve(&start.join("nikaia.toml"));
        if !layout.in_project {
            refuse!(
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
        let settings = Settings::resolve(&manifest, target, user_parallelism)?;
        if let Some(missing) = settings.build.target.unbuildable() {
            refuse!(
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

    /// **The switches, as a file Cargo can see** (ADR-021 D5).
    ///
    /// A build switch is not a source, so nothing Cargo watches changes when
    /// one does — and the lowering happens inside the `rustc` wrapper, which
    /// Cargo runs only for a package it thinks is stale. So flipping
    /// `user-parallelism` in `nikaia.toml` left the package fresh and the
    /// binary at the **old** setting, silently: the cache had the switch as a
    /// dimension (D5) and nothing ever got as far as asking it.
    ///
    /// This is the third member of the same family as the two freshness traps
    /// the project build already carries, and it takes the same answer: give
    /// Cargo a file. The driver writes the resolved switches here before
    /// `cargo` runs, and the wrapper puts it into the dependency file beside
    /// the `.nika` sources — so a switch that changed is an input that changed.
    ///
    /// Under `target/` and outside [`Self::build_dir`], for [`Self::gen_dir`]'s
    /// reason: a file *inside* the path package would make it dirty on every
    /// build rather than on a changed one.
    pub fn switches_path(&self) -> PathBuf {
        self.root
            .join("target")
            .join("nikaia")
            .join("gen")
            .join("switches")
    }

    /// The ledger, in the project root (Part III 13.5).
    pub fn ledger_path(&self) -> PathBuf {
        self.root.join("nikaia.contracts")
    }

    /// The packages this project depends on by path - see [`packages_of`].
    pub fn packages(&self) -> Result<Vec<modules::Dependency>> {
        packages_of(&self.manifest, &self.root, true)
    }

    /// Every package of this build, the entry first - see [`members_of`].
    pub fn members(&self) -> Result<Vec<Member>> {
        members_of(&self.manifest, &self.root)
    }

    /// One member's `nikaia.toml` translated (ADR-002 D1).
    ///
    /// `rust` is that package as this build lowered it, and it decides which of
    /// the compiler's own runtime crates the generated crate declares - a
    /// package that names none of them depends on nothing and builds in a
    /// second.
    ///
    /// `kind` is what puts the entry package's `fn main` in a binary and every
    /// package under it in a library beside it
    /// ([ADR-053](../../docs/specification/adr/adr-053.md) D1).
    fn cargo_member(&self, member: &Member, rust: &str, kind: CrateKind) -> Result<CargoProject> {
        let mut dependencies = runtime_dependencies(rust)?;
        for (dependency, value) in member.manifest.dependencies() {
            match value {
                // The whole point of D1: whatever the author wrote reaches
                // Cargo, and Cargo resolves and links it as it would for any
                // Rust project.
                Dependency::Rust(value) => {
                    dependencies.insert(dependency.clone(), value.clone());
                }
                // **Cargo's renaming form** (ADR-053 D2): the package is named
                // by its identity, the key is the name *this* crate uses for
                // it. So two packages that both want the key `c` are in two
                // different crates and never meet, and one package reached
                // through two parents is one crate - which is ADR-047 D2 rule 3
                // holding because Cargo already works that way.
                Dependency::Path(path) => {
                    let root = canonical(&member.root.join(path));
                    let named = self.crate_named(&root)?;
                    let mut table = toml::value::Table::new();
                    table.insert("package".to_string(), toml::Value::String(named.clone()));
                    // Beside this member, under the generated build directory:
                    // every crate of the build is written there, one directory
                    // each, named by the same name.
                    table.insert(
                        "path".to_string(),
                        toml::Value::String(format!("../{named}")),
                    );
                    dependencies.insert(dependency.clone(), toml::Value::Table(table));
                }
                // A registry, a name space and a distribution format are all
                // undecided, and guessing at one here would be inventing the
                // answer in the place it is hardest to review
                // ([ADR-002](../../docs/specification/adr/adr-002.md) D1 §5).
                Dependency::Nikaia(_) => refuse!(
                    "`{dependency}` is a Nikaia package named by a version, and a version \
                     is not how one is found: no ADR names a registry, a version grammar \
                     or a distribution format.\n\
                     A Nikaia package is depended on **by path** today - write it as \
                     `{dependency} = {{ path = \"../{dependency}\" }}` (ADR-047 D2) - and a \
                     crate from crates.io as \
                     `{dependency} = {{ type = \"rust\", version = \"…\" }}` \
                     (Part III 13.3, ADR-002 D1)."
                ),
            }
        }

        Ok(CargoProject {
            package: Package {
                name: member.name.clone(),
                version: member.manifest.package_version().to_string(),
                // What the emitter writes, and what the tests compile it as.
                edition: "2021".to_string(),
            },
            kind,
            // A `[lib] name` has to be an identifier; a package name does not.
            // Cargo does this same replacement for a lib it names itself.
            bin_name: match kind {
                CrateKind::Bin => member.name.clone(),
                CrateKind::Lib => member.name.replace('-', "_"),
            },
            bin_path: member.entry.clone(),
            dependencies,
        })
    }

    /// The generated workspace: one crate per Nikaia package
    /// ([ADR-053](../../docs/specification/adr/adr-053.md) D1).
    ///
    /// `rust` is each member as this build lowered it, in `members`' order.
    pub fn cargo_workspace(&self, members: &[Member], rust: &[String]) -> Result<Workspace> {
        let mut out = Vec::new();
        for (at, member) in members.iter().enumerate() {
            // The entry package is the program; everything it reaches is a
            // library beside it.
            let kind = match at {
                0 => CrateKind::Bin,
                _ => CrateKind::Lib,
            };
            let rust = rust.get(at).map(String::as_str).unwrap_or("");
            out.push((member.name.clone(), self.cargo_member(member, rust, kind)?));
        }

        let codegen = self.manifest.codegen_for(&self.settings.target);
        Ok(Workspace {
            members: out,
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

    /// What the package at `root` is called as a crate - its own `[package]
    /// name`, which is what every other crate of the build names it by (D2).
    fn crate_named(&self, root: &Path) -> Result<String> {
        let manifest = Manifest::read(&root.join("nikaia.toml"))?;
        Ok(manifest
            .package_name()
            .ok_or_else(|| {
                refused!(
                    "{}/nikaia.toml has no `[package] name`, and Cargo needs one to \
                     name the crate (Part III 13.3)",
                    root.display()
                )
            })?
            .to_string())
    }

    /// What this build asked the code generator for, as the compiled-`std` cache
    /// keys it (ADR-002 D4).
    ///
    /// The same two sources the Cargo profile above is written from - the
    /// machine's `[build.<target>]` table and the panic strategy that follows
    /// from the machine - because it is exactly those that decide what `std`'s
    /// machine code looks like.
    fn codegen(&self) -> Codegen {
        Codegen::new(
            &self.manifest.codegen_for(&self.settings.target),
            self.settings.panic_strategy(),
        )
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
        want: Explain,
    ) -> Result<i32> {
        let entry = self.entry();
        if !entry.is_file() {
            refuse!(
                "{} is not there. A project's entry point is `{ENTRY}` (Part III 13.1).",
                entry.display()
            );
        }

        // Before the build, because an explanation is about the program and a
        // build that reuses a cached lowering still answers the question. The
        // package is read a second time here rather than threaded out of
        // `lower`: reading it is parsing, and paying for it only when somebody
        // asked is cheaper than reshaping the build for a flag nobody usually
        // passes.
        if want.asked() {
            let program = modules::Program::read_with(&entry, &self.packages()?)?;
            explain(&program, &self.settings, want)?;
        }

        // Every package of this build, resolved before anything is read: a
        // dependency that is not there, or two names for one path, is a
        // statement about the project and should not wait for a lowering.
        let members = self.members()?;

        // **Every member is lowered here**, for the reason the doc comment
        // above gives for the entry: the checks and the diagnostics are the
        // compiler's own output rather than something buried in a `cargo`
        // subprocess. Each is lowered again inside the wrapper and each of
        // those is a cache hit, so a package is compiled once however many
        // crates now ask about it.
        let mut rust: Vec<String> = Vec::with_capacity(members.len());
        let mut entry_ledger = None;
        for member in &members {
            let lowered = lower(
                &member.entry,
                &self.settings,
                no_cache,
                &member.dependencies,
            )?;
            if entry_ledger.is_none() {
                entry_ledger = Some(lowered.ledger);
            }
            rust.push(lowered.rust);
        }
        // One ledger, in the project root (Part III 13.5): it is the program's
        // record, and a library's own is written when that library is built.
        if let Some(ledger) = entry_ledger {
            write_ledger(&self.ledger_path(), &ledger, locked)?;
        }

        // **Before `cargo`**, and written only when it differs, so an
        // unchanged switch does not dirty the package every build. What is in
        // it is what the cache keys on, so the two cannot disagree about which
        // switches decide an artifact.
        let switches = self.switches_path();
        if let Some(dir) = switches.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let choices = self.settings.choices();
        write_if_changed(
            &switches,
            &format!("{}\n{}\n", choices.build, choices.backend),
        )?;

        let manifest = self
            .cargo_workspace(&members, &rust)?
            .write_to(&self.build_dir())?;

        let mut env = self.settings.as_env();
        env.push((WRAPPER_MARKER.to_string(), OsString::from("1")));
        env.push((GEN_DIR_VAR.to_string(), self.gen_dir().into_os_string()));
        if no_cache {
            env.push((NO_CACHE_VAR.to_string(), OsString::from("1")));
        }

        let cargo = Cargo {
            manifest,
            // **The compiled-`std` cache** (ADR-002 D4). Cargo builds into a
            // directory in the user's cache named by `Key::sysroot`, so the
            // second project on this machine links the `std` the first one built
            // instead of compiling it and everything under it again. It cannot
            // live under this project's `target/`, because "shared between
            // projects" is the whole of what it is for.
            //
            // `CARGO_TARGET_DIR` still wins where it is set. It is Cargo's own
            // documented override and a build that asked for a directory has to
            // get it.
            target_dir: match std::env::var_os("CARGO_TARGET_DIR") {
                Some(_) => None,
                None => Some(Sysroot::resolve().rlib_cache(&self.settings.target, &self.codegen())),
            },
            wrapper: std::env::current_exe().context("finding this compiler's own path")?,
            env,
        };

        // **`build` first, always, and its diagnostics read rather than
        // relayed** (ADR-005 D7, Part III C.1). Cargo's stderr is Rust about a
        // file the author never opened; `--message-format=json` is the channel
        // the translator can read, and it is only usable for `build` - a `run`
        // writes the *program's* output to the same stdout.
        //
        // So a `nikaia run` is a build and then a run, and the run is a no-op
        // rebuild. That is the same shape the lowering already has (above): the
        // work happens once, and the decision is made where it can be reported.
        let (mut code, messages) = cargo.messages("build", &[])?;
        self.report(&messages)?;
        if code == 0 && subcommand == "run" {
            // **The built program is run directly, and that is Part III C.1
            // rather than a shortcut.** `cargo run` is a second invocation that
            // cannot use the JSON channel - the program's own output is on that
            // stdout - so Cargo renders its **cached** diagnostics to stderr
            // while it checks freshness, and a program with a warning saw it
            // twice: once in this language's words from `report` above, and
            // once as `rustc` about `target/nikaia/gen/….rs`. Measured on
            // Part I 2.3's own example.
            //
            // **The path is read and not computed.** Cargo decides where a
            // binary lands from the target directory, the profile and
            // `CARGO_TARGET_DIR`, and replicating those rules here would be a
            // second place that has to agree with Cargo forever. The build
            // above already captured the answer: a `compiler-artifact` line
            // whose `executable` is not null.
            //
            // Where there is no such line the old path is taken, so this can
            // only remove a duplicate and never lose a run.
            code = match executable_in(&messages) {
                Some(binary) => run_directly(&binary, program_args)?,
                None => cargo.run("run", &[], program_args)?,
            };
        }

        // Cargo has resolved by now, and only now: the versions do not exist
        // before it ran. A build that failed resolved nothing worth recording.
        if code == 0 {
            if let Err(error) = self.record_resolved_dependencies(no_cache) {
                // D12: the record costs the *next* reader some information. It
                // never costs this build, which has already succeeded.
                eprintln!(
                    "warning: the resolved dependency versions could not be recorded \
                     in nikaia.lock: {error:#}"
                );
            }
        }
        Ok(code)
    }

    /// Say what the backend said, against the `.nika` line it was about.
    ///
    /// Part III C.1's Iron Rule: an untranslated backend error reaching the user
    /// is a bug in this compiler. Until this existed the project build had no
    /// interception at all - `cargo`'s stderr *was* the user interface - so every
    /// class ADR-005 D7 enumerates, `E0277` among them since the structural
    /// `Send` check landed, arrived as Rust about `target/nikaia/gen/….rs`.
    ///
    /// **The position is translated and the text is not**, which is ADR-012's
    /// own choice and is honest for most of what arrives: the lowering is name
    /// for name (ADR-011 D2), so "`xs` does not live long enough" is already a
    /// sentence about the Nikaia source and only its place was wrong. It is
    /// *not* honest for a trait-bound error, whose every noun is a Rust type the
    /// author did not write - and rewriting one into Nikaia vocabulary is a
    /// second question, recorded in ADR-005 D7 rather than answered here. What
    /// the frontend can decide about a crossing it now refuses itself, with
    /// `NK2501`/`NK2502` and Nikaia words.
    ///
    /// The map is rebuilt rather than carried: the lowering is deterministic, so
    /// a second one gives the same map (ADR-012), and this way the cost is paid
    /// only by a build the backend had something to say about.
    fn report(&self, messages: &str) -> Result<()> {
        if messages
            .lines()
            .all(|line| !line.contains("\"compiler-message\""))
        {
            return Ok(());
        }

        // **With the packages**, or a `use` line would be read against an empty
        // set and the translation of somebody else's error would be a refusal of
        // a program that is fine.
        let program = modules::Program::read_with(&self.entry(), &self.packages()?)?;
        let lowered = program.emit(self.settings.build)?;
        let sources: Vec<&str> = program.sources();
        let paths: Vec<String> = program
            .units
            .iter()
            .map(|unit| unit.path.display().to_string())
            .collect();
        let generated = self.gen_dir().display().to_string();

        let translated: Vec<_> = diagnostics::translate_units(messages, &lowered.map, &sources)
            .into_iter()
            .filter(diagnostics::is_about_the_program)
            .collect();

        for diagnostic in &translated {
            let at = diagnostic.location.as_ref().map_or(0, |l| l.unit);
            eprint!(
                "{}",
                diagnostics::render(diagnostic, &paths[at], sources[at], &generated)
            );
        }

        // **The backend's own words, kept** (ADR-056 D2). Shown above because
        // the reader is being told this is not their mistake, and written here
        // because a bug report is made later and from a file.
        if let Some(log) = diagnostics::log_internal(&translated, &self.gen_dir()) {
            eprintln!("note: the backend's own messages are in {}", log.display());
        }
        Ok(())
    }

    /// Copy the versions Cargo chose into `nikaia.lock` (ADR-021 D2).
    ///
    /// The toolchain does not resolve versions and must not (ADR-002 D1), so
    /// this reads Cargo's `Cargo.lock` for the generated package rather than
    /// computing anything - D4's distinction exactly: `nikaia.toml` declares
    /// `regex = { type = "rust", version = "1.5" }`, and this records the
    /// `1.13.1` that turned into.
    ///
    /// Two conditions, and neither is tidiness. **`--no-cache` writes nothing**,
    /// because `nikaia.lock` is the cache's own record and a run told not to
    /// keep one must not leave a file behind. **A lockfile that is not there is
    /// not created**, because a file holding resolved versions and no unit
    /// hashes would answer D2's question with half an answer while looking like
    /// a whole one.
    ///
    /// Nothing is written when the versions are unchanged, which is the common
    /// case: a committed file must not turn up as modified after a build that
    /// changed nothing about it.
    fn record_resolved_dependencies(&self, no_cache: bool) -> Result<()> {
        if no_cache {
            return Ok(());
        }
        // The same walk the cache and `Project::open` use, so the three can
        // never disagree about where the record lives.
        let lock_path = Layout::resolve(&self.root.join("nikaia.toml")).lock;
        if !lock_path.is_file() {
            return Ok(());
        }

        let name = self
            .manifest
            .package_name()
            .ok_or_else(|| refused!("the project has no `[package] name`"))?;
        let resolved = resolved_versions(&self.build_dir().join("Cargo.lock"), name)?;

        let mut lock = Lockfile::load(
            &lock_path,
            env!("NIKAIA_RUSTC_VERSION"),
            env!("NIKAIA_COMPILER"),
        )?;
        if lock.dependencies == resolved {
            return Ok(());
        }
        lock.dependencies = resolved;
        lock.save(&lock_path)
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
/// `std` is reached **by path into the sysroot** (ADR-002 D4). It is not
/// published, so there is no registry to reach it through; what changed is that
/// the path is a sysroot's rather than a checkout's, and that the sources there
/// need nothing but `rustc` to build.
///
/// The other two versions are baked into this binary by `build.rs` rather than
/// read from a workspace manifest at run time. A sysroot is not a Cargo
/// workspace and has no `[workspace.dependencies]` above it - and these are the
/// *compiler's* versions anyway, since it is the emitter that writes
/// `winnow::Parser` into a program. Read at the compiler's build time rather
/// than written down twice: a program that linked a different `winnow` from the
/// one `winnow-grammar` was built against is a type error at every `Stream`
/// bound, so the two must not be able to drift apart.
/// The program Cargo built, as its own build said where it was.
///
/// **Read rather than computed.** Cargo decides where a binary lands from the
/// target directory, the profile and `CARGO_TARGET_DIR`; a second place that
/// worked those rules out would have to keep agreeing with Cargo forever. A
/// `--message-format=json` build says it outright, on the `compiler-artifact`
/// line whose `executable` is not null - and this compiler already captures that
/// output to read the diagnostics.
///
/// `None` where nothing says, which is not an error: the caller falls back to
/// `cargo run` and the build behaves as it did before.
fn executable_in(messages: &str) -> Option<std::path::PathBuf> {
    messages
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter(|value| value["reason"] == "compiler-artifact")
        .filter_map(|value| value["executable"].as_str().map(std::path::PathBuf::from))
        .next_back()
}

/// Run it, with the program's own arguments and every stream its own.
///
/// Nothing is captured and nothing is translated: what a running program writes
/// is the program's, and the last thing a compiler should do is stand between
/// the two.
fn run_directly(binary: &std::path::Path, args: &[String]) -> Result<i32> {
    let status = std::process::Command::new(binary)
        .args(args)
        .status()
        .with_context(|| format!("running {}", binary.display()))?;
    // A program killed by a signal has no exit code. `1` is what a shell would
    // report for one, and the alternative - claiming success - is the one answer
    // that must not be given.
    Ok(status.code().unwrap_or(1))
}

fn runtime_dependencies(rust: &str) -> Result<BTreeMap<String, toml::Value>> {
    let mut out = BTreeMap::new();

    if rust.contains("nikaia_std") {
        let mut table = toml::Table::new();
        table.insert(
            "path".to_string(),
            toml::Value::String(Sysroot::resolve().std_dir().to_string_lossy().into_owned()),
        );
        out.insert("nikaia-std".to_string(), toml::Value::Table(table));
    }

    // `winnow_grammar` contains `winnow`, so a grammar pulls in both - which is
    // right: the expansion writes `winnow::Parser` as well.
    for (crate_name, declaration) in [
        ("winnow-grammar", env!("NIKAIA_RUNTIME_WINNOW_GRAMMAR")),
        ("winnow", env!("NIKAIA_RUNTIME_WINNOW")),
    ] {
        if !rust.contains(&crate_name.replace('-', "_")) {
            continue;
        }
        let value: toml::Value = toml::from_str(&format!("value = {declaration}"))
            .map(|table: toml::Table| table["value"].clone())
            .with_context(|| {
                format!("the `{crate_name}` declaration this compiler was built with")
            })?;
        out.insert(crate_name.to_string(), value);
    }

    Ok(out)
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
    // **The packages, found the same way the project build finds them.** Cargo
    // hands this a `.nika` path, so the manifest is the one above it
    // (`Manifest::find` walks to the same root the cache does) - and it has to be
    // read here rather than passed down, because the only channel from the build
    // into the wrapper is the environment and a dependency list is not a switch.
    // `packages_of` is the one resolution rule, so the two cannot disagree about
    // what the program is.
    let manifest = Manifest::find(&source)?;
    let packages = match manifest.root() {
        Some(root) => packages_of(&manifest, root, false)?,
        None => Vec::new(),
    };
    let lowered = lower(
        &source,
        &settings,
        std::env::var_os(NO_CACHE_VAR).is_some(),
        &packages,
    )?;

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
    // **One directory per crate**, and that is not tidiness. Every generated
    // crate root used to sit beside every other, so `rustc` refusing a name from
    // a package this crate does not depend on offered to fix it with
    // `mod deeper` - a file it had found next door, and advice that means
    // nothing here. A crate root with no siblings cannot be offered any.
    let generated = gen_dir.join(&name).join(format!("{name}.rs"));
    write_if_changed(&generated, &lowered.rust)?;

    invocation.replace_source(&generated);
    let code = invocation.run()?;

    // Cargo believes the dependency file `rustc` wrote, and `rustc` only saw
    // the generated Rust. Without this an edit to a `.nika` source leaves the
    // package fresh and the binary stale.
    //
    // **And the switches, for the same reason one step further out**
    // (`Project::switches_path`): a build switch is not a source, so nothing
    // Cargo watches changes when one does — and this wrapper is only run for a
    // package Cargo already thinks is stale. Without the file in here, flipping
    // `user-parallelism` left the program running at the setting it was built
    // at, and said nothing.
    if code == 0 {
        if let Some(dep_info) = invocation.dep_info() {
            let mut inputs = lowered.sources.clone();
            if let Some(switches) = switches_beside(&gen_dir) {
                inputs.push(switches);
            }
            record_extra_dependencies(&dep_info, &inputs)?;
        }
    }
    Ok(code)
}

/// The switches file the driver wrote, where there is one.
///
/// The wrapper is told the generated-source directory and nothing else
/// (`GEN_DIR_VAR` is the one channel), and the file sits in it — so this is a
/// lookup rather than a second answer to where it lives. Absent means the
/// wrapper was run by hand rather than by a project build, and then there is no
/// switch anybody could flip between two runs.
fn switches_beside(gen_dir: &Path) -> Option<PathBuf> {
    let path = gen_dir.join("switches");
    path.is_file().then_some(path)
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

    /// The runtime the emitted Rust names: `std` by path into the sysroot, the
    /// other two from the declarations `build.rs` baked in. Written down a second
    /// time, the copies would drift and a program would link two `winnow`s.
    #[test]
    fn the_runtime_travels_with_the_compiler() {
        let runtime = runtime_dependencies("use winnow_grammar::grammar; use nikaia_std::prelude;")
            .expect("the declarations this compiler was built with parse");
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
        let native = Settings::resolve(&Manifest::default(), None, None).expect("resolves");
        assert_eq!(native.panic_strategy(), "unwind");

        let wasm = Settings::resolve(&Manifest::default(), Some("wasm32-unknown"), None)
            .expect("resolves");
        assert_eq!(
            wasm.panic_strategy(),
            "abort",
            "WebAssembly traps and has no stack to unwind"
        );
    }
}
