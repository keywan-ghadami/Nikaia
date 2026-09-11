//! Compiling emitted Rust from inside a test.
//!
//! A generated program links `nikaia_std`, `winnow_grammar` and `winnow`, and
//! all three are in cargo's deps directory - along with other copies of them:
//! the build-dependency graph builds the parser backend a second time under a
//! different Cargo profile, and an earlier build may have left a stale hash behind.
//! Two copies of `winnow` in one program is a type error at every `Stream`
//! bound, so guessing by modification time is not good enough.
//!
//! The copies that belong together are named in the crate metadata: an rlib
//! lists its dependencies as `name-hash`, and the hash is the one in the
//! file name. So one anchor - the newest `nikaia_std`, of which a build has
//! exactly one - and the rest is read, not guessed.

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// The `rustc` cargo built this test with. `build.rs` hands it over: the crates
/// in the deps directory were built by it, and any other toolchain on PATH
/// would only report that.
pub fn rustc() -> &'static str {
    env!("NIKAIA_RUSTC")
}

pub fn deps_dir() -> PathBuf {
    std::env::current_exe()
        .expect("test binary path")
        .parent()
        .expect("deps directory")
        .to_path_buf()
}

/// The newest rlib of a crate. Used for the anchor only.
fn newest_rlib(crate_name: &str) -> PathBuf {
    let prefix = format!("lib{crate_name}-");
    let mut candidates: Vec<_> = std::fs::read_dir(deps_dir())
        .expect("read deps directory")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default();
            name.starts_with(&prefix) && name.ends_with(".rlib")
        })
        .collect();

    candidates.sort_by_key(|path| std::fs::metadata(path).and_then(|m| m.modified()).ok());
    candidates
        .pop()
        .unwrap_or_else(|| panic!("no {crate_name} rlib in {}", deps_dir().display()))
}

/// The rlib `of` was built against for `crate_name`, from `of`'s own metadata.
fn dependency_of(of: &Path, crate_name: &str) -> PathBuf {
    let listing = Command::new(rustc())
        .arg("-Zls=root")
        .arg(of)
        .output()
        .expect("run rustc -Z ls");
    let listing = String::from_utf8_lossy(&listing.stdout);

    // Lines read `N winnow-97715e7004541405 hash … kind … public`.
    let needle = format!(" {crate_name}-");
    let hash = listing
        .lines()
        .find_map(|line| {
            let start = line.find(&needle)? + needle.len();
            line[start..].split_whitespace().next()
        })
        .unwrap_or_else(|| {
            panic!(
                "{} does not depend on {crate_name}:\n{listing}",
                of.display()
            )
        });

    let path = deps_dir().join(format!("lib{crate_name}-{hash}.rlib"));
    assert!(
        path.exists(),
        "{} named in metadata but missing",
        path.display()
    );
    path
}

/// The `--extern` arguments for a generated program: one consistent set.
pub fn externs() -> Vec<String> {
    let std = newest_rlib("nikaia_std");
    let grammar = dependency_of(&std, "winnow_grammar");
    let winnow = dependency_of(&grammar, "winnow");

    vec![
        format!("nikaia_std={}", std.display()),
        format!("winnow_grammar={}", grammar.display()),
        format!("winnow={}", winnow.display()),
    ]
    .into_iter()
    .flat_map(|e| ["--extern".to_string(), e])
    .collect()
}

/// A scratch directory of this test's own. The tests run in parallel threads
/// of one process, and two of them sharing a file name means one compiles the
/// other's code.
pub fn scratch_dir(purpose: &str) -> PathBuf {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let id = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("nikaia-{purpose}-{}-{id}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("scratch directory");
    dir
}

/// Compile emitted Rust. `args` are rustc's, after the edition and the externs
/// - `--crate-type`, `--emit`, `-O`, `--error-format`, `-o`.
pub fn compile(source: &Path, args: &[&str]) -> Output {
    Command::new(rustc())
        .args(["--edition", "2021"])
        .arg("-L")
        .arg(format!("dependency={}", deps_dir().display()))
        .args(externs())
        .args(args)
        .arg(source)
        .output()
        .expect("run rustc")
}
