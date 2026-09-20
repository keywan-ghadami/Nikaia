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
        let state = match borrows_from_a_caller || !owns_a_buffer(parsed, body, here, library) {
            true => State::Borrowed,
            false => State::Tethered,
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
) -> bool {
    let mut found = false;
    bindings(body, &mut |value| {
        found |= makes_a_buffer(parsed, value, own, library)
    });
    found
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
) -> bool {
    use crate::ast::Expr;
    match expr {
        // `text.to_owned()` and `n.to_string()`: owned text by the name, which
        // every entry of either name agrees on.
        Expr::MethodCall { method, .. } => {
            matches!(parsed.text(*method), "to_owned" | "to_string")
        }
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
                _ => return false,
            };
            let Some((_, contract)) = own
                .lookup(&name)
                .or_else(|| library.lookup(&name))
                .or_else(|| own.lookup(&format!("{name}::new")))
                .or_else(|| library.lookup(&format!("{name}::new")))
            else {
                // A call no ledger describes is a buffer this walk cannot rule
                // out, which is the safe direction here.
                return true;
            };
            contract
                .signature
                .as_ref()
                .and_then(|s| s.result.as_ref())
                .is_some_and(is_a_buffer)
        }
        // `catch` hands back whichever half ran, so either may be the buffer.
        Expr::TryCatch { expr, .. } => makes_a_buffer(parsed, expr, own, library),
        _ => false,
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
