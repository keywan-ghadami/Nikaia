// crates/nikaia/src/contracts/mod.rs
//
// The Borrow Contract Ledger (Part III, 13.5; ADR-005 D3; ADR-020).
//
// What a caller has to know about a function it cannot see the body of: does it
// pause, can it fail, and does what it hands back point into what it was given.
// Part III 13.5 specifies a file that records exactly that, derived rather than
// written, committed like a lockfile, and **shipped with a published package**
// so that a consumer builds against contracts instead of guesses.
//
// This is that file, for what Stage 0 knows, and the three answers come from
// three different places.
//
// `throws` is *declared* in the source and recorded exactly. The borrow
// contract is *inferred from the signature* - the widest one a signature can
// support - rather than by the whole-program analysis ADR-005 D3 describes.
// `sync` is *inferred from the body* (ADR-027): a function that provably cannot
// pause gets the promise whether or not anyone wrote the word, and where the
// word is written it stays an assertion for `NK2202` to check.
//
// That is why the header names the inference that produced the ledger. A later
// compiler that infers more will write a different name there, and `--locked`
// will say so rather than quietly accepting the weaker answer.

pub mod order;
pub mod send;
pub mod sync;
pub mod throws;
pub mod touch;
pub mod trust;
pub mod ty;

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{anyhow, Result};

use crate::ast::Item;
use crate::emit::{borrowing_structs, holds_view, names_borrowing};
use crate::parser::Parsed;

/// The contracts `std` ships, as the library ships them.
///
/// Part III 13.5 has a consumer read a package's ledger from the package.
/// Stage 0's compiler and `std` ship together, so the file is embedded here
/// rather than looked up - the same file, read at build time instead of at run
/// time, and the same one a reviewer reads.
pub const STD: &str = include_str!("../../../nikaia-std/std.contracts");

/// The inference this ledger was produced by.
///
/// Recorded in the header so that a ledger can say what it knows, and a ledger
/// produced by reading signatures must not be mistaken for one produced by
/// reading bodies. Stage 0 reads signatures for the borrow contract and for
/// `throws`; since ADR-027 it reads **bodies** for `sync` and since ADR-023 D1
/// for the *errors* a `throws` names, and the name says which half is which.
/// The whole-program analysis of ADR-005 D3 will read bodies for the borrow
/// contract too and will write a different name again.
pub const INFERENCE: &str = "stage0-signatures+sync-bodies+throws-bodies";

/// The format version of the file itself.
///
/// 2 since [ADR-023](../../../../docs/specification/adr/adr-023.md) D1: `throws`
/// was a boolean and is a list of the errors that can leave the function. A
/// version 1 file still reads - `true` is taken as `["?"]`, which is what it
/// always meant - and the version is what tells a *reader* that the file it has
/// may say more than it knows how to use.
pub const VERSION: u32 = 2;

/// Who supplied the bytes a source hands back (ADR-010 D1).
///
/// A two-state lattice, `Trusted ⊑ Untrusted`, joined in the safe direction:
/// one untrusted input makes the result untrusted. Where provenance cannot be
/// established the answer is `Untrusted`, never `Trusted` - an analysis that
/// fails open is a vulnerability generator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum Provenance {
    /// The operator chose these bytes: files, arguments, the environment,
    /// anything compiled in.
    #[default]
    Trusted,
    /// Someone else chose these bytes: a remote peer, a socket, a database row
    /// holding what a user stored yesterday.
    Untrusted,
}

impl Provenance {
    /// The more cautious of two, which is what a container takes from what goes
    /// into it.
    pub fn join(self, other: Self) -> Self {
        self.max(other)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Provenance::Trusted => "trusted",
            Provenance::Untrusted => "untrusted",
        }
    }
}

/// What the ledger knows about a function's suspension behaviour (Part II, 12.1).
///
/// **Three states, and the third one is why this is not a `bool`.** A function
/// may be `sync` because someone wrote the word, or because nothing it calls
/// can pause. Both are true, and a caller uses them the same way - but a
/// **diff** must not treat them alike. Losing an asserted `sync` is a promise
/// being withdrawn and someone has to have meant it; losing an inferred one is
/// a consequence of an edit somewhere else, and the compiler should say which
/// happened rather than print the same line for both (ADR-027 D3).
///
/// The two also fail differently. An assertion is *checked* - `NK2202` reports
/// the calls that contradict it - and the check is conservative in the
/// permissive direction: it rejects only what it can prove wrong. The inference
/// runs the other way and claims `sync` only where it can prove it right. That
/// is deliberate and it is the whole safety argument: a wrong `sync` in a
/// shipped ledger lets a caller put a pausing body inside `access`, and the
/// ledger has said before what to do about an analysis that cannot decide -
/// "an analysis that fails open is a vulnerability generator" (`Provenance`,
/// above). So it fails closed, and the gap between the two polarities is
/// exactly where a person writes `sync` by hand and gets it checked.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Sync {
    /// Not `sync`: something it calls can pause, or something it calls cannot
    /// be resolved and therefore cannot be vouched for.
    #[default]
    No,
    /// Nothing it calls can pause, and every call it makes was resolvable.
    /// Derived from the body, so an edit elsewhere can take it away.
    Inferred,
    /// Written in the source. `NK2202` is what happens when the body
    /// contradicts it.
    Asserted,
    /// **It does whatever the lambda it is given does** - `sync = "from(f)"`,
    /// naming the parameter that decides (ADR-029).
    ///
    /// `xs.map fn { a + 1 }` cannot pause and `xs.map fn { io::read()… }` can,
    /// and they are the same `map`. Without this a higher-order function has to
    /// commit to one answer for every caller, and the honest one is the
    /// pessimistic one - so no `map`, `filter` or `and_modify` could appear
    /// inside `access` or `par_iter`, whatever its lambda did.
    ///
    /// **A caller reads this as "this call adds no pausing of its own"**, and
    /// that is sound for one reason: the lambda runs *during* the call, so its
    /// body is part of the function that writes it, and its calls are already
    /// counted there (Part I, 5.4's `@immediate`). A parameter the callee
    /// **stores or spawns** - `@detached` - would break that, because then the
    /// lambda's calls belong to nobody the caller is counting. The ledger
    /// cannot spell `@detached` yet, so the rule is written down instead:
    /// `from` is for a lambda that runs before the call returns, and
    /// `a_detached_lambda_may_not_use_from` in `tests/contracts.rs` is what
    /// stops the one `std` entry that could get this wrong.
    From(String),
}

impl Sync {
    /// Whether a caller may treat it as `sync` - which is the question every
    /// caller actually has, and the one place the two positive states are
    /// deliberately the same.
    pub fn is_sync(&self) -> bool {
        !matches!(self, Sync::No)
    }

    /// The parameter that decides, where one does.
    pub fn from(&self) -> Option<&str> {
        match self {
            Sync::From(name) => Some(name),
            _ => None,
        }
    }
}

/// What a caller needs to know about one function.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FnContract {
    /// Callable from outside the unit that declares it. A library's consumers
    /// see only these; the unit's own checks use all of them.
    pub public: bool,
    /// Part II, 12.1: pure computation, cannot pause, cannot do I/O.
    ///
    /// **Absent means not `sync`.** That is the whole of what makes this file
    /// worth shipping: a caller that finds no `sync` here knows the callee may
    /// pause, where a caller that finds no *entry* knows nothing at all.
    ///
    /// Present, it says which of the two kinds it is - see [`Sync`].
    pub sync: Sync,
    /// Kap 7.1: it may fail, and **with what** - the error types that can leave
    /// it, inferred over the call graph ([ADR-023](../../../../docs/specification/adr/adr-023.md) D1).
    ///
    /// Empty means it cannot fail, which is why nothing is written for it: the
    /// file says only what is true. A `"?"` among the names is the absence of a
    /// claim in ADR-024 D1's sense - this function fails with something the
    /// compiler cannot name, today because `std`'s failures are Rust's and have
    /// no Nikaia type. A caller reads `["?"]` as "it fails" and gets exactly
    /// what the old boolean gave, which is what makes the change additive.
    ///
    /// Sorted, because 13.5 makes the file a pure function of (source,
    /// toolchain) and `--locked` compares it byte for byte.
    pub throws: Vec<String>,
    /// Which resources it reaches, and whether it changes them (ADR-033).
    ///
    /// **Empty means it touches everything**, which is the opposite of how
    /// `throws` above reads an empty list and is deliberate: an unknown effect
    /// has to order against everything, or a missing contract would make a
    /// program wrong rather than merely slow. [`touches_known`] is the bit that
    /// tells "nobody said" from "it says it touches nothing".
    pub touches: Vec<touch::Touch>,
    /// Whether the `touches` above is an answer at all.
    ///
    /// A function that genuinely reaches nothing writes `touches = []`, and
    /// that is a *claim* - it may overlap with anything. A function nobody
    /// described has no key, and that is the absence of one.
    pub touches_known: bool,
    /// This function is a **source**: its result is bytes that entered the
    /// program from outside, and this is who chose them (ADR-010 D2).
    ///
    /// `None` is not "trusted" - it is "this is not a source", which is what
    /// almost every function is.
    pub provenance: Option<Provenance>,
    /// The signature, as the source writes it: `(path: &str) -> String`.
    ///
    /// One key rather than two, because that is how a person reads a function
    /// and because the parameters and the result are one fact. A `self`
    /// receiver is in it where there is one, so a method's arguments are the
    /// parameters after the first.
    ///
    /// This is what makes a *type* checker possible across a boundary it cannot
    /// see the body of - which ADR-020 predicted would be an extension of this
    /// file rather than a new one.
    pub signature: Option<Signature>,
    /// The parameters the result may point into, in declaration order.
    ///
    /// Empty when the result holds no view. Stage 0 has one input lifetime, so
    /// a result that is a view may point into *any* view it was given -
    /// `borrows(a | b)`, which is the spec's own spelling and is the widest
    /// contract the signature can support.
    pub borrows: Vec<String>,
}

/// A function's parameters and result.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Signature {
    /// Name and type, in order. A `self` receiver is the first of them where
    /// there is one, named `self`.
    pub params: Vec<(String, ty::Ty)>,
    /// Kap 5.1: what stands after the `;` - options, named at the call.
    ///
    /// A caller needs all three parts: the name, so it can be written; the
    /// type, so it can be checked; and the **default**, because a call that
    /// leaves an option out still passes a value, and only the declaration
    /// knows which. That is what makes this a ledger key rather than something
    /// a caller could work out.
    pub config: Vec<ConfigContract>,
    /// What it hands back. `None` where it hands back nothing.
    pub result: Option<ty::Ty>,
}

/// One option of a function, as a caller has to know it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigContract {
    pub name: String,
    pub ty: ty::Ty,
    /// The default, as the source writes it - a literal, and therefore text
    /// that the language below spells the same way (ADR-011 D2).
    pub default: String,
}

impl Signature {
    /// The arguments a *call* passes, which is the parameters after a receiver.
    pub fn arguments(&self) -> &[(String, ty::Ty)] {
        match self.params.first() {
            Some((name, _)) if name == "self" => &self.params[1..],
            _ => &self.params,
        }
    }

    /// What a call to it hands back.
    ///
    /// A function with no `->` hands back nothing, and nothing is a type: the
    /// empty tuple, which is what makes `let n: i32 = print(x)` a mistake the
    /// checker can see rather than one only `rustc` finds.
    pub fn result_or_unit(&self) -> ty::Ty {
        self.result.clone().unwrap_or(ty::Ty::Tuple(Vec::new()))
    }

    pub fn text(&self) -> String {
        let mut params: Vec<String> = self
            .params
            .iter()
            .map(|(name, ty)| {
                if name == "self" {
                    ty.text()
                } else {
                    format!("{name}: {}", ty.text())
                }
            })
            .collect();
        let mut inside = params.join(", ");
        if !self.config.is_empty() {
            let config: Vec<String> = self
                .config
                .iter()
                .map(|c| format!("{}: {} = {}", c.name, c.ty.text(), c.default))
                .collect();
            inside = format!("{inside}; {}", config.join(", "));
        }
        params.clear();
        match &self.result {
            Some(result) => format!("({inside}) -> {}", result.text()),
            None => format!("({inside})"),
        }
    }

    /// Read one back from the text above.
    pub fn parse(text: &str) -> Result<Signature> {
        let text = text.trim();
        let close = text
            .rfind(')')
            .ok_or_else(|| anyhow!("a signature is `(…) -> T`, found `{text}`"))?;
        let inside = text
            .strip_prefix('(')
            .map(|t| &t[..close - 1])
            .ok_or_else(|| anyhow!("a signature starts with `(`, found `{text}`"))?;

        // Kap 5.1: the `;` divides the subjects from the options.
        let (positional, options) = match split_config(inside) {
            (positional, Some(options)) => (positional, options),
            (positional, None) => (positional, ""),
        };

        let params = ty::split_args(positional)
            .iter()
            .map(|part| match part.split_once(':') {
                Some((name, ty)) => (name.trim().to_string(), ty::Ty::parse(ty)),
                // A bare type is the receiver, which is what `&mut self` is.
                None => ("self".to_string(), ty::Ty::parse(part)),
            })
            .collect();

        let config = ty::split_args(options)
            .iter()
            .map(|part| {
                let (name, rest) = part
                    .split_once(':')
                    .ok_or_else(|| anyhow!("an option is `name: T = value`, found `{part}`"))?;
                let (ty, default) = rest.split_once('=').ok_or_else(|| {
                    anyhow!("an option needs a default: `{}: T = value`", name.trim())
                })?;
                Ok(ConfigContract {
                    name: name.trim().to_string(),
                    ty: ty::Ty::parse(ty),
                    default: default.trim().to_string(),
                })
            })
            .collect::<Result<Vec<_>>>()?;

        let result = text[close + 1..]
            .trim()
            .strip_prefix("->")
            .map(ty::Ty::parse);

        Ok(Signature {
            params,
            config,
            result,
        })
    }
}

/// What a caller needs to know about one type.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TypeContract {
    pub public: bool,
    /// ADR-008 D6: `@borrowed` was asserted in the source.
    pub borrowed: bool,
    /// Every field, with its type - what a checker needs to say that `r.nmae`
    /// is not a field of `Row`.
    pub fields: Vec<(String, ty::Ty)>,
    /// A value of this type **may cross a thread** (ADR-005 §1 Group B).
    ///
    /// Written by hand and never inferred, because it only ever answers for a
    /// type whose parts this compiler cannot see: a Nikaia `struct` records its
    /// `fields`, and `contracts::send` walks those - structurally, which is what
    /// Group B asks for and what cannot be wrong. A Rust type has no fields
    /// here, so without this key it is *undecided*, which is not permission
    /// (ADR-010 D1): the compiler will not put it on a thread of its own
    /// choosing.
    ///
    /// So this is the same kind of line as `sync = true` on a Rust function - a
    /// promise about a body this compiler does not read, written in the file
    /// that ships and reviewed like code. Its absence is never "it may not"; it
    /// is "nobody said", and the two differ in who is to blame.
    pub crosses: bool,
    /// Iterating a value of this type can **fail** (ADR-025 D6).
    ///
    /// A `for` over one is a place the enclosing function can fail from, and
    /// the compiler makes that function declare `throws` (D1). It is recorded
    /// here rather than inferred because the types that have it are `std`'s and
    /// their bodies are Rust - which is the whole reason this file exists.
    pub iterates_fallibly: bool,
    /// The fields that hold a view, directly or through another type that
    /// does. A struct with none of these is free of the input; one with any is
    /// tied to it for as long as it lives (Part II, 10.6).
    pub tethered: Vec<String>,
}

/// One compilation unit's contracts.
///
/// Ordered maps throughout, because the file is a **pure function of (source,
/// toolchain)** (13.5) and a hash map's iteration order is not. The
/// determinism is the feature: `--locked` compares bytes, so it needs no
/// tolerance and no semantic comparison.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Ledger {
    pub version: u32,
    pub toolchain: String,
    pub inference: String,
    pub functions: BTreeMap<String, FnContract>,
    pub types: BTreeMap<String, TypeContract>,
}

impl Ledger {
    /// The contracts of a parsed program.
    ///
    /// Two passes, because the second needs the first to have finished. The
    /// item loop records what each declaration *says*; then [`sync::infer`]
    /// reads the bodies and gives `sync` to what earns it, which it can only do
    /// once every function in the unit has an entry to be looked up in.
    ///
    /// The library it resolves calls against is `std`'s shipped ledger, and it
    /// is not a parameter on purpose. 13.5 makes this file a pure function of
    /// (source, toolchain); a ledger inferred against a *different* library
    /// would be a different file for the same source, and `--locked` compares
    /// bytes. The compiler and `std` ship together, so there is exactly one
    /// answer here and no way to pass the wrong one.
    /// A ledger with a header and nothing in it - what a program of several
    /// files starts from before it absorbs its modules.
    pub fn empty() -> Self {
        Ledger {
            version: VERSION,
            toolchain: toolchain(),
            inference: INFERENCE.to_string(),
            ..Default::default()
        }
    }

    pub fn infer(parsed: &Parsed) -> Self {
        Self::infer_checked(parsed).0
    }

    /// Every entry of `other`, under `module::`.
    ///
    /// A program of several files has **one** ledger (Part III, 13.5 puts it at
    /// the project root), and its keys are qualified exactly the way a caller
    /// writes them - which is the shape `std.contracts` has had since ADR-020:
    /// `fs::map`, `io::lines`, `text::digit_value`.
    ///
    /// That is what makes multi-file compilation almost free for everything
    /// built on the ledger. The type checker, the `sync` check, the provenance
    /// analysis, Kap 5.1's options and ADR-025's fallible loops all resolve a
    /// call by asking a ledger for `a::b`. None of them learns a new trick to
    /// work across files; the file they ask simply has more in it.
    pub fn absorb(&mut self, module: Option<&str>, other: Ledger) {
        for (name, contract) in other.functions {
            let key = match module {
                Some(module) => format!("{module}::{name}"),
                None => name,
            };
            self.functions.insert(key, contract);
        }
        for (name, contract) in other.types {
            let key = match module {
                Some(module) => format!("{module}::{name}"),
                None => name,
            };
            self.types.insert(key, contract);
        }
    }

    /// The contracts, and the type checker's pass that helped produce them.
    ///
    /// Three steps, and the order is forced:
    ///
    /// 1. read the declarations, so every function has an entry to be looked
    ///    up in;
    /// 2. run the **type checker** against that, which resolves each method
    ///    call to the function it goes to (ADR-028);
    /// 3. infer `sync` from the bodies, using both.
    ///
    /// **Step 2 does not depend on step 3, and that is what makes this sound
    /// rather than circular.** The checker reads `signature`, `fields` and
    /// `iterates` from a ledger and never `sync`, so resolving a method against
    /// the step-1 ledger gives the same answer as resolving it against the
    /// finished one. `the_checker_does_not_depend_on_the_sync_it_helps_infer`
    /// in `tests/contracts.rs` is that invariant, held to.
    ///
    /// The pass is handed back rather than thrown away because the compiler
    /// wants it too - it carries the findings and the fallible loops - and
    /// running the checker twice per build to get one of them would be waste,
    /// not caution.
    pub fn infer_checked(parsed: &Parsed) -> (Self, crate::check::Checked) {
        let mut ledger = Ledger {
            version: VERSION,
            toolchain: toolchain(),
            inference: INFERENCE.to_string(),
            ..Default::default()
        };

        let borrowing = borrowing_structs(parsed);

        for item in &parsed.program.items {
            match &item.node {
                Item::Fn { .. } => {
                    let (name, contract) =
                        ledger.function(parsed, &item.node, None, &BTreeSet::new());
                    ledger.functions.insert(name, contract);
                }
                Item::Impl {
                    target, methods, ..
                } => {
                    // `impl Stack[T]` puts `T` in scope for every method in it,
                    // so it is a name that stands for a type there too.
                    let outer: BTreeSet<String> = target
                        .generics
                        .iter()
                        .filter(|g| g.generics.is_empty() && !g.is_tuple)
                        .map(|g| parsed.text(g.name).to_string())
                        .collect();
                    let target = parsed.text(target.name).to_string();
                    for method in methods {
                        let (name, contract) =
                            ledger.function(parsed, &method.node, Some(&target), &outer);
                        ledger.functions.insert(name, contract);
                    }
                }
                Item::Struct {
                    name,
                    generics,
                    fields,
                    is_public,
                    is_borrowed,
                    ..
                } => {
                    let parameters: BTreeSet<String> = generics
                        .iter()
                        .map(|g| parsed.text(g.name).to_string())
                        .collect();
                    let tethered = fields
                        .iter()
                        .filter(|f| holds_view(&f.ty) || names_borrowing(&f.ty, &borrowing))
                        .map(|f| parsed.text(f.name).to_string())
                        .collect();
                    let field_types = fields
                        .iter()
                        .map(|f| {
                            (
                                parsed.text(f.name).to_string(),
                                ty::Ty::from_ast(parsed, &f.ty).erase(&parameters),
                            )
                        })
                        .collect();
                    ledger.types.insert(
                        parsed.text(*name).to_string(),
                        TypeContract {
                            public: *is_public,
                            borrowed: *is_borrowed,
                            fields: field_types,
                            // Never inferred: a `struct` declared here records
                            // its fields, and `contracts::send` walks those.
                            // The key exists for types whose parts are Rust.
                            crosses: false,
                            // Nothing a `.nika` file declares iterates at all
                            // yet, let alone fallibly: the types that do are
                            // `std`'s, and `std` writes them down (ADR-025 D6).
                            iterates_fallibly: false,
                            tethered,
                        },
                    );
                }
                _ => {}
            }
        }

        let checked = crate::check::check(parsed, &ledger, std_ledger());
        sync::infer(&mut ledger, parsed, std_ledger(), &checked.methods);
        // Kap 7.1: `throws` in the source says *that* it fails; this says with
        // what (ADR-023 D1). After `sync`, because both read bodies and only
        // this one needs nothing from the other.
        throws::infer(&mut ledger, parsed, std_ledger());
        (ledger, checked)
    }

    /// One function's entry, named as a caller would reach it.
    fn function(
        &self,
        parsed: &Parsed,
        item: &Item,
        target: Option<&str>,
        outer: &BTreeSet<String>,
    ) -> (String, FnContract) {
        let Item::Fn {
            name,
            generics,
            args,
            config,
            ret_type,
            is_sync,
            is_public,
            throws,
            ..
        } = item
        else {
            unreachable!("only a function is passed here");
        };

        // A generic parameter is a name that stands for a type rather than
        // being one. The ledger records `?` for it, because `?` is what a
        // caller actually knows.
        let mut parameters = outer.clone();
        parameters.extend(generics.iter().map(|g| parsed.text(g.name).to_string()));

        // The anonymous constructor of Kap 4.2 is `Type::new` to a caller,
        // because that is what the lowering names it.
        let own = match name {
            Some(name) => parsed.text(*name).to_string(),
            None => "new".to_string(),
        };
        let key = match target {
            Some(target) => format!("{target}::{own}"),
            None => own,
        };

        let returns_view = ret_type.as_ref().is_some_and(holds_view);
        let borrows = if returns_view {
            args.iter()
                .filter(|a| holds_view(&a.ty))
                .map(|a| parsed.text(a.name).to_string())
                .collect()
        } else {
            Vec::new()
        };

        // A method's receiver is a parameter named `self`, so a caller reads
        // the arguments off the same list either way.
        let mut params: Vec<(String, ty::Ty)> = Vec::new();
        if let Item::Fn {
            receiver: Some(receiver),
            ..
        } = item
        {
            params.push(("self".to_string(), receiver_type(parsed, receiver, target)));
        }
        params.extend(args.iter().map(|a| {
            (
                parsed.text(a.name).to_string(),
                ty::Ty::from_ast(parsed, &a.ty).erase(&parameters),
            )
        }));

        (
            key,
            FnContract {
                public: *is_public,
                // What the *declaration* says. `sync::infer` reads the body
                // afterwards and may raise a `No` to `Inferred`; it never
                // touches this one, because an assertion is what `NK2202`
                // exists to contradict.
                sync: if *is_sync { Sync::Asserted } else { Sync::No },
                // Nothing a `.nika` file declares reaches a resource this
                // vocabulary can name yet: `fs` and `io` are `std`'s, and
                // `std` writes its own down (ADR-033 §6). An empty list with
                // `touches_known` false is "nobody said", which orders.
                touches: Vec::new(),
                touches_known: false,
                // The *declaration* says only that it can fail. Which errors
                // is a question about the body and about everything the body
                // reaches, so `throws::infer` answers it afterwards - the same
                // arrangement `sync` has since ADR-027.
                throws: if *throws {
                    vec![UNNAMED_ERROR.to_string()]
                } else {
                    Vec::new()
                },
                signature: Some(Signature {
                    params,
                    config: config
                        .iter()
                        .map(|c| ConfigContract {
                            name: parsed.text(c.name).to_string(),
                            ty: ty::Ty::from_ast(parsed, &c.ty).erase(&parameters),
                            default: literal_text(parsed, &c.default),
                        })
                        .collect(),
                    result: ret_type
                        .as_ref()
                        .map(|t| ty::Ty::from_ast(parsed, t).erase(&parameters)),
                }),
                borrows,
                // A source is where bytes enter the program from outside, and
                // nothing a `.nika` file can write is one: `fs` and `io` are
                // `std`, and `std` states its own (ADR-010 D2).
                provenance: None,
            },
        )
    }

    /// A function by the name a caller wrote, or by the name the prelude makes
    /// available unqualified.
    ///
    /// Matching on the last segment is name-for-name resolution (ADR-011 D2)
    /// rather than import tracking, and it is what a compiler without a module
    /// graph can honestly do.
    pub fn lookup(&self, name: &str) -> Option<(String, &FnContract)> {
        if let Some(contract) = self.functions.get(name) {
            return Some((name.to_string(), contract));
        }
        if name.contains("::") {
            return None;
        }
        let suffix = format!("::{name}");
        self.functions
            .iter()
            .find(|(key, _)| key.ends_with(&suffix))
            .map(|(key, contract)| (key.clone(), contract))
    }

    /// Every entry a bare method name could resolve to.
    ///
    /// Which one `xs.len()` *is* depends on what `xs` is, and that is the type
    /// checker's answer rather than this file's (ADR-028). But a question
    /// weaker than "which entry" can be answered without it: if **every**
    /// `::len` in the ledger reaches nothing, then `xs.len()` reaches nothing
    /// whatever `xs` turns out to be. An over-approximation over the
    /// candidates, which is the direction ADR-033 D4 requires.
    pub fn candidates(&self, method: &str) -> Vec<(&str, &FnContract)> {
        let suffix = format!("::{method}");
        self.functions
            .iter()
            .filter(|(key, _)| key.ends_with(&suffix))
            .map(|(key, contract)| (key.as_str(), contract))
            .collect()
    }

    /// The file, as it is written out.
    ///
    /// Only what is *true* is recorded: a `sync = false` on every entry would
    /// treble the file and say nothing, and a diff should show a promise being
    /// made or withdrawn rather than a column of falses.
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str("# AUTO-GENERATED by `nikaia`. Commit this file like a lockfile.\n");
        out.push_str("# Do not edit by hand - it is regenerated on every build.\n");
        out.push_str("#\n");
        out.push_str("# What a caller has to know about a function it cannot see the body of:\n");
        out.push_str("# whether it may pause (`sync`), whether it may fail and with what\n");
        out.push_str(
            "# (`throws`), and what its result may point into (`returns`). Part III, 13.5.\n",
        );
        out.push_str("#\n");
        out.push_str("# A `\"?\"` among the errors is the absence of a claim: it fails, with\n");
        out.push_str("# something this compiler cannot name.\n");
        out.push_str(&format!("version = {}\n", self.version));
        out.push_str(&format!("toolchain = \"{}\"\n", self.toolchain));
        out.push_str(&format!("inference = \"{}\"\n", self.inference));

        for (name, contract) in &self.functions {
            out.push_str(&format!("\n[fn.\"{name}\"]\n"));
            if contract.public {
                out.push_str("pub = true\n");
            }
            // `true` is the promise the source made, `"inferred"` the one the
            // body implies. Absent is still "not `sync`", so a reader that only
            // asks `is_sync` reads this file exactly as it did before.
            match &contract.sync {
                Sync::Asserted => out.push_str("sync = true\n"),
                Sync::Inferred => out.push_str("sync = \"inferred\"\n"),
                Sync::From(name) => out.push_str(&format!("sync = \"from({name})\"\n")),
                Sync::No => {}
            }
            if !contract.throws.is_empty() {
                let names = contract
                    .throws
                    .iter()
                    .map(|e| format!("\"{e}\""))
                    .collect::<Vec<_>>()
                    .join(", ");
                out.push_str(&format!("throws = [{names}]\n"));
            }
            if !contract.borrows.is_empty() {
                out.push_str(&format!(
                    "returns = \"borrows({})\"\n",
                    contract.borrows.join(" | ")
                ));
            }
            if contract.touches_known {
                out.push_str(&format!(
                    "touches = [{}]\n",
                    contract
                        .touches
                        .iter()
                        .map(|t: &touch::Touch| format!("\"{}\"", t.text()))
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
            if let Some(provenance) = contract.provenance {
                out.push_str(&format!("provenance = \"{}\"\n", provenance.as_str()));
            }
            if let Some(signature) = &contract.signature {
                out.push_str(&format!("signature = \"{}\"\n", escape(&signature.text())));
            }
        }

        for (name, contract) in &self.types {
            out.push_str(&format!("\n[type.\"{name}\"]\n"));
            if contract.public {
                out.push_str("pub = true\n");
            }
            if contract.borrowed {
                out.push_str("borrowed = true\n");
            }
            if !contract.fields.is_empty() {
                out.push_str(&format!(
                    "fields = [{}]\n",
                    contract
                        .fields
                        .iter()
                        .map(|(name, ty)| format!("\"{name}: {}\"", ty.text()))
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
            if contract.crosses {
                out.push_str("crosses = true\n");
            }
            if contract.iterates_fallibly {
                out.push_str("iterates = \"throws\"\n");
            }
            if !contract.tethered.is_empty() {
                out.push_str(&format!(
                    "tethered = [{}]\n",
                    contract
                        .tethered
                        .iter()
                        .map(|f| format!("\"{f}\""))
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
        }

        out
    }

    /// Read a ledger back - a library's, or this project's own.
    ///
    /// Deliberately a small reader for the small format `render` writes rather
    /// than a TOML parser: the file is generated, so the shapes it can take are
    /// the shapes written above, and a dependency to read one's own output back
    /// is a dependency to keep in step.
    pub fn parse(text: &str) -> Result<Self> {
        let mut ledger = Ledger::default();
        let mut section: Option<(bool, String)> = None;

        for (n, line) in text.lines().enumerate() {
            let line = line.trim();
            let at = || n + 1;
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            if let Some(rest) = line.strip_prefix("[fn.\"") {
                section = Some((true, quoted(rest, "]", at())?));
                ledger
                    .functions
                    .entry(section.as_ref().expect("just set").1.clone())
                    .or_default();
                continue;
            }
            if let Some(rest) = line.strip_prefix("[type.\"") {
                section = Some((false, quoted(rest, "]", at())?));
                ledger
                    .types
                    .entry(section.as_ref().expect("just set").1.clone())
                    .or_default();
                continue;
            }

            let (key, value) = line
                .split_once('=')
                .ok_or_else(|| anyhow!("line {}: not a key and a value: {line}", at()))?;
            let (key, value) = (key.trim(), value.trim());

            match (&section, key) {
                (None, "version") => ledger.version = value.parse()?,
                (None, "toolchain") => ledger.toolchain = unquote(value, at())?,
                (None, "inference") => ledger.inference = unquote(value, at())?,
                (None, _) => return Err(anyhow!("line {}: unknown header key `{key}`", at())),

                (Some((true, name)), _) => {
                    let entry = ledger.functions.entry(name.clone()).or_default();
                    match key {
                        "pub" => entry.public = value == "true",
                        "sync" => entry.sync = sync_of(value, at())?,
                        "throws" => entry.throws = throws_of(value, at())?,
                        "returns" => entry.borrows = borrows_of(&unquote(value, at())?, at())?,
                        "touches" => {
                            entry.touches = string_list(value, at())?
                                .iter()
                                .map(|t| touch::Touch::parse(t))
                                .collect::<Result<Vec<_>>>()?;
                            entry.touches_known = true;
                        }
                        "provenance" => {
                            entry.provenance = Some(provenance_of(&unquote(value, at())?, at())?)
                        }
                        "signature" => {
                            entry.signature = Some(Signature::parse(&unquote(value, at())?)?)
                        }
                        _ => return Err(anyhow!("line {}: unknown key `{key}` on a fn", at())),
                    }
                }
                (Some((false, name)), _) => {
                    let entry = ledger.types.entry(name.clone()).or_default();
                    match key {
                        "pub" => entry.public = value == "true",
                        "borrowed" => entry.borrowed = value == "true",
                        "crosses" => entry.crosses = value == "true",
                        "tethered" => entry.tethered = string_list(value, at())?,
                        "iterates" => {
                            let value = unquote(value, at())?;
                            if value != "throws" {
                                return Err(anyhow!(
                                    "line {}: `iterates` is `throws` and nothing else, \
                                     not `{value}` - a step that cannot fail says nothing",
                                    at()
                                ));
                            }
                            entry.iterates_fallibly = true;
                        }
                        "fields" => {
                            entry.fields = string_list(value, at())?
                                .iter()
                                .map(|field| match field.split_once(':') {
                                    Some((name, ty)) => {
                                        (name.trim().to_string(), ty::Ty::parse(ty))
                                    }
                                    None => (field.trim().to_string(), ty::Ty::Unknown),
                                })
                                .collect()
                        }
                        _ => return Err(anyhow!("line {}: unknown key `{key}` on a type", at())),
                    }
                }
            }
        }

        Ok(ledger)
    }
}

/// `std`'s ledger, parsed once.
///
/// Embedded at build time and parsed on first use rather than on every
/// inference. It failing to parse is a broken compiler, not a broken program:
/// `crates/nikaia/tests/contracts.rs` reads the same bytes and would have said
/// so long before a user got here.
fn std_ledger() -> &'static Ledger {
    static PARSED: std::sync::OnceLock<Ledger> = std::sync::OnceLock::new();
    PARSED.get_or_init(|| Ledger::parse(STD).expect("std ships a ledger this compiler can read"))
}

/// The receiver's type, as a caller sees it: the type the `impl` is for, with
/// the `&` the receiver was written with.
fn receiver_type(parsed: &Parsed, receiver: &crate::ast::Receiver, target: Option<&str>) -> ty::Ty {
    let _ = parsed;
    let name = target.unwrap_or("Self");
    if receiver.is_ref {
        ty::Ty::view(name)
    } else {
        ty::Ty::named(name)
    }
}

/// What produced the contracts: this compiler, not the one it emits Rust for.
///
/// `sync`, `throws` and the borrow contract are decided here and never by
/// `rustc`, so the version that matters to a ledger diff is Nikaia's.
fn toolchain() -> String {
    format!("nikaia {}", env!("CARGO_PKG_VERSION"))
}

/// A literal, written back the way the source wrote it.
///
/// Only a literal reaches here - the grammar allows nothing else as a default -
/// and Nikaia spells every one of them the way the language below does
/// (ADR-011 D2), which is what lets a ledger record the text and an emitter
/// print it.
fn literal_text(parsed: &Parsed, expr: &crate::ast::Expr) -> String {
    use crate::ast::{Expr, UnaryOp};
    let _ = parsed;
    match expr {
        Expr::LitBool(true) => "true".to_string(),
        Expr::LitBool(false) => "false".to_string(),
        Expr::LitInt(n) => n.to_string(),
        Expr::LitFloat(f) => f.clone(),
        Expr::LitChar(c) => format!("'{c}'"),
        Expr::LitStr(s) => format!("\"{s}\""),
        // A default is a constant (Kap 5.1), and `f"…"` is a call to `format!`.
        // The parser admits it here, so this says no in words rather than
        // recording something a reader would take for text.
        Expr::LitInterpolated(_) => {
            unreachable!("a default is a literal, and `f\"…\"` is built at run time")
        }
        Expr::Unary {
            op: UnaryOp::Neg,
            expr,
        } => format!("-{}", literal_text(parsed, expr)),
        // The grammar admits nothing else, so this is unreachable rather than
        // a case with an answer.
        other => unreachable!("a default is a literal, found {other:?}"),
    }
}

/// Split a parameter list on the `;` that is not inside a bracket.
///
/// A `;` cannot appear anywhere else in a signature, but a default can be a
/// string, and a string can hold anything - so the depth is counted for the
/// same reason `ty::split_args` counts it.
fn split_config(text: &str) -> (&str, Option<&str>) {
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for (at, c) in text.char_indices() {
        if in_string {
            match c {
                _ if escaped => escaped = false,
                '\\' => escaped = true,
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }
        match c {
            '"' => in_string = true,
            '[' | '(' => depth += 1,
            ']' | ')' => depth = depth.saturating_sub(1),
            ';' if depth == 0 => return (&text[..at], Some(&text[at + 1..])),
            _ => {}
        }
    }
    (text, None)
}

fn quoted(rest: &str, close: &str, at: usize) -> Result<String> {
    rest.strip_suffix(close)
        .and_then(|r| r.strip_suffix('"'))
        .map(str::to_string)
        .ok_or_else(|| anyhow!("line {at}: unterminated section header"))
}

/// A value that may itself hold a quote - which a signature does, the moment an
/// option's default is a string: `method: &str = "GET"`.
fn escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn unquote(value: &str, at: usize) -> Result<String> {
    let inner = value
        .strip_prefix('"')
        .and_then(|v| v.strip_suffix('"'))
        .ok_or_else(|| anyhow!("line {at}: expected a quoted string, found `{value}`"))?;

    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => match chars.next() {
                Some(escaped) => out.push(escaped),
                None => return Err(anyhow!("line {at}: a `\\` at the end of `{value}`")),
            },
            c => out.push(c),
        }
    }
    Ok(out)
}

fn borrows_of(value: &str, at: usize) -> Result<Vec<String>> {
    let inner = value
        .strip_prefix("borrows(")
        .and_then(|v| v.strip_suffix(')'))
        .ok_or_else(|| anyhow!("line {at}: expected `borrows(…)`, found `{value}`"))?;
    Ok(inner
        .split('|')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect())
}

/// `sync = true` is a promise the source made, `sync = "inferred"` one the body
/// implies.
///
/// `false` is accepted and means the same as leaving the key out - a ledger is
/// generated and never writes it, but a file that says it should not be
/// rejected for saying something true.
fn sync_of(value: &str, at: usize) -> Result<Sync> {
    match value {
        "true" => Ok(Sync::Asserted),
        "false" => Ok(Sync::No),
        "\"inferred\"" => Ok(Sync::Inferred),
        other => {
            let named = other
                .strip_prefix("\"from(")
                .and_then(|rest| rest.strip_suffix(")\""))
                .map(str::trim)
                .filter(|name| !name.is_empty());
            match named {
                Some(name) => Ok(Sync::From(name.to_string())),
                None => Err(anyhow!(
                    "line {at}: `sync` is `true` (the source says so), `\"inferred\"` \
                     (the body implies it) or `\"from(f)\"` (its lambda decides), \
                     not `{other}`"
                )),
            }
        }
    }
}

fn provenance_of(value: &str, at: usize) -> Result<Provenance> {
    match value {
        "trusted" => Ok(Provenance::Trusted),
        "untrusted" => Ok(Provenance::Untrusted),
        other => Err(anyhow!(
            "line {at}: a provenance is `trusted` or `untrusted`, not `{other}`"
        )),
    }
}

/// The name an error gets when the compiler cannot name it - ADR-024 D1's `?`,
/// which is the absence of a claim rather than a type.
pub const UNNAMED_ERROR: &str = "?";

/// Kap 7.1: the errors that can leave a function.
///
/// A ledger written before [ADR-023](../../../../docs/specification/adr/adr-023.md)
/// D1 spells this `true`, and reading that as "it does not fail" would be the
/// worst of the three possible mistakes - so it is read as `["?"]`, which is
/// what `true` always meant: it fails, with something this file does not name.
fn throws_of(value: &str, at: usize) -> Result<Vec<String>> {
    if value == "true" {
        return Ok(vec![UNNAMED_ERROR.to_string()]);
    }
    string_list(value, at)
}

fn string_list(value: &str, at: usize) -> Result<Vec<String>> {
    let inner = value
        .strip_prefix('[')
        .and_then(|v| v.strip_suffix(']'))
        .ok_or_else(|| anyhow!("line {at}: expected a list, found `{value}`"))?;
    inner
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| unquote(s, at))
        .collect()
}
