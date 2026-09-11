# Nikaia Language Specification
**Part III: Tooling, Ecosystem & Interoperability**
**Version:** 0.0.7 (Draft)
**Date:** September 5, 2026

---

## Chapter 13: The Toolchain (CLI)

A modern programming language is more than just a compiler. It requires a suite of tools to manage dependencies, formatting, and building. Nikaia provides a single command-line interface (CLI) called `nikaia`.

### 13.1. Project Structure
When you create a new project (`nikaia new my_project`), the following structure is generated:

* `nikaia.toml`: The **Manifest**. It describes the project, its authors, and its dependencies.
* `nikaia.lock`: The **Lockfile**. It records *everything that determines the build*, and is therefore also the **Cache Key** ([ADR-021](adr/adr-021.md)). One file, because a reproducibility record that omits an input cannot tell you it is incomplete.
    * **Asset Hashing:** If a macro or grammar reads an external file (e.g., `from "schema.sql"`), the compiler stores the file's SHA256 hash here.
    * **Source Hashing:** The SHA256 of each `.nika` source that took part, so an unchanged module skips parsing and expansion entirely.
    * **Resolved Versions:** The exact dependency versions, the toolchain version actually used, and the **Nikaia compiler's own version** - a changed emitter produces different output from identical input, so leaving it out makes the cache serve stale artifacts (ADR-021 D3).
    * **Declaration vs. record:** `nikaia.toml` states what the project *requires*; `nikaia.lock` records what was *resolved and used* - the same relationship `Cargo.toml` has with `Cargo.lock`.
    * **Not in the lockfile:** build-time choices (both switches, opt-level, backend). They are hashed into the cache key but never written, or every change of switch would rewrite a committed file for no reason (ADR-021 D5).
    * **Instant Builds:** On subsequent builds, if the hashes on disk haven't changed, the compiler skips re-processing and reuses the artifact from the content-addressed store under `target/nikaia/cache/` (git-ignored; the lockfile holds inputs, the store holds outputs). Keys are per translation unit, so one changed asset invalidates that unit, not the project (ADR-021 D6).
* `nikaia.contracts`: The **Borrow Contract Ledger** (generated, commit it like the lockfile). Records the borrow contracts the compiler inferred for your functions and the tether relationships of your structs. It is both an incremental-build cache and the basis for the compiler's "what changed and what broke" error messages. Details in Chapter 13.5.
* `src/`: The folder containing your source code.
    * `main.nika`: The entry point.

### 13.2. Core Commands
* `nikaia build`: Compiles the project.
* `nikaia run`: Compiles and executes.
* `nikaia test`: Runs unit tests and fuzzers.
* `nikaia bench`: Runs performance benchmarks.
* `nikaia fmt`: Automatically formats your code.

**Status:** `build` and `run` are built, over `nikaia.toml` translated to a
`Cargo.toml` ([ADR-002](adr/adr-002.md) D1 §5). `test`, `bench` and `fmt` are
not. A single file outside a project is compiled with `nikaia --input
<file>.nika`, which is not one of these commands and stays available on its own
account ([ADR-021](adr/adr-021.md) D11).

### 13.3. Manifest Configuration (`nikaia.toml`)
The manifest defines project metadata and the two build switches of Part I 1.2.

They live in `[build]`, and a flag overrides any of them for a single build —
which is what a benchmark and a bug hunt need, while the committed value is the
one a reviewer sees (ADR-037 D5). A key `[build]` does not know is a typo and
fails the build rather than being ignored.

```toml
[package]
name = "hyper-core"
version = "0.1.0"
authors = ["dev@nikaia.org"]

[build]
# Which machine to build for (ADR-037 D1). `wasm32-unknown` has no threads
# and traps rather than unwinding, which is what decides the panic strategy
# and what `std` can offer.
target = "x86_64-linux"

# May *your* code run concurrently at all (ADR-037 D2)? A permission, not a
# count - how many threads serve a "yes" is the runtime's to decide.
#   "no"  (default) - nothing you wrote ever runs concurrently
#   "yes"           - it may
# This bounds your program, not the compiler: reading a file may still
# validate its text on several cores at "no", because that is not your code
# and changes nothing your program prints.
user-parallelism = "no"

# How long the runtime waits at program end for pending resource cleanups
# (flushes, rollbacks, connection shutdowns — see Part I, 6.4 and ADR-006).
# Generous default: "30s". On expiry, remaining cleanups are cancelled
# (their synchronous fallback runs) and the program exits with a warning
# naming every resource that did not finish cleanly. "0" disables draining.
# This deadline cannot hang: the timer runs in the runtime itself, and
# cancelling a cleanup always terminates (the fallback cannot pause).
cleanup-deadline = "30s"

# How strictly the written order of two operations is taken (ADR-033, Part I 8.1.1).
#   "effects" (default) - two operations that touch disjoint resources may
#       overlap; everything else keeps the order it was written in.
#   "strict"            - the written order, always. The analysis is not applied.
# Not an aid to be removed later: it is the escape for a project that does not
# want this, and the way to rule the analysis out when chasing a bug in the field.
ordering = "effects"

[dependencies]
# A Nikaia package. How one is resolved is not decided: no record names a
# registry, a name space or a distribution format, so the compiler refuses this
# with that reason rather than guessing (ADR-002 D1 §5).
http-server = "1.2"
# Import native Rust Crates. This reaches Cargo with only `type` removed, and
# Cargo resolves, fetches and links it as it would for any Rust project.
regex = { type = "rust", version = "1.5" }

# Code generation, per target. These are choices about output size and speed,
# and they change nothing a program means - which is why they are tables under
# `[build]` rather than switches in it. The table of the machine a build chose
# becomes the generated `Cargo.toml`'s profile (ADR-002 D1); the panic strategy
# is *not* here, because it follows from `target` rather than being a choice.
[build.wasm32-unknown]
opt-level = "z"     # Optimize for binary size

[build.x86_64-linux]
opt-level = 3       # Maximize throughput
lto = true          # Link Time Optimization
```

### 13.4. Build Scripts (`build.nika`)
If a project requires custom build steps (e.g., compiling C-code or generating proto-files), you can place a `build.nika` file in the root. This script is compiled and executed **before** the main build.

It has access to a special `std::build` API to emit instructions to the compiler.

```nika
// build.nika
use std::build

fn main() {
    // Compile a local C library
    build::cc("src/native/mylib.c")
    
    // Link against a system library
    build::rustc_link_lib("z") // links libz
}
```

### 13.5. The Borrow Contract Ledger (`nikaia.contracts`)

Nikaia source code contains no lifetime annotations (Part I, Chapter 6.5; [ADR-005](adr/adr-005.md)). Instead, the compiler infers a **Borrow Contract** for every function whose signature involves references — e.g. *"the result of `longest(a, b)` borrows from `a` or `b`"* — by analyzing all `.nika` sources whole-program. The result is persisted in a generated, human-readable file in the project root: **`nikaia.contracts`**.

**File format (illustrative):**

```toml
# AUTO-GENERATED by `nikaia build`. Commit this file like a lockfile.
# Do not edit by hand — it is regenerated on every build.
version = 1
toolchain = "nightly-2026-01-01"

[fn."text::longest"]
returns = "borrows(a | b)"

[fn."http::Request::header"]
returns = "borrows(self)"

[struct."parser::Token"]
tethered = ["text -> source buffer"]
```

**Why a generated file, not annotations in code?** The contract is *derived* truth — it follows from the function body. Putting derived truth in source code invites it to go stale. Putting it in a diffable artifact gives the compiler a memory: it can compare what a contract *was* with what it *is*, and explain the difference.

**Build semantics.** On every build the compiler infers fresh contracts and diffs them against the ledger:

1. **Unchanged** → fast path. Callers of unchanged contracts are not re-checked; the ledger acts as an incremental-compilation cache key. This is a *different* mechanism from `nikaia.lock`'s, despite the shared name ([ADR-021](adr/adr-021.md) D10): the lock is consulted **before** any work and answers *do I need to start at all?*, while the ledger can only be compared **after** inference has run and answers *which callers must be re-checked?* The two are complementary stages of one build. They also stay separate files, because the ledger ships with published packages while a lockfile does not, and because `--locked` means opposite things for them - regenerate-and-compare for the ledger, do-not-re-resolve for the lock.
2. **Changed, all callers still valid** → the ledger is updated automatically and the build proceeds. The change is noted in the build output.
3. **Changed, and a caller breaks** → the compiler uses the diff to narrate the *cause chain* instead of pointing at a mysterious distant line:

```text
error[NK2401]: a change in `longest` broke its caller `report`
  --> text.nika:8 (the change)
   |
 8 |     return b.trim()
   |            ^^^^^^^^ the result of `longest` now borrows from `b`
   |                     (previously: only from `a`)
  --> main.nika:14 (the breakage)
   |
14 |     let best = longest(title, subtitle)
15 |     drop(subtitle)
16 |     println(best)
   |             ^^^^ `best` may point into `subtitle`, which is already gone
   |
  help: either keep `subtitle` alive until after line 16,
        or return an owned copy from `longest`: `return b.trim().to_string()`
  note: contract change recorded in nikaia.contracts (line 12) — if the
        change was unintentional, this diff shows exactly what to revert.
```

**Trait methods.** Dynamic dispatch requires one contract per trait method. The ledger stores it as the join of all implementations; an implementation that broadens the contract produces a ledger diff and, where callers break, the same narrated error.

**What the ledger records.** Not only borrows. The question it answers is *what must a caller know about a body it cannot see*, and "may it pause" and "may it fail" are two more answers to it ([ADR-020](adr/adr-020.md) D2):

| key | on | meaning |
| :--- | :--- | :--- |
| `pub` | fn, type | reachable from outside the unit that declares it |
| `sync` | fn | Part II 12.1: pure computation, cannot pause, cannot do I/O. `true` where the source asserted it, `"inferred"` where the body implies it ([ADR-027](adr/adr-027.md)), `"from(f)"` where the lambda it is given decides ([ADR-029](adr/adr-029.md)) |
| `throws` | fn | Kap 7.1: it may fail, and **with what** — `throws = ["ConfigError", "IoError"]`, the set inferred over the call graph ([ADR-023](adr/adr-023.md) D1). Stage 0 writes `true`, because a compiler that cannot lower an error type has no names to put in a list |
| `returns` | fn | what the result may point into — `borrows(a \| b)` |
| `signature` | fn | its parameters, its **options** and its result, as the source writes them: `"(path: ?, data: ?; append: bool = false, create: bool = true)"`. An option carries its default, because a call that leaves one out still passes a value and only the declaration knows which (Part I, 5.1). A method's receiver is the first parameter, so a caller reads the arguments off one list either way. A generic parameter is recorded as `?`, because `T` is a name that stands for a type rather than being one |
| `borrowed` | type | ADR-008 D6: `@borrowed` was asserted in the source |
| `fields` | type | every field with its type: `["name: &str", "temp: i32"]` |
| `tethered` | type | the fields that hold a view, directly or through another type that does |
| `touches` | fn | which resources it reaches and whether it reads or writes them — `["file(path) write", "stdout write"]` ([ADR-033](adr/adr-033.md)). **Absent means it touches everything**, so a function nobody has described orders against everything and stays where it was written. *Specified, not implemented.* |

`signature` and `fields` are what make a *type* checker possible across a boundary whose bodies are not visible — the `NK1xxx` diagnostics above are all answered from them ([ADR-024](adr/adr-024.md)). They are also where the ledger's `?` earns its keep: it is **the absence of a claim**, and a checker reports a mismatch only where both sides are written down, so a contract that says less makes the compiler quieter and never wronger.

Only what is *true* is written: a `sync = false` on every entry would treble the file and say nothing, and a diff should show a promise being made or withdrawn. **An absent `sync` therefore means not `sync`** — while an absent *entry* means nothing is known and a caller may not assume. That distinction is what makes the file worth shipping rather than deriving.

**`sync` is written down in two ways, because it is arrived at in two ways** ([ADR-027](adr/adr-027.md)). `sync = true` is a promise the *source* made, and `NK2202` is what the compiler says when the body contradicts it. `sync = "inferred"` is a promise the *body* implies: nothing the function calls can pause, so it cannot pause, and saying otherwise would be the ledger recording something it had already read and knew better about.

A **caller** does not distinguish them. Both mean "this cannot pause", both satisfy `access` and `par_iter`, and any code that asks the ledger the caller's question gets one answer. A **diff** must distinguish them, and that is the whole reason the file spells them differently: withdrawing an asserted `sync` is a decision someone made and has to have meant, while losing an inferred one is a *consequence* of an edit somewhere else — usually in a function further down. The two deserve different sentences, and a `bool` cannot produce them.

**`throws` is a set for the same reason**, and it is the second key this argument has been made about. A boolean answers "can this fail", which is what a *caller* needs in order to declare its own `throws` — and nothing else. A `catch` needs more: whether it still covers everything that can reach it. That question has an answer only if the ledger names the errors, and the answer changes when a function three modules down gains a `throw`. Recorded as a set, the diff says which error appeared and which `catch` stopped covering its arrivals; recorded as a boolean, it says nothing at all, because `true` was already `true`. The narration is `NK24xx` (Appendix C), the same machinery a changed borrow contract uses.

**A function that runs somebody else's code says so** ([ADR-029](adr/adr-029.md)). `xs.map fn { a + 1 }` cannot pause and `xs.map fn { io::read()… }` can, and they are the same `map`. A ledger entry has to hold for every caller, so without a way to say "it depends" a higher-order function has to commit to the pessimistic answer — and since `access`, `par_iter` and a scope's tasks all gate on a `sync` lambda, that commitment reads as *no `map`, `filter` or `fold` inside a lock, over any lambda at all*.

`sync = "from(f)"` is the way to say it, naming the parameter that decides. A caller reads it as **"this call adds no pausing of its own"**, which is sound because the lambda runs *during* the call: its body is part of the function that writes it, and that function has already counted its calls. `from` adds nothing because there is nothing left to add.

That reason is also the limit. A parameter the callee **stores or spawns** — Part I 5.4's `@detached` — breaks it, because then the lambda's calls belong to nobody the caller is counting. `from` is for an immediate lambda only, and until the ledger can spell `@detached` that rule is held by a test over `std`'s own entries rather than by the file format.

**A signature may name its receiver's type arguments** ([ADR-031](adr/adr-031.md)). `HashMap::entry` is written `(&HashMap[$K, $V], key: ?) -> Entry[$V]`: the result holds whatever the map holds. At a call site the receiver's actual type binds the variables and they are substituted away, so `HashMap[&str, Stats]` makes that result an `Entry[Stats]`, and `and_modify`'s `fn(&$V)` a `fn(&Stats)` — which is what gives the `a` in `fn { a.add(t) }` a type at all.

**An unbound variable becomes `?`, never a name**, and that is the whole of why this is safe. A variable that survived into a comparison would make the checker report that `i32` is not `$V` — the false positive ADR-024 D4 erases generics to avoid. Here it cannot survive: it is bound and replaced, or it is the absence of a claim. A map built by `HashMap::new()` says nothing about what it holds, binds nothing, and the chain stops helping rather than guessing.

**A variable says what flows *out*; `?` stays for what flows *in*.** It may appear in a result and in a lambda's parameter type, and never in an argument. What flows out is a promise the ledger makes and is wrong on its own account; what flows in is a constraint on somebody's program — and the language below deliberately accepts more than its type parameters suggest, so a variable there would reject correct code. Binding itself is narrow on purpose: from the receiver, by position, one pattern. This is the first inference in this file's type language rather than more vocabulary, and widening it is meant to be a decision rather than a diff.

The type it names is a **function type**, `fn(&Stats)`, which is the other half of the same decision: it says what the lambda is handed, so that the `a` in `fn { a.add(t) }` has a type and what it is called on can be resolved. Only a ledger writes one — Nikaia's grammar has no syntax for a function type, so no source program can declare a parameter of that shape.

The two are also arrived at with opposite caution, which is worth stating plainly because it looks like an inconsistency and is not:

* The **check** on an assertion is conservative in the *permissive* direction. It reports only calls it can prove will pause, so it never rejects a correct program. A call it cannot resolve is not an error.
* The **inference** is conservative in the *restrictive* direction. It claims `sync` only where every call resolves and every callee is `sync`; that same unresolvable call costs the function its claim. The entry is **shipped**, and a consumer will read it and put the function inside `access` — a wrong `sync` there is a pausing body inside a lock. The ledger already settled this for provenance: an analysis that fails open is a vulnerability generator.

**What counts as resolvable is the type checker's answer, not a second opinion** ([ADR-028](adr/adr-028.md)). A call by name — `helper(x)`, `io::read_to_string()` — is looked up directly. A *method* call needs the receiver's type, and the compiler has one module that infers types; it records where each method call went, and the inference reads that rather than growing an inference of its own. So what the two analyses above can see grows whenever the type checker can name more, and neither of them changes when it does.

The gap between those two polarities is not a defect to close. It is exactly where a person writes `sync` by hand: "I know this cannot pause, hold me to it" — the same move `@borrowed` makes in Part I, 6.6, and checked the same way. What shrinks the gap is not a change to either analysis but a ledger that describes more (ADR-024, ADR-028), at which point more functions earn the promise on their own and nothing else has to move.

One consequence is worth stating for a library author: **writing a signature down is what lets your callers be `sync`.** A method with no entry is an unknown, and an unknown costs every function that calls it its inferred promise — so a library that ships thin contracts makes its consumers' code unusable inside `access` and `par_iter`, however pure that code is. This is the same fact 13.5 opens with, met from the caller's side.

**Which inference wrote it.** The header carries `inference`, because a ledger produced by reading signatures is not one produced by reading bodies and must not be mistaken for it. Today's bootstrap compiler writes `stage0-signatures+sync-bodies`, and the name says which half is which: `throws` is declared in the source and recorded exactly, the borrow contract is the widest one the signature supports — a result that is a view may point into any view it was given — and `sync` is read off the **body** ([ADR-027](adr/adr-027.md)). It wrote `stage0-signatures` before that, and a ledger regenerated by this compiler therefore shows a header change; the mechanism that narrates it is the one this paragraph exists for. The `toolchain` recorded is **Nikaia's** version, not `rustc`'s: these contracts are decided by this compiler and never by the one it emits code for.

**Distribution.** Published packages ship their ledger, so downstream projects build against stable contracts and receive identical diff-based explanations when a dependency upgrade changes one. `std` ships `std.contracts`, and it is the file a program's compiler reads when the program calls `io::…` or `fs::…`. A library whose implementation is partly in another language cannot have all of its contracts inferred, so those are **written in the ledger and reviewed like code**, marked as such, while the ones that can be inferred are regenerated and checked against the sources by the library's own tests ([ADR-020](adr/adr-020.md) D5).

**Version control.** Commit `nikaia.contracts`. Merge conflicts resolve like lockfile conflicts: accept either side and run `nikaia build` to regenerate. The recorded `toolchain` hash lets the compiler detect when a toolchain upgrade (not your code) changed inference results; in that case the build output states explicitly that the contract changes were caused by the toolchain update, not by your code.

**Determinism guarantee.** The ledger is a **pure function of (source tree, toolchain)**: the same sources and the same pinned toolchain produce a byte-identical `nikaia.contracts` on every machine, every run, with any thread count. This is a hard guarantee (see [ADR-005](adr/adr-005.md), D8, including the implementation ban list and the CI tests that enforce it); a violation is treated as a compiler bug. Two consequences worth knowing:

* There is exactly **one** ledger per project — it is valid at every setting of either switch. Borrow contracts and tether relationships are switch-independent by design; switch-dependent checks (such as thread-safety rules) are performed by the compiler directly and are never recorded in the ledger.
* Ledger stability is **not** promised across toolchain *upgrades* — a newer compiler may infer better contracts. The toolchain hash plus the explicit "caused by the toolchain update" narration make such diffs self-explaining instead of alarming.

**Verification mode (`--locked`).** `nikaia build --locked` (and CI setups) verify instead of update: the compiler regenerates the contracts in memory and compares them byte-for-byte against the committed `nikaia.contracts`. Any difference fails the build with the narrated contract diff (see `NK2401` above). Because of the determinism guarantee, this check is exact and needs no tolerance or semantic comparison — the recommended CI line is simply building with `--locked`, which is equivalent to `git diff --exit-code nikaia.contracts` after a regular build.

---

## Chapter 14: Testing and Quality Assurance

Testing and verification are first-class citizens in Nikaia.

### 14.1. Unit Tests (`test`)
Standard tests check specific inputs. These blocks are only compiled during `nikaia test`.

```nika
test "Addition" {
    assert 1 + 1 == 2
}
```

### 14.2. Runtime Assertions (Design by Contract)
You can use `assert` statements inside normal functions to enforce preconditions or invariants.

**Compiler Behavior:**
* **Debug Profile:** Assertions are active. If the condition is false, the program panics with a detailed message.
* **Release Profile:** Assertions are **removed** (optimized out) to ensure maximum performance, unless explicitly enabled via `nikaia build --with-asserts`.

```nika
fn divide(a: i32, b: i32) -> i32 {
    // Precondition: Denominator must not be zero.
    // In Release mode, this check disappears.
    assert b != 0, "Division by zero prohibited"
    
    return a / b
}
```

### 14.3. Property-Based Testing (Fuzzing)
Fuzzing generates random data to find crashes. Nikaia automates this.

**Automatic Data Generation**
If you pass arguments to a test, Nikaia automatically generates inputs.
* **Primitives:** Random integers, strings, bools.
* **Structs:** Nikaia recursively generates data for every field.

```nika
struct User { name: String, age: i32 }

// Nikaia automatically creates random 'User' structs here
test "User Validation" (u: User) {
    assert u.age >= 0 // Might fail if fuzzer generates -1
}
```

**Custom Generators (`impl Generator`)**
Sometimes random data isn't enough (e.g., you need valid email addresses). You can implement the `Generator` trait.

```nika
impl Generator for User {
    // 1. How to generate standard random samples
    fn arbitrary() -> User {
        User { 
            name: String::random_alphanumeric(10), 
            age: i32::random_range(0, 100) 
        }
    }

    // 2. Defining "Edge Cases" (Values likely to break things)
    // The fuzzer will ALWAYS try these values first.
    fn edge_cases() -> [User] {
        [
            User { name: "", age: 0 },         // Empty/Zero
            User { name: "A" * 1000, age: -1 } // Overflow/Negative
        ]
    }
}
```

### 14.4. Benchmarking (`bench`)
Benchmarking measures how fast your code is.

**Regression Detection**
When `nikaia bench` runs, it:
1.  Executes the code block thousands of times.
2.  Calculates the average time and standard deviation.
3.  **Compares** it against the last recorded run.

If the new version is significantly slower (e.g., > 5%), the CLI prints a warning:
> ⚠️ **Performance Regression:** 'Sorting' is 12% slower than commit 8f3a2c.

**Result Storage**
Results are stored in `.nikaia/benchmarks.json`. This file tracks:
* Timestamp
* Git Commit Hash
* Function Name
* Nanoseconds per Operation

```nika
bench "Sorting" {
    let list = [5, 2, 9, 1, 6]
    list.sort()
}
```

**Viewing History**
You can visualize the history using: `nikaia bench --history`.

---

## Chapter 15: Interoperability (FFI)

Nikaia is designed to live in a world dominated by C and Rust.

### 15.1. C Interoperability
Talking to C requires `unsafe` blocks because C is not memory-safe.

```nika
extern "C" {
    fn malloc(size: usize) -> Pointer[u8]
}

fn raw_alloc() {
    unsafe { malloc(1024) }
}
```

### 15.2. Rust Integration (Deep Integration)
Nikaia treats Rust Crates differently than C libraries. Because Rust has a strong type system, Nikaia can verify safety properties.

**Mapping Types**
* Rust `i32` -> Nikaia `i32`
* Rust `String` -> Nikaia `String`
* Rust `Option<T>` -> Nikaia `T?` (Nullable)

**Thread Safety (Send/Sync)**
Nikaia can detect thread safety in Rust code. The compiler reads the metadata of the Rust Crate.

* If a Rust type implements the `Send` trait (safe to move between threads), Nikaia allows using it in `spawn` tasks.
* If a Rust type is `!Send` (e.g., `Rc<T>`), and you try to use it where your code runs in parallel, the Nikaia compiler produces an error:
    > "Error: Cannot move Rust type 'Rc<i32>' to another thread. It is not Thread-Safe."

```nika
// Usage of a Rust crate
[dependencies]
image = { type = "rust", version = "0.24" }

// In code
use crate::image

fn process() {
    // This is safe because the 'image' crate implements proper locking
    let img = image::open("test.png")
}
```

### 15.3. WebAssembly (WASM) Synergy
A single-threaded build possesses a natural affinity for WebAssembly. Since WASM (in its basic form) shares a linear memory model and runs in single-threaded host environments, `user_parallelism = no` is the perfect match.

**Zero Overhead**
Compiling with `nikaia build --target=wasm32-unknown` produces extremely compact binaries because the compiler does not generate OS-level mutexes or atomic operations in this mode.

**JavaScript Interoperability (`dsl js`)**
Instead of trying to map the entire DOM to Nikaia structs, Nikaia embeds raw JavaScript using the `dsl` keyword (Part II, 10.5).

```nika
// main.nika
fn main() {
    let message = "Hello from Nikaia!"

    // The 'js' grammar parses the code. ':msg' is a parameter hole -
    // a deferred parameter, not string interpolation, so the value cannot
    // be spliced into the source text and change its meaning.
    let script = dsl js {
        document.querySelector("#submit").addEventListener("click", () => {
            window.alert(:msg);
        });
    } eod

    // Subject: none ; Config: msg
    script.exec(; msg: message)
}
```

---

## Chapter 16: Hardware Instructions (via DSL)

Hardware instructions are **not** part of the Nikaia core language. They are provided by
library-defined DSLs — `dsl backend::x86`, `dsl backend::arm64`, `dsl backend::wasm` — each
of which validates its own operands.

### 16.1. Why not a core construct

A built-in `asm` block with register constraints — `in(reg)`, `out(reg)`, `clobber("cc")` —
would assume every target has registers. Nikaia targets **WebAssembly**
(Chapter 15), and WASM is a *stack machine*: there is nothing for `in(reg)` to mean. A core
construct that cannot be given meaning on a first-class target is a defect in the core, not in
the target.

As a DSL instead, each backend defines exactly the operand model its hardware has, and the
grammar that validates it ([ADR-007](adr/adr-007.md), D6).

### 16.2. Usage

Assembly uses the standard `dsl` syntax. Unlike SQL — which builds a reusable statement and
takes *deferred* parameters — the assembly DSL uses **immediate capture** (`meta::capture`,
Part II 10.5): it binds variables from the current scope and injects machine code at the call
site. That is the correct choice here, because `val` means *this* `val`, right here.

```nika
use std::backend::x86

fn fast_add(val: i64, ptr: &i64) -> i64 {
    let mut result: i64 = 0

    // The grammar parses the bindings and resolves 'val', 'ptr' and 'result'
    // from the enclosing scope.
    dsl x86 {
        // 1. Binding header - syntax defined by the x86 grammar
        $v = in(reg) val
        $p = in(mem) ptr
        $r = out(reg) result

        // 2. Instructions
        mov $r, $v
        add $r, $p
    } eod

    return result
}
```

The constraint vocabulary (`reg`, `freg`, `mem`, `imm`, clobber declarations) now belongs to
the `x86` grammar and is documented with it, not with the language. A stack-machine backend
declares a different vocabulary — `dsl wasm` has locals and a value stack, not registers.

### 16.3. Consequences

*   **Portability:** the language core makes no assumption about the target's execution model.
*   **Validation:** the DSL parser checks instruction operands at compile time, and reports
    errors through the same diagnostics contract as the rest of the compiler (Appendix C).
*   **Optimization:** a backend DSL can emit target-specific or SIMD instructions without any
    change to the language.
*   **`unsafe`:** this was the keyword's only specified use. It remains **reserved** for future
    FFI work rather than being removed from the grammar.

---

## Chapter 17: The Standard Library ("Batteries Included")

Unlike languages that prefer a minimal core, Nikaia pursues immediate productivity. The standard library consists of universal modules (same API everywhere) and target-specific capabilities.

### 17.1. Universal Modules
These modules rely on Unified Types and function identically at every setting, though their internal implementation differs significantly to match the runtime model.

**`std::io` — standard input**

A stream is not a file, and the surface says so ([ADR-019](adr/adr-019.md)): no `map`, no `seek`,
no length, and no second read of the same bytes. `fs::map` hands back pages that existed before
the program asked for them; standard input's bytes do not exist until they are read, so a program
that wants views into its input owns the buffer first.

```nika
pub fn read_to_string() -> String throws   // all of it, UTF-8 validated
pub fn read() -> Bytes throws              // all of it, as bytes
pub fn lines() -> Lines throws             // one line at a time
pub fn bytes() -> ByteStream throws        // chunks as they arrive
```

**A step of `lines` can fail, and the failure leaves the function** ([ADR-025](adr/adr-025.md) D1). A pipe is where this is unavoidable: its bytes do not exist until they are read, so the failure cannot be moved to the call the way `fs::map` moves it. Nothing marks the loop, for the reason nothing marks a failing call ([ADR-023](adr/adr-023.md) D8) — and the compiler is what makes the enclosing function declare `throws`:

```nika
fn tally() -> i64 throws {          // NK2701 without the `throws`
    let mut n = 0
    for line in io::lines() { n += 1 }
    return n
}
```

What must **not** happen is what a scanner that reports its error afterwards does: a failed read that is indistinguishable from the end of the input, so a truncated stream becomes a shorter one and the tally is quietly wrong. Part I 6.4 refuses that at the *closing* brace of a block; this is the same refusal at the top of a loop.

`read_to_string`, `read` and `lines` are implemented. `bytes` is not, and needs nothing new — the rule above already covers it.

It is `std::fs`'s shape minus what a stream cannot keep, and the same "looks blocking, is not"
applies: no `async` on the signature, no `await` at the call. On a single-threaded runtime the event loop runs
another task while the pipe is empty; with threads the read may resume on a different one.
What *is* visible is the rule that matters — **a `sync` function cannot call it** (Part II, 12.1),
which is what keeps a `par_iter` body from waiting on a pipe.

`lines()` yields **owned** text, where a file's lines are views into the mapping they came from
(`fs::map(path)` and `.lines()`, above): a file is still there to point at, and a stream's bytes
are gone once consumed. Keeping them would be `read_to_string` with extra steps. It is also what
makes the stream expressible at all — an iterator may hand out views into a buffer it does not
own, and never into one it does ([ADR-025](adr/adr-025.md)).

There is one standard input, so these are functions rather than a handle — a handle that can be
held invites two tasks to hold it, and two readers of one pipe get interleaved halves of lines.
Reading it a second time yields what the operating system says, which is nothing.

**Provenance** follows the rule files follow: **Trusted**, because the operator chose what to
connect to the pipe exactly as they chose which path to open. The case that argues otherwise — a
request body arriving on standard input — is the uploaded-file case and has the same answer in
the same place: `io::read_to_string(trusted: false)`.

Output stays `print` and `println` below; `std::io` is here because there was no way to *read*
standard input.

**Writing output**

`println(text)` writes a line to standard output, `print(text)` writes without the newline, and
`eprintln` / `eprint` are the same two on standard error. They are in the prelude rather than in
a module, because a program that says nothing is rare enough not to plan for.

The argument is an ordinary interpolated string (Part I, 2.5), so a hole is written where the
value goes and `{{` is a literal brace:

```nika
print(f"{name}: ")
println(f"{count} rows")
```

`print` exists for output composed piece by piece — a pretty-printer that indents a tree, a
progress line rewritten in place — where a newline after every fragment would be wrong.
`examples/json.nika` is the first program here that needs it.

**`std::http`**
A production-ready HTTP/1.1 and HTTP/2 server and client.
* **At `user_parallelism = no`:** Runs on a single-threaded Event Loop.
* **At `yes`:** Runs on a multi-threaded Work-Stealing Executor.

```nika
use std::http

fn main() {
    // Starts a server on Port 8080.
    // The code looks the same, but the runtime behavior follows `user_parallelism`.
    // The handler is a trailing lambda, outside the parentheses.
    http::Server::new()
        .route("/") fn { "Hello World" }
        .listen(":8080")
}
```

**The handler and the request** ([ADR-018](adr/adr-018.md)). A handler is a lambda, so the rule
about its arguments is the one Part I 5.3 already gives: it takes as many implicit arguments as
its body reaches for. The first — and only — one is the request.

```nika
.route("/")         fn { "Hello World" }                    // mentions none, takes none
.route("/hello")    fn { "Hello, {a.query("name") ?? "world"}" }
.route("/fortunes") fn(request) { render(request) }         // or name it
```

What a handler returns is what answers the request:

| returns | becomes |
| :--- | :--- |
| `String` | 200, `text/plain; charset=utf-8` |
| `html::Raw` | 200, `text/html; charset=utf-8` |
| `Response` | itself |
| `T throws` | the value on success; on failure **500 with a generic body**, the error logged |

The last row is a decision: an error's message is written for the operator, and a handler that
returns one to the client is how internal paths and driver messages end up in a bug report. A
status code, a header or a body of one's own is a `Response`, built where it is returned —
`http::Response(status: 400, body: "id is required")`.

The request's strings are **views** into the bytes the connection read: `path()`, `header(name)`
and `query(name)` yield `&str`, so a parameter used inside the request's scope costs nothing and
one kept past it has to be owned (Part I, 6.6). `query` and `header` return the nullable type of
Part I 3.5 rather than an empty string, and `method()` returns an enum rather than a string.

A handler does I/O, so it is not `sync`; it carries no `async` marker and no `await`, and the
switch chooses the executor and nothing else.

**`std::html`**

The escaping a template's contract rests on ([ADR-017](adr/adr-017.md)).

```nika
pub fn escape(text: &str) -> String        // for a text node or a quoted attribute
pub struct Raw                             // "this is already markup"
pub fn Raw::new(markup: String) -> Raw     // the audit point, and the only constructor
```

A template grammar escapes **every hole, unconditionally** — there is no flag at a hole that
turns it off, and no exemption for data provenance calls trusted, because provenance is evidence
about where bytes came from and escaping is not a place to spend evidence. The one way to say a
value is already markup is to give it the type `Raw`, so that the decision is made where the
value is built rather than at each of the places it is used.

`escape` handles the five characters that change what HTML means in a text node or a quoted
attribute value — `&`, `<`, `>`, `"`, `'` — and returns its input unchanged when none of them are
present. It does **not** make text safe inside `<script>`, inside CSS, in an unquoted attribute or
in a URL: those need different escaping, which is why a hole in one of those positions is a
compile error naming the position rather than a call to this function.

**The template, and where it is compiled.** `dsl html { … } eod` is compiled *where it is
written*: the body is known when the program is compiled, so it is split into literal markup and
holes there, and what comes out is the string building a hand-written renderer would do. That is
what makes the escaping a compile-time property rather than a call somebody has to remember.

```nika
fn row(name: &str, shade: &str) -> String {
    return dsl html {
        <tr class="{shade}"><td>{name}</td></tr>
    } eod
}
```

`{{` is a literal brace, the same rule an interpolated string follows. The framing whitespace —
the newline after `{` and the indentation before `} eod` — is not markup and is removed;
whitespace inside the body is kept exactly.

What may go in a hole is decided by the **type**, through the `Render` trait: a `Raw` renders
itself, text renders escaped, and a type with no impl cannot be placed in a template at all. The
compiler emits the same call for every hole and has no way to emit a different one — choosing is
what the type does. So this rule holds without the Nikaia compiler having to know what a hole's
value is: it is enforced by the language below, on every hole, including the ones the checker of
[ADR-024](adr/adr-024.md) records as `?`.

**Control flow is written as an element**, because the file is markup and an editor that
highlights it keeps working — a second syntax in a file that already has one is a second thing to
know:

```nika
<table>
<for row in :rows><tr><td>{row.id}</td><td>{row.message}</td></tr></for>
</table>
```

`:rows` carries the colon because it is **captured from the enclosing scope** (ADR-007 D4): that
is where the template's names end and the program's begin. The loop becomes the loop of the
language below, over the captured collection, so it borrows rather than copies exactly as it
would in the function around the template. The position check runs through a loop's body — a hole
in a `<script>` does not become safe by being repeated.

**`std::fs` (Compiler Magic)**
File system access is designed to look **blocking** (synchronous) for ease of use. However, the compiler automatically transforms these calls into **non-blocking** state machines backed by the runtime's reactor. You never block the thread, but you never have to write "callback hell".

Every function below may fail for environmental reasons, so every one of them `throws` (Appendix A.1) — a missing file is not a bug in your program. None of them take an `async` marker and none are awaited; that is the whole point.

**Whole-file access**

```nika
// Subject: the path ; Config: options
pub fn read(path: Path) -> Bytes throws                       // whole file, as bytes
pub fn read_to_string(path: Path) -> String throws            // whole file, UTF-8 validated
pub fn write(path: Path, data: &[u8]; append: bool = false, create: bool = true) throws
```

**What Stage 0 has of this today.** `read`, `read_to_string`, `write` — with both of its options,
since Part I 5.1's `;` section parses — and `map`. `open`/`File` and the directory functions are
not here yet; `lines` and `bytes` are gone for a reason of their own, below.

`read` returns **`Bytes`**, not a `List[u8]`: it is one shared buffer, and slices that outlive its scope are tethered to it (Chapter 6.6 in Part I). This is what lets a parser hand back thousands of names that all point into a single allocation.

**Reading a large file: `map`, and the grammar**

There is no `fs::lines` and no `fs::bytes`, and there will not be. The reasons are stated here because this is where a reader looks for them ([ADR-025](adr/adr-025.md) D3):

* **The specified shape cannot exist.** `lines(path)` was to open the file *and* yield tethered `&str` — so the returned value would own the buffer and hand out views into itself. That is the one thing an iterator may not do, and it is why the language below allocates a string per line when it offers the same function.
* **The shape that works is two calls, and it is the model.** `fs::map(path)` owns the pages; `.lines()` borrows views of them. One value owns a buffer, another borrows from it, and Part I 6.6 and [ADR-008](adr/adr-008.md) rest on keeping those apart.
* **The properties `lines` was for are properties of the mapping**: tethered `&str`, no allocation per line, constant memory. They come from `map`, not from the sequence.

```nika
let data = fs::map(&path)
for line in data.lines() { … }
```

And for a file that is a **record per line**, the language already has something better than a sequence of lines — the grammar protocol, where `@frame(boundary: "\n")` says exactly that and drives itself over the pages, in parallel where `user_parallelism` allows (Part II, 10.7). `examples/1brc.nika`, `examples/access-log.nika` and `examples/config.nika` are all that shape; none of them iterates lines.

The WASM question these functions were the answer to comes back with the target: `map` is a compile error there, and what `std::fs` offers instead on `wasm32-*` will be decided with it.

**Handles**

```nika
pub fn open(path: Path; write: bool = false, append: bool = false,
            create: bool = false, truncate: bool = false) -> File throws
```

`File` implements `Cleanup` (Part I, 6.4): the compiler flushes and closes it at the end of the scope, on the normal path *and* while an error is bubbling up, and a flush that fails surfaces as an error instead of being swallowed. Call `close()` explicitly only when you want to handle that error at a precise point.

```nika
impl File {
    pub fn read(&mut self, into: &mut [u8]) -> usize throws
    pub fn write(&mut self, data: &[u8]) -> usize throws
    pub fn flush(&mut self) throws
    pub fn seek(&mut self, to: Seek) -> u64 throws   // Seek::Start(n) | Current(n) | End(n)
    pub fn len(&self) -> u64 throws
    pub fn close(self) throws                        // explicit opt-in; otherwise Cleanup does it
}
```

**Memory mapping**

```nika
pub fn map(path: Path) -> Mapped throws          // read-only memory map
```

`Mapped` derefs to `Bytes`, so a mapped file is a tethered buffer like any other and a parser cannot tell the difference. This is what makes a multi-gigabyte input practical: the pages are the buffer, and nothing is copied.

**Retention.** A slice that escapes the mapping's scope tethers to it, and a tether keeps the *whole* map alive — one twelve-byte station name can pin thirteen gigabytes. The compiler warns where a small extract outlives a large buffer and suggests `.to_owned()`; the mapping is released once the last tether is gone, which may be later than the end of the block that created it ([ADR-008](adr/adr-008.md), D8). Slices that never leave that scope cost nothing and hold nothing. At process exit a read-only mapping with nothing observable attached to it is simply left to the operating system rather than unmapped page by page ([ADR-009](adr/adr-009.md), D7) — at thirteen gigabytes that teardown is measurable, and skipping it changes nothing a program can see.

**Availability is a property of the target, and of nothing else.** Memory mapping is an operating-system service, and whether it exists has nothing to do with whether the runtime is single-threaded. `fs::map` is therefore available at **every** setting on any target whose platform provides it — a single-threaded program compiled for Linux, macOS or Windows maps files exactly like a parallel one. What rules it out is a target without the service: on `wasm32-*` there is no memory mapping to call, so `fs::map` is a **compile-time error** there.

The error is deliberate rather than a silent fallback to `read`: degrading a memory map into a full read turns a constant-memory program into one that allocates its entire input, which is a failure the program would only discover in production. Code that must build for every target, WASM included, uses `lines` or `bytes` — constant-memory everywhere.

**Metadata and directories**

```nika
pub fn exists(path: Path) -> bool throws
pub fn metadata(path: Path) -> Metadata throws   // len, is_dir, is_file, modified
pub fn read_dir(path: Path) -> DirEntries throws
pub fn create_dir(path: Path; recursive: bool = false) throws
pub fn remove(path: Path; recursive: bool = false) throws
pub fn rename(from: Path, to: Path) throws
pub fn copy(from: Path, to: Path) -> u64 throws
```

**Availability by target**

Every setting has the same `std::fs` surface; only the target changes it.

| API | Native | `wasm32-*` |
| :--- | :--- | :--- |
| `read`, `read_to_string`, `write` | yes | yes — backed by OPFS |
| `open` | yes | yes — backed by OPFS |
| `map` | yes | **compile error** — the platform has no memory mapping |
| `metadata`, `read_dir`, `create_dir`, `remove`, `rename`, `copy` | yes | yes — OPFS, within the origin's sandbox |

This is the difference between `fs::map` and `std::thread` (17.2). `std::thread` is barred by **`user_parallelism = no`**: it is share-nothing by design, so manual threading is a compile error even on a native target that has threads. `fs::map` is barred by the **target**: nothing about a single-threaded runtime prevents mapping a file.

**`std::collections` — and where your keys came from**

A hash map can be attacked. If someone else chooses the keys, they can choose keys that collide, and a table that should answer in constant time starts answering in quadratic time. The defence is a hash function with a secret, random key — and it costs a little on every lookup, which is why languages that cannot tell the two situations apart make everybody pay it.

Nikaia can tell them apart, because it knows where a buffer came from (Chapter 6.6 in Part I). Every input carries a **provenance**:

* **Untrusted** — a remote peer chose these bytes: HTTP requests, sockets, IPC, rows read back from a database (data a user stored yesterday is still data a user chose).
* **Trusted** — the operator chose these bytes: files, command-line arguments, the environment, anything compiled in.

Values inherit the provenance of the buffer they come from, and a collection takes the *most cautious* provenance of everything put into it. Where the compiler cannot tell — across a dynamic call, or from a foreign library — the answer is Untrusted. Guessing "safe" is not a guess a compiler may make.

From that, the hasher follows: untrusted keys get a keyed hash with a per-process random seed, trusted keys get a fast one. Nothing else about the map changes — same table, same API, and keys are always compared in full.

**You have the last word, at the place the data enters:**

```nika
// A service that processes files uploaded by strangers:
// a local path, but bytes nobody vetted.
let data = fs::map(path; trusted: false)
```

The reverse (`trusted: true`) exists for the case where you know the peer. Both are recorded in `nikaia.contracts`, so "every place this program declared something safe" is one list in one file, and it shows up in review when it changes. A grammar for a wire format can also pin the floor for everyone who uses it — `@untrusted grammar HttpHeaders` — so no application can lower it by accident (Part II, 10.7 and [ADR-010](adr/adr-010.md)).

`nikaia --trust` prints where the program's bytes came from, which source said so, and which hasher its maps got:

```text
$ nikaia --input 1brc.nika --backend rust --trust
input provenance: trusted
    cli::args is trusted
    fs::map is trusted
hash for a map keyed by the input: fast, fixed seed - no adversary chooses these keys
```

What the bootstrap compiler's analysis is, exactly, so that a later one is not mistaken for it: **one buffer**, because ADR-008 gives a compilation unit one input lifetime — so the join over the sources a program calls *is* the per-buffer answer, for the one buffer this representation can express. The build cache needs no separate key field for it: provenance is a function of the source and of what `std`'s ledger says about the sources that source calls, and the compiler's fingerprint already hashes that ledger (13.6). And because untrusted maps are seeded randomly, **iteration order is not stable between runs** — when order matters, ask for it explicitly rather than relying on what a map happens to do today.

**Other Key Modules:**
* **`std::json`**: High-performance serialization using compile-time code generation (zero-allocation parsing where possible).
* **`std::cli`**: Parsers for command-line arguments, environment variables, and ANSI terminal colors.
* **`std::net`**: Low-level TCP/UDP sockets for building custom protocols.

### 17.2. Availability by Target and by `user_parallelism`
Some modules are only available, or behave restrictively, depending on the machine and on how much of your code may run at once.

* **`std::process`**: Spawning child processes.
* **`std::thread` / `spawn`**:
    * **At `user_parallelism = yes`:** Supports full concurrency. The primary mechanism is `spawn`.
        * **Strict Implicit Move:** To ensure thread safety without complex lifetime tracking, Nikaia enforces **Implicit Move Semantics** for all tasks spawned this way. Ownership of variables used inside the `spawn` block is automatically transferred to the new thread.
    * **At `user_parallelism = no`, and on `wasm32-*` whatever it says:** Direct usage of `std::thread` is a **compile-time error**. A share-nothing architecture is what makes `user_parallelism = no` mean something, and what keeps a program compatible with WASM hosts.

**`std::db` (Universal SQL)**
Nikaia provides a unified SQL interface, starting with SQLite, designed to abstract the underlying platform constraints completely.

* **Zero-Blocking Guarantee:** Database operations are implicitly asynchronous. They never block the Event Loop, nor the Compute Scheduler where there is one.
* **Architecture Adapter:** The implementation switches automatically based on the compilation target:
    * **Native Targets:** Utilizes a dedicated, hidden I/O thread (powered by `tokio-rusqlite`) to offload blocking filesystem operations.
    * **WASM Targets:** Automatically spawns a **Web Worker** and utilizes the **OPFS** (Origin Private File System). This enables native-grade, persistent SQL performance in the browser without freezing the UI thread.

```nika
use std::db::sqlite

fn query_data() {
    // Transparently starts the required Sidecar (Thread or Worker)
    let db = sqlite::open("app.db")
    
    // The 'sql' macro validates syntax at compile-time.
    // At runtime, it performs an async round-trip to the sidecar.
    let active_users = dsl sql db {
        SELECT * FROM users WHERE last_login > 0
    } eod
}
```

---

# Appendix A: Error Hierarchy

Nikaia strictly distinguishes between errors caused by the environment (recoverable) and bugs in the program logic (unrecoverable).

### A.1. Recoverable Errors (`throws`)
Errors arising from external circumstances (File not found, Network timeout).
* **Mechanism:** Must be declared in the function signature via `throws`.
* **Handling:** Enforced by the compiler via `catch{}` blocks or propagation.

### A.2. Unrecoverable Errors (`panic`)
Errors indicating an inconsistent program state (Index Out of Bounds, Division by Zero, explicit `panic()`). What a panic does depends on **both** switches, and on different grounds:

| `user_parallelism` | Panic Behavior | Consequence |
| :--- | :--- | :--- |
| **`no`** | **Abort** | The process terminates immediately. There is no second piece of your code in flight to isolate the failure from, so unwinding would buy nothing and is not done — which also leaves a smaller binary. |
| **`yes`** | **Task Poisoning** | Only the affected task is terminated. The worker thread catches the panic (Fault Isolation). Resources (`Locked[T]`) held by the task are marked "poisoned" so no other thread reads state a half-finished task left behind. |

The `target` decides this independently where the machine leaves no choice: on
`wasm32-unknown` a panic is a **trap** and the module is done, whatever
`user_parallelism` says, because the host offers nothing to unwind to.

On **every** panic path — including the abort and the WASM trap — the application's **Panic Hook** runs first (Part I, 7.2): one global, `sync` handler receiving message, location, and stack trace, intended for crash dumps and reports. This rides on the backend's panic machinery, which invokes the hook before aborting even under `panic = abort`. See [ADR-006](adr/adr-006.md), D6.

# Appendix B: Compiler Internals & Annotations

To enforce the "Contextual Capture" rules (Chapter 5.4) without hard-coding specific function names into the compiler, Nikaia uses internal attributes. These are primarily used by the Standard Library but are available to library authors.

### B.1. Capture Attributes

| Attribute | Internal Name | Default | Description |
| :--- | :--- | :--- | :--- |
| None | `capture_mode = "immediate"` | Yes | The lambda executes within the caller's stack frame. Captured variables are **Borrowed** (`&T`). Used by `map`, `filter`, `lock.access`. |
| `@detached` | `capture_mode = "detached"` | No | The lambda escapes the current stack frame (stored, spawned, or deferred). Captured variables are **Moved** (Owned). Used by `spawn`, `defer`. |

### B.2. Standard Library Signatures

Here is how common standard library functions are annotated internally to drive the compiler's behavior:

```nika
// std::collections::List
// Standard immediate execution
pub fn map[U](self, op: fn(T) -> U) -> List[U]

// std::task (Global Spawn)
// Detached execution: Must take ownership of environment
pub fn spawn(task: @detached fn() -> T) -> TaskHandle[T]

// std::task
// Scope is immediate because it waits for completion
// Where they run in parallel, a scope's child tasks must be 'sync' (see Part II, 12.7)
pub fn scope(f: fn(Scope))
```

# Appendix C: The Diagnostics Contract

Nikaia compiles through the Rust toolchain ([ADR-003](adr/adr-003.md)), but the Rust compiler's error messages — lifetimes, borrow traits, generated code — are exactly the vocabulary Nikaia promises its users they never need. This appendix makes diagnostic quality a **testable requirement**, not an aspiration. Full rationale: [ADR-005](adr/adr-005.md), D7.

### C.1. The Iron Rule

> **An untranslated backend (rustc) error reaching the user is a Nikaia compiler bug.**

The driver registers its own diagnostic emitter and intercepts every backend diagnostic. Each borrow/ownership/lifetime error class is mapped to a Nikaia diagnostic with Nikaia vocabulary, `.nika` spans, and a concrete fix-it. If an unmapped error surfaces, the compiler reports it as an internal error ("this is a Nikaia bug, please report") — never as normal output. Consequence: the error catalogue below doubles as a test suite; every entry has a minimal `.nika` reproduction that must produce the documented message.

### C.2. Requirements for Every Diagnostic

1. **No prior knowledge assumed.** The message must be understandable without Rust or systems-programming background. Terms like "lifetime", "borrow checker", or Rust error codes never appear.
2. **Always say what to do next.** Every error names at least one concrete way out (clone, use `Shared`, use `retain`, mark a function `sync`, move the I/O out of the lock, …), ideally as paste-ready code.
3. **Narrate cause chains.** Errors caused by a *change* (via the Borrow Contract Ledger, 13.5) show both sides: the edit that changed the contract and the caller that broke.
4. **Positive guarantees over prohibitions.** Where the language removes a danger structurally (tethered slices, scope waiting), the docs and messages state the guarantee ("the buffer cannot die while a token lives"), not the forbidden thing.

### C.3. Error Code Catalogue (NK codes)

| Range | Domain | Examples defined so far |
| :--- | :--- | :--- |
| `NK1xxx` | Syntax & types | `NK1101` a call passes the wrong number of arguments. `NK1102` an argument is not what the parameter takes. `NK1103` a `let` says one type and is given another. `NK1104` a `return` - or a body's last expression - is not what was declared. `NK1105` an assignment is not what the target holds. `NK1106` a struct literal gives a field the wrong type. `NK1107` a field that is not there. `NK1108` a condition that is not a `bool`. `NK1109` a call names an option the callee does not have (Part I, 5.1). `NK1110` a call reaches an item another file keeps private (Part I, 9.2). All ten are answered from the ledger (13.5), so a call into a library is checked against the contracts the library ships ([ADR-024](adr/adr-024.md)). `NK1111` (**warning, and temporary**) a plain string holds what looks like a hole, or a doubled brace that used to be an escape - the one-release migration to `f"…"` ([ADR-035](adr/adr-035.md) D5), and the only thing this checker warns about rather than refusing. `NK1112` a call does not pass a parameter the DSL statement it is given declares, and `NK1113` names one that statement does not have (Part II, 10.5) - answered from the statement's own body rather than from the ledger, because the body is where a `:name` is written ([ADR-007](adr/adr-007.md) D5). |
| `NK21xx` | Tasks & capture | `NK2101` task takes ownership of a variable still used afterwards (Part I, 8.3). `NK2102` scoped tasks must be `sync` where they run in parallel (Part II, 12.7). |
| `NK22xx` | Locks & suspension | `NK2201` no I/O while holding locked data (Part II, 12.2). `NK2202` a `sync` function called something that can pause (Part II, 12.1), answered from the ledger (13.5). |
| `NK23xx` | Aliasing | `NK2301` cannot change a collection while looping over it (Part I, 6.8). |
| `NK24xx` | Contract changes | `NK2401` a borrow contract change broke a caller, narrated from the ledger diff (13.5). Reserved: a `catch` that no longer covers every error that can reach it, narrated from the same diff — it needs the ledger to record the *set* rather than a boolean ([ADR-023](adr/adr-023.md) D1), which needs error types the compiler can lower. |
| `NK25xx` | Portability | Reserved: the `Send` rules that parallel code needs, reported at `user_parallelism = no` as a lint, so a library built there stays usable at `yes`. |
| `NK26xx` | Resource cleanup & crash path | `NK2601` function must declare `throws` because a resource's implicit cleanup can fail (Part I, 6.4). `NK2602` a resource with pausable cleanup must not go out of scope in a `sync` context. `NK2603` (warning) cleanup-deadline exceeded at shutdown; lists the resources that did not finish cleanly. `NK2604` only the application may set the panic hook, and the hook must be `sync` (Part I, 7.2). |
| `NK27xx` | Implicit calls | `NK2701` a loop whose step can fail, in a function that does not declare `throws` ([ADR-025](adr/adr-025.md) D5). The same rule as `NK2601` one line earlier in the block: where the language performs a call nobody wrote, a failure of it fails the enclosing function. |

The catalogue grows with the implementation; adding an NK code requires adding its reproduction test and its worked example to the relevant spec chapter.

### C.4. What a Type Error Looks Like

Two of the `NK1xxx` family, on a file that says `io::read_to_string("input.txt")` and puts a literal in a `String` field:

```text
error[NK1101]: `io::read_to_string` takes 0 arguments, and this call passes 1
  --> app.nika:11:5
  11 |     let text = io::read_to_string("input.txt")?
           ^
     = `io::read_to_string() -> String`
     help: call it as `io::read_to_string()`
error[NK1106]: `Reading.name` is `String`, and this is `&str`
  --> app.nika:12:5
  12 |     let r = Reading { name: "Hamburg", temp: 12 }
           ^
     help: write `.to_string()` to make a `String` of it
```

Three things about that shape are deliberate. **The note is the contract**, quoted from the ledger — the compiler shows the caller what the callee promised, because that is the fact the caller was working from. **The caret is on the statement**, not the expression: expression-level spans are open work, and both this checker and `NK2202` report at statement granularity until they exist ([ADR-024](adr/adr-024.md) D7). And **the help is paste-ready**, as C.2 requires: `.to_string()` for text, `as i64` between numbers, and the field you probably meant when a name is close to one that exists.

A message appears only where **both** sides are written down. Where a type is not known — a method on a receiver `std` has no signature for, what a `?` unwraps — the compiler says nothing, which is not the same as approving. That is the property that lets the checker be run on every build: it never rejects a program that is correct.

**A hole is code, and is checked like code.** The expression inside `"total is {stock::total(items)}"`
— and inside a `dsl html` template's `{…}` — goes through the same path as a statement, so
`NK1101` and the rest say the same thing about it that they would say about the same expression
written on a line of its own ([ADR-032](adr/adr-032.md) D3). The `sync` analysis reads holes too,
in both directions: a pausing call inside one costs an inferred `sync` and contradicts an asserted
one. A hole whose text does not parse is reported by the emitter, which has the span, and the
checker stays quiet about it rather than raising a second error for one mistake.


