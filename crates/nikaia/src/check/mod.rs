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
use crate::contracts::{send, ty, ty::Ty, FieldContract, FnContract, Ledger};
use crate::parser::Parsed;
use winnow_grammar::Symbol as Ident;

/// The types Part I 2.2 offers, which is what an `as` may name
/// ([ADR-054](../../../docs/specification/adr/adr-054.md) D1).
///
/// Wider than `record_cast`'s `NUMERIC`, and the two answer different questions:
/// that one is which conversions are **checked** at run time
/// ([ADR-043](../../../docs/specification/adr/adr-043.md) D4), this one is which
/// types a program may **write**. `bool`, `char`, `String` and `&str` are types
/// of this language and no conversion between them narrows anything.
const OFFERED: [&str; 8] = ["i32", "i64", "u8", "f64", "bool", "char", "String", "str"];

/// The note every `NK25xx` carries, because it is the reason the code exists.
///
/// ADR-005 §1 Group B and ADR-037 §3: the verdict may not depend on
/// `user_parallelism`, or a library written at one setting would turn out
/// un-compilable at the other - which is the failure the check was decided to
/// prevent rather than a property of it.
const SAME_AT_BOTH: &str = "a value may cross a thread only if it may cross any thread, \
     so the answer is the same at both settings of `user_parallelism` and a library built at \
     one stays usable at the other (Part III, C.3)";

/// Whether a finding stops the build.
///
/// Everything the checker says is an **error** but one, and that one is a
/// migration: ADR-035 gave the interpolated string an `f`, and a string written
/// before it looks exactly like one that meant its braces. A warning is what
/// that deserves - it cannot be an error, because `"{ margin: 0 }"` is correct
/// CSS and rejecting it would break the property this checker is built on (it
/// never refuses a program that is right); and it cannot be silence, because a
/// silent change of meaning is what Part III C.1 calls a compiler bug.
///
/// **It is temporary on purpose.** The variant exists for one release, with the
/// one code that uses it, and goes when the corpus has moved (ADR-035 D5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Severity {
    /// The build stops.
    #[default]
    Error,
    /// The build goes on and the programmer is told.
    Warning,
}

/// One thing the checker is sure about.
#[derive(Debug, Clone)]
pub struct Finding {
    /// Whether it stops the build.
    pub severity: Severity,
    /// The statement it is in.
    pub span: Span,
    /// Its `NK` code, from the catalogue in Part III, C.3. Mostly `NK1xxx`,
    /// which is types; `NK2501`/`NK2502` are a value on the wrong thread,
    /// `NK2605` a written call that can fail in a function that does not say
    /// so, and `NK2701` the same for a loop's step.
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
    /// The **method calls that can fail**, as the byte the statement they
    /// stand in starts at and the method's name (ADR-023 D8).
    ///
    /// The same arrangement as `fallible_loops`, for the same reason and said
    /// once: a method call's callee is only known to something that knows the
    /// receiver's type, the emitter has no types (ADR-028), so the answer is
    /// computed here and handed over. The emitter writes the `?`.
    ///
    /// The statement and the name, because that is the most an `Expr` can be
    /// pointed at: expressions carry no spans and a method name is an interned
    /// symbol shared by every call that writes it. So a pair is recorded only
    /// where **every** method call of that name in that statement can fail -
    /// `a.read()` beside a `b.read()` that cannot is left out of the set
    /// entirely rather than resolved by position. That is the same
    /// fail-quietly this module's other answers keep: the set never claims a
    /// call fails that does not.
    pub fallible_methods: BTreeSet<(usize, String)>,
    /// The method calls that **pause**, keyed the same way and narrowed the same
    /// way ([ADR-055](../../docs/specification/adr/adr-055.md) D2).
    ///
    /// `throws` becomes a `?` and *can pause* becomes an `.await`, both read off
    /// the callee's contract - and for a method the emitter cannot read it,
    /// because `stats.add(5)` names `add` and only a type checker knows what it
    /// goes to (ADR-028). So this is the one-column-over twin of
    /// `fallible_methods`, and it keeps that set's fail-quietly: a name that in
    /// one statement is both a pausing call and a call that is not is in neither
    /// set.
    ///
    /// **Either ledger**, since §6 step 3 made `std`'s own pausing entries
    /// `async fn`. Before it this was narrowed to the program's own, because
    /// awaiting a `std` entry that blocked its thread would have been awaiting a
    /// value rather than a future.
    pub pausing_methods: BTreeSet<(usize, String)>,
    /// The statements where a **plain value stands in a nullable slot** and the
    /// emitter therefore writes the `Some(…)`, by the byte the statement starts
    /// at (Part I 2.3).
    ///
    /// `let mut m: &str? = null` then `m = "World"`: the second line hands a
    /// `&str` to an `Option<&str>`, and the language below needs the
    /// constructor written. The same arrangement as `shared_sites` and for the
    /// same reason - the emitter has no types (ADR-028), and whether the value
    /// beside the `=` is *already* nullable is the whole question.
    ///
    /// **Only where this checker is sure of both sides.** A value whose type is
    /// `Unknown` is left out, because wrapping a value that is already an
    /// `Option<T>` would make an `Option<Option<T>>` - so the set never claims
    /// a wrap that is not needed, which is the fail-closed direction
    /// ([ADR-010](../../../../docs/specification/adr/adr-010.md) D1).
    pub nullable_sites: BTreeSet<usize>,
    /// The `?.` reaches whose field is **itself** nullable, as the byte the
    /// statement starts at and the field's name (Part I 3.5).
    ///
    /// `x?.a` over a plain `a` is `x.map(|v| v.a)`; over an `a` that is already
    /// a `T?` it has to be `and_then`, or the result holds a nullable of a
    /// nullable and `a?.b?.c` comes out wrong. Which of the two is a question
    /// about the declared type, so it is answered here and the emitter writes
    /// the word (ADR-028).
    ///
    /// The statement and the name, for the reason `fallible_methods` gives at
    /// length: an expression carries no span, so a pair is recorded only where
    /// **every** `?.` of that name in that statement flattens. A mixed
    /// statement is left out of the set entirely rather than resolved by
    /// position, which keeps `map` - the answer that cannot make a nested
    /// option out of a plain field - as the one it falls back to.
    pub flattened_reaches: BTreeSet<(usize, String)>,
    /// The **struct-literal fields** where a plain value stands in a nullable
    /// slot, as the byte the statement starts at and the field's name
    /// (Part I 2.3).
    ///
    /// `nullable_sites` covers the three positions a statement *is* - an
    /// annotated `let`, an assignment, a `return` - and a struct literal has
    /// one position per field, so it needs the name too. Same shape as
    /// `shared_sites`, which covers the same construct for the same kind of
    /// reason.
    pub nullable_fields: BTreeSet<(usize, String)>,
    /// The **call arguments** where a plain value stands in a nullable
    /// parameter, as the byte the statement starts at, the callee as the source
    /// wrote it, and the argument's position (Part I 2.3).
    ///
    /// The third position D4's wrap needs a key for, and the narrowest one that
    /// works: an expression carries no span, a statement may hold several calls,
    /// and one call may pass several arguments - so the callee's written name
    /// and the index together say which. **The written name and not the
    /// resolved key**, because the emitter has only what the source says: a
    /// method's key is `Type::method` and a constructor's is `Type::new`, and
    /// neither is what stands at the call.
    pub nullable_args: BTreeSet<(usize, String, usize)>,
    /// The **narrowing conversions**, as the byte the statement they stand in
    /// starts at and the type converted to (ADR-043 D4).
    ///
    /// The fourth answer this module gives the emitter, and for the reason the
    /// other three have: `as i32` narrows or widens depending on what it is
    /// *given*, and the emitter has no types (ADR-028). The emitter writes the
    /// checked conversion.
    ///
    /// Narrow in the same way `fallible_methods` is narrow, and by the same
    /// limit: an `Expr` carries no span, so a pair is recorded only where
    /// **every** conversion to that type in that statement narrows. A
    /// `big as i32` beside a `small as i32` is left out entirely rather than
    /// resolved by position - a checked conversion on a widening one would be a
    /// `rustc` error about a file nobody wrote.
    pub narrowing_casts: BTreeMap<(usize, String), Narrowing>,
    /// The places where a declared `Shared[T]` makes the **first handle** out
    /// of a plain value (Part I 6.2), by the byte the statement starts at and
    /// the name the sharing is written against.
    ///
    /// The third thing this module answers for the emitter, after
    /// `fallible_loops` and `fallible_methods`, and for the reason said once
    /// there: `let db: Shared[Connection] = connect(…)` needs the constructor
    /// written around the value and `let b: Shared[Connection] = db` does not,
    /// and telling those apart means knowing what `connect` and `db` are. The
    /// emitter has no types (ADR-028), so the answer is computed here and handed
    /// over; the emitter writes the `Rc::new` or the `Arc::new`.
    ///
    /// **Two shapes and no others**, which is Part I 6.2's own list: an
    /// annotated `let`, keyed by the name it binds, and a struct literal's field
    /// whose declared type says so, keyed `<struct>.<field>`. A call site is
    /// deliberately absent - a call in which the word does not appear would move
    /// the cleanup point silently, so it is `NK1115` instead
    /// ([ADR-040](../../../docs/specification/adr/adr-040.md) D1).
    pub shared_sites: BTreeSet<(usize, String)>,
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
        caught: false,
        current: None,
        modules: modules.clone(),
        fallible_methods: BTreeSet::new(),
        pausing_methods: BTreeSet::new(),
        settled_methods: BTreeSet::new(),
        moved_into_a_task: Vec::new(),
        read_at: Vec::new(),
        written_at: Vec::new(),
        opaque_methods: BTreeSet::new(),
        widening_casts: BTreeSet::new(),
        checked: Checked::default(),
    };
    checker.collect_types();
    checker.program();
    // ADR-007 D5: the DSL parameters a call forgot, and the ones it invented.
    // A separate walk because it answers a question about a *statement's
    // holes* rather than about a type, and it needs no ledger to answer it.
    checker.checked.findings.extend(crate::dsl::check(parsed));
    // A naked view parameter that is kept past the call (`NK2302`). Also a
    // separate walk, and for the same reason: it asks where a *value* goes
    // rather than what a type is, and it needs no ledger to answer it.
    checker.checked.findings.extend(crate::views::check(parsed));
    checker.checked.findings.sort_by_key(|f| f.span.start);
    // Only the calls that provably fail, and only where the name is not also a
    // call that does not: the emitter writes a `?` for each of these, and a `?`
    // on something that is not a failure is a `rustc` error about a file the
    // author never wrote.
    checker.checked.fallible_methods = checker
        .fallible_methods
        .difference(&checker.opaque_methods)
        .cloned()
        .collect();
    // The same subtraction one column over: an `.await` on something that is not
    // a future is the same kind of message about the same file nobody wrote.
    checker.checked.pausing_methods = checker
        .pausing_methods
        .difference(&checker.settled_methods)
        .cloned()
        .collect();
    // The same subtraction, for the same reason: a checked conversion written on
    // a widening one does not compile.
    checker
        .checked
        .narrowing_casts
        .retain(|at, _| !checker.widening_casts.contains(at));
    checker.checked
}

/// Which kind of checked conversion a narrowing one needs
/// ([ADR-043](../../../docs/specification/adr/adr-043.md) D4).
///
/// Two kinds because Rust gives one of them and not the other, which is measured
/// in that record: `i32::try_from` exists for an integer, and
/// `i32::try_from(f64)` does not - so a conversion out of a floating-point number
/// needs its own test for the range and for "not a number".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Narrowing {
    /// An integer that may not fit the smaller one: `i64` to `i32`.
    Integer,
    /// A floating-point number to an integer, silent three ways in Rust - it
    /// stops at the limit in both directions and turns "not a number" into zero.
    FromFloat,
}

/// Everything the emitter needs that only a type checker can answer: the loops
/// whose step can fail, the method calls that can fail, and the places where a
/// declared `Shared[T]` makes the first handle.
///
/// All three together because all three come out of one pass, and a second pass
/// would cost a whole type check to answer a question the first one already
/// answered.
#[derive(Debug, Clone, Default)]
pub struct Propagation {
    /// [`Checked::fallible_loops`].
    pub loops: BTreeSet<usize>,
    /// [`Checked::fallible_methods`].
    pub methods: BTreeSet<(usize, String)>,
    /// [`Checked::pausing_methods`].
    pub pausing_methods: BTreeSet<(usize, String)>,
    /// [`Checked::shared_sites`].
    ///
    /// Not about a failure travelling, and here anyway: it is the same
    /// arrangement - an answer only a type checker can give, wanted by the
    /// emitter - and it comes out of the same pass, so carrying it here costs
    /// nothing and a second entry point would cost a whole type check.
    pub shared: BTreeSet<(usize, String)>,
    /// [`Checked::narrowing_casts`].
    pub narrowing: BTreeMap<(usize, String), Narrowing>,
    /// [`Checked::nullable_sites`].
    pub nullable: BTreeSet<usize>,
    /// [`Checked::flattened_reaches`].
    pub flattened: BTreeSet<(usize, String)>,
    /// [`Checked::nullable_fields`].
    pub nullable_in_fields: BTreeSet<(usize, String)>,
    /// [`Checked::nullable_args`].
    pub nullable_in_args: BTreeSet<(usize, String, usize)>,
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
    propagation_against(parsed, own).loops
}

/// Both halves of ADR-023 D8's propagation, against contracts the caller
/// already has.
pub fn propagation_against(parsed: &Parsed, own: &Ledger) -> Propagation {
    let Ok(library) = Ledger::parse(crate::contracts::STD) else {
        return Propagation::default();
    };
    let checked = check(parsed, own, &library);
    Propagation {
        loops: checked.fallible_loops,
        methods: checked.fallible_methods,
        pausing_methods: checked.pausing_methods,
        shared: checked.shared_sites,
        narrowing: checked.narrowing_casts,
        nullable: checked.nullable_sites,
        flattened: checked.flattened_reaches,
        nullable_in_fields: checked.nullable_fields,
        nullable_in_args: checked.nullable_args,
    }
}

/// A name in scope: what it is called, the type it holds, and - where this
/// checker could work it out - the constant integer it stands for.
///
/// **Only a `let` ever fills the third**, and only an immutable one whose value
/// folded ([`Checker::constant_of`]). A parameter, a `for` binding, a `match`
/// arm's name and a lambda's argument all name something no compile-time
/// evaluation reaches, so they carry `None` and the fold stops at them - which
/// is the fail-closed direction the refusal needs (ADR-010 D1): a name this
/// checker cannot evaluate makes the whole expression unevaluable, and an
/// unevaluable expression is never refused.
type Local = (String, Ty, Option<i128>);

struct Checker<'a> {
    parsed: &'a Parsed,
    /// This unit's own contracts, inferred from the source being checked.
    own: &'a Ledger,
    /// `std`'s, as `std` ships them.
    library: &'a Ledger,
    /// Every struct declared here, with its fields. A type whose fields are
    /// not known is simply absent, and an absent type is never an error.
    structs: BTreeMap<String, Vec<FieldContract>>,
    /// Every enum declared here, with its variant names.
    enums: BTreeMap<String, BTreeSet<String>>,
    /// Names in scope, innermost frame last.
    scope: Vec<Vec<Local>>,
    /// What the function being walked declared it hands back.
    expected: Option<Ty>,
    /// Whether it declared `throws` - which is what says a failure may leave
    /// it, whether the failing call was written or implicit (ADR-025 D1).
    throwing: bool,
    /// Whether the expression being walked is the guarded half of a `catch`.
    ///
    /// `fs::read_to_string(p) catch { … }` handles the failure where it
    /// happens, so nothing leaves the function and `NK2605` has nothing to
    /// say. It covers the **whole** guarded expression, because that is what
    /// the handler runs for: in `outer(inner())` both calls are caught. The
    /// handler's own body is not - a failure raised there propagates - so this
    /// goes back to what it was before the handler is walked.
    caught: bool,
    /// The function being walked, by the name the ledger records it under.
    ///
    /// `None` inside a grammar action, a `test` or a `bench` - code that
    /// belongs to no function a caller can name, and whose method calls
    /// therefore have nowhere to be recorded.
    current: Option<String>,
    /// The modules this program is made of (Part I, 9.1). Empty for a single
    /// file, where no call crosses a file boundary.
    modules: BTreeSet<String>,
    /// Method calls whose callee's contract says it can fail, and method calls
    /// where that could not be established - both as the pair
    /// [`Checked::fallible_methods`] is keyed by. The difference is the answer;
    /// a name that is both in one statement is in neither.
    fallible_methods: BTreeSet<(usize, String)>,
    opaque_methods: BTreeSet<(usize, String)>,
    /// The same pair of sets for [`Checked::pausing_methods`]: the calls whose
    /// callee pauses, and the ones where it does not or could not be
    /// established. The difference is the answer.
    pausing_methods: BTreeSet<(usize, String)>,
    settled_methods: BTreeSet<(usize, String)>,
    /// **What a task took with it** (`NK2101`): the name, its type, and the byte
    /// the `spawn`'s statement starts at.
    ///
    /// Collected while the function is walked and answered at the end of it,
    /// because the question is about what comes *after* the `spawn` and a
    /// single pass reaches a later statement later. Cleared per function: a
    /// name is a name of one body.
    /// The `spawn` statement's own **end** is what is kept, not its start: the
    /// reads inside the task's body are the move itself and lie inside that
    /// span, so "after the task" means after the statement closes.
    moved_into_a_task: Vec<(String, Ty, usize)>,
    /// Every place a local name was read, by the byte its statement starts at.
    ///
    /// The other half of the same question. Reads and not writes: an assignment
    /// *revives* a name the task took - `message = "other"` after the `spawn` is
    /// a correct program - so those are collected separately.
    read_at: Vec<(String, usize)>,
    written_at: Vec<(String, usize)>,
    /// The conversions that do **not** narrow, so a statement holding one of
    /// those beside a narrowing one to the same type is left alone entirely.
    widening_casts: BTreeSet<(usize, String)>,
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
                    for f in fields {
                        let name = self.parsed.text(f.name).to_string();
                        self.not_self(&name, &f.span, "a field");
                    }
                    let fields: Vec<FieldContract> = fields
                        .iter()
                        .map(|f| FieldContract {
                            name: self.parsed.text(f.name).to_string(),
                            ty: Ty::from_ast(self.parsed, &f.ty).erase(&parameters),
                            public: f.is_public,
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
    fn bindings_of(&self, pattern: &ast::Pattern, out: &mut Vec<Local>) {
        match pattern {
            ast::Pattern::Bind { name, pat } => {
                out.push((self.parsed.text(*name).to_string(), Ty::Unknown, None));
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
            config,
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

        let mut frame: Vec<Local> = Vec::new();
        if let Some(receiver) = receiver {
            let ty = match target {
                Some(target) if receiver.is_ref => Ty::view(target),
                Some(target) => Ty::named(target),
                None => Ty::Unknown,
            };
            frame.push(("self".to_string(), ty, None));
        }
        for arg in args {
            let name = self.parsed.text(arg.name).to_string();
            self.not_self(&name, &arg.span, "a parameter");
            frame.push((
                name,
                Ty::from_ast(self.parsed, &arg.ty).erase(&parameters),
                None,
            ));
        }
        // **An option is a parameter** (Part I 5.1): it stands after the `;`,
        // it is named at the call rather than passed by position, and it has a
        // default - and none of that changes that the body may use it. It was
        // missing from this frame, which nothing noticed while an undeclared
        // name was only refused in statement position: measured on
        // `examples/tally.nika`, whose `f"{lines}{separator}{blank}"` names one.
        for option in config {
            frame.push((
                self.parsed.text(option.name).to_string(),
                Ty::from_ast(self.parsed, &option.ty).erase(&parameters),
                None,
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

        // **`NK2101`, once the whole body has been seen.** The question is what
        // comes *after* a `spawn`, and a single pass reaches a later statement
        // later - so it is asked here rather than at the `spawn`.
        self.a_task_took_what_is_used_again();

        self.expected = outer;
        self.throwing = outer_throwing;
        self.current = outer_current;
    }

    /// Whether a conversion narrows, recorded for the emitter (ADR-043 D4).
    ///
    /// **Only among the numeric types this compiler knows the range of**, for the
    /// reason `constant_fits` gives: a claim about a type neither the
    /// specification nor the ledger describes would be a claim about a surface
    /// that is not there. Anything this does not recognise is recorded as
    /// widening, which leaves the conversion exactly as it is today.
    ///
    /// The list is the other way round - which conversions **always** fit - and
    /// that is the fail-closed direction (ADR-010 D1): a pair nobody thought
    /// about is checked at run time rather than truncated in silence.
    ///
    /// **The machine-width types are not in it, and no longer need to be**
    /// ([ADR-048](../../../../docs/specification/adr/adr-048.md) D1). They were,
    /// because `len` handed one back and what fits on a large machine does not on
    /// a small one. A length is an `i64` now and `usize` has left the surface a
    /// program can write, so a conversion out of one is a conversion out of a type
    /// nobody can hold - and this list is the writable surface, which is the
    /// reason the entry was there and the reason it is not.
    ///
    /// An integer to `f64` is the one entry that loses something and is still
    /// called fitting (D7): digits go at large values without anything
    /// overflowing, there is no sensible point to stop at, and every language
    /// does it this way. That is a written limit and not a check.
    fn record_cast(&mut self, from: &Ty, into: &Ty, span: &Span) {
        let (Ty::Named { name: from, .. }, Ty::Named { name: into, .. }) = (from, into) else {
            return;
        };
        let (from, into_name) = (from.as_str(), into.as_str());
        const NUMERIC: [&str; 4] = ["i32", "i64", "f64", "u8"];
        let always_fits =
            from == into_name || into_name == "f64" || (from, into_name) == ("i32", "i64");
        let checked = NUMERIC.contains(&from) && NUMERIC.contains(&into_name) && !always_fits;

        let at = (span.start, into.clone());
        match (from, checked) {
            (_, false) => {
                self.widening_casts.insert(at);
            }
            // Out of a floating-point number: silent three ways - it stops at the
            // limit in both directions and turns "not a number" into zero - and
            // `try_from` does not exist for it, so `std` carries the test.
            ("f64", true) => {
                self.checked
                    .narrowing_casts
                    .insert(at, Narrowing::FromFloat);
            }
            (_, true) => {
                self.checked.narrowing_casts.insert(at, Narrowing::Integer);
            }
        }
    }

    /// A name nothing declares (`NK1117`).
    ///
    /// **This exists because a misparse is otherwise a different program.** The
    /// grammar is scannerless, so a word this language does not know is read as a
    /// name and a name in statement position is a legal statement. Measured,
    /// before the keyword boundaries went in (`parser::KW_*`) and after:
    ///
    /// ```text
    /// assert c                 ->  assert;  c;
    /// unsafe { println("x") }  ->  unsafe;  { println!("x") }
    /// let n = 1_000            ->  let n = 1;  _000;
    /// ```
    ///
    /// Every one of those reached `rustc`, which refused it about a file nobody
    /// wrote - the Part III C.1 class. The boundary rules stop the *worse* half,
    /// where the word was swallowed into a neighbour; this stops the rest, here,
    /// in this language's words.
    ///
    /// **Only a statement that is exactly one name**, which is the narrowest rule
    /// that covers the class. A name inside a larger expression is refused by the
    /// type checker if it is refused at all, and a name that is the *value* of a
    /// block - `fn f() -> i64 { x }` - is this same shape, where an undeclared `x`
    /// is equally wrong.
    ///
    /// **Four things count as declaring it**, and the list is the fail-safe
    /// direction (ADR-010 D1 applied to a refusal rather than to a permission): a
    /// local or parameter in scope, a function either ledger describes, a type
    /// declared here, and a module of this program. Anything this cannot see is a
    /// name it must not refuse, because refusing a correct program is the one
    /// thing this checker may never do (Part III, C.4).
    fn nothing_declares_it(&mut self, expr: &Expr, span: &Span) {
        let Expr::Variable(name) = expr else {
            return;
        };
        let name = self.parsed.text(*name).to_string();
        let declared = self.lookup(&name).is_some()
            || self.resolve(&name).is_some()
            || self.structs.contains_key(&name)
            || self.enums.contains_key(&name)
            || self.own.types.contains_key(&name)
            || self.modules.contains(&name);
        if declared {
            return;
        }
        self.checked.findings.push(Finding {
            severity: Severity::Error,
            span: span.clone(),
            code: "NK1117",
            message: format!("nothing declares `{name}`"),
            notes: vec![
                "this language has no word it does not know: one it has no rule for is \
                 read as a name, and a name has to be declared somewhere (Part I, 9.1)"
                    .to_string(),
            ],
            help: Some(format!(
                "if `{name}` is meant to be a value, declare it with `let`; if it is meant to \
                 be a keyword, this language has no such keyword - and a number is written in \
                 digits with no separators, so `1_000` is `1` beside the name `_000` \
                 (Part I, 2.2)"
            )),
        });
    }

    /// Part I 3.5: `?.` is for a value that may be absent.
    ///
    /// `"Ada"?.len` reaches through something that cannot be missing, and the
    /// language below has no `map` on it - so this was `rustc`'s refusal about
    /// the generated file (Part III, C.1). **`NK1121`**, and the way out is the
    /// plain `.`, which is what the program meant.
    ///
    /// **Only where the receiver's type is known.** A receiver this checker
    /// could not work out says nothing: refusing there would refuse a correct
    /// program, which is the one thing it may never do (Part III, C.4).
    fn reaches_through_a_plain_value(&mut self, on: &Ty, field: &str, span: &Span) {
        if on.is_unknown() {
            return;
        }
        self.checked.findings.push(Finding {
            severity: Severity::Error,
            span: span.clone(),
            code: "NK1121",
            message: format!("`?.` reaches through a `{on}`, which cannot be absent"),
            notes: vec![
                "`?.` exists for a nullable type - it reaches the field only where there \
                 is something to reach it on, and answers `null` otherwise (Part I, 3.5). \
                 A type that is not `T?` always has a value"
                    .to_string(),
            ],
            help: Some(format!("write `.{field}`")),
        });
    }

    /// Part I 2.2: **`as` names a type this language offers**
    /// ([ADR-054](../../../docs/specification/adr/adr-054.md) D1).
    ///
    /// Nothing ever decided that a program may write `as u128`, and it could:
    /// the target of an `as` went to the language below unread, so a cast named
    /// any Rust type at all and was emitted verbatim. Two things followed, and
    /// the second is the reason this is a refusal rather than a tidy-up. A
    /// program could hold a value of a type Part I 2.2 does not offer and the
    /// page had no word for what it was. And `-3 as usize` is
    /// 18,446,744,073,709,551,613 - a silent reinterpretation, in a language
    /// where a conversion that does not fit aborts
    /// ([ADR-043](../../../docs/specification/adr/adr-043.md) D4). The same
    /// conversion at an index has reported as an access out of bounds since
    /// [ADR-048](../../../docs/specification/adr/adr-048.md) D1; written by hand
    /// it reported nothing.
    ///
    /// **`NK1122`.** A refusal costs nothing today and would break programs
    /// later, which is why it is made now rather than when somebody depends on
    /// the hole.
    fn cast_names_a_foreign_type(&mut self, into: &str, span: &Span) {
        self.checked.findings.push(Finding {
            severity: Severity::Error,
            span: span.clone(),
            code: "NK1122",
            message: format!("`{into}` is not a type this language offers, and `as` names one"),
            notes: vec![
                "The types are `i32`, `i64`, `u8`, `f64`, `bool`, `char`, `String` and \
                 `&str` (Part I, 2.2). A conversion into anything else went to \
                 the language below unread, so a value could have a type this \
                 page has no word for"
                    .to_string(),
            ],
            help: Some(match into {
                "usize" | "isize" | "u16" | "u32" | "u64" | "u128" | "i8" | "i16" | "i128" => {
                    "a length and an index are `i64` and the compiler writes the \
                     machine's conversion itself (ADR-048 D1), so the cast is \
                     not needed - remove it. Where a narrower type is meant, \
                     `truncating_i32` says so by name"
                        .to_string()
                }
                _ => "write one of the types above, or none".to_string(),
            }),
        });
    }

    /// Part I 2.3: record where the emitter has to write the `Some(…)`.
    ///
    /// A type is non-nullable unless it says otherwise, so a plain `T` standing
    /// where a `T?` is wanted is the one widening this language has - `let mut
    /// m: &str? = null` and then `m = "World"`. `Ty::fits` allows it; the
    /// language below needs the constructor written, and this is where the
    /// emitter is told.
    ///
    /// **The value has to be known not to be nullable already**, because
    /// wrapping one that is would make an `Option<Option<T>>`. Two ways it can
    /// be known, and a type is only the first:
    ///
    /// * its type says so — anything this checker worked out that is not a
    ///   `T?`; or
    /// * **it is a literal**, which no literal ever is. That second one is not
    ///   a convenience: a number has no type of its own on purpose (Part I 2.4,
    ///   so that `add(3)` is right wherever the parameter is numeric), so
    ///   `return 42` against a declared `i64?` answers `Unknown` and the type
    ///   alone would leave the commonest case in the section unwrapped.
    ///
    /// Everything else is left alone rather than guessed at, so the set never
    /// claims a wrap that is not needed (ADR-010 D1). `null` is excluded by the
    /// first rule, being a `T?` itself.
    fn wraps_into_nullable(&mut self, found: &Ty, want: &Ty, value: &Expr, span: &Span) {
        let Ty::Nullable(_) = want else {
            return;
        };
        if matches!(found, Ty::Nullable(_)) {
            return;
        }
        let known = !found.is_unknown() || is_literal(value);
        if !known {
            return;
        }
        self.checked.nullable_sites.insert(span.start);
    }

    /// Part I 2.2: a constant that does not fit the type it is given is a
    /// compile error rather than something the program finds out about at run
    /// time.
    ///
    /// **`NK1116`, and it exists to take a message back rather than to prevent an
    /// abort.** This was already refused where it was written - but by `rustc`, in
    /// Rust's words, down to the lint name `overflowing_literals` and the advice
    /// to use a `u32`, about a file nobody wrote (Part III, C.1). An out-of-range
    /// constant never reached run time and never will; what changes is who says so.
    ///
    /// **A literal alone is asked only where a type stands beside it**, because a
    /// literal has no type of its own on purpose: `Expr::LitInt` answers
    /// `Unknown`, so `add(3)` is right wherever the parameter is numeric, and
    /// `let m = 3000000000` is a correct program where the next line passes `m`
    /// to an `i64` (Part I 2.4, and `docs/open-work.md`'s out-of-range literal).
    /// So the places are an
    /// annotated `let`, a `return` against a declared result, and an argument
    /// whose parameter says what it takes.
    ///
    /// **A sum reaches further, and that is ADR-043 §3's gap closing.** Where an
    /// operand's *declaration* pins the type - `let a: i32 = …`, then `a + 1` -
    /// the arithmetic has a type whatever stands beside it, so the bare `let` is
    /// asked too. That is the one case `rustc` refused about the generated file
    /// with *"attempt to compute `i32::MAX + 1_i32`"*.
    ///
    /// **Only the two integer types Part I 2.2 offers.** `u32` and the rest are
    /// accepted by the compiler below and not offered here, so a range for them
    /// would be a claim about a surface that is not promised.
    fn constant_fits(&mut self, value: &Expr, want: Option<&Ty>, span: &Span) {
        let Some(folded) = self.constant_of(value) else {
            return;
        };
        // The type it answers to: what stands beside it if that is an integer
        // this language offers, and otherwise what an operand's declaration
        // pinned. Neither, and there is nothing to measure against - which is
        // the literal-alone case that must stay accepted (C.4).
        let named = want.and_then(integer_named);
        let Some(ty) = named.or(folded.pinned) else {
            return;
        };
        let fits = match ty.as_str() {
            "i32" => i32::try_from(folded.value).is_ok(),
            "i64" => i64::try_from(folded.value).is_ok(),
            _ => return,
        };
        if fits {
            return;
        }
        let (low, high) = match ty.as_str() {
            "i32" => (i32::MIN as i128, i32::MAX as i128),
            _ => (i64::MIN as i128, i64::MAX as i128),
        };
        // A bare literal says its own digits; anything folded says what it came
        // to, because the expression is on the line the caret is under and the
        // number is the part the reader cannot see.
        let message = match value {
            Expr::LitInt(text) => format!("`{text}` does not fit in an `{ty}`"),
            _ => format!(
                "this comes to {}, which does not fit in an `{ty}`",
                folded.value
            ),
        };
        self.checked.findings.push(Finding {
            severity: Severity::Error,
            span: span.clone(),
            code: "NK1116",
            message,
            notes: vec![format!("an `{ty}` holds {low} to {high} (Part I, 2.2)")],
            help: Some(match ty.as_str() {
                "i32" => "write `i64` where the number needs it".to_string(),
                _ => "an `i64` is the widest number this language has, so this \
                      computation has to be arranged to stay inside it"
                    .to_string(),
            }),
        });
    }

    /// What a constant integer expression comes to, and the type an operand's
    /// declaration pinned.
    ///
    /// **Folded in an `i128`** so that a sum which cannot fit an `i64` is a
    /// number this checker can name rather than one it wrapped - the fold must
    /// not do quietly what it exists to refuse.
    ///
    /// **Every step is `checked_`, and `None` means nothing is claimed.** A name
    /// this checker cannot evaluate, an operator it does not fold, a division by
    /// a constant zero, a fold that leaves the `i128` - each one stops the whole
    /// expression, and an expression that does not fold is never refused. That
    /// is the polarity the checker is held to (Part III, C.4): it may fail to
    /// refuse a program `rustc` will, and it may never refuse one that is right.
    fn constant_of(&self, expr: &Expr) -> Option<Constant> {
        match expr {
            Expr::LitInt(value) => Some(Constant {
                value: *value as i128,
                pinned: None,
            }),
            Expr::Variable(name) => {
                let (ty, constant) = self.local(self.parsed.text(*name))?;
                Some(Constant {
                    value: constant?,
                    pinned: integer_named(&ty),
                })
            }
            Expr::Unary {
                op: UnaryOp::Neg,
                expr,
            } => {
                let inner = self.constant_of(expr)?;
                Some(Constant {
                    value: inner.value.checked_neg()?,
                    pinned: inner.pinned,
                })
            }
            Expr::Binary { op, lhs, rhs } => {
                let lhs = self.constant_of(lhs)?;
                let rhs = self.constant_of(rhs)?;
                // Two operands that pin different types are a mismatch `expect`
                // reports on its own; folding them would be arithmetic in a type
                // neither of them has.
                let pinned = match (&lhs.pinned, &rhs.pinned) {
                    (Some(a), Some(b)) if a != b => return None,
                    (Some(a), _) => Some(a.clone()),
                    (_, pinned) => pinned.clone(),
                };
                let value = match op {
                    BinaryOp::Add => lhs.value.checked_add(rhs.value)?,
                    BinaryOp::Sub => lhs.value.checked_sub(rhs.value)?,
                    BinaryOp::Mul => lhs.value.checked_mul(rhs.value)?,
                    BinaryOp::Div => lhs.value.checked_div(rhs.value)?,
                    BinaryOp::Rem => lhs.value.checked_rem(rhs.value)?,
                    _ => return None,
                };
                Some(Constant { value, pinned })
            }
            _ => None,
        }
    }

    /// `self` is a reserved word, and this is the one position the grammar
    /// cannot refuse it in (`open-decisions.md` §7).
    ///
    /// Every other reserved word is excluded from `NAME` itself, so `let fn = 3`
    /// does not parse. **`self` cannot be**, because it is the one keyword that
    /// *is* a name: `self.min` refers to it, and `NAME` is the rule both for
    /// declaring a name and for referring to one. So the refusal is here, where
    /// the declaration is - and it can say more than a parse error would.
    ///
    /// **Every position that declares a name**: a `let`, a `for` binding, a
    /// lambda's argument, a parameter and a struct field.
    ///
    /// The last two took a span of their own on `FnArg` and `FieldDef` to
    /// reach. Without one the nearest span each walk had was the body's first
    /// statement - a different line - and a caret on the wrong line is worse
    /// than no message, which is why they waited rather than being
    /// approximated.
    ///
    /// **`NK1119`**, and it is the same C.1 case as the rest of the list: `let
    /// self = 3` lowered to `let self = 3;` and `rustc` refused the generated
    /// file with *"expected identifier, found keyword `self`"*.
    fn not_self(&mut self, name: &str, span: &Span, what: &str) {
        if name != "self" {
            return;
        }
        self.checked.findings.push(Finding {
            severity: Severity::Error,
            span: span.clone(),
            code: "NK1119",
            message: format!("`self` is a reserved word, so {what} may not be called that"),
            notes: vec![
                "`self` already names one thing - the value a method was called on - and it \
                 is the only reserved word that is a name at all, which is why the rest of \
                 the list is refused by the grammar and this one is refused here \
                 (Part I, 2.1)"
                    .to_string(),
            ],
            help: Some("pick another name; `it`, `this` and `me` are all free".to_string()),
        });
    }

    /// ADR-043 D5.5: a division whose divisor is a constant zero is refused
    /// here, in this language's words.
    ///
    /// It was already refused where it was written, and by the same mechanism as
    /// the sum `constant_fits` takes back: `rustc`'s `unconditional_panic`, with
    /// *"attempt to divide `1_i32` by zero"* about a file nobody wrote
    /// (Part III, C.1).
    ///
    /// **`NK1118`, and the polarity is the fold's.** A divisor that does not
    /// fold to a constant says nothing: a division by something this checker
    /// cannot evaluate is the ordinary case, and it aborts at run time with the
    /// Nikaia line the table names (ADR-044). Only a zero it can *prove* is
    /// refused.
    fn divisor_is_not_zero(&mut self, op: BinaryOp, rhs: &Expr, span: &Span) {
        if !matches!(op, BinaryOp::Div | BinaryOp::Rem) {
            return;
        }
        let Some(divisor) = self.constant_of(rhs) else {
            return;
        };
        if divisor.value != 0 {
            return;
        }
        let what = match op {
            BinaryOp::Div => "divides",
            _ => "takes the remainder",
        };
        self.checked.findings.push(Finding {
            severity: Severity::Error,
            span: span.clone(),
            code: "NK1118",
            message: format!("this {what} by zero"),
            notes: vec![
                "a division by zero is unrecoverable (Part III, A.2) - and this one is \
                 decidable where it is written, so it is said here rather than left to \
                 abort"
                    .to_string(),
            ],
            help: Some(
                "if the divisor is meant to be able to be zero, it cannot be a constant: \
                 test it before dividing"
                    .to_string(),
            ),
        });
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
                name,
                mutable,
                ty,
                value,
            } => {
                let found = self.expr(value, span);
                let name = self.parsed.text(*name).to_string();
                self.not_self(&name, span, "a `let`");
                let bound = match ty {
                    Some(ty) => {
                        let want = Ty::from_ast(self.parsed, ty);
                        // Part I 6.2: an annotated `let` is where the first
                        // handle on a shared value is made. The annotation *is*
                        // the constructor, so a plain value standing here is not
                        // a mistake - it is the one line that makes one, and the
                        // emitter is told where to write it.
                        self.constant_fits(value, Some(&want), span);
                        self.wraps_into_nullable(&found, &want, value, span);
                        match becomes_shared(&found, &want) {
                            true => {
                                self.checked.shared_sites.insert((span.start, name.clone()));
                            }
                            false => {
                                self.expect(&found, &want, span.clone(), "let", |found, want| {
                                    format!("this is `{found}`, and the `let` says `{want}`")
                                });
                            }
                        }
                        want
                    }
                    None => {
                        // **No annotation, and a type all the same** where an
                        // operand's declaration pinned one: `let a: i32 = …`
                        // then `let b = a + 1` is arithmetic in an `i32`, and
                        // that is the sum ADR-043 §3 left to `rustc`.
                        self.constant_fits(value, None, span);
                        found
                    }
                };
                // **Only an immutable `let` carries its value forward.** A
                // `mut` one may be given another before the name is read again,
                // and this checker does not follow assignments - so the value
                // it started with would be a claim about a program that no
                // longer holds (Part III, C.4).
                let constant = match mutable {
                    false => self.constant_of(value).map(|c| c.value),
                    true => None,
                };
                self.bind_with(name, bound, constant);
                Ty::Tuple(Vec::new())
            }

            Stmt::Assign {
                target, op, value, ..
            } => {
                // `NK2101`: an assignment **revives** a name a task took with
                // it - `message = "other"` after the `spawn` is a correct
                // program - so the target is recorded before it is walked, and
                // walking it records a read this must not be confused with.
                if let Expr::Variable(name) = target {
                    self.written_at
                        .push((self.parsed.text(*name).to_string(), span.start));
                }
                let into = self.expr(target, span);
                let found = self.expr(value, span);
                // Only a plain assignment: `n += 1` is whatever the operator
                // makes of the two, and Stage 0 does not model operators.
                if op.is_none() {
                    self.wraps_into_nullable(&found, &into, value, span);
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
                let frame: Vec<Local> = bindings
                    .iter()
                    .map(|b| (self.parsed.text(*b).to_string(), element.clone(), None))
                    .collect();
                for (name, _, _) in &frame {
                    self.not_self(&name.clone(), span, "a `for` binding");
                }
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
                    if let Some(value) = value {
                        self.constant_fits(value, Some(&expected), span);
                    }
                    if let Some(value) = value {
                        self.wraps_into_nullable(&found, &expected, value, span);
                    }
                    self.expect(&found, &expected, span.clone(), "returns", |found, want| {
                        format!("this returns `{found}`, and the function declares `{want}`")
                    });
                }
                Ty::Unknown
            }

            // **One site, not two.** A statement that is one name is an
            // expression statement, so the walk below reaches it - and the
            // refusal now lives where every name is read rather than only where
            // one stands alone.
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
            // **The type comes from the syntax** (ADR-035 D3). `"…"` is a view
            // of static text and `f"…"` is a `format!`, which is a `String` -
            // and which one a literal is can be read off its first character
            // rather than worked out from whether somebody happened to type a
            // brace somewhere in it.
            Expr::LitStr(text) => {
                self.unmarked_hole(text, span);
                Ty::view("str")
            }
            // **A hole is checked like anything else** (ADR-032 D3). It is
            // Nikaia source written inside a literal, and until that walk
            // existed it was source no analysis could see - the same mistake
            // was caught outside a hole and silently passed inside one.
            Expr::LitInterpolated(_) => {
                self.holes(expr, span);
                Ty::named("String")
            }
            Expr::LitChar(_) => Ty::named("char"),
            Expr::LitBool(_) => Ty::named("bool"),
            // **A nullable of it-does-not-say.** `null` names the absence of a
            // value without naming what value, so the inside is `Unknown` and
            // anything nullable fits it - which is what makes `let mut m: &str?
            // = null` right and what keeps `let m = null` from being refused
            // here: `let mut m = null` then `m = "hi"` is a correct program
            // (Part III, C.4), and it is `rustc` that asks for an annotation
            // where nothing ever says.
            Expr::LitNull => Ty::Nullable(Box::new(Ty::Unknown)),

            Expr::Variable(name) => {
                let name = self.parsed.text(*name);
                // `NK2101`: where this name is read, for the question a `spawn`
                // asks about what comes after it.
                self.read_at.push((name.to_string(), span.start));
                match self.lookup(name) {
                    Some(ty) => ty,
                    None => {
                        // **The specific message wins.** A free `a`, `b` or `c`
                        // is the mistake a reader of the old specification
                        // makes, and `withdrawn_automatic_name` says what
                        // happened to the form rather than only that the name
                        // is unknown (ADR-049 §5). Where it has spoken, the
                        // general refusal would be a second finding about one
                        // mistake.
                        if !self.withdrawn_automatic_name(name, span) {
                            self.nothing_declares_it(expr, span);
                        }
                        Ty::Unknown
                    }
                }
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

            // A `seq` block is a block for every purpose but one: what it says
            // is about the *order* its statements run in (ADR-033 D7), not
            // about what any of them mean or what it hands back.
            Expr::Block(block) | Expr::Seq(block) => self.block(block),

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
                config,
            } => {
                // ADR-007 D5: a DSL's deferred parameters stand here. They are
                // expressions like any other, so they are walked - what checks
                // that they are the *right* names is `dsl::check`, which knows
                // which statement the receiver came from.
                config.iter().for_each(|a| {
                    self.expr(&a.value, span);
                });
                let on = self.expr(receiver, span);
                let Ty::Named { name, .. } = &on else {
                    // The receiver's type is not known, so neither is what this
                    // calls. Recorded, because "I could not find out" is an
                    // answer somebody downstream has to act on.
                    args.iter().for_each(|a| {
                        self.expr(a, span);
                    });
                    self.reached_method(None);
                    self.method_propagates(*method, false, span);
                    self.method_pauses(*method, false, span);
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
                    self.method_pauses(*method, false, span);
                    self.method_propagates(*method, false, span);
                    return Ty::Unknown;
                };
                self.reached_method(Some(&key));
                // ADR-023 D8: the failure leaves at the call, and the emitter
                // is what writes that. Recorded whether or not the function
                // around it declares `throws` - where it does not, `NK2605`
                // below refuses the program and nothing is emitted at all.
                self.method_propagates(*method, !contract.throws.is_empty(), span);
                // ADR-055 D2, the method half. Either ledger since §6 step 3
                // made `std`'s own pausing entries `async fn`: before it, a
                // `std` entry blocked its thread and awaiting one would have
                // been awaiting a value rather than a future.
                self.method_pauses(*method, !contract.sync.is_sync(), span);
                // A method call is a written call, so the rule reaches it too
                // (`NK2605`) - and here the receiver's type was known and a
                // ledger described the method, which is the only case this
                // compiler can answer at all.
                self.may_fail_here(&key, contract, span);

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
                let result = self.arguments(
                    &key,
                    self.parsed.text(*method),
                    contract,
                    args,
                    &found,
                    &[],
                    span,
                );
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
                match fields.iter().find(|f| f.name == field) {
                    Some(found) => {
                        let (ty, declared) = (ty.clone(), found.clone());
                        self.field_is_reachable(&ty, &declared, span);
                        declared.ty
                    }
                    None => {
                        let ty = ty.clone();
                        self.no_such_field(&ty, &field, &fields, span);
                        Ty::Unknown
                    }
                }
            }

            // Part I 3.5: `x?.field`. The receiver must be a `T?` and the
            // result is a `U?` - flattened, because a field that is *itself*
            // nullable would otherwise give a nullable of a nullable.
            Expr::SafeField { base, name } => {
                let on = self.expr(base, span);
                let field = self.parsed.text(*name).to_string();
                let Ty::Nullable(inner) = &on else {
                    self.reaches_through_a_plain_value(&on, &field, span);
                    return Ty::Unknown;
                };
                let Ty::Named { name: ty, .. } = inner.as_ref() else {
                    return Ty::Unknown;
                };
                let Some(fields) = self.fields_of(ty) else {
                    return Ty::Unknown;
                };
                match fields.iter().find(|f| f.name == field) {
                    Some(found) => {
                        let (ty, declared) = (ty.clone(), found.clone());
                        self.field_is_reachable(&ty, &declared, span);
                        // **The `and_then` case is the field that is already a
                        // `T?`**, and the emitter is told which by name: `map`
                        // over one would make an `Option<Option<T>>`, and that
                        // is a question about the declared type, which this
                        // module answers and the emitter cannot (ADR-028).
                        match &declared.ty {
                            Ty::Nullable(_) => {
                                self.checked.flattened_reaches.insert((span.start, field));
                                declared.ty
                            }
                            plain => Ty::Nullable(Box::new(plain.clone())),
                        }
                    }
                    None => {
                        let ty = ty.clone();
                        self.no_such_field(&ty, &field, &fields, span);
                        Ty::Unknown
                    }
                }
            }

            Expr::StructLit { name, fields } => {
                // `unaliased`, the same as a type: `h::Request(path: …)` builds
                // `http::Request` (ADR-046 D3).
                let name = self.parsed.unaliased(self.parsed.text(*name));
                let declared = self.fields_of(&name);
                for init in fields {
                    let field = self.parsed.text(init.name).to_string();
                    // `Reading { name, temp }` is shorthand for `name: name`.
                    let found = match &init.value {
                        Some(value) => self.expr(value, span),
                        None => self.lookup(&field).unwrap_or(Ty::Unknown),
                    };
                    let Some(declared) = &declared else { continue };
                    match declared.iter().find(|f| f.name == field) {
                        Some(found_field) => {
                            let want = found_field.ty.clone();
                            let owner = name.clone();
                            self.field_is_reachable(&name, found_field, span);
                            // The second of Part I 6.2's two places: a field
                            // whose declared type says the value is shared. The
                            // shared type stands in the same line as the value,
                            // which is the whole of what the rule asks.
                            if becomes_shared(&found, &want) {
                                self.checked
                                    .shared_sites
                                    .insert((span.start, format!("{owner}.{field}")));
                                continue;
                            }
                            // Part I 2.3's fourth position: a plain value in a
                            // field the struct declares nullable. The same rule
                            // as the other three (`wraps_into_nullable`), keyed
                            // by the field as well, because a struct literal
                            // has one of these per field and a statement only
                            // one span.
                            let value = init.value.as_ref();
                            let is_literal = value.is_some_and(is_literal);
                            if matches!(want, Ty::Nullable(_))
                                && !matches!(found, Ty::Nullable(_))
                                && (!found.is_unknown() || is_literal)
                            {
                                self.checked
                                    .nullable_fields
                                    .insert((span.start, field.clone()));
                                continue;
                            }
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

            // A lambda's arguments are the ones it names (ADR-049). There is
            // nothing to read off the body any more, so a `fn { … }` pushes an
            // empty frame - and a body reaching for `a` is then a body naming
            // something nothing declares, which `NK1117` refuses.
            Expr::Closure { params, body } => {
                let frame: Vec<Local> = params
                    .iter()
                    .map(|p| (self.parsed.text(*p).to_string(), Ty::Unknown, None))
                    .collect();
                for (name, _, _) in &frame {
                    self.not_self(&name.clone(), span, "a lambda's argument");
                }
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
                self.divisor_is_not_zero(*op, rhs, span);
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
                let from = self.expr(expr, span);
                let into = Ty::from_ast(self.parsed, ty);
                if let Ty::Named { name, .. } = &into {
                    if !OFFERED.contains(&name.as_str()) {
                        self.cast_names_a_foreign_type(name, span);
                    }
                }
                self.record_cast(&from, &into, span);
                into
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
                // Kap 7.1: the handler is what handles the failure, so the
                // guarded expression is where a fallible call needs no
                // `throws` on the function around it (`NK2605`). The handler
                // itself is ordinary code again - a failure raised inside one
                // leaves the function like any other.
                let outer = std::mem::replace(&mut self.caught, true);
                self.expr(expr, span);
                self.caught = outer;
                self.scope
                    .push(vec![("error".to_string(), Ty::Unknown, None)]);
                self.block(handler);
                self.scope.pop();
                Ty::Unknown
            }

            Expr::Spawn { body, .. } => {
                // **Before** the body is walked, so that the task's own `let`s
                // are not yet in scope: a name bound inside the task is the
                // task's own and crosses nothing.
                self.crosses_into_a_task(body, span);

                // **A `spawn` hands back a `TaskHandle` of what the task's body
                // comes to** (Part I 8.2, ADR-055 D5), and that is why the body
                // is walked here rather than through the generic closure arm:
                // what a lambda hands back is not written down anywhere
                // (ADR-029 D1), but a task's body is a *block*, and a block's
                // value is a thing this checker already knows. So `spawn fn {
                // work(21) }` is a `TaskHandle[i64]` and `handle.join()` is an
                // `i64`, with nothing inferred that a signature did not say.
                let Expr::Closure { params, body } = body.as_ref() else {
                    self.expr(body, span);
                    return Ty::Unknown;
                };
                // A task is handed nothing, so a parameter has nothing to be
                // bound from. Refused rather than dropped: `spawn fn (x) { … }`
                // reads as though `x` arrives from somewhere.
                self.a_task_takes_no_arguments(params, span);
                // **What the task takes with it** (`NK2101`), read off the body
                // before it is walked: a name the body binds for itself is the
                // task's own, and the frame pushed below is what keeps the two
                // apart.
                self.a_task_takes_these(
                    &Expr::Closure {
                        params: params.clone(),
                        body: body.clone(),
                    },
                    span,
                );
                self.scope.push(Vec::new());
                let value = self.block(body);
                self.scope.pop();
                Ty::Named {
                    name: "TaskHandle".to_string(),
                    args: vec![value],
                    view: false,
                }
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

    /// **`a`, `b` and `c` were a lambda's arguments, and are not** (`NK1117`).
    ///
    /// [ADR-049](../../../../docs/specification/adr/adr-049.md) D1 withdrew the
    /// three automatic names. Without this, `xs.map fn { a.id }` - the idiom that
    /// existed until that record - lowers to `|| { a.id }` and `rustc` says
    /// *cannot find value `a`* about a file nobody wrote, which is the one thing
    /// Part III C.1 forbids. A form that was in the specification deserves a
    /// sentence, which is the same ground [ADR-022](../../../../docs/specification/adr/adr-022.md)
    /// stands on for `fn: …`.
    ///
    /// **This is not the mechanism that was removed.** That one read a lambda's
    /// *arity* off which of the three its body mentioned, and every lambda in the
    /// language depended on it. This reads nothing: it is a message at the place a
    /// name was used and nothing declares it, and a lambda that declares `a` for
    /// itself never reaches it.
    ///
    /// Keyed on the three names, and **only** on them. A general "this expression
    /// names something nothing declares" is a much wider claim than the statement
    /// rule above makes, and this checker does not make it yet
    /// (`docs/open-work.md`).
    /// Hands back whether it said anything, so the general refusal can stand
    /// aside for it.
    fn withdrawn_automatic_name(&mut self, name: &str, span: &Span) -> bool {
        if !matches!(name, "a" | "b" | "c") {
            return false;
        }
        self.checked.findings.push(Finding {
            severity: Severity::Error,
            span: span.clone(),
            code: "NK1117",
            message: format!("nothing declares `{name}`"),
            notes: vec![
                "`a`, `b` and `c` used to be a lambda's arguments without being written \
                 down, and that form is withdrawn (Part I, 5.3)"
                    .to_string(),
            ],
            help: Some(format!(
                "name the argument: `fn ({name}) {{ … }}`, or give it a name that says what \
                 it is - `fn (user) {{ user.id }}`"
            )),
        });
        true
    }

    /// **The one thing this checker warns about rather than refusing** - a
    /// plain string that was written when every string was a template
    /// (ADR-035 D5).
    ///
    /// Two shapes, because the change has two halves. `"hello {name}"` used to
    /// interpolate and is now text, and `"{{}}"` used to *be* `{}` and is now
    /// four characters. Both change what a program prints without changing
    /// whether it compiles, and Part III C.1 calls a silent change of meaning a
    /// bug in this compiler.
    ///
    /// **Why a warning and not an error.** `"{ margin: 0 }"` is correct CSS and
    /// `"\\d{3}"` a correct regular expression; refusing either would break the
    /// property the whole checker rests on - that it never rejects a program
    /// that is right. So the test is deliberately narrow: the braces have to
    /// hold something that **parses as an expression and resolves to something
    /// that is actually here** - a variable in scope, or a function a ledger
    /// knows. A name nobody declared is text, and text says nothing.
    fn unmarked_hole(&mut self, text: &str, span: &Span) {
        if text.contains("{{") || text.contains("}}") {
            self.warn_migration(
                span,
                "a doubled brace in a plain string is now two braces".to_string(),
                format!(
                    "`{{{{` escaped a brace while every string was a template. A plain string needs no escape: write `\"{}\"`",
                    text.replace("{{", "{").replace("}}", "}")
                ),
            );
            return;
        }

        for hole in brace_groups(text) {
            let Ok(parsed) = crate::parser::parse_expression(&self.parsed.interner, &hole) else {
                continue;
            };
            if !self.names_something_here(&parsed) {
                continue;
            }
            self.warn_migration(
                span,
                format!("`{{{hole}}}` here is text, and used to be a hole"),
                format!("write `f\"{text}\"` if the value was meant to appear (Part I, 2.5)"),
            );
            return;
        }
    }

    fn warn_migration(&mut self, span: &Span, message: String, help: String) {
        self.checked.findings.push(Finding {
            severity: Severity::Warning,
            span: span.clone(),
            code: "NK1111",
            message,
            notes: vec![
                "every string interpolated before ADR-035; now only `f\"…\"` does".to_string(),
            ],
            help: Some(help),
        });
    }

    /// Whether an expression names anything that exists here - a variable in
    /// scope or a function some ledger has. This is what keeps the warning off
    /// a stylesheet: `margin` is nobody's variable.
    /// The name an expression writes, where it writes one.
    ///
    /// For `NK1115`'s help line, which has to say `let db: Shared[…] = …` with
    /// the caller's own name in it. A value that is not a name has no such line
    /// to be pointed at, and the message says so instead of inventing one.
    fn names_of(&self, expr: &Expr) -> Option<String> {
        match expr {
            Expr::Variable(name) => Some(self.parsed.text(*name).to_string()),
            _ => None,
        }
    }

    fn names_something_here(&self, expr: &Expr) -> bool {
        match expr {
            Expr::Variable(name) => {
                let name = self.parsed.text(*name);
                self.lookup(name).is_some() || self.resolve(name).is_some()
            }
            Expr::Field { base, .. } => self.names_something_here(base),
            Expr::MethodCall { receiver, .. } => self.names_something_here(receiver),
            Expr::Call { func, args, .. } => {
                self.names_something_here(func) || args.iter().any(|a| self.names_something_here(a))
            }
            Expr::Path(segments) => {
                let name = segments
                    .iter()
                    .map(|s| self.parsed.text(*s))
                    .collect::<Vec<_>>()
                    .join("::");
                self.resolve(&name).is_some()
            }
            Expr::Binary { lhs, rhs, .. } => {
                self.names_something_here(lhs) || self.names_something_here(rhs)
            }
            Expr::Unary { expr, .. } => self.names_something_here(expr),
            _ => false,
        }
    }

    /// Every expression a literal hides, walked where it stands.
    ///
    /// The result is discarded: what a hole evaluates to is the emitter's
    /// business - `Render` decides for a template and `Display` for a string -
    /// and what the checker is here for is everything *inside* it.
    fn holes(&mut self, literal: &Expr, span: &Span) {
        // **A template's `<for>` declares a name**, and the holes inside it use
        // it: `<for r in :rows>{r.name}</for>` (ADR-017). That is the fifth
        // thing that counts as declaring one, after the four
        // `nothing_declares_it` lists - and the one a reader of a hole cannot
        // see, because the binding is in the markup around it.
        //
        // Measured: widening `NK1117` to a name in an expression refused
        // `examples/escaping.nika` for its `{r.shade}` until this frame existed.
        for (hole, bound) in crate::emit::literal_expressions_bound(self.parsed, literal) {
            let frame: Vec<Local> = bound
                .into_iter()
                // What the element's type is, is a question about the
                // collection, and this walk does not ask it: what it needs is
                // that the name is *declared*.
                .map(|name| (name, Ty::Unknown, None))
                .collect();
            self.scope.push(frame);
            self.expr(&hole, span);
            self.scope.pop();
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
            // `unaliased`: `h::serve()` is `http::serve()` where the file wrote
            // `use http as h` (ADR-046 D3), and the ledger knows only the
            // package's own name.
            Expr::Path(segments) => self.parsed.unaliased(
                &segments
                    .iter()
                    .map(|s| self.parsed.text(*s))
                    .collect::<Vec<_>>()
                    .join("::"),
            ),
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
            // A call nothing describes is a call this compiler cannot see the
            // end of, and a thread of its own is among the things it may do
            // (ADR-038 D7). What it is handed is therefore handed across.
            self.crosses_into_an_unseen_call(&name, args, &found, config, &passed, span);
            return Ty::Unknown;
        };
        self.reachable(&name, contract, span);
        self.may_fail_here(&key, contract, span);
        // `Stats(first)` is the anonymous constructor of Kap 4.2, which the
        // lowering names `Stats::new` - and which hands back the type it is on,
        // whatever its declaration says about `Self`.
        let constructed = key
            .strip_suffix("::new")
            .filter(|_| !name.ends_with("::new"))
            .map(Ty::named);
        let result = self.arguments(&key, &name, contract, args, &found, &passed, span);
        constructed.unwrap_or(result)
    }

    /// The count and the types of what a call passes, against what it takes.
    ///
    /// `given` is the argument expressions, for the one diagnostic that has to
    /// name the **value** rather than its type: `NK1115` points at the line where
    /// the sharing belongs, and that line starts with the name the caller wrote.
    #[allow(clippy::too_many_arguments)]
    fn arguments(
        &mut self,
        key: &str,
        // The callee as the source writes it, which is the part the emitter can
        // match a recorded argument against (`nullable_args`).
        written: &str,
        contract: &FnContract,
        given: &[Expr],
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
                severity: Severity::Error,
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

        for (at, ((name, want), found)) in wanted.iter().zip(found).enumerate() {
            // A literal that cannot fit the parameter it is given, asked here
            // rather than where the argument was walked: a free call walks its
            // arguments before it resolves the callee, so the type to measure
            // against is not known yet at that point (ADR-029's ordering is what
            // gives a *method* call the answer earlier).
            if let Some(given) = given.get(at) {
                self.constant_fits(given, Some(want), span);
            }
            // Part I 2.3's third position for the wrap: a plain value in a
            // parameter the callee declares nullable. Recorded before `fits`
            // is consulted, because this *is* the fit - `Ty::fits` allows it,
            // and what is left is telling the emitter to write the
            // constructor.
            if matches!(want, Ty::Nullable(_)) && !matches!(found, Ty::Nullable(_)) {
                let is_literal = given.get(at).is_some_and(is_literal);
                if !found.is_unknown() || is_literal {
                    self.checked
                        .nullable_args
                        .insert((span.start, written.to_string(), at));
                    continue;
                }
            }
            if self.fits_through_deref(found, want) {
                continue;
            }
            // Part I 6.2: a plain value may become a shared one only where the
            // shared type stands in the same line, and a call site is not such a
            // place - a call in which the word does not appear would move the
            // cleanup point silently, and at such a place there would be no
            // saying whether the value was handed on or duplicated. So this is
            // refused by a code of its own, whose message points at the line
            // where the sharing belongs (ADR-040 D1, Part III C.3).
            if becomes_shared(found, want) {
                let value = given.get(at).and_then(|arg| self.names_of(arg));
                self.checked.findings.push(Finding {
                    severity: Severity::Error,
                    span: span.clone(),
                    code: "NK1115",
                    message: match &value {
                        Some(value) => {
                            format!("`{key}` takes a shared value, and `{value}` is not one")
                        }
                        None => format!("`{key}` takes a shared value, and this is not one"),
                    },
                    notes: vec![format!("`{key}{}`", signature.text())],
                    help: Some(match &value {
                        Some(value) => format!(
                            "write the sharing where it starts: `let {value}: {} = …` - or a \
                             field whose declared type says so (Part I, 6.2)",
                            want.text()
                        ),
                        None => format!(
                            "write the sharing where the value starts - an annotated `let` that \
                             says `{}`, or a field whose declared type does (Part I, 6.2)",
                            want.text()
                        ),
                    }),
                });
                continue;
            }
            self.checked.findings.push(Finding {
                severity: Severity::Error,
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

    /// Whether `found` fits `want` once a transparent container is seen through.
    ///
    /// A type whose ledger records a `deref` is one a caller is not meant to
    /// notice: Part III says of `fs::Mapped` that "a parser cannot tell the
    /// difference", and the entry spells it — `fs::Mapped::deref` is
    /// `(&Mapped) -> &str`. Nothing consulted that entry, so a function taking a
    /// text view refused a mapped file, which is the opposite of what the
    /// specification promises about it.
    ///
    /// **Only as a rescue, never as a rule of its own.** It is asked after a
    /// direct comparison has already failed, so seeing through a container can
    /// make a refusal into an acceptance and can never turn an acceptance into a
    /// refusal. One step only: a container inside a container is two questions,
    /// and nothing in the language has asked the second yet
    /// ([ADR-028](../../../docs/specification/adr/adr-028.md) D5).
    fn fits_through_deref(&self, found: &Ty, want: &Ty) -> bool {
        if found.fits(want) {
            return true;
        }
        let Ty::Named { name, .. } = found else {
            return false;
        };
        let Some((_, contract)) = self.method(&format!("{name}::deref")) else {
            return false;
        };
        // The receiver's own arguments bind the signature's variables, exactly
        // as they do for any other method (ADR-031), so `(&Shared[$T]) -> &$T`
        // answers with what *this* `Shared` holds rather than with a variable.
        let Some(signature) = contract.signature.as_ref() else {
            return false;
        };
        let Some(result) = signature.result.as_ref() else {
            return false;
        };
        let bound = bindings(contract, found);
        ty::substitute(result, &bound).fits(want)
    }

    /// Report only when both sides are known and they disagree.
    fn expect(
        &mut self,
        found: &Ty,
        want: &Ty,
        span: Span,
        what: &str,
        message: impl FnOnce(&str, &str) -> String,
    ) {
        if self.fits_through_deref(found, want) {
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
            severity: Severity::Error,
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
            severity: Severity::Error,
            span: span.clone(),
            code: "NK1108",
            message: format!("this is `{}`, and a condition is a `bool`", found.text()),
            notes: vec![why.to_string()],
            help: Some(
                "compare it: `x != 0`, `text != \"\"`, `xs.len() > 0` (Part I, 3.2)".to_string(),
            ),
        });
    }

    /// A written call to something that can fail (`NK2605`).
    ///
    /// ADR-025 D1 states the rule for the calls **nobody wrote** - a block's
    /// closing brace, a loop's step - and says it is "exactly as a written
    /// call would" behave. The written call is the case the rule was
    /// generalised *from*, and it had no code: a function calling something
    /// that can fail and declaring nothing lowered in silence, and the ledger
    /// then published `[fn."ruft"] signature = "() -> String"` with no
    /// `throws` at all - a committed file that other programs read
    /// ([ADR-020](../../../../docs/specification/adr/adr-020.md)) saying a
    /// function cannot fail when its body can. `rustc` was what refused the
    /// program, about a file the author never wrote, which Part III C.1 calls
    /// a bug in this compiler.
    ///
    /// Reported only where the callee's contract **says** it can fail, which
    /// is what makes this need no guess: the ledger is the fact, and a callee
    /// no ledger describes says nothing here (the same silence `NK2502`'s
    /// method calls keep, C.4). So it never refuses a program that is right,
    /// and it grows as the ledger does.
    fn may_fail_here(&mut self, key: &str, contract: &FnContract, span: &Span) {
        if contract.throws.is_empty() || self.throwing || self.caught {
            return;
        }
        // A grammar action is not code inside a function, and its failure does
        // not travel to one: past the `=>` it leaves the parser as the Nikaia
        // error it is and reaches the `catch` beside the `dsl`
        // ([ADR-023](../../../../docs/specification/adr/adr-023.md) D9). So
        // there is no `throws` to demand and no function to name, and the same
        // is true of a `test` or a `bench` body - which is where `current` is
        // `None`, and the whole of where it is.
        let Some(function) = self.current.clone() else {
            return;
        };
        self.checked.findings.push(Finding {
            severity: Severity::Error,
            span: span.clone(),
            code: "NK2605",
            message: format!("this function can fail because `{key}` can fail"),
            notes: vec![
                format!(
                    "`{key}` carries `throws = {}` in the contracts this program is built \
                     against (Part III, 13.5)",
                    crate::contracts::throws_text(&contract.throws)
                ),
                "nothing marks a failing call, so a failure leaves at a call exactly as it \
                 leaves at a block's closing brace or a loop's step (ADR-023 D8, ADR-025 D1)"
                    .to_string(),
            ],
            help: Some(format!(
                "declare the error: add `throws` to `{function}` - or handle it at the \
                 call, `… catch {{ … }}` (Part I, 7.1)"
            )),
        });
    }

    /// Where a method call stands in ADR-023 D8's propagation, for the emitter
    /// ([`Checked::fallible_methods`]).
    ///
    /// Every method call reaches this, and `fails` says whether the ledger
    /// describing its callee declares `throws`. A call whose receiver or whose
    /// method could not be resolved arrives with `false`, which is not "it
    /// cannot fail" but "there is no answer" - and it lands in the set that
    /// *removes* the pair, so an unresolved call leaves the statement's name
    /// alone rather than speaking for it.
    fn method_propagates(&mut self, method: Ident, fails: bool, span: &Span) {
        let key = (span.start, self.parsed.text(method).to_string());
        if fails {
            self.fallible_methods.insert(key);
        } else {
            self.opaque_methods.insert(key);
        }
    }

    /// **What a task takes with it** (Part I 8.3, `NK2101`).
    ///
    /// Every name the body reads that is a local or a parameter **of the
    /// enclosing function**, with its type - because the type decides whether
    /// taking it is taking it away:
    ///
    ///   * **Data that is copied** - a number, a `bool`, a `char`, a view -
    ///     goes into the task and stays here too. Rust copies it, so there is
    ///     nothing to refuse, and `let n = 7` followed by a `spawn` that prints
    ///     `n` and a `println` that prints it again is a correct program.
    ///   * **A handle on a `Shared[T]`** is *duplicated* rather than moved
    ///     ([ADR-040](../../docs/specification/adr/adr-040.md) D1, D5), so the
    ///     name outside the task keeps working. Part I 8.3 says `NK2101`
    ///     belongs to the data case only, and this is where that holds.
    ///   * **A type nothing describes** is not claimed about at all. Refusing
    ///     one would be refusing on a guess (Part III, C.4).
    ///
    /// What is left is data that a move takes away, which is what the
    /// diagnostic is about.
    fn a_task_takes_these(&mut self, body: &Expr, span: &Span) {
        // The same walk `crosses_into_a_task` uses, and for the same reason: the
        // names a task's body reads are one question, asked once.
        for name in send::names_used(self.parsed, body) {
            let Some((ty, _)) = self.local(&name) else {
                continue;
            };
            if !moves_away(&ty) {
                continue;
            }
            self.moved_into_a_task.push((name, ty, span.end));
        }
    }

    /// `NK2101`: data a task took with it, used again afterwards.
    ///
    /// **The message `rustc` gave instead** is the reason this exists: *"borrow
    /// of moved value: `message`"*, with *"consider cloning the value before
    /// moving it into the closure"* - a closure the program does not have, about
    /// a file nobody wrote (Part III, C.1).
    ///
    /// An **assignment** between the two clears it, and that is not a leniency:
    /// `message = "other"` gives the name a value again, Rust accepts it, and
    /// refusing it would refuse a correct program (C.4).
    fn a_task_took_what_is_used_again(&mut self) {
        let moved = std::mem::take(&mut self.moved_into_a_task);
        let read = std::mem::take(&mut self.read_at);
        let written = std::mem::take(&mut self.written_at);

        for (name, ty, at) in moved {
            // At or after the byte the `spawn` statement **ends** on. The reads
            // inside the task's own body are the move itself and lie inside
            // that statement's span, so they are excluded; a statement span
            // runs up to the next one's first byte, so the next statement
            // starts exactly *at* the end and the comparison is not strict.
            let Some(&(_, used)) = read
                .iter()
                .filter(|(seen, when)| seen == &name && *when >= at)
                .min_by_key(|(_, when)| *when)
            else {
                continue;
            };
            if written
                .iter()
                .any(|(seen, when)| seen == &name && *when >= at && *when <= used)
            {
                continue;
            }
            self.checked.findings.push(Finding {
                code: "NK2101",
                severity: Severity::Error,
                span: Span {
                    start: used,
                    end: used,
                },
                message: format!("this background task takes ownership of `{name}`"),
                notes: vec![
                    format!(
                        "a task started with `spawn` may outlive this function, so it cannot \
                         merely borrow your variables - it takes them with it (Part I, 8.3). \
                         `{name}` is a `{ty}`, which a move takes away"
                    ),
                    "a number, a `bool`, a view or a handle on a `Shared[T]` would not be: \
                     the first three are copied and the last is duplicated (Part I, 6.2)"
                        .to_string(),
                ],
                help: Some(format!(
                    "clone before the task is built, and give the task the copy: \
                     `let copy = {name}.clone()`, then use `copy` inside the task"
                )),
            });
        }
    }

    /// `NK2103`: a `spawn`'s lambda names an argument, and a task is handed
    /// nothing.
    ///
    /// `spawn fn (x) { … }` reads as though `x` arrives from somewhere, and
    /// nothing gives it to a task (Part I 8.2). Dropping the name silently would
    /// be the worse answer: the body would then refer to something nothing
    /// declared, which `NK1117` reports about a name the author *did* write.
    fn a_task_takes_no_arguments(&mut self, params: &[Ident], span: &Span) {
        let Some(first) = params.first() else {
            return;
        };
        let named = params
            .iter()
            .map(|p| format!("`{}`", self.parsed.text(*p)))
            .collect::<Vec<_>>()
            .join(", ");
        self.checked.findings.push(Finding {
            code: "NK2103",
            severity: Severity::Error,
            span: span.clone(),
            message: format!(
                "this task's lambda names {named}, and a task is handed nothing (Part I, 8.2)"
            ),
            notes: vec![
                "`spawn` starts a body, it does not call it with arguments - what the body \
                 needs, it takes from around it, and `spawn` moves those in (Part I, 8.3)"
                    .to_string(),
            ],
            help: Some(format!(
                "drop the argument list: `spawn fn {{ … }}`. Where `{}` was meant to be a \
                 value from here, name it before the task and use it inside",
                self.parsed.text(*first)
            )),
        });
    }

    /// The same, for the `.await` ([`Checked::pausing_methods`]).
    ///
    /// Separate from `method_propagates` because the two answers are separate:
    /// a call can fail without pausing and pause without failing, and the two
    /// sets are narrowed independently. Both are recorded at every method call,
    /// including the ones nothing could be established about - `false` there is
    /// "there is no answer", and it lands in the set that *removes* the pair.
    fn method_pauses(&mut self, method: Ident, pauses: bool, span: &Span) {
        let key = (span.start, self.parsed.text(method).to_string());
        if pauses {
            self.pausing_methods.insert(key);
        } else {
            self.settled_methods.insert(key);
        }
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
            severity: Severity::Error,
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
            severity: Severity::Error,
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

    /// **Part I 9.2, for a field** (`NK1110`).
    ///
    /// A type's fields are private to its package unless they say `pub`, and
    /// until the ledger recorded that ([`FieldContract`]) a type whose fields
    /// were private could be built by name from another package with nothing
    /// saying no. The language below cannot help here the way it does for an
    /// item: the emitted struct is in the **same crate**, so `pub` on a field
    /// buys nothing there ([ADR-047](../../../../docs/specification/adr/adr-047.md)
    /// D2).
    ///
    /// Only for a **qualified** type, which is the same spelling rule
    /// [`Checker::reachable`] uses: a type named `http::Request` is another
    /// package's, and one named `Request` is this package's, where every field is
    /// visible however it is declared.
    fn field_is_reachable(&mut self, ty: &str, field: &FieldContract, span: &Span) {
        if field.public {
            return;
        }
        let Some((package, _)) = ty.trim_start_matches('&').split_once("::") else {
            return;
        };
        if !self.modules.contains(package) {
            return;
        }
        let name = field.name.clone();
        let ty = ty.to_string();
        self.checked.findings.push(Finding {
            severity: Severity::Error,
            span: span.clone(),
            code: "NK1110",
            message: format!("`{ty}.{name}` is private to `{package}`"),
            notes: vec![
                "a field is private to the package that declares its type unless it says \
                 `pub` (Part I, 9.2)"
                    .to_string(),
            ],
            help: Some(format!(
                "write `pub {name}` in `{package}`, or reach it through something that is \
                 public - a type may keep its fields private and offer methods (Part I, 9.3)"
            )),
        });
    }

    /// Part I 9.2: an item is private to its **package** unless it says `pub`.
    ///
    /// The language below enforces this too - `pub` becomes `pub` - but a reader
    /// should not meet the rule as a `rustc` message about a file they did not
    /// write, which is what Part III C.1 calls a bug in this compiler.
    ///
    /// Only a call written `package::item` can leave the package: a call inside
    /// it writes `secret()`, unqualified. So this needs no notion of "which
    /// package am I in" - the spelling says it.
    ///
    /// **No program reaches this today**, and that is [ADR-047](../../../../docs/specification/adr/adr-047.md)
    /// D1 rather than an oversight. The boundary used to be the file, and it is
    /// the package now: the files of one package share a namespace, so a
    /// qualified name is a name from *another* package - and depending on one is
    /// not built (D2). The check stays because the boundary it is about is the
    /// one that is left, and it is the day a package arrives that a private name
    /// needs refusing.
    fn reachable(&mut self, name: &str, contract: &FnContract, span: &Span) {
        let Some((module, item)) = name.split_once("::") else {
            return;
        };
        if !self.modules.contains(module) || contract.public {
            return;
        }
        self.checked.findings.push(Finding {
            severity: Severity::Error,
            span: span.clone(),
            code: "NK1110",
            message: format!("`{item}` is private to `{module}`"),
            notes: vec!["an item is private to the package that declares it unless it says `pub` (Part I, 9.2)".to_string()],
            help: Some(format!(
                "write `pub fn {item}` in `{module}`, or reach it through something that is public"
            )),
        });
    }

    // --- crossing a thread (ADR-005 §1 Group B, `NK25xx`) -------------------
    //
    // Two places a value the *program* wrote reaches another thread, and **the
    // verdict is no longer the same in both** (ADR-045 D1): each asks
    // `contracts::send` about its own destination, and the lock is the type the
    // two answers differ on. Everything else answers alike wherever it is going,
    // so the split costs one argument and changes nothing else.
    //
    // The rule the destination does **not** displace is ADR-038 D7's: a value may
    // cross a thread only if it may cross any thread. What ADR-045 separates is
    // *which* thread is being talked about - one of ours, or one a library we
    // cannot read may start - and each answer is the same at both settings of
    // `user_parallelism`, which is all Group B ever asked. `contracts::send` owns
    // the walk and the reason; these two own the span and the sentence.
    //
    // Both report on `MayNot` and say nothing on `Undecided`, which is
    // `contracts::send`'s module header: refusing what this compiler cannot
    // decide would reject correct programs, and an undecided crossing is not
    // accepted here - `rustc` still type-checks the emitted crate, and
    // ADR-005 D7's `E0277` translation reports that refusal against this same
    // `.nika` line.
    //
    // **A method call is not asked**, and the limit is worth naming: a foreign
    // Rust function is reached by a qualified path (`hyper_shim::serve`), which
    // is what a name-for-name lowering makes callable at all (ADR-011 D2), so
    // the call form below is the one ADR-038 D7 is about. A method on a receiver
    // whose type no ledger describes would need the same question asked of its
    // arguments, and nothing in the corpus reaches it - ADR-028 D5's rule, that
    // an entry exists because a program asked for it.

    /// Part II 11.2: a task runs on a thread of its own, so everything it takes
    /// with it has to be able to cross one (`NK2501`).
    ///
    /// Over-approximate in the direction that costs nothing: the walk collects
    /// every name the body mentions, including function and field names, and a
    /// name that is not in scope is not found and says nothing.
    fn crosses_into_a_task(&mut self, body: &Expr, span: &Span) {
        for name in send::names_used(self.parsed, body) {
            let Some(ty) = self.lookup(&name) else {
                continue;
            };
            let crossing = send::crossing(&ty, self.own, self.library, send::Destination::Ours);
            if crossing.refused().is_none() {
                continue;
            }
            let notes = [
                Some(
                    "a task runs on a thread of its own, so everything it uses has to be able \
                     to cross one (Part II, 11.2)"
                        .to_string(),
                ),
                crossing.note(),
                Some(SAME_AT_BOTH.to_string()),
            ];
            self.checked.findings.push(Finding {
                severity: Severity::Error,
                span: span.clone(),
                code: "NK2501",
                message: format!("`{name}` may not cross into a task, and this task uses it"),
                notes: notes.into_iter().flatten().collect(),
                help: crossing.way_out(),
            });
        }
    }

    /// ADR-038 D7: a value handed to a call this compiler cannot see the end of
    /// may reach a thread that call owns (`NK2502`).
    ///
    /// A Rust dependency may bring its own runtime, so "what does it do with
    /// what I gave it" has no answer here - and a thread of its own is among the
    /// answers. D7's first rule is therefore the same rule as `NK2501`'s, asked
    /// at a call instead of at a task.
    ///
    /// **Unlike `NK2501` this is not hypothetical at `user_parallelism = no`.**
    /// The switch bounds what the *program* runs at once (ADR-037 D2); a foreign
    /// runtime's threads are not the program's, so the crossing is real at both
    /// settings and so is the refusal.
    fn crosses_into_an_unseen_call(
        &mut self,
        callee: &str,
        args: &[Expr],
        found: &[Ty],
        config: &[ast::ConfigArg],
        passed: &[(String, Ty)],
        span: &Span,
    ) {
        let positional = args
            .iter()
            .zip(found)
            .enumerate()
            .map(|(at, (expr, ty))| (self.names_the_argument(expr, at), ty));
        let named = config
            .iter()
            .zip(passed)
            .map(|(arg, (_, ty))| (format!("`{}`", self.parsed.text(arg.name)), ty));

        for (what, ty) in positional.chain(named).collect::<Vec<_>>() {
            let crossing = send::crossing(ty, self.own, self.library, send::Destination::Foreign);
            if crossing.refused().is_none() {
                continue;
            }
            let notes = [
                Some(format!(
                    "nothing written down describes `{callee}`, so this compiler cannot see the \
                     end of it - and starting a thread of its own is among the things it may do \
                     (Part III, 15.2)"
                )),
                crossing.note(),
                Some(SAME_AT_BOTH.to_string()),
            ];
            self.checked.findings.push(Finding {
                severity: Severity::Error,
                span: span.clone(),
                code: "NK2502",
                message: format!("{what} may not cross a thread, and `{callee}` may put it on one"),
                notes: notes.into_iter().flatten().collect(),
                help: crossing.way_out(),
            });
        }
    }

    /// What to call one argument of a call in a message: its own name where it
    /// has one, and its place where it does not.
    fn names_the_argument(&self, expr: &Expr, at: usize) -> String {
        match expr {
            Expr::Variable(name) => format!("`{}`", self.parsed.text(*name)),
            Expr::Field { name, .. } => format!("the `{}` this passes", self.parsed.text(*name)),
            _ => format!("what this passes as argument {}", at + 1),
        }
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
            severity: Severity::Error,
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

    fn no_such_field(&mut self, ty: &str, field: &str, declared: &[FieldContract], span: &Span) {
        let names: Vec<&str> = declared.iter().map(|f| f.name.as_str()).collect();
        let near = nearest(field, &names);
        self.checked.findings.push(Finding {
            severity: Severity::Error,
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

    /// Bind a name, with the constant it stands for where there is one
    /// (ADR-043 D5). Only [`Stmt::Let`] ever passes anything but `None`.
    fn bind_with(&mut self, name: String, ty: Ty, constant: Option<i128>) {
        if let Some(frame) = self.scope.last_mut() {
            frame.push((name, ty, constant));
        }
    }

    fn lookup(&self, name: &str) -> Option<Ty> {
        self.local(name).map(|(ty, _)| ty)
    }

    /// The innermost binding of a name: its type, and the constant it stands
    /// for where the checker could evaluate one.
    fn local(&self, name: &str) -> Option<(Ty, Option<i128>)> {
        self.scope
            .iter()
            .rev()
            .find_map(|frame| frame.iter().rev().find(|(n, _, _)| n == name))
            .map(|(_, ty, constant)| (ty.clone(), *constant))
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
                (Expr::Closure { params, body }, Some(Ty::Fn { params: given })) => {
                    self.lambda(params, body, given)
                }
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
    fn lambda(&mut self, params: &[winnow_grammar::Symbol], body: &Block, given: &[Ty]) -> Ty {
        let frame = params
            .iter()
            .map(|p| self.parsed.text(*p).to_string())
            .enumerate()
            .map(|(at, name)| (name, given.get(at).cloned().unwrap_or(Ty::Unknown), None))
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

    /// A method on a type, by the name `Type::method` the ledger records it
    /// under. A library writes the module in front of it (`fs::Mapped::deref`)
    /// and the receiver's type does not carry one, so the suffix is what
    /// matches - name-for-name resolution, as everywhere else.
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
    fn fields_of(&self, name: &str) -> Option<Vec<FieldContract>> {
        if let Some(fields) = self.structs.get(name) {
            return Some(fields.clone()).filter(|f: &Vec<_>| !f.is_empty());
        }
        // A type of **another file of this program**, by the qualified name a
        // caller writes: `pool::Conn`. By the exact key and never by suffix,
        // which is the difference from the library below - two modules may each
        // declare a `Conn`, and a suffix match would answer with whichever came
        // first. `std` has no such ambiguity and a value's type does not carry
        // the module a library writes in front of it, which is why that one is
        // matched the other way (ADR-011 D2).
        if let Some(contract) = self.own.types.get(name) {
            return Some(contract.fields.clone()).filter(|f: &Vec<_>| !f.is_empty());
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
    fn pattern_bindings(&self, pattern: &MatchPattern) -> Vec<Local> {
        match pattern {
            MatchPattern::Wildcard | MatchPattern::Literal(_) => Vec::new(),
            // A single segment binds; `Op::Times` names a variant.
            MatchPattern::Path(segments) if segments.len() == 1 => {
                vec![(self.parsed.text(segments[0]).to_string(), Ty::Unknown, None)]
            }
            MatchPattern::Path(_) => Vec::new(),
            MatchPattern::Tuple { bindings, .. } | MatchPattern::Named { bindings, .. } => bindings
                .iter()
                .map(|b| (self.parsed.text(*b).to_string(), Ty::Unknown, None))
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
        // A view of a view is the view: `&&str` is not a type this language has.
        Ty::Named { view: true, .. } => inner.clone(),
        // `&String` is the one rewrite, because a text view is spelled `&str`
        // and `&String` is not a type a signature may ask for (6.5).
        Ty::Named { name, args, .. } if args.is_empty() && name == "String" => Ty::view("str"),
        // **Arguments are kept.** `&Vec[i64]` is a view of a `Vec[i64]`, and
        // answering `Unknown` here left every view of a generic type
        // *unchecked* - so a mismatch was reported by `rustc`, about the
        // generated file, which is the one thing Part III C.1 forbids.
        Ty::Named { name, args, .. } => Ty::Named {
            name: name.clone(),
            args: args.clone(),
            view: true,
        },
        // A tuple of views is not a view of a tuple, and nothing writes down
        // what a view of a lambda would be.
        _ => Ty::Unknown,
    }
}

/// The type several parts of a program own at once (Part I 6.2).
const SHARED: &str = "Shared";
/// Part I 6.3's lock, which shares the `Shared` hull's construction line
/// ([ADR-057](../../../docs/specification/adr/adr-057.md)).
const LOCKED: &str = "Locked";

/// Whether a plain value standing where `want` is wanted would be the **first
/// handle** on a shared one.
///
/// `want` is `Shared[T]` and `found` is the `T` it holds. That is the one shape
/// Part I 6.2 gives the annotation: the shared type stands in the line, and the
/// value beside it is what the handle is made of. `Shared::new` does not exist
/// and must not, so the annotation is the constructor
/// ([ADR-040](../../../docs/specification/adr/adr-040.md) §3).
///
/// **It never turns a refusal into an acceptance on its own.** Every caller asks
/// it only after a direct comparison has already failed, and then either records
/// the site (the two places 6.2 permits) or refuses with `NK1115` (everywhere
/// else). A `Shared` already standing where a `Shared` is wanted never reaches
/// here, so a handle is never wrapped twice.
fn becomes_shared(found: &Ty, want: &Ty) -> bool {
    let Ty::Named { name, args, view } = want else {
        return false;
    };
    if name != SHARED || *view {
        return false;
    }
    match args.as_slice() {
        [held] => {
            if found.is_unknown() {
                return false;
            }
            // **`Shared[Locked[T]]` beside a `T` makes both hulls on one line**
            // ([ADR-057](../../../docs/specification/adr/adr-057.md)). The
            // annotation is the constructor, and there is no `Locked::new` in
            // the language any more than there is a `Shared::new` - so the same
            // sentence that gives one hull gives two where two are written.
            //
            // Asked in this order, so a value that is *already* a `Locked`
            // matches at the first level and nothing is wrapped twice - which is
            // the property the paragraph above rests on.
            found.fits(held) || found.fits(&locked_content(held))
        }
        _ => false,
    }
}

/// What a `Locked[T]` holds, or the type itself where it is not one.
fn locked_content(ty: &Ty) -> Ty {
    match ty {
        Ty::Named { name, args, view } if name == LOCKED && !*view => match args.as_slice() {
            [held] => held.clone(),
            _ => ty.clone(),
        },
        _ => ty.clone(),
    }
}

/// Part III C.2: every diagnostic names a concrete way out.
fn convert(found: &Ty, want: &Ty) -> String {
    // **A shared value is wanted and a plain one is here**, and the place this
    // comes up is a `return` into a `-> Shared[T]` signature. Sharing may only
    // begin on a line that writes the type (Part I 6.2), and a signature line is
    // not that line - so the generic "make it a `Shared[T]`" below would name a
    // destination without a road, which is what made this look like a dead end.
    // It is not: there are two roads and this says both.
    if becomes_shared(found, want) {
        return format!(
            "sharing starts on a line that writes the type: either give it a \
             `let x: {} = …` here and return that, or return the plain `{}` and \
             let the caller write the type (Part I, 6.2)",
            want.text(),
            found.text()
        );
    }
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

/// What a constant integer expression came to, and the type an operand's
/// declaration pinned ([`Checker::constant_of`]).
struct Constant {
    /// Folded in an `i128` so a sum that cannot fit an `i64` is still a number
    /// this checker can name rather than one it wrapped.
    value: i128,
    /// The integer type an operand's *declaration* fixed, where one did. **A
    /// literal pins nothing**: `3000000000` is an `i64` wherever a use asks for one
    /// (Part I 2.4), which is why a literal standing alone may not be refused.
    pinned: Option<String>,
}

/// The name of the integer type this is, among the two Part I 2.2 offers.
///
/// A view or a type with arguments is neither: `&i32` is a reference and
/// `Vec[i32]` is a list, and a number does not stand beside either of them.
fn integer_named(ty: &Ty) -> Option<String> {
    let Ty::Named { name, args, view } = ty else {
        return None;
    };
    if !args.is_empty() || *view {
        return None;
    }
    matches!(name.as_str(), "i32" | "i64").then(|| name.clone())
}

/// Whether an expression is a literal — a value written in the source rather
/// than computed.
///
/// **What it is for:** a literal is never a `T?`, whatever this checker did or
/// did not work out about its type. `null` is deliberately absent, because it
/// *is* one.
fn is_literal(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::LitInt(_)
            | Expr::LitFloat(_)
            | Expr::LitStr(_)
            | Expr::LitInterpolated(_)
            | Expr::LitChar(_)
            | Expr::LitBool(_)
    )
}

/// Whether taking a value of this type takes it **away** (`NK2101`).
///
/// Three answers and only one of them is a yes, which is the shape Part III C.4
/// asks for: a refusal on a guess is worse than no refusal.
///
///   * **No, it is copied.** A number, a `bool`, a `char` and a **view** all go
///     into a task and stay here too - Rust copies them, so `let n = 7` and a
///     `spawn` that prints `n` and a `println` that prints it again is a correct
///     program. A view is in this list because `&str` is `Copy`, which is why
///     `let message = "Hello"` is *not* the case Part I 8.3 is about and
///     `"Hello".to_string()` is.
///   * **No, it is duplicated.** A handle on a `Shared[T]` keeps working outside
///     the task ([ADR-040](../../docs/specification/adr/adr-040.md) D1, D5), and
///     Part I 8.3 says `NK2101` belongs to the data case only.
///   * **Nothing is claimed**, for a type nothing describes or a nullable of
///     one: `Unknown` is the absence of an answer and not a licence to refuse.
fn moves_away(ty: &Ty) -> bool {
    match ty {
        Ty::Named { name, view, .. } => {
            !view
                && !is_number(name)
                && !matches!(name.as_str(), "bool" | "char")
                && name != "Shared"
                && name != "SharedMut"
        }
        // A nullable of data is still data: `Option<String>` moves.
        Ty::Nullable(inner) => moves_away(inner),
        // A tuple of copied parts is copied; one with anything else in it is
        // not. `Unknown`, a function type and a type variable claim nothing.
        Ty::Tuple(parts) => parts.iter().any(moves_away),
        _ => false,
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

/// Every `{…}` group in a string, by the text between the braces.
///
/// An escape is skipped whole, so the `{` of a `\u{0041}` does not start one -
/// the same rule the emitter's `interpolation` follows, and for the same
/// reason: a string keeps its escapes as written, so a scanner that does not
/// know that reads `"\u{0041}"` as a hole named `0041`.
fn brace_groups(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                if chars.next() == Some('u') && chars.peek() == Some(&'{') {
                    for c in chars.by_ref() {
                        if c == '}' {
                            break;
                        }
                    }
                }
            }
            '{' => {
                let mut group = String::new();
                for c in chars.by_ref() {
                    if c == '}' {
                        found.push(group);
                        break;
                    }
                    group.push(c);
                }
            }
            _ => {}
        }
    }
    found.retain(|g| !g.trim().is_empty());
    found
}
