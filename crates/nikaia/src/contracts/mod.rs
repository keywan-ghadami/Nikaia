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
// This is that file, for what Stage 0 knows. `sync` and `throws` are *declared*
// in the source and are recorded exactly; the borrow contract is *inferred*,
// and Stage 0's inference is the signature rather than the whole-program
// analysis ADR-005 D3 describes - which is why the header names the inference
// that produced the ledger. A later compiler that infers more will write a
// different name there, and `--locked` will say so rather than quietly
// accepting the weaker answer.

pub mod sync;
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
/// Recorded in the header so that a ledger can say what it knows. Stage 0 reads
/// signatures; the whole-program analysis of ADR-005 D3 will read bodies, and
/// its ledgers must not be mistaken for these.
pub const INFERENCE: &str = "stage0-signatures";

/// The format version of the file itself.
pub const VERSION: u32 = 1;

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
    pub sync: bool,
    /// Kap 7.1: it may fail, so its result is a `Result`.
    pub throws: bool,
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
    /// What it hands back. `None` where it hands back nothing.
    pub result: Option<ty::Ty>,
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
        let params: Vec<String> = self
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
        match &self.result {
            Some(result) => format!("({}) -> {}", params.join(", "), result.text()),
            None => format!("({})", params.join(", ")),
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

        let params = ty::split_args(inside)
            .iter()
            .map(|part| match part.split_once(':') {
                Some((name, ty)) => (name.trim().to_string(), ty::Ty::parse(ty)),
                // A bare type is the receiver, which is what `&mut self` is.
                None => ("self".to_string(), ty::Ty::parse(part)),
            })
            .collect();

        let result = text[close + 1..]
            .trim()
            .strip_prefix("->")
            .map(ty::Ty::parse);

        Ok(Signature { params, result })
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
    pub fn infer(parsed: &Parsed) -> Self {
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
                Item::Impl { target, methods } => {
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
                            tethered,
                        },
                    );
                }
                _ => {}
            }
        }

        ledger
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
                sync: *is_sync,
                throws: *throws,
                signature: Some(Signature {
                    params,
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
        out.push_str("# whether it may pause (`sync`), whether it may fail (`throws`), and what\n");
        out.push_str("# its result may point into (`returns`). Part III, 13.5.\n");
        out.push_str(&format!("version = {}\n", self.version));
        out.push_str(&format!("toolchain = \"{}\"\n", self.toolchain));
        out.push_str(&format!("inference = \"{}\"\n", self.inference));

        for (name, contract) in &self.functions {
            out.push_str(&format!("\n[fn.\"{name}\"]\n"));
            if contract.public {
                out.push_str("pub = true\n");
            }
            if contract.sync {
                out.push_str("sync = true\n");
            }
            if contract.throws {
                out.push_str("throws = true\n");
            }
            if !contract.borrows.is_empty() {
                out.push_str(&format!(
                    "returns = \"borrows({})\"\n",
                    contract.borrows.join(" | ")
                ));
            }
            if let Some(provenance) = contract.provenance {
                out.push_str(&format!("provenance = \"{}\"\n", provenance.as_str()));
            }
            if let Some(signature) = &contract.signature {
                out.push_str(&format!("signature = \"{}\"\n", signature.text()));
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
                        "sync" => entry.sync = value == "true",
                        "throws" => entry.throws = value == "true",
                        "returns" => entry.borrows = borrows_of(&unquote(value, at())?, at())?,
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
                        "tethered" => entry.tethered = string_list(value, at())?,
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

fn quoted(rest: &str, close: &str, at: usize) -> Result<String> {
    rest.strip_suffix(close)
        .and_then(|r| r.strip_suffix('"'))
        .map(str::to_string)
        .ok_or_else(|| anyhow!("line {at}: unterminated section header"))
}

fn unquote(value: &str, at: usize) -> Result<String> {
    value
        .strip_prefix('"')
        .and_then(|v| v.strip_suffix('"'))
        .map(str::to_string)
        .ok_or_else(|| anyhow!("line {at}: expected a quoted string, found `{value}`"))
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

fn provenance_of(value: &str, at: usize) -> Result<Provenance> {
    match value {
        "trusted" => Ok(Provenance::Trusted),
        "untrusted" => Ok(Provenance::Untrusted),
        other => Err(anyhow!(
            "line {at}: a provenance is `trusted` or `untrusted`, not `{other}`"
        )),
    }
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
