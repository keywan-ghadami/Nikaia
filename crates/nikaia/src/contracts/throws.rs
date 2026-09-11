//! Kap 7.1: which errors can leave a function.
//!
//! `throws` in the source says *that* a function can fail. Which errors is a
//! question about its body and about everything its body reaches, so it is
//! derived here rather than written down — the argument of
//! [ADR-005](../../../../docs/specification/adr/adr-005.md) D3, applied to the
//! second kind of contract in
//! [ADR-023](../../../../docs/specification/adr/adr-023.md) D1.
//!
//! This is [`super::sync`]'s walk with the lattice turned around. `sync` is a
//! **greatest** fixpoint over a boolean: everyone starts pure and loses the
//! claim on contact with something that pauses. An error set is a **least**
//! fixpoint: everyone starts with what they throw themselves, and grows by what
//! their callees throw, until nothing changes. Mutual recursion terminates for
//! the same reason it does there — the sets only grow, and there are finitely
//! many names to grow by.
//!
//! The call resolution is `sync`'s, not a second one. ADR-028 recorded what the
//! alternative costs: two analyses that have to agree about what `stats.add(5)`
//! goes to, and eventually do not.

use std::collections::{BTreeMap, BTreeSet};

use super::sync::{reached, visit_stmt, visit_stmt_blocks, Reached};
use super::{FnContract, Ledger, UNNAMED_ERROR};
use crate::ast::{Block, Expr, Item};
use crate::parser::Parsed;

/// What one function contributes to its own error set before its callees are
/// taken into account.
#[derive(Debug, Default)]
struct Contrib {
    /// Named here: `throw ConfigError::NotFound(p)` puts `ConfigError` in.
    /// `"?"` where something can fail and this compiler cannot name what with.
    direct: BTreeSet<String>,
    /// Functions in this unit it calls. Their errors reach it, because nothing
    /// marks a failing call (ADR-023 D8) — propagation is what a call does.
    calls: BTreeSet<String>,
}

/// Give every `throws` function in the ledger the error set its body earns.
///
/// Only a function the source declared `throws` gets one: a function that
/// cannot fail cannot propagate, and a declared `throws` whose body names
/// nothing keeps `["?"]` rather than losing its entry. That last case is the
/// same shape as an asserted `sync` — the declaration is a promise a caller
/// already relies on, and inference is here to say more than it, never less.
pub fn infer(ledger: &mut Ledger, parsed: &Parsed, library: &Ledger) {
    let mut graph: BTreeMap<String, Contrib> = BTreeMap::new();

    for item in &parsed.program.items {
        match &item.node {
            Item::Fn { .. } => {
                if let Some((name, contrib)) = contrib_of(parsed, &item.node, None, ledger, library)
                {
                    graph.insert(name, contrib);
                }
            }
            Item::Impl {
                target, methods, ..
            } => {
                let target = parsed.text(target.name).to_string();
                for method in methods {
                    if let Some((name, contrib)) =
                        contrib_of(parsed, &method.node, Some(&target), ledger, library)
                    {
                        graph.insert(name, contrib);
                    }
                }
            }
            _ => {}
        }
    }

    // Start with what each one throws itself, then grow by what it reaches.
    let mut sets: BTreeMap<String, BTreeSet<String>> = graph
        .iter()
        .map(|(name, c)| (name.clone(), c.direct.clone()))
        .collect();

    loop {
        let mut changed = false;
        for (name, contrib) in &graph {
            let mut grown = sets[name].clone();
            for callee in &contrib.calls {
                if let Some(theirs) = sets.get(callee) {
                    for error in theirs {
                        grown.insert(error.clone());
                    }
                }
            }
            if grown != sets[name] {
                sets.insert(name.clone(), grown);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }

    for (name, set) in sets {
        let Some(contract) = ledger.functions.get_mut(&name) else {
            continue;
        };
        if contract.throws.is_empty() {
            continue;
        }
        contract.throws = if set.is_empty() {
            vec![UNNAMED_ERROR.to_string()]
        } else {
            set.into_iter().collect()
        };
    }
}

/// What one function throws directly, and whom it calls. `None` where the item
/// is not a `throws` function - one that cannot fail has no set to grow.
fn contrib_of(
    parsed: &Parsed,
    item: &Item,
    target: Option<&str>,
    own: &Ledger,
    library: &Ledger,
) -> Option<(String, Contrib)> {
    let Item::Fn {
        name, body, throws, ..
    } = item
    else {
        return None;
    };
    if !*throws {
        return None;
    }

    let own_name = match name {
        Some(name) => parsed.text(*name).to_string(),
        None => "new".to_string(),
    };
    let key = match target {
        Some(target) => format!("{target}::{own_name}"),
        None => own_name,
    };

    let mut contrib = Contrib::default();
    collect(parsed, body, own, library, &mut contrib);
    Some((key, contrib))
}

fn collect(parsed: &Parsed, block: &Block, own: &Ledger, library: &Ledger, into: &mut Contrib) {
    for stmt in &block.stmts {
        visit_stmt(&stmt.node, &mut |expr| {
            if let Expr::Throw(thrown) = expr {
                match error_type(parsed, thrown) {
                    Some(name) => into.direct.insert(name),
                    // `throw` reached something this compiler cannot name -
                    // a value from a call, say. It fails; with what is the
                    // absence of a claim rather than an answer.
                    None => into.direct.insert(UNNAMED_ERROR.to_string()),
                };
                return;
            }
            match reached(parsed, expr, own, library) {
                Some(Reached::Own(name)) => {
                    into.calls.insert(name);
                }
                Some(Reached::Library { key, .. }) => {
                    // A library names its errors in its own ledger, or does not.
                    // `std` does not: its failures are the Rust ones below, and
                    // ADR-020 D5 has those entries written by hand.
                    if let Some(FnContract { throws, .. }) = library.functions.get(&key) {
                        for error in throws {
                            into.direct.insert(error.clone());
                        }
                    }
                }
                // A method call, or a call nobody can name. Either can fail, and
                // treating "I cannot see it" as "it does not fail" is the one
                // direction ADR-010 D1 calls a vulnerability generator.
                Some(Reached::Method) | Some(Reached::Opaque) => {
                    into.direct.insert(UNNAMED_ERROR.to_string());
                }
                None => {}
            }
        });
        visit_stmt_blocks(&stmt.node, &mut |inner| {
            collect(parsed, inner, own, library, into)
        });
    }
}

/// The type a `throw` raises. `ConfigError::NotFound(p)` and
/// `ConfigError::NotFound` are both `ConfigError` - a variant is written under
/// the enum that declares it (Part I, 3.4), so the first segment is the type.
fn error_type(parsed: &Parsed, thrown: &Expr) -> Option<String> {
    match thrown {
        Expr::Path(segments) => segments.first().map(|s| parsed.text(*s).to_string()),
        Expr::Call { func, .. } => error_type(parsed, func),
        Expr::StructLit { name, .. } => Some(parsed.text(*name).to_string()),
        _ => None,
    }
}
