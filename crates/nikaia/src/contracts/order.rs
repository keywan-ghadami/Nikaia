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
// block. It is ADR-033 §6 - a pair of adjacent statements - and everything it
// refuses today it refuses for a reason written down here rather than for lack
// of a case.
//
// **The shapes it sees** (ADR-033 §8.3's first item). The first increment read
// one shape: a `let` bound to exactly one call. Measuring it found that 101 of
// 127 refused pairs in `examples/` fell out on that alone, before any `touches`
// set was consulted - a zero that measured the analysis rather than the corpus.
// So a statement is now reduced where it is
//
//   * a **`let`**, as before, or a **bare expression statement**: `println(x)`,
//     `fs::write(p, d)` - an operation the ledger can account for that nothing
//     binds;
//   * built out of **literals and calls**, at any depth, rather than being one
//     call: `f("a") + g("b")` performs two operations and its touch set is
//     their union.
//
// Everything else is still refused, and the refusals are now *about the
// program* rather than about the analysis not looking: a method call whose
// ledger key needs the type checker, an argument that is not a literal, a
// callee nobody described.

use std::collections::BTreeSet;

use crate::ast::{BinaryOp, Expr, Item, Stmt};
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
/// `None` for everything else: assignments, loops, returns, and any value this
/// analysis will not take apart. An assignment is deliberately among them and
/// will stay there - `x = 1` changes a name without binding one, so the data
/// dependency that [`verdict`] finds by comparing `binds` against `mentions`
/// would not be found at all.
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
    /// It performs nothing at all: a loop, an `if`, an assignment, a `return`,
    /// a `let` of a value that calls nothing.
    ///
    /// The one refusal that is about the **statement** rather than about what
    /// the compiler knows. `let n = 0` has no touch set because it reaches
    /// nothing, and pairing it with the operation next to it would ask a
    /// thread to carry a constant.
    NotAnOperation,
    /// It performs something this analysis will not take apart, named so the
    /// report says *which* thing.
    Opaque(&'static str),
    /// Its `catch` handler can leave the function, so the statement after it is
    /// conditional on this one having succeeded (ADR-034).
    DivertingHandler,
    /// It can fail and nothing catches the failure, so the failure leaves the
    /// function - and the statement after it is conditional on this one having
    /// succeeded, exactly as a diverting handler makes it (ADR-034 D2).
    ///
    /// The same rule as [`Accounted::DivertingHandler`] reached from the other
    /// side, and it has to be here: an uncaught `fs::write(…)` is precisely the
    /// shape a bare expression statement makes common.
    UncaughtFailure(String),
    /// Nothing describes what the call reaches, so it reaches everything (D4).
    NoTouches(String),
    /// A value that is not a literal, which this increment will not send to
    /// another thread.
    ///
    /// Each operation is lowered into a closure that runs somewhere else, and a
    /// closure that captures nothing cannot capture something that must not
    /// cross a thread. What may cross one is a decision of its own and not an
    /// implicit answer here - D9's table calls it "an implementation limit, not
    /// a language question", and it is the one refusal in that table a wider
    /// analysis alone cannot lift.
    NonLiteralArgument(String),
}

impl Accounted {
    /// One line, for a report a person reads - and, where there is one, the way
    /// out. A refusal a reader can act on is worth several they cannot.
    pub fn why(&self) -> String {
        match self {
            Accounted::Operation(_) => "it is an operation".to_string(),
            Accounted::NotAnOperation => "one of them performs no operation at all".to_string(),
            Accounted::Opaque(what) => format!("one of them holds {what}"),
            Accounted::DivertingHandler => {
                "its `catch` can leave the function, so the next statement might never have run \
                 - write the handler so it hands back a value instead, and check afterwards"
                    .to_string()
            }
            Accounted::UncaughtFailure(name) => {
                format!(
                    "`{name}` can fail and nothing catches it, so the failure leaves the function \
                     and the next statement might never have run - `catch` it into a value, and \
                     check afterwards"
                )
            }
            Accounted::NoTouches(name) => {
                format!("nothing says what `{name}` reaches, so it reaches everything")
            }
            Accounted::NonLiteralArgument(name) => {
                format!("`{name}` is not a literal, and only literals are sent to another thread")
            }
        }
    }
}

fn accounted(parsed: &Parsed, stmt: &Stmt, own: &Ledger, library: &Ledger) -> Accounted {
    // Two shapes, and the second is ADR-033 §8.3's first item: an operation
    // the ledger can account for is not always bound to a name. `println(x)`,
    // `out.push(y)` and `fs::write(p, d)` are statements a program is mostly
    // made of, and reading only `let` is what produced §8.1's 101.
    //
    // An **assignment** is not among them and must not be: `x = 1` changes a
    // name without binding one, so [`verdict`]'s data-dependency test - does
    // the later statement mention what the earlier one bound - would miss it
    // entirely. A shape whose dependencies this cannot see is a shape it may
    // not read.
    let (binds, value) = match stmt {
        Stmt::Let {
            name, value, ty, ..
        } => {
            // A written type would have to be carried onto one element of a
            // tuple pattern. Nothing needs it yet.
            if ty.is_some() {
                return Accounted::Opaque(
                    "a written type, which would have to be carried onto one half of a pattern",
                );
            }
            (Some(parsed.text(*name).to_string()), value)
        }
        Stmt::Expr(value) => (None, value),
        _ => return Accounted::NotAnOperation,
    };

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
    let (value, handler, caught) = match value {
        Expr::TryCatch { handler, .. } if diverts(&handler.stmts) => {
            return Accounted::DivertingHandler
        }
        Expr::TryCatch { expr, handler } => (&**expr, Some(handler), true),
        other => (other, None, false),
    };

    // What the statement is made of. Literals and calls, at any depth - a `let`
    // whose initialiser is not a bare call is the other half of §8.3's first
    // item, and `f("a") + g("b")` performs two operations whose touch sets
    // union.
    let mut walked = Walked::default();
    walk(parsed, value, &mut walked);
    // **And the handler's own effects, not only the names it mentions.** The
    // analysis looks *past* a `catch` at the call it guards, so without this a
    // handler that writes a file the next statement reads would be invisible
    // and the pair would overlap - the emitted program producing a different
    // result from the sequential one. A handler is part of the statement, so
    // what it reaches is part of what the statement touches; where that cannot
    // be read, D4 says the statement touches everything and stays put.
    if let Some(handler) = handler {
        walk_block(parsed, handler, &mut walked);
    }
    if walked.calls.is_empty() && !walked.performs {
        // Nothing here reaches the world at all. Not an admission of ignorance:
        // the statement genuinely is not an operation, and saying so keeps a
        // constant out of a thread.
        return Accounted::NotAnOperation;
    }

    // The ledger is consulted **before** the walk's own refusal is reported,
    // and the order is the point: "nothing says what `x` reaches" is a gap
    // somebody can close by writing a contract, where "it holds a method call"
    // is this compiler's own limit. Of two true answers, the one a reader can
    // act on is the one worth printing (D9).
    let mut reaches = Vec::new();
    let mut named_by = Vec::new();
    for call in &walked.calls {
        let Expr::Call { args, .. } = call else {
            unreachable!("`walk` collects only calls");
        };
        let Some(callee) = callee_of(parsed, call) else {
            unreachable!("`walk` refuses a call through anything but a name");
        };

        // The contract has to be found *and* has to describe its effects. An
        // entry without `touches` is the absence of an answer (ADR-033 D4).
        let Some((key, contract)) = own
            .lookup(&callee)
            .or_else(|| library.lookup(&callee))
            .filter(|(_, contract)| contract.touches_known)
        else {
            return Accounted::NoTouches(callee);
        };

        // A failure nobody catches leaves the function, which makes the *next*
        // statement conditional on this one having succeeded - the same reason
        // a diverting handler is refused, and ADR-034 D2 in general (D5's "no
        // speculation" had this case too). Starting the next operation early
        // would perform work the program as written might never have performed.
        if !caught && !contract.throws.is_empty() {
            return Accounted::UncaughtFailure(key);
        }

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

        reaches.extend(contract.touches.iter().map(|touch| {
            let named = touch.parameter.as_ref().and_then(|parameter| {
                let at = parameters.iter().position(|p| p == parameter)?;
                literal_text(args.get(at)?)
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
        }));
        named_by.push(key);
    }

    // A method call is accounted for only where **every** entry of that name
    // reaches nothing. One that may reach the world leaves this analysis unable
    // to say which resource, and D4's answer to "which" being unanswerable is
    // that it is all of them - so the statement stays where it was written.
    for method in &walked.methods {
        let candidates = {
            let mine = own.candidates(method);
            if mine.is_empty() {
                library.candidates(method)
            } else {
                mine
            }
        };
        if candidates.is_empty()
            || candidates
                .iter()
                .any(|(_, contract)| !contract.touches_known || !contract.touches.is_empty())
        {
            return Accounted::NoTouches(method.clone());
        }
    }

    if let Some(refusal) = walked.refused {
        return refusal;
    }

    let mut mentions = BTreeSet::new();
    names_in(parsed, value, &mut mentions);
    // **The handler counts too.** `f("a") catch { w }` is one expression, and
    // `w` is a name it mentions - so if the statement before it bound `w`,
    // that is a data dependency like any other. Looking only past the `catch`
    // would miss it, and the lowering puts the handler in the same closure.
    if let Some(handler) = handler {
        names_in_block(parsed, handler, &mut mentions);
    }

    Accounted::Operation(Operation {
        binds,
        mentions,
        reaches,
        callee: named_by.join(" + "),
    })
}

/// What walking a statement's value found (ADR-033 §8.3, first item).
///
/// Three answers rather than one, because "this performs nothing" and "this
/// performs something I will not look inside" are different facts and the
/// report is only useful if it can tell them apart. A `let n = 0` is the first;
/// a `let n = xs.len()` is the second.
#[derive(Default)]
struct Walked<'a> {
    /// Every [`Expr::Call`] it performs, at whatever depth.
    calls: Vec<&'a Expr>,
    /// It performs *something* - a method call, a block, a grammar - even where
    /// this could not say what. Set wherever the walk stops descending, so that
    /// an unreadable operation is never mistaken for no operation.
    performs: bool,
    /// Every method called, by name. Which ledger entry each *is* needs the
    /// type checker (ADR-028); whether they all reach nothing does not, and
    /// that weaker question is answered in [`accounted`].
    methods: Vec<String>,
    /// Why it could not be taken apart, where it could not. The first reason
    /// only: a refusal is a refusal, and a list of them would be noise.
    refused: Option<Accounted>,
}

impl Walked<'_> {
    /// Record a reason, and say nothing about whether anything was performed.
    /// For a node whose children the walk still descends into.
    fn note(&mut self, why: Accounted) {
        if self.refused.is_none() {
            self.refused = Some(why);
        }
    }

    /// Record a reason for a node the walk stops at. Stopping is exactly when
    /// "performs something" has to be assumed: what is inside was not read.
    fn refuse(&mut self, why: Accounted) {
        self.performs = true;
        self.note(why);
    }
}

/// The same, for the statements of a `catch` handler.
///
/// A handler is code that runs, so its calls belong in the statement's touch
/// set. Only the two shapes whose value `walk` can take apart are read; every
/// other statement in a handler stops the walk, which is what makes the
/// handler's effects unknown rather than empty (D4).
fn walk_block<'a>(parsed: &Parsed, block: &'a crate::ast::Block, out: &mut Walked<'a>) {
    for stmt in &block.stmts {
        match &stmt.node {
            Stmt::Let { value, .. } | Stmt::Expr(value) => walk(parsed, value, out),
            _ => out.refuse(Accounted::Opaque(
                "a `catch` handler doing more than handing back a value",
            )),
        }
    }
}

/// Take a statement's value apart into the calls it performs.
///
/// **The whole expression, not only the arguments.** `f("a") + n` would be
/// lowered into a closure that captures `n`, so a name anywhere in it is the
/// same question the literal-argument rule asks about an argument, and gets the
/// same answer.
///
/// What is walked is the fail-closed half of the design: literals, calls, and
/// operators over them. Everything else records a reason and stops, which makes
/// adding a shape a deliberate act rather than the consequence of a `_` arm.
fn walk<'a>(parsed: &Parsed, expr: &'a Expr, out: &mut Walked<'a>) {
    match expr {
        // A value with no name in it and nothing to evaluate.
        Expr::LitInt(_)
        | Expr::LitFloat(_)
        | Expr::LitBool(_)
        | Expr::LitChar(_)
        | Expr::LitStr(_) => {}

        Expr::Call { args, config, .. } => {
            // Kap 5.1's options are values like any other and would have to be
            // matched against the callee's declaration order before a `touches`
            // parameter could be read off them. They are not, so a call that
            // uses them is refused.
            if !config.is_empty() {
                out.refuse(Accounted::Opaque(
                    "a call with options, which nothing matches against the callee's order yet",
                ));
                return;
            }
            if callee_of(parsed, expr).is_none() {
                out.refuse(Accounted::Opaque(
                    "a call through something that is not a name",
                ));
                return;
            }
            for arg in args {
                walk(parsed, arg, out);
            }
            out.calls.push(expr);
        }

        // Pure structure over the values above. The arithmetic itself reaches
        // nothing, so what the statement touches is what its calls touch.
        Expr::Unary { expr, .. } | Expr::Cast { expr, .. } => walk(parsed, expr, out),
        Expr::Binary { op, lhs, rhs } => {
            // `&&` and `||` evaluate their right side only sometimes, and an
            // operation that only sometimes runs may not be started early
            // (ADR-033 D5).
            if matches!(op, BinaryOp::And | BinaryOp::Or) {
                out.refuse(Accounted::Opaque(
                    "a `&&` or `||`, whose right side runs only sometimes",
                ));
                return;
            }
            walk(parsed, lhs, out);
            walk(parsed, rhs, out);
        }
        Expr::Tuple(parts) => parts.iter().for_each(|part| walk(parsed, part, out)),

        // A name that is not a callee's is a value from somewhere else, and the
        // closure this is lowered into would have to capture it. It performs
        // nothing on its own, so `let b = a` still reports as no operation
        // rather than as a capture this increment will not make.
        Expr::Variable(name) => {
            out.note(Accounted::NonLiteralArgument(
                parsed.text(*name).to_string(),
            ));
        }
        // A value read from somewhere else. Reaching it is not the problem -
        // carrying it to another thread is, and that is the same decision the
        // literal rule defers. The walk goes on through it, so that
        // `f("a").field` is still known to perform `f`.
        Expr::Field { base, .. } => {
            out.note(Accounted::Opaque("a value read from somewhere else"));
            walk(parsed, base, out);
        }
        Expr::Index { base, index } => {
            out.note(Accounted::Opaque("a value read from somewhere else"));
            walk(parsed, base, out);
            walk(parsed, index, out);
        }
        Expr::Path(_) => out.note(Accounted::Opaque("a value read from somewhere else")),
        Expr::StructLit { fields, .. } => {
            out.note(Accounted::Opaque("a value read from somewhere else"));
            for field in fields {
                if let Some(value) = &field.value {
                    walk(parsed, value, out);
                }
            }
        }
        Expr::Range { start, end, .. } => {
            out.note(Accounted::Opaque("a value read from somewhere else"));
            walk(parsed, start, out);
            walk(parsed, end, out);
        }
        // `a ?? b` runs `b` only when `a` had nothing, which is D5's case again.
        Expr::Coalesce { value, fallback } => {
            out.note(Accounted::Opaque(
                "a `??`, whose fallback runs only sometimes",
            ));
            walk(parsed, value, out);
            walk(parsed, fallback, out);
        }

        // --- and the nodes the walk stops at -------------------------------

        // Text with code in it (ADR-035). What it says depends on what its holes
        // hold, and the holes are Nikaia this has not parsed - so what they call
        // is unknown, which is the reason this counts as performing something.
        Expr::LitInterpolated(_) => out.refuse(Accounted::Opaque(
            "text with code in it, whose holes this has not parsed",
        )),
        // *Which* ledger entry `xs.len()` is depends on what `xs` is, and that
        // is the type checker's answer rather than this one's (ADR-028). The
        // weaker question is answerable here: if every `::len` in the ledger
        // reaches nothing, this reaches nothing whatever the receiver is. So
        // the name is recorded and `accounted` decides; anything that may reach
        // the world is refused there (D4).
        Expr::MethodCall {
            receiver,
            method,
            args,
            config,
        } => {
            walk(parsed, receiver, out);
            // Kap 5.1's options are values like any other, and a call this
            // walk did not reach into is a call whose names it does not know.
            for value in args.iter().chain(config.iter().map(|option| &option.value)) {
                walk(parsed, value, out);
            }
            out.methods.push(parsed.text(*method).to_string());
        }
        // Everything with its own control flow: what runs inside it is decided
        // while it runs, and D5 allows only operations that certainly run.
        _ => out.refuse(Accounted::Opaque("something with its own control flow")),
    }
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

/// The ledger key of the call an expression performs, where it performs one.
///
/// One place, so a refusal names the same call the happy path would have.
fn callee_of(parsed: &Parsed, expr: &Expr) -> Option<String> {
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
fn literal_text(expr: &Expr) -> Option<String> {
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
///
/// **Total, with no `_` arm**, and that is load-bearing rather than tidy. This
/// is asked about a `catch` handler as well as about a value, and a handler
/// holds whatever anyone writes - a block, an `f"…"`, a grammar. A variant this
/// walked past silently would be a data dependency it could not see, which is
/// the one direction the analysis may never fail in (D9's first row: "no, and
/// must not"). Where the names cannot be found structurally - inside the holes
/// of an interpolated string, inside a `dsl` body - every word of the raw text
/// counts as one.
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
            config,
        } => {
            names_in(parsed, receiver, out);
            out.insert(parsed.text(*method).to_string());
            args.iter().for_each(|a| names_in(parsed, a, out));
            // What stands after a method call's `;` reads names like anything
            // else - a DSL's deferred parameters arrive there (ADR-007 D5),
            // and a name read inside one is read.
            config.iter().for_each(|c| names_in(parsed, &c.value, out));
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
        Expr::Range { start, end, .. } => {
            names_in(parsed, start, out);
            names_in(parsed, end, out);
        }
        Expr::StructLit { name, fields } => {
            out.insert(parsed.text(*name).to_string());
            for field in fields {
                out.insert(parsed.text(field.name).to_string());
                if let Some(value) = &field.value {
                    names_in(parsed, value, out);
                }
            }
        }
        Expr::Block(block) | Expr::Closure { body: block, .. } => {
            names_in_block(parsed, block, out)
        }
        Expr::If {
            cond,
            then_branch,
            else_branch,
        } => {
            names_in(parsed, cond, out);
            names_in_block(parsed, then_branch, out);
            if let Some(block) = else_branch {
                names_in_block(parsed, block, out);
            }
        }
        Expr::Match { value, arms } => {
            names_in(parsed, value, out);
            for arm in arms {
                if let crate::ast::MatchPattern::Literal(pattern) = &arm.pattern {
                    names_in(parsed, pattern, out);
                }
                names_in(parsed, &arm.body, out);
            }
        }
        Expr::Spawn { body, .. } | Expr::Throw(body) => names_in(parsed, body, out),
        Expr::TryCatch { expr, handler } => {
            names_in(parsed, expr, out);
            names_in_block(parsed, handler, out);
        }
        // The holes of an `f"…"` are Nikaia this has not parsed, and a `dsl`
        // body is a foreign syntax whose actions are Nikaia too. Every word in
        // the raw text counts, which finds every name they could hold and a
        // good many they could not.
        Expr::LitInterpolated(text) => words_in(text, out),
        Expr::Dsl {
            target,
            context,
            content,
        } => {
            out.insert(parsed.text(*target).to_string());
            if let Some(context) = context {
                out.insert(parsed.text(*context).to_string());
            }
            words_in(content, out);
        }
        Expr::DslFrom { grammar, input } => {
            out.insert(parsed.text(*grammar).to_string());
            names_in(parsed, input, out);
        }
        Expr::Asm { bindings, code } => {
            for binding in bindings {
                out.insert(parsed.text(binding.variable).to_string());
            }
            words_in(code, out);
        }
        // A value with no name in it, and the only arms that may say nothing.
        Expr::LitInt(_)
        | Expr::LitFloat(_)
        | Expr::LitBool(_)
        | Expr::LitChar(_)
        | Expr::LitStr(_) => {}
    }
}

/// Every name the statements of a block mention.
///
/// The names a statement *binds* are left out: a `let` inside a block
/// introduces a name rather than reading one, so counting it would only refuse
/// pairs that have nothing to do with each other.
fn names_in_block(parsed: &Parsed, block: &crate::ast::Block, out: &mut BTreeSet<String>) {
    for stmt in &block.stmts {
        match &stmt.node {
            Stmt::Let { value, .. } | Stmt::Expr(value) => names_in(parsed, value, out),
            Stmt::Assign { target, value, .. } => {
                names_in(parsed, target, out);
                names_in(parsed, value, out);
            }
            Stmt::For { iter, body, .. } => {
                names_in(parsed, iter, out);
                names_in_block(parsed, body, out);
            }
            Stmt::While { cond, body } => {
                names_in(parsed, cond, out);
                names_in_block(parsed, body, out);
            }
            Stmt::Return(value) => {
                if let Some(value) = value {
                    names_in(parsed, value, out);
                }
            }
        }
    }
}

/// Every word of a piece of raw text, for the places a name cannot be found by
/// walking. Over-approximate by construction, which is the safe direction.
fn words_in(text: &str, out: &mut BTreeSet<String>) {
    for word in text.split(|c: char| !c.is_alphanumeric() && c != '_') {
        if !word.is_empty() {
            out.insert(word.to_string());
        }
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
    let Item::Fn {
        name,
        body,
        ret_type,
        ..
    } = item
    else {
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

    // The last statement of a value-returning body is the value (Kap 3.1). It
    // is reported, because a reader asking why two lines did not run together
    // deserves an answer for every pair - but the answer is its own, and
    // neither statement's fault.
    let value_at = ret_type
        .as_ref()
        .and_then(|_| body.stmts.len().checked_sub(1));

    let mut lines = Vec::new();
    for (at, pair) in body.stmts.windows(2).enumerate() {
        let earlier = accounted(parsed, &pair[0].node, own, library);
        let later = accounted(parsed, &pair[1].node, own, library);

        if value_at == Some(at + 1) {
            let named = match (&earlier, &later) {
                (Accounted::Operation(earlier), Accounted::Operation(later)) => {
                    format!("{} / {}", earlier.callee, later.callee)
                }
                _ => "…".to_string(),
            };
            lines.push(format!(
                "    {:9} {named} - the second is what this function hands back, and a pair \
                 hands back a tuple",
                "in order"
            ));
            continue;
        }

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
                refused @ (Accounted::NotAnOperation
                | Accounted::Opaque(_)
                | Accounted::DivertingHandler
                | Accounted::UncaughtFailure(_)
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
