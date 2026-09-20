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
// does not give up a stable-only toolchain for it. So this reads `pub fn` and
// `pub struct` items out of `.rs` text — a **signature scraper** and not a Rust
// parser, and the difference is the whole of what it cannot do:
//
//   * an item a macro generates is not in the text and is not found;
//   * a signature this cannot translate is written `?`, which is the absence of
//     a claim and never a guess (D4's own sentence);
//   * a `pub` item inside a `mod` block is read as the crate's own, because the
//     module path a caller writes is a thing only a real parser knows.
//
// Each of those is D5's case: the draft is **committed and reviewed like code**,
// and a `?` in it is a person's to fill. A scraper that guessed would put a
// claim in a file nobody wrote, which is the one thing a boundary description
// may not do.
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
    let mut scraped = BTreeMap::new();
    let mut types = BTreeSet::new();
    let mut hashes = BTreeMap::new();
    for (relative, text) in &sources.files {
        hashes.insert(
            relative.clone(),
            orchestrator::cache::sha256_hex(text.as_bytes()),
        );
        for item in items_of(text) {
            match item {
                Item::Fn(function) => {
                    scraped.insert(function.name.clone(), function);
                }
                Item::Struct(name) => {
                    types.insert(name);
                }
            }
        }
    }

    let mut ledger = Ledger::empty();
    ledger.inference = "described-from-signatures".to_string();
    ledger.sources = hashes;
    let mut unanswered = Vec::new();
    let mut named_types: BTreeSet<String> = BTreeSet::new();
    for name in &wanted {
        let Some(function) = scraped.get(name) else {
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

/// A `pub` item a signature scraper found.
enum Item {
    Fn(Function),
    Struct(String),
}

/// One `pub fn`, as its signature reads.
struct Function {
    name: String,
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

/// Every `pub fn` and `pub struct` in a file's text.
///
/// **A scraper and not a parser**, which the module header says is the scope:
/// the item has to be written `pub fn name(…)` in the source, at the start of a
/// line after whatever indentation. An item a macro writes is not in the text,
/// and one this cannot read is simply not found — which is D1's case again, and
/// the same command.
fn items_of(text: &str) -> Vec<Item> {
    let mut out = Vec::new();
    let bytes: Vec<char> = text.chars().collect();
    for (at, line) in line_starts(text) {
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix("pub struct ") {
            let name: String = rest
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            if !name.is_empty() {
                out.push(Item::Struct(name));
            }
            continue;
        }
        let (pauses, rest) = match trimmed.strip_prefix("pub async fn ") {
            Some(rest) => (true, rest),
            None => match trimmed.strip_prefix("pub fn ") {
                Some(rest) => (false, rest),
                None => continue,
            },
        };
        let name: String = rest
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        if name.is_empty() {
            continue;
        }
        let after = at + (line.len() - trimmed.len()) + (trimmed.len() - rest.len()) + name.len();
        if let Some(function) = signature_at(&bytes, after, name, pauses) {
            out.push(Item::Fn(function));
        }
    }
    out
}

/// Every line of a text with the character offset it starts at.
fn line_starts(text: &str) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    let mut at = 0;
    for line in text.split('\n') {
        out.push((at, line));
        at += line.chars().count() + 1;
    }
    out
}

/// The signature after a function's name: its type parameters, its arguments
/// and its result.
fn signature_at(text: &[char], from: usize, name: String, pauses: bool) -> Option<Function> {
    let mut at = from;
    let mut parameters = Vec::new();
    if text.get(at) == Some(&'<') {
        let close = matching(text, at, '<', '>')?;
        parameters = split_top_level(&text[at + 1..close].iter().collect::<String>())
            .into_iter()
            // A lifetime is not a type parameter, and a bound written inline
            // (`T: Send`) names the parameter before the colon.
            .filter(|p| !p.starts_with('\''))
            .map(|p| p.split(':').next().unwrap_or(&p).trim().to_string())
            .filter(|p| !p.is_empty())
            .collect();
        at = close + 1;
    }
    while text.get(at).is_some_and(|c| c.is_whitespace()) {
        at += 1;
    }
    if text.get(at) != Some(&'(') {
        return None;
    }
    let close = matching(text, at, '(', ')')?;
    let args = split_top_level(&text[at + 1..close].iter().collect::<String>())
        .into_iter()
        .filter_map(|arg| {
            let (name, ty) = arg.split_once(':')?;
            Some((name.trim().to_string(), ty.trim().to_string()))
        })
        .collect();
    let mut at = close + 1;
    while text.get(at).is_some_and(|c| c.is_whitespace()) {
        at += 1;
    }
    let result = match text.get(at) == Some(&'-') && text.get(at + 1) == Some(&'>') {
        false => None,
        true => {
            let mut end = at + 2;
            let mut depth = 0_i32;
            while let Some(c) = text.get(end) {
                match c {
                    '<' | '(' | '[' => depth += 1,
                    '>' | ')' | ']' => depth -= 1,
                    '{' | ';' if depth == 0 => break,
                    _ => {}
                }
                // `where` ends the result as surely as a brace does.
                if depth == 0 && text[end..].starts_with(&['w', 'h', 'e', 'r', 'e']) {
                    break;
                }
                end += 1;
            }
            Some(
                text[at + 2..end]
                    .iter()
                    .collect::<String>()
                    .trim()
                    .to_string(),
            )
        }
    };
    Some(Function {
        name,
        parameters,
        args,
        result,
        pauses,
    })
}

/// The index of the bracket that closes the one at `from`.
fn matching(text: &[char], from: usize, open: char, close: char) -> Option<usize> {
    let mut depth = 0_i32;
    for (at, c) in text.iter().enumerate().skip(from) {
        if *c == open {
            depth += 1;
        } else if *c == close {
            depth -= 1;
            if depth == 0 {
                return Some(at);
            }
        }
    }
    None
}
