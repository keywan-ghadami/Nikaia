// crates/nikaia/src/foreign.rs
//
// A crate is described before it is called - `NK2504`
// ([ADR-104](../../docs/specification/adr/adr-104.md) D1).
//
// ## What this is for
//
// A call into a Rust crate no ledger describes was **silent**. Every analysis
// this compiler has - what may cross a thread, what a call may reach, whether it
// pauses, whether it can fail - reads a contract at the boundary, and where
// there is none they all read the same thing: nothing. Fail-closed answers stop
// a few of the questions (`crosses_into_an_unseen_call` hands the crossing on
// rather than accepting it), and the rest simply do not happen.
//
// D1 removes the third answer. There is a described crate and a refused call,
// and the message names the command that turns the second into the first.
//
// ## Which way it errs
//
// **Only where the build itself declared the crate.** The set comes from
// `[dependencies]` with `type = "rust"` (`manifest::foreign_crates`), so a
// qualified name this compiler cannot account for - a module of the program, a
// Nikaia package, a `std` path, a typo - is not this refusal's business and
// keeps whatever message it already had. Refusing on a name nobody declared
// would be [Part III C.4](../../docs/specification/30-nikaia-tooling.md)'s
// correct program refused, which is the worse of the two mistakes.
//
// **And once per crate, not once per call.** The reader's next move is one
// command for the whole crate, so four calls into `regex` are one thing to do
// and one message to read. The caret is on the first call, which is where they
// will start.

use std::collections::{BTreeMap, BTreeSet};

use crate::ast::{Block, Expr, Item, Span, Stmt, Type};
use crate::check::{Finding, Severity};
use crate::parser::Parsed;

/// Every call and every written type that reaches into a crate nothing
/// describes.
///
/// `declared` is what the manifest says this build links against, under the
/// name a program writes; `described` is the crates a ledger was found for.
/// Both empty - a loose file, with no project around it - is silence, which is
/// the right answer: nothing was declared, so nothing is undescribed.
pub fn check(
    parsed: &Parsed,
    declared: &BTreeSet<String>,
    described: &BTreeSet<String>,
) -> Vec<Finding> {
    if declared.is_empty() {
        return Vec::new();
    }
    let mut first: BTreeMap<String, Span> = BTreeMap::new();
    for item in &parsed.program.items {
        item_names(
            parsed,
            &item.node,
            &item.span,
            &mut |crate_name: &str, span: &Span| {
                if declared.contains(crate_name) && !described.contains(crate_name) {
                    first
                        .entry(crate_name.to_string())
                        .or_insert_with(|| span.clone());
                }
            },
        );
    }
    first
        .into_iter()
        .map(|(crate_name, span)| undescribed(&crate_name, &span))
        .collect()
}

/// `NK2504`, and the message is the command.
fn undescribed(crate_name: &str, span: &Span) -> Finding {
    Finding {
        severity: Severity::Error,
        span: span.clone(),
        code: "NK2504",
        message: format!("`{crate_name}` is not described"),
        notes: vec![
            "every analysis reads a contract at the boundary - what may cross a thread, \
             what the call may reach, whether it pauses, whether it can fail - and an \
             undescribed crate is an absence rather than an answer (ADR-104 D1)"
                .to_string(),
            "the draft is written from the crate's own `pub` signatures, committed as \
             `contracts/<crate>.contracts`, and reviewed like code: what a signature \
             cannot say is written fail-closed (Part III, 15.2)"
                .to_string(),
        ],
        help: Some(format!("run `nikaia describe {crate_name}`")),
    }
}

/// The first segment of every qualified name an item writes, with the span the
/// message should carry.
fn item_names(parsed: &Parsed, item: &Item, span: &Span, found: &mut impl FnMut(&str, &Span)) {
    match item {
        Item::Fn {
            args,
            ret_type,
            body,
            ..
        } => {
            for arg in args {
                ty_names(parsed, &arg.ty, span, found);
            }
            if let Some(ret) = ret_type {
                ty_names(parsed, ret, span, found);
            }
            block_names(parsed, body, found);
        }
        Item::Struct { fields, .. } => {
            for field in fields {
                ty_names(parsed, &field.ty, span, found);
            }
        }
        Item::Impl { methods, .. } => {
            for method in methods {
                item_names(parsed, &method.node, &method.span, found);
            }
        }
        _ => {}
    }
}

fn block_names(parsed: &Parsed, block: &Block, found: &mut impl FnMut(&str, &Span)) {
    for stmt in &block.stmts {
        if let Stmt::Let { ty: Some(ty), .. } = &stmt.node {
            ty_names(parsed, ty, &stmt.span, found);
        }
        crate::contracts::sync::visit_stmt(parsed, &stmt.node, &mut |expr| {
            if let Expr::Path(segments) = expr {
                if let Some(head) = segments.first() {
                    // `unaliased`, because a file may write its own word for a
                    // package (ADR-046 D3) - and the manifest key is the only
                    // name the declaration has.
                    let head = parsed.unaliased(parsed.text(*head));
                    let head = head.split("::").next().unwrap_or(&head).to_string();
                    found(&head, &stmt.span);
                }
            }
        });
        crate::contracts::sync::visit_stmt_blocks(&stmt.node, &mut |inner| {
            block_names(parsed, inner, found)
        });
    }
}

/// A written type may name a crate too - `hyper_shim::Handle` in a parameter is
/// as much a reach across the boundary as a call is, and it is the one the
/// crossing rules are about.
fn ty_names(parsed: &Parsed, ty: &Type, span: &Span, found: &mut impl FnMut(&str, &Span)) {
    let name = parsed.unaliased(parsed.text(ty.name));
    if let Some((head, _)) = name.split_once("::") {
        found(head, span);
    }
    for argument in &ty.generics {
        ty_names(parsed, argument, span, found);
    }
    if let Some(code) = &ty.code {
        if let Some(result) = &code.result {
            ty_names(parsed, result, span, found);
        }
    }
}
