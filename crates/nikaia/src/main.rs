// crates/nikaia/src/main.rs
#![feature(rustc_private)]

// `rustc-executor` links against the compiler's own dylibs, which carry their
// own copies of `std`/`core`. Declaring `rustc_driver` here too makes this
// binary link the same dylib set instead of the regular rlib `std`; without it
// every shared crate "shows up twice" and linking fails.
extern crate rustc_driver;

use anyhow::{bail, Context, Result};
use bridge_ir::BridgeModule;
use bridge_orchestrator::LanguageFrontend;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

use nikaia::contracts::{self, Ledger, STD};
use nikaia::emit;
use nikaia::manifest::Manifest;
use nikaia::project::{self, Project, Settings};
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

    /// `interpreter`, `rust` or `bridge`.
    ///
    /// `cranelift` and `llvm` are named by ADR-002 but not implemented; they
    /// are rejected rather than silently treated as `bridge` (ADR-021 D9).
    #[arg(long, default_value = "bridge")]
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
}

struct NikaiaFrontend;

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
        // question about the build, and two settings answer it no on their
        // own - so say which, rather than letting the report read as a
        // promise the emitter is not keeping.
        if !settings.build.overlaps_user_code() {
            println!(
                "note: `--user-parallelism {}` on `--target {}` runs user code on one thread, \
                 so nothing below overlaps in this build.",
                settings.user_parallelism,
                settings.build.target.triple()
            );
        } else if settings.ordering != emit::Ordering::Effects {
            println!(
                "note: `ordering = {}` keeps the written order, so nothing below overlaps in this build.",
                settings.ordering_word
            );
        }
        let parsed = parser::parse_to_ast(source)?;
        let library = Ledger::parse(STD).context("std's shipped ledger")?;
        let own = Ledger::infer(&parsed);
        print!("{}", contracts::order::report(&parsed, &own, &library));
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

/// `nikaia build` and `nikaia run` (Part III 13.2).
fn project_command(args: &Cli, command: &Command) -> Result<i32> {
    let (subcommand, directory, program_args) = match command {
        Command::Build { project } => ("build", project.clone(), Vec::new()),
        Command::Run { project, args } => ("run", project.clone(), args.clone()),
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
        "bridge" => {
            // For compilation backends we use the orchestrator flow (or similar)
            let bridge_module = NikaiaFrontend.parse(&source)?;

            // Output name based on input
            let file_stem = input.file_stem().unwrap().to_str().unwrap();
            let output_path = format!("./{}", file_stem);

            println!("Compiling {} to {}...", input.display(), output_path);

            // Manually call the backend executor
            rustc_executor::execute(&bridge_module, &output_path)?;

            println!("Compilation successful: {}", output_path);

            Ok(())
        }
        // ADR-002 named these and nothing ever matched them, so they ran as
        // `bridge` without saying so. Accepting a flag and quietly doing
        // something else is worse than either implementing or refusing it.
        backend @ ("cranelift" | "llvm") => bail!(
            "backend `{backend}` is not implemented (ADR-021 D9 records it as an \
             open item); available backends are interpreter, rust and bridge"
        ),
        other => bail!("unknown backend `{other}` (expected interpreter, rust or bridge)"),
    }
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
