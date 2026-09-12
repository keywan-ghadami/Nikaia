//! The sysroot, and the two promises it turns into obligations (ADR-002 D4).
//!
//! `std` ships as sources with its Nikaia half already lowered. That buys a
//! project a build graph with nothing of the compiler in it, and it costs two
//! things that have to be checked rather than hoped for:
//!
//! * the committed `.rs` must be what this compiler lowers the `.nika` to, or a
//!   reviewer is reading a file that does not describe the source beside it;
//! * the lowering happens at **one** setting of the build switches, which is
//!   only sound while nothing in those files is switch-sensitive.

use std::collections::BTreeMap;
use std::path::Path;

use nikaia::emit::{self, Build};
use nikaia::sysroot::{self, Sysroot};

/// What `cargo fmt`'s equivalent is for a generated file: the committed bytes
/// are the compiler's output, or the build is red.
///
/// **This is why the pre-lowered Rust is committed rather than produced by a
/// release step.** A fresh checkout has to build with `cargo test --workspace`
/// and no extra steps, and the only other way for `crates/nikaia-std` to have
/// its `text.rs` at that moment is the build dependency on the compiler this
/// decision removed. Committing it puts a generated file in diffs, which is a
/// real cost; this test is what stops that file from being able to lie.
#[test]
fn the_committed_rust_is_what_this_compiler_lowers() {
    let sysroot = Sysroot::resolve();
    let modules = sysroot.std_modules().expect("std's Nikaia modules");
    assert!(
        !modules.is_empty(),
        "std has a Nikaia half, and this test is about it (ADR-014 D1)"
    );

    for nika in modules {
        let expected = sysroot::lower_std_module(&nika).expect("std's Nikaia half lowers");
        let committed = std::fs::read_to_string(sysroot::lowered_path(&nika))
            .unwrap_or_else(|e| panic!("{} has no committed Rust beside it: {e}", nika.display()));
        assert_eq!(
            committed,
            expected,
            "{} has drifted from what this compiler lowers {} to. \
             Run `cargo run -p nikaia -- lower-std` and commit the result.",
            sysroot::lowered_path(&nika).display(),
            nika.display(),
        );
    }
}

/// The constraint ADR-002 D4 states, enforced.
///
/// `std`'s Nikaia half is lowered once, at `Build::default()`, and linked into
/// programs built at either setting of `user_parallelism`. That is sound only
/// while nothing in those files lowers differently per switch - and `Shared` is
/// exactly what would break it, since ADR-037 D3 makes it `Rc` at `no` and `Arc`
/// at `yes`. The day one appears in a `.nika` file here, this test goes red
/// instead of a program built at `yes` linking an `Rc`.
#[test]
fn stds_nikaia_half_lowers_the_same_at_both_switches() {
    let sysroot = Sysroot::resolve();
    let sequential = Build::parse("x86_64-linux", "no").expect("a switch that exists");
    let concurrent = Build::parse("x86_64-linux", "yes").expect("a switch that exists");

    for nika in sysroot.std_modules().expect("std's Nikaia modules") {
        let source = std::fs::read_to_string(&nika).expect("the source reads");
        let parsed = nikaia::parser::parse_to_ast(&source).expect("std's Nikaia half parses");
        let at_no = emit::emit_program(&parsed, sequential).expect("lowers at `no`");
        let at_yes = emit::emit_program(&parsed, concurrent).expect("lowers at `yes`");
        assert_eq!(
            at_no.rust,
            at_yes.rust,
            "{} lowers differently at the two settings of `user_parallelism`, \
             and it is lowered once for both (ADR-002 D4). Nothing switch-sensitive \
             may live in std's Nikaia half until that decision is revisited.",
            nika.display(),
        );
    }
}

/// The defect measurement made visible, as a structural guard.
///
/// `nikaia-std` had the compiler as a build dependency, so Cargo built the
/// compiler library again inside every project's `target/` while the installed
/// compiler was running - 58 of a hello-world's 103 resolved packages existed
/// only for that. Nothing in this crate's manifest may put it back.
#[test]
fn std_needs_nothing_but_rustc_to_build() {
    let std_dir = Sysroot::resolve().std_dir();
    let manifest = std::fs::read_to_string(std_dir.join("Cargo.toml")).expect("std's manifest");

    assert!(
        !manifest.contains("[build-dependencies]"),
        "std has no build dependencies, which is the whole of ADR-002 D4's \
         package-count claim:\n{manifest}"
    );
    assert!(
        !std_dir.join("build.rs").exists(),
        "std has no build script: the Nikaia half is lowered at release time \
         (`nikaia lower-std`), not when the crate is built"
    );
}

/// The ledger travels with the compiler, not with `std` (ADR-005 D8).
///
/// Ledger stability across toolchain versions is explicitly not required, so a
/// `std` that could be paired with a different compiler would let the ledger
/// describe a compiler that is not there. The compiler answers from the copy
/// `include_str!` baked in, and this is the statement that it is that copy.
#[test]
fn the_ledger_the_compiler_answers_from_is_its_own() {
    let shipped = std::fs::read_to_string(Sysroot::resolve().std_dir().join("std.contracts"))
        .expect("the sysroot carries std's ledger as the source of truth");
    assert_eq!(
        shipped,
        nikaia::contracts::STD,
        "the compiler answers from its own baked-in copy; the file in the sysroot \
         is the source it was built from and may not be able to differ from it"
    );
}

/// D7 for the second store: what separates one compiled `std` from another.
///
/// The codegen table is in because hardware optimisation is the one axis where
/// `std` matters more than the compiler - `std`'s code runs inside every user
/// program, while the compiler's speed only costs build time - so two such builds
/// have to coexist rather than evict each other.
#[test]
fn a_different_codegen_table_is_a_different_compiled_std() {
    let sysroot = Sysroot::resolve();
    let plain = sysroot::Codegen::new(&BTreeMap::new(), "unwind");
    let tuned = sysroot::Codegen::new(
        &BTreeMap::from([("opt-level".to_string(), toml::Value::Integer(3))]),
        "unwind",
    );
    let a = sysroot.rlib_cache("x86_64-linux", &plain);
    let b = sysroot.rlib_cache("x86_64-linux", &tuned);
    assert_ne!(a, b);
    assert_eq!(
        a.parent(),
        b.parent(),
        "both are entries of one cache, distinguished by the key and not by where \
         they were put"
    );
}

/// `NIKAIA_SYSROOT` is the development override, and it names a sysroot rather
/// than a crate directory - which is what `NIKAIA_STD_PATH` got wrong.
#[test]
fn the_override_names_a_sysroot() {
    let sysroot = Sysroot::new(Path::new("/opt/nikaia/lib"));
    assert_eq!(
        sysroot.std_dir(),
        Path::new("/opt/nikaia/lib").join("nikaia-std")
    );
}
