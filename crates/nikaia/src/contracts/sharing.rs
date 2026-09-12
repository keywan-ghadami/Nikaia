// crates/nikaia/src/contracts/sharing.rs
//
// **A prototype, and nothing downstream reads it.** The question it answers is
// the one [ADR-037](../../../../docs/specification/adr/adr-037.md) D3 leaves
// open in as many words - "whether the *choice between `Rc` and `Arc`* could be
// made per value rather than per build is a real question and is not answered
// here" - and the decision is the repository owner's. This file exists so the
// question has a number and three worked cases in front of it instead of an
// intuition; `docs/rc-or-arc.md` is the experiment it belongs to, and says what
// is real here and what was done by hand.
//
// It emits nothing. `Shared` is unbuilt - there is no `Rc::new` or `Arc::new`
// anywhere in the emitter - so there is nothing for an answer here to change.
// What it does is *report* what it would choose, which `--sharing` prints.
//
// ## The question, and whose it is
//
// `contracts::send` asks of a **type**: may a value of it be on a thread other
// than the one that built it. This asks of a **value**:
//
//     must this `Shared` handle's count be atomic?
//
// ## `Plain ⊑ Atomic`, and the safe direction is up
//
// The house shape - `Borrowed ⊑ Tethered ⊑ Owned` (ADR-008), `Trusted ⊑
// Untrusted` (ADR-010) - with the same polarity as the second of those, and for
// a harder reason. An inferred `Arc` where a plain count would have done costs
// an atomic instruction; an inferred plain count on a value that crosses a
// thread is a **data race**. So every case this cannot decide comes out
// [`Count::Atomic`], and every such case carries the reason that says so, because
// "an analysis that fails open is a vulnerability generator" (ADR-010 D1) and
// here failing open is not a wrong refusal - it is undefined behaviour.
//
// **Fail-closed is affordable here in places `NK25xx` could not afford it**, and
// that is the one way this analysis has an easier job than the check it sits
// next to. `send.rs` may not refuse what it cannot decide, because refusing a
// correct program is the one thing this compiler may never do (Part III C.4) -
// so it has a third answer, `Undecided`. This has two, because the cost of
// being wrong in the safe direction is **speed**: the program still compiles and
// still means the same thing. ADR-033 D4's polarity, applied to a question where
// it is cheap.
//
// ## Why the fixpoint degenerates, which is itself a finding
//
// A count belongs to the **allocation**, not to the handle: two handles on one
// `Shared` share one count, so they cannot disagree about whether it is atomic.
// The relation between handles is therefore an *equivalence* and not an ordering,
// and the analysis is union-find over handles plus one colouring pass - a least
// fixpoint reached in a single step, where `sync` (ADR-027 D1) genuinely needs a
// greatest one because its constraint is conjunctive over a call graph that can
// cycle. So the answer to "least or greatest" is that on this lattice the two
// coincide: the least fixpoint of "is atomic" is the complement of the greatest
// fixpoint of "stays plain", because every clause is a Horn clause and the only
// sources of `Atomic` are the seeds below. What carries the weight is not the
// lattice - it is **which crossings are seeds**, and that list is
// `contracts::send` §5.2's list, asked per value.
//
// ## The seeds: where a count is forced to be atomic
//
// The crossings are the ones ADR-005 §5.2 enumerates, plus the two places this
// compiler cannot see past:
//
//   * a name a `spawn` body mentions - Part II 11.2's task runs on a thread of
//     its own (`NK2501`'s crossing);
//   * a value handed to a call nothing written down describes - ADR-038 D7's
//     foreign runtime, which may put it on a thread of its own (`NK2502`'s
//     crossing). Refused there; here it merely costs an atomic;
//   * a name mentioned inside a lambda handed to a parallel method
//     (`par_iter`, `par_fold`);
//   * a **parameter or result of a public function**, because its callers are in
//     a unit this build cannot see. This is the library boundary, and the
//     expensive one - see `docs/rc-or-arc.md` §5.1;
//   * a field of a struct of which some value reaches one of the above.
//
// `task::both` - the crossing the *compiler* chooses for statement overlapping -
// is deliberately **not** a seed. ADR-033 D4's polarity already applies there:
// the compiler was never obliged to overlap, so the answer is that it does not,
// and the statements keep their written order rather than the value paying for
// an atomic it did not ask for.

use std::collections::{BTreeMap, BTreeSet};

use crate::ast::{Block, Expr, Item, Stmt};
use crate::parser::Parsed;

use super::{send, ty::Ty, Ledger};

/// The type whose count this is about.
const SHARED: &str = "Shared";

/// Methods that run their lambda on a thread the program asked for.
///
/// A closed list for the same reason `send::PLAIN` is one: a name this file does
/// not know contributes nothing, and the walk that finds the names inside the
/// lambda is over-approximate, which is the safe direction.
const PARALLEL: &[&str] = &["par_iter", "par_fold", "par_map"];

/// Which kind of count a `Shared` value needs.
///
/// `Plain ⊑ Atomic`, joined upwards: one crossing anywhere in a handle's
/// equivalence class makes the whole class atomic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Count {
    /// `Rc`: a count only the thread that built it ever touches.
    Plain,
    /// `Arc`: a count any thread may touch. The answer wherever this analysis
    /// cannot prove the other one.
    Atomic,
}

impl Count {
    /// The more cautious of two.
    pub fn join(self, other: Self) -> Self {
        self.max(other)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Count::Plain => "plain",
            Count::Atomic => "atomic",
        }
    }
}

/// What this analysis would choose for one `Shared` value, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decision {
    /// The function the value is written in, as a ledger key
    /// (`Counter::record`).
    pub function: String,
    /// The name the source gives it, or `<result>` for what a function hands
    /// back.
    pub value: String,
    /// Its type, as written.
    pub ty: String,
    pub count: Count,
    /// The crossing that forced an atomic count, in the words a user would be
    /// told. `None` for a plain one, which needs no excuse.
    pub why: Option<String>,
    /// Whether `why` is a crossing the analysis *found* or an admission that it
    /// could not see. The second list is the one ADR-010 D1 asks to be named.
    pub undecided: bool,
}

/// Every `Shared` value in a program, and which count it would get.
///
/// Sorted by function and then by name, because a report is read and compared.
pub fn analyse(parsed: &Parsed, own: &Ledger, library: &Ledger) -> Vec<Decision> {
    let mut analysis = Analysis::new(parsed, own, library);
    for item in &parsed.program.items {
        match &item.node {
            Item::Fn { .. } => analysis.function(&item.node, None),
            Item::Impl {
                target, methods, ..
            } => {
                let target = parsed.text(target.name).to_string();
                for method in methods {
                    analysis.function(&method.node, Some(&target));
                }
            }
            _ => {}
        }
    }
    analysis.decide()
}

/// What `--sharing` prints: one line per `Shared` value, and the reason beside
/// every atomic one.
pub fn report(parsed: &Parsed, own: &Ledger, library: &Ledger) -> String {
    let decisions = analyse(parsed, own, library);
    if decisions.is_empty() {
        return "no `Shared` value in this program, so there is nothing to choose.\n".to_string();
    }

    let mut out = String::new();
    for decision in &decisions {
        out.push_str(&format!(
            "{}: `{}`: {} ({})\n",
            decision.function,
            decision.value,
            decision.count.as_str(),
            decision.ty,
        ));
        if let Some(why) = &decision.why {
            out.push_str(&format!(
                "  {} {why}\n",
                match decision.undecided {
                    true => "could not decide:",
                    false => "crosses:",
                }
            ));
        }
    }
    let atomic = decisions
        .iter()
        .filter(|d| d.count == Count::Atomic)
        .count();
    let undecided = decisions.iter().filter(|d| d.undecided).count();
    out.push_str(&format!(
        "\n{} `Shared` value(s): {} plain, {} atomic, of which {} because this analysis \
         could not decide.\n",
        decisions.len(),
        decisions.len() - atomic,
        atomic,
        undecided,
    ));
    out
}

/// One handle, named by the function it is written in.
fn slot(function: &str, name: &str) -> String {
    format!("{function}::{name}")
}

/// Handles joined into allocation classes, with a reason recorded against the
/// ones that cross.
struct Analysis<'a> {
    parsed: &'a Parsed,
    own: &'a Ledger,
    library: &'a Ledger,
    /// Every `Shared` handle: its slot key, and what was written about it.
    handles: BTreeMap<String, Handle>,
    /// Union-find over slot keys, by index into `parent`.
    index: BTreeMap<String, usize>,
    parent: Vec<usize>,
    /// What forced a slot's class to be atomic.
    forced: Vec<(String, String, bool)>,
}

struct Handle {
    function: String,
    value: String,
    ty: Ty,
    /// A slot that stands for a struct's field or a function's result rather
    /// than for something a person wrote. Not reported, but classes join
    /// through it.
    internal: bool,
}

impl<'a> Analysis<'a> {
    fn new(parsed: &'a Parsed, own: &'a Ledger, library: &'a Ledger) -> Self {
        Analysis {
            parsed,
            own,
            library,
            handles: BTreeMap::new(),
            index: BTreeMap::new(),
            parent: Vec::new(),
            forced: Vec::new(),
        }
    }

    // --- union-find ------------------------------------------------------

    fn id(&mut self, key: &str) -> usize {
        if let Some(found) = self.index.get(key) {
            return *found;
        }
        let fresh = self.parent.len();
        self.parent.push(fresh);
        self.index.insert(key.to_string(), fresh);
        fresh
    }

    fn root(&mut self, mut at: usize) -> usize {
        while self.parent[at] != at {
            let up = self.parent[at];
            self.parent[at] = self.parent[up];
            at = self.parent[at];
        }
        at
    }

    /// Two handles on one allocation, which therefore share one count.
    fn join(&mut self, left: &str, right: &str) {
        let (left, right) = (self.id(left), self.id(right));
        let (left, right) = (self.root(left), self.root(right));
        if left != right {
            self.parent[left] = right;
        }
    }

    // --- collecting ------------------------------------------------------

    fn note(&mut self, function: &str, value: &str, ty: Ty, internal: bool) {
        let key = slot(function, value);
        self.id(&key);
        self.handles.entry(key).or_insert(Handle {
            function: function.to_string(),
            value: value.to_string(),
            ty,
            internal,
        });
    }

    /// This slot's class must be atomic, and this is what forced it.
    ///
    /// **The slot is created if it does not exist yet, and that is not a
    /// detail.** A `<struct>.<field>` slot is forced by the function that
    /// crosses with the struct and joined by the function that builds one, and
    /// nothing says which of the two the walk reaches first. Guarding this on
    /// "the slot is already known" made the answer depend on the order the
    /// functions are written in - which is a fail-*open* bug, the one direction
    /// this analysis may never fail in. Union-find does not care about the
    /// order; only a guard here could.
    fn force(&mut self, function: &str, value: &str, why: String, undecided: bool) {
        let key = slot(function, value);
        self.id(&key);
        self.forced.push((key, why, undecided));
    }

    fn function(&mut self, item: &Item, target: Option<&str>) {
        let Item::Fn {
            name,
            args,
            body,
            ret_type,
            is_public,
            ..
        } = item
        else {
            return;
        };
        let own_name = match name {
            Some(name) => self.parsed.text(*name).to_string(),
            None => "new".to_string(),
        };
        let key = match target {
            Some(target) => format!("{target}::{own_name}"),
            None => own_name,
        };

        // Parameters, and the result, are the slots a *caller* meets.
        let mut scope: BTreeMap<String, Ty> = BTreeMap::new();
        for arg in args {
            let ty = Ty::from_ast(self.parsed, &arg.ty);
            let name = self.parsed.text(arg.name).to_string();
            if holds_shared(&ty) {
                self.note(&key, &name, ty.clone(), false);
                if *is_public {
                    // The library boundary. A published function's callers are
                    // in a unit this build never sees, so nothing here can say
                    // that none of them crosses - and a published artifact has
                    // already chosen a representation by the time one does.
                    self.force(
                        &key,
                        &name,
                        format!(
                            "`{key}` is public, so its callers are in a unit this build cannot \
                             see, and one of them may cross a thread with it"
                        ),
                        true,
                    );
                }
            }
            scope.insert(name, ty);
        }
        if let Some(ret) = ret_type {
            let ty = Ty::from_ast(self.parsed, ret);
            if holds_shared(&ty) {
                self.note(&key, "<result>", ty, true);
                if *is_public {
                    self.force(
                        &key,
                        "<result>",
                        format!(
                            "`{key}` is public and hands a `Shared` back, so what the caller does \
                             with it is in a unit this build cannot see"
                        ),
                        true,
                    );
                }
            }
        }

        self.block(&key, body, &mut scope);
    }

    fn block(&mut self, function: &str, block: &Block, scope: &mut BTreeMap<String, Ty>) {
        for stmt in &block.stmts {
            self.stmt(function, &stmt.node, scope);
        }
    }

    fn stmt(&mut self, function: &str, stmt: &Stmt, scope: &mut BTreeMap<String, Ty>) {
        match stmt {
            Stmt::Let {
                name, ty, value, ..
            } => {
                let name = self.parsed.text(*name).to_string();
                let declared = ty.as_ref().map(|t| Ty::from_ast(self.parsed, t));
                // An untyped `let` that names a `Shared` is a second handle on
                // the same allocation, and takes its type from the first.
                let aliased = self.names_a_handle(function, value, scope);
                let ty = declared
                    .clone()
                    .or_else(|| aliased.as_ref().map(|(_, ty)| ty.clone()))
                    // `let k = Counter { … }` is a value of `Counter`, and a
                    // crossing of it reaches the `Shared` its field holds - the
                    // second of the three cases `docs/rc-or-arc.md` §5 names.
                    .or_else(|| match value {
                        Expr::StructLit { name, .. } => Some(Ty::named(self.parsed.text(*name))),
                        _ => None,
                    });
                if let Some(ty) = ty {
                    if holds_shared(&ty) {
                        self.note(function, &name, ty.clone(), false);
                        if let Some((other, _)) = &aliased {
                            self.join(&slot(function, &name), &slot(function, other));
                        }
                    }
                    scope.insert(name.clone(), ty);
                }
                self.expr(function, value, scope);
            }
            Stmt::Assign { target, value, .. } => {
                // `a = b` makes `a` a handle on `b`'s allocation.
                if let (Expr::Variable(target), Some((other, _))) =
                    (target, self.names_a_handle(function, value, scope).clone())
                {
                    let target = self.parsed.text(*target).to_string();
                    if scope.contains_key(&target) {
                        self.join(&slot(function, &target), &slot(function, &other));
                    }
                }
                self.expr(function, target, scope);
                self.expr(function, value, scope);
            }
            Stmt::For { iter, body, .. } => {
                self.expr(function, iter, scope);
                self.block(function, body, scope);
            }
            Stmt::While { cond, body } => {
                self.expr(function, cond, scope);
                self.block(function, body, scope);
            }
            Stmt::Return(Some(value)) => {
                if let Some((other, _)) = self.names_a_handle(function, value, scope) {
                    self.join(&slot(function, "<result>"), &slot(function, &other));
                }
                self.expr(function, value, scope);
            }
            Stmt::Return(None) => {}
            Stmt::Expr(value) => {
                // Part I 3.1: the last statement of a value-returning body is
                // the value, and joining it with the result slot costs nothing
                // where the function returns no `Shared`.
                if let Some((other, _)) = self.names_a_handle(function, value, scope) {
                    self.join(&slot(function, "<result>"), &slot(function, &other));
                }
                self.expr(function, value, scope);
            }
        }
    }

    /// A crossing reached this name: whatever count it owns, or owns through a
    /// field, must be atomic.
    ///
    /// Two ways a crossing lands. The name may *be* a `Shared`, and then its own
    /// class is forced. Or it may be a value of a type that **holds** one, and
    /// then the field's class is - which is `send::crossing`'s transitivity
    /// (ADR-029's walk through the ledger's `fields`) asked the other way round:
    /// there a field that may not cross makes the struct refuse, here a struct
    /// that crosses makes the field atomic.
    fn reached(
        &mut self,
        function: &str,
        name: &str,
        scope: &BTreeMap<String, Ty>,
        why: &str,
        undecided: bool,
    ) {
        let Some(ty) = scope.get(name).cloned() else {
            return;
        };
        if holds_shared(&ty) {
            self.force(function, name, why.to_string(), undecided);
            return;
        }
        for field in shared_fields(&ty.text(), self.own, self.library) {
            self.force("<field>", &field, why.to_string(), undecided);
        }
    }

    /// The handle an expression names, where it names one directly.
    fn names_a_handle(
        &self,
        _function: &str,
        expr: &Expr,
        scope: &BTreeMap<String, Ty>,
    ) -> Option<(String, Ty)> {
        match expr {
            Expr::Variable(name) => {
                let name = self.parsed.text(*name).to_string();
                let ty = scope.get(&name)?;
                holds_shared(ty).then(|| (name, ty.clone()))
            }
            Expr::Block(block) => match block.stmts.last().map(|s| &s.node) {
                Some(Stmt::Expr(tail)) => self.names_a_handle(_function, tail, scope),
                _ => None,
            },
            _ => None,
        }
    }

    fn expr(&mut self, function: &str, expr: &Expr, scope: &mut BTreeMap<String, Ty>) {
        match expr {
            // Part II 11.2: a task runs on a thread of its own.
            Expr::Spawn { body, .. } => {
                for name in send::names_used(self.parsed, body) {
                    self.reached(
                        function,
                        &name,
                        &scope.clone(),
                        "a `spawn` body uses it, and a task runs on a thread of its own \
                         (Part II 11.2)",
                        false,
                    );
                }
                self.expr(function, body, scope);
            }
            Expr::Call { func, args, config } => {
                let callee = self.path_of(func);
                self.arguments(function, callee.as_deref(), args, scope);
                for arg in config {
                    self.expr(function, &arg.value, scope);
                }
                self.expr(function, func, scope);
            }
            Expr::MethodCall {
                receiver,
                method,
                args,
                config,
            } => {
                let method = self.parsed.text(*method).to_string();
                if PARALLEL.contains(&method.as_str()) {
                    let why = format!(
                        "a lambda handed to `{method}` uses it, and that lambda runs on a \
                         thread the program asked for"
                    );
                    for arg in args {
                        for name in send::names_used(self.parsed, arg) {
                            self.reached(function, &name, &scope.clone(), &why, false);
                        }
                    }
                }
                // A method is resolved by name only (ADR-011 D2), so a method
                // nothing describes is a body this compiler cannot see the end
                // of - the same case as an unseen call, and answered the same
                // way.
                self.arguments(function, Some(&method), args, scope);
                for arg in config {
                    self.expr(function, &arg.value, scope);
                }
                self.expr(function, receiver, scope);
            }
            // A `Shared` put into a struct is the struct's field's allocation,
            // and the field is where a crossing of the *struct* lands.
            Expr::StructLit { name, fields } => {
                let struct_name = self.parsed.text(*name).to_string();
                for field in fields {
                    let field_name = self.parsed.text(field.name).to_string();
                    // `Counter { hits }` is the shorthand: the field takes the
                    // variable of its own name (Part I 4.1).
                    let value = field.value.clone().unwrap_or(Expr::Variable(field.name));
                    if let Some((handle, ty)) = self.names_a_handle(function, &value, scope) {
                        let field_slot = format!("{struct_name}.{field_name}");
                        self.note("<field>", &field_slot, ty, true);
                        self.join(&slot(function, &handle), &slot("<field>", &field_slot));
                    }
                    self.expr(function, &value, scope);
                }
            }
            Expr::Closure { body, .. } => self.block(function, body, scope),
            Expr::Block(block) | Expr::Seq(block) => self.block(function, block, scope),
            Expr::If {
                cond,
                then_branch,
                else_branch,
            } => {
                self.expr(function, cond, scope);
                self.block(function, then_branch, scope);
                if let Some(block) = else_branch {
                    self.block(function, block, scope);
                }
            }
            Expr::Match { value, arms } => {
                self.expr(function, value, scope);
                for arm in arms {
                    self.expr(function, &arm.body, scope);
                }
            }
            Expr::Binary { lhs, rhs, .. } => {
                self.expr(function, lhs, scope);
                self.expr(function, rhs, scope);
            }
            Expr::Unary { expr, .. }
            | Expr::Try(expr)
            | Expr::Throw(expr)
            | Expr::Cast { expr, .. } => self.expr(function, expr, scope),
            Expr::Field { base, .. } => self.expr(function, base, scope),
            Expr::Index { base, index } => {
                self.expr(function, base, scope);
                self.expr(function, index, scope);
            }
            Expr::Coalesce { value, fallback } => {
                self.expr(function, value, scope);
                self.expr(function, fallback, scope);
            }
            Expr::TryCatch { expr, handler } => {
                self.expr(function, expr, scope);
                self.block(function, handler, scope);
            }
            Expr::Tuple(parts) => {
                for part in parts {
                    self.expr(function, part, scope);
                }
            }
            Expr::Range { start, end, .. } => {
                self.expr(function, start, scope);
                self.expr(function, end, scope);
            }
            Expr::DslFrom { input, .. } => self.expr(function, input, scope),
            _ => {}
        }
    }

    /// What happens to a handle handed to a call.
    ///
    /// Two answers and they are the two halves of the finding. A callee the
    /// ledger describes has a parameter slot, and the handle joins it: one
    /// allocation, one count, whichever side of the call decides it. A callee
    /// nothing describes is ADR-038 D7's case: it may start a thread of its own,
    /// so the count is atomic, which is the cheap version of `NK2502`'s refusal.
    fn arguments(
        &mut self,
        function: &str,
        callee: Option<&str>,
        args: &[Expr],
        scope: &mut BTreeMap<String, Ty>,
    ) {
        let described = callee.and_then(|callee| self.parameters(callee));
        for (at, arg) in args.iter().enumerate() {
            let Some((handle, _)) = self.names_a_handle(function, arg, scope) else {
                // Not a `Shared` itself. It may still hold one, and a callee
                // nothing describes may put what it is given on a thread.
                if let (Expr::Variable(name), None) = (arg, &described) {
                    let name = self.parsed.text(*name).to_string();
                    let why = format!(
                        "nothing written down describes `{}`, so this compiler cannot see the \
                         end of it - and starting a thread of its own is among the things it \
                         may do (ADR-038 D7)",
                        callee.unwrap_or("the callee")
                    );
                    self.reached(function, &name, &scope.clone(), &why, true);
                }
                self.expr(function, arg, scope);
                continue;
            };
            match (&described, callee) {
                (Some((key, params)), _) => match params.get(at) {
                    Some(param) => {
                        let (key, param) = (key.clone(), param.clone());
                        self.join(&slot(function, &handle), &slot(&key, &param));
                    }
                    // More arguments than the contract has parameters: nothing
                    // written down says where this one goes.
                    None => self.force(
                        function,
                        &handle,
                        format!(
                            "`{}` takes it in a position no contract describes",
                            callee.unwrap_or("the callee")
                        ),
                        true,
                    ),
                },
                (None, Some(callee)) => self.force(
                    function,
                    &handle,
                    format!(
                        "nothing written down describes `{callee}`, so this compiler cannot see \
                         the end of it - and starting a thread of its own is among the things it \
                         may do (ADR-038 D7)"
                    ),
                    true,
                ),
                (None, None) => self.force(
                    function,
                    &handle,
                    "it is handed to something this compiler cannot name".to_string(),
                    true,
                ),
            }
        }
    }

    /// A callee's ledger key and its parameter names, where a ledger has them.
    ///
    /// The program's own ledger first, then `std`'s, with the suffix rule every
    /// other resolution in this compiler uses (ADR-011 D2).
    fn parameters(&self, callee: &str) -> Option<(String, Vec<String>)> {
        let suffix = format!("::{callee}");
        let (key, contract) = [self.own, self.library].into_iter().find_map(|ledger| {
            ledger.functions.get_key_value(callee).or_else(|| {
                ledger
                    .functions
                    .iter()
                    .find(|(key, _)| key.ends_with(&suffix))
            })
        })?;
        let signature = contract.signature.as_ref()?;
        Some((
            key.clone(),
            signature
                .arguments()
                .iter()
                .map(|(name, _)| name.clone())
                .collect(),
        ))
    }

    // --- colouring -------------------------------------------------------

    /// One pass over the seeds, then one answer per handle.
    fn decide(mut self) -> Vec<Decision> {
        let mut atomic: BTreeMap<usize, (String, bool)> = BTreeMap::new();
        for (key, why, undecided) in std::mem::take(&mut self.forced) {
            let id = self.id(&key);
            let root = self.root(id);
            let entry = atomic.entry(root).or_insert((why.clone(), undecided));
            // Of two reasons, print the one that is a crossing rather than an
            // admission of ignorance: ADR-033 D9's rule, that of two true
            // answers the useful one is the one somebody can act on.
            if entry.1 && !undecided {
                *entry = (why, undecided);
            }
        }

        let keys: Vec<String> = self
            .handles
            .keys()
            .filter(|key| !self.handles[*key].internal)
            .cloned()
            .collect();
        let mut out = Vec::new();
        for key in keys {
            let id = self.id(&key);
            let root = self.root(id);
            let handle = &self.handles[&key];
            let (count, why, undecided) = match atomic.get(&root) {
                Some((why, undecided)) => (Count::Atomic, Some(why.clone()), *undecided),
                None => (Count::Plain, None, false),
            };
            out.push(Decision {
                function: handle.function.clone(),
                value: handle.value.clone(),
                ty: handle.ty.text(),
                count,
                why,
                undecided,
            });
        }
        out
    }
}

impl Analysis<'_> {
    /// The qualified name a call names, where it names one.
    fn path_of(&self, func: &Expr) -> Option<String> {
        match func {
            Expr::Variable(name) => Some(self.parsed.text(*name).to_string()),
            Expr::Path(segments) => Some(
                segments
                    .iter()
                    .map(|s| self.parsed.text(*s))
                    .collect::<Vec<_>>()
                    .join("::"),
            ),
            _ => None,
        }
    }
}

/// Whether a type is, or holds, a `Shared`.
fn holds_shared(ty: &Ty) -> bool {
    match ty {
        Ty::Named { name, args, .. } => name == SHARED || args.iter().any(holds_shared),
        Ty::Tuple(parts) => parts.iter().any(holds_shared),
        _ => false,
    }
}

/// The `<struct>.<field>` slots a type reaches that hold a `Shared`, through the
/// ledger's `fields` (ADR-024, the walk ADR-029 established).
fn shared_fields(ty: &str, own: &Ledger, library: &Ledger) -> Vec<String> {
    let name = ty.trim_start_matches('&');
    let suffix = format!("::{name}");
    let Some(contract) = [own, library].into_iter().find_map(|ledger| {
        ledger.types.get(name).or_else(|| {
            ledger
                .types
                .iter()
                .find(|(key, _)| key.ends_with(&suffix))
                .map(|(_, contract)| contract)
        })
    }) else {
        return Vec::new();
    };
    contract
        .fields
        .iter()
        .filter(|(_, ty)| holds_shared(ty))
        .map(|(field, _)| format!("{name}.{field}"))
        .collect()
}

/// Every name a program writes that mentions `Shared`, for a caller that wants
/// to know whether there is anything to decide at all.
pub fn values(parsed: &Parsed, own: &Ledger, library: &Ledger) -> BTreeSet<String> {
    analyse(parsed, own, library)
        .into_iter()
        .map(|d| format!("{}::{}", d.function, d.value))
        .collect()
}

/// Unused, but the shape the emitter would need: what `Shared[T]` lowers to.
///
/// Named here rather than in `emit` because **nothing emits it**. The emitter has
/// no `Rc::new` and no `Arc::new`; this says what the two words would be so that
/// a reader of `docs/rc-or-arc.md` §5.1 can see that the choice is between two Rust
/// types and not between two flags - which is the whole of §4.1's finding.
pub fn rust_name(count: Count) -> &'static str {
    match count {
        Count::Plain => "std::rc::Rc",
        Count::Atomic => "std::sync::Arc",
    }
}
