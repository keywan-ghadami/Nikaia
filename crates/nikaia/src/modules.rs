// crates/nikaia/src/modules.rs
//
// A program is more than one file, and **a package is a directory**
// (Part I, 9.1; ADR-047 D1).
//
// The files of a package see one another with no `use` at all: they share one
// namespace, so a name declared in any of them can be written in any other. What
// the package offers outward is whatever says `pub`, in whichever file it is
// declared - which is what keeps a library's internal file layout from being its
// public surface.
//
// What this does is **resolution**, and resolution is the one thing ADR-011 D2
// said Stage 0 does not do: the lowering is name for name, and nothing looks a
// name up. That stays true of the *emitter* - what changes is that the compiler
// knows which files take part, and hands the emitter each of them in turn.
//
// The shape it produces is the plainest one the language below has: **one crate
// root**, with every file's items in it. No `mod`, no `use super::*`, nothing to
// resolve - because one namespace in this language is one namespace in that one.
// Two files declaring the same name is refused here rather than left to `rustc`,
// which would report it about a file nobody wrote (Part III, C.1).
//
// **Which files** is the directory and not the `use` lines. A `use` names another
// package (ADR-046 D1), and depending on one is not built (ADR-047 §5), so a
// `use` that is not `std`'s is refused with what to do instead.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::ast::Item;
use crate::parser::{self, Parsed};

/// One file of a package.
pub struct Unit {
    /// The package this file belongs to, as a **consumer** would write it -
    /// and `None` for every file of the program being built, whose names are
    /// the program's own.
    ///
    /// It is not the file stem any more. A file is not a unit of naming
    /// (ADR-047 D1), so nothing about a name says which file it came from; the
    /// field stays because a package that arrives by path will fill it in
    /// (ADR-047 D2), and that is the same qualification one level up.
    pub package: Option<String>,
    pub path: PathBuf,
    pub source: String,
    pub parsed: Parsed,
}

impl Unit {
    /// How a consumer writes a name from this file: `http::serve` for another
    /// package, and plain `serve` inside the program's own.
    pub fn qualify(&self, name: &str) -> String {
        match &self.package {
            Some(package) => format!("{package}::{name}"),
            None => name.to_string(),
        }
    }
}

/// A package this program depends on: the name a `use` writes, and where it is
/// ([ADR-047](../../../docs/specification/adr/adr-047.md) D2).
///
/// The name is the **manifest key** and nothing inside the package, which is D2
/// rule 1: a package does not name itself, so two libraries that both want to be
/// `http` are the consumer's to name apart.
#[derive(Debug, Clone)]
pub struct Dependency {
    pub name: String,
    /// The package's own directory - the one its `src/` is in.
    pub root: PathBuf,
}

/// Every file of the package the entry belongs to, entry first.
///
/// **The directory decides, not the `use` lines** (ADR-047 D1). Every `.nika`
/// beside the entry takes part, whether or not anything names it - which is what
/// makes moving a declaration from one file to another housekeeping rather than a
/// change to the package's surface.
///
/// Entry first because the entry is where `fn main` may be written, and the rest
/// sorted by file name: the order files are emitted in has to be a function of
/// the source tree alone (Part III 13.5's determinism), and a directory listing
/// is not sorted anywhere.
pub fn collect(entry: &Path) -> Result<Vec<Unit>> {
    collect_with(entry, &[])
}

/// The same, with the packages this program depends on
/// ([ADR-047](../../../docs/specification/adr/adr-047.md) D2).
///
/// A dependency's files come **after** the program's own, in the order the
/// manifest sorted its names, so the emitted file stays a function of the source
/// tree (Part III 13.5). Each is read from the package's `src/`, because a
/// dependency is a project and that is where a project keeps its files
/// (Part III, 13.1).
pub fn collect_with(entry: &Path, dependencies: &[Dependency]) -> Result<Vec<Unit>> {
    let names: BTreeSet<String> = dependencies.iter().map(|d| d.name.clone()).collect();
    let mut units = package_at(entry, None, &names)?;

    for dependency in dependencies {
        let src = dependency.root.join(SRC);
        if !src.is_dir() {
            return Err(crate::diagnostics::refuse(format!(
                "`{}` is depended on by path, and {} is not there.\n\
                 A package is a directory with a `{SRC}/` in it (Part III, 13.1).",
                dependency.name,
                src.display()
            )));
        }
        // **Its own `use` lines are checked against nothing.** D2 rule 2:
        // transitive dependencies are not visible, so a package that names one is
        // refused rather than silently handed ours - otherwise a library's surface
        // is everything it happens to use.
        units.extend(package_at(
            &src.join(ENTRY),
            Some(&dependency.name),
            &BTreeSet::new(),
        )?);
    }
    Ok(units)
}

/// One package: the directory an entry sits in, checked as a namespace of its
/// own.
fn package_at(
    entry: &Path,
    package: Option<&str>,
    reachable: &BTreeSet<String>,
) -> Result<Vec<Unit>> {
    let base = entry
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));

    let mut beside: Vec<PathBuf> = std::fs::read_dir(&base)
        .with_context(|| format!("cannot read {}", base.display()))?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|e| e == "nika"))
        .filter(|path| path != entry)
        .collect();
    beside.sort();

    // A library needs no `main.nika`: what a package offers is its public
    // surface, and an entry point is what a *program* has.
    let mut units = match entry.is_file() {
        true => vec![read_unit(entry, package, reachable)?],
        false => Vec::new(),
    };
    for path in beside {
        units.push(read_unit(&path, package, reachable)?);
    }
    one_namespace(&units)?;
    Ok(units)
}

/// Where a project keeps its sources and what its entry point is called
/// (Part III, 13.1).
const SRC: &str = "src";
const ENTRY: &str = "main.nika";

/// The entry and nothing beside it - a `.nika` file compiled on its own.
///
/// `--input` outside a project is not a package: a directory of loose examples is
/// a directory of programs, and compiling one of them must not pull in the other
/// ten. A package is a directory **of a project**, which is what a `nikaia.toml`
/// declares.
pub fn collect_one(entry: &Path) -> Result<Vec<Unit>> {
    Ok(vec![read_unit(entry, None, &BTreeSet::new())?])
}

fn read_unit(path: &Path, package: Option<&str>, reachable: &BTreeSet<String>) -> Result<Unit> {
    let source =
        std::fs::read_to_string(path).with_context(|| format!("cannot read {}", path.display()))?;
    // `with_context` and not `anyhow!("{e}")`: formatting the error into a string
    // loses its type, and the type is what says this is a refusal of the program
    // rather than a failure of this compiler (`diagnostics::Refused`).
    let parsed = parser::parse_to_ast(&source).with_context(|| format!("{}", path.display()))?;
    check_imports(&parsed, path, reachable)?;
    Ok(Unit {
        package: package.map(str::to_string),
        path: path.to_path_buf(),
        source,
        parsed,
    })
}

/// **Two files of one package may not declare the same name** (ADR-047 D1).
///
/// One namespace, so two `Row`s in it is an error rather than a rule about which
/// of them a line means. Refused here and not left to `rustc`: it would report a
/// duplicate definition against the generated file, which is exactly what
/// Part III C.1 forbids - and it would name a line the user never wrote.
fn one_namespace(units: &[Unit]) -> Result<()> {
    let mut seen: BTreeMap<String, PathBuf> = BTreeMap::new();
    for unit in units {
        for name in declared_names(&unit.parsed) {
            if let Some(first) = seen.get(&name) {
                return Err(crate::diagnostics::refuse(format!(
                    "`{name}` is declared twice in this package: in {} and in {}.\n\
                     The files of a package share one namespace (Part I, 9.1), so a name \
                     belongs to the package rather than to the file it is written in - \
                     rename one of the two, or keep the declaration in one place and let the \
                     other file use it.",
                    first.display(),
                    unit.path.display()
                )));
            }
            seen.insert(name, unit.path.clone());
        }
    }
    Ok(())
}

/// The names a file declares: what `one_namespace` counts.
///
/// A method is not among them - it belongs to its type and two types may each
/// have a `len`. An `impl` block declares nothing of its own.
fn declared_names(parsed: &Parsed) -> Vec<String> {
    parsed
        .program
        .items
        .iter()
        .filter_map(|item| match &item.node {
            Item::Fn { name, .. } => name.map(|name| parsed.text(name).to_string()),
            Item::Struct { name, .. } | Item::Enum { name, .. } => {
                Some(parsed.text(*name).to_string())
            }
            _ => None,
        })
        .collect()
}

/// **Every `use` in a file, against the packages it may name**
/// ([ADR-046](../../../docs/specification/adr/adr-046.md) D2, D4, D5).
///
/// `use` makes a package reachable and brings **no name** in, so there are four
/// things to say no to and one thing to accept. Each message names the way out,
/// because a rule a reader cannot act on is an obstacle (Part III, C.2):
///
/// * a **path** other than `std`'s — `use net::http` — which is either a nested
///   package (there is no such thing) or an attempt to import a name;
/// * a name that is a **file beside this one**, the shape that worked before
///   ADR-047 D1 made a package a directory;
/// * a name **no dependency declares**, which is D4 read from the other side: a
///   prefix must be introduced, and this is the introduction that names nothing;
/// * the **same name twice**, which is D5 — two packages under one name is an
///   error rather than a rule about which of them wins.
///
/// The braced and glob forms (`use pool::{Conn}`, `use pool::*`) are parse errors
/// at the brace and the star, so D2's sentence about them is not reachable from
/// here; that is [ADR-046](../../../docs/specification/adr/adr-046.md) §5's
/// remaining piece and it belongs in the grammar.
fn check_imports(parsed: &Parsed, at: &Path, reachable: &BTreeSet<String>) -> Result<()> {
    let here = at.display();
    let mut named: Vec<&str> = Vec::new();

    // A path first, and every one of them, because it is the only shape that is
    // wrong about *itself* rather than about what the project declares.
    for item in &parsed.program.items {
        let Item::Import { path } = &item.node else {
            continue;
        };
        let segments: Vec<&str> = path.iter().map(|s| parsed.text(*s)).collect();
        if matches!(segments.as_slice(), ["std", ..]) {
            continue;
        }
        if segments.len() > 1 {
            return Err(crate::diagnostics::refuse(format!(
                "`use {}` in {here}: `use` names one package and brings no name in \
                 (Part I, 9.1). Write `use {}`, and `{}` where you need it.",
                segments.join("::"),
                segments[0],
                segments.join("::")
            )));
        }
        named.push(segments[0]);
    }

    // **D5 before D4**, because a name written twice is a fact about this file and
    // says nothing about whether either of them resolves: checking reachability
    // first would answer a file with two unknown names by naming one of them.
    let mut once: BTreeSet<&str> = BTreeSet::new();
    for name in &named {
        if !once.insert(name) {
            return Err(crate::diagnostics::refuse(format!(
                "`use {name}` in {here}: twice. One name per file (Part I, 9.1) - two \
                 packages under one name is an error rather than a rule about which of them \
                 a line means."
            )));
        }
    }

    for name in named {
        if reachable.contains(name) {
            continue;
        }
        let beside = at
            .parent()
            .map(|dir| dir.join(format!("{name}.nika")))
            .is_some_and(|path| path.is_file());
        return Err(crate::diagnostics::refuse(match beside {
            true => format!(
                "`use {name}` in {here}: the files of a package already see one another \
                 (Part I, 9.1), so there is nothing to bring in - remove the line and write \
                 the name directly."
            ),
            false => format!(
                "`use {name}` in {here}: no dependency is called `{name}`.\n\
                 A package reached by name has to be declared, under that name, in this \
                 project's `[dependencies]` (Part III, 13.3):\n\
                 \x20   [dependencies]\n\
                 \x20   {name} = {{ path = \"../{name}\" }}"
            ),
        }));
    }
    Ok(())
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
        Self::of(collect(entry)?)
    }

    /// The same, with the packages this program depends on
    /// ([ADR-047](../../../docs/specification/adr/adr-047.md) D2).
    pub fn read_with(entry: &Path, dependencies: &[Dependency]) -> Result<Program> {
        Self::of(collect_with(entry, dependencies)?)
    }

    /// The entry compiled on its own - what `--input` outside a project is
    /// (see [`collect_one`]).
    pub fn read_one(entry: &Path) -> Result<Program> {
        Self::of(collect_one(entry)?)
    }

    fn of(units: Vec<Unit>) -> Result<Program> {
        let mut contracts = crate::contracts::Ledger::empty();

        // **A package at a time, not a file at a time.** `absorb` qualifies the
        // types *inside* an entry with the package's name, and it can only do
        // that for the types the ledger it is given declares - so a package whose
        // `Request` is in one file and whose `route(r: Request)` is in another has
        // to arrive as one ledger, or the signature keeps the bare name and a
        // caller writing `http::Request` is told the two are different types.
        // That was the file-level defect (`open-work.md` §1.1) one level up.
        //
        // The units arrive grouped (`collect_with`), so this is a walk and not a
        // sort - and the order inside a package is the order they were read in,
        // which Part III 13.5 needs to stay a function of the tree.
        let mut at = 0;
        while at < units.len() {
            let package = units[at].package.clone();
            let mut own = crate::contracts::Ledger::empty();
            while at < units.len() && units[at].package == package {
                own.absorb(None, crate::contracts::Ledger::infer(&units[at].parsed));
                at += 1;
            }
            contracts.absorb(package.as_deref(), own);
        }

        Ok(Program { units, contracts })
    }

    /// The **packages** this program reaches, by the name a `use` writes.
    ///
    /// What it is for is telling a `package::item` call from a `Type::method`
    /// one. Empty for a program of its own files, which is every program today:
    /// the files of a package share one namespace, so nothing in it is qualified
    /// (ADR-047 D1), and depending on another package is not built (D2).
    pub fn package_names(&self) -> std::collections::BTreeSet<String> {
        self.units
            .iter()
            .filter_map(|u| u.package.clone())
            .collect()
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

    /// The whole package as one Rust file.
    ///
    /// **One crate root per package** (ADR-047 D1). One namespace in this
    /// language is one namespace in the language below, so the program's own
    /// files need no `mod`, no `use super::*` and nothing for a reader to
    /// resolve. `pub` becomes `pub`, which is what publishes a name out of the
    /// package - the same move ADR-017 D2 made with `html::Render`: put the rule
    /// where the language below can act on it rather than re-implementing it
    /// here.
    ///
    /// **A dependency is a `mod` at that root** (D2), named by the manifest key,
    /// so `http::serve()` in Nikaia is `http::serve()` in Rust and nothing
    /// resolved it (ADR-011 D2). Each opens with `use super::*` under an
    /// `#[allow]`, which brings in the preamble the root wrote - the allow is
    /// because the import is **ours**, and a module that happens to use nothing
    /// from the preamble would otherwise produce a warning about a line no Nikaia
    /// source maps to (Part III, C.1).
    ///
    /// The program's own files go **first**, the entry first among them, because
    /// the entry is the file that may declare `main` and a reader opens the
    /// generated file at the top.
    pub fn emit(&self, build: crate::emit::Build) -> Result<crate::emit::Lowered> {
        self.emit_ordered(build, crate::emit::Ordering::default())
    }

    /// The same, saying how strictly the written order is taken (ADR-033).
    pub fn emit_ordered(
        &self,
        build: crate::emit::Build,
        ordering: crate::emit::Ordering,
    ) -> Result<crate::emit::Lowered> {
        use crate::emit::{Lowered, Needs, SourceMap};

        let trust = crate::contracts::trust::analyse(&self.units[0].parsed, &std_ledger());
        let needs = self.units.iter().fold(Needs::default(), |acc, u| {
            acc.join(Needs::of(&u.parsed, build))
        });

        let mut rust = String::new();
        rust.push_str("// Generated by the Nikaia bootstrap compiler (Stage 0).\n");
        rust.push_str("// Edit the .nika source, not this file.\n\n");
        rust.push_str(&needs.preamble());
        rust.push('\n');

        let mut map = SourceMap::default();
        let mut open: Option<&str> = None;
        for (at, unit) in self.units.iter().enumerate() {
            let body = crate::emit::emit_module_body_ordered(
                ordering,
                &unit.parsed,
                build,
                trust.provenance,
                &self.contracts,
                // The entry is the only file ADR-038 D4's generated `fn main`
                // may be written from.
                at == 0,
            )?;

            // The units arrive grouped by package (`collect_with`), so a `mod`
            // opens where the name changes and closes where it changes again -
            // and a package of several files is one `mod` rather than one each.
            if open != unit.package.as_deref() {
                if open.is_some() {
                    rust.push_str("}\n\n");
                }
                if let Some(package) = unit.package.as_deref() {
                    rust.push_str(&format!("pub mod {package} {{\n"));
                    rust.push_str("#[allow(unused_imports)]\nuse super::*;\n");
                }
                open = unit.package.as_deref();
            }

            map.extend(body.map.placed(rust.len(), at));
            rust.push_str(&body.rust);
            rust.push('\n');
        }
        if open.is_some() {
            rust.push_str("}\n");
        }

        Ok(Lowered { rust, map })
    }
}

fn std_ledger() -> crate::contracts::Ledger {
    crate::contracts::Ledger::parse(crate::contracts::STD).unwrap_or_default()
}
