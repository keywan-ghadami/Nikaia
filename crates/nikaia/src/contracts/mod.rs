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

use std::collections::BTreeMap;

use anyhow::{anyhow, Result};

use crate::ast::Item;
use crate::emit::{borrowing_structs, holds_view, names_borrowing};
use crate::parser::Parsed;

/// The inference this ledger was produced by.
///
/// Recorded in the header so that a ledger can say what it knows. Stage 0 reads
/// signatures; the whole-program analysis of ADR-005 D3 will read bodies, and
/// its ledgers must not be mistaken for these.
pub const INFERENCE: &str = "stage0-signatures";

/// The format version of the file itself.
pub const VERSION: u32 = 1;

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
    /// The parameters the result may point into, in declaration order.
    ///
    /// Empty when the result holds no view. Stage 0 has one input lifetime, so
    /// a result that is a view may point into *any* view it was given -
    /// `borrows(a | b)`, which is the spec's own spelling and is the widest
    /// contract the signature can support.
    pub borrows: Vec<String>,
}

/// What a caller needs to know about one type.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TypeContract {
    pub public: bool,
    /// ADR-008 D6: `@borrowed` was asserted in the source.
    pub borrowed: bool,
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
                    let (name, contract) = ledger.function(parsed, &item.node, None);
                    ledger.functions.insert(name, contract);
                }
                Item::Impl { target, methods } => {
                    let target = parsed.text(target.name).to_string();
                    for method in methods {
                        let (name, contract) = ledger.function(parsed, &method.node, Some(&target));
                        ledger.functions.insert(name, contract);
                    }
                }
                Item::Struct {
                    name,
                    fields,
                    is_public,
                    is_borrowed,
                    ..
                } => {
                    let tethered = fields
                        .iter()
                        .filter(|f| holds_view(&f.ty) || names_borrowing(&f.ty, &borrowing))
                        .map(|f| parsed.text(f.name).to_string())
                        .collect();
                    ledger.types.insert(
                        parsed.text(*name).to_string(),
                        TypeContract {
                            public: *is_public,
                            borrowed: *is_borrowed,
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
    fn function(&self, parsed: &Parsed, item: &Item, target: Option<&str>) -> (String, FnContract) {
        let Item::Fn {
            name,
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

        (
            key,
            FnContract {
                public: *is_public,
                sync: *is_sync,
                throws: *throws,
                borrows,
            },
        )
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
        }

        for (name, contract) in &self.types {
            out.push_str(&format!("\n[type.\"{name}\"]\n"));
            if contract.public {
                out.push_str("pub = true\n");
            }
            if contract.borrowed {
                out.push_str("borrowed = true\n");
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
                        _ => return Err(anyhow!("line {}: unknown key `{key}` on a fn", at())),
                    }
                }
                (Some((false, name)), _) => {
                    let entry = ledger.types.entry(name.clone()).or_default();
                    match key {
                        "pub" => entry.public = value == "true",
                        "borrowed" => entry.borrowed = value == "true",
                        "tethered" => entry.tethered = string_list(value, at())?,
                        _ => return Err(anyhow!("line {}: unknown key `{key}` on a type", at())),
                    }
                }
            }
        }

        Ok(ledger)
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
