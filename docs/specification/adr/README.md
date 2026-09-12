# Architecture Decision Records

A record here answers one question: **what was decided, and what made that the
answer.** It is written once, it is not edited as the code moves on, and when a
later decision changes it the change is recorded as a new ADR that says which
one it displaces.

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
| [001](adr-001.md) | One exact nightly pinned per release; the parser is generated from a grammar over bytes, not over Rust tokens | Accepted (§4 superseded by [003](adr-003.md)) | yes |
| [002](adr-002.md) | The CLI wraps Cargo so crates.io works; compile-time code runs in an interpreter, not as a proc-macro; a project reaches `std` through a sysroot of pre-lowered sources, compiled into a keyed rlib cache | Accepted (§4 superseded by [003](adr-003.md)) | D1 for a Rust dependency; D4 - sysroot, pre-lowered `std`, rlib cache (97 packages to 46, 58 of them the compiler built twice); not a Nikaia dependency, not D2 |
| [003](adr-003.md) | Hub-and-spoke: a frontend targets **Bridge-IR** and never `rustc_ast`; CLI, cache and Cargo wrapping are generic | Accepted | yes |
| [004](adr-004.md) | One lowering builds `rustc_ast`; readable Rust is a *print* of it, never a second code generator | Accepted | D1, D2; D3's in-memory exit measured at 0.64 % and not built |
| [021](adr-021.md) | The build cache is ours; what is hashed into its key, and what invalidates what | Accepted | key, store, lockfile - D2's resolved versions included (§5); not `--locked` for the lock |

### Ownership, borrowing, cleanup

| ADR | Decides | Status | Built |
| :--- | :--- | :--- | :--- |
| [005](adr-005.md) | The borrow model: four groups of lifetime situation, and which the compiler solves silently. No lifetime annotations, ever | Accepted | inference, ledger, D8's cross-process check (not its second OS), and §1 Group B's structural `Send` check — `NK2501`/`NK2502`, one verdict at both settings of `user_parallelism` (§5). D7 now translates a trait bound's **position**; its text stays Rust's |
| [006](adr-006.md) | `Cleanup` — teardown that performs I/O, inserted by the compiler on every exit path | Accepted | no |
| [008](adr-008.md) | Tethered slices in user structs: the tether-state lattice, and one handle per container rather than per token | Accepted | views |

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
| [012](adr-012.md) | Errors are reported in the `.nika` file the user wrote, never in generated Rust | Accepted | yes |
| [015](adr-015.md) | The backend's diagnostics are built lazily, because building them eagerly dominated the flagship | Accepted | yes |

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
| [030](adr-030.md) | A program is more than one file, and that is name resolution | Accepted | yes |
| [035](adr-035.md) | `f"…"` interpolates and `"…"` is text — the mark belongs on the construct | Accepted | yes |

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

### The runtime

| ADR | Decides | Status | Built |
| :--- | :--- | :--- | :--- |
| [038](adr-038.md) | The runtime and the HTTP server are ours: `rustls` bound, `io_uring` for files, readiness for sockets, and a Rust crate may bring its own runtime under two rules | Accepted | **D3, D4, D5** — the runtime starts before `main`, files complete (with the blocking path as a run-time-detected fallback), sockets signal readiness, and `nikaia-runtime.toml` carries the four settings; D7's **first** rule is enforced by [005](adr-005.md) §5's `Send` check, as far as a foreign signature is honest; D1's HTTP server, D2's `rustls`, D6's parser and D7's second rule are not built |

### Build switches

| ADR | Decides | Status | Built |
| :--- | :--- | :--- | :--- |
| [037](adr-037.md) | Two switches: `target` names the machine, `user_parallelism` says whether the **user's** code may run concurrently — and the compiler's own threads are not the user's | Accepted | D1–D5; `wasm32-unknown` is refused rather than mis-emitted; §3's structural `Send` check is [005](adr-005.md) §5, and the switch reaches its severity and never its verdict |

## What supersedes what

Every supersession in this directory is **partial** — a later record displaces
a section or a numbered decision, never a whole ADR. That is why the superseded
records are still cited by live ones and must not be deleted.

**[003](adr-003.md) over [001](adr-001.md) and [002](adr-002.md).** What ADR-003
replaced is the *coupling*: the frontend no longer links `rustc_driver` and
`rustc_ast` directly, but speaks Bridge-IR to an orchestrator. What it did not
touch, and what later ADRs still cite, is everything in 001 and 002 that was
never about that coupling:

* ADR-001 D1, one exact nightly pinned per release — the premise
  [005](adr-005.md) §1 Group B.2 rests on when it enables `-Zpolonius=next`, and
  the rule `rust-toolchain.toml` and [021](adr-021.md) implement.
* ADR-001 D2, why the parser backend is `winnow-grammar` and not
  `syn-grammar` — cited by [007](adr-007.md).
* ADR-001 D4, Stage 0 is a transpiler — cited by [011](adr-011.md),
  [012](adr-012.md), [014](adr-014.md).
* ADR-002 D3 — a deterministic key over what a build read, the starting point
  [021](adr-021.md) works out.

Each of those sections is marked in place with what still stands.

**The rest are single decisions**, each named in the superseding record's own
header:

| Displaced or amended | By | What moved |
| :--- | :--- | :--- |
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

## Writing a new one

Take the next free number. Head it with the block every record from
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
