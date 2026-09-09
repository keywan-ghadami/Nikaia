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
    BinaryOp, Block, Expr, FnArg, FoldSpec, FrameAttr, GrammarDef, GrammarRule, Item, Pattern,
    Receiver, Repeat, Span, Spanned, Stmt, Type, UnaryOp,
};
use crate::parser::{parse_expression, Parsed};

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

/// How a view's lifetime is spelled here. The source never says either way
/// (ADR-008); the position does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Lifetimes {
    /// What a `&` becomes.
    reference: &'static str,
    /// What a type that holds a view is parameterised with.
    params: &'static str,
}

impl Lifetimes {
    /// Inside the grammar module and on the structs it builds, where `'a` is
    /// the input's lifetime and has to be named to tie the two together.
    const NAMED: Lifetimes = Lifetimes {
        reference: "&'a ",
        params: "'a",
    };
    /// In a free function's signature, which declares no lifetime of its own.
    const ELIDED: Lifetimes = Lifetimes {
        reference: "&",
        params: "'_",
    };
    /// In a method of an `impl` that does declare `'a`: the reference itself is
    /// still elided, because a borrow of the subject need not live as long as
    /// what the subject points into.
    const INNER: Lifetimes = Lifetimes {
        reference: "&",
        params: "'a",
    };
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
    /// Declared struct names: `Stats(x)` is a call to a constructor, and only
    /// the declarations say which names are types.
    structs: HashSet<Symbol>,
    /// What the program's `impl` blocks declare, which is what makes the fold
    /// adapter of D2 a lookup rather than a guess.
    methods: HashMap<(Symbol, Symbol), Method>,
    /// The same, by method name alone, for the places where the receiver's type
    /// is not written down. `None` marks a name two impls disagree about - and
    /// an ambiguous name is left alone rather than resolved by preference.
    by_name: HashMap<Symbol, Option<Method>>,
    /// Whether the program imports anything from `std`.
    uses_std: bool,
}

/// What an `impl` says about a method. Enough to adapt a `&mut self` method to
/// the monoid a `par_fold` needs, and never more than the declaration states.
#[derive(Debug, Clone)]
struct Method {
    receiver: Option<Receiver>,
    returns_value: bool,
    args: Vec<FnArg>,
}

impl Method {
    /// A method that mutates its subject and yields nothing - the shape that
    /// has to be threaded to become a fold's step or merge.
    fn mutates_in_place(&self) -> bool {
        !self.returns_value && self.receiver.is_some_and(|r| r.is_ref && r.is_mut)
    }
}

/// What surrounds the statements being emitted.
#[derive(Debug, Clone, Copy)]
struct Flow {
    /// Kap 7.1: the enclosing function is `throws`, so a `return` carries `Ok`.
    throws: bool,
}

impl Flow {
    const PLAIN: Flow = Flow { throws: false };
}

/// Whether a `dsl … from …` hands its failure to the enclosing function or to
/// a `catch` beside it. Everywhere else the parse propagates - `catch` is the
/// one place that wants the `Result` itself, and asking for it there is the
/// whole difference (Kap 7.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Propagate {
    Yes,
    No,
}

impl<'p> Emitter<'p> {
    fn new(parsed: &'p Parsed, profile: Profile) -> Self {
        let mut grammars = HashMap::new();
        let mut structs = HashSet::new();
        let mut methods = HashMap::new();
        let mut by_name: HashMap<Symbol, Option<Method>> = HashMap::new();
        let mut uses_std = false;

        for item in &parsed.program.items {
            match &item.node {
                Item::Grammar(def) => {
                    grammars.insert(def.name, def);
                }
                Item::Struct { name, .. } => {
                    structs.insert(*name);
                }
                Item::Import { .. } => uses_std = true,
                Item::Impl {
                    target,
                    methods: body,
                } => {
                    for method in body {
                        let Item::Fn {
                            name: Some(name),
                            receiver,
                            args,
                            ret_type,
                            ..
                        } = &method.node
                        else {
                            continue;
                        };

                        let info = Method {
                            receiver: *receiver,
                            returns_value: ret_type.is_some(),
                            args: args.clone(),
                        };
                        methods.insert((target.name, *name), info.clone());
                        by_name
                            .entry(*name)
                            .and_modify(|known| {
                                // Two impls, one name: nothing here can tell
                                // them apart, so neither is used.
                                if known.is_some() {
                                    *known = None;
                                }
                            })
                            .or_insert(Some(info));
                    }
                }
                _ => {}
            }
        }

        Self {
            parsed,
            profile,
            borrowing: borrowing_structs(parsed),
            grammars,
            structs,
            methods,
            by_name,
            uses_std,
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
        if self.uses_std {
            // Nikaia's `std` is a crate rather than a table in this file, so
            // `fs::map`, `cli::args` and `HashMap` resolve as written and what
            // they mean is code someone can read.
            // `pub use`, because the grammar module the backend generates
            // reaches these names through a glob of its own, and a private
            // import is not re-exported into one.
            out.push("pub use nikaia_std::prelude::*;\n");
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
                        self.ty(&field.ty, Lifetimes::NAMED)
                    ));
                }
                out.push("}\n");
                Ok(())
            }
            Item::Fn { .. } => self.function(out, item, 0, Lifetimes::ELIDED),
            Item::Impl { target, methods } => {
                // A type that holds a view carries the input lifetime, and the
                // impl has to declare the lifetime its methods are written with.
                let borrows = self.borrowing.contains(&target.name);
                let params = if borrows {
                    format!("<{INPUT_LIFETIME}>")
                } else {
                    String::new()
                };
                let lifetimes = if borrows {
                    Lifetimes::INNER
                } else {
                    Lifetimes::ELIDED
                };

                out.push(&format!(
                    "impl{params} {}{params} {{\n",
                    self.text(target.name)
                ));
                for method in methods {
                    out.from(&method.span, |out| {
                        out.push("    ");
                        self.function(out, &method.node, 1, lifetimes)
                    })?;
                }
                out.push("}\n");
                Ok(())
            }
            Item::Import { path } => {
                // The names come in through `nikaia_std::prelude`, emitted once
                // in the preamble; the import itself is kept as a comment so the
                // generated file still says where they were asked for.
                let path = path
                    .iter()
                    .map(|s| self.text(*s))
                    .collect::<Vec<_>>()
                    .join("::");
                out.push(&format!("// use {path}\n"));
                Ok(())
            }
            other => Err(anyhow!("cannot emit item yet: {other:?}")),
        }
    }

    /// A function or a method. Kap 4.2: a `pub fn` with no name is the
    /// anonymous constructor, called as `Type(…)`; Rust has no such thing, so
    /// it is emitted as `new` and the call sites follow.
    fn function(
        &self,
        out: &mut Out,
        item: &Item,
        depth: usize,
        lifetimes: Lifetimes,
    ) -> Result<()> {
        let Item::Fn {
            name,
            receiver,
            args,
            ret_type,
            body,
            is_sync,
            is_public,
            throws,
            ..
        } = item
        else {
            return Err(anyhow!("not a function"));
        };

        let pad = "    ".repeat(depth);
        if *is_sync {
            out.push(&format!(
                "// sync (Part II, 12.1): pure CPU, cannot pause. Not checked yet.\n{pad}"
            ));
        }

        let mut params = Vec::new();
        if let Some(receiver) = receiver {
            params.push(
                match (receiver.is_ref, receiver.is_mut) {
                    (true, true) => "&mut self",
                    (true, false) => "&self",
                    _ => "self",
                }
                .to_string(),
            );
        }
        params.extend(
            args.iter()
                .map(|a| format!("{}: {}", self.text(a.name), self.ty(&a.ty, lifetimes))),
        );

        // Kap 7.1: `throws` becomes a `Result` in the emitted Rust, over
        // `Box<dyn Error>` because Nikaia's own error types are not lowered
        // yet - the `?` the DSL driver needs works against it, and every error
        // keeps its own type behind it.
        let returned = match ret_type {
            Some(ty) => self.ty(ty, lifetimes),
            None => "()".to_string(),
        };
        let ret = if *throws {
            format!(" -> Result<{returned}, Box<dyn std::error::Error>>")
        } else if ret_type.is_some() {
            format!(" -> {returned}")
        } else {
            String::new()
        };

        let vis = if *is_public { "pub " } else { "" };
        let name = match name {
            Some(name) => self.text(*name).to_string(),
            None => "new".to_string(),
        };

        out.push(&format!("{vis}fn {name}({}){ret} ", params.join(", ")));
        self.function_body(out, body, depth, *throws, ret_type.is_some())?;
        out.push("\n");
        Ok(())
    }

    /// The body of a function, which differs from any other block in one way:
    /// a `throws` function has to hand back an `Ok`.
    fn function_body(
        &self,
        out: &mut Out,
        body: &Block,
        depth: usize,
        throws: bool,
        returns_value: bool,
    ) -> Result<()> {
        let flow = Flow { throws };

        if !throws {
            return self.block(out, body, depth, flow, returns_value);
        }

        let pad = "    ".repeat(depth);
        let inner_pad = "    ".repeat(depth + 1);

        out.push("{\n");
        let last = body.stmts.len().saturating_sub(1);
        for (i, stmt) in body.stmts.iter().enumerate() {
            out.push(&inner_pad);
            // A value-returning `throws` function ends in its value; one that
            // returns nothing ends in the `Ok(())` below, so its last statement
            // is a statement like any other.
            let is_tail = returns_value && i == last;
            out.from(&stmt.span, |out| {
                if is_tail {
                    out.push("Ok(");
                }
                self.stmt(out, &stmt.node, depth + 1, is_tail, flow)?;
                if is_tail {
                    out.push(")");
                }
                Ok(())
            })?;
            out.push("\n");
        }
        if !returns_value {
            out.push(&format!("{inner_pad}Ok(())\n"));
        }
        out.push(&pad);
        out.push("}");
        Ok(())
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
            Some(ty) => format!(" -> {}", self.ty(ty, Lifetimes::NAMED)),
            None => String::new(),
        };
        // The label sits between the return type and the `=`, in both
        // languages: `rule expr -> Expr # "expression" = …`.
        let label = match &rule.label {
            Some(text) => format!(" # {text:?}"),
            None => String::new(),
        };
        out.push(&format!(
            "        {vis}rule {}{ret}{label} =\n",
            self.text(rule.name)
        ));

        for (i, alt) in rule.alts.iter().enumerate() {
            let separator = if i == 0 { "  " } else { "| " };

            match &alt.action {
                Some(action) => {
                    out.push(&format!("          {separator}"));
                    self.pattern(out, &alt.pattern)?;
                    out.push("\n            -> ");
                    self.block(out, action, 3, Flow::PLAIN, true)?;
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
            Pattern::Ref {
                name,
                generics,
                args,
            } => {
                out.push(self.text(*name));
                // `dec[i32](…)` -> `dec::<i32>(…)`? No: the backend's builtin
                // takes its type between angles, not after a turbofish, and a
                // grammar body is the backend's syntax once it is emitted.
                if !generics.is_empty() {
                    let params: Vec<String> = generics
                        .iter()
                        .map(|g| self.ty(g, Lifetimes::NAMED))
                        .collect();
                    out.push(&format!("<{}>", params.join(", ")));
                }
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
        self.expr(out, &spec.init, 0, Flow::PLAIN)?;
        out.push(", ");
        self.fold_step(out, &spec.step)?;
        if let Some(merge) = &spec.merge {
            out.push(", ");
            self.fold_merge(out, merge)?;
        }
        out.push(")");
        Ok(())
    }

    /// A fold's step, adapted where the program's own declarations say it has
    /// to be.
    ///
    /// ADR-011 D2 refused to infer this from the shape of the body, and was
    /// right to: `acc.record(m)` and `acc.merged(m)` look identical. What
    /// changed is that `impl` blocks are lowered now, so this is a lookup in
    /// what the program declares - `record` takes `&mut self` and returns
    /// nothing, so the accumulator is threaded; a method that returns a new
    /// accumulator is left exactly as written.
    fn fold_step(&self, out: &mut Out, step: &Expr) -> Result<()> {
        if let Expr::Closure {
            params,
            implicit: false,
            body,
        } = step
        {
            if let [accumulator, item] = params.as_slice() {
                if let Some(Stmt::Expr(Expr::MethodCall {
                    receiver, method, ..
                })) = body.stmts.last().map(|s| &s.node)
                {
                    let on_accumulator =
                        matches!(&**receiver, Expr::Variable(name) if name == accumulator);
                    let mutates = self
                        .by_name
                        .get(method)
                        .and_then(|m| m.as_ref())
                        .is_some_and(Method::mutates_in_place);

                    if on_accumulator && mutates {
                        let accumulator = self.text(*accumulator);
                        out.push(&format!("|mut {accumulator}, {}| {{ ", self.text(*item)));
                        for stmt in &body.stmts {
                            self.stmt(out, &stmt.node, 0, false, Flow::PLAIN)?;
                            out.push(" ");
                        }
                        out.push(&format!("{accumulator} }}"));
                        return Ok(());
                    }
                }
            }
        }

        self.expr(out, step, 0, Flow::PLAIN)
    }

    /// A fold's merge. `Summary::merge` names a method, and the `impl` says
    /// whether it mutates its subject and whether it takes the other by view.
    fn fold_merge(&self, out: &mut Out, merge: &Expr) -> Result<()> {
        if let Expr::Path(segments) = merge {
            if let [owner, name] = segments.as_slice() {
                if let Some(method) = self.methods.get(&(*owner, *name)) {
                    if method.mutates_in_place() && method.args.len() == 1 {
                        let by_view = if method.args[0].ty.is_view { "&" } else { "" };
                        // The accumulator's type is annotated because the path
                        // names it: without that, the backend's `let merge = …`
                        // leaves the closure with nothing to infer from.
                        let owner = self.ty(
                            &Type {
                                name: *owner,
                                generics: Vec::new(),
                                is_view: false,
                                is_tuple: false,
                            },
                            Lifetimes::NAMED,
                        );
                        out.push(&format!(
                            "|mut a: {owner}, b| {{ a.{}({by_view}b); a }}",
                            self.text(*name)
                        ));
                        return Ok(());
                    }
                }
            }
        }

        self.expr(out, merge, 0, Flow::PLAIN)
    }

    /// Kap 5.2: the implicit arguments are `a`, `b`, `c`, and a lambda takes as
    /// many of them as its body reaches for.
    fn implicit_params(&self, body: &Block) -> Vec<String> {
        const NAMES: [&str; 3] = ["a", "b", "c"];

        let mut used = [false; 3];
        visit_block(body, &mut |expr| {
            if let Expr::Variable(name) = expr {
                if let Some(i) = NAMES.iter().position(|n| *n == self.text(*name)) {
                    used[i] = true;
                }
            }
        });

        let count = used.iter().rposition(|u| *u).map_or(0, |i| i + 1);
        NAMES[..count].iter().map(|n| n.to_string()).collect()
    }

    // --- Types ---

    fn ty(&self, ty: &Type, lifetimes: Lifetimes) -> String {
        let mut out = String::new();

        // `(A, B)` in both languages, and the parts are the arguments.
        if ty.is_tuple {
            let parts: Vec<String> = ty.generics.iter().map(|g| self.ty(g, lifetimes)).collect();
            return format!("({})", parts.join(", "));
        }

        // A view is a borrow of the parser's input, and that is where the
        // lifetime comes from - the source never writes one (ADR-008).
        if ty.is_view {
            out.push_str(lifetimes.reference);
        }
        out.push_str(self.text(ty.name));

        let mut params: Vec<String> = ty.generics.iter().map(|g| self.ty(g, lifetimes)).collect();
        // A struct that holds a view carries the input lifetime with it.
        if self.borrowing.contains(&ty.name) {
            params.insert(0, lifetimes.params.to_string());
        }
        if !params.is_empty() {
            out.push_str(&format!("<{}>", params.join(", ")));
        }

        out
    }

    // --- Statements and expressions ---

    /// `tail` says whether the last statement is the block's value. It is for
    /// an expression block (Part I, 3.1) and for a function that returns
    /// something; a function with no return type has no value to leave behind,
    /// so its last statement is a statement like any other.
    fn block(
        &self,
        out: &mut Out,
        block: &Block,
        depth: usize,
        flow: Flow,
        tail: bool,
    ) -> Result<()> {
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
                scratch.from(&only.span, |s| self.stmt(s, &only.node, depth, tail, flow))
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
                self.stmt(out, &stmt.node, depth + 1, tail && i == last, flow)
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
    fn stmt(
        &self,
        out: &mut Out,
        stmt: &Stmt,
        depth: usize,
        is_tail: bool,
        flow: Flow,
    ) -> Result<()> {
        match stmt {
            Stmt::Let {
                name,
                mutable,
                ty,
                value,
            } => {
                let mutable = if *mutable { "mut " } else { "" };
                let annotation = match ty {
                    Some(ty) => format!(": {}", self.ty(ty, Lifetimes::ELIDED)),
                    None => String::new(),
                };
                out.push(&format!("let {mutable}{}{annotation} = ", self.text(*name)));
                self.expr(out, value, depth, flow)?;
                out.push(";");
            }
            Stmt::Assign { target, op, value } => {
                self.expr(out, target, depth, flow)?;
                match op {
                    Some(op) => out.push(&format!(" {}= ", binary_op(*op))),
                    None => out.push(" = "),
                }
                self.expr(out, value, depth, flow)?;
                out.push(";");
            }
            Stmt::For {
                bindings,
                iter,
                body,
            } => {
                let names = bindings
                    .iter()
                    .map(|b| self.text(*b))
                    .collect::<Vec<_>>()
                    .join(", ");
                if bindings.len() > 1 {
                    out.push(&format!("for ({names}) in "));
                } else {
                    out.push(&format!("for {names} in "));
                }
                self.expr(out, iter, depth, flow)?;
                out.push(" ");
                self.block(out, body, depth, flow, false)?;
            }
            // A `return` that is the last statement is the block's value, and
            // is written as one: `fn f() -> T { return x }` is `{ x }`. In a
            // `throws` function the caller of this wraps the tail in `Ok`, so
            // the value is emitted bare here in both cases.
            Stmt::Return(Some(value)) if is_tail => {
                self.expr(out, value, depth, flow)?;
            }
            Stmt::Return(value) => {
                // Kap 7.1: a `throws` function returns a `Result`, so what the
                // source hands back is what goes inside the `Ok`.
                match (value, flow.throws) {
                    (Some(value), true) => {
                        out.push("return Ok(");
                        self.expr(out, value, depth, flow)?;
                        out.push(");");
                    }
                    (Some(value), false) => {
                        out.push("return ");
                        self.expr(out, value, depth, flow)?;
                        out.push(";");
                    }
                    (None, true) => out.push("return Ok(());"),
                    (None, false) => out.push("return;"),
                }
            }
            Stmt::Expr(expr) => {
                self.expr(out, expr, depth, flow)?;
                // `if x { … };` is legal and noisy; a block-shaped statement
                // ends where its brace does.
                let block_shaped = matches!(expr, Expr::If { .. } | Expr::Block(_));
                if !is_tail && !block_shaped {
                    out.push(";");
                }
            }
        }
        Ok(())
    }

    fn expr(&self, out: &mut Out, expr: &Expr, depth: usize, flow: Flow) -> Result<()> {
        match expr {
            Expr::LitInt(v) => out.push(&v.to_string()),
            Expr::LitFloat(v) => out.push(v),
            Expr::LitStr(_) => self.string(out, expr, depth, flow)?,
            Expr::LitBool(b) => out.push(&b.to_string()),
            Expr::Variable(name) => out.push(self.text(*name)),
            Expr::Path(segments) => out.push(
                &segments
                    .iter()
                    .map(|s| self.text(*s))
                    .collect::<Vec<_>>()
                    .join("::"),
            ),
            Expr::Block(block) => self.block(out, block, depth, flow, true)?,
            Expr::If {
                cond,
                then_branch,
                else_branch,
            } => {
                out.push("if ");
                self.expr(out, cond, depth, flow)?;
                out.push(" ");
                self.block(out, then_branch, depth, flow, true)?;
                if let Some(block) = else_branch {
                    out.push(" else ");
                    self.block(out, block, depth, flow, true)?;
                }
            }
            Expr::Call { func, args } => self.call(out, func, args, depth, flow)?,
            Expr::MethodCall {
                receiver,
                method,
                args,
            } => {
                self.expr(out, receiver, depth, flow)?;
                out.push(&format!(".{}", self.text(*method)));
                // Nikaia's `collect` builds a List; Rust's needs to be told
                // what to build, and with no types here that is `Vec<_>`.
                if self.text(*method) == "collect" && args.is_empty() {
                    out.push("::<Vec<_>>");
                }
                out.push("(");
                self.args(out, args, depth, flow)?;
                out.push(")");
            }
            Expr::Tuple(parts) => {
                out.push("(");
                for (i, part) in parts.iter().enumerate() {
                    if i > 0 {
                        out.push(", ");
                    }
                    self.expr(out, part, depth, flow)?;
                }
                out.push(")");
            }
            Expr::Field { base, name } => {
                self.expr(out, base, depth, flow)?;
                out.push(&format!(".{}", self.text(*name)));
            }
            Expr::Index { base, index } => {
                self.expr(out, base, depth, flow)?;
                out.push("[");
                self.expr(out, index, depth, flow)?;
                out.push("]");
            }
            Expr::Cast { expr, ty } => {
                self.nested(out, expr, u8::MAX, depth, flow)?;
                out.push(&format!(" as {}", self.ty(ty, Lifetimes::ELIDED)));
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
                        self.expr(out, value, depth, flow)?;
                    }
                }
                out.push(" }");
            }
            Expr::Closure {
                params,
                implicit,
                body,
            } => {
                let params = if *implicit {
                    self.implicit_params(body)
                } else {
                    params.iter().map(|p| self.text(*p).to_string()).collect()
                };
                out.push(&format!("|{}| ", params.join(", ")));
                // A lambda's `return` leaves the lambda, not the function
                // around it, so it never carries the enclosing `Ok`.
                self.block(out, body, depth, Flow::PLAIN, true)?;
            }
            Expr::Unary { op, expr } => {
                out.push(unary_op(*op));
                self.nested(out, expr, u8::MAX, depth, flow)?;
            }
            Expr::Binary { op, lhs, rhs } => {
                // Parenthesised only where precedence needs it: the operators
                // mean the same in both languages, so `value * 10 + n` should
                // come out the way it went in.
                let here = precedence(*op);
                self.nested(out, lhs, here, depth, flow)?;
                out.push(&format!(" {} ", binary_op(*op)));
                self.nested(out, rhs, here + 1, depth, flow)?;
            }
            Expr::Coalesce { value, fallback } => {
                // Kap 3.5. `into()` because the fallback is written as the
                // value it stands for, not as the type the option holds.
                self.expr(out, value, depth, flow)?;
                out.push(".unwrap_or_else(|| ");
                self.expr(out, fallback, depth, flow)?;
                out.push(".into())");
            }
            Expr::TryCatch { expr, handler } => {
                // Kap 7.1: the handler sees the error as `error`.
                out.push("match ");
                // `catch` needs the `Result`, not the value: a `dsl … from …`
                // propagates on its own everywhere else, and here the handler
                // is what handles it.
                match expr.as_ref() {
                    Expr::DslFrom { grammar, input } => {
                        self.dsl_from(out, *grammar, input, depth, flow, Propagate::No)?
                    }
                    _ => self.expr(out, expr, depth, flow)?,
                }
                let pad = "    ".repeat(depth + 1);
                let close = "    ".repeat(depth);
                out.push(&format!(
                    " {{\n{pad}Ok(value) => value,\n{pad}Err(error) => "
                ));
                self.block(out, handler, depth + 1, flow, true)?;
                out.push(&format!(",\n{close}}}"));
            }
            Expr::Try(inner) => {
                self.expr(out, inner, depth, flow)?;
                out.push("?");
            }
            Expr::Spawn { .. } => {
                // Part II, 11.2. The runtime binding is the next roadmap line.
                return Err(anyhow!(
                    "`spawn` needs the runtime integration; not emitted yet"
                ));
            }
            Expr::DslFrom { grammar, input } => {
                self.dsl_from(out, *grammar, input, depth, flow, Propagate::Yes)?
            }
            other => return Err(anyhow!("cannot emit expression yet: {other:?}")),
        }
        Ok(())
    }

    /// A call. `Stats(temp)` is the anonymous constructor of Kap 4.2 - Rust has
    /// no such thing, so the declaration decides: a name that is a struct is a
    /// call to the `new` its `impl` provides.
    fn call(
        &self,
        out: &mut Out,
        func: &Expr,
        args: &[Expr],
        depth: usize,
        flow: Flow,
    ) -> Result<()> {
        if let Expr::Variable(name) = func {
            let text = self.text(*name);

            // `println` and `eprintln` are macros in Rust, and their argument
            // is an interpolated string, which is a format string already.
            if matches!(text, "println" | "eprintln") {
                if let [Expr::LitStr(literal)] = args {
                    out.push(&format!("{text}!("));
                    self.format_string(out, literal, depth, flow)?;
                    out.push(")");
                    return Ok(());
                }
                out.push(&format!("{text}!(\"{{}}\", "));
                self.args(out, args, depth, flow)?;
                out.push(")");
                return Ok(());
            }

            if self.structs.contains(name) {
                out.push(&format!("{text}::new("));
                self.args(out, args, depth, flow)?;
                out.push(")");
                return Ok(());
            }
        }

        self.expr(out, func, depth, flow)?;
        out.push("(");
        self.args(out, args, depth, flow)?;
        out.push(")");
        Ok(())
    }

    /// A string literal, which is a format string when it has holes in it.
    fn string(&self, out: &mut Out, expr: &Expr, depth: usize, flow: Flow) -> Result<()> {
        let Expr::LitStr(literal) = expr else {
            return Err(anyhow!("not a string literal"));
        };

        if interpolation(literal)?.1.is_empty() {
            out.push(&format!("\"{literal}\""));
            return Ok(());
        }

        out.push("format!(");
        self.format_string(out, literal, depth, flow)?;
        out.push(")");
        Ok(())
    }

    /// The literal as a Rust format string, with its holes as arguments.
    fn format_string(&self, out: &mut Out, literal: &str, depth: usize, flow: Flow) -> Result<()> {
        let (format, holes) = interpolation(literal)?;
        out.push(&format!("\"{format}\""));

        for hole in holes {
            let expr = parse_expression(&self.parsed.interner, &hole)
                .map_err(|e| anyhow!("in the interpolated `{{{hole}}}`: {e}"))?;
            out.push(", ");
            self.expr(out, &expr, depth, flow)?;
        }
        Ok(())
    }

    /// An operand of an operator, parenthesised only where it binds looser
    /// than the position it stands in.
    fn nested(
        &self,
        out: &mut Out,
        expr: &Expr,
        needs: u8,
        depth: usize,
        flow: Flow,
    ) -> Result<()> {
        let parenthesise = match expr {
            Expr::Binary { op, .. } => precedence(*op) < needs,
            Expr::Cast { .. } => needs == u8::MAX,
            _ => false,
        };

        if parenthesise {
            out.push("(");
        }
        self.expr(out, expr, depth, flow)?;
        if parenthesise {
            out.push(")");
        }
        Ok(())
    }

    fn args(&self, out: &mut Out, args: &[Expr], depth: usize, flow: Flow) -> Result<()> {
        for (i, arg) in args.iter().enumerate() {
            if i > 0 {
                out.push(", ");
            }
            self.expr(out, arg, depth, flow)?;
        }
        Ok(())
    }

    /// `dsl Measurements from data` - the whole of what a user writes to run a
    /// grammar. Everything the parallel form needs is already in the grammar
    /// (ADR-009): the frame says where the input may be cut, the `par_fold`
    /// says how the pieces combine. What is left is choosing the executor, and
    /// that is the profile's decision, not the program's.
    fn dsl_from(
        &self,
        out: &mut Out,
        grammar: Symbol,
        input: &Expr,
        depth: usize,
        flow: Flow,
        propagate: Propagate,
    ) -> Result<()> {
        let question = match propagate {
            Propagate::Yes => "?",
            Propagate::No => "",
        };
        let name = self.text(grammar);
        let def = self
            .grammars
            .get(&grammar)
            .ok_or_else(|| anyhow!("no grammar named `{name}` in this file"))?;

        let rule = entry_rule(def)
            .ok_or_else(|| anyhow!("grammar `{name}` has no `pub` rule to enter through"))?;
        let rule_name = self.text(rule.name);

        let pad = "    ".repeat(depth + 1);
        let close = "    ".repeat(depth);

        // The input is bound before it is parsed, because it is needed twice:
        // once to parse and once to say *where* a failure was. A `ParseError`
        // knows the offset and not the text, so only `render` can turn "at
        // 1042" into "at line 37, column 9" - and a program that reports a
        // rejected file without saying which line is not much better than one
        // that panics. `&*` because the parser takes the text: a mapping, an
        // owned string and a view all reach it the same way, and none of them
        // has to be named here.
        if par_fold_of(rule).is_some() {
            out.push(&format!("{{\n{pad}let _source = &*"));
            self.expr(out, input, depth, flow)?;
            out.push(&format!(
                ";\n\
                 {pad}{name}::parse_{rule_name}_pieces(_source, \
                 &ParseContext::<()>::default(), {})\n\
                 {pad}    .map_err(|error| error.render(_source))\n\
                 {close}}}{question}",
                self.profile.parallelism()
            ));
            return Ok(());
        }

        // A sequential entry rule: no pieces to cut, so the parser is driven
        // over the whole input once.
        out.push(&format!(
            "{{\n{pad}use winnow::Parser;\n{pad}let _source = &*"
        ));
        self.expr(out, input, depth, flow)?;
        out.push(&format!(
            ";\n\
             {pad}let mut stream = winnow_grammar::ParseInput::<()> {{\n\
             {pad}    state: winnow_grammar::ParseContext::<()>::default(),\n\
             {pad}    input: winnow::stream::LocatingSlice::new(_source),\n\
             {pad}}};\n\
             {pad}{name}::parse_{rule_name}()\n\
             {pad}    .parse_next(&mut stream)\n\
             {pad}    .map_err(|error| error.render(_source))\n\
             {close}}}{question}"
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
            Stmt::Assign { target, value, .. } => {
                visit_expr(target, f);
                visit_expr(value, f);
            }
            Stmt::For { iter, body, .. } => {
                visit_expr(iter, f);
                visit_block(body, f);
            }
            Stmt::Return(value) => {
                if let Some(value) = value {
                    visit_expr(value, f);
                }
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
        Expr::Tuple(parts) => parts.iter().for_each(|p| visit_expr(p, f)),
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
        Expr::Index { base, index } => {
            visit_expr(base, f);
            visit_expr(index, f);
        }
        Expr::Cast { expr, .. } => visit_expr(expr, f),
        Expr::Coalesce { value, fallback } => {
            visit_expr(value, f);
            visit_expr(fallback, f);
        }
        Expr::TryCatch { expr, handler } => {
            visit_expr(expr, f);
            visit_block(handler, f);
        }
        Expr::Spawn { body, .. } => visit_expr(body, f),
        Expr::DslFrom { input, .. } => visit_expr(input, f),
        _ => {}
    }
}

/// Split an interpolated string into a Rust format string and its holes.
///
/// `"{a}={s.mean()}"` becomes `("{}={}", ["a", "s.mean()"])`. Braces are
/// doubled to be literal, as in every format string; the holes themselves are
/// Nikaia expressions and are parsed as such by the caller.
fn interpolation(literal: &str) -> Result<(String, Vec<String>)> {
    let mut format = String::new();
    let mut holes = Vec::new();
    let mut chars = literal.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '{' if chars.peek() == Some(&'{') => {
                chars.next();
                format.push_str("{{");
            }
            '}' if chars.peek() == Some(&'}') => {
                chars.next();
                format.push_str("}}");
            }
            '{' => {
                let mut hole = String::new();
                let mut spec = None;
                let mut depth = 1;
                let mut nesting = 0;

                for c in chars.by_ref() {
                    match c {
                        '{' => depth += 1,
                        '}' => {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                        }
                        '(' | '[' => nesting += 1,
                        ')' | ']' => nesting -= 1,
                        // The first colon that is not inside a call or an index
                        // separates the expression from how it is to be
                        // written, exactly as a format string does elsewhere.
                        // `Stats(min: 1)` keeps its colon, being inside parens.
                        ':' if nesting == 0 && spec.is_none() => {
                            spec = Some(String::new());
                            continue;
                        }
                        _ => {}
                    }
                    match &mut spec {
                        Some(spec) => spec.push(c),
                        None => hole.push(c),
                    }
                }
                if depth != 0 {
                    return Err(anyhow!("unclosed `{{` in \"{literal}\""));
                }

                holes.push(hole);
                match spec {
                    Some(spec) => format.push_str(&format!("{{:{spec}}}")),
                    None => format.push_str("{}"),
                }
            }
            '}' => {
                return Err(anyhow!(
                    "stray `}}` in \"{literal}\"; write `}}}}` for a brace"
                ))
            }
            _ => format.push(c),
        }
    }

    Ok((format, holes))
}
