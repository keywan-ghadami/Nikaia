# Architecture Decision Records

A record here answers one question: **what was decided, and what made that the
answer.** It is written once, it is not edited as the code moves on, and when a
later decision changes it the change is recorded as a new ADR that says which
one it displaces.

A decision the project **withdraws** is the exception, and it is removed rather
than superseded: the records are rewritten as though it had never been there,
with one sentence for what is done instead. A reader is owed the reason for the
current answer, not the route to it — the route is `CHANGELOG.md` and the notes.
The test of which case you are in: a supersession leaves a live citation behind,
a withdrawal leaves none ([`docs/README.md`](../../README.md) §2).

## What goes where

| | holds | does not hold |
| :--- | :--- | :--- |
| the [specification](..) | what the language *is*: the rule, the syntax, the guarantee | why that rule won, what the alternatives were, what anything cost |
| **an ADR** (here) | one decision, the reasoning that settled it, and the evidence it rests on | the story of the session that produced it |
| the [notes](../..) and `CHANGELOG.md` | what was tried and measured, how, and what went wrong on the way | anything normative |

A measurement belongs in an ADR as *the number that decided it*, with the method
in the notes. A spec section states the rule and links here; a reader who only
wants to write Nikaia never has to open one. The full rule is
[`docs/README.md`](../../README.md).

## The index

**Status** is one of *Accepted*, *Open* (decided nothing yet), or *Superseded*.
**Built** says what of it exists in the compiler today — an ADR is a decision,
and a decision is not an implementation.

### Compiler and toolchain architecture

| ADR | Decides | Status | Built |
| :--- | :--- | :--- | :--- |
| [001](adr-001.md) | Stable is the toolchain, named in one file; the parser is generated from a grammar over bytes, not over Rust tokens; the build is staged and Stage 0 is a transpiler | Accepted | yes — one toolchain, one CI leg |
| [002](adr-002.md) | The CLI wraps Cargo so crates.io works; compile-time code runs in an interpreter, not as a proc-macro; a project reaches `std` through a sysroot of pre-lowered sources, compiled into a keyed rlib cache | Accepted | D1 for a Rust dependency; D4 - sysroot, pre-lowered `std`, rlib cache (97 packages to 46, 58 of them the compiler built twice); not a Nikaia dependency, not D2 |
| [003](adr-003.md) | The language and the machinery that compiles it are two programs, and the interface between them is Rust source text; CLI, cache and Cargo wrapping are generic | Accepted | yes |
| [004](adr-004.md) | One lowering, and it emits Rust source text; the text a person reads is the text that is compiled | Accepted | yes — `crates/nikaia/src/emit`, what a bare `nikaia` runs |
| [021](adr-021.md) | The build cache is ours; what is hashed into its key, and what invalidates what; a backend that is not here refuses by name (D9) | Accepted | key, store, lockfile - D2's resolved versions included (§5); D9's refusal; not `--locked` for the lock |

### Ownership, borrowing, cleanup

| ADR | Decides | Status | Built |
| :--- | :--- | :--- | :--- |
| [005](adr-005.md) | The borrow model: four groups of lifetime situation, and which the compiler solves silently. No lifetime annotations, ever. Group B.2 is the frontend's own desugaring (D2) | Accepted | inference, ledger, D8's cross-process check (not its second OS), and §1 Group B's structural `Send` check — `NK2501`/`NK2502`, one verdict at both settings of `user_parallelism` (§5). D7 now translates a trait bound's **position**; its text stays Rust's. D2's desugaring: not built |
| [006](adr-006.md) | `Cleanup` — teardown that performs I/O, inserted by the compiler on every exit path | Accepted | no |
| [008](adr-008.md) | Tethered slices in user structs: the tether-state lattice, and one handle per container rather than per token | Accepted | views, and **D9** — a result that borrows from nothing says `'static`, which is the only lifetime that can be written there; the body is still checked against it. `@borrowed` and the retention lint are not built |
| [039](adr-039.md) | The lock: two representations stay and the difference becomes unobservable — taking a lock inside a lock is refused, a second derived property says whether a function touches one, the shared mutable type is `SharedMut[T]`, and it has four doors instead of one | Accepted | **no** — neither `SharedMut` nor `Locked` is a type the compiler knows; what exists is the machinery D3 and D6 build on ([027](adr-027.md)'s fixpoint, [005](adr-005.md) §5's structural walk) |
| [045](adr-045.md) | The crossing verdict takes the **destination**: into a task of our own a lock may cross at both settings, into undescribed foreign code it may not — conservative at `yes` and deliberately so | Accepted | **yes** — `send::crossing` takes a destination and the lock is the row it reaches: `NK2502` refuses a lock handed to an undescribed call, a task and the overlapping closure take one, and Part II 12.2's counter is writable. The **type** is still unbuilt ([039](adr-039.md) §4), so such a program is checked and then fails to emit |
| [040](adr-040.md) | A handle on a shared value is duplicated, never moved, and each handle dies at the end of its own block — so there is no operation to name and no second meaning for `.clone()` | Accepted | **partly** — built for a handle handed to a function, including that a borrow duplicates nothing, with `--sharing` naming each site; the task half waits on `spawn` lowering, so there is still no `NK2101` to exempt |

### Grammar, DSLs, parsing

| ADR | Decides | Status | Built |
| :--- | :--- | :--- | :--- |
| [007](adr-007.md) | The scannerless grammar protocol and hybrid DSL binding; retires `unsafe asm` as a core construct | Accepted | D1–D4, D6, D8; D5's shadow type, its call-site check (`NK1112`/`NK1113`) and `...args: Self::dsl`, not its driver's `args.values()`. D7 decides *not* to build auto-AST and regex terminals, so it has nothing to implement |
| [009](adr-009.md) | Parallel parsing: frames, monoid folds, and where a format assumption is written down | Accepted | frames, `par_fold` |
| [011](adr-011.md) | Stage 0 lowering — the grammar protocol onto the parser backend, name for name | Accepted | yes |
| [016](adr-016.md) | The UTF-8 check is divided across frames, not skipped | Accepted | yes |

### Diagnostics

| ADR | Decides | Status | Built |
| :--- | :--- | :--- | :--- |
| [012](adr-012.md) | Errors are reported in the `.nika` file the user wrote, never in generated Rust. D8: a project build runs the binary itself, so Cargo never gets a second chance to replay a cached diagnostic against the generated file | Accepted | yes |
| [015](adr-015.md) | The backend's diagnostics are built lazily, because building them eagerly dominated the flagship | Accepted | yes |
| [044](adr-044.md) | One location table beside the program, and the panic hook looks the site up — so every abort names the Nikaia line, not the generated one | Accepted | **no** — the emitter keeps no line table and the panic hook is itself unbuilt |

### Trust and provenance

| ADR | Decides | Status | Built |
| :--- | :--- | :--- | :--- |
| [010](adr-010.md) | Input provenance: untrusted input is a property the compiler tracks, and an analysis that fails open is a vulnerability generator | Accepted | hasher, `--trust` |
| [017](adr-017.md) | Escaping is the template's contract, not the caller's discipline | Accepted | hole contract |

### `std` and the language surface

| ADR | Decides | Status | Built |
| :--- | :--- | :--- | :--- |
| [013](adr-013.md) | What Stage 0 refuses to infer; `std` is a crate, methods resolve through the receiver | Accepted | yes |
| [014](adr-014.md) | `std` is written in Nikaia where it can be; `fs::map` maps, `par_fold` runs on rayon | Accepted (D1's *when* superseded by [002](adr-002.md) D4) | yes |
| [018](adr-018.md) | The HTTP handler sees the request, because a lambda already could | Accepted | no |
| [019](adr-019.md) | Standard input is a stream, read like everything else | Accepted | yes (`std::io`) |
| [022](adr-022.md) | One lambda form. `fn:` is removed | Accepted | yes |
| [041](adr-041.md) | Naming a lambda's arguments is the normal form; the automatic `a`, `b`, `c` are experimental and warned about where one is actually used | Accepted (D2's experimental carry ended by [049](adr-049.md) D1) | **yes, then removed** — the warning and the mechanism it shared with the emitter are gone with the form |
| [049](adr-049.md) | The automatic `a`, `b`, `c` are **withdrawn** — refused rather than warned about, and not announced a release ahead; what goes with them is the rule that read a lambda's arity off which of the three its body mentioned | Accepted | **yes** — `Expr::Closure` has no implicit flag, `implicit_params` and `NK1114` are removed, and a body reaching for `a` meets `NK1117`; eight sites in four programs rewritten |
| [050](adr-050.md) | Statement order is the written order: ADR-033's reordering, `seq` and the `ordering` switch are withdrawn, and `overlap { … }` is how a program asks for concurrency — with the touch sets kept to **check** the claim rather than to make one. D6 reaches the same schedule at both settings by starting every suspending branch before running one that cannot suspend | Accepted | not built: it needs the runtime binding `spawn` waits on |
| [051](adr-051.md) | This language's keywords are **reserved words**: a name may not be one of them. The grammar sublanguage's vocabulary (`rule`, `boundary`, `fold`, `par_fold`, `unchecked`) is not on the list, and after a `::` or a `.` a reserved word is a name, because no construct begins there. `self` is on the list and is also the receiver's name, so declaring one is `NK1119` from the checker rather than a parse error - except as a parameter, which never parsed at all and says so in a sentence now | Accepted | **yes** — and it closed three silent miscompilations (`if { }`, `let 5 = x`, `if a = b { }`, each read as statements about a variable named after the keyword) plus `let fn = 3`, which `rustc` refused about the generated file. Cost in this repository: two identifiers renamed |
| [052](adr-052.md) | `T?` is a type and `null` is a word, both lowering to the language below's `Option<T>` and `None`. A plain `T` stands where a `T?` is wanted — the one widening this language has — and the `Some(…)` is the **compiler's** to write, decided in the checker because whether the value is already nullable is a question about types and the emitter has none. `?.` reads the same way: `map` over a plain field, `and_then` over one that is itself nullable, or `a?.b?.c` holds a nullable of a nullable | Accepted | **all of it** — the type, the word, the wrap at an annotated `let`, an assignment, a `return`, a struct-literal field and a call argument, and `?.` choosing `map` or `and_then` from the field's declared type. `?.m()` on a *method* is refused with a sentence rather than built, because Part I 3.5 does not write it (§4) |
| [054](adr-054.md) | **`as` names a type Part I 2.2 offers**, and a `std` parameter the language below counts in `usize` is written as the `i64` a program can hold. The parameter direction ADR-048 §4 left open, and the escape hatch it was mistaken for: a cast named any Rust type at all, so `-3 as usize` was 18,446,744,073,709,551,613 in a language where a conversion that does not fit aborts | Accepted | **yes** — `NK1122` at a cast that leaves the eight types; `nikaia_std::count` with a message of its own, because a negative count is not an access out of bounds; the ledger names `usize` in no signature |
| [053](adr-053.md) | **One Rust crate per Nikaia package**, so a package can depend on a package. The manifest key is a crate's local name; rule 2 becomes structural rather than checked, and ADR-046 D4 does not have to widen. Rule 5 — a Nikaia dependency gets the overflow checks — stops being free and is generated per crate. The shape was built by hand and run before it was decided | Accepted | **yes** — a build emits a workspace, always: `members_of` walks the graph, `cargo_workspace` writes a virtual root and one member per package, and rule 2 is enforced nowhere because a transitive crate does not resolve. A dependency's files are still read, for the ledger and the checks, and emitted by nobody. Not built: the diamond in the checker, where a package reached under two keys is two types |
| [057](adr-057.md) | **`Locked[T]` is decided per value**, not per build setting. The safe shape is the floor; at one user thread every lock is the cheap one, because nothing can cross and the mutex would charge exactly what that switch exists to save; at several it is the analysis that already decides the reference count. The two shapes must behave alike, so the mutex shape carries an owner check at +2.0 ns. Displaces Part II 12.2's per-setting split | Accepted | **the shape** — both shapes in `nikaia_std::lock` with one behaviour, chosen per value off the count, the cheap one always at `no`, and the annotation allocating both hulls. Not the surface: what `access` hands its lambda is undecided, and the analysis does not yet watch a `Shared[Locked[T]]` allocation, so the cheap shape never triggers at `yes` |
| [056](adr-056.md) | **Every name the emitter substitutes is put back**, and where putting it back makes the message say the same thing on both sides, that is an **internal error** carrying the backend's own words. The per-name question — name or translation? — is not asked, so a substitution added later needs no entry: it is the collapse that is noticed, not the name. Restores Part III C.1 for `Shared[T]`, which was deliberately left in Rust's words | Accepted | **yes** — `Rc`/`Arc` become `Shared[T]`, nested hulls whole; the collapse is detected by comparing before and after, so a message rustc itself wrote that way stays a message about the program |
| [055](adr-055.md) | The emitted Rust is `async` where a function can pause, and a plain `fn` where [027](adr-027.md)'s `sync` says it cannot — implicit async, which the language was specified with from [005](adr-005.md) on and the lowering never had. A call to a pausing function carries an `.await` the same way a `throws` one carries a `?` ([023](adr-023.md) D8), the executor is ours over the I/O [038](adr-038.md) D3 built, and `user_parallelism` decides only how many threads it has | Accepted | **step 1's `no` half** — `rt::exec` interleaves two tasks on one thread, which is Part II 11.2's sentence and was unreachable before. §6 gives the rest of the order: `async`/`.await` in the emitter, then `std`'s pausing entries, then `spawn`, then [050](adr-050.md) D2's `overlap` |
| [042](adr-042.md) | A view keeps the type it is a view of, arguments and all, and a container whose ledger records a `deref` is seen through once — as a rescue after a comparison has already failed | Accepted | **yes** — `view_of` and `fits_through_deref` in `check`, exercised by `fs::Mapped` and now by `Shared[T]`; a `&Vec[i64]` mismatch is this compiler's `NK1102` where it used to be rustc's |
| [030](adr-030.md) | A program is more than one file, and that is name resolution | Accepted | yes |
| [046](adr-046.md) | `use` makes a unit of code reachable and brings **no name** in — no glob, no braced list, no single name; `use x as y` shortens the prefix, a prefix must be introduced, and one name per file | Accepted | **yes** — the qualified form, all three of D2's refusals in this language's words, `use x as y` resolved away rather than emitted (in one place, on the way to every name), "a prefix must be introduced", and "one name per file" counting the alias |
| [047](adr-047.md) | A **package is a directory**: its files share one namespace and need no `use` between them, privacy is per package, and a library is depended on by **path** — with five rules so the rest is not decided by accident | Accepted | **yes, one level deep** — D1's package is one namespace and one crate root; D2's `path` dependency resolves, becomes a `pub mod` at that root, and all five rules are messages rather than silences. `NK1110` fires for the first time, for items **and** fields, the ledger carrying a field's `pub` because the language below cannot. Not built: everything a version would need. A dependency's own dependencies are [ADR-053](adr-053.md), which replaces the `pub mod` with a crate |
| [035](adr-035.md) | `f"…"` interpolates and `"…"` is text — the mark belongs on the construct | Accepted | yes |
| [048](adr-048.md) | A **length is an `i64`** and so is an index, both conversions emitted rather than written; the machine-width type leaves the writable surface and `u8` is named on it | Accepted | **yes** — four `i64` lengths in `std.contracts`, `NUMERIC` is the four writable types, `xs.len() as i64` and `nikaia_std::index::at` emitted, a negative index reporting as an access out of bounds, and the parentheses around a conversion decided in one place |

### The contract ledger and the type checker

| ADR | Decides | Status | Built |
| :--- | :--- | :--- | :--- |
| [020](adr-020.md) | The ledger records what the compiler knows, and a library brings its own | Accepted | yes |
| [024](adr-024.md) | The type checker says only what is written down; `?` is the absence of a claim | Accepted | yes |
| [027](adr-027.md) | `sync` is earned from the body, not typed by a person | Accepted | yes |
| [028](adr-028.md) | The type checker already knew — the inference needs no second walk | Accepted | yes |
| [029](adr-029.md) | A higher-order function does what its lambda does | Accepted | yes |
| [031](adr-031.md) | A signature may name its receiver's type arguments | Accepted | yes |
| [032](adr-032.md) | A hole is code, and every analysis has to see it | Accepted | yes |

### Errors

| ADR | Decides | Status | Built |
| :--- | :--- | :--- | :--- |
| [023](adr-023.md) | What `throws` declares, where the error set lives, and what an error carries | Accepted | `throw`, `catch`, the inferred set, the site |
| [025](adr-025.md) | A loop's step can fail, and the enclosing function gains a `throws` for it | Accepted | yes |
| [036](adr-036.md) | A stack trace is asked for; the site is free | Accepted | yes |

### Ordering and compile-time execution

| ADR | Decides | Status | Built |
| :--- | :--- | :--- | :--- |
| [026](adr-026.md) | Compile-time I/O — what a build may read, and what may run while it reads | **Open** | no |
| [033](adr-033.md) | Program order is a guarantee only where it is observable: operations with disjoint **touch** sets have no order between them | Accepted, **provisional** (§7–§8) | four increments — a run of any length on `task::both`, `seq` (D7's keyword, still provisional), the resources `file`/`stdin`/`stdout`/`stderr`/`args`, and **D10's completion pair**: two file reads overlap at `user_parallelism = no` too, because the kernel carries them and no thread carries user code; plus `--overlaps`, which now answers per pair, and D8's manifest key; not a method call, not a non-literal argument, not two writes in flight, no socket or lock until something asks |
| [034](adr-034.md) | A handler that can `return` makes the next statement conditional, so it may not be started early | Accepted | yes |
| [043](adr-043.md) | An arithmetic overflow aborts at every build; wrapping and saturating get Rust's names rather than a new operator; a narrowing cast aborts and an out-of-range literal is a compile error; the check is forced through the generated project with an exception for foreign packages | Accepted | **yes** — the overflow aborts at every build, the names exist for `i32` and `i64`, an out-of-range literal is `NK1116`, a narrowing conversion is checked in the emitted code, and `truncating_i32` says what `as` used to do quietly; a **sum** of constants that cannot fit is `NK1116` too, folded where an operand's declaration pins the type, and a division by a constant zero is `NK1118` (D5.5) |

### The runtime

| ADR | Decides | Status | Built |
| :--- | :--- | :--- | :--- |
| [038](adr-038.md) | The runtime and the HTTP server are ours: `rustls` bound, `io_uring` for files, readiness for sockets, and a Rust crate may bring its own runtime under two rules | Accepted | **D3, D4, D5** — the runtime starts before `main`, files complete (with the blocking path as a run-time-detected fallback), sockets signal readiness, and `nikaia-runtime.toml` carries the four settings; D7's **first** rule is enforced by [005](adr-005.md) §5's `Send` check, as far as a foreign signature is honest; D1's HTTP server, D2's `rustls`, D6's parser and D7's second rule are not built |

### Build switches

| ADR | Decides | Status | Built |
| :--- | :--- | :--- | :--- |
| [037](adr-037.md) | Two switches: `target` names the machine, `user_parallelism` says whether the **user's** code may run concurrently — and the compiler's own threads are not the user's | Accepted | D1–D8, the emission included: a `Shared` value is lowered to `Rc` or `Arc` from the count D7 infers for it, so that inference has a real input; `wasm32-unknown` is refused rather than mis-emitted; §3's structural `Send` check is [005](adr-005.md) §5, and the switch reaches its severity and never its verdict |

## What supersedes what

Every supersession in this directory is **partial** — a later record displaces
a section or a numbered decision, never a whole ADR. That is why the superseded
records are still cited by live ones and must not be deleted. Each is named in
the superseding record's own header:

| Displaced or amended | By | What moved |
| :--- | :--- | :--- |
| [041](adr-041.md) D2 | [049](adr-049.md) D1 (completes) | the automatic names were carried as experimental and warned about; they are withdrawn, and the warning and the arity-from-body rule go with them |
| [033](adr-033.md) D1, D7 | [050](adr-050.md) D1, D7 (displaces) | statement overlapping, `seq` and the `ordering` switch are withdrawn; the touch sets stay, to check an `overlap` block's claim rather than to make one |
| [046](adr-046.md) D1 | [047](adr-047.md) D1 (narrows) | which unit `use` names — a package rather than a file. Every rule in 046 stands and ranges over packages instead; the file boundary it was written against stops existing |
| [043](adr-043.md) D7 | [048](adr-048.md) D1 (narrows) | the sources a `truncating_` entry has — the machine-width type leaves the writable surface, so its two entries go and the larger integer and the float remain |
| [030](adr-030.md) | [047](adr-047.md) D1 (narrows) | a file is a module and `use` names one — the files of a directory are one namespace now, and `use` names the directory |
| [039](adr-039.md) §3 | [045](adr-045.md) D1 (answers) | the open half — a lock could not reach a task, so Part II 12.2's counter was unwritable. The verdict takes the destination; §3's other way out, the `Mutex` floor, is not taken |
| [037](adr-037.md) D6 | [045](adr-045.md) D3 (narrows) | its coda that after D6 "no type answers `may not`" — the lock answers it at a foreign call, so `NK2501` and `NK2502` stop sharing one verdict |
| [022](adr-022.md) D1 | [041](adr-041.md) D1 (amends) | which of the two spellings is recommended — one form with two spellings is unchanged, and both still parse everywhere; the named one is what the specification teaches |
| [005](adr-005.md) §3 | [040](adr-040.md) D1 (narrows) | the ban on a clone the user did not write — it reaches a clone of **data** and no longer a **handle** on a shared value, which copies no data and produces no second value |
| [021](adr-021.md) D5 | [039](adr-039.md) D8 (amends) | the cache-key enumeration: the re-entrancy switch is a fourth dimension, because a build with the check and one without lower the same source to different Rust |
| [005](adr-005.md) D6 | [039](adr-039.md) D2 (narrows) | the runtime reentrancy check as the backstop for re-entering one lock through a chain of `sync` calls — that case is refused at compile time now, and the check becomes self-control of the new rule |
| [002](adr-002.md) D3 | [021](adr-021.md) D9 | Cranelift for "sub-second iterations" — never measured; measured, it buys 25 s against 26 s |
| [013](adr-013.md) D5 | [022](adr-022.md) | `fn:` recorded as a chaining limitation; removed as a second way to say one thing |
| [014](adr-014.md) D3 | [015](adr-015.md) | the measurement, once the backend's eager diagnostics were found to dominate it |
| [014](adr-014.md) D1 | [002](adr-002.md) D4 | *when* `std`'s Nikaia half is lowered — a build script made the compiler a build-dependency of `std`, so Cargo built it again inside every project; release time now, and the `.rs` is committed |
| [023](adr-023.md) D7 | [036](adr-036.md) | `{error:full}` as a format spec; `f"{error.full()}"` needs no new syntax |
| [032](adr-032.md) D5 | [035](adr-035.md) | a brace deciding a literal's type — the `f` decides it now |
| [033](adr-033.md) D5 | [034](adr-034.md) D2 (amends) | "started early" widened from *a branch* to *anything the program might not have performed* |
| [033](adr-033.md) §8.2b | [033](adr-033.md) D10 (narrows) | `effects` degrading to `strict` at `user_parallelism = no` — the category distinction stands, the degradation belongs to `task::both`: two reads the kernel performs carry no user code ([038](adr-038.md) D3) |
| [027](adr-027.md) §7 | [029](adr-029.md) | effect polymorphism — a higher-order function's `sync` now depends on its lambda |
| [028](adr-028.md) D6 | [029](adr-029.md) | a higher-order method carrying no `sync` at all |
| [029](adr-029.md) §4 | [031](adr-031.md) | a signature naming its receiver's type arguments |

## Reserved numbers

A number is **taken when it is claimed here**, not when the record lands. Four
collisions in one afternoon are the reason this section exists: two branches
each took what `ls` said was free, and the later one had to renumber its record,
its citations and its index row every time the other merged first.

| ADR | Claimed for | Where |
| :--- | :--- | :--- |
| 051 | A response body may be bytes the program never read, and `std` decides how they travel | [#45](https://github.com/keywan-ghadami/Nikaia/pull/45) |

A row here is a claim and nothing else: it says the number is spoken for, not
what the decision is. Delete the row in the same commit that adds the record.
A claim whose branch is abandoned is deleted by whoever notices.

## Writing a new one

Take the next free number — free means neither a file above nor a row under
**Reserved numbers**, and the first thing to do with it is to claim it there. Head it with the block every record from
[007](adr-007.md) on uses, in this order:

```markdown
# ADR-0NN: A title that states the decision, not the topic

**Status:** Accepted | Open | Superseded by [ADR-0MM](adr-0MM.md)
**Date:** September 11, 2026
**Target Version:** Nikaia 0.0.8
**Supersedes:** — what exactly, if anything
**Related:** [ADR-0XX](adr-0XX.md) (why it matters here)
```

Then: the context, the decisions as numbered `D1…Dn` so they can be cited, the
consequences, and what is **not** built. A decision nobody can cite by number is
a decision that gets re-litigated.
