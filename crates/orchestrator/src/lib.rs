//! The build orchestrator.
//!
//! Its caching half is implemented in [`cache`] and specified by ADR-021. The
//! other half - translating a project manifest into a `Cargo.toml`, driving
//! `cargo` over it and injecting `RUSTC_WORKSPACE_WRAPPER` (ADR-002 D1,
//! ADR-003 D2) - is [`project`]. What is still a placeholder is
//! [`Orchestrator::run`] below, the one-file path from a source file to the
//! Rust source text a language frontend lowers it to.

pub mod cache;
pub mod project;

use anyhow::{Context, Result};
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    #[arg(short, long)]
    pub input: PathBuf,

    #[arg(short, long)]
    pub output: Option<PathBuf>,
}

/// What a language has to offer this orchestrator: a lowering from its own
/// source text to Rust source text (ADR-003 D1).
///
/// The interface is text because the interface between the language and the
/// machinery that compiles it is text: everything the orchestrator does with the
/// answer - hash it, store it, write it where `cargo` will find it - it does to
/// bytes, and a frontend that produced anything else would have to be understood
/// here.
pub trait LanguageFrontend {
    fn lower(&self, source: &str) -> Result<String>;
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

        let rust = self.frontend.lower(&source)?;

        match &args.output {
            Some(path) => std::fs::write(path, &rust)
                .with_context(|| format!("Failed to write output file: {path:?}"))?,
            None => print!("{rust}"),
        }

        Ok(())
    }
}
