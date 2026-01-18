// src/meta_ast.rs
// Der "Meta-AST": Die Struktur unserer .grammar Dateien.
// Dient als Input für den Parser-Generator.

use syn::{Ident, Type, LitStr, Lit};
use proc_macro2::TokenStream;

#[derive(Debug)]
pub struct GrammarDefinition {
    pub name: Ident,
    // Optionen wie 'recursion_limit'
    pub options: Vec<GrammarOption>,
    pub rules: Vec<Rule>,
}

#[derive(Debug)]
pub struct GrammarOption {
    pub name: Ident,
    pub value: Lit,
}

#[derive(Debug)]
pub struct Rule {
    pub is_pub: bool,      // "pub rule"
    pub name: Ident,
    pub return_type: Type, // "-> Stmt"
    
    // Eine Regel besteht aus einer oder mehreren Varianten (getrennt durch |)
    // Beispiel: rule stmt = | Let... | Expr...
    pub variants: Vec<RuleVariant>,
}

#[derive(Debug)]
pub struct RuleVariant {
    // Die Sequenz von Mustern: "let" name:ident() "=" ...
    pub pattern: Vec<Pattern>,
    // Der Rust-Codeblock: -> { Stmt::Let { ... } }
    pub action: TokenStream,
}

#[derive(Debug)]
pub enum Pattern {
    // Ein String-Literal: "let", "=", "fn"
    Lit(LitStr),

    // Ein Aufruf einer anderen Regel: name:ident()
    RuleCall {
        binding: Option<Ident>, // "name"
        rule_name: Ident,       // "ident"
        args: Vec<Lit>,         // Argumente, z.B. für regex("[0-9]+")
    },

    // Wiederholungen oder Optionen: rule*, rule+, rule?
    Repeat {
        binding: Option<Ident>, // "items" bei items:item()*
        pattern: Box<Pattern>,
        kind: RepeatKind,
    },

    // Gruppierungen: (a b)
    Group(Vec<Pattern>),
}

#[derive(Debug, Clone, Copy)]
pub enum RepeatKind {
    Optional,   // ?
    ZeroOrMore, // *
    OneOrMore,  // +
}

