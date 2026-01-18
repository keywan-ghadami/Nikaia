// nikaia/src/parser.rs
use crate::ast::*; // AST muss verfügbar sein

// Inkludiere den generierten Code
include!(concat!(env!("OUT_DIR"), "/generated_parser.rs"));

// Optional: Wrapper für syn::parse::Parse Trait, falls nötig
impl syn::parse::Parse for Program {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        parse_program(input) // Aufruf der generierten Funktion
    }
}
