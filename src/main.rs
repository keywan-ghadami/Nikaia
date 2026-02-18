// src/main.rs

#![feature(rustc_private)]

extern crate rustc_driver;
extern crate rustc_errors;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_session;
extern crate rustc_span;

use rustc_driver::{Callbacks, Compilation};
use rustc_interface::{Config, interface};
use rustc_middle::ty::TyCtxt;
use rustc_session::config::Input;
use rustc_span::FileName;
use winnow::Parser; // Import trait for parse
use winnow::stream::LocatingSlice;

use nikaia_driver::interpreter::Interpreter;

struct NikaiaVirtualInput {
    source_code: String,
}

impl Callbacks for NikaiaVirtualInput {
    fn config(&mut self, config: &mut Config) {
        // HACK: We feed rustc a dummy Rust program so it doesn't crash parsing our Nikaia syntax.
        // The actual Nikaia code is stored in `self.source_code` and processed in the callback.
        let dummy_source = "fn main() {}".to_string();

        config.input = Input::Str {
            name: FileName::Custom("dummy_main.rs".to_string()),
            input: dummy_source,
        };
        println!("::notice title=Nikaia Driver::Dummy Rust Source Injected");
    }

    fn after_analysis<'tcx>(
        &mut self,
        _compiler: &interface::Compiler,
        _tcx: TyCtxt<'tcx>,
    ) -> Compilation {
        println!("::group::Semantic Analysis");
        println!("[Nikaia] Parsing AST from internal source...");

        // Parse the ACTUAL Nikaia source code
        let input = LocatingSlice::new(self.source_code.as_str());
        let parse_result = nikaia_driver::parser::CompilerGrammar::parse_program.parse(input);

        match parse_result {
            Ok(program) => {
                println!(
                    "[Nikaia] Parse Success: {} items found.",
                    program.items.len()
                );

                // EXECUTE THE INTERPRETER
                println!("::endgroup::");
                println!("::group::Nikaia Runtime (Interpreter)");

                let interpreter = Interpreter::new();
                interpreter.run(&program);
            }
            Err(e) => {
                println!("[Nikaia] Parse Error: {}", e);
                return Compilation::Stop;
            }
        }

        println!("::endgroup::");

        // Stop the compiler here
        Compilation::Stop
    }
}

fn main() {
    // Our Nikaia Hello World Program
    let nikaia_src = r#"
        use std::http
        fn main() {
            println("Nikaia System Init...");
            spawn({
                log("Async Task Running")
            })
            dsl js { } 
        }
    "#
    .to_string();

    let mut callbacks = NikaiaVirtualInput {
        source_code: nikaia_src,
    };

    // Rustc driver arguments
    let args = vec![
        "nikaia_driver".to_string(),
        "--crate-type".to_string(),
        "bin".to_string(),
        "-o".to_string(),
        "output_bin".to_string(),
        "dummy_main.rs".to_string(), // Matches the name in config
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
