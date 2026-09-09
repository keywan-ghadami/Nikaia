//! The build cache, driven by the real emitter (ADR-021).
//!
//! `bridge-orchestrator` unit-tests the key's dimensions against synthetic
//! records. What it cannot check from there is the thing the ADR is actually
//! worried about: that the artifacts the cache hands back are the ones the
//! emitter would have produced. That needs a real `.nika` file whose lowering
//! *differs* between the profiles, or the check passes without checking
//! anything - which is the failure mode D5 names by its own test,
//! `the_profiles_agree_on_every_example`.

mod common;

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Command;

use bridge_orchestrator::cache::{Cache, Choices};
use nikaia::emit::{emit_program, Profile};
use nikaia::parser::parse_to_ast;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Lowers `source` the way the `rust` backend does.
fn lower(source: &str, profile: Profile) -> String {
    let parsed = parse_to_ast(source).expect("parse");
    emit_program(&parsed, profile).expect("emit").rust
}

fn cache_in(dir: &std::path::Path) -> Cache {
    Cache::open(
        dir.join("nikaia.lock"),
        dir.join("target/nikaia/cache"),
        "test-toolchain",
        "test-compiler",
    )
    .expect("open cache")
}

/// An example that lowers differently under Lite and Advanced, so that a cache
/// which confused the two would be caught. `1brc.nika` is the one the
/// specification leans on for exactly this difference (ADR-009's `par_fold`).
fn a_profile_sensitive_example() -> String {
    let path = repo_root().join("examples/1brc.nika");
    let source = std::fs::read_to_string(&path).expect("1brc.nika is readable");
    assert_ne!(
        lower(&source, Profile::Lite),
        lower(&source, Profile::Advanced),
        "this test is only meaningful while {} lowers differently per profile; \
         if that changed, pick another example rather than deleting the check",
        path.display()
    );
    source
}

#[test]
fn an_unchanged_unit_comes_back_from_the_cache_unchanged() {
    let dir = common::scratch_dir("cache-roundtrip");
    let source = a_profile_sensitive_example();
    let choices = Choices::new("advanced", "rust");
    let expected = lower(&source, Profile::Advanced);

    let mut cache = cache_in(&dir);
    assert!(
        cache.lookup("1brc.nika", &source, &choices, &dir).is_none(),
        "nothing is cached before the first build"
    );

    cache
        .record("1brc.nika", &source, BTreeMap::new(), &choices, &expected)
        .expect("record");

    assert_eq!(
        cache
            .lookup("1brc.nika", &source, &choices, &dir)
            .as_deref(),
        Some(expected.as_str()),
        "the cached artifact is what the emitter produced"
    );

    std::fs::remove_dir_all(&dir).ok();
}

/// ADR-021 D5. The profile is in the key, so each profile gets its own entry
/// and neither is ever served the other's.
#[test]
fn the_two_profiles_never_serve_each_others_artifacts() {
    let dir = common::scratch_dir("cache-profiles");
    let source = a_profile_sensitive_example();

    let lite = Choices::new("lite", "rust");
    let advanced = Choices::new("advanced", "rust");
    let lowered_lite = lower(&source, Profile::Lite);
    let lowered_advanced = lower(&source, Profile::Advanced);

    let mut cache = cache_in(&dir);
    cache
        .record("1brc.nika", &source, BTreeMap::new(), &lite, &lowered_lite)
        .expect("record lite");
    cache
        .record(
            "1brc.nika",
            &source,
            BTreeMap::new(),
            &advanced,
            &lowered_advanced,
        )
        .expect("record advanced");

    // Same unit, same source, two profiles - and each has to come back as
    // itself. Recording the second must not have displaced the first.
    assert_eq!(
        cache.lookup("1brc.nika", &source, &lite, &dir).as_deref(),
        Some(lowered_lite.as_str()),
        "lite is served lite"
    );
    assert_eq!(
        cache
            .lookup("1brc.nika", &source, &advanced, &dir)
            .as_deref(),
        Some(lowered_advanced.as_str()),
        "advanced is served advanced"
    );

    std::fs::remove_dir_all(&dir).ok();
}

/// A changed source is a different build, and the previous artifact must not
/// answer for it.
#[test]
fn an_edited_source_misses() {
    let dir = common::scratch_dir("cache-edit");
    let source = a_profile_sensitive_example();
    let choices = Choices::new("advanced", "rust");

    let mut cache = cache_in(&dir);
    cache
        .record(
            "1brc.nika",
            &source,
            BTreeMap::new(),
            &choices,
            &lower(&source, Profile::Advanced),
        )
        .expect("record");

    let edited = format!("{source}\n// a comment is still a change\n");
    assert!(
        cache.lookup("1brc.nika", &edited, &choices, &dir).is_none(),
        "an edited source must not be answered by the old artifact"
    );

    std::fs::remove_dir_all(&dir).ok();
}

/// The lockfile is the committed record (D2), and the build-time choices are
/// deliberately not in it (D5) - otherwise switching profile would rewrite a
/// tracked file for a diff that means nothing.
#[test]
fn the_lockfile_survives_a_round_trip_and_holds_no_choices() {
    let dir = common::scratch_dir("cache-lockfile");
    let source = a_profile_sensitive_example();

    let mut cache = cache_in(&dir);
    cache
        .record(
            "1brc.nika",
            &source,
            BTreeMap::new(),
            &Choices::new("lite", "rust"),
            &lower(&source, Profile::Lite),
        )
        .expect("record");
    cache.save().expect("save");

    let text = std::fs::read_to_string(dir.join("nikaia.lock")).expect("lockfile written");
    assert!(text.contains("1brc.nika"), "the unit is recorded:\n{text}");
    assert!(
        !text.contains("lite"),
        "the profile is a choice and must not be recorded:\n{text}"
    );

    // Reopening sees the same units, so a second invocation can look up what
    // the first recorded.
    let reopened = cache_in(&dir);
    assert_eq!(
        reopened.lockfile().units.keys().collect::<Vec<_>>(),
        vec!["1brc.nika"]
    );

    std::fs::remove_dir_all(&dir).ok();
}

/// The CLI's half of the bargain, and the reason the cache can be a default at
/// all: a cache that cannot be written slows the next build down and never
/// fails this one. Driven through the real binary, because the decision that
/// a cache error is not fatal lives in `main.rs` and nowhere else.
///
/// The store is aimed below a *file*, which no process can create a directory
/// inside - including root, so this holds in a container too.
#[test]
fn a_broken_cache_does_not_break_the_build() {
    let dir = common::scratch_dir("cache-degrades");
    let input = dir.join("hello.nika");
    std::fs::copy(repo_root().join("tests/samples/hello_world.nika"), &input).expect("sample");
    let blocker = dir.join("blocker");
    std::fs::write(&blocker, "not a directory").expect("blocker");
    let output = dir.join("out.rs");

    let run = Command::new(env!("CARGO_BIN_EXE_nikaia"))
        .args(["--input", input.to_str().unwrap()])
        .args(["--backend", "rust"])
        .args(["--output", output.to_str().unwrap()])
        .env("NIKAIA_CACHE_DIR", blocker.join("cache"))
        .output()
        .expect("the nikaia binary runs");

    assert!(
        run.status.success(),
        "an unusable cache must not fail the build\nstderr: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert!(
        String::from_utf8_lossy(&run.stderr).contains("cache"),
        "and it should say so rather than failing silently: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    let emitted = std::fs::read_to_string(&output).expect("the Rust is written anyway");
    assert!(!emitted.is_empty(), "and it is not empty");

    std::fs::remove_dir_all(&dir).ok();
}

/// Caching is on without asking, and leaves the source directory alone when
/// there is no project to own a lockfile.
#[test]
fn the_cache_is_on_by_default_and_writes_nothing_beside_the_source() {
    let dir = common::scratch_dir("cache-default-on");
    let src = dir.join("src");
    let home = dir.join("cache");
    std::fs::create_dir_all(&src).expect("dirs");
    let input = src.join("hello.nika");
    std::fs::copy(repo_root().join("tests/samples/hello_world.nika"), &input).expect("sample");

    let run = |args: &[&str]| {
        let out = Command::new(env!("CARGO_BIN_EXE_nikaia"))
            .args(["--input", input.to_str().unwrap()])
            .args(["--backend", "rust"])
            .args(args)
            .env("NIKAIA_CACHE_DIR", &home)
            .output()
            .expect("the nikaia binary runs");
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).into_owned()
    };

    assert!(
        !run(&[]).contains("from cache"),
        "the first run has to lower"
    );
    assert!(
        run(&[]).contains("from cache"),
        "the second must be served without being asked to be"
    );
    assert!(
        !run(&["--no-cache"]).contains("from cache"),
        "and --no-cache is the way out"
    );

    // The whole point of the default: the build's own outputs are welcome
    // beside the source - the emitted Rust, and the ledger ADR-020 writes
    // where a build puts what it produced - but nothing belonging to the
    // *cache* may appear there when no project owns a lockfile.
    let mut left: Vec<String> = std::fs::read_dir(&src)
        .expect("read src")
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    left.sort();
    assert_eq!(
        left,
        vec!["hello.nika", "hello.rs", "nikaia.contracts"],
        "only the source and what the build produced"
    );
    assert!(
        !src.join("nikaia.lock").exists() && !src.join("target").exists(),
        "no cache artifact may be written beside the source outside a project"
    );

    std::fs::remove_dir_all(&dir).ok();
}
