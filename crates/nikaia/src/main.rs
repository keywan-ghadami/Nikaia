// crates/nikaia/src/main.rs
//
// `rustc_private` is the bridge backend's, not this binary's: it is declared
// only when `rustc-backend` is on, so that `cargo build -p nikaia
// --no-default-features` produces a compiler on a stable toolchain
// (ADR-001 D1, and `docs/nightly-cost.md` for what that weighs).
#![cfg_attr(feature = "rustc-backend", feature(rustc_private))]

// `rustc-executor` links against the compiler's own dylibs, which carry their
// own copies of `std`/`core`. Declaring `rustc_driver` here too makes this
// binary link the same dylib set instead of the regular rlib `std`; without it
// every shared crate "shows up twice" and linking fails.
#[cfg(feature = "rustc-backend")]
extern crate rustc_driver;

use anyhow::{bail, Context, Result};
#[cfg(feature = "rustc-backend")]
use bridge_ir::BridgeModule;
#[cfg(feature = "rustc-backend")]
use bridge_orchestrator::LanguageFrontend;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

use nikaia::contracts::{self, Ledger, STD};
use nikaia::emit;
use nikaia::manifest::Manifest;
use nikaia::project::{self, Project, Settings};
use nikaia::sysroot::{self, Sysroot};
use nikaia::{diagnostics, interpreter, parser};

/// The Nikaia compiler: `nikaia build` for a project, `--input` for one file.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// A project command (ADR-002 D1). Without one, `--input` compiles a single
    /// file - the shape the compiler had before there were projects, and the
    /// one every `.nika` file outside a project still uses.
    #[command(subcommand)]
    pub command: Option<Command>,

    #[arg(short, long)]
    pub input: Option<PathBuf>,

    /// `interpreter`, `rust` (the default) or `bridge`.
    ///
    /// `rust` is what a bare invocation uses (ADR-004 D4): it is the backend
    /// that carries the whole corpus, and the only one that is in every build,
    /// because `bridge` is what links `rustc_private` and so what needs the
    /// pinned nightly.
    ///
    /// `bridge` is present when the `rustc-backend` feature is; a build without
    /// it refuses the flag by name rather than serving another backend
    /// (ADR-021 D14).
    ///
    /// `cranelift` and `llvm` are named by ADR-002 but not implemented; they
    /// are rejected rather than silently treated as something else
    /// (ADR-021 D9).
    #[arg(long, default_value = "rust")]
    pub backend: String,

    /// The machine to build for (ADR-037 D1): `x86_64-linux` or
    /// `wasm32-unknown`. It decides what `std` can offer and what a panic
    /// does, and nothing about what a program means.
    ///
    /// Overrides `nikaia.toml`'s `[build] target` for this one build; the
    /// default is `x86_64-linux` where neither says (D5).
    #[arg(long, global = true)]
    pub target: Option<String>,

    /// Whether *your* code may run concurrently at all (ADR-037 D2): `yes`
    /// or `no`.
    ///
    /// Not a count - how many threads serve a `yes` is the runtime's to
    /// decide. And it bounds the program, not the compiler: `fs::map` may
    /// still validate its text on several cores at `no`, because that is not
    /// code you wrote and it changes nothing the program prints.
    ///
    /// Overrides `nikaia.toml`'s `[build] user-parallelism`; the default is
    /// `no` where neither says (D5).
    #[arg(long, global = true)]
    pub user_parallelism: Option<String>,

    /// Where the `rust` backend writes. Defaults to `<input>.rs`.
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Read `rustc --error-format=json` on stdin and report it against the
    /// `.nika` source instead of the emitted Rust (ADR-012).
    ///
    /// The map is not an artifact: the lowering is deterministic, so it is
    /// rebuilt from `--input` here rather than written out and kept in step.
    #[arg(long)]
    pub explain: bool,

    /// Verify the Borrow Contract Ledger instead of updating it (Part III,
    /// 13.5).
    ///
    /// The ledger is a pure function of source and toolchain, so this compares
    /// bytes: any difference fails the build and prints what changed. The
    /// recommended CI line, and the reason the file is committed.
    #[arg(long, global = true)]
    pub locked: bool,

    /// How strictly the written order of two statements is taken (ADR-033).
    ///
    /// `effects` (the default) lets two operations that touch disjoint
    /// resources overlap; `strict` keeps the written order everywhere and does
    /// not apply the analysis.
    ///
    /// Overrides `nikaia.toml`'s `[build] ordering` (D8); the default is
    /// `effects` where neither says.
    #[arg(long, global = true)]
    pub ordering: Option<String>,

    /// Lower from scratch, ignoring the build cache (ADR-021).
    ///
    /// The cache is **on**: reusing an unchanged lowering is the difference in
    /// feel between a Nikaia build and a Rust one, and a default nobody types
    /// is not that. This flag exists for the times that matters less than
    /// seeing the emitter run - debugging it, or comparing its output against
    /// what the cache holds.
    #[arg(long, global = true)]
    pub no_cache: bool,

    /// Print which adjacent statements run together and why the rest do not
    /// (ADR-033 D9).
    ///
    /// Nikaia has no `allow_parallel`: the overlap is the default and the
    /// refusals are the compiler's own. That is only fair if the refusals can
    /// be asked about, and this is the asking. Like `--trust`, it explains a
    /// decision rather than changing one.
    #[arg(long)]
    pub overlaps: bool,

    /// Print where this program's bytes came from and which hash its maps got
    /// (ADR-010 D7).
    ///
    /// The choice is visible, never a mystery: this names every source the
    /// program reads and what each one contributed.
    #[arg(long)]
    pub trust: bool,
}

/// The project commands of Part III 13.2.
#[derive(Subcommand, Debug)]
pub enum Command {
    /// Compile the project in this directory (Part III 13.2).
    ///
    /// `nikaia.toml` is translated to a `Cargo.toml` and `cargo` builds it,
    /// which is how a Nikaia project reaches crates.io without this toolchain
    /// resolving a single version itself (ADR-002 D1).
    Build {
        /// The project directory. Defaults to the working directory, and the
        /// search walks up from there to the nearest `nikaia.toml`.
        #[arg(long)]
        project: Option<PathBuf>,
    },
    /// Compile the project and run it.
    ///
    /// Everything after `--` is the program's own arguments.
    Run {
        #[arg(long)]
        project: Option<PathBuf>,
        #[arg(last = true)]
        args: Vec<String>,
    },
    /// Re-lower the sysroot's `std` from its `.nika` sources (ADR-002 D4).
    ///
    /// The release step, and the *only* way `std`'s Nikaia half is lowered. It
    /// is a command on this **binary** on purpose: the alternative was
    /// `nikaia-std` linking the compiler as a build dependency, which made Cargo
    /// build the compiler a second time inside every project's `target/`.
    ///
    /// A binary install never needs to run it - the `.rs` ships beside the
    /// `.nika`. A from-source install may.
    LowerStd {
        /// The sysroot. Defaults to `NIKAIA_SYSROOT`, or the checkout this
        /// compiler was built from.
        #[arg(long)]
        sysroot: Option<PathBuf>,
    },
}

/// What *this* build can be asked for, for a refusal to name (ADR-021 D14).
///
/// The list is the build's and not the language's: a refusal that offered a
/// backend this binary does not contain would send the reader straight into a
/// second refusal, which is the same fault as answering with the wrong backend.
#[cfg(feature = "rustc-backend")]
const AVAILABLE: &str = "interpreter, rust, bridge";
/// The same list on a build made without `rustc-backend` (ADR-021 D14).
#[cfg(not(feature = "rustc-backend"))]
const AVAILABLE: &str = "interpreter, rust";

/// Bridge-IR is what the bridge backend consumes, and nothing else in the
/// binary asks for it, so a build without that backend has no frontend to
/// register.
#[cfg(feature = "rustc-backend")]
struct NikaiaFrontend;

#[cfg(feature = "rustc-backend")]
impl LanguageFrontend for NikaiaFrontend {
    fn parse(&self, source: &str) -> Result<BridgeModule> {
        parser::parse_to_bridge(source)
    }
}

/// `rustc --error-format=json … | nikaia --input x.nika --explain`
///
/// Every message rustc reports about the emitted file is placed back in the
/// `.nika` it came from. The text is left alone: because the lowering is name
/// for name (ADR-011 D2), only the position was ever wrong.
fn explain(input: &std::path::Path, args: &Cli, settings: &Settings, source: &str) -> Result<()> {
    use std::io::Read;

    // The same switches the build used, or the map would point into a file this
    // run did not emit - which is why they are resolved once and passed in.
    let parsed = parser::parse_to_ast(source)?;
    let lowered = emit::emit_program_ordered(&parsed, settings.build, settings.ordering)?;

    let mut rustc_json = String::new();
    std::io::stdin().read_to_string(&mut rustc_json)?;

    let generated = args
        .output
        .clone()
        .unwrap_or_else(|| input.with_extension("rs"));
    let path = input.display().to_string();
    let generated = generated.display().to_string();

    let diagnostics = diagnostics::translate(&rustc_json, &lowered.map, source);
    let errors = diagnostics.iter().filter(|d| d.level == "error").count();

    for diagnostic in &diagnostics {
        print!(
            "{}",
            diagnostics::render(diagnostic, &path, source, &generated)
        );
    }

    if errors > 0 {
        std::process::exit(1);
    }
    Ok(())
}

/// The single-file `rust` backend: one `.nika` entry to one `.rs` file.
///
/// Unchanged by the project build, deliberately. A one-off transformation of a
/// file that belongs to no project is a real thing to want (ADR-021 D11), and
/// it is what every test that drives the emitter uses.
fn lower_to_rust(
    input: &std::path::Path,
    args: &Cli,
    settings: &Settings,
    source: &str,
) -> Result<()> {
    let output_path = args
        .output
        .clone()
        .unwrap_or_else(|| input.with_extension("rs"));

    // `--trust` and `--overlaps` are explanations, so they are answered before
    // the cache is consulted: a build that reuses a cached lowering still
    // answers the question, and the answer cannot differ from the one that
    // lowering was built with.
    if args.overlaps {
        // The report answers "may these two overlap", which is a question
        // about the program. Whether anything then *does* overlap is a
        // question about the build - and since ADR-033 D10 the answer differs
        // from pair to pair, because the two vehicles are gated by different
        // switches. So the build's half travels *into* the report, one answer
        // per pair, rather than standing at the top as a sentence that is true
        // of some lines and false of others.
        if settings.ordering != emit::Ordering::Effects {
            println!(
                "note: `ordering = {}` keeps the written order, so nothing below overlaps in this build.",
                settings.ordering_word
            );
        } else if !settings.build.overlaps_user_code() {
            println!(
                "note: `--user-parallelism {}` keeps every piece of your own code on one thread. \
                 A pair `std` can put in flight itself still overlaps (ADR-033 D10); a pair that \
                 would need two threads of your own is marked `would` below.",
                settings.user_parallelism
            );
        }
        let parsed = parser::parse_to_ast(source)?;
        let library = Ledger::parse(STD).context("std's shipped ledger")?;
        let own = Ledger::infer(&parsed);
        print!(
            "{}",
            contracts::order::report(&parsed, &own, &library, &overlaps_here(settings))
        );
    }

    if args.trust {
        let parsed = parser::parse_to_ast(source)?;
        let library = Ledger::parse(STD).context("std's shipped ledger")?;
        print!(
            "{}",
            contracts::trust::render(&contracts::trust::analyse(&parsed, &library))
        );
    }

    let lowered = project::lower(input, settings, args.no_cache)?;
    std::fs::write(&output_path, &lowered.rust)?;

    // The ledger goes beside the output, because that is where a build puts
    // what it produced. Part III 13.5 says the project root, which is where
    // `nikaia build` puts it.
    //
    // This runs on a hit too: `--locked` asks whether the *committed* file
    // still matches, and a cached build has as much to answer for there as a
    // fresh one.
    let ledger_path = output_path.with_file_name("nikaia.contracts");
    project::write_ledger(&ledger_path, &lowered.ledger, args.locked)?;

    println!(
        "Lowered {} to {} (target: {}{})",
        input.display(),
        output_path.display(),
        settings.target,
        if lowered.reused {
            ", lowering from cache"
        } else {
            ""
        }
    );

    Ok(())
}

/// What this build answers about each vehicle an overlap could need
/// (ADR-033 D10), for `--overlaps` to print beside the pair that needs it.
///
/// `None` is "this build has that vehicle". A reason names the switch that
/// decided it, because a report that said only "these two did not run together"
/// is the trap D9's refusals exist to keep open to a question.
///
/// The two vehicles answer to **different switches**, which is the whole of what
/// D10 changed here: a completion pair needs only `std`'s runtime, and
/// `task::both` needs permission to run two pieces of the program's own code at
/// once.
fn overlaps_here(settings: &Settings) -> impl Fn(contracts::order::Vehicle) -> Option<String> + '_ {
    use contracts::order::Vehicle;

    move |vehicle| {
        if settings.ordering != emit::Ordering::Effects {
            return Some(format!(
                "`ordering = {}` keeps the written order",
                settings.ordering_word
            ));
        }
        match vehicle {
            Vehicle::Completion if settings.build.overlaps_operations() => None,
            Vehicle::Completion => Some(format!(
                "`--target {}` has no runtime to put two operations in flight",
                settings.target
            )),
            Vehicle::UserClosures if settings.build.overlaps_user_code() => None,
            Vehicle::UserClosures if !settings.build.target.has_threads() => Some(format!(
                "running them together puts two pieces of your own code on two threads, and \
                 `--target {}` has none",
                settings.target
            )),
            Vehicle::UserClosures => Some(format!(
                "running them together puts two pieces of your own code on two threads, which \
                 `--user-parallelism {}` forbids",
                settings.user_parallelism
            )),
        }
    }
}

/// `nikaia lower-std` (ADR-002 D4): `std`'s `.nika` half to the `.rs` beside it.
fn lower_std(sysroot: Option<PathBuf>) -> Result<i32> {
    let sysroot = match sysroot {
        Some(root) => Sysroot::new(root),
        None => Sysroot::resolve(),
    };
    let changed = sysroot::lower_std(&sysroot)?;
    if changed.is_empty() {
        println!(
            "{} is already what this compiler lowers its `.nika` sources to.",
            sysroot.std_dir().display()
        );
    }
    for path in &changed {
        println!("Lowered to {}", path.display());
    }
    Ok(0)
}

/// `nikaia build` and `nikaia run` (Part III 13.2).
fn project_command(args: &Cli, command: &Command) -> Result<i32> {
    let (subcommand, directory, program_args) = match command {
        Command::Build { project } => ("build", project.clone(), Vec::new()),
        Command::Run { project, args } => ("run", project.clone(), args.clone()),
        Command::LowerStd { sysroot } => return lower_std(sysroot.clone()),
    };

    let start = match directory {
        Some(directory) => directory,
        None => std::env::current_dir().context("finding the working directory")?,
    };

    let project = Project::open(
        &start,
        args.target.as_deref(),
        args.user_parallelism.as_deref(),
        args.ordering.as_deref(),
    )?;
    project.drive(subcommand, &program_args, args.no_cache, args.locked)
}

/// What the manifest still accepts and the compiler no longer reads.
///
/// A note rather than a failure: [ADR-038](../../docs/specification/adr/adr-038.md)
/// D5 moved `cleanup-deadline` to the runtime configuration file, and a
/// manifest written to the specification that documented it must not stop
/// compiling because of the move. The note is what makes the move discoverable
/// instead of silent.
fn report_moved_keys(manifest: &Manifest) {
    for note in manifest.notes() {
        eprintln!("note: {note}");
    }
}

/// The single-file path, unchanged: `nikaia --input x.nika --backend …`.
fn single_file(args: &Cli, input: &std::path::Path) -> Result<()> {
    // Resolved before anything is read, so a mistyped switch - in the manifest
    // or on the command line - fails on its own account rather than after a
    // compile. A target the toolchain cannot build for is refused here too:
    // emitting code for a different machine than the one named would be worse
    // than any name this switch replaced (ADR-037 D1).
    let manifest = Manifest::find(input)?;
    report_moved_keys(&manifest);
    let settings = Settings::resolve(
        &manifest,
        args.target.as_deref(),
        args.user_parallelism.as_deref(),
        args.ordering.as_deref(),
    )?;
    if let Some(missing) = settings.build.target.unbuildable() {
        bail!(
            "cannot build for `{}` yet: {missing}",
            settings.build.target.triple()
        );
    }

    let source = std::fs::read_to_string(input)?;

    if args.explain {
        return explain(input, args, &settings, &source);
    }

    match args.backend.as_str() {
        "interpreter" => {
            // For the interpreter, we need to parse to AST, not BridgeIR.
            let parsed = parser::parse_to_ast(&source)?;
            let interpreter = interpreter::Interpreter::new(parsed.interner.clone());
            interpreter.run(&parsed);
            Ok(())
        }
        "rust" => lower_to_rust(input, args, &settings, &source),
        "bridge" => run_bridge_backend(input, &source),
        // ADR-002 named these and nothing ever matched them, so they ran as
        // whatever the default was without saying so. Accepting a flag and
        // quietly doing something else is worse than either implementing or
        // refusing it.
        //
        // This is the *never implemented* refusal of ADR-021 D14, and it says
        // so in those words: there is nothing to install and no build of this
        // compiler has it, which is exactly what distinguishes it from the
        // `bridge` refusal below.
        backend @ ("cranelift" | "llvm") => bail!(
            "backend `{backend}` is not implemented: no build of this compiler has it, \
             and there is nothing to install that would add it (ADR-002 names it; \
             ADR-021 D9 records it as an open item).\n\
             Available here: {AVAILABLE}."
        ),
        other => bail!(
            "unknown backend `{other}` (expected interpreter, rust or bridge; \
             available here: {AVAILABLE})"
        ),
    }
}

/// The Bridge-IR backend (ADR-004 D2): lower, print the crate, hand it to
/// `rustc`. It is the only part of the compiler that links `rustc_private`, so
/// it is the only part that needs the pinned nightly and its `rustc-dev`
/// component (ADR-001 D1).
#[cfg(feature = "rustc-backend")]
fn run_bridge_backend(input: &std::path::Path, source: &str) -> Result<()> {
    // For compilation backends we use the orchestrator flow (or similar)
    let bridge_module = NikaiaFrontend.parse(source)?;

    // Output name based on input
    let file_stem = input.file_stem().unwrap().to_str().unwrap();
    let output_path = format!("./{}", file_stem);

    println!("Compiling {} to {}...", input.display(), output_path);

    // Manually call the backend executor
    rustc_executor::execute(&bridge_module, &output_path)?;

    println!("Compilation successful: {}", output_path);

    Ok(())
}

/// The same backend on a build that does not contain it.
///
/// A compiler built `--no-default-features` has no `rustc_private` linkage and
/// so needs no nightly, which since ADR-004 D4 is what an installation is
/// allowed to be rather than a reduced one. What it must not do is quietly
/// compile through some other backend: that is ADR-021 D9's rule, and D14
/// extends it to a backend that was **compiled out** rather than never written.
///
/// The two refusals have to be told apart from the message alone, because the
/// reader's next move differs. This one is *it exists and this build does not
/// contain it*, and it names the feature, the toolchain component and roughly
/// what that weighs, because there is something to do about it. `cranelift`'s
/// is *nothing implements it*, and offers no remedy because none exists.
#[cfg(not(feature = "rustc-backend"))]
fn run_bridge_backend(_input: &std::path::Path, _source: &str) -> Result<()> {
    bail!(
        "backend `bridge` is not compiled into this nikaia: it exists, and this \
         build does not contain it (ADR-004 D2 is the backend and D4 is why it is \
         optional; ADR-001 D1 is the toolchain it needs).\n\
         This build was made without the `rustc-backend` feature, so it links \
         no `rustc_private` and needs no nightly toolchain.\n\
         To get it: install the nightly named in \
         `bridge-toolchain/rust-toolchain.toml` with its `rustc-dev` component \
         (`scripts/bridge-toolchain.sh` does that, about 0.8 GB more on disk) \
         and rebuild with default features on it.\n\
         Nothing else is missing: `rust` is the default backend here and in a \
         build that has the bridge (ADR-004 D4).\n\
         Available here: {AVAILABLE}."
    )
}

pub fn main() -> Result<()> {
    // Cargo owns the wrapper's argument list, so this cannot be a flag or a
    // subcommand: the marker in the environment is the only channel available,
    // and it is read before `clap` sees an argument list it would not
    // recognise (ADR-002 D1).
    if std::env::var_os(project::WRAPPER_MARKER).is_some() {
        let code = project::wrapper_main()?;
        std::process::exit(code);
    }

    let args = Cli::parse();

    if let Some(command) = &args.command {
        let code = project_command(&args, command)?;
        std::process::exit(code);
    }

    let Some(input) = args.input.clone() else {
        bail!(
            "nothing to build: give `--input <file.nika>` for a single file, or \
             `nikaia build` inside a project (Part III 13.2)"
        );
    };
    single_file(&args, &input)
}
