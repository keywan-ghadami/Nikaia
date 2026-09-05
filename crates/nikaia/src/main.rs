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

use nikaia::{interpreter, parser};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    #[arg(short, long)]
    pub input: PathBuf,

    #[arg(long, default_value = "bridge")]
    pub backend: String, // "interpreter", "bridge", "cranelift", "llvm"
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
        let program = parser::parse_to_ast(&source)?;
        let interpreter = interpreter::Interpreter::new();
        interpreter.run(&program);
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
