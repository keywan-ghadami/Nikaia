//! **What a `String` field or result is below, decided by what flows into it**
//! ([ADR-222](../../../docs/specification/adr/adr-222.md)).
//!
//! [ADR-107](../../../docs/specification/adr/adr-107.md) D1 made `String` the
//! one text type and its state - borrowed, tethered, owned - the compiler's,
//! *the cheapest state that works, chosen per use*. A struct has one
//! representation below, so for a field "per use" means: per field, by every
//! value any line of the package puts into it. The same for what a function
//! declared `-> String` hands back. Three tiers, and each pays only for itself:
//!
//! * **only text of its own** flows in - the ordinary case: it stays `String`,
//!   and nothing about it changes;
//! * **only views** (and literals, which are views of text that lives as long
//!   as the program): it becomes `ref String`, exactly as if the program had
//!   written that, and [ADR-209](../../../docs/specification/adr/adr-209.md)
//!   decides where each buffer lives - in the caller's frame for nothing, with
//!   a handle only where no frame outlives it;
//! * **both**: it becomes `ref String` marked [`crate::ast::Type::either`],
//!   which is `nikaia_std::either_text::EitherText` below. A view is borrowed
//!   and text of its own is owned, **each at its own line**, so no line pays for
//!   another line's kind of text.
//!
//! A literal alone moves nothing: a field only literals and owned text flow
//! into stays `String`, and the literal is built where it stands as before
//! ([ADR-207](../../../docs/specification/adr/adr-207.md) D2).
//!
//! **A `pub` field of a `pub` struct, and a `pub` function's result, may become
//! a view but never both kinds.** Their representation is published with the
//! package, before the programs that use it exist; a view the package itself
//! puts there is in its ledger as `ref String`, and a mixed one would need a
//! type no other package can know to build. There the view is refused as
//! before, saying why ([ADR-208](../../../docs/specification/adr/adr-208.md) D2).
//!
//! Run once, right after parsing, so that the checker, every derived column,
//! the ledger and the emitter all read one answer off the types.

use std::collections::{BTreeMap, BTreeSet};

use crate::ast::{Block, Expr, Item, Program, Stmt, Type};
use crate::parser::Parsed;

/// What a value of text is, as far as where it lives goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Kind {
    /// Text of its own: `f"…"`, `.clone()`, a `String` handed over.
    Owned,
    /// A view into text something else owns.
    View,
    /// A literal, or a view of one: lives as long as the program.
    Static,
    /// What a mixed position hands back: either of the first two, per value.
    Either,
}

/// What a set of kinds makes of a position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tier {
    Owned,
    View,
    Mixed,
}

fn tier(kinds: &BTreeSet<Kind>) -> Tier {
    let viewed = kinds.contains(&Kind::View);
    let owned = kinds.contains(&Kind::Owned);
    match (kinds.contains(&Kind::Either), viewed, owned) {
        (true, _, _) | (false, true, true) => Tier::Mixed,
        (false, true, false) => Tier::View,
        _ => Tier::Owned,
    }
}

/// Methods whose result is text of its own, whatever they are called on.
const OWNS: &[&str] = &[
    "clone",
    "to_owned",
    "to_string",
    "clone_text",
    "to_uppercase",
    "to_lowercase",
    "repeat",
    "replace",
    "join",
    "concat",
];

/// Methods of text whose result is a view **of the text they are called on**.
const VIEWS: &[&str] = &[
    "trim",
    "trim_start",
    "trim_end",
    "trim_matches",
    "trim_start_matches",
    "trim_end_matches",
    "strip_prefix",
    "strip_suffix",
    "lines",
    "split",
    "split_whitespace",
    "splitn",
    "rsplit",
    "rsplitn",
    "split_terminator",
    "matches",
    "as_str",
];

/// Methods whose result is an element of what they are called on, so it is
/// the same kind of text as the elements are.
const ELEMENTS: &[&str] = &[
    "collect", "next", "unwrap", "first", "last", "get", "iter", "nth", "peek",
];

/// Every `String` field and result of the program, by what flows into it.
#[derive(Debug, Default)]
struct Flows {
    fields: BTreeMap<(String, String), BTreeSet<Kind>>,
    results: BTreeMap<String, BTreeSet<Kind>>,
    /// A `let` binding a name to a literal, by the byte it stands at, and the
    /// positions that name is kept in (ADR-222 D4).
    literal_lets: BTreeMap<usize, BTreeSet<Position>>,
}

/// A field or a result.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Position {
    Field(String, String),
    Result(String),
}

/// The positions the tiers are decided for, and what the program declared of
/// each.
struct Declared<'p> {
    parsed: &'p Parsed,
    /// `(struct, field)` for every field declared `String`, and whether the
    /// field is published.
    fields: BTreeMap<(String, String), bool>,
    /// Every field of every struct, for reading a field's declared type.
    all_fields: BTreeMap<(String, String), Type>,
    /// A function key for every function or method declared `-> String`, and
    /// whether its result is published.
    results: BTreeMap<String, bool>,
    /// Every function's declared result, by key.
    all_results: BTreeMap<String, Option<Type>>,
    /// Method names declared in this program, by name, to the keys that
    /// declare them.
    methods: BTreeMap<String, Vec<String>>,
}

fn plain_string(parsed: &Parsed, ty: &Type) -> bool {
    parsed.text(ty.name) == "String"
        && ty.generics.is_empty()
        && !ty.is_view
        && !ty.is_nullable
        && !ty.is_tuple
        && !ty.is_slice
        && ty.code.is_none()
}

impl<'p> Declared<'p> {
    fn of(parsed: &'p Parsed) -> Declared<'p> {
        let mut declared = Declared {
            parsed,
            fields: BTreeMap::new(),
            all_fields: BTreeMap::new(),
            results: BTreeMap::new(),
            all_results: BTreeMap::new(),
            methods: BTreeMap::new(),
        };
        for item in &parsed.program.items {
            match &item.node {
                Item::Struct {
                    name,
                    fields,
                    is_public,
                    ..
                } => {
                    let owner = parsed.text(*name).to_string();
                    for field in fields {
                        let key = (owner.clone(), parsed.text(field.name).to_string());
                        if plain_string(parsed, &field.ty) {
                            declared
                                .fields
                                .insert(key.clone(), *is_public && field.is_public);
                        }
                        declared.all_fields.insert(key, field.ty.clone());
                    }
                }
                Item::Fn { .. } => declared.function(item, None),
                Item::Impl {
                    target, methods, ..
                } => {
                    let target = parsed.text(target.name).to_string();
                    for method in methods {
                        declared.function(method, Some(&target));
                    }
                }
                _ => {}
            }
        }
        declared
    }

    fn function(&mut self, item: &crate::ast::Spanned<Item>, target: Option<&str>) {
        let Item::Fn {
            name,
            ret_type,
            is_public,
            ..
        } = &item.node
        else {
            return;
        };
        let own = name
            .map(|n| self.parsed.text(n).to_string())
            .unwrap_or_else(|| "new".to_string());
        let key = key(target, &own);
        if target.is_some() {
            self.methods.entry(own).or_default().push(key.clone());
        }
        if ret_type
            .as_ref()
            .is_some_and(|t| plain_string(self.parsed, t))
        {
            self.results.insert(key.clone(), *is_public);
        }
        self.all_results.insert(key, ret_type.clone());
    }
}

impl Declared<'_> {
    /// A written `String`, to declare a binding with: the one a field or a
    /// result of this program already names.
    fn string_type(&self) -> Option<Type> {
        let string = |ty: &Type| plain_string(self.parsed, ty).then(|| ty.clone());
        self.all_fields
            .values()
            .find_map(string)
            .or_else(|| self.all_results.values().flatten().find_map(string))
    }
}

fn key(target: Option<&str>, own: &str) -> String {
    match target {
        Some(target) => format!("{target}::{own}"),
        None => own.to_string(),
    }
}

/// One local, as far as this walk needs it.
#[derive(Debug, Clone, Default)]
struct Local {
    kinds: BTreeSet<Kind>,
    /// The struct it is a value of, where that can be read off the program.
    of: Option<String>,
    /// Bound by a `let` to a literal and nothing else: the byte the `let`
    /// stands at.
    literal: Option<usize>,
}

struct Walk<'d, 'p> {
    declared: &'d Declared<'p>,
    /// The tiers found so far, which reading a field or calling a function
    /// asks.
    tiers: &'d Tiers,
    flows: &'d mut Flows,
    scopes: Vec<BTreeMap<String, Local>>,
    /// The function being walked, where its result is a position.
    result: Option<String>,
}

/// The tiers decided so far.
#[derive(Debug, Default, Clone, PartialEq)]
struct Tiers {
    fields: BTreeMap<(String, String), Tier>,
    results: BTreeMap<String, Tier>,
}

impl Tiers {
    fn of(declared: &Declared<'_>, flows: &Flows) -> Tiers {
        let decide = |kinds: Option<&BTreeSet<Kind>>, published: bool| match (
            kinds.map(tier).unwrap_or(Tier::Owned),
            published,
        ) {
            (Tier::Mixed, true) => Tier::Owned,
            (tier, _) => tier,
        };
        Tiers {
            fields: declared
                .fields
                .iter()
                .map(|(key, published)| (key.clone(), decide(flows.fields.get(key), *published)))
                .collect(),
            results: declared
                .results
                .iter()
                .map(|(key, published)| (key.clone(), decide(flows.results.get(key), *published)))
                .collect(),
        }
    }
}

impl Walk<'_, '_> {
    fn text(&self, sym: winnow_grammar::Symbol) -> &str {
        self.declared.parsed.text(sym)
    }

    fn bind(&mut self, name: String, local: Local) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name, local);
        }
    }

    fn local(&self, name: &str) -> Option<&Local> {
        self.scopes.iter().rev().find_map(|s| s.get(name))
    }

    fn block(&mut self, block: &Block) -> BTreeSet<Kind> {
        self.scopes.push(BTreeMap::new());
        let mut tail = BTreeSet::new();
        for (i, stmt) in block.stmts.iter().enumerate() {
            let last = i + 1 == block.stmts.len();
            let kinds = self.stmt(&stmt.node, stmt.span.start);
            if last {
                tail = kinds;
            }
        }
        self.scopes.pop();
        tail
    }

    /// Walk a statement; what it is worth as a block's last value.
    fn stmt(&mut self, stmt: &Stmt, at: usize) -> BTreeSet<Kind> {
        match stmt {
            Stmt::Let {
                names, value, ty, ..
            } => {
                let kinds = self.expr(value);
                let of = ty
                    .as_ref()
                    .map(|t| self.text(t.name).to_string())
                    .or_else(|| self.struct_of(value));
                let literal = (ty.is_none() && matches!(value, Expr::LitStr { .. })).then_some(at);
                if let [name] = names.as_slice() {
                    let name = self.text(*name).to_string();
                    self.bind(name, Local { kinds, of, literal });
                }
                BTreeSet::new()
            }
            Stmt::Assign { target, value, .. } => {
                let kinds = self.expr(value);
                match target {
                    Expr::Field { base, name } => {
                        if let Some(owner) = self.struct_of(base) {
                            let field = self.text(*name).to_string();
                            self.flow_field(owner, field, &kinds);
                        }
                    }
                    Expr::Variable(name) => {
                        let name = self.text(*name).to_string();
                        if let Some(local) =
                            self.scopes.iter_mut().rev().find_map(|s| s.get_mut(&name))
                        {
                            local.kinds.extend(kinds);
                            // Assigned again: no longer the literal it was bound to.
                            local.literal = None;
                        }
                    }
                    _ => {}
                }
                BTreeSet::new()
            }
            Stmt::For {
                bindings,
                iter,
                body,
            } => {
                let kinds = self.expr(iter);
                self.scopes.push(BTreeMap::new());
                for binding in bindings {
                    let name = self.text(*binding).to_string();
                    self.bind(
                        name,
                        Local {
                            kinds: kinds.clone(),
                            of: None,
                            literal: None,
                        },
                    );
                }
                self.block(body);
                self.scopes.pop();
                BTreeSet::new()
            }
            Stmt::While { cond, body } => {
                self.expr(cond);
                self.block(body);
                BTreeSet::new()
            }
            Stmt::Return(Some(value)) => {
                let kinds = self.expr(value);
                self.flow_result(&kinds);
                self.kept_literal(value, None);
                BTreeSet::new()
            }
            Stmt::Expr(expr) => self.expr(expr),
            _ => BTreeSet::new(),
        }
    }

    fn flow_field(&mut self, owner: String, field: String, kinds: &BTreeSet<Kind>) {
        let key = (owner, field);
        if self.declared.fields.contains_key(&key) {
            self.flows
                .fields
                .entry(key)
                .or_default()
                .extend(kinds.iter().copied());
        }
    }

    fn flow_result(&mut self, kinds: &BTreeSet<Kind>) {
        if let Some(result) = &self.result
            && self.declared.results.contains_key(result)
        {
            self.flows
                .results
                .entry(result.clone())
                .or_default()
                .extend(kinds.iter().copied());
        }
    }

    /// A name bound to a literal, kept in a field or handed back: remembered
    /// with the position, so that where the position stays `String` the
    /// binding is declared `String` (ADR-222 D4). `None` for the function's
    /// result.
    fn kept_literal(&mut self, value: &Expr, field: Option<(String, String)>) {
        let Expr::Variable(name) = value else {
            return;
        };
        let Some(at) = self.local(self.text(*name)).and_then(|l| l.literal) else {
            return;
        };
        let position = match field {
            Some((owner, field)) => Position::Field(owner, field),
            None => match &self.result {
                Some(result) => Position::Result(result.clone()),
                None => return,
            },
        };
        self.flows
            .literal_lets
            .entry(at)
            .or_default()
            .insert(position);
    }

    /// The struct a value is of, where the program says so without types: a
    /// literal of it, a name bound to one, `self` in its `impl`.
    fn struct_of(&self, expr: &Expr) -> Option<String> {
        match expr {
            Expr::StructLit { name, .. } => Some(self.text(*name).to_string()),
            Expr::Variable(name) => self.local(self.text(*name)).and_then(|l| l.of.clone()),
            Expr::Unary { expr, .. } | Expr::Try(expr) => self.struct_of(expr),
            _ => None,
        }
    }

    /// Walk an expression, recording what flows into the positions inside it;
    /// what kind of text its value is.
    fn expr(&mut self, expr: &Expr) -> BTreeSet<Kind> {
        let one = |kind| BTreeSet::from([kind]);
        match expr {
            Expr::LitStr { .. } => one(Kind::Static),
            Expr::LitInterpolated(_) => one(Kind::Owned),
            Expr::Variable(name) => self
                .local(self.text(*name))
                .map(|l| l.kinds.clone())
                .unwrap_or_else(|| one(Kind::Owned)),
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
                let received = self.expr(receiver);
                for arg in args {
                    self.expr(arg);
                }
                let method = self.text(*method).to_string();
                // **A method this program declares answers for itself**, before
                // any name in the lists below: a `split` of its own that hands
                // back text of its own is not a view.
                if let Some(keys) = self.declared.methods.get(&method) {
                    return keys.iter().flat_map(|key| self.result_kinds(key)).collect();
                }
                if OWNS.contains(&method.as_str()) {
                    return one(Kind::Owned);
                }
                if VIEWS.contains(&method.as_str()) {
                    return viewed(&received);
                }
                if ELEMENTS.contains(&method.as_str()) {
                    return received;
                }
                one(Kind::Owned)
            }
            Expr::Call { func, args, .. } => {
                for arg in args {
                    self.expr(arg);
                }
                let callee = crate::contracts::keep::callee_name(self.declared.parsed, func);
                match callee {
                    Some(callee) if self.declared.all_results.contains_key(&callee) => {
                        self.result_kinds(&callee)
                    }
                    Some(callee) => std_result(&callee),
                    None => one(Kind::Owned),
                }
            }
            Expr::Field { base, name } | Expr::SafeField { base, name } => {
                let received = self.expr(base);
                let Some(owner) = self.struct_of(base) else {
                    // A field of something this walk cannot name: its declared
                    // type is what anybody reading the field gets.
                    let _ = received;
                    return one(Kind::Owned);
                };
                let key = (owner, self.text(*name).to_string());
                match self.tiers.fields.get(&key) {
                    Some(Tier::View) => one(Kind::View),
                    Some(Tier::Mixed) => one(Kind::Either),
                    Some(Tier::Owned) => one(Kind::Owned),
                    None => match self.declared.all_fields.get(&key) {
                        Some(ty) if ty.is_view => one(Kind::View),
                        _ => one(Kind::Owned),
                    },
                }
            }
            Expr::Index { base, index } => {
                let received = self.expr(base);
                self.expr(index);
                match index.as_ref() {
                    Expr::Range { .. } => viewed(&received),
                    _ => received,
                }
            }
            Expr::StructLit { name, fields } => {
                let owner = self.text(*name).to_string();
                for field in fields {
                    let shorthand = Expr::Variable(field.name);
                    let value = field.value.as_ref().unwrap_or(&shorthand);
                    let kinds = self.expr(value);
                    let field = self.text(field.name).to_string();
                    self.flow_field(owner.clone(), field.clone(), &kinds);
                    self.kept_literal(value, Some((owner.clone(), field)));
                }
                one(Kind::Owned)
            }
            Expr::With { base, fields, .. } => {
                self.expr(base);
                let owner = self.struct_of(base);
                for field in fields {
                    let Some(value) = &field.value else { continue };
                    let kinds = self.expr(value);
                    if let Some(owner) = &owner {
                        let field = self.text(field.name).to_string();
                        self.flow_field(owner.clone(), field, &kinds);
                    }
                }
                one(Kind::Owned)
            }
            Expr::If {
                cond,
                then_branch,
                else_branch,
            } => {
                self.expr(cond);
                let mut out = self.block(then_branch);
                if let Some(block) = else_branch {
                    out.extend(self.block(block));
                }
                out
            }
            Expr::Match { value, arms } => {
                self.expr(value);
                let mut out = BTreeSet::new();
                for arm in arms {
                    if let Some(guard) = &arm.guard {
                        self.expr(guard);
                    }
                    out.extend(self.expr(&arm.body));
                }
                out
            }
            Expr::Block(block) | Expr::Unsafe(block) | Expr::Overlap(block) => self.block(block),
            Expr::Closure { body, .. } => {
                // A lambda's `return` is the lambda's, not the function's.
                let result = self.result.take();
                self.block(body);
                self.result = result;
                one(Kind::Owned)
            }
            Expr::Spawn { body, .. } => {
                let result = self.result.take();
                self.expr(body);
                self.result = result;
                one(Kind::Owned)
            }
            Expr::Coalesce { value, fallback } => {
                let mut out = self.expr(value);
                out.extend(self.expr(fallback));
                out
            }
            Expr::TryCatch { expr, handler } => {
                let mut out = self.expr(expr);
                out.extend(self.block(handler));
                out
            }
            Expr::Try(inner) | Expr::Unary { expr: inner, .. } => self.expr(inner),
            Expr::Throw(inner) => {
                self.expr(inner);
                BTreeSet::new()
            }
            Expr::Return(value) => {
                if let Some(value) = value {
                    let kinds = self.expr(value);
                    self.flow_result(&kinds);
                    self.kept_literal(value, None);
                }
                BTreeSet::new()
            }
            Expr::Binary { lhs, rhs, .. } => {
                self.expr(lhs);
                self.expr(rhs);
                one(Kind::Owned)
            }
            Expr::Cast { expr, .. } => {
                self.expr(expr);
                one(Kind::Owned)
            }
            Expr::Tuple(items) | Expr::ListLit { items, .. } => {
                let mut out = BTreeSet::new();
                for item in items {
                    out.extend(self.expr(item));
                }
                if out.is_empty() {
                    out.insert(Kind::Owned);
                }
                out
            }
            Expr::Range { start, end, .. } => {
                self.expr(start);
                self.expr(end);
                one(Kind::Owned)
            }
            _ => one(Kind::Owned),
        }
    }

    /// What a function of this program hands back.
    fn result_kinds(&self, key: &str) -> BTreeSet<Kind> {
        let one = |kind| BTreeSet::from([kind]);
        match self.tiers.results.get(key) {
            Some(Tier::View) => one(Kind::View),
            Some(Tier::Mixed) => one(Kind::Either),
            Some(Tier::Owned) => one(Kind::Owned),
            None => match self.declared.all_results.get(key) {
                Some(Some(ty)) if ty.is_view => one(Kind::View),
                _ => one(Kind::Owned),
            },
        }
    }
}

/// A view of text of these kinds: a view of a literal is as lasting as the
/// literal, and a view of anything else is a view.
fn viewed(received: &BTreeSet<Kind>) -> BTreeSet<Kind> {
    match received.iter().all(|k| *k == Kind::Static) && !received.is_empty() {
        true => BTreeSet::from([Kind::Static]),
        false => BTreeSet::from([Kind::View]),
    }
}

/// What a function of `std` hands back, off its ledger entry.
fn std_result(callee: &str) -> BTreeSet<Kind> {
    static STD: std::sync::OnceLock<Option<crate::contracts::Ledger>> = std::sync::OnceLock::new();
    let ledger = STD.get_or_init(|| crate::contracts::Ledger::parse(crate::contracts::STD).ok());
    let view = ledger
        .as_ref()
        .and_then(|l| l.lookup(callee))
        .and_then(|(_, c)| c.signature.as_ref())
        .and_then(|s| s.result.as_ref())
        .is_some_and(|r| r.text() == "ref String");
    match view {
        true => BTreeSet::from([Kind::View]),
        false => BTreeSet::from([Kind::Owned]),
    }
}

/// Walk every function of the program once against the tiers found so far.
fn walk(declared: &Declared<'_>, tiers: &Tiers) -> Flows {
    let parsed = declared.parsed;
    let mut flows = Flows::default();
    let function = |item: &crate::ast::Spanned<Item>, target: Option<&str>, flows: &mut Flows| {
        let Item::Fn {
            name,
            receiver,
            args,
            body,
            ..
        } = &item.node
        else {
            return;
        };
        let own = name
            .map(|n| parsed.text(n).to_string())
            .unwrap_or_else(|| "new".to_string());
        let mut frame = BTreeMap::new();
        for arg in args {
            let kind = match arg.ty.is_view {
                true => Kind::View,
                false => Kind::Owned,
            };
            frame.insert(
                parsed.text(arg.name).to_string(),
                Local {
                    kinds: BTreeSet::from([kind]),
                    of: Some(parsed.text(arg.ty.name).to_string()),
                    literal: None,
                },
            );
        }
        if receiver.is_some()
            && let Some(target) = target
        {
            frame.insert(
                "self".to_string(),
                Local {
                    kinds: BTreeSet::from([Kind::Owned]),
                    of: Some(target.to_string()),
                    literal: None,
                },
            );
        }
        let mut walk = Walk {
            declared,
            tiers,
            flows,
            scopes: vec![frame],
            result: Some(key(target, &own)),
        };
        let tail = walk.block(body);
        // The body's last value is what it hands back.
        if let Some(Stmt::Expr(_)) = body.stmts.last().map(|s| &s.node) {
            walk.flow_result(&tail);
        }
    };
    for item in &parsed.program.items {
        match &item.node {
            Item::Fn { .. } => function(item, None, &mut flows),
            Item::Impl {
                target, methods, ..
            } => {
                let target = parsed.text(target.name).to_string();
                for method in methods {
                    function(method, Some(&target), &mut flows);
                }
            }
            _ => {}
        }
    }
    flows
}

/// **Decide the tiers and write them into the types**, so that every later
/// reader of the program reads one answer.
pub fn refine(parsed: &mut Parsed) {
    let declared = Declared::of(parsed);
    if declared.fields.is_empty() && declared.results.is_empty() {
        return;
    }
    // A fixpoint: a field read or a call hands on the tier found for it, and
    // tiers only ever move up (owned, view, mixed), so it ends.
    let mut tiers = Tiers::of(&declared, &Flows::default());
    let mut flows = Flows::default();
    for _ in 0..16 {
        flows = walk(&declared, &tiers);
        let next = Tiers::of(&declared, &flows);
        if next == tiers {
            break;
        }
        tiers = next;
    }
    // **A name bound to a literal and kept where text of its own is wanted is
    // declared `String`** (ADR-222 D4): the literal is built into text once,
    // where it is bound, which is what the program would have written.
    let owned = |position: &Position| match position {
        Position::Field(owner, field) => {
            tiers.fields.get(&(owner.clone(), field.clone())) == Some(&Tier::Owned)
        }
        Position::Result(key) => tiers.results.get(key) == Some(&Tier::Owned),
    };
    let lets: BTreeSet<usize> = flows
        .literal_lets
        .iter()
        .filter(|(_, positions)| positions.iter().any(owned))
        .map(|(at, _)| *at)
        .collect();
    let text = declared.string_type();
    let said = said(&tiers);
    rewrite(parsed, &tiers, &lets, text);
    parsed.text_tiers = said;
}

fn mark(ty: &mut Type, tier: Tier) {
    match tier {
        Tier::Owned => {}
        Tier::View => ty.is_view = true,
        Tier::Mixed => {
            ty.is_view = true;
            ty.either = true;
        }
    }
}

fn rewrite(parsed: &mut Parsed, tiers: &Tiers, lets: &BTreeSet<usize>, text_type: Option<Type>) {
    // The names first, while the program is only read.
    let text = |sym| parsed.text(sym).to_string();
    let mut plan: Vec<(usize, usize, Tier)> = Vec::new();
    let mut fns: Vec<(usize, Option<usize>, Tier)> = Vec::new();
    for (i, item) in parsed.program.items.iter().enumerate() {
        match &item.node {
            Item::Struct { name, fields, .. } => {
                let owner = text(*name);
                for (j, field) in fields.iter().enumerate() {
                    if let Some(tier) = tiers.fields.get(&(owner.clone(), text(field.name))) {
                        plan.push((i, j, *tier));
                    }
                }
            }
            Item::Fn { name, .. } => {
                let own = name.map(text).unwrap_or_else(|| "new".to_string());
                if let Some(tier) = tiers.results.get(&own) {
                    fns.push((i, None, *tier));
                }
            }
            Item::Impl {
                target, methods, ..
            } => {
                let target = text(target.name);
                for (j, method) in methods.iter().enumerate() {
                    if let Item::Fn { name, .. } = &method.node {
                        let own = name.map(text).unwrap_or_else(|| "new".to_string());
                        if let Some(tier) = tiers.results.get(&key(Some(&target), &own)) {
                            fns.push((i, Some(j), *tier));
                        }
                    }
                }
            }
            _ => {}
        }
    }
    let program: &mut Program = &mut parsed.program;
    if let Some(text_type) = text_type
        && !lets.is_empty()
    {
        for item in &mut program.items {
            annotate_item(&mut item.node, lets, &text_type);
        }
    }
    for (i, j, tier) in plan {
        if let Item::Struct { fields, .. } = &mut program.items[i].node {
            mark(&mut fields[j].ty, tier);
        }
    }
    for (i, j, tier) in fns {
        let item = match j {
            None => &mut program.items[i].node,
            Some(j) => match &mut program.items[i].node {
                Item::Impl { methods, .. } => &mut methods[j].node,
                _ => continue,
            },
        };
        if let Item::Fn {
            ret_type: Some(ty), ..
        } = item
        {
            mark(ty, tier);
        }
    }
}

fn annotate_item(item: &mut Item, lets: &BTreeSet<usize>, text: &Type) {
    match item {
        Item::Fn { body, .. } => annotate_block(body, lets, text),
        Item::Impl { methods, .. } => {
            for method in methods {
                annotate_item(&mut method.node, lets, text);
            }
        }
        _ => {}
    }
}

fn annotate_block(block: &mut Block, lets: &BTreeSet<usize>, text: &Type) {
    for stmt in &mut block.stmts {
        let at = stmt.span.start;
        match &mut stmt.node {
            Stmt::Let { ty, value, .. } => {
                if ty.is_none() && lets.contains(&at) {
                    *ty = Some(text.clone());
                }
                annotate_expr(value, lets, text);
            }
            Stmt::For { body, .. } | Stmt::While { body, .. } => annotate_block(body, lets, text),
            Stmt::Expr(expr) | Stmt::Return(Some(expr)) | Stmt::Assign { value: expr, .. } => {
                annotate_expr(expr, lets, text)
            }
            _ => {}
        }
    }
}

/// The blocks inside an expression, for the `let`s in them.
fn annotate_expr(expr: &mut Expr, lets: &BTreeSet<usize>, text: &Type) {
    match expr {
        Expr::If {
            then_branch,
            else_branch,
            ..
        } => {
            annotate_block(then_branch, lets, text);
            if let Some(block) = else_branch {
                annotate_block(block, lets, text);
            }
        }
        Expr::Match { arms, .. } => {
            for arm in arms {
                annotate_expr(&mut arm.body, lets, text);
            }
        }
        Expr::Block(block)
        | Expr::Unsafe(block)
        | Expr::Overlap(block)
        | Expr::Closure { body: block, .. } => annotate_block(block, lets, text),
        Expr::TryCatch { expr, handler } => {
            annotate_expr(expr, lets, text);
            annotate_block(handler, lets, text);
        }
        Expr::Spawn { body, .. } => annotate_expr(body, lets, text),
        _ => {}
    }
}

/// The tiers that are not text of its own, in the words `--tethers` uses.
fn said(tiers: &Tiers) -> Vec<String> {
    let words = |tier: Tier| match tier {
        Tier::View => Some("a view: only views and literals flow into it"),
        Tier::Mixed => Some(
            "a view or text of its own, per value: both flow into it, and each is kept as it is",
        ),
        Tier::Owned => None,
    };
    let fields = tiers.fields.iter().filter_map(|((owner, field), tier)| {
        words(*tier).map(|w| format!("`{owner}.{field}` is {w}"))
    });
    let results = tiers
        .results
        .iter()
        .filter_map(|(key, tier)| words(*tier).map(|w| format!("what `{key}` hands back is {w}")));
    fields.chain(results).collect()
}
