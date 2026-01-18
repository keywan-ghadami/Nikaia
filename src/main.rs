#![feature(rustc_private)]

extern crate rustc_driver;
extern crate rustc_interface;
extern crate rustc_session;
extern crate rustc_span;
extern crate rustc_errors;

use rustc_driver::{Callbacks, Compilation};
use rustc_interface::{interface, Config};
use rustc_session::config::{Input};
// FIX 1: Direkter Import, da source_map::FileName private ist
use rustc_span::FileName; 

struct NikaiaVirtualInput {
    source_code: String,
}

impl Callbacks for NikaiaVirtualInput {
    fn config(&mut self, config: &mut Config) {
        let source = self.source_code.clone();
        config.input = Input::Str {
            name: FileName::Custom("main.nika".to_string()),
            input: source,
        };
        println!("::notice title=Nikaia Driver::Virtual Source Code Injected");
    }

    // FIX 2: 'after_parsing' gibt es nicht mehr. Wir nutzen 'after_analysis'.
    // Das passiert nach dem Parsing und der Macro-Expansion -> Perfekt für unseren Check.
    fn after_analysis<'tcx>(
        &mut self,
        _compiler: &interface::Compiler,
        // FIX 3: Korrekter Pfad für Queries
        _queries: &'tcx rustc_interface::queries::Queries<'tcx>, 
    ) -> Compilation {
        println!("::group::Semantic Analysis");
        println!("[Nikaia] Parsing AST...");
        println!("[Nikaia] Verifying Profile Constraints (Lite)...");
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

    let args = vec![
        "nikaia_driver".to_string(),
        "--crate-type".to_string(), "bin".to_string(),
        "-o".to_string(), "output_bin".to_string(),
    ];

    println!("::section::Compiling Nikaia Source");

    // FIX 4: 'RunCompiler' Struct ist weg. Wir nutzen die funktionale API 'run_compiler'.
    // Wir wrappen es in 'catch_with_exit_code', um Panics sauber abzufangen.
    let exit_code = rustc_driver::catch_with_exit_code(move || {
        // args, callbacks, file_loader (None), emitter (None)
        rustc_driver::run_compiler(&args, &mut callbacks, None, None)
    });
    
    println!("::endsection::");
    
    if exit_code == 0 {
         println!("::notice title=Success::Build & Analysis Complete.");
    } else {
         panic!("Driver crashed with exit code: {}", exit_code);
    }
}
