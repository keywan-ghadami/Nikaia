//! Running Nikaia while the program is built
//! ([ADR-073](../../../docs/specification/adr/adr-073.md) D5's second stage).
//!
//! `comptime` has had an evaluator since the word existed, and what it knew was
//! an integer literal, a name whose value already folded, a negation and
//! `+ - * / %` — [`fold`](crate::fold), 124 lines with no call in it. That is
//! D5's **first** stage, and the second one was written down as *when Q4 is
//! answered*: a call, and with it the file reading
//! [ADR-072](../../../docs/specification/adr/adr-072.md) waits behind.
//!
//! Q4 **is** answered — [ADR-075](../../../docs/specification/adr/adr-075.md)
//! D1 and D2 say what a build-time body may do — so this is the call, and with
//! it the **loop**: a `for` over a range, a `while`, `break`, `continue` and an
//! assignment, because a loop that cannot change anything is not one.
//!
//! **What a block means had to grow a third answer.** It used to be *a value or
//! an error*; a loop's body is neither — it runs to its end and produces
//! nothing, which is ordinary — so [`Flow`] names the four ways out and the
//! frame is shared rather than owned, since a `for` has to see what its body
//! assigned on the last turn.
//!
//! **What bounds it is a ledger column and not a list of allowed functions.** A
//! callee must be `sync` (D1) and its touch set must be empty or exactly the
//! build's own parameters (D2), and both are answers the compiler already
//! derives for every function it sees. Nothing here decides what is safe; it
//! reads what was decided.
//!
//! **There is no step budget** (D4), deliberately and with the cost written
//! down: a body that does not terminate hangs the build. What *is* bounded is
//! the **call depth**, and that is a different thing — an unbounded recursion
//! would overflow this compiler's own stack, and a compiler that falls over is
//! not the hang D4 accepted. The limit is high enough that no terminating
//! program meets it and the message says which call path found it.

use std::collections::BTreeMap;

use crate::ast::{BinaryOp, Block, Expr, Item, Stmt, UnaryOp};
use crate::contracts::{touch, Ledger};
use crate::parser::Parsed;

/// How deep a build-time call may go before this stops
/// ([ADR-075](../../../docs/specification/adr/adr-075.md) D4's neighbour).
///
/// **Not a step budget.** A body that loops forever still hangs the build, which
/// is what that record accepted; what this prevents is a *recursion* that takes
/// the compiler's stack down with it, which it did not.
const DEEPEST: usize = 128;

/// How a block ended.
///
/// Four ways, and the evaluator needed all four the moment it gained a loop: a
/// block that **falls through** has no value and is not an error — it is a
/// loop's body between turns — where before this a block with no value was the
/// only thing `Unevaluable` could mean.
#[derive(Debug, Clone)]
enum Flow {
    /// A `return`, or a last statement that is a value.
    Value(Value),
    /// Ran to the end and produced nothing.
    Fell,
    Broke,
    Continued,
}

/// What a build-time expression came to.
///
/// Three kinds, which is what the declaration can carry: Rust's `const` needs a
/// type this compiler can spell
/// ([ADR-073](../../../docs/specification/adr/adr-073.md) D5) — an integer, a
/// `bool`, and now an **array of them**.
///
/// **The array is the aggregate `open-work.md` §2.8 was about**, and it is an
/// array rather than a `Vec` for the reason that entry gives from the other
/// side: a `Vec` allocates and a `const` cannot hold one, where `[T; N]` is
/// exactly what one holds ([ADR-152](../../../docs/specification/adr/adr-152.md)).
/// So a build-time table is written at its length and filled by index, which is
/// the shape the type system already had — measured: `.push` on a list hands
/// back a `Vec[?]`, and `NK1104` refuses it against an `Array[i64, 5]` before
/// this evaluator is ever reached.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    /// In an `i128`, so a sum that cannot fit an `i64` is a number the caller
    /// can name rather than one that wrapped — [`fold`](crate::fold)'s reason,
    /// one level up.
    Int(i128),
    Bool(bool),
    /// A fixed-length list, every element already a value.
    List(Vec<Value>),
    /// Text, **as the source wrote it** — escapes and all.
    ///
    /// The parser keeps a string's escapes rather than decoding them, and the
    /// emitter passes them through into the Rust literal unchanged, because
    /// this language's escapes are that one's. Holding the written form is
    /// therefore the shape that agrees with the lowering: what a `comptime`
    /// writes down is the same text the same literal would have produced at
    /// run time, character for character.
    ///
    /// **What it costs is every question about the value rather than the
    /// text.** `"\u{0041}"` and `"A"` are one value and two written forms, so
    /// `==` and `.len()` over text are refused here rather than answered
    /// wrongly — a decoder is what they want, and nothing has asked for one.
    Text(String),
}

/// Why a build-time expression did not come to a value.
#[derive(Debug, Clone)]
pub enum Refusal {
    /// This evaluator does not know the shape. **Not an error by itself**: it is
    /// what `NK1127` reports, and what the staging above expects to keep
    /// reporting until every stage lands.
    Unevaluable,
    /// The callee may not be run while the program is built
    /// ([ADR-075](../../../docs/specification/adr/adr-075.md) D1, D2). A
    /// different claim from the one above and it gets a different code: the
    /// shape is understood and the rule says no.
    NotAllowed {
        callee: String,
        because: &'static str,
    },
    /// The call depth above.
    TooDeep { callee: String },
    /// An index this array does not have. **Understood and wrong**, like
    /// `NotAllowed` and unlike `Unevaluable`: the program says `xs[7]` of five
    /// elements, and a build that answered *cannot evaluate* would send the
    /// reader looking for a missing feature instead of at the line
    /// ([Part III C.2](../../../docs/specification/30-nikaia-tooling.md)).
    /// Running it would abort at run time ([ADR-048](../../../docs/specification/adr/adr-048.md)
    /// D1); at build time there is no run to abort.
    OutOfBounds { at: i128, len: usize },
}

/// What a name outside a build-time body is worth: a `comptime` already
/// evaluated, or a `let` whose value folded.
pub type Known<'a> = &'a dyn Fn(&str) -> Option<Value>;

/// The evaluator, over one unit's items.
pub struct BuildTime<'a> {
    parsed: &'a Parsed,
    own: &'a Ledger,
    known: Known<'a>,
    depth: usize,
}

impl<'a> BuildTime<'a> {
    pub fn new(parsed: &'a Parsed, own: &'a Ledger, known: Known<'a>) -> Self {
        Self {
            parsed,
            own,
            known,
            depth: 0,
        }
    }

    /// What an initialiser comes to.
    pub fn evaluate(&mut self, expr: &Expr) -> Result<Value, Refusal> {
        self.expr(expr, &BTreeMap::new())
    }

    fn expr(&mut self, expr: &Expr, frame: &BTreeMap<String, Value>) -> Result<Value, Refusal> {
        match expr {
            Expr::LitInt(value) => Ok(Value::Int(*value as i128)),
            Expr::LitBool(value) => Ok(Value::Bool(*value)),
            Expr::LitStr(text) => Ok(Value::Text(text.clone())),
            // **`f"…"` is text with code in it** (ADR-035), and the code is
            // Nikaia, so this evaluator can read it — which is what makes text
            // at build time worth having at all. A literal alone would be a
            // value somebody could have written down.
            Expr::LitInterpolated(literal) => self.interpolated(literal, frame),
            Expr::Variable(name) => {
                let name = self.parsed.text(*name);
                frame
                    .get(name)
                    .cloned()
                    .or_else(|| (self.known)(name))
                    .ok_or(Refusal::Unevaluable)
            }
            Expr::Unary { op, expr } => {
                let inner = self.expr(expr, frame)?;
                match (op, inner) {
                    (UnaryOp::Neg, Value::Int(v)) => {
                        Ok(Value::Int(v.checked_neg().ok_or(Refusal::Unevaluable)?))
                    }
                    (UnaryOp::Not, Value::Bool(v)) => Ok(Value::Bool(!v)),
                    _ => Err(Refusal::Unevaluable),
                }
            }
            Expr::Binary { op, lhs, rhs, .. } => self.binary(*op, lhs, rhs, frame),
            // **An `if` is an expression always** (Part I 3.1), so it is one
            // here too — and it is what makes a body worth calling at all.
            Expr::If {
                cond,
                then_branch,
                else_branch,
            } => {
                let taken = match self.expr(cond, frame)? {
                    Value::Bool(true) => Some(then_branch),
                    Value::Bool(false) => else_branch.as_ref(),
                    _ => return Err(Refusal::Unevaluable),
                };
                // **The branch gets a frame of its own**, because an `if` in
                // value position is not a place a name is assigned from: what
                // the branch writes is the branch's, and what it hands back is
                // the value.
                let Some(block) = taken else {
                    return Err(Refusal::Unevaluable);
                };
                let mut inner = frame.clone();
                match self.block(block, &mut inner)? {
                    Flow::Value(value) => Ok(value),
                    Flow::Fell | Flow::Broke | Flow::Continued => Err(Refusal::Unevaluable),
                }
            }
            // **A list literal is the aggregate's only constructor here**
            // ([ADR-135](../../../docs/specification/adr/adr-135.md)). There is
            // no `push`: what `.push` hands back is a `Vec[?]`, and `NK1104`
            // refuses one against the `Array[T, N]` a `const` can hold long
            // before this evaluator sees it. So a table is written at its
            // length and filled by index, below.
            Expr::ListLit { items, .. } => {
                let mut values = Vec::with_capacity(items.len());
                for item in items {
                    values.push(self.expr(item, frame)?);
                }
                Ok(Value::List(values))
            }
            Expr::Index { base, index } => {
                let on = self.expr(base, frame)?;
                let at = self.expr(index, frame)?;
                element(&on, &at).cloned()
            }
            // `xs.len()`, which is what a loop over a table is written with —
            // `for i in 0..<xs.len()`. One method and no others: the length of
            // a list this evaluator already holds is a fact it has, where
            // anything else would be a body somewhere it cannot read.
            Expr::MethodCall {
                receiver,
                method,
                args,
                config,
            } if args.is_empty() && config.is_empty() && self.parsed.text(*method) == "len" => {
                match self.expr(receiver, frame)? {
                    Value::List(items) => Ok(Value::Int(items.len() as i128)),
                    _ => Err(Refusal::Unevaluable),
                }
            }
            Expr::Call { func, args, config } if config.is_empty() => {
                let Expr::Variable(name) = func.as_ref() else {
                    return Err(Refusal::Unevaluable);
                };
                let name = self.parsed.text(*name).to_string();
                let mut given = Vec::new();
                for arg in args {
                    given.push(self.expr(arg, frame)?);
                }
                self.call(&name, &given)
            }
            _ => Err(Refusal::Unevaluable),
        }
    }

    fn binary(
        &mut self,
        op: BinaryOp,
        lhs: &Expr,
        rhs: &Expr,
        frame: &BTreeMap<String, Value>,
    ) -> Result<Value, Refusal> {
        // **`&&` and `||` short-circuit**, which is not an optimisation here: a
        // body may guard a call with one, and evaluating the far side of a
        // guard is how a build-time evaluator reaches what the guard was
        // keeping it away from.
        if matches!(op, BinaryOp::And | BinaryOp::Or) {
            let Value::Bool(left) = self.expr(lhs, frame)? else {
                return Err(Refusal::Unevaluable);
            };
            return match (op, left) {
                (BinaryOp::And, false) => Ok(Value::Bool(false)),
                (BinaryOp::Or, true) => Ok(Value::Bool(true)),
                _ => match self.expr(rhs, frame)? {
                    Value::Bool(right) => Ok(Value::Bool(right)),
                    _ => Err(Refusal::Unevaluable),
                },
            };
        }
        let left = self.expr(lhs, frame)?;
        let right = self.expr(rhs, frame)?;
        self.operate(op, left, right)
    }

    /// An operator on two values already in hand.
    ///
    /// **Split out of [`binary`](Self::binary) for `+=`**, whose left side is a
    /// name that is already bound: re-evaluating it as an expression would read
    /// it a second time, which is the same answer here and would stop being one
    /// the moment a left side can call anything.
    fn operate(&self, op: BinaryOp, left: Value, right: Value) -> Result<Value, Refusal> {
        match (left, right) {
            (Value::Int(a), Value::Int(b)) => match op {
                BinaryOp::Add => a.checked_add(b).map(Value::Int),
                BinaryOp::Sub => a.checked_sub(b).map(Value::Int),
                BinaryOp::Mul => a.checked_mul(b).map(Value::Int),
                BinaryOp::Div => a.checked_div(b).map(Value::Int),
                BinaryOp::Rem => a.checked_rem(b).map(Value::Int),
                BinaryOp::Eq => Some(Value::Bool(a == b)),
                BinaryOp::Ne => Some(Value::Bool(a != b)),
                BinaryOp::Lt => Some(Value::Bool(a < b)),
                BinaryOp::Le => Some(Value::Bool(a <= b)),
                BinaryOp::Gt => Some(Value::Bool(a > b)),
                BinaryOp::Ge => Some(Value::Bool(a >= b)),
                BinaryOp::And | BinaryOp::Or => None,
            }
            .ok_or(Refusal::Unevaluable),
            // **Part I 4.7's `+` over text**, and only that one. A comparison
            // would be a question about the *value* where this holds the
            // written form — see [`Value::Text`].
            (Value::Text(a), Value::Text(b)) => match op {
                BinaryOp::Add => Ok(Value::Text(format!("{a}{b}"))),
                _ => Err(Refusal::Unevaluable),
            },
            (Value::Bool(a), Value::Bool(b)) => match op {
                BinaryOp::Eq => Ok(Value::Bool(a == b)),
                BinaryOp::Ne => Ok(Value::Bool(a != b)),
                BinaryOp::And => Ok(Value::Bool(a && b)),
                BinaryOp::Or => Ok(Value::Bool(a || b)),
                _ => Err(Refusal::Unevaluable),
            },
            _ => Err(Refusal::Unevaluable),
        }
    }

    /// **`f"…"` while the program is built** (ADR-035, [ADR-032](../../../docs/specification/adr/adr-032.md)
    /// D3 — a hole is code and every analysis sees it, this one included).
    ///
    /// The holes are split out by the same function the lowering uses, so the
    /// two cannot disagree about where one begins. What is rebuilt is the
    /// **written** form: a number contributes its digits, a `bool` its word,
    /// and text its own written form, which is why nothing has to be escaped
    /// on the way in or out.
    ///
    /// **A format spec is not read.** `f"{n:>8}"` asks for a width, and what
    /// that means is `std::fmt`'s rather than this language's — so it is a
    /// shape this evaluator does not know, not a rule it breaks.
    fn interpolated(
        &mut self,
        literal: &str,
        frame: &BTreeMap<String, Value>,
    ) -> Result<Value, Refusal> {
        let (format, holes) = crate::emit::interpolation(literal).map_err(|_| {
            // A malformed literal is the lowering's refusal and it names the
            // line; saying anything else here would be a second sentence about
            // one mistake.
            Refusal::Unevaluable
        })?;
        let mut values = Vec::with_capacity(holes.len());
        for hole in &holes {
            let expr = crate::parser::parse_expression(&self.parsed.interner, hole)
                .map_err(|_| Refusal::Unevaluable)?;
            values.push(self.expr(&expr, frame)?);
        }

        let mut out = String::with_capacity(format.len());
        let mut taken = values.into_iter();
        let mut chars = format.chars().peekable();
        while let Some(c) = chars.next() {
            match c {
                // `{{` and `}}` are how a format string spells one brace, and
                // a written literal spells it with one.
                '{' if chars.peek() == Some(&'{') => {
                    chars.next();
                    out.push('{');
                }
                '}' if chars.peek() == Some(&'}') => {
                    chars.next();
                    out.push('}');
                }
                '{' => {
                    // `{}` and nothing else: anything between the braces is a
                    // spec, which this does not read.
                    if chars.next() != Some('}') {
                        return Err(Refusal::Unevaluable);
                    }
                    match taken.next() {
                        Some(Value::Int(n)) => out.push_str(&n.to_string()),
                        Some(Value::Bool(yes)) => out.push_str(&yes.to_string()),
                        Some(Value::Text(text)) => out.push_str(&text),
                        _ => return Err(Refusal::Unevaluable),
                    }
                }
                c => out.push(c),
            }
        }
        Ok(Value::Text(out))
    }

    /// A call to a function this unit declares.
    ///
    /// **The permission is read off the ledger**
    /// ([ADR-075](../../../docs/specification/adr/adr-075.md) D1, D2) and not
    /// off a list kept here: `sync` says the body never pauses, and a touch set
    /// that is empty or exactly the build's own parameters says it reaches
    /// nothing else. Both are derived for every function already.
    fn call(&mut self, name: &str, given: &[Value]) -> Result<Value, Refusal> {
        if self.depth >= DEEPEST {
            return Err(Refusal::TooDeep {
                callee: name.to_string(),
            });
        }
        let Some(contract) = self.own.functions.get(name) else {
            // A callee this unit does not declare — `std`, a package — is not
            // refused, it is unevaluable: there is no body here to run, and
            // saying *you may not* about a function whose body is somewhere
            // else would be a claim this cannot make.
            return Err(Refusal::Unevaluable);
        };
        if !contract.sync.is_sync() {
            return Err(Refusal::NotAllowed {
                callee: name.to_string(),
                because: "it can pause, and a build-time body may not (ADR-075 D1)",
            });
        }
        if !contract.touches_known {
            return Err(Refusal::NotAllowed {
                callee: name.to_string(),
                because: "nothing says what it touches, and a build-time body's touch set \
                          has to be an answer (ADR-075 D2)",
            });
        }
        if !contract.touches.iter().all(is_the_builds_own) {
            return Err(Refusal::NotAllowed {
                callee: name.to_string(),
                because: "it reaches the world, and a build-time body touches nothing but \
                          the build's own parameters (ADR-075 D2)",
            });
        }
        let Some((args, body)) = self.body_of(name) else {
            return Err(Refusal::Unevaluable);
        };
        if args.len() != given.len() {
            return Err(Refusal::Unevaluable);
        }
        let mut frame: BTreeMap<String, Value> = args
            .iter()
            .map(|name| name.to_string())
            .zip(given.iter().cloned())
            .collect();
        self.depth += 1;
        let out = self.block(&body, &mut frame);
        self.depth -= 1;
        // A body that fell off its end has no value, and a `break` or a
        // `continue` outside a loop is not a body this evaluator reads — the
        // checker refuses both long before here, and neither is a value.
        match out? {
            Flow::Value(value) => Ok(value),
            Flow::Fell | Flow::Broke | Flow::Continued => Err(Refusal::Unevaluable),
        }
    }

    /// The parameters and the body of a function this unit declares.
    ///
    /// A **receiver** stops it: a method needs a value to be called on, and a
    /// build-time call by name has none.
    fn body_of(&self, name: &str) -> Option<(Vec<String>, Block)> {
        self.parsed.program.items.iter().find_map(|item| {
            let Item::Fn {
                name: declared,
                args,
                receiver,
                config,
                body,
                ..
            } = &item.node
            else {
                return None;
            };
            if receiver.is_some() || !config.is_empty() {
                return None;
            }
            if self.parsed.text((*declared)?) != name {
                return None;
            }
            Some((
                args.iter()
                    .map(|arg| self.parsed.text(arg.name).to_string())
                    .collect(),
                body.clone(),
            ))
        })
    }

    /// A body: `let`s, an early `return`, a loop, and a last statement that is
    /// the value — which is Part I 3.1's rule for every block, read here.
    ///
    /// **The frame is by reference now, and a loop is why.** A body used to get
    /// a fresh map it owned; a `for` has to see what its body assigned on the
    /// last turn, and a `while` has to see what its condition reads.
    fn block(
        &mut self,
        block: &Block,
        frame: &mut BTreeMap<String, Value>,
    ) -> Result<Flow, Refusal> {
        let last = block.stmts.len().saturating_sub(1);
        for (at, stmt) in block.stmts.iter().enumerate() {
            match &stmt.node {
                Stmt::Let { names, value, .. } => {
                    let [name] = names.as_slice() else {
                        return Err(Refusal::Unevaluable);
                    };
                    let value = self.expr(value, frame)?;
                    frame.insert(self.parsed.text(*name).to_string(), value);
                }
                // A `comptime` inside a body is a `let` that must fold
                // ([ADR-073](../../../docs/specification/adr/adr-073.md) D2),
                // and inside a build-time body everything must, so the two are
                // the same statement here.
                Stmt::Comptime { name, value, .. } => {
                    let value = self.expr(value, frame)?;
                    frame.insert(self.parsed.text(*name).to_string(), value);
                }
                // **`xs[i] = …`, which is how a build-time table is filled.**
                // Before the name, because an index is a target this evaluator
                // reads and `Expr::Variable` is not what it looks like.
                Stmt::Assign {
                    target: Expr::Index { base, index },
                    op,
                    value,
                } => {
                    let Expr::Variable(name) = base.as_ref() else {
                        return Err(Refusal::Unevaluable);
                    };
                    let name = self.parsed.text(*name).to_string();
                    let at = self.expr(index, frame)?;
                    let given = self.expr(value, frame)?;
                    let held = frame.get(&name).ok_or(Refusal::Unevaluable)?;
                    let next = match op {
                        None => given,
                        Some(op) => {
                            let before = element(held, &at)?.clone();
                            self.operate(*op, before, given)?
                        }
                    };
                    let Value::List(items) = frame.get_mut(&name).ok_or(Refusal::Unevaluable)?
                    else {
                        return Err(Refusal::Unevaluable);
                    };
                    let Value::Int(at) = at else {
                        return Err(Refusal::Unevaluable);
                    };
                    let len = items.len();
                    let slot = usize::try_from(at)
                        .ok()
                        .and_then(|at| items.get_mut(at))
                        .ok_or(Refusal::OutOfBounds { at, len })?;
                    *slot = next;
                }
                Stmt::Assign { target, op, value } => {
                    let Expr::Variable(name) = target else {
                        return Err(Refusal::Unevaluable);
                    };
                    let name = self.parsed.text(*name).to_string();
                    let given = self.expr(value, frame)?;
                    let next = match op {
                        None => given,
                        Some(op) => {
                            let held = frame.get(&name).cloned().ok_or(Refusal::Unevaluable)?;
                            self.operate(*op, held, given)?
                        }
                    };
                    if !frame.contains_key(&name) {
                        return Err(Refusal::Unevaluable);
                    }
                    frame.insert(name, next);
                }
                Stmt::Return(Some(value)) => return Ok(Flow::Value(self.expr(value, frame)?)),
                Stmt::Break => return Ok(Flow::Broke),
                Stmt::Continue => return Ok(Flow::Continued),
                Stmt::For {
                    bindings,
                    iter,
                    body,
                } => {
                    let flow = self.walk(bindings, iter, body, frame)?;
                    if let Flow::Value(_) = flow {
                        return Ok(flow);
                    }
                }
                Stmt::While { cond, body } => {
                    // **No step budget**
                    // ([ADR-075](../../../docs/specification/adr/adr-075.md)
                    // D4), deliberately and with the cost written down: a
                    // `while` that does not end hangs the build. The call-depth
                    // limit above is not this and does not become it.
                    loop {
                        match self.expr(cond, frame)? {
                            Value::Bool(true) => {}
                            Value::Bool(false) => break,
                            _ => return Err(Refusal::Unevaluable),
                        }
                        match self.block(body, frame)? {
                            Flow::Value(value) => return Ok(Flow::Value(value)),
                            Flow::Broke => break,
                            Flow::Fell | Flow::Continued => {}
                        }
                    }
                }
                // **An `if` is read as a statement here whatever its
                // position**, which is what a loop's body needs: `if i > n
                // { break }` is the last statement of its block and hands back
                // no value at all. A branch that falls through carries on; one
                // that returns, breaks or continues ends the block.
                Stmt::Expr(Expr::If {
                    cond,
                    then_branch,
                    else_branch,
                }) => {
                    let taken = match self.expr(cond, frame)? {
                        Value::Bool(true) => Some(then_branch),
                        Value::Bool(false) => else_branch.as_ref(),
                        _ => return Err(Refusal::Unevaluable),
                    };
                    if let Some(block) = taken {
                        match self.block(block, frame)? {
                            Flow::Fell => {}
                            other => return Ok(other),
                        }
                    }
                }
                // **`xs.push(v)`, which is how a table is *grown* rather than
                // filled** — [ADR-079](../../../docs/specification/adr/adr-079.md)'s
                // own title, *growable going in, fixed coming out*. The body
                // works with a list that does not know its length yet; what
                // crosses into the program is fixed, and the declaration is
                // where it becomes so.
                //
                // A statement and not an expression, because that is what it
                // is: `Vec::push` hands back nothing, and a body that reads
                // its result is not one this evaluator sees.
                Stmt::Expr(Expr::MethodCall {
                    receiver,
                    method,
                    args,
                    config,
                }) if self.parsed.text(*method) == "push"
                    && args.len() == 1
                    && config.is_empty() =>
                {
                    let Expr::Variable(name) = receiver.as_ref() else {
                        return Err(Refusal::Unevaluable);
                    };
                    let name = self.parsed.text(*name).to_string();
                    let given = self.expr(&args[0], frame)?;
                    let Some(Value::List(items)) = frame.get_mut(&name) else {
                        return Err(Refusal::Unevaluable);
                    };
                    items.push(given);
                }
                Stmt::Expr(expr) if at == last => return Ok(Flow::Value(self.expr(expr, frame)?)),
                _ => return Err(Refusal::Unevaluable),
            }
        }
        Ok(Flow::Fell)
    }

    /// A `for` over a range, which is the one shape a build-time loop has: a
    /// list needs a value this evaluator does not carry yet, and that is
    /// `docs/open-work.md` §2.9's next step rather than this one's.
    fn walk(
        &mut self,
        bindings: &[winnow_grammar::Symbol],
        iter: &Expr,
        body: &Block,
        frame: &mut BTreeMap<String, Value>,
    ) -> Result<Flow, Refusal> {
        let [binding] = bindings else {
            return Err(Refusal::Unevaluable);
        };
        let Expr::Range {
            start,
            end,
            inclusive,
        } = iter
        else {
            return Err(Refusal::Unevaluable);
        };
        let (Value::Int(start), Value::Int(end)) =
            (self.expr(start, frame)?, self.expr(end, frame)?)
        else {
            return Err(Refusal::Unevaluable);
        };
        let name = self.parsed.text(*binding).to_string();
        let mut at = start;
        while if *inclusive { at <= end } else { at < end } {
            frame.insert(name.clone(), Value::Int(at));
            match self.block(body, frame)? {
                Flow::Value(value) => return Ok(Flow::Value(value)),
                Flow::Broke => break,
                Flow::Fell | Flow::Continued => {}
            }
            at = at.checked_add(1).ok_or(Refusal::Unevaluable)?;
        }
        // **The binding does not outlive the loop**, which is Part I 3.3's rule
        // and matters here because the frame is shared: a name the loop bound
        // must not be readable after it.
        frame.remove(&name);
        Ok(Flow::Fell)
    }
}

/// One element of a list value, by an index that is a value.
///
/// **An index this array does not have is a refusal and not an
/// `Unevaluable`** — see [`Refusal::OutOfBounds`]. A receiver that is not a
/// list, or an index that is not a number, is a shape this evaluator does not
/// read, which is the other thing entirely.
fn element<'v>(on: &'v Value, at: &Value) -> Result<&'v Value, Refusal> {
    let (Value::List(items), Value::Int(at)) = (on, at) else {
        return Err(Refusal::Unevaluable);
    };
    usize::try_from(*at)
        .ok()
        .and_then(|index| items.get(index))
        .ok_or(Refusal::OutOfBounds {
            at: *at,
            len: items.len(),
        })
}

/// Whether a touch is the build's own parameters
/// ([ADR-075](../../../docs/specification/adr/adr-075.md) D2).
///
/// **`cli::args` passes, deliberately**: same parameters, same code. A build's
/// own arguments are an input like its source is, and a body that shapes a
/// table differently for two declared parameters is doing the thing `comptime`
/// is for.
fn is_the_builds_own(touch: &touch::Touch) -> bool {
    touch.text() == "args read"
}
