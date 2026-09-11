// crates/nikaia/src/contracts/sync.rs
//
// Part II 12.1: a `sync` function may only call `sync` functions.
//
// Two analyses of one rule, running in **opposite directions**, and keeping
// them apart is the whole design (ADR-027).
//
// `check` verifies an assertion. Someone wrote `sync`, and this reports the
// calls that contradict it (`NK2202`). It is conservative in the **permissive**
// direction: with no receiver types, `a.method()` cannot be looked up, so it is
// not an error. What *can* be looked up is a call by name - a function in this
// unit, or a path like `io::read_to_string` into a library's ledger - and that
// is where the rule earns its keep, because `fs::` and `io::` are named rather
// than called on a receiver. The check never rejects a program the rule allows,
// and does not yet catch every program the rule forbids. An unchecked promise
// catches nothing at all, so that is worth having and worth saying.
//
// `infer` makes a claim, and therefore runs the other way. It writes `sync`
// into the ledger for a function nobody annotated, and that entry is **shipped**
// (Part III 13.5): a consumer reads it and puts the function inside `access`.
// So it is conservative in the **restrictive** direction - a call it cannot
// resolve is a call it cannot vouch for, and the function does not get the
// promise. The ledger settled this polarity once already, for provenance: "an
// analysis that fails open is a vulnerability generator". Inferring `sync` from
// a body full of calls one cannot see would be exactly that.
//
// **Why infer at all.** Before this, `sync` was opt-in, so almost nothing was
// `sync`, so `access`, `access_all`, `par_iter` and the panic hook - everything
// Part II 12.2 makes safe by demanding a `sync` lambda - could call almost
// nothing. The restrictive side of the language was the unusable one, and the
// way out was to annotate a chain of pure helpers by hand. Now a body that
// provably cannot pause says so on its own, and `sync` in the source becomes
// what `@borrowed` is in Part I 6.6: an **assertion you write where you want it
// held**, checked against the body, rather than a mode you have to enter.

use std::collections::{BTreeMap, BTreeSet};

use crate::ast::{Block, Expr, Item, Span, Stmt};
use crate::parser::Parsed;

use crate::check::MethodCalls;

use super::{Ledger, Sync};

/// One call that a `sync` function may not make.
#[derive(Debug, Clone)]
pub struct Violation {
    /// The statement the call is in. Expression-level spans are open work
    /// (`docs/project_status_and_roadmap.md`, Phase 2), so this points at the
    /// statement rather than at the call inside it.
    pub span: Span,
    /// The `sync` function making the call.
    pub caller: String,
    /// What it called, as the ledger names it.
    pub callee: String,
    /// Which ledger answered - this program's, or a library's.
    pub from_library: bool,
}

/// Every call a `sync` function makes that the ledgers say can pause.
pub fn check(parsed: &Parsed, own: &Ledger, library: &Ledger) -> Vec<Violation> {
    let mut found = Vec::new();

    for item in &parsed.program.items {
        match &item.node {
            Item::Fn { .. } => walk_fn(parsed, &item.node, None, own, library, &mut found),
            Item::Impl {
                target, methods, ..
            } => {
                let target = parsed.text(target.name).to_string();
                for method in methods {
                    walk_fn(
                        parsed,
                        &method.node,
                        Some(&target),
                        own,
                        library,
                        &mut found,
                    );
                }
            }
            _ => {}
        }
    }

    found.sort_by_key(|v| v.span.start);
    found
}

/// What one function's body does to its own claim to be `sync`.
#[derive(Debug, Default)]
struct Reach {
    /// It calls something that can pause, or something that cannot be resolved.
    /// Either way the claim is off the table and no fixpoint will bring it back.
    blocked: bool,
    /// The functions in this unit it calls. Its claim holds only while all of
    /// theirs do.
    calls: BTreeSet<String>,
}

/// Give every function in the ledger the `sync` its body earns.
///
/// Runs after the entries exist, and only ever *adds* [`Sync::Inferred`]: an
/// assertion in the source is what the source said and is left exactly as it
/// was written, so that `NK2202` still has something to contradict.
///
/// The fixpoint is a greatest one - start from "every candidate is `sync`" and
/// take the claim away from anything that reaches a function without it. Two
/// consequences worth naming. Mutual recursion between pure functions keeps the
/// claim, which is correct and is what a least fixpoint would have got wrong.
/// And the iteration walks a `BTreeMap` and repeats until nothing changes, so
/// the answer does not depend on the order the source declared things in -
/// which it must not, because 13.5 makes this file a pure function of (source,
/// toolchain) and `--locked` compares it byte for byte.
///
/// `resolved` is the type checker's answer to the one question this walk cannot
/// ask: what a method call goes to (ADR-028). Handing it in rather than
/// computing it here keeps one type checker in the compiler; the alternative
/// was a second, worse one living in this file.
pub fn infer(
    ledger: &mut Ledger,
    parsed: &Parsed,
    library: &Ledger,
    resolved: &BTreeMap<String, MethodCalls>,
) {
    let mut graph: BTreeMap<String, Reach> = BTreeMap::new();

    for item in &parsed.program.items {
        match &item.node {
            Item::Fn { .. } => {
                if let Some((name, reach)) =
                    reach_of(parsed, &item.node, None, ledger, library, resolved)
                {
                    graph.insert(name, reach);
                }
            }
            Item::Impl {
                target, methods, ..
            } => {
                let target = parsed.text(target.name).to_string();
                for method in methods {
                    if let Some((name, reach)) = reach_of(
                        parsed,
                        &method.node,
                        Some(&target),
                        ledger,
                        library,
                        resolved,
                    ) {
                        graph.insert(name, reach);
                    }
                }
            }
            _ => {}
        }
    }

    // Start optimistic, then take the claim away until nothing changes.
    let mut holds: BTreeMap<&str, bool> = graph
        .iter()
        .map(|(name, reach)| (name.as_str(), !reach.blocked))
        .collect();

    loop {
        let mut changed = false;
        for (name, reach) in &graph {
            if !holds[name.as_str()] {
                continue;
            }
            // A call to something this unit does not declare was already
            // resolved against the library above and folded into `blocked`;
            // what is left here is this unit's own, and an unknown name among
            // them would be a bug in `reach_of` rather than a licence to assume.
            let reaches_pausing = reach
                .calls
                .iter()
                .any(|callee| !holds.get(callee.as_str()).copied().unwrap_or(false));
            if reaches_pausing {
                holds.insert(name.as_str(), false);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }

    for (name, holds) in holds {
        if !holds {
            continue;
        }
        if let Some(contract) = ledger.functions.get_mut(name) {
            if contract.sync == Sync::No {
                contract.sync = Sync::Inferred;
            }
        }
    }
}

/// One function's calls, split into what settles the question now and what
/// depends on the rest of the unit.
///
/// `None` where the item is not a function. A function whose body cannot be
/// seen at all would be `blocked`, not absent - but Stage 0 has no such thing.
fn reach_of(
    parsed: &Parsed,
    item: &Item,
    target: Option<&str>,
    own: &Ledger,
    library: &Ledger,
    resolved: &BTreeMap<String, MethodCalls>,
) -> Option<(String, Reach)> {
    let Item::Fn { name, body, .. } = item else {
        return None;
    };
    let own_name = match name {
        Some(name) => parsed.text(*name).to_string(),
        None => "new".to_string(),
    };
    let key = match target {
        Some(target) => format!("{target}::{own_name}"),
        None => own_name,
    };

    let mut reach = Reach::default();
    collect_reach(parsed, body, own, library, &mut reach);

    // What the walk above left to somebody else: every method call this
    // function makes, as the type checker resolved it (ADR-028). The two are
    // merged rather than reconciled - the walk skips method calls entirely and
    // this covers exactly those - so nothing is counted twice and nothing is
    // dropped.
    if let Some(methods) = resolved.get(&key) {
        // One method whose receiver is not known is enough. It is the absence
        // of an answer, and D2's polarity says what to do with one.
        reach.blocked |= methods.unresolved;
        for callee in &methods.resolved {
            if own.functions.contains_key(callee) {
                reach.calls.insert(callee.clone());
            } else if !library
                .functions
                .get(callee)
                .is_some_and(|contract| contract.sync.is_sync())
            {
                // A library method that can pause, or one that resolved to a
                // name this ledger does not carry after all.
                reach.blocked = true;
            }
        }
    }

    Some((key, reach))
}

fn collect_reach(
    parsed: &Parsed,
    block: &Block,
    own: &Ledger,
    library: &Ledger,
    reach: &mut Reach,
) {
    for stmt in &block.stmts {
        visit_stmt(
            parsed,
            &stmt.node,
            &mut |expr| match reached(parsed, expr, own, library) {
                Some(Reached::Own(name)) => {
                    reach.calls.insert(name);
                }
                Some(Reached::Library { sync: false, .. }) | Some(Reached::Opaque) => {
                    reach.blocked = true
                }
                // Answered per function by the type checker, and merged in by
                // `reach_of` once this walk is done.
                Some(Reached::Method) => {}
                Some(Reached::Library { sync: true, .. }) | None => {}
            },
        );
        // The same walk the check uses: a nested block, and the body of a
        // trailing lambda, are part of the function that writes them.
        visit_stmt_blocks(&stmt.node, &mut |inner| {
            collect_reach(parsed, inner, own, library, reach)
        });
    }
}

fn walk_fn(
    parsed: &Parsed,
    item: &Item,
    target: Option<&str>,
    own: &Ledger,
    library: &Ledger,
    found: &mut Vec<Violation>,
) {
    let Item::Fn {
        name,
        body,
        is_sync,
        ..
    } = item
    else {
        return;
    };
    if !*is_sync {
        return;
    }

    let own_name = match name {
        Some(name) => parsed.text(*name).to_string(),
        None => "new".to_string(),
    };
    let caller = match target {
        Some(target) => format!("{target}::{own_name}"),
        None => own_name,
    };

    walk_block(parsed, body, &caller, own, library, found);
}

fn walk_block(
    parsed: &Parsed,
    block: &Block,
    caller: &str,
    own: &Ledger,
    library: &Ledger,
    found: &mut Vec<Violation>,
) {
    for stmt in &block.stmts {
        let span = stmt.span.clone();
        visit_stmt(parsed, &stmt.node, &mut |expr| {
            if let Some((callee, from_library)) = called(parsed, expr, own, library) {
                found.push(Violation {
                    span: span.clone(),
                    caller: caller.to_string(),
                    callee,
                    from_library,
                });
            }
        });

        // A nested block is part of the same function, so its calls are the
        // same promise.
        visit_stmt_blocks(&stmt.node, &mut |inner| {
            walk_block(parsed, inner, caller, own, library, found)
        });
    }
}

/// What one expression tells either analysis, where it is a call at all.
///
/// One resolution rule, written once. The check and the inference disagree
/// about what to *do* with `Opaque` - the first shrugs, the second refuses -
/// and that disagreement is the design. Having them disagree about what a call
/// even resolves to would just be a bug waiting to happen.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Reached {
    /// A function this unit declares, by the name the ledger records it under.
    Own(String),
    /// A function in a library, and what that library's ledger says about it.
    Library { key: String, sync: bool },
    /// A method call. Neither analysis here can resolve one: `stats.add(5)`
    /// names `add` and says nothing about what `stats` is.
    ///
    /// The **type checker** can, and does (ADR-028). So this is not "unknown"
    /// but "asked elsewhere", and the two callers of `reached` take it
    /// differently: the inference merges in the checker's answer per function,
    /// and the check looks the resolved name up the same way it looks up any
    /// other. Collapsing this into `Opaque` was what threw the answer away.
    Method,
    /// A call whose target this compiler cannot name and nobody else can
    /// either: a name no ledger knows.
    ///
    /// Also everything that is not a plain call but still *runs* something -
    /// `spawn`, a `dsl` - because a body containing one is not the pure CPU
    /// task Part II 12.1 describes, whatever the thing it runs turns out to do.
    Opaque,
}

/// What a call resolves to, by the same rule for both analyses.
///
/// `None` means the expression is not a call at all, which is the one case
/// neither analysis has anything to say about.
fn reached(parsed: &Parsed, expr: &Expr, own: &Ledger, library: &Ledger) -> Option<Reached> {
    let name = match expr {
        Expr::Call { func, .. } => match &**func {
            Expr::Variable(name) => parsed.text(*name).to_string(),
            Expr::Path(segments) => segments
                .iter()
                .map(|s| parsed.text(*s))
                .collect::<Vec<_>>()
                .join("::"),
            // A call through anything else is a target we cannot name.
            _ => return Some(Reached::Opaque),
        },
        // Answered by the type checker rather than here (ADR-028).
        Expr::MethodCall { .. } => return Some(Reached::Method),
        // Starts a task, or runs a grammar whose actions are arbitrary Nikaia.
        // Neither is pure computation this compiler can see the end of.
        Expr::Spawn { .. } | Expr::Dsl { .. } | Expr::DslFrom { .. } => {
            return Some(Reached::Opaque)
        }
        _ => return None,
    };

    // This unit first: a program's own functions are what it mostly calls, and
    // a local name shadows nothing in a library.
    if own.functions.contains_key(&name) {
        return Some(Reached::Own(name));
    }

    // `Stats(first)` is the anonymous constructor of Kap 4.2, which the
    // lowering names `Stats::new` and the ledger records under that name. A
    // constructor is a function like any other and makes the same promise or
    // does not.
    let constructed = format!("{name}::new");
    if own.functions.contains_key(&constructed) {
        return Some(Reached::Own(constructed));
    }

    // Then the library, by the name the caller wrote or the one the prelude
    // makes available unqualified.
    if let Some((key, contract)) = library.lookup(&name) {
        return Some(Reached::Library {
            key,
            sync: contract.sync.is_sync(),
        });
    }

    Some(Reached::Opaque)
}

/// The name a call resolves to, when a ledger says it can pause.
///
/// `None` covers three different things and the difference does not matter to
/// the *check*: it is not a call, it is a call this compiler cannot resolve, or
/// it resolves to something a ledger says is `sync`. The permissive direction,
/// stated as code.
fn called(parsed: &Parsed, expr: &Expr, own: &Ledger, library: &Ledger) -> Option<(String, bool)> {
    match reached(parsed, expr, own, library)? {
        Reached::Own(name) => {
            let contract = own.functions.get(&name)?;
            (!contract.sync.is_sync()).then_some((name, false))
        }
        Reached::Library { key, sync } => (!sync).then_some((key, true)),
        // The check deliberately does not use ADR-028's resolution, and the
        // reason is the diagnostic rather than the analysis. `NK2202` names one
        // call and puts a caret under it; the checker answers per *function*,
        // because `Symbol` carries no position and there is nothing to key a
        // call site by. "Something in here pauses" is not a message Part III
        // C.2 allows. So the check stays permissive here until expression-level
        // spans exist, and the inference - which needs no caret - does not wait
        // for them.
        Reached::Method | Reached::Opaque => None,
    }
}

/// Every call by name in a block and the blocks inside it.
///
/// Shared with the trust analysis, which asks a different question of the same
/// walk: `sync` asks what a call promises, provenance asks where its result
/// came from.
pub(super) fn walk_calls(parsed: &Parsed, block: &Block, f: &mut impl FnMut(&str)) {
    for stmt in &block.stmts {
        visit_stmt(parsed, &stmt.node, &mut |expr| {
            if let Some(name) = super::trust::call_name(parsed, expr) {
                f(&name);
            }
        });
        visit_stmt_blocks(&stmt.node, &mut |inner| walk_calls(parsed, inner, f));
    }
}

/// Every expression a statement holds, without descending into nested blocks -
/// those are walked separately so that each keeps its own statement's span.
fn visit_stmt(parsed: &Parsed, stmt: &Stmt, f: &mut impl FnMut(&Expr)) {
    match stmt {
        Stmt::Let { value, .. } => visit_expr(parsed, value, f),
        Stmt::Assign { target, value, .. } => {
            visit_expr(parsed, target, f);
            visit_expr(parsed, value, f);
        }
        Stmt::For { iter, .. } => visit_expr(parsed, iter, f),
        Stmt::While { cond, .. } => visit_expr(parsed, cond, f),
        Stmt::Return(Some(value)) => visit_expr(parsed, value, f),
        Stmt::Return(None) => {}
        Stmt::Expr(expr) => visit_expr(parsed, expr, f),
    }
}

fn visit_stmt_blocks(stmt: &Stmt, f: &mut impl FnMut(&Block)) {
    match stmt {
        Stmt::For { body, .. } | Stmt::While { body, .. } => f(body),
        Stmt::Let { value, .. } | Stmt::Expr(value) => visit_expr_blocks(value, f),
        Stmt::Assign { target, value, .. } => {
            visit_expr_blocks(target, f);
            visit_expr_blocks(value, f);
        }
        Stmt::Return(Some(value)) => visit_expr_blocks(value, f),
        Stmt::Return(None) => {}
    }
}

/// Every block an expression holds, including the body of a lambda passed as
/// an argument.
///
/// A trailing lambda runs *during* the call it is given to - `.and_modify fn {
/// … }` is not deferred - so what it calls, the function around it calls. The
/// one shape that is different is `spawn`, whose body runs later and elsewhere;
/// it is a detached context (Part I, 5.4) and is not walked here.
fn visit_expr_blocks(expr: &Expr, f: &mut impl FnMut(&Block)) {
    match expr {
        Expr::Block(block) | Expr::Closure { body: block, .. } => f(block),
        Expr::Call { func, args, .. } => {
            visit_expr_blocks(func, f);
            args.iter().for_each(|a| visit_expr_blocks(a, f));
        }
        Expr::MethodCall { receiver, args, .. } => {
            visit_expr_blocks(receiver, f);
            args.iter().for_each(|a| visit_expr_blocks(a, f));
        }
        Expr::If {
            then_branch,
            else_branch,
            ..
        } => {
            f(then_branch);
            if let Some(block) = else_branch {
                f(block);
            }
        }
        Expr::TryCatch { expr, handler } => {
            visit_expr_blocks(expr, f);
            f(handler);
        }
        Expr::Match { arms, .. } => {
            for arm in arms {
                visit_expr_blocks(&arm.body, f);
            }
        }
        _ => {}
    }
}

/// Every expression inside one, excluding the bodies of nested blocks.
fn visit_expr(parsed: &Parsed, expr: &Expr, f: &mut impl FnMut(&Expr)) {
    f(expr);

    // **A hole is a call like any other.** Its expression is parsed out of the
    // literal on the way to the emitter, so until this walk existed a call
    // inside `"{io::read_to_string()}"` was invisible here - and `sync` is
    // *inferred* from what a body calls (ADR-027), so the function came out of
    // the ledger claiming it cannot pause. ADR-027 D2 and ADR-010 D1 name that
    // direction the dangerous one.
    for hole in crate::emit::literal_expressions(parsed, expr) {
        visit_expr(parsed, &hole, f);
    }

    match expr {
        Expr::Call { func, args, .. } => {
            visit_expr(parsed, func, f);
            args.iter().for_each(|a| visit_expr(parsed, a, f));
        }
        Expr::MethodCall { receiver, args, .. } => {
            visit_expr(parsed, receiver, f);
            args.iter().for_each(|a| visit_expr(parsed, a, f));
        }
        Expr::Binary { lhs, rhs, .. } => {
            visit_expr(parsed, lhs, f);
            visit_expr(parsed, rhs, f);
        }
        Expr::Unary { expr, .. } | Expr::Try(expr) | Expr::Cast { expr, .. } => {
            visit_expr(parsed, expr, f)
        }
        Expr::Field { base, .. } => visit_expr(parsed, base, f),
        Expr::Index { base, index } => {
            visit_expr(parsed, base, f);
            visit_expr(parsed, index, f);
        }
        Expr::Range { start, end, .. } => {
            visit_expr(parsed, start, f);
            visit_expr(parsed, end, f);
        }
        Expr::Tuple(parts) => parts.iter().for_each(|p| visit_expr(parsed, p, f)),
        Expr::Coalesce { value, fallback } => {
            visit_expr(parsed, value, f);
            visit_expr(parsed, fallback, f);
        }
        Expr::TryCatch { expr, .. } => visit_expr(parsed, expr, f),
        Expr::If { cond, .. } => visit_expr(parsed, cond, f),
        Expr::Match { value, .. } => visit_expr(parsed, value, f),
        Expr::StructLit { fields, .. } => fields
            .iter()
            .filter_map(|field| field.value.as_ref())
            .for_each(|value| visit_expr(parsed, value, f)),
        _ => {}
    }
}
