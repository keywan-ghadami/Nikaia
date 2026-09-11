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
        /// Kap 5.1: what stands after the `;` - options, named at the call and
        /// never positional. Empty for a function that has no `;`.
        config: Vec<ConfigParam>,
        /// ADR-007 D5: `...args: Self::dsl` - the typed spread a driver accepts
        /// a DSL's deferred parameters with. It stands where the options stand,
        /// because a DSL parameter *is* configuration, and it is exclusive with
        /// them: a function takes options or a spread, never both.
        spread: Option<Ident>,
        ret_type: Option<Type>,
        body: Block,
        is_sync: bool,   // Kap 12.1: sync keyword
        is_public: bool, // Kap 9.2
        throws: bool,    // Kap 7.1
    },

    // Kap 4.4: enum Message { Quit, Move { x: i32 }, Write(String) }
    Enum {
        name: Ident,
        variants: Vec<EnumVariant>,
        is_public: bool,
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

    // Kap 4.2: impl User { ... }
    //
    // Kap 4.7 and 7.1: `impl Summarize for User` names a trait, and `trait` is
    // `None` for the inherent form. `Error` is the one the compiler reads
    // rather than relays - a type that implements it is a type that may be
    // thrown ([ADR-023](../../../docs/specification/adr/adr-023.md) D3).
    Impl {
        /// The trait being implemented, or `None` for an inherent `impl`.
        trait_name: Option<Ident>,
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

    /// Kap 3.3: `while count < 5 { … }`.
    ///
    /// A statement rather than an expression, like `for` and unlike `if`: it
    /// repeats until a condition stops holding, and what that is worth as a
    /// value is nothing. Part I 3.3 writes both loops as statements and neither
    /// as something a `let` takes.
    While {
        cond: Expr,
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
    /// Kap 3.4: `match value { 1 => …, _ => … }`. An expression, like `if`.
    Match {
        value: Box<Expr>,
        arms: Vec<MatchArm>,
    },

    /// Kap 3.3: `0..n` and `0..=n`. What a `for` counts over, and the reason
    /// the loop needs no index arithmetic of its own.
    Range {
        start: Box<Expr>,
        end: Box<Expr>,
        /// `..=`, which includes its end.
        inclusive: bool,
    },

    /// `(a, b)` - Kap 4.5. Two or more values of different types, with no
    /// name for the pair and none for its parts.
    Tuple(Vec<Expr>),
    /// `"…"` - Kap 2.5. **Inert text.** A `{` is a brace and nothing else, so
    /// a program that writes JSON, CSS or a regular expression says what it
    /// means. The body is kept as written, escapes and all, for the same
    /// reason `LitChar` is.
    LitStr(String),
    /// `f"… {expr} …"` - Kap 2.5. **Text with code in it**, and the `f` is what
    /// says so ([ADR-035](../../../docs/specification/adr/adr-035.md)).
    ///
    /// This is a separate variant rather than a flag for a reason the compiler
    /// has already paid for once: before ADR-032 every analysis walked past a
    /// string as if it held no code, because *whether* it did was a property of
    /// its text and nothing made anyone look. A variant makes the question
    /// unavoidable - every `match` on an expression has to say what it does
    /// with this one, and the compiler names the sites that forgot.
    LitInterpolated(String),
    /// Kap 2.2: `'a'`, `'\n'`. The body is kept **as written**, escape and
    /// all: the language below spells a character literal the same way, so the
    /// lowering is a transcription and nothing has to decode it twice.
    LitChar(String),
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
        /// Kap 5.1: what stands after the `;` at the call - `request(url;
        /// timeout: 60)`. Named, in whatever order the caller wrote them;
        /// putting them in the callee's order is the lowering's business,
        /// because only the declaration knows what that order is.
        config: Vec<ConfigArg>,
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
        /// Kap 5.1 at a method call: what stands after the `;`. A DSL's
        /// deferred parameters arrive here (ADR-007 D5), which is why a method
        /// call carries them at all - before that they were parsed and dropped.
        config: Vec<ConfigArg>,
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

    /// Kap 7.1: `throw ConfigError::NotFound(path)` - raise an error. The value
    /// must implement `Error`; there is no other way to originate one, which is
    /// why [ADR-023](../../../docs/specification/adr/adr-023.md) D2 calls it
    /// load-bearing rather than convenient.
    Throw(Box<Expr>),

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
    /// `(A, B)`: the parts are in `generics` and `name` says nothing. A tuple
    /// is a type with no name and a fixed number of parts, and putting the
    /// parts where the arguments go is what lets the view analysis
    /// (`holds_view`, `names_borrowing`) reach them without knowing about
    /// tuples at all.
    pub is_tuple: bool,
}

/// One variant of an enum (Kap 4.4).
#[derive(Debug, Clone)]
pub struct EnumVariant {
    pub name: Ident,
    pub fields: VariantFields,
}

/// What a variant carries. The three shapes Kap 4.4 shows, and no others.
#[derive(Debug, Clone)]
pub enum VariantFields {
    /// `Quit`
    Unit,
    /// `Write(String)` - positional, read by position.
    Tuple(Vec<Type>),
    /// `Move { x: i32, y: i32 }` - named, read by name.
    Named(Vec<FieldDef>),
}

/// One arm of a `match` (Kap 3.4).
#[derive(Debug, Clone)]
pub struct MatchArm {
    pub pattern: MatchPattern,
    pub body: Expr,
}

/// What an arm matches. Deliberately small: every shape here is one the
/// language it lowers to spells the same way, so the lowering stays name for
/// name (ADR-011 D2) and no pattern means something different in the two.
#[derive(Debug, Clone)]
pub enum MatchPattern {
    /// `_`
    Wildcard,
    /// `1`, `"text"`, `true`
    Literal(Expr),
    /// `Op::Times`, and a bare name - which *binds*, as it does in the
    /// language below. One rule, drawn in one place.
    Path(Vec<Ident>),
    /// `Message::Write(text)`
    Tuple {
        path: Vec<Ident>,
        bindings: Vec<Ident>,
    },
    /// `Message::Move { x, y }` - shorthand only, because a rename is a `let`
    /// in the arm and needs no syntax of its own.
    Named {
        path: Vec<Ident>,
        bindings: Vec<Ident>,
    },
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

/// `timeout: 60` at a call site.
#[derive(Debug, Clone)]
pub struct ConfigArg {
    pub name: Ident,
    pub value: Expr,
}

/// Kap 5.1: one option, after the `;`.
///
/// It always has a default, which is what makes it an *option*: a caller may
/// name it or leave it out, and leaving it out is never a question about what
/// the value is. A parameter that must be passed belongs before the `;`.
#[derive(Debug, Clone)]
pub struct ConfigParam {
    pub name: Ident,
    pub ty: Type,
    /// A **literal**, and only a literal.
    ///
    /// An option's default is a constant in every program anyone writes, and an
    /// arbitrary expression would raise a question Stage 0 has no answer for -
    /// whether it is evaluated where the function is declared or where it is
    /// called. Deciding that is worth doing when something needs it.
    pub default: Expr,
}

#[derive(Debug, Clone)]
pub struct FieldDef {
    pub name: Ident,
    pub ty: Type,
    /// Kap 9.2: a field is visible outside the file that declares its struct
    /// only where it says `pub`.
    pub is_public: bool,
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
    /// `# "expression"`: what this rule is called in a message that fails at
    /// its own start, instead of everything its alternatives could begin with.
    pub label: Option<String>,
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
    pub config: Vec<ConfigParam>,
    /// ADR-007 D5: `...args: Self::dsl`, where a config zone holds one instead
    /// of options.
    pub spread: Option<Ident>,
}

/// What stands after the `;` in a declaration: options, or the typed spread of
/// [ADR-007](../../../docs/specification/adr/adr-007.md) D5.
///
/// Exclusive on purpose. An option has a default and a DSL parameter does not -
/// it is required, and what makes it configuration is that it is named rather
/// than positional. A declaration that mixed them would have to say which rule
/// applies to which name.
#[derive(Debug, Clone)]
pub enum ConfigZone {
    Options(Vec<ConfigParam>),
    /// The name the parameter struct arrives under.
    Spread(Ident),
}
