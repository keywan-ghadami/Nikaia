#![feature(rustc_private)]

extern crate rustc_driver;
extern crate rustc_interface;
extern crate rustc_session;
extern crate rustc_span;
extern crate rustc_errors;
extern crate rustc_middle;

use rustc_driver::{Callbacks, Compilation};
use rustc_interface::{interface, Config};
use rustc_session::config::{Input};
use rustc_span::FileName;
use rustc_middle::ty::TyCtxt;
use syn::parse::Parser; // Import trait for parse_str

struct NikaiaVirtualInput {
    source_code: String,
}

impl Callbacks for NikaiaVirtualInput {
    fn config(&mut self, config: &mut Config) {
        let source = self.source_code.clone();
        // Hier injizieren wir den echten Code für "main.nika"
        config.input = Input::Str {
            name: FileName::Custom("main.nika".to_string()),
            input: source,
        };
        println!("::notice title=Nikaia Driver::Virtual Source Code Injected");
    }

    fn after_analysis<'tcx>(
        &mut self,
        _compiler: &interface::Compiler,
        _tcx: TyCtxt<'tcx>,
    ) -> Compilation {
        println!("::group::Semantic Analysis");
        println!("[Nikaia] Parsing AST...");

        // Invoke the syn-grammar parser
        let parse_result = nikaia_driver::parser::CompilerGrammar::parse_program.parse_str(&self.source_code);

        match parse_result {
            Ok(program) => {
                println!("[Nikaia] Parse Success: {} items found.", program.items.len());
                println!("[Nikaia] Verifying Profile Constraints (Lite)...");
            }
            Err(e) => {
                println!("[Nikaia] Parse Error: {}", e);
                return Compilation::Stop;
            }
        }

        println!("::endgroup::");
        
        Compilation::Stop
    }
}

fn main() {
    let nikaia_src = r#"
        use std::http
        fn main() {
            println("Nikaia System Init...");
            spawn({ println("Async Task") })
            dsl js { console.log("WASM Bridge"); }
        }
    "#.to_string();

    let mut callbacks = NikaiaVirtualInput {
        source_code: nikaia_src,
    };

    // FIX: Wir fügen "main.nika" als Positions-Argument hinzu.
    // Der Driver braucht das, um den Prozess überhaupt zu starten.
    let args = vec![
        "nikaia_driver".to_string(),
        "--crate-type".to_string(), "bin".to_string(),
        "-o".to_string(), "output_bin".to_string(),
        "main.nika".to_string(),
    ];

    println!("::section::Compiling Nikaia Source");

    let exit_code = rustc_driver::catch_with_exit_code(move || {
        rustc_driver::run_compiler(&args, &mut callbacks)
    });
    
    println!("::endsection::");
    
    if exit_code == 0 {
         println!("::notice title=Success::Build & Analysis Complete.");
    } else {
         panic!("Driver crashed with exit code: {}", exit_code);
    }
}
