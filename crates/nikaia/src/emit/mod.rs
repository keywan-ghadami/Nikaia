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
//   * `Name.rule(input)`            ->  the generated `par_fold` driver, with
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
    Pattern, Receiver, Repeat, SelectArm, Span, Spanned, Stmt, Type, UnaryOp, VariantFields,
};
use crate::parser::{parse_expression, Parsed};
use crate::refused;

/// One branch of an `overlap { … }`, where the schedule and the written order
/// differ and the results have to be put back (ADR-050 D2, D6).
const BRANCH: &str = "__nikaia_branch_";

/// The name a `select` arm's winning value arrives under, where it has to be
/// unwrapped before the arm's own binding sees it (ADR-148 D1).
const WINNER: &str = "__nikaia_won";

/// What the variants of `std`'s `Race<n>` are called, in written order.
///
/// **Ordinals and not letters**, so the generated `match` reads as the source
/// does: `Race2::Second(_)` is the second arm of the block (Part III C.1).
const ORDINALS: [&str; MOST_BRANCHES] = [
    "First", "Second", "Third", "Fourth", "Fifth", "Sixth", "Seventh", "Eighth",
];

/// How many branches an `overlap` block may have.
///
/// `std` writes one vehicle per arity, because the branches have different types
/// and a tuple of futures is what that means in the language below. Eight is
/// what is written; a block with more is refused **with its reason** rather than
/// miscompiled, which is the only thing a limit owes a reader.
const MOST_BRANCHES: usize = 8;

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

    /// **May two pieces of the program's own code run at the same time?**
    /// (ADR-037 D2.)
    ///
    /// This gates `task::both` and nothing else. `task::both(|| …, || …)` puts
    /// two user closures on two threads, so it is out at
    /// `user_parallelism = no` whatever the analysis says - ADR-033 decides
    /// whether two operations *may* overlap, and this decides whether there is
    /// anything to overlap them with. A target without threads answers no for
    /// the second reason: `rayon::join` does not link on `wasm32-unknown`
    /// (Part III 15.3).
    ///
    /// **Not the same question as [`Build::overlaps_operations`]**, and the
    /// whole of ADR-033 D10 is that difference. "Two pieces of your code at
    /// once" is forbidden at `no`; "two operations in flight at once" is not,
    /// and never was - Part I 8.4's runtime has had a second thread at `no`
    /// since ADR-038 D4, and it is safe precisely because *user code never
    /// runs there*. What `no` forbids is that anything **you** wrote is in
    /// flight twice at once (Part I 1.2), which is why the word in
    /// `user_parallelism` is `user`.
    pub fn overlaps_user_code(self) -> bool {
        self.user_parallelism.is_concurrent() && self.target.has_threads()
    }

    /// **May two operations be in flight at the same time?** (ADR-033 D10.)
    ///
    /// True at **both** settings of `user_parallelism`, because the vehicle it
    /// gates carries no code the program wrote: two reads handed to the
    /// runtime are two operations the kernel performs while the program is
    /// suspended at both of them, and `std` is what waits. Nothing of the
    /// user's is in flight twice, so ADR-037 D2's promise is untouched - and
    /// `nikaia_std::rt` keeps it by construction rather than by convention,
    /// since an I/O worker's inbox takes operations and no variant of one can
    /// carry a closure (ADR-038 §4.2).
    ///
    /// What it asks of the build is therefore only whether `std`'s runtime is
    /// there at all. `user_parallelism` has no say in it; the *target* does.
    pub fn overlaps_operations(self) -> bool {
        self.target.has_runtime()
    }
}

impl Target {
    pub fn parse(name: &str) -> Result<Target> {
        match name {
            "x86_64-linux" => Ok(Target::X86_64Linux),
            "wasm32-unknown" => Ok(Target::Wasm32Unknown),
            other => Err(refused!(
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

    /// Whether `std`'s own runtime exists for this machine (ADR-038 D4): the
    /// I/O worker that is running before `main`, and the completion queue where
    /// the kernel has one.
    ///
    /// The same answer as [`Target::has_threads`] today and a different
    /// question, which is why it is a second method rather than a second
    /// caller. `wasm32-unknown` has neither `io_uring` nor a thread to fall
    /// back to; a machine with threads but no completion queue has the runtime,
    /// and `std` is what decides there what a pair costs.
    pub fn has_runtime(self) -> bool {
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
            other if other.parse::<u32>().is_ok() => Err(refused!(
                "`user-parallelism` is yes or no, not a count: how many threads \
                 serve a `yes` is the runtime's to decide, not the program's"
            )),
            other => Err(refused!(
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
    /// **In the result of a function with nothing to borrow from.**
    ///
    /// `fn name() -> &str { "Ada" }` has no reference among its arguments and no
    /// receiver, so there is no lifetime for Rust's elision to take - and
    /// `-> &str` is *"missing lifetime specifier"* about a file nobody wrote
    /// (Part III, C.1).
    ///
    /// **`'static` is the only lifetime that can be written there**, which is
    /// what makes this a derivation rather than a choice: a view handed back by
    /// a function that borrowed nothing can only point at something that
    /// outlives the program. And it is safe to write even where the body cannot
    /// honour it - `rustc` still checks the body, and refuses the Nikaia line
    /// through [ADR-012](../../../docs/specification/adr/adr-012.md) rather than
    /// the generated one. So this never accepts a wrong program and never
    /// refuses a right one.
    const STATIC: Lifetimes = Lifetimes {
        reference: "&'static ",
        params: "'static",
    };

    /// The same, with the reference named: a view **of the input buffer** and
    /// not of whatever the caller lends for the call.
    ///
    /// One parameter at a time, for the parameters `views::carried` says the
    /// subject's own buffer already covers. Nothing else about the position
    /// changes, which is why this is derived from the position rather than being
    /// a fourth constant.
    const fn of_the_input(self) -> Lifetimes {
        Lifetimes {
            reference: "&'a ",
            params: self.params,
        }
    }
}

// --- The output, and the way back ---

/// What the lowering produces: the Rust, and where each part of it came from.
#[derive(Debug, Clone)]
pub struct Lowered {
    pub rust: String,
    pub map: SourceMap,
}

/// **[ADR-044](../../../docs/specification/adr/adr-044.md) D1's table, as a Rust
/// item to append to a program.**
///
/// One row per line of the generated file that came from a `.nika` line: the
/// generated line, the file, and the line there. Sorted by the generated line,
/// because the lookup at run time is a binary search and sorting once here costs
/// the program nothing.
///
/// **Every mapped line, and not the ones that "can abort".** Which constructs
/// abort is a list that would have to be kept correct as the emitter grows, and
/// getting it wrong means an abort with no Nikaia line - the defect this closes.
/// The source map already knows which lines came from somewhere, so the table is
/// that knowledge kept rather than a second judgement about it. What it costs is
/// a row per line in the binary, which the record accepts: *"paid by every
/// program and needed by the ones that fail, which is the same trade a panic
/// message itself already makes."*
///
/// **Appended, never prepended.** A table written above the program would move
/// every line it names, and correcting for its own height is a circle. Rust does
/// not care where an item sits.
///
/// `paths` are the program's files, indexed the way the map indexes them.
pub fn abort_table(rust: &str, map: &SourceMap, paths: &[String], sources: &[&str]) -> String {
    // Where each line of the generated file starts, so an offset becomes a line
    // in one binary search rather than by counting newlines per entry.
    let mut starts = vec![0usize];
    starts.extend(
        rust.char_indices()
            .filter(|(_, c)| *c == '\n')
            .map(|(i, _)| i + 1),
    );
    let line_of = |offset: usize| match starts.binary_search(&offset) {
        Ok(i) => i + 1,
        Err(i) => i,
    };

    // The `.nika` line of a span, per file.
    let nika: Vec<Vec<usize>> = sources
        .iter()
        .map(|source| {
            let mut starts = vec![0usize];
            starts.extend(
                source
                    .char_indices()
                    .filter(|(_, c)| *c == '\n')
                    .map(|(i, _)| i + 1),
            );
            starts
        })
        .collect();

    // **The outermost entry wins**, which is the opposite of what a lookup
    // wants. `SourceMap::locate` answers with the innermost span covering an
    // offset, and that is right for a diagnostic about a byte; a whole generated
    // *line* is better named by the statement it came from than by the
    // sub-expression that happens to start it. A `BTreeMap` keyed by the
    // generated line and filled in map order gives that: the emitter records a
    // node before anything it encloses, so the first entry for a line is the
    // outermost one.
    let mut rows: std::collections::BTreeMap<usize, (usize, usize)> =
        std::collections::BTreeMap::new();
    for entry in map.rows() {
        let generated = line_of(entry.0);
        let Some(starts) = nika.get(entry.2) else {
            continue;
        };
        let line = match starts.binary_search(&entry.1) {
            Ok(i) => i + 1,
            Err(i) => i,
        };
        rows.entry(generated).or_insert((entry.2, line));
    }

    let mut out = String::new();
    out.push_str("\n// ADR-044 D1: the generated line, and the `.nika` line it came from.\n");
    out.push_str(&format!(
        "const {ABORT_TABLE}: &[nikaia_std::abort::Site] = &[\n"
    ));
    for (generated, (unit, line)) in rows {
        let path = paths.get(unit).map(String::as_str).unwrap_or("<unknown>");
        out.push_str(&format!("    ({generated}, {path:?}, {line}),\n"));
    }
    out.push_str("];\n");
    out
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

    /// Every entry as `(generated start, source start, unit)`, in the order the
    /// emitter recorded them - which is outermost first for any one place.
    ///
    /// For [`abort_table`], which needs all of them rather than a lookup.
    pub fn rows(&self) -> impl Iterator<Item = (usize, usize, usize)> + '_ {
        self.entries
            .iter()
            .map(|e| (e.generated.start, e.source.start, e.unit))
    }
}

/// The emitted text under construction, and the map being built with it.
#[derive(Debug, Default)]
struct Out {
    buf: String,
    map: SourceMap,
}

/// The name a `?.m()`'s reached value is bound to inside the `match`.
///
/// Prefixed like every other name this emitter invents, so it can never be one
/// a program wrote: `__nikaia_` is reserved by the same rule that reserves the
/// branch names an `overlap` uses.
const REACHED: &str = "__nikaia_it";

/// Kap 7.1: the name a `catch` handler sees the failure under.
///
/// One place, because the emitter writes it and
/// [`contracts::order::block_mentions`](crate::contracts::order::block_mentions)
/// is asked about it — and a second spelling of one name is a binding that goes
/// unread while the walk looks for the other
/// ([ADR-090](../../docs/specification/adr/adr-090.md)).
const CAUGHT: &str = "error";

/// Where the local holding a caught error's **site** is kept
/// ([ADR-157](../../docs/specification/adr/adr-157.md) D2).
///
/// A named channel's envelope is opened at the handler's binding, so the error
/// the source names is the author's own value — and `throw error` needs the
/// site back to pass the error on with the place it was **raised** rather than
/// the place it was caught ([ADR-023](../../docs/specification/adr/adr-023.md)
/// D6). Spelled so that no Nikaia name collides with it.
const SITE: &str = "__nikaia_site";

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
    let trust = crate::contracts::trust::analyse(parsed, &std_ledger());
    Emitter::new(parsed, build, trust.provenance).program()
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
    Emitter::new(parsed, build, provenance).program()
}

/// **`std`'s own Nikaia half**, which is the one program that must not import
/// the prelude: it *is* the prelude ([ADR-014](../../../docs/specification/adr/adr-014.md)
/// D1, [ADR-002](../../../docs/specification/adr/adr-002.md) D4).
///
/// Every other program gets `pub use nikaia_std::prelude::*;` whether or not it
/// asked, because what a program uses is not a list this emitter keeps and a name
/// the prelude provides has to resolve. `std` is where that stops being true, and
/// it is one program rather than a class - so it says so here rather than being
/// detected.
pub fn emit_std(parsed: &Parsed) -> Result<Lowered> {
    let build = Build::default();
    let trust = crate::contracts::trust::analyse(parsed, &std_ledger());
    Emitter::new(parsed, build, trust.provenance)
        .for_std()
        .program()
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
    emit_module_body_at(parsed, build, provenance, contracts, false)
}

/// The same, saying whether these items are the crate root's.
///
/// Only the crate root may carry the `fn main` Rust runs, and ADR-038 D4 makes
/// that one generated function rather than the program's own - so a module is
/// emitted with `false` and a `main` in it stays as written.
pub fn emit_module_body_at(
    parsed: &Parsed,
    build: Build,
    provenance: crate::contracts::Provenance,
    contracts: &crate::contracts::Ledger,
    entry: bool,
) -> Result<Lowered> {
    Emitter::with_contracts(parsed, build, provenance, contracts.clone())
        .for_entry(entry)
        .items_only()
}

/// What a program's preamble has to say, over all of its files.
///
/// `uses_std` and the rest are per-file facts, and the preamble is written
/// once - so they are joined here rather than guessed at from the entry.
#[derive(Debug, Clone, Default)]
pub struct Needs {
    pub grammar: bool,
    pub driver: bool,
    pub std: bool,
    /// Kap 7.1: a function here declares `throws`, so the error surface is
    /// reachable and `std`'s is what carries it.
    pub fails: bool,
    /// Kap 4.7 across a package: every trait an `impl` in this unit names with
    /// a package in front of it
    /// ([ADR-095](../../docs/specification/adr/adr-095.md)).
    ///
    /// Rust needs a trait **in scope** before its methods can be called, and
    /// `impl http::Handler for Fixed` does not put it there. So a trait a
    /// package publishes could be implemented and then not called, in the
    /// backend's words about the generated file: *"trait `Handler` … is
    /// implemented but not in scope; perhaps you want to import it"* — a rule
    /// the program has no way to satisfy, because Nikaia has no import to
    /// write ([ADR-046](../../docs/specification/adr/adr-046.md) D2 brings no
    /// names in).
    ///
    /// The **same shape in one file compiles**, which is what says this is one
    /// missing emitted line rather than a question: there the trait is in the
    /// same module and needs no import.
    pub foreign_traits: std::collections::BTreeSet<String>,
}

impl Needs {
    pub fn of(parsed: &Parsed, build: Build) -> Needs {
        let emitter = Emitter::new(parsed, build, crate::contracts::Provenance::Trusted);
        Needs {
            grammar: parsed
                .program
                .items
                .iter()
                .any(|i| matches!(i.node, Item::Grammar(_))),
            driver: emitter.uses_driver(),
            std: emitter.uses_std,
            fails: emitter.fails,
            foreign_traits: foreign_traits(parsed),
        }
    }

    pub fn join(mut self, other: Needs) -> Needs {
        self.foreign_traits.extend(other.foreign_traits);
        Needs {
            grammar: self.grammar || other.grammar,
            driver: self.driver || other.driver,
            std: self.std || other.std,
            fails: self.fails || other.fails,
            foreign_traits: self.foreign_traits,
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
        // **Always, and allowed to be unused.** It used to be written only where
        // the program had a `use std::…` line of its own, which is not the same
        // question: `HashMap` is a name the prelude provides and a program may
        // write it without importing anything, and such a program lowered to a
        // file where `TrustedMap` was undeclared - `rustc` about a file nobody
        // wrote (Part III, C.1). What a program uses is not a list this emitter
        // keeps, and it does not need one: the import is one line, a locally
        // declared name shadows a glob, and `#[allow]` is what keeps an unused
        // one from reaching a reader (the same move the emitted `use super::*`
        // made).
        // Nikaia's `std` is a crate rather than a table in this file, so
        // `fs::map`, `cli::args` and `HashMap` resolve as written and what
        // they mean is code someone can read.
        // `pub use`, because the grammar module the backend generates
        // reaches these names through a glob of its own, and a private
        // import is not re-exported into one.
        out.push_str("#[allow(unused_imports)]\npub use nikaia_std::prelude::*;\n");
        if self.fails {
            // Kap 7.1: `error.full()` is a method on whatever a `catch` bound, so
            // the trait has to be in scope even in a program that imports no `std`
            // module of its own - `throw` needs `std` whether or not the source
            // mentions it.
            out.push_str("#[allow(unused_imports)]\npub use nikaia_std::error::Full;\n");
        }
        // **A trait reached across a package is imported**
        // ([ADR-095](../../docs/specification/adr/adr-095.md)). Written as the
        // path the program already wrote, because that is the path the
        // generated crate resolves: `impl http::Handler for Fixed` names the
        // crate `http`, and so does this.
        //
        // `allow(unused_imports)` for the same reason the two lines above carry
        // it - a program may implement a trait and never call the method, and a
        // warning about a line nobody wrote is
        // [Part III C.1](../../docs/specification/30-nikaia-tooling.md) one
        // severity down.
        for path in &self.foreign_traits {
            out.push_str(&format!("#[allow(unused_imports)]\npub use {path};\n"));
        }
        out
    }
}

/// Every trait an `impl` in this unit names with a package in front of it.
///
/// **Qualified only**, and that is the whole of the test: a trait of this
/// program's own is in the same module below and needs no import, while a
/// `::` in the name is exactly what says the declaration is in another crate.
/// A trait the compiler reads rather than a `.nika` file wrote - `impl Error
/// for ConfigError` ([ADR-023](../../docs/specification/adr/adr-023.md) D3) -
/// carries no `::` either, so it is left alone by the same test.
/// What each `extern "C"` declaration in this file takes
/// ([ADR-147](../../docs/specification/adr/adr-147.md) D1), by the name a call
/// writes.
fn foreign_params(parsed: &Parsed) -> std::collections::BTreeMap<String, Vec<Type>> {
    let mut out = std::collections::BTreeMap::new();
    for item in &parsed.program.items {
        let Item::Extern { declarations, .. } = &item.node else {
            continue;
        };
        for declaration in declarations {
            out.insert(
                parsed.text(declaration.node.name).to_string(),
                declaration
                    .node
                    .args
                    .iter()
                    .map(|a| a.ty.clone())
                    .collect::<Vec<_>>(),
            );
        }
    }
    out
}

/// The opaque handles this file declares, by name, with the function that ends
/// each one's life ([ADR-147](../../docs/specification/adr/adr-147.md) D3).
fn opaque_handles(parsed: &Parsed) -> std::collections::BTreeMap<String, String> {
    let mut out = std::collections::BTreeMap::new();
    for item in &parsed.program.items {
        let Item::Extern { opaque, .. } = &item.node else {
            continue;
        };
        for handle in opaque {
            out.insert(
                parsed.text(handle.node.name).to_string(),
                parsed.text(handle.node.released_by).to_string(),
            );
        }
    }
    out
}

/// What each `extern "C"` declaration hands back, by the name a call writes
/// ([ADR-155](../../docs/specification/adr/adr-155.md) D3).
fn foreign_results(parsed: &Parsed) -> std::collections::BTreeMap<String, Type> {
    let mut out = std::collections::BTreeMap::new();
    for item in &parsed.program.items {
        let Item::Extern { declarations, .. } = &item.node else {
            continue;
        };
        for declaration in declarations {
            if let Some(ret) = &declaration.node.ret_type {
                out.insert(parsed.text(declaration.node.name).to_string(), ret.clone());
            }
        }
    }
    out
}

fn foreign_traits(parsed: &Parsed) -> std::collections::BTreeSet<String> {
    parsed
        .program
        .items
        .iter()
        .filter_map(|item| match &item.node {
            Item::Impl {
                trait_name: Some(name),
                ..
            } => Some(parsed.text(*name).to_string()),
            _ => None,
        })
        .filter(|name| name.contains("::"))
        .collect()
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

/// The words the **language below** reserves and this one does not.
///
/// Measured rather than copied out of a reference, and measured **twice**: the
/// first sweep read `rustc`'s *"found keyword"* at a `let` and came back with
/// twenty-four, missing `crate`, `super` and `box` because those three fail
/// there with a different message each (`E0532`, `E0433`, *"expected pattern"*).
/// The sweep in `tests/reserved_below.rs` compiles every word in every position
/// instead, which is what found them.
///
/// `gen` and `union` are deliberately **absent**: Rust takes those as
/// identifiers, so escaping them would be a change nothing asked for.
///
/// `crate`, `super`, `self` and `Self` are absent for the opposite reason —
/// **Rust forbids a raw identifier for exactly those four**, so there is no
/// escape to write. They are refused instead (`NK1128`, and `NK1119` for
/// `self`), which is [ADR-076](../../../docs/specification/adr/adr-076.md) D3.
const RESERVED_BELOW: &[&str] = &[
    "abstract", "async", "await", "become", "box", "const", "do", "dyn", "extern", "final", "loop",
    "macro", "mod", "move", "override", "priv", "ref", "static", "trait", "try", "type", "typeof",
    "unsafe", "unsized", "virtual", "where", "yield",
];

/// A source name, written so the language below can read it.
///
/// **This is [ADR-011](../../../docs/specification/adr/adr-011.md) D2 paying one
/// of its bills.** The emitter writes a name for a name and resolves nothing, so
/// a Nikaia name that Rust happens to spell as a keyword went out verbatim and
/// `rustc` answered *"expected identifier, found keyword `type`"* about a file
/// nobody wrote, with *"escape `type` to use it as an identifier"* as the help —
/// advice that means nothing in this language
/// ([Part III C.1](../../../docs/specification/30-nikaia-tooling.md)).
///
/// **Escaped rather than reserved**, which is
/// [ADR-076](../../../docs/specification/adr/adr-076.md) D1: reserving these
/// words in Nikaia would let the backend decide what this language's vocabulary
/// is, and `type` is the field name of every tagged record anybody has ever
/// written.
///
/// **`const` and `loop` joined the list when they left Nikaia's own**
/// ([ADR-117](../../../docs/specification/adr/adr-117.md) D1), which is that
/// record's *answered as it is for `type`*: they were not here before because a
/// Nikaia name could not be one of them, and now it can. `macro` was already
/// here, and `quote` is a keyword in neither language. The three the backend
/// cannot escape at all — `crate`, `super` and `Self` — are `NK1128`'s and not
/// this list's.
///
/// Borrowed where nothing changes, which is every name in every program written
/// today.
fn escaped(name: &str) -> std::borrow::Cow<'_, str> {
    match RESERVED_BELOW.contains(&name) {
        true => std::borrow::Cow::Owned(format!("r#{name}")),
        false => std::borrow::Cow::Borrowed(name),
    }
}

/// `<'a, T>`, or nothing at all where there is nothing to declare.
///
/// One place, because three positions write one - a `fn`, a `struct` and the
/// `impl` over it - and a list that is empty has to be written as the empty
/// string rather than as `<>`, which is not Rust.
fn angled(parts: &[String]) -> String {
    match parts.is_empty() {
        true => String::new(),
        false => format!("<{}>", parts.join(", ")),
    }
}

struct Emitter<'p> {
    parsed: &'p Parsed,
    build: Build,
    /// Structs that hold a view into the input, and so need the input lifetime
    /// wherever they are named.
    ///
    /// Read through [`Emitter::borrows`] and not directly: this half is what
    /// *this file* declares, and a package is several files (Part I, 9.1).
    borrowing: HashSet<Symbol>,
    /// The other half: the types the **package's** ledger records a `tethered`
    /// field for (`contracts::TypeContract`).
    ///
    /// `examples/inventory` is why. `Entry` is declared in `stock.nika` and
    /// named in `page.nika`, and each file is emitted from its own `Parsed`
    /// (`emit_module_body_ordered`) - so the syntactic set above is empty in the
    /// file that writes `Vec[Entry]`, and the lifetime went unwritten there.
    /// Rust's elision covered it in a plain `fn` and stops covering it in an
    /// `async fn` (E0726), which is how a latent defect became a message about
    /// the generated file (Part III, C.1).
    ///
    /// The ledger is the right place to read it from because a package has
    /// exactly one (Part III, 13.5) and its keys for the package's own types are
    /// unqualified - the same name the source writes.
    tethered: std::collections::BTreeSet<String>,
    /// The view parameters written as views of the *input* buffer rather than of
    /// whatever the caller lends for the call, by the byte their method starts
    /// at (`views::carried`).
    ///
    /// A parameter written `&str` says only "a view, for this call". Where the
    /// method stores it into its subject, and the subject already carries the
    /// input buffer, the view it stores is a view of **that** buffer - so the
    /// parameter is written with the input lifetime and the signature says what
    /// the body does. Where nothing names the buffer, `views::check` refuses the
    /// program instead (`NK2302`), so this set is never a guess.
    carries_input: HashMap<usize, HashSet<Symbol>>,
    grammars: HashMap<Symbol, &'p GrammarDef>,
    /// Declared struct names: `Stats(x)` is a call to a constructor, and only
    /// the declarations say which names are types.
    structs: HashSet<Symbol>,
    /// The `for` statements whose step can fail, by the byte they start at
    /// (ADR-025 D1).
    fallible_loops: std::collections::BTreeSet<usize>,
    /// The `let`s whose place-initialiser has to be lent
    /// ([ADR-094](../../docs/specification/adr/adr-094.md) D4), by the byte the
    /// statement starts at. Answered by the checker for the reason every set
    /// beside it is: it is a question about the type, and this has none
    /// (ADR-028).
    lent_lets: std::collections::BTreeSet<usize>,
    /// The list literals that are an **array**
    /// ([ADR-152](../../docs/specification/adr/adr-152.md) D4), by the byte the
    /// `[` stands at. `vec![…]` for one that is not in here and `[…]` for one
    /// that is; the checker decides, because the decision is a type's
    /// (ADR-028).
    array_literals: std::collections::BTreeSet<usize>,
    /// The method calls that can fail, by the byte their statement starts at
    /// and the method's name (ADR-023 D8).
    ///
    /// Handed over exactly as `fallible_loops` is, and for the reason stated
    /// there: `stats.add(5)` names `add` and says nothing about what `stats`
    /// is, so only the type checker can say what it calls (ADR-028). Nothing
    /// here resolves a receiver.
    fallible_methods: std::collections::BTreeSet<(usize, String)>,
    /// The `set` calls that carry a **witness**
    /// ([ADR-111](../../../docs/specification/adr/adr-111.md) D5), by the byte
    /// their statement starts at (`check::Checked::witnessed_sets`).
    ///
    /// `kasse.set(neu; after: stand)` lowers to `set_after(neu, stand)`, which
    /// compares and stores while the lock is open once. Whether the receiver is
    /// a lock at all is the type checker's answer and not this file's (ADR-028),
    /// which is why it arrives rather than being worked out — a user-defined
    /// `set` with an `after:` option of its own is left alone.
    witnessed_sets: std::collections::BTreeSet<usize>,
    /// The lambda arguments written as a closure returning a **boxed future**
    /// ([ADR-122](../../../docs/specification/adr/adr-122.md) D1), by the byte
    /// their statement starts at and the argument's position
    /// (`check::Checked::future_lambdas`).
    ///
    /// Which shape a parameter takes is a question about its **type**, and this
    /// file has none ([ADR-028](../../../docs/specification/adr/adr-028.md)) —
    /// the same arrangement `lent_args` and `nullable_args` arrive by.
    future_lambdas: std::collections::BTreeSet<(usize, usize)>,
    /// The method calls that **pause**, by the byte their statement starts at
    /// and the name written (`check::Checked::pausing_methods`).
    ///
    /// The same arrangement, one ledger column over: `throws` becomes a `?` and
    /// *can pause* becomes an `.await`. The emitter cannot ask this one itself
    /// for a method, because only a type checker knows what `stats.add(5)` goes
    /// to (ADR-028).
    pausing_methods: std::collections::BTreeSet<(usize, String)>,
    /// The conversions that **narrow**, by the byte their statement starts at
    /// and the type converted to
    /// ([ADR-043](../../../docs/specification/adr/adr-043.md) D4).
    ///
    /// Handed over for the reason the two above are: `as i32` narrows or widens
    /// depending on what it is *given*, and nothing here knows that (ADR-028).
    /// The emitter writes the checked conversion and never decides which one it
    /// is.
    narrowing_casts: std::collections::BTreeMap<(usize, String), crate::check::Narrowing>,
    /// Which reference count each `Shared` value got, by the slot key
    /// `function::value` ([ADR-037](../../../docs/specification/adr/adr-037.md)
    /// D7).
    ///
    /// **Per value and not per type**, which is why `map_name` cannot answer it:
    /// `Shared[T]` is one atomic count at both settings of `user_parallelism`
    /// (D6), and a value the analysis proves never crosses a thread gets a plain
    /// one instead. That is a question about where a particular handle goes, so
    /// it is computed by `contracts::sharing` over the whole unit and looked up
    /// here - the same arrangement `fallible_loops` and `fallible_methods` have,
    /// and for the same reason.
    ///
    /// A count belongs to the **allocation**, so every position of one
    /// union-find class carries the same answer and the types a call writes on
    /// both sides of it agree by construction. A slot this map does not have gets
    /// the atomic floor.
    shared: std::collections::BTreeMap<String, crate::contracts::sharing::Count>,
    /// Part I 2.3: the statements where a plain value stands in a nullable slot,
    /// and **which** of the two wraps this emitter writes there
    /// (`check::Checked::nullable_sites`,
    /// [ADR-068](../../../docs/specification/adr/adr-068.md)).
    nullable_sites: std::collections::BTreeMap<usize, crate::check::Wrap>,
    /// The `+`s that join text, by the byte their operator starts at
    /// ([ADR-081](../../docs/specification/adr/adr-081.md) D2).
    ///
    /// The one answer in this channel keyed by an **expression** rather than by
    /// a statement, because `a + b + c` is two of them and the statement they
    /// stand in is one.
    concatenations: std::collections::BTreeSet<usize>,
    /// What each `comptime` is written as below - its type and its value - by the
    /// byte its statement starts at (`check::Checked::comptime_values`,
    /// [ADR-073](../../docs/specification/adr/adr-073.md) D3, D4).
    ///
    /// Here for the reason every table beside it is: this has neither types nor
    /// a scope ([ADR-028](../../docs/specification/adr/adr-028.md)), and a
    /// `const` needs both - a type because Rust's `const` takes one, and a
    /// scope because `const PAIR = PAGE * 2` folds only for something that
    /// knows what `PAGE` is. A statement with no entry never arrives, because
    /// the checker refused it as `NK1127` first.
    comptime_values: std::collections::BTreeMap<usize, (String, String)>,
    /// Part I 3.5: the `?.` reaches whose field is itself nullable and which
    /// therefore flatten (`check::Checked::flattened_reaches`).
    flattened_reaches: std::collections::BTreeSet<(usize, String)>,
    /// Part I 2.3: the struct-literal fields where a plain value stands in a
    /// nullable slot (`check::Checked::nullable_fields`).
    nullable_fields: std::collections::BTreeMap<
        (usize, String, String),
        std::collections::BTreeMap<String, crate::check::Wrap>,
    >,
    /// **The arguments the compiler writes a `&` for**
    /// ([ADR-094](../../docs/specification/adr/adr-094.md) D1), keyed exactly
    /// as `nullable_args` is and for the same reason: a statement may call one
    /// function twice.
    ///
    /// The *declaration* side of this answer is read straight from the ledger
    /// (`contracts::keeps::lends`), because that half is a question about the
    /// callee alone. This half needed the checker, because it also asks whether
    /// this argument is already a view.
    lent_args:
        std::collections::BTreeMap<(usize, String, usize), std::collections::BTreeSet<String>>,
    /// **The arguments the compiler writes a `&mut` for**
    /// ([ADR-094](../../docs/specification/adr/adr-094.md) D3), keyed as
    /// `lent_args` is. The third state, and the one that is a declaration
    /// rather than an inference.
    mut_args:
        std::collections::BTreeMap<(usize, String, usize), std::collections::BTreeSet<String>>,
    /// Part I 2.3: the call arguments where a plain value stands in a nullable
    /// parameter, by statement, callee as written, and position
    /// (`check::Checked::nullable_args`).
    nullable_args: std::collections::BTreeMap<
        (usize, String, usize),
        std::collections::BTreeMap<String, crate::check::Wrap>,
    >,
    /// **The options a method call has**, in the declaration's order, by the
    /// byte the statement starts at and the method's written name
    /// (`check::Checked::method_options`).
    ///
    /// A call **by name** reads its options off the callee's contract here
    /// (`options_of`); a method cannot, because finding the entry means
    /// resolving the receiver and this has no types
    /// ([ADR-028](../../../docs/specification/adr/adr-028.md)).
    method_options: std::collections::BTreeMap<(usize, String), Vec<(String, String)>>,
    /// The handles a task's body uses, by the byte the statement starts at and
    /// the name (`check::Checked::task_handles`).
    ///
    /// A task takes what it names **by value**, and a handle handed on by value
    /// is duplicated ([ADR-040](../../../docs/specification/adr/adr-040.md) D1).
    /// For a call the callee's signature answers it; a task's body has no
    /// signature, so the checker answers it and this writes the step.
    task_handles: std::collections::BTreeSet<(usize, String)>,
    /// ADR-055 D6: what each pausing function can reach, over the pausing ones
    /// ([`pausing_reach`]).
    ///
    /// A recursive `async fn` is an infinitely sized future, so a call that
    /// closes a cycle has to put one behind a pointer - and *which* calls those
    /// are is this map's question: a call from `f` to `g` closes one exactly
    /// where `g` reaches `f` again. Read through
    /// [`Emitter::closes_a_pausing_cycle`].
    pausing_reach: std::collections::BTreeMap<String, std::collections::BTreeSet<String>>,
    /// This unit's own contracts, and `std`'s. A call's options come from the
    /// declaration, and a declaration is what a ledger records (Kap 5.1).
    own_contracts: crate::contracts::Ledger,
    library: crate::contracts::Ledger,
    /// The types **this unit declares and gives an `impl Error`**
    /// ([ADR-157](../../docs/specification/adr/adr-157.md) D1).
    ///
    /// What is thrown implements `Error` and the `impl` line says so (Part I
    /// 7.1, `NK1161`), so this is the set a failure channel may be named after:
    /// a name here resolves in the generated file, and one another package
    /// declares does not.
    declared_errors: std::collections::BTreeSet<String>,
    /// ADR-033: whether two statements that meet on nothing may overlap.
    /// What the program's `impl` blocks declare, which is what makes the fold
    /// adapter of D2 a lookup rather than a guess.
    methods: HashMap<(Symbol, Symbol), Method>,
    /// The same, by method name alone, for the places where the receiver's type
    /// is not written down. `None` marks a name two impls disagree about - and
    /// an ambiguous name is left alone rather than resolved by preference.
    by_name: HashMap<Symbol, Option<Method>>,
    /// Whether the program imports anything from `std`. Kept because a
    /// **grammar** module's glob needs to know whether there is anything to
    /// re-export; the preamble no longer asks (see [`emit_std`]).
    uses_std: bool,
    /// Whether this *is* `std`, the one program that may not import the prelude.
    is_std: bool,
    fails: bool,
    /// ADR-010: nobody outside the program chose the bytes its maps are keyed
    /// by, so a map may have the fast hash.
    trusted_input: bool,
    /// ADR-007 D5: the functions that accept a DSL's deferred parameters, by
    /// name. A `;` at a call means options everywhere else, and this is what
    /// says which calls mean the other thing.
    dsl_drivers: HashSet<String>,
    /// **What an `extern "C"` declaration takes, by name and position**
    /// ([ADR-147](../../docs/specification/adr/adr-147.md) D1).
    ///
    /// A declaration says `&[u8]` and C wants a pointer, so the *call* is where
    /// the pointer is made — `bytes.as_ptr()` rather than `bytes`. It is read
    /// from this file's own items rather than handed over by the checker, which
    /// is the one place that arrangement works: the emitter resolves a free
    /// call by name already ([ADR-011](../../docs/specification/adr/adr-011.md)
    /// D2), and a foreign declaration is a name in this very file.
    foreign_params: std::collections::BTreeMap<String, Vec<Type>>,
    /// **The opaque handles this file declares, and what releases each**
    /// ([ADR-147](../../docs/specification/adr/adr-147.md) D3).
    ///
    /// A handle is **lent** to every declaration but one: it is the address by
    /// value, and the caller keeps the value so its `cleanup` still runs at the
    /// end of its scope. The exception is the type's own release function,
    /// which takes it — that call *is* the cleanup, and a handle given away
    /// must not be released twice.
    opaque_handles: std::collections::BTreeMap<String, String>,
    /// What each `extern "C"` declaration hands back, by the name a call writes
    /// ([ADR-155](../../docs/specification/adr/adr-155.md) D3). A handle needs
    /// its hull put on at the call, and whether the declaration said `?`
    /// decides which one.
    foreign_results: std::collections::BTreeMap<String, Type>,
    /// Whether these items are the crate root - the one file that may carry
    /// the program's entry point.
    ///
    /// [ADR-038](../../../docs/specification/adr/adr-038.md) D4 starts the
    /// runtime before the first statement the user wrote, and the way to do
    /// that is to write the `fn main` Rust runs and call the program's own
    /// `main` from inside it. That may only happen once per program, so a
    /// module's items are emitted with this `false` and a `main` in them is
    /// left exactly as written.
    entry: bool,
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

/// The name a Nikaia program gives its entry point.
const MAIN: &str = "main";

/// The type several parts of a program own at once (Part I 6.2).
///
/// The one name whose lowering is decided per **value**: what it expands to is a
/// reference count, and which of the two a particular value gets is
/// `contracts::sharing`'s answer ([ADR-037](../../../docs/specification/adr/adr-037.md)
/// D7).
use crate::contracts::ty::ARRAY;

/// **`std`'s own handle** ([ADR-147](../../docs/specification/adr/adr-147.md)
/// D4): text a C library owns, which no `extern "C"` block declares because
/// `std` does.
const C_STRING: &str = "CStr";

/// A written type's own name, with the module it is reached through taken off
/// ([ADR-154](../../docs/specification/adr/adr-154.md) D3).
///
/// `foreign::CStr` is what a declaration writes now; what this file asks about
/// is the **type**, which is the last segment. The full name is what goes into
/// the emitted Rust, because the module is in `std`'s own prelude there.
fn base(name: &str) -> &str {
    crate::contracts::ty::base(name)
}

const SHARED: &str = "Shared";
/// Part I 6.3's lock, whose shape is decided per value
/// ([ADR-057](../../../docs/specification/adr/adr-057.md)).
const LOCKED: &str = "Locked";

/// The shared mutable type, which is **one name here and two hulls below**
/// ([ADR-064](../../../docs/specification/adr/adr-064.md) D1).
///
/// It is a name the checker carries and only this module expands, which is the
/// whole of D1: a name that resolved away early would put a spelling nobody wrote
/// into every message about it ([ADR-039](../../../docs/specification/adr/adr-039.md)
/// D9 named that as what one name buys).
const SHARED_MUT: &str = "SharedMut";

/// Whose the one error type in a function's set is
/// ([ADR-159](../../docs/specification/adr/adr-159.md) D1, D2).
///
/// The difference decides the **envelope**. A type this unit declares is one
/// the program `throw`s, so the failure has a site and travels in a
/// `Thrown[E]`. A type a **library** declares arrives from a call, with no
/// `throw` in this program to have a site — so it travels as it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Named<'a> {
    Own(&'a str),
    Library(&'a str),
}

/// The slot a function's result is filed under, as `contracts::sharing` keys it.
const SHARED_RESULT: &str = "<result>";

/// The pseudo-function a `<struct>.<field>` slot is filed under, the same.
const SHARED_FIELDS: &str = "<field>";

/// What the program's own `main` is called in the emitted Rust.
///
/// `fn main` belongs to the runtime now
/// ([ADR-038](../../../docs/specification/adr/adr-038.md) D4): it starts the
/// I/O worker, calls this, and drains. The name is spelled so that no Nikaia
/// program plausibly collides with it, and a `rustc` diagnostic about the
/// program's body still lands on the `.nika` source because the body's spans
/// are unchanged (ADR-012).
const PROGRAM_MAIN: &str = "__nikaia_main";

/// The generated name of [ADR-044](../../../docs/specification/adr/adr-044.md)
/// D1's table. `__nikaia_` for the same reason `__nikaia_main` is: a name a
/// program could also have chosen would be a name this compiler took from it.
const ABORT_TABLE: &str = "__NIKAIA_SITES";

/// What the **last** statement of a block is, which is two questions and not
/// one.
///
/// Nikaia's blocks are expressions (Part I, 3.1), so a block's last statement
/// is usually its value. That much was a `bool`. What a `bool` could not say is
/// *whose* value it is, and exactly one rewrite in this file needs to know:
/// `return x` at the end of a function body is written as `x`, because the
/// value a function body ends in **is** what the function hands back.
///
/// That is true of a function body. It is not true of a `match` arm, a `catch`
/// handler, or a block used as a value - their last statement is the value of
/// the *expression around them*, and a `return` inside one leaves the function
/// past it. Writing such a `return` as a bare value is a different program:
/// where the function returns something it is rustc's `E0308` about a file
/// nobody wrote, and where it returns nothing **nothing complains at all** and
/// the function runs on past the point the source said to leave
/// ([`docs/nightly-cost.md`](../../../docs/nightly-cost.md) §3.3).
///
/// So the rewrite is allowed where this says [`Tail::Return`] and nowhere else.
/// Everywhere else a `return` stays a `return`, which is always a legal Rust
/// statement and always means what the source meant - ADR-011 D2's "the
/// lowering is syntactic and never guesses a meaning", applied to the one place
/// it had been guessing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tail {
    /// Not a value position. The last statement is a statement like any other,
    /// and keeps its semicolon.
    Statement,
    /// The last statement is the block's **value**, and that value is handed to
    /// the expression around the block - not to whoever called the function.
    Value,
    /// The last statement is the **function's** (or the lambda's) return value.
    /// Only here may a `return x` be written as `x`.
    Return,
}

impl Tail {
    /// Whether the last statement is the block's value at all - which is what
    /// decides the semicolon, and what ADR-033's grouping may not take.
    fn is_value(self) -> bool {
        !matches!(self, Tail::Statement)
    }

    /// What the statement at `i` of a block whose last index is `last` is.
    fn at(self, i: usize, last: usize) -> Self {
        if i == last {
            self
        } else {
            Tail::Statement
        }
    }
}

/// Which type a method belongs to, and which trait declared it.
///
/// `target` is what makes the ledger key — `Counter::record` rather than
/// `record` — and the key is what a `Shared` position looks its count up by
/// ([`crate::contracts::sharing`]).
///
/// `declared_by` is the trait an `impl` names, where it names one, and it
/// exists because [ADR-109](../../docs/specification/adr/adr-109.md) D2 makes
/// the **declaration** the wider claim: a trait method without `sync` may
/// pause, so its implementations are `async fn` whether or not their own
/// bodies do. A body that never pauses under such a declaration is correct and
/// still has to hand back a future.
#[derive(Clone, Copy)]
struct MethodOf<'a> {
    target: &'a str,
    declared_by: Option<&'a str>,
}

/// What the declaration a body belongs to says about that body.
///
/// These four travel together because the statements inside a body cannot see
/// any of them: the `Ok` a `throws` function's value is wrapped in, whether
/// there is a value at all, which of its parameters a call has to await, and
/// the ledger key the body is filed under. They are one argument rather than
/// four for the same reason `Argument` is one - a function that takes eight
/// loose values is a function whose callers are easy to get wrong.
#[derive(Clone, Copy)]
struct Declared<'a> {
    /// The function may fail, so its value is handed back as an `Ok`.
    throws: bool,
    /// The function hands back a value at all, so its last statement is that
    /// value rather than a statement like any other.
    returns_value: bool,
    /// The code parameters of this declaration whose type may pause
    /// ([ADR-122](../../docs/specification/adr/adr-122.md) D1): a call to one
    /// of these carries an `.await`.
    awaited: &'a [Symbol],
    /// The ledger key of the function these statements are in: `main`, or
    /// `Counter::record` for a method. Two things read it - a `Shared` value's
    /// count is filed under it (`contracts::sharing`), and a `throw` raised
    /// here reports the function's **own** name, which is the key's last
    /// segment (ADR-023 D6: a `throw` in `main` says `main`).
    key: &'a str,
}

/// **The oldest Rust the emitted code compiles under**
/// ([ADR-109](../../docs/specification/adr/adr-109.md) D4).
///
/// 1.75 is where `-> impl Trait` in a trait's method became stable, which is
/// the form D3 writes for a method that may pause. Every generated
/// `Cargo.toml` carries it, and the build compares `rustc --version` against
/// it before handing anything to `cargo` — because Cargo's own *package
/// requires rustc 1.75 or newer* is a message about a generated file, which
/// [Part III C.1](../../docs/specification/30-nikaia-tooling.md) forbids.
///
/// **It is a different fact from the channel.** `rust-toolchain.toml` names
/// that and stays the only place that does
/// ([ADR-001](../../docs/specification/adr/adr-001.md) D1); this is a version
/// the lowering needs, so it lives here. It rises only by a record — a later
/// feature of the language below that some lowering asks for — and never
/// silently.
pub const RUST_FLOOR: &str = "1.75";

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
    /// Kap 7.1: what is being emitted is the guarded half of a `catch`, so a
    /// failure in it is handled here rather than propagated.
    ///
    /// It reaches inward for the same reason `sequential` does: in
    /// `outer(inner()) catch { … }` the handler runs for either call, so
    /// neither propagates. The handler's own body is emitted with the flow the
    /// `catch` was written in, because a failure raised *there* leaves the
    /// function like any other.
    caught: bool,
    /// What is being emitted is a `catch`'s **handler**, and the error it was
    /// handed came through a **named** channel
    /// ([ADR-157](../../../docs/specification/adr/adr-157.md) D2).
    ///
    /// One statement in a handler reads it: `throw error`, which passes the
    /// error on with the site it was raised at rather than wrapping it again.
    /// It does **not** reach inward past a function boundary, for the reason
    /// the binding does not either: a lambda inside a handler has its own
    /// `error` or none.
    caught_named: bool,
    /// The byte the statement being emitted starts at.
    ///
    /// It is here because the checker's answer about method calls is keyed by
    /// it (`check::Checked::fallible_methods`), and an expression has no span
    /// of its own to look itself up by. `stmt` sets it for every statement it
    /// emits, so a statement nested in a block carries its own and not the
    /// outer one's - which is what the checker records, because it walks
    /// statements the same way.
    statement: usize,
    /// The key the ledger records the function being emitted under - `main`, or
    /// `Counter::record` for a method.
    ///
    /// It is here for the same reason `statement` is: the answer about a
    /// `Shared` value is keyed by the function it is written in and the name the
    /// source gives it (`contracts::sharing`), and a `let` has no way to look
    /// itself up without knowing which function it stands in. `origin` is the
    /// name a `throw` reports and is deliberately not this: an error raised in a
    /// method says the method's own name (ADR-023 D6), and a ledger key says
    /// `Type::method`.
    function: &'a str,
    /// What is being emitted sits inside a **lambda's body**
    /// ([ADR-055](../../../docs/specification/adr/adr-055.md) §6).
    ///
    /// A lambda is a closure in the language below, and Rust has no stable
    /// `async` closure - so a call that pauses cannot be written inside one, and
    /// the lowering says so in Nikaia's words rather than handing `rustc` a file
    /// nobody wrote (Part III, C.1).
    ///
    /// **It is here and not in the checker on purpose.** A lambda that reads a
    /// file is a correct Nikaia program - `examples/fortunes.nika`'s route
    /// handler is one - and refusing a correct program is the one thing the
    /// compiler may never do (Part III, C.4). So the type check accepts it and
    /// the *lowering* is what cannot write it, which is a limit of this compiler
    /// and is where a program that is only checked never meets it.
    ///
    /// It reaches inward the way `sequential` and `caught` do: a call written in
    /// a block inside the lambda is still written inside the lambda. A function
    /// body starts a `Flow` of its own, which is the boundary it does not cross.
    in_lambda: bool,
    /// The lambda parameters written **`mut`** that are in scope here
    /// ([ADR-110](../../docs/specification/adr/adr-110.md) D1).
    ///
    /// `kasse.update fn(mut v) { v += 100 }` lowers to a closure whose
    /// parameter is `&mut T`, because D1 hands the block the **address** in the
    /// lock. So every mention of `v` in the body is written `(*v)`: `+=` and an
    /// assignment need the dereference outright, and a method call and a field
    /// read would get it from Rust anyway — writing it everywhere is one rule
    /// rather than a list of positions, and a parenthesis nobody needs is a
    /// warning about the generated file (Part III C.1), which is why the one
    /// here is always needed.
    ///
    /// **It accumulates rather than replacing**, so a lambda written inside an
    /// `update` block does not lose the dereference for a name it captures.
    changed: &'a [Symbol],
    /// The **code parameters whose type may pause**
    /// ([ADR-122](../../../docs/specification/adr/adr-122.md) D1) that are in
    /// scope here.
    ///
    /// Such a parameter lowers to a closure returning a boxed future, so a call
    /// to it is a call that hands back a future and carries an `.await`. The
    /// emitter can answer this one itself — the type is written in the
    /// declaration it is standing in, which is not the case for anything
    /// `pausing_key` looks up ([ADR-028](../../../docs/specification/adr/adr-028.md)).
    ///
    /// **It does not accumulate the way `changed` does.** A lambda written
    /// inside the body has parameters of its own, and a name it binds is not
    /// this function's parameter any more; the list is the declaration's and
    /// stops at every boundary that starts a `Flow::PLAIN`.
    awaited: &'a [Symbol],
    /// The name the statement being emitted **binds**, where it binds one.
    ///
    /// It is here for the reason `function` is: the count a `Shared` was given is
    /// keyed by the function and the name (`contracts::sharing`), and a hull
    /// written by a **call** ([ADR-064](../../../docs/specification/adr/adr-064.md)
    /// D2) has to look that up from inside an expression. An expression carries no
    /// span and no name of its own.
    ///
    /// Empty where the statement binds nothing, which is the floor's case and
    /// never a wrong answer - only a slower one.
    bound: &'a str,
    /// What is being emitted is **inside a constant this pass decided to write
    /// in the wider type** ([ADR-063](../../../docs/specification/adr/adr-063.md)
    /// D1), so every integer literal in it carries the `i64` suffix.
    ///
    /// It reaches inward, and it can only reach literals: an expression folds
    /// only if it is built from literals and arithmetic, so nothing else is
    /// down there to be reached. It is set on the **outermost** expression that
    /// folds, which is why the arms below ask for it before folding again - one
    /// decision per expression, not one per operator.
    widen: bool,
    /// What is being emitted stands where **Rust's own inference gives the
    /// number its type** - a sequence index and a repeat count, both `usize`.
    ///
    /// Those are the two positions a literal is deliberately left bare in
    /// (`index::at` and `count::of` are skipped there, because `at(0)` has
    /// nothing to infer from), and a suffix written into one would pin the type
    /// the position is supposed to decide. So `widen` does not start here, and
    /// what an index that overflows an `i32` gets is the message it gets today
    /// rather than a worse one.
    inferred: bool,
    /// What is being emitted sits inside the **body of a loop** - so a `break`
    /// or a `continue` here has somewhere to go (Part I, 3.3).
    ///
    /// **The backstop, and the checker is the diagnostic.** `NK1132` is what a
    /// program actually meets, and it is the one worth writing because it names
    /// which of a lambda, a task, an `overlap` branch or a fold's step stands in
    /// the way. This is the guarantee underneath it: a walk that missed a
    /// corner would otherwise put `break;` into a closure and let `rustc` answer
    /// about a file nobody wrote (Part III, C.1), and *"the checker's walk is
    /// complete"* is a thing to hope for rather than a thing that holds. Every
    /// statement is emitted through one place, so asking here cannot be evaded.
    ///
    /// It reaches inward the way `caught` and `in_lambda` do, and it stops at
    /// exactly the constructs that are a function below: each of them starts
    /// from [`Flow::PLAIN`], where this is false.
    in_loop: bool,
}

impl Flow<'_> {
    const PLAIN: Flow<'static> = Flow {
        changed: &[],
        awaited: &[],
        throws: false,
        origin: "",
        caught: false,
        caught_named: false,
        statement: usize::MAX,
        function: "",
        in_lambda: false,
        bound: "",
        widen: false,
        inferred: false,
        in_loop: false,
    };

    /// The same surroundings, for the body of a loop.
    fn inside_a_loop(self) -> Self {
        Flow {
            in_loop: true,
            ..self
        }
    }

    /// The same surroundings, for the expression a `catch` guards.
    fn guarded(self) -> Self {
        Flow {
            caught: true,
            ..self
        }
    }

    /// The surroundings a `catch`'s **handler** is emitted in
    /// ([ADR-157](../../../docs/specification/adr/adr-157.md) D2).
    ///
    /// The handler runs outside the guard — a failure raised *there* leaves
    /// the function like any other — so this is the flow the `catch` was
    /// written in, plus what the handler alone knows: whether the error it was
    /// handed came through a named channel.
    fn handling(self, named: bool) -> Self {
        Flow {
            caught_named: named,
            ..self
        }
    }

    /// The same surroundings, for the statement that starts at this byte.
    fn at(self, statement: usize) -> Self {
        Flow { statement, ..self }
    }

    /// The same surroundings, for a statement that binds this name.
    fn binding<'b>(self, bound: &'b str) -> Flow<'b>
    where
        Self: 'b,
    {
        Flow { bound, ..self }
    }

    /// The same surroundings, inside a constant written in the wider type.
    fn widened(self) -> Self {
        Flow {
            widen: true,
            ..self
        }
    }

    /// The same surroundings, where the position decides the number's type.
    fn inferred(self) -> Self {
        Flow {
            inferred: true,
            ..self
        }
    }
}

impl<'p> Emitter<'p> {
    fn new(parsed: &'p Parsed, build: Build, provenance: crate::contracts::Provenance) -> Self {
        let own = crate::contracts::Ledger::infer(parsed);
        Self::with_contracts(parsed, build, provenance, own)
    }

    /// The same, against contracts that already exist - a program's rather than
    /// a file's.
    fn with_contracts(
        parsed: &'p Parsed,
        build: Build,
        provenance: crate::contracts::Provenance,
        own_contracts: crate::contracts::Ledger,
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

        // One type check, both of its answers about where a failure leaves.
        //
        // ADR-025 D7 for the loops: the type checker knows which `for` iterates
        // something whose step can fail, because it infers the iterator's type
        // and reads the ledger. Matching on the name `io::lines` here would
        // have caught the one-line form and quietly missed
        // `let s = io::lines()` followed by `for line in s`. ADR-028 for the
        // method calls: a receiver's type is the type checker's to know, and
        // there is one type checker (ADR-028).
        let propagation = crate::check::propagation_against(parsed, &own_contracts);

        // ADR-037 D7: which count each `Shared` value gets. Computed over the
        // whole unit, because a count belongs to an allocation and a handle's
        // class may reach into another function.
        let library = std_ledger();
        let shared = crate::contracts::sharing::analyse_program(
            parsed,
            &own_contracts,
            &library,
            build.user_parallelism == UserParallelism::Yes,
        )
        .counts;

        // ADR-055 D6, before `own_contracts` is moved into place.
        let reach = pausing_reach(parsed, &own_contracts);

        Self {
            parsed,
            build,
            borrowing: borrowing_structs(parsed),
            tethered: tethered_types(&own_contracts),
            declared_errors: declared_errors(parsed),
            carries_input: crate::views::carried(parsed),
            grammars,
            structs,
            methods,
            by_name,
            uses_std,
            is_std: false,
            fails,
            trusted_input: provenance == crate::contracts::Provenance::Trusted,
            fallible_loops: propagation.loops,
            fallible_methods: propagation.methods,
            pausing_methods: propagation.pausing_methods,
            witnessed_sets: propagation.witnessed_sets,
            future_lambdas: propagation.future_lambdas,
            narrowing_casts: propagation.narrowing,
            shared,
            nullable_sites: propagation.nullable,
            concatenations: propagation.concatenations,
            lent_lets: propagation.lent_lets,
            array_literals: propagation.array_literals,
            comptime_values: propagation.comptime_values,
            flattened_reaches: propagation.flattened,
            nullable_fields: propagation.nullable_in_fields,
            lent_args: propagation.lent_args,
            mut_args: propagation.mut_args,
            nullable_args: propagation.nullable_in_args,
            task_handles: propagation.task_handles,
            method_options: propagation.method_options,
            pausing_reach: reach,
            own_contracts,
            library,
            dsl_drivers: crate::dsl::drivers(parsed).into_iter().collect(),
            foreign_params: foreign_params(parsed),
            opaque_handles: opaque_handles(parsed),
            foreign_results: foreign_results(parsed),
            entry: true,
        }
    }

    /// The same, said of a module rather than of the crate root.
    fn for_entry(mut self, entry: bool) -> Self {
        self.entry = entry;
        self
    }

    /// This is `std` itself - see [`emit_std`].
    fn for_std(mut self) -> Self {
        self.is_std = true;
        self
    }

    fn text(&self, sym: Symbol) -> &str {
        self.parsed.text(sym)
    }

    /// The same text, written so the language below can read it (`escaped`).
    ///
    /// Deliberately **not** folded into `text` above, although that would be one
    /// line instead of every position below. `text` is also what the recorded
    /// answers from the checker are keyed by — `pausing_methods`,
    /// `fallible_methods`, `nullable_fields` are all keyed by the name the
    /// *source* wrote — so escaping there would make every one of those lookups
    /// miss, silently, on exactly the programs this is for.
    fn name(&self, sym: Symbol) -> std::borrow::Cow<'_, str> {
        escaped(self.text(sym))
    }

    /// A module's items and nothing else - no preamble, no `mod` header.
    fn items_only(&self) -> Result<Lowered> {
        let mut out = Out::default();
        self.shadow_types(&mut out);
        for item in &self.parsed.program.items {
            out.from(&item.span, |out| self.item(out, &item.node, &item.span))?;
            out.push("\n");
        }
        self.entry_point(&mut out);
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
        // Always, except in `std` itself, which is the prelude - see
        // [`emit_std`] and [`Needs::preamble`], the copy of this that a program
        // of several files uses.
        if !self.is_std {
            out.push("#[allow(unused_imports)]\npub use nikaia_std::prelude::*;\n");
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
            out.from(&item.span, |out| self.item(out, &item.node, &item.span))?;
            out.push("\n");
        }
        self.entry_point(&mut out);
        if self.user_main().is_some() {
            // **An empty table, and the reason it is empty.** `fn main` installs
            // the hook that reads it (ADR-044 D2), so the name has to be defined
            // or the program does not compile - and a table needs the *file* each
            // line came from, which this path does not know: it is handed one
            // `Parsed` and no path. The path a user takes goes through
            // `modules::Program`, which knows every file and appends the real
            // table (`abort_table`); this one is `emit_program`, which the tests
            // and `lower-std` use.
            //
            // Empty is the fallback rather than a lie: a location the table does
            // not know is handed to the hook installed before ours, which is
            // Rust's own, so such a program is exactly as well off as it was.
            out.push(&format!(
                "\n// ADR-044 D1: no table - this program was emitted without its file name.\n                 const {ABORT_TABLE}: &[nikaia_std::abort::Site] = &[];\n"
            ));
        }

        Ok(Lowered {
            rust: out.buf,
            map: out.map,
        })
    }

    /// The program's own `main`, if this file carries one the runtime can wrap.
    ///
    /// `Some(throws)` for the plain shape - `fn main()` or `fn main() throws`,
    /// no receiver, no parameters, no returned value - and `None` for anything
    /// else, including a `main` in a module rather than at the crate root. A
    /// `main` this does not recognise is emitted exactly as written and keeps
    /// its name, because a wrapper that guessed at a signature would be worse
    /// than no wrapper at all.
    fn user_main(&self) -> Option<bool> {
        if !self.entry {
            return None;
        }
        self.parsed
            .program
            .items
            .iter()
            .find_map(|item| match &item.node {
                Item::Fn {
                    name: Some(name),
                    receiver: None,
                    args,
                    config,
                    spread: None,
                    ret_type: None,
                    throws,
                    ..
                } if self.text(*name) == MAIN && args.is_empty() && config.is_empty() => {
                    Some(*throws)
                }
                _ => None,
            })
    }

    /// The `fn main` Rust runs: ADR-038 D4, as five lines of generated code.
    ///
    /// The runtime is started *before* the program's first statement and
    /// drained after its last, so an operation inside the program costs no
    /// thread start and no thread wake-up - which is
    /// [ADR-033](../../../docs/specification/adr/adr-033.md) §8.4's finding
    /// read the other way round.
    ///
    /// What starts is what
    /// [ADR-037](../../../docs/specification/adr/adr-037.md) D2 allows, and
    /// the *compiler* is what knows which: `user_parallelism` is a build
    /// switch, so it is written into this call rather than read from the
    /// operator's runtime configuration file (ADR-038 D5 has four settings and
    /// this is not one of them).
    fn entry_point(&self, out: &mut Out) {
        let Some(throws) = self.user_main() else {
            return;
        };
        let user_code = match self.build.user_parallelism {
            UserParallelism::No => "Sequential",
            UserParallelism::Yes => "Concurrent",
        };
        // **The runtime's `main` declares the program's own channel**
        // ([ADR-157](../../docs/specification/adr/adr-157.md) D1): it hands
        // `__nikaia_main`'s outcome straight back, so the two have to agree.
        // `Result<(), E>` is a `Termination` for any `E: Debug`, which every
        // envelope is.
        //
        // `main` has no parameters, so there is nothing to borrow from and an
        // error carrying a view can only be `'static` — D9's derivation, one
        // position over.
        let channel = self.error_channel(MAIN, Lifetimes::ELIDED, false);
        let ret = if throws {
            format!(" -> Result<(), {channel}>")
        } else {
            String::new()
        };

        for line in [
            "",
            "// ADR-038 D4: the runtime is running before the program's first",
            "// statement, so an operation inside it costs no thread wake-up. What",
            "// starts is what `user_parallelism` allows (ADR-037 D2): the I/O",
            "// worker always, a pool for user code only at `yes`.",
        ] {
            out.push(line);
            out.push("\n");
        }
        out.push(&format!("fn main(){ret} {{\n"));
        // **Before anything else** (ADR-044 D2): the hook has to be in place
        // before a line that can abort runs, and the first of those is inside
        // the runtime's own start. The table is defined at the end of this file
        // (`abort_table`), which Rust allows and which is what keeps the line
        // numbers in it true - a table written above the program would move
        // every line it names.
        out.push(&format!(
            "    nikaia_std::abort::report_in_nikaia_terms({ABORT_TABLE});\n"
        ));
        out.push(&format!(
            "    let nikaia_runtime = nikaia_std::rt::start(nikaia_std::rt::UserCode::{user_code});\n"
        ));
        // **ADR-055 D3: `main` is what the executor drives.** Where the
        // program's own entry can pause it is an `async fn` (D1), so the one
        // place a future is driven from the outside is here - `block_on` is the
        // boundary between a program that can pause and an operating system
        // that cannot.
        //
        // A `main` the ledger says is `sync` is a plain call, and writing
        // `block_on` around it would be driving something that is not a future.
        match self.pauses(MAIN) {
            true => out.push(&format!(
                "    let outcome = nikaia_std::rt::exec::block_on({PROGRAM_MAIN}());\n"
            )),
            false => out.push(&format!("    let outcome = {PROGRAM_MAIN}();\n")),
        }
        out.push("    nikaia_runtime.finish();\n");
        out.push("    outcome\n");
        out.push("}\n");
    }

    /// Whether any grammar entry in the program reaches a **parallel** rule,
    /// which is what decides whether the piece driver's names are in the
    /// preamble.
    ///
    /// The entry is a call now ([ADR-082](../../docs/specification/adr/adr-082.md)
    /// D1), so the rule is the one the program **named** rather than the one
    /// this file used to pick. Found by being wrong: the lowering was right and
    /// the preamble was not, and `rustc` answered *"use of undeclared type
    /// `ParseContext`"* about the generated file.
    fn uses_driver(&self) -> bool {
        let mut found = false;
        let mut parallel = |grammar: &Symbol, entry: &Symbol| {
            if let Some(def) = self.grammars.get(grammar) {
                let rule = def.rules.iter().find(|r| r.is_public && r.name == *entry);
                if rule.map(|r| par_fold_of(r).is_some()).unwrap_or(false) {
                    found = true;
                }
            }
        };
        for item in &self.parsed.program.items {
            if let Item::Fn { body, .. } = &item.node {
                visit_block(body, &mut |e| {
                    // **A grammar is entered through a path**
                    // ([ADR-140](../../docs/specification/adr/adr-140.md) D3),
                    // so this reads the shape the entry has rather than the one
                    // it used to have. Missing it emitted a `parse_…_pieces`
                    // call with `ParseContext` and `Parallelism` undeclared —
                    // `rustc` about a file nobody wrote (Part III C.1).
                    if let Expr::Call { func, .. } = e {
                        if let Expr::Path(segments) = func.as_ref() {
                            if let [grammar, rule] = segments.as_slice() {
                                parallel(grammar, rule);
                            }
                        }
                    }
                });
            }
        }
        found
    }

    fn item(&self, out: &mut Out, item: &Item, span: &Span) -> Result<()> {
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
                out.push(&format!("{vis}enum {}{params} {{\n", self.name(*name)));
                for variant in variants {
                    let name = self.name(variant.name);
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
                                        self.name(f.name),
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
                generics,
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
                // The input lifetime first and the type parameters after it,
                // which is the order Rust wants them in (ADR-074 D3).
                let mut parts: Vec<String> = Vec::new();
                if self.borrowing.contains(name) {
                    parts.push(INPUT_LIFETIME.to_string());
                }
                parts.extend(generics.iter().map(|g| self.bounded(g)));
                let params = angled(&parts);
                out.push(&format!("{vis}struct {}{params} {{\n", self.name(*name)));
                for field in fields {
                    // Public, because the actions that build this struct are
                    // generated into the grammar's own module.
                    let slot = format!("{}.{}", self.text(*name), self.text(field.name));
                    out.push(&format!(
                        "    {}{}: {},\n",
                        if field.is_public { "pub " } else { "" },
                        self.name(field.name),
                        self.ty_counted(
                            &field.ty,
                            Lifetimes::NAMED,
                            self.count_at(SHARED_FIELDS, &slot)
                        )
                    ));
                }
                out.push("}\n");
                Ok(())
            }
            Item::Fn { .. } => self.function(out, item, 0, Lifetimes::ELIDED, None, None),
            Item::Impl {
                trait_name,
                target,
                methods,
            } => {
                // A type that holds a view carries the input lifetime, and the
                // impl has to declare the lifetime its methods are written with.
                let borrows = self.borrows(target.name);
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

                // `impl Stack[T]` declares `T` and writes `Stack<T>`; the
                // lifetime, where the type carries one, stands in front of it
                // (ADR-074 D4). The target's arguments are written as the
                // source wrote them, so `impl Stack[i64]` stays `Stack<i64>`
                // and declares nothing.
                let declared = crate::contracts::declared_types(self.parsed);
                let generics = crate::contracts::impl_parameters(self.parsed, target, &declared);
                let mut head_parts: Vec<String> = Vec::new();
                if borrows {
                    head_parts.push(INPUT_LIFETIME.to_string());
                }
                head_parts.extend(generics.iter().cloned());
                let declares = angled(&head_parts);
                let mut target_parts: Vec<String> = Vec::new();
                if borrows {
                    target_parts.push(INPUT_LIFETIME.to_string());
                }
                target_parts.extend(
                    target
                        .generics
                        .iter()
                        .map(|g| self.ty(g, Lifetimes::ELIDED)),
                );
                let applied = angled(&target_parts);
                let head = match trait_name {
                    Some(t) => format!(
                        "impl{declares} {} for {target_name}{applied}",
                        self.text(*t)
                    ),
                    None => format!("impl{declares} {target_name}{applied}"),
                };
                out.push(&format!("{head} {{\n"));
                for method in methods {
                    let carries = self.carries_input.get(&method.span.start);
                    out.from(&method.span, |out| {
                        out.push("    ");
                        self.function(
                            out,
                            &method.node,
                            1,
                            lifetimes,
                            carries,
                            Some(MethodOf {
                                target: &target_name,
                                declared_by: trait_name.map(|t| self.text(t)),
                            }),
                        )
                    })?;
                }
                out.push("}\n");
                Ok(())
            }
            // Kap 4.7: `trait Summarize { … }` becomes the same trait below, and
            // the method is a signature with a `;` where an `impl`'s has a body
            // ([ADR-078](../../docs/specification/adr/adr-078.md) D1).
            //
            // **No method here is `async`.** The ledger's `sync` column decides
            // that for a function (ADR-055 D1), and a declaration has no body
            // for `sync::infer` to read - so ADR-078 D4 asserts `sync` for a
            // trait's methods, which is the only answer this position can
            // write: `async fn` in a trait is something the emitter has no way
            // to ask for. What that costs is a trait whose method genuinely
            // pauses, which `open-work.md` carries with its reproduction.
            Item::Trait {
                name,
                methods,
                is_public,
            } => {
                let vis = if *is_public { "pub " } else { "" };
                out.push(&format!("{vis}trait {} {{\n", self.name(*name)));
                for method in methods {
                    out.from(&method.span, |out| {
                        out.push("    ");
                        self.trait_method(out, &method.node, method.node.is_sync, false)
                    })?;
                }
                out.push("}\n");
                Ok(())
            }
            Item::Import { path, alias } => {
                // The names come in through `nikaia_std::prelude`, emitted once
                // in the preamble; the import itself is kept as a comment so the
                // generated file still says where they were asked for.
                //
                // An alias is **resolved away** rather than emitted as a Rust
                // `use … as …` (ADR-046 D3): it is one file's name for a package
                // and every file's items are in one crate, so a Rust alias at the
                // root would be the whole program's. `Parsed::unaliased` is where
                // the resolving happens, on the way to every name.
                let path = path
                    .iter()
                    .map(|s| self.text(*s))
                    .collect::<Vec<_>>()
                    .join("::");
                match alias {
                    Some(alias) => out.push(&format!("// use {path} as {}\n", self.text(*alias))),
                    None => out.push(&format!("// use {path}\n")),
                }
                Ok(())
            }
            // **The same `const` the body form writes, one level out**
            // ([ADR-097](../../docs/specification/adr/adr-097.md)). The
            // spelling is the checker's, handed over keyed by the item's own
            // byte offset the way a statement's is - this emitter knows no
            // types ([ADR-011](../../docs/specification/adr/adr-011.md) D2), so
            // what `1000` is called below is not a question it can answer.
            //
            // `pub` is Part I 9.2's rule for Constants and reaches Rust as
            // Rust's own ([ADR-047](../../docs/specification/adr/adr-047.md)
            // D2): a package's crate is what another package reads.
            Item::Comptime { name, public, .. } => {
                let bound = self.name(*name);
                let Some((below, written)) = self.comptime_values.get(&span.start).cloned() else {
                    return Err(refused!(
                        "`{bound}` has nothing to write, which `NK1127` reports - \
                         so this item should not have reached the emitter"
                    ));
                };
                let vis = match public {
                    true => "pub ",
                    false => "",
                };
                out.push(&format!("{vis}const {bound}: {below} = {written};\n"));
                Ok(())
            }
            // **Rust's own `extern` block**
            // ([ADR-124](../../docs/specification/adr/adr-124.md) D1), which is
            // the whole of the lowering: the form means the same thing on both
            // sides, and a declaration is `trait_method`'s shape with `sync`
            // said by the caller (D2).
            //
            // **Only `"C"`**, and refusing a second ABI is a check rather than a
            // shape: the grammar takes any string so that the message about an
            // unknown one is this compiler's rather than the backend's about a
            // file nobody wrote (Part III C.1).
            Item::Extern {
                abi,
                declarations,
                opaque,
            } => {
                if abi != "C" {
                    return Err(refused!(
                        "`extern \"{abi}\"` names an ABI this compiler does not write. \
                         The one it writes is `extern \"C\"` (Part III 15.1)"
                    ));
                }
                // **A handle is a type of its own, outside the block**
                // ([ADR-147](../../docs/specification/adr/adr-147.md) D3). It
                // is written before the declarations, because they name it.
                for handle in opaque {
                    out.from(&handle.span, |out| {
                        self.opaque_type(out, &handle.node);
                        Ok(())
                    })?;
                }
                out.push(&format!("extern \"{abi}\" {{\n"));
                for declaration in declarations {
                    out.from(&declaration.span, |out| {
                        out.push("    ");
                        self.trait_method(out, &declaration.node, true, true)
                    })?;
                }
                out.push("}\n");
                Ok(())
            }
            other => Err(refused!("cannot emit item yet: {other:?}")),
        }
    }

    /// **An opaque handle, and the `cleanup` that ends its life**
    /// ([ADR-147](../../docs/specification/adr/adr-147.md) D3).
    ///
    /// ```ignore
    /// #[repr(transparent)]
    /// struct sqlite3(*mut core::ffi::c_void);
    /// impl Drop for sqlite3 { fn drop(&mut self) { unsafe { sqlite3_close(…) } } }
    /// ```
    ///
    /// **`repr(transparent)` and not a plain newtype**, because the whole point
    /// of the type is its layout: a handle *is* the address, so `&mut T` at the
    /// boundary is `T**` the way C writes it, and a handle passed by value is
    /// the pointer. A newtype Rust may lay out as it likes would be a different
    /// program at the boundary.
    ///
    /// **`Drop` and not a call the author writes**, which is Part I 6.4's
    /// `cleanup` read at the C boundary: the release runs at the end of the
    /// handle's scope, so a handle cannot be forgotten. What it cannot answer
    /// for is a C function that keeps the address past its own call, which is
    /// the one thing this language cannot check and D3 says so.
    ///
    /// **The release takes a handle and is handed one**, which is what keeps
    /// this from recursing: a value moved into an `extern "C"` function is the
    /// callee's, and C runs no `Drop`. Measured rather than reasoned — the
    /// release fires once per scope and not twice.
    ///
    /// **The address inside is `NonNull`**, which is
    /// [ADR-155](../../docs/specification/adr/adr-155.md) D2: a handle holds an
    /// address and `T?` is the absence of one, and those are the two states C
    /// spells with a pointer and `NULL`. Rust lays `Option<T>` over the same
    /// word for a type shaped like this, so a nullable handle costs nothing and
    /// `&mut T?` is `T **` exactly as C writes it — the right words around the
    /// right memory, which at this boundary is the only kind that counts.
    fn opaque_type(&self, out: &mut Out, handle: &crate::ast::OpaqueType) {
        let name = self.name(handle.name);
        let release = self.name(handle.released_by);
        out.push(&format!(
            "#[repr(transparent)]\n\
             #[derive(Debug)]\n\
             // A C type keeps the name its header gives it, which is not the\n\
             // shape Rust's own lint expects (Part III, C.1: what `rustc` would\n\
             // say here is about a file nobody wrote).\n\
             #[allow(non_camel_case_types)]\n\
             pub struct {name}(core::ptr::NonNull<core::ffi::c_void>);\n\
             \n\
             #[allow(dead_code)]\n\
             impl {name} {{\n\
             \x20   /// What a declaration that says `-> {name}` hands back\n\
             \x20   /// (ADR-155 D3): the address, or an abort naming the\n\
             \x20   /// declaration that claimed it would be one.\n\
             \x20   #[track_caller]\n\
             \x20   fn from_c(declaration: &str, address: *mut core::ffi::c_void) -> {name} {{\n\
             \x20       match core::ptr::NonNull::new(address) {{\n\
             \x20           Some(address) => {name}(address),\n\
             \x20           None => nikaia_std::foreign::nothing_came_back(declaration),\n\
             \x20       }}\n\
             \x20   }}\n\
             \n\
             \x20   /// The same, where the declaration **does** say `?` (D1).\n\
             \x20   fn maybe(address: *mut core::ffi::c_void) -> Option<{name}> {{\n\
             \x20       core::ptr::NonNull::new(address).map({name})\n\
             \x20   }}\n\
             \n\
             \x20   /// The same address, without giving the handle away\n\
             \x20   /// (ADR-147 D3): what a C function is handed is the\n\
             \x20   /// pointer, and this handle stays the caller's to close.\n\
             \x20   fn lent(&self) -> {name} {{\n\
             \x20       {name}(self.0)\n\
             \x20   }}\n\
             }}\n\
             \n\
             impl Drop for {name} {{\n\
             \x20   fn drop(&mut self) {{\n\
             \x20       let _released = unsafe {{ {release}(self.lent()) }};\n\
             \x20   }}\n\
             }}\n\
             \n"
        ));
    }

    /// **A type in an `extern "C"` declaration, as the pointer C wants**
    /// ([ADR-147](../../docs/specification/adr/adr-147.md) D1).
    ///
    /// The four forms the record writes, and each lives **for the call**, which
    /// is what a view is everywhere else in this language
    /// ([ADR-094](../../docs/specification/adr/adr-094.md)):
    ///
    /// | written | below |
    /// | :--- | :--- |
    /// | `&T` | `*const T` |
    /// | `&mut T` | `*mut T` |
    /// | `&[T]` | `*const T` |
    /// | `&mut [T]` | `*mut T` |
    ///
    /// **A slice loses its length here, and that is the point.** C takes a
    /// pointer and a count as two parameters, and D2 is the rule that keeps
    /// them one fact: the declaration names both and the call is checked.
    /// Rust's own `&[T]` is a *fat* pointer, so writing it in a declaration
    /// would be a signature the two languages disagree about — and what a
    /// reader would get for it is `rustc`'s `improper_ctypes` about a file
    /// nobody wrote (Part III, C.1).
    ///
    /// Anything that is not a view is the ordinary lowering: an `i32` is an
    /// `i32` at both ends.
    fn foreign_ty(&self, ty: &Type) -> String {
        if ty.is_slice {
            let element = match ty.generics.first() {
                Some(element) => self.foreign_ty(element),
                None => "u8".to_string(),
            };
            return format!("*{} {element}", pointing(ty.is_mut));
        }
        if ty.is_view {
            let pointed = Type {
                is_view: false,
                is_mut: false,
                ..ty.clone()
            };
            return format!("*{} {}", pointing(ty.is_mut), self.foreign_ty(&pointed));
        }
        self.ty(ty, Lifetimes::ELIDED)
    }

    /// **The hull a call's result needs**, where the callee is a foreign
    /// declaration handing back a handle
    /// ([ADR-155](../../docs/specification/adr/adr-155.md) D2, D3): the
    /// handle's name, and whether the declaration said it **may be absent**.
    fn handle_from(&self, func: &Expr) -> Option<(String, bool)> {
        let Expr::Variable(name) = func else {
            return None;
        };
        let declared = self.foreign_results.get(self.text(*name))?;
        let handle = self.handle_named(declared)?;
        Some((handle.to_string(), declared.is_nullable))
    }

    /// The callee as the source wrote it, for a message that names it.
    fn called_name(&self, func: &Expr) -> String {
        match func {
            Expr::Variable(name) => self.text(*name).to_string(),
            _ => String::new(),
        }
    }

    /// The handle a type names, where it names one
    /// ([ADR-147](../../docs/specification/adr/adr-147.md) D3,
    /// [ADR-155](../../docs/specification/adr/adr-155.md) D1) — through a `?`,
    /// because `CStr?` is a handle that may be absent and is still a handle.
    ///
    /// `None` for a view or a slice of one: what a `&T` at the boundary is is
    /// D1's pointer, and the handle is what it points at.
    fn handle_named(&self, ty: &Type) -> Option<&str> {
        if ty.is_view || ty.is_slice {
            return None;
        }
        let name = self.text(ty.name);
        match self.opaque_handles.contains_key(name) || base(name) == C_STRING {
            true => Some(name),
            false => None,
        }
    }

    /// What the address inside a handle points at, which is the one thing this
    /// language never looks through.
    fn pointed_at(&self, ty: &Type) -> &'static str {
        match base(self.text(ty.name)) == C_STRING {
            true => "core::ffi::c_char",
            false => "core::ffi::c_void",
        }
    }

    /// Which shape a declared parameter takes at the C boundary, or `None`
    /// where it is an ordinary value an `i32` is at both ends
    /// ([ADR-147](../../docs/specification/adr/adr-147.md) D1, D3).
    fn pointer_for(&self, callee: &str, ty: &Type) -> Option<Pointer> {
        if let Some(release) = self.opaque_handles.get(self.text(ty.name)) {
            if !ty.is_view && !ty.is_slice {
                return Some(Pointer::Handle {
                    give: release == callee,
                });
            }
        }
        pointer_for(ty)
    }

    /// Whether an `extern "C"` declaration takes this position in `size_t`
    /// ([ADR-147](../../docs/specification/adr/adr-147.md) D2).
    ///
    /// The declaration's own word, and not a list of names: `is_count` beside
    /// it is a fact about *Rust's* library that nothing in this compiler can
    /// derive, where a foreign declaration is written in this very file.
    fn takes_a_size(&self, callee: &str, at: usize) -> bool {
        self.foreign_params
            .get(callee)
            .and_then(|params| params.get(at))
            .is_some_and(|ty| !ty.is_view && !ty.is_slice && self.text(ty.name) == "usize")
    }

    /// One method of a `trait`: a signature and a `;`.
    ///
    /// Deliberately **not** `function` with the body switched off. That one
    /// reads the ledger for `sync`, for the `Shared` counts of each position and
    /// for whether the call can fail, and every one of those answers is about a
    /// *body* - which a declaration does not have. A second, smaller writer says
    /// what a declaration is instead of what a definition happens to omit.
    ///
    /// `sync` is the caller's rather than the declaration's, because the same
    /// shape reads two ways: a **trait** method without the word may pause
    /// ([ADR-109](../../docs/specification/adr/adr-109.md) D1), and an
    /// **`extern "C"`** declaration never can
    /// ([ADR-124](../../docs/specification/adr/adr-124.md) D2) — C has no
    /// suspension point, and a C function that sleeps blocks a thread, which is
    /// `println`'s question and not this one.
    fn trait_method(
        &self,
        out: &mut Out,
        method: &crate::ast::TraitMethod,
        is_sync: bool,
        // **Whether this is a C declaration**
        // ([ADR-147](../../docs/specification/adr/adr-147.md) D1), which is
        // what decides how a view is written: at the C boundary it is the
        // pointer C wants, and everywhere else it is the borrow Rust wants.
        // The same `&str` is a thin pointer to C and a fat one to Rust, so a
        // declaration that wrote the second would be a signature the two
        // languages disagree about - and `rustc` says so, about a file nobody
        // wrote (Part III, C.1).
        foreign: bool,
    ) -> Result<()> {
        let mut params: Vec<String> = Vec::new();
        if let Some(receiver) = &method.receiver {
            params.push(
                match (receiver.is_ref, receiver.is_mut) {
                    (true, true) => "&mut self",
                    (true, false) => "&self",
                    _ => "self",
                }
                .to_string(),
            );
        }
        for arg in &method.args {
            params.push(format!(
                "{}: {}",
                self.name(arg.name),
                match foreign {
                    true => self.foreign_ty(&arg.ty),
                    false => self.ty(&arg.ty, Lifetimes::ELIDED),
                }
            ));
        }
        // Kap 5.1: the language below has neither named arguments nor defaults,
        // so an option is an ordinary parameter here too - the same rule a
        // definition follows, because a caller fills in one list either way.
        for option in &method.config {
            params.push(format!(
                "{}: {}",
                self.name(option.name),
                self.ty(&option.ty, Lifetimes::ELIDED)
            ));
        }
        let returned = match &method.ret_type {
            // **A handle comes back as the address it is**
            // ([ADR-155](../../docs/specification/adr/adr-155.md) D3), and the
            // call is where it becomes a handle. Declaring the *type* here
            // would be a lie the moment C hands back nothing: the hull under a
            // handle is non-null, so a null arriving in one is undefined
            // before any check could run.
            Some(ty) if foreign && self.handle_named(ty).is_some() => {
                format!("*mut {}", self.pointed_at(ty))
            }
            Some(ty) => match foreign {
                true => self.foreign_ty(ty),
                false => self.ty(ty, Lifetimes::ELIDED),
            },
            None => "()".to_string(),
        };
        let outcome = if method.throws {
            format!("Result<{returned}, Box<dyn std::error::Error>>")
        } else {
            returned
        };
        // **A method that may pause is declared in the return-position form**
        // ([ADR-109](../../docs/specification/adr/adr-109.md) D3): `-> impl
        // Future<Output = …>`, which an `async fn` in the `impl` satisfies.
        //
        // **The sugar `async fn` is not written here**, and that is the reason
        // the long form is: `async_fn_in_trait` warns on a public trait whose
        // future carries no `Send` bound, and a warning about the generated file
        // is a defect in this project (Part III C.1).
        //
        // **`+ Send` follows the executor, not the type.** At
        // `user_parallelism = yes` a task crosses threads and a spawn needs a
        // `Send` future; at `no` nothing crosses, the counts are plain
        // ([ADR-037](../../docs/specification/adr/adr-037.md) D7) and a `Send`
        // demand would refuse them. It is a requirement of the executor this
        // program is built for, written here — not a claim about a type, which
        // is `contracts::send`'s and is the same at both settings.
        let ret = match (is_sync, method.ret_type.is_some() || method.throws) {
            (false, _) => {
                let send = match self.build.user_parallelism {
                    UserParallelism::Yes => " + Send",
                    UserParallelism::No => "",
                };
                format!(" -> impl std::future::Future<Output = {outcome}>{send}")
            }
            (true, true) => format!(" -> {outcome}"),
            (true, false) => String::new(),
        };
        let generics: Vec<String> = method.generics.iter().map(|g| self.bounded(g)).collect();
        out.push(&format!(
            "fn {}{}({}){ret};\n",
            self.name(method.name),
            angled(&generics),
            params.join(", ")
        ));
        Ok(())
    }

    /// A type parameter with its bounds: `T`, or `T: Summarize + Clone`
    /// ([ADR-078](../../docs/specification/adr/adr-078.md) D2).
    fn bounded(&self, param: &crate::ast::GenericParam) -> String {
        let name = self.name(param.name);
        match param.bounds.is_empty() {
            true => name.into_owned(),
            false => {
                let bounds: Vec<String> = param
                    .bounds
                    .iter()
                    .map(|b| self.name(*b).into_owned())
                    .collect();
                format!("{name}: {}", bounds.join(" + "))
            }
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
            let carries = self.carries_input.get(&method.span.start);
            out.from(&method.span, |out| {
                out.push("    ");
                self.function(
                    out,
                    &method.node,
                    1,
                    lifetimes,
                    carries,
                    Some(MethodOf {
                        target,
                        declared_by: None,
                    }),
                )
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
        // The parameters written as views of the input buffer rather than of
        // the call (`Emitter::carries_input`). `None` where there is no such
        // parameter, which is almost every function.
        carries_input: Option<&HashSet<Symbol>>,
        // The type this is a method of, where it is one. It is what makes the
        // ledger key - `Counter::record` rather than `record` - and the key is
        // what a `Shared` position looks its count up by (`contracts::sharing`).
        owner: Option<MethodOf<'_>>,
    ) -> Result<()> {
        let Item::Fn {
            name,
            generics,
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

        // The key the ledger - and therefore `contracts::sharing` - records this
        // function under, arrived at the same way both of them arrive at it: the
        // anonymous constructor of Part I 4.2 is `new`, and a method carries its
        // type in front.
        let own_name = match name {
            Some(name) => self.text(*name).to_string(),
            None => "new".to_string(),
        };
        let key = match owner {
            Some(owner) => format!("{}::{own_name}", owner.target),
            None => own_name.clone(),
        };

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
        // A parameter the subject's buffer covers is written as a view of that
        // buffer, so the signature says what the body does with it; everything
        // else keeps the position's own spelling.
        let how = |name: Symbol| match carries_input.is_some_and(|set| set.contains(&name)) {
            true => lifetimes.of_the_input(),
            false => lifetimes,
        };
        // **A parameter the body does not keep is a view**
        // ([ADR-094](../../docs/specification/adr/adr-094.md) D2). The `&` is
        // written here, in the declaration, and at the call — one answer read
        // twice (`contracts::keeps::lends`), because the two disagreeing is a
        // `&&T` or a moved value in the language below.
        let lent = self.own_contracts.functions.get(&key);
        params.extend(args.iter().enumerate().map(|(at, a)| {
            // The **source** name is what a `Shared` position is counted by
            // (`count_at`), and the **escaped** one is what is written: two uses
            // of one name that must not be collapsed, or the lookup misses on
            // exactly the programs the escape is for (ADR-076 D2).
            let name = self.text(a.name);
            // The receiver is a parameter of the contract and not of `args`, so
            // the position in the signature is one further along where there is
            // one.
            let at = at + usize::from(receiver.is_some());
            // **And only where the written type does not already carry one.**
            // `&Vec[Entry]` in a declaration is the assertion D2 keeps: the
            // parameter is a view either way, and what D1 changes is the
            // *call*, where the reference is written for both kinds alike.
            let written_as_a_view = lent
                .and_then(|c| c.signature.as_ref())
                .and_then(|s| s.params.get(at))
                .is_some_and(|(_, ty)| ty.is_a_view());
            // **`mut` is a third state and comes first**
            // ([ADR-094](../../docs/specification/adr/adr-094.md) D3): a
            // parameter the callee changes in place lowers to `&mut T`, and the
            // caller's value is what changes. `lends` withholds its own claim
            // on such a position, so the two never both answer.
            let changes = lent
                .and_then(|c| c.signature.as_ref())
                .is_some_and(|s| s.mutable.iter().any(|m| m == name));
            let reference = if changes {
                "&mut "
            } else if lent.is_some_and(|c| crate::contracts::keeps::lends(c, at))
                && !written_as_a_view
            {
                "&"
            } else {
                ""
            };
            format!(
                "{}: {reference}{}",
                escaped(name),
                self.ty_counted(&a.ty, how(a.name), self.count_at(&key, name))
            )
        }));
        // Kap 5.1: the language below has neither named arguments nor defaults,
        // so an option becomes an ordinary parameter here - in declaration
        // order, which is the order every call site fills in. The names stay
        // the source's, so a `rustc` diagnostic about one still lands on the
        // parameter the programmer wrote (ADR-012).
        params.extend(config.iter().map(|c| {
            let name = self.text(c.name);
            format!(
                "{}: {}",
                escaped(name),
                self.ty_counted(&c.ty, how(c.name), self.count_at(&key, name))
            )
        }));

        // ADR-007 D5: `...args: Self::dsl` is the one parameter whose type the
        // *call site* decides, because the DSL string it comes from decides it.
        // A generic parameter is what that is in the language below, and Rust
        // monomorphises it per DSL string exactly as D5 asks - so the driver is
        // written once and pays no heap traffic per statement.
        let dsl = spread.as_ref().map(|name| {
            params.push(format!("{}: {DSL_PARAMETER}", self.text(*name)));
            DSL_PARAMETER.to_string()
        });

        // **`fn hand[T](x: T)` is `fn hand<T>(x: T)`**
        // ([ADR-074](../../docs/specification/adr/adr-074.md) D3). The `[T]` was
        // read by the parser and then fell out here, because this position had
        // no slot for one - so a program that passed every stage of this
        // compiler asked `rustc` about a type nobody had declared, which is
        // Part III C.1's class exactly.
        let declared: Vec<String> = generics
            .iter()
            .map(|g| self.bounded(g))
            .chain(dsl.clone())
            .collect();

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
                    return Err(refused!(
                        "`Self::dsl` names the parameters of a `...args: Self::dsl`, \
                         and this function declares none"
                    ));
                }
                DSL_PARAMETER.to_string()
            }
            Some(ty) => {
                // The result's lifetimes are not always the parameters': a view
                // handed back by a function that borrowed nothing has to say
                // `'static`, because there is nothing for Rust's elision to take
                // and `-> &str` is a message about the generated file
                // (`Lifetimes::STATIC`).
                //
                // A receiver counts as something to borrow from, and so does any
                // parameter that is a view or holds one. Where either is there,
                // the position keeps the spelling it had.
                //
                // **And a struct that holds a view is one too**, which is the
                // half `holds_view` alone could not see: `-> Vec[Entry]` where
                // `Entry` holds a `&str` carries a lifetime exactly as `-> &str`
                // does, and a function with nothing to borrow from lowered it to
                // `Vec<Entry<'_>>` — *missing lifetime specifier* about a file
                // nobody wrote. `carries_a_view` asks the question about the
                // whole type, the declaration included, and it is asked on
                // **both** sides so that `fn f(r: Reading) -> Reading` keeps the
                // elision it needs.
                //
                // **And only where the position declares no lifetime of its
                // own**, which is `Lifetimes::ELIDED` and is D9's own case: *a
                // free function's signature*. Inside an `impl` that declares
                // `'a` the result is the subject's `'a`, and `'static` there
                // says the value outlives the program — which is a promise the
                // `impl` cannot keep. `examples/1brc.nika`'s `Summary()` is
                // what said so: an anonymous constructor has no parameters and
                // no receiver, so the widening reached it.
                let borrows_from_something =
                    receiver.is_some() || args.iter().any(|a| self.carries_a_view(&a.ty));
                let widens = lifetimes == Lifetimes::ELIDED
                    && self.carries_a_view(ty)
                    && !borrows_from_something;
                let result = match widens {
                    true => Lifetimes::STATIC,
                    false => lifetimes,
                };
                self.ty_counted(ty, result, self.count_at(&key, SHARED_RESULT))
            }
            None => "()".to_string(),
        };
        // **The failure channel** ([ADR-157](../../docs/specification/adr/adr-157.md)
        // D1): the error type where the ledger names exactly one, the box
        // otherwise. A method that implements a **trait** keeps the box, and
        // has to: the trait's own declaration writes the channel, and an `impl`
        // answering with another type would not satisfy it.
        let channel = match owner.and_then(|o| o.declared_by) {
            Some(_) => "Box<dyn std::error::Error>".to_string(),
            None => self.error_channel(
                &key,
                lifetimes,
                receiver.is_some() || args.iter().any(|a| self.carries_a_view(&a.ty)),
            ),
        };
        let ret = if *throws {
            format!(" -> Result<{returned}, {channel}>")
        } else if ret_type.is_some() {
            format!(" -> {returned}")
        } else {
            String::new()
        };

        let vis = if *is_public { "pub " } else { "" };
        let name = own_name;
        // ADR-038 D4: `fn main` is the runtime's, and the program's own entry
        // point is called from inside it. Only at the crate root, and only for
        // the shape `entry_point` writes a wrapper for - `user_main` and this
        // ask the same question, so a renamed function always has a caller.
        //
        // The **source** name is kept for the body, because it is what an
        // error raised in here reports as its site (ADR-023 D6, ADR-036): a
        // `throw` in `main` says `main`, and the name this emitter chose for
        // the lowering is not something the author ever wrote.
        let emitted = if depth == 0 && name == MAIN && self.user_main().is_some() {
            PROGRAM_MAIN.to_string()
        } else {
            escaped(&name).into_owned()
        };

        // ADR-055 D1: a function that can pause is an `async fn`, and one the
        // ledger's `sync` column says cannot is a plain `fn`. The property is
        // ADR-027 D1's, already inferred; this reads it.
        //
        // **And a trait's declaration is the wider claim**
        // ([ADR-109](../../docs/specification/adr/adr-109.md) D2): a method
        // declared without `sync` is lowered `-> impl Future<…>`, so every
        // implementation of it hands back a future — whether or not its own
        // body pauses. A body that never pauses under such a declaration is
        // correct and says nothing, which is D2's own sentence; what it cannot
        // do is hand back an `i64` where a future was promised.
        let declared_pausing = owner
            .and_then(|owner| owner.declared_by)
            .map(|t| format!("{t}::{name}"))
            .is_some_and(|key| {
                self.own_contracts
                    .functions
                    .get(&key)
                    .is_some_and(|c| !c.sync.is_sync())
            });
        let pausing = if self.pauses(&key) || declared_pausing {
            "async "
        } else {
            ""
        };
        out.push(&format!(
            "{vis}{pausing}fn {emitted}{}({}){ret} ",
            angled(&declared),
            params.join(", ")
        ));
        // **The parameters a call has to await**
        // ([ADR-122](../../docs/specification/adr/adr-122.md) D1): those whose
        // type is code and does not say `sync`, which is the default.
        let awaited: Vec<Symbol> = args
            .iter()
            .filter(|arg| arg.ty.code.as_ref().is_some_and(|code| !code.is_sync))
            .map(|arg| arg.name)
            .collect();
        self.function_body(
            out,
            body,
            depth,
            Declared {
                throws: *throws,
                returns_value: ret_type.is_some(),
                awaited: &awaited,
                key: &key,
            },
        )?;
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
        declared: Declared<'_>,
    ) -> Result<()> {
        let Declared {
            throws,
            returns_value,
            awaited,
            key,
        } = declared;
        let flow = Flow {
            changed: &[],
            awaited,
            throws,
            origin: key.rsplit("::").next().unwrap_or(key),
            caught: false,
            // A function body is not a handler, whatever the call to it sits
            // inside: the boundary `in_lambda` does not cross, this does not
            // cross either.
            caught_named: false,
            statement: usize::MAX,
            function: key,
            // A function body was not written inside whatever lambda the call
            // to it sits in: this is the one boundary the flag does not cross.
            in_lambda: false,
            // A function body binds nothing until a statement in it does.
            bound: "",
            // Both are decided per expression, so a body starts with neither.
            widen: false,
            inferred: false,
            // And no loop encloses a function's first statement.
            in_loop: false,
        };

        // A function body's last statement is the *function's* value, which is
        // the one place a `return x` may be written as `x`.
        let tail = if returns_value {
            Tail::Return
        } else {
            Tail::Statement
        };

        if !throws {
            return self.block(out, body, depth, flow, tail);
        }

        let pad = "    ".repeat(depth);
        let inner_pad = "    ".repeat(depth + 1);

        out.push("{\n");
        let last = body.stmts.len().saturating_sub(1);
        let mut i = 0;
        while i < body.stmts.len() {
            let stmt = &body.stmts[i];
            out.push(&inner_pad);
            // A value-returning `throws` function ends in its value; one that
            // returns nothing ends in the `Ok(())` below, so its last statement
            // is a statement like any other.
            let here = tail.at(i, last);
            let wrap = here == Tail::Return;
            out.from(&stmt.span, |out| {
                if wrap {
                    out.push("Ok(");
                }
                self.stmt(out, &stmt.node, &stmt.span, depth + 1, here, flow)?;
                if wrap {
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
                    // A grammar action is the rule's own body: what it ends
                    // in is what the rule hands back, so a `return` there is
                    // that value (ADR-009).
                    self.block(out, action, 3, Flow::PLAIN, Tail::Return)?;
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
        if let Expr::Closure { params, body, .. } = step {
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
                            self.stmt(
                                out,
                                &stmt.node,
                                &stmt.span,
                                0,
                                Tail::Statement,
                                Flow::PLAIN,
                            )?;
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
                                is_nullable: false,
                                code: None,
                                count: None,
                                is_mut: false,
                                is_slice: false,
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
                return Err(refused!(
                    "`dsl {name} {{ … }}` with deferred parameters takes no context, \
                     and `{}` was given one",
                    self.text(*context)
                ));
            }
            out.push(&rust_string(content.trim()));
            return Ok(());
        }

        if name != "html" {
            return Err(refused!(
                "`dsl {name} {{ … }}` has no hole, so nothing here says what it \
                 means. A statement with `:name` holes is a deferred-parameter DSL \
                 and lowers (ADR-007 D5); one without them is the target grammar's \
                 to give a meaning, and `{name}` is not a grammar this compiler has \
                 - the one it is itself the grammar for is `html` (ADR-017)."
            ));
        }
        if let Some(context) = context {
            return Err(refused!(
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
            return Err(crate::diagnostics::refuse(message));
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
                        .map_err(|e| refused!("in the template hole `{{{expr}}}`: {e}"))?;
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
                    // **`.iter()` and not `&`**, for the reason the ordinary
                    // `for` has it ([ADR-094](../../docs/specification/adr/adr-094.md)
                    // D4): the captured name may already *be* a view — since
                    // D1 a parameter the body only reads is one — and
                    // `&entries` is then a `&&Vec<Entry>`, which Rust does not
                    // iterate. `.iter()` reads the same through any number of
                    // references.
                    out.push(&format!("{pad}for {binding} in {collection}.iter() {{\n"));
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
        // `unaliased` on the joined path, because an alias is this file's name
        // for a *package* and the head segment is where one can stand
        // ([ADR-046](../../../docs/specification/adr/adr-046.md) D3). Resolved
        // here rather than emitted as a Rust `use … as …`: every file's items are
        // in one crate root, so a Rust alias there would be the whole program's.
        self.parsed.unaliased(
            &segments
                .iter()
                .map(|s| self.map_name(s))
                .collect::<Vec<_>>()
                .join("::"),
        )
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

    /// The same, for a name that may carry its module
    /// ([ADR-154](../../docs/specification/adr/adr-154.md) D3).
    ///
    /// `collections::HashMap` is the spelling a program writes once `HashMap`
    /// is outside the prelude, and the question this answers is about the
    /// **type** and not about the prefix: the prefix rides through untouched,
    /// so the annotation and the constructor cannot disagree about which hash a
    /// map got.
    fn mapped_path(&self, name: &str) -> String {
        match name.rsplit_once("::") {
            Some((module, last)) => format!("{module}::{}", self.map_name(last)),
            None => self.map_name(name).to_string(),
        }
    }

    /// The name a type is written with below, which for one name depends on the
    /// value and not only on the name.
    ///
    /// `Shared[T]` is `Rc<T>` or `Arc<T>`, and **which one is decided per value**
    /// (ADR-037 D7): the atomic count is the floor at both settings of
    /// `user_parallelism` (D6), and a value the analysis proves never crosses a
    /// thread gets the plain one. So the count has to arrive from outside -
    /// `map_name` above sees only a name and could not answer it.
    ///
    /// The **full path** and no `use`: `sharing::rust_name` already names both as
    /// paths, two names as common as `Rc` and `Arc` are two a program may have of
    /// its own, and nothing about the emitted file needs them shortened.
    fn written_name(&self, name: &str, count: crate::contracts::sharing::Count) -> String {
        match name {
            SHARED => crate::contracts::sharing::rust_name(count).to_string(),
            // **The same question, the same answer**
            // ([ADR-057](../../../docs/specification/adr/adr-057.md) D3): a lock
            // is only reachable from two places through a shared handle, so the
            // count that handle was given decides the lock inside it. The count
            // rides down through the arguments already (`ty_counted`), which is
            // exactly the thread this needs.
            //
            // **And at one user thread it is always the cheap shape** (D2).
            // Nothing a user writes can cross there, so the safe shape buys
            // nothing and costs what `user_parallelism = no` exists to save -
            // and the cheap one keeps the diagnostic, which is the only failure
            // reachable at that setting.
            LOCKED => crate::contracts::sharing::lock_name(match self.build.user_parallelism {
                UserParallelism::No => crate::contracts::sharing::Count::Plain,
                UserParallelism::Yes => count,
            })
            .to_string(),
            // `unaliased`, for the reason [`Emitter::path`] gives: a type may be
            // written with this file's own name for the package that declares it
            // ([ADR-046](../../../docs/specification/adr/adr-046.md) D3).
            _ => self.parsed.unaliased(&self.mapped_path(name)),
        }
    }

    /// What `Shared(x)`, `SharedMut(x)` and `Locked(x)` allocate, outermost first
    /// ([ADR-064](../../../docs/specification/adr/adr-064.md) D2).
    ///
    /// Two for the shared mutable type, because it is one name and two hulls; one
    /// for the other two; `None` for a call that is not a hull's.
    fn hull_new(&self, name: &str, count: crate::contracts::sharing::Count) -> Option<Vec<String>> {
        match name {
            SHARED => Some(vec![self.written_name(SHARED, count)]),
            LOCKED => Some(vec![self.written_name(LOCKED, count)]),
            SHARED_MUT => Some(vec![
                self.written_name(SHARED, count),
                self.written_name(LOCKED, count),
            ]),
            _ => None,
        }
    }

    /// The count the analysis gave the `Shared` written at one position, with the
    /// atomic floor where it has no answer (ADR-037 D6).
    fn count_at(&self, function: &str, value: &str) -> crate::contracts::sharing::Count {
        // **At one user thread every count is plain**
        // ([ADR-061](../../../docs/specification/adr/adr-061.md) D2), including at
        // a position the analysis has no entry for. Without this the *floor* would
        // answer for such a position, and the floor is the atomic count - which at
        // `no` is exactly what that switch exists not to pay.
        if self.build.user_parallelism == UserParallelism::No {
            return crate::contracts::sharing::Count::Plain;
        }
        self.shared
            .get(&format!("{function}::{value}"))
            .copied()
            .unwrap_or_default()
    }

    /// A type, with the atomic floor for any `Shared` inside it.
    ///
    /// Every position whose count is actually known names it with
    /// [`Emitter::ty_counted`] instead. The floor is what a position nothing
    /// decided gets, which is ADR-037 D6's answer and never a wrong one - only a
    /// slower one.
    fn ty(&self, ty: &Type, lifetimes: Lifetimes) -> String {
        self.ty_counted(ty, lifetimes, crate::contracts::sharing::Count::Atomic)
    }

    /// The same, where the count a `Shared` in this position was given is known.
    ///
    /// The count rides down through the arguments, because `Vec[Shared[i64]]` in
    /// a parameter is the parameter's class and not a class of its own - a count
    /// belongs to the allocation, and the analysis keys it by the position the
    /// source wrote.
    fn ty_counted(
        &self,
        ty: &Type,
        lifetimes: Lifetimes,
        count: crate::contracts::sharing::Count,
    ) -> String {
        let mut out = String::new();

        // `(A, B)` in both languages, and the parts are the arguments.
        if ty.is_tuple {
            let parts: Vec<String> = ty
                .generics
                .iter()
                .map(|g| self.ty_counted(g, lifetimes, count))
                .collect();
            return format!("({})", parts.join(", "));
        }

        // **A parameter that is code**
        // ([ADR-102](../../docs/specification/adr/adr-102.md) D1), lowered as
        // D5's **run** case: a closure argument, which is what `std`'s own
        // higher-order entries take and what costs nothing. The kept case — a
        // boxed closure over a boxed future — is the step of that record that
        // is not built, and `NK1142` is what a field or a result meets, so this
        // is only ever reached in a parameter.
        //
        // `throws` puts the same `Result` on the closure's result that a
        // `throws` function's own declaration puts on its (Kap 7.1), which is
        // what makes a lambda that fails fit it.
        if let Some(code) = &ty.code {
            let params: Vec<String> = ty
                .generics
                .iter()
                .map(|g| self.ty_counted(g, lifetimes, count))
                .collect();
            let outcome = match (&code.result, code.throws) {
                (Some(r), false) => self.ty_counted(r, lifetimes, count),
                (Some(r), true) => format!(
                    "Result<{}, Box<dyn std::error::Error>>",
                    self.ty_counted(r, lifetimes, count)
                ),
                (None, true) => "Result<(), Box<dyn std::error::Error>>".to_string(),
                (None, false) => "()".to_string(),
            };
            // **The type decides the shape**
            // ([ADR-122](../../docs/specification/adr/adr-122.md) D1), and
            // nothing inferred stands behind it: a parameter whose type may
            // pause — which is the **default**, since `sync` is what says
            // otherwise — is a closure returning a **boxed future**, whether the
            // callee runs it or keeps it. A reader can tell what a signature
            // costs by reading it, and a library author who wants no box writes
            // `sync`, which is what that word already promises.
            //
            // Rust has no stable `async` closure, so the future is named rather
            // than inferred: `Pin<Box<dyn Future<Output = …>>>` is the shape a
            // handler is taken in in practice and the one `|a| Box::pin(async
            // move { … })` produces.
            let shape = match code.is_sync {
                true => match (&code.result, code.throws) {
                    (None, false) => String::new(),
                    _ => format!(" -> {outcome}"),
                },
                false => {
                    format!(" -> std::pin::Pin<Box<dyn std::future::Future<Output = {outcome}>>>")
                }
            };
            return format!("impl Fn({}){shape}", params.join(", "));
        }

        // **`Seen[T]` is erased**
        // ([ADR-111](../../docs/specification/adr/adr-111.md) D1): it is a type
        // here and in the ledger and **not** one in the language below, so a
        // `Seen[i64]` is an `i64`, a field declared `Seen[i64]` is an `i64`
        // field, and a signature with `Seen` in it is one without. No counter,
        // no marker, no check at run time, no bytes.
        //
        // Written where the name is read rather than at every position that
        // takes a type, so that a `Vec[Seen[i64]]` and a `Seen[i64]?` come out
        // right for the same reason `Shared` does one paragraph down.
        if self.text(ty.name) == crate::contracts::ty::SEEN {
            let inner = ty.generics.first().cloned().unwrap_or_else(|| Type {
                name: ty.name,
                generics: Vec::new(),
                is_view: false,
                is_nullable: false,
                is_tuple: false,
                code: None,
                count: None,
                is_mut: false,
                is_slice: false,
            });
            let inner = Type {
                is_nullable: ty.is_nullable || inner.is_nullable,
                is_view: ty.is_view || inner.is_view,
                ..inner
            };
            return self.ty_counted(&inner, lifetimes, count);
        }

        // Part I 2.3: `T?` is an `Option<T>`, which is the mapping Part III 15.2
        // writes the other way round. The `?` is peeled and the rest of this
        // function renders the type it is nullable *of* - so `&str?` is an
        // `Option<&'a str>` and the view still picks up its lifetime, which it
        // would not if the wrapper were applied by name.
        if ty.is_nullable {
            let inner = Type {
                is_nullable: false,
                ..ty.clone()
            };
            return format!("Option<{}>", self.ty_counted(&inner, lifetimes, count));
        }

        // A view is a borrow of the parser's input, and that is where the
        // lifetime comes from - the source never writes one (ADR-008).
        if ty.is_view {
            out.push_str(lifetimes.reference);
        }
        // **`SharedMut[T]` is one name and two hulls**
        // ([ADR-064](../../../docs/specification/adr/adr-064.md) D1). It is
        // expanded here and nowhere earlier, so the checker, the ledger and every
        // message keep the name the source wrote. The two hulls are the ones the
        // count already decides - `written_name` answers for each.
        if self.text(ty.name) == SHARED_MUT {
            if let Some(held) = ty.generics.first() {
                // `out` already carries the `&` of a view, so the expansion is
                // pushed rather than returned on its own.
                out.push_str(&format!(
                    "{}<{}<{}>>",
                    self.written_name(SHARED, count),
                    self.written_name(LOCKED, count),
                    self.ty_counted(held, lifetimes, count)
                ));
                return out;
            }
        }
        // **`Array[T, N]` is `[T; N]`**
        // ([ADR-152](../../docs/specification/adr/adr-152.md) D1, D2): `N`
        // elements inline, no allocation, and the layout C gives it.
        //
        // After the view, so `&Array[f64, 3]` is a borrow of the array rather
        // than an array of borrows, and before the name is written, because the
        // arguments do not go where a generic's do.
        if self.text(ty.name) == ARRAY {
            if let [element, length] = ty.generics.as_slice() {
                if let Some(n) = length.count {
                    out.push_str(&format!(
                        "[{}; {n}]",
                        self.ty_counted(element, lifetimes, count)
                    ));
                    return out;
                }
            }
        }
        out.push_str(&self.written_name(self.text(ty.name), count));

        let mut params: Vec<String> = ty
            .generics
            .iter()
            .map(|g| self.ty_counted(g, lifetimes, count))
            .collect();
        // A struct that holds a view carries the input lifetime with it.
        if self.borrows(ty.name) {
            params.insert(0, lifetimes.params.to_string());
        }
        if !params.is_empty() {
            out.push_str(&format!("<{}>", params.join(", ")));
        }

        out
    }

    // --- Statements and expressions ---

    /// `if c { … } else { … }`, where `tail` says what the whole thing is in
    /// the position of ([`Tail`]).
    ///
    /// It decides what the last statement of each branch means, and the branches
    /// are in the same position the `if` is: as the tail of a function body they
    /// carry the function's value, as `let x = if c { a } else { b }` they carry
    /// the `let`'s, and as a statement they carry nothing.
    #[allow(clippy::too_many_arguments)]
    fn if_expr(
        &self,
        out: &mut Out,
        cond: &Expr,
        then_branch: &Block,
        else_branch: Option<&Block>,
        depth: usize,
        flow: Flow<'_>,
        tail: Tail,
    ) -> Result<()> {
        out.push("if ");
        self.expr(out, cond, depth, flow)?;
        out.push(" ");
        self.block(out, then_branch, depth, flow, tail)?;
        if let Some(block) = else_branch {
            out.push(" else ");
            // **`else if` and not `else { if … }`**
            // ([ADR-132](../../docs/specification/adr/adr-132.md) D2), so the
            // generated Rust reads as the source does. The condition is the
            // block holding **one** `if` and nothing else, which is exactly what
            // the parser makes of an `else if` - and an `else` whose block holds
            // an `if` *and* other statements stays a block, because that is what
            // the author wrote.
            //
            // It is the chain's depth this takes away rather than a brace: three
            // links nested three deep is a line the reader has to unwind, and
            // Part III C.1's rule is about what a reader of the generated file
            // meets.
            if let [only] = block.stmts.as_slice() {
                if let Stmt::Expr(Expr::If {
                    cond,
                    then_branch,
                    else_branch,
                }) = &only.node
                {
                    return self.if_expr(
                        out,
                        cond,
                        then_branch,
                        else_branch.as_ref(),
                        depth,
                        flow,
                        tail,
                    );
                }
            }
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
        tail: Tail,
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
        tail: Tail,
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
            // **Statements run in the order they are written**
            // ([ADR-050](../../../docs/specification/adr/adr-050.md) D1), and no
            // analysis stands between the source and the schedule. A program
            // that wants overlap writes `overlap { … }` and has the claim
            // checked (D3).

            let stmt = &block.stmts[i];
            out.push(&inner_pad);
            out.from(&stmt.span, |out| {
                self.stmt(
                    out,
                    &stmt.node,
                    &stmt.span,
                    depth + 1,
                    tail.at(i, last),
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

    /// Whether a **branch of an `overlap`** can pause
    /// ([ADR-050](../../../docs/specification/adr/adr-050.md) D6).
    ///
    /// A walk and not a lookup, because the question is about *this statement*
    /// and the answers the ledger and the checker give are keyed one by callee
    /// and one by statement. Both are consulted here: a free call by the key it
    /// resolves to, a method call by what the checker said about the statement
    /// it stands in - which is the same pair of sources `call` and the method
    /// arm use, asked ahead of time.
    ///
    /// It was ADR-033's, for the pair the compiler chose to overlap; D1
    /// withdrew that and D6 asks the same question about a branch the
    /// programmer chose.
    fn branch_pauses(&self, stmt: &Spanned<Stmt>, flow: Flow<'_>) -> bool {
        let Stmt::Expr(value) = &stmt.node else {
            return false;
        };
        let flow = flow.at(stmt.span.start);
        let mut pauses = false;
        visit_expr(value, &mut |expr| match expr {
            Expr::Call { func, .. } => pauses |= self.pausing_key(func).is_some(),
            Expr::MethodCall { method, .. } | Expr::SafeMethod { method, .. } => {
                pauses |= self.method_pauses(flow, *method)
            }
            _ => {}
        });
        pauses
    }

    /// `tail` says what this statement is in the position of ([`Tail`]):
    /// Nikaia's blocks are expressions (Part I, 3.1), so a block's last
    /// statement is its value and keeps no semicolon - and where that value is
    /// the *function's*, a `return x` is written as `x`.
    fn stmt(
        &self,
        out: &mut Out,
        stmt: &Stmt,
        span: &Span,
        depth: usize,
        tail: Tail,
        flow: Flow<'_>,
    ) -> Result<()> {
        // Which statement this is, for the one thing that has to look itself up
        // by it: a method call that can fail (ADR-023 D8). Shadowed rather than
        // passed on separately, so that everything emitted from here - an
        // expression, a nested block, a `catch` handler - sees the statement it
        // is actually in.
        let flow = flow.at(span.start);
        match stmt {
            Stmt::Let {
                names,
                mutable,
                ty,
                value,
            } => {
                let mutable = if *mutable { "mut " } else { "" };
                // **A tuple of names is Rust's own tuple pattern**
                // ([ADR-098](../../docs/specification/adr/adr-098.md)): the
                // language below takes a tuple apart by position exactly as
                // this one does, so there is nothing to translate. No count is
                // looked up and no annotation is written, because a hull's
                // count is decided per **value** and a destructure names none -
                // which is the same answer `contracts::sharing` gives, and the
                // two have to agree.
                if let [_, _, ..] = names.as_slice() {
                    let bound: Vec<String> = names
                        .iter()
                        .map(|n| escaped(self.text(*n)).into_owned())
                        .collect();
                    out.push(&format!("let {mutable}({}) = ", bound.join(", ")));
                    let (before, after) =
                        Self::around(self.nullable_sites.get(&span.start).copied());
                    out.push(before);
                    self.expr(out, value, depth, flow)?;
                    out.push(after);
                    out.push(";");
                    return Ok(());
                }
                let bound = self.text(names[0]);
                let count = self.count_at(flow.function, bound);
                // A hull written by a call inside this value looks its count up
                // by the name being bound (ADR-064 D2), and an expression has
                // none of its own.
                let flow = flow.binding(bound);
                let annotation = match ty {
                    Some(ty) => format!(": {}", self.ty_counted(ty, Lifetimes::ELIDED, count)),
                    None => String::new(),
                };
                // **The annotation is no longer the constructor**
                // ([ADR-064](../../../docs/specification/adr/adr-064.md) D2). What
                // used to be allocated here, out of an answer the checker had to
                // compute and hand over, is now written where it happens - and the
                // two positions that could carry one stopped being a list.
                //
                // Part I 2.3: a plain value standing in a nullable slot. That hull
                // stays the compiler's, because it is one a program cannot observe
                // - the same value, possibly absent (ADR-064 D2's own line).
                let (before, after) = Self::around(self.nullable_sites.get(&span.start).copied());
                out.push(&format!("let {mutable}{}{annotation} = ", escaped(bound)));
                out.push(before);
                // **A `let` over a place is a view of it**
                // ([ADR-094](../../docs/specification/adr/adr-094.md) D4).
                // `let s = totals.stations[name]` and `let name = config.name`
                // were never moves: the language below refuses to move a value
                // out of a container or out of a borrowed field, so what the
                // line meant is the `&` it would otherwise have been refused
                // for not writing.
                //
                // **`let y = x` over a whole variable stays a move** — it is a
                // rename — and a `let` whose initialiser is a call or a literal
                // owns what it is given. So this is narrower than `for`'s rule
                // by exactly one shape, and that shape is the reason both are
                // written here rather than in one test.
                if self.lent_lets.contains(&span.start) {
                    out.push("&");
                }
                self.expr(out, value, depth, flow)?;
                out.push(after);
                out.push(";");
            }
            // **The folded value is what is written**, not the expression that
            // folded ([ADR-073](../../docs/specification/adr/adr-073.md) D3).
            // That is the visible half of the demand: `const LIMIT = 4 * 1024`
            // reaches the language below as `4096`, so a reader of the generated
            // file can see that the arithmetic did not survive into the program.
            //
            // The type comes from the checker, because Rust's `const` takes one
            // and this has no types (ADR-028). A `comptime` with no entry there was
            // refused as `NK1127` and never arrives.
            Stmt::Comptime { name, .. } => {
                let bound = self.text(*name);
                let Some((below, written)) = self.comptime_values.get(&span.start).cloned() else {
                    return Err(refused!(
                        "`{bound}` has nothing to write, which `NK1127` reports - \
                         so this statement should not have reached the emitter"
                    ));
                };
                out.push(&format!("const {bound}: {below} = {written};"));
            }

            // **A write through the brackets is not an index**
            // ([ADR-080](../../docs/specification/adr/adr-080.md) D2). Rust's
            // `Index` for a map is over whatever the key *borrows* as, so
            // `scores["Player1"] = 100` left the key type unpinned and the
            // program failed with `E0282` in a message naming `TrustedMap` and
            // a type parameter `K` - three spellings the program never wrote.
            // `index::set` is `insert` for a map and an indexed assignment for a
            // sequence, chosen by the language below on the container's type,
            // because this emitter does not know it (ADR-011 D2).
            //
            // Only the plain `=`: a compound `m[k] += 1` reads the slot as well
            // as writing it, so it is an `Index` either way and a second rule
            // would be needed to say what reading an absent key means. That is
            // a question of its own and not this one.
            Stmt::Assign {
                target: box_index @ Expr::Index { .. },
                op: None,
                value,
            } => {
                let Expr::Index { base, index } = box_index else {
                    unreachable!("matched as an index")
                };
                let (before, after) = Self::around(self.nullable_sites.get(&span.start).copied());
                out.push("nikaia_std::index::set(&mut ");
                self.expr(out, base, depth, flow)?;
                out.push(", nikaia_std::index::at(");
                self.expr(out, index, depth, flow)?;
                out.push("), ");
                out.push(before);
                self.expr(out, value, depth, flow)?;
                out.push(after);
                out.push(");");
            }
            Stmt::Assign { target, op, value } => {
                let (before, after) = Self::around(self.nullable_sites.get(&span.start).copied());
                self.expr(out, target, depth, flow)?;
                match op {
                    Some(op) => out.push(&format!(" {}= ", binary_op(*op))),
                    None => out.push(" = "),
                }
                out.push(before);
                self.expr(out, value, depth, flow)?;
                out.push(after);
                out.push(";");
            }
            // Kap 3.3. Name for name (ADR-011 D2): the language below spells
            // this the same way, so there is nothing to decide here.
            Stmt::While { cond, body } => {
                // **`while true` is emitted as `loop`**, and this is the one
                // place the lowering translates what a loop *means* rather than
                // how it is spelled.
                //
                // It is not a departure from name for name (ADR-011 D2) but the
                // decided case of it:
                // [ADR-070](../../../docs/specification/adr/adr-070.md) D1 says
                // `while true { … }` **is** this language's unconditional loop
                // and that the absence of a second spelling is a decision. Rust's
                // name for the unconditional loop is `loop`. So `loop` is the
                // translation of the program and `while true` is a transcription
                // of its letters - and the letters are what the language below
                // objects to:
                //
                //     warning: denote infinite loops with `loop { ... }`
                //
                // a warning on a line nobody wrote, which is Part III C.1's class
                // one severity down. **The alternative was an `#![allow]` in the
                // preamble**, and it is worse twice over: it silences a symptom
                // on every file this compiler will ever write, and it leaves the
                // second reason below unbuilt.
                //
                // **The second reason, which is why this is not cosmetic.** The
                // two forms do not have the same *type* below. `while true { }`
                // is `()`; `loop { }` diverges and is `!`, so
                // `fn f() -> i32 { loop { } }` compiles and the `while` form is
                // an `E0308`. [ADR-093](../../docs/specification/adr/adr-093.md)
                // wants a function that never
                // returns to stop needing an unreachable `return`, and no
                // checker change can deliver that while the lowering emits the
                // form the language below refuses. This is that prerequisite.
                //
                // **The literal only**, never a name that happens to be true:
                // the equivalence is ADR-070 D1's and it is about the written
                // form, and the polarity is the usual one - claim it where it is
                // certain and nowhere else.
                match cond {
                    Expr::LitBool(true) => out.push("loop "),
                    _ => {
                        out.push("while ");
                        self.expr(out, cond, depth, flow)?;
                        out.push(" ");
                    }
                }
                self.block(out, body, depth, flow.inside_a_loop(), Tail::Statement)?;
            }

            Stmt::For {
                bindings,
                iter,
                body,
            } => {
                let names = bindings
                    .iter()
                    .map(|b| self.name(*b))
                    .collect::<Vec<_>>()
                    .join(", ");
                if bindings.len() > 1 {
                    out.push(&format!("for ({names}) in "));
                } else {
                    out.push(&format!("for {names} in "));
                }
                // **A `for` lends** ([ADR-094](../../docs/specification/adr/adr-094.md)
                // D4). `for e in entries { … }` leaves `entries` where it was,
                // so `entries.len()` on the next line is a program rather than
                // `rustc`'s *use of moved value* about a file nobody wrote.
                // Iteration that takes the elements away is **written** —
                // `for x in xs.drain()` — because removing a name from scope is
                // the rare case and the one worth a word.
                //
                // Off the shape of the expression and not off a column: a
                // **place** is lent, and a call, a range or a literal owns what
                // it made.
                //
                // **`.iter()` and not `&`**, which is one measurement rather
                // than a preference: `entries` may already *be* a view — a
                // parameter declared `&Vec[Entry]` — and `&entries` is then a
                // `&&Vec<Entry>`, which Rust does not iterate. `.iter()` reads
                // the same through any number of references, and this emitter
                // has no types to tell the two apart with (ADR-028).
                let lends = is_a_place(iter);
                self.expr(out, iter, depth, flow)?;
                if lends {
                    out.push(".iter()");
                }
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
                self.block_opening_with(
                    out,
                    body,
                    depth,
                    flow.inside_a_loop(),
                    Tail::Statement,
                    unwrap.as_deref(),
                )?;
            }
            // A `return` that ends a *function body* is that function's
            // value, and is written as one: `fn f() -> T { return x }` is
            // `{ x }`. In a `throws` function the caller of this wraps the tail
            // in `Ok`, so the value is emitted bare here in both cases.
            //
            // `Tail::Return` and not merely "last": the last statement of a
            // `match` arm, a `catch` handler or a block used as a value is the
            // value of the expression around it, and a `return` there leaves
            // the function past that expression. It falls through to the arm
            // below and stays a `return`.
            // Kap 3.3 and [ADR-084](../../../docs/specification/adr/adr-084.md).
            // Name for name, like `while` above and for the same reason
            // (ADR-011 D2): the language below spells both of these the same way
            // and gives them the same meaning, so the lowering is a
            // transcription - which is also what makes the construct cost
            // nothing at run time (`docs/break-continue-cost.md` §2).
            //
            // **No `Tail` arm**, and the semicolon stands even where the
            // statement is a block's last: a block that ends in a jump diverges,
            // and Rust reads `{ break; }` in value position as the `!` it is -
            // the same way `{ return x; }` is read there.
            //
            // **And the `in_loop` test is D6's guarantee**, not a second opinion
            // on the checker's: `NK1132` is the message a program meets, and
            // this is what makes *"the checker's walk is complete"* something
            // that holds rather than something to hope for.
            Stmt::Break | Stmt::Continue => {
                let word = match stmt {
                    Stmt::Break => "break",
                    _ => "continue",
                };
                if !flow.in_loop {
                    return Err(refused!(
                        "`{word}` has no loop to act on here, and the language below \
                         would refuse the file this writes (Part I, 3.3)"
                    ));
                }
                out.push(&format!("{word};"));
            }
            Stmt::Return(Some(value)) if tail == Tail::Return => {
                self.nullable(out, value, span, depth, flow)?;
            }
            Stmt::Return(value) => {
                // Kap 7.1: a `throws` function returns a `Result`, so what the
                // source hands back is what goes inside the `Ok`.
                match (value, flow.throws) {
                    (Some(value), true) => {
                        out.push("return Ok(");
                        self.nullable(out, value, span, depth, flow)?;
                        out.push(");");
                    }
                    (Some(value), false) => {
                        out.push("return ");
                        self.nullable(out, value, span, depth, flow)?;
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
                tail,
            )?,
            Stmt::Expr(expr) => {
                self.expr(out, expr, depth, flow)?;
                // `if x { … };` is legal and noisy; a block-shaped statement
                // ends where its brace does.
                let block_shaped = matches!(
                    expr,
                    Expr::If { .. } | Expr::Block(_) | Expr::Unsafe(_) | Expr::Overlap(_)
                );
                if !tail.is_value() && !block_shaped {
                    out.push(";");
                }
            }
        }
        Ok(())
    }

    /// **What goes before a wrapped value, and what goes after**
    /// ([ADR-068](../../../docs/specification/adr/adr-068.md)).
    ///
    /// One function for all four of Part I 2.3's positions, so the two forms are
    /// written in one place: `Some(` … `)` where the checker knows the value is
    /// a plain `T`, and `` … `.into()` where it could not work the type out —
    /// which is right whether the value turns out to be a `T` or already a `T?`.
    ///
    /// The target is pinned in every one of the four — a declared result, an
    /// annotation, an assignment's left side, a field's or a parameter's
    /// declared type — so the conversion has something to resolve against and
    /// never has to be inferred from the value alone.
    fn around(how: Option<crate::check::Wrap>) -> (&'static str, &'static str) {
        match how {
            Some(crate::check::Wrap::Constructor) => ("Some(", ")"),
            Some(crate::check::Wrap::Conversion) => ("", ".into()"),
            None => ("", ""),
        }
    }

    /// An expression, with Part I 2.3's `Some(…)` around it where the checker
    /// says a plain value stands in a nullable slot.
    ///
    /// A `return` needs this and a `let` writes it inline, because a `let` has
    /// the shared-value constructor to nest inside as well and the order of the
    /// two parentheses is that statement's business.
    fn nullable(
        &self,
        out: &mut Out,
        value: &Expr,
        span: &Span,
        depth: usize,
        flow: Flow<'_>,
    ) -> Result<()> {
        let (before, after) = Self::around(self.nullable_sites.get(&span.start).copied());
        out.push(before);
        self.expr(out, value, depth, flow)?;
        out.push(after);
        Ok(())
    }

    fn expr(&self, out: &mut Out, expr: &Expr, depth: usize, flow: Flow<'_>) -> Result<()> {
        match expr {
            Expr::LitInt(v) => out.push(&integer_literal(*v, flow.widen)),
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
            // Part I 2.3. `None` and nothing around it: `null` has no type of
            // its own, so the type beside it is what says what it is the
            // absence of - and a `None` the language below cannot type is a
            // program `rustc` asks an annotation for, which is the honest
            // answer rather than one this compiler invented.
            Expr::LitNull => out.push("None"),
            // **A `mut` lambda parameter is an address** (ADR-110 D1), so
            // every mention of it is dereferenced.
            Expr::Variable(name) if flow.changed.contains(name) => {
                out.push(&format!("(*{})", self.name(*name)))
            }
            // **A constructor handed over as a value**
            // ([ADR-140](../../docs/specification/adr/adr-140.md) D2). `Summary`
            // in value position is the anonymous constructor, and the language
            // below calls it `Summary::new` — the same two spellings the call
            // path reconciles, one position over. `par_fold(M, Summary, …)` is
            // where it is written.
            //
            // **Only a name this file declares as a type**, or one the library
            // publishes with a constructor: anything else is an ordinary name,
            // and a local that shadows a type is refused a declaration by
            // [ADR-144](../../docs/specification/adr/adr-144.md) rather than
            // guessed at here.
            Expr::Variable(name) if self.constructs_by_name(self.text(*name)) => {
                out.push(&self.path(&[self.text(*name), "new"]))
            }
            Expr::Variable(name) => out.push(&self.name(*name)),
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
            // A block in *expression* position: its last statement is its
            // own value, and a `return` inside it leaves the function around
            // it - so it is not written as the block's value (`Tail`).
            Expr::Block(block) => self.block(out, block, depth, flow, Tail::Value)?,
            // **Rust's own `unsafe`**
            // ([ADR-124](../../docs/specification/adr/adr-124.md) D3), which is
            // the whole of the lowering: the word means the same thing on both
            // sides and the block inside it is an ordinary one.
            Expr::Unsafe(block) => {
                out.push("unsafe ");
                self.block(out, block, depth, flow, Tail::Value)?;
            }

            // **Part I 8.1.2: every branch in flight, and the value is their
            // results in written order** (ADR-050 D2).
            Expr::Overlap(block) => self.overlap(out, block, depth, flow)?,
            // **Part II 12.4: every arm in flight, and the first to finish is
            // the one that is kept** (ADR-148 D1).
            Expr::Select(arms) => self.select(out, arms, depth, flow)?,
            // An `if` in *expression* position - `let x = if c { a } else { b }`
            // - hands its branch's value to whoever asked for it, so a `return`
            // in a branch is the function's and stays one.
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
                Tail::Value,
            )?,
            Expr::Call { func, args, config } => self.call(out, func, args, config, depth, flow)?,
            Expr::MethodCall {
                receiver,
                method,
                args,
                config,
            } => {
                // **`error.full()` in a handler a named channel reached**
                // ([ADR-157](../../docs/specification/adr/adr-157.md) D2). Part
                // I 7.1 writes it as an ordinary call, and it is one — but the
                // envelope was opened at the binding, so the two halves of the
                // long form are `error` and the site beside it rather than one
                // value with a method on it.
                let long_form = flow.caught_named
                    && self.text(*method) == "full"
                    && args.is_empty()
                    && matches!(receiver.as_ref(), Expr::Variable(name) if self.text(*name) == CAUGHT);
                match long_form {
                    true => out.push(&format!("{SITE}.full_of(&{CAUGHT})")),
                    false => {
                        self.method_call(out, Some(receiver), *method, args, config, depth, flow)?
                    }
                }
            }
            // Part I 3.5 onto a **method**
            // ([ADR-066](../../../docs/specification/adr/adr-066.md)): the call
            // happens only where there is something to call it on.
            //
            // **A `match` and not the field's `map`**, and the difference is the
            // difference between a field and a method. A field access can
            // neither pause nor fail, so a closure is somewhere it can happen; a
            // method call may do both, and inside a closure an `.await` does not
            // compile and a `?` has nowhere to go. So the reach is written out,
            // which is the one shape that lets the call be whatever a call is.
            //
            // `None => None` and not `.map(…)`'s implicit one, for the flattened
            // case: where the method's own result is a `T?`, the `Some` arm
            // hands it back as it is and the reach does not nest.
            Expr::SafeMethod {
                receiver,
                method,
                args,
                config,
            } => {
                let name = self.text(*method).to_string();
                let flattens = self
                    .flattened_reaches
                    .contains(&(flow.statement, name.clone()));
                out.push("match ");
                self.postfix_base(out, receiver, depth, flow)?;
                out.push(&format!(
                    " {{
{}",
                    "    ".repeat(depth + 1)
                ));
                out.push(&format!("Some({REACHED}) => "));
                if !flattens {
                    out.push("Some(");
                }
                self.method_call(out, None, *method, args, config, depth + 1, flow)?;
                if !flattens {
                    out.push(")");
                }
                out.push(&format!(
                    ",
{}None => None,
{}}}",
                    "    ".repeat(depth + 1),
                    "    ".repeat(depth)
                ));
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
                    // **The guard is Rust's own**
                    // ([ADR-137](../../../docs/specification/adr/adr-137.md)
                    // D2), written with the same word, so this is a
                    // transcription like everything else about a pattern.
                    if let Some(guard) = &arm.guard {
                        out.push(" if ");
                        self.expr(out, guard, depth + 1, flow)?;
                    }
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
            // **`vec![…]`, which is what a `Vec` already is below**
            // ([ADR-135](../../../docs/specification/adr/adr-135.md) D1). Part
            // I 2.2 offers one container and the literal writes that one, so
            // there is no second shape to choose between here.
            //
            // **Unless the use asked for an array**
            // ([ADR-152](../../../docs/specification/adr/adr-152.md) D4), which
            // is a fact about the literal's *type* and therefore the checker's
            // to hand over: the same three values are `vec![…]` in one position
            // and `[…]` in the other, and nothing in the source distinguishes
            // them.
            Expr::ListLit { items, at } => {
                let array = self.array_literals.contains(at);
                out.push(match array {
                    true => "[",
                    false => "vec![",
                });
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        out.push(", ");
                    }
                    self.expr(out, item, depth, flow)?;
                }
                out.push("]");
            }
            Expr::Field { base, name } => {
                self.postfix_base(out, base, depth, flow)?;
                out.push(&format!(".{}", self.name(*name)));
            }
            // Part I 3.5: `x?.name` reaches the field only where there is
            // something to reach it on.
            //
            // **`map` or `and_then`, and the checker says which.** Over a plain
            // field `map` is right; over a field that is *itself* a `T?` it
            // would make an `Option<Option<T>>`, and `and_then` is what
            // flattens - which is a question about the declared type, so it is
            // answered where the types are (ADR-028). `map` is the fallback,
            // because it is the one that cannot nest a plain field.
            Expr::SafeField { base, name } => {
                let field = self.text(*name).to_string();
                let flattens = self
                    .flattened_reaches
                    .contains(&(flow.statement, field.clone()));
                let how = if flattens { "and_then" } else { "map" };
                self.postfix_base(out, base, depth, flow)?;
                out.push(&format!(".{how}(|__nikaia_it| __nikaia_it.{field})"));
            }
            Expr::Index { base, index } => {
                // **A length is an `i64`, so an index is one too**
                // ([ADR-048](../../../docs/specification/adr/adr-048.md) D1), and
                // Rust indexes a sequence by `usize`. The conversion is emitted
                // and never written, in both directions - which is the whole of
                // what D1 buys.
                //
                // Around **every** index but one, because this emitter does not
                // know types (ADR-011 D2) and a map is indexed too:
                // `counts[path]` takes a `&str`. `nikaia_std::index::at` chooses
                // on the type of what is in the brackets and is the identity for
                // anything that is not a number, so a rule applied everywhere
                // cannot be applied to the wrong index.
                //
                // **The one exception is an index written only in literals.**
                // `xs[0]` and `&text[1..3]` need no conversion - Rust's own
                // inference gives a literal the `usize` the sequence wants - and
                // they cannot *take* one: `at(0)` has nothing to infer `I` from,
                // since every integer type answers with the same `usize`, and
                // `cannot infer type` about a generated file is exactly what
                // Part III C.1 forbids.
                self.postfix_base(out, base, depth, flow)?;
                match only_literals(index) {
                    true => {
                        out.push("[");
                        self.expr(out, index, depth, flow.inferred())?;
                        out.push("]");
                    }
                    false => {
                        out.push("[nikaia_std::index::at(");
                        self.expr(out, index, depth, flow)?;
                        out.push(")]");
                    }
                }
            }
            Expr::Cast { expr, ty } => {
                let into = self.ty(ty, Lifetimes::ELIDED);
                match self.narrows(flow.statement, &into) {
                    // **A narrowing conversion is checked, and an unchecked one
                    // aborts** (ADR-043 D4). Rust's `as` truncates by definition
                    // and no setting changes that, so this is the one place in
                    // the arithmetic decision where the emitted code differs from
                    // what was written - and it differs into what a Rust
                    // programmer would have written here anyway.
                    Some(crate::check::Narrowing::Integer) => {
                        // `unwrap_or_else` and not `expect`: `expect` appends
                        // the error's `Debug`, which is `TryFromIntError(())` -
                        // Rust internals in a sentence a Nikaia user reads, and
                        // the same leak the diagnostics filter exists to stop.
                        out.push(&format!("{into}::try_from("));
                        self.expr(out, expr, depth, flow)?;
                        out.push(&format!(
                            ").unwrap_or_else(|_| panic!(\"the value does not fit in an `{into}`\"))"
                        ));
                    }
                    // `i32::try_from(f64)` does not exist, so the range and
                    // "not a number" are tested by a `std` helper instead.
                    Some(crate::check::Narrowing::FromFloat) => {
                        out.push(&format!("nikaia_std::num::to_{into}("));
                        self.expr(out, expr, depth, flow)?;
                        out.push(")");
                    }
                    None => {
                        self.nested(out, expr, u8::MAX, depth, flow)?;
                        out.push(&format!(" as {into}"));
                    }
                }
            }
            Expr::StructLit { name, fields } => {
                // `unaliased`, the same as a type: `h::Request(path: …)` builds
                // `http::Request` (ADR-046 D3).
                let owner = self.parsed.unaliased(self.text(*name));
                out.push(&format!("{owner} {{ "));
                for (i, field) in fields.iter().enumerate() {
                    if i > 0 {
                        out.push(", ");
                    }
                    out.push(&self.name(field.name));
                    // Part I 2.3: a plain value in a field the struct declares
                    // nullable. Keyed by the field's own name, because a struct
                    // literal has one of these per field and the statement has
                    // only one span - **and by the type and the value's shape**,
                    // because a statement may build two literals
                    // (`check::argument_shape`).
                    let how = self
                        .nullable_fields
                        .get(&(
                            flow.statement,
                            owner.to_string(),
                            self.text(field.name).to_string(),
                        ))
                        .and_then(|by_shape| {
                            by_shape.get(
                                &field
                                    .value
                                    .as_ref()
                                    .map(crate::check::argument_shape)
                                    .unwrap_or_default(),
                            )
                        })
                        .copied();
                    let (before, after) = Self::around(how);
                    if let Some(value) = &field.value {
                        out.push(": ");
                        out.push(before);
                        self.expr(out, value, depth, flow)?;
                        out.push(after);
                    } else if how.is_some() {
                        // `Counter { db }` is the shorthand for `db: db`
                        // (Part I 4.1), and a wrapper has to be written around
                        // the name - which means writing the pair out.
                        let name = self.name(field.name);
                        out.push(&format!(": Some({name})"));
                    }
                }
                out.push(" }");
            }
            // A lambda's arguments are the ones it names (ADR-049): nothing is
            // read off the body, so `fn { … }` is `||`.
            Expr::Closure {
                params,
                mutable,
                body,
            } => {
                let names: Vec<String> =
                    params.iter().map(|p| self.name(*p).into_owned()).collect();
                out.push(&format!("|{}| ", names.join(", ")));
                // **The `mut` ones reach inward** (ADR-110 D1), and the list is
                // the enclosing one plus this lambda's rather than this
                // lambda's alone: a lambda written inside an `update` block
                // still means the address when it names the outer `v`.
                let mut changed: Vec<Symbol> = flow.changed.to_vec();
                changed.extend(mutable.iter().copied());
                let flow = Flow {
                    changed: &changed,
                    ..flow
                };
                // A lambda's `return` leaves the lambda, not the function
                // around it, so it never carries the enclosing `Ok`. The
                // statement is kept, because the checker's answers about the
                // calls in here are keyed by it, and `in_lambda` is what makes
                // a pausing one refusable (ADR-055 §6).
                let inside = Flow {
                    in_lambda: true,
                    statement: flow.statement,
                    // **And the `mut` parameters with it**: the body starts
                    // from `PLAIN` because a lambda's `return` leaves the
                    // lambda, and that must not throw away what D1 needs to
                    // write `(*v)`.
                    changed: &changed,
                    ..Flow::PLAIN
                };
                self.block(out, body, depth, inside, Tail::Return)?;
            }
            Expr::Unary { op, expr } => {
                // **`-2147483648` is an `i32`**, and its digits are not
                // ([ADR-060](../../../docs/specification/adr/adr-060.md) D3): the
                // question is about the value, so a negation is folded here
                // rather than left for `integer_literal` to answer about a number
                // that is one too large.
                if let (UnaryOp::Neg, Expr::LitInt(v)) = (op, &**expr) {
                    if i32::try_from(-(*v as i128)).is_ok() {
                        // The suffix still applies: a small negative number
                        // inside a constant written wide is written wide too,
                        // or the operands of one sum disagree.
                        let wide = match flow.widen {
                            true => "i64",
                            false => "",
                        };
                        out.push(&format!("-{v}{wide}"));
                        return Ok(());
                    }
                }
                out.push(unary_op(*op));
                self.nested(out, expr, u8::MAX, depth, flow)?;
            }
            Expr::Binary {
                op,
                lhs,
                rhs,
                span: at,
            } => {
                // **A constant sum takes the first type that holds it**, the
                // way a constant does
                // ([ADR-063](../../../docs/specification/adr/adr-063.md) D1).
                // Asked only on the outermost expression that folds - `widen`
                // reaching inward is what makes the operands agree - and never
                // where the position decides the type.
                let flow = match flow.widen || flow.inferred {
                    true => flow,
                    false => match crate::fold::constant_of(expr, &crate::fold::nothing_is_known) {
                        Some(folded) if crate::fold::wants_widening(&folded) => flow.widened(),
                        _ => flow,
                    },
                };
                // **A `+` that joins text is a call**
                // ([ADR-081](../../docs/specification/adr/adr-081.md) D2).
                // Rust's operator takes `String + &str` and nothing else, so
                // three of the four shapes a program can write went below and
                // were refused there. `concat::plus` has an impl per shape and
                // the language below picks, which is `index::at`'s arrangement
                // a third time - and it keeps `String + &str` exactly as fast
                // as it is today, which a `format!` for every shape would not.
                //
                // **Only where the checker said so**, by the operator's own
                // span: a number's `+` may not move into `std`, where
                // `overflow-checks` is off and inlining would not carry the
                // abort across (ADR-043 D1).
                if self.concatenations.contains(&at.start) {
                    out.push("nikaia_std::concat::plus(");
                    self.expr(out, lhs, depth, flow)?;
                    out.push(", ");
                    self.expr(out, rhs, depth, flow)?;
                    out.push(")");
                    return Ok(());
                }
                // Parenthesised only where precedence needs it: the operators
                // mean the same in both languages, so `value * 10 + n` should
                // come out the way it went in.
                let here = precedence(*op);
                self.nested(out, lhs, here, depth, flow)?;
                out.push(&format!(" {} ", binary_op(*op)));
                self.nested(out, rhs, here + 1, depth, flow)?;
            }
            // **A fallback that jumps is a `match` and not a closure**
            // ([ADR-138](../../../docs/specification/adr/adr-138.md) D1, and
            // [ADR-084](../../../docs/specification/adr/adr-084.md) D4's rule
            // about what a jump may not cross).
            //
            // `unwrap_or_else` takes a **closure**, so a `return` written in
            // the fallback would return from the closure and the program would
            // carry on — `let user = find(id) ?? throw NotFound(id)`, the shape
            // this record's own example is written in, would have thrown
            // nothing and bound a value nobody produced. The `match` puts the
            // jump in the function it was written in.
            Expr::Coalesce { value, fallback } if jumps(fallback) => {
                out.push("match ");
                self.expr(out, value, depth, flow)?;
                out.push(" { Some(__nikaia_value) => __nikaia_value, None => ");
                self.expr(out, fallback, depth, flow)?;
                out.push(" }");
            }
            Expr::Coalesce { value, fallback } => {
                // Kap 3.5. `into()` because the fallback is written as the
                // value it stands for, not as the type the option holds -
                // `text ?? "none"` on a `String?` is the case it exists for.
                //
                // **Except for a number**, and that exception is Part I 2.4
                // rather than a special case: a number literal takes the type
                // its use asks for, so `0` already *is* whatever the option
                // holds and `0.into()` adds a conversion that has to be
                // resolved. Where the option's own type is still open - a
                // `channel::bounded` whose element type nothing has pinned yet
                // ([ADR-149](../../docs/specification/adr/adr-149.md)) - there
                // is nothing to resolve it from, and what a reader got was
                // *"type annotations needed"* about a file nobody wrote
                // (Part III, C.1). Without the `into` the literal is simply
                // one more use of the type, which is what decides it.
                let bare = a_number(fallback);
                self.expr(out, value, depth, flow)?;
                out.push(".unwrap_or_else(|| ");
                self.expr(out, fallback, depth, flow)?;
                out.push(match bare {
                    true => ")",
                    false => ".into())",
                });
            }
            Expr::TryCatch { expr, handler } => {
                // Kap 7.1: the handler sees the error as `error`.
                out.push("match ");
                // `catch` needs the `Result`, not the value: a `dsl … from …`
                // propagates on its own everywhere else, and here the handler
                // is what handles it.
                // A written call inside the guarded half must not take the
                // `?`: the `match` below is what handles the failure. A grammar
                // entry is one of those calls now
                // ([ADR-082](../../docs/specification/adr/adr-082.md) D1), so it
                // needs no arm of its own — which is what that record meant by
                // *an ordinary call*.
                self.expr(out, expr, depth, flow.guarded())?;
                let pad = "    ".repeat(depth + 1);
                let close = "    ".repeat(depth);
                // **A handler that does not read the error binds `_error`**
                // ([ADR-090](../../docs/specification/adr/adr-090.md)). Kap 7.1
                // gives the failure that name whether or not the handler wants
                // it, so `catch { 1000 }` - the shape Part I 7.1 teaches first -
                // used to get `warning: unused variable: error` about a binding
                // that exists nowhere in the program, which is
                // [Part III C.1](../../docs/specification/30-nikaia-tooling.md)
                // one severity down.
                //
                // The question is `contracts::order`'s, already over-approximate
                // and already asked of this very block for the ordering: a false
                // *yes* keeps today's binding, and a false *no* would not
                // compile.
                let bound =
                    match crate::contracts::order::block_mentions(self.parsed, handler, CAUGHT) {
                        true => CAUGHT,
                        false => "_error",
                    };
                // **What the handler was handed**
                // ([ADR-157](../../docs/specification/adr/adr-157.md) D2). A
                // named channel hands it an **envelope** — the author's error
                // plus the site it was raised at — and what Part I 7.1 gives
                // the handler is the error: `match error { ConfigError::… }`
                // names variants, and `{error}` is the message the author
                // wrote. So the envelope is opened **once, at the binding**,
                // and the site is kept in a local beside it for the one
                // statement that needs it back (`throw error`, D3).
                let named = bound == CAUGHT && self.caught_channel_is_named(expr);
                let flow = flow.handling(named);
                let opened = match named {
                    false => String::new(),
                    true => format!("{{ let ({CAUGHT}, {SITE}) = {CAUGHT}.split(); "),
                };
                out.push(&format!(
                    " {{\n{pad}Ok(value) => value,\n{pad}Err({bound}) => {opened}"
                ));
                // Kap 7.1 and ADR-034: the handler's last statement is the
                // value of the `catch`, so a `return` in it is the *function's*
                // return and has to stay one. `contracts::order`'s `diverts`
                // counts exactly that `return` when it refuses to overlap the
                // guarded read, so dropping it here made the ordering analysis
                // reason about a control flow the emitted program did not have.
                self.block(out, handler, depth + 1, flow, Tail::Value)?;
                if named {
                    out.push(" }");
                }
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
            // **The four jumps, written as the language below writes them**
            // ([ADR-138](../../../docs/specification/adr/adr-138.md) D1): each
            // is `!` there too, so an arm that returns beside an arm that hands
            // back a value is a `match` of that value's type by the backend's
            // own rule and not by a coercion this had to write.
            Expr::Return(value) => match (value, flow.throws) {
                (Some(value), true) => {
                    out.push("return Ok(");
                    self.nullable(out, value, &(0..0), depth, flow)?;
                    out.push(")");
                }
                (Some(value), false) => {
                    out.push("return ");
                    self.nullable(out, value, &(0..0), depth, flow)?;
                }
                (None, true) => out.push("return Ok(())"),
                (None, false) => out.push("return"),
            },
            Expr::Break | Expr::Continue => {
                let word = match expr {
                    Expr::Break => "break",
                    _ => "continue",
                };
                // **The same refusal the statement form has**
                // ([ADR-084](../../../docs/specification/adr/adr-084.md) D6):
                // the lowering states it too, so a path that reached here
                // without the checker's answer still writes no file `rustc`
                // would refuse.
                if !flow.in_loop {
                    return Err(refused!(
                        "`{word}` has no loop to act on here, and the language below \
                         would refuse the file this writes (Part I, 3.3)"
                    ));
                }
                out.push(word);
            }
            Expr::Throw(inner) => {
                // **`throw error` inside a handler passes the error on**
                // ([ADR-157](../../docs/specification/adr/adr-157.md) D3), with
                // the site it was **raised** at and not the one it was caught
                // at, which is what
                // [ADR-023](../../docs/specification/adr/adr-023.md) D6 asks
                // for. The handler's binding opened the envelope; this is where
                // it is closed again.
                let passing_on = flow.caught_named
                    && matches!(inner.as_ref(), Expr::Variable(name) if self.text(*name) == CAUGHT);
                if passing_on {
                    out.push(&format!("return Err({SITE}.refill({CAUGHT}))"));
                    return Ok(());
                }
                // Three channels and three ways in. The **envelope** takes the
                // value and the site; the **box** takes it boxed, which is what
                // `raise` does and what needs `'static`; and a **library's
                // type** takes the value alone, because a channel that carries
                // no envelope has nowhere to put a site
                // ([ADR-159](../../docs/specification/adr/adr-159.md) D2).
                match self.named_error(flow.function) {
                    Some(Named::Library(_)) => {
                        out.push("return Err(");
                        self.expr(out, inner, depth, flow)?;
                        out.push(")");
                        return Ok(());
                    }
                    Some(Named::Own(_)) => out.push("return Err(nikaia_std::error::throwing("),
                    None => out.push("return Err(nikaia_std::error::raise("),
                }
                self.expr(out, inner, depth, flow)?;
                out.push(&format!(", {:?}))", flow.origin));
            }
            // **Part I 8.2 and ADR-055 D5: a task.**
            //
            // `spawn fn { … }` is `TaskHandle::start(async move { … })`. Three
            // things are in that one line and each is a decision somewhere
            // else:
            //
            //   * an `async` **block** and not a closure, because the body may
            //     pause and Rust has no stable `async` closure - the same
            //     reason every vehicle in `task` takes futures (§6 step 3);
            //   * `move`, which is Part I 8.3's implicit move: the captures go
            //     with the task because it may outlive the function that
            //     started it, and `NK2101` is what stands in front of that for
            //     data the parent still wanted;
            //   * a **handle**, always, whether or not the program keeps it. A
            //     task nobody joins still runs (D5), because the executor owns
            //     it - so an unused handle is a value dropped and not work
            //     cancelled.
            //
            // The lambda's `params` are not written: a task is handed nothing,
            // so a `spawn fn (x) { … }` has no argument to bind. `check`
            // refuses that rather than dropping it silently.
            Expr::Spawn { body, .. } => {
                let Expr::Closure { body, .. } = body.as_ref() else {
                    return Err(refused!("`spawn` takes a lambda: write `spawn fn {{ … }}`"));
                };
                // **And which of the two starters**, which is the one place
                // `user_parallelism` reaches a `spawn` (ADR-037 D2). At `yes` a
                // task may be polled on a thread that did not start it, so its
                // future has to be `Send` (§2 D6); at `no` it may not, so it
                // does not - and asking for `Send` there would refuse a task
                // holding the plain count ADR-061 D1 gives a `Shared` at one
                // user thread. One Nikaia line, two lowerings, and the switch
                // is what chooses.
                // **A handle the body names is duplicated, not moved**
                // ([ADR-040](../../../docs/specification/adr/adr-040.md) D1).
                // The step is written *outside* the future, in a block of its
                // own, so the name the body moves is the new handle and the
                // caller's own survives the `spawn` - which is the whole of D1
                // for the task half. Unconditional (D2): a line further down may
                // not decide what a line further up does to a cleanup point.
                let handles: Vec<&str> = self
                    .task_handles
                    .iter()
                    .filter(|(at, _)| *at == flow.statement)
                    .map(|(_, name)| name.as_str())
                    .collect();
                if !handles.is_empty() {
                    out.push("{ ");
                    for name in &handles {
                        out.push(&format!("let {name} = {name}.clone(); "));
                    }
                }
                match self.build.overlaps_user_code() {
                    true => out.push("nikaia_std::task::TaskHandle::start_on_pool(async move "),
                    false => out.push("nikaia_std::task::TaskHandle::start(async move "),
                }
                // `Flow::PLAIN`, like any other lambda body: a `return` inside
                // the task leaves the task, and a failure in it is the task's.
                // `in_lambda` is deliberately **not** set - the body is a
                // future, so a call that pauses is exactly what it may hold.
                let inside = Flow {
                    statement: flow.statement,
                    ..Flow::PLAIN
                };
                self.block(out, body, depth, inside, Tail::Return)?;
                out.push(")");
                if !handles.is_empty() {
                    out.push(" }");
                }
            }
            other => return Err(refused!("cannot emit expression yet: {other:?}")),
        }
        Ok(())
    }

    /// Whether `receiver.method(…)` **enters a grammar**: a receiver naming one
    /// of this file's grammars and a method naming one of its `pub` rules
    /// ([ADR-082](../../docs/specification/adr/adr-082.md) D1, D2).
    fn enters_a_grammar(&self, receiver: &Expr, method: Symbol) -> bool {
        let Expr::Variable(name) = receiver else {
            return false;
        };
        self.grammars
            .get(name)
            .is_some_and(|def| def.rules.iter().any(|r| r.is_public && r.name == method))
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
        // ADR-055 D6: a recursive `async fn` is an infinitely sized future, and
        // Rust says so rather than guessing - so a call that closes a cycle
        // through pausing functions puts the future behind a pointer.
        // `pausing_reach` is computed once per unit (see that function for why
        // it over-approximates), and the emitter only looks names up in it: it
        // still resolves nothing (ADR-011 D2).
        // **A grammar is entered by an ordinary call**
        // ([ADR-082](../../docs/specification/adr/adr-082.md) D1), through a
        // **path** since [ADR-140](../../docs/specification/adr/adr-140.md) D3:
        // `Json::value(input)`, where `Json` names a grammar in this file and
        // `value` one of its `pub` rules. First, because it is not a call to
        // anything the recursion graph or the ledger knows.
        if let Expr::Path(segments) = func {
            if let [grammar, rule] = segments.as_slice() {
                if self.grammars.contains_key(grammar) && args.len() == 1 {
                    return self.grammar_entry(out, *grammar, *rule, &args[0], depth, flow);
                }
            }
        }
        let pausing = self.pausing_key(func);
        if let Some(key) = pausing.as_deref().filter(|_| flow.in_lambda) {
            return Err(pausing_in_a_lambda(key));
        }
        // **A call to a code parameter whose type may pause**
        // ([ADR-122](../../docs/specification/adr/adr-122.md) D1). It hands back
        // a boxed future, so it is awaited — and it is never *boxed* here: a
        // parameter is not a name the recursion graph knows, and what it holds
        // is already behind a pointer.
        let awaits_a_parameter =
            matches!(func, Expr::Variable(name) if flow.awaited.contains(name));
        let boxed = pausing
            .as_deref()
            .is_some_and(|key| self.closes_a_pausing_cycle(flow.function, key));
        if boxed {
            out.push("Box::pin(");
        }

        // **A handle is made at the call and nowhere else**
        // ([ADR-155](../../docs/specification/adr/adr-155.md) D2, D3). The
        // declaration hands back the address C returned; the hull goes on here,
        // where the claim the declaration made can be checked — or, where it
        // said `?`, where `None` is C's own `NULL`.
        let hull = self.handle_from(func);
        if let Some((handle, absent)) = &hull {
            out.push(&match absent {
                true => format!("{handle}::maybe("),
                false => format!("{handle}::from_c(\"{}\", ", self.called_name(func)),
            });
        }

        self.called(out, func, args, config, depth, flow)?;

        if hull.is_some() {
            out.push(")");
        }

        if boxed {
            out.push(")");
        }

        // **ADR-055 D2: the `.await` goes before the `?`**, and the order is not
        // a choice: the future is what can fail, so it has to be driven before
        // there is a `Result` to propagate. `f().await?` and never `f()?.await`.
        if pausing.is_some() || awaits_a_parameter {
            out.push(".await");
        }

        // ADR-023 D8: `throws` propagates on its own, so a call to something
        // that can fail is where the failure leaves - and in the language below
        // that is spelled `?`. This is the written-call twin of the `?` a
        // fallible loop's step gets above: the same rule, one line earlier
        // (ADR-025 D1).
        //
        // Three conditions, and each removes a way of being wrong. The
        // enclosing function must be `throws`, or there is nowhere for the `?`
        // to go - and `NK2605` has already refused the program where it is
        // not, so this is not a silent choice. The call must not be the
        // guarded half of a `catch`, which wants the `Result` itself. And the
        // callee's contract must **say** it can fail, so nothing is added on a
        // guess: a call no ledger describes is left exactly as it was.
        //
        // A lambda's body is emitted with `Flow::PLAIN`, so a fallible call
        // inside one never takes a `?` from the function around it - which is
        // right, because its `return` leaves the lambda and not the function.
        if flow.throws && !flow.caught && self.can_fail(func) {
            out.push("?");
        }
        Ok(())
    }

    /// The call itself, without ADR-023 D8's propagation.
    fn called(
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

            // **A door over several locks**
            // ([ADR-065](../../../docs/specification/adr/adr-065.md)): the locks
            // go in by reference and the block becomes the last argument, which
            // is what the trailing-lambda rule already made of it. The ordered
            // acquisition is `std`'s, because it is about addresses at run time
            // and not about anything this compiler can see.
            if let Some(door) = crate::check::MultiLock::named(text) {
                if let Some((block, locks)) = args.split_last() {
                    out.push(&format!("nikaia_std::lock::{}(", door.written()));
                    for lock in locks {
                        out.push("&");
                        self.expr(out, lock, depth, flow)?;
                        out.push(", ");
                    }
                    self.expr(out, block, depth, flow)?;
                    out.push(")");
                    return Ok(());
                }
            }

            // **A hull you can observe, you write**
            // ([ADR-064](../../../docs/specification/adr/adr-064.md) D2).
            // `Shared(x)`, `SharedMut(x)` and `Locked(x)` are the three, and each
            // expands to the shape `written_name` would give the *type* - so the
            // constructor and the annotation cannot disagree about a value.
            if let [held] = args {
                if let Some(hulls) = self.hull_new(text, self.count_at(flow.function, flow.bound)) {
                    for path in &hulls {
                        out.push(&format!("{path}::new("));
                    }
                    self.expr(out, held, depth, flow)?;
                    for _ in &hulls {
                        out.push(")");
                    }
                    return Ok(());
                }
            }

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
                self.args(out, text, args, &[], depth, flow)?;
                out.push(")");
                return Ok(());
            }

            if self.structs.contains(name) {
                out.push(&format!("{text}::new("));
                let takes = self.takes_a_handle(&format!("{text}::new"));
                self.args(out, text, args, &takes, depth, flow)?;
                out.push(")");
                return Ok(());
            }

            // **`std`'s own types are constructed the same way**
            // ([ADR-140](../../docs/specification/adr/adr-140.md) D2). `Vec()`
            // is the anonymous constructor and `Vec::new()` is what the
            // language below calls it — the same two spellings the arm above
            // reconciles for a type this file declares, one ledger over.
            //
            // **Read off the library rather than from a list of three names**,
            // because `std` gaining a type with a constructor should not need
            // an edit here: the entry `Name::new` *is* the fact.
            if self.library.functions.contains_key(&format!("{text}::new")) {
                // **Through `path`**, because one name below depends on more
                // than the name: a **trusted** map is `TrustedMap::default()`
                // and not `TrustedMap::new()`, since `new` exists only for the
                // default hasher ([ADR-010](../../docs/specification/adr/adr-010.md)
                // D5). Writing `{text}::new(` here would have taken that back
                // for every `HashMap()` in a trusted program.
                out.push(&self.path(&[text, "new"]));
                out.push("(");
                let takes = self.takes_a_handle(&format!("{text}::new"));
                self.args(out, text, args, &takes, depth, flow)?;
                out.push(")");
                return Ok(());
            }
        }

        // **The same constructor, written with its module in front**
        // ([ADR-154](../../docs/specification/adr/adr-154.md) D3):
        // `collections::HashMap()` is `HashMap()` reached the way the prelude's
        // list says a name outside it is reached. The arm above answers the bare
        // spelling and this one the qualified, off the same fact — the entry
        // `Name::new`.
        //
        // The module rides along into the emitted Rust, because `std`'s own
        // prelude publishes the module and the name a **trusted** program gets
        // is in it too (`collections::TrustedMap`). What the prefix must not do
        // is reach `map_name`, which answers about a *type*.
        if let Expr::Path(segments) = func {
            if let [module, name] = segments.as_slice() {
                let module = self.text(*module).to_string();
                let name = self.text(*name).to_string();
                // **The key carries the module**, because that is where the
                // ledger keeps a type that lives in one: `collections::HashMap`
                // and its `::new` beside it.
                let key = format!("{module}::{name}::new");
                if self.library.functions.contains_key(&key) {
                    out.push(&format!("{module}::{}", self.path(&[&name, "new"])));
                    out.push("(");
                    let takes = self.takes_a_handle(&key);
                    self.args(out, &key, args, &takes, depth, flow)?;
                    out.push(")");
                    return Ok(());
                }
            }
        }

        self.expr(out, func, depth, flow)?;
        out.push("(");
        let takes = self.takes_a_handle_at(func);
        // The written callee, which is what the checker keyed its answers by —
        // and **a qualified name is one of them**. It used to be a bare name or
        // nothing, on the reasoning that nothing could have been recorded for
        // anything else; the checker keys by the name as written, so
        // `http::route(r)` recorded under `http::route` and was looked up under
        // the empty string. Found by ADR-094 D1's `&` going missing at a call
        // into a package; it was `nullable_args` and `lent_args` both, so a
        // plain value in a **qualified** callee's nullable parameter had been
        // reaching the language below unwrapped.
        let callee = match func {
            Expr::Variable(name) => self.text(*name).to_string(),
            // **Unaliased, because that is what the checker keyed by**:
            // `h::id_of()` is `http::id_of()` where the file wrote
            // `use http as h` ([ADR-046](../../docs/specification/adr/adr-046.md)
            // D3), and a key built from the source's own word never matched.
            Expr::Path(segments) => self.parsed.unaliased(
                &segments
                    .iter()
                    .map(|s| self.text(*s))
                    .collect::<Vec<_>>()
                    .join("::"),
            ),
            _ => String::new(),
        };
        self.args(out, &callee, args, &takes, depth, flow)?;

        if let Expr::Variable(name) = func {
            self.dsl_parameters(out, self.text(*name), args.len(), config, depth, flow)?;
        }

        // Kap 5.1: the language below has no named arguments and no defaults,
        // so the options become positional here, in the order the *declaration*
        // gives - which is the only order there is, and the reason this needs
        // the callee's contract rather than the call alone.
        if let Some(options) = self.options_of(func) {
            for (n, option) in options.iter().enumerate() {
                // **The comma belongs *between* arguments**, and a call whose
                // arguments are all options has nothing before the first one
                // ([ADR-133](../../docs/specification/adr/adr-133.md) D1).
                // `execute(; target_age: 30)` came out as `execute(, 30)` —
                // invalid Rust, reported by `rustc` about a file nobody wrote
                // (Part III C.1), for the only spelling such a call had. Older
                // than the record that names the shape: nothing in the corpus
                // declares a function whose parameters are all options.
                if n > 0 || !args.is_empty() {
                    out.push(", ");
                }
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

    /// Kap 5.1 at a **method** call: the options become positional, in the order
    /// the declaration gives, and one that was left out becomes its default.
    ///
    /// The same thing `call` does from `options_of`, off the checker's answer
    /// instead ([ADR-028](../../../docs/specification/adr/adr-028.md)) — and a
    /// DSL driver is not this: what stands after *its* `;` is the typed spread
    /// of [ADR-007](../../../docs/specification/adr/adr-007.md) D5, which
    /// `dsl_parameters` has already written as one value.
    fn method_options(
        &self,
        out: &mut Out,
        method: Symbol,
        args: usize,
        config: &[crate::ast::ConfigArg],
        depth: usize,
        flow: Flow<'_>,
    ) -> Result<()> {
        let name = self.text(method).to_string();
        if self.dsl_drivers.contains(&name) {
            return Ok(());
        }
        let Some(options) = self.method_options.get(&(flow.statement, name)) else {
            return Ok(());
        };
        for (n, (option, default)) in options.iter().enumerate() {
            // **The comma belongs *between* arguments**, and a method call whose
            // arguments are all options has nothing before the first one
            // ([ADR-133](../../docs/specification/adr/adr-133.md) D1).
            if n > 0 || args > 0 {
                out.push(", ");
            }
            match config.iter().find(|a| self.text(a.name) == option) {
                Some(passed) => self.expr(out, &passed.value, depth, flow)?,
                None => out.push(default),
            }
        }
        Ok(())
    }

    /// Whether a branch of an `overlap` can fail out of itself
    /// ([ADR-050](../../../docs/specification/adr/adr-050.md) D5).
    ///
    /// **The guarded half of a `catch` does not count**, which is the whole
    /// reason this is its own walk rather than [`visit_expr`]: D5 says
    /// per-branch handling is a `catch` inside the branch, so a branch that
    /// catches its own failure hands back a value and fails nothing. The
    /// handler *is* walked — a `throw` in one leaves the branch like any other
    /// failure.
    fn branch_can_fail(&self, value: &Expr, flow: Flow<'_>) -> bool {
        match value {
            Expr::Call { func, args, config } => {
                self.can_fail(func)
                    || args.iter().any(|a| self.branch_can_fail(a, flow))
                    || config.iter().any(|a| self.branch_can_fail(&a.value, flow))
            }
            Expr::MethodCall {
                receiver,
                method,
                args,
                config,
            }
            // A call that may not happen may still fail when it does, so the
            // branch it is in can fail.
            | Expr::SafeMethod {
                receiver,
                method,
                args,
                config,
            } => {
                // **A grammar entry propagates on its own** (Kap 7.1, and
                // [ADR-082](../../docs/specification/adr/adr-082.md) D2's
                // `throws`): it is an ordinary call now, and this is the arm
                // ordinary calls are in — which is what that record meant.
                self.enters_a_grammar(receiver, *method)
                    || self.method_can_fail(flow, *method)
                    || self.branch_can_fail(receiver, flow)
                    || args.iter().any(|a| self.branch_can_fail(a, flow))
                    || config.iter().any(|a| self.branch_can_fail(&a.value, flow))
            }
            // The guarded half is handled here; the handler is not.
            Expr::TryCatch { handler, .. } => handler.stmts.iter().any(
                |stmt| matches!(&stmt.node, Stmt::Expr(value) if self.branch_can_fail(value, flow)),
            ),
            Expr::Throw(_) => true,
            Expr::Try(inner) | Expr::Unary { expr: inner, .. } => self.branch_can_fail(inner, flow),
            Expr::Binary { lhs, rhs, .. } => {
                self.branch_can_fail(lhs, flow) || self.branch_can_fail(rhs, flow)
            }
            Expr::Coalesce { value, fallback } => {
                self.branch_can_fail(value, flow) || self.branch_can_fail(fallback, flow)
            }
            Expr::Field { base, .. } | Expr::SafeField { base, .. } => {
                self.branch_can_fail(base, flow)
            }
            Expr::Index { base, index } => {
                self.branch_can_fail(base, flow) || self.branch_can_fail(index, flow)
            }
            _ => false,
        }
    }

    /// **`overlap { … }`** — Part I 8.1.2,
    /// [ADR-050](../../../docs/specification/adr/adr-050.md) D2 and D6.
    ///
    /// Each statement is a branch, each branch becomes an `async` block, and
    /// `task::overlap<n>` polls all of them in one pass. An `async` **block** and
    /// not a closure, for the reason a task's body is one: a branch may pause,
    /// and Rust has no stable `async` closure. No `move`, because D4 says
    /// nothing outlives the block — a branch borrows what is around it exactly
    /// as an ordinary statement does, and that is what makes the form lighter
    /// than two `spawn`s.
    ///
    /// **D6 is the argument order.** *"A branch is started up to its first
    /// suspension point before any branch that cannot suspend is run"* — so the
    /// branches that can pause are handed over first, and the join polls in the
    /// order it is given. Which those are is the ledger's `sync` column, read
    /// the same way [`Emitter::branch_pauses`] reads it for ADR-033's pairs.
    ///
    /// **And the value goes back into written order**, because D2 says it is the
    /// tuple in written order and the reordering above is a schedule. Where the
    /// two differ the tuple is rebuilt, which costs a move per branch and is
    /// visible in the emitted Rust as the permutation it is.
    fn overlap(&self, out: &mut Out, block: &Block, depth: usize, flow: Flow<'_>) -> Result<()> {
        if block.stmts.len() < 2 {
            return Err(refused!(
                "an `overlap` block needs at least two branches; one statement has \
                 nothing to overlap with (Part I, 8.1.2)"
            ));
        }
        if block.stmts.len() > MOST_BRANCHES {
            return Err(refused!(
                "an `overlap` block of {} branches is more than this compiler builds \
                 ({MOST_BRANCHES}); `std` has one vehicle per arity (ADR-050 D2)",
                block.stmts.len()
            ));
        }

        // D6: the branches that can pause, in written order among themselves,
        // then the ones that cannot. `sort_by_key` is stable, so written order
        // survives inside each half - which is what makes the schedule
        // reproducible rather than merely correct.
        let mut order: Vec<usize> = (0..block.stmts.len()).collect();
        order.sort_by_key(|&at| !self.branch_pauses(&block.stmts[at], flow));

        // **D5: a branch whose failure is uncaught fails the block, and the
        // first in written order wins.** The `?`s below are written in written
        // order, and `?` returns at the first `Err` - so the rule is the
        // language below's own control flow rather than a comparison this
        // compiler makes. Per-branch handling is a `catch` inside the branch,
        // which `branch_can_fail` does not count.
        //
        // All or none: a branch that cannot fail is wrapped in `Ok` too, so the
        // vehicle sees one shape and the `?`s line up. The error type is named
        // rather than inferred, because an `async` block with a `?` in it and
        // nothing to infer from is *"type annotations needed"* about a file
        // nobody wrote (Part III, C.1).
        let fallible = flow.throws
            && block.stmts.iter().any(|stmt| match &stmt.node {
                Stmt::Expr(value) => self.branch_can_fail(value, flow.at(stmt.span.start)),
                _ => false,
            });

        let pad = "    ".repeat(depth);
        let inner = "    ".repeat(depth + 1);
        let reordered = order.iter().enumerate().any(|(at, &from)| at != from);
        let bound = reordered || fallible;

        if bound {
            out.push("{\n");
            out.push(&inner);
            out.push(&format!(
                "// ADR-050 D6: started in this order, answered in the written one.\n{inner}"
            ));
            out.push("let (");
            for (at, _) in order.iter().enumerate() {
                if at > 0 {
                    out.push(", ");
                }
                out.push(&format!("{BRANCH}{at}"));
            }
            out.push(") = ");
        }

        out.push(&format!(
            "nikaia_std::task::overlap{}(\n",
            block.stmts.len()
        ));
        let body = "    ".repeat(depth + 1 + usize::from(bound));
        for &from in &order {
            let stmt = &block.stmts[from];
            out.push(&body);
            out.push("async { ");
            // A branch's own statement, as an expression. `Flow::PLAIN` for the
            // reason a lambda's body gets it: a `return` inside a branch would
            // leave the branch, and `catch` is how D5 says a branch handles its
            // own failure.
            let inside = Flow {
                statement: stmt.span.start,
                throws: fallible,
                origin: flow.origin,
                ..Flow::PLAIN
            };
            match &stmt.node {
                Stmt::Expr(value) => {
                    if fallible {
                        out.push("Ok::<_, Box<dyn std::error::Error>>(");
                    }
                    out.from(&stmt.span, |out| self.expr(out, value, depth + 1, inside))?;
                    if fallible {
                        out.push(")");
                    }
                }
                _ => {
                    return Err(refused!(
                        "a branch of an `overlap` is an expression (Part I, 8.1.2)"
                    ))
                }
            }
            out.push(" },\n");
        }
        out.push(&"    ".repeat(depth + usize::from(bound)));
        out.push(")");

        if bound {
            out.push(&format!(".await;\n{inner}("));
            // Written order out of start order: branch `written` was handed
            // over at `order.iter().position(…)`, so that is the name it came
            // back under. Where nothing was reordered the two are the same, and
            // this still writes the tuple out because the `?`s hang off it.
            for written in 0..block.stmts.len() {
                if written > 0 {
                    out.push(", ");
                }
                let at = order
                    .iter()
                    .position(|&from| from == written)
                    .expect("every branch is handed over exactly once");
                out.push(&format!("{BRANCH}{at}"));
                if fallible {
                    out.push("?");
                }
            }
            out.push(&format!(")\n{pad}}}"));
        } else {
            out.push(".await");
        }
        Ok(())
    }

    /// **`select { … }`** — Part II 12.4,
    /// [ADR-148](../../../docs/specification/adr/adr-148.md) D1 and D2.
    ///
    /// Each arm's expression becomes an `async` block, `task::race<n>` polls all
    /// of them in one pass, and what comes back says **which** arm won. So the
    /// construct lowers to a `match` over a sum, which is what an arm binding a
    /// name and then running a block *is* in the language below.
    ///
    /// **The losers are cancelled, and nothing here writes that** (D2). The
    /// losing futures are dropped when `race<n>` returns; a future dropped at
    /// its suspension point tears its values down, and a `cleanup` that pauses
    /// is adopted by the runtime and bounded by the `cleanup-deadline`
    /// ([ADR-006](../../../docs/specification/adr/adr-006.md) D3). That is the
    /// mechanism this runtime already had, which is the whole argument of D3.
    ///
    /// **A failing arm**, where the enclosing function is `throws`: every arm is
    /// wrapped in `Ok` so the vehicle sees one shape, and the winner is
    /// unwrapped at the top of its own body — `?` there and not at the `match`,
    /// because only the arm that won has a value to propagate.
    fn select(
        &self,
        out: &mut Out,
        arms: &[SelectArm],
        depth: usize,
        flow: Flow<'_>,
    ) -> Result<()> {
        if arms.len() < 2 {
            return Err(refused!(
                "a `select` needs at least two arms; one arm has nothing to race \
                 against (Part II, 12.4)"
            ));
        }
        if arms.len() > MOST_BRANCHES {
            return Err(refused!(
                "a `select` of {} arms is more than this compiler builds \
                 ({MOST_BRANCHES}); `std` has one vehicle per arity (ADR-148 D1)",
                arms.len()
            ));
        }

        // The same all-or-none the `overlap` above writes, and for the same
        // reason: the vehicle sees one shape and the error type is named rather
        // than inferred, because an `async` block with a `?` in it and nothing
        // to infer from is *"type annotations needed"* about a file nobody
        // wrote (Part III, C.1).
        let fallible = flow.throws
            && arms
                .iter()
                .any(|arm| self.branch_can_fail(&arm.value, flow.at(arm.at)));

        let pad = "    ".repeat(depth);
        let inner = "    ".repeat(depth + 1);
        out.push(&format!("match nikaia_std::task::race{}(\n", arms.len()));
        for arm in arms {
            out.push(&inner);
            out.push("async { ");
            // `Flow::PLAIN` for the reason an `overlap` branch gets it: a
            // `return` written inside the *raced expression* would leave that
            // block. What an arm's **body** does is the function's, and the body
            // is emitted below with the flow it was called with.
            let inside = Flow {
                statement: arm.at,
                throws: fallible,
                origin: flow.origin,
                ..Flow::PLAIN
            };
            if fallible {
                out.push("Ok::<_, Box<dyn std::error::Error>>(");
            }
            self.expr(out, &arm.value, depth + 1, inside)?;
            if fallible {
                out.push(")");
            }
            out.push(" },\n");
        }
        out.push(&format!("{pad}).await {{\n"));

        for (at, arm) in arms.iter().enumerate() {
            let bound = match (fallible, arm.binding) {
                (true, _) => WINNER.to_string(),
                (false, Some(name)) => self.text(name).to_string(),
                (false, None) => "_".to_string(),
            };
            out.push(&format!(
                "{inner}nikaia_std::task::Race{}::{}({bound}) => ",
                arms.len(),
                ORDINALS[at]
            ));
            // The winner's `?`, at the top of the arm that won. A `_` arm still
            // propagates, because an arm that ignores a value does not ignore a
            // failure.
            let opening = fallible.then(|| match arm.binding {
                Some(name) => format!("let {} = {WINNER}?;", self.text(name)),
                None => format!("{WINNER}?;"),
            });
            out.from(&Span::from(arm.at..arm.at), |out| {
                self.block_opening_with(
                    out,
                    &arm.body,
                    depth + 1,
                    flow,
                    Tail::Value,
                    opening.as_deref(),
                )
            })?;
            out.push("\n");
        }
        out.push(&format!("{pad}}}"));
        Ok(())
    }

    /// Whether a call from `caller` to `callee` closes a cycle of pausing
    /// functions, and so needs its future behind a pointer (ADR-055 D6).
    ///
    /// **The question is about the call and not about the callee.** A recursive
    /// `countdown` boxes the call it makes to itself; the call `main` makes to it
    /// holds that future exactly once and is finite, so it pays no allocation.
    /// Asking it the other way - boxing every call to anything recursive - was
    /// the first version, and it charged every caller for its callee's shape.
    ///
    /// `A -> A` beside `B -> B` where `B` also calls `A` is what makes "both are
    /// in a cycle" the wrong test: nothing about `B -> A` is recursive. So the
    /// test is that the callee can reach the caller again, which is what closing
    /// a cycle *is*.
    fn closes_a_pausing_cycle(&self, caller: &str, callee: &str) -> bool {
        self.pausing_reach
            .get(callee)
            .is_some_and(|seen| seen.contains(caller))
    }

    /// The same, for a call whose callee this emitter cannot name
    /// ([ADR-055](../../docs/specification/adr/adr-055.md) D6, the method
    /// half).
    ///
    /// `stats.add(5)` names `add` and says nothing about what `stats` is
    /// ([ADR-028](../../docs/specification/adr/adr-028.md)), so the candidates
    /// are **every** pausing key ending in that name — the same widening
    /// `pausing_reach` draws its edges with, asked here at the call. Where the
    /// two readings differ this takes the wider, for that function's own
    /// reason: a box nobody needed costs one allocation, and a box that was
    /// needed and is missing is a program that does not compile.
    fn method_closes_a_pausing_cycle(&self, caller: &str, method: &str) -> bool {
        let suffix = format!("::{method}");
        self.pausing_reach
            .iter()
            .filter(|(key, _)| key.ends_with(&suffix))
            .any(|(_, seen)| seen.contains(caller))
    }

    /// Whether a type named here holds a view, and so carries the input lifetime
    /// wherever it is written (Part II, 10.6).
    ///
    /// Two sources and one answer: the file's own declarations, and the
    /// package's ledger for the types another file declares
    /// ([`Emitter::tethered`]).
    fn borrows(&self, name: Symbol) -> bool {
        self.borrowing.contains(&name) || self.tethered.contains(self.text(name))
    }

    /// Whether a written type **carries** a view: it is one, it names a struct
    /// that holds one, or one of its arguments does
    /// ([ADR-008](../../docs/specification/adr/adr-008.md) D1, D9).
    ///
    /// [`holds_view`] asks the first of the three, off the written type alone,
    /// and that is all a walk without the declarations can do. This one has them:
    /// `Vec[Entry]` carries a view exactly when `Entry` does, and D9's
    /// derivation — *a function with nothing to borrow from can only write
    /// `'static`* — is about the **lifetime** in the signature rather than about
    /// the `&`, so it is this question it wants answered.
    fn carries_a_view(&self, ty: &Type) -> bool {
        ty.is_view || self.borrows(ty.name) || ty.generics.iter().any(|g| self.carries_a_view(g))
    }

    /// The **one error type** a function's failures can be, where the ledger
    /// names exactly one ([ADR-157](../../docs/specification/adr/adr-157.md)
    /// D1).
    ///
    /// `throws` in the source says *that* a function fails; the ledger's set
    /// says *with what*, derived over the call graph
    /// ([ADR-023](../../docs/specification/adr/adr-023.md) D1). Where that set
    /// has exactly one member and the member is a type **this unit declares**,
    /// the failure channel is that type, and a `catch` can match on its
    /// variants — which is Part I 7.1's own example and what a box makes
    /// impossible.
    ///
    /// `None` is the box, and it is the answer for everything else: a `"?"` in
    /// the set is *something this compiler cannot name*, a set with two members
    /// needs the generated sum that is not built, and a type another package
    /// declares is not this unit's to name in a signature.
    fn named_error(&self, key: &str) -> Option<Named<'_>> {
        let throws = &self.own_contracts.functions.get(key)?.throws;
        let [one] = throws.as_slice() else {
            return None;
        };
        if one == crate::contracts::UNNAMED_ERROR {
            return None;
        }
        if self.declared_errors.contains(one) {
            return Some(Named::Own(one.as_str()));
        }
        // **A type a ledger describes is a name too**
        // ([ADR-159](../../docs/specification/adr/adr-159.md) D1). `io::IoError`
        // is not this unit's to declare and is every bit as nameable: `std`
        // publishes it, the generated file reaches it through the prelude, and
        // a program that reads a file has it as its whole set.
        self.library
            .types
            .contains_key(one)
            .then_some(Named::Library(one.as_str()))
    }

    /// The type a function's failures travel in.
    ///
    /// `borrows` is the same question the result position asks
    /// ([ADR-008](../../docs/specification/adr/adr-008.md) D9): an error that
    /// carries a view — `ConfigError::NotFound(path)`, Part I 7.1's own — is a
    /// type with a lifetime, and a function with nothing to borrow from can
    /// only write `'static` for it.
    fn error_channel(&self, key: &str, lifetimes: Lifetimes, borrows: bool) -> String {
        match self.named_error(key) {
            None => "Box<dyn std::error::Error>".to_string(),
            // **A library's error travels bare**
            // ([ADR-159](../../docs/specification/adr/adr-159.md) D2): there is
            // no envelope because there is no `throw` in this program to record
            // the site of. `std` hands the value back as it is, so propagating
            // it is a plain `?` and a `catch` binds what the source names.
            Some(Named::Library(name)) => name.to_string(),
            Some(Named::Own(name)) => {
                let params = match self.borrows_named(name) {
                    false => String::new(),
                    true => match lifetimes == Lifetimes::ELIDED && !borrows {
                        true => format!("<{}>", Lifetimes::STATIC.params),
                        false => format!("<{}>", lifetimes.params),
                    },
                };
                format!("nikaia_std::error::Thrown<{name}{params}>")
            }
        }
    }

    /// [`Emitter::borrows`] for a type named as text rather than as a `Symbol`.
    ///
    /// The ledger's `throws` column carries names, and a name interned by
    /// another parse is not this one's `Symbol`.
    fn borrows_named(&self, name: &str) -> bool {
        self.tethered.contains(name) || self.borrowing.iter().any(|sym| self.text(*sym) == name)
    }

    /// Whether what a `catch` guards fails through a **named** channel.
    ///
    /// Only the outermost call, and only a call by name: a `catch` guards one
    /// call in every program in the tree, and a guess about a shape nobody
    /// writes would be a guess in the lowering. A `false` where the answer was
    /// yes is today's lowering, unchanged.
    fn caught_channel_is_named(&self, expr: &Expr) -> bool {
        let name = match expr {
            Expr::Call { func, .. } => match func.as_ref() {
                Expr::Variable(name) => self.text(*name).to_string(),
                Expr::Path(segments) => segments
                    .iter()
                    .map(|s| self.text(*s))
                    .collect::<Vec<_>>()
                    .join("::"),
                _ => return false,
            },
            Expr::Try(inner) => return self.caught_channel_is_named(inner),
            _ => return false,
        };
        matches!(
            self.named_error(&self.parsed.unaliased(&name)),
            Some(Named::Own(_))
        )
    }

    /// Whether a function of **this program** can pause, and is therefore an
    /// `async fn` ([ADR-055](../../../docs/specification/adr/adr-055.md) D1).
    ///
    /// **The answer is the ledger's `sync` column and nothing computed here.**
    /// [ADR-027](../../../docs/specification/adr/adr-027.md) D1 infers it per
    /// function over the call graph, and its conservatism is already the safe
    /// direction (D2): an unresolved call costs a function its claim, which here
    /// makes it `async` - and an `async fn` that never awaits finishes on its
    /// first poll, while a plain `fn` that needed to pause does not compile.
    fn pauses(&self, key: &str) -> bool {
        self.own_contracts
            .functions
            .get(key)
            .is_some_and(|contract| !contract.sync.is_sync())
    }

    /// Whether a call to a **library** entry pauses (ADR-055 §6 step 3), and
    /// under which key.
    ///
    /// **An exact lookup**, and after
    /// [ADR-150](../../docs/specification/adr/adr-150.md) that is a decision
    /// rather than an accident. A suffix match here — `read` finding
    /// `fs::read` — is the emitter *guessing* at a callee the unit does not
    /// describe, and a unit built from `--input` does not carry the ledger of
    /// the package beside it: `examples/inventory` writes its own `read`, and a
    /// name-for-name fallback put an `.await` on a call to a function that is
    /// not a future. So a `std` function a program writes **bare** is keyed
    /// bare, which `print` and `println` already were, and `sleep` now is.
    ///
    /// The **key** and not a yes, because D6's boxing needs to know which
    /// function is being called.
    ///
    /// Step 2 deliberately did not ask this: a pausing `std` entry blocked its
    /// thread then, so awaiting one would have been awaiting a value. Step 3 is
    /// what made `std`'s pausing entries `async fn`, and this is the line that
    /// reads them.
    fn library_pauses(&self, key: &str) -> Option<String> {
        self.library
            .functions
            .get(key)
            .filter(|contract| !contract.sync.is_sync())
            .map(|_| key.to_string())
    }

    /// Whether a call to this callee carries an `.await` (ADR-055 D2).
    ///
    /// The same resolution [`Emitter::can_fail`] does, one ledger column over:
    /// `throws` becomes a `?` and *can pause* becomes an `.await`, both read off
    /// the callee's contract and neither resolving a name
    /// ([ADR-011](../../../docs/specification/adr/adr-011.md) D2).
    ///
    /// The ledger key a call resolves to, when that key pauses - `None` when the
    /// call takes no `.await` at all.
    ///
    /// D6's boxing needs to know *which* function is being called and not just
    /// that it pauses, so the answer is the name rather than a yes.
    ///
    /// **Both ledgers**, since §6 step 3 made `std`'s own pausing entries
    /// `async fn`. Step 2 asked only this program's own, because a `std` entry
    /// blocked its thread then and awaiting one would have been awaiting a value
    /// rather than a future - which is what let step 2 land on its own.
    fn pausing_key(&self, func: &Expr) -> Option<String> {
        let name = match func {
            Expr::Variable(name) => self.text(*name).to_string(),
            Expr::Path(segments) => segments
                .iter()
                .map(|s| self.text(*s))
                .collect::<Vec<_>>()
                .join("::"),
            _ => return None,
        };
        let name = self.parsed.unaliased(&name);
        // **This unit's own, then its constructors, then `std`'s — and the
        // *first* ledger that has the name is the one that answers.**
        //
        // The chain matters and not only the order: a program with its own
        // `fn read` has a name `std` also has, and falling through once the
        // local entry says *this does not pause* would put an `.await` on a call
        // to a function that is not a future. `can_fail`, one column over, has
        // read its two ledgers this way all along.
        //
        // Kap 4.2's anonymous constructor is the middle link: `Stats(temp)` is a
        // call to the `new` the `impl` provides, so that is the contract to
        // read.
        let constructor = format!("{name}::new");
        let resolved = self
            .own_contracts
            .functions
            .get(&name)
            .map(|contract| (name.clone(), contract))
            .or_else(|| {
                self.own_contracts
                    .functions
                    .get(&constructor)
                    .map(|contract| (constructor.clone(), contract))
            });
        match resolved {
            Some((key, contract)) => (!contract.sync.is_sync()).then_some(key),
            None => self.library_pauses(&name),
        }
    }

    /// Whether a bare name in **value** position is a type's anonymous
    /// constructor ([ADR-140](../../docs/specification/adr/adr-140.md) D2).
    ///
    /// A type this file declares, or one the library publishes with a `::new`
    /// entry — read off the ledger rather than from a list of three names, for
    /// the reason the call path gives.
    fn constructs_by_name(&self, name: &str) -> bool {
        if self.structs.iter().any(|s| self.text(*s) == name) {
            return self
                .own_contracts
                .functions
                .contains_key(&format!("{name}::new"));
        }
        self.library.functions.contains_key(&format!("{name}::new"))
    }

    /// Whether the contracts say a call by name can fail (Kap 7.1).
    ///
    /// The same resolution `options_of` uses, and for the same reason: the
    /// callee's contract is the fact, and this unit's own comes before `std`'s
    /// because a local name shadows nothing in a library. `false` where the
    /// callee cannot be resolved - which is the emitter *not guessing* rather
    /// than an answer, and is safe here because the checker has already
    /// reported every resolvable fallible call the function did not declare
    /// (`NK2605`); a call nothing describes reaches `rustc` as before.
    ///
    /// A **method** call is not asked here, because the answer is not the
    /// emitter's to work out: `stats.add(5)` names `add` and only the type
    /// checker knows what it goes to (ADR-028). `method_can_fail` reads the
    /// answer the checker handed over instead.
    fn can_fail(&self, func: &Expr) -> bool {
        let name = match func {
            Expr::Variable(name) => self.text(*name).to_string(),
            Expr::Path(segments) => segments
                .iter()
                .map(|s| self.text(*s))
                .collect::<Vec<_>>()
                .join("::"),
            _ => return false,
        };
        self.own_contracts
            .functions
            .get(&name)
            .or_else(|| self.own_contracts.functions.get(&format!("{name}::new")))
            .or_else(|| self.library.lookup(&name).map(|(_, c)| c))
            .is_some_and(|contract| !contract.throws.is_empty())
    }

    /// Whether the checker said this conversion narrows, and which kind it is.
    ///
    /// A lookup, not an analysis: `as i32` narrows or widens by what it is given,
    /// and nothing here knows that (ADR-028). `None` where the checker had no
    /// answer - a type it does not recognise, or two conversions to the same type
    /// in one statement where one of them widens - which leaves the conversion
    /// exactly as it was rather than guessing at it.
    fn narrows(&self, statement: usize, into: &str) -> Option<crate::check::Narrowing> {
        self.narrowing_casts
            .get(&(statement, into.to_string()))
            .copied()
    }

    /// Whether the checker said this method call can fail (Kap 7.1).
    ///
    /// A lookup, not an analysis: `fallible_methods` was computed by the one
    /// type checker this compiler has, against the same ledgers the program is
    /// built with, and nothing here resolves a receiver type. `false` where the
    /// checker had no answer - an unresolved receiver, a method no ledger
    /// describes, or two calls in one statement writing the same name where one
    /// of them cannot fail - which is the emitter leaving the call exactly as it
    /// was rather than guessing at it.
    fn method_can_fail(&self, flow: Flow<'_>, method: Symbol) -> bool {
        self.fallible_methods
            .contains(&(flow.statement, self.text(method).to_string()))
    }

    /// Whether the checker said this method call pauses (ADR-055 D2).
    ///
    /// [`Emitter::method_can_fail`]'s twin, and a lookup for the same reason:
    /// the answer was computed once by the one type checker this compiler has,
    /// and `false` here is the emitter leaving the call as it was rather than
    /// guessing at it.
    fn method_pauses(&self, flow: Flow<'_>, method: Symbol) -> bool {
        self.pausing_methods
            .contains(&(flow.statement, self.text(method).to_string()))
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
                .map_err(|e| refused!("in the interpolated `{{{hole}}}`: {e}"))?;
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
            // Rust's own catch-all, which is what the arm was before
            // ([ADR-145](../../docs/specification/adr/adr-145.md) D3).
            MatchPattern::Otherwise => out.push("_"),
            MatchPattern::Literal(value) => self.expr(out, value, depth, flow)?,
            MatchPattern::Path(p) => out.push(&path(p)),
            // **The parts are patterns**, so this recurses — which is the whole
            // of what *nested* means
            // ([ADR-137](../../docs/specification/adr/adr-137.md) D1). An empty
            // path is the bare tuple `(0, 0)`, which names no type.
            MatchPattern::Tuple { path: p, parts } => {
                out.push(&path(p));
                out.push("(");
                for (i, part) in parts.iter().enumerate() {
                    if i > 0 {
                        out.push(", ");
                    }
                    self.match_pattern(out, part, depth, flow)?;
                }
                out.push(")");
            }
            MatchPattern::Named {
                path: p,
                bindings,
                rest,
            } => {
                let inside = match (bindings.is_empty(), rest) {
                    (true, _) => "..".to_string(),
                    (false, true) => format!("{}, ..", names(bindings)),
                    (false, false) => names(bindings),
                };
                out.push(&format!("{} {{ {inside} }}", path(p)));
            }
            MatchPattern::Or(alternatives) => {
                for (i, alternative) in alternatives.iter().enumerate() {
                    if i > 0 {
                        out.push(" | ");
                    }
                    self.match_pattern(out, alternative, depth, flow)?;
                }
            }
            // **`..=`, because a pattern's range includes both ends** (D3) and
            // that is how the language below spells one. The two spellings mean
            // the same set; one of them is this language's and the other is
            // Rust's.
            MatchPattern::Range { start, end } => {
                self.expr(out, start, depth, flow)?;
                out.push("..=");
                self.expr(out, end, depth, flow)?;
            }
        }
        Ok(())
    }
    /// **A method call, for both of the ways a program may write one.**
    ///
    /// `x.m(…)` and `x?.m(…)` differ in *whether* the call happens and in
    /// nothing else ([ADR-066](../../../../docs/specification/adr/adr-066.md)),
    /// so what a call lowers to is written once: the truncating conversion, the
    /// `len` that becomes an `i64`, `collect`'s turbofish, the arguments, Kap
    /// 5.1's deferred parameters, ADR-055 D2's `.await` and ADR-023 D8's `?`.
    ///
    /// `receiver` is `None` for the safe form, where the receiver is the name
    /// the enclosing `match` already bound - so the call is written on
    /// [`REACHED`] rather than on an expression, and the value is reached
    /// exactly once whatever the receiver cost to produce.
    ///
    /// Eight arguments, and each is a part of the call this has to write: the
    /// receiver, the name, the positional arguments, Kap 5.1's zone, and the
    /// three the emitter carries everywhere (`out`, `depth`, `flow`). Splitting
    /// them into a struct would name a shape the AST already has.
    #[allow(clippy::too_many_arguments)]
    fn method_call(
        &self,
        out: &mut Out,
        receiver: Option<&Expr>,
        method: Symbol,
        args: &[Expr],
        config: &[crate::ast::ConfigArg],
        depth: usize,
        flow: Flow<'_>,
    ) -> Result<()> {
        // **`x.truncating_i32()` is Rust's `as`** (ADR-043 D7): the
        // operation the name says. It is a name and not the operator
        // because keeping the low digits is said rather than assumed,
        // exactly as wrapping is (D2) - and since D4 made `as i32`
        // checked, this is the only way left to ask for truncation.
        //
        // Bare, and parenthesised by whoever needs it: `as` sits between
        // the unary operators and the binary ones in Rust's precedence
        // while a method call sits above all of them, so
        // `-x.truncating_i32()` and `x.truncating_i32().abs()` both need
        // the conversion to happen first. [`Emitter::emits_as_cast`] is
        // where that is decided, for the same reason a written `as`
        // decides it there - **a parenthesis nobody needs is a warning
        // about the generated file** (`let n = (big as i32);` is
        // "unnecessary parentheses around assigned value"), and Part III
        // C.1 says a reader must not meet one.
        // **A grammar is entered by an ordinary call**
        // ([ADR-082](../../docs/specification/adr/adr-082.md) D1):
        // `Json.value(input)`, where `Json` names a grammar in this file and
        // `value` one of its `pub` rules. It is a method call in the grammar
        // of this language and nothing else could have been — a grammar name is
        // not a value, so there is no receiver to resolve and no ambiguity to
        // settle.
        if let Some(Expr::Variable(name)) = receiver {
            if self.grammars.contains_key(name) && args.len() == 1 {
                // **The dot is gone** (ADR-140 D3). `NK1147` is what a program
                // meets; this is here so that a caller who lowers without
                // checking gets the same sentence rather than a method call on
                // a name that is not a value.
                return Err(refused!(
                    "a rule of grammar `{}` is reached with `::`, not with a dot \
                     (ADR-140 D3): write `{}::{}(…)`",
                    self.text(*name),
                    self.text(*name),
                    self.text(method)
                ));
            }
        }
        if let Some(into) = truncating(self.text(method)) {
            self.receiver(out, receiver, depth, flow, true)?;
            out.push(&format!(" as {into}"));
            return Ok(());
        }
        // **`len` hands back an `i64`** (ADR-048 D1), and Rust's hands
        // back a `usize`. Parenthesised where it has to be and nowhere
        // else, exactly as the conversion above is.
        //
        // A name and not a rule, because what it encodes is a fact about
        // *Rust's* library rather than about this language: four entries
        // in `std.contracts` return a length, `len` is what all four are
        // called, and `the_four_lengths_are_i64` in `tests/contracts.rs`
        // is what keeps the two from drifting apart.
        // **`xs.drain()` is the written form of taking the elements away**
        // ([ADR-094](../../docs/specification/adr/adr-094.md) D4). Since a
        // `for` lends, a body that hands an element to a callee which *keeps*
        // it has to say so — and what it says is this. Below, that is
        // `into_iter`: D4's words are *"removing a name from scope"*, which is
        // consuming the container rather than emptying one somebody still
        // holds, and Rust spells the first `into_iter` and the second `drain`.
        //
        // A name and not a rule, for `len`'s reason one paragraph down: it
        // encodes a fact about *Rust's* library rather than about this
        // language.
        // **D5's one door for a stamped value**
        // ([ADR-111](../../docs/specification/adr/adr-111.md)).
        // `kasse.set(neu; after: stand)` is written `set_after(neu, stand)`:
        // the compare and the store happen while the lock is open **once**,
        // which is the whole of what the door is for and what a `get` followed
        // by a `set` cannot do.
        //
        // Two questions and not one. The type checker's, that this receiver is
        // a lock (`witnessed_sets`, ADR-028); and this file's, that the `after:`
        // is written *here* — because the set is keyed by the statement, and a
        // statement may hold a second `set` that carries none.
        let witness =
            match self.text(method) == "set" && self.witnessed_sets.contains(&flow.statement) {
                true => config
                    .iter()
                    .find(|a| self.text(a.name) == "after")
                    .map(|a| &a.value),
                false => None,
            };
        let written = match self.text(method) {
            "drain" if args.is_empty() => "into_iter",
            "set" if witness.is_some() => "set_after",
            other => other,
        };
        let length = is_length(self.text(method), args);
        // **ADR-055 D6, the method half.** A recursive `async fn` is an
        // infinitely sized future, and a call that closes a cycle of pausing
        // functions puts it behind a pointer. `call` has asked this since D6's
        // first sharp edge; a *method* could not be asked, because the emitter
        // cannot name its callee — `stats.add(5)` names `add` and only the type
        // checker knows what it goes to (ADR-028). It does not have to name it:
        // `pausing_reach` already draws an edge to **every** pausing method of
        // that name, which is the over-approximation its own note describes, so
        // the same widening answers the question here. Boxing a call that did
        // not need it costs one allocation; missing one is `rustc`'s *recursion
        // in an async fn requires boxing*, about a file nobody wrote (C.1).
        let boxed = self.method_closes_a_pausing_cycle(flow.function, self.text(method));
        if boxed {
            out.push("Box::pin(");
        }
        self.receiver(out, receiver, depth, flow, false)?;
        out.push(&format!(".{written}"));
        // Nikaia's `collect` builds a List; Rust's needs to be told
        // what to build, and with no types here that is `Vec<_>`.
        if self.text(method) == "collect" && args.is_empty() {
            out.push("::<Vec<_>>");
        }
        out.push("(");
        let takes = self.takes_a_handle(self.text(method));
        self.args(out, self.text(method), args, &takes, depth, flow)?;
        match witness {
            // The witness is an **argument** of the door and not an option of
            // it, so it is written where the signature puts it — after the
            // value, and never through `dsl_parameters`.
            //
            // **The `&` is written here rather than looked up**, and that is
            // the one place this file decides a reference for itself. `lends`
            // withholds its claim on every *method* argument, because the
            // emitter cannot resolve a receiver
            // ([ADR-028](../../docs/specification/adr/adr-028.md)) — and this
            // position needs no resolving: the checker has already said this
            // call is the door, and the door's witness is `seen: &$T` in the
            // ledger, always. A written `&` is `NK1137` before it gets here.
            Some(seen) => {
                out.push(", &");
                // Parenthesised exactly where a postfix would be: `&` binds
                // tighter than every binary operator, so `&a + b` is `(&a) + b`
                // and a witness that is an expression would mean something
                // else. A name gets no parentheses, which is where every
                // witness anybody writes lands.
                self.postfix_base(out, seen, depth, flow)?;
            }
            None => {
                self.dsl_parameters(out, self.text(method), args.len(), config, depth, flow)?;
                self.method_options(out, method, args.len(), config, depth, flow)?;
            }
        }
        out.push(")");

        // ADR-023 D8, the method half. The same three conditions the
        // call by name is given in `call` below, and the same rule -
        // only the last question is asked of a different source,
        // because the emitter cannot ask it itself. The enclosing
        // function must be `throws`, or there is nowhere for the `?` to
        // go, and `NK2605` has already refused the program where it is
        // not. The call must not be the guarded half of a `catch`,
        // which wants the `Result`. And the callee must be one the
        // checker established can fail.
        // ADR-055 D2, the method half, and **before the `?`** for the
        // reason `call` gives: the future is what can fail, so it has
        // to be driven before there is a `Result` to propagate.
        // **Before the `.await`**, because it is the future that is boxed and
        // not what awaiting it comes to.
        if boxed {
            out.push(")");
        }

        if self.method_pauses(flow, method) {
            if flow.in_lambda {
                return Err(pausing_in_a_lambda(self.text(method)));
            }
            out.push(".await");
        }

        if flow.throws && !flow.caught && self.method_can_fail(flow, method) {
            out.push("?");
        }
        if length {
            out.push(" as i64");
        }
        Ok(())
    }

    /// The receiver of a [`Emitter::method_call`]: the expression, or the name
    /// a `?.` already bound.
    ///
    /// `tight` is for the one caller that needs the conversion to bind tighter
    /// than everything around it - `x.truncating_i32()` is Rust's `as`, which
    /// sits below the unary operators in its precedence.
    fn receiver(
        &self,
        out: &mut Out,
        receiver: Option<&Expr>,
        depth: usize,
        flow: Flow<'_>,
        tight: bool,
    ) -> Result<()> {
        match (receiver, tight) {
            (Some(expr), true) => self.nested(out, expr, u8::MAX, depth, flow),
            (Some(expr), false) => self.postfix_base(out, expr, depth, flow),
            (None, _) => {
                out.push(REACHED);
                Ok(())
            }
        }
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
        let parenthesise = self.emits_as_cast(expr)
            || matches!(
                expr,
                Expr::Binary { .. }
                    | Expr::Unary { .. }
                    | Expr::Range { .. }
                    | Expr::If { .. }
                    | Expr::Match { .. }
                    // It lowers to a `match`, so it needs what a `match` needs.
                    | Expr::SafeMethod { .. }
                    | Expr::Block(_)
                    | Expr::Unsafe(_)
                    | Expr::Closure { .. }
                    | Expr::TryCatch { .. }
                    | Expr::Dsl { .. }
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

    /// **Whether this comes out as a Rust `as` conversion**, whatever it was
    /// written as.
    ///
    /// Three things do: a written `as` (ADR-043 D4), `x.truncating_i32()`, which
    /// *is* Rust's `as` (D7), and `xs.len()`, whose `usize` becomes the `i64` the
    /// ledger promises ([ADR-048](../../../../docs/specification/adr/adr-048.md)
    /// D1). What they have in common is where they sit in Rust's precedence -
    /// below the unary operators, above the binary ones - which is not where a
    /// method call sits, so the two places that decide parentheses have to ask
    /// about the emitted shape rather than the written one.
    ///
    /// Asked in one place because the alternative is parenthesising always, and a
    /// parenthesis nobody needs is a Rust warning about a file nobody wrote
    /// (Part III, C.1).
    fn emits_as_cast(&self, expr: &Expr) -> bool {
        match expr {
            Expr::Cast { .. } => true,
            Expr::MethodCall { method, args, .. } => {
                let name = self.text(*method);
                truncating(name).is_some() || is_length(name, args)
            }
            _ => false,
        }
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
            // **An operand that comes out as a conversion**, in the two
            // positions where Rust reads it wrongly without parentheses.
            //
            // Beside a **unary** operator, because `as` binds looser: `-x as i64`
            // is `(-x) as i64`. And as the **left of a comparison**, where it is
            // not a precedence question at all - `x as i64 < k` is read as
            // `i64<k>`, the start of a generic argument list, and the program
            // does not parse. Both would otherwise be a `rustc` message about a
            // file nobody wrote (Part III, C.1).
            //
            // Nowhere else, and that is the same rule: `x as i64 + 1` needs no
            // parentheses and a parenthesis nobody needs is the other half of
            // C.1 - `unused_parens` is a warning about the generated file.
            _ => {
                self.emits_as_cast(expr) && (needs == u8::MAX || needs == precedence(BinaryOp::Lt))
            }
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

    fn args(
        &self,
        out: &mut Out,
        // The callee as the source wrote it. Part I 2.3's wrap at an argument
        // is keyed by it, because a statement may hold several calls and one
        // call several arguments (`check::Checked::nullable_args`).
        callee: &str,
        args: &[Expr],
        // Which positional parameters of the callee take a **handle** on a shared
        // value by value, where a ledger describes the callee. Empty where nothing
        // does or nothing is known, which is almost every call.
        takes_a_handle: &[bool],
        depth: usize,
        flow: Flow<'_>,
    ) -> Result<()> {
        for (i, arg) in args.iter().enumerate() {
            if i > 0 {
                out.push(", ");
            }
            // ADR-040 D1: a handle on a shared value is **duplicated, never
            // moved**, where it is handed on by value. There is no method for a
            // programmer to call, so this is where the step is written - and
            // unconditionally, not only where the name is used again (D2): a line
            // further down may not decide what a line further up does to a
            // cleanup point.
            //
            // Only where the argument **names** a handle. A temporary - a call
            // that hands one back - has no block of its own to die at the end of,
            // so there is no second owner and nothing to duplicate.
            let duplicate = takes_a_handle.get(i).copied().unwrap_or(false)
                && matches!(arg, Expr::Variable(_) | Expr::Field { .. });
            // Part I 2.3: a plain value in a parameter the callee declares
            // nullable.
            // **And which of the statement's calls this is**
            // (`check::argument_shape`): the three parts above name a
            // *parameter*, and a statement may call one function twice. The
            // shape is built only where something was recorded for this
            // statement, callee and position, so the common argument pays a
            // lookup and no allocation beyond the one this key already made.
            let (before, after) = Self::around(
                self.nullable_args
                    .get(&(flow.statement, callee.to_string(), i))
                    .and_then(|by_shape| by_shape.get(&crate::check::argument_shape(arg)))
                    .copied(),
            );
            // **A count the language below wants in `usize`**
            // ([ADR-054](../../../docs/specification/adr/adr-054.md) D2), which
            // is the parameter direction of ADR-048 D1. The ledger writes such a
            // parameter as the `i64` a program can hold and the conversion is
            // emitted, so a user writes `"  ".repeat(indent)` and never
            // `indent as usize` - a conversion into a type Part I 2.2 does not
            // offer, which is what that line used to be.
            //
            // Not where the argument is written **only in literals**, for
            // `index::at`'s reason: `of(2)` has nothing to infer its argument
            // type from, every integer type answers with the same `usize`, and
            // `cannot infer type` about a generated file is what Part III C.1
            // forbids. Rust's own inference already gives a literal the `usize`.
            // **And a `usize` a C declaration names is one too**
            // ([ADR-147](../../docs/specification/adr/adr-147.md) D2): the
            // declaration says `size_t` and a caller hands over the `i64` this
            // language has, so the conversion is written here. Read off the
            // declaration rather than off a list of names, which is what
            // `is_count` is and has to be for a ledger's entries.
            let wants_a_size = is_count(callee, i) || self.takes_a_size(callee, i);
            let count = wants_a_size && !only_literals(arg);
            // **The caller writes no `&`** ([ADR-094](../../docs/specification/adr/adr-094.md)
            // D1): where the callee reads this argument rather than keeping it,
            // the reference is the compiler's, and it is written here. The
            // *declaration* reads the same column, so the two cannot disagree —
            // and where the source wrote one itself, `NK1137` has already
            // refused the program rather than letting a `&&T` reach the
            // language below.
            let shape = crate::check::argument_shape(arg);
            let key = (flow.statement, callee.to_string(), i);
            let lend = self
                .lent_args
                .get(&key)
                .is_some_and(|shapes| shapes.contains(&shape));
            // **And `&mut` for a parameter the callee declared `mut`** (D3),
            // which is the one of the three states the *author* wrote rather
            // than the inference. The two maps are disjoint by construction:
            // `lends` withholds its claim on a `mut` position.
            let change = self
                .mut_args
                .get(&key)
                .is_some_and(|shapes| shapes.contains(&shape));
            // **A lambda handed to a parameter whose type may pause**
            // ([ADR-122](../../docs/specification/adr/adr-122.md) D1): a
            // closure that returns a **boxed future**, which is what a callee
            // declared `impl Fn(A) -> Pin<Box<dyn Future<…>>>` takes. Rust has
            // no stable `async` closure, so the shape is written out.
            //
            // `move`, because the future outlives the closure body it is made
            // in and what it captures has to go with it — which is Part I 5.4's
            // detached case, arrived at from the lowering rather than from a
            // word.
            let future = matches!(arg, Expr::Closure { .. })
                && self.future_lambdas.contains(&(flow.statement, i));
            // **The pointer a C declaration takes**
            // ([ADR-147](../../docs/specification/adr/adr-147.md) D1). The
            // declaration says `&[u8]` and C wants an address, so the address
            // is made here: `bytes.as_ptr()` for a run and a plain `&` for one
            // value, which Rust coerces to `*const T` on its own.
            //
            // It is exclusive with the two above by construction rather than by
            // an `else`: a boundary type is not a view in the ledger's type
            // language and not something that moves, so `keeps::lends` withholds
            // its claim and `mut_args` never names the position.
            let pointer = self
                .foreign_params
                .get(callee)
                .and_then(|params| params.get(i))
                .and_then(|ty| self.pointer_for(callee, ty));
            out.push(before);
            // **Neither `&` at the boundary**, and that is not an ordering
            // choice: what a C declaration takes is decided by the declaration
            // ([ADR-147](../../docs/specification/adr/adr-147.md) D1, D3), and
            // a Rust reference in front of it would be the wrong address —
            // `&FILE` is `FILE**` where C wants `FILE*`.
            if pointer.is_none() {
                if change {
                    out.push("&mut ");
                } else if lend {
                    out.push("&");
                }
            }
            if let Some(Pointer::Reference { mutable }) = pointer {
                out.push(match mutable {
                    true => "&mut ",
                    false => "&",
                });
            }
            if count {
                out.push("nikaia_std::count::of(");
            }
            // A count written only in literals is left for Rust to infer as a
            // `usize`, so a suffix may not be written into it either.
            let inside = match wants_a_size && !count {
                true => flow.inferred(),
                false => flow,
            };
            match (future, arg) {
                // `|a| Box::pin(async move { … })`, which is what a parameter
                // declared `impl Fn(A) -> Pin<Box<dyn Future<…>>>` takes
                // ([ADR-122](../../docs/specification/adr/adr-122.md) D1).
                //
                // **`in_lambda` is deliberately not set**, and that is D2: the
                // refusal of a pausing body inside a lambda existed because the
                // lowering had no shape for one. This *is* the shape, so a body
                // that pauses is an ordinary body here — the `async` block is
                // where its `.await`s belong.
                (
                    true,
                    Expr::Closure {
                        params,
                        mutable,
                        body,
                    },
                ) => {
                    let names: Vec<String> =
                        params.iter().map(|p| self.name(*p).into_owned()).collect();
                    out.push(&format!("|{}| Box::pin(async move ", names.join(", ")));
                    let mut changed: Vec<Symbol> = inside.changed.to_vec();
                    changed.extend(mutable.iter().copied());
                    let body_flow = Flow {
                        changed: &changed,
                        statement: inside.statement,
                        ..Flow::PLAIN
                    };
                    self.block(out, body, depth, body_flow, Tail::Return)?;
                    out.push(")");
                }
                _ => self.expr(out, arg, depth, inside)?,
            }
            // `.as_ptr()` and `.as_mut_ptr()`, which a `Vec`, an `Array` and
            // text all answer - so one call writes the address of whatever a
            // caller lends a `&[T]` parameter (D1).
            if let Some(Pointer::Run { mutable }) = pointer {
                out.push(match mutable {
                    true => ".as_mut_ptr()",
                    false => ".as_ptr()",
                });
            }
            // **A handle is lent unless this call is its cleanup** (D3): the
            // address goes by value and the caller keeps the handle, so its
            // release still runs at the end of its scope. The release function
            // itself takes the handle, and Rust's own move is what keeps it
            // from being released twice.
            if let Some(Pointer::Handle { give: false }) = pointer {
                out.push(".lent()");
            }
            if count {
                out.push(")");
            }
            out.push(after);
            if duplicate {
                // `.clone()` and not `Rc::clone(&x)`: it is right under either
                // count, so it cannot disagree with the type the position was
                // lowered to. `Rc<T>` and `Arc<T>` implement `Clone` themselves,
                // so this steps the count and never copies `T`.
                out.push(".clone()");
            }
        }
        Ok(())
    }

    /// Which of a callee's positional parameters take a handle on a shared value
    /// **by value**, where a ledger describes the callee.
    ///
    /// Resolved the way `contracts::sharing::Analysis::parameters` resolves it -
    /// this program's own ledger first, then `std`'s, by name and then by suffix
    /// (ADR-011 D2) - so that the duplication written here and the duplication
    /// `--sharing` names are the same call's.
    ///
    /// A `&Shared[T]` parameter answers **no**, which is ADR-040 D1's correction:
    /// lending the inner value out hands no handle on.
    fn takes_a_handle(&self, callee: &str) -> Vec<bool> {
        let suffix = format!("::{callee}");
        let contract = [&self.own_contracts, &self.library]
            .into_iter()
            .find_map(|ledger| {
                ledger.functions.get(callee).or_else(|| {
                    ledger
                        .functions
                        .iter()
                        .find(|(key, _)| key.ends_with(&suffix))
                        .map(|(_, contract)| contract)
                })
            });
        contract
            .and_then(|contract| contract.signature.as_ref())
            .map(|signature| {
                signature
                    .arguments()
                    .iter()
                    .map(|(_, ty)| {
                        // `SharedMut[T]` is a count around a lock
                        // ([ADR-064](../../../docs/specification/adr/adr-064.md)
                        // D1), so a handle on one is handed on the same way.
                        matches!(
                            ty,
                            crate::contracts::ty::Ty::Named { name, view: false, .. }
                                if name == SHARED || name == SHARED_MUT
                        )
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The same, for the callee a `Call` names.
    fn takes_a_handle_at(&self, func: &Expr) -> Vec<bool> {
        let name = match func {
            Expr::Variable(name) => self.text(*name).to_string(),
            Expr::Path(segments) => segments
                .iter()
                .map(|s| self.text(*s))
                .collect::<Vec<_>>()
                .join("::"),
            _ => return Vec::new(),
        };
        // Part I 4.2's anonymous constructor is reached as `Type(…)` and recorded
        // as `Type::new`, which is the one place the call's spelling and the
        // ledger's key differ.
        match self.structs.iter().any(|s| self.text(*s) == name) {
            true => self.takes_a_handle(&format!("{name}::new")),
            false => self.takes_a_handle(&name),
        }
    }

    /// `Measurements.file(data)` - the whole of what a user writes to run a
    /// grammar. Everything the parallel form needs is already in the grammar
    /// (ADR-009): the frame says where the input may be cut, the `par_fold`
    /// says how the pieces combine. What is left is choosing the executor, and
    /// that is the build's decision, not the program's.
    fn grammar_entry(
        &self,
        out: &mut Out,
        grammar: Symbol,
        entry: Symbol,
        input: &Expr,
        depth: usize,
        flow: Flow<'_>,
    ) -> Result<()> {
        // **The `?` is the call's, and a `catch` is what takes it away.** A
        // grammar entry propagates on its own everywhere else (Kap 7.1); inside
        // the half a `catch` guards, the `match` around it handles the failure,
        // so the `Result` has to arrive whole. Read off `flow` rather than
        // passed in, which is how every other call decides it.
        let question = match flow.caught {
            false => "?",
            true => "",
        };
        let name = self.text(grammar);
        let def = self
            .grammars
            .get(&grammar)
            .ok_or_else(|| refused!("no grammar named `{name}` in this file"))?;

        // **The rule is named at the call** ([ADR-082](../../docs/specification/adr/adr-082.md)
        // D1, D2). It used to be picked here — the first `pub` rule, a
        // `par_fold` one beating an earlier one — so a grammar with two of them
        // got one by source order, in silence. Every `pub` rule is an entry
        // now, and which one is what the program wrote.
        let rule = def
            .rules
            .iter()
            .find(|r| r.is_public && r.name == entry)
            .ok_or_else(|| {
                let rule = self.text(entry);
                match def.rules.iter().any(|r| r.name == entry) {
                    // **A rule that is not `pub` is not an entry** (D2), and
                    // saying *there is no such rule* about one written three
                    // lines up is the message a reader cannot act on.
                    true => refused!(
                        "`{rule}` is a rule of grammar `{name}` and is not `pub`, \
                         so it is not an entry (ADR-082 D2) - write `pub rule {rule}` \
                         to make it one"
                    ),
                    false => refused!("grammar `{name}` has no `pub` rule called `{rule}`"),
                }
            })?;
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

/// **An integer literal, and the one case it has to carry its own type**
/// ([ADR-060](../../../docs/specification/adr/adr-060.md) D2).
///
/// Part I 2.4 says a number takes the type its use asks for, and `i32` where
/// nothing asks. The first half is the language below's inference and works;
/// the second half was that language's *default*, inherited rather than chosen,
/// and it is what refused `let big = 3000000000` in the backend's words about a
/// type the program never wrote (Part III C.1).
///
/// So a literal an `i32` does not hold is written as an `i64`, and D3 is why
/// that needs no analysis at all: the integer types a program may write are
/// `i32` and `i64` ([ADR-048](../../../docs/specification/adr/adr-048.md)), an
/// integer literal is never a float, and `u8` is narrower — so a value an `i32`
/// cannot hold has no second answer a use could ask for. There is nothing to
/// find out.
///
/// **A literal that fits an `i32` is untouched**, which is what keeps every
/// program that compiles today compiling: the suffix would pin what the use is
/// supposed to decide, and `let small = 42` passed to a parameter taking an
/// `i64` is Part I 2.4's own example.
///
/// The value and not the digits, which is why `-2147483648` never reaches here
/// as `2147483648`: the negation is folded at the `Unary` arm above.
fn integer_literal(value: i64, widen: bool) -> String {
    match i32::try_from(value) {
        Ok(_) if !widen => value.to_string(),
        _ => format!("{value}i64"),
    }
}

/// Whether this expression is a **number written down**, sign and all.
///
/// The negation is part of the answer rather than a wrapper around it: the
/// unary arm above folds `-1` into one literal, and `-1.into()` in the language
/// below is `-(1.into())`, which is a second reason not to write the `into` at
/// all.
fn a_number(expr: &Expr) -> bool {
    match expr {
        Expr::LitInt(_) | Expr::LitFloat(_) => true,
        Expr::Unary {
            op: UnaryOp::Neg,
            expr,
        } => a_number(expr),
        _ => false,
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
/// Whether an index is written **only in literals**, and therefore needs no
/// conversion and can take none.
///
/// `xs[0]`, `xs[-1]`, `&text[1..3]`, `xs[2 * 3]`: Rust's own inference gives each
/// of these the `usize` a sequence wants. And `nikaia_std::index::at` cannot help
/// here even where it would be harmless, because there is nothing to infer its
/// argument type *from* - every integer type answers with the same `usize`, so the
/// call is ambiguous and `cannot infer type` about a generated file is what
/// Part III C.1 forbids.
/// Whether this expression **leaves** rather than coming to a value
/// ([ADR-138](../../docs/specification/adr/adr-138.md) D1).
///
/// What it decides is where the jump is *written*: a closure is a function
/// boundary ([ADR-084](../../docs/specification/adr/adr-084.md) D4), so a
/// lowering that puts an expression inside one has to know whether that
/// expression is a jump before it does.
fn jumps(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::Throw(_) | Expr::Return(_) | Expr::Break | Expr::Continue
    )
}

/// **How a call makes the address an `extern "C"` declaration takes**
/// ([ADR-147](../../docs/specification/adr/adr-147.md) D1).
///
/// Two shapes, because C's two are a run of elements and one value: a run is a
/// pointer to the first element and the length is a parameter of its own (D2),
/// and one value is an address Rust writes for a `&` on its own.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Pointer {
    /// `&[T]` and `&mut [T]`: `.as_ptr()`, `.as_mut_ptr()`.
    Run { mutable: bool },
    /// `&T` and `&mut T`: a plain `&`, which Rust coerces to `*const T`.
    Reference { mutable: bool },
    /// **An opaque handle** (D3), passed as the address by value.
    ///
    /// `give` is true for the type's own **release** function and false for
    /// every other declaration: that one call *is* the cleanup, so the handle
    /// goes with it and Rust's own move is what keeps it from being released
    /// twice. Everywhere else the caller keeps the handle, because a C function
    /// that is handed one does not take it — `fileno(f)` reads it and `f` is
    /// still the caller's to close.
    Handle { give: bool },
}

/// Which of the two a declared parameter is, or `None` where it is an ordinary
/// value an `i32` is at both ends.
fn pointer_for(ty: &Type) -> Option<Pointer> {
    match (ty.is_slice, ty.is_view) {
        (true, _) => Some(Pointer::Run { mutable: ty.is_mut }),
        (false, true) => Some(Pointer::Reference { mutable: ty.is_mut }),
        (false, false) => None,
    }
}

/// `const` or `mut`, the word Rust puts between the `*` and the type.
fn pointing(mutable: bool) -> &'static str {
    match mutable {
        true => "mut",
        false => "const",
    }
}

fn only_literals(index: &Expr) -> bool {
    match index {
        Expr::LitInt(_) => true,
        Expr::Unary { expr, .. } => only_literals(expr),
        Expr::Binary { lhs, rhs, .. } => only_literals(lhs) && only_literals(rhs),
        Expr::Range { start, end, .. } => only_literals(start) && only_literals(end),
        _ => false,
    }
}

/// `xs.len()` - the four `std` entries that hand back a length, by the name all
/// four of them have ([ADR-048](../../../docs/specification/adr/adr-048.md) D1).
fn is_length(method: &str, args: &[Expr]) -> bool {
    method == "len" && args.is_empty()
}

/// Whether a callee's argument at `at` is a **count** the language below takes in
/// `usize` ([ADR-054](../../../docs/specification/adr/adr-054.md) D2).
///
/// A name and a position, not a rule, for the reason `is_length` is one: what it
/// encodes is a fact about *Rust's* library rather than about this language. The
/// list is the entries of `std.contracts` whose Rust counterpart counts in
/// `usize`, and today it is one of them - `str::repeat`, which is the site
/// ADR-048 §3 named when it wrote this direction down as open.
/// `a_count_parameter_is_an_i64` in `tests/contracts.rs` keeps the two from
/// drifting apart.
///
/// Being a name is safe here because `count::of` is the **identity** for
/// anything that is not a number: a method of the program's own called `repeat`
/// passes through it untouched, which is the same property that lets `index::at`
/// be written around every index.
fn is_count(callee: &str, at: usize) -> bool {
    matches!((callee, at), ("repeat", 0))
}

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

/// **A lambda whose body pauses, which this lowering cannot write.**
///
/// Rust has no stable `async` closure, so there is no shape for a lambda that
/// gives the thread up - and a plain closure holding an `.await` is a `rustc`
/// error about a file nobody wrote (Part III, C.1). This is that error in
/// Nikaia's words, at the one place that has the fact: the emitter.
///
/// **A limit of this compiler and not of the language.** Nikaia is implicitly
/// async ([ADR-055](../../../docs/specification/adr/adr-055.md) D1), so
/// `examples/fortunes.nika`'s route handler - a lambda that queries a database -
/// is a correct program. It still type-checks, and it is only a *build* that
/// meets this. What closes it is §6 step 3 and step 4, where `std`'s own
/// signatures say which parameters take something that may pause, and a lambda
/// handed to one of those can be written as a closure that returns a future.
fn pausing_in_a_lambda(callee: &str) -> anyhow::Error {
    refused!(
        "this lambda calls `{callee}`, which can pause - and a lambda that pauses is \
         not something this compiler can build yet (ADR-055 §6). Call `{callee}` \
         outside the lambda and hand it the value, or give it a `sync` body"
    )
}

/// Whether an expression names a **place** — something that already exists and
/// can be pointed at — rather than a value this expression makes
/// ([ADR-094](../../../docs/specification/adr/adr-094.md) D4).
///
/// A name, a field of one, an element of one. A call, a range, a literal and a
/// method call are not: `for i in 0..n` counts, `for line in io::lines()` reads
/// a stream, and `for x in xs.drain()` is the written form of taking the
/// elements away — none of the three has anything to lend.
fn is_a_place(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::Variable(_) | Expr::Field { .. } | Expr::SafeField { .. } | Expr::Index { .. }
    )
}

/// The types a ledger records a view-holding field for.
///
/// The package-wide half of [`Emitter::borrows`]. `tethered` is non-empty for
/// exactly the structs [`borrowing_structs`] finds in the file that declares
/// them (`Ledger::infer_checked` computes it from that set), so this carries the
/// same fact across a file boundary rather than recomputing a different one.
/// The types this unit declares an `impl Error for …` for.
///
/// The trait's name and nothing more: `Error` is the one trait this compiler
/// reads by name (`NK1130`'s note says so), and Part I 7.1 makes the `impl`
/// line the marker — *what is thrown implements `Error`, and the `impl` line
/// says so*.
fn declared_errors(parsed: &Parsed) -> std::collections::BTreeSet<String> {
    let mut out = std::collections::BTreeSet::new();
    for item in &parsed.program.items {
        if let Item::Impl {
            trait_name: Some(trait_name),
            target,
            ..
        } = &item.node
        {
            if parsed.text(*trait_name) == "Error" {
                out.insert(parsed.text(target.name).to_string());
            }
        }
    }
    out
}

fn tethered_types(contracts: &crate::contracts::Ledger) -> std::collections::BTreeSet<String> {
    contracts
        .types
        .iter()
        .filter(|(_, contract)| !contract.tethered.is_empty())
        .map(|(name, _)| name.clone())
        .collect()
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

/// The type a `truncating_` method converts to, if the name is one
/// ([ADR-043](../../../docs/specification/adr/adr-043.md) D7).
///
/// The target is **in the name** rather than inferred, for the reason the
/// `wrapping_` names have their own entry per type: there is one method per
/// destination, and the ledger has to be able to write its signature down.
///
/// The list is closed and matches the ledger's entries. A name this does not
/// recognise is an ordinary method call, so a program that writes
/// `truncating_u8` is refused by the type checker for the method not existing
/// rather than quietly emitting a conversion nobody described.
fn truncating(method: &str) -> Option<&'static str> {
    match method.strip_prefix("truncating_")? {
        "i32" => Some("i32"),
        "i64" => Some("i64"),
        _ => None,
    }
}

/// Walk every expression in a block, including the ones inside statements.
/// The functions of this program that can pause **and take part in a cycle**
/// ([ADR-055](../../../docs/specification/adr/adr-055.md) D6's first sharp
/// edge).
///
/// A recursive `async fn` has an infinitely sized future, and `rustc` says so:
/// *"recursion in an async fn requires boxing"*. `examples/json.nika` has two of
/// them - `show` and `longest` both walk a tree - so this is not a case to meet
/// later, it is the corpus on day one.
///
/// **The graph is syntactic, and over-approximating is the safe direction.** A
/// free call names its callee outright; a method call does not name its receiver's
/// type, so an edge is drawn to *every* pausing method of that name. Boxing a
/// call that did not need it costs one allocation and nothing else, while missing
/// one is a program that does not compile - so where the two readings differ this
/// takes the wider.
///
/// **The cycle test is reachability from a function to itself**, computed to a
/// fixpoint. `n` is the number of functions in one program, and the obvious
/// algorithm is the one whose correctness a reader can check.
fn pausing_reach(
    parsed: &Parsed,
    contracts: &crate::contracts::Ledger,
) -> std::collections::BTreeMap<String, std::collections::BTreeSet<String>> {
    use std::collections::{BTreeMap, BTreeSet};

    let pausing: BTreeSet<&str> = contracts
        .functions
        .iter()
        .filter(|(_, contract)| !contract.sync.is_sync())
        .map(|(name, _)| name.as_str())
        .collect();
    if pausing.is_empty() {
        return BTreeMap::new();
    }

    // A bare method name to every pausing key that ends in it, which is the
    // over-approximation above.
    let mut by_last: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for key in &pausing {
        let last = key.rsplit("::").next().unwrap_or(key);
        by_last.entry(last).or_default().push(key);
    }

    let edges_of = |body: &Block| -> BTreeSet<String> {
        let mut out = BTreeSet::new();
        let mut note = |name: &str| {
            let name = parsed.unaliased(name);
            if pausing.contains(name.as_str()) {
                out.insert(name);
            } else if let Some(keys) = by_last.get(name.as_str()) {
                out.extend(keys.iter().map(|k| k.to_string()));
            }
        };
        visit_block(body, &mut |expr| match expr {
            Expr::Call { func, .. } => match func.as_ref() {
                Expr::Variable(name) => note(parsed.text(*name)),
                Expr::Path(segments) => {
                    let joined: Vec<&str> = segments.iter().map(|s| parsed.text(*s)).collect();
                    note(&joined.join("::"));
                }
                _ => {}
            },
            Expr::MethodCall { method, .. } | Expr::SafeMethod { method, .. } => {
                note(parsed.text(*method))
            }
            _ => {}
        });
        out
    };

    let mut graph: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for item in &parsed.program.items {
        match &item.node {
            Item::Fn {
                name: Some(name),
                body,
                ..
            } => {
                graph.insert(parsed.text(*name).to_string(), edges_of(body));
            }
            Item::Impl {
                target, methods, ..
            } => {
                let target = parsed.text(target.name).to_string();
                for method in methods {
                    if let Item::Fn { name, body, .. } = &method.node {
                        let own = match name {
                            Some(name) => parsed.text(*name).to_string(),
                            None => "new".to_string(),
                        };
                        graph.insert(format!("{target}::{own}"), edges_of(body));
                    }
                }
            }
            _ => {}
        }
    }

    // Reachability to a fixpoint, then "reaches itself".
    let mut reaches: BTreeMap<String, BTreeSet<String>> = graph.clone();
    loop {
        let mut changed = false;
        for key in graph.keys() {
            let grown: BTreeSet<String> = reaches[key]
                .iter()
                .filter_map(|next| reaches.get(next))
                .flatten()
                .cloned()
                .collect();
            let entry = reaches.get_mut(key).expect("every key was inserted");
            let before = entry.len();
            entry.extend(grown);
            changed |= entry.len() != before;
        }
        if !changed {
            break;
        }
    }

    // Only what pauses, because only a pausing call is a future at all - and a
    // caller that cannot pause holds no future to be infinitely sized.
    reaches.retain(|key, _| pausing.contains(key.as_str()));
    for seen in reaches.values_mut() {
        seen.retain(|key| pausing.contains(key.as_str()));
    }
    reaches
}

/// **Which branches an `overlap` starts first**, for `--overlaps`
/// ([ADR-050](../../../docs/specification/adr/adr-050.md) D6).
///
/// The report is `contracts::order`'s and the answer is the emitter's: whether a
/// branch can pause is the ledger's `sync` column and the checker's answers
/// about method calls, and no analysis in that file may reach for either
/// (ADR-033 §8.2b, which is why nothing in that file takes a build setting). So
/// it is handed over as a function, closed over an emitter built exactly as the
/// one that will lower the program.
pub fn branch_starts_first<'p>(
    parsed: &'p Parsed,
    build: Build,
    contracts: &crate::contracts::Ledger,
) -> impl Fn(&Spanned<Stmt>) -> bool + 'p {
    let emitter = Emitter::with_contracts(
        parsed,
        build,
        crate::contracts::Provenance::Trusted,
        contracts.clone(),
    );
    move |stmt| emitter.branch_pauses(stmt, Flow::PLAIN)
}

pub(crate) fn visit_block(block: &Block, f: &mut impl FnMut(&Expr)) {
    for stmt in &block.stmts {
        match &stmt.node {
            Stmt::Let { value, .. } | Stmt::Comptime { value, .. } => visit_expr(value, f),
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
            // Neither holds an expression, so there is nothing here to walk.
            Stmt::Break | Stmt::Continue => {}
            Stmt::Expr(expr) => visit_expr(expr, f),
        }
    }
}

/// Every expression inside this one, this one included.
///
/// `pub(crate)` because the checker reads it too
/// ([ADR-099](../../docs/specification/adr/adr-099.md)): `NK2205` asks whether
/// a `get` is written anywhere inside a `set`'s argument, and a second walk
/// over the same shape is a second thing to keep in step with the AST.
pub(crate) fn visit_expr(expr: &Expr, f: &mut impl FnMut(&Expr)) {
    f(expr);
    match expr {
        Expr::Block(block) | Expr::Unsafe(block) | Expr::Overlap(block) => visit_block(block, f),
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
        Expr::MethodCall { receiver, args, .. } | Expr::SafeMethod { receiver, args, .. } => {
            visit_expr(receiver, f);
            args.iter().for_each(|a| visit_expr(a, f));
        }
        Expr::Match { value, arms } => {
            visit_expr(value, f);
            for arm in arms {
                visit_expr(&arm.body, f);
            }
        }
        Expr::Tuple(parts) | Expr::ListLit { items: parts, .. } => {
            parts.iter().for_each(|p| visit_expr(p, f))
        }
        Expr::Field { base, .. } | Expr::SafeField { base, .. } => visit_expr(base, f),
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
        Expr::Return(Some(value)) => visit_expr(value, f),
        Expr::Return(None) | Expr::Break | Expr::Continue => {}
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
/// The same as [`literal_expressions`], with the names a template's `<for>`
/// binds around each hole. Empty for anything that is not a template.
pub(crate) fn literal_expressions_bound(parsed: &Parsed, expr: &Expr) -> Vec<(Expr, Vec<String>)> {
    match expr {
        Expr::Dsl {
            target, content, ..
        } if crate::dsl::is_deferred(parsed.text(*target), content) => Vec::new(),
        Expr::Dsl { content, .. } => match template::split(content.trim()) {
            Ok(segments) => template_holes_bound(&segments)
                .into_iter()
                .filter_map(|(hole, bound)| {
                    parse_expression(&parsed.interner, &hole)
                        .ok()
                        .map(|expr| (expr, bound))
                })
                .collect(),
            Err(_) => Vec::new(),
        },
        _ => literal_expressions(parsed, expr)
            .into_iter()
            .map(|hole| (hole, Vec::new()))
            .collect(),
    }
}

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
    template_holes_bound(segments)
        .into_iter()
        .map(|(hole, _)| hole)
        .collect()
}

/// The same, with **what a `<for>` binds around each hole**.
///
/// `<for r in :rows>{r.name}</for>`: `r` is declared by the template and named
/// in the hole, so a reader of a hole that does not know that sees a name
/// nothing declares. Which is what a checker asking *"does anything declare
/// this?"* needs, and nothing else does - the emitter only needs the text
/// ([ADR-017](../../../docs/specification/adr/adr-017.md)).
///
/// Innermost last, so a nested `<for>` shadowing the same word behaves the way
/// a nested block does.
pub(crate) fn template_holes_bound(segments: &[template::Segment]) -> Vec<(String, Vec<String>)> {
    let mut holes = Vec::new();
    walk_template_holes(segments, &mut Vec::new(), &mut holes);
    holes
}

fn walk_template_holes(
    segments: &[template::Segment],
    bound: &mut Vec<String>,
    out: &mut Vec<(String, Vec<String>)>,
) {
    for segment in segments {
        match segment {
            template::Segment::Text(_) => {}
            template::Segment::Hole { expr, .. } => out.push((expr.clone(), bound.clone())),
            template::Segment::For {
                binding,
                collection,
                body,
            } => {
                // The collection is captured from the enclosing scope with `:`
                // (ADR-007 D4), and it is a name this program wrote - so it is
                // read *outside* the binding, which is where it lives.
                out.push((collection.clone(), bound.clone()));
                bound.push(binding.clone());
                walk_template_holes(body, bound, out);
                bound.pop();
            }
        }
    }
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
                    return Err(refused!("string ends in a `\\`: \"{literal}\""));
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
                    return Err(refused!("unclosed `{{` in \"{literal}\""));
                }

                holes.push(hole);
                match spec {
                    Some(spec) => format.push_str(&format!("{{:{spec}}}")),
                    None => format.push_str("{}"),
                }
            }
            '}' => {
                return Err(refused!(
                    "stray `}}` in \"{literal}\"; write `}}}}` for a brace"
                ))
            }
            _ => format.push(c),
        }
    }

    Ok((format, holes))
}
