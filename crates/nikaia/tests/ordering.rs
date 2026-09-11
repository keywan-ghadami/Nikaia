//! Order is kept where it can be seen (ADR-033, Part I 8.1.1).
//!
//! A run of statements whose calls reach different resources is lowered to run
//! at the same time. Everything here is either that working, or one of the
//! reasons it must not - and the second list is the longer one on purpose,
//! because the decision is only safe if every "no" is reliable.
//!
//! The shapes are ADR-033 §8.3's first item: a `let`, a **bare expression
//! statement**, and a value built out of literals and calls rather than being
//! one call. §8.1 measured the narrow version and found 101 of 127 refusals
//! falling out on the shape alone, which measured the analysis and not the
//! corpus - so each widening below has a test for the operation it now sees and
//! a test for the refusal it must still make.
//!
//! Three of §6's "not built" list are here too. A **group** of three or more
//! mutually disjoint statements, where the safety argument is that disjointness
//! is not transitive and every pair has to be asked about rather than every
//! adjacent one. **`seq`**, the block that states an order the compiler cannot
//! see (D7) - and the keyword is provisional, as D7 says itself. And a
//! **vocabulary** past `file` and `stdout`, whose own fail-closed question is
//! what the compiler does with a resource named in a word it does not know.
//!
//! The lowering is not taken on trust: the emitted Rust is compiled and run,
//! and the program prints what the sequential one would have printed.

mod common;

use std::path::PathBuf;

use nikaia::emit::{self, Build, Ordering};
use nikaia::parser::parse_to_ast;

/// Overlapping is only reachable with parallelism asked for: at
/// `user_parallelism = no` nothing the user wrote may run concurrently, and
/// the two closures `task::both` takes are code the user wrote (ADR-037 D2).
/// So these tests are about `yes`, and the one below checks the `no` side.
fn lowered(source: &str, ordering: Ordering) -> String {
    let parsed = parse_to_ast(source).expect("the source parses");
    emit::emit_program_ordered(&parsed, Build::parallel(), ordering)
        .expect("the source lowers")
        .rust
}

/// Whether the emitted Rust runs the two calls together.
fn overlaps(source: &str) -> bool {
    lowered(source, Ordering::Effects).contains("task::both")
}

const TWO_READS: &str = "use std::fs\n\
     fn main() throws {\n\
         let a = fs::read_to_string(\"eins.txt\") catch { \"\".to_string() }\n\
         let b = fs::read_to_string(\"zwei.txt\") catch { \"\".to_string() }\n\
         println(f\"{a.len()} {b.len()}\")\n\
     }";

/// Two reads of different files meet on nothing.
#[test]
fn two_reads_of_different_files_overlap() {
    assert!(
        overlaps(TWO_READS),
        "{}",
        lowered(TWO_READS, Ordering::Effects)
    );
}

/// At `user_parallelism = no` there is nothing to overlap with.
///
/// Part I 1.2 promises that nothing **you** wrote ever runs concurrently at
/// `no`, and Part III 15.3 promises a `wasm32-unknown` build with no OS-level
/// mutexes or atomics - a target where `rayon::join` does not even link.
/// `--ordering effects` is a question about the program; whether a vehicle
/// exists to answer it with is a question about the build, and `no` answers
/// that one no. So this degrades exactly as `par_fold` degrades to a
/// sequential `fold` (ADR-009), rather than quietly contradicting the switch
/// in the same file that emits `Parallelism::Off`.
#[test]
fn no_user_parallelism_never_spawns_a_thread() {
    let parsed = parse_to_ast(TWO_READS).expect("the source parses");
    let sequential = emit::emit_program_ordered(&parsed, Build::default(), Ordering::Effects)
        .expect("the source lowers")
        .rust;
    assert!(
        !sequential.contains("task::both"),
        "`user_parallelism = no` overlapped under `--ordering effects`:\n{sequential}"
    );

    // …and it is the sequential program, not merely a different one.
    let strict = emit::emit_program_ordered(&parsed, Build::default(), Ordering::Strict)
        .expect("the source lowers")
        .rust;
    assert_eq!(sequential, strict, "the two orderings differ at `no`");

    // The guard has to be the switch and not the analysis: `yes` still
    // overlaps the same program, or this test would pass for the wrong reason.
    assert!(overlaps(TWO_READS));
}

/// A `catch` handler's own effects are part of what the statement touches.
///
/// The analysis looks *past* a `catch` at the call it guards, so without this
/// the handler below would be invisible: the pair would meet on nothing and
/// overlap, and the overlapped program could print its two lines in either
/// order. A handler is code that runs, so what it reaches counts (D2), and
/// where it cannot be read D4 says the statement reaches everything.
#[test]
fn a_handlers_own_effects_are_part_of_the_statement() {
    const HANDLER_WRITES_STDOUT: &str = "use std::fs\n\
         fn main() {\n\
         \x20   fs::write(\"a.txt\", \"x\") catch { println(\"failed\") }\n\
         \x20   println(\"next\")\n\
         }";

    assert!(
        !overlaps(HANDLER_WRITES_STDOUT),
        "the handler writes stdout and so does the next statement:\n{}",
        report(HANDLER_WRITES_STDOUT)
    );
    let report = report(HANDLER_WRITES_STDOUT);
    assert!(
        report.contains("stdout") && report.contains("println"),
        "and the refusal must name what they meet on: {report}"
    );
}

/// A handler this analysis cannot read makes the statement reach everything.
///
/// `catch { …; … }` is more than one statement, which the walk stops at. The
/// answer then has to be the fail-closed one, or a handler could smuggle an
/// effect past the touch set - which is exactly the hole this pair is here to
/// keep shut (ADR-010 D1's polarity, applied to a third question).
#[test]
fn an_unreadable_handler_is_not_an_empty_one() {
    const HANDLER_DOES_MORE: &str = "use std::fs\n\
         fn main() throws {\n\
         \x20   let a = fs::read_to_string(\"eins.txt\") catch { \
         fs::write(\"zwei.txt\", \"x\") catch { }; \"\".to_string() }\n\
         \x20   let b = fs::read_to_string(\"zwei.txt\") catch { \"\".to_string() }\n\
         \x20   println(f\"{a.len()} {b.len()}\")\n\
         }";

    assert!(
        !overlaps(HANDLER_DOES_MORE),
        "the first handler writes the file the second statement reads:\n{}",
        report(HANDLER_DOES_MORE)
    );
}

/// … and the emitted Rust compiles and prints what the sequential one would.
///
/// The half that cannot be checked by reading the output: a lowering that
/// produces plausible-looking Rust which does not build, or builds and prints
/// something else, is worth nothing at all.
#[test]
fn the_overlapped_program_compiles_and_runs() {
    let dir = common::scratch_dir("ordering");
    let source = dir.join("two_reads.rs");
    std::fs::write(&source, lowered(TWO_READS, Ordering::Effects)).expect("write the Rust");
    std::fs::write(dir.join("eins.txt"), "hallo").expect("write eins");
    std::fs::write(dir.join("zwei.txt"), "welt!!").expect("write zwei");

    let binary = dir.join("two_reads");
    let built = common::compile(&source, &["-o", &binary.to_string_lossy()]);
    assert!(
        built.status.success(),
        "the overlapped lowering does not compile:\n{}",
        String::from_utf8_lossy(&built.stderr)
    );

    let run = std::process::Command::new(&binary)
        .current_dir(&dir)
        .output()
        .expect("run it");
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout).trim(), "5 6");
}

/// `ordering = "strict"` turns it off, and that is the whole of what it does.
///
/// ADR-033 D8: not an aid to be removed later. The same source, both ways, and
/// the strict one is the program this compiler emitted before any of this.
#[test]
fn strict_ordering_leaves_the_program_alone() {
    let strict = lowered(TWO_READS, Ordering::Strict);
    assert!(!strict.contains("std::thread::scope"), "{strict}");
    assert!(strict.contains("let a = match"), "{strict}");
    assert!(strict.contains("let b = match"), "{strict}");
}

// --- the shapes the analysis learned to see (ADR-033 §8.3, first item) -------

/// Two bare expression statements overlap.
///
/// The widening that matters most: `fs::write(…)` binds nothing, and §8.1's 101
/// refusals were mostly statements of exactly this shape falling out before any
/// `touches` set was consulted. Two writes to different files meet on nothing,
/// so neither waits for the other.
const TWO_WRITES: &str = "use std::fs\n\
     fn main() throws {\n\
         fs::write(\"drei.txt\", \"abc\") catch { }\n\
         fs::write(\"vier.txt\", \"defg\") catch { }\n\
         let a = fs::read_to_string(\"drei.txt\") catch { \"\".to_string() }\n\
         let b = fs::read_to_string(\"vier.txt\") catch { \"\".to_string() }\n\
         println(f\"{a.len()} {b.len()}\")\n\
     }";

#[test]
fn two_expression_statements_overlap() {
    assert!(
        overlaps(TWO_WRITES),
        "{}",
        lowered(TWO_WRITES, Ordering::Effects)
    );
}

/// … and a pair that binds nothing is lowered as a statement, not as a binding.
///
/// `let (_, _) = …` would be a pattern that says nothing, and the Rust this
/// emits is read by people.
#[test]
fn a_pair_that_binds_nothing_binds_nothing() {
    let rust = lowered(TWO_WRITES, Ordering::Effects);
    assert!(rust.contains("\n    task::both("), "{rust}");
    assert!(!rust.contains("let (_, _)"), "{rust}");
}

/// … and the program still writes both files and still prints what the
/// sequential one printed.
///
/// The half that cannot be checked by reading the output: an overlap that
/// produces plausible-looking Rust which does not build, or builds and does
/// something else, is worth nothing at all.
#[test]
fn the_overlapped_expression_statements_compile_and_run() {
    let dir = common::scratch_dir("ordering-expression-statements");
    let source = dir.join("two_writes.rs");
    std::fs::write(&source, lowered(TWO_WRITES, Ordering::Effects)).expect("write the Rust");

    let binary = dir.join("two_writes");
    let built = common::compile(&source, &["-o", &binary.to_string_lossy()]);
    assert!(
        built.status.success(),
        "the overlapped lowering does not compile:\n{}",
        String::from_utf8_lossy(&built.stderr)
    );

    let run = std::process::Command::new(&binary)
        .current_dir(&dir)
        .output()
        .expect("run it");
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    // What the sequential program prints, and what it leaves behind.
    assert_eq!(String::from_utf8_lossy(&run.stdout).trim(), "3 4");
    assert_eq!(
        std::fs::read_to_string(dir.join("drei.txt")).expect("drei.txt"),
        "abc"
    );
    assert_eq!(
        std::fs::read_to_string(dir.join("vier.txt")).expect("vier.txt"),
        "defg"
    );
}

/// A `let` whose initialiser is not a bare call is weighed, and its touch set
/// is the union of every call in it.
///
/// The other half of §8.3's first item. Two reads inside one statement reach
/// two files, and the statement next to them is compared against both.
#[test]
fn a_value_that_is_not_a_bare_call_is_weighed() {
    let why = report(
        "use std::fs\n\
         fn main() throws {\n\
             let beide = (fs::read_to_string(\"eins.txt\"), fs::read_to_string(\"zwei.txt\")) catch { (\"\".to_string(), \"\".to_string()) }\n\
             let c = fs::read_to_string(\"drei.txt\") catch { \"\".to_string() }\n\
             println(\"x\")\n\
         }",
    );
    assert!(
        why.contains("fs::read_to_string + fs::read_to_string / fs::read_to_string"),
        "{why}"
    );
    assert!(why.contains("together"), "{why}");

    // … and the union is what is compared: one of the two inner reads meets the
    // write next to it, so the pair keeps its order.
    let why = report(
        "use std::fs\n\
         fn main() throws {\n\
             let beide = (fs::read_to_string(\"eins.txt\"), fs::read_to_string(\"zwei.txt\")) catch { (\"\".to_string(), \"\".to_string()) }\n\
             fs::write(\"zwei.txt\", \"x\") catch { }\n\
             println(\"x\")\n\
         }",
    );
    assert!(why.contains("both reach file `zwei.txt`"), "{why}");
}

// --- more than two at a time (ADR-033 §6) ------------------------------------

/// Three reads of three files are one group, not a pair and a leftover.
///
/// The pair lowering left the third read waiting for two that had nothing to do
/// with it - one thread wake-up more than the work needs, which at ~46 µs each
/// (§8.4) is the whole of what the overlap has to spend.
const THREE_READS: &str = "use std::fs\n\
     fn main() throws {\n\
         let a = fs::read_to_string(\"eins.txt\") catch { \"\".to_string() }\n\
         let b = fs::read_to_string(\"zwei.txt\") catch { \"\".to_string() }\n\
         let c = fs::read_to_string(\"drei.txt\") catch { \"\".to_string() }\n\
         println(f\"{a.len()} {b.len()} {c.len()}\")\n\
     }";

#[test]
fn three_statements_overlap_as_one_group() {
    let rust = lowered(THREE_READS, Ordering::Effects);
    // One group and not a pair with a statement after it: three closures, and
    // the pattern that collects them is nested exactly as the calls are.
    assert_eq!(rust.matches("task::both").count(), 2, "{rust}");
    assert!(rust.contains("let (a, (b, c)) = task::both("), "{rust}");
    // … and nothing is left behind: no third `let a =`-shaped read outside it.
    assert_eq!(rust.matches("fs::read_to_string").count(), 3, "{rust}");
}

/// … and the emitted Rust compiles and prints what the sequential one printed.
#[test]
fn the_overlapped_group_compiles_and_runs() {
    let dir = common::scratch_dir("ordering-group");
    let source = dir.join("three_reads.rs");
    std::fs::write(&source, lowered(THREE_READS, Ordering::Effects)).expect("write the Rust");
    std::fs::write(dir.join("eins.txt"), "hallo").expect("write eins");
    std::fs::write(dir.join("zwei.txt"), "welt!!").expect("write zwei");
    std::fs::write(dir.join("drei.txt"), "abc").expect("write drei");

    let binary = dir.join("three_reads");
    let built = common::compile(&source, &["-o", &binary.to_string_lossy()]);
    assert!(
        built.status.success(),
        "the grouped lowering does not compile:\n{}",
        String::from_utf8_lossy(&built.stderr)
    );

    let run = std::process::Command::new(&binary)
        .current_dir(&dir)
        .output()
        .expect("run it");
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout).trim(), "5 6 3");
}

/// A group is every pair, not every **adjacent** pair - and this is the one
/// place where the difference is a soundness hole rather than a missed chance.
///
/// `read eins / read zwei / write eins`: both adjacent pairs meet on nothing,
/// and the run does not. In a group all three run at once, so a chain of
/// adjacent answers would have overlapped a read of `eins.txt` with the write
/// of it and the program would print either the old contents or the new,
/// depending on a race. The group is therefore the first two, and the write
/// stands where it was written.
#[test]
fn a_group_is_checked_pairwise_and_not_by_its_neighbours() {
    const READ_READ_WRITE: &str = "use std::fs\n\
         fn main() throws {\n\
             let a = fs::read_to_string(\"eins.txt\") catch { \"\".to_string() }\n\
             let b = fs::read_to_string(\"zwei.txt\") catch { \"\".to_string() }\n\
             fs::write(\"eins.txt\", \"x\") catch { }\n\
             println(f\"{a.len()} {b.len()}\")\n\
         }";

    let rust = lowered(READ_READ_WRITE, Ordering::Effects);
    assert_eq!(rust.matches("task::both").count(), 1, "{rust}");
    assert!(rust.contains("let (a, b) = task::both("), "{rust}");
    // The write is not in the group, so it is a statement of its own after it.
    let group_ends = rust.find(");").expect("the group closes");
    let write_at = rust.find("fs::write").expect("the write is emitted");
    assert!(write_at > group_ends, "{rust}");
}

/// A group takes what binds and what does not, and the pattern says which.
///
/// A bare expression statement binds nothing (§8.3's first item), so its place
/// in the pattern is `_`. Three of them and there would be no pattern at all.
#[test]
fn a_group_may_mix_bindings_with_statements() {
    let rust = lowered(
        "use std::fs\n\
         fn main() throws {\n\
             let a = fs::read_to_string(\"eins.txt\") catch { \"\".to_string() }\n\
             fs::write(\"zwei.txt\", \"x\") catch { }\n\
             let c = fs::read_to_string(\"drei.txt\") catch { \"\".to_string() }\n\
             println(f\"{a.len()} {c.len()}\")\n\
         }",
        Ordering::Effects,
    );
    assert!(rust.contains("let (a, (_, c)) = task::both("), "{rust}");
}

/// A group of four, mixed, compiled and run.
///
/// The nesting is where a lowering like this breaks: a `_` two levels into a
/// pattern, four closures, and a tuple shape that has to match the calls
/// exactly. Reading the emitted Rust is not evidence that Rust will take it.
#[test]
fn a_group_of_four_compiles_and_runs() {
    const FOUR: &str = "use std::fs\n\
         fn main() throws {\n\
             fs::write(\"eins.txt\", \"a\") catch { }\n\
             let b = fs::read_to_string(\"zwei.txt\") catch { \"leer\".to_string() }\n\
             fs::write(\"drei.txt\", \"ccc\") catch { }\n\
             let d = fs::read_to_string(\"vier.txt\") catch { \"leer\".to_string() }\n\
             println(f\"{b} {d}\")\n\
         }";

    let rust = lowered(FOUR, Ordering::Effects);
    assert_eq!(rust.matches("task::both").count(), 3, "{rust}");
    assert!(
        rust.contains("let (_, (b, (_, d))) = task::both("),
        "{rust}"
    );

    let dir = common::scratch_dir("ordering-group-four");
    let source = dir.join("four.rs");
    std::fs::write(&source, &rust).expect("write the Rust");
    std::fs::write(dir.join("zwei.txt"), "zwei").expect("write zwei");
    std::fs::write(dir.join("vier.txt"), "vier").expect("write vier");

    let binary = dir.join("four");
    let built = common::compile(&source, &["-o", &binary.to_string_lossy()]);
    assert!(
        built.status.success(),
        "the grouped lowering does not compile:\n{}",
        String::from_utf8_lossy(&built.stderr)
    );

    let run = std::process::Command::new(&binary)
        .current_dir(&dir)
        .output()
        .expect("run it");
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout).trim(), "zwei vier");
    assert_eq!(
        std::fs::read_to_string(dir.join("eins.txt")).expect("eins.txt"),
        "a"
    );
    assert_eq!(
        std::fs::read_to_string(dir.join("drei.txt")).expect("drei.txt"),
        "ccc"
    );
}

/// A group stops before the statement that is the block's value.
///
/// Kap 3.1: a block's last statement is what it hands back, and a group hands
/// back a tuple. Three accountable operations where the third is the value are
/// a group of two, not a group of three that changes what the function means.
#[test]
fn a_group_stops_before_the_value_of_a_function() {
    let source = "use std::fs\n\
         fn drei() -> String throws {\n\
             fs::write(\"eins.txt\", \"1\") catch { }\n\
             fs::write(\"zwei.txt\", \"2\") catch { }\n\
             fs::read_to_string(\"drei.txt\") catch { \"\".to_string() }\n\
         }";
    let rust = lowered(source, Ordering::Effects);
    assert_eq!(rust.matches("task::both").count(), 1, "{rust}");
    // The read is the value, so it is emitted on its own and without a `;`.
    let group_ends = rust.find(");").expect("the group closes");
    assert!(
        rust.find("fs::read_to_string")
            .expect("the read is emitted")
            > group_ends,
        "{rust}"
    );
}

/// `--ordering strict` still turns the whole thing off, groups included.
#[test]
fn strict_ordering_leaves_a_group_alone() {
    let strict = lowered(THREE_READS, Ordering::Strict);
    assert!(!strict.contains("task::both"), "{strict}");
    for name in ["let a = match", "let b = match", "let c = match"] {
        assert!(strict.contains(name), "{strict}");
    }
}

// --- `seq`: an order the compiler cannot see (ADR-033 D7) --------------------
//
// The keyword is **provisional**, and D7 says so itself: it has to read as "in
// this order, whatever you think", and `seq` is a placeholder for a word chosen
// later. What is decided is the construct and its meaning, which is what these
// pin.

/// Two writes to different files overlap - unless they are inside a `seq`.
///
/// Both halves are the test. Without the block they are a pair, so the `seq`
/// case cannot pass because the analysis failed to see the statements at all.
#[test]
fn seq_keeps_the_order_its_statements_were_written_in() {
    const TWO_WRITES_IN_SEQ: &str = "use std::fs\n\
         fn main() throws {\n\
             seq {\n\
                 fs::write(\"eins.txt\", \"a\") catch { }\n\
                 fs::write(\"zwei.txt\", \"b\") catch { }\n\
             }\n\
             println(\"fertig\")\n\
         }";

    assert!(
        !overlaps(TWO_WRITES_IN_SEQ),
        "{}",
        lowered(TWO_WRITES_IN_SEQ, Ordering::Effects)
    );
    assert!(overlaps(
        "use std::fs\n\
         fn main() throws {\n\
             fs::write(\"eins.txt\", \"a\") catch { }\n\
             fs::write(\"zwei.txt\", \"b\") catch { }\n\
             println(\"fertig\")\n\
         }"
    ));
}

/// … and it reaches inward: a block written inside a `seq` is written inside it.
///
/// The rule is about a *sequence*, and a nested block is a sequence in the same
/// one. A `seq` that stopped at the first brace would be an escape that leaks.
#[test]
fn seq_reaches_into_the_blocks_inside_it() {
    assert!(!overlaps(
        "use std::fs\n\
         fn main() throws {\n\
             seq {\n\
                 println(\"erst\")\n\
                 {\n\
                     fs::write(\"eins.txt\", \"a\") catch { }\n\
                     fs::write(\"zwei.txt\", \"b\") catch { }\n\
                 }\n\
             }\n\
         }"
    ));
}

/// A `seq` block does not overlap with the statement beside it either.
///
/// D7 decides the order **inside** the block and says nothing about the block
/// itself, so the block keeps its place - the fail-closed answer, and the only
/// one the record decides. A reader who wrote `seq` said the compiler cannot
/// see what the order is for; moving the block would be answering the question
/// they just said could not be answered.
#[test]
fn a_seq_block_keeps_its_own_place() {
    let why = report(
        "use std::fs\n\
         fn main() throws {\n\
             seq {\n\
                 fs::write(\"eins.txt\", \"a\") catch { }\n\
             }\n\
             fs::write(\"zwei.txt\", \"b\") catch { }\n\
             println(\"fertig\")\n\
         }",
    );
    let about_the_block = why
        .lines()
        .find(|line| line.contains("`seq` block"))
        .unwrap_or_else(|| panic!("the report says nothing about the block: {why}"));
    assert!(about_the_block.contains("in order"), "{why}");
}

/// … and the program inside a `seq` compiles and prints what it says.
///
/// `seq` asks for *less* than the compiler would otherwise do, so the risk is
/// not a race but a lowering that emits a block Rust will not take.
#[test]
fn the_sequential_block_compiles_and_runs() {
    const SEQ_PROGRAM: &str = "use std::fs\n\
         fn main() throws {\n\
             seq {\n\
                 fs::write(\"eins.txt\", \"abc\") catch { }\n\
                 fs::write(\"zwei.txt\", \"defg\") catch { }\n\
             }\n\
             let a = fs::read_to_string(\"eins.txt\") catch { \"\".to_string() }\n\
             let b = fs::read_to_string(\"zwei.txt\") catch { \"\".to_string() }\n\
             println(f\"{a.len()} {b.len()}\")\n\
         }";

    let dir = common::scratch_dir("ordering-seq");
    let source = dir.join("seq.rs");
    std::fs::write(&source, lowered(SEQ_PROGRAM, Ordering::Effects)).expect("write the Rust");

    let binary = dir.join("seq");
    let built = common::compile(&source, &["-o", &binary.to_string_lossy()]);
    assert!(
        built.status.success(),
        "the `seq` lowering does not compile:\n{}",
        String::from_utf8_lossy(&built.stderr)
    );

    let run = std::process::Command::new(&binary)
        .current_dir(&dir)
        .output()
        .expect("run it");
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout).trim(), "3 4");
}

/// `seq` is a keyword in expression position, so it may be a value too.
///
/// Blocks are expressions (Kap 3.1) and this is a block; refusing it in a
/// `let` would be a second rule about where a block may stand.
#[test]
fn seq_is_an_expression_like_any_other_block() {
    let rust = lowered(
        "fn main() {\n\
             let n = seq { 1 }\n\
             println(f\"{n}\")\n\
         }",
        Ordering::Effects,
    );
    assert!(rust.contains("let n ="), "{rust}");
}

// --- the vocabulary (ADR-033 D2) ---------------------------------------------

/// The arguments the program was started with are a resource like any other.
///
/// `cli::args` was the most-refused callee in `examples/` - ten adjacent pairs
/// named it - and it was refused because nobody had written down what it
/// reaches. Two reads of it meet on nothing, and a read of it meets nothing a
/// `println` writes either.
#[test]
fn the_programs_arguments_are_a_resource() {
    assert!(overlaps(
        "use std::cli\n\
         fn main() {\n\
             let a = cli::args()\n\
             let b = cli::args()\n\
             println(f\"{a.len()} {b.len()}\")\n\
         }"
    ));

    // … and `args` is a kind of its own, so reading it does not meet what
    // `println` writes.
    let why = report(
        "use std::cli\n\
         fn main() {\n\
             let a = cli::args()\n\
             println(\"x\")\n\
             println(f\"{a.len()}\")\n\
         }",
    );
    assert!(why.contains("together"), "{why}");
}

/// The two console handles keep their order, because `2>&1` makes them one.
///
/// What widening the groups turned up, and it is the same shape as the `catch`
/// handler whose effects nobody counted (§8.3): an effect that *was* in the
/// touch set, against a resource whose identity nobody had checked. `stdout`
/// and `stderr` are two handles and one destination as soon as anybody
/// redirects one onto the other, so three console writes would have been a
/// group of three whose output interleaves differently on every run.
#[test]
fn the_two_console_handles_keep_their_order() {
    assert!(!overlaps(
        "fn main() {\n\
             println(\"out\")\n\
             eprintln(\"err\")\n\
             println(\"out2\")\n\
         }"
    ));

    // … and the refusal is about the resource rather than about ignorance:
    // both are described, and what they meet on is what is printed.
    let why = report(
        "fn main() {\n\
             println(\"out\")\n\
             eprintln(\"err\")\n\
             println(\"out2\")\n\
         }",
    );
    assert!(
        why.contains("may be the same destination") && why.contains("stderr"),
        "{why}"
    );
    // … and the way out is named, because a refusal a reader can act on is
    // worth several they cannot (D9).
    assert!(why.contains("`seq`"), "{why}");
}

/// A resource named in a word this compiler does not know reaches everything.
///
/// The fail-open shape this closes is the worst one an analysis like this can
/// have. Two touches of *different* kinds never conflict, so a kind nobody
/// knows is disjoint from every kind there is - which would make a typo in a
/// hand-maintained ledger *buy* an overlap. D4's polarity says what to do
/// instead, and it is the same answer it gives everywhere else.
#[test]
fn a_resource_this_compiler_cannot_name_reaches_everything() {
    const LEDGER: &str = "version = 2\n\
         toolchain = \"nikaia 0.1.0\"\n\
         inference = \"stage0-signatures\"\n\
         \n\
         [fn.\"net::post\"]\n\
         pub = true\n\
         sync = true\n\
         touches = [\"endpoint(url) write\"]\n\
         signature = \"(url: ?) -> ?\"\n";

    let library = nikaia::contracts::Ledger::parse(LEDGER).expect("the ledger parses");
    let source = "fn main() {\n\
             net::post(\"https://eins\")\n\
             net::post(\"https://zwei\")\n\
             println(\"x\")\n\
         }";
    let parsed = parse_to_ast(source).expect("the source parses");
    let own = nikaia::contracts::Ledger::infer(&parsed);
    let why = nikaia::contracts::order::report(&parsed, &own, &library);

    assert!(why.contains("does not know about"), "{why}");
    assert!(why.contains("endpoint"), "{why}");
    assert!(!why.contains("together"), "{why}");
}

/// `std`'s hand-maintained ledger names only resources this compiler knows.
///
/// The check the `kind_is_known` rule cannot make on its own: an unknown kind
/// costs a program its overlap silently and correctly, which means a typo in
/// `std.contracts` would be a pessimisation nobody notices. The file is
/// reviewed like code (ADR-020 D5), and this is the part of that review a
/// reviewer cannot do by eye.
#[test]
fn the_std_ledger_names_only_known_resources() {
    let library = nikaia::contracts::Ledger::parse(nikaia::contracts::STD).expect("std's ledger");
    for (name, contract) in &library.functions {
        for touch in &contract.touches {
            assert!(
                touch.kind_is_known(),
                "`{name}` reaches `{}`, which is not in the vocabulary",
                touch.kind
            );
        }
    }
}

// --- and every reason two statements must keep their order --------------------

/// A data dependency: the second uses what the first bound.
#[test]
fn a_data_dependency_keeps_the_order() {
    assert!(!overlaps(
        "use std::fs\n\
         fn main() throws {\n\
             let a = fs::read_to_string(\"eins.txt\") catch { \"\".to_string() }\n\
             let b = fs::read_to_string(a) catch { \"\".to_string() }\n\
             println(f\"{b.len()}\")\n\
         }"
    ));
}

/// The same file, written by one of them.
#[test]
fn a_write_to_the_same_file_keeps_the_order() {
    assert!(!overlaps(
        "use std::fs\n\
         fn main() throws {\n\
             let a = fs::write(\"log.txt\", \"x\") catch { }\n\
             let b = fs::read_to_string(\"log.txt\") catch { \"\".to_string() }\n\
             println(f\"{b.len()}\")\n\
         }"
    ));
}

/// A file this compiler cannot name is every file of its kind (ADR-033 D4).
///
/// `fs::read_to_string(pfad)` where `pfad` is computed could be the file the
/// other one writes. The alternative to keeping the order is a program that is
/// right on some inputs and wrong on others.
#[test]
fn a_file_that_cannot_be_named_keeps_the_order() {
    assert!(!overlaps(
        "use std::fs\n\
         fn main(pfad: &str) throws {\n\
             let a = fs::write(\"log.txt\", \"x\") catch { }\n\
             let b = fs::read_to_string(pfad) catch { \"\".to_string() }\n\
             println(f\"{b.len()}\")\n\
         }"
    ));
}

/// A function nobody described touches everything.
///
/// `etwas_unbekanntes` has no entry in any ledger, so it orders against
/// everything (D4) - and the refusal is about a contract somebody could write
/// rather than about this compiler's own limits.
#[test]
fn a_function_with_no_contract_keeps_the_order() {
    assert!(!overlaps(
        "use std::fs\n\
         fn main() throws {\n\
             let a = fs::read_to_string(\"eins.txt\") catch { \"\".to_string() }\n\
             let b = etwas_unbekanntes() catch { \"\".to_string() }\n\
             println(f\"{a.len()}\")\n\
         }"
    ));
}

/// A handler that can leave the function makes the next statement conditional.
///
/// This is what building the increment found, and it was not in ADR-033 D5 when
/// it was written (ADR-034). If the first read fails and its handler `return`s,
/// the sequential program never performs the second read at all - so performing
/// it early is speculation, which D5 forbids.
#[test]
fn a_diverting_handler_keeps_the_order() {
    assert!(!overlaps(
        "use std::fs\n\
         fn main() throws {\n\
             let a = fs::read_to_string(\"eins.txt\") catch { return }\n\
             let b = fs::read_to_string(\"zwei.txt\") catch { \"\".to_string() }\n\
             println(f\"{a.len()} {b.len()}\")\n\
         }"
    ));
}

/// A data dependency inside a `catch` handler keeps the order too.
///
/// `f("a") catch { … }` is one expression, so a name its handler mentions is a
/// name the statement mentions. Looking only past the `catch` missed it, and
/// the lowering puts the handler in the same closure - so the second statement
/// would have read what the first had not finished binding.
#[test]
fn a_dependency_in_a_handler_keeps_the_order() {
    assert!(!overlaps(
        "use std::fs\n\
         fn main() throws {\n\
             let a = fs::read_to_string(\"eins.txt\") catch { \"\".to_string() }\n\
             let b = fs::read_to_string(\"zwei.txt\") catch { a }\n\
             println(f\"{a.len()} {b.len()}\")\n\
         }"
    ));

    // … including where the name is inside the handler's interpolated text,
    // whose holes this analysis has not parsed. Every word of the raw text
    // counts, which is over-approximate on purpose.
    assert!(!overlaps(
        "use std::fs\n\
         fn main() throws {\n\
             let a = fs::read_to_string(\"eins.txt\") catch { \"\".to_string() }\n\
             let b = fs::read_to_string(\"zwei.txt\") catch { f\"{a}\" }\n\
             println(f\"{a.len()} {b.len()}\")\n\
         }"
    ));
}

/// An argument that is not a literal is not sent to another thread.
///
/// The first increment's own restriction rather than the rule's: a closure that
/// captures nothing cannot capture something that must not cross a thread, and
/// what may cross one deserves its own decision.
#[test]
fn a_non_literal_argument_keeps_the_order() {
    assert!(!overlaps(
        "use std::fs\n\
         fn main(eins: &str, zwei: &str) throws {\n\
             let a = fs::read_to_string(eins) catch { \"\".to_string() }\n\
             let b = fs::read_to_string(zwei) catch { \"\".to_string() }\n\
             println(f\"{a.len()} {b.len()}\")\n\
         }"
    ));
}

/// Two `println`s keep their order, which is D6's own test of the model.
///
/// ADR-033 D6 asks for the obvious without an exception for it, and this is
/// where a wider analysis could have lost it: a bare `println(x)` is now a
/// shape the analysis reads, so the answer has to come from somewhere. It comes
/// from the place D6 says it should - `println` is entered as reaching `stdout`
/// and writing it, so two of them meet, and there is no special case for
/// printing anywhere in this compiler.
#[test]
fn two_printlns_keep_their_order() {
    assert!(!overlaps(
        "fn main() {\n\
             println(\"a\")\n\
             println(\"b\")\n\
         }"
    ));
}

/// A failure nobody catches keeps the order (ADR-034 D2).
///
/// The same rule as a diverting handler, reached from the other side: if the
/// first write fails, the failure leaves the function and the sequential
/// program never performs the second. Starting it early would perform work the
/// program as written might never have performed - which is exactly what D5
/// forbids, and what a bare expression statement makes easy to write.
#[test]
fn an_uncaught_failure_keeps_the_order() {
    assert!(!overlaps(
        "use std::fs\n\
         fn main() throws {\n\
             fs::write(\"eins.txt\", \"a\")\n\
             fs::write(\"zwei.txt\", \"b\")\n\
             println(\"x\")\n\
         }"
    ));

    // … and the same statement with the failure caught does overlap, so the
    // test cannot pass because the shape was not read at all.
    assert!(overlaps(
        "use std::fs\n\
         fn main() throws {\n\
             fs::write(\"eins.txt\", \"a\") catch { }\n\
             fs::write(\"zwei.txt\", \"b\") catch { }\n\
             println(\"x\")\n\
         }"
    ));
}

/// An assignment is never an operation, however plain it looks.
///
/// Not an implementation gap but a rule: `x = 1` changes a name without binding
/// one, so the data-dependency test - does the later statement mention what the
/// earlier one bound - would find nothing to compare. A shape whose
/// dependencies the analysis cannot see is a shape it may not read.
#[test]
fn an_assignment_is_not_an_operation() {
    assert!(!overlaps(
        "use std::fs\n\
         fn main() throws {\n\
             let mut a = \"\".to_string()\n\
             a = fs::read_to_string(\"eins.txt\") catch { \"\".to_string() }\n\
             let b = fs::read_to_string(\"zwei.txt\") catch { \"\".to_string() }\n\
             println(f\"{a.len()} {b.len()}\")\n\
         }"
    ));
}

/// A method call keeps the order: which ledger entry it is depends on the type
/// of its receiver, and that is the type checker's answer (ADR-028).
#[test]
fn a_method_call_keeps_the_order() {
    assert!(!overlaps(
        "use std::fs\n\
         fn main() throws {\n\
             let mut out = Vec::new()\n\
             out.push(\"eins\")\n\
             let b = fs::read_to_string(\"zwei.txt\") catch { \"\".to_string() }\n\
             println(f\"{b.len()}\")\n\
         }"
    ));
}

/// A value-returning function overlaps the same way - its tail is the
/// expression, and the two `let`s before it are an ordinary pair.
///
/// The guard against pairing the tail itself is defensive rather than
/// observable: a block's last statement being a `let` means the block hands
/// back nothing, so a value-returning body cannot end in one. It is in the code
/// because a rule that holds by accident somewhere else is a rule that breaks
/// when the accident does.
#[test]
fn a_value_returning_body_overlaps_before_its_tail() {
    assert!(overlaps(
        "use std::fs\n\
         fn beides() -> i64 throws {\n\
             let a = fs::read_to_string(\"eins.txt\") catch { \"\".to_string() }\n\
             let b = fs::read_to_string(\"zwei.txt\") catch { \"\".to_string() }\n\
             a.len() + b.len()\n\
         }"
    ));
}

/// Every `.nika` in the repository lowers the same under both orderings, or
/// differs only by an overlap.
///
/// The corpus guard, in the form this decision needs: turning the analysis on
/// must not change a program into one that does not compile, and the cheapest
/// check of that is that the two lowerings agree wherever no pair was found.
#[test]
fn the_corpus_lowers_under_both_orderings() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut seen = 0;
    let mut overlapped = Vec::new();

    for dir in ["examples", "benches"] {
        let Ok(entries) = std::fs::read_dir(root.join(dir)) else {
            continue;
        };
        for entry in entries {
            let path = entry.expect("dir entry").path();
            if path.extension().and_then(|e| e.to_str()) != Some("nika") {
                continue;
            }
            let source = std::fs::read_to_string(&path).expect("read it");
            let Ok(parsed) = parse_to_ast(&source) else {
                continue;
            };
            let one = emit::emit_program_ordered(&parsed, Build::parallel(), Ordering::Effects);
            let other = emit::emit_program_ordered(&parsed, Build::parallel(), Ordering::Strict);
            match (one, other) {
                (Ok(one), Ok(other)) => {
                    seen += 1;
                    if one.rust != other.rust {
                        overlapped.push(path.file_name().unwrap().to_string_lossy().to_string());
                    }
                }
                // A file the bootstrap compiler cannot lower at all fails the
                // same way under both, which is not this test's business.
                (one, other) => assert_eq!(one.is_err(), other.is_err(), "{}", path.display()),
            }
        }
    }

    assert!(seen > 0, "no example was lowered");
    // Today: still none of them, and `--overlaps` now says why in terms of the
    // programs rather than of the analysis. The same 127 pairs are weighed; the
    // 101 that used to fall out on "not a `let` of a single call" are gone, and
    // what stands in their place is 30 method calls (whose ledger entry needs
    // the type checker), 29 statements that perform no operation at all, and 28
    // callees nobody has described - D4's fail-closed polarity, which doubled
    // from 14 once the shapes stopped hiding it (ADR-033 §8.1).
    //
    // So the zero is no longer a measurement of the analysis. It is a corpus of
    // microsecond work written in method calls, and §8.4 says the overlap could
    // not pay for it even if every one of them were accounted for.
    assert!(
        overlapped.is_empty(),
        "these examples now lower differently: {overlapped:?}"
    );
}

// --- why a pair did not overlap (ADR-033 D9) ---------------------------------

/// The report answers "may these two overlap", which is a question about the
/// program alone - whether a vehicle then exists to overlap them with is a
/// question about the build, and the CLI says so separately (ADR-033 §8.2b).
/// So no build setting reaches this.
fn report(source: &str) -> String {
    let parsed = parse_to_ast(source).expect("the source parses");
    let library = nikaia::contracts::Ledger::parse(nikaia::contracts::STD).expect("std's ledger");
    let own = nikaia::contracts::Ledger::infer(&parsed);
    nikaia::contracts::order::report(&parsed, &own, &library)
}

/// The refusal that the language decided **not** to give a keyword names its
/// own way out.
///
/// ADR-033 D9: Nikaia has no `allow_parallel`, on the argument that the one
/// case it would serve has a clearer spelling already. That argument only holds
/// if the compiler says so - a silent refusal with no way to ask would be
/// exactly the trap the keyword was supposed to be an escape from.
#[test]
fn a_diverting_handler_is_told_what_to_write_instead() {
    let why = report(
        "use std::fs\n\
         fn main() throws {\n\
             let a = fs::read_to_string(\"eins.txt\") catch { return }\n\
             let b = fs::read_to_string(\"zwei.txt\") catch { \"\".to_string() }\n\
             println(\"x\")\n\
         }",
    );
    assert!(why.contains("can leave the function"), "{why}");
    assert!(why.contains("hands back a value instead"), "{why}");
}

/// The report does not promise an overlap the emitter will not make.
///
/// A value-returning body ends in its value (Kap 3.1), and a pair hands back a
/// tuple - so the last statement is never half of one. Reading only `let`s hid
/// this, because a `let` cannot be a block's value; a bare expression statement
/// can, and a report that said "together" where nothing overlaps would be worse
/// than no report.
#[test]
fn the_value_of_a_function_is_never_half_a_pair() {
    let source = "use std::fs\n\
         fn zwei() -> String throws {\n\
             fs::write(\"eins.txt\", \"1\") catch { }\n\
             fs::read_to_string(\"zwei.txt\") catch { \"\".to_string() }\n\
         }";
    let why = report(source);
    assert!(why.contains("what this function hands back"), "{why}");
    assert!(!why.contains("together"), "{why}");
    assert!(!overlaps(source), "{}", lowered(source, Ordering::Effects));
}

/// A resource collision names the resource.
#[test]
fn a_collision_names_the_file_it_is_about() {
    let why = report(
        "use std::fs\n\
         fn main() throws {\n\
             let a = fs::write(\"log.txt\", \"x\") catch { }\n\
             let b = fs::read_to_string(\"log.txt\") catch { \"\".to_string() }\n\
             println(\"x\")\n\
         }",
    );
    assert!(why.contains("both reach file `log.txt`"), "{why}");
    assert!(why.contains("one writes it"), "{why}");
}

/// A function nobody described says that, rather than "not accounted for".
#[test]
fn an_undescribed_call_is_named() {
    let why = report(
        "fn main() throws {\n\
             let a = unbekannt() catch { \"\".to_string() }\n\
             let b = auch_unbekannt() catch { \"\".to_string() }\n\
             println(\"x\")\n\
         }",
    );
    assert!(
        why.contains("nothing says what `unbekannt` reaches"),
        "{why}"
    );
}

/// A pair that does overlap says so, so the report is readable as a whole and
/// not only as a list of complaints.
#[test]
fn a_pair_that_overlaps_is_reported_too() {
    let why = report(TWO_READS);
    assert!(why.contains("together"), "{why}");
    assert!(why.contains("they meet on nothing"), "{why}");
}

/// The rewrite the refusal recommends actually works.
///
/// The whole argument against `allow_parallel` is that `catch { return }`
/// conflates two things - what to do about the failure, and whether to go on -
/// and that separating them is clearer *and* overlaps. If the rewrite did not
/// overlap, the advice would be wrong and the keyword would be needed.
#[test]
fn the_recommended_rewrite_overlaps() {
    assert!(overlaps(
        "use std::fs\n\
         fn main() throws {\n\
             let a = fs::read_to_string(\"eins.txt\") catch { \"\".to_string() }\n\
             let b = fs::read_to_string(\"zwei.txt\") catch { \"\".to_string() }\n\
             if a.is_empty() { return }\n\
             println(\"{a.len()} {b.len()}\")\n\
         }"
    ));
}
