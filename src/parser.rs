// nikaia/src/parser.rs
use crate::ast::*;
use syn_grammar::grammar;

grammar! {
    grammar CompilerGrammar {
        // Placeholder: The full grammar will be defined here or in an external .g file
        // matching the goal defined in ADR-001.
        rule program -> Program = items:item* -> { 
            Program { items } 
        }

        rule item -> Item = 
            // Temporary stub for compilation
            "fn" name:ident "(" ")" "{" "}" -> {
                Item::Fn {
                    name,
                    generics: vec![],
                    args: vec![],
                    ret_type: None,
                    body: Block { stmts: vec![] },
                    is_sync: false,
                }
            }
    }
}

impl syn::parse::Parse for Program {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        CompilerGrammar::parse_program(input)
    }
}
