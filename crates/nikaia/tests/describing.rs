//! `nikaia describe <crate>` — the draft a boundary is described by
//! ([ADR-104](../../../docs/specification/adr/adr-104.md) D2, D3, D4).
//!
//! D1 refuses a call into a crate nothing describes and names this command.
//! What it writes is the file every analysis then reads there, so what it gets
//! wrong is wrong at the boundary of a program — which is why the strongest
//! test here is not a fixture: it reads the repository's own
//! `examples/foreign-runtime/shim` and asserts the draft against the file a
//! reviewer wrote by hand, entry for entry.
//!
//! **What a scraper cannot do is asserted too.** It reads `pub fn` and
//! `pub struct` out of `.rs` text; a signature it cannot translate is written
//! `?`, which is the absence of a claim and D5's own `?` for a person to fill.
//! A test that only showed the cases it gets right would be describing a
//! different tool.

use std::path::{Path, PathBuf};

use nikaia::describe::draft;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// A project with one Rust path dependency, and whatever that crate's sources
/// and the program's text are.
fn project(name: &str, crate_source: &str, program: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "nikaia-describe-{name}-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("src")).expect("a directory to work in");
    // The path is relative to the **generated** manifest, which a build writes
    // to `target/nikaia/build/` — so four levels up from there is the project's
    // own directory, and the crate sits beside `src/`.
    std::fs::create_dir_all(root.join("fremd/src")).expect("a crate to describe");
    std::fs::write(
        root.join("nikaia.toml"),
        "[package]\n\
         name = \"probe\"\n\
         version = \"0.1.0\"\n\
         \n\
         [dependencies]\n\
         fremd = { type = \"rust\", path = \"../../../fremd\" }\n",
    )
    .expect("write the manifest");
    std::fs::write(
        root.join("fremd/Cargo.toml"),
        "[package]\nname = \"fremd\"\nversion = \"2.1.0\"\nedition = \"2021\"\n",
    )
    .expect("write the crate's manifest");
    std::fs::write(root.join("fremd/src/lib.rs"), crate_source).expect("write the crate");
    std::fs::write(root.join("src/main.nika"), program).expect("write the program");
    root
}

/// The entries of a draft, as the file would read them.
fn entries(root: &Path) -> String {
    let (ledger, described) = draft(root, "fremd").expect("the draft is written");
    let text = ledger.render_description("fremd", &described.version);
    let _ = std::fs::remove_dir_all(root);
    text
}

/// **D3's table, row by row**, on signatures written to exercise each one.
#[test]
fn a_signature_is_translated_by_the_table() {
    let root = project(
        "table",
        "pub struct Handle { inner: u32 }\n\
         pub fn plain(port: i64) -> String { String::new() }\n\
         pub fn lent(text: &str) -> bool { true }\n\
         pub fn owned(text: String) -> Handle { Handle { inner: 0 } }\n\
         pub async fn pausing(n: i64) -> i64 { n }\n\
         pub fn failing(n: i64) -> Result<i64, Handle> { Ok(n) }\n\
         pub fn maybe(n: i64) -> Option<String> { None }\n\
         pub fn over<T>(value: T) -> String where T: Send { String::new() }\n\
         pub fn unreadable(x: std::collections::HashMap<u8, u8>) -> u8 { 0 }\n",
        "fn main() {\n\
         \x20   fremd::plain(1)\n\
         \x20   fremd::lent(\"a\")\n\
         \x20   fremd::owned(\"a\")\n\
         \x20   fremd::pausing(1)\n\
         \x20   fremd::failing(1)\n\
         \x20   fremd::maybe(1)\n\
         \x20   fremd::over(1)\n\
         \x20   fremd::unreadable(1)\n\
         }\n",
    );
    let text = entries(&root);

    // `T` by value is **kept**: the crate takes ownership, so the caller may
    // not lend it. A number is not, because there is nothing a caller could
    // otherwise have gone on using.
    assert!(
        text.contains("[fn.\"fremd::plain\"]\npub = true\nsync = true\nsignature = \"(port: i64) -> String\"\n"),
        "{text}"
    );
    // `&T` is a view, and a view is not kept.
    assert!(
        text.contains(
            "[fn.\"fremd::lent\"]\npub = true\nsync = true\nsignature = \"(text: &str) -> bool\"\n"
        ),
        "{text}"
    );
    assert!(text.contains("keeps = [\"text\"]"), "{text}");
    assert!(
        text.contains("signature = \"(text: String) -> fremd::Handle\""),
        "the crate's own type carries the crate's word: {text}"
    );
    // `async fn` may pause; a plain `fn` is `sync`.
    assert!(
        text.contains("[fn.\"fremd::pausing\"]\npub = true\nsignature ="),
        "an `async fn` gets no `sync` line: {text}"
    );
    // `Result<T, E>` is a `throws`, with the error named where it can be.
    assert!(text.contains("throws = [\"fremd::Handle\"]"), "{text}");
    assert!(
        text.contains("[fn.\"fremd::failing\"]")
            && text.contains("signature = \"(n: i64) -> i64\""),
        "{text}"
    );
    // `Option<T>` is `T?`.
    assert!(
        text.contains("signature = \"(n: i64) -> String?\""),
        "{text}"
    );
    // A type parameter is the ledger's variable.
    assert!(
        text.contains("signature = \"(value: $T) -> String\""),
        "{text}"
    );
    // And a type this cannot account for is `?` — the absence of a claim, for
    // a reviewer to fill (D5), never a guess.
    assert!(text.contains("signature = \"(x: ?) -> u8\""), "{text}");

    // The type an entry names gets an entry of its own, and its `crosses`:
    // whether a value of it may cross a thread follows from its *fields*, which
    // the describer reads (ADR-123 D2): `Handle`'s one field is a `u32`.
    assert!(
        text.contains("[type.\"fremd::Handle\"]\npub = true\ncrosses = true\n"),
        "{text}"
    );
}

/// **The one claim that comes from a field** — `crosses`
/// ([ADR-123](../../../docs/specification/adr/adr-123.md) D2), and the one
/// thing here a Rust *signature* could never say.
///
/// Three answers and the third is the common one: `false` where a field holds
/// something the language below marks as not sendable, `true` where every field
/// is something the scraper knows to be sendable, and **nothing** where it
/// cannot tell. Silence is *nobody said*, which is not permission
/// ([ADR-010](../../../docs/specification/adr/adr-010.md) D1) and not a refusal
/// either.
#[test]
fn a_types_fields_answer_whether_it_crosses() {
    let root = project(
        "crosses",
        "pub struct Held { inner: std::rc::Rc<String> }\n\
         pub struct Pointed { at: *const u8 }\n\
         pub struct Plain { n: i64, name: String, more: Vec<i64> }\n\
         pub struct Opaque { held: OtherCratesThing }\n\
         pub struct Counted { shared: std::sync::Arc<String> }\n\
         pub struct Tuple(i64);\n\
         pub fn a(x: Held) -> i64 { 0 }\n\
         pub fn b(x: Pointed) -> i64 { 0 }\n\
         pub fn c(x: Plain) -> i64 { 0 }\n\
         pub fn d(x: Opaque) -> i64 { 0 }\n\
         pub fn e(x: Counted) -> i64 { 0 }\n\
         pub fn f(x: Tuple) -> i64 { 0 }\n",
        "fn main() {\n\
         \x20   fremd::a(1)\n\
         \x20   fremd::b(1)\n\
         \x20   fremd::c(1)\n\
         \x20   fremd::d(1)\n\
         \x20   fremd::e(1)\n\
         \x20   fremd::f(1)\n\
         }\n",
    );
    let text = entries(&root);
    let says = |name: &str| {
        let at = text
            .find(&format!("[type.\"fremd::{name}\"]"))
            .unwrap_or_else(|| panic!("no entry for {name}:\n{text}"));
        let rest = &text[at..];
        let end = rest[1..].find("\n[").map(|e| e + 1).unwrap_or(rest.len());
        rest[..end].to_string()
    };

    // An `Rc` and a raw pointer are what D2 names, and a field reader sees both.
    assert!(says("Held").contains("crosses = false"), "{}", says("Held"));
    assert!(
        says("Pointed").contains("crosses = false"),
        "{}",
        says("Pointed")
    );

    // Every field a scalar, a `String`, or one of those in a container that
    // changes nothing.
    assert!(
        says("Plain").contains("crosses = true"),
        "{}",
        says("Plain")
    );

    // **And silence for the rest**, which is the common answer rather than the
    // exception: a field of another crate's type says nothing, and an `Arc<T>`
    // is `Send` exactly when its `T` is `Send` *and* `Sync` — two questions a
    // scraper does not have.
    assert!(!says("Opaque").contains("crosses"), "{}", says("Opaque"));
    assert!(!says("Counted").contains("crosses"), "{}", says("Counted"));

    // A tuple struct's body is not read at all, and *nobody looked* is not
    // *it holds nothing*.
    assert!(!says("Tuple").contains("crosses"), "{}", says("Tuple"));
}

/// **An entry exists because a program asked for it** ([ADR-028](../../../docs/specification/adr/adr-028.md)
/// D5), so the draft is proportional to use and not to the crate.
#[test]
fn only_what_the_program_calls_is_described() {
    let root = project(
        "asked",
        "pub fn used(n: i64) -> i64 { n }\n\
         pub fn unused(n: i64) -> i64 { n }\n",
        "fn main() { fremd::used(1) }\n",
    );
    let text = entries(&root);
    assert!(text.contains("fremd::used"), "{text}");
    assert!(!text.contains("fremd::unused"), "{text}");
}

/// **A name no signature answered is named**, because that is what a reviewer
/// does next: it is a macro's item, or a `mod` this scraper read flat, or a
/// typo — and a draft that silently left it out would look complete.
#[test]
fn a_name_no_signature_answers_is_reported() {
    let root = project(
        "unanswered",
        "pub fn used(n: i64) -> i64 { n }\n",
        "fn main() {\n\
         \x20   fremd::used(1)\n\
         \x20   fremd::from_a_macro(1)\n\
         }\n",
    );
    let (_, described) = draft(&root, "fremd").expect("the draft is written");
    assert_eq!(described.unanswered, ["fremd::from_a_macro"]);
    let _ = std::fs::remove_dir_all(&root);
}

/// **A version dependency says why it cannot be read**, rather than guessing at
/// Cargo's registry cache — which would be resolving a version, the one thing
/// [ADR-002](../../../docs/specification/adr/adr-002.md) D1 hands to Cargo and
/// never does itself.
#[test]
fn a_version_dependency_says_what_is_missing() {
    let root = std::env::temp_dir().join(format!("nikaia-describe-version-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("a directory to work in");
    std::fs::write(
        root.join("nikaia.toml"),
        "[package]\nname = \"probe\"\nversion = \"0.1.0\"\n\n\
         [dependencies]\nregex = { type = \"rust\", version = \"1\" }\n",
    )
    .expect("write the manifest");
    let said = format!(
        "{:#}",
        draft(&root, "regex").expect_err("no sources to read")
    );
    assert!(said.contains("registry cache"), "{said}");
    assert!(said.contains("ADR-104 D5"), "{said}");
    let _ = std::fs::remove_dir_all(&root);
}

/// And a crate the manifest does not declare is not describable, because the
/// project is what says a build links against it (D1).
#[test]
fn a_crate_nothing_declares_is_not_described() {
    let root = project(
        "undeclared",
        "pub fn used(n: i64) -> i64 { n }\n",
        "fn main() { }\n",
    );
    let said = format!(
        "{:#}",
        draft(&root, "regex").expect_err("nothing declares it")
    );
    assert!(
        said.contains("nothing in this project's `nikaia.toml`"),
        "{said}"
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// **The measurement**: the draft for the repository's own experiment, against
/// the file a reviewer wrote by hand before this command existed.
///
/// `examples/foreign-runtime/*/contracts/hyper_shim.contracts` is what
/// [ADR-104](../../../docs/specification/adr/adr-104.md) §5 calls *the file
/// `nikaia describe` will write, produced the way a reviewer would check it*.
/// This is that claim, tested — and it needs no network, because the crate is
/// in the tree.
///
/// **One thing in the reviewed file is not in the draft, and it is the
/// comments** — a reviewer's, and the only part of D5's review a command cannot
/// do. `crosses = false` on `LocalHandle` **is** in the draft since the
/// describer reads fields ([ADR-123](../../../docs/specification/adr/adr-123.md)
/// D2), which is the line ADR-104 §5 said the crossing refusals were waiting
/// for.
#[test]
fn the_draft_for_the_experiment_is_the_file_a_reviewer_wrote() {
    for (project, expected) in [
        (
            "serve",
            vec![
                "[fn.\"hyper_shim::across_a_thread\"]\npub = true\nsync = true\nkeeps = [\"value\"]\nsignature = \"(value: $T) -> String\"",
                "[fn.\"hyper_shim::serve_once\"]\npub = true\nsync = true\nsignature = \"(port: i64) -> String\"",
            ],
        ),
        (
            "crossing",
            vec![
                "[fn.\"hyper_shim::across_a_thread\"]\npub = true\nsync = true\nkeeps = [\"value\"]\nsignature = \"(value: $T) -> String\"",
                "[fn.\"hyper_shim::local_handle\"]\npub = true\nsync = true\nkeeps = [\"name\"]\nsignature = \"(name: String) -> hyper_shim::LocalHandle\"",
                "[type.\"hyper_shim::LocalHandle\"]\npub = true\ncrosses = false",
            ],
        ),
        (
            "smuggled",
            vec![
                "[fn.\"hyper_shim::across_a_thread_unchecked\"]\npub = true\nsync = true\nkeeps = [\"value\"]\nsignature = \"(value: $T) -> String\"",
            ],
        ),
    ] {
        let root = repo_root().join("examples/foreign-runtime").join(project);
        let (ledger, described) = draft(&root, "hyper_shim").expect("the draft is written");
        assert_eq!(described.version, "0.1.0", "{project}");
        let text = ledger.render_description("hyper_shim", &described.version);
        for entry in expected {
            assert!(text.contains(entry), "{project}:\n{text}");
        }
        // The hash the reviewed file records is the hash of the crate's source,
        // which is what makes the description believable while it holds (D5).
        let reviewed = std::fs::read_to_string(root.join("contracts/hyper_shim.contracts"))
            .expect("the reviewed file");
        let hash = reviewed
            .lines()
            .find(|line| line.starts_with("\"src/lib.rs\""))
            .expect("the reviewed file records the source's hash");
        assert!(text.contains(hash), "{project}: the draft records {hash}\n{text}");
    }
}

/// **Nothing is written by a draft**, which is what makes the test above able
/// to read the repository's own projects without putting them back.
#[test]
fn a_draft_writes_nothing() {
    let root = repo_root().join("examples/foreign-runtime/serve");
    let before = std::fs::read_to_string(root.join("contracts/hyper_shim.contracts"))
        .expect("the reviewed file");
    let _ = draft(&root, "hyper_shim").expect("the draft is written");
    let after = std::fs::read_to_string(root.join("contracts/hyper_shim.contracts"))
        .expect("the reviewed file");
    assert_eq!(before, after);
}
