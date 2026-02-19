// crates/nikaia/src/main.rs
use anyhow::Result;
use bridge_ir::BridgeModule;
use bridge_orchestrator::{LanguageFrontend, Orchestrator};
use clap::Parser;
use std::path::PathBuf;

// Modules
mod ast;
mod interpreter;
mod parser;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    #[arg(short, long)]
    pub input: PathBuf,

    #[arg(long, default_value = "bridge")]
    pub backend: String, // "interpreter", "bridge", "cranelift", "llvm"
}

struct NikaiaFrontend {
    backend: String,
}

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
        let frontend = NikaiaFrontend {
            backend: args.backend.clone(),
        };
        let bridge_module = frontend.parse(&source)?;

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
