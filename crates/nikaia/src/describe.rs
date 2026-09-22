// crates/nikaia/src/describe.rs
//
// `nikaia describe <crate>` — a draft ledger for a Rust crate's boundary
// ([ADR-104](../../docs/specification/adr/adr-104.md) D2, D3, D4).
//
// ## What it is for
//
// D1 refuses a call into a crate nothing describes and names this command. What
// the command writes is `contracts/<crate>.contracts`, the file every analysis
// then reads at that boundary — the crossing verdict, `keeps`, `sync` and
// `throws` stop failing closed on a call whose signature answers plainly.
//
// ## What it reads, and what it cannot
//
// **The crate's sources**, because D4's better half is not available: rustdoc's
// JSON is unstable, and [ADR-001](../../docs/specification/adr/adr-001.md) D1
// does not give up a stable-only toolchain for it.
//
// **They are read by a grammar written in Nikaia**
// ([ADR-195](../../docs/specification/adr/adr-195.md) D3):
// `crates/nikaia-std/src/tools/rust.nika`, lowered ahead of time and reached
// from here as an ordinary Rust module — `nikaia_std::tools::rust`, a call and
// nothing else ([ADR-196](../../docs/specification/adr/adr-196.md) D1). What
// stood here before was a hand-written character scanner that matched `pub fn`
// at the start of a line and counted braces without knowing what a brace is;
// on one crate it wrote entries for **four functions that do not exist**
// (`crates/nikaia/tests/describing.rs`).
//
// What is left that it cannot do:
//
//   * an item a **macro** generates is not in the text and is not found. That
//     limit is [ADR-001](../../docs/specification/adr/adr-001.md) D1's and not
//     the parser's: it survives `syn` too, for the reason rustdoc's JSON is out
//     of reach;
//   * a signature this cannot translate is written `?`, which is the absence of
//     a claim and never a guess (D4's own sentence);
//   * a **method** is not described: what an `impl`'s `pub fn` is at a foreign
//     boundary is D4's own question and nothing here asks it. The parser reads
//     them; this does not write them down.
//   * a **macro** is still the only one. The `mod`-path limit is **closed**: a
//     module is a block or a file, `src/foo/bar.rs` is `foo::bar`, and whether
//     a caller may write it is read from the `pub mod foo;` in its parent. A
//     module nothing declares is **not** offered, which is fail-closed and
//     [ADR-010](../../docs/specification/adr/adr-010.md) D1's polarity — the
//     absence is *nobody said this is public*.
//
// Each of those is D5's case: the draft is **committed and reviewed like code**,
// and a `?` in it is a person's to fill. A describer that guessed would put a
// claim in a file nobody wrote, which is the one thing a boundary description
// may not do.
//
// **And the direction that matters more than dropping what is not there**: a
// `pub use` is read, and what it carries out of a private `mod` is offered
// under the name it gives. Refusing a call a crate really answers is
// [Part III C.4](../../docs/specification/30-nikaia-tooling.md), and the
// scanner refused every one of them.
//
// ## Which way it errs
//
// Fail-closed, everywhere the signature is silent: no `touches` (which reads as
// *touches everything*), no `locks` (that column's third answer), and no
// `crosses` (the absence is *nobody said*, never *it may not*). A Rust
// signature cannot tell anybody any of the three, and reading silence as
// "nothing" is the polarity [ADR-010](../../docs/specification/adr/adr-010.md)
// D1 forbids.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

use crate::contracts::{ty::Ty, FnContract, Ledger, Signature, Sync, TypeContract};

/// What a run of the command did, for the line it prints.
#[derive(Debug)]
pub struct Described {
    /// Where the draft was written — empty from [`draft`], which writes
    /// nothing.
    pub path: PathBuf,
    /// The crate's version, as the manifest declares it or `"?"`.
    pub version: String,
    /// How many functions and types the draft carries.
    pub functions: usize,
    pub types: usize,
    /// The names the program writes that no `pub` signature answered — D5's
    /// `?`, named so a reviewer knows what to fill rather than what to find.
    pub unanswered: Vec<String>,
}

/// Write `contracts/<crate>.contracts` for the crate the program calls.
pub fn describe(root: &Path, crate_word: &str) -> Result<Described> {
    let (ledger, mut written) = draft(root, crate_word)?;
    let directory = root.join(crate::project::CONTRACTS);
    std::fs::create_dir_all(&directory).with_context(|| format!("{}", directory.display()))?;
    let path = directory.join(format!("{crate_word}.contracts"));
    std::fs::write(
        &path,
        ledger.render_description(crate_word, &written.version),
    )
    .with_context(|| format!("{}", path.display()))?;
    written.path = path;
    Ok(written)
}

/// The same draft, **without writing it**.
///
/// Split out because the interesting assertion is the file's *contents* and
/// every test that made one would otherwise have to write into a tree and take
/// it back out again — including the one that matters most, which reads the
/// repository's own `examples/foreign-runtime/shim` and would have to put the
/// reviewed file back afterwards.
pub fn draft(root: &Path, crate_word: &str) -> Result<(Ledger, Described)> {
    let manifest = crate::manifest::Manifest::read(&root.join("nikaia.toml"))
        .with_context(|| format!("{}/nikaia.toml", root.display()))?;
    let (declared, value) = rust_dependency(&manifest, crate_word)?;
    let sources = crate_sources(root, &value, crate_word, &declared)?;

    let wanted = names_the_program_writes(root, crate_word)?;
    let mut surface = Surface::default();
    let mut hashes = BTreeMap::new();
    for (relative, text) in &sources.files {
        hashes.insert(
            relative.clone(),
            orchestrator::cache::sha256_hex(text.as_bytes()),
        );
        surface.read(relative, text)?;
    }
    surface.resolve();
    let types = surface.types.clone();
    let fields = surface.fields.clone();

    let mut ledger = Ledger::empty();
    ledger.inference = "described-from-signatures".to_string();
    ledger.sources = hashes;
    let mut unanswered = Vec::new();
    let mut named_types: BTreeSet<String> = BTreeSet::new();
    for name in &wanted {
        // **The path a caller writes, resolved through the `pub use` items**
        // ([ADR-196](../../docs/specification/adr/adr-196.md) D2's own reason
        // for reading them): a `pub fn` inside a private `mod` is reachable
        // after all when one says so, and refusing such a call would be
        // [Part III C.4](../../docs/specification/30-nikaia-tooling.md).
        let Some(function) = surface
            .reachable
            .get(name)
            .and_then(|at| surface.functions.get(at))
        else {
            unanswered.push(format!("{crate_word}::{name}"));
            continue;
        };
        let (contract, mentions) = function.contract(crate_word, &types);
        named_types.extend(mentions);
        ledger
            .functions
            .insert(format!("{crate_word}::{name}"), contract);
    }
    // **A written type is a reach across the boundary too** (D1), and a type a
    // described signature *names* is one the caller's compiler will look up. So
    // both sets get an entry: what the program wrote, and what the entries
    // mention.
    for name in wanted.iter().filter(|name| types.contains(*name)) {
        named_types.insert(name.clone());
    }
    for name in named_types {
        ledger.types.insert(
            format!("{crate_word}::{name}"),
            TypeContract {
                public: true,
                // **The one claim that comes from a field**
                // ([ADR-123](../../docs/specification/adr/adr-123.md) D2), and
                // the one thing here a Rust *signature* could never say.
                crosses: crosses(fields.get(&name).map(Vec::as_slice)),
                ..TypeContract::default()
            },
        );
    }

    let described = Described {
        path: PathBuf::new(),
        version: sources.version,
        functions: ledger.functions.len(),
        types: ledger.types.len(),
        unanswered,
    };
    Ok((ledger, described))
}

/// The manifest's declaration for this crate, under the name a program writes.
///
/// The key is the manifest's and the word is Cargo's, which is the translation
/// D1 already makes: `hyper-shim` in the manifest is `hyper_shim` in a program.
fn rust_dependency(
    manifest: &crate::manifest::Manifest,
    crate_word: &str,
) -> Result<(String, toml::Value)> {
    for (key, declared) in manifest.dependencies() {
        if key.replace('-', "_") != crate_word {
            continue;
        }
        let crate::manifest::Dependency::Rust(value) = declared else {
            bail!(
                "`{key}` is declared in this project, and not as a Rust crate - `nikaia \
                 describe` is for `[dependencies]` entries with `type = \"rust\"` (ADR-104 D1)"
            );
        };
        return Ok((key.clone(), value.clone()));
    }
    bail!(
        "nothing in this project's `nikaia.toml` declares `{crate_word}` - a crate is \
         described because the build links against it, and `[dependencies]` is where it says \
         so (ADR-104 D1)"
    )
}

/// The crate's `.rs` files, by their path inside the crate.
struct Sources {
    version: String,
    files: BTreeMap<String, String>,
}

/// Where the crate's sources are, and every `.rs` under its `src/`.
///
/// **A `path` dependency only**, and that is this step's honest scope rather
/// than a gap: a version dependency's sources are in Cargo's registry cache,
/// under a layout this compiler does not resolve — [ADR-002](../../docs/specification/adr/adr-002.md)
/// D1 hands versions to Cargo and never resolves one itself, and a describer
/// that guessed at that cache's shape would be resolving one.
///
/// The path is **relative to the generated manifest**, which is where the
/// manifest's own comment says it is: a build writes `Cargo.toml` to
/// `target/nikaia/build/`, and `type = "rust"` is a passthrough, so the value
/// reaches Cargo verbatim and means what it means there.
fn crate_sources(root: &Path, value: &toml::Value, crate_word: &str, key: &str) -> Result<Sources> {
    let version = value
        .get("version")
        .and_then(toml::Value::as_str)
        .unwrap_or("?")
        .to_string();
    let Some(declared) = value.get("path").and_then(toml::Value::as_str) else {
        bail!(
            "`{key}` is declared by version and its sources are in Cargo's registry cache, \
             which this compiler does not resolve (ADR-002 D1). Describe a `path` dependency, \
             or write `contracts/{crate_word}.contracts` by hand the way ADR-104 D5 expects a \
             reviewer to read it"
        );
    };
    // **Resolved lexically and not by the filesystem**: `target/nikaia/build`
    // is written by a build, and a crate may perfectly well be described before
    // the project has ever been built - which is the order `NK2504` puts a
    // reader in. `canonicalize` on a directory that is not there yet fails, and
    // what it would have answered is a `..` this can walk off itself.
    let crate_root = without_dots(&root.join("target/nikaia/build").join(declared));
    if !crate_root.is_dir() {
        bail!(
            "the sources of `{key}` are not at {} - `path` in the manifest is relative to \
             the **generated** manifest, which a build writes to `target/nikaia/build/` \
             (ADR-002 D1's passthrough)",
            crate_root.display()
        );
    }
    let version = match version.as_str() {
        "?" => cargo_version(&crate_root).unwrap_or_else(|| "?".to_string()),
        given => given.to_string(),
    };

    let mut files = BTreeMap::new();
    let src = crate_root.join("src");
    collect_rust(&src, &src, &mut files)?;
    if files.is_empty() {
        bail!(
            "no `.rs` file under {} - `nikaia describe` reads the crate's own sources (ADR-104 \
             D4)",
            src.display()
        );
    }
    Ok(Sources { version, files })
}

/// **Where a described crate's sources are**, for a reader other than the
/// describer: the hash rule compares what a description recorded against what
/// is there now, and *where is there* is this one question.
///
/// `None` for anything this cannot answer — a crate the manifest does not
/// declare, one declared by version, one whose directory is not there. Each of
/// those is an absence rather than a difference, and a refusal may not rest on
/// one ([ADR-169](../../docs/specification/adr/adr-169.md) D1).
pub fn crate_root(root: &Path, crate_word: &str) -> Option<PathBuf> {
    let manifest = crate::manifest::Manifest::read(&root.join("nikaia.toml")).ok()?;
    let (_, value) = rust_dependency(&manifest, crate_word).ok()?;
    let declared = value.get("path").and_then(toml::Value::as_str)?;
    let at = without_dots(&root.join("target/nikaia/build").join(declared));
    at.is_dir().then_some(at)
}

/// A path with its `.` and `..` components walked off, without asking the
/// filesystem whether any of it exists.
fn without_dots(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                out.pop();
            }
            other => out.push(other),
        }
    }
    out
}

/// The crate's own version, from its `Cargo.toml`.
fn cargo_version(crate_root: &Path) -> Option<String> {
    let text = std::fs::read_to_string(crate_root.join("Cargo.toml")).ok()?;
    let document: toml::Value = toml::from_str(&text).ok()?;
    Some(
        document
            .get("package")?
            .get("version")?
            .as_str()?
            .to_string(),
    )
}

/// Every `.rs` under a directory, keyed by its path inside it.
fn collect_rust(base: &Path, at: &Path, out: &mut BTreeMap<String, String>) -> Result<()> {
    let Ok(entries) = std::fs::read_dir(at) else {
        return Ok(());
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rust(base, &path, out)?;
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let text = std::fs::read_to_string(&path).with_context(|| format!("{}", path.display()))?;
        let relative = path
            .strip_prefix(base)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        out.insert(format!("src/{relative}"), text);
    }
    Ok(())
}

/// The names of this crate every `.nika` of the project writes, without the
/// crate word in front.
///
/// [`crate::foreign::qualified_names`] is the walk, which is the one `NK2504`
/// runs: what the refusal asks is which crates a program reaches into, and this
/// asks which names of one — the same walk, read one segment further.
fn names_the_program_writes(root: &Path, crate_word: &str) -> Result<BTreeSet<String>> {
    let mut out = BTreeSet::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                // `target/` is the build's own and holds nothing a program
                // wrote; `contracts/` holds this command's own output.
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if name != "target" && name != crate::project::CONTRACTS {
                    pending.push(path);
                }
                continue;
            }
            if path.extension().and_then(|e| e.to_str()) != Some("nika") {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            let Ok(parsed) = crate::parser::parse_to_ast(&text) else {
                // A file that does not parse is the build's to report, not
                // this command's: it would be the same message twice.
                continue;
            };
            for name in crate::foreign::qualified_names(&parsed).keys() {
                let Some(rest) = name.strip_prefix(&format!("{crate_word}::")) else {
                    continue;
                };
                out.insert(rest.to_string());
            }
        }
    }
    Ok(out)
}

/// What a crate's sources say, before anything is asked of them.
///
/// **Read by a grammar and not by a scanner**
/// ([ADR-195](../../docs/specification/adr/adr-195.md) D3,
/// [ADR-196](../../docs/specification/adr/adr-196.md) D1): the parser is
/// `crates/nikaia-std/src/tools/rust.nika`, written in Nikaia, lowered ahead of
/// time and reached from here as `nikaia_std::tools::rust` — an ordinary Rust
/// module and an ordinary call.
///
/// What that buys, measured on one file: the scanner this replaced reported
/// **four functions that do not exist** — one inside a block comment, one on
/// the second line of a string literal, two inside a private `mod` — and put a
/// fifth at the crate root rather than under its module.
#[derive(Default)]
struct Surface {
    /// Every `pub fn`, by the path it is **defined** at. A private `mod`'s are
    /// here too, because a `pub use` may reach one.
    functions: BTreeMap<String, Function>,
    /// Every `pub struct`'s field types, by the path the type is defined at.
    /// A `pub enum` is a type without an entry here, which is *nobody looked*
    /// and not *it holds nothing* — the difference [`crosses`] rests on.
    fields: BTreeMap<String, Vec<String>>,
    /// Every `pub struct` and `pub enum`, by the path it is defined at.
    types: BTreeSet<String>,
    /// Every `mod` declaration found, by the module's path, and whether it was
    /// written `pub`.
    ///
    /// **A module nothing declares is not offered**, which is fail-closed and
    /// [ADR-010](../../docs/specification/adr/adr-010.md) D1's polarity: what
    /// is missing here is *nobody said this module is public*, and reading that
    /// as *it is* would put a claim in the draft that no source supports. The
    /// name reaches the reviewer as a `?` instead
    /// ([ADR-104](../../docs/specification/adr/adr-104.md) D4, D5).
    modules: BTreeMap<String, bool>,
    /// A path a **caller** may write, and the path it resolves to. An item in a
    /// public module is here under its own path; a re-exported one is here
    /// under the name the re-export gives it. Filled by [`Surface::resolve`],
    /// because whether a path is offered takes every file to answer.
    reachable: BTreeMap<String, String>,
    /// `pub use` items, kept until every file has been read: one may name
    /// something another file declares.
    exports: Vec<Export>,
}

/// One name a `pub use` offers, or a whole module where it is a glob.
struct Export {
    /// The module the `use` was written in.
    at: String,
    /// What stands before the name, as it was written.
    prefix: String,
    /// The name and what it is offered as, or `None` for `::*`.
    name: Option<(String, String)>,
}

impl Surface {
    /// Read one file's items into this, under the module the **file** is.
    ///
    /// `src/lib.rs` is the crate root, `src/foo.rs` and `src/foo/mod.rs` are
    /// `foo`, `src/foo/bar.rs` is `foo::bar`. A binary's root and anything
    /// under `src/bin/` are not modules of the library at all and are skipped —
    /// a program that calls into this crate cannot reach them.
    ///
    /// **Whether any of it is offered is not decided here**: that takes the
    /// `mod foo;` in the parent, which may be in another file, so it is
    /// [`Surface::resolve`]'s.
    fn read(&mut self, relative: &str, text: &str) -> Result<()> {
        let Some(at) = module_of(relative) else {
            return Ok(());
        };
        let items = nikaia_std::tools::rust::file(text)
            .map_err(|error| anyhow::anyhow!("{relative}: {error}"))?;
        self.walk(&items, &at);
        Ok(())
    }

    fn walk(&mut self, items: &[nikaia_std::tools::rust::Item<'_>], at: &str) {
        use nikaia_std::tools::rust::Item;
        for item in items {
            match item {
                Item::Fun(f) => {
                    self.functions.insert(joined(at, f.name), Function::of(f));
                }
                Item::Rec(r) => {
                    let path = joined(at, r.name);
                    if r.what == "struct" {
                        self.fields.insert(
                            path.clone(),
                            r.parts.iter().map(|p| p.ty.to_string()).collect(),
                        );
                    }
                    self.types.insert(path);
                }
                Item::Export(text) => self.exports.extend(exported(at, text)),
                Item::Group(g) if g.what == "mod" => {
                    let path = joined(at, g.name);
                    self.modules.insert(path.clone(), g.visible);
                    self.walk(&g.items, &path);
                }
                // An `impl` and a `trait` are not described yet: what a method
                // is at a foreign boundary is [ADR-104](../../docs/specification/adr/adr-104.md)
                // D4's own question and nothing here asks it.
                Item::Group(_) => {}
            }
        }
    }

    /// Whether a caller outside the crate may write this path: every module on
    /// the way to it was declared `pub`.
    fn offered(&self, path: &str) -> bool {
        let mut at = String::new();
        let mut parts: Vec<&str> = path.split("::").collect();
        parts.pop();
        for part in parts {
            at = joined(&at, part);
            if self.modules.get(&at) != Some(&true) {
                return false;
            }
        }
        true
    }

    /// Follow the `pub use` items until nothing new becomes reachable.
    ///
    /// **A re-export may name a re-export**, so this runs to a fixed point
    /// rather than once — bounded, because a crate that re-exports in a cycle
    /// does not compile and this is not the place to say so.
    fn resolve(&mut self) {
        // Every item in a module chain that is `pub` all the way, under its own
        // path. This is what the scanner did for **every** item it found, which
        // is how four functions that do not exist reached a draft.
        let direct: Vec<String> = self
            .functions
            .keys()
            .chain(self.types.iter())
            .filter(|path| self.offered(path))
            .cloned()
            .collect();
        for path in direct {
            self.reachable.insert(path.clone(), path);
        }
        // A `pub use` written in a module nobody outside can reach offers
        // nothing to anybody outside.
        let mut exports = std::mem::take(&mut self.exports);
        exports.retain(|export| self.offered(&joined(&export.at, "x")));
        self.exports = exports;
        for _ in 0..8 {
            let mut added = false;
            let exports = std::mem::take(&mut self.exports);
            for export in &exports {
                match &export.name {
                    Some((name, alias)) => {
                        let offered = joined(&export.at, alias);
                        for candidate in self.candidates(export, name) {
                            if !self.functions.contains_key(&candidate)
                                && !self.types.contains(&candidate)
                            {
                                continue;
                            }
                            added |= self.reachable.insert(offered, candidate).is_none();
                            break;
                        }
                    }
                    // A glob offers everything **directly** under the prefix,
                    // which is what `::*` means: a module's own items and not
                    // its submodules'.
                    None => {
                        for base in self.bases(export) {
                            let under = format!("{base}::");
                            let names: Vec<String> = self
                                .functions
                                .keys()
                                .chain(self.types.iter())
                                .filter_map(|path| path.strip_prefix(&under))
                                .filter(|rest| !rest.contains("::"))
                                .map(str::to_string)
                                .collect();
                            for name in names {
                                let offered = joined(&export.at, &name);
                                let target = format!("{base}::{name}");
                                added |= self.reachable.insert(offered, target).is_none();
                            }
                        }
                    }
                }
            }
            self.exports = exports;
            if !added {
                break;
            }
        }
    }

    /// Where a `use` path could resolve, most specific first.
    ///
    /// Rust 2018's uniform paths mean a bare first segment is a name in scope
    /// where the `use` was written *or* at the crate root, and this cannot tell
    /// which without a name table — so it tries both and takes the first that
    /// names something. An item neither names is not resolved, which is the
    /// absence of a claim ([ADR-104](../../docs/specification/adr/adr-104.md)
    /// D4) and reaches the draft as a `?` for a reviewer.
    fn candidates(&self, export: &Export, name: &str) -> Vec<String> {
        self.bases(export)
            .into_iter()
            .map(|base| match base.is_empty() {
                true => name.to_string(),
                false => format!("{base}::{name}"),
            })
            .collect()
    }

    fn bases(&self, export: &Export) -> Vec<String> {
        let prefix = export.prefix.trim();
        // The three words a `use` path may start from, each answered from
        // where the `use` was written.
        if let Some(rest) = after(prefix, "crate") {
            return vec![rest.to_string()];
        }
        if let Some(rest) = after(prefix, "self") {
            return vec![joined(&export.at, rest)];
        }
        if let Some(rest) = after(prefix, "super") {
            let parent = match export.at.rsplit_once("::") {
                Some((up, _)) => up.to_string(),
                None => String::new(),
            };
            return vec![joined(&parent, rest)];
        }
        // Uniform paths: relative to where it was written, or from the root.
        let here = joined(&export.at, prefix);
        let root = prefix.to_string();
        match here == root {
            true => vec![root],
            false => vec![here, root],
        }
    }
}

/// What follows one of a `use` path's leading words, where it starts with it.
///
/// `"crate"` alone and `"crate::a"` are both that word; `"crated"` is not, and
/// the `::` is what says so.
fn after<'a>(path: &'a str, word: &str) -> Option<&'a str> {
    if path == word {
        return Some("");
    }
    path.strip_prefix(word)?.strip_prefix("::")
}

/// The module a file **is**, or `None` where it is not one of the library's.
///
/// `src/main.rs` and `src/bin/*.rs` are a binary's, which a program that calls
/// into this crate cannot reach; `build.rs` is not under `src/` and never
/// arrives here.
fn module_of(relative: &str) -> Option<String> {
    let inside = relative.strip_prefix("src/")?.strip_suffix(".rs")?;
    if inside == "lib.rs" || inside == "lib" {
        return Some(String::new());
    }
    if inside == "main" || inside.starts_with("bin/") {
        return None;
    }
    let path = inside.strip_suffix("/mod").unwrap_or(inside);
    match path.is_empty() {
        true => Some(String::new()),
        false => Some(path.replace('/', "::")),
    }
}

/// `a::b` from `a` and `b`, and either alone where the other is empty.
fn joined(at: &str, name: &str) -> String {
    match (at.is_empty(), name.is_empty()) {
        (true, _) => name.to_string(),
        (_, true) => at.to_string(),
        _ => format!("{at}::{name}"),
    }
}

/// What one `pub use` offers, from the text between the keyword and the `;`.
///
/// **The grammar hands the text over rather than splitting it**, because
/// splitting a path is string work and not parsing work — its own header says
/// so. This is where the pieces are taken.
fn exported(at: &str, text: &str) -> Vec<Export> {
    let text = text.trim();
    // A brace group is the only place several names stand, and the `::` before
    // it is the last one outside it.
    if let Some(open) = text.find("::{") {
        if text.ends_with('}') {
            let prefix = text[..open].to_string();
            let inside = &text[open + 3..text.len() - 1];
            return split_top_level(inside)
                .into_iter()
                .filter(|one| !one.is_empty())
                .map(|one| Export {
                    at: at.to_string(),
                    prefix: prefix.clone(),
                    name: named(&one),
                })
                .collect();
        }
    }
    if let Some(prefix) = text.strip_suffix("::*") {
        return vec![Export {
            at: at.to_string(),
            prefix: prefix.to_string(),
            name: None,
        }];
    }
    let (path, alias) = match text.split_once(" as ") {
        Some((path, alias)) => (path.trim(), Some(alias.trim().to_string())),
        None => (text, None),
    };
    let (prefix, name) = match path.rsplit_once("::") {
        Some((prefix, name)) => (prefix.to_string(), name.trim().to_string()),
        None => (String::new(), path.to_string()),
    };
    let alias = alias.unwrap_or_else(|| name.clone());
    vec![Export {
        at: at.to_string(),
        prefix,
        name: Some((name, alias)),
    }]
}

/// One entry of a `use` list: `name`, or `name as other`.
fn named(one: &str) -> Option<(String, String)> {
    let one = one.trim();
    match one.split_once(" as ") {
        Some((name, alias)) => Some((name.trim().to_string(), alias.trim().to_string())),
        None => Some((one.to_string(), one.to_string())),
    }
}

/// The type constructors that make a value **not sendable** in the language
/// below ([ADR-123](../../docs/specification/adr/adr-123.md) D2).
///
/// **`Cell` and `RefCell` are deliberately not here**, and D2's own list names
/// one of them. They are `!Sync`, not `!Send`: a `Cell<T>` may be *moved* to
/// another thread exactly when its `T` may, and it is being *looked at* from
/// two threads that Rust forbids. A draft that wrote `crosses = false` for one
/// would put a claim in the file that is false, and a reviewer would have to
/// undo it — which is the opposite of what D5 asks a review to do. The record
/// carries the correction.
const NOT_SENDABLE: &[&str] = &["Rc<", "rc::Rc<", "*const ", "*mut ", "NonNull<"];

/// The type constructors a field may be wrapped in without changing the
/// answer, for the `crosses = true` half.
const PASSES_THROUGH: &[&str] = &["Vec<", "Option<", "Box<", "VecDeque<"];

/// One `pub fn`, as its signature reads.
///
/// **No name**: an entry is keyed by the path a caller writes, which
/// [`Surface`] holds and a signature does not.
struct Function {
    /// The crate's own type parameters, which the ledger spells `$T`.
    parameters: Vec<String>,
    /// `name: Type` pairs, in order, as text.
    args: Vec<(String, String)>,
    /// The text after `->`, where there is one.
    result: Option<String>,
    /// `async fn` — a plain `fn` is `sync` (D3).
    pauses: bool,
}

impl Function {
    /// One `pub fn` as the grammar read it.
    ///
    /// **The three text splits left here are lists and not syntax.** A type
    /// parameter list, a `where` bound's head, a receiver — the grammar hands
    /// over what was written and this takes the pieces, which is the same
    /// division `Item::Export` is read under and for the same reason.
    fn of(f: &nikaia_std::tools::rust::Fun<'_>) -> Function {
        Function {
            parameters: split_top_level(f.generics)
                .into_iter()
                // A lifetime is not a type parameter, and a bound written
                // inline (`T: Send`) names the parameter before the colon.
                .filter(|p| !p.starts_with('\''))
                .map(|p| p.split(':').next().unwrap_or(&p).trim().to_string())
                .filter(|p| !p.is_empty())
                .collect(),
            args: f
                .parts
                .iter()
                // The receiver is not an argument a caller writes.
                .filter(|p| p.name != "self")
                .map(|p| (p.name.to_string(), p.ty.to_string()))
                .collect(),
            result: match f.result.is_empty() {
                true => None,
                false => Some(f.result.to_string()),
            },
            pauses: f.pauses,
        }
    }

    /// The entry, and the crate types its signature named.
    fn contract(
        &self,
        crate_word: &str,
        types: &BTreeSet<String>,
    ) -> (FnContract, BTreeSet<String>) {
        let mut mentioned = BTreeSet::new();
        let mut keeps = Vec::new();
        let mut params = Vec::new();
        for (name, text) in &self.args {
            let (ty, kept) = self.translate(text, crate_word, types, &mut mentioned);
            if kept {
                keeps.push(name.clone());
            }
            params.push((name.clone(), ty));
        }
        let mut throws = Vec::new();
        let result = self.result.as_ref().map(|text| {
            let (ty, failing) = self.result_of(text, crate_word, types, &mut mentioned);
            if let Some(error) = failing {
                throws.push(error);
            }
            ty
        });
        let contract = FnContract {
            public: true,
            // **D3's row, and the default is the strict one**: a plain `fn`
            // cannot pause, and an `async fn` can. Nothing between them.
            sync: match self.pauses {
                true => Sync::No,
                false => Sync::Asserted,
            },
            throws,
            keeps,
            signature: Some(Signature {
                params,
                result,
                ..Signature::default()
            }),
            ..FnContract::default()
        };
        (contract, mentioned)
    }

    /// `Result<T, E>` in the result position: the `T` is the type and the `E`
    /// is a `throws`, named where this can name it and `"?"` where it cannot.
    fn result_of(
        &self,
        text: &str,
        crate_word: &str,
        types: &BTreeSet<String>,
        mentioned: &mut BTreeSet<String>,
    ) -> (Ty, Option<String>) {
        if let Some(inner) = generic_of(text, "Result") {
            let parts = split_top_level(&inner);
            let ok = parts.first().cloned().unwrap_or_default();
            let error = parts.get(1).cloned();
            let (ty, _) = self.translate(&ok, crate_word, types, mentioned);
            // The error type where 15.2 can name it (D3). A crate's own error
            // is that crate's type; anything else is the absence of a name,
            // which the ledger spells `"?"`.
            let named = error.map(|error| match types.contains(error.trim()) {
                true => format!("{crate_word}::{}", error.trim()),
                false => "?".to_string(),
            });
            return (ty, Some(named.unwrap_or_else(|| "?".to_string())));
        }
        let (ty, _) = self.translate(text, crate_word, types, mentioned);
        (ty, None)
    }

    /// One Rust type as the ledger's, and whether a value of it is **kept**.
    ///
    /// D3's first row: `&T` is a view and `T` by value is kept — the crate
    /// takes ownership, so the caller may not lend it. A number is not kept in
    /// any sense a caller can act on, so the column names what a caller could
    /// otherwise have gone on using.
    fn translate(
        &self,
        text: &str,
        crate_word: &str,
        types: &BTreeSet<String>,
        mentioned: &mut BTreeSet<String>,
    ) -> (Ty, bool) {
        let text = text.trim();
        if let Some(rest) = text.strip_prefix("&mut ") {
            // `&mut T` is *changed in place*, which the ledger says with
            // `mutates` on the entry rather than on the type — and this
            // scraper has no place to put it. `?` is the absence of a claim,
            // which is the fail-closed direction (D4).
            let _ = rest;
            return (Ty::Unknown, false);
        }
        if let Some(rest) = text.strip_prefix('&') {
            let (ty, _) = self.translate(rest, crate_word, types, mentioned);
            return (view_of(ty), false);
        }
        if let Some(inner) = generic_of(text, "Option") {
            let (ty, kept) = self.translate(&inner, crate_word, types, mentioned);
            return (Ty::Nullable(Box::new(ty)), kept);
        }
        if let Some(inner) = generic_of(text, "Vec") {
            let (ty, _) = self.translate(&inner, crate_word, types, mentioned);
            return (
                Ty::Named {
                    name: "Vec".to_string(),
                    args: vec![ty],
                    view: false,
                },
                true,
            );
        }
        // A type parameter of this function is the ledger's variable.
        if self.parameters.iter().any(|p| p == text) {
            return (
                Ty::Var {
                    name: text.to_string(),
                    view: false,
                },
                true,
            );
        }
        if PLAIN.contains(&text) {
            return (Ty::named(text), false);
        }
        if text == "String" {
            return (Ty::named("String"), true);
        }
        if types.contains(text) {
            mentioned.insert(text.to_string());
            return (Ty::named(format!("{crate_word}::{text}")), true);
        }
        // Anything else is a name this scraper cannot account for — another
        // crate's type, a trait object, a path with segments. `?` is the
        // absence of a claim and D5's own `?`, for a reviewer to fill.
        (Ty::Unknown, true)
    }
}

/// Whether a value of a type may cross a thread, read off its **fields**
/// ([ADR-123](../../docs/specification/adr/adr-123.md) D2).
///
/// Three answers and the third is the common one. `false` where a field holds
/// something the language below marks as not sendable, which a field reader can
/// see and a reviewer can check; `true` where every field is something this
/// knows to be sendable; and **nothing** where it cannot tell — a field of
/// another crate's type, a tuple struct, a body this could not read. Silence is
/// *nobody said*, which is not permission
/// ([ADR-010](../../docs/specification/adr/adr-010.md) D1) and not a refusal
/// either.
///
/// **The `true` half is narrow on purpose.** A promise that a value may cross
/// is a promise a *refusal* is withheld on, so it is made only where every
/// field is a scalar, a `String`, or one of those inside a container that
/// changes nothing. An `Arc<T>` is `Send` exactly when its `T` is `Send` **and**
/// `Sync`, which is two questions a scraper does not have; it falls to silence.
fn crosses(fields: Option<&[String]>) -> crate::contracts::Crosses {
    use crate::contracts::Crosses;
    let Some(fields) = fields.filter(|fields| !fields.is_empty()) else {
        return Crosses::Undecided;
    };
    if fields
        .iter()
        .any(|ty| NOT_SENDABLE.iter().any(|marker| ty.contains(marker)))
    {
        return Crosses::MayNot;
    }
    match fields.iter().all(|ty| plainly_sendable(ty)) {
        true => Crosses::May,
        false => Crosses::Undecided,
    }
}

/// Whether one field's type is something this can say crosses, with nothing
/// left over to be wrong about.
fn plainly_sendable(ty: &str) -> bool {
    let ty = ty.trim();
    if let Some(rest) = ty.strip_prefix('&') {
        // A view crosses where what it points at does — and whether *that* is
        // a view of something borrowed for long enough is a lifetime question
        // this does not have. Silence.
        let _ = rest;
        return false;
    }
    for wrapper in PASSES_THROUGH {
        if let Some(inner) = generic_of(ty, wrapper.trim_end_matches('<')) {
            return split_top_level(&inner)
                .iter()
                .all(|part| plainly_sendable(part));
        }
    }
    PLAIN.contains(&ty) || ty == "String"
}

/// A view of a type, which the ledger's language writes with the `&` on the
/// name.
fn view_of(ty: Ty) -> Ty {
    match ty {
        Ty::Named { name, args, .. } => Ty::Named {
            name,
            args,
            view: true,
        },
        Ty::Var { name, .. } => Ty::Var { name, view: true },
        other => other,
    }
}

/// The scalar names that travel unchanged, plus `str` which is only ever seen
/// behind a `&`.
const PLAIN: &[&str] = &[
    "bool", "char", "str", "i8", "i16", "i32", "i64", "i128", "isize", "u8", "u16", "u32", "u64",
    "u128", "usize", "f32", "f64", "()",
];

/// `Name<inner>` unwrapped, where the text is exactly that.
fn generic_of(text: &str, name: &str) -> Option<String> {
    let rest = text.trim().strip_prefix(name)?.trim_start();
    let rest = rest.strip_prefix('<')?;
    let inner = rest.strip_suffix('>')?;
    Some(inner.to_string())
}

/// Split on commas that are not inside brackets.
fn split_top_level(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut depth = 0_i32;
    let mut current = String::new();
    for c in text.chars() {
        match c {
            '<' | '(' | '[' => depth += 1,
            '>' | ')' | ']' => depth -= 1,
            ',' if depth == 0 => {
                out.push(current.trim().to_string());
                current.clear();
                continue;
            }
            _ => {}
        }
        current.push(c);
    }
    if !current.trim().is_empty() {
        out.push(current.trim().to_string());
    }
    out
}
