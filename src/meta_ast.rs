// src/meta_ast.rs
// Stage 0 Meta-AST mit Vererbungs-Support

use syn::{Ident, Type, LitStr, Lit};
use proc_macro2::TokenStream;

#[derive(Debug, Clone)] // Clone ist wichtig für das Mergen!
pub struct GrammarDefinition {
    pub name: Ident,
    // Die Vererbung: grammar Full : Core
    pub inherits: Option<Ident>, 
    pub options: Vec<GrammarOption>,
    pub rules: Vec<Rule>,
}

#[derive(Debug, Clone)]
pub struct GrammarOption {
    pub name: Ident,
    pub value: Lit,
}

#[derive(Debug, Clone)]
pub struct Rule {
    pub is_pub: bool,
    pub name: Ident,
    pub return_type: Type,
    pub variants: Vec<RuleVariant>,
}

#[derive(Debug, Clone)]
pub struct RuleVariant {
    pub pattern: Vec<Pattern>,
    pub action: TokenStream,
}

#[derive(Debug, Clone)]
pub enum Pattern {
    Lit(LitStr),
    RuleCall {
        binding: Option<Ident>,
        rule_name: Ident,
        args: Vec<Lit>,
    },
    Repeat {
        binding: Option<Ident>,
        pattern: Box<Pattern>,
        kind: RepeatKind,
    },
    Group(Vec<Pattern>),
}

#[derive(Debug, Clone, Copy)]
pub enum RepeatKind {
    Optional,   // ?
    ZeroOrMore, // *
    OneOrMore,  // +
}
