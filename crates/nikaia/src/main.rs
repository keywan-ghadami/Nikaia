// crates/nikaia/src/main.rs
#![feature(rustc_private)]

// `rustc-executor` links against the compiler's own dylibs, which carry their
// own copies of `std`/`core`. Declaring `rustc_driver` here too makes this
// binary link the same dylib set instead of the regular rlib `std`; without it
// every shared crate "shows up twice" and linking fails.
extern crate rustc_driver;

use anyhow::{bail, Context, Result};
use bridge_ir::BridgeModule;
use bridge_orchestrator::cache::{Artifacts, Cache, Choices, Layout};
use bridge_orchestrator::LanguageFrontend;
use clap::Parser;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use nikaia::contracts::{self, sync, Ledger, STD};
use nikaia::emit::{self, Profile};
use nikaia::{diagnostics, interpreter, parser};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    #[arg(short, long)]
    pub input: PathBuf,

    /// `interpreter`, `rust` or `bridge`.
    ///
    /// `cranelift` and `llvm` are named by ADR-002 but not implemented; they
    /// are rejected rather than silently treated as `bridge` (ADR-021 D9).
    #[arg(long, default_value = "bridge")]
    pub backend: String,

    /// Which runtime the program is compiled for (Part I/II).
    ///
    /// Not a dialect: the same source compiles under both. It decides how the
    /// generated parser is driven - under Lite a `par_fold` runs as a
    /// sequential fold, which is ADR-009's degradation and nothing more.
    #[arg(long, default_value = "advanced")]
    pub profile: String,

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
    #[arg(long)]
    pub locked: bool,

    /// Lower from scratch, ignoring the build cache (ADR-021).
    ///
    /// The cache is **on**: reusing an unchanged lowering is the difference in
    /// feel between a Nikaia build and a Rust one, and a default nobody types
    /// is not that. This flag exists for the times that matters less than
    /// seeing the emitter run - debugging it, or comparing its output against
    /// what the cache holds.
    #[arg(long)]
    pub no_cache: bool,

    /// Print where this program's bytes came from and which hash its maps got
    /// (ADR-010 D7).
    ///
    /// The choice is visible, never a mystery: this names every source the
    /// program reads and what each one contributed.
    #[arg(long)]
    pub trust: bool,
}

/// Part II 12.1, checked: a `sync` function may only call `sync` functions.
///
/// The ledger is what makes this possible across the `std` boundary - a call to
/// `io::read_to_string` is only a violation if something says that function can
/// pause, and `std.contracts` is where it says so (ADR-020).
fn check_sync(parsed: &parser::Parsed, path: &Path, source: &str) -> Result<()> {
    let own = Ledger::infer(parsed);
    let library = Ledger::parse(STD).context("std's shipped ledger")?;
    let violations = sync::check(parsed, &own, &library);

    if violations.is_empty() {
        return Ok(());
    }

    let path = path.display().to_string();
    for violation in &violations {
        eprint!(
            "{}",
            diagnostics::render_sync_violation(violation, &path, source)
        );
    }
    anyhow::bail!(
        "{} call{} a `sync` function may not make",
        violations.len(),
        if violations.len() == 1 { "" } else { "s" }
    )
}

/// Write the ledger, or - under `--locked` - check that it did not need
/// writing.
///
/// The determinism guarantee (13.5) is what lets this compare bytes rather than
/// meanings: the same sources and the same compiler produce the same file, so a
/// difference is a change in a contract and never in the formatting.
fn contracts(path: &std::path::Path, ledger: &str, locked: bool) -> Result<()> {
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
        let changed: Vec<&str> = ledger
            .lines()
            .filter(|line| !committed.lines().any(|c| c == *line))
            .collect();
        anyhow::bail!(
            "--locked: the contracts changed and {} does not say so.\n\
             What the build inferred and the ledger does not have:\n{}\n\
             Run without --locked to record it, and read the diff.",
            path.display(),
            changed
                .iter()
                .map(|l| format!("    {l}"))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }

    Ok(())
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
fn explain(args: &Cli, source: &str) -> Result<()> {
    use std::io::Read;

    let profile = Profile::parse(&args.profile)?;
    let parsed = parser::parse_to_ast(source)?;
    let lowered = emit::emit_program(&parsed, profile)?;

    let mut rustc_json = String::new();
    std::io::stdin().read_to_string(&mut rustc_json)?;

    let generated = args
        .output
        .clone()
        .unwrap_or_else(|| args.input.with_extension("rs"));
    let path = args.input.display().to_string();
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

/// Stage 0: the transpiler. A `grammar` item reaches the parser backend only
/// through here - the Bridge IR has no macro to carry it.
///
/// The build cache (ADR-021) covers the whole of it: on a hit nothing is
/// parsed, checked, lowered or inferred, because only a build that passed every
/// check was recorded and the key holds everything those checks depend on
/// (D13). What a hit does *not* skip is `--locked`: comparing the committed
/// ledger against what this build determined is the user's check, not the
/// compiler's, and skipping it would change what the flag means.
/// The names `lower_to_rust` stores its outputs under.
const RUST: &str = "rust";
const CONTRACTS: &str = "contracts";

fn lower_to_rust(args: &Cli, source: &str) -> Result<()> {
    let profile = Profile::parse(&args.profile)?;
    let output_path = args
        .output
        .clone()
        .unwrap_or_else(|| args.input.with_extension("rs"));

    // `Layout` decides where the lock and the store go, and guarantees that
    // outside a `nikaia.toml` project nothing is written into the source tree
    // - which is what makes caching-by-default something other than littering.
    // It also names the unit relative to its root: an absolute path is D7's
    // first failure direction, a key that moves with the checkout.
    let layout = Layout::resolve(&args.input);
    let unit = layout.unit_name(&args.input);
    let choices = Choices::new(&args.profile, "rust");

    // `--trust` is an explanation, so it is answered here rather than in the
    // miss branch below: a build that reuses a cached lowering still answers
    // the question, and the answer cannot differ from the one that lowering was
    // built with. Provenance is a function of the source and of what `std`'s
    // ledger says about the sources it calls, and both are already in the key -
    // the compiler's fingerprint covers `std.contracts` by name (`build.rs`).
    if args.trust {
        let parsed = parser::parse_to_ast(source)?;
        let library = Ledger::parse(STD).context("std's shipped ledger")?;
        print!(
            "{}",
            contracts::trust::render(&contracts::trust::analyse(&parsed, &library))
        );
    }

    // A cache that cannot be opened is a slower build, never a failed one
    // (D12). Once the cache is the default, a read-only checkout or a full
    // disk must not turn a build that would have succeeded into one that does
    // not.
    let mut cache = if args.no_cache {
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

    // An entry that predates an artifact this build needs is a miss, not a
    // gap: adding an output stays a safe change.
    let cached = cache
        .as_ref()
        .and_then(|cache| cache.lookup(&unit, source, &choices, &layout.root))
        .filter(|artifacts| artifacts.has_all(&[RUST, CONTRACTS]));
    let reused = cached.is_some();

    let (rust, ledger) = match &cached {
        Some(artifacts) => (
            artifacts.get(RUST).expect("checked above").to_string(),
            artifacts.get(CONTRACTS).expect("checked above").to_string(),
        ),
        None => {
            let parsed = parser::parse_to_ast(source)?;
            check_sync(&parsed, &args.input, source)?;
            let lowered = emit::emit_program(&parsed, profile)?;
            let ledger = Ledger::infer(&parsed).render();

            if let Some(cache) = &mut cache {
                // Nothing reports assets yet: compile-time I/O
                // (`from "schema.sql"`) is specified and not implemented. The
                // dimension travels through the key regardless, so switching it
                // on later does not reshape the key.
                let artifacts = Artifacts::new()
                    .with(RUST, &lowered.rust)
                    .with(CONTRACTS, &ledger);
                let stored = cache
                    .record(&unit, source, BTreeMap::new(), &choices, &artifacts)
                    .and_then(|()| cache.save());
                if let Err(error) = stored {
                    // The outputs are in hand; only the next build is slower.
                    eprintln!("warning: the build cache could not be updated: {error:#}");
                }
            }
            (lowered.rust, ledger)
        }
    };

    std::fs::write(&output_path, &rust)?;

    // The ledger goes beside the output, because that is where a build puts
    // what it produced. Part III 13.5 says the project root, which is what this
    // is once the orchestrator compiles a project rather than a file.
    //
    // This runs on a hit too: `--locked` asks whether the *committed* file
    // still matches, and a cached build has as much to answer for there as a
    // fresh one.
    let ledger_path = output_path.with_file_name("nikaia.contracts");
    contracts(&ledger_path, &ledger, args.locked)?;

    println!(
        "Lowered {} to {} (profile: {}{})",
        args.input.display(),
        output_path.display(),
        args.profile,
        if reused { ", lowering from cache" } else { "" }
    );

    Ok(())
}

pub fn main() -> Result<()> {
    let args = Cli::parse();

    let source = std::fs::read_to_string(&args.input)?;

    if args.explain {
        return explain(&args, &source);
    }

    match args.backend.as_str() {
        "interpreter" => {
            // For the interpreter, we need to parse to AST, not BridgeIR.
            let parsed = parser::parse_to_ast(&source)?;
            let interpreter = interpreter::Interpreter::new(parsed.interner.clone());
            interpreter.run(&parsed);
            Ok(())
        }
        "rust" => lower_to_rust(&args, &source),
        "bridge" => {
            // For compilation backends we use the orchestrator flow (or similar)
            let bridge_module = NikaiaFrontend.parse(&source)?;

            // Output name based on input
            let file_stem = args.input.file_stem().unwrap().to_str().unwrap();
            let output_path = format!("./{}", file_stem);

            println!("Compiling {} to {}...", args.input.display(), output_path);

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
