// crates/nikaia/src/main.rs
#![feature(rustc_private)]

// `rustc-executor` links against the compiler's own dylibs, which carry their
// own copies of `std`/`core`. Declaring `rustc_driver` here too makes this
// binary link the same dylib set instead of the regular rlib `std`; without it
// every shared crate "shows up twice" and linking fails.
extern crate rustc_driver;

use anyhow::{bail, Result};
use bridge_ir::BridgeModule;
use bridge_orchestrator::cache::{Cache, Choices};
use bridge_orchestrator::LanguageFrontend;
use clap::Parser;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

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
    /// are rejected rather than silently treated as `bridge` (ADR-019 D9).
    #[arg(long, default_value = "bridge")]
    pub backend: String,

    /// Reuse unchanged lowerings via the build cache (ADR-019).
    ///
    /// Off by default while the CLI is a one-shot transpiler: it writes a
    /// `nikaia.lock` beside the input and a store under `target/nikaia/`, and
    /// a project root only becomes well defined once `nikaia.toml` and
    /// `nikaia build` exist. It is the mechanism that is finished here, not
    /// its default.
    #[arg(long)]
    pub cache: bool,

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
/// With `--cache`, an unchanged unit is served from the store instead of being
/// lowered again (ADR-019).
fn lower_to_rust(args: &Cli, source: &str) -> Result<()> {
    let profile = Profile::parse(&args.profile)?;
    let output_path = args
        .output
        .clone()
        .unwrap_or_else(|| args.input.with_extension("rs"));

    // The unit is named relative to its root rather than by absolute path. A
    // path is exactly the kind of dimension ADR-019 D7 warns about: it varies
    // between checkouts while nothing about the build has changed, and a key
    // that moves with the directory never hits.
    let root = args.input.parent().unwrap_or(Path::new("."));
    let unit = args
        .input
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| args.input.display().to_string());
    let choices = Choices::new(&args.profile, "rust");

    let mut cache = if args.cache {
        Some(Cache::open(
            root.join("nikaia.lock"),
            root.join("target").join("nikaia").join("cache"),
            env!("NIKAIA_RUSTC_VERSION"),
            env!("CARGO_PKG_VERSION"),
        )?)
    } else {
        None
    };

    if let Some(cache) = &cache {
        if let Some(cached) = cache.lookup(&unit, source, &choices, root) {
            std::fs::write(&output_path, &cached)?;
            println!(
                "Reused {} for {} (profile: {}, from cache)",
                output_path.display(),
                args.input.display(),
                args.profile
            );
            return Ok(());
        }
    }

    let parsed = parser::parse_to_ast(source)?;
    let lowered = emit::emit_program(&parsed, profile)?;
    std::fs::write(&output_path, &lowered.rust)?;

    if let Some(cache) = &mut cache {
        // Nothing reports assets yet: compile-time I/O (`from "schema.sql"`)
        // is specified and not implemented. The dimension travels through the
        // key regardless, so switching it on later does not reshape the key.
        cache.record(&unit, source, BTreeMap::new(), &choices, &lowered.rust)?;
        cache.save()?;
    }

    println!(
        "Lowered {} to {} (profile: {})",
        args.input.display(),
        output_path.display(),
        args.profile
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
            "backend `{backend}` is not implemented (ADR-019 D9 records it as an \
             open item); available backends are interpreter, rust and bridge"
        ),
        other => bail!("unknown backend `{other}` (expected interpreter, rust or bridge)"),
    }
}
