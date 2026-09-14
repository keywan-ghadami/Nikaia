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
| [064](adr-064.md) | **`SharedMut[T]` names the capability**, not the mechanism, and survives to the emitter - one name above, two hulls below, chosen per value like the count. **A hull you can observe, you write**: `Shared(x)`, `SharedMut(x)` and `Locked(x)` are calls, so the annotation stops being the constructor and the two positions that could make one stop being a list. And `Shared[Locked[T]]` is **refused** - one type, one spelling | Accepted | **yes** — and Part II 12.2's counter compiles and runs at both settings for the first time, which is the program `user_parallelism` exists for. Four holes closed by one change, because they were one hole: a number literal could not be given a hull at all while a *position* had to know one was wanted |
| [065](adr-065.md) | **A door over several locks is a call the compiler types** — not a ledger entry (a signature binds from a receiver and there is none, the locks hold different types) and not a construct (the grammar has taken the shape since the trailing lambda). **`update_all` hands back one new value per lock**, which is `update`'s rule widened rather than a second rule — so the block stays a pure function of what it was given and the retry route stays open. The order is by **address**, so two tasks naming the same two locks the other way round take them alike | Accepted | **yes** — Part II 12.3's transfer runs at both settings, and two threads transferring in opposite directions neither deadlock nor lose anything. Two locks at a time; `NK1124` for a door handed what is not one. The ledger carries the `sync` column with **no signature**, which is the half it can say |
| [067](adr-067.md) | **`sync` promises one thing — it never pauses** — and Part II's prose, which also promised no I/O, is corrected: `println` is `sync` and so is a panic hook, which **may block**. What a function *reaches* is the second question, and **`touches` gets the inference it was written for** — the fourth derived column, specified with the same fail-closed polarity as the three beside it and never filled. **`lock` joins the vocabulary** the day the doors exist | Accepted | **yes** — `touch::infer` over the call graph, reusing the one call walk the checker and `sync` share; `lock` on all ten doors. One missing pass, three consumers: the lock-inside-a-lock refusal has its fact, `overlap` sees past a call into user code, and *"touches nothing"* is very nearly *"repeating it is unobservable"*. No new word — neither `pure` nor `repeatable` |
| [039](adr-039.md) | The lock: two representations stay and the difference becomes unobservable — taking a lock inside a lock is refused, a second derived property says whether a function touches one, the shared mutable type is `SharedMut[T]`, and it has four doors instead of one | Accepted | **D9 and D10** — `Locked` since [057](adr-057.md) and [059](adr-059.md), `SharedMut` since [064](adr-064.md), both with their four doors and their `std` entries. What is left is D2's refusal of a lock inside a lock, D3's lock-touching property, D7's lambda rule and the `NK22xx` codes, which are catalogued and not emitted |
| [045](adr-045.md) | The crossing verdict takes the **destination**: into a task of our own a lock may cross at both settings, into undescribed foreign code it may not — conservative at `yes` and deliberately so | Accepted | **yes** — `send::crossing` takes a destination and the lock is the row it reaches: `NK2502` refuses a lock handed to an undescribed call, a task and the overlapping closure take one. The type is built too since [064](adr-064.md), so Part II 12.2's counter is not only writable but **runs**, at both settings |
| [040](adr-040.md) | A handle on a shared value is duplicated, never moved, and each handle dies at the end of its own block — so there is no operation to name and no second meaning for `.clone()` | Accepted | **yes, both halves** — a handle handed to a **function** (a borrow duplicating nothing, `--sharing` naming each site) and one a **task** uses, the step written outside the future so the caller's own handle survives the `spawn`. The task half was unrunnable until `spawn` lowered and wrong for one commit after it. `NK2101` stays about data, which is what Part I 8.3 says |

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
| [066](adr-066.md) | **`?.` reaches a member, and a method is one.** Part I 3.5's own word settles what [052](adr-052.md) left as the owner's question: a method call is reached like a field, with its arguments, its config zone and its trailing lambda, and the result flattens where the method's own result is a `T?`. It lowers to a **`match`** and not to the field's `map`, which is the one place the two members differ — a method may pause and may fail, and a closure is where neither can happen. And `??` chains, right-associatively, which was a precedence accident rather than a decision. **D6**: a plain `.` on a `T?` is refused (`NK1125`), because a `?.` guards its own member and no more and a `T?` is a type of its own — where other languages crash at run time this answers where it is written | Accepted | **yes** — `Expr::SafeMethod` is its own variant so no analysis can forget it, one `call_on` checks both spellings and one `method_call` writes them, and `tests/nullable.rs` *runs* four programs because short-circuiting is not a question a type check settles |
| [068](adr-068.md) | **A value whose type is not known is converted into the nullable slot**, rather than left alone. [052](adr-052.md) D4 writes `Some(…)` only where it **knows** the value is not already a `T?` — right, since wrapping one that is makes an `Option<Option<T>>` — and stayed silent otherwise, so the program failed in the language below instead. `value.into()` is correct in **both** directions (Rust has `From<T> for Option<T>` and the identity `From<T> for T`), so the question the checker could not answer stops needing one. `Some(…)` stays where the type is known, because it says what the source means | Accepted | **yes** — one `wrap_for` for all four positions, answering *how* rather than *whether*, and one `around` in the emitter. The test runs a file whose two unknown-typed values are a plain `String` and an `Option<&str>`: the same lowering wraps one and passes the other through |
| [069](adr-069.md) | **`http` leaves `std` and becomes a package**, reached by a path and living in `examples/` until its surface stops moving. The specification had been answering this both ways — the tooling chapter headed a section `std::http`, the first chapter used `http` as its worked example of a *foreign* package — and nobody noticed, because neither exists. Subtract what needs the compiler and nothing HTTP-shaped is left in `std`: the untrusted-path rule is [010](adr-010.md) D8's, escaping is `std::html`, a body the program never allocated is `fs::map`. What remains is the protocol and the framework, which age — and the direction is settled by which mistake stays cheap, since moving a package *into* `std` later is an addition and moving a module *out* is a break | Accepted | **the specification and the README**, which say one thing again; not the package — `std::http` never existed to remove |
| [070](adr-070.md) | **There is no unconditional loop keyword** — `while true { … }` is it, and Part I 3.3 now says the absence is a decision. Go is the precedent read carefully: it has no `while` **at all**, so what it shows is that *one* keyword carries every loop shape, not that the unconditional form is unnecessary. Rust's case for `loop` does not reach here, because `break` and `continue` are not in the grammar, the parser, or the 29 reserved words — so there is no value for a `break` to hand back, and the day there is is the day to reopen it. The divergence half is a checker improvement rather than a keyword: `fn forever() -> i32 { while true { … } }` is refused today and needs an unreachable `return` | Accepted | **the specification**; the compiler needed no change, and D3's unreachability is on `open-work.md` §2.12 |
| [071](adr-071.md) | **`break`, `continue` and `loop` are reserved words**, and none of them is a construct. [070](adr-070.md) D2 found the first two are not in the language at all - not in the grammar, the parser, or the list - and [051](adr-051.md) D1's rule is about timing rather than taste: *reserving a word costs nothing before programs exist and breaks them afterwards*. `loop` is reserved although [070](adr-070.md) D1 decided against the keyword, because that no has a written condition for reopening and [050](adr-050.md) D7 recorded that un-reserving is the free direction - a reservation is reversible and its absence is not | Accepted | **yes** - 29 words to 32, a `RESERVED_C` alternation rather than two more arms on a tuple whose width forced the first split, and Part I 2.1's table, which turned out to be missing `null` as well |
| [072](adr-072.md) | **A file a build reads is named three times** - in the code as a `from "…"` literal, in an allowlist file one path per line, and in the invocation that puts that list in effect (`--allow-read-from-list=…`) - and **a build given no list reads nothing** at compile time. Two of the three are committed, so adding a read is a diff a reviewer sees; [021](adr-021.md)'s lockfile makes a new read visible *after* it happened and this makes it impossible *before*. The list binds the whole build, dependencies included, so a package cannot bring its own permission - the friction is the feature, and it is [026](adr-026.md) Q6's `build.rs` surface answered rather than recorded. No computed paths, no patterns: the property is that a read is decidable by looking at the line | Accepted | **no** - `const` has no syntax to hang the check on, and is not reserved |
| [073](adr-073.md) | **`const` is a declaration whose initialiser *must* be evaluated at build time** - the compiler already folds constants ([063](adr-063.md)), so the keyword adds a **demand** rather than an ability: *a `let` may fold; a `const` must, and says so where it cannot*. It stands where an item stands and inside a body, its type may be written and need not be, and it is a value rather than a place. What may stand in the initialiser is [026](adr-026.md) Q4's and is staged - literals and arithmetic now, a call and `dsl … from "…"` when Q4 answers - with everything in the second group refused by name rather than quietly run at program time. Measured for the record: a table known at build time looks up in **9 ns against a `HashMap`'s 18**, and the 3.9 µs of startup it also saves is *not* the argument | Accepted | **D1 only** - the 33rd reserved word; the declaration is not built |
| [074](adr-074.md) | **A type parameter is a type inside its own body and a variable at every call site**, which is [024](adr-024.md) D4's erasure applied to the one place D4's reason holds. The `[T]` used to be read by the parser, erased by the checker and dropped by the emitter, so `fn hand[T]` asked `rustc` about a type nobody declared - [Part III C.1](../30-nikaia-tooling.md)'s class. Writing the `<T>` alone would have closed a third of it: the rest is binding from **arguments** ([031](adr-031.md) bound from the receiver, on purpose) and `NK1126`, which refuses a member on a parameter that has no bound rather than letting the backend say it. Measured, because it was asked: monomorphisation costs the program nothing at run time and `rustc` 11 % *less* than writing the copies out | Accepted | **yes** - `fn`, `struct` and `impl`, the ledger's `$T`, and nine programs |
| [054](adr-054.md) | **`as` names a type Part I 2.2 offers**, and a `std` parameter the language below counts in `usize` is written as the `i64` a program can hold. The parameter direction ADR-048 §4 left open, and the escape hatch it was mistaken for: a cast named any Rust type at all, so `-3 as usize` was 18,446,744,073,709,551,613 in a language where a conversion that does not fit aborts | Accepted | **yes** — `NK1122` at a cast that leaves the eight types; `nikaia_std::count` with a message of its own, because a negative count is not an access out of bounds; the ledger names `usize` in no signature |
| [053](adr-053.md) | **One Rust crate per Nikaia package**, so a package can depend on a package. The manifest key is a crate's local name; rule 2 becomes structural rather than checked, and ADR-046 D4 does not have to widen. Rule 5 — a Nikaia dependency gets the overflow checks — stops being free and is generated per crate. The shape was built by hand and run before it was decided | Accepted | **yes** — a build emits a workspace, always: `members_of` walks the graph, `cargo_workspace` writes a virtual root and one member per package, and rule 2 is enforced nowhere because a transitive crate does not resolve. A dependency's files are still read, for the ledger and the checks, and emitted by nobody. The diamond unifies in the checker too: a type's identity is the package's directory rather than the key a consumer reached it through |
| [057](adr-057.md) | **`Locked[T]` is decided per value**, not per build setting. The safe shape is the floor; at one user thread every lock is the cheap one, because nothing can cross and the mutex would charge exactly what that switch exists to save; at several it is the analysis that already decides the reference count. The two shapes must behave alike, so the mutex shape carries an owner check at +2.0 ns. Displaces Part II 12.2's per-setting split | Accepted | **yes** — both shapes in `nikaia_std::lock` with one behaviour, chosen per value off the count, the cheap one always at `no`. The surface is [059](adr-059.md)'s and the name and constructor are [064](adr-064.md)'s, which is also what let the analysis watch the allocation: **the cheap shape triggers at `yes` now**, for a value nothing crosses a thread with, and a value a task takes comes out atomic in both hulls. §4's **+2.0 ns** owner check was not what shipped — it was a second mutex and a hash per door, at 63.5 ns, and is now 17.3 ([`lock-free.md`](../../lock-free.md) §3) |
| [056](adr-056.md) | **Every name the emitter substitutes is put back**, and where putting it back makes the message say the same thing on both sides, that is an **internal error** carrying the backend's own words. The per-name question — name or translation? — is not asked, so a substitution added later needs no entry: it is the collapse that is noticed, not the name. Restores Part III C.1 for `Shared[T]`, which was deliberately left in Rust's words | Accepted | **yes** — `Rc`/`Arc` become `Shared[T]`, nested hulls whole; the collapse is detected by comparing before and after, so a message rustc itself wrote that way stays a message about the program |
| [055](adr-055.md) | The emitted Rust is `async` where a function can pause, and a plain `fn` where [027](adr-027.md)'s `sync` says it cannot — implicit async, which the language was specified with from [005](adr-005.md) on and the lowering never had. A call to a pausing function carries an `.await` the same way a `throws` one carries a `?` ([023](adr-023.md) D8), the executor is ours over the I/O [038](adr-038.md) D3 built, and `user_parallelism` decides only how many threads it has | Accepted | **all five steps of §6, at both settings** — `rt::exec` interleaves two tasks on one thread (Part II 11.2's sentence, unreachable before), `rt::pool` runs them on threads of their own at `yes`, `async`/`.await` come off the ledger, `std`'s entries suspend for real, and `spawn` and [050](adr-050.md) D2's `overlap` lower. Not built: D6's `Send` as a **refusal** of this compiler's rather than `rustc`'s |
| [061](adr-061.md) | **A `Shared` does not go into code nothing describes** — the refusal a lock already had, for the sentence that was already its reason: a Rust library has one signature, and since ADR-037 D7 the count's is chosen per value. **So at one user thread every count is plain**: nothing can cross there and D1 closed the last way out. Narrows D6, whose floor was bought to keep a *verdict* switch-independent — which it still is | Accepted | **yes** — measured: at `no` a public signature's parameter is plain where it was atomic; at `yes` the per-value answer is unchanged. A bridge type for the foreign case is reserved and not built |
| [063](adr-063.md) | **A constant written only in literals takes the first type that holds it**, so `2000000000 + 2000000000` compiles where `3000000000 + 1` already did — the unit is the expression and not the token. **A name pins the type its own value took**, so `a + a` is arithmetic in that type and is refused in *this* language's words rather than widened, which would mean walking back from a use to a declaration. One fold for both passes, with the **lookup** as the parameter that separates them | Accepted | **yes** — `crate::fold`, the emitter folding at the outermost expression and carrying the answer inward, and a sequence index and a repeat count left alone because the position gives them their type. Every arrangement of a constant now answers from this compiler; none reaches `rustc`'s overflow lint |
| [060](adr-060.md) | **A literal no use constrains takes the first type that holds it** — `i32`, else `i64` — so `let big = 3000000000` compiles, because it looks like a mistake and is not one. Kotlin's rule as a **fallback** rather than as the rule, so backward inference survives and nothing that compiles today stops. And it needs no analysis: a value an `i32` cannot hold has no second answer a use could ask for | Accepted | **yes** — one rule in the emitter (`integer_literal`), with the negation folded beside it so the question is about the value and not the digits |
| [059](adr-059.md) | **`access` reads and `update` writes**, so no lambda in this language is handed something it may change — and the question of how to spell a mutable parameter stops being one. Opened because `access`'s block mutated through its parameter; closed because its justification was a copy that does not happen: `let mut v = old` inside `update` is a **move**, whatever the value's size. Costs one visible line, and leaves a write across several locks with no door | Accepted | **yes** — all four doors on both shapes, the value in an `Option` so `update` moves it out and back without a `Default` bound or unsafe code, and all four in `std.contracts`. The write across several locks it left open is [065](adr-065.md)'s `update_all` |
| [058](adr-058.md) | A response body may be bytes the program never allocated — a `Bytes` or a mapping — and, where the request names the file, one it never read; which mechanism carries them is `std`'s choice, because what wins depends on whether anything may be kept: `mmap` per request measured **2.5× worse than plainly reading**. A path out of a request is `Untrusted` and may not reach the filesystem unchecked ([010](adr-010.md) D8's third consumer) | Accepted | **no** — `std::http` does not exist; what is built is the bench that decided it, and D7 is the one piece buildable before the server |
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
| [062](adr-062.md) | **A target that lets foreign code call in is a target**, not a third value of `user_parallelism`: that switch bounds *your* code and cannot answer for a caller's threads. So an exported entry point and everything it reaches take the safe shape, and the rest of the library keeps [037](adr-037.md) D7's per-value answer — one build, safe at its boundary, rather than two artifacts or the atomic count on everything. `extern "C"` brings back **not** the representation-in-a-signature problem ([061](adr-061.md) D1 — a C signature can name neither shape) but the ownership of threads; Python's GIL was never the guarantee it is remembered as | Accepted | **no** — and nothing can be: `extern "C"` is a parse error and this repository has no notion of a linkable artifact. The checks need no change at all, because [045](adr-045.md) D1 kept every verdict off the switch |

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
| 074 | what a build-time body may do: `sync`, and touches at most the parameters | `claude/mutex-boden-entscheidung-x291jd` |

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
