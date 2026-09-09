// crates/nikaia/src/main.rs
#![feature(rustc_private)]

// `rustc-executor` links against the compiler's own dylibs, which carry their
// own copies of `std`/`core`. Declaring `rustc_driver` here too makes this
// binary link the same dylib set instead of the regular rlib `std`; without it
// every shared crate "shows up twice" and linking fails.
extern crate rustc_driver;

use anyhow::{Context, Result};
use bridge_ir::BridgeModule;
use bridge_orchestrator::LanguageFrontend;
use clap::Parser;
use std::path::PathBuf;

use nikaia::contracts::Ledger;
use nikaia::emit::{self, Profile};
use nikaia::{diagnostics, interpreter, parser};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    #[arg(short, long)]
    pub input: PathBuf,

    #[arg(long, default_value = "bridge")]
    pub backend: String, // "interpreter", "rust", "bridge", "cranelift", "llvm"

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

pub fn main() -> Result<()> {
    let args = Cli::parse();

    let source = std::fs::read_to_string(&args.input)?;

    if args.explain {
        return explain(&args, &source);
    }

    if args.backend == "interpreter" {
        // For the interpreter, we need to parse to AST, not BridgeIR.
        let parsed = parser::parse_to_ast(&source)?;
        let interpreter = interpreter::Interpreter::new(parsed.interner.clone());
        interpreter.run(&parsed);
        Ok(())
    } else if args.backend == "rust" {
        // Stage 0: the transpiler. A `grammar` item reaches the parser backend
        // only through here - the Bridge IR has no macro to carry it.
        let profile = Profile::parse(&args.profile)?;
        let parsed = parser::parse_to_ast(&source)?;
        let lowered = emit::emit_program(&parsed, profile)?;

        let output_path = args
            .output
            .clone()
            .unwrap_or_else(|| args.input.with_extension("rs"));
        std::fs::write(&output_path, &lowered.rust)?;

        // The ledger goes beside the output, because that is where a build
        // puts what it produced. Part III 13.5 says the project root, which is
        // what this is once the orchestrator compiles a project rather than a
        // file.
        let ledger_path = output_path.with_file_name("nikaia.contracts");
        let ledger = Ledger::infer(&parsed).render();
        contracts(&ledger_path, &ledger, args.locked)?;

        println!(
            "Lowered {} to {} (profile: {})",
            args.input.display(),
            output_path.display(),
            args.profile
        );

        Ok(())
    } else {
        // For compilation backends (bridge, llvm, etc.), we use the orchestrator flow (or similar)
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
}
