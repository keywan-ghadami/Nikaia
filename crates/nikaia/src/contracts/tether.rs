// crates/nikaia/src/contracts/tether.rs
//
// Which of Part I 6.6's three states each view in a signature is in
// ([ADR-008](../../../../docs/specification/adr/adr-008.md) D2, D7).
//
// ## What this is, and what it deliberately is not
//
// D2 gives every view one of three states and says the compiler picks the least
// one that makes the program valid:
//
//     Borrowed ⊑ Tethered ⊑ Owned
//     Transient = borrow. Escaping = tether. Copying = yours to ask for.
//
// **Only the first of those is built as a representation.** Borrowed is the
// language below's own lifetime and costs nothing; Owned is `.to_owned()`,
// written by the program; and **Tethered is not built at all** — where a value
// would tether, the program is refused today (`NK2302`, or the backend on the
// Nikaia line for a view of a local that escapes). So this file computes the
// state and **changes no lowering**. It writes a column, which is D7, and the
// column is what a later change reads when the representation exists.
//
// That order is deliberate: an analysis whose answers nothing depends on can be
// held against the whole corpus and read, and being wrong costs a wrong line in
// a file rather than a wrong program.
//
// ## Owned is never this file's word
//
// D5: *promotion to `Owned` is never automatic*. A `.to_owned()` hands back a
// `String`, which is not a view at all — so no **view** position is ever Owned,
// and the lattice this file solves over is the two states below it. The top of
// the lattice exists for the program to reach, not for the analysis to assign.
//
// ## Which way it errs
//
// **Towards Tethered**, which is D7's own polarity for the case it names: a
// state barrier *widens* to Tethered, *"the widest representation, never to
// Owned"*. A position this file cannot decide is therefore Tethered, and the
// cost of being wrong that way is a representation wider than it had to be —
// never a program that does the wrong thing. The other direction would tell a
// later reader that a value borrows when it escapes, which is the use-after-free
// the state exists to prevent.
//
// ## What is not here, and why
//
// **D7's state barrier is not applied.** *Crossing a `dyn` boundary or a
// published non-generic API is a state barrier* is a consequence of there being
// three representations and one ABI to choose; with one representation there is
// no barrier to cross. It belongs with the layouts rather than with the
// analysis, and marking every `pub` signature Tethered today would fill the
// column with a state nothing has.
//
// **The buffer table is not here either** (D4). Its shape is a fact about a
// container's representation, and this file writes no representation.

use std::collections::{BTreeMap, BTreeSet};

use crate::ast::{Item, Type};
use crate::emit::{borrowing_structs, holds_view};
use crate::parser::Parsed;

use super::Ledger;

/// The position a function's result occupies in the column.
///
/// The same spelling `sharing` uses for the same thing, because they are the
/// same position read by two analyses.
pub const RESULT: &str = "<result>";

/// One of Part I 6.6's states, as far as this analysis assigns them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum State {
    /// A plain reference into a buffer that outlives every use. Costs nothing.
    Borrowed,
    /// The value outlives the scope that owns its buffer, so the buffer has to
    /// be kept alive with it. **Not built** — a program that reaches this state
    /// is refused today.
    Tethered,
}

impl State {
    pub fn as_str(self) -> &'static str {
        match self {
            State::Borrowed => "borrowed",
            State::Tethered => "tethered",
        }
    }

    pub fn parse(text: &str) -> Option<State> {
        match text.trim() {
            "borrowed" => Some(State::Borrowed),
            "tethered" => Some(State::Tethered),
            _ => None,
        }
    }

    /// The lattice's join: the wider of the two.
    pub fn or(self, other: State) -> State {
        self.max(other)
    }
}

/// One view position in a signature, and the state it solved to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Held {
    /// A parameter's name, `self`, or [`RESULT`].
    pub position: String,
    pub state: State,
}

impl Held {
    /// `path: borrowed`, the shape a `sharing` class is written in.
    pub fn text(&self) -> String {
        format!("{}: {}", self.position, self.state.as_str())
    }

    pub fn parse(text: &str) -> Option<Held> {
        let (position, state) = text.rsplit_once(':')?;
        Some(Held {
            position: position.trim().to_string(),
            state: State::parse(state)?,
        })
    }
}

/// Give every entry of this package the states its views solved to (D7).
///
/// After the other inferences and reading none of them: what it asks is about
/// the **shape** of a signature and about which buffer a returned view came
/// from, and no column above answers either.
pub fn infer(ledger: &mut Ledger, units: &[&Parsed], library: &Ledger) {
    // Which struct or enum holds a view, per unit: a `Symbol` belongs to the
    // parse that interned it, so the set cannot be shared between files.
    let mut solved: BTreeMap<String, Vec<Held>> = BTreeMap::new();
    // A type whose instances a function hands back tethered. `D3`: all view
    // fields of an instance share one state, so a type has one answer.
    let mut tethering: BTreeSet<String> = BTreeSet::new();

    for parsed in units.iter().copied() {
        // By **name** rather than by `Symbol`, because a receiver is asked
        // about through its `impl`'s target, which is text here.
        let borrowing: BTreeSet<String> = borrowing_structs(parsed)
            .into_iter()
            .map(|s| parsed.text(s).to_string())
            .collect();
        let carries = |ty: &Type| carries_a_view(parsed, ty, &borrowing);
        let by_name = |name: &str| borrowing.contains(name);
        for item in &parsed.program.items {
            match &item.node {
                Item::Fn { .. } => {
                    if let Some((key, held, tethers)) = of(
                        parsed, &item.node, None, &carries, &by_name, ledger, library,
                    ) {
                        tethering.extend(tethers);
                        solved.insert(key, held);
                    }
                }
                Item::Impl {
                    target, methods, ..
                } => {
                    let target = parsed.text(target.name).to_string();
                    for method in methods {
                        if let Some((key, held, tethers)) = of(
                            parsed,
                            &method.node,
                            Some(&target),
                            &carries,
                            &by_name,
                            ledger,
                            library,
                        ) {
                            tethering.extend(tethers);
                            solved.insert(key, held);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    for (key, held) in solved {
        if let Some(contract) = ledger.functions.get_mut(&key) {
            contract.views = held;
        }
    }
    // **D7's second half is not written**, and that is the same line the module
    // header draws: *per struct: the solved state and the buffer-table shape*
    // is a fact about a **representation**, and the buffer table is named in the
    // same breath. A type already records which of its fields hold a view
    // (`tethered`); what state its instances take belongs with the layouts.
    let _ = tethering;
}

/// One function's positions, and the type names its result tethers.
fn of(
    parsed: &Parsed,
    item: &Item,
    target: Option<&str>,
    carries: &impl Fn(&Type) -> bool,
    carries_by_name: &impl Fn(&str) -> bool,
    here: &Ledger,
    library: &Ledger,
) -> Option<(String, Vec<Held>, Vec<String>)> {
    let Item::Fn {
        name,
        receiver,
        args,
        ret_type,
        body,
        ..
    } = item
    else {
        return None;
    };
    let own = match name {
        Some(name) => parsed.text(*name).to_string(),
        None => "new".to_string(),
    };
    let key = match target {
        Some(target) => format!("{target}::{own}"),
        None => own,
    };

    let mut held = Vec::new();
    // **A parameter is lent for the call**, so the buffer is the caller's and
    // outlives every use inside the body. Whether the body *keeps* it past the
    // call is `NK2302`'s question and a refusal rather than a state.
    // A receiver carries whatever its type does, and that is the `impl`'s
    // target — a view carrier exactly when the type is.
    if receiver.is_some() && target.is_some_and(carries_by_name) {
        held.push(Held {
            position: "self".to_string(),
            state: State::Borrowed,
        });
    }
    for arg in args {
        if carries(&arg.ty) {
            held.push(Held {
                position: parsed.text(arg.name).to_string(),
                state: State::Borrowed,
            });
        }
    }

    let mut tethers = Vec::new();
    if let Some(ret) = ret_type.as_ref().filter(|t| carries(t)) {
        // **Something to borrow from** is what decides it, and it is the same
        // question [ADR-008](../../../../docs/specification/adr/adr-008.md) D9
        // asks about the lifetime: where a receiver or a parameter carries a
        // view, the buffer is the caller's and the result borrows it. Where
        // nothing does, the view can only come from a local buffer or from
        // something that outlives the program.
        let borrows_from_a_caller = receiver.is_some() || args.iter().any(|a| carries(&a.ty));
        let buffer = match borrows_from_a_caller {
            true => Buffer::None,
            false => owns_a_buffer(parsed, body, here, library),
        };
        let state = match buffer.is_some() {
            true => State::Tethered,
            false => State::Borrowed,
        };
        if state == State::Tethered {
            tethers.extend(names_in(parsed, ret));
        }
        held.push(Held {
            position: RESULT.to_string(),
            state,
        });
    }

    (!held.is_empty()).then_some((key, held, tethers))
}

/// Whether this body **owns a buffer** a returned view could point into.
///
/// That is the whole of the question, once the caller's buffers are out of the
/// picture: a view handed back by a function with no view among its parameters
/// points either at something that outlives the program — `fn name() -> &str {
/// "Ada" }`, D9's own example — or at a buffer this body made. The second is
/// the escape, and the first is Borrowed.
///
/// **What counts as making one** is read off the ledger rather than from a list
/// of names: a `let` whose initialiser is a call whose **result** is owned text
/// or bytes. `fs::read_to_string` hands back a `String`, `fs::read` a
/// `Vec[u8]`, `fs::map` a `Mapped`. Beside that, `to_owned` and `to_string` by
/// name, which is the same kind of fact about the language below that
/// `is_length` records for `len`: all four entries of either name hand back
/// owned text.
///
/// **Unsure is *owns*,** which is the module header's polarity: an unresolved
/// call is a buffer this walk could not rule out, and the wider state is the
/// safe one.
fn owns_a_buffer(
    parsed: &Parsed,
    body: &crate::ast::Block,
    own: &Ledger,
    library: &Ledger,
) -> Buffer {
    let mut found = Buffer::None;
    bindings(body, &mut |value| {
        found = found
            .clone()
            .or(makes_a_buffer(parsed, value, own, library))
    });
    found
}

/// Whether a body makes a buffer, and whether this walk could **name** it.
///
/// The difference is what a refusal may be built on. A named buffer is a fact:
/// this body made a `String` and hands back a view of it. `Unknown` is a call
/// no ledger describes, which the **state** errs towards Tethered for — and a
/// refusal on that would refuse a correct program, which is the one thing this
/// compiler may never do
/// ([Part III C.4](../../../docs/specification/30-nikaia-tooling.md)). So the
/// column reads the two alike and a refusal reads only the first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Buffer {
    /// Nothing in this body owns text or bytes.
    None,
    /// This type, made here: `String`, `Bytes`, `Mapped`, a run of `u8`.
    Named(String),
    /// A call this walk could not resolve, which may be one.
    Unknown,
}

impl Buffer {
    /// The join, keeping the most that is known: a named buffer beats a doubt,
    /// because the doubt adds nothing once one is in hand.
    fn or(self, other: Buffer) -> Buffer {
        match (self, other) {
            (Buffer::Named(name), _) | (_, Buffer::Named(name)) => Buffer::Named(name),
            (Buffer::Unknown, _) | (_, Buffer::Unknown) => Buffer::Unknown,
            _ => Buffer::None,
        }
    }

    fn is_some(&self) -> bool {
        !matches!(self, Buffer::None)
    }
}

/// Every `let`'s initialiser in a body, the blocks inside it included.
fn bindings(block: &crate::ast::Block, f: &mut impl FnMut(&crate::ast::Expr)) {
    use crate::ast::Stmt;
    for stmt in &block.stmts {
        if let Stmt::Let { value, .. } = &stmt.node {
            f(value);
        }
        super::sync::visit_stmt_blocks(&stmt.node, &mut |inner| bindings(inner, f));
    }
}

/// Whether this expression hands back a buffer of its own.
fn makes_a_buffer(
    parsed: &Parsed,
    expr: &crate::ast::Expr,
    own: &Ledger,
    library: &Ledger,
) -> Buffer {
    use crate::ast::Expr;
    match expr {
        // `text.to_owned()` and `n.to_string()`: owned text by the name, which
        // every entry of either name agrees on.
        Expr::MethodCall { method, .. } => match parsed.text(*method) {
            "to_owned" | "to_string" => Buffer::Named("String".to_string()),
            _ => Buffer::None,
        },
        Expr::Call { func, .. } => {
            let name = match func.as_ref() {
                Expr::Variable(name) => parsed.text(*name).to_string(),
                Expr::Path(segments) => parsed.unaliased(
                    &segments
                        .iter()
                        .map(|s| parsed.text(*s))
                        .collect::<Vec<_>>()
                        .join("::"),
                ),
                _ => return Buffer::None,
            };
            let Some((_, contract)) = own
                .lookup(&name)
                .or_else(|| library.lookup(&name))
                .or_else(|| own.lookup(&format!("{name}::new")))
                .or_else(|| library.lookup(&format!("{name}::new")))
            else {
                // A call no ledger describes is a buffer this walk cannot rule
                // out, which is the safe direction for the **state** and not
                // one a refusal may stand on.
                return Buffer::Unknown;
            };
            match contract
                .signature
                .as_ref()
                .and_then(|s| s.result.as_ref())
                .filter(|ty| is_a_buffer(ty))
            {
                Some(ty) => Buffer::Named(ty.to_string()),
                None => Buffer::None,
            }
        }
        // `catch` hands back whichever half ran, so either may be the buffer.
        Expr::TryCatch { expr, .. } => makes_a_buffer(parsed, expr, own, library),
        _ => Buffer::None,
    }
}

/// Whether a type is owned text or bytes — something a view can point **into**.
///
/// `String`, `Bytes` and a mapped file are what a `&str` points into; a run of
/// `u8` is what a `&[u8]` does. A `Vec[Entry]` is not a buffer: nothing views
/// it, its elements are values.
///
/// **The one shape this does not decide** is a buffer built element by element
/// into a `Vec` whose element type the ledger does not name — `Vec()` hands
/// back `Vec[?]`, and reading that as a buffer would make every empty list in
/// every body one. No program in the tree builds a byte buffer that way and
/// then hands back a view into it; where one does, the backend is still what
/// refuses it, because nothing reads this column yet.
fn is_a_buffer(ty: &super::ty::Ty) -> bool {
    let super::ty::Ty::Named { name, args, .. } = ty else {
        return false;
    };
    match super::ty::base(name) {
        "String" | "Bytes" | "Mapped" => true,
        "Vec" | "List" => matches!(
            args.first(),
            Some(super::ty::Ty::Named { name, .. }) if super::ty::base(name) == "u8"
        ),
        _ => false,
    }
}

/// The type names a written type mentions, so that a result that tethers can
/// say which types it tethers.
fn names_in(parsed: &Parsed, ty: &Type) -> Vec<String> {
    let mut out = vec![parsed.text(ty.name).to_string()];
    for argument in &ty.generics {
        out.extend(names_in(parsed, argument));
    }
    out
}

/// Whether a written type **carries** a view: it is one, it names a type that
/// holds one, or one of its arguments does.
fn carries_a_view(parsed: &Parsed, ty: &Type, borrowing: &BTreeSet<String>) -> bool {
    holds_view(ty)
        || borrowing.contains(parsed.text(ty.name))
        || ty
            .generics
            .iter()
            .any(|g| carries_a_view(parsed, g, borrowing))
}

/// `--tethers`: what the analysis solved, per function
/// ([ADR-008](../../../../docs/specification/adr/adr-008.md) D6).
///
/// *The inverse tool is inspection, not assertion* — `@borrowed` forbids a
/// transition and this shows what was chosen without being asked. Read off the
/// ledger the build produced rather than computed again here, so that what a
/// person reads is what the file records.
pub fn report(parsed: &Parsed, ledger: &Ledger) -> String {
    // **This file's own entries**, because a report is about one file and the
    // ledger is the package's. A key names a function of this unit exactly when
    // this unit's own inference produced it.
    let here = Ledger::infer(parsed);
    let mut lines: Vec<String> = Vec::new();
    for (key, contract) in &ledger.functions {
        if contract.views.is_empty() || !here.functions.contains_key(key) {
            continue;
        }
        lines.push(format!("{key}:\n"));
        for held in &contract.views {
            lines.push(format!(
                "    {:<9} `{}`\n",
                held.state.as_str(),
                held.position
            ));
        }
    }
    match lines.is_empty() {
        // "here" and not "in this program", for the reason `sharing`'s report
        // gives: a report is about one file.
        true => "no view in a signature here, so there is no state to solve.\n".to_string(),
        false => lines.concat(),
    }
}

// ---------------------------------------------------------------------------
// The refusal: where the tether would be needed and is not built
// ---------------------------------------------------------------------------

/// A view handed back that points into a buffer the body **owns** (`NK2303`).
///
/// This is the one shape Part I 6.6's `Tethered` exists for, and
/// [ADR-156](../../../../docs/specification/adr/adr-156.md) D4 is what to do
/// about it while the state is not built: refuse on the Nikaia line, naming the
/// buffer and the mechanism, rather than lower a function whose result outlives
/// the buffer it points into and let `rustc` speak about the generated file
/// ([Part III C.1](../../../docs/specification/30-nikaia-tooling.md)).
///
/// **It stands on a named buffer and never on a doubt.** [`Buffer::Unknown`] is
/// a call no ledger describes; the *column* errs towards Tethered for it,
/// because a wide state costs a wide representation. A refusal cannot err that
/// way — refusing a correct program is the worse of the two mistakes
/// ([Part III C.4](../../../docs/specification/30-nikaia-tooling.md)) — so this
/// walk asks for the buffer **by name** and for the returned expression to be
/// derived from it. Where either is missing the program is lowered exactly as
/// it was before this check existed.
pub fn check(parsed: &Parsed, own: &Ledger, library: &Ledger) -> Vec<crate::check::Finding> {
    let borrowing: BTreeSet<String> = borrowing_structs(parsed)
        .into_iter()
        .map(|s| parsed.text(s).to_string())
        .collect();
    let carries = |ty: &Type| carries_a_view(parsed, ty, &borrowing);

    let mut out = Vec::new();
    for item in &parsed.program.items {
        match &item.node {
            Item::Fn { .. } => escaping(parsed, &item.node, None, &carries, own, library, &mut out),
            Item::Impl {
                target, methods, ..
            } => {
                let target = parsed.text(target.name).to_string();
                for method in methods {
                    escaping(
                        parsed,
                        &method.node,
                        Some(&target),
                        &carries,
                        own,
                        library,
                        &mut out,
                    );
                }
            }
            _ => {}
        }
    }
    out
}

/// One function's refusal, where it has one.
fn escaping(
    parsed: &Parsed,
    item: &Item,
    target: Option<&str>,
    carries: &impl Fn(&Type) -> bool,
    own: &Ledger,
    library: &Ledger,
    out: &mut Vec<crate::check::Finding>,
) {
    let Item::Fn {
        name,
        receiver,
        args,
        ret_type,
        body,
        ..
    } = item
    else {
        return;
    };
    // Only a result that carries a view can tether at all.
    let Some(ret) = ret_type.as_ref().filter(|t| carries(t)) else {
        return;
    };
    // **A caller's buffer does not excuse this one**, and that is where this
    // walk parts company with `of` above. The column asks *which* buffer a
    // result could point into and answers Borrowed the moment the caller has
    // one, because a signature is what it reads. A refusal reads the body: a
    // function that takes a `&str` and hands back a view of a `String` it made
    // tethers all the same, and it is the case the column's polarity would
    // miss — the one where the lowering ties the result to the parameter's
    // lifetime and `rustc` is left to explain the generated file.
    let _ = (receiver, args);

    let owned = named_buffers(parsed, body, own, library);
    if owned.is_empty() {
        return;
    }

    for (span, expr) in handed_back(body) {
        let Some(root) = root_of(parsed, expr) else {
            continue;
        };
        let Some(buffer) = owned.get(&root) else {
            continue;
        };
        let own_name = match name {
            Some(name) => parsed.text(*name).to_string(),
            None => "new".to_string(),
        };
        let of = match target {
            Some(target) => format!("`{target}::{own_name}`"),
            None => format!("`{own_name}`"),
        };
        let result = crate::views::write_type(parsed, ret);
        out.push(crate::check::Finding {
            severity: crate::check::Severity::Error,
            span,
            code: "NK2303",
            message: format!(
                "{of} hands back a view of `{root}`, and `{root}` is a `{buffer}` this body owns"
            ),
            notes: vec![
                format!(
                    "the result is `{result}`, which is a view, and `{root}`'s buffer is dropped \
                     when the call returns - so the view would outlive what it points into"
                ),
                "a view that outlives its buffer is **tethered** to it (Part I, 6.6), and the \
                 tether is not built yet: `Bytes` is the buffer it needs and the reference count \
                 that keeps one alive past its scope does not exist \
                 ([ADR-156](docs/specification/adr/adr-156.md) D4)"
                    .to_string(),
            ],
            help: Some(format!(
                "take the buffer as a parameter, so the view points into the caller's and the \
                 result borrows it:\n\
                 \x20          fn …(input: &str) -> {result} {{ … }}\n\
                 \x20      or hand back a copy with `.to_owned()`, which costs one allocation and \
                 says so (Part I, 6.6)"
            )),
        });
        return;
    }
}

/// Every local that **makes a buffer this walk can name**, by the name it is
/// bound to.
///
/// One name per `let`: a `let (a, b) = …` binds two names to the halves of a
/// pair and which half the buffer is is a question this walk cannot answer, so
/// it answers neither — the safe direction for a refusal.
fn named_buffers(
    parsed: &Parsed,
    body: &crate::ast::Block,
    own: &Ledger,
    library: &Ledger,
) -> BTreeMap<String, String> {
    use crate::ast::Stmt;
    let mut out = BTreeMap::new();
    fn walk(
        parsed: &Parsed,
        block: &crate::ast::Block,
        own: &Ledger,
        library: &Ledger,
        out: &mut BTreeMap<String, String>,
    ) {
        for stmt in &block.stmts {
            if let Stmt::Let { names, value, .. } = &stmt.node {
                if let [name] = names.as_slice() {
                    if let Buffer::Named(ty) = makes_a_buffer(parsed, value, own, library) {
                        out.insert(parsed.text(*name).to_string(), ty);
                    }
                }
            }
            super::sync::visit_stmt_blocks(&stmt.node, &mut |inner| {
                walk(parsed, inner, own, library, out)
            });
        }
    }
    walk(parsed, body, own, library, &mut out);
    out
}

/// Every expression a body **hands back**, with the statement it stands in.
///
/// A `return` anywhere, and the block's own last expression — reached through
/// the tails a value can come out of, so `if … { data.text() } else { "" }` is
/// two answers and not one statement nobody looked into.
fn handed_back(body: &crate::ast::Block) -> Vec<(crate::ast::Span, &crate::ast::Expr)> {
    use crate::ast::Stmt;
    let mut out = Vec::new();
    fn returns<'a>(
        block: &'a crate::ast::Block,
        out: &mut Vec<(crate::ast::Span, &'a crate::ast::Expr)>,
    ) {
        for stmt in &block.stmts {
            if let Stmt::Return(Some(value)) = &stmt.node {
                out.push((stmt.span.clone(), value));
            }
            super::sync::visit_stmt_blocks(&stmt.node, &mut |inner| returns(inner, out));
        }
    }
    returns(body, &mut out);
    if let Some(last) = body.stmts.last() {
        if let Stmt::Expr(value) = &last.node {
            tails(value, &last.span, &mut out);
        }
    }
    out
}

/// The expressions one tail position can turn out to be.
fn tails<'a>(
    expr: &'a crate::ast::Expr,
    span: &crate::ast::Span,
    out: &mut Vec<(crate::ast::Span, &'a crate::ast::Expr)>,
) {
    use crate::ast::{Expr, Stmt};
    match expr {
        Expr::Block(block) | Expr::Unsafe(block) => {
            if let Some(Stmt::Expr(value)) = block.stmts.last().map(|s| &s.node) {
                tails(value, span, out);
            }
        }
        Expr::If {
            then_branch,
            else_branch,
            ..
        } => {
            if let Some(Stmt::Expr(value)) = then_branch.stmts.last().map(|s| &s.node) {
                tails(value, span, out);
            }
            if let Some(block) = else_branch {
                if let Some(Stmt::Expr(value)) = block.stmts.last().map(|s| &s.node) {
                    tails(value, span, out);
                }
            }
        }
        other => out.push((span.clone(), other)),
    }
}

/// The local a view came out of, where one expression names it.
///
/// `data`, `&data`, `data.text()`, `data[..]`, `held.name` and a struct literal
/// built out of any of them all point into whatever `data` is. Anything else —
/// a literal, a call, an expression this walk does not recognise — names no
/// local, and a refusal that cannot name one does not fire.
fn root_of(parsed: &Parsed, expr: &crate::ast::Expr) -> Option<String> {
    use crate::ast::Expr;
    match expr {
        Expr::Variable(name) => Some(parsed.text(*name).to_string()),
        Expr::MethodCall { receiver, .. } | Expr::SafeMethod { receiver, .. } => {
            root_of(parsed, receiver)
        }
        Expr::Field { base, .. } | Expr::SafeField { base, .. } | Expr::Index { base, .. } => {
            root_of(parsed, base)
        }
        Expr::Unary { expr, .. } | Expr::Try(expr) | Expr::Cast { expr, .. } => {
            root_of(parsed, expr)
        }
        Expr::StructLit { fields, .. } => fields
            .iter()
            .filter_map(|f| f.value.as_ref())
            .find_map(|value| root_of(parsed, value)),
        _ => None,
    }
}
