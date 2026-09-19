//! What the C boundary lends: a view, and no raw pointer
//! ([ADR-147](../../../docs/specification/adr/adr-147.md) D1).
//!
//! `extern "C" { fn malloc(size: usize) -> Pointer[u8] }` was refused with
//! `NK1135`, and [ADR-124](../../../docs/specification/adr/adr-124.md) §4 left
//! it that way on purpose — *a pointer that outlives what it points at wants a
//! record with a lifetime story, not a name.* What that cost was most of C:
//! every function whose signature has a pointer in it was unwritable, so
//! `getpid` compiled and nothing with a buffer did.
//!
//! **The thing to prevent is the dangling dereference**, and D1's answer is
//! that a buffer is a **view** and lives for the call — which is what a view is
//! everywhere else in this language
//! ([ADR-094](../../../docs/specification/adr/adr-094.md)). Nothing is stored
//! and nothing escapes, so the shape that dangles cannot be written.

mod common;

use nikaia::contracts::ty::Ty;
use nikaia::contracts::{Ledger, STD};
use nikaia::emit::{emit_program, Build};
use nikaia::parser::parse_to_ast;

fn findings(source: &str) -> Vec<nikaia::check::Finding> {
    let parsed = parse_to_ast(source).expect("the source parses");
    let own = Ledger::infer(&parsed);
    let library = Ledger::parse(STD).expect("std ships a ledger");
    nikaia::check::check_program(&parsed, &own, &library, &std::collections::BTreeSet::new())
        .findings
}

fn lowered(source: &str) -> String {
    let parsed = parse_to_ast(source).expect("the source parses");
    emit_program(&parsed, Build::default())
        .expect("the source lowers")
        .rust
}

/// **The four forms lower to the pointer C wants** (D1).
///
/// Rust's own `&[T]` is a **fat** pointer, so a declaration that wrote it would
/// be a signature the two languages disagree about — and what a reader would
/// get for it is `rustc`'s `improper_ctypes` about a file nobody wrote
/// ([Part III C.1](../../../docs/specification/30-nikaia-tooling.md)).
#[test]
fn a_view_in_a_declaration_is_the_pointer() {
    let rust = lowered(
        "extern \"C\" {\n\
         \x20   fn strlen(s: &[u8]) -> usize\n\
         \x20   fn read(fd: i32, buf: &mut [u8], count: usize) -> i64\n\
         \x20   fn takes_one(n: &i32) -> i32\n\
         \x20   fn fills_one(n: &mut i32) -> i32\n\
         }\n\
         \n\
         fn main() { println(\"x\") }\n",
    );
    for expected in [
        "fn strlen(s: *const u8) -> usize;",
        "fn read(fd: i32, buf: *mut u8, count: usize) -> i64;",
        "fn takes_one(n: *const i32) -> i32;",
        "fn fills_one(n: *mut i32) -> i32;",
    ] {
        assert!(rust.contains(expected), "{expected}\n--- got ---\n{rust}");
    }
}

/// **And the call is where the address is made.** A declaration says `&[u8]`
/// and C takes the first element's address, so `bytes` becomes
/// `bytes.as_ptr()` — one call that a `Vec`, an `Array` and text all answer.
#[test]
fn a_call_hands_over_the_address() {
    let rust = lowered(
        "extern \"C\" {\n\
         \x20   fn strlen(s: &[u8]) -> usize\n\
         }\n\
         \n\
         fn main() {\n\
         \x20   let text = \"hello\"\n\
         \x20   let n = unsafe { strlen(text) }\n\
         \x20   println(f\"{n}\")\n\
         }\n",
    );
    assert!(rust.contains("strlen(text.as_ptr())"), "{rust}");
}

/// **An ordinary value is untouched**: an `i32` is an `i32` at both ends, and
/// nothing is written around it.
#[test]
fn a_value_parameter_is_passed_as_it_was_written() {
    let rust = lowered(
        "extern \"C\" {\n\
         \x20   fn abs(n: i32) -> i32\n\
         }\n\
         \n\
         fn main() {\n\
         \x20   let a = unsafe { abs(-3) }\n\
         \x20   println(f\"{a}\")\n\
         }\n",
    );
    assert!(rust.contains("abs(-3)"), "{rust}");
}

/// **A `&[u8]` takes what lends a run of bytes** (D1), and the fit is where the
/// declaration and the caller meet: a list, a fixed-size array and text all
/// hand over the same thing.
#[test]
fn what_a_caller_may_hand_a_run() {
    let run = Ty::parse("&[u8]");
    for written in ["Vec[u8]", "Array[u8, 3]", "String", "&str", "?"] {
        assert!(
            Ty::parse(written).fits(&run),
            "`{written}` lends a run of bytes"
        );
    }
    for written in ["i64", "Vec[i64]", "bool"] {
        assert!(!Ty::parse(written).fits(&run), "`{written}` does not");
    }
}

/// **The three shapes read back as they were written**, which is what lets a
/// declaration ship in the ledger (Part III 13.5).
#[test]
fn the_boundary_types_round_trip_through_their_text() {
    for written in ["&[u8]", "&mut [u8]", "&mut i32"] {
        let ty = Ty::parse(written);
        assert_eq!(ty.text(), written);
        assert_eq!(Ty::parse(&ty.text()), ty);
        assert!(matches!(ty, Ty::Pointed { .. }), "{ty:?}");
    }
    // A plain `&T` is the view every declaration in this language writes, and
    // it stays what it was: one type, one spelling.
    assert!(matches!(Ty::parse("&str"), Ty::Named { .. }));
}

/// **A `&mut [u8]` is not a `&[u8]`**, because the second promises not to
/// write; and neither is a `&u8`, because one is a run and the other is one
/// element.
#[test]
fn the_shapes_do_not_fit_each_other() {
    assert!(!Ty::parse("&mut [u8]").fits(&Ty::parse("&[u8]")));
    assert!(!Ty::parse("&[u8]").fits(&Ty::parse("&mut [u8]")));
    assert!(!Ty::parse("&mut i32").fits(&Ty::parse("&mut [i32]")));
    assert!(Ty::parse("&mut [u8]").fits(&Ty::parse("&mut [u8]")));
}

/// **Both forms are the C boundary's and nowhere else's** (`NK1158`).
///
/// A parameter this language may change is written `mut name: T`
/// ([ADR-094](../../../docs/specification/adr/adr-094.md) D3) — the word goes
/// in front of the **name**, because what it decides is also what the caller
/// sees — and a run of elements is a `Vec[T]` or an `Array[T, N]` here, both of
/// which carry their length.
#[test]
fn neither_form_is_a_type_away_from_the_boundary() {
    let refused: Vec<_> = findings(
        "fn fill(out: &mut Vec[i64]) {\n\
         \x20   out.push(1)\n\
         }\n\
         \n\
         fn total(xs: &[i64]) -> i64 {\n\
         \x20   return xs.len()\n\
         }\n\
         \n\
         fn main() { println(\"x\") }\n",
    )
    .into_iter()
    .filter(|f| f.code == "NK1158")
    .collect();
    assert_eq!(refused.len(), 2, "{refused:#?}");
    assert!(
        refused.iter().any(|f| f.message.contains("`&mut`")),
        "{refused:#?}"
    );
    assert!(
        refused.iter().any(|f| f.message.contains("`[T]`")),
        "{refused:#?}"
    );
    assert!(
        refused
            .iter()
            .any(|f| f.help.as_deref().unwrap_or_default().contains("mut out")),
        "and the help names the shape this language does have:\n{refused:#?}"
    );
}

/// **A declaration that uses them is not refused**, which is the other half of
/// the same rule.
#[test]
fn the_boundary_itself_is_left_alone() {
    let found = findings(
        "extern \"C\" {\n\
         \x20   fn strlen(s: &[u8]) -> usize\n\
         \x20   fn read(fd: i32, buf: &mut [u8], count: usize) -> i64\n\
         }\n\
         \n\
         fn main() { println(\"x\") }\n",
    );
    assert!(
        found
            .iter()
            .all(|f| f.code != "NK1158" && f.code != "NK1135"),
        "{found:#?}"
    );
}

/// **The declaration reaches the ledger as it was written** (Part III 13.5), so
/// a consumer reads `&[u8]` and not the pointer it lowers to — which is
/// [Part III C.1](../../../docs/specification/30-nikaia-tooling.md)'s rule for
/// every message this compiler writes.
#[test]
fn a_declaration_ships_in_this_languages_words() {
    let parsed = parse_to_ast(
        "extern \"C\" {\n\
         \x20   fn read(fd: i32, buf: &mut [u8], count: usize) -> i64\n\
         }\n",
    )
    .expect("the source parses");
    let written = Ledger::infer(&parsed).render();
    assert!(
        written.contains("signature = \"(fd: i32, buf: &mut [u8], count: usize) -> i64\""),
        "{written}"
    );
}

/// **Measured where it matters: it compiles against real C, and it runs.**
///
/// `strlen` and `abs` are libc, which every Rust program links already — so
/// this needs no library the machine may not have, and it is the whole of D1
/// end to end: the declaration, the pointer, the call, the answer.
#[test]
fn the_boundary_compiles_and_runs() {
    let rust = lowered(
        r#"
extern "C" {
    fn strlen(s: &[u8]) -> usize
    fn abs(n: i32) -> i32
}

fn main() {
    let text = "hello\0"
    let n = unsafe { strlen(text) }
    let a = unsafe { abs(-3) }
    println(f"{n} {a}")
}
"#,
    );
    let dir = common::scratch_dir("foreign-pointers");
    let file = dir.join("main.rs");
    std::fs::write(&file, &rust).expect("write the Rust");
    let binary = dir.join("program");
    let out = common::compile(&file, &["-o", binary.to_str().expect("utf-8 path")]);
    let said = String::from_utf8_lossy(&out.stderr).into_owned();
    assert!(
        out.status.success(),
        "the lowering compiles:\n{said}\n--- the Rust ---\n{rust}"
    );
    assert!(
        !said.contains("improper_ctypes"),
        "and `rustc` says nothing about the signature it was handed:\n{said}"
    );
    let ran = std::process::Command::new(&binary)
        .output()
        .expect("run the program");
    assert_eq!(
        String::from_utf8_lossy(&ran.stdout).trim(),
        "5 3",
        "{}",
        String::from_utf8_lossy(&ran.stderr)
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// **A length beside a view is checked at the call** (D2).
///
/// A pointer and a count are one fact in C — the first says where and the
/// second says how far — so the declaration is where this language says they
/// belong together, and a call that passes a longer count is the buffer
/// overrun the boundary exists to stop.
#[test]
fn a_length_a_buffer_covers_is_accepted() {
    for count in ["room.len()", "32", "0"] {
        let source = format!(
            "extern \"C\" {{\n\
             \x20   fn read(fd: i32, buf: &mut [u8], count: usize) -> i64\n\
             }}\n\
             \n\
             fn main() {{\n\
             \x20   let mut room: Array[u8, 2] = [0, 0]\n\
             \x20   let n = unsafe {{ read(0, room, {count}) }}\n\
             \x20   println(f\"{{n}}\")\n\
             }}\n",
        );
        // `32` is longer than two, so only the other two stand on their own
        // here; the length that does not fit is the test below.
        let found: Vec<_> = findings(&source)
            .into_iter()
            .filter(|f| f.code == "NK1159")
            .collect();
        match count {
            "32" => assert_eq!(found.len(), 1, "`{count}` is longer than the array"),
            _ => assert!(found.is_empty(), "`{count}` fits: {found:#?}"),
        }
    }
}

/// **And a constant the array's own length covers is accepted**, because an
/// `Array[T, N]` carries its length in its type
/// ([ADR-152](../../../docs/specification/adr/adr-152.md) D1).
#[test]
fn a_constant_within_a_known_length_is_accepted() {
    let source = "extern \"C\" {\n\
                  \x20   fn read(fd: i32, buf: &mut [u8], count: usize) -> i64\n\
                  }\n\
                  \n\
                  fn main() {\n\
                  \x20   let mut room: Array[u8, 4] = [0, 0, 0, 0]\n\
                  \x20   let n = unsafe { read(0, room, 4) }\n\
                  \x20   println(f\"{n}\")\n\
                  }\n";
    assert!(findings(source).is_empty(), "{:#?}", findings(source));
}

/// **A count nothing can show to fit is refused**, naming both ways out — which
/// is D2's *narrow on purpose*: the buffer's own `len()`, or a constant a known
/// length covers, and anything else asks for one of those two.
#[test]
fn a_length_that_cannot_be_shown_to_fit_is_refused() {
    let found: Vec<_> = findings(
        "extern \"C\" {\n\
         \x20   fn read(fd: i32, buf: &mut [u8], count: usize) -> i64\n\
         }\n\
         \n\
         fn main() {\n\
         \x20   let mut buf: Vec[u8] = []\n\
         \x20   buf.push(0)\n\
         \x20   let n = unsafe { read(0, buf, 8) }\n\
         \x20   println(f\"{n}\")\n\
         }\n",
    )
    .into_iter()
    .filter(|f| f.code == "NK1159")
    .collect();
    assert_eq!(found.len(), 1, "{found:#?}");
    assert!(found[0].message.contains("`buf`"), "{found:#?}");
    assert!(
        found[0]
            .help
            .as_deref()
            .unwrap_or_default()
            .contains("buf.len()"),
        "{found:#?}"
    );
}

/// **A `usize` at the boundary takes this language's own integer** (D2,
/// [ADR-048](../../../docs/specification/adr/adr-048.md) D1).
///
/// A length here is an `i64` and the machine-width type left the surface a
/// program can write, so a declaration that says `size_t` is handed an `i64`
/// and the **conversion is emitted** — a user writes `buf.len()` and never
/// `buf.len() as usize`, which is a cast into a type Part I 2.2 does not offer.
#[test]
fn a_size_takes_an_i64_and_the_conversion_is_written() {
    let source = "extern \"C\" {\n\
                  \x20   fn read(fd: i32, buf: &mut [u8], count: usize) -> i64\n\
                  }\n\
                  \n\
                  fn main() {\n\
                  \x20   let mut buf: Vec[u8] = []\n\
                  \x20   buf.push(0)\n\
                  \x20   let n = unsafe { read(0, buf, buf.len()) }\n\
                  \x20   println(f\"{n}\")\n\
                  }\n";
    assert!(findings(source).is_empty(), "{:#?}", findings(source));
    let rust = lowered(source);
    assert!(
        rust.contains("nikaia_std::count::of(buf.len() as i64)"),
        "{rust}"
    );
    // **And no `&` in front of it.** `keeps::moves` did not name `usize` or
    // `isize`, so a type it did not name was one that *moves* and the compiler
    // lent the count ([ADR-094](../../../docs/specification/adr/adr-094.md)
    // D1) — `read(0, buf.as_mut_ptr(), &buf.len() as i64)`, which is not Rust.
    // No program could write a `usize` before this record, so the first
    // declaration to name one is the first program to meet it.
    assert!(!rust.contains("&nikaia_std::count::of"), "{rust}");
    assert!(!rust.contains(", &buf.len()"), "{rust}");
}

/// **The pair is the declaration's own types**, so a `usize` with no buffer in
/// front of it is an ordinary parameter and nothing is claimed about it.
#[test]
fn a_size_with_no_buffer_before_it_is_left_alone() {
    let source = "extern \"C\" {\n\
                  \x20   fn sleep(seconds: usize) -> i32\n\
                  }\n\
                  \n\
                  fn main() {\n\
                  \x20   let n = unsafe { sleep(1) }\n\
                  \x20   println(f\"{n}\")\n\
                  }\n";
    assert!(findings(source).is_empty(), "{:#?}", findings(source));
}

/// **A handle is opaque, declared, and released by a `cleanup`** (D3).
///
/// The shape every C library with a session in it is made of — a database
/// connection, an HTTP client, a compressor — and the reason the record exists
/// beside the buffer: a library that hands one out was unwritable, because
/// `Pointer[T]` is a type nothing declares.
#[test]
fn an_opaque_handle_is_an_address_and_a_cleanup() {
    let rust = lowered(
        "extern \"C\" {\n\
         \x20   opaque type FILE released by fclose\n\
         \x20   fn fopen(path: &[u8], mode: &[u8]) -> FILE\n\
         \x20   fn fclose(f: FILE) -> i32\n\
         }\n\
         \n\
         fn main() { println(\"x\") }\n",
    );
    // `repr(transparent)`, because the whole point of the type is its layout:
    // a handle **is** the address, so what C is handed is the pointer.
    assert!(rust.contains("#[repr(transparent)]"), "{rust}");
    assert!(
        rust.contains("pub struct FILE(*mut core::ffi::c_void);"),
        "{rust}"
    );
    // Part I 6.4's `cleanup` read at the C boundary: the release runs at the
    // end of the handle's scope, so a handle cannot be forgotten.
    assert!(rust.contains("impl Drop for FILE"), "{rust}");
    assert!(
        rust.contains("let _released = unsafe { fclose(self.lent()) };"),
        "{rust}"
    );
}

/// **A handle is lent to every declaration but its release** (D3).
///
/// `fileno(f)` reads the handle and `f` is still the caller's to close, so the
/// address goes by value and the value stays here; `fclose(f)` **is** the
/// cleanup, so the handle goes with it and Rust's own move is what keeps it
/// from being released twice.
///
/// The `&` [ADR-094](../../../docs/specification/adr/adr-094.md) D1 would
/// otherwise write is the wrong address as well as the wrong ownership:
/// `&FILE` is `FILE**` where C wants `FILE*`.
#[test]
fn a_handle_is_lent_and_only_its_release_takes_it() {
    let rust = lowered(
        "extern \"C\" {\n\
         \x20   opaque type FILE released by fclose\n\
         \x20   fn fopen(path: &[u8], mode: &[u8]) -> FILE\n\
         \x20   fn fclose(f: FILE) -> i32\n\
         \x20   fn fileno(f: FILE) -> i32\n\
         }\n\
         \n\
         fn main() {\n\
         \x20   let f = unsafe { fopen(\"/etc/hosts\\0\", \"r\\0\") }\n\
         \x20   let fd = unsafe { fileno(f) }\n\
         \x20   let g = unsafe { fopen(\"/etc/hosts\\0\", \"r\\0\") }\n\
         \x20   let closed = unsafe { fclose(g) }\n\
         \x20   println(f\"{fd} {closed}\")\n\
         }\n",
    );
    assert!(rust.contains("fileno(f.lent())"), "{rust}");
    assert!(rust.contains("fclose(g)"), "{rust}");
    assert!(!rust.contains("fileno(&f)"), "{rust}");
}

/// **A handle has no fields and no indexing** (`NK1160`, D3): it is an address
/// this language never dereferences, so there is nothing inside it to name.
#[test]
fn nothing_reaches_inside_a_handle() {
    let found: Vec<_> = findings(
        "extern \"C\" {\n\
         \x20   opaque type FILE released by fclose\n\
         \x20   fn fopen(path: &[u8], mode: &[u8]) -> FILE\n\
         \x20   fn fclose(f: FILE) -> i32\n\
         }\n\
         \n\
         fn main() {\n\
         \x20   let f = unsafe { fopen(\"/etc/hosts\\0\", \"r\\0\") }\n\
         \x20   println(f\"{f.handle}\")\n\
         \x20   println(f\"{f[0]}\")\n\
         }\n",
    )
    .into_iter()
    .filter(|f| f.code == "NK1160")
    .collect();
    assert_eq!(found.len(), 2, "{found:#?}");
    assert!(
        found
            .iter()
            .any(|f| f.message.contains("the field `handle`")),
        "{found:#?}"
    );
    assert!(
        found.iter().any(|f| f.message.contains("an index")),
        "{found:#?}"
    );
}

/// **None of the four words is reserved.** The grammar is scannerless, so a
/// word means something only where a rule asks for it — and `opaque`, `type`,
/// `released` and `by` are all names a program may want. Reserving a word buys
/// exactly one thing, the sentence a reader who writes it gets
/// ([ADR-117](../../../docs/specification/adr/adr-117.md) D2), and this
/// position says that sentence without taking the word away.
#[test]
fn the_four_words_are_still_names() {
    for word in ["opaque", "type", "released", "by"] {
        let source = format!(
            "fn main() {{\n\
             \x20   let {word} = 3\n\
             \x20   println(f\"{{{word}}}\")\n\
             }}\n"
        );
        assert!(
            parse_to_ast(&source).is_ok(),
            "`{word}` is a name everywhere but the one position that asks for it"
        );
    }
}

/// **Measured where it matters: it compiles against real C, and it runs.**
///
/// `fopen`, `fclose` and `fileno` are libc, which every Rust program links
/// already — so this needs no library the machine may not have. It is the whole
/// of D3 end to end: the handle, the declarations that take it, the lending,
/// and the cleanup that fires once at the end of the scope.
#[test]
fn a_handle_compiles_and_runs_and_is_released_once() {
    let rust = lowered(
        r#"
extern "C" {
    opaque type FILE released by fclose
    fn fopen(path: &[u8], mode: &[u8]) -> FILE
    fn fclose(f: FILE) -> i32
    fn fileno(f: FILE) -> i32
}

fn main() {
    let f = unsafe { fopen("/etc/hosts\0", "r\0") }
    let fd = unsafe { fileno(f) }
    println(f"{fd > 2}")
}
"#,
    );
    let dir = common::scratch_dir("opaque-handle");
    let file = dir.join("main.rs");
    std::fs::write(&file, &rust).expect("write the Rust");
    let binary = dir.join("program");
    let out = common::compile(&file, &["-o", binary.to_str().expect("utf-8 path")]);
    let said = String::from_utf8_lossy(&out.stderr).into_owned();
    assert!(
        out.status.success(),
        "the lowering compiles:\n{said}\n--- the Rust ---\n{rust}"
    );
    assert!(
        !said.contains("improper_ctypes") && !said.contains("warning:"),
        "and `rustc` says nothing about the file it was handed:\n{said}"
    );
    let ran = std::process::Command::new(&binary)
        .output()
        .expect("run the program");
    assert_eq!(
        String::from_utf8_lossy(&ran.stdout).trim(),
        "true",
        "{}",
        String::from_utf8_lossy(&ran.stderr)
    );
    let _ = std::fs::remove_dir_all(&dir);
}
