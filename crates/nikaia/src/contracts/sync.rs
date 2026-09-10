// crates/nikaia/src/contracts/sync.rs
//
// Part II 12.1: a `sync` function may only call `sync` functions.
//
// The keyword has parsed and been carried in the AST since ADR-013, and the
// emitter has written `// sync … Not checked yet.` above every one of them.
// This is the check, and it is possible now for one reason: the ledger
// (ADR-020) says what a function promises, including the functions in `std`
// that this compiler cannot see the bodies of.
//
// **What it can and cannot resolve, stated rather than implied.** With no type
// checker there is no receiver type, so `a.method()` cannot be looked up and is
// not an error. What *can* be looked up is a call by name - a function in this
// unit, or a path like `io::read_to_string` into a library's ledger - and that
// is exactly where the rule earns its keep: `fs::` and `io::` are what a `sync`
// function must not reach, and they are named, not called on a receiver.
//
// So the check is conservative in the permissive direction. It never rejects a
// program the rule allows, and it does not yet catch every program the rule
// forbids. That is worth having and worth saying: an unchecked promise catches
// nothing at all.

use crate::ast::{Block, Expr, Item, Span, Stmt};
use crate::parser::Parsed;

use super::Ledger;

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
            Item::Impl { target, methods } => {
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
        visit_stmt(&stmt.node, &mut |expr| {
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

/// The name a call resolves to, when a ledger has something to say about it.
///
/// `None` covers three different things and the difference does not matter
/// here: it is not a call, it is a call this compiler cannot resolve, or it
/// resolves to something a ledger says is `sync`.
fn called(parsed: &Parsed, expr: &Expr, own: &Ledger, library: &Ledger) -> Option<(String, bool)> {
    let Expr::Call { func, .. } = expr else {
        return None;
    };

    let name = match &**func {
        Expr::Variable(name) => parsed.text(*name).to_string(),
        Expr::Path(segments) => segments
            .iter()
            .map(|s| parsed.text(*s))
            .collect::<Vec<_>>()
            .join("::"),
        // A method call needs the receiver's type, which Stage 0 does not have.
        _ => return None,
    };

    // This unit first: a program's own functions are what it mostly calls, and
    // a local name shadows nothing in a library.
    if let Some(contract) = own.functions.get(&name) {
        return (!contract.sync).then_some((name, false));
    }

    // `Stats(first)` is the anonymous constructor of Kap 4.2, which the
    // lowering names `Stats::new` and the ledger records under that name. A
    // constructor is a function like any other and makes the same promise or
    // does not.
    let constructed = format!("{name}::new");
    if let Some(contract) = own.functions.get(&constructed) {
        return (!contract.sync).then_some((constructed, false));
    }

    // Then the library, by the name the caller wrote or the one the prelude
    // makes available unqualified.
    if let Some((key, contract)) = library.lookup(&name) {
        return (!contract.sync).then_some((key, true));
    }

    None
}

/// Every call by name in a block and the blocks inside it.
///
/// Shared with the trust analysis, which asks a different question of the same
/// walk: `sync` asks what a call promises, provenance asks where its result
/// came from.
pub(super) fn walk_calls(parsed: &Parsed, block: &Block, f: &mut impl FnMut(&str)) {
    for stmt in &block.stmts {
        visit_stmt(&stmt.node, &mut |expr| {
            if let Some(name) = super::trust::call_name(parsed, expr) {
                f(&name);
            }
        });
        visit_stmt_blocks(&stmt.node, &mut |inner| walk_calls(parsed, inner, f));
    }
}

/// Every expression a statement holds, without descending into nested blocks -
/// those are walked separately so that each keeps its own statement's span.
fn visit_stmt(stmt: &Stmt, f: &mut impl FnMut(&Expr)) {
    match stmt {
        Stmt::Let { value, .. } => visit_expr(value, f),
        Stmt::Assign { target, value, .. } => {
            visit_expr(target, f);
            visit_expr(value, f);
        }
        Stmt::For { iter, .. } => visit_expr(iter, f),
        Stmt::Return(Some(value)) => visit_expr(value, f),
        Stmt::Return(None) => {}
        Stmt::Expr(expr) => visit_expr(expr, f),
    }
}

fn visit_stmt_blocks(stmt: &Stmt, f: &mut impl FnMut(&Block)) {
    match stmt {
        Stmt::For { body, .. } => f(body),
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
        Expr::Call { func, args } => {
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
fn visit_expr(expr: &Expr, f: &mut impl FnMut(&Expr)) {
    f(expr);
    match expr {
        Expr::Call { func, args } => {
            visit_expr(func, f);
            args.iter().for_each(|a| visit_expr(a, f));
        }
        Expr::MethodCall { receiver, args, .. } => {
            visit_expr(receiver, f);
            args.iter().for_each(|a| visit_expr(a, f));
        }
        Expr::Binary { lhs, rhs, .. } => {
            visit_expr(lhs, f);
            visit_expr(rhs, f);
        }
        Expr::Unary { expr, .. } | Expr::Try(expr) | Expr::Cast { expr, .. } => visit_expr(expr, f),
        Expr::Field { base, .. } => visit_expr(base, f),
        Expr::Index { base, index } => {
            visit_expr(base, f);
            visit_expr(index, f);
        }
        Expr::Range { start, end, .. } => {
            visit_expr(start, f);
            visit_expr(end, f);
        }
        Expr::Tuple(parts) => parts.iter().for_each(|p| visit_expr(p, f)),
        Expr::Coalesce { value, fallback } => {
            visit_expr(value, f);
            visit_expr(fallback, f);
        }
        Expr::TryCatch { expr, .. } => visit_expr(expr, f),
        Expr::If { cond, .. } => visit_expr(cond, f),
        Expr::Match { value, .. } => visit_expr(value, f),
        Expr::StructLit { fields, .. } => fields
            .iter()
            .filter_map(|field| field.value.as_ref())
            .for_each(|value| visit_expr(value, f)),
        _ => {}
    }
}
