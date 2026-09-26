// crates/nikaia/src/contracts/keep.rs
//
// Where a buffer lives once views of it outlive the scope that made it
// ([ADR-209](../../../../docs/specification/adr/adr-209.md), Part I 6.6).
//
// ## The question, and the one answer to it
//
// A body reads a file into `text`, cuts it into views, and something keeps the
// views longer than `text` would live: the result, a `mut` parameter, a list
// declared outside the loop, a task. [ADR-008](../../../../docs/specification/adr/adr-008.md)
// calls that value **tethered** and says the buffer has to live as long as the
// views do. This file decides **where** it lives, and the answer is always the
// same rule (D1):
//
// > **A buffer lives in the keep of whatever keeps its views.**
//
// What differs is who owns that keep, and that is read off facts this compiler
// already has rather than chosen once for every program (D2-D4):
//
// * **a frame** - the nearest caller, or this function's own frame for a list
//   declared outside the loop. Costs nothing: no count, no handle, and the body
//   may pause (D2);
// * **a handle that travels with the value**, where no frame outlives it - a
//   task (D3);
// * **a handle per element**, where a container keeps views across a loop and
//   drops entries as it goes - one keep for the loop would keep every buffer it
//   ever read (D4).
//
// ## How a view is followed
//
// By **origin**: every local carries the set of places its views may point
// into. A `let` whose initialiser makes a buffer (`makes_a_buffer`, the same
// question the column above asks) is an origin; so is a call to a function that
// itself hands back tethered views, because its buffer needs a keep from here.
// Origins flow through methods, fields, indexing, `for` bindings, struct and
// list literals, `push` and `insert`, and they stop at anything that hands back
// something of its own: `to_owned`, `len`, a call whose declared result holds no
// view. That last stop is the one `views::check` was missing, and why it blamed
// `path` for a view of the text `fs::read_to_string(path)` read.
//
// ## Which way it errs
//
// **Towards a keep, never towards a refusal.** A keep nobody needed costs a
// buffer living until the end of a scope instead of the end of a statement. A
// refusal nobody needed is a correct program refused
// ([Part III C.4](../../../../docs/specification/30-nikaia-tooling.md)). So a
// destination whose type this walk cannot read is taken to hold a view, and a
// refusal is only ever raised where the type says so.

use std::collections::{BTreeMap, BTreeSet};

use crate::ast::{Block, Expr, Item, Span, Stmt, Type};
use crate::parser::Parsed;

use super::tether::{Buffer, State, RESULT};
use super::Ledger;

/// A source, by the statement it stands in and - for a call - the callee:
/// what the emitter has in hand where it writes either.
pub type Id = (usize, String);

/// Who owns the keep a buffer, or a call's buffers, go into.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum KeepAt {
    /// The keep this function was **given**: its views leave through the
    /// result, a `mut` parameter or the subject (D2).
    Param,
    /// A keep declared first in this function's body, for views kept by a
    /// local that outlives the scope the buffer was read in (D2).
    Frame,
    /// A keep declared just before the statement that needs it: a call whose
    /// views go nowhere further than this scope.
    Local(usize),
    /// The shared keep of this function's tasks (D3).
    Task,
    /// A keep of its own, one per buffer, for a container that drops entries
    /// (D4). The number is the statement that reads the buffer.
    Element(usize),
}

/// The words `--tethers` uses for a keep.
pub fn describe(keep: KeepAt) -> &'static str {
    match keep {
        KeepAt::Param => "in the caller's keep, because views of it leave this function",
        KeepAt::Frame => {
            "in a keep of this function until it returns, because something outside the \
             loop or block keeps views of it"
        }
        KeepAt::Local(_) => "in a keep beside the call, as long as what the call hands back",
        KeepAt::Task => "in a keep the task carries with it (one count per task, not per view)",
        KeepAt::Element(_) => {
            "in a keep of its own, held by each view kept of it, because what keeps the \
             views drops entries while the loop goes on"
        }
    }
}

/// Where a view leaves the scope that made it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Escape {
    /// Handed back to the caller.
    Result,
    /// Kept by a `mut` parameter, or by the subject.
    Param(String),
    /// Kept by a local that outlives the buffer's scope.
    Outer(String),
    /// Captured by a task.
    Task,
}

/// Something views may point into.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// `let text = fs::read_to_string(…)`: a buffer this body makes.
    Buffer {
        at: usize,
        name: String,
        ty: String,
        span: Span,
    },
    /// `load(path)`, where `load` hands back tethered views: its buffers need a
    /// keep from here.
    Call {
        at: usize,
        callee: String,
        span: Span,
    },
}

impl Source {
    pub fn at(&self) -> usize {
        match self {
            Source::Buffer { at, .. } | Source::Call { at, .. } => *at,
        }
    }

    fn span(&self) -> &Span {
        match self {
            Source::Buffer { span, .. } | Source::Call { span, .. } => span,
        }
    }

    /// How a message names it.
    pub fn named(&self) -> String {
        match self {
            Source::Buffer { name, ty, .. } => format!("`{name}` (a `{ty}` this body reads)"),
            Source::Call { callee, .. } => format!("what `{callee}` hands back"),
        }
    }
}

/// One function's answer: where each of its buffers lives, and what the
/// lowering writes for it.
#[derive(Debug, Clone, Default)]
pub struct Plan {
    /// The ledger key.
    pub key: String,
    /// The body's first statement, before which the function's own keeps are
    /// declared.
    pub first: Option<usize>,
    /// It takes a keep from its caller (D2), and the ledger says so.
    pub takes_keep: bool,
    /// It declares a keep first in its body.
    pub frame_keep: bool,
    /// It declares the shared keep of its tasks first in its body (D3).
    pub task_keep: bool,
    /// Statements before which a keep of their own is declared.
    pub local_keeps: BTreeSet<usize>,
    /// A buffer `let`, by its statement, and the keep it goes into.
    pub puts: BTreeMap<usize, KeepAt>,
    /// A call to a function that takes a keep, by its statement and callee.
    pub calls: BTreeMap<(usize, String), KeepAt>,
    /// Bindings a task captures whose views need the task's keep (D3): the
    /// name, and the statement that binds it.
    pub tethered: BTreeMap<String, usize>,
    /// Locals whose text views are held one handle per element (D4).
    pub element_keepers: BTreeSet<String>,
    /// Statements that put a view into such a local: by statement and local,
    /// which argument of the call carries it and the buffer statement whose
    /// keep it is held with.
    pub holds: BTreeMap<(usize, String), BTreeMap<usize, usize>>,
    /// Every place a view leaves its scope, for the report and the refusals.
    pub escapes: Vec<(Source, Escape)>,
    /// What cannot be lowered, said in this language's words.
    pub refusals: Vec<crate::check::Finding>,
}

impl Plan {
    /// Whether anything in this function moved.
    pub fn is_empty(&self) -> bool {
        !self.takes_keep
            && !self.frame_keep
            && !self.task_keep
            && self.local_keeps.is_empty()
            && self.puts.is_empty()
            && self.calls.is_empty()
            && self.refusals.is_empty()
    }
}

/// Every function of a package, planned, with the `views` column of each
/// function that takes a keep set to `tethered` (D6).
///
/// **A fixpoint**, because a function takes a keep exactly when one of its
/// sources leaves through its result or a parameter, and a source may be a call
/// to another function that takes one. Monotone - a function never stops taking
/// a keep once it does - so it ends.
pub fn infer(ledger: &mut Ledger, units: &[&Parsed], library: &Ledger) {
    for _ in 0..32 {
        let mut changed = false;
        for parsed in units.iter().copied() {
            for plan in plans(parsed, ledger, library) {
                let Some(contract) = ledger.functions.get_mut(&plan.key) else {
                    continue;
                };
                for position in keeping_positions(&plan) {
                    match contract.views.iter_mut().find(|h| h.position == position) {
                        Some(held) if held.state == State::Tethered => {}
                        Some(held) => {
                            held.state = State::Tethered;
                            changed = true;
                        }
                        None => {
                            contract.views.push(super::tether::Held {
                                position,
                                state: State::Tethered,
                            });
                            changed = true;
                        }
                    }
                }
            }
        }
        if !changed {
            break;
        }
    }
}

/// The positions through which a plan's views leave.
fn keeping_positions(plan: &Plan) -> BTreeSet<String> {
    plan.escapes
        .iter()
        .filter_map(|(_, escape)| match escape {
            Escape::Result => Some(RESULT.to_string()),
            Escape::Param(name) => Some(name.clone()),
            _ => None,
        })
        .collect()
}

/// Whether a contract takes a keep from its caller: some position of it is
/// tethered.
pub fn takes_a_keep(contract: &super::FnContract) -> bool {
    contract.views.iter().any(|h| h.state == State::Tethered)
}

/// Every function of one unit, planned against the ledger as it stands.
pub fn plans(parsed: &Parsed, ledger: &Ledger, library: &Ledger) -> Vec<Plan> {
    let context = Context::of(parsed);
    let mut out = Vec::new();
    for item in &parsed.program.items {
        match &item.node {
            Item::Fn { .. } => {
                if let Some(plan) = plan(parsed, item, None, ledger, library, &context) {
                    out.push(plan);
                }
            }
            Item::Impl {
                target, methods, ..
            } => {
                let target = parsed.text(target.name).to_string();
                for method in methods {
                    if let Some(plan) =
                        plan(parsed, method, Some(&target), ledger, library, &context)
                    {
                        out.push(plan);
                    }
                }
            }
            _ => {}
        }
    }
    out
}

/// What a whole unit says about views: which types hold one.
struct Context {
    borrowing: BTreeSet<String>,
}

impl Context {
    fn of(parsed: &Parsed) -> Context {
        let borrowing = crate::emit::borrowing_structs(parsed)
            .into_iter()
            .map(|s| parsed.text(s).to_string())
            .collect();
        Context { borrowing }
    }

    /// Whether a written type holds a view: it is one, or names a type that
    /// holds one, or an argument of it does.
    fn carries(&self, parsed: &Parsed, ty: &Type) -> bool {
        ty.is_view
            || self.borrowing.contains(parsed.text(ty.name))
            || ty.generics.iter().any(|g| self.carries(parsed, g))
    }
}

/// One local, as far as this walk needs it.
#[derive(Debug, Clone)]
struct Local {
    /// The statement that binds it, which orders it against a buffer's.
    at: usize,
    /// The blocks it is declared inside, outermost first, each named by the
    /// statement that opens it; a loop's is marked.
    path: Vec<(usize, bool)>,
    origins: BTreeSet<Id>,
    /// Its written type, where the `let` wrote one.
    ty: Option<Type>,
    /// A parameter, or the subject: a destination with a name of its own.
    parameter: bool,
    /// A `mut` parameter or a `ref mut self`: a place views may be kept in.
    keeps: bool,
    /// **The buffer itself**, not a view of it: `let text = fs::read(…)`, or
    /// a second name for the same value. Handed on whole it is a move, which
    /// takes the buffer along and leaves nothing to keep alive.
    buffer: bool,
}

struct Walk<'a> {
    parsed: &'a Parsed,
    ledger: &'a Ledger,
    library: &'a Ledger,
    target: Option<&'a str>,
    /// Scopes of locals, innermost last.
    scopes: Vec<BTreeMap<String, Local>>,
    /// The blocks the walk is inside, outermost first.
    path: Vec<(usize, bool)>,
    sources: BTreeMap<Id, (Source, Vec<(usize, bool)>)>,
    escapes: BTreeSet<(Id, Escape)>,
    /// Locals something removes entries from, or assigns again.
    sheds: BTreeSet<String>,
    /// Statements that put a value with origins into a local:
    /// `(statement, local) -> argument -> origins`.
    puts_into: BTreeMap<(usize, String), BTreeMap<usize, BTreeSet<Id>>>,
    /// Task captures: `(binding, its let statement)`.
    captured: BTreeMap<String, (usize, BTreeSet<Id>)>,
    /// The statement being walked.
    statement: usize,
    /// Whether the declared result can hold a view at all.
    result_carries: bool,
    /// Every local as it was last bound, for questions asked after its scope
    /// has closed.
    last_seen: BTreeMap<String, Local>,
}

/// One function's plan, or `None` for something that is not a function.
fn plan(
    parsed: &Parsed,
    item: &crate::ast::Spanned<Item>,
    target: Option<&str>,
    ledger: &Ledger,
    library: &Ledger,
    context: &Context,
) -> Option<Plan> {
    let Item::Fn {
        name,
        receiver,
        args,
        ret_type,
        body,
        ..
    } = &item.node
    else {
        return None;
    };
    let own = match name {
        Some(name) => parsed.text(*name).to_string(),
        None => "new".to_string(),
    };
    let key = match target {
        Some(target) => format!("{target}::{own}"),
        None => own.clone(),
    };

    let mut frame = BTreeMap::new();
    for arg in args {
        frame.insert(
            parsed.text(arg.name).to_string(),
            Local {
                at: 0,
                path: Vec::new(),
                origins: BTreeSet::new(),
                ty: Some(arg.ty.clone()),
                parameter: true,
                keeps: arg.mutable && context.carries(parsed, &arg.ty),
                buffer: false,
            },
        );
    }
    if let Some(receiver) = receiver {
        frame.insert(
            "self".to_string(),
            Local {
                at: 0,
                path: Vec::new(),
                origins: BTreeSet::new(),
                ty: None,
                parameter: true,
                keeps: receiver.is_mut && target.is_some_and(|t| context.borrowing.contains(t)),
                buffer: false,
            },
        );
    }

    let mut walk = Walk {
        parsed,
        ledger,
        library,
        target,
        scopes: vec![frame],
        path: Vec::new(),
        sources: BTreeMap::new(),
        escapes: BTreeSet::new(),
        sheds: BTreeSet::new(),
        puts_into: BTreeMap::new(),
        captured: BTreeMap::new(),
        statement: 0,
        last_seen: BTreeMap::new(),
        result_carries: ret_type
            .as_ref()
            .is_some_and(|t| context.carries(parsed, t)),
    };
    let result_carries = walk.result_carries;
    // Twice, so that an origin a loop's later statement gives a local reaches
    // the loop's earlier ones: origins only grow, and the second pass sees
    // everything the first one found.
    for _ in 0..2 {
        walk.scopes.truncate(1);
        walk.path.clear();
        // The body's own scope stays open for its last expression, which is
        // what the function hands back and reads the body's locals.
        walk.scopes.push(BTreeMap::new());
        for stmt in &body.stmts {
            walk.statement = stmt.span.start;
            walk.stmt(&stmt.node, &stmt.span);
        }
        if result_carries {
            if let Some(last) = body.stmts.last() {
                if let Stmt::Expr(value) = &last.node {
                    walk.statement = last.span.start;
                    let origins = walk.origins(value);
                    if !walk.is_the_buffer(value) {
                        walk.escape_all(&origins, Escape::Result);
                    }
                }
            }
        }
        walk.scopes.pop();
    }
    let _ = result_carries;

    Some(decide(parsed, item, key, walk, context))
}

impl Walk<'_> {
    fn block(&mut self, block: &Block) {
        self.scopes.push(BTreeMap::new());
        for stmt in &block.stmts {
            self.statement = stmt.span.start;
            self.stmt(&stmt.node, &stmt.span);
        }
        self.scopes.pop();
    }

    fn nested(&mut self, at: usize, looping: bool, block: &Block) {
        self.path.push((at, looping));
        self.block(block);
        self.path.pop();
    }

    /// **Whether a `clone` here is a copy of text**
    /// ([ADR-216](../../../docs/specification/adr/adr-216.md) D2), which is
    /// text of its own and points into nothing - where a copy of a list of
    /// views still points where the views did. Read off what this walk can see
    /// without types: a literal, or a name whose type was written as text.
    /// Anything else is the list's answer, which is the one that keeps more.
    fn copies_text(&self, receiver: &Expr) -> bool {
        match receiver {
            Expr::LitStr { .. } | Expr::LitInterpolated(_) => true,
            Expr::Variable(name) => self
                .local(self.parsed.text(*name))
                .and_then(|local| local.ty.as_ref())
                .is_some_and(|ty| {
                    matches!(self.parsed.text(ty.name), "String" | "str") && ty.generics.is_empty()
                }),
            _ => false,
        }
    }

    /// Whether `to_string` of this makes text rather than handing back the
    /// text it is: a name whose written type is something other than text.
    fn formats(&self, receiver: &Expr) -> bool {
        match receiver {
            Expr::Variable(name) => self
                .local(self.parsed.text(*name))
                .and_then(|local| local.ty.as_ref())
                .is_some_and(|ty| !matches!(self.parsed.text(ty.name), "String" | "str")),
            _ => false,
        }
    }

    fn local(&self, name: &str) -> Option<&Local> {
        self.scopes.iter().rev().find_map(|scope| scope.get(name))
    }

    fn local_mut(&mut self, name: &str) -> Option<&mut Local> {
        self.scopes
            .iter_mut()
            .rev()
            .find_map(|scope| scope.get_mut(name))
    }

    fn bind(
        &mut self,
        name: String,
        at: usize,
        origins: BTreeSet<Id>,
        ty: Option<Type>,
        buffer: bool,
    ) {
        let local = Local {
            at,
            path: self.path.clone(),
            origins,
            ty,
            parameter: false,
            keeps: false,
            buffer,
        };
        self.last_seen.insert(name.clone(), local.clone());
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name, local);
        }
    }

    fn stmt(&mut self, stmt: &Stmt, span: &Span) {
        let at = span.start;
        match stmt {
            Stmt::Let {
                names, value, ty, ..
            } => {
                self.spawns_in(value);
                self.nested_blocks(value, at);
                let made = match names.as_slice() {
                    [_] => match value {
                        // A copy of text is a buffer of its own (ADR-216 D2).
                        Expr::MethodCall {
                            receiver, method, ..
                        } if self.parsed.text(*method) == "clone" && self.copies_text(receiver) => {
                            Buffer::Named("String".to_string())
                        }
                        // And so is the text form of something that is not
                        // text - `seed.to_string()` for `seed: i64` - where
                        // text's own is the text itself (ADR-216 D4).
                        Expr::MethodCall {
                            receiver, method, ..
                        } if self.parsed.text(*method) == "to_string" && self.formats(receiver) => {
                            Buffer::Named("String".to_string())
                        }
                        _ => match super::tether::makes_a_buffer(
                            self.parsed,
                            value,
                            self.ledger,
                            self.library,
                        ) {
                            // A name declared `String` is text of this
                            // frame's own, built or moved here.
                            Buffer::Named(name) => Buffer::Named(name),
                            _ if !self.is_the_buffer(value)
                                && super::tether::declares_text(self.parsed, ty.as_ref()) =>
                            {
                                Buffer::Named("String".to_string())
                            }
                            other => other,
                        },
                    },
                    _ => Buffer::None,
                };
                // A second name for the buffer is the buffer.
                let renames_a_buffer = self.is_the_buffer(value);
                let is_buffer = matches!(made, Buffer::Named(_)) || renames_a_buffer;
                let origins = match made {
                    Buffer::Named(buffer) => {
                        let name = self.parsed.text(names[0]).to_string();
                        self.sources.entry((at, String::new())).or_insert((
                            Source::Buffer {
                                at,
                                name,
                                ty: buffer,
                                span: span.clone(),
                            },
                            self.path.clone(),
                        ));
                        BTreeSet::from([(at, String::new())])
                    }
                    _ => self.origins(value),
                };
                for name in names {
                    let name = self.parsed.text(*name).to_string();
                    self.bind(name, at, origins.clone(), ty.clone(), is_buffer);
                }
            }
            Stmt::Comptime { .. } => {}
            Stmt::Assign { target, op, value } => {
                self.spawns_in(value);
                self.nested_blocks(value, at);
                let origins = self.origins(value);
                if let Some(root) = root_of(self.parsed, target) {
                    if op.is_none() && matches!(target, Expr::Variable(_)) {
                        self.sheds.insert(root.clone());
                    }
                    self.flow_into(&root, origins);
                }
            }
            Stmt::For {
                bindings,
                iter,
                body,
            } => {
                let origins = self.origins(iter);
                self.path.push((at, true));
                self.scopes.push(BTreeMap::new());
                for name in bindings {
                    let name = self.parsed.text(*name).to_string();
                    self.bind(name, at, origins.clone(), None, false);
                }
                for inner in &body.stmts {
                    self.statement = inner.span.start;
                    self.stmt(&inner.node, &inner.span);
                }
                self.scopes.pop();
                self.path.pop();
            }
            Stmt::While { cond, body } => {
                let _ = self.origins(cond);
                self.nested(at, true, body);
            }
            Stmt::Return(Some(value)) => {
                self.spawns_in(value);
                self.nested_blocks(value, at);
                let origins = self.origins(value);
                // Only a result that can hold a view hands one back, and a
                // buffer handed back whole is a move.
                if self.result_carries && !self.is_the_buffer(value) {
                    self.escape_all(&origins, Escape::Result);
                }
            }
            Stmt::Return(None) | Stmt::Break | Stmt::Continue => {}
            Stmt::Expr(expr) => {
                self.spawns_in(expr);
                self.nested_blocks(expr, at);
                self.effects(expr);
            }
        }
    }

    /// The blocks inside an expression that are not a value's arms: a lambda
    /// run during a call, an `if` or `match` in statement position. Walked for
    /// what their statements do.
    fn nested_blocks(&mut self, expr: &Expr, at: usize) {
        let mut blocks: Vec<&Block> = Vec::new();
        super::sync::visit_expr_blocks(expr, &mut |b| blocks.push(b));
        for block in blocks {
            let statement = self.statement;
            self.nested(at, false, block);
            self.statement = statement;
        }
    }

    /// What an expression in statement position does to the places views may
    /// be kept in: `xs.push(v)`, `m.insert(k, v)`, a call that keeps into one
    /// of its arguments.
    fn effects(&mut self, expr: &Expr) {
        match expr {
            Expr::MethodCall {
                receiver,
                method,
                args,
                ..
            } => {
                let method_name = self.parsed.text(*method).to_string();
                if let Some(root) = root_of(self.parsed, receiver) {
                    if SHEDS.contains(&method_name.as_str()) {
                        self.sheds.insert(root.clone());
                    }
                    if KEEPS.contains(&method_name.as_str()) {
                        let mut origins = BTreeSet::new();
                        for (at, arg) in args.iter().enumerate() {
                            let of = match self.is_the_buffer(arg) {
                                // The buffer itself goes in: a move.
                                true => BTreeSet::new(),
                                false => self.origins(arg),
                            };
                            self.puts_into
                                .entry((self.statement, root.clone()))
                                .or_default()
                                .entry(at)
                                .or_default()
                                .extend(of.iter().cloned());
                            origins.extend(of);
                        }
                        self.flow_into(&root, origins);
                    }
                }
                let _ = self.origins(expr);
            }
            Expr::TryCatch { expr, .. } | Expr::Try(expr) => self.effects(expr),
            _ => {
                let _ = self.origins(expr);
            }
        }
    }

    /// Whether an expression **is** a buffer rather than a view of one: a
    /// name bound to one, or its copy.
    fn is_the_buffer(&self, expr: &Expr) -> bool {
        match expr {
            Expr::Variable(name) => self
                .local(self.parsed.text(*name))
                .is_some_and(|l| l.buffer),
            Expr::MethodCall {
                receiver, method, ..
            } if matches!(
                self.parsed.text(*method),
                "clone" | "to_owned" | "to_string"
            ) =>
            {
                self.is_the_buffer(receiver)
            }
            _ => false,
        }
    }

    /// Views with these origins now live in `root` as well.
    fn flow_into(&mut self, root: &str, origins: BTreeSet<Id>) {
        if origins.is_empty() {
            return;
        }
        let Some(local) = self.local(root).cloned() else {
            return;
        };
        if local.parameter {
            if local.keeps {
                self.escape_all(&origins, Escape::Param(root.to_string()));
            }
            return;
        }
        // **A local declared before the buffer, in a scope around it, outlives
        // it**: the loop's list, or a list declared first in the same block -
        // which the language below drops *after* a buffer declared later, and
        // refuses for it.
        for origin in &origins {
            let Some((source, path)) = self.sources.get(origin) else {
                continue;
            };
            let _ = source;
            let around =
                local.path.len() <= path.len() && path[..local.path.len()] == local.path[..];
            if around && local.at < origin.0 {
                self.escapes
                    .insert((origin.clone(), Escape::Outer(root.to_string())));
            }
        }
        if let Some(local) = self.local_mut(root) {
            local.origins.extend(origins);
        }
    }

    fn escape_all(&mut self, origins: &BTreeSet<Id>, escape: Escape) {
        for origin in origins {
            if self.sources.contains_key(origin) {
                self.escapes.insert((origin.clone(), escape.clone()));
            }
        }
    }

    /// A `spawn` anywhere in this expression: every local its body names that
    /// carries origins leaves with the task (D3).
    fn spawns_in(&mut self, expr: &Expr) {
        let parsed = self.parsed;
        // Cloned, because a hole's expressions are parsed out of the literal
        // for the walk and do not outlive it.
        let mut bodies: Vec<Expr> = Vec::new();
        super::sync::visit_expr(parsed, expr, &mut |e| {
            if let Expr::Spawn { body, .. } = e {
                bodies.push((**body).clone());
            }
        });
        for body in &bodies {
            let mut named = BTreeSet::new();
            names_in(parsed, body, &mut named);
            for name in named {
                let Some(local) = self.local(&name).cloned() else {
                    continue;
                };
                // A buffer a task takes is moved into it, whole.
                if local.origins.is_empty() || local.buffer {
                    continue;
                }
                self.escape_all(&local.origins, Escape::Task);
                self.captured
                    .entry(name)
                    .or_insert((local.at, BTreeSet::new()))
                    .1
                    .extend(local.origins.iter().cloned());
            }
        }
    }

    /// The origins of an expression's value: where its views may point.
    fn origins(&mut self, expr: &Expr) -> BTreeSet<Id> {
        match expr {
            Expr::Variable(name) => self
                .local(self.parsed.text(*name))
                .map(|l| l.origins.clone())
                .unwrap_or_default(),
            Expr::MethodCall {
                receiver,
                method,
                args,
                ..
            }
            | Expr::SafeMethod {
                receiver,
                method,
                args,
                ..
            } => {
                let name = self.parsed.text(*method).to_string();
                let mut out = BTreeSet::new();
                if let Some(callee) = self.keeping_method(receiver, &name) {
                    out.extend(self.call_source(&callee));
                }
                if OWNED.contains(&name.as_str()) || (name == "clone" && self.copies_text(receiver))
                {
                    for arg in args {
                        let _ = self.origins(arg);
                    }
                    let _ = self.origins(receiver);
                    return out;
                }
                out.extend(self.origins(receiver));
                for arg in args {
                    out.extend(self.origins(arg));
                }
                out
            }
            Expr::Call { func, args, .. } => {
                let callee = callee_name(self.parsed, func);
                let mut out = BTreeSet::new();
                let mut arg_origins = Vec::new();
                for arg in args {
                    arg_origins.push(self.origins(arg));
                }
                let contract = callee
                    .as_deref()
                    .and_then(|c| self.ledger.lookup(c).or_else(|| self.library.lookup(c)));
                if let Some((key, contract)) = &contract {
                    if takes_a_keep(contract) {
                        let source = self.call_source(key);
                        // A call that keeps into one of its arguments: the
                        // argument's root now keeps this call's buffers.
                        let params: Vec<String> = contract
                            .signature
                            .as_ref()
                            .map(|s| s.params.iter().map(|(n, _)| n.clone()).collect())
                            .unwrap_or_default();
                        for held in &contract.views {
                            if held.state != State::Tethered || held.position == RESULT {
                                continue;
                            }
                            if let Some(at) = params.iter().position(|p| *p == held.position) {
                                if let Some(root) =
                                    args.get(at).and_then(|a| root_of(self.parsed, a))
                                {
                                    self.flow_into(&root, source.clone());
                                }
                            }
                        }
                        let hands_back = contract
                            .views
                            .iter()
                            .any(|h| h.position == RESULT && h.state == State::Tethered);
                        if hands_back {
                            out.extend(source);
                        }
                    }
                    // **A result that holds no view is the call's own**, and
                    // that is where a chain of views stops: `fs::read_to_string
                    // (path)` hands back a `String`, not a piece of `path`.
                    let owned = contract
                        .signature
                        .as_ref()
                        .and_then(|s| s.result.as_ref())
                        .is_some_and(|ty| !ty_holds_view(ty));
                    if owned {
                        return out;
                    }
                }
                for origins in arg_origins {
                    out.extend(origins);
                }
                out
            }
            Expr::Field { base, .. } | Expr::SafeField { base, .. } => self.origins(base),
            Expr::Index { base, index } => {
                let _ = self.origins(index);
                self.origins(base)
            }
            Expr::StructLit { fields, .. } => {
                let mut out = BTreeSet::new();
                for field in fields {
                    match &field.value {
                        Some(value) => out.extend(self.origins(value)),
                        None => {
                            if let Some(l) = self.local(self.parsed.text(field.name)) {
                                out.extend(l.origins.iter().cloned());
                            }
                        }
                    }
                }
                out
            }
            Expr::With { base, fields, .. } => {
                let mut out = self.origins(base);
                for field in fields {
                    if let Some(value) = &field.value {
                        out.extend(self.origins(value));
                    }
                }
                out
            }
            Expr::ListLit { items, .. } | Expr::Tuple(items) => {
                let mut out = BTreeSet::new();
                for item in items {
                    out.extend(self.origins(item));
                }
                out
            }
            Expr::If {
                then_branch,
                else_branch,
                cond,
            } => {
                let _ = self.origins(cond);
                let mut out = self.tail_origins(then_branch);
                if let Some(block) = else_branch {
                    out.extend(self.tail_origins(block));
                }
                out
            }
            Expr::Match { value, arms } => {
                let _ = self.origins(value);
                let mut out = BTreeSet::new();
                for arm in arms {
                    out.extend(self.origins(&arm.body));
                }
                out
            }
            Expr::Block(block) | Expr::Unsafe(block) => self.tail_origins(block),
            Expr::Unary { expr, .. } | Expr::Try(expr) | Expr::Cast { expr, .. } => {
                self.origins(expr)
            }
            Expr::TryCatch { expr, handler } => {
                let mut out = self.origins(expr);
                out.extend(self.tail_origins(handler));
                out
            }
            Expr::Coalesce { value, fallback } => {
                let mut out = self.origins(value);
                out.extend(self.origins(fallback));
                out
            }
            // **A hole is code** (ADR-032): a call in `f"{load(p).len()}"` is
            // a call of this statement, and the text built around it is text
            // of its own.
            Expr::LitInterpolated(_) => {
                for hole in crate::emit::literal_expressions(self.parsed, expr) {
                    let _ = self.origins(&hole);
                }
                BTreeSet::new()
            }
            // A comparison or arithmetic is a value of its own; text joined
            // with `+` is text of its own.
            Expr::Binary { lhs, rhs, .. } => {
                let _ = self.origins(lhs);
                let _ = self.origins(rhs);
                BTreeSet::new()
            }
            _ => BTreeSet::new(),
        }
    }

    /// The origins of what a block ends in, without walking its statements a
    /// second time for their effects.
    ///
    /// **As the statement it is**, because a call there is written by the
    /// emitter as part of that statement and not of the `if` around it: the
    /// key a call's keep is found by has to be the one the emitter has in hand.
    fn tail_origins(&mut self, block: &Block) -> BTreeSet<Id> {
        match block.stmts.last() {
            Some(last) => match &last.node {
                Stmt::Expr(value) => {
                    let outer = std::mem::replace(&mut self.statement, last.span.start);
                    let origins = self.origins(value);
                    self.statement = outer;
                    origins
                }
                _ => BTreeSet::new(),
            },
            None => BTreeSet::new(),
        }
    }

    /// The source a call to a keeping function stands for, made on first sight.
    ///
    /// Keyed by statement and callee, which is what the emitter has in hand
    /// where it writes the call: one keep per callee per statement.
    fn call_source(&mut self, callee: &str) -> BTreeSet<Id> {
        let id: Id = (self.statement, callee.to_string());
        self.sources.entry(id.clone()).or_insert((
            Source::Call {
                at: self.statement,
                callee: callee.to_string(),
                span: self.statement..self.statement,
            },
            self.path.clone(),
        ));
        BTreeSet::from([id])
    }

    /// A method call to a function of this package that takes a keep.
    fn keeping_method(&self, receiver: &Expr, method: &str) -> Option<String> {
        let on_self = matches!(receiver, Expr::Variable(name) if self.parsed.text(*name) == "self");
        keeping_method_key(self.ledger, self.target, on_self, method)
    }
}

/// The ledger key of a method that takes a keep: `self.load(…)` inside its
/// `impl`, or a name only one keeping function in the ledger has. One answer
/// for the plan and for the emitter, so the call and its keep cannot part.
pub fn keeping_method_key(
    ledger: &Ledger,
    target: Option<&str>,
    on_self: bool,
    method: &str,
) -> Option<String> {
    if let (true, Some(target)) = (on_self, target) {
        let key = format!("{target}::{method}");
        return ledger
            .functions
            .get(&key)
            .filter(|c| takes_a_keep(c))
            .map(|_| key);
    }
    let suffix = format!("::{method}");
    let mut found = ledger
        .functions
        .iter()
        .filter(|(k, c)| k.ends_with(&suffix) && takes_a_keep(c))
        .map(|(k, _)| k.clone());
    let first = found.next()?;
    found.next().is_none().then_some(first)
}

/// Methods whose result is a value of its own, not a view of the receiver.
const OWNED: &[&str] = &[
    "to_owned",
    "clone_text",
    "len",
    "is_empty",
    "count",
    "starts_with",
    "ends_with",
    "contains",
    "contains_key",
    "parse",
    "to_uppercase",
    "to_lowercase",
    "join",
    "repeat",
];

/// Methods that keep what they are given in their receiver.
const KEEPS: &[&str] = &[
    "push",
    "insert",
    "extend",
    "append",
    "push_front",
    "push_back",
];

/// Methods that drop entries from their receiver.
const SHEDS: &[&str] = &[
    "clear",
    "remove",
    "drain",
    "pop",
    "truncate",
    "retain",
    "pop_front",
    "pop_back",
    "take",
];

/// Whether a ledger type holds a view anywhere in it.
fn ty_holds_view(ty: &super::ty::Ty) -> bool {
    match ty {
        super::ty::Ty::Named { view, args, .. } => *view || args.iter().any(ty_holds_view),
        super::ty::Ty::Nullable(inner) => ty_holds_view(inner),
        super::ty::Ty::Tuple(parts) => parts.iter().any(ty_holds_view),
        super::ty::Ty::Unknown => true,
        _ => false,
    }
}

/// The name a call's callee is written as.
pub fn callee_name(parsed: &Parsed, func: &Expr) -> Option<String> {
    match func {
        Expr::Variable(name) => Some(parsed.text(*name).to_string()),
        Expr::Path(segments) => Some(
            parsed.unaliased(
                &segments
                    .iter()
                    .map(|s| parsed.text(*s))
                    .collect::<Vec<_>>()
                    .join("::"),
            ),
        ),
        _ => None,
    }
}

/// The local an expression is rooted in: `xs`, `self.items`, `m[k]`.
pub fn root_of(parsed: &Parsed, expr: &Expr) -> Option<String> {
    match expr {
        Expr::Variable(name) => Some(parsed.text(*name).to_string()),
        Expr::Field { base, .. } | Expr::Index { base, .. } | Expr::SafeField { base, .. } => {
            root_of(parsed, base)
        }
        Expr::Unary { expr, .. } => root_of(parsed, expr),
        _ => None,
    }
}

/// Every name an expression mentions, lambdas and blocks included.
fn names_in(parsed: &Parsed, expr: &Expr, out: &mut BTreeSet<String>) {
    super::sync::visit_expr(parsed, expr, &mut |e| {
        if let Expr::Variable(name) = e {
            out.insert(parsed.text(*name).to_string());
        }
    });
    let mut blocks: Vec<&Block> = Vec::new();
    super::sync::visit_expr_blocks(expr, &mut |b| blocks.push(b));
    if let Expr::Spawn { body, .. } = expr {
        names_in(parsed, body, out);
    }
    for block in blocks {
        names_in_block(parsed, block, out);
    }
}

fn names_in_block(parsed: &Parsed, block: &Block, out: &mut BTreeSet<String>) {
    for stmt in &block.stmts {
        super::sync::visit_stmt(parsed, &stmt.node, &mut |e| {
            if let Expr::Variable(name) = e {
                out.insert(parsed.text(*name).to_string());
            }
        });
        super::sync::visit_stmt_blocks(&stmt.node, &mut |b| names_in_block(parsed, b, out));
    }
}

/// From what the walk found to what the lowering writes (D2-D5), and the
/// refusals where it cannot.
fn decide(
    parsed: &Parsed,
    item: &crate::ast::Spanned<Item>,
    key: String,
    walk: Walk<'_>,
    context: &Context,
) -> Plan {
    let Item::Fn { body, .. } = &item.node else {
        return Plan::default();
    };
    let mut plan = Plan {
        key: key.clone(),
        first: body.stmts.first().map(|s| s.span.start),
        ..Plan::default()
    };
    let mut by_source: BTreeMap<Id, BTreeSet<Escape>> = BTreeMap::new();
    for (source, escape) in &walk.escapes {
        by_source
            .entry(source.clone())
            .or_default()
            .insert(escape.clone());
    }

    for (id, (source, path)) in &walk.sources {
        let escapes = by_source.get(id).cloned().unwrap_or_default();
        for escape in &escapes {
            plan.escapes.push((source.clone(), escape.clone()));
        }
        let leaves = escapes
            .iter()
            .any(|e| matches!(e, Escape::Result | Escape::Param(_)));
        let task = escapes.contains(&Escape::Task);
        let outer: Vec<&String> = escapes
            .iter()
            .filter_map(|e| match e {
                Escape::Outer(r) => Some(r),
                _ => None,
            })
            .collect();

        // **A keeper that drops entries across a loop** (D4): the buffer is
        // read inside a loop the keeper is declared outside of.
        let shedding: Vec<&String> = outer
            .iter()
            .copied()
            .filter(|r| walk.sheds.contains(*r))
            .filter(|r| {
                let declared = walk_local_path(&walk, r);
                path.iter()
                    .any(|(at, looping)| *looping && !declared.contains(&(*at, true)))
            })
            .collect();

        let keep = if task && leaves {
            plan.refusals.push(refusal(
                source,
                "is handed both to a task and out of this function",
                "a task may outlive the caller that would keep the buffer, and the caller \
                 may outlive the task - there is no one owner for it",
                "hand the task a copy with `.clone()`, or give the task only what it \
                 needs and hand the rest back",
            ));
            continue;
        } else if task {
            KeepAt::Task
        } else if leaves {
            KeepAt::Param
        } else if !shedding.is_empty() {
            KeepAt::Element(source.at())
        } else if !outer.is_empty() {
            KeepAt::Frame
        } else {
            match source {
                // A buffer nothing keeps past its scope stays what it was.
                Source::Buffer { .. } => continue,
                // A call's buffers need a keep all the same; one declared
                // just before the statement lives exactly as long as the
                // value the call hands back.
                Source::Call { at, .. } => KeepAt::Local(*at),
            }
        };

        match keep {
            KeepAt::Param => plan.takes_keep = true,
            KeepAt::Frame => plan.frame_keep = true,
            KeepAt::Task => plan.task_keep = true,
            KeepAt::Local(at) => {
                plan.local_keeps.insert(at);
            }
            KeepAt::Element(_) => {
                for keeper in &shedding {
                    plan.element_keepers.insert((*keeper).clone());
                }
                for ((statement, into), by_arg) in &walk.puts_into {
                    if !shedding.contains(&into) {
                        continue;
                    }
                    for (arg, origins) in by_arg {
                        if origins.contains(id) {
                            plan.holds
                                .entry((*statement, into.clone()))
                                .or_default()
                                .insert(*arg, source.at());
                        }
                    }
                }
            }
        }
        match source {
            Source::Buffer { at, .. } => {
                plan.puts.insert(*at, keep);
            }
            Source::Call { at, callee, .. } => {
                plan.calls.insert((*at, callee.clone()), keep);
            }
        }
        if keep == KeepAt::Task {
            for (name, (at, origins)) in &walk.captured {
                if origins.contains(id) {
                    plan.tethered.insert(name.clone(), *at);
                }
            }
        }
    }

    // **No permission is asked for** ([ADR-209](../../../../docs/specification/adr/adr-209.md)
    // D5, withdrawing [ADR-201](../../../../docs/specification/adr/adr-201.md)
    // D2): which buffer lives where is the compiler's decision, like which
    // count a `Shared` gets, and it is shown by `--tethers` and in the ledger
    // rather than demanded of the source. A word the program had to write for
    // something the compiler already knows is the bookkeeping this language
    // exists to take away.
    for refusal in walk_refusals(&plan, &walk, parsed, context) {
        plan.refusals.push(refusal);
    }
    plan
}

/// The refusals that are about the lowering rather than the permission.
fn walk_refusals(
    plan: &Plan,
    walk: &Walk<'_>,
    parsed: &Parsed,
    context: &Context,
) -> Vec<crate::check::Finding> {
    let mut out = Vec::new();
    // D4 holds **text**: a container of `ref String` becomes a container of
    // held text. A keeper whose element is a struct would need its every
    // reader rewritten, and that is said rather than guessed at.
    for keeper in &plan.element_keepers {
        // **An unwritten type is not asked for**: the language below reads
        // the held views off what goes in, as it reads any other element type.
        let Some(ty) = walk_local_type(walk, keeper) else {
            continue;
        };
        if !only_text_views(parsed, &ty, context) {
            out.push(element_refusal(
                keeper,
                "it holds views inside a struct, and only text is held one view at a time",
                "keep text (`ref String`) in it, or copy what it keeps with `.clone()`",
                plan,
                walk,
            ));
        }
    }
    out
}

fn element_refusal(
    keeper: &str,
    why: &str,
    help: &str,
    plan: &Plan,
    _walk: &Walk<'_>,
) -> crate::check::Finding {
    let span = plan
        .holds
        .keys()
        .find(|(_, k)| k == keeper)
        .map(|(at, _)| *at..*at)
        .unwrap_or(0..0);
    crate::check::Finding {
        severity: crate::check::Severity::Error,
        span,
        code: "NK2304",
        message: format!(
            "`{keeper}` keeps views of buffers read inside a loop and drops entries as it \
             goes, and {why}"
        ),
        notes: vec![
            "one keep for the whole loop would keep every buffer the loop ever read, including \
             the ones whose entries are gone - so each view carries its own handle on its \
             buffer instead (ADR-209 D4)"
                .to_string(),
        ],
        help: Some(help.to_string()),
    }
}

fn refusal(source: &Source, what: &str, why: &str, help: &str) -> crate::check::Finding {
    crate::check::Finding {
        severity: crate::check::Severity::Error,
        span: source.span().clone(),
        code: "NK2304",
        message: format!("{} {what}", capitalised(&source.named())),
        notes: vec![why.to_string()],
        help: Some(help.to_string()),
    }
}

fn capitalised(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

/// Whether every view in a type is bare text, the one shape D4 holds.
fn only_text_views(parsed: &Parsed, ty: &Type, context: &Context) -> bool {
    if context.borrowing.contains(parsed.text(ty.name)) {
        return false;
    }
    ty.generics
        .iter()
        .all(|g| only_text_views(parsed, g, context))
}

fn walk_local_path(walk: &Walk<'_>, name: &str) -> Vec<(usize, bool)> {
    walk.last_seen
        .get(name)
        .map(|l| l.path.clone())
        .unwrap_or_default()
}

fn walk_local_type(walk: &Walk<'_>, name: &str) -> Option<Type> {
    walk.last_seen.get(name).and_then(|l| l.ty.clone())
}
