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
use crate::fold::Constant;
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
    /// How many such calls, which the boolean above cannot say.
    ///
    /// Nothing in the compiler reads it: every analysis wants *is there one*,
    /// and a count would be a number to be wrong about. It is here for the
    /// **measurement** [ADR-105](../../docs/specification/adr/adr-105.md) §1
    /// makes a claim with — *35 unanswered method calls in the corpus, every one
    /// downstream of a `?` that is a sequence* — so that the claim can be
    /// checked again rather than remembered. `crates/nikaia/tests/sequences.rs`
    /// is what checks it.
    pub unanswered: usize,
    /// Which of `resolved` were reached from inside a **`spawn`** body, and
    /// whether any unresolvable call was
    /// ([ADR-039](../../docs/specification/adr/adr-039.md) D3).
    ///
    /// A task started with `spawn` runs **later and elsewhere**, so what it
    /// does is not what the surrounding function does — where a trailing
    /// lambda's body *is*, because it runs during the call
    /// ([ADR-029](../../docs/specification/adr/adr-029.md) D4). An analysis
    /// that asks about the calling function's own reach subtracts this; one
    /// that asks what the whole text mentions does not, which is why it is a
    /// second set rather than a narrowing of the first.
    pub in_a_task: CallsInATask,
}

/// The half of [`MethodCalls`] that is inside a `spawn` body.
///
/// A type of its own rather than a second [`MethodCalls`], which would nest
/// forever — and a name that says which half it is, so that a reader of
/// `calls.in_a_task.resolved` does not have to hold the nesting in their head.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CallsInATask {
    /// The ledger keys resolved inside a task's body.
    pub resolved: BTreeSet<String>,
    /// A call inside a task's body that could not be resolved.
    pub unresolved: bool,
}

/// What one pass of the checker learned.
#[derive(Debug, Clone, Default)]
pub struct Checked {
    /// Every mistake it is sure about.
    pub findings: Vec<Finding>,
    /// What each `comptime` is written as below - its type and its **value** - by
    /// the byte its statement starts at
    /// ([ADR-073](../../docs/specification/adr/adr-073.md) D3, D4).
    ///
    /// The same arrangement as `fallible_methods` and for the same reason, said
    /// twice over. The **type** may be left out and Rust's `const` demands one,
    /// so somebody has to infer it, and the emitter has no types
    /// ([ADR-028](../../docs/specification/adr/adr-028.md)). The **value** is
    /// here for the sharper half of the same reason: folding `PAGE * 2` means
    /// knowing what `PAGE` is, which is a scope - and the emitter has none of
    /// those either, so a constant built out of another would reach the
    /// language below unfolded and D3's demand would be `rustc`'s to keep.
    ///
    /// Both are spelled in the language below, so the emitter writes the pair
    /// and decides nothing.
    pub comptime_values: BTreeMap<usize, (String, String)>,
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
    /// The `set` calls that carry a **witness**
    /// ([ADR-111](../../docs/specification/adr/adr-111.md) D5), by the byte
    /// their statement starts at.
    ///
    /// `kasse.set(neu; after: stand)` is a different door from `set` — it
    /// compares, and it can fail — and only a type checker knows that this
    /// receiver is a lock, which is what `Locked::set(after)` in the ledger is
    /// keyed on. The emitter writes `set_after(neu, stand)` for a call in here
    /// and the ordinary `set` for one that is not, so a user-defined `set` with
    /// an option of its own is untouched.
    ///
    /// The statement and not the call, which is `fallible_methods`' shape and
    /// carries its limit: two `set`s in one statement, one witnessed and one
    /// not, is a program this cannot tell apart. The emitter asks for the
    /// written `after:` as well, so what that costs is bounded by the pair
    /// being written at all.
    /// The lambda arguments that lower to a closure returning a **boxed
    /// future** ([ADR-122](../../docs/specification/adr/adr-122.md) D1), by the
    /// byte their statement starts at and the argument's position.
    ///
    /// The parameter's **type** decides the shape — it may pause unless it says
    /// `sync` — and a type is what the emitter has none of
    /// ([ADR-028](../../docs/specification/adr/adr-028.md)), so the answer is
    /// computed where it is known and looked up where it is needed. The same
    /// arrangement `lent_args` and `nullable_args` have.
    pub future_lambdas: BTreeSet<(usize, usize)>,
    pub witnessed_sets: BTreeSet<usize>,
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
    pub nullable_sites: BTreeMap<usize, Wrap>,
    /// The `+`s that join text, by the byte their **operator** starts at
    /// ([ADR-081](../../docs/specification/adr/adr-081.md) D2).
    ///
    /// Keyed by the operator and not by the statement, which is the whole reason
    /// `Expr::Binary` gained a span: `a + b + c` is two of them and the statement
    /// they stand in is one. Every other answer in this channel is keyed by a
    /// statement because every other answer is about one thing per statement.
    ///
    /// **Only text is in here.** A number's `+` stays exactly where it is,
    /// because arithmetic that moved into `std` would silently lose
    /// [ADR-043](../../docs/specification/adr/adr-043.md) D1's overflow abort -
    /// `overflow-checks` is on per Nikaia crate and off for the profile, and
    /// inlining does not carry the check across.
    pub concatenations: BTreeSet<usize>,
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
    /// one position per field, so it needs the name too.
    ///
    /// **And the type, and [`argument_shape`]**, for `nullable_args`' reason and
    /// found by the same probe: a statement may build two literals.
    /// `f(P { x: 1 }, P { x: null })` had one key for two fields and came out
    /// `P { x: Some(1) }, P { x: Some(None) }`; `f(P { x }, Q { x })` would have
    /// had one for two *structs*. The type is `unaliased` on both sides, so the
    /// two passes spell it the same.
    pub nullable_fields: BTreeMap<(usize, String, String), BTreeMap<String, Wrap>>,
    /// The **call arguments** where a plain value stands in a nullable
    /// parameter, as the byte the statement starts at, the callee as the source
    /// wrote it, and the argument's position (Part I 2.3).
    ///
    /// The third position D4's wrap needs a key for. An expression carries no
    /// span, so the key is built out of what both sides can see: the statement,
    /// the callee's **written** name - the emitter has only what the source
    /// says, and a method's key is `Type::method` while a constructor's is
    /// `Type::new`, neither of which stands at the call - the argument's
    /// position, and [`argument_shape`].
    ///
    /// **The shape is what says which *call***, and leaving it out was a
    /// defect rather than a simplification. The three outer parts name a
    /// *parameter*; a statement may call the same function twice, and then
    /// `let r = pick(1) + pick(null)` had one key for two arguments. The last
    /// one walked won, so one of them was emitted with the other's answer -
    /// `pick(Some(1)) + pick(Some(None))`, which `rustc` refuses about a file
    /// nobody wrote (Part III, C.1).
    pub nullable_args: BTreeMap<(usize, String, usize), BTreeMap<String, Wrap>>,
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
    /// The **handles a task's body uses**, by the byte the statement starts at
    /// and the name ([ADR-040](../../docs/specification/adr/adr-040.md) D1).
    ///
    /// A task takes what it names by value, and a handle handed on by value is
    /// **duplicated**. For a call the emitter reads that off the callee's
    /// signature; a task's body has no signature, so which names are handles is
    /// a question about types and is answered here - the same arrangement
    /// `fallible_methods` and the rest use.
    pub task_handles: BTreeSet<(usize, String)>,
    /// The **call arguments the compiler writes a `&` for**
    /// ([ADR-094](../../docs/specification/adr/adr-094.md) D1), keyed the way
    /// [`Checked::nullable_args`] is: the byte the statement starts at, the
    /// callee as the source wrote it, the argument's position, and
    /// [`argument_shape`].
    ///
    /// Whether an argument is lent or handed over is the **callee's** answer
    /// (`contracts::keeps::lends`), and the emitter reads it for the
    /// declaration too — one answer, two positions, because the two disagreeing
    /// is a `&&T` or a moved value in the language below. What the emitter
    /// cannot decide on its own is the other half: whether *this* argument is
    /// already a view, which is a question about its type (ADR-028).
    ///
    /// The four-part key is `nullable_args`' and is there for the same reason:
    /// a statement may call one function twice, and `f(a) + f(b)` has two
    /// arguments in one position.
    /// The **options a method call has**, in the order the declaration gives,
    /// by the byte the statement starts at and the method's written name
    /// (Part I 5.1, [ADR-133](../../docs/specification/adr/adr-133.md) D1).
    ///
    /// The language below has no named arguments and no defaults, so an option
    /// becomes positional at the call and a missing one becomes its default.
    /// For a call **by name** the emitter reads that off the callee's contract
    /// itself; for a **method** it cannot, because finding the entry means
    /// resolving the receiver and the emitter has no types
    /// ([ADR-028](../../docs/specification/adr/adr-028.md)) — the same
    /// arrangement `lent_args` and `nullable_args` have.
    ///
    /// Without it a method's options were **dropped**: `q.execute(target_age: 30)`
    /// came out as `q.execute()` and `rustc` answered about the arity of a file
    /// nobody wrote ([Part III C.1](../../docs/specification/30-nikaia-tooling.md)).
    /// Older than the spelling that makes it easy to hit, because the `;` form
    /// went down the same path.
    pub method_options: BTreeMap<(usize, String), Vec<(String, String)>>,
    pub lent_args: BTreeMap<(usize, String, usize), BTreeSet<String>>,
    /// The call arguments the compiler writes a **`&mut`** for
    /// ([ADR-094](../../docs/specification/adr/adr-094.md) D3), keyed as
    /// [`Checked::lent_args`] is.
    ///
    /// The third state beside *lent* and *handed over*, and the one that is a
    /// **declaration** rather than an inference: `mut out: Vec[i64]` is the
    /// claim, and a parameter without the word does not become one whatever its
    /// body does. `keeps::lends` withholds its own answer on such a position,
    /// so the two never both write a reference.
    ///
    /// What the caller has to do is what it already does to call `xs.push(1)`:
    /// write `let mut xs`. The call itself shows nothing, which is D3's whole
    /// sentence — a language that hides mutation through a receiver and shows
    /// it through an argument has two rules for one thing.
    pub mut_args: BTreeMap<(usize, String, usize), BTreeSet<String>>,
    /// What each `spawn` body **binds and then holds across a pause**, by the
    /// byte the `spawn` starts at and the name
    /// ([ADR-055](../../docs/specification/adr/adr-055.md) §2 D6).
    ///
    /// **Written down because it is a computation and not a lookup.** The
    /// refusal beside it (`NK2501`) is silent today and will be until a type
    /// answers `MayNot` at `Destination::Ours` — no type does, and
    /// `contracts::send` says why. So the *liveness* half, which is an
    /// over-approximation that can be wrong, would otherwise be code nothing
    /// can see the answer of. `contracts::send::held_across_a_pause` is the
    /// rule itself and is tested on its own; this is what the walk feeding it
    /// found in a real program.
    pub held_across_a_pause: BTreeSet<(usize, String)>,
    /// The `let` statements whose initialiser is a **place** holding a value
    /// that would have to move, by the byte the statement starts at
    /// ([ADR-094](../../docs/specification/adr/adr-094.md) D4).
    ///
    /// `let s = totals.stations[name]` and `let name = config.name` are views:
    /// the language below refuses to move a value out of a container or out of
    /// a borrowed field, so a move there was never what the line meant, and the
    /// emitter writes the `&` it would otherwise have been refused for leaving
    /// out.
    ///
    /// **And only where the value would move.** `let mi = self.bodies[i].mass`
    /// over an `f64` is a *copy*, and a `&` there is a borrow held across the
    /// loop that writes the same field — `E0502` about a file nobody wrote.
    /// Which of the two is a question about the type, the emitter has none
    /// (ADR-028), and [`moves_away`] is the same answer `NK2101` reads.
    ///
    /// **Silence where the type is unknown**, which leaves the statement
    /// exactly where every program already is. The two wrong answers are both
    /// `rustc`'s words about the generated file — a borrow that conflicts, or a
    /// move out of a container — so this is a set that has to be *right* rather
    /// than one that can be safe.
    pub lent_lets: BTreeSet<usize>,
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
        grammars: parsed
            .program
            .items
            .iter()
            .filter_map(|item| match &item.node {
                Item::Grammar(def) => Some((
                    parsed.text(def.name).to_string(),
                    def.rules
                        .iter()
                        .filter(|r| r.is_public)
                        .map(|r| parsed.text(r.name).to_string())
                        .collect(),
                )),
                _ => None,
            })
            .collect(),
        scope: Vec::new(),
        at_a_write_door: false,
        set_receiver: None,
        stamped_condition: None,
        inside_a_door: false,
        task_bindings: Vec::new(),
        said_mut: BTreeSet::new(),
        expected: None,
        type_parameters: BTreeMap::new(),
        struct_parameters: BTreeMap::new(),
        borrowing_self: false,
        enclosing: BTreeMap::new(),
        throwing: false,
        caught: false,
        guarded: None,
        loops: 0,
        barrier: None,
        current: None,
        modules: modules.clone(),
        fallible_methods: BTreeSet::new(),
        pausing_methods: BTreeSet::new(),
        settled_methods: BTreeSet::new(),
        handed_over: None,
        foreign_names: parsed
            .program
            .items
            .iter()
            .filter_map(|item| match &item.node {
                Item::Extern { declarations, .. } => Some(declarations),
                _ => None,
            })
            .flatten()
            .map(|d| parsed.text(d.node.name).to_string())
            .collect(),
        inside_unsafe: false,
        moved_into_a_task: Vec::new(),
        walked: Vec::new(),
        receiver_name: None,
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
    // Kap 4.7: what an `impl` owes the `trait` it names. Separate for a reason
    // of its own - it reads the ledger's **finished** `sync` column, and the
    // type walk runs before `sync::infer` fills that in
    // ([ADR-080](../../docs/specification/adr/adr-080.md)).
    checker
        .checked
        .findings
        .extend(crate::traits::check(parsed, own));
    // Part I 2.2: a **type** nothing declares, which had nothing where a value
    // has had `NK1117` since ADR-051
    // ([ADR-096](../../docs/specification/adr/adr-096.md)). Separate for the
    // same reason as the three above: it asks about a written *name* rather
    // than about a value's type, so it needs the item tree and neither the
    // scope stack nor the inference.
    checker
        .checked
        .findings
        .extend(crate::types::check(parsed, own, library));
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
/// Which wrap a value standing in `want` needs, and `None` where it needs none.
///
/// One function for all four of Part I 2.3's positions, so the rule is stated
/// once: the annotated `let`, the assignment, the `return`, and a struct
/// literal's field — and a call's argument, which is the fourth written a fifth
/// way. Before this they were four copies of the same three conditions, and the
/// copies had already drifted in what they did with an unknown type.
/// **Which of a statement's calls an argument belongs to**, as text both the
/// checker and the emitter can produce from the same tree.
///
/// A key of statement, callee and position names a *parameter*; a statement may
/// call one function more than once, and the argument itself is what tells the
/// two calls apart. The alternatives were weighed and each fails on something
/// this does not:
///
///   * **the argument's span** - an argument has none. One node type does:
///     [ADR-081](../../docs/specification/adr/adr-081.md) D1 gave `Expr::Binary`
///     a span of its own, for a key of exactly this kind. It is one node and an
///     argument is rarely that one, so there is nothing general to key by -
///     which is the reason this whole side table exists;
///   * **the address of the `Expr`** - stable for a statement walked twice, and
///     not for an `f"…"` hole, which is *text* until each pass parses it into a
///     tree of its own ([`crate::emit::literal_expressions`]);
///   * **the order the calls are walked in** - which would make the two passes
///     agree by assumption rather than by construction.
///
/// So it is structural: the same expression, from either pass, renders the
/// same. The interner is shared, so an identifier renders as the same `Symbol`
/// on both sides - including in a re-parsed hole, which is parsed with that
/// interner.
///
/// **Two textually identical arguments share an entry, and want the same
/// answer**: the wrap is a function of the argument's type and the parameter's,
/// and within one statement the same expression in the same position has the
/// same type.
///
/// It is built only where a wrap was recorded for this statement, callee and
/// position - which is rare - so the cost is not on the path every argument
/// takes.
/// Whether an expression **can only** be a value: a name, a literal, an
/// operator, a field, an index.
///
/// The half of `NK1141`'s question that is safe to answer from the shape. A
/// call and a method call are deliberately not here: whether one comes to a
/// value is a question about its callee, and answering it wrongly is a correct
/// program refused ([Part III C.4](../../docs/specification/30-nikaia-tooling.md)).
fn plainly_a_value(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::Variable(_)
            | Expr::LitInt(_)
            | Expr::LitFloat(_)
            | Expr::LitStr(_)
            | Expr::LitBool(_)
            | Expr::Binary { .. }
            | Expr::Field { .. }
            | Expr::Index { .. }
            | Expr::Tuple(_)
    )
}

pub fn argument_shape(expr: &Expr) -> String {
    format!("{expr:?}")
}

fn wrap_for(found: &Ty, want: &Ty, literal: bool) -> Option<Wrap> {
    if !matches!(want, Ty::Nullable(_)) || matches!(found, Ty::Nullable(_)) {
        return None;
    }
    match !found.is_unknown() || literal {
        true => Some(Wrap::Constructor),
        false => Some(Wrap::Conversion),
    }
}

/// **How a plain value is put into a nullable slot** (Part I 2.3,
/// [ADR-068](../../../docs/specification/adr/adr-068.md)).
///
/// The question the four positions of [ADR-052](../../../docs/specification/adr/adr-052.md)
/// D4 ask was never *whether* to wrap - it was **how**, and the second answer
/// was missing. A value this checker worked out to be a plain `T` takes the
/// constructor; one whose type it could not work out takes the conversion,
/// which is right whichever the value turns out to be.
///
/// A value already known to be a `T?` is in neither: it needs nothing, and the
/// position is simply not recorded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wrap {
    /// `Some(value)`. The type is known and it is not a `T?`.
    Constructor,
    /// `value.into()`. The type is **not** known, and this is correct in both
    /// directions - Rust has `From<T> for Option<T>` and the identity
    /// `From<T> for T` - so the uncertainty stops needing an answer.
    Conversion,
}

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
    /// [`Checked::witnessed_sets`].
    pub witnessed_sets: BTreeSet<usize>,
    /// [`Checked::future_lambdas`].
    pub future_lambdas: BTreeSet<(usize, usize)>,
    /// [`Checked::narrowing_casts`].
    pub narrowing: BTreeMap<(usize, String), Narrowing>,
    /// [`Checked::nullable_sites`].
    pub nullable: BTreeMap<usize, Wrap>,
    /// [`Checked::flattened_reaches`].
    pub flattened: BTreeSet<(usize, String)>,
    /// [`Checked::nullable_fields`].
    pub nullable_in_fields: BTreeMap<(usize, String, String), BTreeMap<String, Wrap>>,
    /// [`Checked::nullable_args`].
    pub nullable_in_args: BTreeMap<(usize, String, usize), BTreeMap<String, Wrap>>,
    /// [`Checked::task_handles`].
    pub task_handles: BTreeSet<(usize, String)>,
    /// [`Checked::comptime_values`].
    pub comptime_values: BTreeMap<usize, (String, String)>,
    /// [`Checked::concatenations`].
    pub concatenations: BTreeSet<usize>,
    /// [`Checked::lent_lets`].
    pub lent_lets: BTreeSet<usize>,
    /// [`Checked::lent_args`].
    pub lent_args: BTreeMap<(usize, String, usize), BTreeSet<String>>,
    /// [`Checked::mut_args`].
    pub mut_args: BTreeMap<(usize, String, usize), BTreeSet<String>>,
    /// [`Checked::method_options`].
    pub method_options: BTreeMap<(usize, String), Vec<(String, String)>>,
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
        witnessed_sets: checked.witnessed_sets,
        future_lambdas: checked.future_lambdas,
        narrowing: checked.narrowing_casts,
        nullable: checked.nullable_sites,
        flattened: checked.flattened_reaches,
        nullable_in_fields: checked.nullable_fields,
        nullable_in_args: checked.nullable_args,
        task_handles: checked.task_handles,
        comptime_values: checked.comptime_values,
        concatenations: checked.concatenations,
        lent_lets: checked.lent_lets,
        lent_args: checked.lent_args,
        mut_args: checked.mut_args,
        method_options: checked.method_options,
    }
}

/// How a `comptime`'s type is spelled in the language below, where this compiler
/// can spell it ([ADR-073](../../docs/specification/adr/adr-073.md) D5).
///
/// **A short list on purpose.** Rust's `const` takes a type and no inference,
/// so a Nikaia type this cannot name is a `comptime` this cannot write - and
/// `None` here becomes `NK1127` rather than a guess. The list grows with D5's
/// stages: a `String` is missing because Rust has no `const String`, and what
/// a literal string would become - a `&'static str` - is a different type from
/// the one Part I gives it, which is a decision rather than a mapping.
fn rust_constant_type(ty: &Ty) -> Option<String> {
    match ty {
        Ty::Named { name, args, view } if args.is_empty() && !*view => match name.as_str() {
            "i32" | "i64" | "u32" | "u64" | "f32" | "f64" | "bool" | "char" => Some(name.clone()),
            _ => None,
        },
        _ => None,
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
struct Local {
    name: String,
    ty: Ty,
    constant: Option<i128>,
    /// Where the binding was written and what kind it is, when it did **not**
    /// say `mut` — `None` for one a change to is nobody's business here.
    ///
    /// It lives on the binding rather than in a map of its own so that the
    /// **scope** is the one `scope` already keeps: a `let` inside a block stops
    /// being the answer when the block closes, and an outer `mut` name is the
    /// answer again. A parallel map would have had to be pushed and popped at
    /// twenty-eight places, and getting one of them wrong is a *correct
    /// program refused* ([Part III
    /// C.4](../../docs/specification/30-nikaia-tooling.md)).
    immutable: Option<Immutable>,
}

/// A binding that did not say `mut`: where it was written, and which of the two
/// kinds it is.
///
/// The two are one rule reached from two sides — *what is changed says `mut`* —
/// and they are two codes because the way out is written in a different place
/// and a reader is doing a different thing. `NK1138` is a **parameter**'s
/// ([ADR-094](../../docs/specification/adr/adr-094.md) D3), where the word also
/// decides what the caller sees; `NK1139` is a **`let`**'s, which [Part I
/// 2.1](../../docs/specification/10-nikaia-light.md) states outright and which
/// `rustc` had been answering instead.
#[derive(Clone)]
struct Immutable {
    at: Span,
    kind: Kind,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Parameter,
    Let,
}

impl Local {
    /// A binding nothing here refuses a change to: `self`, an option, a `for`
    /// binding, a pattern's name. Each of those is either not a place a
    /// program assigns to or one whose `mut` is a question of its own, and
    /// silence is what [Part III
    /// C.4](../../docs/specification/30-nikaia-tooling.md) asks for.
    /// The same binding again, for the one frame walked twice (a constructor's
    /// parameters, checked once as written and once as `Type::new`).
    fn again(local: &Local) -> Self {
        Local {
            name: local.name.clone(),
            ty: local.ty.clone(),
            constant: local.constant,
            immutable: local.immutable.clone(),
        }
    }

    fn free(name: String, ty: Ty) -> Self {
        Local {
            name,
            ty,
            constant: None,
            immutable: None,
        }
    }
}

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
    /// Every **grammar** declared here, with the names of its `pub` rules
    /// ([ADR-082](../../docs/specification/adr/adr-082.md) D1, D2).
    ///
    /// A grammar is entered by an ordinary call — `Json.value(input)` — so its
    /// name has to be something `NK1117` counts as declared, and which rules
    /// may stand after the dot is what says `Json.internal(x)` is not an entry.
    grammars: BTreeMap<String, BTreeSet<String>>,
    /// Names in scope, innermost frame last.
    scope: Vec<Vec<Local>>,
    /// What the function being walked declared it hands back.
    expected: Option<Ty>,
    /// The type parameters in scope where the body being walked stands - the
    /// function's own `[T]` and the `[T]` of the `impl` around it.
    ///
    /// A name in here is a **type** while the body is walked and a **variable**
    /// at every call site ([ADR-074](../../docs/specification/adr/adr-074.md)
    /// D1). Empty everywhere else, which is every function written today.
    type_parameters: BTreeMap<String, Vec<String>>,
    /// Every generic struct declared here, with its parameters in **declaration
    /// order** - which is what makes a type argument's position mean something.
    ///
    /// Absent for a struct with no parameters, which is every struct written
    /// today, so the two questions below cost a failed lookup and nothing else.
    struct_parameters: BTreeMap<String, Vec<String>>,
    /// Whether the method being walked took its subject by **reference**.
    ///
    /// `&self` and `&mut self` borrow it; a bare `self` owns it. What hangs on
    /// the difference is whether a field may be handed out by value at all
    /// ([ADR-083](../../docs/specification/adr/adr-083.md)). `false` for a free
    /// function, which has no subject to borrow.
    borrowing_self: bool,
    /// The `[T]` of the `impl` whose methods are being walked, on its own.
    ///
    /// Separate from the field above because `function` rebuilds that one per
    /// method and has to start from what the `impl` put in scope rather than
    /// from nothing.
    enclosing: BTreeMap<String, Vec<String>>,
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
    /// What the guarded expression of the nearest `catch` turned out to hold,
    /// while that expression is being walked - and `None` everywhere else.
    ///
    /// The mirror of [`Self::caught`], asked in the other direction: that one
    /// says *a failure here needs no `throws`*, this one says *was there a
    /// failure at all*. A `catch` over an expression that cannot fail lowers to
    /// a `match` over something that is not a `Result`, which is `NK1134`
    /// ([ADR-091](../../../docs/specification/adr/adr-091.md)).
    guarded: Option<Guarded>,
    /// How many loops are open around the statement being walked, counted from
    /// the nearest boundary a jump may not cross rather than from the function.
    ///
    /// A `break` is legal where this is not zero, and `NK1132` is what it meets
    /// where it is (Part I 3.3,
    /// [ADR-084](../../../docs/specification/adr/adr-084.md) D4).
    loops: usize,
    /// What that nearest boundary is, where one stands between here and the
    /// function's own body: a lambda, a task, an `overlap` branch.
    ///
    /// Each of those is **a function of its own in the language below**, and a
    /// jump does not leave a function - so a `break` inside one whose loop is
    /// outside it is not a program this compiler may lower
    /// ([ADR-084](../../../docs/specification/adr/adr-084.md) D4). It is carried
    /// rather than derived because the message is the whole value of catching
    /// it here: without the word, the refusal would be `rustc`'s, about a file
    /// nobody wrote (Part III, C.1).
    ///
    /// **A `catch` handler is deliberately not one of these** (D5): it lowers to
    /// a `match` arm, and a jump in one reaches the loop around it.
    barrier: Option<&'static str>,
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
    /// **What the lambda being walked has been seen to do**
    /// ([ADR-102](../../docs/specification/adr/adr-102.md) D2).
    ///
    /// A function type carries two promises and the defaults are the
    /// language's: without `sync` the code may pause, without `throws` it
    /// cannot fail. A lambda handed to one has to keep them, and *keeping* is
    /// a question about its body — so the body is walked with this in hand and
    /// the calls it makes put their answers in it.
    ///
    /// **It does not cross a boundary.** A lambda written inside another one
    /// is a function of its own: what its body does happens when *its* callee
    /// runs it, not when this one does. So [`Checker::past_a_boundary`] clears
    /// it and puts the outer one back, the way it does with the loop count.
    ///
    /// `None` where nothing is being asked — outside a lambda, and inside one
    /// whose parameter's type nothing describes.
    handed_over: Option<Handed>,
    /// **The names an `extern "C"` block declares**
    /// ([ADR-124](../../docs/specification/adr/adr-124.md) D3).
    ///
    /// Collected from the item tree rather than read off the ledger, because
    /// the ledger records what a name *is* and this asks where it **came
    /// from**: a Nikaia function and a C declaration are both entries, and only
    /// one of them has to be called inside an `unsafe` block.
    foreign_names: BTreeSet<String>,
    /// Whether what is being walked stands inside one.
    ///
    /// It reaches inward the way `caught` and `in_lambda` do, and it stops at
    /// nothing: a lambda written inside an `unsafe` block runs later, and D3
    /// asks about where the **call** is written rather than about when it runs.
    inside_unsafe: bool,
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
    /// **A name whose sequence was walked** (`NK2702`,
    /// [ADR-105](../../docs/specification/adr/adr-105.md) D2): the name, the type
    /// it held, and the byte the walking statement **ends** on.
    ///
    /// `moved_into_a_task`'s shape and for its reason: the question is about what
    /// comes *after* the walk, and a single pass reaches a later statement later.
    /// The statement's end and not its start, because the walk itself is a read
    /// inside that statement.
    walked: Vec<(String, Ty, usize)>,
    /// The **name** of the receiver of the method call being walked, where it is
    /// a plain name ([ADR-105](../../docs/specification/adr/adr-105.md) D2).
    ///
    /// `set_receiver`'s arrangement and for its reason: the rule wants the name
    /// and runs in `call_on`, where the contract is in hand and the receiver's
    /// expression is not. `None` for a temporary — `map.keys().collect()` walks
    /// one, and a temporary has no second use to refuse.
    receiver_name: Option<String>,
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
    /// The `let`s made inside each enclosing `spawn` body, innermost last: the
    /// name, its type, and the byte its statement starts at
    /// ([ADR-055](../../docs/specification/adr/adr-055.md) D6).
    ///
    /// **`NK2501` asks about what a task *captures*; this is what it *binds*.**
    /// A value bound inside the body and still live at a suspension point
    /// further down is held **inside the future**, so it crosses a thread just
    /// as a captured one does — and the pool's starter asks for `Send` of the
    /// whole future. Nothing in the frame `scope` pushes for the body survives
    /// the walk, so the types are collected here while they are known.
    ///
    /// A stack, because a task may `spawn` another: each body's bindings are
    /// its own.
    task_bindings: Vec<Vec<(String, Ty, usize)>>,
    /// Whether the lambda being walked is a **write door**'s block — `update`
    /// or `update_all` ([ADR-110](../../docs/specification/adr/adr-110.md) D1,
    /// D6).
    ///
    /// It is what narrows `NK1138` at a lambda parameter to where D1 asks for
    /// it. Everywhere else a parameter without the word is left alone, because
    /// the emitter writes `mut` itself for a fold's accumulator and refusing
    /// there would refuse a program that compiles.
    at_a_write_door: bool,
    /// The name the `set` being typed was called on, for `NK2205`'s message
    /// ([ADR-111](../../docs/specification/adr/adr-111.md) D4).
    ///
    /// The rule wants the receiver's **name** and the argument's **type**, and
    /// those are known in two different places: the name where the method call
    /// is walked, the types after `arguments_given` has run. Carried rather
    /// than passed, the way the door flags beside it are.
    set_receiver: Option<String>,
    /// Where the condition this stands under was read from a lock, if it was
    /// ([ADR-111](../../docs/specification/adr/adr-111.md) D4's second shape).
    ///
    /// `if stand > 100 { kasse.set(0) }` stores a plain value; the **decision**
    /// is the stale thing. A `while`, a `match` and a nested `if` are
    /// conditions alike, which is why this is a flag over a block rather than a
    /// question asked of one statement.
    stamped_condition: Option<usize>,
    /// Whether what is being walked is **inside a door's block** — `access`,
    /// `update`, `access_all` or `update_all`
    /// ([ADR-039](../../docs/specification/adr/adr-039.md) D10).
    ///
    /// `at_a_write_door` above is the narrower question `NK1138` and `NK1141`
    /// ask; this is the one `NK2203` asks, because *a lock taken while a lock
    /// is held* is about **any** door being open and not about which
    /// ([ADR-039](../../docs/specification/adr/adr-039.md) D2).
    ///
    /// `get` and `set` are doors and are **not** here, and D10 says why: while
    /// the lock is open in either of them no code of the program's runs, so
    /// there is nothing that could take a second one.
    inside_a_door: bool,
    /// The bindings `NK1138` and `NK1139` have already been said about, by the
    /// byte each declaration starts at.
    ///
    /// Said once per binding: a body that changes one usually does so several
    /// times, and three carets on one declaration is noise rather than
    /// information. It is not scoped, and must not be — two blocks that each
    /// declare an `xs` have two spans, and one that is walked twice is one
    /// span and one message.
    said_mut: BTreeSet<usize>,
    checked: Checked,
}

/// What a lambda's body was seen to do
/// ([ADR-102](../../docs/specification/adr/adr-102.md) D2).
#[derive(Debug, Clone, Copy)]
struct Handed {
    pauses: bool,
    fails: bool,
    /// What the type it was handed to allows, kept beside what it was seen to
    /// do so that the one message about a mismatch is the one that is said.
    promised: Promises,
}

/// What a function type **allows** the code it names to do (D2), which is the
/// reading a declaration already has: without `sync` it may pause, without
/// `throws` it cannot fail.
#[derive(Debug, Clone, Copy)]
struct Promises {
    may_pause: bool,
    may_fail: bool,
}

/// One argument as [`Checker::the_compiler_writes_the_reference`] reads it:
/// where it stands in the call, what was written there, the type that was
/// found, and the one the parameter wants. Four values that only ever travel
/// together, and a name for them so the rule reads as one question.
struct Argument<'a> {
    /// The position among the arguments, the receiver not counted.
    at: usize,
    /// What the source wrote there, where there is an expression to read.
    given: Option<&'a Expr>,
    /// The type of what was written.
    found: &'a Ty,
    /// The type the parameter declares.
    want: &'a Ty,
}

/// What the guarded expression of a `catch` turned out to hold.
///
/// Two answers and not one, because *nothing can fail here* and *nothing here
/// could be looked up* are different facts and only the first may be acted on.
/// [Part III C.4](../../../docs/specification/30-nikaia-tooling.md): a call no
/// ledger describes says nothing about whether it throws, and refusing a
/// `catch` over one would refuse a program that is right. The refusal fires on
/// **known not to fail**, never on *not known to fail* - which is
/// [`Checker::may_fail_here`]'s own standard, read from the other end.
#[derive(Default)]
struct Guarded {
    /// A call the ledger describes, whose contract carries a `throws`.
    fallible: bool,
    /// Something this compiler could not look up, or a nested `catch` - either
    /// way, no answer, and the refusal stays quiet.
    unanswered: bool,
}

/// Whether a body's last statement is a loop no jump leaves
/// ([ADR-093](../../../docs/specification/adr/adr-093.md)).
///
/// [ADR-070](../../../docs/specification/adr/adr-070.md) D3: a function that
/// genuinely never returns — an accept loop, an event loop, a supervisor — had
/// to end with a `return 0` that cannot be reached, and a reader of that line
/// could not tell dead code from a mistake.
///
/// **The literal `true` only**, never a name that happens to be true: the
/// equivalence is D1's and it is about the written form. `emit` makes exactly
/// this shape Rust's `loop`, which is `!` and fits any declared type
/// ([ADR-085](../../../docs/specification/adr/adr-085.md)) — so the two halves
/// agree by construction, and the checker could not have claimed this before
/// that record, because `while true { }` is `()` below and the refusal would
/// only have moved to `rustc`.
///
/// **And no `break` bound to this loop**, which is what
/// [ADR-084](../../../docs/specification/adr/adr-084.md) added to the question:
/// before it, the condition was the whole test. The walk over-approximates — it
/// descends into a lambda, where a `break` is not bound to this loop at all —
/// and that is the safe direction: a false *"a jump leaves it"* asks for the
/// `return` this record removes, which is where every program already is, while
/// a false *"nothing leaves it"* would let a body fall off its end and hand
/// `rustc` a file nobody wrote.
fn never_ends(body: &Block) -> bool {
    let Some(last) = body.stmts.last() else {
        return false;
    };
    let Stmt::While { cond, body } = &last.node else {
        return false;
    };
    matches!(cond, Expr::LitBool(true)) && !a_jump_leaves(body, 0)
}

/// Whether a `break` in this block is bound to the loop **around** it rather
/// than to one written inside.
///
/// A `continue` is not one: it starts the loop's next turn and never leaves.
fn a_jump_leaves(block: &Block, loops: usize) -> bool {
    block.stmts.iter().any(|stmt| match &stmt.node {
        Stmt::Break => loops == 0,
        Stmt::For { body, .. } | Stmt::While { body, .. } => a_jump_leaves(body, loops + 1),
        other => {
            let mut inner: Vec<&Block> = Vec::new();
            crate::contracts::sync::visit_stmt_blocks(other, &mut |b| inner.push(b));
            inner.iter().any(|b| a_jump_leaves(b, loops))
        }
    })
}

/// **What a `?.` reaches**, which Part I 3.5 calls a *member*
/// ([ADR-066](../../../docs/specification/adr/adr-066.md)).
///
/// The two spellings are one operator, and the only place the difference has to
/// be said out loud is the refusal: a reader who wrote a call is owed `.m(…)`
/// as the way out and not `.m`.
enum Reached<'a> {
    Field(&'a str),
    Method(&'a str),
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
                    // In **declaration order**, because that is what a type
                    // argument's position means: `Pair[i64]`'s `i64` is the
                    // first parameter and nothing else says which
                    // ([ADR-074](../../docs/specification/adr/adr-074.md) D2).
                    let order: Vec<String> = generics
                        .iter()
                        .map(|g| self.parsed.text(g.name).to_string())
                        .collect();
                    let parameters: BTreeSet<String> = order.iter().cloned().collect();
                    for f in fields {
                        let name = self.parsed.text(f.name).to_string();
                        self.nameable(&name, &f.span, "a field");
                    }
                    let fields: Vec<FieldContract> = fields
                        .iter()
                        .map(|f| FieldContract {
                            name: self.parsed.text(f.name).to_string(),
                            ty: self.declared(&f.ty, &f.span).parameterise(&parameters),
                            public: f.is_public,
                        })
                        .collect();
                    let own = self.parsed.text(*name).to_string();
                    // **An item's own name was never asked**, which `not_self`
                    // did not have to be: `struct self` cannot parse, because
                    // the receiver takes the word. `struct crate` parses fine
                    // and went to the language below (ADR-076 D3).
                    self.nameable(&own, &item.span, "a struct");
                    if !order.is_empty() {
                        self.struct_parameters.insert(own.clone(), order);
                    }
                    self.structs.insert(own, fields);
                }
                Item::Enum { name, variants, .. } => {
                    let own = self.parsed.text(*name).to_string();
                    self.nameable(&own, &item.span, "an enum");
                    let named: Vec<String> = variants
                        .iter()
                        .map(|v| self.parsed.text(v.name).to_string())
                        .collect();
                    for name in &named {
                        self.nameable(name, &item.span, "a variant");
                    }
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
        // **A frame under every body, filled before any of them is walked**
        // ([ADR-097](../../../docs/specification/adr/adr-097.md)). A `comptime`
        // at item level is in scope for the whole file, and this checker's
        // scope is a stack pushed per function — so the name has to be there
        // before the first `fn` is entered, and it has to be there for a
        // function declared *above* the constant as much as below it. That is
        // the whole of why this is a pass and not an arm.
        self.item_constants();
        for item in &self.parsed.program.items {
            match &item.node {
                // Walked by `item_constants` above, in a frame that stays.
                Item::Comptime { .. } => {}
                Item::Fn { .. } => self.function(&item.node, None),
                Item::Impl {
                    target, methods, ..
                } => {
                    // `impl Stack[T]` puts `T` in scope for every method in it,
                    // exactly as the ledger reads it (`Ledger::of`), so a method
                    // body sees the same names its signature was recorded with -
                    // one rule, called from both (`contracts::impl_parameters`).
                    let declared = crate::contracts::declared_types(self.parsed);
                    // An `impl`'s own parameters carry no bounds: the head
                    // writes `impl Stack[T]` and there is nowhere in it for a
                    // `: Summarize` to stand
                    // ([ADR-078](../../docs/specification/adr/adr-078.md) §4).
                    let outer: BTreeMap<String, Vec<String>> =
                        crate::contracts::impl_parameters(self.parsed, target, &declared)
                            .into_iter()
                            .map(|name| (name, Vec::new()))
                            .collect();
                    let target = self.parsed.text(target.name).to_string();
                    for method in methods {
                        self.enclosing = outer.clone();
                        self.function(&method.node, Some(&target));
                    }
                    self.enclosing = BTreeMap::new();
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
                let mut frame = Vec::new();
                self.bindings_of(&alt.pattern.node, &mut frame);
                // **A fold's lambdas first, and in the pattern's own frame**
                // ([ADR-092](../../../docs/specification/adr/adr-092.md)),
                // because a fold may stand beside a binding in a sequence:
                // `head:N rest:fold(N, zero, fn(acc, m) { acc + m + head })`
                // parses, and without this `head` is refused. And outside
                // `expected`, which is the rule's declared type - what a fold's
                // `init` and `step` build is the parser backend's arithmetic on
                // the way to that type, not the type itself.
                self.scope.push(frame.iter().map(Local::again).collect());
                self.folds_in(&alt.pattern.node, &alt.pattern.span);
                self.scope.pop();
                let Some(action) = &alt.action else { continue };
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

    /// **A `fold`'s `init`, `step` and `merge`, walked like any other code**
    /// ([ADR-092](../../../docs/specification/adr/adr-092.md)).
    ///
    /// They are expressions inside a **pattern**, and the walk above takes a
    /// rule's *action block* and nothing else - so
    /// `fn(acc, m) { nothing_declares_this }` lowered without a word and the
    /// message was `rustc`'s about a file nobody wrote (Part III, C.1).
    ///
    /// [ADR-084](../../../docs/specification/adr/adr-084.md) D4 closed the half
    /// a **jump** can reach and deliberately no more, with its own walk of
    /// these same expressions. This subsumes that walk rather than standing
    /// beside it: [`Expr::Closure`]'s arm already crosses a boundary and counts
    /// loops from zero, so a jump here meets `NK1132` through the path every
    /// other lambda's does - and gets the better of the two messages, naming
    /// the lambda instead of saying there is no loop.
    fn folds_in(&mut self, pattern: &ast::Pattern, span: &Span) {
        match pattern {
            ast::Pattern::Fold(spec) => {
                for part in [Some(&spec.init), Some(&spec.step), spec.merge.as_ref()]
                    .into_iter()
                    .flatten()
                {
                    self.expr(part, span);
                }
            }
            ast::Pattern::Seq(parts) | ast::Pattern::Choice(parts) => {
                for part in parts {
                    self.folds_in(&part.node, &part.span);
                }
            }
            ast::Pattern::Bind { pat, .. }
            | ast::Pattern::Repeat { pat, .. }
            | ast::Pattern::Group(pat) => self.folds_in(&pat.node, &pat.span),
            ast::Pattern::Ref { args, .. } => {
                for arg in args {
                    self.folds_in(&arg.node, &arg.span);
                }
            }
            ast::Pattern::Literal(_) | ast::Pattern::Cut => {}
        }
    }

    /// Every `name:pattern` in a pattern, all of them `?`.
    fn bindings_of(&self, pattern: &ast::Pattern, out: &mut Vec<Local>) {
        match pattern {
            ast::Pattern::Bind { name, pat } => {
                out.push(Local::free(
                    self.parsed.text(*name).to_string(),
                    Ty::Unknown,
                ));
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
        // A function's own name, for the reason the struct's is asked: `fn self`
        // cannot parse and `fn crate` can (ADR-076 D3). The span is the body's
        // first statement where there is one, which is the nearest this walk has
        // - a declaration with a span of its own is `FnArg`'s and not the item's.
        if let Some(span) = body.stmts.first().map(|s| s.span.clone()) {
            self.nameable(&own_name.clone(), &span, "a function");
        }
        let key = match target {
            Some(target) => format!("{target}::{own_name}"),
            None => own_name,
        };
        let outer_current = self.current.replace(key);

        // **Inside its own body a type parameter is a type**
        // ([ADR-074](../../docs/specification/adr/adr-074.md) D1). `T` is not a
        // hole here: the caller picked it, this body did not, and a body that
        // put an `i64` where its caller's `T` is wanted would be wrong. So the
        // name stays a name and `fits` compares it, which is what lets `NK1126`
        // below say that nothing describes what a `T` can do. Only at a *call*
        // is it a variable (`bindings`), and that is the whole of the split.
        let mut declared: BTreeMap<String, Vec<String>> = self.enclosing.clone();
        declared.extend(generics.iter().map(|g| {
            (
                self.parsed.text(g.name).to_string(),
                g.bounds
                    .iter()
                    .map(|b| self.parsed.text(*b).to_string())
                    .collect(),
            )
        }));
        // `Self` stands for the type the `impl` is on, and nothing here
        // resolves it. Unlike `T` it is bound by no call site either, so it
        // stays erased: a comparison against it could only be a false positive.
        let parameters: BTreeSet<String> = ["Self".to_string()].into_iter().collect();
        let outer_declared = std::mem::replace(&mut self.type_parameters, declared);

        let outer_borrowing = std::mem::replace(
            &mut self.borrowing_self,
            receiver.as_ref().is_some_and(|r| r.is_ref),
        );

        let mut frame: Vec<Local> = Vec::new();
        if let Some(receiver) = receiver {
            let ty = match target {
                Some(target) if receiver.is_ref => Ty::view(target),
                Some(target) => Ty::named(target),
                None => Ty::Unknown,
            };
            frame.push(Local::free("self".to_string(), ty));
        }
        for arg in args {
            let name = self.parsed.text(arg.name).to_string();
            self.nameable(&name, &arg.span, "a parameter");
            frame.push(Local {
                name,
                ty: self.declared(&arg.ty, &arg.span).erase(&parameters),
                constant: None,
                // **D3**: without the word, a body that changes this parameter
                // is `NK1138`.
                immutable: (!arg.mutable).then(|| Immutable {
                    at: arg.span.clone(),
                    kind: Kind::Parameter,
                }),
            });
        }
        // **An option is a parameter** (Part I 5.1): it stands after the `;`,
        // it is named at the call rather than passed by position, and it has a
        // default - and none of that changes that the body may use it. It was
        // missing from this frame, which nothing noticed while an undeclared
        // name was only refused in statement position: measured on
        // `examples/tally.nika`, whose `f"{lines}{separator}{blank}"` names one.
        for option in config {
            frame.push(Local::free(
                self.parsed.text(option.name).to_string(),
                // Not `declared`: an option has no span of its own, and a
                // caret on the wrong line is worse than no message. Its default
                // is a literal (Part I 5.1), so a hull cannot stand here anyway.
                Ty::from_ast(self.parsed, &option.ty).erase(&parameters),
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
        // answers to the declared type exactly as a `return` does - unless the
        // body **cannot get there**
        // ([ADR-093](../../../docs/specification/adr/adr-093.md)).
        let ends = !never_ends(body);
        if let (Some(expected), Some(span)) = (&expected, tail_span.filter(|_| ends)) {
            self.expect(&tail, expected, span, "returns", |found, want| {
                format!("this function hands back `{found}`, and it declares `{want}`")
            });
        }

        // **`NK2101`, once the whole body has been seen.** The question is what
        // comes *after* a `spawn`, and a single pass reaches a later statement
        // later - so it is asked here rather than at the `spawn`.
        // **`NK2702` first, because it borrows what the next one takes**
        // ([ADR-105](../../docs/specification/adr/adr-105.md) D2). A second walk
        // is a later statement and a single pass reaches one later, so both
        // questions are asked here; `a_task_took_what_is_used_again` empties
        // `read_at`, and this one reads it.
        self.a_sequence_was_walked_twice();
        self.a_task_took_what_is_used_again();

        self.expected = outer;
        self.throwing = outer_throwing;
        self.type_parameters = outer_declared;
        self.borrowing_self = outer_borrowing;
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
            || self.modules.contains(&name)
            // A grammar's name stands where a callee stands (ADR-082 D1), so
            // it is declared in exactly the way a module is.
            || self.grammars.contains_key(&name);
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
            help: Some(match a_word_that_was_reserved(&name) {
                // **What each word used to be told a reserved one, this tells a
                // stray one** ([ADR-117](../../docs/specification/adr/adr-117.md)
                // D2). Reserving a word buys exactly one thing, which is the
                // sentence a reader who writes it gets; the four that left the
                // list were paying for nothing, because this message can say it
                // about a name.
                Some(instead) => instead.to_string(),
                None => format!(
                    "if `{name}` is meant to be a value, declare it with `let`; if it is meant \
                     to be a keyword, this language has no such keyword - and a number is \
                     written in digits with no separators, so `1_000` is `1` beside the name \
                     `_000` (Part I, 2.2)"
                ),
            }),
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
    ///
    /// `member` is the field or the method, and `Member` says which: the way out
    /// is the plain `.`, and a reader is owed it in the spelling they wrote
    /// ([ADR-066](../../../docs/specification/adr/adr-066.md)).
    fn reaches_through_a_plain_value(&mut self, on: &Ty, member: Reached<'_>, span: &Span) {
        if on.is_unknown() {
            return;
        }
        let (what, plain) = match member {
            Reached::Field(name) => ("field", format!(".{name}")),
            Reached::Method(name) => ("method", format!(".{name}(…)")),
        };
        self.checked.findings.push(Finding {
            severity: Severity::Error,
            span: span.clone(),
            code: "NK1121",
            message: format!("`?.` reaches through a `{on}`, which cannot be absent"),
            notes: vec![format!(
                "`?.` exists for a nullable type - it reaches the {what} only where there \
                 is something to reach it on, and answers `null` otherwise (Part I, 3.5). \
                 A type that is not `T?` always has a value"
            )],
            help: Some(format!("write `{plain}`")),
        });
    }

    /// **Part I 2.3: a `T?` is a type of its own, and `.` is not one of its
    /// members** ([ADR-066](../../../docs/specification/adr/adr-066.md) D6).
    ///
    /// `NK1125`, and it is [`Checker::reaches_through_a_plain_value`] the other
    /// way round: that one refuses a `?.` where there is nothing to reach
    /// through, this one refuses a plain `.` where there is.
    ///
    /// **What the other languages do here is crash.** In a language where
    /// `null` inhabits every reference type, `a?.b.c` guards `a` and leaves
    /// `a.b` unguarded, so a `null` there is a `NullReferenceException` at run
    /// time. This language has no such value to crash on: types are
    /// non-nullable by default and `T?` is a **separate type**, so `.c` on one
    /// is a member the type does not have — the same kind of mistake as `.c` on
    /// an `i64`, and answerable where it is written.
    ///
    /// So the short-circuit is unchanged and is not what this is about: `?.`
    /// still stops at its own member and still answers `null`. What changes is
    /// that the *next* access is refused rather than emitted, which is what
    /// `find(1)?.b.c` used to become —
    /// `find(1).map(|it| it.b).c`, a field read off an `Option` (Part III C.1).
    fn reaches_into_a_nullable(&mut self, on: &Ty, member: Reached<'_>, span: &Span) {
        let (what, safe) = match member {
            Reached::Field(name) => ("field", format!("?.{name}")),
            Reached::Method(name) => ("method", format!("?.{name}(…)")),
        };
        self.checked.findings.push(Finding {
            severity: Severity::Error,
            span: span.clone(),
            code: "NK1125",
            message: format!("`{on}` may be absent, so it has no {what} to reach"),
            notes: vec![
                "a `T?` is a type of its own and not a `T` that might be missing \
                 (Part I, 2.3), so a member of `T` is not a member of it - which is \
                 why there is no null reference to fail on here"
                    .to_string(),
            ],
            help: Some(format!(
                "write `{safe}`, which answers `null` where there is nothing to reach \
                 on - or end the chain with `??` and reach into the value it gives"
            )),
        });
    }

    /// **`NK1126`: a member reached on a type parameter, which has no bound.**
    ///
    /// `fn shout[T](x: T) -> String { return x.to_uppercase() }` is the program.
    /// It has a `T`, the `T` has no bound, and a value of it can therefore be
    /// moved and passed and nothing else - so `to_uppercase` is not a member of
    /// it, in the same way that it is not a member of an `i64`.
    ///
    /// **This is the refusal that makes writing the `<T>` worth anything**
    /// ([ADR-074](../../docs/specification/adr/adr-074.md) D5). Emitting the
    /// parameter without it turns *"every generic function fails in `rustc`"*
    /// into *"every generic function whose body uses its parameter fails in
    /// `rustc`"* - the same [Part III C.1](../../docs/specification/30-nikaia-tooling.md)
    /// class one size smaller, because the message is still `rustc`'s, about a
    /// file nobody wrote, saying `T` in the current scope.
    ///
    /// Why it is not a warning: the alternative is a program this compiler
    /// accepts and the backend refuses, which is the state the whole of
    /// Part III C is about. And why it says *"no bound"* rather than *"no such
    /// method"*: the method may well exist on every type the caller will ever
    /// pass, and what is missing is the sentence that says so.
    /// The **bound** that answers a member reached on a type parameter, if one
    /// does ([ADR-078](../../docs/specification/adr/adr-078.md) D3).
    ///
    /// `fn shout[T: Summarize](x: T)` and `x.summary()`: `T`'s bound names
    /// `Summarize`, the trait declares `summary`, and the ledger records that
    /// signature under `Summarize::summary` — the same key shape an `impl`'s
    /// methods get, because a bound and a receiver ask one question. So this
    /// hands back a **type to look the call up on**, and the whole of the rest
    /// of a call is unchanged: arity, argument types, `sync`, `throws` and the
    /// binding of the signature's variables all happen exactly as they do for a
    /// receiver whose type was written down.
    ///
    /// The first bound that declares the member wins, and `[T: A + B]` where
    /// both declare it is not a question this can answer — Rust's own answer is
    /// that the call is ambiguous, and nothing here can write the
    /// disambiguation. That is §4's, not this.
    fn bound_that_answers(&self, parameter: &str, member: &str) -> Option<Ty> {
        let bounds = self.type_parameters.get(parameter)?;
        bounds
            .iter()
            .find(|bound| {
                self.own
                    .traits
                    .get(bound.as_str())
                    .is_some_and(|methods| methods.contains(member))
            })
            .map(Ty::named)
    }

    fn nothing_says_what_a_parameter_can_do(
        &mut self,
        parameter: &str,
        member: Reached<'_>,
        span: &Span,
    ) {
        if !self.type_parameters.contains_key(parameter) {
            return;
        }
        let (what, name) = match member {
            Reached::Field(name) => ("field", name),
            Reached::Method(name) => ("method", name),
        };
        self.checked.findings.push(Finding {
            severity: Severity::Error,
            span: span.clone(),
            code: "NK1126",
            message: format!(
                "`{parameter}` stands for a type the caller picks, and nothing says it has a \
                 {what} `{name}`"
            ),
            notes: vec![
                "a type parameter with no bound can be moved and passed and nothing \
                 else, because every type the caller may pick has to answer for what \
                 the body does (Part I, 4.6)"
                    .to_string(),
            ],
            help: Some(format!(
                "write the type the value actually has, or take `{parameter}` out and \
                 declare the parameter as that type - a bound that says which types \
                 `{parameter}` may be is not built yet"
            )),
        });
    }

    /// **`NK1131`: a field of a borrowed subject, handed out by value.**
    ///
    /// ```nika
    /// impl User {
    ///     fn name_of(&self) -> String {
    ///         return self.username     // error[NK1131]
    ///     }
    /// }
    /// ```
    ///
    /// `&self` borrows the subject, so what the body has is a loan of it; giving
    /// the `username` away by value would take a piece out of something it does
    /// not own. The language below says
    /// *"cannot move out of `self.username` which is behind a shared reference"*
    /// about a file nobody wrote — and
    /// [Part I 6.8](../../docs/specification/10-nikaia-light.md) is what decides
    /// that this is a refusal rather than something to paper over: *ownership
    /// rules occasionally reject code, every such error explains itself in plain
    /// language, and a raw internal error reaching you is a Nikaia bug.*
    ///
    /// **Not a hidden `.clone()`**, which was the other option and is against a
    /// decision already made: [ADR-064](../../docs/specification/adr/adr-064.md)
    /// D2 wrote *a hull you can see is one you write*, and a copy the author
    /// cannot see is the same thing one position over. Both ways out already
    /// exist and both are one word — `.clone()`, written where it happens, or a
    /// `self` receiver where the method is meant to consume its subject.
    ///
    /// **Asked only where the field's type is known and does not copy.** A
    /// number, a `bool`, a `char` and a view copy, so handing one out takes
    /// nothing away; a field this compiler cannot type says nothing, and
    /// [Part III C.4](../../docs/specification/30-nikaia-tooling.md) is why that
    /// is silence rather than a guess.
    /// The ledger key `Json.value(input)` enters through, where the receiver
    /// names a grammar of this file and the method one of its `pub` rules
    /// ([ADR-082](../../docs/specification/adr/adr-082.md) D1, D2).
    ///
    /// `None` for everything else, which is every other method call: a grammar
    /// name is not a value, so there is nothing to confuse this with.
    fn grammar_entry(&self, receiver: &Expr, method: Ident) -> Option<String> {
        let Expr::Variable(name) = receiver else {
            return None;
        };
        let grammar = self.parsed.text(*name).to_string();
        let rule = self.parsed.text(method).to_string();
        self.grammars
            .get(&grammar)
            .filter(|rules| rules.contains(&rule))
            .map(|_| format!("{grammar}::{rule}"))
    }

    /// The same key off a **path**, which is how a grammar is entered
    /// ([ADR-140](../../docs/specification/adr/adr-140.md) D3): `Json::value(x)`
    /// names the grammar `Json` and its `pub` rule `value`.
    ///
    /// A grammar's name is a name and a rule of it is reached the way every
    /// other qualified name is. The dot is for a **value's** members, and a
    /// namespace behind one was the single place this language asked a reader
    /// to tell two things apart by what the left side happens to be.
    fn grammar_path(&self, name: &str) -> Option<String> {
        let (grammar, rule) = name.split_once("::")?;
        self.grammars
            .get(grammar)
            .filter(|rules| rules.contains(rule))
            .map(|_| name.to_string())
    }

    /// `NK1147`: a grammar's rule reached through a dot
    /// ([ADR-140](../../docs/specification/adr/adr-140.md) D3).
    ///
    /// Raised only where the receiver **is** a grammar of this file and the
    /// name **is** one of its rules, so the message can carry the whole
    /// rewrite — and so that a method call on an ordinary value named like a
    /// grammar is untouched.
    fn a_grammar_reached_through_a_dot(&mut self, grammar: &str, rule: &str, span: &Span) {
        self.checked.findings.push(Finding {
            severity: Severity::Error,
            span: span.clone(),
            code: "NK1147",
            message: format!("a rule of grammar `{grammar}` is reached with `::`, not with a dot"),
            notes: vec![
                "a grammar's name is a name, and a rule of it is a qualified name like \
                 every other (ADR-140 D3). The dot is for a **value's** members, and a \
                 namespace behind one was the single place this language asked a reader \
                 to tell two things apart by what the left side happens to be"
                    .to_string(),
            ],
            help: Some(format!("write `{grammar}::{rule}(…)`")),
        });
    }

    /// The entry call itself: the input is an expression like any other, and
    /// what comes back is what the rule declares.
    ///
    /// **Whether it can fail comes from the contract**, which is the whole of
    /// what D2 changed: a `pub` rule is an entry in the ledger with
    /// `throws = ["?"]`, because a rule past a commit point can fail
    /// ([ADR-023](../../docs/specification/adr/adr-023.md) D9). The old form
    /// was not a call and carried no contract, so `NK1134` had to be told in
    /// one line ([ADR-091](../../docs/specification/adr/adr-091.md) D4); that
    /// line is gone with it.
    fn grammar_call(&mut self, key: &str, args: &[Expr], span: &Span) -> Ty {
        for arg in args {
            self.expr(arg, span);
        }
        let fallible = self
            .own
            .functions
            .get(key)
            .is_some_and(|c| !c.throws.is_empty());
        if fallible {
            if let Some(guarded) = &mut self.guarded {
                guarded.fallible = true;
            }
        }
        self.own
            .functions
            .get(key)
            .and_then(|c| c.signature.as_ref())
            .and_then(|s| s.result.clone())
            .unwrap_or(Ty::Unknown)
    }

    /// **The compiler writes the reference at the call, and a written one is
    /// refused** ([ADR-094](../../docs/specification/adr/adr-094.md) D1).
    ///
    /// The callee's answer is `contracts::keeps::lends`, read here and read
    /// again by the emitter when it writes that callee's *declaration* — one
    /// answer in two positions, because the two disagreeing is a `&&T` or a
    /// moved value in the language below. What this adds is the half the
    /// emitter cannot see: whether the argument is **already** a view, which is
    /// a question about its type (ADR-028).
    ///
    /// **`NK1137` lands exactly where the compiler would have written one**,
    /// and nowhere else. A `&` in a position the callee keeps, or one in front
    /// of a value whose type this checker could not work out, is still the
    /// program's own and is left alone — refusing it would take away the only
    /// way to say what the line means before the inference that replaces it can
    /// answer.
    fn the_compiler_writes_the_reference(
        &mut self,
        contract: &FnContract,
        argument: Argument<'_>,
        written: &str,
        span: &Span,
    ) -> bool {
        let Argument {
            at,
            given,
            found,
            want,
        } = argument;
        // `arguments()` drops the receiver, and the contract's positions do
        // not, so a method's argument sits one further along.
        let position = at
            + usize::from(
                contract
                    .signature
                    .as_ref()
                    .is_some_and(|s| s.takes_a_receiver()),
            );
        // **`mut` is asked first, and it is a declaration** (D3): the
        // parameter lowers to `&mut T` and the argument gains a `&mut`, both
        // off the word the author wrote. `lends` withholds its claim on such a
        // position, so exactly one of the two answers.
        let changes = contract
            .signature
            .as_ref()
            .and_then(|s| s.params.get(position).map(|(name, _)| (s, name)))
            .is_some_and(|(s, name)| s.mutable.iter().any(|m| m == name));
        if changes {
            let Some(given) = given else {
                return false;
            };
            self.checked
                .mut_args
                .entry((span.start, written.to_string(), at))
                .or_default()
                .insert(argument_shape(given));
            return true;
        }
        if !crate::contracts::keeps::lends(contract, position) {
            return false;
        }
        let Some(given) = given else {
            return false;
        };
        let wrote_a_reference = matches!(
            given,
            Expr::Unary {
                op: crate::ast::UnaryOp::Ref,
                ..
            }
        );
        // **Only where the call would be right with the reference in it.** A
        // `&i64` handed to a `&Request` is `NK1102` and stays one: saying *the
        // `&` here is the compiler's* about an argument that is the wrong value
        // would send the reader to fix the punctuation of a line whose type is
        // wrong.
        //
        // The hull's path counts as a fit: a `Shared[Conn]` reaches a `&Conn`
        // through its deref ([ADR-040](../../docs/specification/adr/adr-040.md)
        // D5), and that fit was the *source's* `&` before this.
        //
        // **Which fit to ask depends on which of the two kinds this is.** A
        // parameter written `&str` is a view in the declaration already, so
        // what has to fit it is the argument *with* the reference the compiler
        // writes — a `String` becomes a `&String` and reaches `&str` the way
        // the language below reaches it. A parameter written `Stats` and
        // inferred lent gains its `&` in the declaration too, so both sides
        // move together and the fit is the one it always was.
        let fits = match want.is_a_view() {
            true => {
                // **`view_of` and not a `&` pasted on the front**, because
                // a view of a `String` is a `&str` — the one rewrite Part I 6.5
                // makes, and the same one this checker performs for a `&`
                // somebody writes. Keeping the name refused `count(dna)` for a
                // `dna` every caller had been writing `count(&dna)` for.
                let lent = view_of(found);
                lent.fits(want) || self.fits_through_deref(&lent, want)
            }
            false => {
                found.fits(want)
                    || self.fits_through_deref(found, want)
                    // **A `&` the source wrote is already the view**, and this
                    // parameter is not one: what has to fit `want` is what the
                    // `&` was put in front of, because that is the expression
                    // the compiler would have written its own `&` before. Asked
                    // the other way, `width(&t)` for a `text: String` that
                    // `width` only reads came back `&str` against `String` and
                    // was answered with `NK1102`'s *write `.to_string()`* — a
                    // help that sends the reader the wrong way about a line
                    // whose only fault is a character the compiler writes.
                    || (wrote_a_reference
                        && viewed_from(found).iter().any(|owned| {
                            owned.fits(want) || self.fits_through_deref(owned, want)
                        }))
            }
        };
        if !fits {
            return false;
        }
        if wrote_a_reference {
            // **A type this compiler could not work out is not one to refuse
            // punctuation over.** `Ty::Unknown` fits everything, which is the
            // right answer for an absent claim
            // ([ADR-024](../../docs/specification/adr/adr-024.md) D1) and no
            // ground to stand on here: `route(&n)` for an `n` whose type was
            // never pinned would be refused on the strength of a fit that means
            // *nothing was checked*, which is [Part III
            // C.4](../../docs/specification/30-nikaia-tooling.md)'s correct
            // program refused. Left alone, the `&` the source wrote is the same
            // character the compiler would have written, so the lowering is the
            // one either way.
            //
            // **Which is why the test sits here and not above the fit.** The
            // declaration is written off `lends` alone, so anything this
            // refuses to record is a call whose callee takes a `&T` and whose
            // argument does not have one - `rustc` about a file nobody wrote.
            // A condition that skips the *recording* may only be one the
            // emitter shares; this one skips the *refusal*, and the source's
            // own `&` stands in for what would have been recorded.
            if matches!(found, Ty::Unknown) {
                return false;
            }
            self.checked.findings.push(Finding {
                severity: Severity::Error,
                span: span.clone(),
                code: "NK1137",
                message: "the `&` here is the compiler's to write".to_string(),
                notes: vec![format!(
                    "`{written}` reads this argument rather than keeping it, so the \
                     reference is what the call already means (ADR-094 D1) - and which \
                     of the two it is comes from the callee's body, not from this line"
                )],
                help: Some("take the `&` off".to_string()),
            });
            return true;
        }
        // A value that is already a view needs nothing: the declaration and the
        // argument agree without a second `&`.
        if found.is_a_view() {
            return false;
        }
        self.checked
            .lent_args
            .entry((span.start, written.to_string(), at))
            .or_default()
            .insert(argument_shape(given));
        true
    }

    /// The name a place is **rooted** at: `out` for `out`, `out.f` and
    /// `out[i].f`, and nothing for anything that is not a place.
    ///
    /// A field of a parameter is the parameter's, which is why the whole chain
    /// is followed rather than only its last step: `row.total = 0` changes
    /// `row` ([ADR-094](../../docs/specification/adr/adr-094.md) D3).
    fn rooted_at(&self, place: &Expr) -> Option<String> {
        match place {
            Expr::Variable(name) => Some(self.parsed.text(*name).to_string()),
            Expr::Field { base, .. } | Expr::SafeField { base, .. } => self.rooted_at(base),
            Expr::Index { base, .. } => self.rooted_at(base),
            _ => None,
        }
    }

    /// The name a free call names, unaliased — or nothing where this is not one.
    fn free_callee(&self, func: &Expr) -> Option<String> {
        let name = match func {
            Expr::Variable(name) => self.parsed.text(*name).to_string(),
            Expr::Path(segments) => segments
                .iter()
                .map(|s| self.parsed.text(*s))
                .collect::<Vec<_>>()
                .join("::"),
            _ => return None,
        };
        Some(self.parsed.unaliased(&name))
    }

    /// **`NK2203`: a lock taken while a lock is held**
    /// ([ADR-039](../../docs/specification/adr/adr-039.md) D2, D3).
    ///
    /// Part II 12.3 called manual nesting an anti-pattern and *"often a
    /// compile-time error"*; D2 makes *often* into **always**, and with that no
    /// program exists in which a lock's two representations behave differently
    /// — which is the whole reason the switch may pick one.
    ///
    /// **Written one inside the other, or reached through a chain of calls**,
    /// and the second is what needs the column: `contracts::locks` propagates
    /// *touches a lock* over the call graph, so a callee three deep is the same
    /// answer as one written here.
    ///
    /// **`Holds` and nothing else.** `Undecided` is not permission
    /// ([ADR-010](../../docs/specification/adr/adr-010.md) D1) and not a
    /// refusal either, because Stage 0 knows the type of rather less than half
    /// of what a program writes, and refusing on doubt is [Part III
    /// C.4](../../docs/specification/30-nikaia-tooling.md)'s correct program
    /// refused. What a silent `Undecided` costs is the runtime check, which is
    /// where every program already is.
    ///
    /// **A `println` is one of these**, and that is the case
    /// [ADR-067](../../docs/specification/adr/adr-067.md) D1 was written about:
    /// it never pauses, so `sync` says nothing about it, and it takes standard
    /// output's own lock while yours is open. Two conditions and not one.
    fn a_lock_inside_a_lock(&mut self, called: &str, holds: crate::contracts::Lock, span: &Span) {
        if !self.inside_a_door || !holds.holds() {
            return;
        }
        self.checked.findings.push(Finding {
            severity: Severity::Error,
            span: span.clone(),
            code: "NK2203",
            message: format!("`{called}` takes a lock, and this runs with one already held"),
            notes: vec![
                "a door's block runs with the lock open, so a second one taken inside it \
                 is a lock inside a lock - which is a deadlock rather than a risk \
                 (ADR-039 D2)"
                    .to_string(),
                "what a function reaches is its own column in the ledger (13.5), so this \
                 is answered through a chain of calls as well as for one written here"
                    .to_string(),
            ],
            help: Some(
                "ask for both at once - `access_all(a, b) fn(x, y) { … }` - or compute \
                 outside the block and hand the value in"
                    .to_string(),
            ),
        });
    }

    /// **`NK1141`: an `update` block hands a value back**
    /// ([ADR-110](../../docs/specification/adr/adr-110.md) D1).
    ///
    /// `update` used to take the value by value and put back what the block
    /// returned; D1 hands it the address instead, so *there is nothing to
    /// return*. Without this the old shape — `kasse.update fn(old) { old + 1 }`
    /// — lowers to a closure whose value is an `i64` where `()` is wanted, and
    /// the answer comes from `rustc` about a file nobody wrote
    /// ([Part III C.1](../../docs/specification/30-nikaia-tooling.md)).
    ///
    /// **Asked of the shape and not of the type**, and only where the shape
    /// says so outright: a last statement that is a call, a method call or an
    /// assignment is left alone, because whether *those* come to a value is a
    /// question this walk would have to type the call to answer, and answering
    /// it wrongly refuses a correct program (C.4). What is refused is a last
    /// statement that can only be a value — a name, a literal, an operator, a
    /// field — and a `return` that carries one.
    fn an_update_block_returns_nothing(&mut self, body: &Block, span: &Span) {
        let hands_back = body.stmts.iter().any(|stmt| match &stmt.node {
            Stmt::Return(Some(_)) => true,
            Stmt::Expr(value) => {
                // The last statement is the block's value; an expression
                // statement anywhere else is evaluated and dropped.
                std::ptr::eq(stmt, body.stmts.last().expect("there is one"))
                    && plainly_a_value(value)
            }
            _ => false,
        });
        if !hands_back {
            return;
        }
        self.checked.findings.push(Finding {
            severity: Severity::Error,
            span: span.clone(),
            code: "NK1141",
            message: "an `update` changes its value; there is nothing to return".to_string(),
            notes: vec![
                "the block is handed the value where it lies and changes it in place \
                 (ADR-110 D1), so what it comes to is not put anywhere"
                    .to_string(),
            ],
            help: Some(
                "change it instead: `fn(mut v) { v += 1 }` rather than `fn(v) { v + 1 }`"
                    .to_string(),
            ),
        });
    }

    /// One lambda parameter, bound — and refusable where it is changed without
    /// `mut` ([ADR-110](../../docs/specification/adr/adr-110.md) D1).
    ///
    /// `kasse.update fn(mut v) { v += 100 }` is where the word earns its place:
    /// the block changes the value in place and the caller whose value changes
    /// is the **lock**. D1 says a block that changes `v` without the word is
    /// `NK1138`, and it says it **about a door** — which is where this asks it
    /// and nowhere else.
    ///
    /// **Held for every lambda it would refuse correct programs**, and the
    /// corpus is what said so: `par_fold(…, fn(acc, m) { acc.record(m) })` in
    /// `examples/1brc.nika` changes `acc` and has no `mut`, and it compiles,
    /// because the **emitter** writes the word itself where it recognises a
    /// fold's accumulator; `and_modify fn(tally) { tally.bump() }` in
    /// `k-nucleotide.nika` is the same shape one library over. So *a lambda
    /// parameter changed without `mut` is already broken Rust* is false, and a
    /// rule built on it would be [Part III
    /// C.4](../../docs/specification/30-nikaia-tooling.md)'s correct program
    /// refused. Widening it is a question for whoever takes that `mut` out of
    /// the emitter, and it belongs with that work rather than ahead of it.
    fn lambda_parameter(
        &mut self,
        name: winnow_grammar::Symbol,
        mutable: &[winnow_grammar::Symbol],
        ty: Ty,
        span: &Span,
    ) -> Local {
        let asked = self.at_a_write_door && !mutable.contains(&name);
        Local {
            name: self.parsed.text(name).to_string(),
            ty,
            constant: None,
            immutable: asked.then(|| Immutable {
                at: span.clone(),
                kind: Kind::Parameter,
            }),
        }
    }

    /// **What is changed says `mut`** — `NK1138` for a parameter
    /// ([ADR-094](../../docs/specification/adr/adr-094.md) D3) and `NK1139` for
    /// a `let` ([Part I 2.1](../../docs/specification/10-nikaia-light.md)).
    ///
    /// One rule reached from two sides, and two codes because the word goes in
    /// a different place and a reader is doing a different thing. A parameter's
    /// `mut` also decides what the **caller** sees — the value it hands over is
    /// the one that changes — where a `let`'s is only about this body.
    ///
    /// **Both were `rustc`'s until now.** Part I 2.1 writes
    /// `// x = 20  <-- This would cause a Compiler Error` and this compiler was
    /// not the one giving it: the binding lowered without its `mut` and the
    /// answer came back about a file nobody wrote, which is [Part III
    /// C.1](../../docs/specification/30-nikaia-tooling.md).
    ///
    /// **Only where the change is certain.** A method whose entry no ledger
    /// has, or whose candidates do not agree, is not one to refuse on;
    /// answering *it might change* would refuse a correct program, which is C.4
    /// and the worse of the two mistakes.
    fn a_changed_binding_says_mut(&mut self, name: &str, how: &str) {
        let Some(at) = self
            .binding(name)
            .and_then(|local| local.immutable.clone())
            .filter(|at| !self.said_mut.contains(&at.at.start))
        else {
            return;
        };
        let (code, what, where_the_word_goes, way_out) = match at.kind {
            Kind::Parameter => (
                "NK1138",
                "a parameter",
                "`mut` in the declaration is where in-place change is written, and \
                 the caller's value is what changes, exactly as it is for `&mut self` \
                 (ADR-094 D3)",
                // D3 names both ways out, and the second is the one that keeps
                // the caller's value as it was.
                format!(
                    "write `mut {name}` - or, if the caller's value should stay as it \
                     was, take a copy with `let mut {name}_own = {name}`"
                ),
            ),
            Kind::Let => (
                "NK1139",
                "a `let`",
                "a binding changes only where it says so (Part I, 2.1), and this one \
                 does not",
                format!("write `let mut {name}`"),
            ),
        };
        // Said once per binding: a body that changes one usually does so
        // several times, and three carets on one declaration is noise.
        self.said_mut.insert(at.at.start);
        self.checked.findings.push(Finding {
            severity: Severity::Error,
            span: at.at,
            code,
            message: format!("`{name}` is changed, and {what} that is changed says `mut`"),
            notes: vec![format!("{how} - {where_the_word_goes}")],
            help: Some(way_out),
        });
    }

    /// **`NK1137`: the `&` is the compiler's to write**
    /// ([ADR-094](../../docs/specification/adr/adr-094.md) D4).
    ///
    /// A `for` lends what it iterates and a `let` over a place is a view of it,
    /// so a `&` written in front of either says what the line already means.
    /// Left alone it would be a second reference — `&&Vec<Entry>`, which Rust
    /// does not iterate — and reported about a file nobody wrote
    /// ([Part III C.1](../../docs/specification/30-nikaia-tooling.md)).
    ///
    /// **Refused rather than absorbed**, which is D1's rule and is the whole
    /// point of the record: two spellings for one thing is the state a reader
    /// cannot tell a rule from a habit in. A `&` in a **declaration** is
    /// untouched (D6) — that is where it lives.
    ///
    /// **The `for` position only, and the `let` one is open work.** A `for`
    /// lends whatever place it is given, so a written `&` there is always the
    /// compiler's line said twice. A `let` lends only where the checker could
    /// *type* the place ([`Checked::lent_lets`]), and where it could not, the
    /// written `&` is the program's only way to say what the line means — so
    /// refusing it there would take away the escape hatch before the inference
    /// that replaces it exists. `open-work.md` carries that as the rest of D4.
    fn the_caller_writes_no_reference(&mut self, value: &Expr, span: &Span, because: &str) {
        if !matches!(
            value,
            Expr::Unary {
                op: crate::ast::UnaryOp::Ref,
                ..
            }
        ) {
            return;
        }
        self.checked.findings.push(Finding {
            severity: Severity::Error,
            span: span.clone(),
            code: "NK1137",
            message: "the `&` here is the compiler's to write".to_string(),
            notes: vec![format!(
                "{because}, so the reference is already what this line means                  (ADR-094 D4) - written twice it is a reference to a reference,                  which the language below reports about a file nobody wrote"
            )],
            help: Some("take the `&` off".to_string()),
        });
    }

    fn a_field_of_a_borrowed_subject(&mut self, value: &Expr, span: &Span, what: &str) {
        if !self.borrowing_self {
            return;
        }
        let Expr::Field { base, name } = value else {
            return;
        };
        let Expr::Variable(subject) = base.as_ref() else {
            return;
        };
        if self.parsed.text(*subject) != "self" {
            return;
        }
        let field = self.parsed.text(*name).to_string();
        let ty = self.expr(value, span);
        if ty.is_unknown() || copies(&ty) {
            return;
        }
        self.checked.findings.push(Finding {
            severity: Severity::Error,
            span: span.clone(),
            code: "NK1131",
            message: format!("`self` is borrowed here, so `{field}` cannot be {what} by value"),
            notes: vec![format!(
                "`&self` is a loan of the subject, and `{field}` is a `{}` - giving it \
                 away would take a piece out of something this method does not own \
                 (Part I, 6.5)",
                ty.text()
            )],
            help: Some(format!(
                "hand back a view and it costs nothing: declare the result `{}` and \
                 write `return &self.{field}` (Part I, 6.5). Or `self.{field}.clone()` \
                 for a copy, or `fn …(self)` where the method is meant to consume its \
                 subject",
                a_view_of(&ty)
            )),
        });
    }

    /// **What a method call is**, asked of a receiver whose type is already in
    /// hand.
    ///
    /// One function and not two, because there are two ways to write the call
    /// and only one thing a call *is*: `x.m()` and `x?.m()` differ in whether
    /// the call happens, never in what it throws, whether it pauses, what its
    /// arguments have to be, or what the receiver's type binds in its signature
    /// ([ADR-066](../../../docs/specification/adr/adr-066.md)). Two copies of
    /// these rules would be that many chances for the two spellings to drift.
    ///
    /// `on` is the receiver's type - for a `?.` the type **inside** the `T?`,
    /// which is the whole of what that operator changes here.
    ///
    /// `entry` is the name the **ledger** knows this call by, which is the
    /// written one everywhere but at
    /// [ADR-111](../../docs/specification/adr/adr-111.md) D5's witness door:
    /// `kasse.set(neu; after: stand)` is `set(after)`, a different operation
    /// from `set` and a different entry. `method` stays the name the *program*
    /// wrote, and everything the **emitter** is handed stays keyed by that —
    /// an argument it has to put a `&` in front of is found under `set`,
    /// because `set` is what is written on the line.
    fn call_on(&mut self, on: Ty, method: Ident, args: &[Expr], entry: &str, span: &Span) -> Ty {
        // **A produced sequence answers under its own word**
        // ([ADR-105](../../docs/specification/adr/adr-105.md) D1): `Seq` and
        // `Par` are what the ledger calls the receiver, so `Seq::collect` is
        // found exactly as `Vec::push` is. A `Par[T]` falls back to `Seq`'s
        // entries where it has none of its own, which is D3's *otherwise
        // `Par[T]` has `Seq[T]`'s surface* - one sentence rather than a second
        // copy of twelve entries.
        let named;
        let name = match &on {
            Ty::Seq { parallel, .. } => {
                named = match parallel {
                    true => ty::PAR,
                    false => ty::SEQ,
                };
                Some(named)
            }
            Ty::Named { name, .. } => Some(name.as_str()),
            _ => None,
        };
        let Some(name) = name else {
            // The receiver's type is not known, so neither is what this
            // calls. Recorded, because "I could not find out" is an
            // answer somebody downstream has to act on.
            args.iter().for_each(|a| {
                self.expr(a, span);
            });
            self.reached_method(None);
            self.method_propagates(method, false, span);
            self.method_pauses(method, false, span);
            return Ty::Unknown;
        };
        let key = format!("{name}::{entry}");
        let found = self.method(&key).or_else(|| match &on {
            // D3's *otherwise `Par[T]` has `Seq[T]`'s surface*.
            Ty::Seq { parallel: true, .. } => self.method(&format!("{}::{entry}", ty::SEQ)),
            _ => None,
        });
        let Some((key, contract)) = found else {
            // The type is known and no ledger describes this method of
            // it - `HashMap::entry` until something writes it down.
            // **A bound is looked up before anything is refused**
            // ([ADR-078](../../docs/specification/adr/adr-078.md) D3): where
            // `T: Summarize` and the trait declares this method, the call is
            // asked again on the trait and everything about it - arity,
            // argument types, `sync`, `throws` - is answered from the
            // declaration, exactly as it would be from a written-down receiver.
            if let Some(bound) = self.bound_that_answers(name, entry) {
                return self.call_on(bound, method, args, entry, span);
            }
            // Unless the type is a **parameter**, where nothing will ever
            // describe it and saying so now is the whole of `NK1126`.
            self.nothing_says_what_a_parameter_can_do(name, Reached::Method(entry), span);
            args.iter().for_each(|a| {
                self.expr(a, span);
            });
            self.reached_method(None);
            self.method_pauses(method, false, span);
            self.method_propagates(method, false, span);
            return Ty::Unknown;
        };
        self.reached_method(Some(&key));
        // **A method that walks a produced sequence consumes it**
        // ([ADR-105](../../docs/specification/adr/adr-105.md) D2): every `Seq`
        // entry writes its receiver `(Seq[$T], …)` and not `&Seq[$T]`, so the
        // signature is what says so rather than a list of method names. A
        // container's methods take a view and are untouched.
        if matches!(&on, Ty::Seq { .. }) && walks_by_value(contract) {
            if let Some(name) = self.receiver_name.clone() {
                self.walked.push((name, on.clone(), span.end));
            }
        }
        // ADR-023 D8: the failure leaves at the call, and the emitter
        // is what writes that. Recorded whether or not the function
        // around it declares `throws` - where it does not, `NK2605`
        // below refuses the program and nothing is emitted at all.
        self.method_options(method, contract, span);
        self.method_propagates(method, !contract.throws.is_empty(), span);
        // ADR-055 D2, the method half. Either ledger since §6 step 3
        // made `std`'s own pausing entries `async fn`: before it, a
        // `std` entry blocked its thread and awaiting one would have
        // been awaiting a value rather than a future.
        self.method_pauses(method, !contract.sync.is_sync(), span);
        self.a_call_that_may_pause(contract);
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
        let found =
            self.arguments_given(args, &expected, self.own.functions.contains_key(&key), span);
        self.a_set_that_reads_what_it_writes(&on, entry, &found, span);
        // **The source's own name and not the ledger's**, because what
        // `arguments` records is read back by the emitter off the line as it is
        // written: a `&` the compiler owes the witness is looked up under
        // `set`, which is the word on the page.
        let written = self.parsed.text(method).to_string();
        let result = self.arguments(&key, &written, contract, args, &found, &[], span);
        // The receiver first (ADR-031), then whatever the arguments can still
        // say (ADR-074 D2) - `or_insert` on a map that bound `$V` already has
        // its answer, and `bind` does not overwrite one.
        let mut bound = bound;
        for (name, ty) in from_arguments(contract, &found) {
            bound.entry(name).or_insert(ty);
        }
        let result = ty::substitute(&result, &bound);
        self.stamped_through(contract, &found, result)
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
    /// **Which of the two**, and the answer used to be *"one, or nothing"*
    /// ([ADR-068](../../../docs/specification/adr/adr-068.md)).
    ///
    /// The constructor may only be written where the value is known **not** to
    /// be a `T?` already, because wrapping one that is would make an
    /// `Option<Option<T>>`. Two ways it can be known, and a type is only the
    /// first:
    ///
    /// * its type says so — anything this checker worked out that is not a
    ///   `T?`; or
    /// * **it is a literal**, which no literal ever is. That second one is not
    ///   a convenience: a number has no type of its own on purpose (Part I 2.4,
    ///   so that `add(3)` is right wherever the parameter is numeric), so
    ///   `return 42` against a declared `i64?` answers `Unknown` and the type
    ///   alone would leave the commonest case in the section unwrapped.
    ///
    /// **And everything else takes the conversion**, which is what changed:
    /// `value.into()` is correct whether the value is a `T` or already a `T?`,
    /// so a type this checker could not work out stops being a position it has
    /// to stay silent about. It used to be left alone — right about the risk,
    /// and the program then failed in the language below with `rustc`'s *"try
    /// wrapping the expression in `Some`"* about a form Nikaia does not have
    /// (Part III C.1).
    ///
    /// `null` is in neither, being a `T?` itself.
    fn wraps_into_nullable(&mut self, found: &Ty, want: &Ty, value: &Expr, span: &Span) {
        let Some(how) = wrap_for(found, want, is_literal(value)) else {
            return;
        };
        self.checked.nullable_sites.insert(span.start, how);
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
        let ty = match named.or(folded.pinned) {
            Some(ty) => ty,
            // **Nothing beside it and nothing pinning it**, which is the
            // constant that decides its own type
            // ([ADR-063](../../docs/specification/adr/adr-063.md) D1): it takes
            // the first that holds it, so the only thing left to report here is
            // a value no type holds at all. Reported against the wider one,
            // because that is the one it fell out of.
            None if i64::try_from(folded.value).is_err() => "i64".to_string(),
            None => return,
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
    /// **The arithmetic is [`crate::fold`]'s**, and what this adds is the half
    /// the emitter cannot have: a name resolved to what it is worth
    /// ([ADR-063](../../docs/specification/adr/adr-063.md) D2). Only an
    /// **immutable** `let` carries a value forward to be found here, which
    /// `bind_with` decides; a declaration on the way pins the type, and that is
    /// what makes `let a: i32 = 2` then `a + a` arithmetic in an `i32`.
    fn constant_of(&self, expr: &Expr) -> Option<Constant> {
        crate::fold::constant_of(expr, &|name| {
            let (ty, constant) = self.local(self.parsed.text(name))?;
            let value = constant?;
            Some(Constant {
                // **A name pins, and a literal does not**
                // ([ADR-063](../../docs/specification/adr/adr-063.md) D2). A
                // declaration pins what it says; a bare `let` pins the type its
                // own value took, which is the first that holds it - so
                // `let a = 2000000000` is an `i32` and `a + a` is arithmetic in
                // one, the same as in every language that has both widths. The
                // way out is one word: `let a: i64 = …`.
                pinned: integer_named(&ty).or_else(|| {
                    Some(match i32::try_from(value) {
                        Ok(_) => "i32".to_string(),
                        Err(_) => "i64".to_string(),
                    })
                }),
                value,
            })
        })
    }

    /// `self` is a reserved word, and this is the one position the grammar
    /// cannot refuse it in ([ADR-051](../../docs/specification/adr/adr-051.md)).
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
    /// The four words the **language below** reserves and cannot escape.
    ///
    /// [ADR-076](../../docs/specification/adr/adr-076.md) D1 escapes every other
    /// one — `type` becomes `r#type` and the Nikaia name stays legal. These four
    /// have no escape at all: *"`crate` cannot be a raw identifier"* is Rust's
    /// own answer to `r#crate`, and the same for `super`, `self` and `Self`. So
    /// the one rule has one forced exception, and it is the target's rather than
    /// this language's.
    ///
    /// `self` is not here because it is already refused by `NK1119`, which says
    /// more: it is a reserved word of *this* language, with a reason of its own.
    const UNESCAPABLE_BELOW: &'static [&'static str] = &["crate", "super", "Self"];

    /// Every question about whether a name may be one, in one place.
    ///
    /// Two rules with the same six call sites, kept together so that adding a
    /// position adds it to both: `self` is this language's own reserved word
    /// (`NK1119`) and `crate`, `super` and `Self` are the language below's
    /// (`NK1128`).
    fn nameable(&mut self, name: &str, span: &Span, what: &str) {
        self.not_self(name, span, what);
        self.not_unescapable(name, span, what);
    }

    /// **`NK1128`: a name the language below reserves and cannot escape.**
    ///
    /// `let crate = 3` used to lower to `let crate = 3;` and `rustc` answered
    /// `E0532` about a module — about a file nobody wrote, which is
    /// [Part III C.1](../../docs/specification/30-nikaia-tooling.md)'s class.
    /// Every other such word is escaped where it is written
    /// ([ADR-076](../../docs/specification/adr/adr-076.md) D1); these three have
    /// no escape, so a refusal here is the only thing left that is not a message
    /// in the backend's words.
    ///
    /// Three words rather than twenty-seven, and that is the whole point of D1:
    /// `type` is the field name of every tagged record anybody has written and
    /// it stays available. `crate` and `super` name a module tree this language
    /// does not have, and `Self` is already how it writes the type an `impl` is
    /// on — so the vocabulary this costs is one nobody reaches for.
    fn not_unescapable(&mut self, name: &str, span: &Span, what: &str) {
        if !Self::UNESCAPABLE_BELOW.contains(&name) {
            return;
        }
        self.checked.findings.push(Finding {
            severity: Severity::Error,
            span: span.clone(),
            code: "NK1128",
            message: format!(
                "`{name}` is a name the language below reserves and cannot escape, \
                 so {what} may not be called that"
            ),
            notes: vec![
                "every other such name is written escaped and stays a name here - \
                 a field called `type` is fine. `crate`, `super` and `Self` are the \
                 three the language below refuses even escaped, which is why they are \
                 the three refused here (ADR-076 D3)"
                    .to_string(),
            ],
            help: Some(format!(
                "pick another name - `{}` is free, and so is anything else that is not \
                 one of those three",
                match name {
                    "crate" => "package",
                    "super" => "parent",
                    _ => "own",
                }
            )),
        });
    }

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
            // **`break i` is two statements, and that is the point** - a jump
            // takes no value, so the value becomes a statement of its own and
            // the program written is not the program compiled. `while_stmt`
            // has the identical note about the identical failure: three
            // statements, no error, and `rustc` complaining about a file
            // nobody wrote. Asked here rather than in the grammar because the
            // grammar cannot see what follows without a lookahead over every
            // expression there is - and asked of the **next** statement's span,
            // because that is what the author would have to delete.
            if at < last && matches!(stmt.node, Stmt::Break | Stmt::Continue) {
                self.nothing_follows_a_jump(&stmt.node, &block.stmts[at + 1].span);
            }
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
                names,
                mutable,
                ty,
                value,
            } => {
                self.a_field_of_a_borrowed_subject(value, span, "bound");
                let found = self.expr(value, span);
                // **A `let` over a place is a view of it** (ADR-094 D4), and
                // the emitter needs to know before it writes the line. A bare
                // name is deliberately not a place here: `let y = x` is a
                // rename and stays a move, which is the one shape that
                // separates this rule from `for`'s.
                if matches!(value, Expr::Field { .. } | Expr::Index { .. }) && moves_away(&found) {
                    self.checked.lent_lets.insert(span.start);
                }
                // **A tuple of names takes the value apart**
                // ([ADR-098](../../../docs/specification/adr/adr-098.md)). The
                // parts come from the value's own type where it is a tuple of
                // the right width, and are `?` otherwise - the same silence
                // every other unanswered question here keeps, and never a guess
                // that the widths match.
                if let [_, _, ..] = names.as_slice() {
                    return self.tuple_let(names, ty.as_ref(), &found, span);
                }
                let name = self.parsed.text(names[0]).to_string();
                // **`let _ = expr` is a statement wearing a `let`**
                // ([ADR-126](../../docs/specification/adr/adr-126.md) D2). A
                // binding that ignores its whole value binds nothing, so the
                // word `let` says something that does not happen - and below it
                // is Rust's `let _ =`, which **discards** the value where the
                // source said *bound*. For a file handle or a lock guard those
                // are different programs, which is why the one thing Rust's form
                // is used for is the one thing this refuses.
                //
                // Only the single-name form: `let (a, _) = pair()` is D1's tuple
                // position and binds `a`.
                if name == "_" {
                    self.a_let_that_binds_nothing(span);
                }
                self.nameable(&name, span, "a `let`");

                let bound = match ty {
                    Some(ty) => {
                        let want = self.declared(ty, span);
                        // **The annotation is no longer the constructor**
                        // ([ADR-064](../../docs/specification/adr/adr-064.md) D2):
                        // a hull is made by a call, so a plain value standing here
                        // is an ordinary mismatch - and `convert` is what names the
                        // way out.
                        self.constant_fits(value, Some(&want), span);
                        self.wraps_into_nullable(&found, &want, value, span);
                        self.expect(&found, &want, span.clone(), "let", |found, want| {
                            format!("this is `{found}`, and the `let` says `{want}`")
                        });
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
                // **What a task binds for itself** (ADR-055 D6): recorded
                // where the type is known, because the frame goes when the body
                // does.
                if let Some(task) = self.task_bindings.last_mut() {
                    task.push((name.clone(), bound.clone(), span.start));
                }
                self.bind_local(Local {
                    name,
                    ty: bound,
                    constant,
                    // **Part I 2.1**: without the word, a change to this name
                    // is `NK1139`. The span is the statement's, which is the
                    // `let` itself — a binding has no narrower one.
                    immutable: (!mutable).then(|| Immutable {
                        at: span.clone(),
                        kind: Kind::Let,
                    }),
                });
                Ty::Tuple(Vec::new())
            }

            // **`comptime` is a `let` that has to fold**
            // ([ADR-073](../../docs/specification/adr/adr-073.md) D3). The fold
            // is the one [ADR-063](../../docs/specification/adr/adr-063.md)
            // already shares, so nothing new evaluates anything: what this arm
            // adds is the refusal where it comes back empty, and the type the
            // emitter will need (D4).
            //
            // **Refused by name rather than run at program time** (D5). Falling
            // back would break the promise the word is for, and quietly - the
            // program would still work and the guarantee would be gone.
            Stmt::Comptime { name, ty, value } => {
                self.comptime_binding(*name, ty.as_ref(), value, span)
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
                // **D3**: an assignment into a parameter, or into a place
                // rooted at one, changes the caller's value and says `mut`.
                if let Some(root) = self.rooted_at(target) {
                    let how = match target {
                        Expr::Variable(_) => "this assigns to it",
                        _ => "this assigns into it",
                    };
                    self.a_changed_binding_says_mut(&root, how);
                }
                let into = self.expr(target, span);
                let found = self.expr(value, span);
                // **A write to shared mutable state goes through a door**
                // ([ADR-099](../../../docs/specification/adr/adr-099.md)).
                self.a_write_that_skips_the_door(target, &into, value, span);
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
                // **The body and not the condition.** A `break` written in the
                // condition is bound to this very loop in the language below,
                // which is legal there and is a program nobody writes; refusing
                // it is the direction that can be taken back later, and
                // accepting it is not.
                self.loops += 1;
                self.block(body);
                self.loops -= 1;
                self.scope.pop();
                Ty::Tuple(Vec::new())
            }

            Stmt::For {
                bindings,
                iter,
                body,
            } => {
                self.the_caller_writes_no_reference(iter, span, "a `for` lends what it iterates");
                let over = self.expr(iter, span);
                self.fallible_step(&over, bindings.len(), span);
                // **A `for` walks a produced sequence, and that consumes it**
                // ([ADR-105](../../docs/specification/adr/adr-105.md) D2). A
                // container is walked by view and as often as one likes, which
                // is why the record is keyed on the type being a `Seq`.
                self.a_sequence_is_walked(iter, &over, span);
                let element = element_of(&over, bindings.len());
                let frame: Vec<Local> = bindings
                    .iter()
                    .map(|b| Local::free(self.parsed.text(*b).to_string(), element.clone()))
                    .collect();
                for local in &frame {
                    self.nameable(&local.name.clone(), span, "a `for` binding");
                }
                self.scope.push(frame);
                self.loops += 1;
                self.block(body);
                self.loops -= 1;
                self.scope.pop();
                Ty::Tuple(Vec::new())
            }

            Stmt::Return(value) => {
                if let Some(value) = value {
                    self.a_field_of_a_borrowed_subject(value, span, "handed back");
                }
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

            // Part I 3.3. **`Ty::Unknown` and not `()`**, for the reason a
            // `return` hands back one: a statement that jumps is not a value,
            // and a block that ends in one is a block nothing arrives at the
            // end of. `if c { break } else { 1 }` would otherwise be an `if`
            // whose halves disagree, which is a refusal about a program that is
            // right - the language below reads the jumping half as the `!` it
            // is and coerces it to the other.
            Stmt::Break | Stmt::Continue => {
                let word = match stmt {
                    Stmt::Break => "break",
                    _ => "continue",
                };
                if self.loops == 0 {
                    self.a_jump_with_nowhere_to_go(word, span);
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
            // **`unsafe { … }` is a block with a value and no other rule**
            // ([ADR-124](../../docs/specification/adr/adr-124.md) D3): what is
            // inside is checked exactly as anything else is.
            Expr::Block(block) => self.block(block),

            // **`unsafe { … }` is a block with a value and no other rule**
            // ([ADR-124](../../docs/specification/adr/adr-124.md) D3): what is
            // inside is checked exactly as anything else is. The one thing it
            // changes is that a call to an `extern` name is allowed here, which
            // is the whole of what the word buys.
            Expr::Unsafe(block) => {
                let outer = std::mem::replace(&mut self.inside_unsafe, true);
                let value = self.block(block);
                self.inside_unsafe = outer;
                value
            }

            // **Part I 8.1.2: each statement is a branch, and the block's value
            // is the tuple of their results in written order**
            // ([ADR-050](../../docs/specification/adr/adr-050.md) D2).
            //
            // Walked here rather than through the `Block` arm above, because a
            // block hands back its *last* statement and this hands back all of
            // them. The scope frame is the block's own, so a name a branch binds
            // does not leak - and `branches_meet_on_nothing` is what refuses one
            // that binds at all, since the block's value already carries it.
            Expr::Overlap(block) => {
                self.scope.push(Vec::new());
                let parts: Vec<Ty> = block
                    .stmts
                    .iter()
                    .map(|stmt| {
                        // Each branch is an `async` block of its own (ADR-050
                        // D2), so the boundary is per branch rather than per
                        // block.
                        self.past_a_boundary("`overlap` branch", |me| match &stmt.node {
                            Stmt::Expr(value) => me.expr(value, &stmt.span),
                            other => {
                                me.stmt(other, &stmt.span);
                                Ty::Unknown
                            }
                        })
                    })
                    .collect();
                self.scope.pop();
                self.branches_meet_on_nothing(block, span);
                Ty::Tuple(parts)
            }

            Expr::If {
                cond,
                then_branch,
                else_branch,
            } => {
                let cond_ty = self.expr(cond, span);
                self.expect_bool(&cond_ty, span, "an `if` decides on a `bool`");
                // **A decision taken on what a lock said is stale too**
                // ([ADR-111](../../docs/specification/adr/adr-111.md) D4's
                // second shape). It reaches inward and **accumulates**: a
                // nested `if` under a stamped one is still under it, and a
                // plain condition inside does not clear the outer one.
                let outer_condition = self.stamped_condition;
                if cond_ty.is_seen() {
                    self.stamped_condition = Some(span.start);
                }
                let then = self.block(then_branch);
                let branches = match else_branch {
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
                };
                self.stamped_condition = outer_condition;
                branches
            }

            Expr::Match { value, arms } => {
                let on = self.expr(value, span);
                // **A `match` is a condition too** (ADR-111 D4).
                let outer_condition = self.stamped_condition;
                if on.is_seen() {
                    self.stamped_condition = Some(span.start);
                }
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
                self.stamped_condition = outer_condition;
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
                // **`after:` on a `set` is a witness and not an option**
                // ([ADR-111](../../docs/specification/adr/adr-111.md) D5): it
                // is walked below as an *argument*, where the door's signature
                // is what says it has to be a `$T`. Found here by name, and
                // confirmed to be the door once the receiver's type is in hand
                // - a `set` on something that is not a lock takes this back.
                let witness = match self.parsed.text(*method) == "set" {
                    true => config
                        .iter()
                        .position(|a| self.parsed.text(a.name) == "after"),
                    false => None,
                };
                // ADR-007 D5: a DSL's deferred parameters stand here. They are
                // expressions like any other, so they are walked - what checks
                // that they are the *right* names is `dsl::check`, which knows
                // which statement the receiver came from.
                config.iter().enumerate().for_each(|(at, a)| {
                    if Some(at) != witness {
                        self.expr(&a.value, span);
                    }
                });
                // **A grammar is entered by an ordinary call**
                // ([ADR-082](../../docs/specification/adr/adr-082.md) D1), so
                // this is that call: a receiver naming a grammar of this file
                // and a method naming one of its `pub` rules. Answered before
                // the receiver is typed, because a grammar name is not a value
                // and typing it would be asking the wrong question.
                // **Both ways out of here owe the witness its walk.** It was
                // held back above so that the door can walk it as an argument,
                // and neither of these reaches the door: a grammar rule is not
                // a lock, and a `T?` receiver is answered before the call is.
                // A value that nothing walks is a mistake nothing reports.
                if let Some(entered) = self.grammar_entry(receiver, *method) {
                    if let Some(at) = witness {
                        self.expr(&config[at].value, span);
                    }
                    // **The dot is gone** (ADR-140 D3), and the refusal is
                    // here rather than in the parser because only this side
                    // knows the receiver names a grammar: `Json.value(x)` and
                    // `text.value(x)` are the same five tokens.
                    let (grammar, rule) = entered.split_once("::").unwrap_or((&entered, ""));
                    self.a_grammar_reached_through_a_dot(grammar, rule, span);
                    return self.grammar_call(&entered, args, span);
                }
                let on = self.expr(receiver, span);
                if let Ty::Nullable(_) = &on {
                    let name = self.parsed.text(*method).to_string();
                    self.reaches_into_a_nullable(&on, Reached::Method(&name), span);
                    // The arguments are still walked: a mistake inside one is a
                    // mistake whatever is wrong with the receiver.
                    args.iter().for_each(|a| {
                        self.expr(a, span);
                    });
                    if let Some(at) = witness {
                        self.expr(&config[at].value, span);
                    }
                    return Ty::Unknown;
                }
                // **The witness door, now that the receiver is typed**
                // ([ADR-111](../../docs/specification/adr/adr-111.md) D5).
                // `kasse.set(neu; after: stand)` is `Locked::set(after)` in the
                // ledger — a key no program can write as a method name, which
                // is what keeps D5's *one door* one (`NK2208` is what a program
                // that tries meets). The witness joins the arguments, so
                // everything a call is checked for reaches it through the one
                // path: the fit against `$T`, the lend, the stamp.
                //
                // **Before `NK2203`**, which names the entry it refuses and
                // would otherwise name `set` for a call that is not one.
                let door = witness
                    .filter(|_| locked_content_of(&on).is_some())
                    .map(|at| &config[at].value);
                if let (Some(at), None) = (witness, door) {
                    // Not a lock after all, so it was an option like any other
                    // and the walk above owed it a visit.
                    self.expr(&config[at].value, span);
                }
                let given: Vec<Expr>;
                let (args, written) = match door {
                    Some(seen) => {
                        self.the_witness_takes_no_reference(seen, span);
                        self.checked.witnessed_sets.insert(span.start);
                        given = args.iter().chain([seen]).cloned().collect();
                        (&given[..], "set(after)".to_string())
                    }
                    None => (&args[..], self.parsed.text(*method).to_string()),
                };
                // **A `set` given a stamped value** (D4). The
                // rule wants the receiver's **name** for its message and the
                // argument's **type** for its answer, and those are known in
                // two different places — so the name is carried the way the
                // door flags above are, and the rule itself runs in `call_on`
                // where the arguments have been typed.
                let outer_receiver = std::mem::replace(
                    &mut self.set_receiver,
                    match receiver.as_ref() {
                        Expr::Variable(name) => Some(self.parsed.text(*name).to_string()),
                        _ => Some("this lock".to_string()),
                    },
                );
                // The same carrying, for D2's once-only rule, and `None` where
                // the receiver is a temporary.
                let outer_named = std::mem::replace(
                    &mut self.receiver_name,
                    match receiver.as_ref() {
                        Expr::Variable(name) => Some(self.parsed.text(*name).to_string()),
                        _ => None,
                    },
                );
                self.a_door_that_is_not_written(&on, &written, span);
                // **`NK2203`, the method half**: which entry `other.get()` goes
                // to is the type checker's answer (ADR-028), so it is asked
                // here where the receiver's type is in hand.
                if self.inside_a_door {
                    if let Ty::Named { name, .. } = &on {
                        let key = format!("{name}::{written}");
                        if let Some((key, contract)) = self.method(&key) {
                            let (key, holds) = (key.clone(), contract.touches_a_lock);
                            self.a_lock_inside_a_lock(&key, holds, span);
                        }
                    }
                }
                // **`update`'s block is a write door's**
                // ([ADR-110](../../docs/specification/adr/adr-110.md) D1), and
                // that is where a parameter without `mut` is refused. Set
                // around the call and restored after it, so a lambda written
                // inside the block is not one.
                let at_a_door =
                    self.parsed.text(*method) == "update" && locked_content_of(&on).is_some();
                if at_a_door {
                    if let Some(Expr::Closure { params, body, .. }) = args.last() {
                        self.an_update_block_returns_nothing(body, span);
                        self.an_update_block_reads_what_it_writes(params, body, span);
                    }
                }
                let outer_door = std::mem::replace(&mut self.at_a_write_door, at_a_door);
                // **And `access` holds one open too** (ADR-039 D10): `get` and
                // `set` do not, because no code of the program's runs while the
                // lock is open in either.
                let holding = matches!(self.parsed.text(*method), "update" | "access")
                    && locked_content_of(&on).is_some();
                // Kept for the stamp below, since `call_on` takes `on` by value.
                let on_for_the_stamp = on.clone();
                let outer_inside = std::mem::replace(&mut self.inside_a_door, holding);
                // **D3**: a method that changes its subject, called on a
                // parameter. Asked of the `mutates` column (D3's own, recorded
                // because the ledger's `&T` cannot spell `&mut`), and only
                // where **every** candidate for the name agrees — one that does
                // not is a name this compiler cannot resolve, and refusing on
                // it would refuse a correct program (C.4).
                if let Some(root) = self.rooted_at(receiver) {
                    let name = self.parsed.text(*method);
                    let candidates: Vec<_> = self
                        .own
                        .candidates(name)
                        .into_iter()
                        .chain(self.library.candidates(name))
                        .collect();
                    let changes = !candidates.is_empty()
                        && candidates.iter().all(|(_, contract)| contract.mutates);
                    if changes {
                        self.a_changed_binding_says_mut(
                            &root,
                            &format!("`{name}` changes what it is called on"),
                        );
                    }
                }
                let value = self.call_on(on, *method, args, &written, span);
                self.receiver_name = outer_named;
                self.at_a_write_door = outer_door;
                self.inside_a_door = outer_inside;
                self.set_receiver = outer_receiver;
                // **And what `access` hands back came out of a lock** (D1).
                let value = match self.parsed.text(*method) == "access"
                    && locked_content_of(&on_for_the_stamp).is_some()
                {
                    true => Ty::seen(value.unseen()),
                    false => value,
                };
                value
            }

            // Part I 3.5: `x?.m(…)`. The receiver must be a `T?`, the call
            // happens only where there is something to call it on, and the
            // result is flattened for [`Expr::SafeField`]'s reason - a method
            // that hands back a `T?` would otherwise give a nullable of a
            // nullable ([ADR-066](../../../docs/specification/adr/adr-066.md)).
            Expr::SafeMethod {
                receiver,
                method,
                args,
                config,
            } => {
                // **The witness is held back here too**
                // ([ADR-111](../../docs/specification/adr/adr-111.md) D5), for
                // the reason a `?.` is one arm and not two: it decides
                // *whether* the call happens and never *what* a call is. A
                // `SharedMut[i64]?` reached with `?.set(neu; after: stand)` is
                // the same door, and the witness that stayed an option here
                // would have been **dropped in the lowering** rather than
                // compared — a silent wrong value, which is worse than
                // anything a refusal costs.
                let witness = match self.parsed.text(*method) == "set" {
                    true => config
                        .iter()
                        .position(|a| self.parsed.text(a.name) == "after"),
                    false => None,
                };
                config.iter().enumerate().for_each(|(at, a)| {
                    if Some(at) != witness {
                        self.expr(&a.value, span);
                    }
                });
                let on = self.expr(receiver, span);
                let name = self.parsed.text(*method).to_string();
                let Ty::Nullable(inner) = on else {
                    self.reaches_through_a_plain_value(&on, Reached::Method(&name), span);
                    // The arguments are still walked: a mistake inside one is
                    // a mistake whatever is wrong with the receiver, and a
                    // reader owed two messages should get two.
                    args.iter().for_each(|a| {
                        self.expr(a, span);
                    });
                    if let Some(at) = witness {
                        self.expr(&config[at].value, span);
                    }
                    return Ty::Unknown;
                };
                // **The same call, on the value inside.** Everything a method
                // call is checked for - what it may throw, whether it pauses,
                // what its arguments have to be, what the receiver's own type
                // binds - is unchanged by the reach: a `?.` decides *whether*
                // the call happens and never *what* a call is. The door is one
                // of those things, so it is resolved here exactly as it is
                // there, off the type **inside** the `T?`.
                let door = witness
                    .filter(|_| locked_content_of(&inner).is_some())
                    .map(|at| &config[at].value);
                if let (Some(at), None) = (witness, door) {
                    self.expr(&config[at].value, span);
                }
                let given: Vec<Expr>;
                let (args, written) = match door {
                    Some(seen) => {
                        self.the_witness_takes_no_reference(seen, span);
                        self.checked.witnessed_sets.insert(span.start);
                        given = args.iter().chain([seen]).cloned().collect();
                        (&given[..], "set(after)".to_string())
                    }
                    None => (&args[..], self.parsed.text(*method).to_string()),
                };
                match self.call_on(*inner, *method, args, &written, span) {
                    // The `and_then` case, recorded by name for the emitter
                    // exactly as a nullable field is (ADR-028: the emitter has
                    // no types and this is a question about one).
                    Ty::Nullable(result) => {
                        self.checked.flattened_reaches.insert((span.start, name));
                        Ty::Nullable(result)
                    }
                    Ty::Unknown => Ty::Unknown,
                    plain => Ty::Nullable(Box::new(plain)),
                }
            }

            Expr::Field { base, name } => {
                let on = self.expr(base, span);
                let field = self.parsed.text(*name).to_string();
                if let Ty::Nullable(_) = &on {
                    self.reaches_into_a_nullable(&on, Reached::Field(&field), span);
                    return Ty::Unknown;
                }
                let Ty::Named { name: ty, .. } = &on else {
                    return Ty::Unknown;
                };
                let Some(fields) = self.fields_of(ty) else {
                    // `NK1126` where the type is a parameter: nothing will ever
                    // describe `T`, so a field on one is refused here rather
                    // than by `rustc` about the generated file.
                    let ty = ty.clone();
                    self.nothing_says_what_a_parameter_can_do(&ty, Reached::Field(&field), span);
                    return Ty::Unknown;
                };
                match fields.iter().find(|f| f.name == field) {
                    Some(found) => {
                        let (ty, declared) = (ty.clone(), found.clone());
                        self.field_is_reachable(&ty, &declared, span);
                        // `Pair[i64].first` is an `i64`: the receiver's own
                        // arguments bind the declaration's parameters, exactly
                        // as a method's receiver binds its signature's
                        // (ADR-031, and ADR-074 D2 for a `.nika` declaration).
                        ty::substitute(&declared.ty, &self.arguments_of(&on))
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
                    self.reaches_through_a_plain_value(&on, Reached::Field(&field), span);
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
                // **A struct literal naming nothing this compiler declares**
                // ([ADR-096](../../docs/specification/adr/adr-096.md), `NK1135`),
                // and it is the silence that let
                // [ADR-133](../../docs/specification/adr/adr-133.md)'s collision
                // through: `Stats(min: first)` and `execute(target_age: 30)` are
                // one spelling, `ctor_lit` reads both as a literal, and a literal
                // for a struct nothing declares walked out of here on the
                // `continue` below and lowered verbatim. `rustc` answered
                // *cannot find struct `execute`* about a file nobody wrote, which
                // is Part III C.1's class and the very hole `NK1135` was built
                // to close for a written annotation.
                //
                // **Unqualified only**, which is `NK1135`'s own convention:
                // `pool::Conn(id: 1)` names a package's type, and whether this
                // build can see that package is a question with a message of its
                // own (ADR-046 D2).
                if declared.is_none() && !name.contains("::") && !self.declares_a_type(&name) {
                    self.a_struct_nothing_declares(&name, span);
                }
                // **What a generic struct's literal binds**
                // ([ADR-074](../../docs/specification/adr/adr-074.md) D2).
                // `Pair { first: 1, second: 2 }` is a `Pair[i64]` and nothing
                // else says so: the declaration writes `first: $T` and the
                // value is an `i64`, which is one `bind` per field.
                let mut bound: BTreeMap<String, Ty> = BTreeMap::new();
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
                            ty::bind(&want, &found, &mut bound);
                            let owner = name.clone();
                            self.field_is_reachable(&name, found_field, span);
                            // **No longer a constructor either**
                            // ([ADR-064](../../docs/specification/adr/adr-064.md)
                            // D2). It was the second of the two positions, and the
                            // two positions are what stopped being a list.
                            let _ = &owner;
                            // Part I 2.3's fourth position: a plain value in a
                            // field the struct declares nullable. The same rule
                            // as the other three (`wraps_into_nullable`), keyed
                            // by the field as well, because a struct literal
                            // has one of these per field and a statement only
                            // one span.
                            let value = init.value.as_ref();
                            let is_literal = value.is_some_and(is_literal);
                            if let Some(how) = wrap_for(&found, &want, is_literal) {
                                self.checked
                                    .nullable_fields
                                    .entry((span.start, owner.clone(), field.clone()))
                                    .or_default()
                                    .insert(value.map(argument_shape).unwrap_or_default(), how);
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
                match self.struct_parameters.get(&name) {
                    Some(order) => {
                        let args = order
                            .iter()
                            .map(|p| bound.get(p).cloned().unwrap_or(Ty::Unknown))
                            .collect();
                        Ty::Named {
                            name,
                            args,
                            view: false,
                        }
                    }
                    None => Ty::named(name),
                }
            }

            // A lambda's arguments are the ones it names (ADR-049). There is
            // nothing to read off the body any more, so a `fn { … }` pushes an
            // empty frame - and a body reaching for `a` is then a body naming
            // something nothing declares, which `NK1117` refuses.
            Expr::Closure {
                params,
                mutable,
                body,
            } => {
                let frame: Vec<Local> = params
                    .iter()
                    .map(|p| self.lambda_parameter(*p, mutable, Ty::Unknown, span))
                    .collect();
                for local in &frame {
                    self.nameable(&local.name.clone(), span, "a lambda's argument");
                }
                self.scope.push(frame);
                // Part I 3.3: a lambda is a closure below, and a jump does not
                // leave one.
                self.past_a_boundary("lambda", |me| me.block(body));
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

            Expr::Binary {
                op,
                lhs,
                rhs,
                span: at,
            } => {
                let left = self.expr(lhs, span);
                let right = self.expr(rhs, span);
                self.divisor_is_not_zero(*op, rhs, span);
                // **The stamp sticks**
                // ([ADR-111](../../docs/specification/adr/adr-111.md) D2):
                // `stand + 100` is a `Seen[i64]` and `stand > 100` a
                // `Seen[bool]`. An operator cannot put a value back into the
                // lock it came from, so what it makes still carries where it
                // came from — which is what lets `set` refuse it and every
                // other sink take it.
                let stamped = left.is_seen() || right.is_seen();
                let outcome = match op {
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
                    // **A `+` where either side is text is a concatenation**
                    // ([ADR-081](../../docs/specification/adr/adr-081.md) D2),
                    // and it comes to a `String` whichever side was owned. This
                    // used to be the `_ => Ty::Unknown` below, with a comment
                    // saying that guessing which side names the result was the
                    // one guess this checker does not make. It was not a guess
                    // that was missing but a **decision**: a concatenation makes
                    // a new value, so `String` is the only thing it can be, and
                    // saying `&str` - which is what two borrowed sides used to
                    // come to - made `-> String` a false refusal (`NK1104`).
                    //
                    // Recorded by the operator's own span, because `a + b + c`
                    // is two of them and the statement they stand in is one.
                    BinaryOp::Add if is_text(&left) || is_text(&right) => {
                        self.checked.concatenations.insert(at.start);
                        Ty::named("String")
                    }
                    // Arithmetic on two of the same thing is that thing, and
                    // a bare number is neither - so one known side decides. Two
                    // known sides that disagree still decide nothing, now that
                    // the one case anybody met is taken above.
                    _ => match (left.is_unknown(), right.is_unknown()) {
                        (true, _) => right,
                        (_, true) => left,
                        _ if left == right => left,
                        _ => Ty::Unknown,
                    },
                };
                match stamped {
                    true => Ty::seen(outcome.unseen()),
                    false => outcome,
                }
            }

            Expr::Cast { expr, ty } => {
                let from = self.expr(expr, span);
                let into = self.declared(ty, span);
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
                // **A guard of its own, and the enclosing one set aside**
                // ([ADR-091](../../../docs/specification/adr/adr-091.md)). A
                // `catch` inside another one's guarded expression handles its
                // own failures, so those are not the outer one's to count -
                // `(a.parse() catch { 1 }) catch { 2 }` has nothing left for the
                // second. The outer is told *no answer* rather than *nothing
                // fails*, because what a handler's own failure does is
                // [ADR-034](../../../docs/specification/adr/adr-034.md)'s
                // question and this refusal does not need it answered.
                let enclosing = self.guarded.replace(Guarded::default());
                self.expr(expr, span);
                let guarded = std::mem::replace(&mut self.guarded, enclosing);
                if self.guarded.is_some() {
                    self.guard_has_no_answer();
                }
                self.caught = outer;
                self.nothing_here_can_fail(guarded.unwrap_or_default(), span);
                self.scope
                    .push(vec![Local::free("error".to_string(), Ty::Unknown)]);
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
                let Expr::Closure { params, body, .. } = body.as_ref() else {
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
                        // A task is handed nothing, so no parameter of one is
                        // ever `mut` - `a_task_takes_no_arguments` above has
                        // already refused the list that is not empty.
                        mutable: Vec::new(),
                        body: body.clone(),
                    },
                    span,
                );
                self.scope.push(Vec::new());
                self.task_bindings.push(Vec::new());
                // **A task started with `spawn` runs later and elsewhere**
                // ([ADR-039](../../docs/specification/adr/adr-039.md) D3), so
                // taking a lock inside one is the ordinary case and not this
                // door's reach. `contracts::locks` makes the same split when it
                // builds the column; this is it at the refusal.
                //
                // A **scope**'s tasks are the other way round and are not this:
                // a scope waits for them, so they run during the call.
                let outer_inside = std::mem::replace(&mut self.inside_a_door, false);
                // A task's body is an `async` block below (ADR-055 §6), which
                // is a function too: `break` may not leave it either.
                let value = self.past_a_boundary("task", |me| me.block(body));
                self.scope.pop();
                self.inside_a_door = outer_inside;
                let bound = self.task_bindings.pop().unwrap_or_default();
                // **After** the walk, because the question needs both halves of
                // what the walk found: the types of what the body bound, and
                // which of its method calls pause (`pausing_methods`).
                self.a_task_holds_these_across_a_pause(body, &bound, span);
                Ty::Named {
                    name: "TaskHandle".to_string(),
                    args: vec![value],
                    view: false,
                }
            }

            // A template's holes are Nikaia too (ADR-017), and what the
            // template *produces* is still the emitter's business.
            Expr::Dsl { .. } => {
                // Which is also why `NK1134` says nothing about one: what this
                // becomes is decided after the checker has run, so whether it
                // can fail is not this walk's to answer.
                self.guard_has_no_answer();
                self.holes(expr, span);
                Ty::Unknown
            }

            // A grammar and an `asm` block: what these produce is the business
            // of the emitter that compiles them.
            Expr::Asm { .. } => {
                self.guard_has_no_answer();
                Ty::Unknown
            }
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
            Expr::MethodCall { receiver, .. } | Expr::SafeMethod { receiver, .. } => {
                self.names_something_here(receiver)
            }
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
                .map(|name| Local::free(name, Ty::Unknown))
                .collect();
            self.scope.push(frame);
            self.expr(&hole, span);
            self.scope.pop();
        }
    }

    /// `f(a, b)`, `Stats(first)`, `io::read_to_string()`, `write(p, d; append: true)`.
    /// A door over several locks: `access_all` reads them, `update_all` writes
    /// them ([ADR-065](../../docs/specification/adr/adr-065.md)).
    ///
    /// **It is typed here and not in the ledger**, for the reason the hull
    /// constructors are: a signature binds its type variables from the
    /// **receiver** (ADR-031), and a door over several locks has none - the
    /// locks are arguments, they hold different types, and how many there are is
    /// open. What a written signature cannot say, this says.
    ///
    /// `access_all` hands each value as a **view**, the way `access` does;
    /// `update_all` hands each in **by value** and takes a new one back per lock
    /// (D2). Both refuse anything that is not a lock, because a door that let a
    /// plain value through would be a door about nothing.
    fn locks(&mut self, door: MultiLock, args: &[Expr], span: &Span) -> Ty {
        let Some((last, locks)) = args.split_last() else {
            self.no_door(door, "it takes the locks and then the block", span);
            return Ty::Unknown;
        };
        let Expr::Closure {
            params,
            mutable,
            body,
        } = last
        else {
            self.no_door(door, "the block comes last: `fn(a, b) { … }`", span);
            return Ty::Unknown;
        };
        if locks.len() < 2 {
            self.no_door(door, "it is for **several** locks: name at least two", span);
            return Ty::Unknown;
        }

        let mut held = Vec::with_capacity(locks.len());
        for lock in locks {
            let found = self.expr(lock, span);
            match locked_content_of(&found) {
                Some(inside) => held.push(match (door, &inside) {
                    // A view of what it holds, the way `access` hands one over.
                    (MultiLock::Reading, Ty::Named { name, args, .. }) => Ty::Named {
                        name: name.clone(),
                        args: args.clone(),
                        view: true,
                    },
                    (MultiLock::Reading, _) => inside,
                    (MultiLock::Writing, _) => inside,
                }),
                // Nothing written down says this is a lock. A type nothing
                // describes is not claimed about at all (Part III, C.4) - but a
                // **number** is not such a type: a literal carries no type on
                // purpose (Part I 2.4), and reading that absence as "might be a
                // lock" would hand `rustc` a program about a trait bound.
                None if found.is_unknown() && !self.is_a_number(lock) => held.push(Ty::Unknown),
                None => {
                    self.checked.findings.push(Finding {
                        severity: Severity::Error,
                        span: span.clone(),
                        code: "NK1124",
                        message: format!(
                            "`{}` takes locks, and this is a `{}`",
                            door.written(),
                            found.text()
                        ),
                        notes: vec![
                            "a door over several locks is what keeps them in one order, and \
                             there is nothing to order about a value that is not one \
                             (Part II, 12.3)"
                                .to_string(),
                        ],
                        help: Some(format!(
                            "name `{SHARED_MUT}[T]` values, or `{LOCKED}[T]` fields"
                        )),
                    });
                    held.push(Ty::Unknown);
                }
            }
        }
        if !params.is_empty() && params.len() != locks.len() {
            self.no_door(
                door,
                "the block names one value per lock, in the order they are written",
                span,
            );
        }
        // D6 is D1 widened, so the same question is asked of this block.
        // D6 is D1 widened, so the same two questions are asked of this block.
        if matches!(door, MultiLock::Writing) {
            self.an_update_block_returns_nothing(body, span);
            self.an_update_block_reads_what_it_writes(params, body, span);
        }
        let outer_door = std::mem::replace(
            &mut self.at_a_write_door,
            matches!(door, MultiLock::Writing),
        );
        // Both kinds hold their locks open across the block, so both are one.
        let outer_inside = std::mem::replace(&mut self.inside_a_door, true);
        // **Nothing is asked of this block's promises**
        // ([ADR-102](../../docs/specification/adr/adr-102.md) D2), and that is
        // deliberate rather than an omission: `access_all` and `update_all` are
        // not ledger entries at all ([ADR-065](../../docs/specification/adr/adr-065.md)
        // D1), so there is no written type here to read a promise off. What
        // Part II 12.2 requires of the block — `sync`, and no lock — is
        // `NK2202`'s and `NK2203`'s, and a second refusal saying the same thing
        // in other words would be two rules for one mistake.
        self.lambda(
            params,
            mutable,
            body,
            &held,
            Promises {
                may_pause: true,
                may_fail: true,
            },
            span,
        );
        self.at_a_write_door = outer_door;
        self.inside_a_door = outer_inside;
        match door {
            // What a lambda hands back is not written down (ADR-029 D1) — but
            // that it came **out of a lock** is
            // ([ADR-111](../../docs/specification/adr/adr-111.md) D1), and the
            // stamp is what a `set` reads. `Seen[?]` is the honest pair: the
            // type is unknown and where it came from is not.
            MultiLock::Reading => Ty::seen(Ty::Unknown),
            MultiLock::Writing => Ty::Tuple(Vec::new()),
        }
    }

    /// Whether an expression is certainly a number, where its *type* says
    /// nothing.
    ///
    /// A literal has no type of its own (Part I 2.4), and neither has the name a
    /// bare `let` binds one to - but the constant fold watched both, so the
    /// absence of a type is not the absence of knowledge here.
    fn is_a_number(&self, expr: &Expr) -> bool {
        match expr {
            Expr::LitInt(_) | Expr::LitFloat(_) => true,
            Expr::Variable(name) => self
                .local(self.parsed.text(*name))
                .is_some_and(|(_, constant)| constant.is_some()),
            _ => false,
        }
    }

    /// The one message shape for a door written wrong.
    fn no_door(&mut self, door: MultiLock, wanted: &str, span: &Span) {
        self.checked.findings.push(Finding {
            severity: Severity::Error,
            span: span.clone(),
            code: "NK1124",
            message: format!("`{}` is written `{}`", door.written(), door.shape()),
            notes: vec![wanted.to_string()],
            help: None,
        });
    }

    /// `Shared(x)`, `SharedMut(x)` and `Locked(x)` make a hull
    /// ([ADR-064](../../docs/specification/adr/adr-064.md) D2).
    ///
    /// **One argument, and the hull takes the type of what it is handed.** A
    /// literal hands over `Unknown`, which is not a failure here: the hull is made
    /// by a *call* in the emitted code too, so the language below gives the number
    /// its type the way it gives one to any argument. That is the difference
    /// between a constructor and an annotation, and it is why the annotation could
    /// not answer `SharedMut(0)` at all.
    /// A type **as it is written**, checked for the one spelling this language
    /// does not have ([ADR-064](../../docs/specification/adr/adr-064.md) D3).
    ///
    /// `Shared[Locked[T]]` is what `SharedMut[T]` is, and two ways to write one
    /// type is the thing the short name was given a name *instead* of
    /// ([ADR-039](../../docs/specification/adr/adr-039.md) D9). Worse than
    /// untidy: the two are the same bytes below and two different types up here,
    /// so a value of one would not fit the other while the emitted Rust could not
    /// tell them apart.
    ///
    /// It runs at the positions a **program** writes a type, and the message
    /// carries the replacement.
    fn declared(&mut self, ty: &ast::Type, span: &Span) -> Ty {
        self.spelling(ty, span);
        Ty::from_ast(self.parsed, ty)
    }

    /// The walk under [`Checker::declared`], over a written type and everything
    /// inside it.
    fn spelling(&mut self, ty: &ast::Type, span: &Span) {
        if self.parsed.text(ty.name) == SHARED {
            if let Some(held) = ty.generics.first() {
                if self.parsed.text(held.name) == LOCKED {
                    let inside = held
                        .generics
                        .first()
                        .map(|t| self.parsed.text(t.name).to_string())
                        .unwrap_or_else(|| "T".to_string());
                    self.checked.findings.push(Finding {
                        severity: Severity::Error,
                        span: span.clone(),
                        code: "NK1123",
                        message: format!(
                            "a `{SHARED}` around a lock is what `{SHARED_MUT}[{inside}]` is called"
                        ),
                        notes: vec![
                            "the common case has the short name, and it is the only way \
                                     to write it - one type, one spelling (Part I, 6.2)"
                                .to_string(),
                        ],
                        help: Some(format!("write `{SHARED_MUT}[{inside}]`")),
                    });
                }
            }
        }
        for inner in &ty.generics {
            self.spelling(inner, span);
        }
    }

    fn hull(&mut self, name: &str, args: &[Expr], found: &[Ty], span: &Span) -> Ty {
        if args.len() != 1 {
            self.checked.findings.push(Finding {
                severity: Severity::Error,
                span: span.clone(),
                code: "NK1101",
                message: format!(
                    "`{name}` takes the value to put in it, and {} were passed",
                    args.len()
                ),
                notes: vec![format!(
                    "`{name}[T]` is made from a `T`: `{name}(value)` (Part I, 6.2)"
                )],
                help: None,
            });
            return Ty::Unknown;
        }
        let held = found.first().cloned().unwrap_or(Ty::Unknown);
        // A handle of a handle is two counts around one value and means nothing
        // the one count does not: it is refused where the type says so, and a
        // value nothing describes is not claimed about (C.4).
        if let Ty::Named { name: inner, .. } = &held {
            if is_hull(inner) {
                self.checked.findings.push(Finding {
                    severity: Severity::Error,
                    span: span.clone(),
                    code: "NK1123",
                    message: format!("this is already a `{inner}`, so `{name}` has nothing to add"),
                    notes: vec![
                        "a handle is duplicated by being handed on, never by being wrapped \
                         again (Part I, 6.2)"
                            .to_string(),
                    ],
                    help: Some(format!("hand the `{inner}` on as it is")),
                });
                return held;
            }
        }
        Ty::Named {
            name: name.to_string(),
            args: vec![held],
            view: false,
        }
    }

    fn call(&mut self, func: &Expr, args: &[Expr], config: &[ast::ConfigArg], span: &Span) -> Ty {
        // **A door over several locks types its own lambda**
        // ([ADR-065](../../docs/specification/adr/adr-065.md) D1), and it has to
        // be answered before the arguments are walked: what the lambda is handed
        // comes from what the *locks* hold, so the locks are typed first and the
        // lambda after. Every other call can walk its arguments in one pass.
        if let Expr::Variable(name) = func {
            if let Some(door) = MultiLock::named(self.parsed.text(*name)) {
                return self.locks(door, args, span);
            }
        }
        // **`NK2203`**: a free call inside a door's block, to something that
        // takes a lock (ADR-039 D2). `println` is the one every program writes,
        // and ADR-067 D1 is where it was pinned down: it never pauses, so
        // `sync` says nothing about it, and it takes standard output's own lock
        // while yours is open.
        if self.inside_a_door {
            if let Some(name) = self.free_callee(func) {
                let holds = self
                    .own
                    .functions
                    .get(&name)
                    .or_else(|| self.library.functions.get(&name))
                    .map(|c| c.touches_a_lock)
                    .unwrap_or_default();
                self.a_lock_inside_a_lock(&name, holds, span);
            }
        }
        // **The callee is named and resolved before the arguments are walked**
        // ([ADR-029](../../docs/specification/adr/adr-029.md), the free half).
        // A lambda's parameters are typed from the callee's signature, so the
        // signature has to be in hand first — which is exactly the ordering the
        // *method* path was given when that record landed, and which this one
        // did not have: `hand(fn(n) { … })` left `n` with no type at all, and
        // [ADR-102](../../docs/specification/adr/adr-102.md) D2's promises had
        // nothing to be asked of.
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

        // **A call to an `extern` name is written inside `unsafe { … }`**
        // ([ADR-124](../../docs/specification/adr/adr-124.md) D3), and this is
        // where that is asked: the name is in hand and the block is a flag the
        // walk carries.
        self.a_foreign_call_outside_unsafe(&name, span);

        // **A grammar is entered by an ordinary call**
        // ([ADR-082](../../docs/specification/adr/adr-082.md) D1), through a
        // **path** since [ADR-140](../../docs/specification/adr/adr-140.md) D3.
        // Answered before anything is resolved, because a grammar is not a
        // ledger entry a `resolve` would find.
        if let Some(entered) = self.grammar_path(&name) {
            return self.grammar_call(&entered, args, span);
        }

        // **A struct literal written like a call**
        // ([ADR-140](../../docs/specification/adr/adr-140.md) D1). `Stats(min: 1)`
        // parsed as Kap 4.2's named literal until D1 took that spelling out of the
        // language; it is a call with options now, and where the name is a **type**
        // that is the old form rather than a call anybody meant. Asked here because
        // the parser cannot ask it - the two forms are one spelling and a name
        // denotes one of them - and asked before the resolution below, which would
        // otherwise answer about the anonymous constructor: the silent lowering was
        // `Stats::new()` with the fields dropped, which `rustc` then refused about a
        // file nobody wrote ([Part III C.1](../../docs/specification/30-nikaia-tooling.md)).
        if !config.is_empty() && args.is_empty() && self.declares_a_type(&name) {
            self.a_literal_written_like_a_call(&name, config, span);
            for option in config {
                self.expr(&option.value, span);
            }
            return Ty::named(&name);
        }

        // A tuple variant of an enum declared here - `Op::Plus(1)` - is a value
        // of that enum, not a call to a function. Its arguments are still
        // walked, below, with the rest.
        let variant = name
            .split_once("::")
            .filter(|(ty, variant)| self.is_variant(ty, variant))
            .map(|(ty, _)| ty.to_string());
        // Neither a variant nor a hull is a ledger entry, so neither has a
        // signature to type an argument from.
        let resolved = match variant.is_some() || is_hull(&name) {
            true => None,
            false => self.resolve(&name),
        };
        let expected: Vec<Ty> = resolved
            .as_ref()
            .map(|(_, contract)| expected_arguments(contract))
            .unwrap_or_default();
        // The same walk the method path uses, so the same questions are asked
        // in both - or `takes(self.name)` slips past `NK1131` while
        // `x.takes(self.name)` does not.
        let found = self.arguments_given(
            args,
            &expected,
            resolved
                .as_ref()
                .is_some_and(|(key, _)| self.own.functions.contains_key(key)),
            span,
        );
        let passed: Vec<(String, Ty)> = config
            .iter()
            .map(|a| {
                self.a_field_of_a_borrowed_subject(&a.value, span, "passed");
                (
                    self.parsed.text(a.name).to_string(),
                    self.expr(&a.value, span),
                )
            })
            .collect();

        if let Some(ty) = variant {
            return Ty::named(&ty);
        }

        // **A hull you can observe, you write**
        // ([ADR-064](../../docs/specification/adr/adr-064.md) D2). The three hull
        // types are made by a call, and it is this compiler's to type rather than
        // the ledger's: the ledger binds `$T` from a **receiver** (ADR-031) and a
        // constructor has none, so the type argument would have to come from what
        // is handed in.
        if is_hull(&name) {
            return self.hull(&name, args, &found, span);
        }

        let Some((key, contract)) = resolved else {
            // A call nothing describes is a call this compiler cannot see the
            // end of, and a thread of its own is among the things it may do
            // (ADR-038 D7). What it is handed is therefore handed across.
            self.crosses_into_an_unseen_call(&name, args, &found, config, &passed, span);
            // And a call this compiler cannot see the end of says nothing about
            // whether it can **fail**, either
            // ([ADR-091](../../docs/specification/adr/adr-091.md)).
            self.guard_has_no_answer();
            return Ty::Unknown;
        };
        self.reachable(&name, contract, span);
        self.may_fail_here(&key, contract, span);
        self.a_call_that_may_pause(contract);
        // `Stats(first)` is the anonymous constructor of Kap 4.2, which the
        // lowering names `Stats::new` - and which hands back the type it is on,
        // whatever its declaration says about `Self`.
        let constructed = key
            .strip_suffix("::new")
            .filter(|_| !name.ends_with("::new"))
            .map(Ty::named);
        let result = self.arguments(&key, &name, contract, args, &found, &passed, span);
        // **What the arguments tell the signature**
        // ([ADR-074](../../docs/specification/adr/adr-074.md) D2). A free
        // function has no receiver, so ADR-031's binding had nothing to work
        // from and `hand(7)` handed back `?`; the same `bind` pointed at the
        // parameters answers `i64`. After `arguments` rather than before it,
        // because a variable fits everything and the check is therefore the
        // same either way round - while the *types* are not: a lambda's
        // parameters are typed from the signature (ADR-029), so the signature
        // has to reach them unsubstituted.
        let bound = from_arguments(contract, &found);
        let result = constructed.unwrap_or_else(|| ty::substitute(&result, &bound));
        self.stamped_through(contract, &found, result)
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
            // **The compiler writes the `&` at the call** (ADR-094 D1), where
            // the callee lends this position and the value is not already a
            // view. Asked before the wrap and the deref for `nullable_args`'
            // reason: a lent argument *is* a fit, and what is left is telling
            // the emitter what to write. The rule asks the fit itself, so an
            // argument that is simply the wrong value falls through to the
            // message it always had.
            if self.the_compiler_writes_the_reference(
                contract,
                Argument {
                    at,
                    given: given.get(at),
                    found,
                    want,
                },
                written,
                span,
            ) {
                continue;
            }
            let is_literal = given.get(at).is_some_and(is_literal);
            if let Some(how) = wrap_for(found, want, is_literal) {
                self.checked
                    .nullable_args
                    .entry((span.start, written.to_string(), at))
                    .or_default()
                    .insert(given.get(at).map(argument_shape).unwrap_or_default(), how);
                continue;
            }
            if self.fits_through_deref(found, want) {
                continue;
            }
            // A plain value where a hull is wanted. It is refused by a code of
            // its own because the sentence is worth more than a type mismatch's:
            // it names the hull and the value. **What the way out is changed**
            // ([ADR-064](../../docs/specification/adr/adr-064.md) D2) - the word
            // goes here, at the call, where it used to have to go somewhere else.
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
                    help: Some(match (&value, want) {
                        (Some(value), Ty::Named { name, .. }) => {
                            format!("write `{name}({value})` - a hull you can see is one you write")
                        }
                        (None, Ty::Named { name, .. }) => {
                            format!("write `{name}(…)` around it (Part I, 6.2)")
                        }
                        _ => format!("make it a `{}`", want.text()),
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
        // **Before the early return**, because the guard's question is asked of
        // exactly the calls this one declines to report: inside a `catch`,
        // `self.caught` is what sends this function home, and a call that
        // carries a `throws` is precisely what gives that `catch` something to
        // do ([ADR-091](../../../docs/specification/adr/adr-091.md)).
        if !contract.throws.is_empty() {
            if let Some(guarded) = &mut self.guarded {
                guarded.fallible = true;
            }
            // **And the lambda this may be inside**
            // ([ADR-102](../../docs/specification/adr/adr-102.md) D2). Here for
            // the same reason the line above is here: this is the one place
            // every written call — free or method — has its callee's contract
            // in hand, and a `catch` further out does not change what the
            // *lambda* was seen to do.
            if let Some(handed) = &mut self.handed_over {
                handed.fails = true;
                // **And `NK2606` is then the whole message.** Where the type
                // the lambda was handed to declares no failure, the function
                // *around* the lambda is not the one that has to answer for it
                // — the type is the contract, and telling the reader to write
                // `throws` on a function that does not fail would be a second
                // message for one mistake, in the wrong place. Where the type
                // **does** allow failing, the failure travels to the caller by
                // [ADR-029](../../docs/specification/adr/adr-029.md) D3 and
                // `NK2605` is right.
                if !handed.promised.may_fail {
                    return;
                }
            }
        }
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

    /// **`NK2206` and `NK2606`: a handler that does more than its type allows**
    /// ([ADR-102](../../docs/specification/adr/adr-102.md) D2).
    ///
    /// A lambda that does **less** fits a type that allows more: one that never
    /// pauses goes where pausing is allowed, one that cannot fail goes where
    /// failing is. The other direction is the assertion
    /// [ADR-027](../../docs/specification/adr/adr-027.md) makes about a
    /// declaration, made about somebody else's code — a caller who writes
    /// `fn() sync` in a signature has promised their own callers something, and
    /// a handler that pauses takes the promise away without saying so.
    ///
    /// **Two codes and not one**, because the reader is doing two different
    /// things: `NK2206` is the shape `NK2202` has one level over — a body that
    /// pauses where the word says it does not — and `NK2606` is `NK2605`'s, a
    /// failure with nowhere declared to go.
    ///
    /// **Only where the ledger answered.** A call whose callee nothing
    /// describes leaves both flags where they were, which is the silence every
    /// other rule here keeps about an absent claim
    /// ([Part III C.4](../../../docs/specification/30-nikaia-tooling.md)): a
    /// refusal on a guess is a correct program refused, and it is the worse of
    /// the two mistakes.
    fn a_handler_that_does_more_than_the_type_allows(
        &mut self,
        promised: Promises,
        seen: Handed,
        span: &Span,
    ) {
        if seen.pauses && !promised.may_pause {
            self.checked.findings.push(Finding {
                severity: Severity::Error,
                span: span.clone(),
                code: "NK2206",
                message: "this lambda can pause, and the parameter it is given to says `sync`"
                    .to_string(),
                notes: vec![
                    "a function type says what the code it names may do, and `sync` is the \
                     same assertion a declaration makes - made about somebody else's code \
                     (ADR-102 D2, ADR-027)"
                        .to_string(),
                    "a lambda that does less fits a type that allows more, never the other \
                     way round"
                        .to_string(),
                ],
                help: Some(
                    "keep what pauses outside the lambda and hand its answer in - or take \
                     the `sync` off the parameter's type, which tells this function's own \
                     callers what changed"
                        .to_string(),
                ),
            });
        }
        if seen.fails && !promised.may_fail {
            self.checked.findings.push(Finding {
                severity: Severity::Error,
                span: span.clone(),
                code: "NK2606",
                message: "this lambda can fail, and the parameter it is given to does not say \
                          `throws`"
                    .to_string(),
                notes: vec![
                    "nothing marks a failing call, so a failure leaves at a call exactly as \
                     it leaves at a block's closing brace (ADR-023 D8) - and here there is \
                     nowhere for it to go, because the type it was handed to declares none \
                     (ADR-102 D2)"
                        .to_string(),
                ],
                help: Some(
                    "handle it here, `… catch { … }` - or write `throws` on the parameter's \
                     type, which is what says the callee has to answer for it"
                        .to_string(),
                ),
            });
        }
    }

    /// **What a lambda's body was seen to do, one call at a time**
    /// ([ADR-102](../../docs/specification/adr/adr-102.md) D2).
    ///
    /// The failing half is recorded in [`Self::may_fail_here`], where every
    /// written call already has its callee's contract in hand. This is the
    /// pausing half, and it needs its own line because only the *method* path
    /// records pausing for the emitter — a free call's `.await` is written from
    /// the name, which the emitter can resolve itself (ADR-028).
    fn a_call_that_may_pause(&mut self, contract: &FnContract) {
        if contract.sync.is_sync() {
            return;
        }
        if let Some(handed) = &mut self.handed_over {
            handed.pauses = true;
        }
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
    /// [`Checked::method_options`]: the callee's options, in the declaration's
    /// order, for the emitter to make positional.
    ///
    /// Recorded from the **contract** and not from the call, so a call that
    /// leaves an option out still gets its default — which is the half the
    /// emitter could not work out on its own either.
    fn method_options(&mut self, method: Ident, contract: &FnContract, span: &Span) {
        let Some(signature) = contract.signature.as_ref() else {
            return;
        };
        if signature.config.is_empty() {
            return;
        }
        let options = signature
            .config
            .iter()
            .map(|option| (option.name.clone(), option.default.clone()))
            .collect();
        self.checked
            .method_options
            .insert((span.start, self.parsed.text(method).to_string()), options);
    }

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
            // **A handle is duplicated into the task, not moved**
            // ([ADR-040](../../docs/specification/adr/adr-040.md) D1). It is
            // recorded rather than refused, because the emitter is what writes
            // the step and it has no types (ADR-028) - the same arrangement
            // every other answer this module hands over uses. Unconditional, and
            // not only where the name is used again (D2): a line further down
            // may not decide what a line further up does to a cleanup point.
            if is_a_handle(&ty) {
                self.checked.task_handles.insert((span.start, name.clone()));
            }
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

    /// Note that `iter`'s sequence was walked here, where it is a plain name
    /// ([ADR-105](../../docs/specification/adr/adr-105.md) D2).
    ///
    /// **A name and nothing else**, which is the narrowing `NK2101` has for the
    /// same reason: `map.keys().collect()` walks a temporary, and a temporary has
    /// no second use to refuse.
    fn a_sequence_is_walked(&mut self, iter: &Expr, over: &Ty, span: &Span) {
        if !matches!(over, Ty::Seq { .. }) {
            return;
        }
        let Expr::Variable(name) = iter else {
            return;
        };
        let name = self.parsed.text(*name).to_string();
        self.walked.push((name, over.clone(), span.end));
    }

    /// **`NK2702`: a sequence walked a second time**
    /// ([ADR-105](../../docs/specification/adr/adr-105.md) D2).
    ///
    /// A produced sequence is walked **once**: the walk takes it by value, which
    /// is ADR-094 D2's *a value walked is a value kept*, and there is nothing
    /// left afterwards. A container is walked by view and as often as one likes,
    /// which is why nothing here asks about a `Vec`.
    ///
    /// **`NK2101`'s analysis with one word changed**, deliberately: the shape is
    /// the same question — a name used after something took it — and an
    /// assignment in between **revives** it, because giving the name a value
    /// again is a correct program.
    fn a_sequence_was_walked_twice(&mut self) {
        let walked = std::mem::take(&mut self.walked);
        let read = &self.read_at;
        let written = &self.written_at;
        let mut said: BTreeSet<usize> = BTreeSet::new();
        let mut findings = Vec::new();

        for (name, ty, at) in walked {
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
            // Once per site: a name walked twice and read three times is one
            // mistake, and three carets on it is three readings of the same line.
            if !said.insert(used) {
                continue;
            }
            let item = match &ty {
                Ty::Seq { item, .. } => item.text(),
                _ => "?".to_string(),
            };
            findings.push(Finding {
                code: "NK2702",
                severity: Severity::Error,
                span: Span {
                    start: used,
                    end: used,
                },
                message: format!("`{name}` is a sequence that was already walked"),
                notes: vec![
                    format!(
                        "a sequence of `{item}` produces its elements as they are asked \
                         for, so walking it consumes it - a `for`, a `collect()`, a \
                         `count()` and every other walk takes it by value (ADR-094 D2)"
                    ),
                    "a `Vec` is not this: a container has its elements already and is \
                     walked by view, as often as you like"
                        .to_string(),
                ],
                help: Some(format!(
                    "collect it first and walk the collection: \
                     `let {name} = {name}.collect()`"
                )),
            });
        }
        self.checked.findings.extend(findings);
    }

    /// **`NK2104`: two branches of an `overlap` meet on something**
    /// ([ADR-050](../../docs/specification/adr/adr-050.md) D3).
    ///
    /// **This is [ADR-033](../../docs/specification/adr/adr-033.md)'s analysis
    /// used the other way round**, and it is the return on machinery built for
    /// an inference that D1 withdraws. The touch sets no longer decide *whether
    /// the compiler may* overlap two statements; they decide *whether the
    /// programmer was right* to say so. Every mainstream form for "run these
    /// together" takes the programmer's word for the independence — here the
    /// claim is checked against what the ledger records each call touching.
    ///
    /// **A branch this compiler cannot account for is not refused.** `verdict`
    /// answers `NotAccountedFor` where a statement performs nothing it can
    /// name, and reading that as "they meet" would refuse a correct program on
    /// an absence (Part III, C.4) — which is the same polarity every other
    /// answer in this compiler takes about a thing nobody wrote down.
    fn branches_meet_on_nothing(&mut self, block: &Block, span: &Span) {
        use crate::contracts::order;

        // A branch that **binds** is refused on its own: the block's value
        // already carries every branch's result, so `let x = …` inside one
        // would name a thing that leaves by two doors (D2).
        for stmt in &block.stmts {
            if let Stmt::Let { names, .. } = &stmt.node {
                let bound: Vec<String> = names
                    .iter()
                    .map(|n| self.parsed.text(*n).to_string())
                    .collect();
                let name = bound.join("`, `");
                self.checked.findings.push(Finding {
                    code: "NK2104",
                    severity: Severity::Error,
                    span: stmt.span.clone(),
                    message: format!("a branch of an `overlap` binds `{name}`"),
                    notes: vec![
                        "each statement in the block is a branch, and the block's value is \
                         the tuple of their results in written order (Part I, 8.1.2)"
                            .to_string(),
                    ],
                    help: Some(format!(
                        "take the value from the block instead: \
                         `let ({name}, …) = overlap {{ … }}`"
                    )),
                });
            }
        }

        let operations: Vec<Option<order::Operation>> = block
            .stmts
            .iter()
            .map(|stmt| order::operation(self.parsed, &stmt.node, self.own, self.library))
            .collect();

        for (i, earlier) in operations.iter().enumerate() {
            for (j, later) in operations.iter().enumerate().skip(i + 1) {
                let (Some(earlier), Some(later)) = (earlier, later) else {
                    continue;
                };
                let verdict = order::verdict(earlier, later);
                if verdict.is_overlap() {
                    continue;
                }
                self.checked.findings.push(Finding {
                    code: "NK2104",
                    severity: Severity::Error,
                    span: block.stmts[j].span.clone(),
                    message: "these two branches cannot run together".to_string(),
                    notes: vec![
                        format!("{} (Part I, 8.1.2)", verdict.why()),
                        "`overlap` says the branches have no order between them, and two \
                         that meet on something do"
                            .to_string(),
                    ],
                    help: Some(
                        "if you meant them in order, write them as ordinary statements".to_string(),
                    ),
                });
                // One finding per branch: a branch that meets two others has
                // one thing wrong with it, and three messages about it is the
                // same mistake told three times.
                break;
            }
        }
        let _ = span;
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
        // **Two spellings of one claim, and they are the same claim.**
        // `[type."io::Lines"] iterates = "throws"` says it of a named type, and
        // [ADR-105](../../docs/specification/adr/adr-105.md) D1's `throws` after
        // a `Seq[T]` says it of the sequence a signature hands back — *`throws`
        // on a `Seq` is the one spelling of a step can fail*, which is that
        // record's §3. The second is the one a produced sequence can use,
        // because it has no name in the `types` table to hang a column on.
        // The name a message uses is the **ledger's** for a named type and a
        // description for a `Seq`, because D4 says a program cannot write
        // `Seq[String]` — and a message that names a spelling its reader has no
        // way to type is the shape Part III C.1 is about.
        let name = match over {
            Ty::Named { name, .. } if self.iterates_fallibly(name) => format!("`{name}`"),
            Ty::Seq {
                throws: true, item, ..
            } => match &**item {
                Ty::Unknown => "a sequence".to_string(),
                item => format!("a sequence of `{item}`"),
            },
            _ => return,
        };
        let name = &name;

        // A stream of pairs does not exist in `std`, and taking one apart while
        // also unwrapping a failure is a shape to design rather than to guess
        // at (ADR-025 §7).
        if bindings != 1 {
            self.checked.findings.push(Finding {
                severity: Severity::Error,
                span: span.clone(),
                code: "NK2701",
                message: format!("a `for` over {name} binds one name, and this binds {bindings}"),
                notes: vec![format!(
                    "each turn of {name} can fail, and the failure is what the one binding unwraps"
                )],
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
                "{name} reads as it goes, and a read can fail - so the failure leaves this \
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

    /// **What a task holds across a pause** ([ADR-055](../../docs/specification/adr/adr-055.md)
    /// §2 D6's third sharp edge), which is `NK2501`'s question asked of what a
    /// body **binds** rather than of what it captures.
    ///
    /// A task's body is an `async` block below, and a value bound inside it and
    /// still live at a suspension point further down is held **inside the
    /// future**. The pool's starter asks for `Send` of that whole future, so
    /// such a value crosses a thread exactly as a captured one does — and until
    /// now the only thing that said so was the backend, about the generated
    /// file, which [Part III C.1](../../docs/specification/30-nikaia-tooling.md)
    /// calls a bug in this compiler.
    ///
    /// **Live is over-approximated, and the over-approximation is bounded by
    /// the verdict rather than by the walk.** *Bound before a pause and named
    /// after it* is all this asks; Rust's own answer is narrower. That would be
    /// a *correct program refused* ([Part III
    /// C.4](../../docs/specification/30-nikaia-tooling.md)) if the refusal
    /// stood on it alone — it does not. It stands on `contracts::send`'s
    /// `MayNot`, which is a claim about a **type** and is never `Undecided`'s
    /// silence, so a name this walk is too generous about is refused only if a
    /// value of its type could not have crossed from anywhere.
    ///
    /// **It reports nothing today, and that is why it is built now.** No type
    /// answers `MayNot` at `Destination::Ours`: `Shared` stopped being one when
    /// [ADR-037](../../docs/specification/adr/adr-037.md) D6 gave it one
    /// representation at both settings, and a lock is `MayNot` only at a
    /// *foreign* destination. A refusal costs nothing before there are programs
    /// it would reject, and the same refusal added afterwards breaks them. What
    /// reaches it first is a type from outside this language, which is what
    /// [ADR-104](../../docs/specification/adr/adr-104.md) is for.
    fn a_task_holds_these_across_a_pause(
        &mut self,
        body: &Block,
        bound: &[(String, Ty, usize)],
        span: &Span,
    ) {
        if bound.is_empty() {
            return;
        }
        let (pauses, named) = self.what_the_body_does(body);
        let places: Vec<(String, usize)> = bound
            .iter()
            .map(|(name, _, at)| (name.clone(), *at))
            .collect();
        let held = send::held_across_a_pause(&places, &pauses, &named);
        self.checked.held_across_a_pause.extend(
            held.iter()
                .map(|name| (span.start, name.clone()))
                .collect::<Vec<_>>(),
        );
        for (name, ty, _) in bound.iter().filter(|(name, ..)| held.contains(name)) {
            let crossing = send::crossing(ty, self.own, self.library, send::Destination::Ours);
            if crossing.refused().is_none() {
                continue;
            }
            let notes = [
                Some(
                    "a task's body is one function below, so a value it binds and still \
                     uses after a pause is held inside it - and the task runs on a thread \
                     of its own (Part II, 11.2)"
                        .to_string(),
                ),
                crossing.note(),
                Some(SAME_AT_BOTH.to_string()),
            ];
            self.checked.findings.push(Finding {
                severity: Severity::Error,
                span: span.clone(),
                code: "NK2501",
                message: format!(
                    "`{name}` may not cross into a task, and this task holds it across a pause"
                ),
                notes: notes.into_iter().flatten().collect(),
                help: crossing.way_out(),
            });
        }
    }

    /// Where a block **pauses**, and where each name it mentions was last
    /// mentioned — both by the byte a statement starts at, and both reaching
    /// through the blocks a statement holds.
    ///
    /// A method call is answered from [`Checked::pausing_methods`], which the
    /// walk that just ran filled: which entry `db.load()` goes to is the type
    /// checker's answer ([ADR-028](../../docs/specification/adr/adr-028.md)). A
    /// free call is answered from the ledgers directly, the way
    /// [`contracts::sync`] answers it.
    fn what_the_body_does(&self, body: &Block) -> (Vec<usize>, BTreeMap<String, usize>) {
        let mut pauses = Vec::new();
        let mut named: BTreeMap<String, usize> = BTreeMap::new();
        self.walk_the_body(body, &mut pauses, &mut named);
        (pauses, named)
    }

    fn walk_the_body(
        &self,
        body: &Block,
        pauses: &mut Vec<usize>,
        named: &mut BTreeMap<String, usize>,
    ) {
        for stmt in &body.stmts {
            let at = stmt.span.start;
            crate::contracts::sync::visit_stmt(self.parsed, &stmt.node, &mut |expr| {
                if self.pauses_here(expr, at) {
                    pauses.push(at);
                }
                for name in crate::contracts::send::names_used(self.parsed, expr) {
                    let last = named.entry(name).or_insert(at);
                    *last = (*last).max(at);
                }
            });
            // The blocks a statement holds - an `if`, a loop, a lambda. Their
            // statements have spans of their own, so the order is the source's.
            crate::contracts::sync::visit_stmt_blocks(&stmt.node, &mut |inner| {
                self.walk_the_body(inner, pauses, named);
            });
        }
    }

    /// Whether this one expression is a suspension point.
    fn pauses_here(&self, expr: &Expr, at: usize) -> bool {
        match crate::contracts::sync::reached(self.parsed, expr, self.own, self.library) {
            Some(crate::contracts::sync::Reached::Method) => match expr {
                Expr::MethodCall { method, .. } | Expr::SafeMethod { method, .. } => self
                    .pausing_methods
                    .contains(&(at, self.parsed.text(*method).to_string())),
                _ => false,
            },
            Some(crate::contracts::sync::Reached::Own(name)) => self
                .own
                .functions
                .get(&name)
                .is_some_and(|c| !c.sync.is_sync()),
            Some(crate::contracts::sync::Reached::Library { sync, .. }) => !sync,
            // A call this compiler cannot name may do anything, pausing
            // included - which is the fail-closed direction for a question
            // whose wrong answer here is a value silently crossing a thread.
            Some(crate::contracts::sync::Reached::Opaque(_)) => true,
            None => false,
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

    /// Whether any ledger or this unit declares a **type** by this name.
    ///
    /// Presence and not fields, which is the difference from [`Self::fields_of`]:
    /// *an empty field list means nothing recorded and not nothing inside*
    /// (Part III 15.2), so a described foreign type has an entry and no fields,
    /// and it is declared.
    fn declares_a_type(&self, name: &str) -> bool {
        if self.structs.contains_key(name) || self.own.types.contains_key(name) {
            return true;
        }
        // `Self` inside an `impl`, and the type parameters an item brought in:
        // both are names a literal may wear and neither is in a `types` map.
        if name == "Self" || self.enums.contains_key(name) {
            return true;
        }
        let suffix = format!("::{name}");
        self.library
            .types
            .keys()
            .any(|key| key == name || key.ends_with(&suffix))
    }

    /// `NK1135` for a struct literal, with the sentence a *call* needs.
    ///
    /// The same code a written annotation gets, because it is the same claim -
    /// a name in type position that nothing declares. What differs is the way
    /// out, and it differs because of one shape: where the name is a **function**
    /// the author almost certainly meant
    /// [ADR-133](../../docs/specification/adr/adr-133.md) D1's options-only call,
    /// which this compiler does not parse yet, and *nothing declares a struct*
    /// would send them looking for a `struct` they never wanted.
    fn a_struct_nothing_declares(&mut self, name: &str, span: &Span) {
        let is_a_function = self.own.functions.contains_key(name)
            || self.library.lookup(name).is_some()
            || self.own.functions.contains_key(&format!("{name}::new"));
        self.checked.findings.push(Finding {
            severity: Severity::Error,
            span: span.clone(),
            code: "NK1135",
            message: format!("nothing declares a struct called `{name}`"),
            notes: vec![match is_a_function {
                true => format!(
                    "a struct literal names a type, and `{name}` is a **function**. A \
                     call whose arguments are all options is written \
                     `{name}(option: value)` since ADR-133 D1 - with parentheses and no \
                     `;`, which is what the braces here would have been"
                ),
                false => "a struct literal names a type, and this name is not one Part I \
                          2.2 offers, nor one a ledger declares, nor one this file \
                          declares (ADR-096)"
                    .to_string(),
            }],
            help: Some(match is_a_function {
                true => format!("write `{name}(option: value)`"),
                false => "declare the `struct`, or correct the name".to_string(),
            }),
        });
    }

    /// `NK1146`: a struct literal written like a call
    /// ([ADR-140](../../docs/specification/adr/adr-140.md) D1).
    ///
    /// `Stats(min: first, max: first)` and `Reading { name, temp }` both built a
    /// struct, and `Stats(first)` called the anonymous constructor - so `Foo(x: 1)`
    /// went *round* a type's invariants and `Foo(1)` went *through* them, told
    /// apart by a colon. D1 keeps the braces and gives the parentheses to the
    /// call, which is also what freed [ADR-133](../../docs/specification/adr/adr-133.md)
    /// D1's options-only call: the two were one spelling.
    ///
    /// The message carries the whole rewrite rather than the rule, because the
    /// rewrite is mechanical and the reader has the fields in hand.
    fn a_literal_written_like_a_call(
        &mut self,
        name: &str,
        config: &[ast::ConfigArg],
        span: &Span,
    ) {
        let fields = config
            .iter()
            .map(|a| self.parsed.text(a.name).to_string())
            .collect::<Vec<_>>()
            .join(": …, ");
        self.checked.findings.push(Finding {
            severity: Severity::Error,
            span: span.clone(),
            code: "NK1146",
            message: format!("a struct literal is written with braces, and `{name}` is a type"),
            notes: vec![
                "`Name(field: value)` was a second spelling of `Name { field: value }` \
                 and is gone (ADR-140 D1): the parentheses are a call now, so \
                 `Name(a, b)` reaches the anonymous constructor and nothing goes round \
                 it by carrying a colon"
                    .to_string(),
            ],
            help: Some(format!("write `{name} {{ {fields}: … }}`")),
        });
    }

    /// `NK1144`: a `let` whose only name is the ignore pattern
    /// ([ADR-126](../../docs/specification/adr/adr-126.md) D2).
    fn a_let_that_binds_nothing(&mut self, span: &Span) {
        self.checked.findings.push(Finding {
            severity: Severity::Error,
            span: span.clone(),
            code: "NK1144",
            message: "`_` ignores a value inside a pattern, and this `let` has nothing else \
                      to bind"
                .to_string(),
            notes: vec![
                "a call made for its effect is written as the call, `f()`; a resource torn \
                 down at a moment of the program's choosing is closed by name or lives in a \
                 scope it can end with (Part I, 6.4)"
                    .to_string(),
                "and what this lowers to is Rust's `let _ =`, which **discards** the value \
                 rather than binding it - so the cleanup runs here and not at the end of the \
                 block, which is the one thing a reader cannot see in the line"
                    .to_string(),
            ],
            help: Some("write the expression as a statement, or bind it to a name".to_string()),
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
        self.bind_local(Local {
            name,
            ty,
            constant,
            immutable: None,
        });
    }

    fn bind_local(&mut self, local: Local) {
        if let Some(frame) = self.scope.last_mut() {
            frame.push(local);
        }
    }

    fn lookup(&self, name: &str) -> Option<Ty> {
        self.local(name).map(|(ty, _)| ty)
    }

    /// The innermost binding of a name, whole.
    fn binding(&self, name: &str) -> Option<&Local> {
        self.scope
            .iter()
            .rev()
            .find_map(|frame| frame.iter().rev().find(|local| local.name == name))
    }

    /// The innermost binding of a name: its type, and the constant it stands
    /// for where the checker could evaluate one.
    fn local(&self, name: &str) -> Option<(Ty, Option<i128>)> {
        self.scope
            .iter()
            .rev()
            .find_map(|frame| frame.iter().rev().find(|local| local.name == name))
            .map(|local| (local.ty.clone(), local.constant))
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
    ///
    /// `declared_here` says whether the signature came from **this program's**
    /// ledger rather than from `std`'s. It decides one thing and
    /// [ADR-122](../../docs/specification/adr/adr-122.md) D3 is why: a `std`
    /// entry's lambda type is a hand-written description of a **Rust**
    /// signature, which takes a plain closure whatever its `sync` column says,
    /// so the future shape must not be written for one.
    fn arguments_given(
        &mut self,
        args: &[Expr],
        expected: &[Ty],
        declared_here: bool,
        span: &Span,
    ) -> Vec<Ty> {
        args.iter()
            .enumerate()
            .map(|(at, arg)| match (arg, expected.get(at)) {
                (
                    Expr::Closure {
                        params,
                        mutable,
                        body,
                    },
                    Some(Ty::Fn {
                        params: given,
                        is_sync,
                        throws,
                        ..
                    }),
                ) => {
                    // **A lambda handed to a parameter whose type may pause is
                    // written as a closure returning a boxed future**
                    // ([ADR-122](../../docs/specification/adr/adr-122.md) D1).
                    // Recorded here because it is a question about the
                    // parameter's **type**, which the emitter has no way to ask
                    // ([ADR-028](../../docs/specification/adr/adr-028.md)) — the
                    // same arrangement `lent_args` and `nullable_args` have, and
                    // keyed the same way.
                    if !is_sync && declared_here {
                        self.checked.future_lambdas.insert((span.start, at));
                    }
                    self.lambda(
                        params,
                        mutable,
                        body,
                        given,
                        Promises {
                            may_pause: !is_sync,
                            may_fail: *throws,
                        },
                        span,
                    )
                }
                _ => {
                    self.a_field_of_a_borrowed_subject(arg, span, "passed");
                    self.expr(arg, span)
                }
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
        mutable: &[winnow_grammar::Symbol],
        body: &Block,
        given: &[Ty],
        promised: Promises,
        span: &Span,
    ) -> Ty {
        let frame = params
            .iter()
            .enumerate()
            .map(|(at, p)| {
                self.lambda_parameter(
                    *p,
                    mutable,
                    given.get(at).cloned().unwrap_or(Ty::Unknown),
                    span,
                )
            })
            .collect();

        self.scope.push(frame);
        // The other door into a lambda's body, and it needs the same boundary
        // as the one in `expr`: which of the two a lambda arrives through is
        // whether the callee's signature typed its parameters, and that has
        // nothing to do with what a `break` in it may reach.
        let seen = self.past_a_boundary("lambda", |me| {
            me.handed_over = Some(Handed {
                pauses: false,
                fails: false,
                promised,
            });
            me.block(body);
            me.handed_over.take()
        });
        self.scope.pop();
        if let Some(seen) = seen {
            self.a_handler_that_does_more_than_the_type_allows(promised, seen, span);
        }
        // What a lambda hands back is not written down anywhere yet, and
        // claiming it here would be inventing one (ADR-029 D1).
        Ty::Unknown
    }

    /// Walk something the language below makes **a function of its own**.
    ///
    /// A lambda, a task, an `overlap` branch: three constructs that are one
    /// block above and a closure or an `async` block below. A loop outside one
    /// of them is not reachable from inside it, so the count starts again at
    /// zero and the word for what stands in the way is carried for the message.
    fn past_a_boundary<T>(&mut self, what: &'static str, walk: impl FnOnce(&mut Self) -> T) -> T {
        let loops = std::mem::replace(&mut self.loops, 0);
        let barrier = self.barrier.replace(what);
        // **And what a lambda was seen to do stops here too**
        // ([ADR-102](../../docs/specification/adr/adr-102.md) D2), for the same
        // reason the loop count does: what a body written *inside* this one
        // does happens when its own callee runs it. `lambda` sets its own
        // accumulator inside the walk and reads it back there.
        let handed = self.handed_over.take();
        let value = walk(self);
        self.handed_over = handed;
        self.loops = loops;
        self.barrier = barrier;
        value
    }

    /// `NK1134`: a `catch` over an expression that cannot fail
    /// ([ADR-091](../../../docs/specification/adr/adr-091.md)).
    ///
    /// The lowering makes a `catch` a `match` over a `Result`, so a guarded
    /// expression that is not one produces `E0308` about the generated file -
    /// [Part III C.1](../../../docs/specification/30-nikaia-tooling.md)'s class,
    /// naming a `match` and an `Ok` arm nobody wrote.
    ///
    /// **Refused rather than dropped**, and that is the decision rather than
    /// the smaller change. Emitting the expression without the `match` would
    /// take the program the author wrote and quietly delete a block from it -
    /// including a `return` inside the handler, which
    /// [ADR-034](../../../docs/specification/adr/adr-034.md) makes the
    /// *function's* return. A handler that never runs is a belief about the
    /// program, and a belief that is wrong is worth a sentence.
    ///
    /// **Only where the ledger says so.** [`Self::may_fail_here`] refuses a
    /// call that can fail outside a `catch`, reported "only where the callee's
    /// contract **says** it can fail"; this is that standard read from the
    /// other end, and [`Guarded`] carries the two answers apart so that *could
    /// not look it up* never becomes *cannot fail*.
    fn nothing_here_can_fail(&mut self, guarded: Guarded, span: &Span) {
        if guarded.fallible || guarded.unanswered {
            return;
        }
        self.checked.findings.push(Finding {
            severity: Severity::Error,
            span: span.clone(),
            code: "NK1134",
            message: "nothing in this expression can fail, so the `catch` has nothing to handle"
                .to_string(),
            notes: vec![
                "a failure comes from a call whose contract carries a `throws` (Part I, 7.1)"
                    .to_string(),
                "every call here is one this compiler could look up, and none of them \
                 declares one"
                    .to_string(),
                "a call no contract describes would leave this unsaid rather than \
                 refused (Part III, C.4)"
                    .to_string(),
            ],
            help: Some(
                "delete the `catch` and its handler: the expression is the value".to_string(),
            ),
        });
    }

    /// `NK2205`: a `set` whose argument reads the same container with `get`
    /// ([ADR-039](../../../docs/specification/adr/adr-039.md) D10,
    /// [ADR-099](../../../docs/specification/adr/adr-099.md)).
    ///
    /// `kasse.set(kasse.get() + 100)` takes the lock **twice** — once to read
    /// and once to store — and between the two the value can change, so what is
    /// stored is computed from a state that may no longer hold. Making a new
    /// value out of the old one is what the third door is for, and it takes the
    /// lock once.
    ///
    /// **Syntactic, on purpose** (D10's own line): it catches the `get`
    /// written *inside* the `set`, which is the shape people write, and not the
    /// same pair spread over two lines. The second is a question about what
    /// happened between two statements, and nothing here answers that — so
    /// widening this rule would mean guessing, and the narrow one is right
    /// about what it does see.
    /// **The stamp passes through a call the callee cannot put it back with**
    /// ([ADR-111](../../docs/specification/adr/adr-111.md) D2).
    ///
    /// A call whose argument is stamped is allowed where the callee's `touches`
    /// column names no lock — it cannot write the value into one — and **its
    /// result is stamped**. That is what makes `kasse.set(bumped(stand))` the
    /// same answer as `kasse.set(stand + 1)` without an analysis following the
    /// value through `bumped`.
    ///
    /// **A callee that touches a lock, or that nothing describes, does not
    /// pass one on.** An absent `touches` reads as *touches everything*
    /// ([ADR-033](../../docs/specification/adr/adr-033.md)), so a call nobody
    /// wrote down hands back a plain value rather than a stamped one: the
    /// wrong answer that way costs a refusal that is not raised, and the wrong
    /// answer the other way refuses a program (C.4).
    fn stamped_through(&self, contract: &FnContract, given: &[Ty], result: Ty) -> Ty {
        if result.is_seen() || !given.iter().any(|ty| ty.is_seen()) {
            return result;
        }
        let reaches_a_lock =
            !contract.touches_known || contract.touches.iter().any(|t| t.names_a_lock());
        match reaches_a_lock {
            true => result,
            false => Ty::seen(result.unseen()),
        }
    }

    /// **`NK2207`: an `update` block that assigns to its `mut v` without
    /// reading it** ([ADR-111](../../docs/specification/adr/adr-111.md) D4).
    ///
    /// `update fn(mut v) { v = n }` is a `set` through the back door and is
    /// refused as one: nothing about it decides *inside* the lock, which is
    /// what the door is for. A block that **reads** `v` — `v += n`,
    /// `v.push(e)`, `if v > 100 { v = 0 }` — is the door working.
    ///
    /// **Read means mentioned anywhere but as the whole target**, which is the
    /// safe direction for a refusal: `v += n` mentions it, `v.f = n` mentions
    /// it, and only `v = …` does not. Asking a narrower question would refuse a
    /// block that reads `v` in a way this walk did not recognise, and that is
    /// [Part III C.4](../../docs/specification/30-nikaia-tooling.md)'s correct
    /// program refused.
    fn an_update_block_reads_what_it_writes(
        &mut self,
        params: &[winnow_grammar::Symbol],
        body: &Block,
        span: &Span,
    ) {
        for name in params {
            let name = self.parsed.text(*name).to_string();
            let mut assigned = false;
            let mut read = false;
            for stmt in &body.stmts {
                // A whole-name assignment is the shape the rule is about; a
                // mention anywhere else is a read.
                if let Stmt::Assign { target, op, value } = &stmt.node {
                    if op.is_none()
                        && matches!(target, Expr::Variable(n) if self.parsed.text(*n) == name)
                    {
                        assigned = true;
                        // **And the value being stored is still a read**:
                        // `v = v + 1` mentions `v`, and it decides inside the
                        // lock exactly as `v += 1` does.
                        let mentioned = crate::contracts::send::names_used(self.parsed, value);
                        read |= mentioned.contains(&name);
                        continue;
                    }
                }
                let mentioned = crate::contracts::send::names_used_in_stmt(self.parsed, &stmt.node);
                read |= mentioned.contains(&name);
            }
            if !assigned || read {
                continue;
            }
            self.checked.findings.push(Finding {
                severity: Severity::Error,
                span: span.clone(),
                code: "NK2207",
                message: format!(
                    "this `update` block replaces `{name}` without reading it, which is a `set`"
                ),
                notes: vec![
                    "the door exists to decide **inside** the lock; a block that only stores \
                     takes the lock for nothing the `set` door does not already do \
                     (ADR-111 D4)"
                        .to_string(),
                ],
                help: Some(
                    "use `set` if the value is computed outside - or read the old value \
                     here, which is what the block is for: `v += n`, `v.push(e)`, \
                     `if v > 100 { v = 0 }`"
                        .to_string(),
                ),
            });
        }
    }

    /// **The witness takes no `&`, and a written one is `NK1137`**
    /// ([ADR-094](../../docs/specification/adr/adr-094.md) D1, at
    /// [ADR-111](../../docs/specification/adr/adr-111.md) D5's door).
    ///
    /// The ledger declares `seen: &$T` — the witness is read and never stored,
    /// so the caller keeps it — and the `&` that says so is the compiler's, as
    /// it is everywhere else. It is refused here rather than through `lends`
    /// because `lends` withholds its claim on every *method* argument
    /// ([ADR-028](../../docs/specification/adr/adr-028.md): the emitter cannot
    /// resolve a receiver), and this one position the emitter is told about
    /// outright. Without the refusal a written `&` would come out `&&`, which
    /// is `rustc`'s words about a file nobody wrote (Part III C.1).
    fn the_witness_takes_no_reference(&mut self, seen: &Expr, span: &Span) {
        if !matches!(
            seen,
            Expr::Unary {
                op: ast::UnaryOp::Ref,
                ..
            }
        ) {
            return;
        }
        self.checked.findings.push(Finding {
            severity: Severity::Error,
            span: span.clone(),
            code: "NK1137",
            message: "the `&` here is the compiler's to write".to_string(),
            notes: vec![
                "`after:` is a witness the door only reads, so the reference is what the \
                 call already means (ADR-094 D1, ADR-111 D5)"
                    .to_string(),
            ],
            help: Some("take the `&` off".to_string()),
        });
    }

    /// **`NK1143`: a call to an `extern` name outside an `unsafe` block**
    /// ([ADR-124](../../docs/specification/adr/adr-124.md) D3).
    ///
    /// That is the whole of what the word buys, and it is why it is a word
    /// rather than an attribute on the declaration: the boundary is visible
    /// **at the call**, in the body somebody reads, rather than in a file
    /// beside it.
    ///
    /// **This compiler's refusal and not the language below's.** Rust makes the
    /// functions of an `extern` block `unsafe fn`, so the program would be
    /// refused either way — with a message about a generated file, which is
    /// [Part III C.1](../../../docs/specification/30-nikaia-tooling.md)'s class.
    ///
    /// **Only a name this file declared.** A Nikaia function and a C
    /// declaration are both ledger entries, and only one of them is this; the
    /// set comes from the item tree, so a name nothing here declared is not
    /// this refusal's business.
    fn a_foreign_call_outside_unsafe(&mut self, name: &str, span: &Span) {
        if self.inside_unsafe || !self.foreign_names.contains(name) {
            return;
        }
        self.checked.findings.push(Finding {
            severity: Severity::Error,
            span: span.clone(),
            code: "NK1143",
            message: format!(
                "`{name}` is declared `extern` and this call is not in an `unsafe` block"
            ),
            notes: vec![
                "C is not memory-safe, so the boundary is written where it is crossed \
                 rather than once beside the declaration (Part III, 15.1; ADR-124 D3)"
                    .to_string(),
                "the block makes no other rule: what is inside it is checked exactly as \
                 anything else is"
                    .to_string(),
            ],
            help: Some(format!("write `unsafe {{ {name}(…) }}`")),
        });
    }

    /// **`NK2208`: the lowering of a door, written as a door**
    /// ([ADR-111](../../docs/specification/adr/adr-111.md) D5).
    ///
    /// `kasse.set(neu; after: stand)` lowers to `set_after(neu, stand)`, and
    /// what is below is reachable by name from above: `kasse.set_after(a, b)`
    /// is a Rust method that exists, takes two arguments, and hands back a
    /// `Result`. Nothing in the ledger describes it, so nothing would refuse it
    /// and nothing would write the `?` — the failure would be **dropped**, and
    /// `rustc` would then warn about an unused `Result` in a file nobody wrote
    /// (Part III C.1).
    ///
    /// **So it is refused by name**, which is the cheap half of D5's *one
    /// door*: a program has one way to ask for a compare-and-store, the message
    /// says what it is, and the stamp discipline has no second entrance. The
    /// receiver has to be a lock — `set_after` on a type of the program's own
    /// is that program's own method and nothing to do with this.
    fn a_door_that_is_not_written(&mut self, on: &Ty, written: &str, span: &Span) {
        if written != "set_after" {
            return;
        }
        let Ty::Named { name, .. } = on else {
            return;
        };
        if !is_hull(name) {
            return;
        }
        let container = match &self.set_receiver {
            Some(name) => name.clone(),
            None => "this lock".to_string(),
        };
        self.checked.findings.push(Finding {
            severity: Severity::Error,
            span: span.clone(),
            code: "NK2208",
            message: format!(
                "`{container}` has no `set_after`; it is how `set(…; after: …)` is written below"
            ),
            notes: vec![
                "the compare and the store happen while the lock is open once, and the door \
                 that asks for that is `set` with a witness (ADR-111 D5)"
                    .to_string(),
                "written this way the failure would be dropped rather than propagated, \
                 because nothing in the contracts describes this name"
                    .to_string(),
            ],
            help: Some(format!(
                "write `{container}.set(neu; after: seen)`, where `seen` is what the lock \
                 handed out"
            )),
        });
    }

    /// **`NK2205`: a `set` given a stamped value, or standing under a stamped
    /// condition** ([ADR-111](../../docs/specification/adr/adr-111.md) D4).
    ///
    /// `kasse.set(stand + 100)` is a read-modify-write through two doors: the
    /// lock is taken once to read and once to store, and anything may happen
    /// between. It is refused **whether `stand` was read on the line above, in
    /// another function, or in another request** — which is what the stamp
    /// buys, since the value carries where it came from and no analysis has to
    /// follow it.
    ///
    /// **The second shape is the condition.** `if stand > 100 { kasse.set(0) }`
    /// stores a plain value, and the *decision* is the stale thing. A `while`,
    /// a `match` and a nested `if` are conditions alike.
    ///
    /// **There is no way around either** (D4): no `.value`, no `overwrite`, no
    /// word that takes a stamp off. `set` is for a starting value, a
    /// configuration that arrived from outside, a reset an operator asked for —
    /// and `set(neu; after: seen)` is the one door for a stamped one, which is
    /// D5 and is not built.
    ///
    /// **It used to ask a narrower question**: whether the argument contained a
    /// `get` **on the same container**, which caught the one line and nothing
    /// else. The stamp is what widened it, and widened it without an analysis.
    fn a_set_that_reads_what_it_writes(
        &mut self,
        on: &Ty,
        written: &str,
        found: &[Ty],
        span: &Span,
    ) {
        // **`set(after)` is not `set`**, and that is D5's whole point: the
        // witness covers both shapes, so neither is refused there. It is the
        // ledger's name that decides rather than a flag, because the name is
        // what already says which operation this is.
        if written != "set" {
            return;
        }
        let Ty::Named { name, .. } = on else {
            return;
        };
        if !is_hull(name) {
            return;
        }
        let container = self
            .set_receiver
            .clone()
            .unwrap_or_else(|| "this lock".to_string());

        // **The value**, wherever it was read.
        if found.iter().any(|ty| ty.is_seen()) {
            self.checked.findings.push(Finding {
                severity: Severity::Error,
                span: span.clone(),
                code: "NK2205",
                message: format!(
                    "this `set` stores a value that was read from a lock, so `{container}` is \
                     taken twice"
                ),
                notes: vec![
                    "`set` is for a value computed outside the lock; a value a lock handed \
                     out is stale the moment the lock is let go, and anything may happen \
                     between the read and the store (ADR-111 D4)"
                        .to_string(),
                    "the stamp travels with the value, so this is the same answer whether it \
                     was read on the line above, in another function, or in another request"
                        .to_string(),
                ],
                help: Some(format!(
                    "write `{container}.update fn(mut v) {{ … }}`, which decides inside the \
                     lock (ADR-110 D1)"
                )),
            });
            return;
        }

        // **And the decision.** The value stored is plain; what is stale is the
        // condition this stands under.
        if let Some(at) = self.stamped_condition {
            let _ = at;
            self.checked.findings.push(Finding {
                severity: Severity::Error,
                span: span.clone(),
                code: "NK2205",
                message: format!(
                    "this `set` stands under a condition read from a lock, so `{container}` is \
                     taken twice"
                ),
                notes: vec![
                    "the value stored is plain and the **decision** is not: what the lock \
                     said may have changed before this line runs (ADR-111 D4)"
                        .to_string(),
                ],
                help: Some(format!(
                    "write `{container}.update fn(mut v) {{ … }}` and take the decision inside \
                     the lock (ADR-110 D1)"
                )),
            });
        }
    }

    /// `NK2204`: an assignment straight into a `SharedMut`
    /// ([ADR-039](../../../docs/specification/adr/adr-039.md) D10,
    /// [ADR-099](../../../docs/specification/adr/adr-099.md)).
    ///
    /// `kasse = 42` looks like an ordinary assignment and is not: the lock has
    /// to be taken for the write, and what takes it is a **door**. Without this
    /// the program meets `rustc` about a type it never wrote — the hull, not
    /// the value inside it — which is
    /// [Part III C.1](../../../docs/specification/30-nikaia-tooling.md).
    ///
    /// **The message names the door with the value in it**, because the repair
    /// is mechanical and a help line that says *"use a door"* leaves the reader
    /// to work out which of the four.
    fn a_write_that_skips_the_door(&mut self, target: &Expr, into: &Ty, value: &Expr, span: &Span) {
        let Ty::Named { name, .. } = into else {
            return;
        };
        if name != SHARED_MUT {
            return;
        }
        // The name, where the target is one. A field or an index into a hull is
        // a shape nothing decides yet, and saying nothing is the direction that
        // cannot refuse a program that is right.
        let Expr::Variable(bound) = target else {
            return;
        };
        let bound = self.parsed.text(*bound).to_string();
        self.checked.findings.push(Finding {
            severity: Severity::Error,
            span: span.clone(),
            code: "NK2204",
            message: format!(
                "`{bound}` holds shared mutable state, and this assigns to it directly"
            ),
            notes: vec![
                "a write goes through a door, because the lock has to be taken for it \
                 (Part II, 12.2)"
                    .to_string(),
            ],
            help: Some(format!("write `{bound}.set({})`", self.written(value))),
        });
    }

    /// The value as the author wrote it, where this compiler can say so.
    ///
    /// **An expression has no span** — [ADR-081](../../../docs/specification/adr/adr-081.md)
    /// D2 gave one to `Binary` and to nothing else — so there is no source text
    /// to quote back. What can be rebuilt is rebuilt, and the rest is an
    /// ellipsis rather than a guess: a help line that quoted the wrong thing
    /// would be worse than one that quotes nothing, and the shape the
    /// specification's own example writes (`kasse = 42`) is a literal.
    fn written(&self, value: &Expr) -> String {
        match value {
            Expr::LitInt(n) => n.to_string(),
            Expr::LitBool(yes) => yes.to_string(),
            Expr::LitStr(text) => format!("{text:?}"),
            Expr::Variable(name) => self.parsed.text(*name).to_string(),
            Expr::Unary {
                op: UnaryOp::Neg,
                expr,
            } => format!("-{}", self.written(expr)),
            _ => "…".to_string(),
        }
    }

    /// **A `let` that binds several names at once**
    /// ([ADR-098](../../../docs/specification/adr/adr-098.md)).
    ///
    /// Part I 8.1.2 writes `let (user, rights, prefs) = overlap { … }` and Part
    /// II 12.5 writes `let (tx, rx) = channel::bounded(100)`. Both take a value
    /// apart by **position**, and neither writes a type - so this binds each
    /// name to the part at its index where the value's type is a tuple of the
    /// right width, and to `?` where it is not.
    ///
    /// **The widths are not compared**, and that is deliberate rather than
    /// unfinished. A value whose type this compiler cannot see is `?`, and
    /// refusing a destructure of one because it *looks* like the wrong width
    /// would refuse a correct program
    /// ([Part III C.4](../../../docs/specification/30-nikaia-tooling.md)). What
    /// a wrong width costs today is the language below saying so about a `let`
    /// the author did write, which is a smaller and honest failure - and the
    /// day the tuple's own width is known for certain is the day to refuse it
    /// here.
    ///
    /// **A written type is refused**, because `let (a, b): T = …` would have to
    /// say which name `T` is about and nothing decides that. Refused rather than
    /// ignored: ignoring it takes something the author wrote and drops it.
    fn tuple_let(
        &mut self,
        names: &[Ident],
        ty: Option<&crate::ast::Type>,
        found: &Ty,
        span: &Span,
    ) -> Ty {
        if ty.is_some() {
            self.checked.findings.push(Finding {
                severity: Severity::Error,
                span: span.clone(),
                code: "NK1136",
                message: "a `let` that binds several names takes no type".to_string(),
                notes: vec![
                    "the names are taken apart by position, and one written type cannot say \
                     which of them it is about (Part I, 2.1)"
                        .to_string(),
                ],
                help: Some(
                    "take the type off, or bind one name and read the parts from it".to_string(),
                ),
            });
        }
        let parts = match found {
            Ty::Tuple(parts) if parts.len() == names.len() => parts.clone(),
            _ => vec![Ty::Unknown; names.len()],
        };
        for (name, part) in names.iter().zip(parts) {
            let bound = self.parsed.text(*name).to_string();
            self.nameable(&bound, span, "a `let`");
            self.bind_with(bound, part, None);
        }
        Ty::Tuple(Vec::new())
    }

    /// **Every item-level `comptime`, in a frame that stays under the whole
    /// file** ([ADR-097](../../../docs/specification/adr/adr-097.md)).
    ///
    /// A pass of its own, and both halves of that are load-bearing. It runs
    /// *before* the item loop, because a `fn` declared **above** a constant
    /// sees it as much as one below — an item is visible in its whole scope,
    /// which is what makes it an item. And the frame is pushed and never
    /// popped, because this checker's scope is a stack pushed per function and
    /// there is nothing under all of them otherwise.
    ///
    /// The bodies go through [`Self::comptime_binding`], the same function the
    /// statement form uses, so the fold, the refusal and the spelling the
    /// emitter is handed are one thing in one place
    /// ([ADR-073](../../docs/specification/adr/adr-073.md) D2: the difference
    /// between the two places is where the name is visible and nothing else).
    fn item_constants(&mut self) {
        self.scope.push(Vec::new());
        for item in &self.parsed.program.items {
            let Item::Comptime {
                name, ty, value, ..
            } = &item.node
            else {
                continue;
            };
            self.comptime_binding(*name, ty.as_ref(), value, &item.span);
        }
    }

    /// **One `comptime`, wherever it stands**
    /// ([ADR-097](../../../docs/specification/adr/adr-097.md)).
    ///
    /// [ADR-073](../../docs/specification/adr/adr-073.md) D2 decided both
    /// places and said the difference is *"where the name is visible, never
    /// what may stand to the right of the `='"*. So there is one function and
    /// the two callers differ only in which frame the name lands in - a second
    /// copy of the fold, the refusal and the emitter's spelling would be three
    /// chances for the two places to disagree about what a constant is.
    fn comptime_binding(
        &mut self,
        name: Ident,
        ty: Option<&crate::ast::Type>,
        value: &Expr,
        span: &Span,
    ) -> Ty {
        let found = self.expr(value, span);
        let bound = self.parsed.text(name).to_string();
        self.nameable(&bound, span, "a `comptime`");
        let want = ty.as_ref().map(|ty| self.declared(ty, span));
        if let Some(want) = &want {
            self.constant_fits(value, Some(want), span);
            self.expect(&found, want, span.clone(), "const", |found, want| {
                format!("this is `{found}`, and the `const` says `{want}`")
            });
        } else {
            self.constant_fits(value, None, span);
        }

        // What the emitter writes, spelled in the language below. An
        // integer takes the type its declaration pinned, and otherwise
        // the first one that holds it - Part I 2.4's rule, applied here
        // because Rust's `const` will not take the absence.
        let folded = self.constant_of(value);
        let below = match (&want, &folded, value) {
            (Some(want), _, _) => rust_constant_type(want),
            (None, Some(folded), _) => Some(match &folded.pinned {
                Some(pinned) => pinned.clone(),
                None => match i32::try_from(folded.value) {
                    Ok(_) => "i32".to_string(),
                    Err(_) => "i64".to_string(),
                },
            }),
            (None, None, Expr::LitBool(_)) => Some("bool".to_string()),
            _ => None,
        };

        // The value, spelled below. An integer is what the fold came
        // to; `true` and `false` are themselves.
        let written = match (&folded, value) {
            (Some(folded), _) => Some(folded.value.to_string()),
            (None, Expr::LitBool(yes)) => Some(yes.to_string()),
            _ => None,
        };
        match (&below, &written) {
            (Some(below), Some(written)) => {
                self.checked
                    .comptime_values
                    .insert(span.start, (below.clone(), written.clone()));
            }
            _ => self.checked.findings.push(Finding {
                code: "NK1127",
                severity: Severity::Error,
                span: span.clone(),
                message: format!("this compiler cannot evaluate `{bound}` while it builds"),
                notes: vec![
                    "a `comptime` is a `let` that *must* fold, so one that cannot is \
                         refused rather than computed while the program runs (Part II, 10.2)"
                        .to_string(),
                    "what it evaluates today is an integer - a literal, arithmetic \
                         over literals and over other constants - and `true` or `false`. \
                         A call is not in it yet"
                        .to_string(),
                ],
                help: Some(format!(
                    "write `let {bound} = …` if it is meant to be computed while the \
                         program runs"
                )),
            }),
        }

        let held = want.unwrap_or(found);
        self.bind_with(bound, held, folded.map(|c| c.value));
        Ty::Tuple(Vec::new())
    }

    /// `NK1133`: a statement after a `break` or a `continue`, in the same block.
    ///
    /// **The shape this is really about is `break i`**
    /// ([ADR-084](../../../docs/specification/adr/adr-084.md) D3). A `break` in
    /// Rust carries a value out of a `loop`; here a loop is a statement and
    /// hands back nothing ([ADR-070](../../../docs/specification/adr/adr-070.md)
    /// D2), so the word takes no value - and a value written after it parses as
    /// a statement of its own. Without this the program compiles, the value is
    /// dropped, and nothing says so.
    ///
    /// It is stated as what it is rather than as a guess at intent: the
    /// statement is not reached. That covers `break i`, `continue x` and the
    /// line somebody left below a `break` while editing, in one sentence and
    /// with one caret.
    fn nothing_follows_a_jump(&mut self, jump: &Stmt, span: &Span) {
        let word = match jump {
            Stmt::Break => "break",
            _ => "continue",
        };
        self.checked.findings.push(Finding {
            severity: Severity::Error,
            span: span.clone(),
            code: "NK1133",
            message: format!("nothing after a `{word}` in the same block is reached"),
            notes: vec![format!(
                "`{word}` takes no value: a loop is a statement here and hands back nothing, \
                 so `{word} x` is two statements rather than one (Part I, 3.3)"
            )],
            help: Some(format!(
                "delete it - or, where a value was meant, bind it before the `{word}`"
            )),
        });
    }

    /// `NK1132`: a `break` or a `continue` with no loop to act on.
    ///
    /// Two shapes and one code, because they are one mistake reached from two
    /// sides - there is no loop, or there is one and a function boundary
    /// stands between. The second is the one worth the code: it is a program
    /// that *looks* right, and without this it would be refused by `rustc`
    /// about the emitted file (Part III, C.1).
    fn a_jump_with_nowhere_to_go(&mut self, word: &str, span: &Span) {
        let does = match word {
            "break" => "leaves the innermost loop around it",
            _ => "starts the innermost loop's next turn",
        };
        let (message, note, help) = match self.barrier {
            Some(what) => (
                format!("`{word}` {does}, and the nearest loop is outside this {what}"),
                format!(
                    "{} is a function of its own in the language below, and a jump does \
                     not leave a function",
                    an(what)
                ),
                format!(
                    "decide inside the {what} and act on the answer outside it - a `bool` it \
                     hands back, tested by the loop"
                ),
            ),
            None => (
                format!("`{word}` {does}, and this is not in a loop"),
                "a `while` or a `for` is what it acts on, and there is none here (Part I, 3.3)"
                    .to_string(),
                match word {
                    "break" => "to leave the function rather than a loop, write `return`",
                    _ => "to leave the function rather than start a turn, write `return`",
                }
                .to_string(),
            ),
        };
        self.checked.findings.push(Finding {
            severity: Severity::Error,
            span: span.clone(),
            code: "NK1132",
            message,
            notes: vec![note],
            help: Some(help),
        });
    }

    /// Note where a method call in the function being walked went (ADR-028).
    ///
    /// `None` is "I could not find out", and it is recorded rather than
    /// dropped: an analysis that claims a property has to be able to tell that
    /// apart from a body that called nothing.
    fn reached_method(&mut self, key: Option<&str>) {
        // **Before the `current` gate**, which is about which function's set an
        // answer lands in and not about whether there is one: a method nothing
        // describes says nothing about failing, wherever it stands
        // ([ADR-091](../../../docs/specification/adr/adr-091.md)).
        if key.is_none() {
            self.guard_has_no_answer();
        }
        let Some(current) = &self.current else {
            return;
        };
        // **Which side of a `spawn` this call is on** (ADR-039 D3). The stack
        // is the one the task walk already keeps, so "inside a task" is
        // exactly what it says.
        let inside_a_task = !self.task_bindings.is_empty();
        let entry = self.checked.methods.entry(current.clone()).or_default();
        match key {
            Some(key) => {
                entry.resolved.insert(key.to_string());
                if inside_a_task {
                    entry.in_a_task.resolved.insert(key.to_string());
                }
            }
            None => {
                entry.unresolved = true;
                entry.unanswered += 1;
                entry.in_a_task.unresolved |= inside_a_task;
            }
        }
    }

    /// **Something the guarded expression of a `catch` holds and this compiler
    /// cannot see the end of** ([ADR-091](../../../docs/specification/adr/adr-091.md)).
    ///
    /// Called where a call could not be resolved and where a `catch` stands
    /// inside another one's guarded expression. A no-op outside a guarded
    /// expression, which is most of a program.
    fn guard_has_no_answer(&mut self) {
        if let Some(guarded) = &mut self.guarded {
            guarded.unanswered = true;
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
    /// What a value's own type arguments bind its declaration's parameters to.
    ///
    /// `Pair[i64]` against `struct Pair[T]` gives `T = i64`, positionally,
    /// because a type argument's position is the only thing that says which
    /// parameter it fills. Empty for every type that declares none, which is
    /// every type written today.
    fn arguments_of(&self, on: &Ty) -> BTreeMap<String, Ty> {
        let Ty::Named { name, args, .. } = on else {
            return BTreeMap::new();
        };
        let Some(order) = self.struct_parameters.get(name) else {
            return BTreeMap::new();
        };
        order
            .iter()
            .cloned()
            .zip(args.iter().cloned())
            .filter(|(_, arg)| !arg.is_unknown())
            .collect()
    }

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
                vec![Local::free(
                    self.parsed.text(segments[0]).to_string(),
                    Ty::Unknown,
                )]
            }
            MatchPattern::Path(_) => Vec::new(),
            MatchPattern::Tuple { bindings, .. } | MatchPattern::Named { bindings, .. } => bindings
                .iter()
                .map(|b| Local::free(self.parsed.text(*b).to_string(), Ty::Unknown))
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
/// Whether a method takes its receiver **by value**, which for a produced
/// sequence is what consumes it ([ADR-105](../../docs/specification/adr/adr-105.md) D2).
///
/// Read off the signature rather than off a list of names: every `Seq` entry
/// writes `(Seq[$T], …)` and a container's writes `(&Vec[$T], …)`, so the file
/// that describes the method is what says whether the walk keeps it.
fn walks_by_value(contract: &FnContract) -> bool {
    let Some(signature) = &contract.signature else {
        return false;
    };
    matches!(
        signature.params.first().map(|(_, ty)| ty),
        Some(Ty::Seq { .. })
    )
}

/// The sentence a word that used to be reserved gets instead
/// ([ADR-117](../../docs/specification/adr/adr-117.md) D2).
///
/// `None` for every other name, which keeps the general help exactly as it was:
/// this adds a sentence where the word is one of the four and changes nothing
/// anywhere else. Where the word **is** declared — a local, a parameter, a field
/// — the caller never gets here, which is D2's *nothing is said*.
fn a_word_that_was_reserved(name: &str) -> Option<&'static str> {
    match name {
        "loop" => Some(
            "there is no unconditional loop keyword: write `while true { … }` (Part I, 3.3). \
             `loop` is an ordinary name otherwise, so `let loop = 3` is a program",
        ),
        "const" => Some(
            "a value the compiler must work out while it builds is `comptime X = …` \
             (Part II, 10.2) - the word says *time* rather than *mutability*, which is what \
             `const` means in the languages it comes from. `const` is an ordinary name \
             otherwise",
        ),
        "macro" | "quote" => Some(
            "Nikaia has no macros: generating code from a type's shape is `comptime` and a \
             bound (Part II, 10.3), and a grammar is how text becomes a program \
             (Part II, 10.1). Both words are ordinary names otherwise",
        ),
        _ => None,
    }
}

fn element_of(over: &Ty, bindings: usize) -> Ty {
    match over {
        Ty::Named { name, args, view } if bindings == 1 && !view && args.len() == 1 => {
            match name.as_str() {
                "Vec" | "List" => args[0].clone(),
                _ => Ty::Unknown,
            }
        }
        // **A produced sequence is what a `for` walks**
        // ([ADR-105](../../docs/specification/adr/adr-105.md) D1), and its item
        // is the binding's type: `for c in text.chars()` binds a `char`. A
        // container is walked *as* a `Seq` by a `for` and is not one, which is
        // why the arm above stays where it is.
        //
        // One binding only, as above: `for (k, v) in map.drain()` takes the
        // pair apart, and which half is which is the tuple arm's question
        // rather than this one's.
        Ty::Seq { item, .. } if bindings == 1 => (**item).clone(),
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

/// What a `&x` was a view **of**, as far as [`view_of`] runs backwards.
///
/// It does not run backwards cleanly — a `&str` is a view of a `String` and
/// also a `&str` standing on its own — so this answers the owned types that
/// would have produced the view, and the caller asks the fit of each.
///
/// The one place that needs it is [`Checker::the_compiler_writes_the_reference`],
/// deciding whether a `&` the source wrote is the one the compiler would have
/// written anyway. There, a list that is too short costs a refusal that is not
/// raised and a list that is too long costs a correct program refused, so it is
/// the short one: only the views this compiler itself makes.
fn viewed_from(view: &Ty) -> Vec<Ty> {
    match view {
        Ty::Named {
            name,
            args,
            view: true,
        } if args.is_empty() && name == "str" => vec![Ty::named("String")],
        Ty::Named {
            name,
            args,
            view: true,
        } => vec![Ty::Named {
            name: name.clone(),
            args: args.clone(),
            view: false,
        }],
        _ => Vec::new(),
    }
}

/// The type several parts of a program own at once (Part I 6.2).
const SHARED: &str = "Shared";
/// Part I 6.3's lock, which shares the `Shared` hull's construction line
/// ([ADR-057](../../../docs/specification/adr/adr-057.md)).
const LOCKED: &str = "Locked";

/// The shared mutable type: a value several parts own at once and any of them may
/// change ([ADR-039](../../../docs/specification/adr/adr-039.md) D9).
///
/// It is a **name this module carries whole**. Only the emitter expands it to the
/// count around the lock, which is what keeps a spelling nobody wrote out of every
/// message about the language's most common type
/// ([ADR-064](../../../docs/specification/adr/adr-064.md) D1).
const SHARED_MUT: &str = "SharedMut";

/// The three types whose hull a program writes by calling their name
/// ([ADR-064](../../../docs/specification/adr/adr-064.md) D2).
///
/// **A hull you cannot observe, the compiler writes; a hull you can observe, you
/// write.** A `T?` costs nothing and hides nothing - the same value, possibly
/// absent - so its `Some(…)` is written for you (ADR-052). These three change
/// **when a value is cleaned up**, which Part I 6.2 says a program can see, so the
/// word stands where it happens.
fn is_hull(name: &str) -> bool {
    matches!(name, SHARED | SHARED_MUT | LOCKED)
}

/// The two doors that take **several** locks at once
/// ([ADR-065](../../../docs/specification/adr/adr-065.md)).
///
/// They are not ledger entries and not grammar: the parser already takes
/// `access_all(a, b) fn(x, y) { … }` as an ordinary call with a trailing lambda
/// (Part I 5.3), and what is missing is only somebody to type it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MultiLock {
    /// `access_all` - every lock read in place, none of them changed.
    Reading,
    /// `update_all` - every lock written, one new value each.
    Writing,
}

impl MultiLock {
    /// The name a program writes, or `None` for anything else.
    pub fn named(name: &str) -> Option<MultiLock> {
        match name {
            "access_all" => Some(MultiLock::Reading),
            "update_all" => Some(MultiLock::Writing),
            _ => None,
        }
    }

    pub fn written(self) -> &'static str {
        match self {
            MultiLock::Reading => "access_all",
            MultiLock::Writing => "update_all",
        }
    }

    fn shape(self) -> &'static str {
        match self {
            MultiLock::Reading => "access_all(a, b) fn(x, y) { … }",
            MultiLock::Writing => "update_all(a, b) fn(x, y) { … }",
        }
    }
}

/// What a lock holds, where the type says it is one.
///
/// `SharedMut[T]` is a count around a lock and `Locked[T]` is the lock on its
/// own; both hold a `T` and a door over several takes either.
fn locked_content_of(ty: &Ty) -> Option<Ty> {
    let Ty::Named { name, args, .. } = ty else {
        return None;
    };
    if name != SHARED_MUT && name != LOCKED {
        return None;
    }
    args.first().cloned()
}

/// Whether a value **is** a handle on a shared one, held by value.
///
/// `&Shared[T]` is not: lending the inner value out hands no handle on, so
/// nothing is duplicated ([ADR-040](../../../docs/specification/adr/adr-040.md)
/// D1's correction). A type that merely *holds* one is not either - a struct is
/// carried by whatever holds it.
fn is_a_handle(ty: &Ty) -> bool {
    matches!(ty, Ty::Named { name, view: false, .. } if name == SHARED || name == SHARED_MUT)
}

/// Whether a plain value stands where a **hull** is wanted - the shape whose
/// message names the constructor.
///
/// `want` is `Shared[T]`, `SharedMut[T]` or `Locked[T]` and `found` is the `T` it
/// would hold. It used to *permit* that at two positions, because the annotation
/// was the constructor ([ADR-040](../../../docs/specification/adr/adr-040.md) §3).
/// It permits nothing now
/// ([ADR-064](../../../docs/specification/adr/adr-064.md) D2): a hull is made by a
/// call, wherever a call may stand, and what this answers is only *which sentence
/// to write* when one is missing.
///
/// That is the whole of why the positions stopped being a list. A literal could
/// not be handled at all while the annotation was the constructor - it has no
/// type of its own, so nothing knew the hull was wanted - and `SharedMut(0)`
/// needs nobody to know.
fn becomes_shared(found: &Ty, want: &Ty) -> bool {
    let Ty::Named { name, args, view } = want else {
        return false;
    };
    if !is_hull(name) || *view {
        return false;
    }
    match args.as_slice() {
        [held] => {
            if found.is_unknown() {
                return false;
            }
            // A `SharedMut[T]` beside a `T` is the same shape as a `Shared[T]`
            // beside one: the value that would go in. Asked in this order so a
            // value that is *already* what the hull holds matches at the first
            // level ([ADR-064](../../docs/specification/adr/adr-064.md) D2).
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
        if let Ty::Named { name, .. } = want {
            return format!("write `{name}(…)` around it - a hull you can see is one you write");
        }
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

/// `a` or `an` in front of a word, which may be spelled in backticks.
///
/// Small and here because the alternative is worse: the three boundaries a
/// jump may not cross are named in one place, and carrying the article beside
/// each of them means the next one is added in two places or in one.
fn an(what: &str) -> String {
    let first = what.chars().find(|c| c.is_alphanumeric()).unwrap_or(' ');
    match first.to_ascii_lowercase() {
        'a' | 'e' | 'i' | 'o' | 'u' => format!("an {what}"),
        _ => format!("a {what}"),
    }
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

/// What the **arguments** bind this signature's variables to.
///
/// ADR-031 bound from the receiver only, and said so on purpose: binding from
/// arguments is where a signature language grows into a unification algorithm.
/// [ADR-074](../../../docs/specification/adr/adr-074.md) D2 takes that step and
/// keeps it one step. This is the same `ty::bind` against one pattern per
/// argument - a structural walk, no queue, no fixpoint, no occurs check -
/// because a type parameter is **written** rather than inferred, so there is
/// never a variable on the right-hand side to solve for.
///
/// A mismatch binds nothing, exactly as the receiver's does: the question is
/// what a caller can *tell* the signature, and `substitute` turns what it could
/// not tell into `?`.
fn from_arguments(contract: &FnContract, found: &[Ty]) -> BTreeMap<String, Ty> {
    let mut bound = BTreeMap::new();
    let Some(signature) = &contract.signature else {
        return bound;
    };
    for ((_, pattern), actual) in signature.arguments().iter().zip(found) {
        ty::bind(pattern, actual, &mut bound);
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

/// How this language spells a **view** of a value of this type.
///
/// `String` is the one that is not `&` plus its own name: Part I 2.2 writes a
/// view of text as `&str`, which is the spelling
/// [Part III 15.2](../../docs/specification/30-nikaia-tooling.md)'s mapping
/// gives it. Everything else is `&` and the type, which `NK1131`'s help needs
/// in order to name a way out that is right for the field it is about rather
/// than only for text.
fn a_view_of(ty: &Ty) -> String {
    match ty {
        Ty::Named { name, args, .. } if args.is_empty() && name == "String" => "&str".to_string(),
        other => format!("&{}", other.text()),
    }
}

/// Whether handing a value of this type out takes nothing away.
///
/// Part I 2.2's numbers, `bool` and `char`, and any **view**: all of them are
/// copied rather than moved in the language below, so a field of one may leave a
/// borrowed subject freely. Everything else - a `String`, a `Vec`, a struct this
/// program declares - is moved, which is what `NK1131` is about.
///
/// A short list on purpose, and the polarity is the usual one: a type not on it
/// is one this says nothing about only when it is also unknown.
fn copies(ty: &Ty) -> bool {
    match ty {
        Ty::Named { name, view, .. } => {
            *view
                || matches!(
                    name.as_str(),
                    "i32" | "i64" | "u8" | "f64" | "bool" | "char"
                )
        }
        _ => false,
    }
}

/// Whether a type is one this language calls text.
///
/// `String` and `&str`, and nothing else. A `T?` is deliberately **not** text:
/// `maybe + "x"` is a member reached off a nullable, which Part I 2.3 answers
/// and this must not quietly paper over.
fn is_text(ty: &Ty) -> bool {
    matches!(ty, Ty::Named { name, args, .. } if args.is_empty() && (name == "String" || name == "str"))
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
