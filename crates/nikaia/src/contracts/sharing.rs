// crates/nikaia/src/contracts/sharing.rs
//
// Which reference count a **particular** `Shared` value gets
// ([ADR-037](../../../../docs/specification/adr/adr-037.md) D7).
//
// ## One rule, and everything here is downstream of it
//
//     It may only ever take an atomic away.
//
// ADR-037 D6 makes the atomic count the **floor**: a `Shared` is atomic at both
// settings of `user_parallelism`, which is what makes it crossable at all, and
// what `contracts::send` now concludes about the type. This file is an
// **optimisation** on top of that floor and nothing else. It may lower a
// particular value to a plain count where it *proves* that nothing crosses a
// thread with it; where it cannot prove that, the answer stays atomic. It may
// never add an atomic, and it may never make a program unsafe - which are the
// same sentence read from both ends.
//
// **That is why being wrong here costs speed in one direction and correctness in
// the other**, and why the polarity is not negotiable. An atomic count where a
// plain one would have done costs about 9 ns per clone-and-drop pair
// (`docs/rc-or-arc.md` §3). A plain count on a value that crosses is a data
// race. So every case this cannot decide comes out [`Count::Atomic`], every such
// case carries the reason, and the reasons are a **closed list** - see
// [`Fallback`], which is the enumeration ADR-037 D8 answers "would an override
// help?" for.
//
// **Fail-closed is affordable here in places `NK25xx` could not afford it.**
// `send.rs` may not refuse what it cannot decide, because refusing a correct
// program is the one thing this compiler may never do (Part III C.4) - so it has
// a third answer, `Undecided`. This has two, because the cost of being wrong in
// the safe direction is speed: the program still compiles and still means the
// same thing. ADR-033 D4's polarity, applied where it is cheap.
//
// ## What this file promises `send.rs`
//
// `send.rs` answers `May` for a `Shared` of crossable data, and that answer is
// only true because the count is atomic. So the two files have a contract with
// each other, and it runs one way:
//
//     a value this file lowers to a plain count must not cross a thread.
//
// Every crossing is therefore a **seed**, and a missed seed is unsoundness
// rather than a missed optimisation. The seeds are ADR-005 §5.2's own
// enumeration of the crossings, asked per value rather than per type - including
// the one D6 made live. `task::both`, the crossing the *compiler* chooses for
// statement overlapping, used to be safe to leave out because a `Shared` was
// `MayNot` and was never overlapped; since D6 the result of an overlapped
// operation is a `Shared` that crosses back to the thread that started it, so it
// is a seed now, reached through [`Fallback::UnseenOrigin`] - a handle whose
// allocation this analysis did not watch being made.
//
// ## Why the fixpoint degenerates, which is itself a finding
//
// A count belongs to the **allocation**, not to the handle: two handles on one
// `Shared` share one count, so they cannot disagree about whether it is atomic.
// The relation between handles is therefore an *equivalence* and not an ordering,
// and the analysis is union-find over handles plus one colouring pass - a least
// fixpoint reached in a single step, where `sync` (ADR-027 D1) genuinely needs a
// greatest one because its constraint is conjunctive over a call graph that can
// cycle. The least fixpoint of "is atomic" is the complement of the greatest
// fixpoint of "stays plain", because every clause is a Horn clause and the only
// sources of `Atomic` are the seeds. What carries the weight is not the lattice -
// it is **which crossings are seeds**.
//
// ## What counts as an account, and the one thing it rests on
//
// A handle may stay plain only if every place it goes is a place something
// written down accounts for. Three kinds of place do:
//
//   * **a body this build lowers** - a function of this unit that is not
//     public. The handle joins the callee's parameter slot, one allocation gets
//     one answer, and whichever side of the call the crossing is on decides both;
//   * **a field of a type this unit declares**, through the ledger's `fields`
//     (ADR-024, the walk ADR-029 established) - run in the opposite direction
//     from `send.rs`: there a field that may not cross makes the struct refuse,
//     here a struct that crosses makes the field's count atomic;
//   * **a call the ledger describes**. This is the one that rests on something,
//     and it rests on exactly what ADR-005 §5.2 already rests on: a contract is
//     an account, `std`'s entries are written in the file that ships and
//     reviewed like code (ADR-005 §5.1), and a call nothing describes is the
//     crossing. The residual is the one §5.3 names - "enforceable only as far as
//     a foreign crate is honest". Today no `std` entry takes or hands back a
//     `Shared` at all, and `a_std_entry_that_takes_a_shared_needs_a_second_look`
//     in `tests/sharing.rs` is what makes somebody look on the day one does.
//
// Everywhere else is [`Fallback`], and the remedy for every row of it is to
// write the contract down - ADR-033 D4's own closing argument, which is why
// ADR-037 D8 has no keyword in it.

use std::collections::{BTreeMap, BTreeSet};

use crate::ast::{Block, Expr, Item, Stmt};
use crate::parser::Parsed;

use super::{send, ty::Ty, Ledger};

/// The type whose count this is about.
const SHARED: &str = "Shared";

/// The name of the slot standing for what a function hands back.
const RESULT: &str = "<result>";

/// The pseudo-function every `<struct>.<field>` slot is filed under.
const FIELDS: &str = "<field>";

/// Methods that run their lambda on a thread the program asked for.
///
/// A closed list for the same reason `send::PLAIN` is one: a name this file does
/// not know contributes nothing, and the walk that finds the names inside the
/// lambda is over-approximate, which is the safe direction.
const PARALLEL: &[&str] = &["par_iter", "par_fold", "par_map"];

/// Which kind of count a `Shared` value needs.
///
/// `Plain ⊑ Atomic`, joined upwards: one crossing anywhere in a handle's
/// equivalence class makes the whole class atomic. [`Count::Atomic`] is the
/// default because it is the floor (ADR-037 D6) - a value nothing is known about
/// gets the safe answer without anybody remembering to ask for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum Count {
    /// A count only the thread that built it ever touches. What this analysis
    /// may lower a value to, where it proves nothing crosses.
    Plain,
    /// A count any thread may touch. The floor, and the answer wherever this
    /// analysis cannot prove the other one.
    #[default]
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

    /// Read one back from the ledger.
    pub fn parse(text: &str) -> Option<Self> {
        match text.trim() {
            "plain" => Some(Count::Plain),
            "atomic" => Some(Count::Atomic),
            _ => None,
        }
    }
}

/// Every reason this analysis answers `atomic` **because nothing decided it**,
/// as a closed list.
///
/// The list is the deliverable and not a detail. ADR-010 D1 asks a fail-closed
/// analysis to name what it could not see, ADR-037 D8 asks "would an override
/// help?" of every row of it, and the answer is no for every row - the first
/// five want a **contract** and the last one is a promise about somebody else's
/// code. A row added here is a row that has to be answered there.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Fallback {
    /// The value is a parameter or the result of a **public** function, so its
    /// callers are in a unit this build never reads.
    PublicSignature,
    /// The value is held by a **public field of a public type**, which code this
    /// build never reads may take out of it.
    PublicField,
    /// The value was handed to a **call** nothing written down describes.
    UnseenCall,
    /// The value was handed to a **method** nothing written down describes.
    UnseenMethod,
    /// The value was handed over in an **argument position** no contract covers.
    UncoveredArgument,
    /// The handle exists and this analysis did not watch its allocation being
    /// made - it came out of a call, an index, a field of a type whose `fields`
    /// are unrecorded, or some other expression nothing here accounts for.
    UnseenOrigin,
}

impl Fallback {
    /// Every row, for a report that enumerates rather than summarises.
    pub const ALL: &'static [Fallback] = &[
        Fallback::PublicSignature,
        Fallback::PublicField,
        Fallback::UnseenCall,
        Fallback::UnseenMethod,
        Fallback::UncoveredArgument,
        Fallback::UnseenOrigin,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Fallback::PublicSignature => "a public signature",
            Fallback::PublicField => "a public field",
            Fallback::UnseenCall => "a call nothing describes",
            Fallback::UnseenMethod => "a method nothing describes",
            Fallback::UncoveredArgument => "an argument position no contract covers",
            Fallback::UnseenOrigin => "an origin this analysis cannot see",
        }
    }

    /// What a person would do about it, which is ADR-037 D8's whole answer: for
    /// five of the six, write the contract; for the sixth, nothing, because the
    /// answer belongs to callers who do not exist yet.
    pub fn remedy(self) -> &'static str {
        match self {
            Fallback::PublicSignature => {
                "nothing here - whether it crosses is decided by callers this build does not \
                 read, so an answer would be a promise about somebody else's code"
            }
            Fallback::PublicField => {
                "keep the field out of the published surface, or accept the atomic - code this \
                 build does not read can take the value out of it"
            }
            Fallback::UnseenCall | Fallback::UnseenMethod | Fallback::UncoveredArgument => {
                "write the callee down in a ledger, which is how every other derived fact in \
                 this project gets sharper (ADR-033 D4)"
            }
            Fallback::UnseenOrigin => {
                "write down the call the value came out of, so this analysis can follow it to \
                 the allocation"
            }
        }
    }
}

/// What this analysis chose for one `Shared` value, and why.
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
    /// What kept the count atomic, in the words a user would be told. `None`
    /// for a plain one, which needs no excuse.
    pub why: Option<String>,
    /// Which row of [`Fallback`] this is, where it is one. `None` where the
    /// analysis *found* the crossing rather than failing to rule one out - a
    /// `spawn` is a crossing, not a mystery.
    pub fallback: Option<Fallback>,
}

impl Decision {
    /// Whether the atomic count is the floor holding rather than a crossing
    /// this analysis found. The list ADR-010 D1 asks to be named.
    pub fn undecided(&self) -> bool {
        self.fallback.is_some()
    }
}

/// One allocation class of a function's caller-visible positions, as the ledger
/// records it.
///
/// This is the summary `docs/rc-or-arc.md` §5.1 names as the thing that
/// composes: **which parameters and result are one class, and which count that
/// class gets**. A caller reads it to know what it is handing over; a later
/// build that can read a dependency's source reads it to continue the analysis
/// across the boundary without a representation axis.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Class {
    /// The positions that are one allocation: parameter names, and `<result>`
    /// for what the function hands back. Sorted, because 13.5 makes the file a
    /// pure function of (source, toolchain).
    pub members: Vec<String>,
    pub count: Count,
}

impl Class {
    /// `counts | <result>: atomic`, which is the `borrows(a | b)` spelling
    /// ADR-005 D3 already gave a list of positions.
    pub fn text(&self) -> String {
        format!("{}: {}", self.members.join(" | "), self.count.as_str())
    }

    /// Read one back from the ledger.
    pub fn parse(text: &str) -> Option<Self> {
        let (members, count) = text.rsplit_once(':')?;
        let members: Vec<String> = members
            .split('|')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect();
        match members.is_empty() {
            true => None,
            false => Some(Class {
                members,
                count: Count::parse(count)?,
            }),
        }
    }
}

/// Everything one program's `Shared` values got, and the summary the ledger
/// keeps.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Sharing {
    /// One per `Shared` value a person wrote, sorted by function and then by
    /// name, because a report is read and compared.
    pub decisions: Vec<Decision>,
    /// Per function key, the allocation classes of its caller-visible
    /// positions. What [`super::FnContract::sharing`] records.
    pub summaries: BTreeMap<String, Vec<Class>>,
}

/// Every `Shared` value in a program, which count it would get, and the
/// per-function summary.
pub fn analyse_program(parsed: &Parsed, own: &Ledger, library: &Ledger) -> Sharing {
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
            // A `pub` field of a `pub` type is a place code this build never
            // reads can take the value out of, which is the library boundary
            // one level in from a signature (`docs/rc-or-arc.md` §5.1).
            Item::Struct {
                name,
                fields,
                is_public: true,
                ..
            } => {
                let struct_name = parsed.text(*name).to_string();
                for field in fields.iter().filter(|f| f.is_public) {
                    let ty = Ty::from_ast(parsed, &field.ty);
                    if !holds_shared(&ty) {
                        continue;
                    }
                    let key = format!("{struct_name}.{}", parsed.text(field.name));
                    analysis.note(FIELDS, &key, ty, true, false);
                    analysis.force(
                        FIELDS,
                        &key,
                        format!(
                            "`{key}` is a public field of a public type, so code this build \
                             cannot see may take the value out of it and cross with it"
                        ),
                        Some(Fallback::PublicField),
                    );
                }
            }
            _ => {}
        }
    }
    analysis.decide()
}

/// Every `Shared` value in a program, and which count it would get.
pub fn analyse(parsed: &Parsed, own: &Ledger, library: &Ledger) -> Vec<Decision> {
    analyse_program(parsed, own, library).decisions
}

/// Fill in the `sharing` column of a ledger, from the bodies.
///
/// Runs after the other inferences for the same reason `throws::infer` does:
/// it needs every function to have a `signature` to resolve a callee's
/// parameters against, and every type to have its `fields`. It reads nothing any
/// of them wrote.
pub fn infer(ledger: &mut Ledger, parsed: &Parsed, library: &Ledger) {
    let summaries = analyse_program(parsed, ledger, library).summaries;
    for (key, classes) in summaries {
        if let Some(contract) = ledger.functions.get_mut(&key) {
            contract.sharing = classes;
        }
    }
}

/// What `--sharing` prints: one line per `Shared` value, the reason beside every
/// atomic one, and the enumeration of what the floor caught.
///
/// The shape is `--overlaps`' and `--trust`'s: a heading per function, then one
/// indented line per thing decided, then what it adds up to. It **explains a
/// decision rather than changing one** (ADR-033 D9), which is what a person
/// reads when they want the 9 ns back.
pub fn report(parsed: &Parsed, own: &Ledger, library: &Ledger) -> String {
    let sharing = analyse_program(parsed, own, library);
    if sharing.decisions.is_empty() {
        return "no `Shared` value in this program, so there is nothing to choose.\n".to_string();
    }

    let mut out = String::new();
    let mut at: Option<&str> = None;
    for decision in &sharing.decisions {
        if at != Some(decision.function.as_str()) {
            out.push_str(&format!("{}:\n", decision.function));
            at = Some(&decision.function);
        }
        out.push_str(&format!(
            "    {:<7} `{}` ({})\n",
            decision.count.as_str(),
            decision.value,
            decision.ty,
        ));
        match (&decision.why, decision.fallback) {
            (Some(why), Some(_)) => {
                out.push_str(&format!("             could not decide: {why}\n"))
            }
            (Some(why), None) => out.push_str(&format!("             crosses: {why}\n")),
            (None, _) => out.push_str(
                "             nothing crosses a thread with it, so the count is lowered\n",
            ),
        }
    }

    let atomic = sharing
        .decisions
        .iter()
        .filter(|d| d.count == Count::Atomic)
        .count();
    let undecided: BTreeSet<Fallback> = sharing
        .decisions
        .iter()
        .filter_map(|d| d.fallback)
        .collect();
    out.push_str(&format!(
        "\n{} `Shared` value(s): {} plain, {} atomic, of which {} because this analysis \
         could not decide.\n",
        sharing.decisions.len(),
        sharing.decisions.len() - atomic,
        atomic,
        sharing.decisions.iter().filter(|d| d.undecided()).count(),
    ));
    out.push_str(
        "atomic is the floor and not a verdict (ADR-037 D6). A plain count is an optimisation, \
         and where it is not taken the reason is one of these:\n",
    );
    for fallback in Fallback::ALL {
        out.push_str(&format!(
            "  {} {} - {}\n",
            match undecided.contains(fallback) {
                true => "*",
                false => " ",
            },
            fallback.as_str(),
            fallback.remedy(),
        ));
    }
    out.push_str(
        "The five that want a contract are what ADR-037 D8 answers with `std.contracts` \
         growing rather than with a keyword.\n",
    );
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
    forced: Vec<(String, String, Option<Fallback>)>,
}

struct Handle {
    function: String,
    value: String,
    ty: Ty,
    /// A slot that stands for a struct's field or a function's result rather
    /// than for something a person wrote. Not reported, but classes join
    /// through it.
    internal: bool,
    /// A position a **caller** meets - a parameter, or the result. What the
    /// ledger's summary is about.
    position: bool,
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

    fn note(&mut self, function: &str, value: &str, ty: Ty, internal: bool, position: bool) {
        let key = slot(function, value);
        self.id(&key);
        self.handles.entry(key).or_insert(Handle {
            function: function.to_string(),
            value: value.to_string(),
            ty,
            internal,
            position,
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
    ///
    /// **So there is no guard here, and none may be added.** The same applies to
    /// every other spelling of "only if we have seen it": a fail-closed analysis
    /// fails open through a guard added for tidiness, not through a missing
    /// crossing (`docs/rc-or-arc.md` §8).
    fn force(&mut self, function: &str, value: &str, why: String, fallback: Option<Fallback>) {
        let key = slot(function, value);
        self.id(&key);
        self.forced.push((key, why, fallback));
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
                self.note(&key, &name, ty.clone(), false, true);
                if *is_public {
                    self.force(
                        &key,
                        &name,
                        published(&key, &name),
                        Some(Fallback::PublicSignature),
                    );
                }
            } else if *is_public {
                // A public function's parameter may *hold* a `Shared` without
                // being one, and the field's class is then as exposed as a
                // parameter is. `send::crossing`'s walk through the ledger's
                // `fields`, run the other way round.
                for field in shared_fields(&ty.text(), self.own, self.library) {
                    self.force(
                        FIELDS,
                        &field,
                        published(&key, &name),
                        Some(Fallback::PublicSignature),
                    );
                }
            }
            scope.insert(name, ty);
        }
        if let Some(ret) = ret_type {
            let ty = Ty::from_ast(self.parsed, ret);
            if holds_shared(&ty) {
                self.note(&key, RESULT, ty, true, true);
                if *is_public {
                    self.force(
                        &key,
                        RESULT,
                        published(&key, RESULT),
                        Some(Fallback::PublicSignature),
                    );
                }
            } else if *is_public {
                for field in shared_fields(&ty.text(), self.own, self.library) {
                    self.force(
                        FIELDS,
                        &field,
                        published(&key, RESULT),
                        Some(Fallback::PublicSignature),
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
                let aliased = self.names_a_handle(value, scope);
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
                        self.note(function, &name, ty.clone(), false, false);
                        match &aliased {
                            Some((other, _)) => {
                                self.join(&slot(function, &name), &slot(function, other))
                            }
                            // Nothing here watched this allocation being made.
                            // It came out of a call, an index, or a field of a
                            // type whose parts this compiler cannot walk - and a
                            // count belongs to the allocation, so a handle whose
                            // allocation is elsewhere is not one this analysis
                            // may lower.
                            None => match self.slot_of(function, value, scope) {
                                Some(source) => {
                                    self.join(&slot(function, &name), &source);
                                }
                                None => self.origin_unseen(function, &name, value),
                            },
                        }
                    }
                    scope.insert(name.clone(), ty);
                }
                self.expr(function, value, scope);
            }
            Stmt::Assign { target, value, .. } => {
                // `a = b` makes `a` a handle on `b`'s allocation - and `a` may
                // be a field of a struct as easily as a name.
                if let Some(target_slot) = self.slot_of(function, target, scope) {
                    match self.slot_of(function, value, scope) {
                        Some(source) => self.join(&target_slot, &source),
                        None => {
                            let (at, name) = split_slot(&target_slot);
                            self.origin_unseen(&at, &name, value)
                        }
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
                self.hands_back(function, value, scope);
                self.expr(function, value, scope);
            }
            Stmt::Return(None) => {}
            Stmt::Expr(value) => {
                // Part I 3.1: the last statement of a value-returning body is
                // the value, and joining it with the result slot costs nothing
                // where the function returns no `Shared`.
                self.hands_back(function, value, scope);
                self.expr(function, value, scope);
            }
        }
    }

    /// What a function hands back is the same allocation as whatever it names.
    fn hands_back(&mut self, function: &str, value: &Expr, scope: &BTreeMap<String, Ty>) {
        if let Some(source) = self.slot_of(function, value, scope) {
            self.join(&slot(function, RESULT), &source);
        }
    }

    /// A handle whose allocation this analysis did not watch being made.
    ///
    /// The floor holds and says so. It is [`Fallback::UnseenOrigin`], and it is
    /// also the seed that step 1 made necessary: since ADR-037 D6 the
    /// overlapping analysis may run a call on a thread of its own and hand the
    /// result back (ADR-005 §5.2's `task::both` row), so a `Shared` that came
    /// out of a call is a `Shared` that crosses.
    fn origin_unseen(&mut self, function: &str, name: &str, value: &Expr) {
        let from = match value {
            Expr::Call { func, .. } => match self.path_of(func) {
                Some(path) => format!("`{path}` hands it back"),
                None => "a call hands it back".to_string(),
            },
            Expr::MethodCall { method, .. } => {
                format!("`{}` hands it back", self.parsed.text(*method))
            }
            Expr::Field { name, .. } => {
                format!("it is read out of `{}`", self.parsed.text(*name))
            }
            Expr::Index { .. } => "it is read out of a collection".to_string(),
            _ => "it comes from an expression this analysis does not follow".to_string(),
        };
        self.force(
            function,
            name,
            format!(
                "{from}, so this analysis did not watch the allocation being made - and the \
                 count belongs to the allocation. A call handed to another thread hands its \
                 result back across one (ADR-033)"
            ),
            Some(Fallback::UnseenOrigin),
        );
    }

    /// A crossing reached this name: whatever count it owns, or owns through a
    /// field, must be atomic.
    ///
    /// Two ways a crossing lands. The name may *be* a `Shared`, and then its own
    /// class is forced. Or it may be a value of a type that **holds** one, and
    /// then the field's class is - which is `send::crossing`'s transitivity
    /// (ADR-029's walk through the ledger's `fields`) asked the other way round:
    /// there a field that may not cross makes the struct refuse, here a struct
    /// that crosses makes the field's count atomic.
    fn reached(
        &mut self,
        function: &str,
        name: &str,
        scope: &BTreeMap<String, Ty>,
        why: &str,
        fallback: Option<Fallback>,
    ) {
        let Some(ty) = scope.get(name).cloned() else {
            return;
        };
        if holds_shared(&ty) {
            self.force(function, name, why.to_string(), fallback);
            return;
        }
        for field in shared_fields(&ty.text(), self.own, self.library) {
            self.force(FIELDS, &field, why.to_string(), fallback);
        }
    }

    /// The handle an expression names, where it names one in this function's own
    /// scope.
    fn names_a_handle(&self, expr: &Expr, scope: &BTreeMap<String, Ty>) -> Option<(String, Ty)> {
        match expr {
            Expr::Variable(name) => {
                let name = self.parsed.text(*name).to_string();
                let ty = scope.get(&name)?;
                holds_shared(ty).then(|| (name, ty.clone()))
            }
            Expr::Block(block) => match block.stmts.last().map(|s| &s.node) {
                Some(Stmt::Expr(tail)) => self.names_a_handle(tail, scope),
                _ => None,
            },
            _ => None,
        }
    }

    /// The slot an expression names, where this analysis has one for it.
    ///
    /// A name in scope, or a **field** of a value whose type the ledger records:
    /// `c.hits` is the `Counter.hits` slot, which is the same slot the function
    /// that built the `Counter` joined its handle to. Without this, reading a
    /// handle back out of a field produced a fresh class nothing forced - a
    /// sibling of the guard `docs/rc-or-arc.md` §8 describes, and fail-open in
    /// the same direction.
    fn slot_of(
        &mut self,
        function: &str,
        expr: &Expr,
        scope: &BTreeMap<String, Ty>,
    ) -> Option<String> {
        if let Some((name, _)) = self.names_a_handle(expr, scope) {
            return Some(slot(function, &name));
        }
        match expr {
            Expr::Field { base, name } => {
                let base = self.names_the_type(base, scope)?;
                let field = self.parsed.text(*name).to_string();
                let ty = field_type(&base, &field, self.own, self.library)?;
                if !holds_shared(&ty) {
                    return None;
                }
                let key = format!("{base}.{field}");
                self.note(FIELDS, &key, ty, true, false);
                Some(slot(FIELDS, &key))
            }
            _ => None,
        }
    }

    /// The name of the type an expression has, where the scope says.
    fn names_the_type(&self, expr: &Expr, scope: &BTreeMap<String, Ty>) -> Option<String> {
        match expr {
            Expr::Variable(name) => {
                let name = self.parsed.text(*name).to_string();
                Some(scope.get(&name)?.text().trim_start_matches('&').to_string())
            }
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
                        None,
                    );
                }
                self.expr(function, body, scope);
            }
            Expr::Call { func, args, config } => {
                let callee = self.path_of(func);
                self.arguments(function, callee.as_deref(), false, args, config, scope);
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
                            self.reached(function, &name, &scope.clone(), &why, None);
                        }
                    }
                }
                // A method is resolved by name only (ADR-011 D2), so a method
                // nothing describes is a body this compiler cannot see the end
                // of - the same case as an unseen call, and answered the same
                // way.
                self.arguments(function, Some(&method), true, args, config, scope);
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
                    let field_slot = format!("{struct_name}.{field_name}");
                    match self.names_a_handle(&value, scope) {
                        Some((handle, ty)) => {
                            self.note(FIELDS, &field_slot, ty, true, false);
                            self.join(&slot(function, &handle), &slot(FIELDS, &field_slot));
                        }
                        None => {
                            // A `Shared`-holding field filled from something
                            // this analysis cannot follow. The floor holds for
                            // the field, and therefore for every handle that
                            // ever joins it.
                            let declared =
                                field_type(&struct_name, &field_name, self.own, self.library);
                            if let Some(ty) = declared.filter(holds_shared) {
                                self.note(FIELDS, &field_slot, ty, true, false);
                                match self.slot_of(function, &value, scope) {
                                    Some(source) => self.join(&source, &slot(FIELDS, &field_slot)),
                                    None => self.origin_unseen(FIELDS, &field_slot, &value),
                                }
                            }
                        }
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
    /// so the count is atomic.
    ///
    /// **Every name inside the argument counts, not only a bare variable.** An
    /// argument may be a lambda that *captures* a handle, or a tuple, or an
    /// expression with a handle somewhere in it, and a callee nothing describes
    /// may cross with any of them. `send::names_used` is the same
    /// over-approximate walk `spawn` uses, and over-approximate is the safe
    /// direction here too. Leaving it at `Expr::Variable` was a sibling of
    /// `docs/rc-or-arc.md` §8's guard: it read reasonably and it failed open.
    ///
    /// **And the options after the `;` are arguments.** `f(x; opt: handle)` hands
    /// a handle over as surely as `f(x, handle)` does, and no contract covers
    /// where it went, so it is [`Fallback::UncoveredArgument`].
    fn arguments(
        &mut self,
        function: &str,
        callee: Option<&str>,
        is_method: bool,
        args: &[Expr],
        config: &[crate::ast::ConfigArg],
        scope: &mut BTreeMap<String, Ty>,
    ) {
        let described = callee.and_then(|callee| self.parameters(callee));
        let kind = match is_method {
            true => Fallback::UnseenMethod,
            false => Fallback::UnseenCall,
        };
        for (at, arg) in args.iter().enumerate() {
            let Some((handle, _)) = self.names_a_handle(arg, scope) else {
                // Not a handle itself. It may hold one, capture one, or be an
                // expression with one inside it - and a callee nothing describes
                // may put any of those on a thread.
                if described.is_none() {
                    let why = unseen(callee);
                    for name in send::names_used(self.parsed, arg) {
                        self.reached(function, &name, &scope.clone(), &why, Some(kind));
                    }
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
                        Some(Fallback::UncoveredArgument),
                    ),
                },
                (None, callee_name) => {
                    self.force(function, &handle, unseen(callee_name), Some(kind))
                }
            }
        }
        for arg in config {
            for name in send::names_used(self.parsed, &arg.value) {
                self.reached(
                    function,
                    &name,
                    &scope.clone(),
                    &format!(
                        "it is handed to `{}` as the option `{}`, and no contract says what \
                         happens to a `Shared` in an option",
                        callee.unwrap_or("the callee"),
                        self.parsed.text(arg.name)
                    ),
                    Some(Fallback::UncoveredArgument),
                );
            }
            self.expr(function, &arg.value, scope);
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

    /// One pass over the seeds, then one answer per handle - and the summary.
    fn decide(mut self) -> Sharing {
        let mut atomic: BTreeMap<usize, (String, Option<Fallback>)> = BTreeMap::new();
        for (key, why, fallback) in std::mem::take(&mut self.forced) {
            let id = self.id(&key);
            let root = self.root(id);
            let entry = atomic.entry(root).or_insert((why.clone(), fallback));
            // Of two reasons, print the one that is a crossing rather than an
            // admission of ignorance: ADR-033 D9's rule, that of two true
            // answers the useful one is the one somebody can act on.
            if entry.1.is_some() && fallback.is_none() {
                *entry = (why, fallback);
            }
        }

        let keys: Vec<String> = self.handles.keys().cloned().collect();
        let mut decisions = Vec::new();
        let mut classes: BTreeMap<String, BTreeMap<usize, Class>> = BTreeMap::new();
        for key in keys {
            let id = self.id(&key);
            let root = self.root(id);
            let answer = atomic.get(&root).cloned();
            let handle = &self.handles[&key];
            let (count, why, fallback) = match answer {
                Some((why, fallback)) => (Count::Atomic, Some(why), fallback),
                None => (Count::Plain, None, None),
            };
            if handle.position {
                let entry = classes
                    .entry(handle.function.clone())
                    .or_default()
                    .entry(root)
                    .or_insert(Class {
                        members: Vec::new(),
                        count,
                    });
                entry.members.push(handle.value.clone());
                entry.count = entry.count.join(count);
            }
            if !handle.internal {
                decisions.push(Decision {
                    function: handle.function.clone(),
                    value: handle.value.clone(),
                    ty: handle.ty.text(),
                    count,
                    why,
                    fallback,
                });
            }
        }

        let summaries = classes
            .into_iter()
            .map(|(function, roots)| {
                let mut found: Vec<Class> = roots
                    .into_values()
                    .map(|mut class| {
                        class.members.sort();
                        class.members.dedup();
                        class
                    })
                    .collect();
                found.sort();
                (function, found)
            })
            .collect();

        Sharing {
            decisions,
            summaries,
        }
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

/// The sentence a published position gets.
fn published(key: &str, at: &str) -> String {
    match at == RESULT {
        true => format!(
            "`{key}` is public and hands a `Shared` back, so what the caller does with it is in \
             a unit this build cannot see"
        ),
        false => format!(
            "`{key}` is public, so its callers are in a unit this build cannot see, and one of \
             them may cross a thread with it"
        ),
    }
}

/// The sentence a call nothing describes gets.
fn unseen(callee: Option<&str>) -> String {
    format!(
        "nothing written down describes `{}`, so this compiler cannot see the end of it - and \
         starting a thread of its own is among the things it may do (ADR-038 D7)",
        callee.unwrap_or("the callee")
    )
}

/// `"Counter::record"` back into its two halves.
fn split_slot(key: &str) -> (String, String) {
    match key.rsplit_once("::") {
        Some((function, name)) => (function.to_string(), name.to_string()),
        None => (String::new(), key.to_string()),
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

/// What the ledgers say about a type, by the name a program writes.
fn described<'a>(
    ty: &str,
    own: &'a Ledger,
    library: &'a Ledger,
) -> Option<&'a super::TypeContract> {
    let name = ty.trim_start_matches('&');
    let suffix = format!("::{name}");
    [own, library].into_iter().find_map(|ledger| {
        ledger.types.get(name).or_else(|| {
            ledger
                .types
                .iter()
                .find(|(key, _)| key.ends_with(&suffix))
                .map(|(_, contract)| contract)
        })
    })
}

/// The `<struct>.<field>` slots a type reaches that hold a `Shared`, through the
/// ledger's `fields` (ADR-024, the walk ADR-029 established).
fn shared_fields(ty: &str, own: &Ledger, library: &Ledger) -> Vec<String> {
    let name = ty.trim_start_matches('&');
    let Some(contract) = described(ty, own, library) else {
        return Vec::new();
    };
    contract
        .fields
        .iter()
        .filter(|(_, ty)| holds_shared(ty))
        .map(|(field, _)| format!("{name}.{field}"))
        .collect()
}

/// One field's declared type, through the same lookup.
fn field_type(ty: &str, field: &str, own: &Ledger, library: &Ledger) -> Option<Ty> {
    described(ty, own, library)?
        .fields
        .iter()
        .find(|(name, _)| name == field)
        .map(|(_, ty)| ty.clone())
}

/// What `Shared[T]` lowers to, which since ADR-037 D6 is one type with one
/// optimisation on top of it.
///
/// Named here rather than in `emit` because **nothing emits it**: the emitter has
/// no `Rc::new` and no `Arc::new`, and the grammar has no way to construct a
/// `Shared`. D6 decides that the floor is `std::sync::Arc`; D7 decides that a
/// value this analysis proves never crosses may be `std::rc::Rc` instead.
pub fn rust_name(count: Count) -> &'static str {
    match count {
        Count::Plain => "std::rc::Rc",
        Count::Atomic => "std::sync::Arc",
    }
}
