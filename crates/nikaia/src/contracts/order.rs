// crates/nikaia/src/contracts/order.rs
//
// Whether two statements have to keep the order they were written in
// (ADR-033, Part I 8.1.1).
//
// The rule is one sentence - *two operations whose touch sets are disjoint have
// no order between them* - and this file is the part of it that looks at a
// program rather than at a contract. It answers one question:
//
//     may these two adjacent statements overlap?
//
// **It says "no" for every reason it can think of, and for every reason it
// cannot.** That polarity is the decision (ADR-033 D4): a statement whose
// effects the compiler cannot enumerate keeps its position, so a program built
// against libraries that describe nothing behaves exactly as it does today.
// Every `false` below is either a real dependency or an admission of ignorance,
// and the two are deliberately worth the same.
//
// What this is *not*: a scheduler, a cost model, or an answer for a whole
// block. It is the first increment of ADR-033 §6 - two adjacent `let`s whose
// values are single calls - and everything it refuses today it refuses for a
// reason written down here rather than for lack of a case.

use std::collections::BTreeSet;

use crate::ast::{Expr, Item, Stmt};
use crate::parser::Parsed;

use super::touch::Reached;
use super::Ledger;

/// One statement, reduced to what deciding an order needs.
#[derive(Debug, Clone)]
pub struct Operation {
    /// The name it binds, where it binds one.
    pub binds: Option<String>,
    /// Every name its value mentions. A statement that mentions what the one
    /// before it bound is a data dependency, and nothing else has to be
    /// consulted.
    pub mentions: BTreeSet<String>,
    /// What the call it performs reaches, as the ledger describes it with the
    /// call's own arguments filled in.
    pub reaches: Vec<Reached>,
    /// The ledger key of the call, for a diagnostic that wants to name it.
    pub callee: String,
}

/// Reduce a statement to an [`Operation`], where it is one this can reason about.
///
/// `None` for everything else, and "everything else" is most of a language:
/// assignments, loops, returns, a `let` whose value is anything but one plain
/// call. Each of those could be handled and none of them is, because the first
/// increment is two `let`s and a case nobody has needed yet is a case nobody
/// has tested.
pub fn operation(
    parsed: &Parsed,
    stmt: &Stmt,
    own: &Ledger,
    library: &Ledger,
) -> Option<Operation> {
    match accounted(parsed, stmt, own, library) {
        Accounted::Operation(operation) => Some(operation),
        _ => None,
    }
}

/// A statement, reduced - or the reason it could not be.
///
/// The reason is the whole point (ADR-033 D9). Nikaia has no `allow_parallel`,
/// so the only thing standing between a refusal and a mystery is the compiler
/// being able to say which refusal it was.
#[derive(Debug, Clone)]
pub enum Accounted {
    Operation(Operation),
    /// Not a `let` whose value is one plain call.
    NotAPlainCall,
    /// Its `catch` handler can leave the function, so the statement after it is
    /// conditional on this one having succeeded (ADR-034).
    DivertingHandler,
    /// Nothing describes what the call reaches, so it reaches everything (D4).
    NoTouches(String),
    /// An argument that is not a literal, which the first increment will not
    /// send to another thread.
    NonLiteralArgument(String),
}

impl Accounted {
    /// One line, for a report a person reads - and, where there is one, the way
    /// out. A refusal a reader can act on is worth several they cannot.
    pub fn why(&self) -> String {
        match self {
            Accounted::Operation(_) => "it is an operation".to_string(),
            Accounted::NotAPlainCall => "one of them is not a `let` of a single call".to_string(),
            Accounted::DivertingHandler => {
                "its `catch` can leave the function, so the next statement might never have run \
                 - write the handler so it hands back a value instead, and check afterwards"
                    .to_string()
            }
            Accounted::NoTouches(name) => {
                format!("nothing says what `{name}` reaches, so it reaches everything")
            }
            Accounted::NonLiteralArgument(name) => {
                format!("`{name}` is given an argument that is not a literal")
            }
        }
    }
}

fn accounted(parsed: &Parsed, stmt: &Stmt, own: &Ledger, library: &Ledger) -> Accounted {
    let Stmt::Let {
        name, value, ty, ..
    } = stmt
    else {
        return Accounted::NotAPlainCall;
    };
    // A written type would have to be carried onto one element of a tuple
    // pattern. Nothing needs it yet.
    if ty.is_some() {
        return Accounted::NotAPlainCall;
    }

    // A real program writes `fs::read_to_string(p) catch { … }`, so the call is
    // usually wrapped. Looking through the wrapper is what makes this apply to
    // code anyone actually writes.
    //
    // **But only where the handler cannot divert.** A handler that `return`s
    // makes the *next* statement conditional on this one having succeeded, and
    // ADR-033 D5 forbids running a conditional operation early - overlapping
    // them would perform a read the sequential program would never have
    // performed. That case was not in D5 when it was written; it is the first
    // thing building this found, and it is recorded in ADR-034.
    let value = match value {
        Expr::TryCatch { expr, handler } if !diverts(&handler.stmts) => &**expr,
        Expr::TryCatch { .. } => return Accounted::DivertingHandler,
        other => other,
    };

    let Expr::Call { func, args, config } = value else {
        return Accounted::NotAPlainCall;
    };
    // Kap 5.1's options are values like any other and would have to be walked
    // for dependencies. They are not, so a call that uses them is refused.
    if !config.is_empty() {
        return Accounted::NotAPlainCall;
    }

    let callee = match &**func {
        Expr::Variable(name) => parsed.text(*name).to_string(),
        Expr::Path(segments) => segments
            .iter()
            .map(|s| parsed.text(*s))
            .collect::<Vec<_>>()
            .join("::"),
        _ => return Accounted::NotAPlainCall,
    };

    // The contract has to be found *and* has to describe its effects. An entry
    // without `touches` is the absence of an answer (ADR-033 D4).
    let Some((key, contract)) = own
        .lookup(&callee)
        .or_else(|| library.lookup(&callee))
        .filter(|(_, contract)| contract.touches_known)
    else {
        return Accounted::NoTouches(callee);
    };

    // Which argument goes with which parameter, so that `file(path)` can be
    // turned into "the file named by this call's first argument".
    let Some(signature) = contract.signature.as_ref() else {
        return Accounted::NoTouches(key);
    };
    let parameters: Vec<&str> = signature
        .arguments()
        .iter()
        .map(|(name, _)| name.as_str())
        .collect();

    let reaches = contract
        .touches
        .iter()
        .map(|touch| {
            let named = touch.parameter.as_ref().and_then(|parameter| {
                let at = parameters.iter().position(|p| p == parameter)?;
                literal_text(parsed, args.get(at)?)
            });
            Reached {
                kind: touch.kind.clone(),
                // A resource with a parameter whose argument could not be read
                // is one this compiler cannot name, and `unknown` is what says
                // so: it then conflicts with its whole kind.
                unknown: touch.parameter.is_some() && named.is_none(),
                named,
                write: touch.write,
            }
        })
        .collect();

    // **Every** argument a literal, not only the one that names the resource.
    // The two calls are lowered into closures that run on other threads, and a
    // closure that captures nothing cannot capture something that must not
    // cross one. It is the first increment's restriction (ADR-033 §6) and the
    // cheapest possible answer to a question - what may be sent - that deserves
    // its own decision rather than an implicit one here.
    if !args.iter().all(is_literal) {
        return Accounted::NonLiteralArgument(key);
    }

    let mut mentions = BTreeSet::new();
    for arg in args {
        names_in(parsed, arg, &mut mentions);
    }

    Accounted::Operation(Operation {
        binds: Some(parsed.text(*name).to_string()),
        mentions,
        reaches,
        callee: key,
    })
}

/// Why two statements keep the order they were written in - or that they need
/// not (ADR-033 D9).
///
/// The analysis knew every one of these and threw all but the boolean away. It
/// is kept because the decision *not* to give the language a word for "run
/// these together anyway" is only defensible if the compiler can say what it
/// refused and why: a silent refusal with no way to ask is the trap the
/// keyword would have been an escape from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// They meet on nothing and neither waits for the other.
    Overlap,
    /// `later` uses what `earlier` bound.
    DataDependency(String),
    /// Both bind the same name, so the order decides which value survives.
    Shadowed(String),
    /// Their touch sets meet on something one of them writes.
    SameResource { kind: String, named: Option<String> },
    /// One of them is not a statement this analysis can account for at all -
    /// a call it cannot resolve, a handler that can leave the function, an
    /// argument that is not a literal. An admission of ignorance, and it keeps
    /// the order for the same reason a real dependency does (D4).
    NotAccountedFor,
}

impl Verdict {
    pub fn is_overlap(&self) -> bool {
        matches!(self, Verdict::Overlap)
    }

    /// One line, for a report a person reads.
    pub fn why(&self) -> String {
        match self {
            Verdict::Overlap => "they meet on nothing".to_string(),
            Verdict::DataDependency(name) => format!("the second uses `{name}`"),
            Verdict::Shadowed(name) => format!("both bind `{name}`"),
            Verdict::SameResource {
                kind,
                named: Some(named),
            } => {
                format!("both reach {kind} `{named}`, and one writes it")
            }
            Verdict::SameResource { kind, named: None } => {
                format!("both reach a {kind} this compiler cannot name, and one writes it")
            }
            Verdict::NotAccountedFor => {
                "one of them is not something this compiler can account for".to_string()
            }
        }
    }
}

/// Whether `later` may run at the same time as `earlier`, and why not.
///
/// Both halves of the rule, in the order they are cheapest to refuse:
/// a **data** dependency - `later` uses what `earlier` bound - and an **effect**
/// dependency, where their touch sets meet on something one of them writes.
pub fn verdict(earlier: &Operation, later: &Operation) -> Verdict {
    if let Some(bound) = &earlier.binds {
        if later.mentions.contains(bound) {
            return Verdict::DataDependency(bound.clone());
        }
    }
    // Two `let`s of the same name would make the order decide which value
    // survives. The parser allows shadowing, so this is reachable.
    if earlier.binds.is_some() && earlier.binds == later.binds {
        return Verdict::Shadowed(earlier.binds.clone().unwrap_or_default());
    }

    for a in &earlier.reaches {
        for b in &later.reaches {
            if a.conflicts_with(b) {
                // The one that is named is the more useful half to print; where
                // neither is, saying so is the point.
                let named = a
                    .named
                    .clone()
                    .filter(|_| !a.unknown)
                    .or_else(|| b.named.clone().filter(|_| !b.unknown));
                return Verdict::SameResource {
                    kind: a.kind.clone(),
                    named,
                };
            }
        }
    }

    Verdict::Overlap
}

/// The same question as a boolean, for a caller that only has to decide.
pub fn may_overlap(earlier: &Operation, later: &Operation) -> bool {
    verdict(earlier, later).is_overlap()
}

/// Whether an argument is a literal - something with no name in it at all.
///
/// `LitInterpolated` is deliberately **not** one: `f"{path}.log"` has a name in
/// it, and a name is the thing this asks about.
fn is_literal(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::LitStr(_) | Expr::LitInt(_) | Expr::LitFloat(_) | Expr::LitBool(_) | Expr::LitChar(_)
    )
}

/// Whether a block can leave the function it is in.
///
/// Conservative and shallow on purpose: any `return` or `throw` anywhere in it,
/// however deeply nested, counts. A handler that merely supplies a fallback
/// value does not, and that is the case this exists to let through.
fn diverts(stmts: &[crate::ast::Spanned<Stmt>]) -> bool {
    stmts.iter().any(|stmt| match &stmt.node {
        Stmt::Return(_) => true,
        Stmt::Expr(expr) | Stmt::Let { value: expr, .. } => holds_throw(expr),
        Stmt::Assign { value, .. } => holds_throw(value),
        Stmt::For { body, .. } | Stmt::While { body, .. } => diverts(&body.stmts),
    })
}

/// Whether an expression can throw out of the block it is in.
fn holds_throw(expr: &Expr) -> bool {
    match expr {
        Expr::Throw(_) => true,
        Expr::Block(block) => diverts(&block.stmts),
        Expr::If {
            then_branch,
            else_branch,
            ..
        } => {
            diverts(&then_branch.stmts)
                || else_branch
                    .as_ref()
                    .is_some_and(|block| diverts(&block.stmts))
        }
        Expr::Match { arms, .. } => arms.iter().any(|arm| holds_throw(&arm.body)),
        _ => false,
    }
}

/// The text of a literal argument, where the argument is one.
///
/// Only a literal. `fs::read(pfad)` names a file this compiler cannot identify,
/// and ADR-033 D4 says what happens then - it is not that the compiler guesses.
fn literal_text(parsed: &Parsed, expr: &Expr) -> Option<String> {
    let _ = parsed;
    match expr {
        Expr::LitStr(text) => Some(text.clone()),
        // An `f"…"` is not a constant: what it says depends on what its holes
        // hold, and this wants the text of a file name written down.
        _ => None,
    }
}

/// Every name an expression mentions.
///
/// Deliberately over-approximate: a field access `a.b` contributes `a`, and a
/// name that happens to be a function's rather than a variable's is counted
/// too. Both make the answer "keep the order", which is the safe direction.
fn names_in(parsed: &Parsed, expr: &Expr, out: &mut BTreeSet<String>) {
    match expr {
        Expr::Variable(name) => {
            out.insert(parsed.text(*name).to_string());
        }
        Expr::Path(segments) => {
            if let Some(first) = segments.first() {
                out.insert(parsed.text(*first).to_string());
            }
        }
        Expr::Call { func, args, config } => {
            names_in(parsed, func, out);
            args.iter().for_each(|a| names_in(parsed, a, out));
            config.iter().for_each(|c| names_in(parsed, &c.value, out));
        }
        Expr::MethodCall {
            receiver,
            args,
            method,
        } => {
            names_in(parsed, receiver, out);
            out.insert(parsed.text(*method).to_string());
            args.iter().for_each(|a| names_in(parsed, a, out));
        }
        Expr::Field { base, .. } => names_in(parsed, base, out),
        Expr::Binary { lhs, rhs, .. } => {
            names_in(parsed, lhs, out);
            names_in(parsed, rhs, out);
        }
        Expr::Unary { expr, .. } | Expr::Try(expr) | Expr::Cast { expr, .. } => {
            names_in(parsed, expr, out)
        }
        Expr::Index { base, index } => {
            names_in(parsed, base, out);
            names_in(parsed, index, out);
        }
        Expr::Tuple(parts) => parts.iter().for_each(|p| names_in(parsed, p, out)),
        Expr::Coalesce { value, fallback } => {
            names_in(parsed, value, out);
            names_in(parsed, fallback, out);
        }
        // Anything with a block in it is not an [`Operation`] in the first
        // place, so a name inside one never has to be found here.
        _ => {}
    }
}

/// Every adjacent pair in a program, and what was decided about it (ADR-033 D9).
///
/// The answer to "why did these two not run together", which is the question a
/// language without an `allow_parallel` owes its user. Nothing prints it on its
/// own: it is asked for (`--overlaps`), because a compiler that volunteered a
/// paragraph per pair would be noise in exactly the programs that are fine.
pub fn report(parsed: &Parsed, own: &Ledger, library: &Ledger) -> String {
    let mut out = String::new();

    for item in &parsed.program.items {
        match &item.node {
            Item::Fn { .. } => function_report(parsed, &item.node, None, own, library, &mut out),
            Item::Impl {
                target, methods, ..
            } => {
                let target = parsed.text(target.name).to_string();
                for method in methods {
                    function_report(parsed, &method.node, Some(&target), own, library, &mut out);
                }
            }
            _ => {}
        }
    }

    if out.is_empty() {
        out.push_str("no two adjacent statements in this program were compared.\n");
    }
    out
}

fn function_report(
    parsed: &Parsed,
    item: &Item,
    target: Option<&str>,
    own: &Ledger,
    library: &Ledger,
    out: &mut String,
) {
    let Item::Fn { name, body, .. } = item else {
        return;
    };
    let own_name = match name {
        Some(name) => parsed.text(*name).to_string(),
        None => "new".to_string(),
    };
    let key = match target {
        Some(target) => format!("{target}::{own_name}"),
        None => own_name,
    };

    let mut lines = Vec::new();
    for pair in body.stmts.windows(2) {
        let earlier = accounted(parsed, &pair[0].node, own, library);
        let later = accounted(parsed, &pair[1].node, own, library);

        let (mark, what, why) = match (&earlier, &later) {
            (Accounted::Operation(earlier), Accounted::Operation(later)) => {
                let verdict = verdict(earlier, later);
                let mark = if verdict.is_overlap() {
                    "together"
                } else {
                    "in order"
                };
                (
                    mark,
                    format!("{} / {}", earlier.callee, later.callee),
                    verdict.why(),
                )
            }
            // One of the two could not be reduced at all, and *that* reason is
            // the one worth printing: it is the one a reader can usually act on.
            (
                refused @ (Accounted::NotAPlainCall
                | Accounted::DivertingHandler
                | Accounted::NoTouches(_)
                | Accounted::NonLiteralArgument(_)),
                other,
            )
            | (other @ Accounted::Operation(_), refused) => {
                let named = match other {
                    Accounted::Operation(operation) => operation.callee.clone(),
                    _ => "…".to_string(),
                };
                ("in order", named, refused.why())
            }
        };
        lines.push(format!("    {mark:9} {what} - {why}"));
    }

    if !lines.is_empty() {
        out.push_str(&format!("{key}:\n"));
        for line in lines {
            out.push_str(&line);
            out.push('\n');
        }
    }
}
