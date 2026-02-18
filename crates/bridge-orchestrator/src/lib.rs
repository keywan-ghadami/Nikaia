use anyhow::{Context, Result};
use bridge_ir::BridgeModule;
use clap::Parser;
use std::path::PathBuf;
use std::process::Command;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    #[arg(short, long)]
    pub input: PathBuf,

    #[arg(short, long)]
    pub output: Option<PathBuf>,
}

pub trait LanguageFrontend {
    fn parse(&self, source: &str) -> Result<BridgeModule>;
}

pub struct Orchestrator<F: LanguageFrontend> {
    frontend: F,
}

impl<F: LanguageFrontend> Orchestrator<F> {
    pub fn new(frontend: F) -> Self {
        Self { frontend }
    }

    pub fn run(&self) -> Result<()> {
        let args = Cli::parse();

        let source = std::fs::read_to_string(&args.input)
            .with_context(|| format!("Failed to read input file: {:?}", args.input))?;

        let bridge_module = self.frontend.parse(&source)?;

        let bridge_json = serde_json::to_string(&bridge_module)?;

        // Invoke rustc-executor
        // This is a simplified version. In a real scenario, we'd pass the bridge_json
        // to the executor, potentially via a temporary file or stdin.
        // For this vertical slice, we'll assume the executor is available in the path or we call it directly.

        println!("Generated Bridge IR: {}", bridge_json);

        // TODO: Call rustc-executor with the generated Bridge IR

        Ok(())
    }
}
