// crates/nikaia/src/contracts/trust.rs
//
// Where a program's bytes came from (ADR-010).
//
// `Trusted ⊑ Untrusted`, joined in the safe direction: one untrusted input
// makes the result untrusted. The sources are `std`'s, stated in its ledger
// (ADR-020) - a file the operator named, a pipe they connected, the arguments
// they typed - and a program's own trust is the join over the ones it calls.
//
// **One buffer, because Stage 0 has one input lifetime.** ADR-008's model gives
// a compilation unit a single lifetime, so it has a single input buffer: every
// view a parser cuts, every struct tied to it, and every key of a map keyed by
// a view point into the same thing. The join over a program's sources is
// therefore not a coarsening of a per-buffer analysis - it *is* the per-buffer
// analysis, for the one buffer this representation can express. When the
// representation grows more than one, this becomes the join over each.
//
// **A barrier widens to untrusted, never the other way** (ADR-010 D1). A call
// this compiler cannot resolve could be anything, so a program that reaches one
// whose result it then treats as input has no proof of who chose those bytes.
// What Stage 0 can reach is `std`, which states all of its sources, so today
// there is no such call - and `Reason::Unresolved` is what will carry it when
// there is.

use crate::ast::{Expr, Item};
use crate::parser::Parsed;

use super::{Ledger, Provenance};

/// What the analysis concluded, and what it concluded it from.
#[derive(Debug, Clone)]
pub struct Trust {
    pub provenance: Provenance,
    /// Every source the program calls, in the order they were found, with what
    /// each contributed. This is what `--trust` prints: the choice is visible,
    /// never a mystery (ADR-010 D7).
    pub reasons: Vec<Reason>,
}

#[derive(Debug, Clone)]
pub struct Reason {
    /// The source, as the ledger names it.
    pub source: String,
    pub provenance: Provenance,
}

/// The provenance of this program's input.
pub fn analyse(parsed: &Parsed, library: &Ledger) -> Trust {
    let mut reasons: Vec<Reason> = Vec::new();

    for item in &parsed.program.items {
        let bodies = match &item.node {
            Item::Fn { body, .. } => vec![body],
            Item::Impl { methods, .. } => methods
                .iter()
                .filter_map(|m| match &m.node {
                    Item::Fn { body, .. } => Some(body),
                    _ => None,
                })
                .collect(),
            _ => continue,
        };

        for body in bodies {
            super::sync::walk_calls(parsed, body, &mut |name| {
                let Some((key, contract)) = library.lookup(name) else {
                    return;
                };
                let Some(provenance) = contract.provenance else {
                    return;
                };
                if !reasons.iter().any(|r| r.source == key) {
                    reasons.push(Reason {
                        source: key,
                        provenance,
                    });
                }
            });
        }
    }

    reasons.sort_by(|a, b| a.source.cmp(&b.source));

    // A program that reads nothing has no input to distrust. Its maps are keyed
    // by what it wrote itself, which is the compiled-in case ADR-010 D2 calls
    // trusted.
    let provenance = reasons
        .iter()
        .map(|r| r.provenance)
        .fold(Provenance::Trusted, Provenance::join);

    Trust {
        provenance,
        reasons,
    }
}

/// What `nikaia --explain --trust` prints (ADR-010 D7).
pub fn render(trust: &Trust) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "input provenance: {}\n",
        trust.provenance.as_str()
    ));

    if trust.reasons.is_empty() {
        out.push_str("    no source is read, so nothing entered from outside\n");
    }
    for reason in &trust.reasons {
        out.push_str(&format!(
            "    {} is {}\n",
            reason.source,
            reason.provenance.as_str()
        ));
    }

    out.push_str(&format!(
        "hash for a map keyed by the input: {}\n",
        match trust.provenance {
            Provenance::Trusted => "fast, fixed seed - no adversary chooses these keys",
            Provenance::Untrusted =>
                "keyed, per-process random seed - key choice cannot be aimed at the table",
        }
    ));
    out
}

/// Whether a call names something, for the walker below.
pub(super) fn call_name(parsed: &Parsed, expr: &Expr) -> Option<String> {
    let Expr::Call { func, .. } = expr else {
        return None;
    };
    match &**func {
        Expr::Variable(name) => Some(parsed.text(*name).to_string()),
        Expr::Path(segments) => Some(
            segments
                .iter()
                .map(|s| parsed.text(*s))
                .collect::<Vec<_>>()
                .join("::"),
        ),
        _ => None,
    }
}
