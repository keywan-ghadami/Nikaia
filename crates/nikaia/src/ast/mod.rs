// crates/nikaia/src/ast/mod.rs
// Nikaia AST definition matching Spec 0.0.4
// Based on ADR-001 and Part I/II/III documents.

use syn::Ident;

/// Ein Nikaia-Programm ist eine Liste von Top-Level Items.
#[derive(Debug, Clone)]
pub struct Program {
    pub items: Vec<Item>,
}

/// Top-Level Konstrukte (außerhalb von Funktionen)
#[derive(Debug, Clone)]
pub enum Item {
    // Kap 5.1: fn add(a: i32) -> i32 { ... }
    Fn {
        name: Ident,
        generics: Vec<GenericParam>, // Kap 4.5: [T]
        args: Vec<FnArg>,
        ret_type: Option<Type>,
        body: Block,
        is_sync: bool, // Kap 12.1: sync keyword
    },

    // Kap 4.1: struct User { ... }
    Struct {
        name: Ident,
        generics: Vec<GenericParam>,
        fields: Vec<FieldDef>,
    },

    // Kap 4.3: enum Message { ... }
    Enum {
        name: Ident,
        generics: Vec<GenericParam>,
        variants: Vec<EnumVariant>,
    },

    // Kap 4.2: impl User { ... }
    Impl {
        target: Type,
        methods: Vec<Item>, // Enthält Item::Fn
    },

    // Part III, Kap 14.1: test "Name" { ... }
    Test {
        name: String,
        body: Block,
    },

    // Part III, Kap 13.4: bench "Name" { ... }
    Bench {
        name: String,
        body: Block,
    },

    // Part II, Kap 10.1: grammar ColorParser { ... }
    Grammar {
        name: Ident,
        content: String, // Simplified from TokenStream
    },

    // Kap 9.2: use std::http
    Import {
        path: String,
    },
}

/// Ein Block von Statements { ... }
#[derive(Debug, Clone)]
pub struct Block {
    pub stmts: Vec<Stmt>,
}

/// Anweisungen innerhalb eines Blocks
#[derive(Debug, Clone)]
pub enum Stmt {
    // Kap 2.1: let mut x = 10
    Let {
        name: Ident,
        mutable: bool,
        ty: Option<Type>, // Type Inference macht dies optional
        value: Expr,
    },

    // Kap 2.1: x = 20
    Assign {
        target: Expr,
        value: Expr,
    },

    // Ein "nackter" Ausdruck (z.B. Funktionsaufruf oder Return-Value)
    Expr(Expr),
}

/// Ausdrücke (Alles, was einen Wert zurückgibt)
#[derive(Debug, Clone)]
pub enum Expr {
    // Primitive
    LitInt(i64),
    LitStr(String),
    LitBool(bool),
    Variable(Ident),

    // Kap 3.1: Blöcke sind Expressions
    Block(Block),

    // Kap 3.2: if cond { ... } else { ... }
    If {
        cond: Box<Expr>,
        then_branch: Block,
        else_branch: Option<Block>,
    },

    // Kap 5.1: Funktionsaufruf add(1, 2)
    Call {
        func: Box<Expr>,
        args: Vec<Expr>,
    },

    // Kap 8.2: spawn({ ... }) oder spawn(move { ... })
    // Auch Kap 5.2: Block Lambdas
    Spawn {
        body: Box<Expr>, // Meistens ein Expr::Block
        is_move: bool,   // Kap 8.3
    },

    // Part II, Kap 10.5: dsl sql db { ... }
    Dsl {
        target: Ident,          // z.B. sql
        context: Option<Ident>, // z.B. db (optional)
        content: String,        // Simplified from TokenStream
    },

    // Part III, Kap 16: unsafe asm { Bindings } { Body }
    Asm {
        bindings: Vec<AsmBinding>, // Block 1
        code: String,              // Block 2 (Roher Text)
    },

    // Kap 7.1: Error Handling ?{ ... }
    TryCatch {
        expr: Box<Expr>,
        handler: Block, // Der Block mit 'error' Variable
    },

    // Kap 3.4: match value { ... }
    Match {
        expr: Box<Expr>,
        arms: Vec<MatchArm>,
    },
}

// --- Helper Strukturen ---

#[derive(Debug, Clone)]
pub struct Type {
    pub name: Ident,
    pub generics: Vec<Type>, // Recursive: Shared[Locked[T]]
}

#[derive(Debug, Clone)]
pub struct GenericParam {
    pub name: Ident,
    // Constraints wie T: Drawable fehlen hier noch vereinfacht
}

#[derive(Debug, Clone)]
pub struct FnArg {
    pub name: Ident,
    pub ty: Type,
}

#[derive(Debug, Clone)]
pub struct FieldDef {
    pub name: Ident,
    pub ty: Type,
}

#[derive(Debug, Clone)]
pub struct EnumVariant {
    pub name: Ident,
    pub data: Option<Vec<FieldDef>>, // Für: Variant { x: i32 }
}

#[derive(Debug, Clone)]
pub struct MatchArm {
    pub pattern: Expr, // Vereinfacht
    pub body: Expr,
}

// Part III, Kap 16.1: $dst = out(reg) result
#[derive(Debug, Clone)]
pub struct AsmBinding {
    pub alias: Ident,      // $dst
    pub direction: String, // "out", "in", "inout"
    pub location: String,  // "reg", "mem"
    pub variable: Ident,   // result
}
