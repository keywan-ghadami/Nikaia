// crates/nikaia-std/build.rs
//
// The parts of Nikaia's std that are written in Nikaia are compiled here, by
// the Stage 0 compiler, into `OUT_DIR`, and `lib.rs` includes the result. Every
// `src/*.nika` becomes a module of the same name.
//
// This is the first place the compiler compiles something the project itself
// runs on. It is deliberately the library alone (`default-features = false`):
// a build script that linked the rustc backend would link `rustc_driver`, and
// nothing about turning `digit_value` into Rust needs that.

use std::path::Path;

fn main() -> anyhow::Result<()> {
    let out_dir = std::env::var("OUT_DIR")?;
    let sources = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");

    for entry in std::fs::read_dir(&sources)? {
        let path = entry?.path();
        if path.extension().and_then(|e| e.to_str()) != Some("nika") {
            continue;
        }
        println!("cargo:rerun-if-changed={}", path.display());

        let source = std::fs::read_to_string(&path)?;
        let parsed = nikaia::parser::parse_to_ast(&source)
            .map_err(|e| anyhow::anyhow!("{}: {e}", path.display()))?;
        let lowered = nikaia::emit::emit_program(&parsed, nikaia::emit::Build::default())
            .map_err(|e| anyhow::anyhow!("{}: {e}", path.display()))?;

        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .expect("a file name");
        std::fs::write(Path::new(&out_dir).join(format!("{name}.rs")), lowered.rust)?;
    }

    println!("cargo:rerun-if-changed=build.rs");
    Ok(())
}
