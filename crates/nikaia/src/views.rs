// crates/nikaia/src/views.rs
//
// Where a naked view parameter's view ends up - `NK2302`.
//
// ## What this is for
//
// A view (`&str`, `&[u8]`, `&T`) is a slice of a buffer somebody else owns
// (Part I, 6.6). A struct that holds one says so in its own declaration, and the
// struct therefore *carries* the buffer it points into wherever it goes - which
// is what makes `examples/1brc.nika` free: `Reading` holds a `&str`, so a
// function that takes a `Reading` has been told which buffer the name came from.
//
// A parameter written `&str` on its own has been told nothing of the kind. It is
// a view of *some* buffer, for the duration of the call, and that is all it
// says. So a function that takes one and **stores it** - into a field of its
// subject, into a struct it hands back - is asking to keep something past the
// only moment it was given. Before this check the program reached the language
// below anyway and came back refused in the vocabulary of the generated file,
// which Part III C.1 calls a bug in this compiler rather than in the program.
//
// This module answers the question in Nikaia instead, and the answer points at
// the form that works.
//
// ## Which way it errs, and why
//
// **Fail-closed on "is it stored"**
// ([ADR-010](../../docs/specification/adr/adr-010.md) D1: an analysis that fails
// open is a vulnerability generator - the same polarity holds here, where failing
// open means emitting Rust that does not compile and breaking C.1). Where a
// carrier of the view is handed to a call on the subject, or on a field of the
// subject whose declared type holds a view, this module **cannot see** whether
// the callee keeps it: `insert` does and `contains_key` does not, and `std`'s
// ledger records `key: ?` for both (`crates/nikaia-std/std.contracts`). So it
// reports it as stored. That refuses `self.names.contains_key(name)`, which the
// language below accepts; the cost is named here rather than hidden.
//
// **But the domain is narrow on purpose, and that is not the same thing.** The
// only destinations considered are the ones whose buffer is known *not* to be
// the parameter's own:
//
//   1. a field reached from `self`, whose declared type holds a view;
//   2. a field of a struct or enum the function builds, where that field holds a
//      view;
//   3. the result, where the result holds a view and the language below would
//      tie it to something other than this parameter;
//   4. a task's body, which outlives the call by construction.
//
// Everything else is left alone, and deliberately: `examples/k-nucleotide.nika`
// writes views of `seq` into a `HashMap[&str, Tally]` it returns, and that is
// correct because the result *is* `seq`'s buffer - case 3 asks exactly that
// question before it reports anything. A local whose declared type holds a view
// is **not** a destination here, because deciding it needs to know which buffer
// each local views, which this compiler does not compute.
//
// ## Where the reach stops
//
// One forward pass, no reassignment tracking, names only. A name that ever
// carries the view keeps carrying it, and a view that reaches a field through a
// local is missed.

use std::collections::{BTreeSet, HashMap, HashSet};

use winnow_grammar::Symbol as Ident;

use crate::ast::{Block, Expr, Item, Span, Stmt, Type, VariantFields};
use crate::check::{Finding, Severity};
use crate::emit::{borrowing_structs, holds_view, names_borrowing};
use crate::parser::Parsed;

/// One naked view parameter, and the place its view ends up.
#[derive(Debug, Clone)]
pub struct Stored {
    /// The statement that stores it - what the caret goes under.
    pub span: Span,
    /// The parameter, as the source names it.
    pub param: String,
    /// The parameter's declared type, as the source writes it: `&str`.
    pub ty: String,
    /// The function or method it belongs to.
    pub function: String,
    /// The `impl` target, where the function is a method of one.
    pub subject: Option<String>,
    /// Where the view goes.
    pub into: Destination,
}

/// The kind of place a stored view reaches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Destination {
    /// A field whose declared type holds a view. `owner` declares it.
    Field {
        owner: String,
        field: String,
        ty: String,
    },
    /// The subject itself: a call on `self`, whose end is not visible here.
    Subject { owner: String },
    /// The result, and the result holds a view of a buffer that is not this
    /// parameter's.
    Result { ty: String },
    /// A task's body. It outlives the call whatever it does with the view.
    Task,
}

/// Every naked view parameter in this unit whose view is stored.
pub fn analyse(parsed: &Parsed) -> Vec<Stored> {
    let borrowing = borrowing_structs(parsed);
    let fields = fields_of(parsed);
    let mut found = Vec::new();

    for item in &parsed.program.items {
        match &item.node {
            Item::Fn { .. } => scan(parsed, &borrowing, &fields, None, &item.node, &mut found),
            Item::Impl {
                target, methods, ..
            } => {
                for method in methods {
                    scan(
                        parsed,
                        &borrowing,
                        &fields,
                        Some(target),
                        &method.node,
                        &mut found,
                    );
                }
            }
            _ => {}
        }
    }
    found.sort_by_key(|s| s.span.start);
    found
}

/// The refusals: every stored naked view.
pub fn check(parsed: &Parsed) -> Vec<Finding> {
    analyse(parsed).iter().map(finding).collect()
}

/// What one refusal says.
///
/// Part III C.2: the headline says what is wrong, the notes say why the compiler
/// believes it, and the help is the shape that works - written out rather than
/// described, because `examples/1brc.nika` already writes it and a reader who
/// has to invent it has been told half an answer.
fn finding(stored: &Stored) -> Finding {
    let Stored {
        param,
        ty,
        function,
        subject,
        into,
        ..
    } = stored;

    let where_it_goes = match into {
        Destination::Field { owner, field, .. } => format!("`{owner}.{field}`"),
        Destination::Subject { owner } => format!("`{owner}`, through a call on it"),
        Destination::Result { .. } => "the value this function hands back".to_string(),
        Destination::Task => "a task, which goes on running after the call".to_string(),
    };

    let why = match into {
        Destination::Field {
            owner,
            field,
            ty: field_ty,
        } => format!(
            "`{owner}.{field}` is `{field_ty}`, so what it keeps points into a buffer - and \
             `{param}: {ty}` does not say which buffer it views"
        ),
        Destination::Subject { owner } => format!(
            "a call on `{owner}` may keep what it is given, and nothing written down here says it \
             does not - so `{param}` is treated as kept"
        ),
        Destination::Result { ty: result } => format!(
            "the result is `{result}`, which holds a view, and this function names another buffer \
             besides `{param}`'s - so the result does not point into `{param}`'s"
        ),
        Destination::Task => {
            "a task goes on running after the call that spawned it (Part I, 8.3), \
             so anything it views has to outlive the call"
                .to_string()
        }
    };

    let of = match subject {
        Some(subject) => format!("`{subject}.{function}`"),
        None => format!("`{function}`"),
    };

    Finding {
        severity: Severity::Error,
        span: stored.span.clone(),
        code: "NK2302",
        message: format!(
            "{of} keeps `{param}` past this call, and `{param}: {ty}` does not say which buffer it \
             views"
        ),
        notes: vec![format!("it goes into {where_it_goes}"), why],
        help: Some(format!(
            "put the view in a struct and take the struct: the struct's declaration says which \
             buffer, the way `Reading` does in `examples/1brc.nika`:\n\
             \x20          @borrowed\n\
             \x20          struct Held {{ {param}: {ty} }}\n\
             \x20          …\n\
             \x20          fn {function}(…, held: Held) {{ … held.{param} … }}\n\
             \x20      or take a copy of the text with `.to_owned()`, which costs one allocation \
             and says so (Part I, 6.6)"
        )),
    }
}

/// Every struct's and enum's fields, by the name that declares them.
///
/// An enum's variants are flattened into it: for the one question this map is
/// asked - does this field hold a view - a variant's field is a field of the
/// enum.
fn fields_of(parsed: &Parsed) -> HashMap<Ident, Vec<(String, Type)>> {
    let mut out: HashMap<Ident, Vec<(String, Type)>> = HashMap::new();
    for item in &parsed.program.items {
        match &item.node {
            Item::Struct { name, fields, .. } => {
                out.insert(
                    *name,
                    fields
                        .iter()
                        .map(|f| (parsed.text(f.name).to_string(), f.ty.clone()))
                        .collect(),
                );
            }
            Item::Enum { name, variants, .. } => {
                let mut carried = Vec::new();
                for variant in variants {
                    match &variant.fields {
                        VariantFields::Unit => {}
                        VariantFields::Tuple(types) => {
                            for (i, ty) in types.iter().enumerate() {
                                carried.push((i.to_string(), ty.clone()));
                            }
                        }
                        VariantFields::Named(fields) => {
                            for field in fields {
                                carried
                                    .push((parsed.text(field.name).to_string(), field.ty.clone()));
                            }
                        }
                    }
                }
                out.insert(*name, carried);
            }
            _ => {}
        }
    }
    out
}

/// One function or method, asked about each of its naked view parameters.
fn scan(
    parsed: &Parsed,
    borrowing: &HashSet<Ident>,
    fields: &HashMap<Ident, Vec<(String, Type)>>,
    target: Option<&Type>,
    item: &Item,
    out: &mut Vec<Stored>,
) {
    let Item::Fn {
        name,
        receiver,
        args,
        config,
        ret_type,
        body,
        ..
    } = item
    else {
        return;
    };

    // A parameter and an option are the same thing here: both arrive from the
    // caller and both are written with a type (Part I, 5.1).
    let declared: Vec<(Ident, &Type)> = args
        .iter()
        .map(|a| (a.name, &a.ty))
        .chain(config.iter().map(|c| (c.name, &c.ty)))
        .collect();
    if !declared.iter().any(|(_, ty)| ty.is_view) {
        return;
    }

    // Which parameters name a buffer at all. The language below ties an elided
    // result to the subject where there is one, and otherwise to the single
    // input that has a buffer - so a result holding a view belongs to *this*
    // parameter only where it is the one and only source.
    let brings_buffer = |ty: &&Type| holds_view(ty) || names_borrowing(ty, borrowing);
    let sources: Vec<Ident> = declared
        .iter()
        .filter(|(_, ty)| brings_buffer(ty))
        .map(|(name, _)| *name)
        .collect();
    let subject_is_a_source = receiver.is_some_and(|r| r.is_ref);
    let result_holds_view = ret_type.as_ref().is_some_and(|ty| brings_buffer(&ty));

    let subject_name = target.map(|t| parsed.text(t.name).to_string());
    let function = match name {
        Some(name) => parsed.text(*name).to_string(),
        // Part I 4.2's anonymous constructor.
        None => "new".to_string(),
    };
    let empty: Vec<(String, Type)> = Vec::new();
    let subject_fields = target
        .and_then(|t| fields.get(&t.name))
        .unwrap_or(&empty)
        .as_slice();

    for (param, ty) in declared.iter().filter(|(_, ty)| ty.is_view) {
        // Whether the result is this parameter's own buffer. Where it is,
        // handing the view back is exactly what the signature already says.
        let result_is_ours = !subject_is_a_source && sources.len() == 1 && sources[0] == *param;
        let mut scanner = Scanner {
            parsed,
            borrowing,
            fields,
            subject: target,
            subject_fields,
            result: if result_holds_view && !result_is_ours {
                ret_type.as_ref()
            } else {
                None
            },
            carriers: BTreeSet::from([parsed.text(*param).to_string()]),
            found: Vec::new(),
        };
        scanner.block(body, true);

        // **One refusal per parameter.** Every store of the same view is the
        // same mistake in the signature, and a reader fixes it once; the place
        // reported is the one that says the most about where the view went
        // (see [`rank`]).
        let Some((span, into)) = scanner
            .found
            .into_iter()
            .min_by_key(|(span, into)| (rank(into), span.start))
        else {
            continue;
        };
        out.push(Stored {
            span,
            param: parsed.text(*param).to_string(),
            ty: write_type(parsed, ty),
            function: function.clone(),
            subject: subject_name.clone(),
            into,
        });
    }
}

/// How much a destination says about where the view went, lowest first.
///
/// A named field is the most useful thing to put under the caret: it is a
/// declaration the reader can go and look at. The result is the least, because
/// it says only that the view left.
fn rank(into: &Destination) -> u8 {
    match into {
        Destination::Field { .. } => 0,
        Destination::Subject { .. } => 1,
        Destination::Task => 2,
        Destination::Result { .. } => 3,
    }
}

/// A type as the source writes it, for a message to quote back.
fn write_type(parsed: &Parsed, ty: &Type) -> String {
    let mut out = String::new();
    if ty.is_view {
        out.push('&');
    }
    let parts = || -> Vec<String> { ty.generics.iter().map(|g| write_type(parsed, g)).collect() };
    if ty.is_tuple {
        out.push_str(&format!("({})", parts().join(", ")));
        return out;
    }
    out.push_str(parsed.text(ty.name));
    if !ty.generics.is_empty() {
        out.push_str(&format!("[{}]", parts().join(", ")));
    }
    out
}

/// The walk, for one parameter of one function.
struct Scanner<'p> {
    parsed: &'p Parsed,
    borrowing: &'p HashSet<Ident>,
    fields: &'p HashMap<Ident, Vec<(String, Type)>>,
    subject: Option<&'p Type>,
    subject_fields: &'p [(String, Type)],
    /// The declared result, where handing the view back would be storing it.
    result: Option<&'p Type>,
    /// Names that may carry this parameter's view. Monotone: a name that ever
    /// carries it keeps carrying it, which is the fail-closed direction.
    carriers: BTreeSet<String>,
    found: Vec<(Span, Destination)>,
}

/// What the receiver of a call is rooted in.
enum Root {
    /// `self`.
    Subject,
    /// `self.stations`, `self.a.b` - the field off `self` is what carries.
    SubjectField(Ident),
    /// A local, a literal, a free call: not a place this module decides about.
    Elsewhere,
}

impl Scanner<'_> {
    /// `tail` says whether the last statement of this block is the function's
    /// result - true for the body itself and false inside a loop or a lambda,
    /// where a trailing expression is not what the function hands back.
    fn block(&mut self, block: &Block, tail: bool) {
        let last = block.stmts.len().saturating_sub(1);
        for (i, stmt) in block.stmts.iter().enumerate() {
            let returning = tail && i == last;
            match &stmt.node {
                Stmt::Let { name, value, .. } => {
                    if self.mentions(value) {
                        self.carriers.insert(self.parsed.text(*name).to_string());
                    }
                    self.stores(value, &stmt.span);
                }
                Stmt::Assign { target, value, .. } => {
                    if self.mentions(value) {
                        match target {
                            Expr::Variable(name) => {
                                self.carriers.insert(self.parsed.text(*name).to_string());
                            }
                            _ => self.assigned(target, &stmt.span),
                        }
                    }
                    self.stores(target, &stmt.span);
                    self.stores(value, &stmt.span);
                }
                Stmt::Return(Some(value)) => {
                    self.returned(value, &stmt.span);
                    self.stores(value, &stmt.span);
                }
                Stmt::Return(None) => {}
                Stmt::Expr(expr) => {
                    if returning {
                        self.returned(expr, &stmt.span);
                    }
                    self.stores(expr, &stmt.span);
                }
                Stmt::For {
                    bindings,
                    iter,
                    body,
                } => {
                    if self.mentions(iter) {
                        for binding in bindings {
                            self.carriers.insert(self.parsed.text(*binding).to_string());
                        }
                    }
                    self.stores(iter, &stmt.span);
                    self.block(body, false);
                }
                Stmt::While { cond, body } => {
                    self.stores(cond, &stmt.span);
                    self.block(body, false);
                }
            }
        }
    }

    /// `self.label = name`, and the deeper forms of it.
    fn assigned(&mut self, target: &Expr, span: &Span) {
        match self.root(target) {
            Root::SubjectField(field) => self.field_of_subject(field, span),
            Root::Subject => self.subject_destination(span),
            Root::Elsewhere => {}
        }
    }

    /// A value handed back, where the result holds a view of another buffer.
    fn returned(&mut self, value: &Expr, span: &Span) {
        let Some(result) = self.result else { return };
        if self.mentions(value) {
            self.found.push((
                span.clone(),
                Destination::Result {
                    ty: write_type(self.parsed, result),
                },
            ));
        }
    }

    /// Every store inside one expression.
    fn stores(&mut self, expr: &Expr, span: &Span) {
        match expr {
            Expr::MethodCall {
                receiver,
                args,
                config,
                ..
            } => {
                let handed_over = args.iter().any(|a| self.mentions(a))
                    || config.iter().any(|c| self.mentions(&c.value));
                if handed_over {
                    match self.root(receiver) {
                        Root::SubjectField(field) => self.field_of_subject(field, span),
                        Root::Subject => self.subject_destination(span),
                        Root::Elsewhere => {}
                    }
                }
                self.descend(expr, span);
            }
            Expr::StructLit { name, fields } => {
                if self.borrowing.contains(name) {
                    let declared = self.fields.get(name).cloned().unwrap_or_default();
                    for init in fields {
                        let field = self.parsed.text(init.name).to_string();
                        let mentioned = match &init.value {
                            Some(value) => self.mentions(value),
                            // `Reading { name, temp }` - the shorthand names a
                            // variable spelled like the field.
                            None => self.carriers.contains(&field),
                        };
                        if !mentioned {
                            continue;
                        }
                        let Some((_, ty)) = declared.iter().find(|(f, _)| *f == field) else {
                            continue;
                        };
                        if !holds_view(ty) && !names_borrowing(ty, self.borrowing) {
                            continue;
                        }
                        self.found.push((
                            span.clone(),
                            Destination::Field {
                                owner: self.parsed.text(*name).to_string(),
                                field,
                                ty: write_type(self.parsed, ty),
                            },
                        ));
                    }
                }
                self.descend(expr, span);
            }
            Expr::Spawn { body, .. } => {
                if self.mentions(body) {
                    self.found.push((span.clone(), Destination::Task));
                }
                self.descend(expr, span);
            }
            other => self.descend(other, span),
        }
    }

    /// A call on the subject itself, whose end this module does not see.
    fn subject_destination(&mut self, span: &Span) {
        if let Some(owner) = self.subject_text() {
            self.found
                .push((span.clone(), Destination::Subject { owner }));
        }
    }

    /// A store into the field `field` of the subject, where that field holds a
    /// view. A field that holds none cannot be where the view went.
    fn field_of_subject(&mut self, field: Ident, span: &Span) {
        let name = self.parsed.text(field).to_string();
        let Some((_, ty)) = self
            .subject_fields
            .iter()
            .find(|(declared, _)| *declared == name)
        else {
            return;
        };
        if !holds_view(ty) && !names_borrowing(ty, self.borrowing) {
            return;
        }
        let Some(owner) = self.subject_text() else {
            return;
        };
        self.found.push((
            span.clone(),
            Destination::Field {
                owner,
                field: name,
                ty: write_type(self.parsed, ty),
            },
        ));
    }

    fn subject_text(&self) -> Option<String> {
        self.subject.map(|t| self.parsed.text(t.name).to_string())
    }

    /// What a call's receiver is rooted in.
    fn root(&self, expr: &Expr) -> Root {
        match expr {
            Expr::Variable(name) if self.parsed.text(*name) == "self" => Root::Subject,
            Expr::Field { base, name } => match self.root(base) {
                // The outer field sits inside the root one, so the root field
                // is what carries the buffer.
                Root::Subject => Root::SubjectField(*name),
                other => other,
            },
            Expr::MethodCall { receiver, .. } => self.root(receiver),
            Expr::Index { base, .. } => self.root(base),
            Expr::Try(inner) => self.root(inner),
            _ => Root::Elsewhere,
        }
    }

    /// Whether a carrier of the view appears anywhere in this expression.
    ///
    /// By name, and nothing more: `name.len()` mentions `name`, and so does
    /// `name.to_owned()`. Over-reporting here is the fail-closed direction, and
    /// the destination's declared type is what keeps it from mattering - a field
    /// that holds no view is never a destination.
    fn mentions(&self, expr: &Expr) -> bool {
        let mut found = false;
        each(expr, &mut |inner| {
            if let Expr::Variable(name) = inner {
                if self.carriers.contains(self.parsed.text(*name)) {
                    found = true;
                }
            }
        });
        found
    }

    /// The sub-expressions of `expr`, for [`Scanner::stores`] to recurse into.
    fn descend(&mut self, expr: &Expr, span: &Span) {
        let mut children: Vec<&Expr> = Vec::new();
        let mut blocks: Vec<&Block> = Vec::new();
        parts(expr, &mut children, &mut blocks);
        for child in children {
            self.stores(child, span);
        }
        for block in blocks {
            // A lambda's trailing expression is the lambda's result, not the
            // function's.
            self.block(block, false);
        }
    }
}

/// Every expression inside `expr`, itself included.
fn each(expr: &Expr, f: &mut impl FnMut(&Expr)) {
    f(expr);
    let mut children: Vec<&Expr> = Vec::new();
    let mut blocks: Vec<&Block> = Vec::new();
    parts(expr, &mut children, &mut blocks);
    for child in children {
        each(child, f);
    }
    for block in blocks {
        each_block(block, f);
    }
}

fn each_block(block: &Block, f: &mut impl FnMut(&Expr)) {
    for stmt in &block.stmts {
        for child in stmt_exprs(&stmt.node) {
            each(child, f);
        }
        if let Stmt::For { body, .. } | Stmt::While { body, .. } = &stmt.node {
            each_block(body, f);
        }
    }
}

/// The expressions a statement holds.
fn stmt_exprs(stmt: &Stmt) -> Vec<&Expr> {
    match stmt {
        Stmt::Let { value, .. } => vec![value],
        Stmt::Expr(expr) | Stmt::Return(Some(expr)) => vec![expr],
        Stmt::Assign { target, value, .. } => vec![target, value],
        Stmt::For { iter, .. } => vec![iter],
        Stmt::While { cond, .. } => vec![cond],
        Stmt::Return(None) => Vec::new(),
    }
}

/// An expression's immediate parts: the expressions it holds, and the blocks.
///
/// Every variant is named, so a variant added later is a compile error here
/// rather than a hole in the walk - the same argument [`Expr::LitInterpolated`]
/// makes about itself.
fn parts<'e>(expr: &'e Expr, children: &mut Vec<&'e Expr>, blocks: &mut Vec<&'e Block>) {
    match expr {
        Expr::MethodCall {
            receiver,
            args,
            config,
            ..
        } => {
            children.push(receiver);
            children.extend(args);
            children.extend(config.iter().map(|c| &c.value));
        }
        Expr::Call { func, args, config } => {
            children.push(func);
            children.extend(args);
            children.extend(config.iter().map(|c| &c.value));
        }
        Expr::Field { base, .. } => children.push(base),
        Expr::Index { base, index } => {
            children.push(base);
            children.push(index);
        }
        Expr::Binary { lhs, rhs, .. } => {
            children.push(lhs);
            children.push(rhs);
        }
        Expr::Unary { expr, .. }
        | Expr::Try(expr)
        | Expr::Throw(expr)
        | Expr::Cast { expr, .. }
        | Expr::Spawn { body: expr, .. }
        | Expr::DslFrom { input: expr, .. } => children.push(expr),
        Expr::Coalesce { value, fallback } => {
            children.push(value);
            children.push(fallback);
        }
        Expr::Range { start, end, .. } => {
            children.push(start);
            children.push(end);
        }
        Expr::Tuple(parts) => children.extend(parts),
        Expr::StructLit { fields, .. } => {
            children.extend(fields.iter().filter_map(|f| f.value.as_ref()))
        }
        Expr::Match { value, arms } => {
            children.push(value);
            children.extend(arms.iter().map(|a| &a.body));
        }
        Expr::If {
            cond,
            then_branch,
            else_branch,
        } => {
            children.push(cond);
            blocks.push(then_branch);
            blocks.extend(else_branch.as_ref());
        }
        Expr::Block(block) | Expr::Seq(block) | Expr::Closure { body: block, .. } => {
            blocks.push(block)
        }
        Expr::TryCatch { expr, handler } => {
            children.push(expr);
            blocks.push(handler);
        }
        Expr::Asm { .. }
        | Expr::Dsl { .. }
        | Expr::Path(_)
        | Expr::Variable(_)
        | Expr::LitInt(_)
        | Expr::LitFloat(_)
        | Expr::LitStr(_)
        | Expr::LitInterpolated(_)
        | Expr::LitChar(_)
        | Expr::LitBool(_) => {}
    }
}
