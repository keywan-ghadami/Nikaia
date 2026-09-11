// crates/nikaia/src/check/mod.rs
//
// The type checker.
//
// It answers one question per construct - "are these two types the same?" - and
// it answers it only where both sides are written down. That is the whole
// design, and `contracts::ty::Ty::Unknown` is what makes it honest: Stage 0 has
// no signatures for the Rust half of `std`, and a checker that guessed at
// `push_str`, `entry` or `chars` would report errors that are not there. So
// every rule below has the same shape - infer both sides, and report only when
// **both are known and they disagree**.
//
// What it therefore promises, exactly:
//
//   * it never rejects a program that is correct;
//   * what it catches grows as the ledger grows, without this file changing.
//
// The second is the point of building it on the ledger (ADR-020) rather than
// beside it. A program's own functions are checked because `Ledger::infer` read
// their signatures out of the source; `std`'s are checked because
// `std.contracts` writes them down; a package's will be because a package ships
// its ledger (Part III, 13.5). One mechanism, three sources.
//
// Spans are the enclosing statement's, as they are for the `sync` check:
// expression-level spans are open work in the parser, and a caret on the right
// line is worth more than none at all.

use std::collections::{BTreeMap, BTreeSet};

use crate::ast::{self, BinaryOp, Block, Expr, Item, MatchPattern, Span, Stmt, UnaryOp};
use crate::contracts::{ty, ty::Ty, FnContract, Ledger};
use crate::parser::Parsed;

/// One thing the checker is sure about.
#[derive(Debug, Clone)]
pub struct Finding {
    /// The statement it is in.
    pub span: Span,
    /// Its `NK1xxx` code, from the catalogue in Part III, C.3.
    pub code: &'static str,
    /// The headline, which says what is wrong and never how to think about it.
    pub message: String,
    /// Why the compiler believes it - the two types, and where each came from.
    pub notes: Vec<String>,
    /// One concrete way out. Part III C.2 requires it of every diagnostic.
    pub help: Option<String>,
}

/// Where one function's method calls went (ADR-028).
///
/// A method call is the one shape neither `sync.rs` analysis can resolve on its
/// own: `stats.add(5)` names `add` and says nothing about what `stats` is, and
/// only a type checker knows. This is that answer, handed over.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MethodCalls {
    /// The ledger keys its method calls resolved to.
    pub resolved: BTreeSet<String>,
    /// It calls a method whose receiver type is not known, or one no ledger
    /// has an entry for. **Not the same as calling nothing** - it is the
    /// absence of an answer, and an analysis that claims a property must treat
    /// it as such (ADR-027 D2).
    pub unresolved: bool,
}

/// What one pass of the checker learned.
#[derive(Debug, Clone, Default)]
pub struct Checked {
    /// Every mistake it is sure about.
    pub findings: Vec<Finding>,
    /// The `for` statements whose **step can fail** (ADR-025 D1), by the byte
    /// the statement starts at.
    ///
    /// The emitter reads this. It is here rather than in the emitter because
    /// answering it means inferring the type of the iterator expression, which
    /// is what this module does - and because `let stream = io::lines()`
    /// followed by `for line in stream` has to be the same as the one-line
    /// form, which matching on a name would not give (ADR-025 D7).
    pub fallible_loops: BTreeSet<usize>,
    /// Per function - by the name the ledger records it under - where its
    /// method calls went (ADR-028).
    ///
    /// The second thing the checker answers for somebody else, after
    /// `fallible_loops`, and for the same reason: the question is about types,
    /// and this is the module that has them.
    ///
    /// **A `spawn` body's method calls land on the function around it**, where
    /// `sync.rs` would not walk into one at all. That is harmless rather than
    /// agreed: a function containing a `spawn` has already lost its claim to be
    /// `sync` on the strength of the `spawn`, so nothing is decided by what the
    /// task's body calls.
    pub methods: BTreeMap<String, MethodCalls>,
}

/// Every type mistake the ledgers are enough to see, and every loop that can
/// fail.
///
/// For one file. A program of several (Part I, 9.1) uses [`check_program`],
/// which additionally knows which names are *modules* - and therefore which
/// qualified calls cross a file boundary.
pub fn check(parsed: &Parsed, own: &Ledger, library: &Ledger) -> Checked {
    check_program(parsed, own, library, &BTreeSet::new())
}

/// The same, for one file of a program made of several.
///
/// `modules` is what the program is made of, and it is the whole difference: a
/// qualified call is either into another **module**, where Part I 9.2 says
/// `pub` decides, or into a **type** (`Stats::new`), where it does not. Without
/// the set there is no telling those apart, and a private constructor called in
/// its own file would be reported as a privacy violation.
pub fn check_program(
    parsed: &Parsed,
    own: &Ledger,
    library: &Ledger,
    modules: &BTreeSet<String>,
) -> Checked {
    let mut checker = Checker {
        parsed,
        own,
        library,
        structs: BTreeMap::new(),
        enums: BTreeMap::new(),
        scope: Vec::new(),
        expected: None,
        throwing: false,
        current: None,
        modules: modules.clone(),
        checked: Checked::default(),
    };
    checker.collect_types();
    checker.program();
    checker.checked.findings.sort_by_key(|f| f.span.start);
    checker.checked
}

/// The loops whose step can fail, for a caller that wants only those.
///
/// The emitter's entry point: it builds the ledgers a program is compiled
/// against and asks this, rather than carrying the checker's findings around.
pub fn fallible_loops(parsed: &Parsed) -> BTreeSet<usize> {
    fallible_loops_against(parsed, &Ledger::infer(parsed))
}

/// The same, against contracts the caller already has - which for a program of
/// several files is the **program's** ledger and not this file's (Part I, 9.1).
pub fn fallible_loops_against(parsed: &Parsed, own: &Ledger) -> BTreeSet<usize> {
    let Ok(library) = Ledger::parse(crate::contracts::STD) else {
        return BTreeSet::new();
    };
    check(parsed, own, &library).fallible_loops
}

struct Checker<'a> {
    parsed: &'a Parsed,
    /// This unit's own contracts, inferred from the source being checked.
    own: &'a Ledger,
    /// `std`'s, as `std` ships them.
    library: &'a Ledger,
    /// Every struct declared here, with its fields. A type whose fields are
    /// not known is simply absent, and an absent type is never an error.
    structs: BTreeMap<String, Vec<(String, Ty)>>,
    /// Every enum declared here, with its variant names.
    enums: BTreeMap<String, BTreeSet<String>>,
    /// Names in scope, innermost frame last.
    scope: Vec<Vec<(String, Ty)>>,
    /// What the function being walked declared it hands back.
    expected: Option<Ty>,
    /// Whether it declared `throws` - which is what says a failure may leave
    /// it, whether the failing call was written or implicit (ADR-025 D1).
    throwing: bool,
    /// The function being walked, by the name the ledger records it under.
    ///
    /// `None` inside a grammar action, a `test` or a `bench` - code that
    /// belongs to no function a caller can name, and whose method calls
    /// therefore have nowhere to be recorded.
    current: Option<String>,
    /// The modules this program is made of (Part I, 9.1). Empty for a single
    /// file, where no call crosses a file boundary.
    modules: BTreeSet<String>,
    checked: Checked,
}

impl<'a> Checker<'a> {
    // --- the shape of a program ---------------------------------------------

    fn collect_types(&mut self) {
        for item in &self.parsed.program.items {
            match &item.node {
                Item::Struct {
                    name,
                    generics,
                    fields,
                    ..
                } => {
                    let parameters: BTreeSet<String> = generics
                        .iter()
                        .map(|g| self.parsed.text(g.name).to_string())
                        .collect();
                    let fields = fields
                        .iter()
                        .map(|f| {
                            (
                                self.parsed.text(f.name).to_string(),
                                Ty::from_ast(self.parsed, &f.ty).erase(&parameters),
                            )
                        })
                        .collect();
                    self.structs
                        .insert(self.parsed.text(*name).to_string(), fields);
                }
                Item::Enum { name, variants, .. } => {
                    let variants = variants
                        .iter()
                        .map(|v| self.parsed.text(v.name).to_string())
                        .collect();
                    self.enums
                        .insert(self.parsed.text(*name).to_string(), variants);
                }
                _ => {}
            }
        }
    }

    fn program(&mut self) {
        for item in &self.parsed.program.items {
            match &item.node {
                Item::Fn { .. } => self.function(&item.node, None),
                Item::Impl {
                    target, methods, ..
                } => {
                    let target = self.parsed.text(target.name).to_string();
                    for method in methods {
                        self.function(&method.node, Some(&target));
                    }
                }
                // A test and a bench are code, and nothing about them is
                // exempt from the language's rules (Part III, 14.1 and 13.4).
                // Nothing produces these yet - the items are in the AST and the
                // grammar has no rule for either - so this is what stops their
                // bodies from arriving unchecked on the day it does.
                Item::Test { body, .. } | Item::Bench { body, .. } => {
                    let outer = self.expected.take();
                    self.scope.push(Vec::new());
                    self.block(body);
                    self.scope.pop();
                    self.expected = outer;
                }
                Item::Grammar(grammar) => self.grammar(grammar),
                _ => {}
            }
        }
    }

    /// A grammar's action blocks are Nikaia, and they build the rule's value.
    ///
    /// What a pattern binds has no type here - that is the parser backend's,
    /// and Stage 0 does not read it - so every binding is `?`. What is written
    /// down is the rule's **return type**, and an action that builds something
    /// else is the mistake worth catching: a rule is where a struct literal is
    /// most often typed out in full.
    fn grammar(&mut self, grammar: &ast::GrammarDef) {
        for rule in &grammar.rules {
            let expected = rule.ret_type.as_ref().map(|t| Ty::from_ast(self.parsed, t));
            for alt in &rule.alts {
                let Some(action) = &alt.action else { continue };
                let mut frame = Vec::new();
                self.bindings_of(&alt.pattern.node, &mut frame);
                let outer = std::mem::replace(&mut self.expected, expected.clone());
                self.scope.push(frame);
                let tail_span = action.stmts.last().map(|s| s.span.clone());
                let tail = self.block(action);
                self.scope.pop();
                if let (Some(expected), Some(span)) = (&expected, tail_span) {
                    self.expect(&tail, expected, span, "returns", |found, want| {
                        format!("this action builds `{found}`, and its rule declares `{want}`")
                    });
                }
                self.expected = outer;
            }
        }
    }

    /// Every `name:pattern` in a pattern, all of them `?`.
    fn bindings_of(&self, pattern: &ast::Pattern, out: &mut Vec<(String, Ty)>) {
        match pattern {
            ast::Pattern::Bind { name, pat } => {
                out.push((self.parsed.text(*name).to_string(), Ty::Unknown));
                self.bindings_of(&pat.node, out);
            }
            ast::Pattern::Seq(parts) | ast::Pattern::Choice(parts) => {
                for part in parts {
                    self.bindings_of(&part.node, out);
                }
            }
            ast::Pattern::Ref { args, .. } => {
                for arg in args {
                    self.bindings_of(&arg.node, out);
                }
            }
            ast::Pattern::Repeat { pat, .. } | ast::Pattern::Group(pat) => {
                self.bindings_of(&pat.node, out)
            }
            ast::Pattern::Literal(_) | ast::Pattern::Cut | ast::Pattern::Fold(_) => {}
        }
    }

    fn function(&mut self, item: &Item, target: Option<&str>) {
        let Item::Fn {
            name,
            generics,
            receiver,
            args,
            ret_type,
            body,
            throws,
            ..
        } = item
        else {
            return;
        };

        // The same key the ledger uses, arrived at the same way - the anonymous
        // constructor of Kap 4.2 included, which a caller reaches as
        // `Type::new`. Two spellings of one name would silently drop every
        // method call in a constructor.
        let own_name = match name {
            Some(name) => self.parsed.text(*name).to_string(),
            None => "new".to_string(),
        };
        let key = match target {
            Some(target) => format!("{target}::{own_name}"),
            None => own_name,
        };
        let outer_current = self.current.replace(key);

        let mut parameters: BTreeSet<String> = generics
            .iter()
            .map(|g| self.parsed.text(g.name).to_string())
            .collect();
        // `Self` stands for the type the `impl` is on, and nothing here
        // resolves it - so it is a name that stands for a type, like `T`.
        parameters.insert("Self".to_string());

        let mut frame: Vec<(String, Ty)> = Vec::new();
        if let Some(receiver) = receiver {
            let ty = match target {
                Some(target) if receiver.is_ref => Ty::view(target),
                Some(target) => Ty::named(target),
                None => Ty::Unknown,
            };
            frame.push(("self".to_string(), ty));
        }
        for arg in args {
            frame.push((
                self.parsed.text(arg.name).to_string(),
                Ty::from_ast(self.parsed, &arg.ty).erase(&parameters),
            ));
        }

        let expected = ret_type
            .as_ref()
            .map(|t| Ty::from_ast(self.parsed, t).erase(&parameters));
        let outer = std::mem::replace(&mut self.expected, expected.clone());
        let outer_throwing = std::mem::replace(&mut self.throwing, *throws);

        self.scope.push(frame);
        let tail_span = body.stmts.last().map(|s| s.span.clone());
        let tail = self.block(body);
        self.scope.pop();

        // The last expression of a body is what the function hands back, so it
        // answers to the declared type exactly as a `return` does.
        if let (Some(expected), Some(span)) = (&expected, tail_span) {
            self.expect(&tail, expected, span, "returns", |found, want| {
                format!("this function hands back `{found}`, and it declares `{want}`")
            });
        }

        self.expected = outer;
        self.throwing = outer_throwing;
        self.current = outer_current;
    }

    // --- statements ---------------------------------------------------------

    /// Walk a block and hand back the type of its tail.
    fn block(&mut self, block: &Block) -> Ty {
        self.scope.push(Vec::new());
        let mut tail = Ty::Tuple(Vec::new());
        let last = block.stmts.len().saturating_sub(1);
        for (at, stmt) in block.stmts.iter().enumerate() {
            let ty = self.stmt(&stmt.node, &stmt.span);
            if at == last {
                tail = ty;
            }
        }
        self.scope.pop();
        tail
    }

    fn stmt(&mut self, stmt: &Stmt, span: &Span) -> Ty {
        match stmt {
            Stmt::Let {
                name, ty, value, ..
            } => {
                let found = self.expr(value, span);
                let bound = match ty {
                    Some(ty) => {
                        let want = Ty::from_ast(self.parsed, ty);
                        self.expect(&found, &want, span.clone(), "let", |found, want| {
                            format!("this is `{found}`, and the `let` says `{want}`")
                        });
                        want
                    }
                    None => found,
                };
                let name = self.parsed.text(*name).to_string();
                self.bind(name, bound);
                Ty::Tuple(Vec::new())
            }

            Stmt::Assign {
                target, op, value, ..
            } => {
                let into = self.expr(target, span);
                let found = self.expr(value, span);
                // Only a plain assignment: `n += 1` is whatever the operator
                // makes of the two, and Stage 0 does not model operators.
                if op.is_none() {
                    self.expect(&found, &into, span.clone(), "assign", |found, want| {
                        format!("this is `{found}`, and what it is assigned to is `{want}`")
                    });
                }
                Ty::Tuple(Vec::new())
            }

            Stmt::While { cond, body } => {
                let cond_ty = self.expr(cond, span);
                self.expect_bool(&cond_ty, span, "a `while` repeats while a `bool` holds");
                self.scope.push(Vec::new());
                self.block(body);
                self.scope.pop();
                Ty::Tuple(Vec::new())
            }

            Stmt::For {
                bindings,
                iter,
                body,
            } => {
                let over = self.expr(iter, span);
                self.fallible_step(&over, bindings.len(), span);
                let element = element_of(&over, bindings.len());
                let frame = bindings
                    .iter()
                    .map(|b| (self.parsed.text(*b).to_string(), element.clone()))
                    .collect();
                self.scope.push(frame);
                self.block(body);
                self.scope.pop();
                Ty::Tuple(Vec::new())
            }

            Stmt::Return(value) => {
                let found = match value {
                    Some(value) => self.expr(value, span),
                    None => Ty::Tuple(Vec::new()),
                };
                if let Some(expected) = self.expected.clone() {
                    self.expect(&found, &expected, span.clone(), "returns", |found, want| {
                        format!("this returns `{found}`, and the function declares `{want}`")
                    });
                }
                Ty::Unknown
            }

            Stmt::Expr(expr) => self.expr(expr, span),
        }
    }

    // --- expressions --------------------------------------------------------

    fn expr(&mut self, expr: &Expr, span: &Span) -> Ty {
        match expr {
            // A bare number fits every numeric type, exactly as it does in the
            // language below. Committing it to one here would make `add(3)`
            // wrong wherever the parameter is not that one.
            Expr::LitInt(_) | Expr::LitFloat(_) => Ty::Unknown,
            // Part I 2.4 calls a string literal a `String`; Stage 0 emits a
            // Rust string literal, which is a view of static text. The checker
            // says what is emitted - see ADR-024 D5.
            //
            // **Unless it has a hole in it.** `"{a} rows"` is not a literal in
            // the emitted Rust, it is a `format!`, and a `format!` is a
            // `String`. Two spellings in Nikaia, two types below, and the
            // checker follows the lowering rather than the syntax.
            Expr::LitStr(text) => {
                // **A hole is checked like anything else.** It is Nikaia source
                // written inside a literal, and until this walk existed it was
                // source no analysis could see - the same mistake was caught
                // outside a hole and silently passed inside one.
                self.holes(expr, span);
                match crate::emit::interpolation(text) {
                    Ok((_, holes)) if !holes.is_empty() => Ty::named("String"),
                    Ok(_) => Ty::view("str"),
                    // A literal the emitter will refuse. It reports that in its
                    // own words; this one says nothing rather than guessing.
                    Err(_) => Ty::Unknown,
                }
            }
            Expr::LitChar(_) => Ty::named("char"),
            Expr::LitBool(_) => Ty::named("bool"),

            Expr::Variable(name) => {
                let name = self.parsed.text(*name);
                self.lookup(name).unwrap_or(Ty::Unknown)
            }

            Expr::Path(segments) => {
                // `Op::Times` is a value of the enum that declares it. Anything
                // else a path can name, this compiler does not resolve.
                let names: Vec<&str> = segments.iter().map(|s| self.parsed.text(*s)).collect();
                match names.as_slice() {
                    [ty, variant] if self.is_variant(ty, variant) => Ty::named(*ty),
                    _ => Ty::Unknown,
                }
            }

            Expr::Block(block) => self.block(block),

            Expr::If {
                cond,
                then_branch,
                else_branch,
            } => {
                let cond_ty = self.expr(cond, span);
                self.expect_bool(&cond_ty, span, "an `if` decides on a `bool`");
                let then = self.block(then_branch);
                match else_branch {
                    Some(otherwise) => {
                        let other = self.block(otherwise);
                        // Only when both arms agree is there something to say.
                        if then == other {
                            then
                        } else {
                            Ty::Unknown
                        }
                    }
                    // An `if` with no `else` is a statement's worth of value.
                    None => Ty::Unknown,
                }
            }

            Expr::Match { value, arms } => {
                self.expr(value, span);
                let mut result: Option<Ty> = None;
                let mut agree = true;
                for arm in arms {
                    let frame = self.pattern_bindings(&arm.pattern);
                    self.scope.push(frame);
                    let ty = self.expr(&arm.body, span);
                    self.scope.pop();
                    match &result {
                        None => result = Some(ty),
                        Some(seen) if *seen == ty => {}
                        Some(_) => agree = false,
                    }
                }
                // Every arm of a `match` is a value of the same type, but what
                // that type is, is only known when every arm says the same.
                match result {
                    Some(ty) if agree => ty,
                    _ => Ty::Unknown,
                }
            }

            Expr::Call { func, args, config } => self.call(func, args, config, span),

            Expr::MethodCall {
                receiver,
                method,
                args,
            } => {
                let on = self.expr(receiver, span);
                let Ty::Named { name, .. } = &on else {
                    // The receiver's type is not known, so neither is what this
                    // calls. Recorded, because "I could not find out" is an
                    // answer somebody downstream has to act on.
                    args.iter().for_each(|a| {
                        self.expr(a, span);
                    });
                    self.reached_method(None);
                    return Ty::Unknown;
                };
                let key = format!("{name}::{}", self.parsed.text(*method));
                let Some((key, contract)) = self.method(&key) else {
                    // The type is known and no ledger describes this method of
                    // it - `HashMap::entry` until something writes it down.
                    args.iter().for_each(|a| {
                        self.expr(a, span);
                    });
                    self.reached_method(None);
                    return Ty::Unknown;
                };
                self.reached_method(Some(&key));

                // What the receiver's own type tells the signature (ADR-031).
                // `HashMap[&str, Stats]` against `&HashMap[$K, $V]` binds `$V`
                // to `Stats`, so `-> Entry[$V]` is an `Entry[Stats]` and the
                // next call in the chain has something to bind from in turn.
                let bound = bindings(contract, &on);

                // The arguments are walked **after** the contract is in hand,
                // which is what lets a lambda's parameters have types (ADR-029).
                // The old order walked them first and could not: `a` in
                // `.and_modify fn { a.add(t) }` is named nowhere and typed by
                // nothing but the callee's signature.
                let expected: Vec<Ty> = expected_arguments(contract)
                    .iter()
                    .map(|ty| ty::substitute(ty, &bound))
                    .collect();
                let found = self.arguments_given(args, &expected, span);
                let result = self.arguments(&key, contract, &found, &[], span);
                ty::substitute(&result, &bound)
            }

            Expr::Field { base, name } => {
                let on = self.expr(base, span);
                let field = self.parsed.text(*name).to_string();
                let Ty::Named { name: ty, .. } = &on else {
                    return Ty::Unknown;
                };
                let Some(fields) = self.fields_of(ty) else {
                    return Ty::Unknown;
                };
                match fields.iter().find(|(f, _)| *f == field) {
                    Some((_, ty)) => ty.clone(),
                    None => {
                        let ty = ty.clone();
                        self.no_such_field(&ty, &field, &fields, span);
                        Ty::Unknown
                    }
                }
            }

            Expr::StructLit { name, fields } => {
                let name = self.parsed.text(*name).to_string();
                let declared = self.fields_of(&name);
                for init in fields {
                    let field = self.parsed.text(init.name).to_string();
                    // `Reading { name, temp }` is shorthand for `name: name`.
                    let found = match &init.value {
                        Some(value) => self.expr(value, span),
                        None => self.lookup(&field).unwrap_or(Ty::Unknown),
                    };
                    let Some(declared) = &declared else { continue };
                    match declared.iter().find(|(f, _)| *f == field) {
                        Some((_, want)) => {
                            let want = want.clone();
                            let owner = name.clone();
                            self.expect(
                                &found,
                                &want,
                                span.clone(),
                                "field",
                                move |found, want| {
                                    format!("`{owner}.{field}` is `{want}`, and this is `{found}`")
                                },
                            );
                        }
                        None => self.no_such_field(&name, &field, declared, span),
                    }
                }
                Ty::named(name)
            }

            Expr::Closure { params, body, .. } => {
                let frame = params
                    .iter()
                    .map(|p| (self.parsed.text(*p).to_string(), Ty::Unknown))
                    .collect();
                self.scope.push(frame);
                self.block(body);
                self.scope.pop();
                Ty::Unknown
            }

            Expr::Unary { op, expr } => {
                let inner = self.expr(expr, span);
                match op {
                    UnaryOp::Neg => inner,
                    // `!` is a `bool`'s, and the language below spells a
                    // bitwise complement the same way. Nikaia has no bitwise
                    // operator today, so this claims `bool` where the operand
                    // agrees and claims nothing where it does not - rather than
                    // insisting on `bool` and being wrong the day one arrives.
                    UnaryOp::Not => {
                        let boolean = Ty::named("bool");
                        if inner.fits(&boolean) {
                            boolean
                        } else {
                            Ty::Unknown
                        }
                    }
                    UnaryOp::Ref => view_of(&inner),
                }
            }

            Expr::Binary { op, lhs, rhs } => {
                let left = self.expr(lhs, span);
                let right = self.expr(rhs, span);
                match op {
                    BinaryOp::And | BinaryOp::Or => {
                        self.expect_bool(&left, span, "`&&` and `||` join two `bool`s");
                        self.expect_bool(&right, span, "`&&` and `||` join two `bool`s");
                        Ty::named("bool")
                    }
                    BinaryOp::Eq
                    | BinaryOp::Ne
                    | BinaryOp::Lt
                    | BinaryOp::Le
                    | BinaryOp::Gt
                    | BinaryOp::Ge => Ty::named("bool"),
                    // Arithmetic on two of the same thing is that thing, and
                    // a bare number is neither - so one known side decides. Two
                    // known sides that disagree decide nothing: `a + b` over a
                    // `String` and a `&str` is a concatenation in the language
                    // below, and guessing which side names the result would be
                    // the one guess this checker does not make.
                    _ => match (left.is_unknown(), right.is_unknown()) {
                        (true, _) => right,
                        (_, true) => left,
                        _ if left == right => left,
                        _ => Ty::Unknown,
                    },
                }
            }

            Expr::Cast { expr, ty } => {
                self.expr(expr, span);
                Ty::from_ast(self.parsed, ty)
            }

            Expr::Tuple(parts) => Ty::Tuple(parts.iter().map(|p| self.expr(p, span)).collect()),

            // A `?` unwraps a failure, a `??` unwraps an absence, an index
            // reaches into a container and a range is an iterator: four things
            // Stage 0 has no signature for.
            Expr::Try(inner) => {
                self.expr(inner, span);
                Ty::Unknown
            }
            // Kap 7.1: `throw` leaves the function, so it has no value of its
            // own - the same shape a `return` has. What it throws is walked,
            // because a mistyped constructor inside it is still a mistake.
            Expr::Throw(inner) => {
                self.expr(inner, span);
                Ty::Unknown
            }
            Expr::Coalesce { value, fallback } => {
                self.expr(value, span);
                self.expr(fallback, span);
                Ty::Unknown
            }
            // Indexing a container yields what the container holds - but only
            // where the container's type says so.
            //
            // This claimed nothing at all until ADR-028, and the cost was not
            // the missing type but everything downstream of it: in
            // `n-body.nika`, `let b = &self.bodies[i]` made `b` unknown, so
            // `b.x` was unknown, so `dx * dx + dy * dy` was unknown, so
            // `.sqrt()` could not be resolved and `energy` could not be shown
            // to be pure computation. One `Unknown` at the bottom of an
            // expression erases everything built on it.
            //
            // A shape it does not recognise still claims nothing: `s[i]` over
            // text is a slice in some languages and a byte in others, and
            // Nikaia has not said. `?` is the absence of a claim (ADR-024 D1).
            Expr::Index { base, index } => {
                let on = self.expr(base, span);
                self.expr(index, span);
                let Ty::Named { name, args, .. } = &on else {
                    return Ty::Unknown;
                };
                match (name.as_str(), args.as_slice()) {
                    ("Vec" | "List", [item]) => item.clone(),
                    // A map is indexed by its key and yields its value.
                    ("HashMap" | "Map", [_, value]) => value.clone(),
                    _ => Ty::Unknown,
                }
            }
            Expr::Range { start, end, .. } => {
                self.expr(start, span);
                self.expr(end, span);
                Ty::Unknown
            }

            Expr::TryCatch { expr, handler } => {
                self.expr(expr, span);
                self.scope.push(vec![("error".to_string(), Ty::Unknown)]);
                self.block(handler);
                self.scope.pop();
                Ty::Unknown
            }

            Expr::Spawn { body, .. } => {
                self.expr(body, span);
                Ty::Unknown
            }

            Expr::DslFrom { input, .. } => {
                self.expr(input, span);
                Ty::Unknown
            }

            // A template's holes are Nikaia too (ADR-017), and what the
            // template *produces* is still the emitter's business.
            Expr::Dsl { .. } => {
                self.holes(expr, span);
                Ty::Unknown
            }

            // A grammar and an `asm` block: what these produce is the business
            // of the emitter that compiles them.
            Expr::Asm { .. } => Ty::Unknown,
        }
    }

    /// Every expression a literal hides, walked where it stands.
    ///
    /// The result is discarded: what a hole evaluates to is the emitter's
    /// business - `Render` decides for a template and `Display` for a string -
    /// and what the checker is here for is everything *inside* it.
    fn holes(&mut self, literal: &Expr, span: &Span) {
        for hole in crate::emit::literal_expressions(self.parsed, literal) {
            self.expr(&hole, span);
        }
    }

    /// `f(a, b)`, `Stats(first)`, `io::read_to_string()`, `write(p, d; append: true)`.
    fn call(&mut self, func: &Expr, args: &[Expr], config: &[ast::ConfigArg], span: &Span) -> Ty {
        let found: Vec<Ty> = args.iter().map(|a| self.expr(a, span)).collect();
        let passed: Vec<(String, Ty)> = config
            .iter()
            .map(|a| {
                (
                    self.parsed.text(a.name).to_string(),
                    self.expr(&a.value, span),
                )
            })
            .collect();

        let name = match func {
            Expr::Variable(name) => self.parsed.text(*name).to_string(),
            Expr::Path(segments) => segments
                .iter()
                .map(|s| self.parsed.text(*s))
                .collect::<Vec<_>>()
                .join("::"),
            other => {
                self.expr(other, span);
                return Ty::Unknown;
            }
        };

        // A tuple variant of an enum declared here - `Op::Plus(1)` - is a value
        // of that enum, not a call to a function.
        if let Some((ty, variant)) = name.split_once("::") {
            if self.is_variant(ty, variant) {
                return Ty::named(ty);
            }
        }

        let Some((key, contract)) = self.resolve(&name) else {
            return Ty::Unknown;
        };
        self.reachable(&name, contract, span);
        // `Stats(first)` is the anonymous constructor of Kap 4.2, which the
        // lowering names `Stats::new` - and which hands back the type it is on,
        // whatever its declaration says about `Self`.
        let constructed = key
            .strip_suffix("::new")
            .filter(|_| !name.ends_with("::new"))
            .map(Ty::named);
        let result = self.arguments(&key, contract, &found, &passed, span);
        constructed.unwrap_or(result)
    }

    /// The count and the types of what a call passes, against what it takes.
    fn arguments(
        &mut self,
        key: &str,
        contract: &FnContract,
        found: &[Ty],
        passed: &[(String, Ty)],
        span: &Span,
    ) -> Ty {
        // No signature is no claim. The hand-written half of `std.contracts`
        // has some, and an entry that says nothing is checked against nothing.
        let Some(signature) = contract.signature.clone() else {
            return Ty::Unknown;
        };
        let wanted = signature.arguments();

        if wanted.len() != found.len() {
            self.checked.findings.push(Finding {
                span: span.clone(),
                code: "NK1101",
                message: format!(
                    "`{key}` takes {}, and this call passes {}",
                    plural(wanted.len(), "argument"),
                    found.len()
                ),
                notes: vec![format!("`{key}{}`", signature.text())],
                help: Some(match wanted.len() {
                    0 => format!("call it as `{key}()`"),
                    _ => format!(
                        "it takes {}",
                        wanted
                            .iter()
                            .map(|(n, t)| format!("`{n}: {}`", t.text()))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                }),
            });
            return signature.result_or_unit();
        }

        // Kap 5.1: an option is named, so it is checked by name - that it
        // exists, and that what is passed is what it takes.
        for (name, found) in passed {
            match signature.config.iter().find(|c| c.name == *name) {
                Some(option) => {
                    if found.fits(&option.ty) {
                        continue;
                    }
                    let (want, ty) = (option.ty.clone(), option.ty.text());
                    let name = name.clone();
                    let key = key.to_string();
                    self.expect(found, &want, span.clone(), "field", move |found, _| {
                        format!("`{key}` takes `{name}: {ty}`, and this passes `{found}`")
                    });
                }
                None => self.no_such_option(key, name, &signature, span),
            }
        }

        for ((name, want), found) in wanted.iter().zip(found) {
            if found.fits(want) {
                continue;
            }
            self.checked.findings.push(Finding {
                span: span.clone(),
                code: "NK1102",
                message: format!(
                    "`{key}` takes `{name}: {}`, and this call passes `{}`",
                    want.text(),
                    found.text()
                ),
                notes: vec![format!("`{key}{}`", signature.text())],
                help: Some(convert(found, want)),
            });
        }

        signature.result_or_unit()
    }

    // --- the one shape every check has --------------------------------------

    /// Report only when both sides are known and they disagree.
    fn expect(
        &mut self,
        found: &Ty,
        want: &Ty,
        span: Span,
        what: &str,
        message: impl FnOnce(&str, &str) -> String,
    ) {
        if found.fits(want) {
            return;
        }
        let code = match what {
            "let" => "NK1103",
            "returns" => "NK1104",
            "assign" => "NK1105",
            "field" => "NK1106",
            other => unreachable!("no code for `{other}`"),
        };
        self.checked.findings.push(Finding {
            span,
            code,
            message: message(&found.text(), &want.text()),
            notes: Vec::new(),
            help: Some(convert(found, want)),
        });
    }

    fn expect_bool(&mut self, found: &Ty, span: &Span, why: &str) {
        let bool_ty = Ty::named("bool");
        if found.fits(&bool_ty) {
            return;
        }
        self.checked.findings.push(Finding {
            span: span.clone(),
            code: "NK1108",
            message: format!("this is `{}`, and a condition is a `bool`", found.text()),
            notes: vec![why.to_string()],
            help: Some(
                "compare it: `x != 0`, `text != \"\"`, `xs.len() > 0` (Part I, 3.2)".to_string(),
            ),
        });
    }

    /// A loop over something whose step can fail (ADR-025 D1).
    ///
    /// Two things follow, and they are the two halves of the decision: the
    /// enclosing function must declare `throws`, and the emitter has to make
    /// the step propagate. This records the second and reports the first.
    fn fallible_step(&mut self, over: &Ty, bindings: usize, span: &Span) {
        let Ty::Named { name, .. } = over else {
            return;
        };
        if !self.iterates_fallibly(name) {
            return;
        }

        // A stream of pairs does not exist in `std`, and taking one apart while
        // also unwrapping a failure is a shape to design rather than to guess
        // at (ADR-025 §7).
        if bindings != 1 {
            self.checked.findings.push(Finding {
                span: span.clone(),
                code: "NK2701",
                message: format!("a `for` over `{name}` binds one name, and this binds {bindings}"),
                notes: vec![format!("each turn of `{name}` can fail, and the failure is what the one binding unwraps")],
                help: Some("bind one name and take the pair apart inside the loop".to_string()),
            });
            return;
        }

        self.checked.fallible_loops.insert(span.start);

        if self.throwing {
            return;
        }
        self.checked.findings.push(Finding {
            span: span.clone(),
            code: "NK2701",
            message: "this function can fail because a turn of this loop can fail".to_string(),
            notes: vec![format!(
                "`{name}` reads as it goes, and a read can fail - so the failure leaves this \
                 function, exactly as a failing call would"
            )],
            help: Some("declare the error: add `throws` to this function".to_string()),
        });
    }

    /// Part I 9.2: an item is private to its file unless it says `pub`.
    ///
    /// The language below enforces this too - `pub` becomes `pub` and a `mod`
    /// keeps what it was not given - but a reader should not meet the rule as a
    /// `rustc` message about a file they did not write, which is what
    /// Part III C.1 calls a bug in this compiler.
    ///
    /// Only a call written `module::item` can be from another file: a call
    /// inside `utils.nika` writes `secret()`, unqualified. So this needs no
    /// notion of "which file am I in" - the spelling says it.
    fn reachable(&mut self, name: &str, contract: &FnContract, span: &Span) {
        let Some((module, item)) = name.split_once("::") else {
            return;
        };
        if !self.modules.contains(module) || contract.public {
            return;
        }
        self.checked.findings.push(Finding {
            span: span.clone(),
            code: "NK1110",
            message: format!("`{item}` is private to `{module}.nika`"),
            notes: vec!["an item is private to the file that declares it unless it says `pub` (Part I, 9.2)".to_string()],
            help: Some(format!(
                "write `pub fn {item}` in `{module}.nika`, or reach it through something that is public"
            )),
        });
    }

    /// An option the callee does not have (Kap 5.1).
    fn no_such_option(
        &mut self,
        key: &str,
        name: &str,
        signature: &crate::contracts::Signature,
        span: &Span,
    ) {
        let names: Vec<&str> = signature.config.iter().map(|c| c.name.as_str()).collect();
        let near = nearest(name, &names);
        self.checked.findings.push(Finding {
            span: span.clone(),
            code: "NK1109",
            message: format!("`{key}` has no option `{name}`"),
            notes: vec![match names.is_empty() {
                true => format!("`{key}` takes no options at all - it has no `;`"),
                false => format!("`{key}` takes {}", list(&names)),
            }],
            help: Some(match near {
                Some(near) => format!("did you mean `{near}`?"),
                None if names.is_empty() => {
                    "everything before the `;` is positional (Part I, 5.1)".to_string()
                }
                None => "name one of the options it has".to_string(),
            }),
        });
    }

    fn no_such_field(&mut self, ty: &str, field: &str, declared: &[(String, Ty)], span: &Span) {
        let names: Vec<&str> = declared.iter().map(|(f, _)| f.as_str()).collect();
        let near = nearest(field, &names);
        self.checked.findings.push(Finding {
            span: span.clone(),
            code: "NK1107",
            message: format!("`{ty}` has no field `{field}`"),
            notes: vec![format!("`{ty}` has {}", list(&names))],
            help: Some(match near {
                Some(near) => format!("did you mean `{near}`?"),
                None => format!("add `{field}` to `{ty}`, or use one of the fields it has"),
            }),
        });
    }

    // --- looking things up ---------------------------------------------------

    fn bind(&mut self, name: String, ty: Ty) {
        if let Some(frame) = self.scope.last_mut() {
            frame.push((name, ty));
        }
    }

    fn lookup(&self, name: &str) -> Option<Ty> {
        self.scope
            .iter()
            .rev()
            .find_map(|frame| frame.iter().rev().find(|(n, _)| n == name))
            .map(|(_, ty)| ty.clone())
    }

    /// A function by the name a call wrote: this unit's, then a constructor,
    /// then a library's - the same order the `sync` check resolves in.
    fn resolve(&self, name: &str) -> Option<(String, &'a FnContract)> {
        if let Some(contract) = self.own.functions.get(name) {
            return Some((name.to_string(), contract));
        }
        let constructed = format!("{name}::new");
        if let Some(contract) = self.own.functions.get(&constructed) {
            return Some((constructed, contract));
        }
        self.library.lookup(name)
    }

    /// A method on a type, by the name `Type::method` the ledger records it
    /// under. A library writes the module in front of it (`fs::Mapped::deref`)
    /// and the receiver's type does not carry one, so the suffix is what
    /// matches - name-for-name resolution, as everywhere else.
    /// Walk a call's arguments, telling a lambda what it will be handed.
    ///
    /// The types come from the callee's signature, so this can only run once
    /// the callee is known - which is why the resolution moved ahead of the
    /// walk (ADR-029). An argument whose parameter says nothing is walked
    /// exactly as it was before.
    fn arguments_given(&mut self, args: &[Expr], expected: &[Ty], span: &Span) -> Vec<Ty> {
        args.iter()
            .enumerate()
            .map(|(at, arg)| match (arg, expected.get(at)) {
                (
                    Expr::Closure {
                        params,
                        implicit,
                        body,
                    },
                    Some(Ty::Fn { params: given }),
                ) => self.lambda(params, *implicit, body, given),
                _ => self.expr(arg, span),
            })
            .collect()
    }

    /// A lambda whose parameters have types, because the callee said so.
    ///
    /// The implicit form is the one that matters and the one that had no way
    /// to work: `fn { a.add(t) }` records **no parameters at all** in the AST -
    /// Part I 5.3 settles how many it takes from which of `a`, `b`, `c` the
    /// body mentions, and that is decided when it is emitted. So the names are
    /// bound here, in order, to whatever the signature says the lambda is
    /// handed. A body that mentions fewer of them simply leaves the later
    /// bindings unused.
    fn lambda(
        &mut self,
        params: &[winnow_grammar::Symbol],
        implicit: bool,
        body: &Block,
        given: &[Ty],
    ) -> Ty {
        const IMPLICIT: [&str; 3] = ["a", "b", "c"];

        let names: Vec<String> = if implicit {
            IMPLICIT
                .iter()
                .take(given.len())
                .map(|n| n.to_string())
                .collect()
        } else {
            params
                .iter()
                .map(|p| self.parsed.text(*p).to_string())
                .collect()
        };

        let frame = names
            .into_iter()
            .enumerate()
            .map(|(at, name)| (name, given.get(at).cloned().unwrap_or(Ty::Unknown)))
            .collect();

        self.scope.push(frame);
        self.block(body);
        self.scope.pop();
        // What a lambda hands back is not written down anywhere yet, and
        // claiming it here would be inventing one (ADR-029 D1).
        Ty::Unknown
    }

    /// Note where a method call in the function being walked went (ADR-028).
    ///
    /// `None` is "I could not find out", and it is recorded rather than
    /// dropped: an analysis that claims a property has to be able to tell that
    /// apart from a body that called nothing.
    fn reached_method(&mut self, key: Option<&str>) {
        let Some(current) = &self.current else {
            return;
        };
        let entry = self.checked.methods.entry(current.clone()).or_default();
        match key {
            Some(key) => {
                entry.resolved.insert(key.to_string());
            }
            None => entry.unresolved = true,
        }
    }

    fn method(&self, key: &str) -> Option<(String, &'a FnContract)> {
        if let Some(contract) = self.own.functions.get(key) {
            return Some((key.to_string(), contract));
        }
        let suffix = format!("::{key}");
        self.library
            .functions
            .iter()
            .find(|(name, _)| *name == key || name.ends_with(&suffix))
            .map(|(name, contract)| (name.clone(), contract))
    }

    /// The fields of a type, when something knows them.
    fn fields_of(&self, name: &str) -> Option<Vec<(String, Ty)>> {
        if let Some(fields) = self.structs.get(name) {
            return Some(fields.clone()).filter(|f: &Vec<_>| !f.is_empty());
        }
        let suffix = format!("::{name}");
        self.library
            .types
            .iter()
            .find(|(key, _)| *key == name || key.ends_with(&suffix))
            .map(|(_, contract)| contract.fields.clone())
            .filter(|f| !f.is_empty())
    }

    /// Whether a step of this type can fail, as a ledger records it
    /// (ADR-025 D6). Matched by suffix, because a library writes the module in
    /// front of a type's name and a value's type does not carry one.
    fn iterates_fallibly(&self, name: &str) -> bool {
        let suffix = format!("::{name}");
        [self.own, self.library].iter().any(|ledger| {
            ledger.types.iter().any(|(key, contract)| {
                (key == name || key.ends_with(&suffix)) && contract.iterates_fallibly
            })
        })
    }

    fn is_variant(&self, ty: &str, variant: &str) -> bool {
        self.enums
            .get(ty)
            .is_some_and(|variants| variants.contains(variant))
    }

    /// The names a `match` arm brings into scope, all of them unknown: what a
    /// variant carries is not in the ledger yet.
    fn pattern_bindings(&self, pattern: &MatchPattern) -> Vec<(String, Ty)> {
        match pattern {
            MatchPattern::Wildcard | MatchPattern::Literal(_) => Vec::new(),
            // A single segment binds; `Op::Times` names a variant.
            MatchPattern::Path(segments) if segments.len() == 1 => {
                vec![(self.parsed.text(segments[0]).to_string(), Ty::Unknown)]
            }
            MatchPattern::Path(_) => Vec::new(),
            MatchPattern::Tuple { bindings, .. } | MatchPattern::Named { bindings, .. } => bindings
                .iter()
                .map(|b| (self.parsed.text(*b).to_string(), Ty::Unknown))
                .collect(),
        }
    }
}

/// What one turn of a `for` binds, when the collection's element type is
/// written down.
///
/// Only a list with one element type and one binding: `for (k, v) in map`
/// takes apart a pair whose shape Stage 0 has no signature for, and a view of
/// a collection yields views whose spelling the language below chooses.
fn element_of(over: &Ty, bindings: usize) -> Ty {
    match over {
        Ty::Named { name, args, view } if bindings == 1 && !view && args.len() == 1 => {
            match name.as_str() {
                "Vec" | "List" => args[0].clone(),
                _ => Ty::Unknown,
            }
        }
        _ => Ty::Unknown,
    }
}

/// `&x`, as far as Stage 0 can say.
///
/// A view of a `String` is a `&str`, because that is what the language below
/// does at a call and what a Nikaia programmer means by writing it. A view of
/// anything with type arguments is not stated: `&Vec[T]` and `&[T]` are the
/// same expression there, and picking one would report an error that is not
/// there.
fn view_of(inner: &Ty) -> Ty {
    match inner {
        Ty::Named { name, args, view } if args.is_empty() => {
            if *view {
                inner.clone()
            } else if name == "String" {
                Ty::view("str")
            } else {
                Ty::view(name.clone())
            }
        }
        _ => Ty::Unknown,
    }
}

/// Part III C.2: every diagnostic names a concrete way out.
fn convert(found: &Ty, want: &Ty) -> String {
    let (found, want) = (found.text(), want.text());
    match (found.as_str(), want.as_str()) {
        ("&str", "String") => "write `.to_string()` to make a `String` of it".to_string(),
        ("String", "&str") => "write `&` to take a view of it".to_string(),
        _ if is_number(&found) && is_number(&want) => {
            format!("write `as {want}` - Nikaia converts where you say so, never quietly")
        }
        _ => format!("make it a `{want}`, or change what is declared to `{found}`"),
    }
}

fn is_number(name: &str) -> bool {
    matches!(
        name,
        "i8" | "i16"
            | "i32"
            | "i64"
            | "isize"
            | "u8"
            | "u16"
            | "u32"
            | "u64"
            | "usize"
            | "f32"
            | "f64"
    )
}

/// The closest field name, when one is close enough to be worth suggesting.
fn nearest<'n>(name: &str, among: &[&'n str]) -> Option<&'n str> {
    among
        .iter()
        .map(|candidate| (distance(name, candidate), *candidate))
        .filter(|(d, _)| *d * 3 <= name.len().max(1))
        .min_by_key(|(d, _)| *d)
        .map(|(_, candidate)| candidate)
}

/// Edit distance counting a swapped pair as **one** edit.
///
/// Plain Levenshtein charges two for `nmae` against `name`, which puts the most
/// common typo there is outside any threshold worth having. This is the
/// optimal-string-alignment variant, which charges one.
fn distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut d = vec![vec![0usize; b.len() + 1]; a.len() + 1];
    for (i, row) in d.iter_mut().enumerate() {
        row[0] = i;
    }
    for (j, cell) in d[0].iter_mut().enumerate() {
        *cell = j;
    }

    for i in 1..=a.len() {
        for j in 1..=b.len() {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            let mut best = (d[i - 1][j] + 1)
                .min(d[i][j - 1] + 1)
                .min(d[i - 1][j - 1] + cost);
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                best = best.min(d[i - 2][j - 2] + 1);
            }
            d[i][j] = best;
        }
    }

    d[a.len()][b.len()]
}

fn plural(n: usize, what: &str) -> String {
    if n == 1 {
        format!("1 {what}")
    } else {
        format!("{n} {what}s")
    }
}

fn list(names: &[&str]) -> String {
    match names {
        [] => "no fields".to_string(),
        [one] => format!("`{one}`"),
        [rest @ .., last] => format!(
            "{} and `{last}`",
            rest.iter()
                .map(|n| format!("`{n}`"))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

/// What the receiver's actual type binds this signature's variables to.
///
/// The receiver is the signature's first parameter where there is one, so this
/// is one `bind` against one pattern - the narrowness is ADR-031's decision
/// rather than a gap. A signature with no variables produces an empty map and
/// every substitution below is the identity.
fn bindings(contract: &FnContract, receiver: &Ty) -> BTreeMap<String, Ty> {
    let mut bound = BTreeMap::new();
    let Some(signature) = &contract.signature else {
        return bound;
    };
    if let Some((name, pattern)) = signature.params.first() {
        if name == "self" {
            ty::bind(pattern, receiver, &mut bound);
        }
    }
    bound
}

/// The types a callee's parameters expect, as a call site sees them.
///
/// A method's receiver is the first parameter, so the arguments a *call* writes
/// are the ones after it - `Signature::arguments` already draws that line.
fn expected_arguments(contract: &FnContract) -> Vec<Ty> {
    contract
        .signature
        .as_ref()
        .map(|signature| {
            signature
                .arguments()
                .iter()
                .map(|(_, ty)| ty.clone())
                .collect()
        })
        .unwrap_or_default()
}
