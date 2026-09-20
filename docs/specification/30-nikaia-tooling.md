# Nikaia Language Specification
**Part III: Tooling, Ecosystem & Interoperability**
**Version:** 0.0.94 (Draft)
**Date:** 2026-09-20

---

## Chapter 13: The Toolchain (CLI)

Nikaia provides one command-line interface, `nikaia`. It builds and runs a project, manages its dependencies, formats its source and runs its tests.

**Prerequisites.** A Nikaia installation requires a **stable** Rust toolchain and nothing else. The compiler emits stable Rust and hands it to `cargo`; no unstable compiler feature and no `-Z` flag is used ([ADR-001](adr/adr-001.md) D1, [ADR-004](adr/adr-004.md) D1). The emitted Rust requires Rust **1.75 or newer**; the floor is set by the lowering of `async fn` in a trait. The compiler writes that number as `rust-version` into every generated `Cargo.toml` and compares it with `rustc --version` before `cargo` runs. An older toolchain is refused by the compiler, before the backend runs ([ADR-109](adr/adr-109.md) D4).

### 13.1. Project Structure
`nikaia new my_project` generates the following structure:

* `nikaia.toml`: the **manifest**. It describes the project, its authors and its dependencies.
* `nikaia.lock`: the **lockfile**. It records everything that determines the build and is therefore also the **cache key** ([ADR-021](adr/adr-021.md)).
    * **Asset hashing:** where a grammar reads an external file (`asset("schema.sql")`, [ADR-116](adr/adr-116.md)), the compiler records the file's SHA256 hash here. The lockfile is thereby also the **allowlist** that permitted the read ([ADR-072](adr/adr-072.md) D7). A build given no allowlist reads nothing at compile time (D1).
    * **Source hashing:** the SHA256 of each `.nika` source that took part. An unchanged module skips parsing and expansion.
    * **Resolved versions:** the exact dependency versions, the toolchain version used, and the Nikaia compiler's own version (ADR-021 D3). The dependency versions are recorded without being hashed into the key: a dependency bump changes the machine code Cargo produces, never the Rust the compiler emits, and Cargo's own fingerprinting covers that half (ADR-021 §5).
    * **Declaration and record:** `nikaia.toml` states what the project requires; `nikaia.lock` records what was resolved and used. This is the relationship `Cargo.toml` has with `Cargo.lock`.
    * **Not in the lockfile:** the build options of Part I 1.2, `opt-level` and the backend. They are hashed into the cache key and never written (ADR-021 D5).
    * **Incremental builds:** where the hashes on disk are unchanged, the compiler skips re-processing and reuses the artifact from the content-addressed store under `target/nikaia/cache/`. The store is git-ignored; the lockfile holds inputs and the store holds outputs. Keys are per translation unit, so one changed asset invalidates that unit and not the project (ADR-021 D6).
* `nikaia.contracts`: the **borrow contract ledger**. It is generated and committed like the lockfile. It records the borrow contracts the compiler inferred for the program's functions and the tether relationships of its structs. It is an incremental-build cache and the basis of the compiler's contract-change diagnostics (13.5).
* `src/`: the source.
    * `main.nika`: the entry point.

*Design rationale:* a reproducibility record that omits an input cannot report that it is incomplete, so the lockfile is one file; a changed emitter produces different output from identical input, so the compiler's own version is in the key ([ADR-021](adr/adr-021.md) D3).

### 13.2. Core Commands
* `nikaia build`: compiles the project.
* `nikaia run`: compiles and executes.
* `nikaia test`: runs unit tests and fuzzers.
* `nikaia bench`: runs performance benchmarks.
* `nikaia fmt`: formats the source.
* `nikaia describe <crate>`: writes the draft ledger for a Rust crate the program calls (15.2, [ADR-104](adr/adr-104.md)).
* `nikaia bind <language>`: writes a binding for a library build from the ledger: `python` over `ctypes`, `js` for a WebAssembly build (15.1, [ADR-131](adr/adr-131.md)).

**Backends.** `nikaia build`, `nikaia run` and a single-file `nikaia --input <file>.nika` that names no `--backend` compile through the `rust` backend, the Stage 0 transpiler. It is the default, the only code generator, and part of every installation ([ADR-004](adr/adr-004.md) D1). `--backend interpreter` runs a program instead of producing one. `cranelift` and `llvm` are named by [ADR-002](adr/adr-002.md) and not implemented; a build that names one is refused ([ADR-021](adr/adr-021.md) D9).

> **Implementation status:** Partially implemented. `build` and `run` are implemented, over `nikaia.toml` translated to a `Cargo.toml`, and so are the `rust` and `interpreter` backends ([ADR-002](adr/adr-002.md) D1 §5). `new`, `test`, `bench`, `fmt`, `describe`, `bind` and the `explain` of Part I 6.6 and 7.1 are not implemented; the binary's subcommands are `build`, `run` and `lower-std` (13.2b). A single file outside a project is compiled with `nikaia --input <file>.nika`, which is not a subcommand ([ADR-021](adr/adr-021.md) D11).

### 13.2b. Where `std` Comes From (the Sysroot)

`std` is not a package a registry resolves, and it is not reached the way 13.3's
`type = "rust"` dependencies are. A generated project depends on it by path into
a **sysroot**: a directory that travels with the compiler and holds `std`'s
sources. `NIKAIA_SYSROOT` names the sysroot. By default it is the checkout the
compiler was built from, so a build inside the repository needs no configuration
([ADR-002](adr/adr-002.md) D4).

* **`std` ships as sources, with its Nikaia half already lowered.** The parts of
  `std` written in Nikaia ([ADR-014](adr/adr-014.md) D1) are lowered to Rust at
  release time, and the `.rs` sits beside the `.nika` it came from. Building
  `std` therefore needs nothing but `rustc`; the compiler is not in a project's
  build graph. `nikaia lower-std` re-lowers it by invoking the compiler binary.
* **`std` is compiled once per machine, not once per project.** The compiled
  `std` lives in the user's cache directory, in an entry keyed by the compiler's
  own fingerprint, the toolchain, the `target` and the codegen flags of 13.3's
  `[build.<target>]` table. Two builds that differ in any of those keep their own
  entry ([ADR-021](adr/adr-021.md) D7). `NIKAIA_CACHE_DIR` moves the cache.
  `CARGO_TARGET_DIR` disables it: a build that asked Cargo for a directory gets
  it.
* **The newest three entries are kept; idle entries below that are removed.**
  Each entry is a whole Cargo target directory, and a rebuilt compiler starts a
  new one, so the number of entries is bounded ([ADR-021](adr/adr-021.md)
  D12.5). A tree written to within the hour is never removed, so a build running
  beside this one keeps the directory it is using.
* **`user-parallelism` is not one of those keys.** The build option reaches
  `std` as a value its runtime is started with, never as a compile-time
  condition, so one compiled `std` serves both values
  ([ADR-037](adr/adr-037.md) D2).
* **Nothing in `std`'s Nikaia half may lower differently per build option.**
  The Nikaia half is lowered at one value of every build option. `Shared` is
  therefore ruled out of it: the emission of a `Shared`'s count follows
  `user-parallelism` ([ADR-037](adr/adr-037.md) D3, D6,
  [ADR-061](adr/adr-061.md) D2), so a `Shared` in a `.nika` file of `std` would
  be lowered once with the cheap count and handed to a program built at `yes`.
  `Locked` is ruled out for the same reason (Part II, 12.2). The toolchain checks
  the constraint: every module is lowered at both values, the bytes must agree,
  and a difference fails the toolchain's own build.
* **`std`'s ledger travels inside the compiler.** `std.contracts` (13.5) is part
  of the compiler rather than of the sysroot copy it is read from. A ledger is
  not required to be stable across toolchain versions
  ([ADR-005](adr/adr-005.md) D8), so a `std` paired with a different compiler
  would describe a compiler that is not there.

> **Implementation status:** Partially implemented. The layout, `NIKAIA_SYSROOT`, the per-machine cache and `nikaia lower-std` are implemented. The packaging step that produces a sysroot outside a checkout is not implemented ([ADR-002](adr/adr-002.md) §5).

### 13.3. Manifest Configuration (`nikaia.toml`)
The manifest defines the project's metadata and the build options of Part I 1.2.

The build options live in `[build]`. `--target` and `--user-parallelism`
override `target` and `user-parallelism` for a single build; the committed value
is the one a reviewer sees (ADR-037 D5). `reentrancy-check` is read from the
manifest. A key `[build]` does not know is a typo and fails the build.

The manifest carries what the **compiler** must know. How the program behaves
on the machine it runs on — the number of I/O workers, the size of the pool for
user code, the I/O mechanism, the shutdown drain — is runtime configuration
(13.3b), read at startup by whoever runs the program
([ADR-038](adr/adr-038.md) D5).

`reentrancy-check` is a build option, and `cleanup-deadline` is runtime
configuration ([ADR-039](adr/adr-039.md) D8). `cleanup-deadline` changes what a
running program waits for; the compiler does not read it. `reentrancy-check`
changes what the compiler emits; it is a cache-key dimension like `target` and
`user-parallelism` ([ADR-037](adr/adr-037.md) D4).

*Design rationale:* a build option the compiler acts on is the compiler's to
read, and a build whose options are not in the committed file cannot be
reproduced from it ([ADR-039](adr/adr-039.md) D8).

```toml
[package]
name = "hyper-core"
version = "0.1.0"
authors = ["dev@nikaia.org"]

[build]
# The machine the program is built for (ADR-037 D1). `wasm32-unknown` has no
# threads and traps rather than unwinding; the target decides the panic
# strategy and what `std` offers.
target = "x86_64-linux"

# What is made: a program (the default), or a library other languages call.
# `artifact = "c-library"` exports every `pub extern "C" fn` with a body and
# generates the header (ADR-125 D1). With `target`, this is what ADR-062
# means by a target that lets foreign code call in.
# artifact = "program"
# The prefix of every exported symbol and constant. The default is the package
# name with `-` written `_` (ADR-128 D1).
# symbol-prefix = "hc"

# Whether user code may run concurrently (ADR-037 D2). A permission, not a
# count; how many threads serve a "yes" is the runtime's decision.
#   "no"  (default) - no two pieces of user code are ever in flight at once
#   "yes"           - user code may run concurrently
# The option bounds user code, not the compiler or the runtime: reading a file
# may validate its text on several cores at "no", because that is not user
# code and changes nothing the program prints.
user-parallelism = "no"

# `cleanup-deadline` is runtime configuration (13.3b, ADR-038 D5): how long a
# program waits at exit is a property of the machine it runs on. A manifest
# that carries the key compiles, with a note naming where it went.

# `ordering` is withdrawn (ADR-050 D1, D7): statements run in the order they
# are written, and a program that wants overlap writes `overlap { … }`
# (Part I 8.1.2). A manifest that carries the key fails the build, with a
# message naming the withdrawal and the replacement. A moved key keeps working
# and says where to look; a withdrawn key decides nothing and is not ignored.

# Whether the compiled program notices a lock taken while a lock is held
# (ADR-039 D8).
#   "on" (default) - the check is emitted, and a violation panics where it happens
#   "off"          - the check is not emitted
# Taking a lock inside a lock is refused at build time (Part II 12.3,
# ADR-039 D2), so in a program the compiler accepted the check cannot fire; it
# guards a hole in that refusal. Every program the refusal accepts behaves the
# same at both values. The option decides only whether a violation is noticed.
# It is a guarantee that may be declined, not a development aid.
reentrancy-check = "on"

[dependencies]
# A Nikaia package by path (ADR-047 D2). The key is the name a `use` writes;
# the path says where the package comes from, so two packages that both want
# to be `http` are told apart by the key. The package's files are read into
# this program: `pub` is what it offers, its own `[build]` is ignored with a
# note (a package is built with the options of the program that uses it), and
# the overflow checks of A.2 reach it, because a Nikaia dependency is part of
# the program rather than a foreign package.
http = { path = "../http" }
# A Nikaia package by version resolves through Cargo (ADR-103): on crates.io
# it is the crate `nikaia_http_server`, and the generated `Cargo.toml` writes
# the rename, so the prefix never reaches a `.nika` file. `"1.2"` is Cargo's
# semver; a `git` table with a `tag` is the same arm without an index. The key
# is an identifier, because it is the `use` name.
http_server = "1.2"
# A native Rust crate. The entry reaches Cargo with only `type` removed, and
# Cargo resolves, fetches and links it as for any Rust project.
regex = { type = "rust", version = "1.5" }

# Code generation, per target. These keys decide output size and speed and
# change nothing a program means, so they are tables under `[build]` rather
# than build options in it. The table of the chosen target becomes the
# generated `Cargo.toml`'s profile (ADR-002 D1). The panic strategy is not
# here: it follows from `target`.
[build.wasm32-unknown]
opt-level = "z"     # Optimize for binary size

[build.x86_64-linux]
opt-level = 3       # Maximize throughput
lto = true          # Link Time Optimization
```

> **Implementation status:** Partially implemented. `target` and `user-parallelism` are read; a manifest that carries `cleanup-deadline` compiles with a note naming where it went, and one that carries `ordering` is refused as withdrawn. A `path` dependency is read one level deep: a package that declares Nikaia dependencies of its own is refused rather than resolved ([ADR-047](adr/adr-047.md) §5). `reentrancy-check` is not implemented: `[build]` does not know the key, so a manifest that writes it fails the build as an unknown key, and the check it would control is not emitted ([ADR-039](adr/adr-039.md) §4, Part II 12.2).

### 13.3b. Runtime Configuration (`nikaia-runtime.toml`)
The runtime configuration is set by whoever runs the program and is read when
the program starts. It has four keys ([ADR-038](adr/adr-038.md) D5):

```toml
# The number of I/O threads the runtime runs. One always runs. What runs on it
# is `std`'s own code and never user code, so it exists at
# `user-parallelism = "no"` too (ADR-037 D2, ADR-038 D4). More than one lets a
# pair of operations overlap on a machine with no completion queue.
io-workers = 1

# The size of the pool for user code at `user-parallelism = "yes"`. "0" means
# as many threads as the machine has. The key is read at both values of the
# build option and used at one; a key that is present is never dropped
# silently.
user-pool = 0

# The mechanism that serves a file (ADR-038 D3).
#   "auto"     (default) - the kernel completes the operation where the machine
#                          can, and the blocking path serves it where it
#                          cannot. Decided when the program starts, never when
#                          it was compiled: a binary built on a machine with
#                          io_uring runs on one without.
#   "uring"              - pinned to completion. A machine without it refuses
#                          to start; there is no silent fallback.
#   "blocking"           - pinned to the blocking path, whatever the machine has.
io-method = "auto"

# How long the runtime waits at program end for pending resource cleanups
# (flushes, rollbacks, connection shutdowns; Part I 6.4 and ADR-006 D5). The
# default is "30s". On expiry the remaining cleanups are cancelled (their
# synchronous fallback runs), and the program ends with exit status 70 and a
# message naming every resource that did not finish cleanly, on stderr and
# through the panic hook, never on stdout and never as a 0 (ADR-112). "0"
# disables draining. The deadline cannot hang: the timer runs in the runtime
# itself, and cancelling a cleanup always terminates, because the fallback
# cannot pause.
cleanup-deadline = "30s"
```

The file has these four keys and no other. A key outside them is a typo and
fails, as under `[build]`. The file is read from the working directory;
`NIKAIA_RUNTIME_CONFIG` names a file outright, for one binary in several
deployments. Without a file, the defaults above apply.

A build option is not runtime configuration: `target` and `user-parallelism`
change what the program means and stay in `nikaia.toml` (ADR-037 D5).

> **Implementation status:** Partially implemented. `nikaia_std::rt` reads the four keys; `cleanup-deadline` bounds the drain of pending I/O, and an expiry ends the program with exit status 70 on the panic path ([ADR-112](adr/adr-112.md)). The parked-cleanup queue of [ADR-006](adr/adr-006.md) D3 is not implemented, so the message counts abandoned I/O operations and does not yet name resources; under `panic = "abort"` the process ends with the abort's own status and keeps the message. The file's name and search path are not decided by a record.

### 13.4. Build Scripts (`build.nika`)
A project that requires custom build steps, such as compiling C code or generating protocol files, places a `build.nika` file in its root. The script is compiled and executed **before** the main build.

The script has access to the `std::build` API, which emits instructions to the compiler.

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

Nikaia source contains no lifetime annotations (Part I, 6.5; [ADR-005](adr/adr-005.md)). The compiler infers a **borrow contract** for every function whose signature involves references, such as *the result of `longest(a, b)` borrows from `a` or `b`*, by analysing all `.nika` sources of the package as one graph. The result is written to a generated, human-readable file in the project root, **`nikaia.contracts`**: the ledger.

**File format (illustrative):**

```toml
# AUTO-GENERATED by `nikaia build`. Commit this file like a lockfile.
# Do not edit by hand — it is regenerated on every build.
version = 1
toolchain = "nikaia 0.1.0"

# What these entries were derived from; a consumer believes them while it holds.
[sources]
"src/text.nika" = "sha256:9f3a…"
"src/main.nika" = "sha256:41c0…"

[fn."text::longest"]
returns = "borrows(a | b)"

[fn."http::Request::header"]
returns = "borrows(self)"

[struct."parser::Token"]
tethered = ["text -> source buffer"]
```

*Design rationale:* a contract is derived from the function body, and derived truth written into source goes stale. A diffable artifact gives the compiler a memory: it compares what a contract was with what it is and explains the difference ([ADR-005](adr/adr-005.md)).

**Build semantics.** On every build the compiler infers fresh contracts and diffs them against the ledger:

1. **Unchanged:** the fast path. Callers of unchanged contracts are not re-checked; the ledger acts as an incremental-compilation cache key. This mechanism is distinct from `nikaia.lock`'s ([ADR-021](adr/adr-021.md) D10): the lock is consulted before any work and decides whether a build starts at all, and the ledger is compared after inference has run and decides which callers are re-checked. The two stay separate files: the ledger ships with a published package and a lockfile does not, and `--locked` means regenerate-and-compare for the ledger and do-not-re-resolve for the lock.
2. **Changed, all callers still valid:** the ledger is updated and the build proceeds. The change is noted in the build output.
3. **Changed, and a caller breaks:** the compiler uses the diff to narrate the cause chain:

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

**Trait methods.** Dynamic dispatch requires one contract per trait method. The ledger stores the join of all implementations. An implementation that broadens the contract produces a ledger diff and, where callers break, the same narrated error.

**What the ledger records.** The ledger answers one question: *what must a caller know about a body it cannot see*. Borrows are one answer; whether a function may pause and whether it may fail are two more ([ADR-020](adr/adr-020.md) D2):

| key | on | meaning |
| :--- | :--- | :--- |
| `pub` | fn, type | reachable from outside the unit that declares it |
| `sync` | fn | Part II 12.1: pure computation, cannot pause, cannot do I/O. `true` where the source asserted it, `"inferred"` where the body implies it ([ADR-027](adr/adr-027.md)), `"from(f)"` where the lambda it is given decides ([ADR-029](adr/adr-029.md)) |
| `throws` | fn | Part I 7.1: it may fail, and **with what**: `throws = ["ConfigError", "IoError"]`, the set inferred over the call graph ([ADR-023](adr/adr-023.md) D1). The set names error **types** and never one of their variants, on either side of D4's two axes ([ADR-157](adr/adr-157.md) D4). A member written `"?"` is *something this compiler cannot name* — an unresolved call, or a call into code no ledger describes. **`std`'s own entries all name what they throw** ([ADR-158](adr/adr-158.md) D1): `io::IoError` for the seven that read, write or check text, `Overtaken` for the lock's two doors. Where the set has exactly one member and the member is a type **any ledger describes** — the unit's own, or `std`'s ([ADR-159](adr/adr-159.md) D1) — the **failure channel is that type**; where it has two or more, the channel is a **generated sum** over them ([ADR-160](adr/adr-160.md) D1). Either way a `catch` matches on the members' variants ([ADR-157](adr/adr-157.md) D1). A set with a `"?"` in it travels in one opaque error, which is what *something this compiler cannot name* leaves |
| `returns` | fn | what the result may point into: `borrows(a \| b)` |
| `signature` | fn | its parameters, its **options** and its result, as the source writes them: `"(path: ?, data: ?; append: bool = false, create: bool = true)"`. An option carries its default, because a call that leaves one out still passes a value and only the declaration knows which (Part I, 5.1). A method's receiver is the first parameter, so a caller reads the arguments off one list either way. A generic parameter is recorded as a **variable**, `$T`, so a caller binds it from what it passes and reads the result off the same signature ([ADR-074](adr/adr-074.md) D2): `hand` records `"(x: $T) -> $T"`, and a call that passes an `i64` gets one back. `Self` is `?`, because no call site binds it. Shared mutable state is written `SharedMut[T]`, the one name the language has for it ([ADR-039](adr/adr-039.md) D9) |
| `borrowed` | type | ADR-008 D6: `@borrowed` was asserted in the source |
| `fields` | type | every field with its type: `["name: &str", "temp: i32"]` |
| `tethered` | type | the fields that hold a view, directly or through another type that does |
| `doc` | fn, type | the run of `///` lines standing in front of the declaration, with its line breaks kept and no markup the ledger has to agree about ([ADR-139](adr/adr-139.md) D2). **Only on a `pub` entry**: the ledger records what a consumer may reach, and a private item's prose is the source's. Derived like every other column, so a hand-edited one is overwritten by the package's own build. The compiler does not read it — what it does is travel, and it is the one line in this file that is for a person |
| `trait."…"` | table | a trait, with its methods as ordinary `fn` entries (signature, `sync`, `throws`) and no `fields`; a bound and an `impl … for` must name one ([ADR-106](adr/adr-106.md) D3). *Not implemented.* |
| `impl."A for T"` | table | an `impl`, in the ledger of the package that wrote it. Whether `T` implements `A` is the union over every ledger a program reads plus its own; no ledger claims completeness ([ADR-106](adr/adr-106.md) D4). *Not implemented.* |
| `crosses` | type | a value of this type **may cross a thread** (`true`), **may not** (`false`, which the crossing refusals fire on, [ADR-123](adr/adr-123.md)), or nothing was recorded (absent) ([ADR-005](adr/adr-005.md) §1 Group B, `NK25xx`). Written by hand, or by `nikaia describe` from a foreign type's fields, and never inferred: it answers only for a type whose parts this compiler cannot walk, since a Nikaia `struct` records its `fields` and the check walks those. Absence means *nobody said*, not *it may not*; and *nobody said* is not permission, so the compiler does not put such a value on a thread of its own choosing |
| `touches` | fn | which resources it reaches and whether it reads or writes them: `["file(path) write", "stdout write"]` ([ADR-033](adr/adr-033.md)). **Absent means it touches everything**, so a function nobody has described orders against everything and stays where it was written. *Not implemented.* |
| `locks` | fn | whether the body may **acquire a lock** anywhere it reaches: the property that decides whether a call may appear inside an open lock ([ADR-039](adr/adr-039.md) D3). Propagated over the same call graph as `sync`, from the opposite end: nothing touches a lock until something it reaches says it does. **An absent entry means it touches a lock**, the one inverted key in this file (below). The key is coarse: it says *a lock*, never which lock, so it never answers an ordering question, which is `touches`'s line above ([ADR-039](adr/adr-039.md) D4). *Not implemented.* |
| `sharing` | fn | which of its `Shared` positions are one allocation, and which reference count each of those classes gets: `["counts \| <result>: plain", "hits: atomic"]` ([ADR-037](adr/adr-037.md) D7). The first fact is one a caller cannot work out for itself: `counts` and the result are **the same allocation**, so they have the same count whichever side of the call decides it. The count is `atomic` unless the compiler proved that nothing crosses a thread with the class. **Absent means the function has no `Shared` position**, not *nobody said*: the atomic count is the floor, and the worst an entry can say is `atomic` |
| `views` | fn | which of Part I 6.6's states each view in its signature solved to: `["data: borrowed", "<result>: tethered"]` ([ADR-008](adr/adr-008.md) D7). A parameter is lent for the call, so a view in one **borrows**; a result borrows where a receiver or a parameter carries a view, and **tethers** where the buffer is one the body made. `Owned` never appears: a `.to_owned()` hands back a `String`, which is not a view at all (D5). **Absent means the signature holds no view.** Nothing reads this column yet — a state is a representation and only Borrowed is emitted — so it is the analysis standing on its own until the other two are built; `nikaia --tethers` prints what it solved (D6) |

`sharing` and `views` are the function keys whose content cannot make a program wrong. The other function keys are read as permissions: a caller that trusts a wrong `sync` puts a pausing body inside a lock, and one that trusts a wrong `locks` puts a second lock inside the first. `sharing` records an **optimisation** on a floor that is already safe ([ADR-037](adr/adr-037.md) D6): the worst a class can be given is the atomic count every `Shared` would otherwise have, so a ledger that says nothing, or says `atomic` about everything, still describes a correct program. The classes compose across a call: a build that can read a dependency's sources continues the analysis through its published functions, and the two reference counts never become two types.

`signature` and `fields` are what make a type check possible across a boundary whose bodies are not visible; the `NK1xxx` diagnostics of C.3 are answered from them ([ADR-024](adr/adr-024.md)). A `?` in either is **the absence of a claim**. A checker reports a mismatch only where both sides are written down, so a contract that says less makes the compiler quieter and never wronger.

Only what is true is written. **An absent `sync` on an entry means not `sync`.** An absent *entry* means nothing is known, and a caller may not assume.

*Design rationale:* a `sync = false` on every entry would treble the file and say nothing, and a diff should show a promise being made or withdrawn ([ADR-027](adr/adr-027.md)).

`locks` is the one key whose absence is the restrictive answer written the other way round. **An absent `locks` means the function touches a lock.** A function that reaches no lock says `locks = false`; a function that says nothing is read as reaching one.

*Design rationale:* for `sync`, absence lands on the restrictive answer and costs nothing. For `locks` it would land on permission, and reading the absence of an answer as a yes is the polarity [ADR-010](adr/adr-010.md) D1 forbids ([ADR-039](adr/adr-039.md) D5).

**A changed `doc` is not a changed contract.** `--locked` compares this file byte for byte and a changed sentence fails that, which is correct: the file is derived and the build regenerates it. `NK2401`, which narrates what *broke a caller*, says nothing about prose ([ADR-139](adr/adr-139.md) D4).

**`sync` is written in two forms** ([ADR-027](adr/adr-027.md)). `sync = true` is a promise the source made; a body that contradicts it is refused with `NK2202`. `sync = "inferred"` is a promise the body implies: nothing the function calls can pause, so the function cannot pause.

A caller does not distinguish the two forms. Both mean *this cannot pause*, both satisfy the `sync` half of what `access` and `par_iter` require, and a query for the caller's question gets one answer. A diff distinguishes them: withdrawing an asserted `sync` is a decision, and losing an inferred one is a consequence of an edit elsewhere, usually in a function further down the call graph. The narration gives the two different sentences, which a `bool` cannot produce.

`sync` is one of two conditions for a body inside a lock. A body goes inside a lock only if it cannot pause and reaches no lock of its own; the first condition is `sync`, the second is `locks` ([ADR-039](adr/adr-039.md) D3).

**`throws` is a set for the same reason.** A boolean answers whether a callee can fail, which is what a caller needs to declare its own `throws`. A `catch` needs to know whether it still covers everything that can reach it, and that question has an answer only if the ledger names the errors. Recorded as a set, the diff says which error appeared and which `catch` stopped covering it. The narration is `NK24xx` (Appendix C), the same machinery a changed borrow contract uses.

**A function that runs a caller's lambda says so** ([ADR-029](adr/adr-029.md)). `xs.map fn(n) { n + 1 }` cannot pause and `xs.map fn(n) { io::read()… }` can, and both call the same `map`. A ledger entry holds for every caller. Without a way to say *it depends*, a higher-order function commits to the pessimistic answer, and `access`, `par_iter` and a scope's tasks would refuse every `map`, `filter` or `fold` inside a lock.

`sync = "from(f)"` names the parameter that decides. A caller reads it as *this call adds no pausing of its own*. The lambda runs during the call, so its body is part of the function that writes it, and that function has already counted its calls.

The same reason bounds the form. A parameter the callee stores or spawns (`@detached`, Part I 5.4) breaks it, because the lambda's calls then belong to nobody the caller is counting. `from` is for an immediate lambda only. Until the ledger can spell `@detached`, the rule is held by a check over `std`'s own entries rather than by the file format ([ADR-029](adr/adr-029.md) D4).

`locks` has no `from(f)` form. The refusal `locks` serves is decided on the caller's own body, where the lambda stands in front of the check, so the check reads the lambda directly ([ADR-039](adr/adr-039.md) D3). The premise is the same immediate lambda. A lambda the callee stores may run while a lock is open somewhere the walk cannot see, which is [ADR-039](adr/adr-039.md) D7's case; a lambda it spawns runs later and elsewhere.

**A signature may name its receiver's type arguments** ([ADR-031](adr/adr-031.md)). `HashMap::entry` is written `(&HashMap[$K, $V], key: ?) -> Entry[$V]`: the result holds whatever the map holds. At a call site the receiver's actual type binds the variables, and they are substituted away: `HashMap[&str, Stats]` makes the result an `Entry[Stats]` and `and_modify`'s `fn(&$V)` a `fn(&Stats)`, which gives the `s` in `fn(s) { s.add(t) }` its type.

**A sequence has a name in this language, and a container keeps its own** ([ADR-105](adr/adr-105.md)). `Seq[T]` is elements of `T` produced step by step; `keys()`, `chars()`, `io::lines()` and `xs.map fn …` hand one back, and `sync`, `pauses` or `throws` after it say what one step may do, as after a function type. The first two are **three states and not two** ([ADR-172](adr/adr-172.md) D1): `sync` promises the step does not pause, `pauses` warns that it does, and neither is *nobody said* — a `map`'s step runs the lambda. A `for` over a sequence that says `pauses` gives its thread up instead of holding it; one that says nothing keeps the blocking step, because awaiting a step that has none would be a correct program refused (C.4). A `Seq` is consumed by walking it, so a second walk is refused. A `Vec[T]` or a `HashMap[K, V]` is a container, walked by view and as often as a program likes. `Par[T]` is what `par_iter()` hands back, and a lambda handed to it must be `sync` (Part II 12.6). Neither word is in a program's type grammar. **`Seen[T]` is what a lock hands out** ([ADR-111](adr/adr-111.md)): `get` is `-> Seen[$T]`, the result of `access` is a `Seen`, the stamp sticks through arithmetic and through calls to entries whose `touches` names no lock, and a `set` given one is refused. `Seen` is a type in the ledger and in the checker and nothing in the language below: a `Seen[i64]` is emitted as an `i64`. It is in a program's type grammar at two places: a struct field, and a parameter of a function that touches a lock.

> **Implementation status:** Partially implemented. `Seq[T]` and `Par[T]` are words of the ledger's type language with all three trailing words, six `std` entries hand one back and six consume one, a `for` over one binds its item, and `NK2702` refuses a second walk ([ADR-105](adr/adr-105.md) §5). `pauses` is read by the `for` and by nothing else: the consumers have no pausing form, so `io::lines().count()` holds a thread where the `for` does not ([ADR-172](adr/adr-172.md) §4).

**An unbound variable becomes `?`, never a name.** A variable that survived into a comparison would make the checker report that `i32` is not `$V`, the false positive ADR-024 D4 erases generics to avoid. A variable is bound and replaced, or it is the absence of a claim. A map built by `HashMap()` says nothing about what it holds and binds nothing, and the chain stops there.

**A variable says what flows out; `?` stays for what flows in.** A variable may appear in a result and in a lambda's parameter type, and never in an argument. What flows out is a promise the ledger makes on its own account. What flows in is a constraint on a program, and the language below accepts more than its type parameters suggest, so a variable there would reject correct code. Binding is narrow: from the receiver, by position, one pattern. Widening it is a decision for a record.

The type a lambda parameter names is a **function type**, `fn(&Stats)`. It says what the lambda is handed, so that the `s` in `fn(s) { s.add(t) }` has a type and the call on it resolves. Only a ledger writes one: Nikaia's grammar has no syntax for a function type, so no source program declares a parameter of that shape.

The check on an asserted `sync` and the inference of `sync` are conservative in opposite directions:

* The **check** is conservative in the permissive direction. It reports only calls it can prove will pause, so it never rejects a correct program. A call it cannot resolve is not an error.
* The **inference** is conservative in the restrictive direction. It claims `sync` only where every call resolves and every callee is `sync`; an unresolvable call costs the function its claim. The entry is shipped, and a consumer will put the function inside `access`: a wrong `sync` there is a pausing body inside a lock, and a wrong `locks` is a second lock taken inside the first.

**What counts as resolvable is the type checker's answer** ([ADR-028](adr/adr-028.md)). A call by name, such as `helper(x)` or `io::read_to_string()`, is looked up directly. A method call needs the receiver's type. The compiler has one module that infers types; it records where each method call went, and the `sync` inference reads that record. What the two analyses can see grows whenever the type checker can name more, and neither changes when it does.

The gap between the two polarities is where a program writes `sync` by hand: *this cannot pause, hold me to it*, the same move `@borrowed` makes in Part I 6.6, checked the same way. A ledger that describes more (ADR-024, ADR-028) shrinks the gap, because more functions earn the promise on their own.

A method with no entry is an unknown, and an unknown costs every function that calls it its inferred promise. A library that ships thin contracts therefore makes its consumers' code unusable inside `access` and `par_iter`, however pure that code is.

> Writing a signature down is what lets a library's callers be `sync`.

**Which inference wrote it.** The header carries `inference`, because a ledger produced by reading signatures is not one produced by reading bodies. The bootstrap compiler writes `stage0-signatures+sync-bodies+throws-bodies+sharing-bodies`: the borrow contract is the widest one the signature supports (a result that is a view may point into any view it was given), and `sync` ([ADR-027](adr/adr-027.md)), the errors a `throws` names ([ADR-023](adr/adr-023.md) D1) and `sharing` ([ADR-037](adr/adr-037.md) D7) are read off the body. A ledger regenerated by a compiler that reads one more body shows a header change, and the change is narrated. The `toolchain` recorded is Nikaia's version, not `rustc`'s: the contracts are decided by this compiler and never by the backend.

**Distribution.** A published package ships its ledger, so a downstream project builds against stable contracts and receives the same diff-based explanation when a dependency upgrade changes one. `std` ships `std.contracts`; it is the file a program's compiler reads when the program calls `io::…` or `fs::…`. A library whose implementation is partly in another language cannot have all of its contracts inferred. Those contracts are written in the ledger, marked as such, and reviewed like code; the ones that can be inferred are regenerated and checked against the sources by the library's own tests ([ADR-020](adr/adr-020.md) D5).

**A consumer reads a dependency's ledger; it never derives a dependency's contracts itself** ([ADR-100](adr/adr-100.md)). A package's ledger is written by the package's own build, in which its own dependencies are in view, and read by every consumer; this is the rule `std` has, for every package. The inference runs over the package as one graph, so a call from one file of a package to another resolves (D2). A call into a dependency is answered from that dependency's ledger. Only a call into code no ledger describes is unresolved, and it fails closed.

**A ledger is believed only while the sources it came from are unchanged.** The header records, per unit, the SHA-256 of the file the entries were derived from, the hash `nikaia.lock` already holds. At a consumer's build a dependency whose sources hash as recorded is believed, and nothing is inferred. A dependency whose sources changed has its ledger derived again, written, and the difference narrated. A dependency with a ledger and no sources is believed. A dependency is never believed against its own sources: a stale ledger is a hash that does not match, and that is a derivation rather than a belief (D3). The build is ordered by the dependency graph, so a ledger exists before its consumer is checked (D5). A mismatch the backend reports at a package boundary is translated as *the ledger of that package does not match its sources* (D6).

> **Implementation status:** Partially implemented. A package's units are inferred as one graph, its ledger is written in its own root in dependency order, the header carries a SHA-256 per unit, a consumer believes the ledger while the hashes match and derives the package again where they do not, and `--locked` compares each package's ledger byte for byte. D6's translation of a boundary mismatch reported by the backend is not implemented ([ADR-100](adr/adr-100.md) §5).

**Version control.** `nikaia.contracts` is committed. A merge conflict resolves like a lockfile conflict: accept either side and run `nikaia build` to regenerate. The recorded `toolchain` lets the compiler detect that a toolchain upgrade, and not user code, changed the inference results; the build output then states that the contract changes were caused by the toolchain update.

**Determinism guarantee.** The ledger is a **pure function of (source tree, toolchain)**: the same sources and the same pinned toolchain produce a byte-identical `nikaia.contracts` on every machine, every run, with any thread count ([ADR-005](adr/adr-005.md) D8). A violation is a compiler bug. Two consequences:

* There is exactly **one** ledger per project, valid at every value of every build option. Borrow contracts and tether relationships do not depend on a build option; a check that does, such as a thread-safety rule, is performed by the compiler directly and is never recorded in the ledger.
* Ledger stability is **not** promised across toolchain upgrades: a newer compiler may infer better contracts. The recorded toolchain and the narration make such a diff self-explaining.

**Verification mode (`--locked`).** `nikaia build --locked` verifies instead of updating: the compiler regenerates the contracts in memory, the program's and those of every path dependency it has sources for, and compares each byte for byte against its committed `nikaia.contracts`. Any difference fails the build with the narrated contract diff (`NK2401` above). This is the one place contracts are compared rather than hashes; a development build compares hashes and derives only what changed ([ADR-100](adr/adr-100.md) D3, D4). Because of the determinism guarantee the check is exact and needs no tolerance or semantic comparison. A CI build with `--locked` is equivalent to `git diff --exit-code nikaia.contracts` after a regular build.

---

## Chapter 14: Testing and Quality Assurance

Testing and verification are part of the language.

> **Implementation status:** Not implemented. `test` and `bench` are parse errors, and `assert` is not a keyword: `assert cond` parses as two statements and is refused with `NK1117`, `assert(cond)` is a call to a function of that name, and `assert cond, "message"` does not parse. There is no `nikaia test` and no `nikaia bench` (13.2), so there is no fuzzing, no `impl Generator` dispatch, no `--with-asserts` and no `--history`.

### 14.1. Unit Tests (`test`)
A `test` block checks specific inputs. `test` blocks are compiled only during `nikaia test`.

```nika
test "Addition" {
    assert 1 + 1 == 2
}
```

### 14.2. Runtime Assertions (Design by Contract)
An `assert` statement inside an ordinary function enforces a precondition or an invariant.

**Compiler behaviour:**
* **Debug profile:** assertions are active. A false condition panics with a detailed message.
* **Release profile:** assertions are removed, unless the build enables them with `nikaia build --with-asserts`.

```nika
fn divide(a: i32, b: i32) -> i32 {
    // Precondition: Denominator must not be zero.
    // In Release mode, this check disappears.
    assert b != 0, "Division by zero prohibited"
    
    return a / b
}
```

### 14.3. Property-Based Testing (Fuzzing)
Fuzzing generates random data to find crashes. The test runner does this for a test that declares parameters.

**Automatic data generation**
A test that declares parameters is given generated inputs.
* **Primitives:** random integers, strings, bools.
* **Structs:** data is generated recursively for every field.

```nika
struct User { name: String, age: i32 }

// Nikaia automatically creates random 'User' structs here
test "User Validation" (u: User) {
    assert u.age >= 0 // Might fail if fuzzer generates -1
}
```

**Custom generators (`impl Generator`)**
Where random data does not fit the type, such as a field that must hold a valid email address, the type implements the `Generator` trait.

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
A `bench` block measures the speed of user code.

**Regression detection**
`nikaia bench`:
1.  executes the block thousands of times;
2.  calculates the average time and the standard deviation;
3.  compares the result against the last recorded run.

Where the new version is significantly slower (more than 5%), the CLI prints a warning:
> **Performance regression:** 'Sorting' is 12% slower than commit 8f3a2c.

**Result storage**
Results are stored in `.nikaia/benchmarks.json`. The file records:
* the timestamp
* the git commit hash
* the function name
* nanoseconds per operation

```nika
bench "Sorting" {
    let mut list = [5, 2, 9, 1, 6]
    list.sort()
}
```

**Viewing history**
`nikaia bench --history` shows the recorded history.

---

## Chapter 15: Interoperability (FFI)

A Nikaia program calls C and Rust, and a Nikaia library is called from C and from every language that speaks C.

### 15.1. C Interoperability
A call into C is written inside an `unsafe` block. C is not memory-safe, and the boundary is marked where it is crossed.

```nika
extern "C" {
    fn strlen(s: &[u8]) -> usize
}

fn main() {
    let text = "hello\0"
    let n = unsafe { strlen(text) }
    println(f"{n}")
}
```

The `\0` is C's, not this language's: a C string ends at a zero byte and the caller is what puts one there.

`extern` and `unsafe` are reserved words with constructs ([ADR-124](adr/adr-124.md) D1). An `extern "C"` block lowers to Rust's own, and the call lowers to Rust's own `unsafe`. A call to an `extern` name outside an `unsafe` block is refused with `NK1143`. A declaration in an `extern "C"` block is `sync` and carries no `throws` without writing either (D2): C has no suspension point, and a C function that sleeps blocks a thread, which is Part II 12.1's question. Its `touches` and `locks` are fail-closed, as a described crate's are (D4).

**What the boundary lends is a view** ([ADR-147](adr/adr-147.md) D1). `&T`, `&mut T`, `&[T]` and `&mut [T]` in an `extern` declaration are the pointer C wants, and each lives **for the call** — which is what a view is everywhere else in this language ([ADR-094](adr/adr-094.md)). Nothing is stored and nothing escapes, so the shape that dangles cannot be written. A `&[T]` takes whatever lends a run of elements — a `Vec[T]`, an `Array[T, N]`, and text where the element is `u8` — and the call is where the address is made.

**A length beside a view is checked at the call** ([ADR-147](adr/adr-147.md) D2). A `&[T]` and, right after it, a `usize` are **one fact** in C: the pointer says where and the count says how far. A call whose count cannot be shown to fit the buffer is refused with `NK1159` — because a longer count is the buffer overrun this boundary exists to stop, and it belongs here rather than at the operating system. Two shapes are accepted and everything else asks for one of them: the buffer's own `len()`, and a constant a known length covers, which is an `Array[T, N]`. The *type* is what says *length* rather than the name, because `usize` is not a type this language's own values have ([ADR-048](adr/adr-048.md) D1) — so a declaration that writes one is naming C's `size_t`, a caller hands it the `i64` this language does have, and the conversion is emitted.

**Both forms are the boundary's and nowhere else's.** Away from it, a parameter this language may change is written `mut name: T` ([ADR-094](adr/adr-094.md) D3) and a run of elements is a `Vec[T]` or an `Array[T, N]`, each of which carries its length; a `&mut` or a `[T]` outside an `extern "C"` declaration is refused with `NK1158`.

**A handle a library hands out is `opaque`** ([ADR-147](adr/adr-147.md) D3):

```nika
extern "C" {
    opaque type FILE released by fclose
    fn fopen(path: &[u8], mode: &[u8]) -> FILE
    fn fclose(f: FILE) -> i32
}
```

An opaque type is an address the language **never dereferences**. It is moved and stored like any value, it has no fields and no indexing — both are `NK1160`, and so is writing one as a call, because a handle has no constructor and an address somebody chose is the one thing it may never be. Its release is a `cleanup` the compiler runs at the end of its scope (Part I 6.4), so a handle cannot be forgotten; what no rule can answer for is a C function that keeps the address past its own call, and that is said rather than checked.

A handle is **lent** to every declaration but its release: `fileno(f)` reads it and `f` is still the caller's to close, where `fclose(f)` takes it and *is* the cleanup. None of `opaque`, `type`, `released` and `by` is a reserved word — the grammar is scannerless, so each means something only in this one position and stays a name everywhere else.

**A handle that may be absent is a `T?`** ([ADR-155](adr/adr-155.md)), meaning what Part I 2.3 means by it: `??` and `?.` are how a program gets past it. It costs nothing — a handle holds a **non-null** address and `T?` is the absence of one, which are the two states C spells with a pointer and `NULL`, so a nullable handle is one machine word and `&mut sqlite3?` is `sqlite3 **` exactly as C writes it. That is what makes an **out-parameter** work:

```nika
extern "C" {
    opaque type Block released by free
    fn posix_memalign(out: &mut Block?, alignment: usize, size: usize) -> i32
    fn free(b: Block)
}

fn main() {
    let mut room: Block? = null
    let rc = unsafe { posix_memalign(room, 64, 128) }
    println(f"{rc}")
}
```

At this boundary a `?` on a `&mut T` or a `&[T]` belongs to **what it points at** and not to the view: a view here lives for the call and is never absent, so `&mut sqlite3?` is a slot that holds a handle or nothing. A plain `&T?` is untouched and is 2.3's own nullable view.

A declaration that does **not** say `?` is a **claim**: C may still hand back nothing, and what the program gets then is an abort naming the declaration ([Appendix A.2](#appendix-a-the-runtime-model)) rather than a handle that is secretly null. The claim is the author's and the check is the compiler's, which is the arrangement `sync` on a declaration already has.

**Text a C library hands back is a `CStr`, and `std` copies it** ([ADR-147](adr/adr-147.md) D4):

```nika
use std::foreign

extern "C" {
    fn getenv(name: &[u8]) -> foreign::CStr
}

fn main() throws {
    let raw = unsafe { getenv("HOME\0") }
    println(raw.to_string())
}
```

`getenv` hands back memory the caller does not own, whose lifetime is the library's, and which ends at a zero byte rather than carrying a length. A `CStr` is that address — an opaque handle with **no** cleanup, which is what tells it from the handle above: a `FILE` is the program's to close and a C string is not the program's at all. `to_string` copies it into a `String`, and the `unsafe` walk to the zero byte is written **once**, in `std`, so no program writes one. It **fails** in one way — the bytes may not be UTF-8, which text in this language is, and which is what `fs::read_to_string` fails on for the same reason. *There is no text* is a **value**: a declaration says `-> CStr?` where the library may find nothing, and the program writes `??`.

**There is no raw pointer, and `Pointer[u8]` is not a type** ([ADR-147](adr/adr-147.md) D5, [ADR-124](adr/adr-124.md) §4). Memory this language will index has to arrive with a length it knows, and `malloc` hands back an address and no length — so it has no shape here, which is the decision rather than a gap. A program that needs a buffer makes one in Nikaia and lends it.

> **Implementation status:** Implemented, and measured against a real library: `examples/sqlite/main.nika` opens a database, prepares a statement, steps it and reads a row back, and what closes the database and the statement is written nowhere. `extern "C"` blocks, `unsafe { … }`, `NK1143`, all five decisions of [ADR-147](adr/adr-147.md) and all five of [ADR-155](adr/adr-155.md) are built — the four view forms, the length check, the opaque handle with its `cleanup`, the `CStr` copy, the nullable handle and the claim a declaration without `?` makes — with `NK1158` for either view form away from the boundary, `NK1159` for a length that cannot be shown to fit and `NK1160` for reaching past a handle. All four blocks above compile and run against libc. `Pointer[u8]` is refused with `NK1135`, which [ADR-147](adr/adr-147.md) D5 makes permanent rather than pending. A **`T?` reaches the `T` a parameter wants** through `?? throw`, which is [ADR-138](adr/adr-138.md) D1's jump-as-an-expression meeting [ADR-089](adr/adr-089.md)'s `??`: `slot ?? throw Refused::NoDatabase` is a `sqlite3`.

**A build that lets C call in is a target** ([ADR-062](adr/adr-062.md) D1). An exported entry point may be called twice at once from threads the caller owns. `user_parallelism` bounds user code and cannot answer for a caller's threads, so the answer is a property of the target. Such a build is one artifact, safe at its boundary: the entry points and everything they reach take the safe shape, and the rest of the library keeps the per-value answer.

**Providing a library to C, and to everything that speaks it** ([ADR-125](adr/adr-125.md)).
A `pub extern "C" fn greet(name: &str) -> String { … }` with a body is an **entry point**. A build
with `artifact = "c-library"` in `[build]` makes a shared and a static library of the package and
generates `<package>.h` from the ledger. What a C programmer sees follows two rules: the caller
owns the memory, and a call never surprises.

* every entry point returns a status (`0` is `<PACKAGE>_OK`, the error variants are numbered per
  library, seven negative codes are the boundary's own) and a value travels in an out-parameter;
  `<package>_last_error` renders the failure with its site and its `secondary` list, per thread;
* a text or byte result is written into the **caller's buffer** (`out, cap, written`; `NULL` asks
  the size; too small is a status with the size that would do), and the caller may supply the
  **allocator** for everything the library holds across calls, before `init`;
* an exported `struct` is an opaque handle with `_new`, `_free`, a getter per `pub` field and its
  `extern` methods; text and bytes go in as pointer and length; `T?` is `NULL` or a status;
* a function that may pause is exported **blocking** and **`_async`** with a callback on a library
  thread; a `sync` one only blocking;
* a panic is caught at the boundary, returns a status, and poisons the library until
  `shutdown` and `init` have run; a handle carries a lock, and a re-entrant call on it from a
  callback is a status rather than a deadlock;
* everything that enters is `Untrusted`, the header carries the ledger's hash, and `--locked`
  refuses an ABI change nobody committed;
* a **`pub extern "C" struct`** has C's layout and crosses **by value** — numbers, `bool`, `char`,
  payload-free enums and other such structs as fields, every field `pub`, anything else `NK1145`
  with the handle named as the shape; a `Vec` of them is an array in and the caller's buffer out
  ([ADR-127](adr/adr-127.md));
* the symbol prefix is the package's name, or the one `symbol-prefix = "…"` the build sets; no
  declaration renames itself ([ADR-128](adr/adr-128.md));
* the `_async` form takes a **ticket** (`<package>_op**`, or `NULL`) that `<package>_cancel` cancels
  at the task's next pause point, with `cleanup` run and `done` called exactly once, `E_CANCELLED`
  if the cancellation came first; **many results** are a function taking `fn(item) -> bool sync`,
  whose `false` stops the stream ([ADR-129](adr/adr-129.md));
* on `target = "wasm32-unknown"` the same declarations make a `.wasm` with a generated `.js` and
  `.d.ts`: the host takes its buffers from `<package>_alloc`, there is no blocking form and no
  `set_allocator`, a pausing entry point is a Promise, and there is **no `extern "wasm"`** — the
  convention is `"C"` on every target ([ADR-130](adr/adr-130.md));
* `nikaia bind python` writes a `ctypes` binding from the ledger — exceptions for statuses,
  `str`/`bytes` for buffers, classes for handles, generators for streams, awaitables for `_async` —
  and `nikaia bind js` is the WebAssembly build's `.js`; there is no second artifact
  ([ADR-131](adr/adr-131.md)).

> **Implementation status:** Not implemented. `extern "C"` with a body is a parse error, and there is no library artifact and no header generator ([ADR-125](adr/adr-125.md) §5). The five records that extend it (ADR-127 to ADR-131) are not implemented with it; a native Node add-on, a fixed-size array field and the component model are what they leave open.

### 15.2. Rust Integration (Deep Integration)
Nikaia treats a Rust crate differently from a C library. Rust has a strong type system, so the compiler verifies safety properties at the boundary.

**A value handed to a Rust function may reach a thread that function owns.** A Rust crate may bring its own runtime and its own threads ([ADR-038](adr/adr-038.md) D7). A call whose body the compiler cannot see may put what it is given on a thread of its own, and a value may cross into a foreign thread only if it may cross any thread. The crossing the compiler can decide about is refused with `NK2502` (C.5), at both values of `user_parallelism`: the build option bounds what user code runs at once, and a foreign runtime's threads are not user code.

The rule reaches exactly as far as the Rust signature is true. A Rust API that declares a type safe to send when it is not puts the value on another thread, and no check in the frontend can see that: where Nikaia reads the signature, the value is crossable by declaration. A narrowing shim is reviewed like the boundary it is.

**A call into foreign code is judged by what its arguments can reach.** Foreign code touches only what it reaches, and the language has no global mutable data. Where no lock is reachable from the arguments, transitively and through the fields of a struct, the call is allowed and the compiler says nothing. Otherwise the call is refused with `NK2503` (C.3, worked through in C.6); the way out is to keep the lock out of the call's reach and hand over a copy of what it needs. This extends the foreign-thread rule above ([ADR-038](adr/adr-038.md) D7) to locks ([ADR-039](adr/adr-039.md) D6). A lambda among the arguments is its captures, which nothing writes down, so whether it touches a lock is read off its body (Part II, 12.3; [ADR-039](adr/adr-039.md) D7).

**"No lock reachable" is an answer, not the absence of one.** Where nothing written down says what a value contains, the question is undecided: C.5's third answer, which is not permission and is handed on. An undecided type is not a type with no lock in it. A ledger entry with an empty field list, which is what a type whose fields are Rust has, means *nothing recorded* and not *nothing inside* (13.5).

> **Implementation status:** Implemented for a call into a crate nothing describes. `NK2502`, `NK2503` and the lock the compiler knows exists ([ADR-064](adr/adr-064.md)) are all in, and the reachability walk is `NK2502`'s own rather than a copy of it ([ADR-039](adr/adr-039.md) D6): one walk per argument, through the fields of a struct and as deep as the ledger reaches, and the refusal's own reason decides which of the two codes it prints. A **described** foreign function is not asked — the rule is about a body this compiler cannot see — and a lambda among the arguments is still read as `Undecided` rather than through its body ([ADR-039](adr/adr-039.md) D7).

**Mapping Types**
* Rust `i32` -> Nikaia `i32`
* Rust `i64`, `u8` -> Nikaia `i64`, `u8` — the rest of the numeric surface (Part I, 2.2)
* Rust `&str` and Rust `String` -> Nikaia `String`, whose state the compiler
  picks ([ADR-107](adr/adr-107.md)): a Nikaia `String` crosses to Rust `&str`
  for free, and to Rust `String` only by a `.to_owned()` the program writes
* Rust `Option<T>` -> Nikaia `T?` (Nullable)
* Rust `Vec<T>` -> Nikaia `Vec[T]`, and `HashMap<K, V>` -> `HashMap[K, V]`
* Rust `Rc<T>` **or** `Arc<T>` -> Nikaia `Shared[T]`. One Nikaia type, two Rust
  ones, and the compiler decides per value which it becomes
  ([ADR-037](adr/adr-037.md) D7). This is the one row of the table that does not
  cross: a `Shared[T]` handed to a call whose body the compiler cannot see is
  refused, because a Rust signature names one of the two shapes and the program
  may be using the other one for that value ([ADR-061](adr/adr-061.md) D1); a
  lock is refused by the same rule. The way across is what is inside: a view or
  a copy. A foreign library that means to keep the value clones it into a hull of
  its own.

**A crate is described before it is called** ([ADR-104](adr/adr-104.md)). A call
into a Rust crate no ledger describes is refused, and the message names the
command. `nikaia describe <crate>` reads the crate's `pub` signatures, from
rustdoc-JSON where the toolchain offers it and from the sources where it does
not, and writes a draft entry for every function the program calls and the types
those signatures name, translated by the table above. The draft is committed as
`contracts/<crate>.contracts`, believed while the crate's version and source
hash hold, and reviewed like code: what a signature cannot say (`touches`,
`locks`) is written fail-closed, what neither reader can read is written `?`,
and a signature that lies is the reviewer's to correct. Every analysis then
reads an entry at the boundary, never an absence.

> **Implementation status:** Partially implemented. A call into a crate the manifest declares with `type = "rust"` that no `contracts/<crate>.contracts` describes is refused with `NK2504`, once per crate and with the command in the message; a written type from such a crate counts as a call, and a name the build did not declare is left alone (C.4). **And the description's entries are read**, which is D1's own first sentence: the signature types the call and what it hands back, `throws` makes it a place that can fail, and `sync` makes it one a `sync` function may not make. A described crate is a **package** by the spelling rule and not one of `std`'s modules, so nothing about it is imported. `nikaia describe` (D2, D4) and the hash rule (D5) are not implemented, so a description is written by hand today, and `examples/foreign-runtime/` ships one ([ADR-104](adr/adr-104.md) §5).

**Thread Safety (Send/Sync)**
Whether a value may cross into foreign code is decided from the Nikaia type of
the argument, not from the Rust crate. No crate metadata is read: a foreign call
is a call no ledger describes ([ADR-024](adr/adr-024.md)), and the verdict is
taken on the argument's Nikaia type.

* A type the compiler knows may cross is allowed in a `spawn` task and in a
  foreign call.
* A type it knows may not, such as a `Shared[T]`, is refused with `NK2502`
  (C.5); where what it may not cross with is a **lock**, the refusal is about
  the call and the code is `NK2503` (C.6). Either diagnostic names the Nikaia
  type, because that is the one the program wrote.

> **Implementation status:** Implemented. `NK2502` and `NK2503` fire on the crossings the compiler can decide about, at both values of `user_parallelism` ([ADR-038](adr/adr-038.md) D7, [ADR-061](adr/adr-061.md) D1); everything else is C.5's third answer, undecided.

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
WebAssembly in its basic form has a linear memory model and runs in a single-threaded host. `user_parallelism = no` matches that host.

**Zero overhead.** `nikaia build --target=wasm32-unknown` produces compact
binaries. The runtime a `no` build starts is the I/O worker and nothing else
([ADR-038](adr/adr-038.md) D4), and no OS-level mutex is generated. A
`Shared[T]`'s owner count is chosen per value ([ADR-037](adr/adr-037.md) D7),
and at `user_parallelism = no` every one of them is the cheap count
([ADR-061](adr/adr-061.md) D2): there is one thread of user code, the runtime's
own threads carry no user code, and a `Shared` may not leave the program into
code nothing describes (D1). The verdict on a crossing does not move with the
build option ([ADR-045](adr/adr-045.md) D1): whether a value may cross is a
question about a type and a destination, and a call whose body the compiler
cannot see is refused a lock and a `Shared` at both values.

**A library for the web** is the same build with `artifact = "c-library"`
(15.1, [ADR-130](adr/adr-130.md)): the `pub extern "C" fn` declarations become
the module's exports, a generated `.js` and `.d.ts` stand where the header
would, the host obtains its buffers from `<package>_alloc` because the module
sees no memory but its own, and a function that may pause is a Promise driven
from the host's event loop. There is no `extern "wasm"`.

> **Implementation status:** Partially implemented. The `no` runtime starts the I/O worker only, and the cheap count is chosen at `no` ([ADR-061](adr/adr-061.md) §5). The library build for the web is not implemented ([ADR-130](adr/adr-130.md) §5).

**JavaScript Interoperability (`dsl js`)**
Nikaia does not map the DOM to Nikaia structs. A program embeds JavaScript with the `dsl` keyword (Part II, 10.5).

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
    script.exec(msg: message)
}
```

---

## Chapter 16: Hardware Instructions (via DSL)

Hardware instructions are **not** part of the Nikaia core language. They are provided by
library-defined DSLs, `dsl backend::x86`, `dsl backend::arm64` and `dsl backend::wasm`, each
of which validates its own operands.

### 16.1. Why not a core construct

The core language has no `asm` block and no register constraints. Each backend DSL defines the
operand model its hardware has and the grammar that validates it ([ADR-007](adr/adr-007.md), D6).

*Design rationale:* a built-in `asm` block with `in(reg)`, `out(reg)` and `clobber("cc")` assumes
every target has registers. WebAssembly (Chapter 15) is a stack machine, and a core construct
that has no meaning on a first-class target is a defect of the core ([ADR-007](adr/adr-007.md) D6).

### 16.2. Usage

Assembly uses the standard `dsl` syntax. The assembly DSL uses **immediate capture**
(`meta::capture`, Part II 10.5): it binds variables from the current scope and injects machine
code at the call site. A SQL DSL, by contrast, builds a reusable statement and takes deferred
parameters. In an assembly block, `val` is the `val` in the enclosing scope.

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

The constraint vocabulary (`reg`, `freg`, `mem`, `imm`, clobber declarations) belongs to the
`x86` grammar and is documented with it, not with the language. A stack-machine backend declares
a different vocabulary: `dsl wasm` has locals and a value stack, not registers.

### 16.3. Consequences

*   **Portability:** the language core makes no assumption about the target's execution model.
*   **Validation:** the DSL parser checks instruction operands at compile time and reports
    errors through the same diagnostics contract as the rest of the compiler (Appendix C).
*   **Optimization:** a backend DSL can emit target-specific or SIMD instructions without any
    change to the language.
*   **`unsafe`:** `unsafe { … }` is a construct, and the word is on Part I 2.1's list
    ([ADR-124](adr/adr-124.md) D1). 15.1 writes it for a call across the C boundary.

> **Implementation status:** Not implemented. No backend DSL exists, and an `asm` block under `unsafe` is not built; `unsafe { … }` itself is implemented ([ADR-124](adr/adr-124.md) D1).

---

## Chapter 17: The Standard Library ("Batteries Included")

The standard library consists of universal modules, with the same API on every target, and target-specific capabilities.

### 17.1. Universal Modules
These modules rely on the unified types and behave identically at every value of every build option. Their implementation differs to match the runtime model.

**`std::io` — standard input**

A stream is not a file, and the surface says so ([ADR-019](adr/adr-019.md)): no `map`, no `seek`,
no length, and no second read of the same bytes. `fs::map` hands back pages that existed before
the program asked for them. Standard input's bytes do not exist until they are read, so a program
that wants views into its input owns the buffer first.

```nika
pub fn read_to_string() -> String throws   // all of it, UTF-8 validated
pub fn read() -> Bytes throws              // all of it, as bytes
pub fn lines() -> Lines throws             // one line at a time
pub fn bytes() -> ByteStream throws        // chunks as they arrive
```

**A step of `lines` can fail, and the failure leaves the function** ([ADR-025](adr/adr-025.md) D1). A pipe's bytes do not exist until they are read, so the failure cannot be moved to the call the way `fs::map` moves it. Nothing marks the loop, as nothing marks a failing call ([ADR-023](adr/adr-023.md) D8). The compiler requires the enclosing function to declare `throws`:

```nika
use std::io

fn tally() -> i64 throws {          // NK2701 without the `throws`
    let mut n = 0
    for line in io::lines() { n += 1 }
    return n
}
```

A failed read is never indistinguishable from the end of the input. Part I 6.4 refuses that at the closing brace of a block; this is the same refusal at the top of a loop.

*Design rationale:* a scanner that reports its error afterwards turns a truncated stream into a shorter one, and the tally is quietly wrong ([ADR-025](adr/adr-025.md) D1).

The surface is `std::fs`'s shape minus what a stream cannot keep. A read looks blocking and is not: no `async` on the signature, no `await` at the call. **And a `for` over `lines()` is the same**, which is what it means for a *step* to pause ([ADR-172](adr/adr-172.md) D1): nothing in the loop says so, and the thread is given up between lines rather than held for the length of the stream. On a single-threaded runtime the event loop runs another task while the pipe is empty; with threads the read may resume on a different one. A `sync` function cannot call it (Part II, 12.1), which keeps a `par_iter` body from waiting on a pipe.

> **Implementation status:** Partially implemented. `read_to_string`, `read` and `lines` are implemented; `bytes` is not. `fs::read`, `fs::read_to_string` and `fs::write` go through [ADR-038](adr/adr-038.md) D3's mechanism, and all three of `io`'s reads are performed on an I/O worker and **awaited** ([ADR-121](adr/adr-121.md) D4, [ADR-172](adr/adr-172.md) D4) — a stream has no size to `stat`, so it is the blocking read moved off the program's thread rather than a ring path. `io::lines` asks for a **chunk** per hop and finds the line endings itself, because a hop per line would cost more than the suspension saves.

`lines()` yields **owned** text, where a file's lines are views into the mapping they came from
(`fs::map(path)` and `.lines()`, below): a file is still there to point at, and a stream's bytes
are gone once consumed. An iterator may hand out views into a buffer it does not own, and never
into one it does ([ADR-025](adr/adr-025.md)).

There is one standard input, so these are functions rather than a handle. Reading it a second
time yields what the operating system says, which is nothing.

*Design rationale:* a handle that can be held invites two tasks to hold it, and two readers of
one pipe get interleaved halves of lines ([ADR-019](adr/adr-019.md)).

**Provenance** follows the rule files follow: **Trusted**, because the operator chose what to
connect to the pipe as they chose which path to open. A request body arriving on standard input
is the uploaded-file case and has the same answer in the same place:
`io::read_to_string(trusted: false)`.

**Writing output**

`println(text)` writes a line to standard output, `print(text)` writes without the newline, and
`eprintln` / `eprint` are the same two on standard error. They are in the prelude rather than in
a module.

The argument is an ordinary interpolated string (Part I, 2.5), so a hole is written where the
value goes and `{{` is a literal brace:

```nika
print(f"{name}: ")
println(f"{count} rows")
```

`print` is for output composed piece by piece, such as a pretty-printer that indents a tree or a
progress line rewritten in place, where a newline after every fragment would be wrong.

**`http` — a package, not a module of `std`**
An HTTP/1.1 and HTTP/2 server and client. `http` is not part of `std` ([ADR-069](adr/adr-069.md) D1): it is a package reached by path, `http = { path = "../http" }`. The server is Nikaia's own rather than a binding to a finished one ([ADR-038](adr/adr-038.md) D1, D6).

* **At `user_parallelism = no`:** the server runs on a single-threaded event loop.
* **At `yes`:** the server runs on a multi-threaded work-stealing executor.

> **Implementation status:** Not implemented. The package lives in `examples/` until its surface stops moving ([ADR-069](adr/adr-069.md) D4). HTTP/1.1 comes first; HTTP/2 is intended and does not exist.

```nika
use http

fn main() {
    // Starts a server on Port 8080.
    // The code looks the same, but the runtime behavior follows `user_parallelism`.
    // The handler is a trailing lambda, outside the parentheses.
    http::Server::new()
        .route("/") fn { "Hello World" }
        .listen(":8080")
}
```

**The handler and the request** ([ADR-018](adr/adr-018.md)). A handler is a lambda, so its
arguments follow Part I 5.3: they are the ones it names. The first and only one is the request,
and a handler that does not need it names nothing.

```nika
.route("/")         fn { "Hello World" }                    // names none, takes none
.route("/hello")    fn(request) { f"Hello, {request.query("name") ?? "world"}" }
.route("/fortunes") fn(request) { render(request) }
```

What a handler returns is what answers the request:

| returns | becomes |
| :--- | :--- |
| `String` | 200, `text/plain; charset=utf-8` |
| `html::Raw` | 200, `text/html; charset=utf-8` |
| `Bytes` | 200, `application/octet-stream` |
| `Response` | itself |
| `T throws` | the value on success; on failure **500 with a generic body**, the error logged |

A status code, a header or a body of the handler's own is a `Response`, built where it is
returned: `http::Response { status: 400, body: "id is required" }`.

*Design rationale:* an error's message is written for the operator, and a handler that returns
one to the client is how internal paths and driver messages end up in a bug report
([ADR-018](adr/adr-018.md)).

**A body need not be bytes the program allocated** ([ADR-058](adr/adr-058.md) D1). `Bytes` is the
shared buffer of Part I 6.6 and `Mapped` derefs to it, so a page mapped once, outside the
handler, is a body that costs a reference count per request rather than a read. The handler
answering with it duplicates the handle rather than moving it ([ADR-040](adr/adr-040.md) D1):

```nika
use std::fs

fn main() throws {
    let page = fs::map("index.html")

    http::Server::new()
        .route("/") fn { page }
        .listen(":8080")
}
```

*Design rationale:* reading the file inside the handler is a syscall, an allocation and a UTF-8
validation per request ([ADR-058](adr/adr-058.md) §1).

**A response may be a file the program never read** ([ADR-058](adr/adr-058.md) D2):
`http::File("index.html")` is a body whose bytes go from the page cache to the socket without
entering the process. Its `Content-Type` follows the extension. Its length, and any failure to
open it, are settled before the status line, because after the headers are written there is no
status code left to send (D6). Whether the transfer is `sendfile(2)`, a mapping or an ordinary
read is the library's choice at run time and not the program's to name (D3). `http::File` is
unavailable under TLS and under HTTP/2 (D5).

**When the request names the file, the name and its root arrive together.** A download route is
a different program from the one above:

```nika
// The request chose this name; the call says which directory it may not leave.
http::File(request.query("file") ?? "", fs::Root::Dir(store))
```

Every `std` function that takes a path takes its root right after it, with no default
([ADR-108](adr/adr-108.md) D1): `http::File` like `fs::map`, `fs::read` and `fs::write`. The
root is an `fs::Root`: `Dir(store)`, under which the joined name is resolved and compared
component by component, or `Anywhere`, the one way around the check, recorded per site and
listed by `nikaia --trust` (D2, D4). A name that leaves its `Dir` is `fs::Outside`, and the
handler answers 404: the call refuses, it does not rewrite (D3). Nothing is inferred about where
the name came from and no analysis follows it; a `../../etc/shadow` is stopped at the call,
before the headers are written (D6). `trusted: false` on `fs::map` is about the file's
content ([ADR-010](adr/adr-010.md) D3) and is a different question.

What the library keeps between requests is bounded, dropped when the file's identity or
modification time moves, and sized by the operator rather than the program
([ADR-058](adr/adr-058.md) D8). A page that must be held for certain is mapped by the program
itself, outside the handler, as in the first example above.

> **Implementation status:** Not implemented. `http` is not built ([ADR-038](adr/adr-038.md) §4.5), so neither the `Bytes` row nor `http::File` exists, and `fs::Root` is not built ([ADR-108](adr/adr-108.md) §5). `fs::map` exists; **`Bytes` does not** — it is the tether's container and the tether is Part I 6.6's unbuilt half, so `fs::read` hands back a `Vec[u8]` today. `docs/open-decisions.md` carries the question of where it lives.

The request's strings are **views** into the bytes the connection read: `path()`, `header(name)`
and `query(name)` yield `&str`, so a parameter used inside the request's scope costs nothing and
one kept past it has to be owned (Part I, 6.6). `query` and `header` return the nullable type of
Part I 3.5 rather than an empty string, and `method()` returns an enum rather than a string.

A handler does I/O, so it is not `sync`. It carries no `async` marker and no `await`; the build
option chooses the executor and nothing else.

**`std::html`**

The escaping a template's contract rests on ([ADR-017](adr/adr-017.md)).

```nika
pub fn escape(text: &str) -> String        // for a text node or a quoted attribute
pub struct Raw                             // "this is already markup"
pub fn Raw::new(markup: String) -> Raw     // the audit point, and the only constructor
```

A template grammar escapes **every hole, unconditionally**. There is no flag at a hole that
turns it off, and no exemption for data whose provenance is trusted: provenance is evidence about
where bytes came from, and escaping is not a place to spend evidence. The one way to say a value
is already markup is to give it the type `Raw`, so the decision is made where the value is built
rather than at each place it is used.

`escape` handles the five characters that change what HTML means in a text node or a quoted
attribute value, `&`, `<`, `>`, `"` and `'`, and returns its input unchanged when none of them is
present. It does **not** make text safe inside `<script>`, inside CSS, in an unquoted attribute or
in a URL. Those positions need different escaping, so a hole in one of them is a compile error
naming the position.

**The template, and where it is compiled.** `dsl html { … } eod` is compiled where it is
written: the body is known when the program is compiled, so it is split into literal markup and
holes there, and what comes out is the string building a hand-written renderer would do. The
escaping is thereby a compile-time property rather than a call a program has to remember.

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
itself, text renders escaped, and a type with no impl cannot be placed in a template. The
compiler emits the same call for every hole and has no way to emit a different one; the type
chooses. The rule therefore holds without the Nikaia compiler knowing what a hole's value is: it
is enforced by the language below, on every hole, including the ones the checker of
[ADR-024](adr/adr-024.md) records as `?`.

**Control flow is written as an element.** The file is markup, and an editor that highlights it
keeps working:

```nika
<table>
<for row in :rows><tr><td>{row.id}</td><td>{row.message}</td></tr></for>
</table>
```

`:rows` carries the colon because it is **captured from the enclosing scope** (ADR-007 D4): the
colon is where the template's names end and the program's begin. The loop becomes the loop of the
language below, over the captured collection, so it borrows rather than copies, as it would in
the function around the template. The position check runs through a loop's body; a hole in a
`<script>` does not become safe by being repeated.

**`std::fs` (Compiler Magic)**
File system access looks **blocking**. The compiler transforms each call into a **non-blocking** state machine backed by the runtime's reactor. User code never blocks the thread and never writes a callback.

Every function below may fail for environmental reasons, so every one of them `throws` (Appendix A.1); a missing file is not a bug in the program. **What they throw is `io::IoError`** ([ADR-158](adr/adr-158.md) D1), with `NotFound`, `PermissionDenied`, `NotText` and `Other` — a handler that only passes the failure on names nothing, and one that takes it apart writes `use std::io` and `io::IoError::NotFound(p)` (D2). None of them takes an `async` marker, and none is awaited.

**Whole-file access**

```nika
// Subject: the path, and the root it may not leave ; Config: options
pub fn read(path: Path, root: Root) -> Bytes throws                  // whole file, as bytes
pub fn read_to_string(path: Path, root: Root) -> String throws       // whole file, UTF-8 validated
pub fn write(path: Path, root: Root, data: &[u8]; append: bool = false, create: bool = true) throws
```

**Every path names its root** ([ADR-108](adr/adr-108.md)). `root` is an `fs::Root`.
`Dir(store)` resolves the name under that directory and throws `fs::Outside` where it would leave
it. `Anywhere` performs no check; it is the word a review looks for, and `nikaia --trust` lists
every site that writes it (D4). There is no default and no exception for a literal: a relative
name is resolved against the working directory, which is somebody's to change (D1).
`fs::map(&path, fs::Root::Anywhere)` is what a command-line program whose path the operator typed
writes.

**The reactor** ([ADR-038](adr/adr-038.md) D3, D4). `read`, `read_to_string` and `write` go
through the runtime's reactor. Where the machine has a completion queue, the kernel performs the
operation and reports when it is done; where it does not, such as an older kernel or a sandbox
that forbids the syscalls, the runtime's own I/O thread performs it. Which one serves is decided
when the program starts, never when it was compiled, and an operator may pin it (13.3b). The
reactor runs before the program's first statement, so an operation costs no thread start and no
thread wake-up; a pair of reads put in flight together costs nothing per pair on the completion
path ([ADR-033](adr/adr-033.md) §8.4, ADR-038 §4.3). `map` is not on the reactor: it hands back
pages the operating system owns, and there is no transfer for a completion queue to report. A
read is a slot the caller polls rather than a call that blocks its thread, and the executor runs
something else while it is in flight ([ADR-055](adr/adr-055.md) §6). What puts two reads in
flight is a program writing `overlap { … }` (Part I 8.1.2) or a `spawn`, never the compiler
([ADR-050](adr/adr-050.md) D1).

> **Implementation status:** Partially implemented. `read`, `read_to_string`, `write` with both of its options, and `map` are implemented, each without its `root` parameter ([ADR-108](adr/adr-108.md) §5); `open`, `File` and the directory functions are not implemented. The reactor and the state machine are implemented ([ADR-038](adr/adr-038.md) D3, [ADR-055](adr/adr-055.md) §6).

`read` returns **`Bytes`**, not a `Vec[u8]`: it is one shared buffer, and slices that outlive its scope are tethered to it (Part I 6.6). A parser can therefore hand back thousands of names that all point into a single allocation.

> **Implementation status:** Implemented, but for the tether. `Bytes` is **the language's** ([ADR-156](adr/adr-156.md) D1): a name written bare, one reference-counted buffer (D2), and `fs::read` hands one back (D3), so passing a file on costs a count rather than a copy. What is **not** built is the sentence's second half — a slice that outlives the buffer's scope is Part I 6.6's **tether**, which is that section's unbuilt half ([ADR-008](adr/adr-008.md)). Where a program would need it, the compiler refuses on the Nikaia line and names the buffer (`NK2303`, C.3), rather than lowering a function whose result outlives what it points into. `Mapped` does not deref to `Bytes` yet ([ADR-156](adr/adr-156.md) D6); `docs/open-work.md` carries both.

**Reading a large file: `map`, and the grammar**

There is no `fs::lines` and no `fs::bytes` ([ADR-025](adr/adr-025.md) D3). A program maps the file and walks the mapping's lines.

*Design rationale:* `lines(path)` would open the file and yield tethered `&str`, so the returned value would own the buffer and hand out views into itself, which is the one thing an iterator may not do. `fs::map(path)` owns the pages and `.lines()` borrows views of them: one value owns a buffer and another borrows from it, which is what Part I 6.6 and [ADR-008](adr/adr-008.md) rest on. The properties `lines` was for, tethered `&str`, no allocation per line and constant memory, are properties of the mapping ([ADR-025](adr/adr-025.md) D3).

```nika
let data = fs::map(&path, fs::Root::Anywhere)
for line in data.lines() { … }
```

A file that is a **record per line** is read with the grammar protocol: `@frame(boundary: "\n")` says exactly that, and the grammar drives itself over the pages, in parallel where `user_parallelism` allows (Part II, 10.7). `examples/1brc.nika`, `examples/access-log.nika` and `examples/config.nika` have that shape; none of them iterates lines.

`map` is a compile error on `wasm32-*`. What `std::fs` offers instead on that target is not decided.

**Handles**

```nika
pub fn open(path: Path, root: Root; write: bool = false, append: bool = false,
            create: bool = false, truncate: bool = false) -> File throws
```

`File` implements `Cleanup` (Part I, 6.4): the compiler flushes and closes it at the end of the scope, on the normal path and while an error is bubbling up, and a flush that fails surfaces as an error instead of being swallowed. A program calls `close()` explicitly where it handles that error at a precise point.

```nika
impl File {
    pub fn read(&mut self, into: &mut [u8]) -> i64 throws
    pub fn write(&mut self, data: &[u8]) -> i64 throws
    pub fn flush(&mut self) throws
    pub fn seek(&mut self, to: Seek) -> i64 throws   // Seek::Start(n) | Current(n) | End(n)
    pub fn len(&self) -> i64 throws
    pub fn close(self) throws                        // explicit opt-in; otherwise Cleanup does it
}
```

**Memory mapping**

```nika
pub fn map(path: Path, root: Root) -> Mapped throws     // read-only memory map
```

`Mapped` derefs to `Bytes`, so a mapped file is a tethered buffer like any other, and a parser cannot tell the difference. The pages are the buffer, and nothing is copied, which is what makes a multi-gigabyte input practical.

**Retention.** A slice that escapes the mapping's scope tethers to it, and a tether keeps the whole map alive: one twelve-byte station name can pin thirteen gigabytes. The compiler warns where a small extract outlives a large buffer and suggests `.to_owned()`. The mapping is released once the last tether is gone, which may be later than the end of the block that created it ([ADR-008](adr/adr-008.md), D8). Slices that never leave that scope cost nothing and hold nothing. At process exit a read-only mapping with nothing observable attached to it is left to the operating system rather than unmapped page by page ([ADR-009](adr/adr-009.md), D7); skipping the teardown changes nothing a program can see.

**Availability is a property of the target, and of nothing else.** Memory mapping is an operating-system service, and whether it exists has nothing to do with whether the runtime is single-threaded. `fs::map` is available at every value of every build option on any target whose platform provides it: a single-threaded program compiled for Linux, macOS or Windows maps files exactly like a parallel one. On `wasm32-*` there is no memory mapping to call, so `fs::map` is a **compile-time error** there. What `std::fs` offers on `wasm32-*` in place of `fs::map` is decided with that target (17.2).

*Design rationale:* a silent fallback to `read` would turn a constant-memory program into one that allocates its entire input, a failure the program would discover only in production ([ADR-009](adr/adr-009.md)).

**Metadata and directories**

```nika
pub fn exists(path: Path, root: Root) -> bool throws
pub fn metadata(path: Path, root: Root) -> Metadata throws   // len, is_dir, is_file, modified
pub fn read_dir(path: Path, root: Root) -> DirEntries throws
pub fn create_dir(path: Path, root: Root; recursive: bool = false) throws
pub fn remove(path: Path, root: Root; recursive: bool = false) throws
pub fn rename(from: Path, to: Path, root: Root) throws       // both names under one root
pub fn copy(from: Path, to: Path, root: Root) -> u64 throws
```

**Availability by target**

Every value of every build option has the same `std::fs` surface; only the target changes it.

| API | Native | `wasm32-*` |
| :--- | :--- | :--- |
| `read`, `read_to_string`, `write` | yes | yes — backed by OPFS |
| `open` | yes | yes — backed by OPFS |
| `map` | yes | **compile error** — the platform has no memory mapping |
| `metadata`, `read_dir`, `create_dir`, `remove`, `rename`, `copy` | yes | yes — OPFS, within the origin's sandbox |

`std::thread` (17.2) is barred by **`user_parallelism = no`**: the runtime is share-nothing under that value, so manual threading is a compile error even on a native target that has threads. `fs::map` is barred by the **target**: nothing about a single-threaded runtime prevents mapping a file.

**`std::collections` — and where keys come from**

Every input carries a **provenance**, because the compiler knows where a buffer came from (Part I 6.6):

* **Untrusted**: a remote peer chose these bytes. HTTP requests, sockets, IPC, rows read back from a database (data a user stored yesterday is still data a user chose).
* **Trusted**: the operator chose these bytes. Files, command-line arguments, the environment, anything compiled in.

A value inherits the provenance of the buffer it comes from, and a collection takes the most cautious provenance of everything put into it. Where the compiler cannot tell, across a dynamic call or from a foreign library, the answer is Untrusted.

The hasher follows from the provenance: untrusted keys get a keyed hash with a per-process random seed, and trusted keys get a fast one. Nothing else about the map changes: same table, same API, and keys are always compared in full.

*Design rationale:* a hash map whose keys an adversary chooses can be made to answer in quadratic time. A keyed hash with a secret, random seed is the defence, and it costs on every lookup; a language that cannot tell the two situations apart makes every map pay it ([ADR-010](adr/adr-010.md)).

**User code has the last word, at the place the data enters:**

```nika
// A service that processes files uploaded by strangers:
// a local path, but bytes nobody vetted.
let data = fs::map(path; trusted: false)
```

The reverse, `trusted: true`, exists for the case where the program knows the peer. Both are recorded in the ledger, so every place the program declared something safe is one list in one file, and it shows up in review when it changes. A grammar for a wire format can pin the floor for everyone who uses it, `@untrusted grammar HttpHeaders`, so that no application can lower it by accident (Part II, 10.7 and [ADR-010](adr/adr-010.md)).

`nikaia --trust` prints where the program's bytes came from, which source said so, and which hasher its maps got:

```text
$ nikaia --input 1brc.nika --backend rust --trust
input provenance: trusted
    cli::args is trusted
    fs::map is trusted
hash for a map keyed by the input: fast, fixed seed - no adversary chooses these keys
```

An untrusted map is seeded randomly, so **its iteration order is not stable between runs**. A program that needs an order asks for it explicitly.

> **Implementation status:** Partially implemented. The analysis answers for one buffer, because [ADR-008](adr/adr-008.md) gives a compilation unit one input lifetime, so the join over the sources a program calls is the per-buffer answer. Provenance is a function of the source and of `std`'s ledger, which the compiler's fingerprint already hashes, so the build cache has no separate key for it (13.1).

**Other key modules:**
* **`std::json`**: serialization using compile-time code generation, with zero-allocation parsing where possible.
* **`std::cli`**: parsers for command-line arguments, environment variables and ANSI terminal colours.
* **`std::net`**: low-level TCP/UDP sockets for building custom protocols.

### 17.2. Availability by Target and by `user_parallelism`
Some modules are available, or behave restrictively, depending on the target and on whether user code may run concurrently.

**A target without an operating system** ([ADR-119](adr/adr-119.md)) is one more column of this section. `user_parallelism` is `no` on it. `fs`, `net`, `process`, threads, memory mapping and `http` do not exist on it. Everything the language itself is carries over: `overlap`, `spawn` as a coroutine, the four doors, `Cleanup`, `Seen`, grammars, `comptime` and Chapter 16's assembly. The emitted Rust is `no_std`, the executor is the target's with interrupts as wakers, an interrupt handler is a `fn() sync` that touches no lock, a lock is a critical section the length of its block, and a build may forbid allocation after start. What the compiler promises about time and what it leaves to analysis is D6 of that record.

> **Implementation status:** Not implemented. No target without an operating system exists in the compiler.

* **`std::process`**: spawning child processes.
* **`std::thread` / `spawn`**:
    * **At `user_parallelism = yes`:** full concurrency. The primary mechanism is `spawn`.
        * **Strict implicit move:** ownership of the ordinary data used inside a `spawn` block is transferred to the new task. A handle on a shared value is the one exception and is duplicated instead, so the name outside keeps working ([ADR-040](adr/adr-040.md) D1, Part I 6.2).
    * **At `user_parallelism = no`, and on `wasm32-*` whatever it says:** direct use of `std::thread` is a **compile-time error**. The runtime is share-nothing under that value, which is what makes `user_parallelism = no` mean something and what keeps a program compatible with WASM hosts.

At `no` the runtime starts its I/O workers and nothing else, so no thread carries user code; at `yes` a pool for user code starts with it, sized by `user-pool` (13.3b) ([ADR-038](adr/adr-038.md) D4, §4.1). A task interleaves in the executor at `no`, and nothing of user code runs concurrently there; at `yes` a task goes to a pool of futures over the `user-pool` worker count and runs on a thread of its own ([ADR-055](adr/adr-055.md) §6).

> Interleaved is not concurrent.

> **Implementation status:** Partially implemented. The thread count and `spawn` at both values are implemented ([ADR-038](adr/adr-038.md) D4, [ADR-055](adr/adr-055.md) §6); the refusal at `no` is the overlap's vehicle degrading rather than a diagnostic ([ADR-033](adr/adr-033.md) §8.2b). A task holding a value that may not cross a thread is refused by the backend rather than by the compiler ([ADR-055](adr/adr-055.md) D6), and `task::scope` is not implemented.

**`std::db` (the protocol, and nothing else)** ([ADR-143](adr/adr-143.md))
`std::db` holds what two database drivers must agree on without depending on one another: the `Connection` and `Transaction` traits, the `Statement` protocol a `dsl` block's prepared statement speaks, and the values a row may carry: the language's numbers, `bool`, `String`, `Bytes`, and `T?` for `NULL`. **No SQL, no grammar, no dialect**: the compiler knows none. A dialect is a grammar in a **driver package** (`sqlite`, `postgres`, a vendor's own), which also brings the DDL grammar for the schema, the connection and the target adapter.

* **The driver checks the query while the program is built.** `dsl sqlite(schema: app) { SELECT name, email FROM users WHERE age >= :min_age } eod` hands the statement and a schema file (a `comptime` asset) to the driver's grammar. A missing table or column is a build error at the query, every `:hole` is a typed named parameter, and the grammar declares the result columns with `meta::column`, from which the compiler derives a **row type** with named, typed fields (Part II 10.5). No live database is opened while building; whether the one opened at runtime still matches the file is the driver's check at `open`.
* **Zero-blocking guarantee:** database operations are implicitly asynchronous. They never block the event loop, nor the compute scheduler where there is one.
* **Architecture adapter**, the driver's: on native targets the runtime's own I/O thread carries the blocking calls; on the web a **Web Worker** with **OPFS** (Origin Private File System) does, so a persistent database runs in the browser without freezing the UI thread.
* **No expression capture and no object-relational mapper**: a query in memory is the `Seq` combinators, the row type is the mapping, a migration is a SQL file. Dynamic SQL is a driver's `raw(text)` with untyped, `Untrusted` rows.

> **Implementation status:** Not implemented. The I/O thread the native adapter would offload to exists ([ADR-038](adr/adr-038.md) D4); it starts before `main` and runs `std`'s own code, so it exists at `user_parallelism = no` (ADR-037 D2). `meta::column` and a block's build-time arguments are not implemented, and no record decides which SQLite binding the `sqlite` driver stands on ([ADR-143](adr/adr-143.md) §4); the specification names none ([ADR-038](adr/adr-038.md) §1).

```nika
use sqlite                                    // a driver package, not `std`

let app = comptime asset("schema.sql")        // the schema, read while building

fn query_data() {
    // Transparently starts the required sidecar (thread or worker)
    let db = sqlite::open("app.db")

    // The driver's grammar checks the statement against the schema while
    // the program is built, and declares the columns; the row has fields.
    let active_users = dsl sqlite(schema: app) {
        SELECT name, last_login FROM users WHERE last_login > :since
    } eod

    for u in active_users.execute(db; since: 0) {
        println(u.name)
    }
}
```

---

# Appendix A: Error Hierarchy

Nikaia distinguishes between errors caused by the environment, which are recoverable, and bugs in the program logic, which are not.

### A.1. Recoverable Errors (`throws`)
Errors arising from external circumstances, such as a missing file or a network timeout.
* **Mechanism:** declared in the function signature with `throws`.
* **Handling:** enforced by the compiler through `catch{}` blocks or propagation.

### A.2. Unrecoverable Errors (`panic`)
Errors indicating an inconsistent program state: an index out of bounds on a **list**, whose index is the program's own arithmetic (a map read through the brackets is a `T?` and never panics, [ADR-114](adr/adr-114.md)); division by zero; **arithmetic overflow**; **a conversion whose value does not fit**; and an explicit `panic()`.

An overflow is in this list at **every** build, which keeps Part I 1.2's rule, *how, never what*, true of arithmetic. Where a program means to wrap or to stop at the limit, it says so by name (Part I, 2.2) ([ADR-043](adr/adr-043.md) D1). A conversion is in the list for the same reason, by a different mechanism: the check is in the emitted code rather than in a build option, because `as` in the language below truncates by definition. `truncating_i32` is how a program says it wanted the low digits ([ADR-043](adr/adr-043.md) D4, D7).

Two of these are also refused at compile time where the answer is already on the page: a constant that cannot fit the type it is given is refused with `NK1116`, and a division whose divisor is a constant zero is refused with `NK1118` ([ADR-043](adr/adr-043.md) D5 and D5.5). Neither takes a case out of the list: a program whose divisor the compiler cannot evaluate divides by whatever it is handed, and a zero there is unrecoverable as written here. The compile-time refusal replaces the message in the one case where no program had to run to know it (C.1).

Three build options exist (13.3), and a panic depends on **two** of them, `user_parallelism` and `target`, on different grounds:

| `user_parallelism` | Panic Behavior | Consequence |
| :--- | :--- | :--- |
| **`no`** | **Abort** | The process terminates immediately. There is no second piece of user code in flight to isolate the failure from, so the stack is not unwound; the binary is smaller for it. |
| **`yes`** | **Task Poisoning** | Only the affected task is terminated. The worker thread catches the panic (fault isolation). Resources (`SharedMut[T]`) held by the task are marked poisoned, so no other thread reads state a half-finished task left behind. |

**One end that is not a panic and is not success either.** A `cleanup-deadline` that expires ends the program with **exit status 70** (`EX_SOFTWARE`) and a message naming every resource whose cleanup was cut off, delivered on the panic path: standard error and the panic hook ([ADR-112](adr/adr-112.md)). No build option and no runtime configuration turns that into a `0`. `cleanup-deadline = "0"` does not drain and therefore never expires.

*Design rationale:* a flush that did not happen is a failure the thing that started the program has to see, and it reads the status, not a log ([ADR-112](adr/adr-112.md)).

The `target` decides independently where the machine leaves no choice: on
`wasm32-unknown` a panic is a **trap** and the module is done, whatever
`user_parallelism` says, because the host offers nothing to unwind to.

**The third build option changes nothing on this page.** `reentrancy-check` (13.3) decides whether a compiled program notices a lock taken while a lock is held. Taking one is refused at build time ([ADR-039](adr/adr-039.md) D2), so in a program the compiler accepted the check cannot fire; if it fires, the refusal has a hole ([ADR-039](adr/adr-039.md) D8). The re-entrancy panic Part II 12.2 describes at `user_parallelism = no` is that check and nothing else, so the table above carries no row for it. Poisoning is unchanged: at `no` a panic is an abort, so no task survives for which poisoning would be done, and the `yes` row is the only place it happens ([ADR-039](adr/adr-039.md) D1).

> **Implementation status:** Partially implemented. `SharedMut[T]` is implemented ([ADR-064](adr/adr-064.md)), and the re-entrancy panic at `user_parallelism = no` is carried by the cheap shape ([ADR-057](adr/adr-057.md)). `reentrancy-check` and the option to turn the check off are not implemented (13.3, [ADR-039](adr/adr-039.md) §4).

On **every** panic path, including the abort and the WASM trap, the application's **panic hook** runs first (Part I, 7.2): one global `sync` handler receiving message, location and stack trace, intended for crash dumps and reports. It rides on the backend's panic machinery, which invokes the hook before aborting even under `panic = abort` ([ADR-006](adr/adr-006.md), D6).

**The abort names the `.nika` line.** Every program carries a table of
generated line to Nikaia file and line, and the hook looks the site up before it
prints anything. An overflow, a conversion that does not fit, an index out of
bounds and a written `panic()` all read
`src/main.nika:2: the program stopped: attempt to multiply with overflow`, and
never name a generated file ([ADR-044](adr/adr-044.md)). A location the table
does not know, such as a panic inside `std`'s own Rust or a foreign crate's, is
left in the words of whoever wrote it.

> **Implementation status:** Partially implemented. The table is every line the emitter wrote from a `.nika` line, appended to the program and sorted, and the hook is installed by the generated `fn main` before the runtime starts. A user hook is not implemented: `NK2604` is catalogued and not raised (C.3).

# Appendix B: Compiler Internals & Annotations

To enforce the contextual capture rules (Part I 5.4) without hard-coding function names into the compiler, Nikaia uses internal attributes. They belong to the standard library.

> **Implementation status:** Not implemented. `@detached` is a ledger fact and not a word a program writes: whether a function-typed parameter is run or kept is inferred from the body ([ADR-102](adr/adr-102.md) D3), and the type itself is not yet in the grammar (Part I, 5.4 C). The immediate/detached rule is held by a check over `std`'s entries until then ([ADR-029](adr/adr-029.md) D4).

### B.1. Capture Attributes

| Attribute | Internal Name | Default | Description |
| :--- | :--- | :--- | :--- |
| None | `capture_mode = "immediate"` | Yes | The lambda executes within the caller's stack frame. Captured variables are **Borrowed** (`&T`). Used by `map`, `filter`, `lock.access`. |
| `@detached` | `capture_mode = "detached"` | No | The lambda escapes the current stack frame (stored, spawned, or deferred). Captured variables are **Moved** (Owned). Used by `spawn`, `defer`. |

### B.2. Standard Library Signatures

Common standard library functions are annotated internally as follows:

```nika
// std::collections::List
// Standard immediate execution
pub fn map[U](self, op: fn(T) -> U) -> Vec[U]

// std::task (Global Spawn)
// Detached execution: Must take ownership of environment
pub fn spawn(task: @detached fn() -> T) -> TaskHandle[T]

// std::task
// Scope is immediate because it waits for completion
// Where they run in parallel, a scope's child tasks must be 'sync' (see Part II, 12.7)
pub fn scope(f: fn(Scope))
```

# Appendix C: The Diagnostics Contract

Nikaia compiles through the Rust toolchain ([ADR-003](adr/adr-003.md)). The backend's error messages, about lifetimes, borrow traits and generated code, are the vocabulary a Nikaia program's author never needs. This appendix makes diagnostic quality a **testable requirement** ([ADR-005](adr/adr-005.md), D7).

### C.1. The Iron Rule

> **An untranslated backend (rustc) error reaching the user is a Nikaia compiler bug.**

The driver registers its own diagnostic emitter and intercepts every backend diagnostic. Each borrow, ownership and lifetime error class is mapped to a Nikaia diagnostic with Nikaia vocabulary, `.nika` spans and a concrete fix-it. An unmapped error is reported as an internal error, never as normal output. The error catalogue below therefore doubles as a test suite: every entry has a minimal `.nika` reproduction that must produce the documented message.

### C.2. Requirements for Every Diagnostic

1. **No prior knowledge assumed.** The message is understandable without Rust or systems-programming background. The terms "lifetime" and "borrow checker" and Rust error codes never appear.
2. **Always say what to do next.** Every error names at least one concrete way out (clone, use `Shared`, use `retain`, mark a function `sync`, move the I/O out of the lock, …), as paste-ready code where possible.
3. **Narrate cause chains.** An error caused by a change (through the ledger, 13.5) shows both sides: the edit that changed the contract and the caller that broke.
4. **Positive guarantees over prohibitions.** Where the language removes a danger structurally (tethered slices, scope waiting), the documentation and the messages state the guarantee, such as *the buffer cannot die while a token lives*, not the forbidden thing.

### C.3. Error Code Catalogue (NK codes)

| Range | Domain | Examples defined so far |
| :--- | :--- | :--- |
| `NK1xxx` | Syntax & types | `NK1101` a call passes the wrong number of arguments. `NK1102` an argument is not what the parameter takes. `NK1103` a `let` says one type and is given another. `NK1104` a `return`, or a body's last expression, is not what was declared. `NK1105` an assignment is not what the target holds. `NK1106` a struct literal gives a field the wrong type. `NK1107` a field that is not there. `NK1108` a condition that is not a `bool`. `NK1109` a call names an option the callee does not have (Part I, 5.1). `NK1110` a call reaches an item, or a field, another **package** keeps private (Part I, 9.2): *`secret` is private to `http`*, and *`http::Request.method` is private to `http`* for reading such a field and for giving one a value in a struct literal. A field's `pub` is in the ledger because a dependency's items are in the same crate below ([ADR-047](adr/adr-047.md) D2). These ten are answered from the ledger (13.5), so a call into a library is checked against the contracts the library ships ([ADR-024](adr/adr-024.md)). `NK1111` (**warning, and temporary**) a plain string holds what looks like a hole, or a doubled brace: the one-release migration to `f"…"` ([ADR-035](adr/adr-035.md) D5), and the only thing this checker warns about rather than refusing. `NK1112` a call does not pass a parameter the DSL statement it is given declares; `NK1113` a call names one that statement does not have (Part II, 10.5). Both are answered from the statement's own body, where a `:name` is written ([ADR-007](adr/adr-007.md) D5). `NK1114` is **retired**: the automatic argument names `a`, `b`, `c` are withdrawn ([ADR-049](adr/adr-049.md)), a body that reaches for one meets `NK1117`, and the number is not reused. `NK1115` a call wants a shared value and is given a plain one (Part I, 6.2). The way out is an explicit wrap at the call, `Shared(db)`, never an implicit one ([ADR-064](adr/adr-064.md) D2, [ADR-040](adr/adr-040.md) D1). `NK1116` a constant does not fit the type it is given (Part I, 2.2): at an annotated `let`, a `return` against a declared result, an argument whose parameter says what it takes, a bare `let` where an operand's declaration pins the type ([ADR-043](adr/adr-043.md) §4), a constant no type holds, and a constant reached through a name ([ADR-063](adr/adr-063.md) D2). A literal standing with nothing beside it keeps its type-less reading (Part I, 2.4), and an expression between those cases is widened rather than refused. The refusal prevents no abort, because an out-of-range literal never reaches run time; it replaces the backend's message ([ADR-043](adr/adr-043.md) D5, C.1). `NK1117` a statement is one name and nothing declares it. The grammar is scannerless, so a word it has no rule for is read as a name, and a name on its own is a legal statement; `assert c` and a leading `_000` are refused here. `1_000` is the number `1000` ([ADR-136](adr/adr-136.md)), so the help no longer names it. The help explains a word that is not reserved: `loop { … }` is answered with *write `while true`*, `const X = …` with *write `comptime`*, and `macro` and `quote` with *Nikaia has no macros* ([ADR-117](adr/adr-117.md) D2). Four things declare a name: a local or parameter in scope, a function either ledger describes, a type declared here, and a module of this program. A name this compiler cannot see is not refused (C.4). **The same code carries the prelude's two rules** ([ADR-154](adr/adr-154.md)): a `std` name that lives in a module written **without** its prefix — *`std` has `text::digit_value`* — and a module used **before** it is introduced — *`fs` is used here and introduced nowhere*. Both hand over the line to add. Asked only where `std`'s ledger has exactly one entry for the name, and never about a module of this program or a package: what needs no `use` is the list on Part I 1.3, and it is the entries that ledger keys **bare**. `NK1118` a division, or a remainder, whose divisor is a constant zero ([ADR-043](adr/adr-043.md) D5.5). A division by zero at run time stays where A.2 puts it. Both this and `NK1116` read the same constant fold: a literal, an immutable `let` whose value folded, `+ - * / %` and a negation, in an `i128`; a divisor the fold cannot evaluate is claimed nothing about. `NK1119` a `let`, a `for` binding, a lambda's argument or a struct field called `self` ([ADR-051](adr/adr-051.md) D4). `self` is the one reserved word that is a name, so one grammar rule serves declaring a name and referring to one; every other reserved word does not parse in a name position. A parameter named `self` is refused by the grammar, because the receiver takes the word. `NK1120` is **unused** and the number is not reused. `NK1121` a `?.` reaches through a value that cannot be absent ([ADR-052](adr/adr-052.md) D7); the way out is the plain `.`. Asked only where the receiver's type is known (C.4). `NK1123` a hull written a second way, or a hull of a hull ([ADR-064](adr/adr-064.md) D3): `Shared[Locked[T]]` is `SharedMut[T]`, and the message carries the replacement; `Shared(x)` where `x` is already a handle adds a second count around one value. `NK1125` a member reached off a `T?` with a plain `.` ([ADR-066](adr/adr-066.md) D6), `NK1121`'s mirror. `a?.b.c` guards `a` and nothing else, so the unguarded `.c` reaches into a `T?`; a `T?` is a type of its own (Part I, 2.3), and a member of `T` is not a member of it. The way out is the guarded form, or `??` and then the plain `.`. `NK1124` a door over several locks written wrong ([ADR-065](adr/adr-065.md)): handed something that is not a lock, handed one where several are wanted, or given a block that does not name one value per lock. A number is refused there although a literal carries no type (Part I, 2.4). `NK1126` a field or a method reached on a **type parameter** that has no bound ([ADR-074](adr/adr-074.md) D5). A `T` with no bound can be moved and passed and nothing else, because every type a caller may pick has to answer for what the body does. The message says *no bound* rather than *no such method*, because the method may exist on every type the caller passes. `NK1127` a `comptime` binding this compiler cannot evaluate while it builds ([ADR-073](adr/adr-073.md) D3, D5; [ADR-077](adr/adr-077.md) for the word). A `let` may fold; a `comptime` must. What is evaluated today is an integer or a `bool`: a literal, arithmetic and comparisons over literals and over other constants, an `if`, a **call** to a function of this program whose body is made of those, and a `for` over a range or a `while` inside such a body — D5's second stage, which is implemented. What is not is a value that is not one number or one `bool`. The way out is named in the message: `let`, for a value that was never a constant. `NK1128` a name the **language below** reserves and cannot escape ([ADR-076](adr/adr-076.md) D3): `crate`, `super` and `Self`. Every other such name is written escaped and stays a name here, so a field called `type` is fine (D1). Asked at every position that declares a name, including an item's own name (D4). `NK1129` a trait's method that an implementation **pauses** in where the declaration says `sync` ([ADR-080](adr/adr-080.md) D1, [ADR-109](adr/adr-109.md) D2). A declaration reads like a function type: without the word a method may pause. `NK1140` the same comparison for `throws`: a body that fails under a declaration without the word. `NK1130` an `impl` and the `trait` it names disagree about **which methods exist**: one the trait does not declare, or one it declares that the `impl` leaves out; two messages under one code ([ADR-078](adr/adr-078.md) §4). A trait this unit does not declare is not checked against: `impl Error for ConfigError` names the one trait the compiler reads, and silence is the correct answer about a declaration that is not here. `NK1131` a field of a **borrowed** subject handed out by value ([ADR-083](adr/adr-083.md)), such as `return self.username` out of a `&self` method ([Part I 6.8](10-nikaia-light.md)). The emitter writes no `.clone()` ([ADR-064](adr/adr-064.md) D2); the message names both ways out. Asked at a `return`, a `let` and either kind of call argument, and only where the field's type is known and does not copy: a number, a `bool`, a `char` and a view take nothing away, and a field this compiler cannot type is not claimed about (C.4). `NK1132` a `break` or a `continue` with no loop to act on ([ADR-084](adr/adr-084.md) D4, Part I 3.3): there is no loop, or a function boundary stands between (a lambda, a task, an `overlap` branch or a DSL fold's step, each a closure or an `async` block below), and a jump does not leave a function. A `catch` handler is not such a boundary and is not refused (D5). The refusal is in the lowering as well (D6). `NK1133` a statement after a `break` or a `continue`, in the same block ([ADR-084](adr/adr-084.md) D3). `break x` is the shape: `break` carries no value in this language, so a value written after it parses as a statement of its own. `NK1134` a `catch` over an expression that **cannot fail** ([ADR-091](adr/adr-091.md)). *Nothing here can fail* and *nothing here could be looked up* are two answers, and only the first is a mistake. `NK1135` a **type** nothing declares ([ADR-096](adr/adr-096.md)), or one written **without its module** ([ADR-154](adr/adr-154.md) D3): *`HashMap` is written without its module*, with `use std::collections` and `collections::HashMap` handed over. A type that lives in a module is reached through it, as a package's is; what needs no prefix is the list on Part I 1.3, and those are the names `std`'s ledger keys **bare**. Only `std`'s modules say `use std::…`, because a package's prefix is the package's name. The known set is Part I 2.2's own types, the ledger's `types` map and the declaration's own type parameters; a qualified name is left alone, because whether this build can see that package is a question with a message of its own. A bound naming a trait neither this unit nor a ledger declares ([ADR-106](adr/adr-106.md)) is the same refusal one position over, with the message saying *trait* ([ADR-140](adr/adr-140.md) §5). `NK1136` a `let` that binds several names and is given a type ([ADR-098](adr/adr-098.md)): the names are taken apart by position, and one written type cannot say which of them it is about. `NK1137` a `&` the **compiler** writes ([ADR-094](adr/adr-094.md) D1 and D4), in either position it writes one: a `for` lends the place it is given, and at a call the argument gains its `&` wherever the callee's `keeps` column says the parameter is only read. A `&` in a declaration is untouched (D6). A `&` in a position the callee keeps, in front of a copy type, at a method call, or in front of a value whose type this compiler could not work out is left alone (C.4). Asked after the fit, so a `&i64` handed to a `&Request` stays `NK1102`. `NK1138` a parameter a body **changes** where the declaration does not say `mut` ([ADR-094](adr/adr-094.md) D3). `fn fill(mut out: Vec[i64])` is where in-place change is written, and the caller's value is what changes. Raised only where the change is certain: an assignment into the parameter or into a place rooted at it, or a method every candidate entry marks `mutates`. A name a `let` has bound is no longer the parameter, which is D3's way out (`let mut v = x`), and a method no ledger describes is not refused on (C.4). Said once per parameter, with the caret on the declaration. Asked at a door and nowhere else for a fold's accumulator: `par_fold(…, fn(acc, m) { acc.record(m) })` compiles, because the emitter writes the word for it. `NK1139` a **`let`** whose value is changed, and no `mut` on it ([Part I 2.1](10-nikaia-light.md)). A code of its own beside `NK1138`, because a parameter's `mut` also decides what the caller sees, where a `let`'s is only about this body. Both read the binding in scope, so an inner block's `xs` stops being it when the block closes and an outer `mut xs` is it again. `NK1141` an `update` block that hands a value back ([ADR-110](adr/adr-110.md) D1). The block takes `mut v` and changes it, and the change is the result, so a returned value has nowhere to go. Asked of the shape only: a last statement that can only be a value, or a `return` carrying one. A last statement that is a call is left alone, because whether it comes to a value is a question about its callee (C.4). `NK1143` a call to a name an `extern "C"` block declares, **outside** an `unsafe` block ([ADR-124](adr/adr-124.md) D3, 15.1). C is not memory-safe, so the boundary is written where it is crossed. Only a name this file declared `extern`. `NK1144` a `let _ = expr` ([ADR-126](adr/adr-126.md) D2): `_` is the ignore pattern and stands where a name would be bound (a tuple position, a parameter, a `match` arm), never as a whole binding. The message says to write the expression as a statement or bind it to a name. `NK1145` a field of an `extern "C"` struct that is not a C value: text, a list, an optional, a lock, a `Shared`, a function or a struct without the word. The message names the handle as the shape to use ([ADR-127](adr/adr-127.md) D2). `NK1152` a build-time body the rule forbids ([ADR-075](adr/adr-075.md) D1, D2): a callee that can **pause**, or whose touch set is anything but the build's own parameters. Distinct from `NK1127`, which says *this compiler cannot evaluate it*; a program waits for a stage for the one and changes the callee for the other. The rule is two ledger columns and not a list of allowed functions. The same code carries the **call depth**: [ADR-075](adr/adr-075.md) D4 has no step budget, and a recursion without a base case is refused rather than taking the compiler's stack. `NK1153` an **empty list** whose element type nothing ever says ([ADR-135](adr/adr-135.md) D2): `[]` carries none, so it takes one from the first use that needs one. Asked once the whole body has been walked, and only where nothing at all uses the name; a use this checker cannot read a type out of is left to the language below (C.4). `NK1154` two elements of one list literal that do not agree ([ADR-135](adr/adr-135.md) D1). The message names the two types, or the kind (*a number* beside *text*) where an element has no type yet. Once per literal. `NK1155` the alternatives of an `|` pattern bind different names ([ADR-137](adr/adr-137.md) D1). Every alternative binds the same set, because the arm's body reads those names and does not know which alternative matched. Recursive, so an `|` inside a tuple's part is the same refusal one level down. `NK1156` a `use std::…` whose last segment is a **type** ([ADR-140](adr/adr-140.md) D5): a `use` names a module and brings no name in, for `std` as for a package, so the line does nothing at all. Where the type lives in a module the help names that module — `use std::fs` and `fs::Mapped` — and only a type keyed without one needs no line at all (Part I, 1.3). What tells a module from a type in a ledger key is the **`self`**: a type's entries are called on a value and a module's are not. A module nothing describes yet is left alone (C.4). `NK1158` a `&mut` or a `[T]` written outside an `extern "C"` declaration ([ADR-147](adr/adr-147.md) D1). Both forms exist for the C boundary and have no meaning away from it: a parameter this language may change is written `mut name: T` ([ADR-094](adr/adr-094.md) D3), and a run of elements is a `Vec[T]` or an `Array[T, N]`, each of which carries its length. Refused rather than lowered, because what `rustc` would say about `&mut i64` in a generated signature is about a file nobody wrote (C.1). `NK1159` a length handed to an `extern "C"` declaration beside a buffer it cannot be shown to fit ([ADR-147](adr/adr-147.md) D2). A `&[T]` and a `usize` right after it are one fact in C, and a count longer than the buffer is the overrun this boundary exists to stop. Two shapes are accepted and everything else asks for one of them: the buffer's own `len()`, and a constant a known length covers — an `Array[T, N]` carries its length and a `Vec[T]` does not; a zero is covered by every buffer. Nothing is claimed where the callee is not a foreign declaration, or where the buffer's type is unknown (C.4). `NK1161` a `throw` of something that is not an error (Part I 7.1). *What is thrown implements `Error`, and the `impl` line says so* — and a number, a `bool`, a character and text are Part I 2.2's own types, none of which does or ever will. Only those: a type this file declares may have its `impl` in another file of the same package, a package's type is not this compiler's to answer for, and a caught error re-thrown is a `?` (C.4). The **kind** and not the type, because a bare `3` fits every numeric type and arrives as `?` (Part I, 2.4). `NK1160` a field, an index or a **call** reaching past an `opaque` handle ([ADR-147](adr/adr-147.md) D3). A handle is an address this language never dereferences, so there is nothing inside it to name and nothing to count — and nothing here makes one either, because a constructor would have to invent an address. Refused here rather than below, where the words would name the wrapper the emitter wrote rather than the handle (C.1). `NK1157` a list literal standing where an `Array[T, N]` is wanted, with a different number of elements ([ADR-152](adr/adr-152.md) D4). The length is part of the type, so the message names **both** numbers: what a reader has to do about it is count. The literal takes the array type from its use and the element question stays with that use, under the code that position always used — an element that is not what the array holds is the `let`'s, the argument's or the field's own refusal. `NK1151` a `match` that does not cover every case ([ADR-146](adr/adr-146.md) D1). An **enum** is complete when every variant is named; anything else needs `else` ([ADR-145](adr/adr-145.md)), and `bool` is complete with `true` and `false`. A bare name catches and covers. Where the scrutinee's type is not known, nothing is claimed (C.4). `NK1150` a `match` arm written `_` ([ADR-145](adr/adr-145.md) D1): the catch-all arm is `else`, and `_` is the **ignore pattern**, for a value that arrived ([ADR-126](adr/adr-126.md) D1). Raised by the parser; the lowering is Rust's own `_`. `NK1149` a type's constructor written `Type::new` ([ADR-140](adr/adr-140.md) D2): a type is constructed the way a `.nika` file declares one, `Vec()`, `String()`, `HashMap()`, `Stats(first)` (Part I 4.2). Asked of the name and not of the position, so a constructor handed over as a value goes with it: `par_fold(M, Summary, …)`. The ledger keeps writing `Type::new`, the name the lowering uses ([ADR-011](adr/adr-011.md) D2); a qualified name is left alone for `NK1135`'s reason. `NK1148` a name declared twice in one file ([ADR-144](adr/adr-144.md) D1): a `fn`, a `struct`, an `enum`, a `trait` and a `grammar` declare a name; a **method** does not, because it belongs to its type, and a **rule** does not, because it belongs to its grammar. Across files the same rule is the module layer's message ([ADR-047](adr/adr-047.md) D1); `nikaia --input` skips the module layer. The caret is on the second declaration, and the message names both kinds. `NK1147` a grammar's rule reached through a **dot** ([ADR-140](adr/adr-140.md) D3): a rule of a grammar is a qualified name, `Json::value(input)`, and the dot is for a value's members. Raised in the checker and not in the parser, because only the checker knows `Json` names a grammar, and only where the receiver is a grammar of this file and the name is one of its rules; the message carries the whole rewrite. `NK1146` a struct literal written like a **call** ([ADR-140](adr/adr-140.md) D1): `Name { field: value }` is the literal, and `Name(first)` calls the constructor, which is also what makes [ADR-133](adr/adr-133.md) D1's options-only call unambiguous. Asked in the checker, where the name is known to denote a type, and before the call resolves (C.1). A name nothing declares is not this: `Widgit(size: 3)` is a call to something no ledger describes and is silent, and the brace form keeps `NK1135`'s claim. `NK1142` a **function type** outside a parameter ([ADR-102](adr/adr-102.md) D1, D5). A parameter may be code, `fn(Request) -> Response` with `sync` and `throws` after the result, and a parameter the callee runs lowers to a closure argument, which is what `std`'s own `map` and `access` take. D1 lets the type stand in a field, a result or a `let`, the positions the callee **keeps**; D5 lowers a kept one to a boxed closure over a boxed future, and that is not implemented. A field written `impl Fn(…)` is not Rust, so the refusal is the compiler's (C.1). `NK1162` a compound assignment on a **map** slot ([ADR-114](adr/adr-114.md) D2): `m[k] += 1` reads the slot as well as writing it, and a map read is a `T?`, so the line has to say what an absent key counts as — the message hands over `m[k] = (m[k] ?? 0) + 1`. A **sequence** is untouched, because `xs[i]` is a `T` and there is no absent case to answer for (D3). Asked only where the read is nullable, so a container this compiler cannot type is claimed nothing about (C.4). `NK1163` a name that needs **no** `use`, written with a `std` module in front of it ([ADR-167](adr/adr-167.md) D2): `io::println("x")` is a spelling a reader reaches for because every *other* `std` name wants its module, and what it got was `rustc` saying *cannot find function `println` in module `io`* about a file nobody wrote (C.1). `NK1117`'s two rules are the list enforced in the direction a name **lives in** a module; this is the same rule read the other way. Both halves are required — the module genuinely has no such name **and** the bare one is a name `std` keys — so it cannot refuse a correct program the way refusing on a module nothing describes yet would ([ADR-140](adr/adr-140.md) D5). |
| `NK21xx` | Running at once, and capture | `NK2101` a task takes ownership of a variable still used afterwards (Part I, 8.3) ([ADR-055](adr/adr-055.md) §6). Raised only where the type is known and a move takes it away: a number, a `bool`, a `char` and a view are copied, so the name keeps working; a handle on a `Shared[T]` is duplicated ([ADR-040](adr/adr-040.md) D1, D5), the exemption Part I 8.3 states; a type nothing describes is not claimed about (C.4). An assignment between the task and the later use clears it, because giving the name a value again is a correct program. `NK2102` scoped tasks must be `sync` where they run in parallel (Part II, 12.7). `NK2103` a `spawn`'s lambda names an argument, and a task is handed nothing (Part I, 8.2): `spawn` starts a body, it does not call it. `NK2104` two branches of an `overlap { … }` cannot run together, and the message names what they meet on ([ADR-050](adr/adr-050.md) D3): the touch sets of [ADR-033](adr/adr-033.md) decide whether the program was right to say the branches are independent. The same code refuses a branch that **binds** a name, because the block's value already carries every branch's result (D2). A branch this compiler cannot account for is not refused (C.4). An `overlap` is not a task (D4). |
| `NK22xx` | Locks & suspension | `NK2201` no I/O while holding locked data (Part II, 12.2) — the **third** of the three that sentence forbids, and the one that is neither of the other two ([ADR-067](adr/adr-067.md) D1 left *whether any exists* as a question and [ADR-169](adr/adr-169.md) answers it: exactly one). Reading a `fs::Mapped` is a **page fault**, a disk read that neither suspends nor takes a lock, so `NK2202` and `NK2203` say nothing about it; inside a door it is a disk read with the lock held. Both read shapes are asked — a method on it and an index of it — and the claim is the **type's own** `touches` column rather than a list of names here, so a second such type is a line in a ledger. Silence is not a claim, against this compiler's usual reading, because what is built on it is a refusal (C.4). The way out is reading what the block needs before the door. `NK2202` a `sync` function called something that can pause (Part II, 12.1), answered from the ledger (13.5). Asked in two places for one reason: a **free** call is resolved by name, and a **method** call is resolved by the type checker, because only a type checker knows what `tx.send(1)` goes to ([ADR-028](adr/adr-028.md), [ADR-149](adr/adr-149.md) D2). `NK2203` a lock taken while a lock is held: written one inside the other, reached through a chain of calls, or opened as a scope inside the block, because a scope's tasks run during the call and one of them waiting for the held lock is a deadlock ([ADR-039](adr/adr-039.md) D2, D3). The `locks` column propagates *touches a lock* over the call graph `sync` uses, with a `spawn`'s body excluded, because a task runs later and elsewhere, and a trailing lambda's counted. A `println` is one of these ([ADR-067](adr/adr-067.md) D1): it never pauses, so `sync` says nothing about it, and it takes standard output's own lock while the program's is open. Asked on `Holds` and never on the column's third answer: doubt is not permission ([ADR-010](adr/adr-010.md) D1) and not a refusal either (C.4), and what silence costs is the runtime check. `get` and `set` hold nothing open (D10), so neither is asked. The way out is asking for both at once: `access_all(a, b) fn(x, y) { … }` (Part II, 12.3). `NK2204` an assignment to a `SharedMut` directly; the message names the door, `kasse.set(42)` for `kasse = 42` ([ADR-039](adr/adr-039.md) D10). `NK2205` a `set` given a value that was **seen** in a lock, or standing under a condition that was ([ADR-039](adr/adr-039.md) D10, [ADR-111](adr/adr-111.md) D4): what a lock hands out is a `Seen[T]`, the stamp travels with the value, and the message names `update` and `set(…; after:)`. The `get` written inside the `set` is the smallest case. `set(…; after: seen)` is not asked, because there the witness answers it: if the lock still holds what was seen, every decision taken on it still holds (D5). `NK2207` an `update` or `update_all` block that assigns to its `mut v` without reading it: a `set` through the back door, refused as one ([ADR-111](adr/adr-111.md) D4). `NK2209` a call that can **pause**, inside a grammar's action ([ADR-142](adr/adr-142.md) D1), or inside a fold's `init`, `step` or `merge`, which is action code too ([ADR-092](adr/adr-092.md)). A parse that can be cut into pieces and run on several cores at once ([ADR-009](adr/adr-009.md)) is one whose steps do not wait on the world; this is the demand [ADR-050](adr/adr-050.md) makes of an `overlap` branch and Part II 12.6 of a `par_iter` lambda. Asked where the free call and the method call meet, and only where the ledger answered; a callee nothing describes is not refused (C.4). The message names the rule, because a grammar is a page of rules. `NK2206` a lambda that **pauses**, handed to a parameter whose function type says `sync` ([ADR-102](adr/adr-102.md) D2): `NK2202`'s shape one level over, because a type that says `sync` is the same assertion a declaration makes, about somebody else's code. A lambda that does less fits a type that allows more, never the other way round. Asked of what the body's calls say in the ledger; a callee nothing describes leaves it unasked (C.4). `NK2208` `set_after` written by hand on a lock: it is how `set(…; after: …)` is spelled in the language below, and writing it directly would drop the failure nothing in the contracts describes (D5, C.1). Worked through in C.6. |
| `NK23xx` | Aliasing | `NK2301` cannot change a collection while looping over it (Part I, 6.8). `NK2302` a parameter written `&str` is kept past the call it was given in, and nothing names the buffer it views (Part I, 6.6). A view inside a struct carries the buffer it points into ([ADR-008](adr/adr-008.md) D1) and a naked one does not, so the message names the struct form as the way out. Reported where the destination names no buffer: a function or method whose subject holds no view, a view handed back through the result, a view given to a task. Where the destination names one, a field of a subject that holds a view, the program is lowered instead, with the parameter written as a view of that buffer. `NK2303` is the other half of the same section: a function hands back a view of a buffer its **own body** made, which is a view that would outlive what it points into. That is Part I 6.6's `Tethered`, and the state is not built ([ADR-156](adr/adr-156.md) D4), so the program is refused with the buffer named and the two ways out — take the buffer as a parameter, or `.to_owned()`. It stands on a buffer the compiler can **name** and never on a call no ledger describes, because refusing a correct program is the worse of the two mistakes (C.4). That last case is accepted without being decided: a call on the subject may or may not keep what it is given, nothing written down says which, and it is treated as keeping it, because an analysis that fails open here emits Rust that does not compile ([ADR-010](adr/adr-010.md) D1); the cost of the safe direction is a narrower signature rather than a refusal. |
| `NK24xx` | Contract changes | `NK2401` a borrow contract change broke a caller, narrated from the ledger diff (13.5). Reserved: a `catch` that no longer covers every error that can reach it, narrated from the same diff; it needs the ledger to record the set rather than a boolean ([ADR-023](adr/adr-023.md) D1), which needs error types the compiler can lower. |
| `NK25xx` | Portability | The `Send` rules that parallel code needs ([ADR-005](adr/adr-005.md) §1 Group B), decided the same way at **both** values of `user_parallelism`, so that a library built at one stays usable at the other, and asked of a value **and a destination**, so the two codes below may answer differently ([ADR-045](adr/adr-045.md) D1): a lock goes into a task of the program's own and not into code nothing describes. `NK2501` a value that may not cross a thread is used by a task (Part II, 11.2): an **error** at `user_parallelism = yes` and a **lint** at `no`, where the task does not run and so the crossing does not happen. `NK2502` a value that may not cross a thread is handed to a call this compiler cannot see the end of ([ADR-038](adr/adr-038.md) D7's foreign runtime): an error at both values, because a Rust dependency's own threads are not bounded by a build option about user code ([ADR-037](adr/adr-037.md) D2). Worked through in C.5. `NK2503` a call into foreign code from which a **lock** is reachable through its arguments, transitively and through the fields of a struct ([ADR-039](adr/adr-039.md) D6): `NK2502`'s walk generalised, and a refusal about the call rather than about a type crossing. A call that can reach no lock is allowed without a word, and the way out of one that can is keeping the lock out of its reach (15.2). Worked through in C.6. `NK2504` a call, or a written type, reaching into a **Rust crate no ledger describes** ([ADR-104](adr/adr-104.md) D1): every analysis reads a contract at the boundary, and an undescribed crate is an absence rather than an answer. Once per crate, because the way out is one command for the whole crate; and only where the manifest declared the crate with `type = "rust"`, because refusing on a name nobody declared would refuse a correct program (C.4). |
| `NK26xx` | Failure declaration, resource cleanup & crash path | `NK2601` function must declare `throws` because a resource's implicit cleanup can fail (Part I, 6.4). `NK2602` a resource with pausable cleanup must not go out of scope in a `sync` context. `NK2603` (warning) cleanup-deadline exceeded at shutdown; lists the resources that did not finish cleanly. `NK2604` only the application may set the panic hook, and the hook must be `sync` (Part I, 7.2). `NK2606` a lambda that can **fail**, handed to a parameter whose function type does not say `throws` ([ADR-102](adr/adr-102.md) D2): `NK2605`'s shape one level over, and the whole message, because the function around the lambda is not the one that has to answer for a failure the type refuses, so `NK2605` does not stand beside it. Where the type does say `throws`, the failure travels to the caller ([ADR-029](adr/adr-029.md) D3) and `NK2605` applies. `NK2605` a **written** call that can fail, in a function that does not declare `throws` (Part I, 7.1): answered from the ledger (13.5), so it says which contract it read and grows as the ledger does. The same rule as `NK2601` and `NK2701` ([ADR-025](adr/adr-025.md) D1). Nothing else in the language says a function can fail, so accepting the program would publish the absence of `throws` as a fact about a body that contradicts it. |
| `NK27xx` | Implicit calls | `NK2701` a loop whose step can fail, in a function that does not declare `throws` ([ADR-025](adr/adr-025.md) D5). The same rule as `NK2601`: where the language performs a call nobody wrote, a failure of it fails the enclosing function. |

The catalogue grows with the implementation; adding an NK code requires adding its reproduction test and its worked example to the relevant spec chapter.

A code specified ahead of its check has no reproduction test. The obligation above is owed by a code the compiler emits; a test that cannot fail would claim a check that is not there.

`NK2605` is reported for a call whose callee a ledger describes: a function in this program, one in another module of it, or one of `std`'s, by name or as a method on a receiver whose type is known. A call nothing describes is silence rather than approval, which is C.4's property for every check here.

> **Implementation status:** Partially implemented. The codes the compiler emits are `NK1101`–`NK1113`, `NK1115`–`NK1139`, `NK1141`–`NK1163`, `NK2101`, `NK2103`, `NK2104`, `NK2201`–`NK2209`, `NK2302`, `NK2303`, `NK2501`–`NK2504`, `NK2605`, `NK2606` and `NK2701`; `NK1114` is retired and `NK1120` is unused. `NK2102`, `NK2301`, `NK2401` and `NK2601`–`NK2604` are specified ahead of the check that raises them ([ADR-055](adr/adr-055.md) §6, [ADR-047](adr/adr-047.md) D1, D2).

### C.4. What a Type Error Looks Like

Two of the `NK1xxx` family, on a file that says `io::read_to_string("input.txt")` and puts a literal in a `String` field:

```text
error[NK1101]: `io::read_to_string` takes 0 arguments, and this call passes 1
  --> app.nika:11:5
  11 |     let text = io::read_to_string("input.txt")
           ^
     = `io::read_to_string() -> String`
     help: call it as `io::read_to_string()`
error[NK1106]: `Reading.name` is `String`, and this is `&str`
  --> app.nika:12:5
  12 |     let r = Reading { name: "Hamburg", temp: 12 }
           ^
     help: write `.to_string()` to make a `String` of it
```

Three things about that shape are deliberate. **The note is the contract**, quoted from the ledger: the compiler shows the caller what the callee promised, because that is the fact the caller was working from. **The caret is on the statement**, not the expression: expression-level spans are open work, and both this checker and `NK2202` report at statement granularity until they exist ([ADR-024](adr/adr-024.md) D7). **The help is paste-ready**, as C.2 requires: `.to_string()` for text, `as i64` between numbers, and the nearest existing field when a name is close to one that exists.

A message appears only where **both** sides are written down. Where a type is not known, such as a method on a receiver `std` has no signature for, or what a `?` unwraps, the compiler says nothing, which is not the same as approving. The checker never rejects a program that is correct, which is what lets it run on every build.

**A hole is code, and is checked like code.** The expression inside `"total is {stock::total(items)}"`,
and inside a `dsl html` template's `{…}`, goes through the same path as a statement, so
`NK1101` and the rest say the same thing about it that they would say about the same expression
written on a line of its own ([ADR-032](adr/adr-032.md) D3). The `sync` analysis reads holes too,
in both directions: a pausing call inside one costs an inferred `sync` and contradicts an asserted
one. A hole whose text does not parse is reported by the emitter, which has the span, and the
checker stays quiet about it rather than raising a second error for one mistake.

### C.5. What a Crossing Refused Looks Like

The `NK25xx` pair covers the two places a value the program wrote reaches another thread. Both come from one question, **may a value of this type go to this destination?**, asked of the value's type and of where it is going ([ADR-045](adr/adr-045.md) D1). Each destination's answer does not depend on which build this is ([ADR-005](adr/adr-005.md) §1 Group B).

The two codes do not share one verdict. Into a task of the program's own a lock may go, at both values (D2). Into code nothing written down describes it may not, at both values (D3). `NK2501` therefore refuses nothing a Nikaia program can write, and `NK2502` refuses the shared value — a `Shared[T]`, whose count is chosen per value and so has no one shape a foreign signature could name ([ADR-061](adr/adr-061.md) D1), and a described type that says `crosses = false` ([ADR-123](adr/adr-123.md) D1). **A lock is the third, and it has a code of its own**: what the refusal is about there is the call and not the value, so it is `NK2503` and C.6 (15.2, [ADR-039](adr/adr-039.md) D6). One walk answers all three, and the reason it comes back with is what picks the code.

*Design rationale:* at `user_parallelism = yes` a real operating-system lock is underneath and the crossing into foreign code would be safe. It is refused anyway, so that a library written at one value stays usable at the other ([ADR-045](adr/adr-045.md) D3). The same sentence is the shared count's reason without the lock: a `Shared[T]`'s count is chosen per value, so there is no one shape of it to write a foreign signature against at either value ([ADR-061](adr/adr-061.md) D1).

> **Implementation status:** Implemented. The verdict is asked of the destination and carries **why** it refused, which is what tells `NK2502` from `NK2503`: a program that hands a `Shared[T]` to an undescribed call gets `NK2502`, one that hands it a `SharedMut[T]` or a `Locked[T]` gets `NK2503` (C.6), and all three types are built ([ADR-064](adr/adr-064.md)). No type a program can write reaches `NK2501`'s shape: the `Held` in the two shapes below is a type nothing describes.

A task runs somewhere else, so everything it uses goes with it:

```text
error[NK2501]: `counts` may not cross into a task, and this task uses it
  --> app.nika:7:5
   7 |     spawn fn { total(counts) }
           ^
     = a task runs on a thread of its own, so everything it uses has to be able to cross one (Part II, 11.2)
     = `Held[i64]` is one the records say may not be on a thread other than the one that built it, at either setting of `user_parallelism` (Part III, C.5)
     = a value may cross a thread only if it may cross any thread, so the answer is the same at both settings of `user_parallelism` and a library built at one stays usable at the other (Part III, C.3)
     help: keep the value on the thread that built it
```

A lock goes into a task of the program's own ([ADR-045](adr/adr-045.md) D2), and everything else a program can write goes with it. The shape above is what a type that answers *may not* into the program's own code gets.

A call whose body the compiler cannot see may start a thread of its own (15.2, [ADR-038](adr/adr-038.md) D7):

```text
error[NK2502]: `counter` may not cross a thread, and `hyper_shim::across_a_thread` may put it on one
  --> app.nika:14:5
  14 |     let crossed = hyper_shim::across_a_thread(counter)
           ^
     = nothing written down describes `hyper_shim::across_a_thread`, so this compiler cannot see the end of it - and starting a thread of its own is among the things it may do (Part III, 15.2)
     = `Shared[i64]` is shared, and which way a shared value is counted is chosen for each value rather than once for the type - so there is no one shape of it for code outside this language to be written against, at either setting of `user_parallelism` and deliberately so (Part III, C.5)
     = a value may cross a thread only if it may cross any thread, so the answer is the same at both settings of `user_parallelism` and a library built at one stays usable at the other (Part III, C.3)
     help: hand over what the shared value holds - a view of it or a copy - rather than the shared value itself
```

**The way out is one line.** The caller passes what is inside — a view of it or a copy — so the foreign function sees an ordinary number or connection and nothing of this language's sharing at all, the shape an ordinary function already has ([ADR-042](adr/adr-042.md) D1, D2, Part I 6.2). The same shape is the way out of the lock's refusal one section down, which is why `NK2503` is a second code and not a second rule. The message also says that the refusal was chosen, so that a decision is visible behind it.

Four things about that pair are deliberate.

**The rule is structural and transitive.** A `struct` with one field that may not cross may not cross, and the note names the field that decided it rather than the struct. The fields come from the ledger (13.5), so the rule reaches a type declared in another file for the same reason a type error does.

**Three answers, and the third is the design.** A type may cross, may not, or **nothing written down says**. The third is not permission: reading the absence of an answer as a yes is the polarity [ADR-010](adr/adr-010.md) D1 forbids. It is not a refusal either, because the compiler must never reject a program that is correct (C.4). An undecided crossing is handed on: `rustc` still type-checks the emitted crate, and the trait-bound error it raises is reported against the `.nika` line by the translation C.1 requires. Nothing is silently accepted, and nothing correct is refused. A library may end the uncertainty about one of its own types with a line in its ledger, as it ends it about pausing. The same third answer governs whether a call into foreign code can reach a lock (15.2, [ADR-039](adr/adr-039.md) D6): an undecided type is not a type with no lock in it, and *allowed silently* is for a call whose arguments reach nothing, never for one whose contents nobody wrote down.

**One verdict per destination, two severities.** At `user_parallelism = no` no user code runs concurrently, so the task above does not run and `NK2501`'s crossing does not happen: refusing it would refuse a program that compiles, and saying nothing would let a library built there turn out un-compilable at `yes`. A lint is the third answer, and it carries a note saying which of the two it is. `NK2502` is not downgraded, because a foreign runtime's threads run whatever the build option says. The destination is not a third severity and not the build option coming back in: each of the two answers is the same at both values, which is what Group B asks; its rule is about the verdict, not about the severity.

**The crossing the compiler chooses for itself gets no diagnostic.** Statement overlapping ([ADR-033](adr/adr-033.md)) puts each of a pair inside a closure that runs elsewhere, so what the pair hands back crosses a thread. Where that cannot be shown, the statements keep the order they were written in, and `--overlaps` says so among the other refusals: not overlapping is a step the compiler was never obliged to take, and costs speed rather than a program. That closure is the program's own code, so it is the same destination a task is: a result that holds a lock overlaps ([ADR-045](adr/adr-045.md) D2).

### C.6. What a Refused Lock Looks Like

Five refusals come with the rule that a lock may not be taken while a lock is
held, with the doors shared mutable state is reached through, and with a foreign
call that could reach a lock ([ADR-039](adr/adr-039.md) D2, D6, D10). Each names
a way out, as C.2 requires, and each way out is one line of code. The shapes
below are what the codes print (C.3).

> **Implementation status:** Partially implemented. `SharedMut[T]`, `Locked[T]` and the doors are built ([ADR-064](adr/adr-064.md), [ADR-110](adr/adr-110.md)), and `NK2203` (over the `locks` column, so a lock reached through a chain of calls is refused too), `NK2204`, `NK2205`, `NK2208` and `NK2503` are emitted.

A lock taken inside a lock, which is what `access_all` exists for (Part II, 12.3):

```text
error[NK2203]: `account_b` takes a lock, and a lock is already held here
  --> main.nika:9:5
   9 |     account_b.access fn(to) { to.balance += 100 }
           ^
     = the block this sits in holds `account_a` (main.nika:8), and one lock taken inside another is the inconsistent order two tasks deadlock on (Part II, 12.3)
     help: ask for both at once, which locks them in one order for everybody:
           access_all(account_a, account_b) fn(a, b) { … }
```

A chain reads the same, with one note more: it names the call, and the function
inside it that takes the lock, because that is the line to change. A scope
opened inside the block is the same refusal ([ADR-039](adr/adr-039.md) D3): the
scope waits for its tasks, so a task waiting for the held lock waits for the
block that is waiting for it.

The two that come with the doors:

```text
error[NK2204]: `kasse` holds shared mutable state, and this assigns to it directly
  --> main.nika:4:5
   4 |     kasse = 42
           ^
     = a write goes through a door, because the lock has to be taken for it (Part II, 12.2)
     help: write `kasse.set(42)`
```

```text
error[NK2205]: this `set` reads `kasse` while computing what to store in it
  --> main.nika:6:5
   6 |     kasse.set(kasse.get() + 100)
           ^
     = `set` is for a value computed outside the lock, so this takes the lock twice: once to read and once to store
     = making a new value out of the old one is what the third door is for, and it takes the lock once
     help: write `kasse.update fn(mut v) { v += 100 }`
```

The second one reads the stamp a value carries out of a lock
([ADR-111](adr/adr-111.md)): the inline `get` is its smallest case, and the same
pair spread over two lines, two functions or two requests is the same refusal,
with the note naming where the value was seen.

The lowering of a door, written as a door ([ADR-111](adr/adr-111.md) D5).
`set_after` is a function in the language below; written directly it would take
the program's two arguments, drop the failure nobody declared, and leave the
backend to report against a generated file (C.1):

```text
error[NK2208]: `kasse` has no `set_after`; it is how `set(…; after: …)` is written below
  --> main.nika:5:5
   5 |     kasse.set_after(stand + 1, stand)
           ^
     = the compare and the store happen while the lock is open once, and the door that asks for that is `set` with a witness (Part II, 12.2)
     = written this way the failure would be dropped rather than propagated, because nothing in the contracts describes this name
     help: write `kasse.set(neu; after: seen)`, where `seen` is what the lock handed out
```

The foreign call, judged by what its arguments can reach (15.2):

```text
error[NK2503]: `hyper_shim::render` can reach a lock through `state`
  --> main.nika:12:5
  12 |     hyper_shim::render(state)
           ^
     = nothing written down describes `hyper_shim::render`, so this compiler cannot see what it does with what it is handed (Part III, 15.2)
     = `state.counts` is a `SharedMut[i64]`, and a lock is what the call must not be able to reach
     help: hand it a copy of what it needs instead of the container:
           hyper_shim::render(state.counts.get())
```

The note names the **field** that decided it rather than the struct, as
`NK2501`'s does, and for the same reason: the fields come from the ledger (13.5),
so the rule reaches a type declared in another file. A call whose arguments can
reach no lock gets no diagnostic and no note. It is allowed silently, which is
not the same as a call whose contents nobody wrote down (C.5).

> **Implementation status:** Implemented. The walk is `NK2502`'s, asked once per argument and never copied ([ADR-039](adr/adr-039.md) D6): what it comes back with says *why* it refused, and a lock is what makes the refusal this one. The way out prints the path it found — the argument's own name with the field that decided it behind it — where the argument is a name or a chain of fields, and says the general form where it is neither, because an expression has no span to quote ([ADR-081](adr/adr-081.md) D2).
