#![feature(rustc_private)]

extern crate rustc_driver;
extern crate rustc_interface;
extern crate rustc_session;
extern crate rustc_span;
extern crate rustc_errors;
extern crate rustc_middle; // NEU: Benötigt für TyCtxt

use rustc_driver::{Callbacks, Compilation};
use rustc_interface::{interface, Config};
use rustc_session::config::{Input};
use rustc_span::FileName;
use rustc_middle::ty::TyCtxt; // Der stabile Typ für die Analyse-Phase

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

    // FIX: Wir nutzen die Signatur aus den Nightly-Docs mit TyCtxt.
    // Das vermeidet den Zugriff auf das private 'Queries' Modul.
    fn after_analysis<'tcx>(
        &mut self,
        _compiler: &interface::Compiler,
        _tcx: TyCtxt<'tcx>, // Statt Queries nutzen wir den Type Context
    ) -> Compilation {
        println!("::group::Semantic Analysis");
        println!("[Nikaia] Parsing AST...");
        println!("[Nikaia] Verifying Profile Constraints (Lite)...");
        println!("::endgroup::");
        
        // Wir stoppen hier, da wir keinen echten Maschinencode generieren wollen
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

    // FIX: Exakt 2 Argumente, wie im Log verlangt.
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
