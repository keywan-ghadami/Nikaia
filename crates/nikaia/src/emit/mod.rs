// crates/nikaia/src/emit/mod.rs
//
// Stage 0 of the bootstrap compiler: Nikaia in, Rust out.
//
// The three things this file exists for are the three halves of the grammar
// protocol that the parser backend already implements and Nikaia could not yet
// reach (README, "Bootstrap Compiler"):
//
//   * `grammar Name { rule ... }`  ->  `grammar! { grammar Name { ... } }`
//   * `@frame(boundary: "\n")`     ->  `#[frame(boundary = "\n")]`
//   * `dsl Name from input`        ->  the generated `par_fold` driver, with
//                                      the `Parallelism` the build asks for
//
// What it is *not* is a type checker. The lowering is syntactic: every action
// block, every `init`/`step`/`merge` and every function body is emitted as
// written. Where Nikaia and Rust disagree about what a name means - a `&mut
// self` method used as a monoid's merge, `std::fs` - the emitted program says
// so by not compiling, rather than this file guessing. That is deliberate:
// guessing a receiver would be a semantic decision made in a printer.
//
// It emits a [`SourceMap`] alongside the code, because a transpiler that only
// emits code can only be told about errors in a file nobody wrote (ADR-012).

use std::collections::{HashMap, HashSet};

use anyhow::{anyhow, Result};

pub mod template;
use winnow_grammar::Symbol;

use crate::ast::{
    BinaryOp, Block, Expr, FnArg, FoldSpec, FrameAttr, GrammarDef, GrammarRule, Item, MatchPattern,
    Pattern, Receiver, Repeat, Span, Spanned, Stmt, Type, UnaryOp, VariantFields,
};
use crate::parser::{parse_expression, Parsed};

/// The machine a program is built for (ADR-037 D1).
///
/// It decides what `std` can offer and what a panic does. It decides nothing
/// about what a program means: the same source compiles for every target and
/// prints the same bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Target {
    #[default]
    X86_64Linux,
    Wasm32Unknown,
}

/// Whether the **user's** code may run concurrently at all (ADR-037 D2).
///
/// A yes-or-no question, deliberately: *how many* threads or cores serve that
/// answer is the runtime's business, and a number in the language would be a
/// promise the language cannot keep on a machine it has not seen.
///
/// It does not bind the compiler - `fs::map` may still validate its text on
/// four cores at `No`, because that is not code the user wrote and it changes
/// nothing the program prints (ADR-016 D3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UserParallelism {
    /// `no` - the default. Nothing the user wrote ever runs concurrently.
    #[default]
    No,
    /// `yes` - it may, and the runtime decides how widely.
    Yes,
}

/// How strictly the written order of two statements is taken (ADR-033, D8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Ordering {
    /// Two operations that touch disjoint resources may overlap.
    #[default]
    Effects,
    /// The written order, always. The analysis is not applied.
    ///
    /// Not an aid to be removed later: it is the escape for a project that does
    /// not want this, and the way to rule the analysis out when chasing a bug.
    Strict,
}

impl Ordering {
    pub fn parse(name: &str) -> Result<Ordering> {
        match name {
            "effects" => Ok(Ordering::Effects),
            "strict" => Ok(Ordering::Strict),
            other => Err(anyhow!(
                "unknown ordering `{other}` (expected effects or strict)"
            )),
        }
    }
}

/// The two build switches together (ADR-037).
///
/// One value rather than two parameters: a third switch is then a field, not a
/// change at every call site.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Build {
    pub target: Target,
    pub user_parallelism: UserParallelism,
}

impl Build {
    pub fn parse(target: &str, user_parallelism: &str) -> Result<Build> {
        Ok(Build {
            target: Target::parse(target)?,
            user_parallelism: UserParallelism::parse(user_parallelism)?,
        })
    }

    /// The default machine, with parallelism asked for.
    pub fn parallel() -> Build {
        Build {
            user_parallelism: UserParallelism::Yes,
            ..Build::default()
        }
    }

    /// How the generated driver is asked to cut the input.
    fn parallelism(self) -> &'static str {
        self.user_parallelism.parallelism(self.target)
    }

    /// Whether a vehicle exists here for running two pieces of **user** code
    /// at the same time (ADR-033 §8.2).
    ///
    /// Not "does the process have more than one thread". It has a second one
    /// at `user_parallelism = no` too, and always did: Part I 8.4's Runtime
    /// Sidecar offloads blocking I/O to a background thread on native (a Web
    /// Worker on WASM) so the event loop never stalls, and it stays safe
    /// precisely because *user code never runs there* - the exchange is
    /// ownership-transferring message passing, so there is nothing to race
    /// over. What `no` forbids is the other thing: that anything **you** wrote
    /// is in flight twice at once (Part I 1.2).
    ///
    /// `task::both(|| …, || …)` puts two user closures on two threads, so it
    /// is out at `no` whatever the analysis says - ADR-033 decides whether two
    /// operations *may* overlap, and this decides whether there is anything to
    /// overlap them with. A target without threads answers no for the second
    /// reason: `rayon::join` does not link on `wasm32-unknown` (Part III 15.3).
    ///
    /// This is **not** a statement that ADR-033 means nothing at `no`. Two
    /// reads in flight at once with their results collected on the main thread
    /// in written order is concurrency without parallelism, which is what an
    /// event loop and a sidecar are *for* - and it would carry none of the
    /// per-pair thread wake-up ADR-033 §8.4 measured. That vehicle is not
    /// built (no event loop, no sidecar, no async lowering), so `effects`
    /// degrades to `strict` here the way `par_fold` degrades to `fold`
    /// (ADR-009) - for now, and for a narrower reason than "no threads".
    pub fn overlaps_user_code(self) -> bool {
        self.user_parallelism.is_concurrent() && self.target.has_threads()
    }
}

impl Target {
    pub fn parse(name: &str) -> Result<Target> {
        match name {
            "x86_64-linux" => Ok(Target::X86_64Linux),
            "wasm32-unknown" => Ok(Target::Wasm32Unknown),
            other => Err(anyhow!(
                "unknown target `{other}` (expected x86_64-linux or wasm32-unknown)"
            )),
        }
    }

    /// The triple handed to the backend.
    pub fn triple(self) -> &'static str {
        match self {
            Target::X86_64Linux => "x86_64-unknown-linux-gnu",
            Target::Wasm32Unknown => "wasm32-unknown-unknown",
        }
    }

    /// Whether this machine has threads at all. A target without them bounds
    /// `user_parallelism` to `0` however it is set.
    pub fn has_threads(self) -> bool {
        matches!(self, Target::X86_64Linux)
    }

    /// What a target still needs before a program can be built for it.
    ///
    /// `Some(reason)` is a refusal that names the gap, never code emitted for
    /// a different machine (ADR-037 D1).
    pub fn unbuildable(self) -> Option<&'static str> {
        match self {
            Target::Wasm32Unknown => Some(
                "`std` reaches for a memory mapping and a thread pool, and neither \
                 exists on wasm32-unknown-unknown; what `std::fs` offers there is \
                 undecided",
            ),
            Target::X86_64Linux => None,
        }
    }
}

impl UserParallelism {
    pub fn parse(value: &str) -> Result<UserParallelism> {
        match value {
            "no" => Ok(UserParallelism::No),
            "yes" => Ok(UserParallelism::Yes),
            // A number is the plausible mistake, and it has a reason rather
            // than a typo behind it. Say which.
            other if other.parse::<u32>().is_ok() => Err(anyhow!(
                "`user-parallelism` is yes or no, not a count: how many threads \
                 serve a `yes` is the runtime's to decide, not the program's"
            )),
            other => Err(anyhow!(
                "unknown user-parallelism `{other}` (expected yes or no)"
            )),
        }
    }

    /// Whether any code the user wrote may run concurrently.
    pub fn is_concurrent(self) -> bool {
        matches!(self, UserParallelism::Yes)
    }

    /// How the generated driver is asked to cut the input.
    ///
    /// A target without threads pins this to `Off` whatever was asked for: the
    /// switch bounds what may run at once, it cannot conjure a thread the
    /// machine does not have.
    fn parallelism(self, target: Target) -> &'static str {
        if !target.has_threads() {
            return "Parallelism::Off";
        }
        match self {
            UserParallelism::No => "Parallelism::Off",
            UserParallelism::Yes => "Parallelism::Auto",
        }
    }
}

/// The lifetime the parser backend gives its input. A Nikaia view (`&str`) is
/// tied to it, which is the whole of what `&` means here: ADR-008 - a view
/// marker, never an annotation the user writes.
const INPUT_LIFETIME: &str = "'a";

/// How a view's lifetime is spelled here. The source never says either way
/// (ADR-008); the position does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Lifetimes {
    /// What a `&` becomes.
    reference: &'static str,
    /// What a type that holds a view is parameterised with.
    params: &'static str,
}

impl Lifetimes {
    /// Inside the grammar module and on the structs it builds, where `'a` is
    /// the input's lifetime and has to be named to tie the two together.
    const NAMED: Lifetimes = Lifetimes {
        reference: "&'a ",
        params: "'a",
    };
    /// In a free function's signature, which declares no lifetime of its own.
    const ELIDED: Lifetimes = Lifetimes {
        reference: "&",
        params: "'_",
    };
    /// In a method of an `impl` that does declare `'a`: the reference itself is
    /// still elided, because a borrow of the subject need not live as long as
    /// what the subject points into.
    const INNER: Lifetimes = Lifetimes {
        reference: "&",
        params: "'a",
    };
}

// --- The output, and the way back ---

/// What the lowering produces: the Rust, and where each part of it came from.
#[derive(Debug, Clone)]
pub struct Lowered {
    pub rust: String,
    pub map: SourceMap,
}

/// Emitted byte range -> the `.nika` span that produced it.
///
/// This is the whole answer to "the error points at a line nobody wrote": the
/// compiler that consumes the emitted Rust reports offsets into it, and every
/// one of them can be traded back for a place in the source the user has open.
#[derive(Debug, Clone, Default)]
pub struct SourceMap {
    /// Innermost first: a lookup returns the first range that contains the
    /// offset, and the emitter records a node before recording anything that
    /// encloses it.
    entries: Vec<MapEntry>,
}

#[derive(Debug, Clone)]
struct MapEntry {
    generated: std::ops::Range<usize>,
    source: Span,
    /// Which file it came from, as an index into the program's units. A
    /// single-file program has one, and it is 0.
    unit: usize,
}

impl SourceMap {
    /// The narrowest source span whose emitted text covers `offset`, and the
    /// file it is in.
    pub fn locate(&self, offset: usize) -> Option<(usize, Span)> {
        self.entries
            .iter()
            .find(|e| e.generated.contains(&offset))
            .map(|e| (e.unit, e.source.clone()))
    }

    /// The narrowest source span whose emitted text covers `offset`.
    pub fn source_span(&self, offset: usize) -> Option<Span> {
        self.locate(offset).map(|(_, span)| span)
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// This map, as it reads once its module's text has been placed `by` bytes
    /// into a larger file - and tagged with which file it came from.
    ///
    /// A program of several modules is emitted one module at a time and joined;
    /// this is what keeps ADR-012's promise across the join. Without it a
    /// `rustc` message about the third module would be traded back for a place
    /// in the first.
    pub fn placed(mut self, by: usize, unit: usize) -> SourceMap {
        for entry in &mut self.entries {
            entry.generated = entry.generated.start + by..entry.generated.end + by;
            entry.unit = unit;
        }
        self
    }

    /// Take everything `other` holds, which is already placed.
    pub fn extend(&mut self, other: SourceMap) {
        self.entries.extend(other.entries);
    }
}

/// The emitted text under construction, and the map being built with it.
#[derive(Debug, Default)]
struct Out {
    buf: String,
    map: SourceMap,
}

impl Out {
    fn push(&mut self, text: &str) {
        self.buf.push_str(text);
    }

    /// Record that everything `f` writes came from `span`.
    fn from<R>(&mut self, span: &Span, f: impl FnOnce(&mut Out) -> Result<R>) -> Result<R> {
        let start = self.buf.len();
        let value = f(self)?;
        self.map.entries.push(MapEntry {
            generated: start..self.buf.len(),
            source: span.clone(),
            // The emitter writes one module at a time and does not know which:
            // `SourceMap::placed` stamps it when the driver joins them.
            unit: 0,
        });
        Ok(value)
    }

    /// Render into a buffer of its own, so the caller can decide what to do
    /// with the result before committing to it.
    fn scratch(f: impl FnOnce(&mut Out) -> Result<()>) -> Result<Out> {
        let mut out = Out::default();
        f(&mut out)?;
        Ok(out)
    }

    /// Append a scratch buffer, moving its map entries into place. Order is
    /// preserved, so "innermost first" survives.
    fn append(&mut self, other: Out) {
        let offset = self.buf.len();
        self.buf.push_str(&other.buf);
        for mut entry in other.map.entries {
            entry.generated.start += offset;
            entry.generated.end += offset;
            self.map.entries.push(entry);
        }
    }
}

pub fn emit_program(parsed: &Parsed, build: Build) -> Result<Lowered> {
    emit_program_ordered(parsed, build, Ordering::default())
}

/// The same, saying how strictly the written order is to be taken (ADR-033).
pub fn emit_program_ordered(parsed: &Parsed, build: Build, ordering: Ordering) -> Result<Lowered> {
    let trust = crate::contracts::trust::analyse(parsed, &std_ledger());
    Emitter::new(parsed, build, trust.provenance, ordering).program()
}

/// The same, with the provenance already decided.
///
/// The CLI analyses once and prints the answer under `--trust`; this is what it
/// hands back in so the emitted code and the explanation cannot disagree.
pub fn emit_program_with_trust(
    parsed: &Parsed,
    build: Build,
    provenance: crate::contracts::Provenance,
) -> Result<Lowered> {
    Emitter::new(parsed, build, provenance, Ordering::default()).program()
}

/// A module's items, with no preamble and no `mod` around them.
///
/// The driver writes the preamble once for the whole program and the `mod`
/// header itself, because only it knows what the program is made of
/// (`modules::collect`).
///
/// `contracts` are the **program's**, not this file's: a call to
/// `page::render(entries, total)` fills in Kap 5.1's defaults from the
/// declaration, and the declaration is in another file.
pub fn emit_module_body(
    parsed: &Parsed,
    build: Build,
    provenance: crate::contracts::Provenance,
    contracts: &crate::contracts::Ledger,
) -> Result<Lowered> {
    emit_module_body_ordered(Ordering::default(), parsed, build, provenance, contracts)
}

/// The same, saying how strictly the written order is taken (ADR-033).
///
/// The ordering is the first parameter because it is the one a caller is most
/// likely to be threading through from a flag, and burying it behind four
/// others is how it ends up defaulted by accident.
pub fn emit_module_body_ordered(
    ordering: Ordering,
    parsed: &Parsed,
    build: Build,
    provenance: crate::contracts::Provenance,
    contracts: &crate::contracts::Ledger,
) -> Result<Lowered> {
    Emitter::with_contracts(parsed, build, provenance, contracts.clone(), ordering).items_only()
}

/// What a program's preamble has to say, over all of its files.
///
/// `uses_std` and the rest are per-file facts, and the preamble is written
/// once - so they are joined here rather than guessed at from the entry.
#[derive(Debug, Clone, Copy, Default)]
pub struct Needs {
    pub grammar: bool,
    pub driver: bool,
    pub std: bool,
    /// Kap 7.1: a function here declares `throws`, so the error surface is
    /// reachable and `std`'s is what carries it.
    pub fails: bool,
}

impl Needs {
    pub fn of(parsed: &Parsed, build: Build) -> Needs {
        let emitter = Emitter::new(
            parsed,
            build,
            crate::contracts::Provenance::Trusted,
            Ordering::default(),
        );
        Needs {
            grammar: parsed
                .program
                .items
                .iter()
                .any(|i| matches!(i.node, Item::Grammar(_))),
            driver: emitter.uses_driver(),
            std: emitter.uses_std,
            fails: emitter.fails,
        }
    }

    pub fn join(self, other: Needs) -> Needs {
        Needs {
            grammar: self.grammar || other.grammar,
            driver: self.driver || other.driver,
            std: self.std || other.std,
            fails: self.fails || other.fails,
        }
    }

    /// The lines at the top of the emitted file.
    pub fn preamble(self) -> String {
        let mut out = String::new();
        if self.grammar {
            out.push_str("use winnow_grammar::grammar;\n");
        }
        if self.driver {
            out.push_str("use winnow_grammar::rt::Parallelism;\n");
            out.push_str("use winnow_grammar::ParseContext;\n");
        }
        if self.std {
            // Nikaia's `std` is a crate rather than a table in this file, so
            // `fs::map`, `cli::args` and `HashMap` resolve as written and what
            // they mean is code someone can read.
            // `pub use`, because the grammar module the backend generates
            // reaches these names through a glob of its own, and a private
            // import is not re-exported into one.
            out.push_str("pub use nikaia_std::prelude::*;\n");
        }
        if self.fails {
            // Kap 7.1: `error.full()` is a method on whatever a `catch` bound, so
            // the trait has to be in scope even in a program that imports no `std`
            // module of its own - `throw` needs `std` whether or not the source
            // mentions it.
            out.push_str("#[allow(unused_imports)]\npub use nikaia_std::error::Full;\n");
        }
        out
    }
}

/// `std`'s shipped contracts, parsed once.
///
/// A malformed ledger is a bug in this repository rather than in a user's
/// program, and the tests read the same file - so failing to parse it here
/// means treating the input as untrusted, which is the safe direction
/// (ADR-010 D1) and never a silent upgrade.
fn std_ledger() -> crate::contracts::Ledger {
    crate::contracts::Ledger::parse(crate::contracts::STD).unwrap_or_default()
}

struct Emitter<'p> {
    parsed: &'p Parsed,
    build: Build,
    /// Structs that hold a view into the input, and so need the input lifetime
    /// wherever they are named.
    borrowing: HashSet<Symbol>,
    grammars: HashMap<Symbol, &'p GrammarDef>,
    /// Declared struct names: `Stats(x)` is a call to a constructor, and only
    /// the declarations say which names are types.
    structs: HashSet<Symbol>,
    /// The `for` statements whose step can fail, by the byte they start at
    /// (ADR-025 D1).
    fallible_loops: std::collections::BTreeSet<usize>,
    /// This unit's own contracts, and `std`'s. A call's options come from the
    /// declaration, and a declaration is what a ledger records (Kap 5.1).
    own_contracts: crate::contracts::Ledger,
    library: crate::contracts::Ledger,
    /// ADR-033: whether two statements that meet on nothing may overlap.
    ordering: Ordering,
    /// What the program's `impl` blocks declare, which is what makes the fold
    /// adapter of D2 a lookup rather than a guess.
    methods: HashMap<(Symbol, Symbol), Method>,
    /// The same, by method name alone, for the places where the receiver's type
    /// is not written down. `None` marks a name two impls disagree about - and
    /// an ambiguous name is left alone rather than resolved by preference.
    by_name: HashMap<Symbol, Option<Method>>,
    /// Whether the program imports anything from `std`.
    uses_std: bool,
    fails: bool,
    /// ADR-010: nobody outside the program chose the bytes its maps are keyed
    /// by, so a map may have the fast hash.
    trusted_input: bool,
    /// ADR-007 D5: the functions that accept a DSL's deferred parameters, by
    /// name. A `;` at a call means options everywhere else, and this is what
    /// says which calls mean the other thing.
    dsl_drivers: HashSet<String>,
}

/// What an `impl` says about a method. Enough to adapt a `&mut self` method to
/// the monoid a `par_fold` needs, and never more than the declaration states.
#[derive(Debug, Clone)]
struct Method {
    receiver: Option<Receiver>,
    returns_value: bool,
    args: Vec<FnArg>,
}

impl Method {
    /// A method that mutates its subject and yields nothing - the shape that
    /// has to be threaded to become a fold's step or merge.
    fn mutates_in_place(&self) -> bool {
        !self.returns_value && self.receiver.is_some_and(|r| r.is_ref && r.is_mut)
    }
}

/// How `Self::dsl` is spelled in the source, and what it becomes below.
///
/// ADR-007 D5 gives a driver one way to name the type a DSL string generates,
/// and the emitted Rust needs a name for it that no program can collide with.
const SELF_DSL: &str = "Self::dsl";
const DSL_PARAMETER: &str = "NikaiaDsl";

/// What surrounds the statements being emitted.
#[derive(Debug, Clone, Copy)]
struct Flow<'a> {
    /// Kap 7.1: the enclosing function is `throws`, so a `return` carries `Ok`.
    throws: bool,
    /// The function a `throw` inside this flow is raised from.
    ///
    /// ADR-023 D6 wants a raise site keyed by what the source says rather than
    /// by where it sits, so that an edit above it does not move it. A name is
    /// the part of that key this compiler has; the ordinal within the function
    /// waits on the mark table.
    origin: &'a str,
}

impl Flow<'_> {
    const PLAIN: Flow<'static> = Flow {
        throws: false,
        origin: "",
    };
}

/// Whether a `dsl … from …` hands its failure to the enclosing function or to
/// a `catch` beside it. Everywhere else the parse propagates - `catch` is the
/// one place that wants the `Result` itself, and asking for it there is the
/// whole difference (Kap 7.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Propagate {
    Yes,
    No,
}

impl<'p> Emitter<'p> {
    fn new(
        parsed: &'p Parsed,
        build: Build,
        provenance: crate::contracts::Provenance,
        ordering: Ordering,
    ) -> Self {
        let own = crate::contracts::Ledger::infer(parsed);
        Self::with_contracts(parsed, build, provenance, own, ordering)
    }

    /// The same, against contracts that already exist - a program's rather than
    /// a file's.
    fn with_contracts(
        parsed: &'p Parsed,
        build: Build,
        provenance: crate::contracts::Provenance,
        own_contracts: crate::contracts::Ledger,
        ordering: Ordering,
    ) -> Self {
        let mut grammars = HashMap::new();
        let mut structs = HashSet::new();
        let mut methods = HashMap::new();
        let mut by_name: HashMap<Symbol, Option<Method>> = HashMap::new();
        let mut uses_std = false;
        let mut fails = false;

        for item in &parsed.program.items {
            match &item.node {
                Item::Grammar(def) => {
                    grammars.insert(def.name, def);
                }
                Item::Struct { name, .. } => {
                    structs.insert(*name);
                }
                Item::Import { .. } => uses_std = true,
                Item::Fn { throws: true, .. } => fails = true,
                Item::Impl {
                    trait_name: _,
                    target,
                    methods: body,
                } => {
                    for method in body {
                        let Item::Fn {
                            name: Some(name),
                            receiver,
                            args,
                            ret_type,
                            ..
                        } = &method.node
                        else {
                            continue;
                        };

                        let info = Method {
                            receiver: *receiver,
                            returns_value: ret_type.is_some(),
                            args: args.clone(),
                        };
                        methods.insert((target.name, *name), info.clone());
                        by_name
                            .entry(*name)
                            .and_modify(|known| {
                                // Two impls, one name: nothing here can tell
                                // them apart, so neither is used.
                                if known.is_some() {
                                    *known = None;
                                }
                            })
                            .or_insert(Some(info));
                    }
                }
                _ => {}
            }
        }

        Self {
            parsed,
            build,
            borrowing: borrowing_structs(parsed),
            grammars,
            structs,
            methods,
            by_name,
            uses_std,
            fails,
            trusted_input: provenance == crate::contracts::Provenance::Trusted,
            // ADR-025 D7: the type checker knows which `for` iterates something
            // whose step can fail, because it infers the iterator's type and
            // reads the ledger. Matching on the name `io::lines` here would
            // have caught the one-line form and quietly missed
            // `let s = io::lines()` followed by `for line in s`.
            fallible_loops: crate::check::fallible_loops_against(parsed, &own_contracts),
            own_contracts,
            library: std_ledger(),
            ordering,
            dsl_drivers: crate::dsl::drivers(parsed).into_iter().collect(),
        }
    }

    fn text(&self, sym: Symbol) -> &str {
        self.parsed.text(sym)
    }

    /// A module's items and nothing else - no preamble, no `mod` header.
    fn items_only(&self) -> Result<Lowered> {
        let mut out = Out::default();
        self.shadow_types(&mut out);
        for item in &self.parsed.program.items {
            out.from(&item.span, |out| self.item(out, &item.node))?;
            out.push("\n");
        }
        Ok(Lowered {
            rust: out.buf,
            map: out.map,
        })
    }

    fn program(&self) -> Result<Lowered> {
        let mut out = Out::default();
        out.push("// Generated by the Nikaia bootstrap compiler (Stage 0).\n");
        out.push("// Edit the .nika source, not this file.\n\n");

        if self
            .parsed
            .program
            .items
            .iter()
            .any(|i| matches!(i.node, Item::Grammar(_)))
        {
            out.push("use winnow_grammar::grammar;\n");
        }
        if self.uses_driver() {
            out.push("use winnow_grammar::rt::Parallelism;\n");
            out.push("use winnow_grammar::ParseContext;\n");
        }
        if self.uses_std {
            // Nikaia's `std` is a crate rather than a table in this file, so
            // `fs::map`, `cli::args` and `HashMap` resolve as written and what
            // they mean is code someone can read.
            // `pub use`, because the grammar module the backend generates
            // reaches these names through a glob of its own, and a private
            // import is not re-exported into one.
            out.push("pub use nikaia_std::prelude::*;\n");
        }
        if self.fails {
            // Kap 7.1: `error.full()` is a method on whatever a `catch` bound, so
            // the trait has to be in scope even in a program that imports no `std`
            // module of its own - `throw` needs `std` whether or not the source
            // mentions it.
            out.push("#[allow(unused_imports)]\npub use nikaia_std::error::Full;\n");
        }
        out.push("\n");

        self.shadow_types(&mut out);

        for item in &self.parsed.program.items {
            out.from(&item.span, |out| self.item(out, &item.node))?;
            out.push("\n");
        }

        Ok(Lowered {
            rust: out.buf,
            map: out.map,
        })
    }

    /// Whether any `dsl … from …` in the program reaches a parallel entry rule.
    fn uses_driver(&self) -> bool {
        let mut found = false;
        for item in &self.parsed.program.items {
            if let Item::Fn { body, .. } = &item.node {
                visit_block(body, &mut |e| {
                    if let Expr::DslFrom { grammar, .. } = e {
                        if let Some(def) = self.grammars.get(grammar) {
                            if entry_rule(def)
                                .map(|r| par_fold_of(r).is_some())
                                .unwrap_or(false)
                            {
                                found = true;
                            }
                        }
                    }
                });
            }
        }
        found
    }

    fn item(&self, out: &mut Out, item: &Item) -> Result<()> {
        match item {
            Item::Grammar(def) => self.grammar(out, def),
            Item::Enum {
                name,
                variants,
                is_public,
            } => {
                out.push("#[derive(Debug, Clone)]\n");
                let vis = if *is_public { "pub " } else { "" };
                let params = if self.borrowing.contains(name) {
                    format!("<{INPUT_LIFETIME}>")
                } else {
                    String::new()
                };
                out.push(&format!("{vis}enum {}{params} {{\n", self.text(*name)));
                for variant in variants {
                    let name = self.text(variant.name);
                    match &variant.fields {
                        VariantFields::Unit => out.push(&format!("    {name},\n")),
                        VariantFields::Tuple(types) => {
                            let parts: Vec<String> =
                                types.iter().map(|t| self.ty(t, Lifetimes::NAMED)).collect();
                            out.push(&format!("    {name}({}),\n", parts.join(", ")));
                        }
                        VariantFields::Named(fields) => {
                            let parts: Vec<String> = fields
                                .iter()
                                .map(|f| {
                                    format!(
                                        "{}: {}",
                                        self.text(f.name),
                                        self.ty(&f.ty, Lifetimes::NAMED)
                                    )
                                })
                                .collect();
                            out.push(&format!("    {name} {{ {} }},\n", parts.join(", ")));
                        }
                    }
                }
                out.push("}\n");
                Ok(())
            }
            Item::Struct {
                name,
                fields,
                is_public,
                is_borrowed,
                ..
            } => {
                if *is_borrowed {
                    out.push(
                        "// @borrowed (ADR-008 D6): asserted in the source; the check that no\n\
                         // value of this type escapes its buffer is not implemented yet.\n",
                    );
                }
                out.push("#[derive(Debug, Clone)]\n");
                let vis = if *is_public { "pub " } else { "" };
                let params = if self.borrowing.contains(name) {
                    format!("<{INPUT_LIFETIME}>")
                } else {
                    String::new()
                };
                out.push(&format!("{vis}struct {}{params} {{\n", self.text(*name)));
                for field in fields {
                    // Public, because the actions that build this struct are
                    // generated into the grammar's own module.
                    out.push(&format!(
                        "    {}{}: {},\n",
                        if field.is_public { "pub " } else { "" },
                        self.text(field.name),
                        self.ty(&field.ty, Lifetimes::NAMED)
                    ));
                }
                out.push("}\n");
                Ok(())
            }
            Item::Fn { .. } => self.function(out, item, 0, Lifetimes::ELIDED),
            Item::Impl {
                trait_name,
                target,
                methods,
            } => {
                // A type that holds a view carries the input lifetime, and the
                // impl has to declare the lifetime its methods are written with.
                let borrows = self.borrowing.contains(&target.name);
                let params = if borrows {
                    format!("<{INPUT_LIFETIME}>")
                } else {
                    String::new()
                };
                let lifetimes = if borrows {
                    Lifetimes::INNER
                } else {
                    Lifetimes::ELIDED
                };
                let target_name = self.text(target.name).to_string();

                // Kap 7.1: `Error` is the one trait the compiler reads rather
                // than relays. An error travels in the failure channel, which
                // the language below spells `Box<dyn std::error::Error>`, and a
                // type gets in there by being `Display` plus `Error` - neither
                // of which the `.nika` source mentions, because neither is a
                // decision the author makes. `message` is what they wrote.
                let is_error_impl = trait_name.is_some_and(|t| self.text(t) == "Error");
                if is_error_impl {
                    return self.error_impl(out, &target_name, &params, methods, lifetimes);
                }

                let head = match trait_name {
                    Some(t) => format!("impl{params} {} for {target_name}{params}", self.text(*t)),
                    None => format!("impl{params} {target_name}{params}"),
                };
                out.push(&format!("{head} {{\n"));
                for method in methods {
                    out.from(&method.span, |out| {
                        out.push("    ");
                        self.function(out, &method.node, 1, lifetimes)
                    })?;
                }
                out.push("}\n");
                Ok(())
            }
            Item::Import { path } => {
                // The names come in through `nikaia_std::prelude`, emitted once
                // in the preamble; the import itself is kept as a comment so the
                // generated file still says where they were asked for.
                let path = path
                    .iter()
                    .map(|s| self.text(*s))
                    .collect::<Vec<_>>()
                    .join("::");
                out.push(&format!("// use {path}\n"));
                Ok(())
            }
            other => Err(anyhow!("cannot emit item yet: {other:?}")),
        }
    }

    /// A function or a method. Kap 4.2: a `pub fn` with no name is the
    /// anonymous constructor, called as `Type(…)`; Rust has no such thing, so
    /// it is emitted as `new` and the call sites follow.
    /// Kap 7.1: `impl Error for T` becomes the three impls the failure channel
    /// needs, from the one method the source wrote.
    ///
    /// The author writes `message`. Rust wants `Display` for the text, and
    /// `std::error::Error` for the value to be accepted as an error at all;
    /// `Box<dyn Error>` then takes it. None of that is a decision - it is the
    /// same transcription ADR-011 D2 asks of every other lowering - so the
    /// `.nika` file says the part that is one and the emitter supplies the rest.
    ///
    /// `Debug` comes along because `std::error::Error` requires it, and a
    /// derived one is what a user would have written.
    fn error_impl(
        &self,
        out: &mut Out,
        target: &str,
        params: &str,
        methods: &[Spanned<Item>],
        lifetimes: Lifetimes,
    ) -> Result<()> {
        out.push(&format!("impl{params} {target}{params} {{\n"));
        for method in methods {
            out.from(&method.span, |out| {
                out.push("    ");
                self.function(out, &method.node, 1, lifetimes)
            })?;
        }
        out.push("}\n");

        out.push(&format!(
            "impl{params} std::fmt::Display for {target}{params} {{\n\
             \x20   fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {{\n\
             \x20       f.write_str(&self.message())\n\
             \x20   }}\n\
             }}\n"
        ));
        out.push(&format!(
            "impl{params} std::error::Error for {target}{params} {{}}\n"
        ));
        Ok(())
    }

    fn function(
        &self,
        out: &mut Out,
        item: &Item,
        depth: usize,
        lifetimes: Lifetimes,
    ) -> Result<()> {
        let Item::Fn {
            name,
            receiver,
            args,
            config,
            spread,
            ret_type,
            body,
            is_sync,
            is_public,
            throws,
            ..
        } = item
        else {
            return Err(anyhow!("not a function"));
        };

        let pad = "    ".repeat(depth);
        if *is_sync {
            out.push(&format!(
                "// sync (Part II, 12.1): pure CPU, cannot pause. Checked before\n{pad}// this was written - see `contracts::sync`.\n{pad}"
            ));
        }

        let mut params = Vec::new();
        if let Some(receiver) = receiver {
            params.push(
                match (receiver.is_ref, receiver.is_mut) {
                    (true, true) => "&mut self",
                    (true, false) => "&self",
                    _ => "self",
                }
                .to_string(),
            );
        }
        params.extend(
            args.iter()
                .map(|a| format!("{}: {}", self.text(a.name), self.ty(&a.ty, lifetimes))),
        );
        // Kap 5.1: the language below has neither named arguments nor defaults,
        // so an option becomes an ordinary parameter here - in declaration
        // order, which is the order every call site fills in. The names stay
        // the source's, so a `rustc` diagnostic about one still lands on the
        // parameter the programmer wrote (ADR-012).
        params.extend(
            config
                .iter()
                .map(|c| format!("{}: {}", self.text(c.name), self.ty(&c.ty, lifetimes))),
        );

        // ADR-007 D5: `...args: Self::dsl` is the one parameter whose type the
        // *call site* decides, because the DSL string it comes from decides it.
        // A generic parameter is what that is in the language below, and Rust
        // monomorphises it per DSL string exactly as D5 asks - so the driver is
        // written once and pays no heap traffic per statement.
        let dsl = spread.as_ref().map(|name| {
            params.push(format!("{}: {DSL_PARAMETER}", self.text(*name)));
            format!("<{DSL_PARAMETER}>")
        });

        // Kap 7.1: `throws` becomes a `Result` in the emitted Rust, over
        // `Box<dyn Error>` because Nikaia's own error types are not lowered
        // yet - the `?` the DSL driver needs works against it, and every error
        // keeps its own type behind it.
        let returned = match ret_type {
            // `Self::dsl` in the result position is the same type the spread
            // parameter has: a driver that hands the bound parameters back
            // names them the only way D5 gives it to name them.
            Some(ty) if self.text(ty.name) == SELF_DSL => {
                if dsl.is_none() {
                    return Err(anyhow!(
                        "`Self::dsl` names the parameters of a `...args: Self::dsl`, \
                         and this function declares none"
                    ));
                }
                DSL_PARAMETER.to_string()
            }
            Some(ty) => self.ty(ty, lifetimes),
            None => "()".to_string(),
        };
        let ret = if *throws {
            format!(" -> Result<{returned}, Box<dyn std::error::Error>>")
        } else if ret_type.is_some() {
            format!(" -> {returned}")
        } else {
            String::new()
        };

        let vis = if *is_public { "pub " } else { "" };
        let name = match name {
            Some(name) => self.text(*name).to_string(),
            None => "new".to_string(),
        };

        out.push(&format!(
            "{vis}fn {name}{}({}){ret} ",
            dsl.unwrap_or_default(),
            params.join(", ")
        ));
        self.function_body(out, body, depth, *throws, ret_type.is_some(), &name)?;
        out.push("\n");
        Ok(())
    }

    /// The body of a function, which differs from any other block in one way:
    /// a `throws` function has to hand back an `Ok`.
    fn function_body(
        &self,
        out: &mut Out,
        body: &Block,
        depth: usize,
        throws: bool,
        returns_value: bool,
        origin: &str,
    ) -> Result<()> {
        let flow = Flow { throws, origin };

        if !throws {
            return self.block(out, body, depth, flow, returns_value);
        }

        let pad = "    ".repeat(depth);
        let inner_pad = "    ".repeat(depth + 1);

        out.push("{\n");
        let last = body.stmts.len().saturating_sub(1);
        let mut i = 0;
        while i < body.stmts.len() {
            // The same pairing as in `block_opening_with`, through the same
            // helper. A `throws` body has a loop of its own because its last
            // statement may need wrapping in `Ok(…)`, and two loops that decide
            // this separately would drift.
            let tail_at = if returns_value { Some(last) } else { None };
            if self.overlap_at(out, &body.stmts, i, tail_at, depth + 1, flow)? {
                i += 2;
                continue;
            }

            let stmt = &body.stmts[i];
            out.push(&inner_pad);
            // A value-returning `throws` function ends in its value; one that
            // returns nothing ends in the `Ok(())` below, so its last statement
            // is a statement like any other.
            let is_tail = returns_value && i == last;
            out.from(&stmt.span, |out| {
                if is_tail {
                    out.push("Ok(");
                }
                self.stmt(out, &stmt.node, &stmt.span, depth + 1, is_tail, flow)?;
                if is_tail {
                    out.push(")");
                }
                Ok(())
            })?;
            out.push("\n");
            i += 1;
        }
        if !returns_value {
            out.push(&format!("{inner_pad}Ok(())\n"));
        }
        out.push(&pad);
        out.push("}");
        Ok(())
    }

    // --- Grammars ---

    fn grammar(&self, out: &mut Out, def: &GrammarDef) -> Result<()> {
        out.push("grammar! {\n");
        out.push(&format!("    grammar {} {{\n", self.text(def.name)));

        for rule in &def.rules {
            out.push("\n");
            out.from(&rule.span, |out| self.grammar_rule(out, rule))?;
        }

        out.push("    }\n");
        out.push("}\n");
        Ok(())
    }

    fn grammar_rule(&self, out: &mut Out, rule: &GrammarRule) -> Result<()> {
        if let Some(frame) = &rule.frame {
            out.push(&format!("        {}\n", frame_attribute(frame)));
        }

        let vis = if rule.is_public { "pub " } else { "" };
        let ret = match &rule.ret_type {
            Some(ty) => format!(" -> {}", self.ty(ty, Lifetimes::NAMED)),
            None => String::new(),
        };
        // The label sits between the return type and the `=`, in both
        // languages: `rule expr -> Expr # "expression" = …`.
        let label = match &rule.label {
            Some(text) => format!(" # {text:?}"),
            None => String::new(),
        };
        out.push(&format!(
            "        {vis}rule {}{ret}{label} =\n",
            self.text(rule.name)
        ));

        for (i, alt) in rule.alts.iter().enumerate() {
            let separator = if i == 0 { "  " } else { "| " };

            match &alt.action {
                Some(action) => {
                    out.push(&format!("          {separator}"));
                    self.pattern(out, &alt.pattern)?;
                    out.push("\n            -> ");
                    self.block(out, action, 3, Flow::PLAIN, true)?;
                    out.push("\n");
                }
                None => {
                    // The one body that needs no action: a `par_fold` is the
                    // whole rule (ADR-009 D2), so the value of the rule is the
                    // value of the fold. The binding the backend wants is
                    // supplied here rather than demanded from the user.
                    if rule.ret_type.is_some() {
                        out.push(&format!("          {separator}folded:"));
                        self.pattern(out, &alt.pattern)?;
                        out.push("\n            -> { folded }\n");
                    } else {
                        out.push(&format!("          {separator}"));
                        self.pattern(out, &alt.pattern)?;
                        out.push("\n");
                    }
                }
            }
        }

        Ok(())
    }

    fn pattern(&self, out: &mut Out, pattern: &Spanned<Pattern>) -> Result<()> {
        out.from(&pattern.span, |out| match &pattern.node {
            Pattern::Seq(parts) => self.patterns(out, parts, " "),
            Pattern::Choice(parts) => self.patterns(out, parts, " | "),
            Pattern::Bind { name, pat } => {
                out.push(&format!("{}:", self.text(*name)));
                self.pattern(out, pat)
            }
            Pattern::Literal(text) => {
                out.push(&format!("\"{text}\""));
                Ok(())
            }
            Pattern::Ref {
                name,
                generics,
                args,
            } => {
                out.push(self.text(*name));
                // `dec[i32](…)` -> `dec::<i32>(…)`? No: the backend's builtin
                // takes its type between angles, not after a turbofish, and a
                // grammar body is the backend's syntax once it is emitted.
                if !generics.is_empty() {
                    let params: Vec<String> = generics
                        .iter()
                        .map(|g| self.ty(g, Lifetimes::NAMED))
                        .collect();
                    out.push(&format!("<{}>", params.join(", ")));
                }
                if !args.is_empty() {
                    out.push("(");
                    self.patterns(out, args, ", ")?;
                    out.push(")");
                }
                Ok(())
            }
            Pattern::Repeat { pat, rep } => {
                // A sequence or a choice has to be grouped before a suffix can
                // apply to all of it rather than to its last element.
                let group = matches!(pat.node, Pattern::Seq(_) | Pattern::Choice(_));
                if group {
                    out.push("(");
                }
                self.pattern(out, pat)?;
                if group {
                    out.push(")");
                }
                out.push(&repeat_suffix(*rep));
                Ok(())
            }
            Pattern::Group(inner) => {
                out.push("(");
                self.pattern(out, inner)?;
                out.push(")");
                Ok(())
            }
            Pattern::Cut => {
                out.push("=>");
                Ok(())
            }
            Pattern::Fold(spec) => self.fold(out, spec),
        })
    }

    fn patterns(
        &self,
        out: &mut Out,
        patterns: &[Spanned<Pattern>],
        separator: &str,
    ) -> Result<()> {
        for (i, pattern) in patterns.iter().enumerate() {
            if i > 0 {
                out.push(separator);
            }
            self.pattern(out, pattern)?;
        }
        Ok(())
    }

    fn fold(&self, out: &mut Out, spec: &FoldSpec) -> Result<()> {
        let rule = self.text(spec.rule);
        let name = if spec.merge.is_some() {
            "par_fold"
        } else {
            "fold"
        };

        out.push(&format!("{name}({rule}, "));
        self.expr(out, &spec.init, 0, Flow::PLAIN)?;
        out.push(", ");
        self.fold_step(out, &spec.step)?;
        if let Some(merge) = &spec.merge {
            out.push(", ");
            self.fold_merge(out, merge)?;
        }
        out.push(")");
        Ok(())
    }

    /// A fold's step, adapted where the program's own declarations say it has
    /// to be.
    ///
    /// ADR-011 D2 refused to infer this from the shape of the body, and was
    /// right to: `acc.record(m)` and `acc.merged(m)` look identical. What
    /// changed is that `impl` blocks are lowered now, so this is a lookup in
    /// what the program declares - `record` takes `&mut self` and returns
    /// nothing, so the accumulator is threaded; a method that returns a new
    /// accumulator is left exactly as written.
    fn fold_step(&self, out: &mut Out, step: &Expr) -> Result<()> {
        if let Expr::Closure {
            params,
            implicit: false,
            body,
        } = step
        {
            if let [accumulator, item] = params.as_slice() {
                if let Some(Stmt::Expr(Expr::MethodCall {
                    receiver, method, ..
                })) = body.stmts.last().map(|s| &s.node)
                {
                    let on_accumulator =
                        matches!(&**receiver, Expr::Variable(name) if name == accumulator);
                    let mutates = self
                        .by_name
                        .get(method)
                        .and_then(|m| m.as_ref())
                        .is_some_and(Method::mutates_in_place);

                    if on_accumulator && mutates {
                        let accumulator = self.text(*accumulator);
                        out.push(&format!("|mut {accumulator}, {}| {{ ", self.text(*item)));
                        for stmt in &body.stmts {
                            self.stmt(out, &stmt.node, &stmt.span, 0, false, Flow::PLAIN)?;
                            out.push(" ");
                        }
                        out.push(&format!("{accumulator} }}"));
                        return Ok(());
                    }
                }
            }
        }

        self.expr(out, step, 0, Flow::PLAIN)
    }

    /// A fold's merge. `Summary::merge` names a method, and the `impl` says
    /// whether it mutates its subject and whether it takes the other by view.
    fn fold_merge(&self, out: &mut Out, merge: &Expr) -> Result<()> {
        if let Expr::Path(segments) = merge {
            if let [owner, name] = segments.as_slice() {
                if let Some(method) = self.methods.get(&(*owner, *name)) {
                    if method.mutates_in_place() && method.args.len() == 1 {
                        let by_view = if method.args[0].ty.is_view { "&" } else { "" };
                        // The accumulator's type is annotated because the path
                        // names it: without that, the backend's `let merge = …`
                        // leaves the closure with nothing to infer from.
                        let owner = self.ty(
                            &Type {
                                name: *owner,
                                generics: Vec::new(),
                                is_view: false,
                                is_tuple: false,
                            },
                            Lifetimes::NAMED,
                        );
                        out.push(&format!(
                            "|mut a: {owner}, b| {{ a.{}({by_view}b); a }}",
                            self.text(*name)
                        ));
                        return Ok(());
                    }
                }
            }
        }

        self.expr(out, merge, 0, Flow::PLAIN)
    }

    /// Kap 5.2: the implicit arguments are `a`, `b`, `c`, and a lambda takes as
    /// many of them as its body reaches for.
    ///
    /// *Reaches for* is any mention, a local of the same name included - which
    /// is why the three names are the lambda's and a body must not bind them
    /// (Part I, 5.2). Telling a use from a shadowing binding would need the
    /// scope analysis Stage 0 does not have, and guessing wrong either way
    /// produces a closure whose arity does not match its call.
    fn implicit_params(&self, body: &Block) -> Vec<String> {
        const NAMES: [&str; 3] = ["a", "b", "c"];

        let mut used = [false; 3];
        let mut mark = |expr: &Expr| {
            if let Expr::Variable(name) = expr {
                if let Some(i) = NAMES.iter().position(|n| *n == self.text(*name)) {
                    used[i] = true;
                }
            }
        };
        visit_block(body, &mut |expr| {
            mark(expr);
            // A string's holes are expressions too, and they are the one place
            // a body can reach for `a` without the AST showing it: a literal
            // keeps its text and the holes are parsed when it is emitted. A
            // lambda whose whole body is `"{a.0} {a.1}"` would otherwise be
            // generated with no parameters at all.
            for hole in literal_expressions(self.parsed, expr) {
                visit_expr(&hole, &mut mark);
            }
        });

        let count = used.iter().rposition(|u| *u).map_or(0, |i| i + 1);
        NAMES[..count].iter().map(|n| n.to_string()).collect()
    }

    // --- Types ---

    /// The shadow type of every deferred-parameter DSL in this unit
    /// (ADR-007 D5).
    ///
    /// One struct per parameter list, a field per `:name`, and **generic in
    /// every field**: nothing in `… WHERE id = :id …` says what `:id` is, so
    /// the type comes from the argument at the call site and is monomorphised
    /// there. Inventing `i32` here is exactly the guess ADR-011 D2 forbids an
    /// emitter to make.
    ///
    /// A plain struct, passed by value: D5 asks for the parameters on the
    /// stack, and a struct of the caller's own values is what that is.
    fn shadow_types(&self, out: &mut Out) {
        for (name, parameters) in crate::dsl::shadow_types(self.parsed) {
            let generics: Vec<String> = (0..parameters.len()).map(|i| format!("P{i}")).collect();
            let fields: Vec<String> = parameters
                .iter()
                .zip(&generics)
                .map(|(field, ty)| format!("    pub {field}: {ty},"))
                .collect();
            out.push(&format!(
                "// ADR-007 D5: the parameters of a `dsl … {{ … }} eod` statement, \
                 as a type.\n\
                 #[derive(Debug, Clone, Copy, PartialEq)]\n\
                 #[allow(non_camel_case_types, dead_code)]\n\
                 pub struct {name}<{}> {{\n{}\n}}\n\n",
                generics.join(", "),
                fields.join("\n"),
            ));
        }
    }

    /// A `dsl … { … } eod` body: the `html` template compiled here (ADR-017),
    /// or a statement whose holes the call site fills (ADR-007 D5).
    ///
    /// Which it is follows from the target and the holes, and not from anything
    /// inferred about the body. `html` is the one grammar this compiler *is*,
    /// so `:name` there is an immediate capture (ADR-007 D4); anywhere else a
    /// `:name` is a deferred parameter and the body reaches its driver intact.
    /// A body with neither is refused, because nothing here knows what it
    /// means.
    fn template(
        &self,
        out: &mut Out,
        target: Symbol,
        context: Option<&Symbol>,
        content: &str,
        depth: usize,
        flow: Flow<'_>,
    ) -> Result<()> {
        let name = self.text(target);

        // ADR-007 D5: a body with `:name` holes and a target this compiler is
        // not itself the grammar for is a *statement*, and its value is its own
        // text. Nothing is substituted into it: a deferred parameter is not
        // string interpolation, so the value may not be spliced into the source
        // and change what it means (Part III, 15.3). The holes stay as written
        // and the driver binds them - which is also the only lowering that
        // needs to know nothing about the foreign syntax (ADR-011 D2).
        if crate::dsl::is_deferred(name, content) {
            if let Some(context) = context {
                return Err(anyhow!(
                    "`dsl {name} {{ … }}` with deferred parameters takes no context, \
                     and `{}` was given one",
                    self.text(*context)
                ));
            }
            out.push(&rust_string(content.trim()));
            return Ok(());
        }

        if name != "html" {
            return Err(anyhow!(
                "`dsl {name} {{ … }}` has no hole, so nothing here says what it \
                 means. A statement with `:name` holes is a deferred-parameter DSL \
                 and lowers (ADR-007 D5); one without them is the target grammar's \
                 to give a meaning, and `{name}` is not a grammar this compiler has \
                 - the one it is itself the grammar for is `html` (ADR-017)."
            ));
        }
        if let Some(context) = context {
            return Err(anyhow!(
                "`dsl html` takes no context, and `{}` was given one",
                self.text(*context)
            ));
        }

        // The framing whitespace is not markup: the newline after `{` and the
        // indentation before `} eod` are there because the template is written
        // in a file, and a block form that kept them would make every value it
        // produces carry the indentation of the function it was written in.
        // Whitespace *inside* the body is kept exactly.
        let segments = template::split(content.trim())?;

        // ADR-017 D3. Every hole that escaping cannot make safe, at once: a
        // template with three of them should say so three times rather than
        // once per build.
        let illegal = template::illegal(&segments);
        if !illegal.is_empty() {
            let mut message = String::from("the template has holes escaping cannot make safe\n");
            for (expr, at) in &illegal {
                message.push_str(&format!("  {}\n", template::illegal_message(expr, *at)));
            }
            return Err(anyhow!(message));
        }

        let pad = "    ".repeat(depth + 1);
        let close = "    ".repeat(depth);
        // The compiler knows every literal byte of this template, so it knows
        // what the result is at least as long as - and a `String` that starts
        // that size does not double its way there
        // (`docs/staging-candidates.md` §3).
        //
        // It is a *floor*, not a guess: the holes and the bodies of `<for>`
        // add to it and never subtract, so reserving it can never be too much
        // to be worth it, and this needs no threshold and no measurement to
        // decide between two shapes. What it needed a measurement for is
        // whether it is worth doing at all, and that is in
        // `crates/nikaia/tests/measure.rs`.
        out.push(&format!(
            "{{\n{pad}let mut __html = String::with_capacity({});\n",
            template::literal_length(&segments)
        ));
        self.template_segments(out, &segments, depth + 1, flow)?;
        out.push(&format!("{pad}__html\n{close}}}"));
        Ok(())
    }

    /// The pieces of a template, appended to `__html` in order.
    fn template_segments(
        &self,
        out: &mut Out,
        segments: &[template::Segment],
        depth: usize,
        flow: Flow<'_>,
    ) -> Result<()> {
        let pad = "    ".repeat(depth);

        for segment in segments {
            match segment {
                template::Segment::Text(text) => {
                    out.push(&format!("{pad}__html.push_str({});\n", rust_string(text)));
                }
                template::Segment::Hole { expr, .. } => {
                    // Parsed as Nikaia and emitted as Nikaia: a hole holds an
                    // expression of this language, not a foreign one.
                    let parsed = parse_expression(&self.parsed.interner, expr)
                        .map_err(|e| anyhow!("in the template hole `{{{expr}}}`: {e}"))?;
                    out.push(&format!(
                        "{pad}__html.push_str(&::nikaia_std::html::Render::render(&"
                    ));
                    self.expr(out, &parsed, depth, flow)?;
                    out.push("));\n");
                }
                // The loop of the language below, over the captured collection:
                // it borrows rather than copies, exactly as it would in the
                // function around the template.
                template::Segment::For {
                    binding,
                    collection,
                    body,
                } => {
                    out.push(&format!("{pad}for {binding} in &{collection} {{\n"));
                    self.template_segments(out, body, depth + 1, flow)?;
                    out.push(&format!("{pad}}}\n"));
                }
            }
        }
        Ok(())
    }

    /// A path, with the map it names chosen the same way its type is.
    ///
    /// `HashMap::new` has no counterpart on a map with a hasher of its own -
    /// `new` exists only for the default one - so the trusted map is built with
    /// `default`, which is what every `HashMap<_, _, S>` is built with.
    fn path(&self, segments: &[&str]) -> String {
        if let [container, "new"] = segments {
            let chosen = self.map_name(container);
            if chosen != *container {
                return format!("{chosen}::default");
            }
        }
        segments
            .iter()
            .map(|s| self.map_name(s))
            .collect::<Vec<_>>()
            .join("::")
    }

    /// `HashMap` under the name the provenance of this program's input picked
    /// (ADR-010 D5).
    ///
    /// Trusted keys get a fast, fixed-seed hash; untrusted keys keep the keyed,
    /// randomly seeded one `std` gives every map by default. Nothing else about
    /// the map changes - same table, same API, same full-content equality - so
    /// this is a name and not a translation.
    fn map_name<'n>(&self, name: &'n str) -> &'n str {
        match (name, self.trusted_input) {
            ("HashMap", true) => "TrustedMap",
            ("HashSet", true) => "TrustedSet",
            _ => name,
        }
    }

    fn ty(&self, ty: &Type, lifetimes: Lifetimes) -> String {
        let mut out = String::new();

        // `(A, B)` in both languages, and the parts are the arguments.
        if ty.is_tuple {
            let parts: Vec<String> = ty.generics.iter().map(|g| self.ty(g, lifetimes)).collect();
            return format!("({})", parts.join(", "));
        }

        // A view is a borrow of the parser's input, and that is where the
        // lifetime comes from - the source never writes one (ADR-008).
        if ty.is_view {
            out.push_str(lifetimes.reference);
        }
        out.push_str(self.map_name(self.text(ty.name)));

        let mut params: Vec<String> = ty.generics.iter().map(|g| self.ty(g, lifetimes)).collect();
        // A struct that holds a view carries the input lifetime with it.
        if self.borrowing.contains(&ty.name) {
            params.insert(0, lifetimes.params.to_string());
        }
        if !params.is_empty() {
            out.push_str(&format!("<{}>", params.join(", ")));
        }

        out
    }

    // --- Statements and expressions ---

    /// `tail` says whether the last statement is the block's value. It is for
    /// an expression block (Part I, 3.1) and for a function that returns
    /// something; a function with no return type has no value to leave behind,
    /// so its last statement is a statement like any other.
    /// `if c { … } else { … }`, where `tail` says whether the whole thing is in
    /// value position.
    ///
    /// It decides what the last statement of each branch means. In an
    /// expression - `let x = if c { a } else { b }` - the branches *are* the
    /// value and their last statement is written as one. As a statement they
    /// are not, and a `return` in one has to stay a `return`.
    #[allow(clippy::too_many_arguments)]
    fn if_expr(
        &self,
        out: &mut Out,
        cond: &Expr,
        then_branch: &Block,
        else_branch: Option<&Block>,
        depth: usize,
        flow: Flow<'_>,
        tail: bool,
    ) -> Result<()> {
        out.push("if ");
        self.expr(out, cond, depth, flow)?;
        out.push(" ");
        self.block(out, then_branch, depth, flow, tail)?;
        if let Some(block) = else_branch {
            out.push(" else ");
            self.block(out, block, depth, flow, tail)?;
        }
        Ok(())
    }

    fn block(
        &self,
        out: &mut Out,
        block: &Block,
        depth: usize,
        flow: Flow<'_>,
        tail: bool,
    ) -> Result<()> {
        self.block_opening_with(out, block, depth, flow, tail, None)
    }

    /// The same, with one line written before the statements.
    ///
    /// One caller: a `for` whose step can fail opens its body by unwrapping the
    /// step (ADR-025 D7). It is a parameter rather than a wrapping block
    /// because an extra brace level in the emitted Rust is a level the source
    /// map has to explain and the reader has to skip.
    fn block_opening_with(
        &self,
        out: &mut Out,
        block: &Block,
        depth: usize,
        flow: Flow<'_>,
        tail: bool,
        opening: Option<&str>,
    ) -> Result<()> {
        if block.stmts.is_empty() {
            match opening {
                Some(line) => {
                    out.push(&format!("{{ {line} }}"));
                    return Ok(());
                }
                None => {
                    out.push("{ }");
                    return Ok(());
                }
            }
        }

        // A one-statement block stays on its line. Most action blocks are one
        // expression, and a grammar reads better when its actions do not push
        // the pattern three lines apart. Whether it fits is decided by
        // rendering it, not by guessing from the shape.
        if block.stmts.len() == 1 && opening.is_none() {
            let only = block.stmts.first().expect("one statement");
            let rendered = Out::scratch(|scratch| {
                scratch.from(&only.span, |s| {
                    self.stmt(s, &only.node, &only.span, depth, tail, flow)
                })
            })?;
            if !rendered.buf.contains('\n') {
                out.push("{ ");
                out.append(rendered);
                out.push(" }");
                return Ok(());
            }
        }

        let pad = "    ".repeat(depth);
        let inner_pad = "    ".repeat(depth + 1);

        out.push("{\n");
        if let Some(line) = opening {
            out.push(&inner_pad);
            out.push(line);
            out.push("\n");
        }
        let last = block.stmts.len() - 1;
        let mut i = 0;
        while i < block.stmts.len() {
            // ADR-033: two statements that meet on nothing need not wait for one
            // another. Only a *pair* today, and only where neither is the tail -
            // a block's last statement is its value (Kap 3.1) and lowering it
            // through a join would change what the block hands back.
            // Neither may be the tail: a block's last statement is its value
            // (Kap 3.1), and lowering it through a join would change what the
            // block hands back.
            let tail_at = if tail { Some(last) } else { None };
            if self.overlap_at(out, &block.stmts, i, tail_at, depth + 1, flow)? {
                i += 2;
                continue;
            }

            let stmt = &block.stmts[i];
            out.push(&inner_pad);
            out.from(&stmt.span, |out| {
                self.stmt(
                    out,
                    &stmt.node,
                    &stmt.span,
                    depth + 1,
                    tail && i == last,
                    flow,
                )
            })?;
            out.push("\n");
            i += 1;
        }
        out.push(&pad);
        out.push("}");
        Ok(())
    }

    /// Write `stmts[i]` and `stmts[i + 1]` as one overlapped pair, where they may
    /// be one (ADR-033).
    ///
    /// `Ok(false)` means nothing was written and the caller should emit
    /// `stmts[i]` the ordinary way. The two statement loops in this file - a
    /// block's and a `throws` body's - both go through here, because a rule
    /// about what may be reordered that two places decide separately is a rule
    /// that will eventually be two rules.
    ///
    /// `tail_at` is the index of the statement that is the block's **value**,
    /// where there is one. Neither half of a pair may be it: a block's last
    /// statement is what it hands back (Kap 3.1), and a join hands back a tuple.
    fn overlap_at(
        &self,
        out: &mut Out,
        stmts: &[Spanned<Stmt>],
        i: usize,
        tail_at: Option<usize>,
        depth: usize,
        flow: Flow<'_>,
    ) -> Result<bool> {
        if !self.build.overlaps_user_code()
            || self.ordering != Ordering::Effects
            || i + 1 >= stmts.len()
        {
            return Ok(false);
        }
        if tail_at.is_some_and(|tail| tail == i || tail == i + 1) {
            return Ok(false);
        }
        let (earlier, later) = (&stmts[i], &stmts[i + 1]);
        if !self.may_overlap(&earlier.node, &later.node) {
            return Ok(false);
        }

        out.push(&"    ".repeat(depth));
        self.overlapped(out, earlier, later, depth, flow)?;
        out.push("\n");
        Ok(true)
    }

    /// Whether two adjacent statements may run at the same time (ADR-033).
    ///
    /// Both have to be operations this compiler can account for *completely* -
    /// `contracts::order::operation` says `None` for everything else - and their
    /// touch sets have to meet on nothing either of them writes. A `None` on
    /// either side is an admission of ignorance and keeps the order, which is
    /// the same answer a real dependency gets and deliberately so (D4).
    fn may_overlap(&self, earlier: &Stmt, later: &Stmt) -> bool {
        let earlier = crate::contracts::order::operation(
            self.parsed,
            earlier,
            &self.own_contracts,
            &self.library,
        );
        let later = crate::contracts::order::operation(
            self.parsed,
            later,
            &self.own_contracts,
            &self.library,
        );
        match (earlier, later) {
            (Some(earlier), Some(later)) => crate::contracts::order::may_overlap(&earlier, &later),
            _ => false,
        }
    }

    /// Two statements, lowered to run at the same time and be collected
    /// together.
    ///
    /// ```text
    /// let (a, b) = task::both(
    ///     || … ,
    ///     || … ,
    /// );
    /// ```
    ///
    /// Either of them may be a **bare expression statement** rather than a
    /// `let` (ADR-033 §8.3's first item): `fs::write("a.txt", "1")` binds
    /// nothing, so its half of the pattern is `_` - and where neither binds,
    /// there is no pattern at all and the call stands as a statement.
    ///
    /// Threads and not a runtime: `std`'s I/O is blocking Rust (`std::fs::read`
    /// behind `fs::read`), so overlapping it means threads. *Which* threads is
    /// `nikaia_std::task` deciding and not this function - it runs the pair on
    /// the pool the program already has, so a handler that overlaps under load
    /// asks for a bounded number of threads (ADR-033 §8.4). Naming one `std`
    /// function also keeps this lowering one line long instead of a scope, two
    /// spawns and two joins spelled into every program that uses it.
    ///
    /// A panic inside either reaches the caller, so a program that would have
    /// panicked still panics with its own message and its own payload. Turning
    /// somebody's panic into `called Result::unwrap on an Err` would be this
    /// lowering putting its own words in the program's mouth.
    fn overlapped(
        &self,
        out: &mut Out,
        earlier: &Spanned<Stmt>,
        later: &Spanned<Stmt>,
        depth: usize,
        flow: Flow<'_>,
    ) -> Result<()> {
        let (first_bind, first_value) = self.overlapped_half(&earlier.node);
        let (second_bind, second_value) = self.overlapped_half(&later.node);

        let pad = "    ".repeat(depth);
        let inner = "    ".repeat(depth + 1);

        out.push(&format!(
            "// ADR-033: these two meet on nothing, so neither waits for the other.\n{pad}"
        ));
        // A pair where nothing is bound is a statement and not a binding: `let
        // (_, _) = …` would be a pattern that says nothing, and the emitted
        // Rust is read by people.
        let pattern = match (&first_bind, &second_bind) {
            (None, None) => String::new(),
            (first, second) => format!(
                "let ({}, {}) = ",
                first.as_deref().unwrap_or("_"),
                second.as_deref().unwrap_or("_"),
            ),
        };
        // `task::both` and not `std::thread::scope` inline: the vehicle is
        // `std`'s decision, not a shape baked into every generated program.
        // It runs on the pool the program already has, so a handler that
        // overlaps two reads under a thousand concurrent requests asks for a
        // bounded number of threads rather than two thousand (ADR-033 §8.4).
        out.push(&format!("{pattern}task::both(\n{inner}|| "));

        out.from(&earlier.span, |out| {
            self.expr(out, first_value, depth + 1, flow)
        })?;
        out.push(&format!(",\n{inner}|| "));

        out.from(&later.span, |out| {
            self.expr(out, second_value, depth + 1, flow)
        })?;
        out.push(&format!(",\n{pad});"));
        Ok(())
    }

    /// One half of an overlapped pair: what it binds, and what it evaluates.
    ///
    /// `contracts::order` accounts for a `let` and for a bare expression
    /// statement, and those are the only two shapes that reach here.
    fn overlapped_half<'s>(&self, stmt: &'s Stmt) -> (Option<String>, &'s Expr) {
        match stmt {
            Stmt::Let {
                name,
                mutable,
                value,
                ..
            } => (
                Some(format!(
                    "{}{}",
                    if *mutable { "mut " } else { "" },
                    self.text(*name)
                )),
                value,
            ),
            Stmt::Expr(value) => (None, value),
            _ => unreachable!("`may_overlap` accepts only a `let` or an expression statement"),
        }
    }

    /// `is_tail` marks the last statement of a block: Nikaia's blocks are
    /// expressions (Part I, 3.1), so the last expression is the value and keeps
    /// no semicolon.
    fn stmt(
        &self,
        out: &mut Out,
        stmt: &Stmt,
        span: &Span,
        depth: usize,
        is_tail: bool,
        flow: Flow<'_>,
    ) -> Result<()> {
        match stmt {
            Stmt::Let {
                name,
                mutable,
                ty,
                value,
            } => {
                let mutable = if *mutable { "mut " } else { "" };
                let annotation = match ty {
                    Some(ty) => format!(": {}", self.ty(ty, Lifetimes::ELIDED)),
                    None => String::new(),
                };
                out.push(&format!("let {mutable}{}{annotation} = ", self.text(*name)));
                self.expr(out, value, depth, flow)?;
                out.push(";");
            }
            Stmt::Assign { target, op, value } => {
                self.expr(out, target, depth, flow)?;
                match op {
                    Some(op) => out.push(&format!(" {}= ", binary_op(*op))),
                    None => out.push(" = "),
                }
                self.expr(out, value, depth, flow)?;
                out.push(";");
            }
            // Kap 3.3. Name for name (ADR-011 D2): the language below spells
            // this the same way, so there is nothing to decide here.
            Stmt::While { cond, body } => {
                out.push("while ");
                self.expr(out, cond, depth, flow)?;
                out.push(" ");
                self.block(out, body, depth, flow, false)?;
            }

            Stmt::For {
                bindings,
                iter,
                body,
            } => {
                let names = bindings
                    .iter()
                    .map(|b| self.text(*b))
                    .collect::<Vec<_>>()
                    .join(", ");
                if bindings.len() > 1 {
                    out.push(&format!("for ({names}) in "));
                } else {
                    out.push(&format!("for {names} in "));
                }
                self.expr(out, iter, depth, flow)?;
                out.push(" ");

                // ADR-025 D1: a step that can fail fails the enclosing
                // function, exactly as a written call would. This is that,
                // and it is the line a Rust programmer writes by hand. The
                // compiler has already refused the program if the function
                // does not declare `throws` (`NK2701`), so the `?` always has
                // somewhere to go.
                let unwrap = self
                    .fallible_loops
                    .contains(&span.start)
                    .then(|| format!("let {names} = {names}?;"));
                self.block_opening_with(out, body, depth, flow, false, unwrap.as_deref())?;
            }
            // A `return` that is the last statement is the block's value, and
            // is written as one: `fn f() -> T { return x }` is `{ x }`. In a
            // `throws` function the caller of this wraps the tail in `Ok`, so
            // the value is emitted bare here in both cases.
            Stmt::Return(Some(value)) if is_tail => {
                self.expr(out, value, depth, flow)?;
            }
            Stmt::Return(value) => {
                // Kap 7.1: a `throws` function returns a `Result`, so what the
                // source hands back is what goes inside the `Ok`.
                match (value, flow.throws) {
                    (Some(value), true) => {
                        out.push("return Ok(");
                        self.expr(out, value, depth, flow)?;
                        out.push(");");
                    }
                    (Some(value), false) => {
                        out.push("return ");
                        self.expr(out, value, depth, flow)?;
                        out.push(";");
                    }
                    (None, true) => out.push("return Ok(());"),
                    (None, false) => out.push("return;"),
                }
            }
            // An `if` in *statement* position is not a value, and its branches
            // are not tails. Emitting them as tails is right for
            // `let x = if c { a } else { b }` and wrong here: a `return` at the
            // end of a branch would be written as the branch's value and stop
            // returning - `if seq.len() < k { return counts }` came out as
            // `if seq.len() < k { counts }`, which is a different program.
            Stmt::Expr(Expr::If {
                cond,
                then_branch,
                else_branch,
            }) => self.if_expr(
                out,
                cond,
                then_branch,
                else_branch.as_ref(),
                depth,
                flow,
                is_tail,
            )?,
            Stmt::Expr(expr) => {
                self.expr(out, expr, depth, flow)?;
                // `if x { … };` is legal and noisy; a block-shaped statement
                // ends where its brace does.
                let block_shaped = matches!(expr, Expr::If { .. } | Expr::Block(_));
                if !is_tail && !block_shaped {
                    out.push(";");
                }
            }
        }
        Ok(())
    }

    fn expr(&self, out: &mut Out, expr: &Expr, depth: usize, flow: Flow<'_>) -> Result<()> {
        match expr {
            Expr::LitInt(v) => out.push(&v.to_string()),
            Expr::LitFloat(v) => out.push(v),
            Expr::LitStr(_) | Expr::LitInterpolated(_) => self.string(out, expr, depth, flow)?,
            Expr::LitChar(c) => out.push(&format!("'{c}'")),
            Expr::Range {
                start,
                end,
                inclusive,
            } => {
                self.expr(out, start, depth, flow)?;
                out.push(if *inclusive { "..=" } else { ".." });
                self.expr(out, end, depth, flow)?;
            }
            Expr::LitBool(b) => out.push(&b.to_string()),
            Expr::Variable(name) => out.push(self.text(*name)),
            // ADR-017: the template is compiled where it is written. What comes
            // out is the string building a hand-written renderer would do, with
            // `html::Render` at every hole - which is what makes the escaping a
            // property of the template rather than of whoever filled it in.
            Expr::Dsl {
                target,
                content,
                context,
            } => self.template(out, *target, context.as_ref(), content, depth, flow)?,
            Expr::Path(segments) => {
                let path: Vec<&str> = segments.iter().map(|s| self.text(*s)).collect();
                out.push(&self.path(&path));
            }
            Expr::Block(block) => self.block(out, block, depth, flow, true)?,
            Expr::If {
                cond,
                then_branch,
                else_branch,
            } => self.if_expr(
                out,
                cond,
                then_branch,
                else_branch.as_ref(),
                depth,
                flow,
                true,
            )?,
            Expr::Call { func, args, config } => self.call(out, func, args, config, depth, flow)?,
            Expr::MethodCall {
                receiver,
                method,
                args,
                config,
            } => {
                self.postfix_base(out, receiver, depth, flow)?;
                out.push(&format!(".{}", self.text(*method)));
                // Nikaia's `collect` builds a List; Rust's needs to be told
                // what to build, and with no types here that is `Vec<_>`.
                if self.text(*method) == "collect" && args.is_empty() {
                    out.push("::<Vec<_>>");
                }
                out.push("(");
                self.args(out, args, depth, flow)?;
                self.dsl_parameters(out, self.text(*method), args.len(), config, depth, flow)?;
                out.push(")");
            }
            Expr::Match { value, arms } => {
                out.push("match ");
                self.expr(out, value, depth, flow)?;
                let pad = "    ".repeat(depth + 1);
                let close = "    ".repeat(depth);
                out.push(" {\n");
                for arm in arms {
                    out.push(&pad);
                    self.match_pattern(out, &arm.pattern, depth + 1, flow)?;
                    out.push(" => ");
                    self.expr(out, &arm.body, depth + 1, flow)?;
                    out.push(",\n");
                }
                out.push(&format!("{close}}}"));
            }
            Expr::Tuple(parts) => {
                out.push("(");
                for (i, part) in parts.iter().enumerate() {
                    if i > 0 {
                        out.push(", ");
                    }
                    self.expr(out, part, depth, flow)?;
                }
                out.push(")");
            }
            Expr::Field { base, name } => {
                self.postfix_base(out, base, depth, flow)?;
                out.push(&format!(".{}", self.text(*name)));
            }
            Expr::Index { base, index } => {
                self.postfix_base(out, base, depth, flow)?;
                out.push("[");
                self.expr(out, index, depth, flow)?;
                out.push("]");
            }
            Expr::Cast { expr, ty } => {
                self.nested(out, expr, u8::MAX, depth, flow)?;
                out.push(&format!(" as {}", self.ty(ty, Lifetimes::ELIDED)));
            }
            Expr::StructLit { name, fields } => {
                out.push(&format!("{} {{ ", self.text(*name)));
                for (i, field) in fields.iter().enumerate() {
                    if i > 0 {
                        out.push(", ");
                    }
                    out.push(self.text(field.name));
                    if let Some(value) = &field.value {
                        out.push(": ");
                        self.expr(out, value, depth, flow)?;
                    }
                }
                out.push(" }");
            }
            Expr::Closure {
                params,
                implicit,
                body,
            } => {
                let params = if *implicit {
                    self.implicit_params(body)
                } else {
                    params.iter().map(|p| self.text(*p).to_string()).collect()
                };
                out.push(&format!("|{}| ", params.join(", ")));
                // A lambda's `return` leaves the lambda, not the function
                // around it, so it never carries the enclosing `Ok`.
                self.block(out, body, depth, Flow::PLAIN, true)?;
            }
            Expr::Unary { op, expr } => {
                out.push(unary_op(*op));
                self.nested(out, expr, u8::MAX, depth, flow)?;
            }
            Expr::Binary { op, lhs, rhs } => {
                // Parenthesised only where precedence needs it: the operators
                // mean the same in both languages, so `value * 10 + n` should
                // come out the way it went in.
                let here = precedence(*op);
                self.nested(out, lhs, here, depth, flow)?;
                out.push(&format!(" {} ", binary_op(*op)));
                self.nested(out, rhs, here + 1, depth, flow)?;
            }
            Expr::Coalesce { value, fallback } => {
                // Kap 3.5. `into()` because the fallback is written as the
                // value it stands for, not as the type the option holds.
                self.expr(out, value, depth, flow)?;
                out.push(".unwrap_or_else(|| ");
                self.expr(out, fallback, depth, flow)?;
                out.push(".into())");
            }
            Expr::TryCatch { expr, handler } => {
                // Kap 7.1: the handler sees the error as `error`.
                out.push("match ");
                // `catch` needs the `Result`, not the value: a `dsl … from …`
                // propagates on its own everywhere else, and here the handler
                // is what handles it.
                match expr.as_ref() {
                    Expr::DslFrom { grammar, input } => {
                        self.dsl_from(out, *grammar, input, depth, flow, Propagate::No)?
                    }
                    _ => self.expr(out, expr, depth, flow)?,
                }
                let pad = "    ".repeat(depth + 1);
                let close = "    ".repeat(depth);
                out.push(&format!(
                    " {{\n{pad}Ok(value) => value,\n{pad}Err(error) => "
                ));
                self.block(out, handler, depth + 1, flow, true)?;
                out.push(&format!(",\n{close}}}"));
            }
            Expr::Try(inner) => {
                self.postfix_base(out, inner, depth, flow)?;
                out.push("?");
            }
            // Kap 7.1: `throw e` leaves the function with `e` in the failure
            // channel. `Box::new` is what puts it there, and it is the emitter's
            // to write rather than the author's - ADR-023 D2 gives the language
            // one way to raise an error, not one way plus a conversion.
            // Kap 7.1: `throw e` leaves the function with `e` in the failure
            // channel. `raise` is what puts it there, and it carries the site
            // the compiler knew and the author did not have to write (D6). The
            // trace it may attach is off unless the program asked - measured at
            // 28 300 instructions an error, which is not a default.
            Expr::Throw(inner) => {
                out.push("return Err(nikaia_std::error::raise(");
                self.expr(out, inner, depth, flow)?;
                out.push(&format!(", {:?}))", flow.origin));
            }
            Expr::Spawn { .. } => {
                // Part II, 11.2. The runtime binding is the next roadmap line.
                return Err(anyhow!(
                    "`spawn` needs the runtime integration; not emitted yet"
                ));
            }
            Expr::DslFrom { grammar, input } => {
                self.dsl_from(out, *grammar, input, depth, flow, Propagate::Yes)?
            }
            other => return Err(anyhow!("cannot emit expression yet: {other:?}")),
        }
        Ok(())
    }

    /// A call. `Stats(temp)` is the anonymous constructor of Kap 4.2 - Rust has
    /// no such thing, so the declaration decides: a name that is a struct is a
    /// call to the `new` its `impl` provides.
    fn call(
        &self,
        out: &mut Out,
        func: &Expr,
        args: &[Expr],
        config: &[crate::ast::ConfigArg],
        depth: usize,
        flow: Flow<'_>,
    ) -> Result<()> {
        if let Expr::Variable(name) = func {
            let text = self.text(*name);

            // `println`, `print` and their `stderr` halves are macros in
            // Rust, and their argument is an interpolated string, which is a
            // format string already. `print` is here because output composed
            // piece by piece - a pretty-printer, a progress line - cannot be
            // written with the newline attached.
            if matches!(text, "println" | "eprintln" | "print" | "eprint") {
                if let [Expr::LitInterpolated(literal)] = args {
                    out.push(&format!("{text}!("));
                    self.format_string(out, literal, depth, flow)?;
                    out.push(")");
                    return Ok(());
                }
                // A plain string is text, and Rust's macro would read a brace
                // in it as a hole of its own - so the braces are escaped on the
                // way down rather than the argument being passed separately
                // (ADR-035 D2). `print("{")` prints a brace.
                if let [Expr::LitStr(literal)] = args {
                    out.push(&format!("{text}!(\"{}\")", rust_format_escape(literal)));
                    return Ok(());
                }
                out.push(&format!("{text}!(\"{{}}\", "));
                self.args(out, args, depth, flow)?;
                out.push(")");
                return Ok(());
            }

            if self.structs.contains(name) {
                out.push(&format!("{text}::new("));
                self.args(out, args, depth, flow)?;
                out.push(")");
                return Ok(());
            }
        }

        self.expr(out, func, depth, flow)?;
        out.push("(");
        self.args(out, args, depth, flow)?;

        if let Expr::Variable(name) = func {
            self.dsl_parameters(out, self.text(*name), args.len(), config, depth, flow)?;
        }

        // Kap 5.1: the language below has no named arguments and no defaults,
        // so the options become positional here, in the order the *declaration*
        // gives - which is the only order there is, and the reason this needs
        // the callee's contract rather than the call alone.
        if let Some(options) = self.options_of(func) {
            for option in options {
                out.push(", ");
                match config.iter().find(|a| self.text(a.name) == option.name) {
                    Some(passed) => self.expr(out, &passed.value, depth, flow)?,
                    // Not passed, so the declaration's default is the value.
                    // It is a literal, and Nikaia spells a literal the way the
                    // language below does (ADR-011 D2).
                    None => out.push(&option.default),
                }
            }
        }

        out.push(")");
        Ok(())
    }

    /// ADR-007 D5: the shadow struct a call to a DSL driver builds.
    ///
    /// A `;` at a call site means options everywhere else, and the callee's
    /// declaration is what tells the two apart: a driver said
    /// `...args: Self::dsl`, so what stands after its `;` is a DSL's deferred
    /// parameters and becomes one value rather than one argument each.
    ///
    /// A struct literal names its fields, so a call may write them in any
    /// order - the body's order decides the type's fields and nothing else.
    fn dsl_parameters(
        &self,
        out: &mut Out,
        callee: &str,
        args: usize,
        config: &[crate::ast::ConfigArg],
        depth: usize,
        flow: Flow<'_>,
    ) -> Result<()> {
        if config.is_empty() || !self.dsl_drivers.contains(callee) {
            return Ok(());
        }
        let names: Vec<String> = config
            .iter()
            .map(|a| self.text(a.name).to_string())
            .collect();
        if args > 0 {
            out.push(", ");
        }
        out.push(&format!("{} {{ ", crate::dsl::type_name(&names)));
        for (i, argument) in config.iter().enumerate() {
            if i > 0 {
                out.push(", ");
            }
            out.push(&format!("{}: ", self.text(argument.name)));
            self.expr(out, &argument.value, depth, flow)?;
        }
        out.push(" }");
        Ok(())
    }

    /// The options a call by name has, from the contract that declares them.
    ///
    /// This unit's own first, then `std`'s - the order every other name
    /// resolution here uses. `None` where the callee has none, and where the
    /// callee cannot be resolved at all: an unresolvable call has no options to
    /// fill in, and the type checker is what says so in Nikaia's words.
    fn options_of(&self, func: &Expr) -> Option<&[crate::contracts::ConfigContract]> {
        let name = match func {
            Expr::Variable(name) => self.text(*name).to_string(),
            Expr::Path(segments) => segments
                .iter()
                .map(|s| self.text(*s))
                .collect::<Vec<_>>()
                .join("::"),
            _ => return None,
        };

        let contract = self
            .own_contracts
            .functions
            .get(&name)
            .or_else(|| self.own_contracts.functions.get(&format!("{name}::new")))
            .or_else(|| self.library.lookup(&name).map(|(_, c)| c))?;

        let config = &contract.signature.as_ref()?.config;
        (!config.is_empty()).then_some(config.as_slice())
    }

    /// A string literal. Which of the two it is decides what comes out, and
    /// that is read off the syntax rather than the text (ADR-035 D3).
    fn string(&self, out: &mut Out, expr: &Expr, depth: usize, flow: Flow<'_>) -> Result<()> {
        match expr {
            // Inert text, transcribed. A brace is a brace, so nothing has to be
            // escaped on the way into a Rust string literal - only a *format*
            // string treats one specially, and this is not one.
            Expr::LitStr(literal) => {
                out.push(&format!("\"{literal}\""));
                Ok(())
            }
            // `f"…"` is a `String` whether or not anyone put a hole in it,
            // because the checker says it is and the two have to agree. With no
            // hole there is nothing to format, and `format!("x")` is a
            // roundabout way of writing what `.to_string()` says plainly.
            Expr::LitInterpolated(literal) => {
                if interpolation(literal)?.1.is_empty() {
                    out.push(&format!("\"{literal}\".to_string()"));
                    return Ok(());
                }
                out.push("format!(");
                self.format_string(out, literal, depth, flow)?;
                out.push(")");
                Ok(())
            }
            _ => Err(anyhow!("not a string literal")),
        }
    }

    /// The literal as a Rust format string, with its holes as arguments.
    fn format_string(
        &self,
        out: &mut Out,
        literal: &str,
        depth: usize,
        flow: Flow<'_>,
    ) -> Result<()> {
        let (format, holes) = interpolation(literal)?;
        out.push(&format!("\"{format}\""));

        for hole in holes {
            let expr = parse_expression(&self.parsed.interner, &hole)
                .map_err(|e| anyhow!("in the interpolated `{{{hole}}}`: {e}"))?;
            out.push(", ");
            self.expr(out, &expr, depth, flow)?;
        }
        Ok(())
    }

    /// One arm's pattern. Every shape is one the language below spells the
    /// same way, so this is a transcription rather than a translation - a bare
    /// name binds here because it binds there, and the rule is drawn once.
    fn match_pattern(
        &self,
        out: &mut Out,
        pattern: &MatchPattern,
        depth: usize,
        flow: Flow<'_>,
    ) -> Result<()> {
        let path = |p: &[Symbol]| {
            p.iter()
                .map(|s| self.text(*s).to_string())
                .collect::<Vec<_>>()
                .join("::")
        };
        let names = |b: &[Symbol]| {
            b.iter()
                .map(|s| self.text(*s).to_string())
                .collect::<Vec<_>>()
                .join(", ")
        };
        match pattern {
            MatchPattern::Wildcard => out.push("_"),
            MatchPattern::Literal(value) => self.expr(out, value, depth, flow)?,
            MatchPattern::Path(p) => out.push(&path(p)),
            MatchPattern::Tuple { path: p, bindings } => {
                out.push(&format!("{}({})", path(p), names(bindings)));
            }
            MatchPattern::Named { path: p, bindings } => {
                out.push(&format!("{} {{ {} }}", path(p), names(bindings)));
            }
        }
        Ok(())
    }

    /// The thing a `.` or a `[` is applied to, parenthesised where it binds
    /// looser than the postfix does.
    ///
    /// A postfix binds tighter than everything except another postfix in both
    /// languages, so `(a as f64).sqrt()` and `(a + b).len()` need their
    /// parentheses back: the parser drops them - a group is not a node, it is
    /// how the tree was written - and without them the emitted Rust means
    /// something else and often still compiles.
    fn postfix_base(&self, out: &mut Out, expr: &Expr, depth: usize, flow: Flow<'_>) -> Result<()> {
        let parenthesise = matches!(
            expr,
            Expr::Binary { .. }
                | Expr::Unary { .. }
                | Expr::Cast { .. }
                | Expr::Range { .. }
                | Expr::If { .. }
                | Expr::Match { .. }
                | Expr::Block(_)
                | Expr::Closure { .. }
                | Expr::TryCatch { .. }
                | Expr::Dsl { .. }
                | Expr::DslFrom { .. }
                | Expr::Asm { .. }
        );

        if parenthesise {
            out.push("(");
        }
        self.expr(out, expr, depth, flow)?;
        if parenthesise {
            out.push(")");
        }
        Ok(())
    }

    /// An operand of an operator, parenthesised only where it binds looser
    /// than the position it stands in.
    fn nested(
        &self,
        out: &mut Out,
        expr: &Expr,
        needs: u8,
        depth: usize,
        flow: Flow<'_>,
    ) -> Result<()> {
        let parenthesise = match expr {
            Expr::Binary { op, .. } => precedence(*op) < needs,
            Expr::Cast { .. } => needs == u8::MAX,
            _ => false,
        };

        if parenthesise {
            out.push("(");
        }
        self.expr(out, expr, depth, flow)?;
        if parenthesise {
            out.push(")");
        }
        Ok(())
    }

    fn args(&self, out: &mut Out, args: &[Expr], depth: usize, flow: Flow<'_>) -> Result<()> {
        for (i, arg) in args.iter().enumerate() {
            if i > 0 {
                out.push(", ");
            }
            self.expr(out, arg, depth, flow)?;
        }
        Ok(())
    }

    /// `dsl Measurements from data` - the whole of what a user writes to run a
    /// grammar. Everything the parallel form needs is already in the grammar
    /// (ADR-009): the frame says where the input may be cut, the `par_fold`
    /// says how the pieces combine. What is left is choosing the executor, and
    /// that is the build's decision, not the program's.
    fn dsl_from(
        &self,
        out: &mut Out,
        grammar: Symbol,
        input: &Expr,
        depth: usize,
        flow: Flow<'_>,
        propagate: Propagate,
    ) -> Result<()> {
        let question = match propagate {
            Propagate::Yes => "?",
            Propagate::No => "",
        };
        let name = self.text(grammar);
        let def = self
            .grammars
            .get(&grammar)
            .ok_or_else(|| anyhow!("no grammar named `{name}` in this file"))?;

        let rule = entry_rule(def)
            .ok_or_else(|| anyhow!("grammar `{name}` has no `pub` rule to enter through"))?;
        let rule_name = self.text(rule.name);

        let pad = "    ".repeat(depth + 1);
        let close = "    ".repeat(depth);

        // The input is bound before it is parsed, because it is needed twice:
        // once to parse and once to say *where* a failure was. A `ParseError`
        // knows the offset and not the text, so only `render` can turn "at
        // 1042" into "at line 37, column 9" - and a program that reports a
        // rejected file without saying which line is not much better than one
        // that panics. `&*` because the parser takes the text: a mapping, an
        // owned string and a view all reach it the same way, and none of them
        // has to be named here.
        if par_fold_of(rule).is_some() {
            out.push(&format!("{{\n{pad}let _source = &*"));
            self.expr(out, input, depth, flow)?;
            out.push(&format!(
                ";\n\
                 {pad}{name}::parse_{rule_name}_pieces(_source, \
                 &ParseContext::<()>::default(), {})\n\
                 {pad}    .map_err(|error| error.render(_source))\n\
                 {close}}}{question}",
                self.build.parallelism()
            ));
            return Ok(());
        }

        // A sequential entry rule: no pieces to cut, so the parser is driven
        // over the whole input once.
        out.push(&format!(
            "{{\n{pad}use winnow::Parser;\n{pad}let _source = &*"
        ));
        self.expr(out, input, depth, flow)?;
        out.push(&format!(
            ";\n\
             {pad}let mut stream = winnow_grammar::ParseInput::<()> {{\n\
             {pad}    state: winnow_grammar::ParseContext::<()>::default(),\n\
             {pad}    input: winnow::stream::LocatingSlice::new(_source),\n\
             {pad}}};\n\
             {pad}{name}::parse_{rule_name}()\n\
             {pad}    .parse_next(&mut stream)\n\
             {pad}    .map_err(|error| error.render(_source))\n\
             {close}}}{question}"
        ));
        Ok(())
    }
}

// --- Free helpers ---

/// ADR-009 D1: the attribute is keyed in both languages, and the lowering is
/// one-to-one. A bare `@frame` stays bare - the boundary is then the rule's
/// trailing literal, which the backend infers and checks.
fn frame_attribute(frame: &FrameAttr) -> String {
    let mut keys = Vec::new();
    if let Some(boundary) = &frame.boundary {
        keys.push(format!("boundary = \"{boundary}\""));
    }
    if frame.unchecked {
        keys.push("unchecked".to_string());
    }

    if keys.is_empty() {
        "#[frame]".to_string()
    } else {
        format!("#[frame({})]", keys.join(", "))
    }
}

fn repeat_suffix(rep: Repeat) -> String {
    match rep {
        Repeat::Star => "*".to_string(),
        Repeat::Plus => "+".to_string(),
        Repeat::Optional => "?".to_string(),
        Repeat::Exactly(n) => format!("{{{n}}}"),
        Repeat::AtLeast(n) => format!("{{{n},}}"),
        Repeat::Between(n, m) => format!("{{{n},{m}}}"),
    }
}

fn unary_op(op: UnaryOp) -> &'static str {
    match op {
        UnaryOp::Neg => "-",
        UnaryOp::Not => "!",
        UnaryOp::Ref => "&",
    }
}

/// Rust's binding strength, which Nikaia shares.
fn precedence(op: BinaryOp) -> u8 {
    match op {
        BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem => 10,
        BinaryOp::Add | BinaryOp::Sub => 9,
        BinaryOp::Eq | BinaryOp::Ne | BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => {
            7
        }
        BinaryOp::And => 4,
        BinaryOp::Or => 3,
    }
}

fn binary_op(op: BinaryOp) -> &'static str {
    match op {
        BinaryOp::Add => "+",
        BinaryOp::Sub => "-",
        BinaryOp::Mul => "*",
        BinaryOp::Div => "/",
        BinaryOp::Rem => "%",
        BinaryOp::Eq => "==",
        BinaryOp::Ne => "!=",
        BinaryOp::Lt => "<",
        BinaryOp::Le => "<=",
        BinaryOp::Gt => ">",
        BinaryOp::Ge => ">=",
        BinaryOp::And => "&&",
        BinaryOp::Or => "||",
    }
}

/// The rule a `dsl … from …` enters through: the first public one.
fn entry_rule(def: &GrammarDef) -> Option<&GrammarRule> {
    def.rules
        .iter()
        .find(|r| r.is_public && par_fold_of(r).is_some())
        .or_else(|| def.rules.iter().find(|r| r.is_public))
}

/// The `par_fold` a rule *is*, if it is one. A fold with a merge is the only
/// body the backend generates a piece driver for.
fn par_fold_of(rule: &GrammarRule) -> Option<&FoldSpec> {
    rule.alts.iter().find_map(|alt| match &alt.pattern.node {
        Pattern::Fold(spec) if spec.parallel => Some(&**spec),
        Pattern::Bind { pat, .. } => match &pat.node {
            Pattern::Fold(spec) if spec.parallel => Some(&**spec),
            _ => None,
        },
        _ => None,
    })
}

/// Which structs hold a view into the parser's input.
///
/// Part II, 10.6: a view is a slice of the input, so a struct holding one is
/// tied to the input as well - transitively, which is why this is a fixpoint
/// and not one pass.
pub(crate) fn borrowing_structs(parsed: &Parsed) -> HashSet<Symbol> {
    let mut fields_of: HashMap<Symbol, Vec<&Type>> = HashMap::new();
    let mut borrowing = HashSet::new();

    for item in &parsed.program.items {
        // An enum holds views the same way a struct does - in the types of
        // what its variants carry - so it is walked the same way.
        if let Item::Enum { name, variants, .. } = &item.node {
            let types: Vec<&Type> = variants
                .iter()
                .flat_map(|v| match &v.fields {
                    VariantFields::Unit => Vec::new(),
                    VariantFields::Tuple(types) => types.iter().collect(),
                    VariantFields::Named(fields) => fields.iter().map(|f| &f.ty).collect(),
                })
                .collect();
            if types.iter().any(|t| holds_view(t)) {
                borrowing.insert(*name);
            }
            fields_of.insert(*name, types);
        }
        if let Item::Struct { name, fields, .. } = &item.node {
            let types: Vec<&Type> = fields.iter().map(|f| &f.ty).collect();
            if types.iter().any(|t| holds_view(t)) {
                borrowing.insert(*name);
            }
            fields_of.insert(*name, types);
        }
    }

    loop {
        let mut changed = false;
        for (name, types) in &fields_of {
            if borrowing.contains(name) {
                continue;
            }
            if types.iter().any(|t| names_borrowing(t, &borrowing)) {
                borrowing.insert(*name);
                changed = true;
            }
        }
        if !changed {
            return borrowing;
        }
    }
}

pub(crate) fn holds_view(ty: &Type) -> bool {
    ty.is_view || ty.generics.iter().any(holds_view)
}

pub(crate) fn names_borrowing(ty: &Type, borrowing: &HashSet<Symbol>) -> bool {
    borrowing.contains(&ty.name) || ty.generics.iter().any(|g| names_borrowing(g, borrowing))
}

/// A Rust string literal for text the template wrote itself.
///
/// The text is markup by definition - the template author typed it - so nothing
/// is escaped here; what is quoted is what Rust needs quoted to read the same
/// bytes back.
fn rust_string(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Walk every expression in a block, including the ones inside statements.
fn visit_block(block: &Block, f: &mut impl FnMut(&Expr)) {
    for stmt in &block.stmts {
        match &stmt.node {
            Stmt::Let { value, .. } => visit_expr(value, f),
            Stmt::Assign { target, value, .. } => {
                visit_expr(target, f);
                visit_expr(value, f);
            }
            Stmt::For { iter, body, .. } => {
                visit_expr(iter, f);
                visit_block(body, f);
            }
            Stmt::While { cond, body } => {
                visit_expr(cond, f);
                visit_block(body, f);
            }
            Stmt::Return(value) => {
                if let Some(value) = value {
                    visit_expr(value, f);
                }
            }
            Stmt::Expr(expr) => visit_expr(expr, f),
        }
    }
}

fn visit_expr(expr: &Expr, f: &mut impl FnMut(&Expr)) {
    f(expr);
    match expr {
        Expr::Block(block) => visit_block(block, f),
        Expr::If {
            cond,
            then_branch,
            else_branch,
        } => {
            visit_expr(cond, f);
            visit_block(then_branch, f);
            if let Some(block) = else_branch {
                visit_block(block, f);
            }
        }
        Expr::Call { func, args, .. } => {
            visit_expr(func, f);
            args.iter().for_each(|a| visit_expr(a, f));
        }
        Expr::MethodCall { receiver, args, .. } => {
            visit_expr(receiver, f);
            args.iter().for_each(|a| visit_expr(a, f));
        }
        Expr::Match { value, arms } => {
            visit_expr(value, f);
            for arm in arms {
                visit_expr(&arm.body, f);
            }
        }
        Expr::Tuple(parts) => parts.iter().for_each(|p| visit_expr(p, f)),
        Expr::Field { base, .. } => visit_expr(base, f),
        Expr::StructLit { fields, .. } => fields
            .iter()
            .filter_map(|field| field.value.as_ref())
            .for_each(|value| visit_expr(value, f)),
        Expr::Closure { body, .. } => visit_block(body, f),
        Expr::Unary { expr, .. } => visit_expr(expr, f),
        Expr::Binary { lhs, rhs, .. } => {
            visit_expr(lhs, f);
            visit_expr(rhs, f);
        }
        Expr::Try(inner) | Expr::Throw(inner) => visit_expr(inner, f),
        Expr::Index { base, index } => {
            visit_expr(base, f);
            visit_expr(index, f);
        }
        Expr::Cast { expr, .. } => visit_expr(expr, f),
        Expr::Coalesce { value, fallback } => {
            visit_expr(value, f);
            visit_expr(fallback, f);
        }
        Expr::TryCatch { expr, handler } => {
            visit_expr(expr, f);
            visit_block(handler, f);
        }
        Expr::Spawn { body, .. } => visit_expr(body, f),
        Expr::DslFrom { input, .. } => visit_expr(input, f),
        _ => {}
    }
}

/// Split an interpolated string into a Rust format string and its holes.
///
/// `"{a}={s.mean()}"` becomes `("{}={}", ["a", "s.mean()"])`. Braces are
/// doubled to be literal, as in every format string; the holes themselves are
/// Nikaia expressions and are parsed as such by the caller.
/// Every expression a literal holds - the holes of an interpolated string, and
/// the holes of a template.
///
/// **The emitter is not the only thing that needs these.** A hole is Nikaia
/// source, and until this existed it was source no analysis could see: the
/// splitting happened here, on the way out, so the type checker, the `sync`
/// check and the provenance analysis all walked past a `LitStr` as if it were
/// a string.
///
/// That was not only a missed diagnostic. `sync` is *inferred* from what a body
/// calls (ADR-027), so a function whose only pausing call sat inside a hole was
/// recorded `sync = "inferred"` - a claim, in a file a library ships, that it
/// cannot pause. ADR-027 D2 and ADR-010 D1 both say the same thing about that
/// direction: an analysis that fails open is a vulnerability generator.
///
/// A hole that does not parse yields nothing here. The emitter reports it, in
/// its own words, at the place it happens; an analysis has nothing to add.
pub(crate) fn literal_expressions(parsed: &Parsed, expr: &Expr) -> Vec<Expr> {
    match expr {
        Expr::LitInterpolated(literal) => match interpolation(literal) {
            Ok((_, holes)) => holes
                .iter()
                .filter_map(|hole| parse_expression(&parsed.interner, hole).ok())
                .collect(),
            Err(_) => Vec::new(),
        },
        // A template's holes are Nikaia too (ADR-017), and reach the emitter by
        // the same route: text, split on the way out. A deferred-parameter
        // statement has none: its braces are the foreign syntax's, and reading
        // them as holes would be this compiler speaking for a grammar it does
        // not have (ADR-007 D5).
        Expr::Dsl {
            target, content, ..
        } if crate::dsl::is_deferred(parsed.text(*target), content) => Vec::new(),
        Expr::Dsl { content, .. } => match template::split(content.trim()) {
            Ok(segments) => template_holes(&segments)
                .iter()
                .filter_map(|hole| parse_expression(&parsed.interner, hole).ok())
                .collect(),
            Err(_) => Vec::new(),
        },
        _ => Vec::new(),
    }
}

/// Every hole in a template, including the ones inside a `<for>` body - a hole
/// does not become invisible by being repeated, any more than it becomes safe
/// by it (ADR-017 D3).
fn template_holes(segments: &[template::Segment]) -> Vec<String> {
    let mut holes = Vec::new();
    for segment in segments {
        match segment {
            template::Segment::Text(_) => {}
            template::Segment::Hole { expr, .. } => holes.push(expr.clone()),
            template::Segment::For {
                collection, body, ..
            } => {
                // The collection is captured from the enclosing scope with `:`
                // (ADR-007 D4), and it is a name this program wrote.
                holes.push(collection.clone());
                holes.extend(template_holes(body));
            }
        }
    }
    holes
}

/// A plain string on its way into a Rust *format* string.
///
/// Nikaia's plain string is inert - `print("{")` prints a brace (ADR-035 D1) -
/// and Rust's `println!` would read that brace as a hole of its own. Doubling
/// is what says "this one is text" to the macro, and it is the only place the
/// two languages disagree about a string literal.
/// An escape is copied whole, `\u{0041}` included: Rust's lexer turns that into
/// a character before the macro ever sees a brace, so doubling the braces
/// inside one would hand `println!` a `\u` with nothing after it. The same rule
/// `interpolation` follows, for the same reason.
fn rust_format_escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                out.push('\\');
                let Some(escape) = chars.next() else { break };
                out.push(escape);
                if escape == 'u' && chars.peek() == Some(&'{') {
                    for c in chars.by_ref() {
                        out.push(c);
                        if c == '}' {
                            break;
                        }
                    }
                }
            }
            '{' => out.push_str("{{"),
            '}' => out.push_str("}}"),
            _ => out.push(c),
        }
    }
    out
}

pub(crate) fn interpolation(literal: &str) -> Result<(String, Vec<String>)> {
    let mut format = String::new();
    let mut holes = Vec::new();
    let mut chars = literal.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            // An escape is copied whole, and the `{` inside `\u{…}` is part of
            // one. The body arrives here as it was written - the parser keeps a
            // string's escapes rather than decoding them - so a scanner that
            // does not know that reads `"\u{0041}"` as a hole named `0041`.
            '\\' => {
                format.push('\\');
                let Some(escape) = chars.next() else {
                    return Err(anyhow!("string ends in a `\\`: \"{literal}\""));
                };
                format.push(escape);
                if escape == 'u' && chars.peek() == Some(&'{') {
                    for c in chars.by_ref() {
                        format.push(c);
                        if c == '}' {
                            break;
                        }
                    }
                }
            }
            '{' if chars.peek() == Some(&'{') => {
                chars.next();
                format.push_str("{{");
            }
            '}' if chars.peek() == Some(&'}') => {
                chars.next();
                format.push_str("}}");
            }
            '{' => {
                let mut hole = String::new();
                let mut spec: Option<String> = None;
                let mut depth = 1;
                let mut nesting = 0;

                while let Some(c) = chars.next() {
                    // A hole is Nikaia source that was written *inside* a string
                    // literal, so the escaping it carries is that literal's. The
                    // two characters the enclosing string had to escape are the
                    // two undone here - without this, `"{f(\"a\")}"` hands the
                    // parser `f(\"a\")`, which is not an expression.
                    if c == '\\' {
                        match chars.peek() {
                            Some('"') | Some('\\') => {
                                let c = chars.next().expect("peeked");
                                match &mut spec {
                                    Some(spec) => spec.push(c),
                                    None => hole.push(c),
                                }
                                continue;
                            }
                            _ => {}
                        }
                    }
                    match c {
                        '{' => depth += 1,
                        '}' => {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                        }
                        '(' | '[' => nesting += 1,
                        ')' | ']' => nesting -= 1,
                        // `::` is a path, not a format specifier. A hole is
                        // Nikaia source, and Nikaia source names things across
                        // modules (Part I, 9.1) - `"{utils::double(21)}"` was
                        // read as the expression `utils` written with the
                        // specifier `:double(21)`, which is a format string
                        // nobody wrote and a program nobody meant.
                        ':' if chars.peek() == Some(&':') => {
                            let second = chars.next().expect("peeked");
                            match &mut spec {
                                Some(spec) => {
                                    spec.push(c);
                                    spec.push(second);
                                }
                                None => {
                                    hole.push(c);
                                    hole.push(second);
                                }
                            }
                            continue;
                        }
                        // The first colon that is not inside a call or an index
                        // separates the expression from how it is to be
                        // written, exactly as a format string does elsewhere.
                        // `Stats(min: 1)` keeps its colon, being inside parens.
                        ':' if nesting == 0 && spec.is_none() => {
                            spec = Some(String::new());
                            continue;
                        }
                        _ => {}
                    }
                    match &mut spec {
                        Some(spec) => spec.push(c),
                        None => hole.push(c),
                    }
                }
                if depth != 0 {
                    return Err(anyhow!("unclosed `{{` in \"{literal}\""));
                }

                holes.push(hole);
                match spec {
                    Some(spec) => format.push_str(&format!("{{:{spec}}}")),
                    None => format.push_str("{}"),
                }
            }
            '}' => {
                return Err(anyhow!(
                    "stray `}}` in \"{literal}\"; write `}}}}` for a brace"
                ))
            }
            _ => format.push(c),
        }
    }

    Ok((format, holes))
}
