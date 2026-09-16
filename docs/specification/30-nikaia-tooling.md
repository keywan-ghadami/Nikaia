# Nikaia Language Specification
**Part III: Tooling, Ecosystem & Interoperability**
**Version:** 0.0.7 (Draft)
**Date:** September 5, 2026

---

## Chapter 13: The Toolchain (CLI)

A modern programming language is more than just a compiler. It requires a suite of tools to manage dependencies, formatting, and building. Nikaia provides a single command-line interface (CLI) called `nikaia`.

**What you need installed.** A **stable** Rust toolchain, and nothing more. The
compiler emits ordinary stable Rust and hands it to `cargo`, so a Nikaia
installation assumes no unstable compiler feature and no `-Z` flag
([ADR-001](adr/adr-001.md) D1, [ADR-004](adr/adr-004.md) D1). The Rust the
compiler emits needs **1.75 or newer** — `async fn` in a trait is the lowering
that set the floor — and the number is written as `rust-version` into every
generated `Cargo.toml` and compared to `rustc --version` before `cargo` runs,
so an older toolchain is refused in this language's words
([ADR-109](adr/adr-109.md) D4).

### 13.1. Project Structure
When you create a new project (`nikaia new my_project`), the following structure is generated:

* `nikaia.toml`: The **Manifest**. It describes the project, its authors, and its dependencies.
* `nikaia.lock`: The **Lockfile**. It records *everything that determines the build*, and is therefore also the **Cache Key** ([ADR-021](adr/adr-021.md)). One file, because a reproducibility record that omits an input cannot tell you it is incomplete.
    * **Asset Hashing:** If a grammar reads an external file (e.g., `asset("schema.sql")`, [ADR-116](adr/adr-116.md)), the compiler stores the file's SHA256 hash here - and so is the **allowlist** that permitted the read, because it is a file the build read too ([ADR-072](adr/adr-072.md) D7). A build **given no allowlist reads nothing** at compile time (D1): the strict default is what happens when nothing is passed, so *"this build reads nothing while building"* is not a claim anybody has to keep true.
    * **Source Hashing:** The SHA256 of each `.nika` source that took part, so an unchanged module skips parsing and expansion entirely.
    * **Resolved Versions:** The exact dependency versions, the toolchain version actually used, and the **Nikaia compiler's own version** - a changed emitter produces different output from identical input, so leaving it out makes the cache serve stale artifacts (ADR-021 D3). The dependency versions are the one thing here that is *recorded without being hashed into the key*: a dependency bump changes the machine code Cargo produces, never the Rust the compiler emits, and Cargo's own fingerprinting covers that half (ADR-021 §5).
    * **Declaration vs. record:** `nikaia.toml` states what the project *requires*; `nikaia.lock` records what was *resolved and used* - the same relationship `Cargo.toml` has with `Cargo.lock`.
    * **Not in the lockfile:** build-time choices (every switch of Part I 1.2, opt-level, backend). They are hashed into the cache key but never written, or every change of switch would rewrite a committed file for no reason (ADR-021 D5).
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
* `nikaia describe <crate>`: Writes the draft ledger for a Rust crate the program calls (15.2, [ADR-104](adr/adr-104.md)).

**Which backend a command uses.** `nikaia build` and `nikaia run` compile through
the `rust` backend, the Stage 0 transpiler, and so does a single-file
`nikaia --input <file>.nika` that names no `--backend`: it is the default, it is
the only code generator, and it is in every installation
([ADR-004](adr/adr-004.md) D1). `--backend interpreter` runs a program instead of
producing one. `cranelift` and `llvm` are named by
[ADR-002](adr/adr-002.md) and not implemented, and are refused in their own
terms — there is nothing to install that would add them
([ADR-021](adr/adr-021.md) D9).

**Status:** `build` and `run` are built, over `nikaia.toml` translated to a
`Cargo.toml` ([ADR-002](adr/adr-002.md) D1 §5). `test`, `bench` and `fmt` are
not, and neither is `new` — so 13.1's layout is what a project has, not yet what
a command produces — nor `explain`, named in Part I 6.6 and 7.1. The binary's
subcommands are `build`, `run` and `lower-std`, and nothing else. A single file
outside a project is compiled with `nikaia --input <file>.nika`, which is not
one of these commands and stays available on its own account
([ADR-021](adr/adr-021.md) D11). `nikaia lower-std` is 13.2b's
toolchain-maintenance command and not one of these either. Both backends above
are built.

### 13.2b. Where `std` Comes From (the Sysroot)

`std` is not a package a registry resolves, so it is not reached the way 13.3's
`type = "rust"` dependencies are. A generated project depends on it **by path
into a sysroot**: a directory that travels with the compiler and holds `std`'s
sources. `NIKAIA_SYSROOT` names one; by default it is the checkout the compiler
was built from, which is why a build inside the repository needs no configuration
([ADR-002](adr/adr-002.md) D4).

* **`std` ships as sources, with its Nikaia half already lowered.** The parts of
  `std` written in Nikaia ([ADR-014](adr/adr-014.md) D1) are lowered to Rust at
  release time, and the `.rs` sits beside the `.nika` it came from. **Building
  `std` therefore needs nothing but `rustc`** — no compiler in the build graph,
  which is what keeps a project's dependency graph to what the project asked for.
  `nikaia lower-std` re-lowers it, by invoking the compiler **binary**.
* **It is compiled once per machine, not once per project.** The compiled `std`
  lives in the user's cache directory, in an entry keyed by the compiler's own
  fingerprint, the toolchain, the `target`, and the codegen flags of
  13.3's `[build.<target>]` table. Two builds that differ in any of those keep
  their own entry rather than evicting each other's
  ([ADR-021](adr/adr-021.md) D7). `NIKAIA_CACHE_DIR` moves the cache;
  `CARGO_TARGET_DIR` switches it off, because a build that asked Cargo for a
  directory gets it.
* **The newest three entries are kept, and idle ones below that are removed.**
  Each is a whole Cargo target directory and the key holds the compiler's own
  fingerprint, so a rebuilt compiler starts a new one: coexisting has to be
  bounded or the disk runs out ([ADR-021](adr/adr-021.md) D12.5). A tree written
  to within the hour is never removed, so a build running beside this one keeps
  the directory it is using.
* **`user-parallelism` is not one of those keys.** The switch reaches `std` as a
  value its runtime is started with, never as a compile-time condition, so one
  compiled `std` serves both settings ([ADR-037](adr/adr-037.md) D2).
* **The constraint that makes the pre-lowering sound.** `std`'s Nikaia half is
  lowered at **one** setting of the build switches, so nothing in it may lower
  differently per switch, and the toolchain fails its own build rather than
  letting such a file through. **`Shared` is what that rules out**, and it has
  been ruled out twice for two different reasons: [ADR-037](adr/adr-037.md) D3
  made the count follow the switch directly, D6 took it off the switch, and
  [ADR-061](adr/adr-061.md) D2 put the *emission* back on it — at one user thread
  every count is the cheap one. So a `Shared` in a `.nika` file here would be
  lowered once with the cheap count and handed to a program built at `yes`. It is
  **checked and not only written down**: every module is lowered at both settings
  and the bytes must agree, so the day one arrives it is a red build rather than a
  miscompile. `Locked` is ruled out the same way and for the same reason
  (Part II, 12.2).
* **`std`'s ledger travels inside the compiler.** `std.contracts` (13.5) is part
  of the compiler rather than of the sysroot copy it is read from, because a
  ledger is not required to be stable across toolchain versions
  ([ADR-005](adr/adr-005.md) D8) and a `std` paired with a different compiler
  would describe a compiler that is not there.

**Status:** built. What is not built is the packaging step that produces a
sysroot outside a checkout; the layout and the variable exist
([ADR-002](adr/adr-002.md) §5).

### 13.3. Manifest Configuration (`nikaia.toml`)
The manifest defines project metadata and the build switches of Part I 1.2.

They live in `[build]`, and `--target` and `--user-parallelism` override those two
for a single build — which is what a benchmark and a bug hunt need, while the
committed value is the one a reviewer sees (ADR-037 D5). `reentrancy-check` is
read from the manifest, and why it belongs there rather than in 13.3b's file is
two paragraphs down. A key `[build]` does not know is a typo and fails the build
rather than being ignored.

**What is *not* here is as much the point.** The manifest carries what the
**compiler** must know. How the program behaves on the machine it runs on — how
many I/O workers, how large the pool for user code, which I/O mechanism, how
long shutdown drains — is 13.3b's file, read at startup by whoever runs the
program ([ADR-038](adr/adr-038.md) D5).

**Why `reentrancy-check` is here and `cleanup-deadline` is not.** The two went
opposite ways, so the line between them needs saying rather than assuming
([ADR-039](adr/adr-039.md) D8). `cleanup-deadline` changes what a *running
program waits for*, and the compiler need not know it — which is the migration
note below. `reentrancy-check` changes **what the compiler emits**, and a choice
the compiler acts on is the compiler's to read. It is a cache-key dimension like
`target` and `user-parallelism` for the same reason ([ADR-037](adr/adr-037.md)
D4), and it lives in the manifest rather than on the command line alone, because
a build whose switches are not in the committed file cannot be reproduced from
it.

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

# `cleanup-deadline` was here and has moved to the runtime configuration
# file (13.3b, ADR-038 D5): how long a program waits at exit is a property of
# the machine it runs on, and a build-time key cannot be tuned by the operator.
# A manifest that still carries it compiles, and says where it went.

# `ordering` was here and is **withdrawn** (ADR-050 D1, D7): statements run in
# the order they are written, so there is nothing left for it to switch, and a
# program that wants overlap writes `overlap { … }` (Part I 8.1.2). A manifest
# that still carries the key **fails the build**, saying it is withdrawn and
# what replaced it - and not as a misspelling, because whoever wrote it meant
# it. That is the difference from the moved key above: a key that decides
# something somewhere else keeps working and says where to look, and a key that
# decides nothing anywhere may not be ignored quietly, or the program would
# differ from the file describing it.

# Does the compiled program still notice a lock taken while a lock is held
# (ADR-039 D8)?
#   "on" (default) - the check is emitted, and a violation panics where it happens
#   "off"          - it is not emitted
# Taking a lock inside a lock is refused when you build (Part II 12.3,
# ADR-039 D2), so in a program the compiler accepted this can never fire: what
# it guards is a hole in that refusal, not a mistake in your program. Every
# program the refusal accepts behaves the same at both settings - the switch
# decides only whether a violation would be *noticed*, which is why turning it
# off changes nothing a correct program means. Not a development aid to be
# removed later: it is a guarantee that can be declined - the one such escape
# this file still has, now that `ordering` is withdrawn.
reentrancy-check = "on"

[dependencies]
# A Nikaia package, **by path** (ADR-047 D2). The key is the name a `use` writes
# and the path says only where it comes from, so two libraries that both want to
# be `http` are yours to name apart. Its files are read into this program: `pub`
# is what it offers, its own `[build]` is ignored with a note - a package is
# built with the settings of the program that uses it - and the overflow checks
# of A.2 reach it, because a Nikaia dependency is part of the program rather
# than a foreign package.
http = { path = "../http" }
# A Nikaia package by **version** resolves through Cargo (ADR-103): on crates.io
# it is the crate `nikaia_http_server`, and the generated `Cargo.toml` writes the
# rename, so the prefix never reaches a `.nika` file. `"1.2"` is Cargo's semver;
# a `git` table with a `tag` is the same arm without an index. The key is an
# identifier, because it is the `use` name.
http_server = "1.2"
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

> **Status:** `target` and `user-parallelism` are read, and so is the moved
> `cleanup-deadline`, which compiles with a note saying where it went.
> `ordering` is refused in the words above. A
> `path` dependency is read, one level deep: a package that declares Nikaia
> dependencies **of its own** is refused rather than resolved, and what is missing
> there is the resolution and not the visibility rule — transitive dependencies
> are not visible either way ([ADR-047](adr/adr-047.md) §5).
> **`reentrancy-check` is specified and not built**: `[build]` does not know the
> key, so a manifest that writes it today fails the build as a typo would — the
> rule above, applied to a key the specification has and the compiler has not.
> What it would switch is not built either, because nothing refuses the nesting
> it is the self-check for and no lock exists to re-enter
> ([ADR-039](adr/adr-039.md) §4, Part II 12.2).

### 13.3b. Runtime Configuration (`nikaia-runtime.toml`)
A compiled program is tuned by the person running it, who is not the person who
compiled it. Four settings, read when the program starts
([ADR-038](adr/adr-038.md) D5):

```toml
# How many I/O threads the runtime runs. One always does, and it is the
# compiler's thread rather than yours: what runs on it is `std`'s own code, so
# it exists at `user-parallelism = "no"` too (ADR-037 D2, ADR-038 D4).
# More than one is what lets a pair of operations overlap on a machine with no
# completion queue.
io-workers = 1

# How large the pool for code *you* wrote is, at `user-parallelism = "yes"`.
# "0" means as many as the machine has, which is not the same as a count
# somebody typed. Read at both settings and used at one, because a
# configuration file that silently drops a key you wrote is worse than one that
# reads a key it will not use.
user-pool = 0

# Which mechanism serves a file (ADR-038 D3).
#   "auto"     (default) - the kernel completes it where this machine can, and
#                          the blocking path where it cannot. Decided when the
#                          program starts, never when it was compiled: a binary
#                          built on a machine with io_uring runs on one without.
#   "uring"              - pinned to completion. A machine without it is a
#                          refusal to start, not a silent fallback - the point
#                          of pinning is to find out.
#   "blocking"           - pinned to the blocking path, whatever the machine has.
io-method = "auto"

# How long the runtime waits at program end for pending resource cleanups
# (flushes, rollbacks, connection shutdowns — see Part I, 6.4 and ADR-006 D5).
# Generous default: "30s". On expiry, remaining cleanups are cancelled (their
# synchronous fallback runs) and the program ends with exit status 70 and a
# message naming every resource that did not finish cleanly, on stderr and
# through the panic hook — never on stdout, and never as a 0 (ADR-112). "0"
# disables draining. This deadline
# cannot hang: the timer runs in the runtime itself, and cancelling a cleanup
# always terminates (the fallback cannot pause).
cleanup-deadline = "30s"
```

Four keys and no fifth. A key outside them is a typo and fails, the same rule
`[build]` follows. The file is read from the working directory, and
`NIKAIA_RUNTIME_CONFIG` names one outright — for one binary in several
deployments. No file at all is the defaults above.

A build switch is **not** an operating property and does not belong here:
`target` and `user-parallelism` change what the program *means* and stay in
`nikaia.toml` (ADR-037 D5).

> **Status:** built, and its four settings reach the runtime. What reads them is
> `nikaia_std::rt`; `cleanup-deadline` bounds the drain of pending I/O, and an
> expiry ends the program with **70** on the panic path exactly as written above
> ([ADR-112](adr/adr-112.md)). The parked-cleanup queue
> [ADR-006](adr/adr-006.md) D3 describes does not exist yet, because `Cleanup`
> does not — so what the message counts today is the I/O operations that were
> abandoned, and it names resources when that queue lands. Under
> `panic = "abort"` the process is gone with the abort's own status before the
> exit code can be set: that profile keeps the message and loses the number. **The file's name and search path are not
> decided by an ADR** — D5 names the four settings and says "read at startup",
> and this spelling is the implementation's choice until a record makes it.

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
| `signature` | fn | its parameters, its **options** and its result, as the source writes them: `"(path: ?, data: ?; append: bool = false, create: bool = true)"`. An option carries its default, because a call that leaves one out still passes a value and only the declaration knows which (Part I, 5.1). A method's receiver is the first parameter, so a caller reads the arguments off one list either way. A generic parameter is recorded as a **variable** — `$T` — so a caller binds it from what it passes and reads the result off the same signature ([ADR-074](adr/adr-074.md) D2): `hand` records `"(x: $T) -> $T"` and a call that passes an `i64` gets one back. It used to be `?`, which said only that nothing was known. `Self` is still `?`, because no call site binds it. Shared mutable state is written `SharedMut[T]`, which is the one name the language has for it, so no internal spelling can reach an entry a reader sees ([ADR-039](adr/adr-039.md) D9) |
| `borrowed` | type | ADR-008 D6: `@borrowed` was asserted in the source |
| `fields` | type | every field with its type: `["name: &str", "temp: i32"]` |
| `tethered` | type | the fields that hold a view, directly or through another type that does |
| `trait."…"` | table | a trait, with its methods as ordinary `fn` entries — signature, `sync`, `throws` — and no `fields`; a bound and an `impl … for` must name one ([ADR-106](adr/adr-106.md) D3). *Specified, not implemented.* |
| `impl."A for T"` | table | an `impl`, in the ledger of the package that wrote it; whether `T` implements `A` is the union over every ledger a program reads plus its own, and no ledger claims completeness ([ADR-106](adr/adr-106.md) D4). *Specified, not implemented.* |
| `crosses` | type | a value of this type **may cross a thread** ([ADR-005](adr/adr-005.md) §1 Group B, `NK25xx`). Written by hand and never inferred, because it only ever answers for a type whose parts this compiler cannot walk: a Nikaia `struct` records its `fields`, and the check walks those. Its absence is "nobody said" and not "it may not" — and "nobody said" is not permission, so the compiler will not put such a value on a thread of its own choosing |
| `touches` | fn | which resources it reaches and whether it reads or writes them — `["file(path) write", "stdout write"]` ([ADR-033](adr/adr-033.md)). **Absent means it touches everything**, so a function nobody has described orders against everything and stays where it was written. *Specified, not implemented.* |
| `locks` | fn | whether the body may **acquire a lock**, anywhere it reaches: the property that decides whether a call may appear inside an open lock ([ADR-039](adr/adr-039.md) D3). Propagated over the same call graph as `sync` and from the opposite end — nobody touches a lock until something reached says it does. **An absent entry means it touches a lock**, which is the one inverted key in this file; see below. It is coarse on purpose: it says "a lock", never *which* lock, so it can never answer an ordering question. That is `touches`'s line above, and the two must not be mistaken for one another ([ADR-039](adr/adr-039.md) D4). *Specified, not implemented.* |
| `sharing` | fn | which of its `Shared` positions are one allocation, and which reference count each of those classes gets — `["counts \| <result>: plain", "hits: atomic"]` ([ADR-037](adr/adr-037.md) D7). Two facts in one line, and the first is the one a caller could not work out for itself: `counts` and the result are **the same allocation**, so they have the same count whichever side of the call decides it. The count is `atomic` unless the compiler proved that nothing crosses a thread with the class. **Absent means the function has no `Shared` position**, which is almost every function — not "nobody said", because the atomic count is the floor and the worst an entry can say is `atomic` |

**`sharing` is the sixth of these and the only one that cannot make a program wrong.** The other five are read as permissions of one kind or another — a caller that trusts a wrong `sync` puts a pausing body inside a lock, and one that trusts a wrong `locks` puts a second lock inside the first. This one records an **optimisation** on a floor that is already safe ([ADR-037](adr/adr-037.md) D6): the worst a class can be given is the atomic count every `Shared` would otherwise have, so a ledger that says nothing, or says `atomic` about everything, still describes a program that runs correctly and merely pays about 9 ns per clone-and-drop pair for the privilege. What it buys is that the classes **compose** across a call: a build that can read a dependency's sources continues the analysis through its published functions without the two reference counts ever becoming two types.

`signature` and `fields` are what make a *type* checker possible across a boundary whose bodies are not visible — the `NK1xxx` diagnostics above are all answered from them ([ADR-024](adr/adr-024.md)). They are also where the ledger's `?` earns its keep: it is **the absence of a claim**, and a checker reports a mismatch only where both sides are written down, so a contract that says less makes the compiler quieter and never wronger.

Only what is *true* is written: a `sync = false` on every entry would treble the file and say nothing, and a diff should show a promise being made or withdrawn. **An absent `sync` therefore means not `sync`** — while an absent *entry* means nothing is known and a caller may not assume. That distinction is what makes the file worth shipping rather than deriving.

**`locks` is the one key where absence means the opposite, and that is written here rather than left to be worked out.** **An absent `locks` means the function touches a lock.** For `sync`, absence lands on the restrictive answer and costs nothing. For this key it would land on *permission*, and an entry nobody wrote would be a hole through which every rule about taking a lock inside a lock falls — reading the absence of an answer as a yes is the polarity [ADR-010](adr/adr-010.md) D1 forbids ([ADR-039](adr/adr-039.md) D5). So this is the one key whose negative is worth the space: a function that reaches no lock says `locks = false`, and a function that says nothing is read as reaching one.

**`sync` is written down in two ways, because it is arrived at in two ways** ([ADR-027](adr/adr-027.md)). `sync = true` is a promise the *source* made, and `NK2202` is what the compiler says when the body contradicts it. `sync = "inferred"` is a promise the *body* implies: nothing the function calls can pause, so it cannot pause, and saying otherwise would be the ledger recording something it had already read and knew better about.

A **caller** does not distinguish them. Both mean "this cannot pause", both satisfy the `sync` half of what `access` and `par_iter` require, and any code that asks the ledger the caller's question gets one answer. A **diff** must distinguish them, and that is the whole reason the file spells them differently: withdrawing an asserted `sync` is a decision someone made and has to have meant, while losing an inferred one is a *consequence* of an edit somewhere else — usually in a function further down. The two deserve different sentences, and a `bool` cannot produce them.

**`sync` is one of two conditions for a body inside a lock, not the whole of it.** Since [ADR-039](adr/adr-039.md) D3 a body goes inside a lock only if it cannot pause **and** reaches no lock of its own — the first is `sync`, the second is `locks`, and a caller that checks one of them has checked half.

**`throws` is a set for the same reason**, and it is the second key this argument has been made about. A boolean answers "can this fail", which is what a *caller* needs in order to declare its own `throws` — and nothing else. A `catch` needs more: whether it still covers everything that can reach it. That question has an answer only if the ledger names the errors, and the answer changes when a function three modules down gains a `throw`. Recorded as a set, the diff says which error appeared and which `catch` stopped covering its arrivals; recorded as a boolean, it says nothing at all, because `true` was already `true`. The narration is `NK24xx` (Appendix C), the same machinery a changed borrow contract uses.

**A function that runs somebody else's code says so** ([ADR-029](adr/adr-029.md)). `xs.map fn(n) { n + 1 }` cannot pause and `xs.map fn(n) { io::read()… }` can, and they are the same `map`. A ledger entry has to hold for every caller, so without a way to say "it depends" a higher-order function has to commit to the pessimistic answer — and since `access`, `par_iter` and a scope's tasks all gate on a `sync` lambda, that commitment reads as *no `map`, `filter` or `fold` inside a lock, over any lambda at all*.

`sync = "from(f)"` is the way to say it, naming the parameter that decides. A caller reads it as **"this call adds no pausing of its own"**, which is sound because the lambda runs *during* the call: its body is part of the function that writes it, and that function has already counted its calls. `from` adds nothing because there is nothing left to add.

That reason is also the limit. A parameter the callee **stores or spawns** — Part I 5.4's `@detached` — breaks it, because then the lambda's calls belong to nobody the caller is counting. `from` is for an immediate lambda only, and until the ledger can spell `@detached` that rule is held by a test over `std`'s own entries rather than by the file format.

**`locks` needs no `from(f)`, and the reason bounds it the same way.** `sync` needs a fourth state because an entry for `map` has to answer a question whose answer is in the caller's lambda. `locks` is never asked of an entry at the moment it matters: the refusal is decided on the caller's own body, where the lambda is in front of the check, so what `sync` has to summarise in an entry this reads directly ([ADR-039](adr/adr-039.md) D3). The premise is the same immediate lambda. A lambda the callee **stores** may run while a lock is open somewhere this walk cannot see, which is [ADR-039](adr/adr-039.md) D7's case; a lambda it **spawns** runs later and elsewhere and is neither.

**A signature may name its receiver's type arguments** ([ADR-031](adr/adr-031.md)). `HashMap::entry` is written `(&HashMap[$K, $V], key: ?) -> Entry[$V]`: the result holds whatever the map holds. At a call site the receiver's actual type binds the variables and they are substituted away, so `HashMap[&str, Stats]` makes that result an `Entry[Stats]`, and `and_modify`'s `fn(&$V)` a `fn(&Stats)` — which is what gives the `s` in `fn(s) { s.add(t) }` a type at all.

**A sequence has a name in this language, and a container keeps its own** ([ADR-105](adr/adr-105.md)). `Seq[T]` is elements of `T` produced step by step — what `keys()`, `chars()`, `io::lines()` and `xs.map fn …` hand back — and `sync` or `throws` after it say what one step may do, as they do after a function type. A `Seq` is consumed by walking it, so a second walk is refused; a `Vec[T]` or a `HashMap[K, V]` is a container, walked by view and as often as one likes. `Par[T]` is what `par_iter()` hands back, and a lambda handed to it must be `sync` (Part II 12.6). Neither word is in a program's type grammar. **`Seen[T]` is what a lock hands out** ([ADR-111](adr/adr-111.md)): `get` is `-> Seen[$T]`, the result of `access` is a `Seen`, the stamp sticks through arithmetic and through calls to entries whose `touches` names no lock, and a `set` given one is refused. It is a type here and in the checker and nothing in the language below — a `Seen[i64]` is emitted as an `i64`. It **is** in a program's type grammar, at the two places it is written: a struct field, and a parameter of a function that touches a lock.

> **Status:** not built — nine `std` entries say `-> ?` where they would say
> `Seq[…]`, and no method has a `Seq` receiver. [ADR-105](adr/adr-105.md) §5 is
> the order of work.

**An unbound variable becomes `?`, never a name**, and that is the whole of why this is safe. A variable that survived into a comparison would make the checker report that `i32` is not `$V` — the false positive ADR-024 D4 erases generics to avoid. Here it cannot survive: it is bound and replaced, or it is the absence of a claim. A map built by `HashMap::new()` says nothing about what it holds, binds nothing, and the chain stops helping rather than guessing.

**A variable says what flows *out*; `?` stays for what flows *in*.** It may appear in a result and in a lambda's parameter type, and never in an argument. What flows out is a promise the ledger makes and is wrong on its own account; what flows in is a constraint on somebody's program — and the language below deliberately accepts more than its type parameters suggest, so a variable there would reject correct code. Binding itself is narrow on purpose: from the receiver, by position, one pattern. This is the first inference in this file's type language rather than more vocabulary, and widening it is meant to be a decision rather than a diff.

The type it names is a **function type**, `fn(&Stats)`, which is the other half of the same decision: it says what the lambda is handed, so that the `s` in `fn(s) { s.add(t) }` has a type and what it is called on can be resolved. Only a ledger writes one — Nikaia's grammar has no syntax for a function type, so no source program can declare a parameter of that shape.

The two are also arrived at with opposite caution, which is worth stating plainly because it looks like an inconsistency and is not:

* The **check** on an assertion is conservative in the *permissive* direction. It reports only calls it can prove will pause, so it never rejects a correct program. A call it cannot resolve is not an error.
* The **inference** is conservative in the *restrictive* direction. It claims `sync` only where every call resolves and every callee is `sync`; that same unresolvable call costs the function its claim. The entry is **shipped**, and a consumer will read it and put the function inside `access` — a wrong `sync` there is a pausing body inside a lock, and a wrong `locks` is a second lock taken inside the first. The ledger already settled this for provenance: an analysis that fails open is a vulnerability generator.

**What counts as resolvable is the type checker's answer, not a second opinion** ([ADR-028](adr/adr-028.md)). A call by name — `helper(x)`, `io::read_to_string()` — is looked up directly. A *method* call needs the receiver's type, and the compiler has one module that infers types; it records where each method call went, and the inference reads that rather than growing an inference of its own. So what the two analyses above can see grows whenever the type checker can name more, and neither of them changes when it does.

The gap between those two polarities is not a defect to close. It is exactly where a person writes `sync` by hand: "I know this cannot pause, hold me to it" — the same move `@borrowed` makes in Part I, 6.6, and checked the same way. What shrinks the gap is not a change to either analysis but a ledger that describes more (ADR-024, ADR-028), at which point more functions earn the promise on their own and nothing else has to move.

One consequence is worth stating for a library author: **writing a signature down is what lets your callers be `sync`.** A method with no entry is an unknown, and an unknown costs every function that calls it its inferred promise — so a library that ships thin contracts makes its consumers' code unusable inside `access` and `par_iter`, however pure that code is. This is the same fact 13.5 opens with, met from the caller's side.

**Which inference wrote it.** The header carries `inference`, because a ledger produced by reading signatures is not one produced by reading bodies and must not be mistaken for it. Today's bootstrap compiler writes `stage0-signatures+sync-bodies+throws-bodies+sharing-bodies`, and the name says which half is which: the borrow contract is the widest one the signature supports — a result that is a view may point into any view it was given — while `sync` ([ADR-027](adr/adr-027.md)), the *errors* a `throws` names ([ADR-023](adr/adr-023.md) D1) and `sharing` ([ADR-037](adr/adr-037.md) D7) are each read off the **body**. It wrote `stage0-signatures` before any of them, and a ledger regenerated by a compiler that reads one more body therefore shows a header change; the mechanism that narrates it is the one this paragraph exists for. The `toolchain` recorded is **Nikaia's** version, not `rustc`'s: these contracts are decided by this compiler and never by the one it emits code for.

**Distribution.** Published packages ship their ledger, so downstream projects build against stable contracts and receive identical diff-based explanations when a dependency upgrade changes one. `std` ships `std.contracts`, and it is the file a program's compiler reads when the program calls `io::…` or `fs::…`. A library whose implementation is partly in another language cannot have all of its contracts inferred, so those are **written in the ledger and reviewed like code**, marked as such, while the ones that can be inferred are regenerated and checked against the sources by the library's own tests ([ADR-020](adr/adr-020.md) D5).

**A consumer reads a dependency's ledger; it never derives a dependency's contracts itself** ([ADR-100](adr/adr-100.md)). A package's ledger is written by the package's own build, in which its own dependencies are in view, and read by every consumer — the rule `std` has, for every package. The inference that writes it runs over the **package** as one graph, so a call from one file of a package to another resolves (D2); a call into a dependency is answered from that dependency's ledger; only a call into code no ledger describes is unresolved, and fails closed.

**And it is believed only while the sources it came from are unchanged.** The header records, per unit, the SHA-256 of the file the entries were derived from — the hash `nikaia.lock` already holds. At a consumer's build a dependency whose sources hash as recorded is believed and nothing is inferred; one whose sources changed has its ledger derived again, written, and the difference narrated; one with a ledger and no sources is believed. A dependency is never believed against its own sources, which is what keeps the fast path honest: a stale ledger is a hash that does not match, and that is a derivation rather than a belief (D3). The build is ordered by the dependency graph, so a ledger exists before its consumer is checked (D5), and a mismatch the language below reports at a package boundary is translated as *the ledger of that package does not match its sources* (D6).

> **Status:** built, except D6. A package's units are inferred as one graph, its
> ledger is written in its own root by its own build in dependency order, the
> header carries a SHA-256 per unit, a consumer believes that ledger while the
> hashes match and derives that package again where they do not, and `--locked`
> compares each package's ledger byte for byte. What is not built is D6's
> translation of a boundary mismatch the language below reports — the message
> for a hash edited by hand ([ADR-100](adr/adr-100.md) §5).

**Version control.** Commit `nikaia.contracts`. Merge conflicts resolve like lockfile conflicts: accept either side and run `nikaia build` to regenerate. The recorded `toolchain` hash lets the compiler detect when a toolchain upgrade (not your code) changed inference results; in that case the build output states explicitly that the contract changes were caused by the toolchain update, not by your code.

**Determinism guarantee.** The ledger is a **pure function of (source tree, toolchain)**: the same sources and the same pinned toolchain produce a byte-identical `nikaia.contracts` on every machine, every run, with any thread count. This is a hard guarantee (see [ADR-005](adr/adr-005.md), D8, including the implementation ban list and the CI tests that enforce it); a violation is treated as a compiler bug. Two consequences worth knowing:

* There is exactly **one** ledger per project — it is valid at every setting of every switch. Borrow contracts and tether relationships are switch-independent by design; switch-dependent checks (such as thread-safety rules) are performed by the compiler directly and are never recorded in the ledger.
* Ledger stability is **not** promised across toolchain *upgrades* — a newer compiler may infer better contracts. The toolchain hash plus the explicit "caused by the toolchain update" narration make such diffs self-explaining instead of alarming.

**Verification mode (`--locked`).** `nikaia build --locked` (and CI setups) verify instead of update: the compiler regenerates the contracts in memory — the program's, and those of every path dependency it has sources for — and compares each byte-for-byte against its committed `nikaia.contracts`. Any difference fails the build with the narrated contract diff (see `NK2401` above). This is the one place contracts are compared rather than hashes; a development build compares hashes and derives only what changed ([ADR-100](adr/adr-100.md) D3, D4). Because of the determinism guarantee, this check is exact and needs no tolerance or semantic comparison — the recommended CI line is simply building with `--locked`, which is equivalent to `git diff --exit-code nikaia.contracts` after a regular build.

---

## Chapter 14: Testing and Quality Assurance

Testing and verification are first-class citizens in Nikaia.

> **Status for the whole chapter:** not built. `test` and `bench` are parse
> errors; `assert` is not a keyword — it is not on Part I 2.1's list, because that
> list is what the grammar *has* and this chapter is unbuilt — so `assert cond`
> parses as two statements and is then **refused** by `NK1117`, *"nothing declares
> `assert`, and this statement is just that name"*. `assert(cond)` is a call to a
> function of that name and `assert cond, "message"` does not parse at all. There
> is no `nikaia test` or `nikaia bench` command (13.2), so no fuzzing, no
> `impl Generator` dispatch, no `--with-asserts` and no `--history`.

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

> **Status:** not built. `extern "C"` is a parse error, and `unsafe` is not a
> keyword — it is not on Part I 2.1's list, because that list is what the grammar
> *has*. So `unsafe { … }` parses as a name followed by a block and is then
> **refused** by `NK1117`, *"nothing declares `unsafe`"*: the example above does
> not mean something other than what it says, it does not compile.

> **And when it is built, it is a target.** Talking *to* C is what this section
> writes; letting C talk *in* is the other direction, and it takes the threads
> away from the program — an exported entry point may be called twice at once from
> threads the caller owns, which `user_parallelism` never said yes to. That switch
> bounds **your** code and cannot answer for a caller's, so the answer lives where
> the answer to *"what is outside this program"* already lives
> ([ADR-062](adr/adr-062.md) D1). Such a build is one artifact, safe at its
> boundary: the entry points and everything they reach take the safe shape, the
> rest of the library keeps the per-value answer.

### 15.2. Rust Integration (Deep Integration)
Nikaia treats Rust Crates differently than C libraries. Because Rust has a strong type system, Nikaia can verify safety properties.

**A value handed to a Rust function may reach a thread that function owns.** A Rust crate may bring its own runtime and its own threads ([ADR-038](adr/adr-038.md) D7), so a call whose body this compiler cannot see is a call that may put what it is given on a thread of its own — and a value may cross into a foreign thread only if it may cross *any* thread. The compiler refuses the crossing it can decide about as `NK2502` (Part III, C.5), at **both** settings of `user_parallelism`: that switch bounds what *your* code runs at once, and a foreign runtime's threads are not yours.

The rule reaches exactly as far as the Rust signature is true. A Rust API that declares a type safe to send when it is not puts the value on another thread with nothing complaining, and no check in the frontend can see that: where Nikaia reads the signature, the value is crossable by declaration. That is the one place interoperability costs a guarantee rather than only convenience, and it is why a narrowing shim is worth reviewing like the boundary it is.

**A call into foreign code is judged by what its arguments can reach.** Foreign code can only touch what it reaches, and this language has no global mutable data, so reachability is the whole question. If no lock is reachable from the arguments — transitively, and through the fields of a struct — the call is allowed, and the compiler says nothing about it at all. Otherwise it is refused as `NK2503` (C.3, worked through in C.6), and the way out is keeping the lock out of what the call can reach: hand over a copy of what it needs. This **extends** the foreign-thread rule above ([ADR-038](adr/adr-038.md) D7) to locks and displaces nothing in it ([ADR-039](adr/adr-039.md) D6). A **lambda** among the arguments is the one thing no type can answer for — a lambda is its captures, and nothing writes those down — so whether it touches a lock is read off its body (Part II, 12.3; [ADR-039](adr/adr-039.md) D7).

**"No lock reachable" is an answer, not the absence of one.** Where nothing written down says what a value contains, the question is *undecided* — C.5's third answer, which is not permission and is handed on rather than accepted. An undecided type is therefore not a type with no lock in it. The case that makes this worth its own sentence is a ledger entry with an **empty** field list, which is what a type whose fields are Rust has: read as a structure that looks like "nothing inside", and it means "nothing recorded" (13.5).

> **Status:** not built, and the reason has moved. The rule needed a lock the
> compiler knows, and it has one since [ADR-064](adr/adr-064.md) — so a call
> **can** now have one among what its arguments reach. What is missing is the
> reachability walk itself: `NK2503` is catalogued and not emitted (C.3).
> `NK2502` above is built.

**Mapping Types**
* Rust `i32` -> Nikaia `i32`
* Rust `i64`, `u8` -> Nikaia `i64`, `u8` — the rest of the numeric surface (Part I, 2.2)
* Rust `&str` and Rust `String` -> Nikaia `String`, whose state the compiler
  picks ([ADR-107](adr/adr-107.md)): a Nikaia `String` crosses to Rust `&str`
  for free, and to Rust `String` only by a `.to_owned()` the program writes
* Rust `Option<T>` -> Nikaia `T?` (Nullable)
* Rust `Vec<T>` -> Nikaia `Vec[T]`, and `HashMap<K, V>` -> `HashMap[K, V]`
* Rust `Rc<T>` **or** `Arc<T>` -> Nikaia `Shared[T]`. **One Nikaia type, two Rust
  ones**, and which it becomes is the compiler's to decide per value
  ([ADR-037](adr/adr-037.md) D7). So it is the one row of this table that does not
  cross: a `Shared[T]` handed to a call whose body this compiler cannot see is
  **refused**, because a Rust signature names one of the two shapes and this
  program may be using the other one for that very value
  ([ADR-061](adr/adr-061.md) D1 — the same refusal a lock gets, for the same
  sentence). **The way across is what is inside**: a view or a copy. A foreign
  library that means to keep the value clones it into a hull of its own anyway.

**A crate is described before it is called** ([ADR-104](adr/adr-104.md)). A call
into a Rust crate no ledger describes is refused, and the message names the
command: `nikaia describe <crate>` reads the crate's `pub` signatures — from
rustdoc-JSON where the toolchain offers it, from the sources where it does not —
and writes a draft entry for every function the program calls and the types
those signatures name, translated by the table below. The draft is committed as
`contracts/<crate>.contracts`, believed while the crate's version and source
hash hold, and **reviewed like code**: what a signature cannot say (`touches`,
`locks`) is written fail-closed, what neither reader can read is written `?`,
and a signature that lies is the reviewer's to correct. Every analysis then
reads an entry at the boundary, never an absence.

> **Status:** not built — a foreign call is silent today, and `nikaia describe`
> does not exist. [ADR-104](adr/adr-104.md) §5 is the order of work.

**Thread Safety (Send/Sync)**
Nikaia decides whether a value may cross into foreign code from the **Nikaia type
of the argument**, not from the Rust crate.

* A type this compiler knows may cross is allowed in a `spawn` task and in a
  foreign call.
* A type it knows may not — a `Shared[T]` that is an `Rc<T>` on this side — is
  refused as `NK2502` (C.5), and the diagnostic names the Nikaia type, because
  that is the one the program wrote.

> **Status:** **no crate metadata is read**, and this section used to say it was.
> A foreign call is one no ledger describes ([ADR-024](adr/adr-024.md)), and the
> verdict is taken on the argument's Nikaia type — so there is no reading of a
> Rust crate's `Send` implementations, and there is no message about an
> `Rc<i32>` in Rust's words. What is built is `NK2502` on the crossing this
> compiler can decide about, at **both** settings of `user_parallelism`
> ([ADR-038](adr/adr-038.md) D7), and C.5's third answer — *undecided* — for
> everything else. Reading the metadata would be a different and larger
> mechanism; nothing in this repository needs it yet, and the sentence claiming
> it is gone rather than left standing.

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
Compiling with `nikaia build --target=wasm32-unknown` produces compact binaries:
the runtime a `no` build starts is the I/O worker and nothing else
([ADR-038](adr/adr-038.md) D4), and no OS-level mutex is generated.

> **Status:** the mutex half is true, and the **atomic** half was written twice
> the wrong way round. A `Shared[T]`'s owner count is chosen **per value**
> ([ADR-037](adr/adr-037.md) D7), and at `user_parallelism = no` every one of them
> is the cheap count ([ADR-061](adr/adr-061.md) D2): there is one thread of yours,
> the runtime's own threads carry no code you wrote, and since D1 a `Shared` may
> not leave the program into code nothing describes — so nothing can cross and
> nothing needs the atomic one. What does *not* move with the switch is the
> **verdict** ([ADR-045](adr/adr-045.md) D1): whether a value may cross is a
> question about a type and a destination, and a call whose body this compiler
> cannot see is refused a lock and a `Shared` at both settings alike.
> **Measured, not argued**: `benches/refcount` is what settled
> the cost, and it is small enough that one count per program is not the place to
> win it back.

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
*   **`unsafe`:** this was the keyword's only specified use. It remains
    **held open** for the FFI work of 15.1 rather than being dropped from the
    specification — and it is deliberately *not* on Part I 2.1's reserved list,
    which is the list of words the grammar has a construct for
    ([ADR-051](adr/adr-051.md) D1). A word held open for later is a name today,
    refused by `NK1117` where a program writes it, and it joins the list on the
    day the construct does.

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

> **Status:** the runtime underneath exists and standard input is not on it yet.
> [ADR-038](adr/adr-038.md) D3's mechanism serves **files** — `fs::read`,
> `fs::read_to_string` and `fs::write` are completed by the kernel where the
> machine can, and by the runtime's I/O thread where it cannot. `io::lines` and
> the two whole-stream reads here are still an ordinary blocking read on the
> calling thread: a pipe is neither a file nor a socket, and which half of D3
> it belongs to is not decided. What the surface says is unaffected, which is
> the point of the surface.

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

**`http` — a package, not a module of `std`**
An HTTP/1.1 and HTTP/2 server and client, aimed at production and not there yet.

> **Status:** not built, and **not part of `std`**
> ([ADR-069](adr/adr-069.md) D1). It is a package reached by a path —
> `http = { path = "../http" }` — and it lives in `examples/` until its surface
> stops moving, so that it can ripen at its own speed rather than the language's
> (D4). The server is Nikaia's own rather than a binding to a finished one, and
> HTTP/1.1 comes first ([ADR-038](adr/adr-038.md) D1, D6). HTTP/2 is named here
> because it is intended, not because it exists.
* **At `user_parallelism = no`:** Runs on a single-threaded Event Loop.
* **At `yes`:** Runs on a multi-threaded Work-Stealing Executor.

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

**The handler and the request** ([ADR-018](adr/adr-018.md)). A handler is a lambda, so the rule
about its arguments is the one Part I 5.3 already gives: they are the ones it names. The first —
and only — one is the request, and a handler that does not need it names nothing.

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

The last row is a decision: an error's message is written for the operator, and a handler that
returns one to the client is how internal paths and driver messages end up in a bug report. A
status code, a header or a body of one's own is a `Response`, built where it is returned —
`http::Response(status: 400, body: "id is required")`.

**A body need not be bytes the program allocated** ([ADR-058](adr/adr-058.md) D1). `Bytes` is the
shared buffer of Part I 6.6 and `Mapped` derefs to it, so a page mapped **once, outside the
handler** is a body that costs a reference count per request rather than a read — the handler
answering with it duplicates the handle rather than moving it
([ADR-040](adr/adr-040.md) D1's rule, applied to a buffer):

```nika
fn main() throws {
    let page = fs::map("index.html")

    http::Server::new()
        .route("/") fn { page }
        .listen(":8080")
}
```

Reading the file inside the handler instead is a syscall, an allocation and a UTF-8 validation
per request, and on the measurement [ADR-058](adr/adr-058.md) §1 quotes it is **twice** the
machine's CPU for a 4 KiB page.

**And a response may be a file the program never read at all** ([ADR-058](adr/adr-058.md) D2):
`http::File("index.html")` is a body whose bytes go from the page cache to the socket without
entering the process. Its `Content-Type` follows the extension, and its length — and any failure
to open it — are settled before the status line, because after the headers are written there is
no status code left to send (D6). Whether that becomes `sendfile(2)`, a mapping, or an ordinary
read is the library's to choose at run time and not the program's to name (D3), and it is unavailable
under TLS and under HTTP/2 (D5).

**When the request names the file, the name and its root arrive together.** A
download route is the other program, and it is not the one above with a variable in it:

```nika
// The request chose this name; the call says which directory it may not leave.
http::File(request.query("file") ?? "", fs::Root::Dir(store))
```

Every `std` function that takes a path takes its root right after it, with no default
([ADR-108](adr/adr-108.md) D1) — `http::File` like `fs::map`, `fs::read` and `fs::write`. The
root is an `fs::Root`: `Dir(store)`, under which the joined name is resolved and compared
component by component, or `Anywhere`, the one way around the check, recorded per site and
listed by `nikaia --trust` (D2, D4). A name that leaves its `Dir` is `fs::Outside` and the
handler answers 404: the call refuses, it does not rewrite (D3). Nothing is inferred about where
the name came from and no analysis follows it — a `../../etc/shadow` is stopped at the call,
before the headers are written (D6). `trusted: false` on `fs::map` is about the file's
*content* ([ADR-010](adr/adr-010.md) D3) and is a different question.

What the library keeps between requests is bounded, dropped when the file's identity or modification
time moves, and sized by the operator rather than the program ([ADR-058](adr/adr-058.md) D8). A
page that must be held for certain is mapped by the program itself, outside the handler, which is
the first example above.

> **Status:** `http` is not built ([ADR-038](adr/adr-038.md) §4.5), so neither the `Bytes`
> row nor `http::File` exists, and `fs::Root` is not built either ([ADR-108](adr/adr-108.md)
> §5). `fs::map` and `Bytes` exist.

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
// Subject: the path, and the root it may not leave ; Config: options
pub fn read(path: Path, root: Root) -> Bytes throws                  // whole file, as bytes
pub fn read_to_string(path: Path, root: Root) -> String throws       // whole file, UTF-8 validated
pub fn write(path: Path, root: Root, data: &[u8]; append: bool = false, create: bool = true) throws
```

**Every path names its root** ([ADR-108](adr/adr-108.md)). `root` is an `fs::Root`:
`Dir(store)` resolves the name under that directory and throws `fs::Outside` where it would leave
it; `Anywhere` performs no check and is the word a review looks for, listed by `nikaia --trust`
(D4). There is no default and no exception for a literal — a relative name is resolved against
the working directory, which is somebody's to change (D1). `fs::map(&path, fs::Root::Anywhere)`
is what a command-line program whose path the operator typed writes.

**What Stage 0 has of this today.** `read`, `read_to_string`, `write` — with both of its options,
since Part I 5.1's `;` section parses — and `map`, each **without its root** yet
([ADR-108](adr/adr-108.md) §5). `open`/`File` and the directory functions are
not here yet; `lines` and `bytes` are gone for a reason of their own, below.

> **Status of "the runtime's reactor" above.** It exists, and this is what it
> is ([ADR-038](adr/adr-038.md) D3, D4).
>
> * `read`, `read_to_string` and `write` go through it. Where the machine has a
>   completion queue the **kernel performs the read** and reports when it is
>   done; where it does not — an older kernel, a sandbox that forbids the
>   syscalls — the runtime's own I/O thread performs it. Which one is decided
>   **when the program starts**, never when it was compiled, and an operator may
>   pin it (13.3b).
> * It is **running before your first statement**, so an operation costs no
>   thread start and no thread wake-up. That is measured rather than claimed:
>   a pair of reads put in flight together costs **nothing per pair** on the
>   completion path, against the ~46 µs a thread vehicle costs
>   ([ADR-033](adr/adr-033.md) §8.4, ADR-038 §4.3).
> * `map` is **not** on it, and will not be: it hands back pages the operating
>   system owns, and there is no transfer for a completion queue to report.
> * **The state machine this paragraph describes is built**
>   ([ADR-055](adr/adr-055.md) §6): a read is a slot the caller polls rather than
>   a call that blocks its thread, and the executor runs something else while it
>   is in flight. What puts two of them in flight is a program writing
>   `overlap { … }` (Part I 8.1.2) or a `spawn`, and no longer the compiler
>   choosing — [ADR-050](adr/adr-050.md) D1 withdrew that choice.

`read` returns **`Bytes`**, not a `List[u8]`: it is one shared buffer, and slices that outlive its scope are tethered to it (Chapter 6.6 in Part I). This is what lets a parser hand back thousands of names that all point into a single allocation.

**Reading a large file: `map`, and the grammar**

There is no `fs::lines` and no `fs::bytes`, and there will not be. The reasons are stated here because this is where a reader looks for them ([ADR-025](adr/adr-025.md) D3):

* **The specified shape cannot exist.** `lines(path)` was to open the file *and* yield tethered `&str` — so the returned value would own the buffer and hand out views into itself. That is the one thing an iterator may not do, and it is why the language below allocates a string per line when it offers the same function.
* **The shape that works is two calls, and it is the model.** `fs::map(path)` owns the pages; `.lines()` borrows views of them. One value owns a buffer, another borrows from it, and Part I 6.6 and [ADR-008](adr/adr-008.md) rest on keeping those apart.
* **The properties `lines` was for are properties of the mapping**: tethered `&str`, no allocation per line, constant memory. They come from `map`, not from the sequence.

```nika
let data = fs::map(&path, fs::Root::Anywhere)
for line in data.lines() { … }
```

And for a file that is a **record per line**, the language already has something better than a sequence of lines — the grammar protocol, where `@frame(boundary: "\n")` says exactly that and drives itself over the pages, in parallel where `user_parallelism` allows (Part II, 10.7). `examples/1brc.nika`, `examples/access-log.nika` and `examples/config.nika` are all that shape; none of them iterates lines.

The WASM question these functions were the answer to comes back with the target: `map` is a compile error there, and what `std::fs` offers instead on `wasm32-*` will be decided with it.

**Handles**

```nika
pub fn open(path: Path, root: Root; write: bool = false, append: bool = false,
            create: bool = false, truncate: bool = false) -> File throws
```

`File` implements `Cleanup` (Part I, 6.4): the compiler flushes and closes it at the end of the scope, on the normal path *and* while an error is bubbling up, and a flush that fails surfaces as an error instead of being swallowed. Call `close()` explicitly only when you want to handle that error at a precise point.

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

`Mapped` derefs to `Bytes`, so a mapped file is a tethered buffer like any other and a parser cannot tell the difference. This is what makes a multi-gigabyte input practical: the pages are the buffer, and nothing is copied.

**Retention.** A slice that escapes the mapping's scope tethers to it, and a tether keeps the *whole* map alive — one twelve-byte station name can pin thirteen gigabytes. The compiler warns where a small extract outlives a large buffer and suggests `.to_owned()`; the mapping is released once the last tether is gone, which may be later than the end of the block that created it ([ADR-008](adr/adr-008.md), D8). Slices that never leave that scope cost nothing and hold nothing. At process exit a read-only mapping with nothing observable attached to it is simply left to the operating system rather than unmapped page by page ([ADR-009](adr/adr-009.md), D7) — at thirteen gigabytes that teardown is measurable, and skipping it changes nothing a program can see.

**Availability is a property of the target, and of nothing else.** Memory mapping is an operating-system service, and whether it exists has nothing to do with whether the runtime is single-threaded. `fs::map` is therefore available at **every** setting on any target whose platform provides it — a single-threaded program compiled for Linux, macOS or Windows maps files exactly like a parallel one. What rules it out is a target without the service: on `wasm32-*` there is no memory mapping to call, so `fs::map` is a **compile-time error** there.

The error is deliberate rather than a silent fallback to `read`: degrading a memory map into a full read turns a constant-memory program into one that allocates its entire input, which is a failure the program would only discover in production. Code that must build for every target, WASM included, uses `lines` or `bytes` — constant-memory everywhere.

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
        * **Strict Implicit Move:** To ensure thread safety without complex lifetime tracking, Nikaia enforces **Implicit Move Semantics** for a task spawned this way: ownership of the ordinary data used inside the `spawn` block is transferred to the new thread. A handle on a shared value is the one exception and is *duplicated* instead, so the name outside keeps working ([ADR-040](adr/adr-040.md) D1, Part I 6.2) — which is why this says "ordinary data" rather than "variables".
    * **At `user_parallelism = no`, and on `wasm32-*` whatever it says:** Direct usage of `std::thread` is a **compile-time error**. A share-nothing architecture is what makes `user_parallelism = no` mean something, and what keeps a program compatible with WASM hosts.

> **Status:** the thread count this switch decides is built
> ([ADR-038](adr/adr-038.md) D4, §4.1): at `no` the runtime starts its I/O
> workers and nothing else, so there is no vehicle for anything **you** wrote
> to run on; at `yes` a pool for user code starts with it, sized by
> `user-pool` (13.3b). The refusal at `no` is the overlap's vehicle degrading
> rather than a diagnostic ([ADR-033](adr/adr-033.md) §8.2b).
>
> **`spawn` is built at both settings** ([ADR-055](adr/adr-055.md) §6): a
> pausing function is an `async fn`, a file read suspends rather than blocking
> its thread, and the one place a program parks is the executor. At `no` a task
> interleaves in it, so "no vehicle for anything **you** wrote to run on" is
> about a *thread* and not about a task — nothing of yours runs concurrently
> there, and interleaved is not concurrent. At `yes` a task goes to a pool of
> futures over the `user-pool` worker count and does run on a thread of its own.
> What this switch decides and this compiler does not yet **say** is §2 D6's
> `Send`: a task holding something that may not cross a thread is refused by the
> backend rather than here. `task::scope` is not built.

**`std::db` (Universal SQL)**
Nikaia provides a unified SQL interface, starting with SQLite, designed to abstract the underlying platform constraints completely.

* **Zero-Blocking Guarantee:** Database operations are implicitly asynchronous. They never block the Event Loop, nor the Compute Scheduler where there is one.
* **Architecture Adapter:** The implementation switches automatically based on the compilation target:
    * **Native Targets:** Utilizes the runtime's own dedicated I/O thread to offload blocking filesystem operations.
    * **WASM Targets:** Automatically spawns a **Web Worker** and utilizes the **OPFS** (Origin Private File System). This enables native-grade, persistent SQL performance in the browser without freezing the UI thread.

> **Status:** not built. The I/O thread the native adapter would offload to
> does exist ([ADR-038](adr/adr-038.md) D4): it starts before `main`, and what
> runs on it is `std`'s own code — which is why it may exist at
> `user_parallelism = no` at all (ADR-037 D2). Nothing of `std::db` itself
> exists, and no record decides which SQLite binding it would be. An earlier
> draft of this section named `tokio-rusqlite`, which is exactly the kind of
> dependency-by-repetition [ADR-038](adr/adr-038.md) §1 was written about.

```nika
use std::db::sqlite

fn query_data() {
    // Transparently starts the required Sidecar (Thread or Worker)
    let db = sqlite::open("app.db")
    
    // The `sql` grammar validates the syntax while the program is built.
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
Errors indicating an inconsistent program state (Index Out of Bounds — on a **list**, whose index is the program's own arithmetic; a map read through the brackets is a `T?` and never here, [ADR-114](adr/adr-114.md) — Division by Zero, **Arithmetic Overflow**, **a conversion whose value does not fit**, explicit `panic()`). An overflow is in this list at **every** build, which is what keeps Part I 1.2's rule — you choose *how*, never *what* — true of arithmetic as well; where a program means to wrap or to stop at the limit it says so by name (Part I, 2.2) ([ADR-043](adr/adr-043.md) D1). A conversion is the same kind of thing and is in the list for the same reason, by a different mechanism: the check is in the emitted code rather than in a build setting, because `as` in the language below truncates by definition and offers nothing to switch on — and `truncating_i32` is how a program says it wanted the low digits ([ADR-043](adr/adr-043.md) D4, D7). **Two of these are also said at compile time where the answer is already on the page, and that changes nothing in this list:** a constant that cannot fit the type it is given is `NK1116` and a division whose divisor is a *constant* zero is `NK1118` ([ADR-043](adr/adr-043.md) D5 and D5.5). Neither takes a case out of the list - a program whose divisor the compiler cannot evaluate divides by whatever it is handed, and a zero there is unrecoverable exactly as written here. What they take back is the message in the one case where no program had to run to know it (C.1). Three build switches exist (13.3) and a panic depends on **two** of them — `user_parallelism` and `target` — on different grounds:

| `user_parallelism` | Panic Behavior | Consequence |
| :--- | :--- | :--- |
| **`no`** | **Abort** | The process terminates immediately. There is no second piece of your code in flight to isolate the failure from, so unwinding would buy nothing and is not done — which also leaves a smaller binary. |
| **`yes`** | **Task Poisoning** | Only the affected task is terminated. The worker thread catches the panic (Fault Isolation). Resources (`SharedMut[T]`) held by the task are marked "poisoned" so no other thread reads state a half-finished task left behind. |

**One end that is not a panic and is not success either.** A `cleanup-deadline` that expires ends the program with **exit status 70** (`EX_SOFTWARE`) and a message naming every resource whose cleanup was cut off, delivered on the panic path — standard error and the panic hook — because a flush that did not happen is a failure the thing that started the program has to see, and it reads the status, not a log ([ADR-112](adr/adr-112.md)). No setting turns that into a `0`; `cleanup-deadline = "0"` does not drain and therefore never expires.

The `target` decides this independently where the machine leaves no choice: on
`wasm32-unknown` a panic is a **trap** and the module is done, whatever
`user_parallelism` says, because the host offers nothing to unwind to.

**The third switch changes nothing on this page.** `reentrancy-check` (13.3) decides whether a compiled program still notices a lock taken while a lock is held. Taking one is refused when you build ([ADR-039](adr/adr-039.md) D2), so in a program the compiler accepted the check cannot fire, and if it ever does the refusal has a hole rather than the program ([ADR-039](adr/adr-039.md) D8). The re-entrancy panic Part II 12.2 describes at `user_parallelism = no` is that check and nothing else, which is why the table above does not carry a row for it. **Poisoning is unchanged by any of this**: at `no` a panic is an abort, so nobody survives for whom poisoning would be done, and the `yes` row stays the only place it happens ([ADR-039](adr/adr-039.md) D1).

> **Status:** `SharedMut[T]` is built ([ADR-064](adr/adr-064.md)); the third
> switch and the check it controls are specified and not (13.3,
> [ADR-039](adr/adr-039.md) §4). The re-entrancy panic is real at
> `user_parallelism = no` — the cheap shape carries it ([ADR-057](adr/adr-057.md))
> — but no build switch turns it off yet. Nothing in that changes what this table
> says about `user_parallelism` and `target`.

On **every** panic path — including the abort and the WASM trap — the application's **Panic Hook** runs first (Part I, 7.2): one global, `sync` handler receiving message, location, and stack trace, intended for crash dumps and reports. This rides on the backend's panic machinery, which invokes the hook before aborting even under `panic = abort`. See [ADR-006](adr/adr-006.md), D6.

**And the abort names the `.nika` line.** Every program carries a table of
generated line to Nikaia file and line, and the hook looks the site up before it
prints anything — so an overflow, a conversion that does not fit, an index out of
bounds and a written `panic()` all read
`src/main.nika:2: the program stopped: attempt to multiply with overflow`, rather
than naming a file nobody wrote ([ADR-044](adr/adr-044.md)). A location the table
does not know — a panic inside `std`'s own Rust, or a foreign crate's — is left in
the words of whoever wrote it.

> **Status:** built. The table is every line the emitter wrote from a `.nika`
> line, appended to the program and sorted; the hook is installed by the
> generated `fn main` before the runtime starts. A **user** hook is still
> unbuilt — `NK2604` is catalogued and not raised (C.3), so nothing yet competes
> for the one global handler.

# Appendix B: Compiler Internals & Annotations

To enforce the "Contextual Capture" rules (Chapter 5.4) without hard-coding specific function names into the compiler, Nikaia uses internal attributes. They belong to the Standard Library.

> **Status:** not built. `@detached` is a ledger fact and not a word a program
> writes: whether a function-typed parameter is run or kept is inferred from
> the body ([ADR-102](adr/adr-102.md) D3), and the type itself is not yet in
> the grammar (Part I, 5.4 C). The immediate/detached rule is held by a check
> over `std`'s entries until then ([ADR-029](adr/adr-029.md) D4).

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
| `NK1xxx` | Syntax & types | `NK1101` a call passes the wrong number of arguments. `NK1102` an argument is not what the parameter takes. `NK1103` a `let` says one type and is given another. `NK1104` a `return` - or a body's last expression - is not what was declared. `NK1105` an assignment is not what the target holds. `NK1106` a struct literal gives a field the wrong type. `NK1107` a field that is not there. `NK1108` a condition that is not a `bool`. `NK1109` a call names an option the callee does not have (Part I, 5.1). `NK1110` a call reaches an item — or a field — another **package** keeps private (Part I, 9.2). Raised since a package can be depended on by path: *`secret` is private to `http`*, and *`http::Request.method` is private to `http`* for reading such a field and for giving one a value in a struct literal. A field's `pub` is in the ledger because the language below cannot enforce it: a dependency's items are in the same crate ([ADR-047](adr/adr-047.md) D2). All ten are answered from the ledger (13.5), so a call into a library is checked against the contracts the library ships ([ADR-024](adr/adr-024.md)). `NK1111` (**warning, and temporary**) a plain string holds what looks like a hole, or a doubled brace that used to be an escape - the one-release migration to `f"…"` ([ADR-035](adr/adr-035.md) D5), and the only thing this checker warns about rather than refusing. `NK1112` a call does not pass a parameter the DSL statement it is given declares, and `NK1113` names one that statement does not have (Part II, 10.5) - answered from the statement's own body rather than from the ledger, because the body is where a `:name` is written ([ADR-007](adr/adr-007.md) D5). `NK1114` is **retired**: it warned about a lambda reaching for one of the automatic argument names `a`, `b`, `c`, and those are withdrawn ([ADR-049](adr/adr-049.md)). A body that reaches for one now names something nothing declares, which is `NK1117` below — so the rule the warning made visible is gone rather than made quieter, and the number is not reused. `NK1115` a call wants a shared value and is given a plain one (Part I, 6.2). The way out is never an **implicit** wrap at the call — one in which the word does not appear would move the cleanup point silently, and there would be no saying whether the value was handed on or duplicated — but since [ADR-064](adr/adr-064.md) D2 it is an **explicit** one right there: *"write `Shared(db)` — a hull you can see is one you write"*. The word stands where it happens, which is what the old refusal was protecting and what sending the reader to another line only approximated ([ADR-040](adr/adr-040.md) D1). `NK1116` a constant does not fit the type it is given (Part I, 2.2) - at an annotated `let`, a `return` against a declared result, or an argument whose parameter says what it takes, and at a bare `let` where an operand's **declaration** pins the type, which is what makes `let b = a + 1` answerable ([ADR-043](adr/adr-043.md) §4). A literal standing with nothing beside it keeps its own type-less reading, because a use may still ask for an `i64` (Part I, 2.4). It prevents no abort, because an out-of-range literal never reached run time: what it takes back is the **message**, which was the backend's, in Rust's words, about a file nobody wrote ([ADR-043](adr/adr-043.md) D5, C.1). Since [ADR-063](adr/adr-063.md) it reaches the last two arrangements that escaped it, and the class is closed: a constant **no type holds**, where nothing stands beside it and nothing pinned it — *"this comes to 18000000000000000000, which does not fit in an `i64`"* — and a constant reached **through a name**, which is arithmetic in the type that name took (D2). Everything between the two is widened rather than refused, so the sum that used to be `rustc`'s *"this arithmetic operation will overflow"* on a Nikaia line is now a program that runs. `NK1117` a statement is one name and nothing declares it. This language has no word it does not know: the grammar is scannerless, so a word it has no rule for is read as a name, and a name on its own is a legal statement. `assert c`, `unsafe { … }` and the `_000` of `1_000` (Part I, 2.2) all used to lower and be refused by `rustc` about a file nobody wrote, which is the C.1 class. The help is where a word that is **not** reserved is explained: `loop { … }` is answered with *write `while true`*, `const X = …` with *write `comptime`*, `macro` and `quote` with *Nikaia has no macros* ([ADR-117](adr/adr-117.md) D2). Four things count as declaring a name - a local or parameter in scope, a function either ledger describes, a type declared here, and a module of this program - and anything this compiler cannot see is a name it does not refuse (C.4). `NK1118` a division - or a remainder - whose divisor is a constant zero ([ADR-043](adr/adr-043.md) D5.5). A division by zero stays where Part III A.2 puts it, unrecoverable and at run time; what this takes back is the one case already decided on the page, which `rustc` refused with *"attempt to divide `1_i32` by zero"* about the generated file. Both this and `NK1116` read the same constant fold - a literal, an immutable `let` whose value folded, `+ - * / %` and a negation, in an `i128` - and a divisor the fold cannot evaluate is a divisor nothing is claimed about. `NK1119` a `let`, a `for` binding or a lambda's argument is called `self` ([ADR-051](adr/adr-051.md) D4). Every other reserved word is excluded from the grammar's name rule, so `let fn = 3` does not parse at all; `self` cannot be, because it is the one reserved word that *is* a name - `self.min` refers to it - and one rule serves both declaring a name and referring to one. Reached at a `let`, a `for` binding, a lambda's argument and a **struct field** - the last taking a span of its own on `FieldDef`, since the nearest one that walk had was a statement's and a caret on the wrong line is worse than no message. A **parameter** named `self` is refused by the grammar instead: it never parsed, because the receiver takes the word. `NK1120` is **unused** and the number is not reused. `NK1121` a `?.` reaches through a value that cannot be absent ([ADR-052](adr/adr-052.md) D7): `?.` exists for a `T?` and answers `null` where there is nothing to reach on, so a type that always has a value has nothing for it to do - the way out is the plain `.`. Asked only where the receiver's type is known, because refusing an unknown one would refuse a correct program (C.4). `NK1123` a hull written a second way, or a hull of a hull ([ADR-064](adr/adr-064.md) D3): `Shared[Locked[T]]` is what `SharedMut[T]` is called and the message carries the replacement, because two spellings of one type were the same bytes below and two different types above - and `Shared(x)` where `x` is already a handle adds a second count around one value, which a handle handed on gets for free. `NK1125` a member reached off a `T?` with a plain `.` ([ADR-066](adr/adr-066.md) D6), which is `NK1121`'s mirror: that one refuses a `?.` where there is nothing to reach *through*, this one refuses a `.` where there is. `a?.b.c` guards `a` and nothing else — the short-circuit decides whether what follows is *evaluated* when the guard fails, never whether what follows is itself safe — so the unguarded `.c` is left reaching into a `T?`. Where `null` inhabits every reference type that is a crash at run time; here it cannot be, because a `T?` is a **type of its own** (Part I, 2.3) and a member of `T` is not a member of it, which is answerable where it is written. Both directions had escaped: `find(1)?.b.c` and a bare `maybe().c` alike were emitted as a field read off an `Option`. The way out is the guarded form, or `??` and then the plain `.`. `NK1124` a door over several locks written wrong ([ADR-065](adr/adr-065.md)): handed something that is not a lock, handed one where several are wanted, or given a block that does not name one value per lock. A **number** is refused there although its type says nothing, because a literal carries none on purpose (Part I, 2.4) and reading that absence as *"might be a lock"* is what used to send the program to `rustc` about a trait bound. `NK1126` a field or a method reached on a **type parameter**, which has no bound ([ADR-074](adr/adr-074.md) D5). A `T` with no bound can be moved and passed and nothing else, because every type a caller may pick has to answer for what the body does — so `x.to_uppercase()` on one is not a member the type has, in the same way `.to_uppercase()` is not a member of an `i64`. It is what makes writing the `<T>` worth anything: without it a generic function whose body uses its parameter still fails below, and the message is still `rustc`'s, about a file nobody wrote, saying `T` in the current scope. It says *no bound* rather than *no such method* because the method may exist on every type the caller will ever pass, and what is missing is the sentence that says so. `NK1127` a `comptime` binding this compiler cannot evaluate while it builds ([ADR-073](adr/adr-073.md) D3, D5; [ADR-077](adr/adr-077.md) for the word). The compiler folded constants long before the keyword existed, so `comptime` adds no arithmetic — it adds a **demand**, and this is the demand failing: *a `let` may fold; a `comptime` must*. What it evaluates today is an integer — a literal, arithmetic over literals and over other constants — and `true` or `false`; a call is D5's second stage and is not in it yet. It is refused rather than computed while the program runs, because a guarantee that silently degrades into *it happened to be cheap* is worse than one that is absent, and the way out is named in the message: `let`, for a value that was never a constant. `NK1128` a name the **language below** reserves and cannot escape ([ADR-076](adr/adr-076.md) D3): `crate`, `super` and `Self`. Every other such name is written **escaped** and stays a name here — a field called `type` is fine, and `type` is the field name of every tagged record anybody has written, which is why reserving the twenty-seven words this language does not reserve was the wrong answer (D1). These three have no escape at all: Rust's own reply to `r#crate` is *"`crate` cannot be a raw identifier"*. So the refusal is the target's exception rather than a claim that they are keywords here, and the message says so. Asked at every position that declares a name, beside `NK1119` and by one call — the failure that was waiting is that `NK1119`'s walk never covered an item's **own** name, because `struct self` cannot parse and `struct crate` parses fine (D4). `NK1129` a trait's method that an implementation **pauses** in where the declaration says `sync` ([ADR-080](adr/adr-080.md) D1, [ADR-109](adr/adr-109.md) D2). A declaration reads like a function type — without the word a method may pause — and a body that pauses under `sync` lowered to an `async fn` against a plain `fn`, which the language below answered with *"method `load` has an incompatible type for trait"*. `NK1140` is the same comparison for `throws`: a body that fails under a declaration without the word. `NK1141` an `update` block that hands a value back ([ADR-110](adr/adr-110.md) D1): the block takes `mut v` and changes it, and *the change is the result*, so a returned value has nowhere to go and would leave a reader guessing which of the two is stored. `NK1130` an `impl` and the `trait` it names disagreeing about **which methods exist** — one the trait does not declare, or one it declares that the `impl` leaves out. Two messages under one code, because a reader is doing two different things: moving a method, and finishing an `impl`. Both were `rustc`'s (`E0407`, `E0046`) until the declaration existed to be checked against ([ADR-078](adr/adr-078.md) §4). A trait this unit does not **declare** is not checked against at all, which is the rule rather than a gap: `impl Error for ConfigError` names the one trait the compiler reads rather than one a `.nika` file wrote, and silence is the only correct answer about a declaration that is not here. `NK1131` a field of a **borrowed** subject handed out by value ([ADR-083](adr/adr-083.md)). `return self.username` out of a `&self` method was `E0507` about a file nobody wrote, which is every accessor over a field that is not a number — and [Part I 6.8](10-nikaia-light.md) had already decided both halves of the answer: *ownership rules occasionally reject code, every such error explains itself in plain language, and a raw internal error reaching you is a Nikaia bug*. Not a `.clone()` the emitter writes, which would be against [ADR-064](adr/adr-064.md) D2's *a hull you can see is one you write* and would buy nothing, since both ways out are one word and the message names them. Asked at a `return`, a `let` and either kind of call argument, and only where the field's type is **known and does not copy**: a number, a `bool`, a `char` and a view take nothing away, and a field this compiler cannot type says nothing at all (C.4).. `NK1132` a `break` or a `continue` with no loop to act on ([ADR-084](adr/adr-084.md) D4, Part I 3.3). Two shapes and one code, because they are one mistake reached from two sides: there is **no** loop, or there is one and a function boundary stands between — a lambda, a task, an `overlap` branch or a DSL fold's step, each of which is a closure or an `async` block in the language below, and a jump does not leave a function. The second is the one the code exists for: it is a program that *looks* right, and without this the answer would be `rustc`'s *"`break` outside of a loop"* about a file nobody wrote (C.1). A `catch` handler is not such a boundary and is deliberately not refused (D5). The refusal is in the **lowering** as well, where it cannot be evaded by a walk that missed a corner (D6) — a program should never see that one. `NK1133` a statement after a `break` or a `continue`, in the same block ([ADR-084](adr/adr-084.md) D3). The shape it is really about is `break x`: Rust's `break` carries a value out of a `loop` and this language's carries none, so a value written after it parses as a statement of its own — the program compiles, the value is dropped, and nothing says so. It states what is true rather than guessing at intent, which is what makes it cover the line somebody left below a `break` while editing as well. `NK1134` a `catch` over an expression that **cannot fail** ([ADR-091](adr/adr-091.md)): a handler with nothing to handle lowered to a `match` over something that is not a `Result`, and the polarity had to be split into two answers rather than one — *nothing here can fail* and *nothing here could be looked up* — because only the first is a mistake. `NK1135` a **type** nothing declares ([ADR-096](adr/adr-096.md)): `let x: Widgit = 3` lowered verbatim and came back as `rustc`'s *"cannot find type `Widgit` in this scope"*, which is `NK1117`'s rule held for one half of this language's names and not the other. The known set is derived rather than listed — Part I 2.2's own types, the ledger's `types` map, and the declaration's own type parameters — and a **qualified** name is left alone, because whether this build can see that package is a question with a message of its own. `NK1136` a `let` that binds several names and is given a type ([ADR-098](adr/adr-098.md)): the names are taken apart by position, and one written type cannot say which of them it is about. `NK1137` a `&` the **compiler** writes ([ADR-094](adr/adr-094.md) D1 and D4), in either of the two positions it writes one. A `for` lends the place it is given, so the reference is already what the line means; written twice it is a reference to a reference, which Rust does not iterate. At a **call** the same holds wherever the callee's `keeps` column says the parameter is only read: the declaration is lowered `&T` and the argument gains its `&`, both off one answer, so a written one says the line twice. Refused rather than absorbed, because two spellings for one thing is the state a reader cannot tell a rule from a habit in — and a `&` in a **declaration** is untouched (D6), which is where it lives. **It lands exactly where the compiler would have written one and nowhere else**: a `&` in a position the callee *keeps*, in front of a copy type, at a **method** call, or in front of a value whose type this compiler could not work out is still the program's own and is left alone (C.4). It is also asked **after** the fit, so a `&i64` handed to a `&Request` stays `NK1102`: sending a reader to fix the punctuation of a line whose type is wrong is the worse of the two messages. `NK1138` a parameter a body **changes** and the declaration does not say `mut` ([ADR-094](adr/adr-094.md) D3). `fn fill(mut out: Vec[i64])` is where in-place change is written and the caller's value is what changes — `&mut self`'s rule held for every parameter — and the call shows nothing, exactly as `xs.push(1)` shows nothing. Without the word the parameter lowered to a Rust one with no `mut` on it and `rustc` answered *cannot borrow as mutable* about a file nobody wrote, which is the C.1 class; the hole is older than the record that named the word. Raised only where the change is **certain**: an assignment into the parameter or into a place rooted at it, or a method every candidate entry marks `mutates`. A name a `let` has bound has stopped being the parameter — which is D3's own way out, *take a copy with `let mut v = x`* — and a method no ledger describes is not one to refuse on (C.4). Said once per parameter, with the caret on the declaration, because that is where the word has to go. `NK1139` the same rule one binding over: a **`let`** whose value is changed, and no `mut` on it ([Part I 2.1](10-nikaia-light.md)). That page writes `// x = 20  <-- This would cause a Compiler Error` and this compiler was not the one giving it — the binding lowered without its `mut` and the answer came back about the generated file, which is C.1 again. Two codes rather than one, because the word goes in a different place and a reader is doing a different thing: a parameter's `mut` also decides what the **caller** sees, where a `let`'s is only about this body. Both read one answer that lives on the binding in scope, so an inner block's `xs` stops being it when the block closes and an outer `mut xs` is it again. `NK1141` an `update` block that hands a value back ([ADR-110](adr/adr-110.md) D1). `update` used to take the value by value and put back what the block returned; D1 hands it the **address**, so there is nothing to return — and without this the old shape lowered to a closure whose value is an `i64` where `()` is wanted, which is C.1 again. Asked of the **shape** and only where the shape says so outright: a last statement that can only be a value, or a `return` carrying one. A last statement that is a call is left alone, because whether *that* comes to a value is a question about its callee and answering it wrongly refuses a correct program (C.4). `NK1138` is the other half at the same door — a block that changes `v` without `mut` — and it is asked **at a door and nowhere else**: `par_fold(…, fn(acc, m) { acc.record(m) })` changes `acc`, has no `mut`, and compiles, because the emitter writes the word itself for a fold's accumulator. `NK1142` a **function type** outside a parameter ([ADR-102](adr/adr-102.md) D1, D5). A parameter may be code — `fn(Request) -> Response`, with `sync` and `throws` after the result — and a parameter the callee *runs* lowers to a closure argument, which is what `std`'s own `map` and `access` take. D1 lets the type stand in a field, a result or a `let` as well, and those are the ones the callee **keeps**: D5 lowers a kept one to a boxed closure over a boxed future, and that is not built. A field written `impl Fn(…)` is not Rust, so this is refused here rather than handed down, where the answer would be about a file nobody wrote (C.1). || `NK21xx` | Running at once, and capture | `NK2101` task takes ownership of a variable still used afterwards (Part I, 8.3) — raised since [ADR-055](adr/adr-055.md) §6 step 4, and narrow on purpose: only where the type is **known** and a move takes it away. A number, a `bool`, a `char` and a **view** are *copied*, so the name keeps working and there is nothing to refuse — which is why `let message = "Hello"` is not this case and `"Hello".to_string()` is; a handle on a `Shared[T]` is *duplicated* ([ADR-040](adr/adr-040.md) D1, D5), which is the exemption Part I 8.3 states; and a type nothing describes is not claimed about at all (C.4). An **assignment** between the task and the later use clears it, because giving the name a value again is a correct program. What it takes back is `rustc`'s *"borrow of moved value"*, whose advice named a **closure** the program does not have (C.1). `NK2102` scoped tasks must be `sync` where they run in parallel (Part II, 12.7). `NK2103` a `spawn`'s lambda names an argument, and a task is handed nothing (Part I, 8.2): `spawn` starts a body, it does not call it. Dropping the name silently would be worse — the body would then refer to something nothing declared, which `NK1117` reports about a name the author *did* write. `NK2104` two branches of an `overlap { … }` cannot run together, and the message names what they meet on ([ADR-050](adr/adr-050.md) D3) — **this is [ADR-033](adr/adr-033.md)'s analysis used the other way round**, and the return on machinery built for an inference D1 withdraws: the touch sets no longer decide whether the compiler *may* overlap two statements, they decide whether the programmer was *right* to say so. Every mainstream form for "run these together" takes the programmer's word for the independence. The same code also refuses a branch that **binds** a name, because the block's value already carries every branch's result (D2), and a branch this compiler cannot account for is **not** refused — reading an absence as "they meet" would refuse a correct program on nothing (C.4). An `overlap` is not a task (D4), which is why the band's name is the wider one. || `NK21xx` | Running at once, and capture | `NK2101` task takes ownership of a variable still used afterwards (Part I, 8.3) — raised since [ADR-055](adr/adr-055.md) §6 step 4, and narrow on purpose: only where the type is **known** and a move takes it away. A number, a `bool`, a `char` and a **view** are *copied*, so the name keeps working and there is nothing to refuse — which is why `let message = "Hello"` is not this case and `"Hello".to_string()` is; a handle on a `Shared[T]` is *duplicated* ([ADR-040](adr/adr-040.md) D1, D5), which is the exemption Part I 8.3 states; and a type nothing describes is not claimed about at all (C.4). An **assignment** between the task and the later use clears it, because giving the name a value again is a correct program. What it takes back is `rustc`'s *"borrow of moved value"*, whose advice named a **closure** the program does not have (C.1). `NK2102` scoped tasks must be `sync` where they run in parallel (Part II, 12.7). `NK2103` a `spawn`'s lambda names an argument, and a task is handed nothing (Part I, 8.2): `spawn` starts a body, it does not call it. Dropping the name silently would be worse — the body would then refer to something nothing declared, which `NK1117` reports about a name the author *did* write. `NK2104` two branches of an `overlap { … }` cannot run together, and the message names what they meet on ([ADR-050](adr/adr-050.md) D3) — **this is [ADR-033](adr/adr-033.md)'s analysis used the other way round**, and the return on machinery built for an inference D1 withdraws: the touch sets no longer decide whether the compiler *may* overlap two statements, they decide whether the programmer was *right* to say so. Every mainstream form for "run these together" takes the programmer's word for the independence. The same code also refuses a branch that **binds** a name, because the block's value already carries every branch's result (D2), and a branch this compiler cannot account for is **not** refused — reading an absence as "they meet" would refuse a correct program on nothing (C.4). An `overlap` is not a task (D4), which is why the band's name is the wider one. |
| `NK22xx` | Locks & suspension | `NK2201` no I/O while holding locked data (Part II, 12.2). `NK2202` a `sync` function called something that can pause (Part II, 12.1), answered from the ledger (13.5). `NK2203` a lock taken while a lock is held — written one inside the other, reached through a chain of calls, or opened as a scope inside the block, because a scope's tasks run during the call and one of them waiting for the held lock is a deadlock rather than a risk ([ADR-039](adr/adr-039.md) D2, D3). **Raised** since the `locks` column exists to answer the chain: `contracts::locks` propagates *touches a lock* over the call graph `sync` uses, with a `spawn`'s body excluded — a task runs later and elsewhere, so a lock in one is the ordinary case — and a trailing lambda's counted. A **`println` is one of these**, which is the case [ADR-067](adr/adr-067.md) D1 was written about: it never pauses, so `sync` says nothing about it, and it takes standard output's own lock while yours is open. Asked on `Holds` and never on the column's third answer: doubt is not permission ([ADR-010](adr/adr-010.md) D1) and not a refusal either (C.4), and what silence costs is the runtime check, which is where every program already is. `get` and `set` hold nothing open (D10), so neither is asked. The way out is asking for both at once: `access_all(a, b) fn(x, y) { … }` (Part II, 12.3). `NK2204` an assignment to a `SharedMut` directly; the message names the door — `kasse.set(42)` for `kasse = 42` ([ADR-039](adr/adr-039.md) D10). `NK2205` a `set` given a value that was **seen** in a lock, or standing under a condition that was ([ADR-039](adr/adr-039.md) D10, [ADR-111](adr/adr-111.md) D4): what a lock hands out is a `Seen[T]`, the stamp travels with the value, and the message names `update` and `set(…; after:)`. The `get` written inside the `set` is the smallest case. **`set(…; after: seen)` is not asked**, because there the witness answers it: the stored value may be stamped and the call may stand under a stamped condition, since if the lock still holds what was seen every decision taken on it still holds (D5). `NK2207` an `update` or `update_all` block that assigns to its `mut v` without reading it — a `set` through the back door, refused as one ([ADR-111](adr/adr-111.md) D4). `NK2208` `set_after` written by hand on a lock: it is how `set(…; after: …)` is spelled in the language below, and writing it directly would drop the failure nothing in the contracts describes (D5, and Part III C.1 is why it matters). Worked through in C.6. |
| `NK23xx` | Aliasing | `NK2301` cannot change a collection while looping over it (Part I, 6.8). `NK2302` a parameter written `&str` is kept past the call it was given in, and nothing names the buffer it views (Part I, 6.6). A view inside a struct carries the buffer it points into ([ADR-008](adr/adr-008.md) D1) and a naked one does not, so the message names the struct form as the way out. Reported where the destination names no buffer: a function or method whose subject holds no view, a view handed back through the result, a view given to a task. Where the destination *does* name one — a field of a subject that holds a view — the program is lowered instead, with the parameter written as a view of that buffer. That last case is accepted **without being decided**: a call on the subject may or may not keep what it is given, nothing written down says which, and an analysis that fails open here emits Rust that does not compile — so it is treated as keeping it ([ADR-010](adr/adr-010.md) D1's polarity, where the cost of the safe direction is a narrower signature rather than a refusal). |
| `NK24xx` | Contract changes | `NK2401` a borrow contract change broke a caller, narrated from the ledger diff (13.5). Reserved: a `catch` that no longer covers every error that can reach it, narrated from the same diff — it needs the ledger to record the *set* rather than a boolean ([ADR-023](adr/adr-023.md) D1), which needs error types the compiler can lower. |
| `NK25xx` | Portability | The `Send` rules that parallel code needs ([ADR-005](adr/adr-005.md) §1 Group B), decided the same way at **both** settings of `user_parallelism` so that a library built at one stays usable at the other — and asked of a value **and a destination**, so the two codes below may answer differently ([ADR-045](adr/adr-045.md) D1): a lock goes into a task of yours and not into code nothing describes. `NK2501` a value that may not cross a thread is used by a task (Part II, 11.2) - an **error** at `user_parallelism = yes` and a **lint** at `no`, where the task does not run and so the crossing does not happen. `NK2502` a value that may not cross a thread is handed to a call this compiler cannot see the end of ([ADR-038](adr/adr-038.md) D7's foreign runtime) - an error at both settings, because a Rust dependency's own threads are not bounded by a switch about *your* code ([ADR-037](adr/adr-037.md) D2). Worked through in C.5. `NK2503` a call into foreign code from which a **lock** is reachable through its arguments, transitively and through the fields of a struct ([ADR-039](adr/adr-039.md) D6) — `NK2502`'s walk generalised rather than a second one of its own, and here the refusal is about the *call* rather than about a type crossing: a call that can reach no lock is allowed without a word, and the way out of one that can is keeping the lock out of its reach (15.2). Worked through in C.6. |
| `NK26xx` | Failure declaration, resource cleanup & crash path | `NK2601` function must declare `throws` because a resource's implicit cleanup can fail (Part I, 6.4). `NK2602` a resource with pausable cleanup must not go out of scope in a `sync` context. `NK2603` (warning) cleanup-deadline exceeded at shutdown; lists the resources that did not finish cleanly. `NK2604` only the application may set the panic hook, and the hook must be `sync` (Part I, 7.2). `NK2605` a **written** call that can fail, in a function that does not declare `throws` (Part I, 7.1) — answered from the ledger (13.5), so it says which contract it read and it grows as the ledger does. The same rule as `NK2601` and `NK2701` ([ADR-025](adr/adr-025.md) D1); what makes it the one that had to exist is that nothing else in the language says a function can fail, so accepting the program publishes `throws`'s *absence* as a fact about a body that contradicts it. |
| `NK27xx` | Implicit calls | `NK2701` a loop whose step can fail, in a function that does not declare `throws` ([ADR-025](adr/adr-025.md) D5). The same rule as `NK2601` one line earlier in the block: where the language performs a call nobody wrote, a failure of it fails the enclosing function. |

The catalogue grows with the implementation; adding an NK code requires adding its reproduction test and its worked example to the relevant spec chapter.

> **Status:** "defined so far" above means defined *here*, not emitted. The codes
> the compiler reports are `NK1101`–`NK1113`, `NK1115`–`NK1139`, `NK1141` and `NK1142` — every one
> of them except the **retired** `NK1114` and the unused `NK1120` — and
> `NK2101`, `NK2103`, `NK2104`, `NK2202`, `NK2203`–`NK2205`, `NK2207`, `NK2208`,
> `NK2302`, `NK2501`, `NK2502`, `NK2605` and
> `NK2701`; `NK2102`, `NK2201`, `NK2206`, `NK2301`, `NK2401`, `NK2601`–`NK2604`
> and `NK2503` are specified ahead of the check that would
> raise them. `NK2101` was the one of those that waited on a *construct* rather
> than on a check, and the construct is built: `spawn` lowers
> ([ADR-055](adr/adr-055.md) §6 step 4), so there is a task to hand data to and
> this is what a program that hands it away and then wants it back is told.
> `NK1115` was the same shape one session earlier and waited on a *type* rather
> than on a construct: `Shared[T]` is a type a program can write (Part I, 6.2),
> so handing a plain value where a shared one is wanted became a program
> somebody could write.
>
> **A code specified ahead of its check has no reproduction test, and must not
> have one.** The paragraph above asks for one per code, and that obligation is
> owed by a code the compiler emits: for the twelve above there is nothing to
> reproduce, and a test that cannot fail would claim a check that is not there.
> `NK1110` was among them for one session and is not: its check never changed —
> the **boundary** moved out from under it when privacy became a package's, and
> then a package arrived to put it back ([ADR-047](adr/adr-047.md) D1, D2).
>
> The four that [ADR-039](adr/adr-039.md) adds are in exactly the position of
> the nine before them — catalogued, with their shapes written down (C.6), and
> neither the types nor the checks they are about exist
> ([ADR-039](adr/adr-039.md) §4).
>
> `NK2302` is the one code here whose question is answered without being
> decided: a read-only call on a field that holds views is treated as keeping
> what it is given, because nothing written down tells it from one that does. It
> still refuses no program the language below accepts — what the undecided case
> costs is a narrower signature, not a refusal.
>
> `NK2605` is reported for a call whose callee a ledger describes — a function
> in this program, one in another module of it, or one of `std`'s, by name or
> as a method on a receiver whose type is known. A call **nothing** describes
> is silence rather than approval, which is C.4's property for every check
> here: it never refuses a program that is right.

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

### C.5. What a Crossing Refused Looks Like

The `NK25xx` pair, on the two places a value the program wrote reaches another thread. Both come from one question - **may a value of this type go to *this* destination?** - asked of the value's type **and of where it is going** ([ADR-045](adr/adr-045.md) D1), and each destination's answer is deliberately not allowed to depend on which build this is ([ADR-005](adr/adr-005.md) §1 Group B).

**The two codes no longer share one verdict**, and the lock is why. Into a task of your own a lock may go, at both settings (D2). Into code nothing written down describes it may not, at both settings — and that is deliberately the worse answer, because at `user_parallelism = yes` a real operating-system lock is underneath and the crossing would be safe; it is refused anyway, so that a library written at one setting stays usable at the other (D3). So `NK2501` refuses nothing a Nikaia program can write, and `NK2502` refuses the lock.

> **Status: built, and the program that reaches `NK2502` is a real one now.** The verdict is asked of the destination and the refusal fires on a program that writes a `SharedMut[T]` or a `Locked[T]` and hands it to an undescribed call. Both are types the backend builds since [ADR-064](adr/adr-064.md), so such a program is refused here rather than refused here *and* unable to emit. The `Held` in the two shapes below is a type nothing describes, which is a different case and still hypothetical.

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

> **No type reaches this shape today**, and `Held` is what says so: a lock goes into a task of your own ([ADR-045](adr/adr-045.md) D2) and everything else a program can write goes with it. The shape stays because the arm does — the day a type answers *may not* into our own code, this is the sentence it gets.

…and a call whose body this compiler cannot see may start a thread of its own (15.1, [ADR-038](adr/adr-038.md) D7):

```text
error[NK2502]: `counter` may not cross a thread, and `hyper_shim::across_a_thread` may put it on one
  --> app.nika:14:5
  14 |     let crossed = hyper_shim::across_a_thread(counter)
           ^
     = nothing written down describes `hyper_shim::across_a_thread`, so this compiler cannot see the end of it - and starting a thread of its own is among the things it may do (Part III, 15.2)
     = `Locked[i32]` holds a lock, and a lock may not go into code nothing written down describes - at either setting of `user_parallelism`, and deliberately so: where the setting makes it safe the answer is kept anyway, so that a library written at one setting stays usable at the other (Part III, C.5)
     = a value may cross a thread only if it may cross any thread, so the answer is the same at both settings of `user_parallelism` and a library built at one stays usable at the other (Part III, C.3)
     help: open the lock where you are and hand over the value inside it - the called code then sees an ordinary value and no lock
```

**The way out is not "don't do that".** The caller opens the lock and passes the **value inside it** in, so the foreign function sees an ordinary number or connection and no lock at all — the shape an ordinary function already has ([ADR-042](adr/adr-042.md) D1, D2, Part I 6.2), so nothing new has to be learned to follow the advice. The sentence also says the refusal was *chosen*: anyone who trips over it in two years should be able to see from the message that a decision is behind it and not an oversight.

Four things about that pair are deliberate.

**The rule is structural and transitive.** A `struct` with one field that may not cross may not cross, and the note names the field that decided it rather than the struct. The fields come from the ledger (13.5), so the rule reaches a type declared in another file for the same reason a type error does.

**Three answers, and the third is the design.** A type may cross, may not, or **nothing written down says**. The third is not permission - reading the absence of an answer as a yes is the polarity [ADR-010](adr/adr-010.md) D1 forbids - and it is not a refusal either, because this compiler knows the type of rather less than half of what a program writes and must never reject a program that is correct (C.4). So an undecided crossing is *handed on*: `rustc` still type-checks the emitted crate, and the trait-bound error it raises is reported against the `.nika` line by the translation C.1 requires. Nothing is silently accepted, and nothing correct is refused. A library may end the uncertainty about one of its own types with a line in its ledger, the way it already ends it about pausing. The same third answer governs whether a call into foreign code can reach a lock (15.2, [ADR-039](adr/adr-039.md) D6): an undecided type is not a type with no lock in it, and "allowed silently" is for a call whose arguments reach nothing — never for one whose contents nobody wrote down.

**One verdict per destination, two severities.** At `user_parallelism = no` nothing you wrote runs concurrently, so the task above does not run and `NK2501`'s crossing does not happen: refusing it would refuse a program that compiles, and saying nothing would let a library built there turn out un-compilable at `yes`. A lint is the third answer, and it carries a note saying which of the two it is. `NK2502` is not downgraded, because a foreign runtime's threads run whatever this switch says. **The destination is not a third severity and not the switch coming back in**: each of the two answers is the same at both settings, which is the whole of what Group B asks — its rule is about the verdict, not about the severity.

**The crossing the compiler chooses for itself gets no diagnostic at all.** Statement overlapping ([ADR-033](adr/adr-033.md)) puts each of a pair inside a closure that runs elsewhere, so what the pair hands back crosses a thread. Where that cannot be shown the statements simply keep the order they were written in, and `--overlaps` says so among the other refusals - because not overlapping is a step the compiler was never obliged to take, and costs speed rather than a program. That closure is **our own code**, so it is the same destination a task is: a result that holds a lock overlaps ([ADR-045](adr/adr-045.md) D2).




### C.6. What a Refused Lock Looks Like

Five refusals come with the rule that a lock may not be taken while a lock is
held, with the doors shared mutable state is reached through, and with a foreign
call that could reach a lock ([ADR-039](adr/adr-039.md) D2, D6, D10). Each names
a way out, as C.2 requires, and each way out is one line of code.

> **Status: four of the five are emitted.** `SharedMut[T]`, `Locked[T]` and the
> doors are built ([ADR-064](adr/adr-064.md), [ADR-110](adr/adr-110.md)), and so
> are `NK2203` — over the `locks` column, so a lock reached through a chain of
> calls is refused and not only one written inside another — `NK2204`, `NK2205`
> and `NK2208`. What is still specified ahead of its check is **`NK2503`**, the
> foreign call, which waits on what 15.2 describes rather than on a type. The
> shapes below are what the codes print (C.3).

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
inside it that takes the lock, because that is the line the reader has to change.
A scope opened inside the block is the same refusal for the reason
[ADR-039](adr/adr-039.md) D3 gives — the scope waits for its tasks, so a task
waiting for the held lock waits for the block that is waiting for it.

Then the two that come with the doors:

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

That second one reads the stamp a value carries out of a lock
([ADR-111](adr/adr-111.md)): the inline `get` is its smallest case, and the same
pair spread over two lines, two functions or two requests is the same refusal,
with the note naming where the value was seen.

And the lowering of a door, written as a door
([ADR-111](adr/adr-111.md) D5). `set_after` is a real function in the language
below and would take the program's two arguments happily, drop the failure
nobody declared, and leave `rustc` to say something about a file nobody wrote
(C.1):

```text
error[NK2208]: `kasse` has no `set_after`; it is how `set(…; after: …)` is written below
  --> main.nika:5:5
   5 |     kasse.set_after(stand + 1, stand)
           ^
     = the compare and the store happen while the lock is open once, and the door that asks for that is `set` with a witness (Part II, 12.2)
     = written this way the failure would be dropped rather than propagated, because nothing in the contracts describes this name
     help: write `kasse.set(neu; after: seen)`, where `seen` is what the lock handed out
```

And the foreign call, judged by what its arguments can reach (15.2):

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

The note names the **field** that decided it rather than the struct, the way
`NK2501`'s does, and for the same reason: the fields come from the ledger (13.5),
so the rule reaches a type declared in another file. A call whose arguments can
reach no lock gets no diagnostic and no note — it is allowed silently, which is
not the same as a call whose contents nobody wrote down (C.5).
