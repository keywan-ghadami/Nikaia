// crates/nikaia/src/emit/mod.rs
//
// Stage 0 of the bootstrap compiler: Nikaia in, Rust out.
//
// The three things this file exists for are the three halves of the grammar
// protocol that the parser backend already implements and Nikaia could not yet
// reach (README, "Bootstrap Compiler"):
//
//   * `grammar Name { rule ... }`  ->  `grammar! { grammar Name { ... } }`
//   * `@frame(boundary: "\n")`     ->  `#[frame(boundary = "\n")]`
//   * `dsl Name from input`        ->  the generated `par_fold` driver, with
//                                      the `Parallelism` the profile asks for
//
// What it is *not* is a type checker. The lowering is syntactic: every action
// block, every `init`/`step`/`merge` and every function body is emitted as
// written. Where Nikaia and Rust disagree about what a name means - a `&mut
// self` method used as a monoid's merge, `std::fs` - the emitted program says
// so by not compiling, rather than this file guessing. That is deliberate:
// guessing a receiver would be a semantic decision made in a printer.
//
// It emits a [`SourceMap`] alongside the code, because a transpiler that only
// emits code can only be told about errors in a file nobody wrote (ADR-012).

use std::collections::{HashMap, HashSet};

use anyhow::{anyhow, Result};
use winnow_grammar::Symbol;

use crate::ast::{
    BinaryOp, Block, Expr, FoldSpec, FrameAttr, GrammarDef, GrammarRule, Item, Pattern, Repeat,
    Span, Spanned, Stmt, Type, UnaryOp,
};
use crate::parser::Parsed;

/// Part I/II: the runtime a program is compiled for.
///
/// It is not a dialect - the same source compiles under both. ADR-009: under
/// Lite a `par_fold` runs as a sequential fold, which is the driver's
/// `Parallelism::Off` and nothing else.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Profile {
    Lite,
    #[default]
    Advanced,
}

impl Profile {
    pub fn parse(name: &str) -> Result<Profile> {
        match name {
            "lite" => Ok(Profile::Lite),
            "advanced" => Ok(Profile::Advanced),
            other => Err(anyhow!(
                "unknown profile `{other}` (expected lite or advanced)"
            )),
        }
    }

    /// How the generated driver is asked to cut the input.
    fn parallelism(self) -> &'static str {
        match self {
            Profile::Lite => "Parallelism::Off",
            Profile::Advanced => "Parallelism::Auto",
        }
    }
}

/// The lifetime the parser backend gives its input. A Nikaia view (`&str`) is
/// tied to it, which is the whole of what `&` means here: ADR-008 - a view
/// marker, never an annotation the user writes.
const INPUT_LIFETIME: &str = "'a";

/// Where a type is being written, which is all that decides how a view's
/// lifetime is spelled. The source never says either way (ADR-008).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Lifetimes {
    /// Inside the grammar module and on the structs it builds, where `'a` is
    /// the input's lifetime and has to be named to tie the two together.
    Named,
    /// In a function signature, where Rust elides it and naming a lifetime that
    /// the signature does not declare would not compile.
    Elided,
}

// --- The output, and the way back ---

/// What the lowering produces: the Rust, and where each part of it came from.
#[derive(Debug, Clone)]
pub struct Lowered {
    pub rust: String,
    pub map: SourceMap,
}

/// Emitted byte range -> the `.nika` span that produced it.
///
/// This is the whole answer to "the error points at a line nobody wrote": the
/// compiler that consumes the emitted Rust reports offsets into it, and every
/// one of them can be traded back for a place in the source the user has open.
#[derive(Debug, Clone, Default)]
pub struct SourceMap {
    /// Innermost first: a lookup returns the first range that contains the
    /// offset, and the emitter records a node before recording anything that
    /// encloses it.
    entries: Vec<MapEntry>,
}

#[derive(Debug, Clone)]
struct MapEntry {
    generated: std::ops::Range<usize>,
    source: Span,
}

impl SourceMap {
    /// The narrowest source span whose emitted text covers `offset`.
    pub fn source_span(&self, offset: usize) -> Option<Span> {
        self.entries
            .iter()
            .find(|e| e.generated.contains(&offset))
            .map(|e| e.source.clone())
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// The emitted text under construction, and the map being built with it.
#[derive(Debug, Default)]
struct Out {
    buf: String,
    map: SourceMap,
}

impl Out {
    fn push(&mut self, text: &str) {
        self.buf.push_str(text);
    }

    /// Record that everything `f` writes came from `span`.
    fn from<R>(&mut self, span: &Span, f: impl FnOnce(&mut Out) -> Result<R>) -> Result<R> {
        let start = self.buf.len();
        let value = f(self)?;
        self.map.entries.push(MapEntry {
            generated: start..self.buf.len(),
            source: span.clone(),
        });
        Ok(value)
    }

    /// Render into a buffer of its own, so the caller can decide what to do
    /// with the result before committing to it.
    fn scratch(f: impl FnOnce(&mut Out) -> Result<()>) -> Result<Out> {
        let mut out = Out::default();
        f(&mut out)?;
        Ok(out)
    }

    /// Append a scratch buffer, moving its map entries into place. Order is
    /// preserved, so "innermost first" survives.
    fn append(&mut self, other: Out) {
        let offset = self.buf.len();
        self.buf.push_str(&other.buf);
        for mut entry in other.map.entries {
            entry.generated.start += offset;
            entry.generated.end += offset;
            self.map.entries.push(entry);
        }
    }
}

pub fn emit_program(parsed: &Parsed, profile: Profile) -> Result<Lowered> {
    Emitter::new(parsed, profile).program()
}

struct Emitter<'p> {
    parsed: &'p Parsed,
    profile: Profile,
    /// Structs that hold a view into the input, and so need the input lifetime
    /// wherever they are named.
    borrowing: HashSet<Symbol>,
    grammars: HashMap<Symbol, &'p GrammarDef>,
}

impl<'p> Emitter<'p> {
    fn new(parsed: &'p Parsed, profile: Profile) -> Self {
        let mut grammars = HashMap::new();
        for item in &parsed.program.items {
            if let Item::Grammar(def) = &item.node {
                grammars.insert(def.name, def);
            }
        }

        Self {
            parsed,
            profile,
            borrowing: borrowing_structs(parsed),
            grammars,
        }
    }

    fn text(&self, sym: Symbol) -> &str {
        self.parsed.text(sym)
    }

    fn program(&self) -> Result<Lowered> {
        let mut out = Out::default();
        out.push("// Generated by the Nikaia bootstrap compiler (Stage 0).\n");
        out.push("// Edit the .nika source, not this file.\n\n");

        if self
            .parsed
            .program
            .items
            .iter()
            .any(|i| matches!(i.node, Item::Grammar(_)))
        {
            out.push("use winnow_grammar::grammar;\n");
        }
        if self.uses_driver() {
            out.push("use winnow_grammar::rt::Parallelism;\n");
            out.push("use winnow_grammar::ParseContext;\n");
        }
        out.push("\n");

        for item in &self.parsed.program.items {
            out.from(&item.span, |out| self.item(out, &item.node))?;
            out.push("\n");
        }

        Ok(Lowered {
            rust: out.buf,
            map: out.map,
        })
    }

    /// Whether any `dsl … from …` in the program reaches a parallel entry rule.
    fn uses_driver(&self) -> bool {
        let mut found = false;
        for item in &self.parsed.program.items {
            if let Item::Fn { body, .. } = &item.node {
                visit_block(body, &mut |e| {
                    if let Expr::DslFrom { grammar, .. } = e {
                        if let Some(def) = self.grammars.get(grammar) {
                            if entry_rule(def)
                                .map(|r| par_fold_of(r).is_some())
                                .unwrap_or(false)
                            {
                                found = true;
                            }
                        }
                    }
                });
            }
        }
        found
    }

    fn item(&self, out: &mut Out, item: &Item) -> Result<()> {
        match item {
            Item::Grammar(def) => self.grammar(out, def),
            Item::Struct {
                name,
                fields,
                is_public,
                is_borrowed,
                ..
            } => {
                if *is_borrowed {
                    out.push(
                        "// @borrowed (ADR-008 D6): asserted in the source; the check that no\n\
                         // value of this type escapes its buffer is not implemented yet.\n",
                    );
                }
                out.push("#[derive(Debug, Clone)]\n");
                let vis = if *is_public { "pub " } else { "" };
                let params = if self.borrowing.contains(name) {
                    format!("<{INPUT_LIFETIME}>")
                } else {
                    String::new()
                };
                out.push(&format!("{vis}struct {}{params} {{\n", self.text(*name)));
                for field in fields {
                    // Public, because the actions that build this struct are
                    // generated into the grammar's own module.
                    out.push(&format!(
                        "    pub {}: {},\n",
                        self.text(field.name),
                        self.ty(&field.ty, Lifetimes::Named)
                    ));
                }
                out.push("}\n");
                Ok(())
            }
            Item::Fn {
                name,
                args,
                ret_type,
                body,
                is_sync,
                ..
            } => {
                if *is_sync {
                    out.push("// sync (Part II, 12.1): pure CPU, cannot pause. Not checked yet.\n");
                }
                let params = args
                    .iter()
                    .map(|a| {
                        format!(
                            "{}: {}",
                            self.text(a.name),
                            self.ty(&a.ty, Lifetimes::Elided)
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                let ret = match ret_type {
                    Some(ty) => format!(" -> {}", self.ty(ty, Lifetimes::Elided)),
                    None => String::new(),
                };
                out.push(&format!("fn {}({params}){ret} ", self.text(*name)));
                self.block(out, body, 0)?;
                out.push("\n");
                Ok(())
            }
            Item::Import { path } => {
                // Nikaia's `std` is not mapped onto Rust's yet; saying so is
                // better than emitting a `use` that cannot resolve.
                let path = path
                    .iter()
                    .map(|s| self.text(*s))
                    .collect::<Vec<_>>()
                    .join("::");
                out.push(&format!("// use {path} (Nikaia std is not mapped yet)\n"));
                Ok(())
            }
            other => Err(anyhow!("cannot emit item yet: {other:?}")),
        }
    }

    // --- Grammars ---

    fn grammar(&self, out: &mut Out, def: &GrammarDef) -> Result<()> {
        out.push("grammar! {\n");
        out.push(&format!("    grammar {} {{\n", self.text(def.name)));

        for rule in &def.rules {
            out.push("\n");
            out.from(&rule.span, |out| self.grammar_rule(out, rule))?;
        }

        out.push("    }\n");
        out.push("}\n");
        Ok(())
    }

    fn grammar_rule(&self, out: &mut Out, rule: &GrammarRule) -> Result<()> {
        if let Some(frame) = &rule.frame {
            out.push(&format!("        {}\n", frame_attribute(frame)));
        }

        let vis = if rule.is_public { "pub " } else { "" };
        let ret = match &rule.ret_type {
            Some(ty) => format!(" -> {}", self.ty(ty, Lifetimes::Named)),
            None => String::new(),
        };
        out.push(&format!(
            "        {vis}rule {}{ret} =\n",
            self.text(rule.name)
        ));

        for (i, alt) in rule.alts.iter().enumerate() {
            let separator = if i == 0 { "  " } else { "| " };

            match &alt.action {
                Some(action) => {
                    out.push(&format!("          {separator}"));
                    self.pattern(out, &alt.pattern)?;
                    out.push("\n            -> ");
                    self.block(out, action, 3)?;
                    out.push("\n");
                }
                None => {
                    // The one body that needs no action: a `par_fold` is the
                    // whole rule (ADR-009 D2), so the value of the rule is the
                    // value of the fold. The binding the backend wants is
                    // supplied here rather than demanded from the user.
                    if rule.ret_type.is_some() {
                        out.push(&format!("          {separator}folded:"));
                        self.pattern(out, &alt.pattern)?;
                        out.push("\n            -> { folded }\n");
                    } else {
                        out.push(&format!("          {separator}"));
                        self.pattern(out, &alt.pattern)?;
                        out.push("\n");
                    }
                }
            }
        }

        Ok(())
    }

    fn pattern(&self, out: &mut Out, pattern: &Spanned<Pattern>) -> Result<()> {
        out.from(&pattern.span, |out| match &pattern.node {
            Pattern::Seq(parts) => self.patterns(out, parts, " "),
            Pattern::Choice(parts) => self.patterns(out, parts, " | "),
            Pattern::Bind { name, pat } => {
                out.push(&format!("{}:", self.text(*name)));
                self.pattern(out, pat)
            }
            Pattern::Literal(text) => {
                out.push(&format!("\"{text}\""));
                Ok(())
            }
            Pattern::Ref { name, args } => {
                out.push(self.text(*name));
                if !args.is_empty() {
                    out.push("(");
                    self.patterns(out, args, ", ")?;
                    out.push(")");
                }
                Ok(())
            }
            Pattern::Repeat { pat, rep } => {
                // A sequence or a choice has to be grouped before a suffix can
                // apply to all of it rather than to its last element.
                let group = matches!(pat.node, Pattern::Seq(_) | Pattern::Choice(_));
                if group {
                    out.push("(");
                }
                self.pattern(out, pat)?;
                if group {
                    out.push(")");
                }
                out.push(&repeat_suffix(*rep));
                Ok(())
            }
            Pattern::Group(inner) => {
                out.push("(");
                self.pattern(out, inner)?;
                out.push(")");
                Ok(())
            }
            Pattern::Cut => {
                out.push("=>");
                Ok(())
            }
            Pattern::Fold(spec) => self.fold(out, spec),
        })
    }

    fn patterns(
        &self,
        out: &mut Out,
        patterns: &[Spanned<Pattern>],
        separator: &str,
    ) -> Result<()> {
        for (i, pattern) in patterns.iter().enumerate() {
            if i > 0 {
                out.push(separator);
            }
            self.pattern(out, pattern)?;
        }
        Ok(())
    }

    fn fold(&self, out: &mut Out, spec: &FoldSpec) -> Result<()> {
        let rule = self.text(spec.rule);
        let name = if spec.merge.is_some() {
            "par_fold"
        } else {
            "fold"
        };

        out.push(&format!("{name}({rule}, "));
        self.expr(out, &spec.init, 0)?;
        out.push(", ");
        self.expr(out, &spec.step, 0)?;
        if let Some(merge) = &spec.merge {
            out.push(", ");
            self.expr(out, merge, 0)?;
        }
        out.push(")");
        Ok(())
    }

    // --- Types ---

    fn ty(&self, ty: &Type, lifetimes: Lifetimes) -> String {
        let lifetime = match lifetimes {
            Lifetimes::Named => INPUT_LIFETIME,
            Lifetimes::Elided => "'_",
        };

        let mut out = String::new();

        // A view is a borrow of the parser's input, and that is where the
        // lifetime comes from - the source never writes one (ADR-008).
        if ty.is_view {
            match lifetimes {
                Lifetimes::Named => out.push_str(&format!("&{INPUT_LIFETIME} ")),
                Lifetimes::Elided => out.push('&'),
            }
        }
        out.push_str(self.text(ty.name));

        let mut params: Vec<String> = ty.generics.iter().map(|g| self.ty(g, lifetimes)).collect();
        // A struct that holds a view carries the input lifetime with it.
        if self.borrowing.contains(&ty.name) {
            params.insert(0, lifetime.to_string());
        }
        if !params.is_empty() {
            out.push_str(&format!("<{}>", params.join(", ")));
        }

        out
    }

    // --- Statements and expressions ---

    fn block(&self, out: &mut Out, block: &Block, depth: usize) -> Result<()> {
        if block.stmts.is_empty() {
            out.push("{ }");
            return Ok(());
        }

        // A one-statement block stays on its line. Most action blocks are one
        // expression, and a grammar reads better when its actions do not push
        // the pattern three lines apart. Whether it fits is decided by
        // rendering it, not by guessing from the shape.
        if block.stmts.len() == 1 {
            let only = block.stmts.first().expect("one statement");
            let rendered = Out::scratch(|scratch| {
                scratch.from(&only.span, |s| self.stmt(s, &only.node, depth, true))
            })?;
            if !rendered.buf.contains('\n') {
                out.push("{ ");
                out.append(rendered);
                out.push(" }");
                return Ok(());
            }
        }

        let pad = "    ".repeat(depth);
        let inner_pad = "    ".repeat(depth + 1);

        out.push("{\n");
        let last = block.stmts.len() - 1;
        for (i, stmt) in block.stmts.iter().enumerate() {
            out.push(&inner_pad);
            out.from(&stmt.span, |out| {
                self.stmt(out, &stmt.node, depth + 1, i == last)
            })?;
            out.push("\n");
        }
        out.push(&pad);
        out.push("}");
        Ok(())
    }

    /// `is_tail` marks the last statement of a block: Nikaia's blocks are
    /// expressions (Part I, 3.1), so the last expression is the value and keeps
    /// no semicolon.
    fn stmt(&self, out: &mut Out, stmt: &Stmt, depth: usize, is_tail: bool) -> Result<()> {
        match stmt {
            Stmt::Let {
                name,
                mutable,
                ty,
                value,
            } => {
                let mutable = if *mutable { "mut " } else { "" };
                let annotation = match ty {
                    Some(ty) => format!(": {}", self.ty(ty, Lifetimes::Elided)),
                    None => String::new(),
                };
                out.push(&format!("let {mutable}{}{annotation} = ", self.text(*name)));
                self.expr(out, value, depth)?;
                out.push(";");
            }
            Stmt::Assign { target, value } => {
                self.expr(out, target, depth)?;
                out.push(" = ");
                self.expr(out, value, depth)?;
                out.push(";");
            }
            Stmt::For {
                binding,
                iter,
                body,
            } => {
                out.push(&format!("for {} in ", self.text(*binding)));
                self.expr(out, iter, depth)?;
                out.push(" ");
                self.block(out, body, depth)?;
            }
            Stmt::Expr(expr) => {
                self.expr(out, expr, depth)?;
                if !is_tail {
                    out.push(";");
                }
            }
        }
        Ok(())
    }

    fn expr(&self, out: &mut Out, expr: &Expr, depth: usize) -> Result<()> {
        match expr {
            Expr::LitInt(v) => out.push(&v.to_string()),
            Expr::LitStr(s) => out.push(&format!("\"{s}\"")),
            Expr::LitBool(b) => out.push(&b.to_string()),
            Expr::Variable(name) => out.push(self.text(*name)),
            Expr::Path(segments) => out.push(
                &segments
                    .iter()
                    .map(|s| self.text(*s))
                    .collect::<Vec<_>>()
                    .join("::"),
            ),
            Expr::Block(block) => self.block(out, block, depth)?,
            Expr::If {
                cond,
                then_branch,
                else_branch,
            } => {
                out.push("if ");
                self.expr(out, cond, depth)?;
                out.push(" ");
                self.block(out, then_branch, depth)?;
                if let Some(block) = else_branch {
                    out.push(" else ");
                    self.block(out, block, depth)?;
                }
            }
            Expr::Call { func, args } => {
                self.expr(out, func, depth)?;
                out.push("(");
                self.args(out, args, depth)?;
                out.push(")");
            }
            Expr::MethodCall {
                receiver,
                method,
                args,
            } => {
                self.expr(out, receiver, depth)?;
                out.push(&format!(".{}(", self.text(*method)));
                self.args(out, args, depth)?;
                out.push(")");
            }
            Expr::Field { base, name } => {
                self.expr(out, base, depth)?;
                out.push(&format!(".{}", self.text(*name)));
            }
            Expr::StructLit { name, fields } => {
                out.push(&format!("{} {{ ", self.text(*name)));
                for (i, field) in fields.iter().enumerate() {
                    if i > 0 {
                        out.push(", ");
                    }
                    out.push(self.text(field.name));
                    if let Some(value) = &field.value {
                        out.push(": ");
                        self.expr(out, value, depth)?;
                    }
                }
                out.push(" }");
            }
            Expr::Closure { params, body } => {
                let params = params
                    .iter()
                    .map(|p| self.text(*p))
                    .collect::<Vec<_>>()
                    .join(", ");
                out.push(&format!("|{params}| "));
                self.block(out, body, depth)?;
            }
            Expr::Unary { op, expr } => {
                out.push(unary_op(*op));
                self.nested(out, expr, u8::MAX, depth)?;
            }
            Expr::Binary { op, lhs, rhs } => {
                // Parenthesised only where precedence needs it: the operators
                // mean the same in both languages, so `value * 10 + n` should
                // come out the way it went in.
                let here = precedence(*op);
                self.nested(out, lhs, here, depth)?;
                out.push(&format!(" {} ", binary_op(*op)));
                self.nested(out, rhs, here + 1, depth)?;
            }
            Expr::Try(inner) => {
                self.expr(out, inner, depth)?;
                out.push("?");
            }
            Expr::Spawn { .. } => {
                // Part II, 11.2. The runtime binding is the next roadmap line.
                return Err(anyhow!(
                    "`spawn` needs the runtime integration; not emitted yet"
                ));
            }
            Expr::DslFrom { grammar, input } => self.dsl_from(out, *grammar, input, depth)?,
            other => return Err(anyhow!("cannot emit expression yet: {other:?}")),
        }
        Ok(())
    }

    /// An operand of an operator, parenthesised only where it binds looser
    /// than the position it stands in.
    fn nested(&self, out: &mut Out, expr: &Expr, needs: u8, depth: usize) -> Result<()> {
        let parenthesise = match expr {
            Expr::Binary { op, .. } => precedence(*op) < needs,
            _ => false,
        };

        if parenthesise {
            out.push("(");
        }
        self.expr(out, expr, depth)?;
        if parenthesise {
            out.push(")");
        }
        Ok(())
    }

    fn args(&self, out: &mut Out, args: &[Expr], depth: usize) -> Result<()> {
        for (i, arg) in args.iter().enumerate() {
            if i > 0 {
                out.push(", ");
            }
            self.expr(out, arg, depth)?;
        }
        Ok(())
    }

    /// `dsl Measurements from data` - the whole of what a user writes to run a
    /// grammar. Everything the parallel form needs is already in the grammar
    /// (ADR-009): the frame says where the input may be cut, the `par_fold`
    /// says how the pieces combine. What is left is choosing the executor, and
    /// that is the profile's decision, not the program's.
    fn dsl_from(&self, out: &mut Out, grammar: Symbol, input: &Expr, depth: usize) -> Result<()> {
        let name = self.text(grammar);
        let def = self
            .grammars
            .get(&grammar)
            .ok_or_else(|| anyhow!("no grammar named `{name}` in this file"))?;

        let rule = entry_rule(def)
            .ok_or_else(|| anyhow!("grammar `{name}` has no `pub` rule to enter through"))?;
        let rule_name = self.text(rule.name);

        if par_fold_of(rule).is_some() {
            out.push(&format!("{name}::parse_{rule_name}_pieces("));
            self.expr(out, input, depth)?;
            out.push(&format!(
                ", ParseContext::<()>::default, {})?",
                self.profile.parallelism()
            ));
            return Ok(());
        }

        // A sequential entry rule: no pieces to cut, so the parser is driven
        // over the whole input once.
        let pad = "    ".repeat(depth + 1);
        let close = "    ".repeat(depth);
        out.push(&format!(
            "{{\n\
             {pad}use winnow::Parser;\n\
             {pad}let mut stream = winnow_grammar::ParseInput::<()> {{\n\
             {pad}    state: winnow_grammar::ParseContext::<()>::default(),\n\
             {pad}    input: winnow::stream::LocatingSlice::new("
        ));
        self.expr(out, input, depth)?;
        out.push(&format!(
            "),\n\
             {pad}}};\n\
             {pad}{name}::parse_{rule_name}().parse_next(&mut stream)?\n\
             {close}}}"
        ));
        Ok(())
    }
}

// --- Free helpers ---

/// ADR-009 D1: the attribute is keyed in both languages, and the lowering is
/// one-to-one. A bare `@frame` stays bare - the boundary is then the rule's
/// trailing literal, which the backend infers and checks.
fn frame_attribute(frame: &FrameAttr) -> String {
    let mut keys = Vec::new();
    if let Some(boundary) = &frame.boundary {
        keys.push(format!("boundary = \"{boundary}\""));
    }
    if frame.unchecked {
        keys.push("unchecked".to_string());
    }

    if keys.is_empty() {
        "#[frame]".to_string()
    } else {
        format!("#[frame({})]", keys.join(", "))
    }
}

fn repeat_suffix(rep: Repeat) -> String {
    match rep {
        Repeat::Star => "*".to_string(),
        Repeat::Plus => "+".to_string(),
        Repeat::Optional => "?".to_string(),
        Repeat::Exactly(n) => format!("{{{n}}}"),
        Repeat::AtLeast(n) => format!("{{{n},}}"),
        Repeat::Between(n, m) => format!("{{{n},{m}}}"),
    }
}

fn unary_op(op: UnaryOp) -> &'static str {
    match op {
        UnaryOp::Neg => "-",
        UnaryOp::Not => "!",
        UnaryOp::Ref => "&",
    }
}

/// Rust's binding strength, which Nikaia shares.
fn precedence(op: BinaryOp) -> u8 {
    match op {
        BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem => 10,
        BinaryOp::Add | BinaryOp::Sub => 9,
        BinaryOp::Eq | BinaryOp::Ne | BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => {
            7
        }
        BinaryOp::And => 4,
        BinaryOp::Or => 3,
    }
}

fn binary_op(op: BinaryOp) -> &'static str {
    match op {
        BinaryOp::Add => "+",
        BinaryOp::Sub => "-",
        BinaryOp::Mul => "*",
        BinaryOp::Div => "/",
        BinaryOp::Rem => "%",
        BinaryOp::Eq => "==",
        BinaryOp::Ne => "!=",
        BinaryOp::Lt => "<",
        BinaryOp::Le => "<=",
        BinaryOp::Gt => ">",
        BinaryOp::Ge => ">=",
        BinaryOp::And => "&&",
        BinaryOp::Or => "||",
    }
}

/// The rule a `dsl … from …` enters through: the first public one.
fn entry_rule(def: &GrammarDef) -> Option<&GrammarRule> {
    def.rules
        .iter()
        .find(|r| r.is_public && par_fold_of(r).is_some())
        .or_else(|| def.rules.iter().find(|r| r.is_public))
}

/// The `par_fold` a rule *is*, if it is one. A fold with a merge is the only
/// body the backend generates a piece driver for.
fn par_fold_of(rule: &GrammarRule) -> Option<&FoldSpec> {
    rule.alts.iter().find_map(|alt| match &alt.pattern.node {
        Pattern::Fold(spec) if spec.parallel => Some(&**spec),
        Pattern::Bind { pat, .. } => match &pat.node {
            Pattern::Fold(spec) if spec.parallel => Some(&**spec),
            _ => None,
        },
        _ => None,
    })
}

/// Which structs hold a view into the parser's input.
///
/// Part II, 10.6: a view is a slice of the input, so a struct holding one is
/// tied to the input as well - transitively, which is why this is a fixpoint
/// and not one pass.
fn borrowing_structs(parsed: &Parsed) -> HashSet<Symbol> {
    let mut fields_of: HashMap<Symbol, Vec<&Type>> = HashMap::new();
    let mut borrowing = HashSet::new();

    for item in &parsed.program.items {
        if let Item::Struct { name, fields, .. } = &item.node {
            let types: Vec<&Type> = fields.iter().map(|f| &f.ty).collect();
            if types.iter().any(|t| holds_view(t)) {
                borrowing.insert(*name);
            }
            fields_of.insert(*name, types);
        }
    }

    loop {
        let mut changed = false;
        for (name, types) in &fields_of {
            if borrowing.contains(name) {
                continue;
            }
            if types.iter().any(|t| names_borrowing(t, &borrowing)) {
                borrowing.insert(*name);
                changed = true;
            }
        }
        if !changed {
            return borrowing;
        }
    }
}

fn holds_view(ty: &Type) -> bool {
    ty.is_view || ty.generics.iter().any(holds_view)
}

fn names_borrowing(ty: &Type, borrowing: &HashSet<Symbol>) -> bool {
    borrowing.contains(&ty.name) || ty.generics.iter().any(|g| names_borrowing(g, borrowing))
}

/// Walk every expression in a block, including the ones inside statements.
fn visit_block(block: &Block, f: &mut impl FnMut(&Expr)) {
    for stmt in &block.stmts {
        match &stmt.node {
            Stmt::Let { value, .. } => visit_expr(value, f),
            Stmt::Assign { target, value } => {
                visit_expr(target, f);
                visit_expr(value, f);
            }
            Stmt::For { iter, body, .. } => {
                visit_expr(iter, f);
                visit_block(body, f);
            }
            Stmt::Expr(expr) => visit_expr(expr, f),
        }
    }
}

fn visit_expr(expr: &Expr, f: &mut impl FnMut(&Expr)) {
    f(expr);
    match expr {
        Expr::Block(block) => visit_block(block, f),
        Expr::If {
            cond,
            then_branch,
            else_branch,
        } => {
            visit_expr(cond, f);
            visit_block(then_branch, f);
            if let Some(block) = else_branch {
                visit_block(block, f);
            }
        }
        Expr::Call { func, args } => {
            visit_expr(func, f);
            args.iter().for_each(|a| visit_expr(a, f));
        }
        Expr::MethodCall { receiver, args, .. } => {
            visit_expr(receiver, f);
            args.iter().for_each(|a| visit_expr(a, f));
        }
        Expr::Field { base, .. } => visit_expr(base, f),
        Expr::StructLit { fields, .. } => fields
            .iter()
            .filter_map(|field| field.value.as_ref())
            .for_each(|value| visit_expr(value, f)),
        Expr::Closure { body, .. } => visit_block(body, f),
        Expr::Unary { expr, .. } => visit_expr(expr, f),
        Expr::Binary { lhs, rhs, .. } => {
            visit_expr(lhs, f);
            visit_expr(rhs, f);
        }
        Expr::Try(inner) => visit_expr(inner, f),
        Expr::Spawn { body, .. } => visit_expr(body, f),
        Expr::DslFrom { input, .. } => visit_expr(input, f),
        _ => {}
    }
}
