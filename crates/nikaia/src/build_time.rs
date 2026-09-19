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
//! D1 and D2 say what a build-time body may do — so this is the call.
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

/// What a build-time expression came to.
///
/// Two kinds, which is exactly what the declaration can carry today: Rust's
/// `const` needs a type this compiler can spell
/// ([ADR-073](../../../docs/specification/adr/adr-073.md) D5), and an integer
/// and a `bool` are what that list holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Value {
    /// In an `i128`, so a sum that cannot fit an `i64` is a number the caller
    /// can name rather than one that wrapped — [`fold`](crate::fold)'s reason,
    /// one level up.
    Int(i128),
    Bool(bool),
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
    NotAllowed { callee: String, because: &'static str },
    /// The call depth above.
    TooDeep { callee: String },
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
            Expr::Variable(name) => {
                let name = self.parsed.text(*name);
                frame
                    .get(name)
                    .copied()
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
            Expr::Binary { op, lhs, rhs } => self.binary(*op, lhs, rhs, frame),
            // **An `if` is an expression always** (Part I 3.1), so it is one
            // here too — and it is what makes a body worth calling at all.
            Expr::If {
                cond,
                then_branch,
                else_branch,
            } => match self.expr(cond, frame)? {
                Value::Bool(true) => self.block(then_branch, frame.clone()),
                Value::Bool(false) => match else_branch {
                    Some(block) => self.block(block, frame.clone()),
                    None => Err(Refusal::Unevaluable),
                },
                Value::Int(_) => Err(Refusal::Unevaluable),
            },
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
                    Value::Int(_) => Err(Refusal::Unevaluable),
                },
            };
        }
        let left = self.expr(lhs, frame)?;
        let right = self.expr(rhs, frame)?;
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
            (Value::Bool(a), Value::Bool(b)) => match op {
                BinaryOp::Eq => Ok(Value::Bool(a == b)),
                BinaryOp::Ne => Ok(Value::Bool(a != b)),
                _ => Err(Refusal::Unevaluable),
            },
            _ => Err(Refusal::Unevaluable),
        }
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
        let frame: BTreeMap<String, Value> = args
            .iter()
            .map(|name| name.to_string())
            .zip(given.iter().copied())
            .collect();
        self.depth += 1;
        let out = self.block(&body, frame);
        self.depth -= 1;
        out
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

    /// A body: `let`s, an early `return`, and a last statement that is the
    /// value — which is Part I 3.1's rule for every block, read here.
    fn block(&mut self, block: &Block, mut frame: BTreeMap<String, Value>) -> Result<Value, Refusal> {
        let last = block.stmts.len().saturating_sub(1);
        for (at, stmt) in block.stmts.iter().enumerate() {
            match &stmt.node {
                Stmt::Let { names, value, .. } => {
                    let [name] = names.as_slice() else {
                        return Err(Refusal::Unevaluable);
                    };
                    let value = self.expr(value, &frame)?;
                    frame.insert(self.parsed.text(*name).to_string(), value);
                }
                Stmt::Return(Some(value)) => return self.expr(value, &frame),
                Stmt::Expr(expr) if at == last => return self.expr(expr, &frame),
                // **An `if` that returns out of both arms is a body's shape**,
                // and it is not the last statement — `if n < 2 { return 1 }`
                // followed by the rest is how a base case is written.
                Stmt::Expr(Expr::If {
                    cond,
                    then_branch,
                    else_branch,
                }) => {
                    let taken = match self.expr(cond, &frame)? {
                        Value::Bool(true) => Some(then_branch),
                        Value::Bool(false) => else_branch.as_ref(),
                        Value::Int(_) => return Err(Refusal::Unevaluable),
                    };
                    if let Some(block) = taken {
                        if returns_out(block) {
                            return self.block(block, frame.clone());
                        }
                        return Err(Refusal::Unevaluable);
                    }
                }
                _ => return Err(Refusal::Unevaluable),
            }
        }
        Err(Refusal::Unevaluable)
    }
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

/// Whether every path out of this block is a `return`.
///
/// Read rather than assumed, because a branch that falls through has a value
/// this evaluator would have to carry past the `if` — which is a shape the
/// staging leaves for later rather than one to guess at.
fn returns_out(block: &Block) -> bool {
    matches!(
        block.stmts.last().map(|s| &s.node),
        Some(Stmt::Return(Some(_)))
    )
}
