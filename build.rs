// nikaia/build.rs
use syn_grammar::Generator;
use std::env;
use std::fs;
use std::path::Path;

fn main() {
    let out_dir = env::var_os("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("generated_parser.rs");

    // Generator initialisieren (Root ist 'grammar/' Ordner)
    let gen = Generator::new("grammar");
    
    // Nikaia.grammar (und seine Eltern) kompilieren
    let rust_code = gen.generate("Nikaia.grammar").expect("Parser generation failed");

    fs::write(&dest_path, rust_code.to_string()).unwrap();
    
    // Rebuild wenn sich Grammatiken ändern
    println!("cargo:rerun-if-changed=grammar");
}
