// crates/nikaia/src/ast/mod.rs
// Nikaia AST definition.
// Based on ADR-001 and Part I/II/III documents.

// Identifiers are interned: the AST stores a `Symbol` handle and the text is
// recovered through the `InternerContext` that produced it.
//
// This is Spec Part II, 10.6 in practice - identifiers are short, repeat
// constantly, and are compared far more often than they are read, so interning
// stores each distinct spelling once and makes equality an integer comparison.
use winnow_grammar::Symbol as Ident;

/// A byte range in the `.nika` source a node was parsed from.
///
/// Every diagnostic a user ever sees has to end up pointing at one of these:
/// the compiler emits Rust, and an error reported against the emitted file
/// names a line nobody wrote.
pub type Span = std::ops::Range<usize>;

/// A node and where it came from.
#[derive(Debug, Clone)]
pub struct Spanned<T> {
    pub node: T,
    pub span: Span,
}

impl<T> Spanned<T> {
    pub fn new(node: T, span: Span) -> Self {
        Self { node, span }
    }
}

/// Lets a rule written `-> Spanned<T> @=` wrap its value without an action.
impl<T> winnow_grammar::WithSpan<T> for Spanned<T> {
    fn with_span(node: T, span: Span) -> Self {
        Self { node, span }
    }
}

/// Ein Nikaia-Programm ist eine Liste von Top-Level Items.
#[derive(Debug, Clone)]
pub struct Program {
    pub items: Vec<Spanned<Item>>,
}

/// Top-Level Konstrukte (außerhalb von Funktionen)
#[derive(Debug, Clone)]
pub enum Item {
    // Kap 5.1: fn add(a: i32) -> i32 { ... }
    Fn {
        /// `None` for the anonymous constructor of Kap 4.2 - a `pub fn` with no
        /// name inside an `impl`, called as `Type(…)`.
        name: Option<Ident>,
        generics: Vec<GenericParam>, // Kap 4.5: [T]
        /// Kap 4.2/5.1: `&self`, `&mut self` or `self`, when this is a method.
        receiver: Option<Receiver>,
        args: Vec<FnArg>,
        ret_type: Option<Type>,
        body: Block,
        is_sync: bool,   // Kap 12.1: sync keyword
        is_public: bool, // Kap 9.2
        throws: bool,    // Kap 7.1
    },

    // Kap 4.1: struct User { ... }
    Struct {
        name: Ident,
        generics: Vec<GenericParam>,
        fields: Vec<FieldDef>,
        is_public: bool,
        // ADR-008, D6: `@borrowed` asserts that no value of this type escapes
        // the buffer it points into.
        is_borrowed: bool,
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
        methods: Vec<Spanned<Item>>, // Enthält Item::Fn
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
    //
    // The rules are parsed, not kept as text: the whole point of this item is
    // that the compiler lowers it onto the parser backend (`grammar!`), and it
    // cannot check a frame or generate a fold driver from a string.
    Grammar(GrammarDef),

    // Kap 9.2: use std::http
    Import {
        path: Vec<Ident>,
    },
}

/// Ein Block von Statements { ... }
#[derive(Debug, Clone)]
pub struct Block {
    pub stmts: Vec<Spanned<Stmt>>,
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

    // Kap 2.1: x = 20, und x += 1
    Assign {
        target: Expr,
        /// `Some(Add)` for `+=`; `None` for a plain assignment.
        op: Option<BinaryOp>,
        value: Expr,
    },

    // Kap 3.3: for x in xs { ... }, for (k, v) in map { ... }
    For {
        bindings: Vec<Ident>,
        iter: Expr,
        body: Block,
    },

    // Kap 7.1: return, return value
    Return(Option<Expr>),

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

    // Part II, Kap 10.2/10.5: dsl Json from input
    //
    // The other half of the grammar protocol: `from` runs a named grammar over
    // an input that is already a value, rather than over a foreign-syntax block.
    DslFrom {
        grammar: Ident,
        input: Box<Expr>,
    },

    // Ein qualifizierter Pfad: Summary::new, u8::from_str_radix
    Path(Vec<Ident>),

    // Methodenaufruf: acc.record(m)
    MethodCall {
        receiver: Box<Expr>,
        method: Ident,
        args: Vec<Expr>,
    },

    // Feldzugriff: self.min
    Field {
        base: Box<Expr>,
        name: Ident,
    },

    // Struct-Literal: Reading { name, temp }
    StructLit {
        name: Ident,
        fields: Vec<FieldInit>,
    },

    // Kap 5.2/5.3: `fn(acc, m) { ... }`, and the implicit form `fn: a + b`,
    // whose parameters are the `a`, `b`, `c` its body uses.
    Closure {
        params: Vec<Ident>,
        implicit: bool,
        body: Block,
    },

    // -value, !flag
    Unary {
        op: UnaryOp,
        expr: Box<Expr>,
    },

    // a + b, a < b
    Binary {
        op: BinaryOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },

    // Kap 7.1: expr?
    Try(Box<Expr>),

    // Kap 2.2: 10.0
    LitFloat(String),

    // Kap 4.5: map[key]
    Index {
        base: Box<Expr>,
        index: Box<Expr>,
    },

    // `self.sum as f64`
    Cast {
        expr: Box<Expr>,
        ty: Type,
    },

    // Kap 3.5: value ?? fallback
    Coalesce {
        value: Box<Expr>,
        fallback: Box<Expr>,
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
    // Part II, 10.6 / ADR-008: `&str` is a view marker, not a lifetime. The
    // flag records that the source said `&`; what it lowers to is the emitter's
    // business.
    pub is_view: bool,
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

/// `Reading { name, temp }` - a field with no value is shorthand for `name: name`.
#[derive(Debug, Clone)]
pub struct FieldInit {
    pub name: Ident,
    pub value: Option<Expr>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Neg,
    Not,
    Ref,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
}

// --- Part II, Kapitel 10: Grammatiken ---

/// `grammar Measurements { ... }`
#[derive(Debug, Clone)]
pub struct GrammarDef {
    pub name: Ident,
    pub rules: Vec<GrammarRule>,
}

/// `@frame(boundary: "\n") pub rule MEASUREMENT -> Reading = ... -> { ... }`
#[derive(Debug, Clone)]
pub struct GrammarRule {
    pub name: Ident,
    pub is_public: bool,
    /// Part II, 10.7: the rule is a resynchronization unit.
    pub frame: Option<FrameAttr>,
    pub ret_type: Option<Type>,
    pub alts: Vec<GrammarAlt>,
    pub span: Span,
}

/// The keyed attribute of ADR-009 D1: `@frame`, `@frame(boundary: "\n")`,
/// `@frame(boundary: "\n", unchecked)`. A bare `@frame` leaves `boundary`
/// empty and the boundary is inferred downstream from the trailing literal.
#[derive(Debug, Clone, Default)]
pub struct FrameAttr {
    pub boundary: Option<String>,
    pub unchecked: bool,
}

/// One alternative of a rule: a pattern and the action that builds its value.
#[derive(Debug, Clone)]
pub struct GrammarAlt {
    pub pattern: Spanned<Pattern>,
    pub action: Option<Block>,
}

#[derive(Debug, Clone)]
pub enum Pattern {
    /// `a b c` - matched in order.
    Seq(Vec<Spanned<Pattern>>),
    /// `a | b` - first match wins.
    Choice(Vec<Spanned<Pattern>>),
    /// `name:pattern` - binds the result for the action block.
    Bind {
        name: Ident,
        pat: Box<Spanned<Pattern>>,
    },
    /// `";"`
    Literal(String),
    /// A rule reference (`NAME`), a built-in (`digit`, `frame_end`), or a call
    /// to either (`until(";" | frame_end)`, `list(pair, ",")`). One node,
    /// because the grammar cannot tell them apart and does not need to.
    ///
    /// `generics` is what `dec[i32](digit{1,2})` writes between the name and
    /// the arguments. Nikaia spells a type argument with brackets everywhere
    /// (Kap 4.3), and the backend spells it with angles; the emitter is where
    /// the two meet.
    Ref {
        name: Ident,
        generics: Vec<Type>,
        args: Vec<Spanned<Pattern>>,
    },
    /// `p*`, `p+`, `p?`, `p{1,2}`
    Repeat {
        pat: Box<Spanned<Pattern>>,
        rep: Repeat,
    },
    /// `( ... )` - grouping only, never a delimiter.
    Group(Box<Spanned<Pattern>>),
    /// `=>` - the commit point (Part II, 10.1).
    Cut,
    /// `fold(...)` / `par_fold(...)`
    Fold(Box<FoldSpec>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Repeat {
    Star,
    Plus,
    Optional,
    /// `p{n}`
    Exactly(u32),
    /// `p{n,}`
    AtLeast(u32),
    /// `p{n,m}`
    Between(u32, u32),
}

/// `fold(rule, init, step)` and `par_fold(rule, init, step, merge)`.
///
/// The merge is what separates them: ADR-009 D2 - parallel parsing is a frame
/// plus a monoid, and the monoid is exactly this merge.
#[derive(Debug, Clone)]
pub struct FoldSpec {
    pub parallel: bool,
    pub rule: Ident,
    pub init: Expr,
    pub step: Expr,
    pub merge: Option<Expr>,
}

/// Kap 4.2: what a method takes as its subject.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Receiver {
    pub is_ref: bool,
    pub is_mut: bool,
}

/// What a function declaration takes: an optional subject and the rest.
#[derive(Debug, Clone, Default)]
pub struct FnParams {
    pub receiver: Option<Receiver>,
    pub args: Vec<FnArg>,
}
