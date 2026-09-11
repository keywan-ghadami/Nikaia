// crates/nikaia/src/modules.rs
//
// A program is more than one file (Part I, 9.1: every file is a module).
//
// What this does is **resolution**, and resolution is the one thing ADR-011 D2
// said Stage 0 does not do: the lowering is name for name, and nothing looks a
// module up. That stays true of the *emitter* - what changes is that the
// compiler now knows which files take part, and hands the emitter each of them
// in turn.
//
// The shape it produces is the one the language below already has: every module
// becomes a `mod` at the crate root, `pub` becomes `pub`, and `utils::helper()`
// is `utils::helper()`. Privacy is then enforced by `rustc` for the same reason
// escaping is (ADR-017 D2) - the rule is put where the language below can act
// on it, rather than re-implemented here.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};

use crate::ast::Item;
use crate::parser::{self, Parsed};

/// One file of a program.
pub struct Unit {
    /// The module's name, which is its file stem - and `None` for the entry,
    /// whose items live at the crate root because `main` has to.
    pub module: Option<String>,
    pub path: PathBuf,
    pub source: String,
    pub parsed: Parsed,
}

impl Unit {
    /// How a caller in another module writes a name from this one: `utils::f`,
    /// or plain `f` at the root.
    pub fn qualify(&self, name: &str) -> String {
        match &self.module {
            Some(module) => format!("{module}::{name}"),
            None => name.to_string(),
        }
    }
}

/// Every file a program is made of, entry first.
///
/// Depth-first from the entry, and each module is parsed once however many
/// files import it. Two modules that import each other are fine: they become
/// two `mod` blocks in one crate, which the language below allows, so nothing
/// here has to break the cycle - only stop walking it.
pub fn collect(entry: &Path) -> Result<Vec<Unit>> {
    let entry = entry.to_path_buf();
    let base = entry
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));

    let source = std::fs::read_to_string(&entry)
        .with_context(|| format!("cannot read {}", entry.display()))?;
    let parsed = parser::parse_to_ast(&source).map_err(|e| anyhow!("{}: {e}", entry.display()))?;

    let mut pending: Vec<String> = imports_of(&parsed, &entry)?;
    let mut units = vec![Unit {
        module: None,
        path: entry,
        source,
        parsed,
    }];

    // A `BTreeMap` rather than a set plus a vector: the order modules are
    // emitted in has to be a function of the source alone (13.5's determinism
    // guarantee), and "sorted by name" is that where "the order imports were
    // discovered in" is only nearly that.
    let mut found: BTreeMap<String, Unit> = BTreeMap::new();

    while let Some(module) = pending.pop() {
        if found.contains_key(&module) {
            continue;
        }
        let path = base.join(format!("{module}.nika"));
        if !path.is_file() {
            return Err(anyhow!(
                "`use {module}` names no file: {} is not there.\n\
                 A module is a file (Part I, 9.1), looked for beside the program's \
                 entry - so `use {module}` wants `{module}.nika` in {}.",
                path.display(),
                base.display()
            ));
        }
        let source = std::fs::read_to_string(&path)
            .with_context(|| format!("cannot read {}", path.display()))?;
        let parsed =
            parser::parse_to_ast(&source).map_err(|e| anyhow!("{}: {e}", path.display()))?;
        pending.extend(imports_of(&parsed, &path)?);
        found.insert(
            module.clone(),
            Unit {
                module: Some(module),
                path,
                source,
                parsed,
            },
        );
    }

    units.extend(found.into_values());
    Ok(units)
}

/// The modules a file imports - which is every `use` that is not `std`'s.
///
/// `use std::fs` names the library and never a file. Anything else names a
/// module of this program, and Stage 0 takes **one segment**: a `mod` at the
/// crate root is what the emitter writes, and a nested path would be a nested
/// `mod` with a resolution rule of its own. Refused rather than guessed at.
fn imports_of(parsed: &Parsed, at: &Path) -> Result<Vec<String>> {
    let mut modules = Vec::new();
    for item in &parsed.program.items {
        let Item::Import { path } = &item.node else {
            continue;
        };
        let segments: Vec<&str> = path.iter().map(|s| parsed.text(*s)).collect();
        match segments.as_slice() {
            ["std", ..] => {}
            [one] => modules.push((*one).to_string()),
            many => {
                return Err(anyhow!(
                    "`use {}` in {}: a module of this program is one name (Part I, 9.1), \
                     because a module is a file beside the entry. Only `std::…` has a path, \
                     and it names the library rather than a file.",
                    many.join("::"),
                    at.display()
                ))
            }
        }
    }
    Ok(modules)
}

/// A whole program: one ledger, one Rust file, one source map.
pub struct Program {
    pub units: Vec<Unit>,
    /// One ledger for the project (Part III, 13.5), with every module's entries
    /// under the name a caller writes.
    pub contracts: crate::contracts::Ledger,
}

impl Program {
    /// Parse every file, and infer the contracts of all of them together.
    ///
    /// Together, and not one at a time: `sync` is a fixpoint over the call
    /// graph (ADR-027), and a call graph that stops at a file boundary would
    /// give a different answer depending on which file was looked at first.
    /// 13.5 makes the ledger a pure function of the source tree, which is a
    /// promise about the tree and not about each file in it.
    pub fn read(entry: &Path) -> Result<Program> {
        let units = collect(entry)?;
        let mut contracts = crate::contracts::Ledger::empty();
        for unit in &units {
            contracts.absorb(
                unit.module.as_deref(),
                crate::contracts::Ledger::infer(&unit.parsed),
            );
        }
        Ok(Program { units, contracts })
    }

    /// What this program is made of, by name - which is what says a
    /// `module::item` call crosses a file boundary and a `Type::method` one
    /// does not.
    pub fn module_names(&self) -> std::collections::BTreeSet<String> {
        self.units.iter().filter_map(|u| u.module.clone()).collect()
    }

    /// Whether this is one file, in which case nothing about the build changes.
    pub fn is_single_file(&self) -> bool {
        self.units.len() == 1
    }

    /// Every source that took part, in the order they are emitted - which is
    /// what the cache key has to cover (Part III, 13.1: "the SHA256 of each
    /// `.nika` source that took part").
    pub fn sources(&self) -> Vec<&str> {
        self.units.iter().map(|u| u.source.as_str()).collect()
    }

    /// The whole program as one Rust file.
    ///
    /// Every module is a `mod` at the crate root and the entry's items are the
    /// root itself, because `main` has to be there. Each `mod` opens with
    /// `use super::*;`, which is one line doing two jobs: it brings in the
    /// preamble the crate root wrote, and it brings in the **sibling modules**,
    /// because a `mod b` at the root is a name at the root. So `utils::helper()`
    /// in Nikaia is `utils::helper()` in Rust and nothing resolved it
    /// (ADR-011 D2).
    ///
    /// `pub` becomes `pub`, so Part I 9.2's privacy is enforced by the language
    /// below rather than re-implemented here - the same move ADR-017 D2 made
    /// with `html::Render`.
    pub fn emit(&self, profile: crate::emit::Profile) -> Result<crate::emit::Lowered> {
        use crate::emit::{Lowered, Needs, SourceMap};

        let trust = crate::contracts::trust::analyse(&self.units[0].parsed, &std_ledger());
        let needs = self.units.iter().fold(Needs::default(), |acc, u| {
            acc.join(Needs::of(&u.parsed, profile))
        });

        let mut rust = String::new();
        rust.push_str("// Generated by the Nikaia bootstrap compiler (Stage 0).\n");
        rust.push_str("// Edit the .nika source, not this file.\n\n");
        rust.push_str(&needs.preamble());
        rust.push('\n');

        let mut map = SourceMap::default();

        // The modules first, then the root: an item at the root may name a
        // module, and a reader should meet the parts before the whole.
        for (at, unit) in self.units.iter().enumerate() {
            let body = crate::emit::emit_module_body(
                &unit.parsed,
                profile,
                trust.provenance,
                &self.contracts,
            )?;
            match &unit.module {
                Some(module) => {
                    rust.push_str(&format!("pub mod {module} {{\n"));
                    rust.push_str("use super::*;\n");
                    map.extend(body.map.placed(rust.len(), at));
                    rust.push_str(&body.rust);
                    rust.push_str("}\n\n");
                }
                None => {
                    // Held back to the end, below.
                    let _ = body;
                }
            }
        }

        let entry = crate::emit::emit_module_body(
            &self.units[0].parsed,
            profile,
            trust.provenance,
            &self.contracts,
        )?;
        map.extend(entry.map.placed(rust.len(), 0));
        rust.push_str(&entry.rust);

        Ok(Lowered { rust, map })
    }
}

fn std_ledger() -> crate::contracts::Ledger {
    crate::contracts::Ledger::parse(crate::contracts::STD).unwrap_or_default()
}
