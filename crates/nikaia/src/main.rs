// crates/nikaia/src/main.rs
#![feature(rustc_private)]

// `rustc-executor` links against the compiler's own dylibs, which carry their
// own copies of `std`/`core`. Declaring `rustc_driver` here too makes this
// binary link the same dylib set instead of the regular rlib `std`; without it
// every shared crate "shows up twice" and linking fails.
extern crate rustc_driver;

use anyhow::Result;
use bridge_ir::BridgeModule;
use bridge_orchestrator::LanguageFrontend;
use clap::Parser;
use std::path::PathBuf;

use nikaia::emit::{self, Profile};
use nikaia::{interpreter, parser};

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
}

struct NikaiaFrontend;

impl LanguageFrontend for NikaiaFrontend {
    fn parse(&self, source: &str) -> Result<BridgeModule> {
        parser::parse_to_bridge(source)
    }
}

pub fn main() -> Result<()> {
    let args = Cli::parse();

    let source = std::fs::read_to_string(&args.input)?;

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
        let rust = emit::emit_program(&parsed, profile)?;

        let output_path = args
            .output
            .unwrap_or_else(|| args.input.with_extension("rs"));
        std::fs::write(&output_path, rust)?;

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
