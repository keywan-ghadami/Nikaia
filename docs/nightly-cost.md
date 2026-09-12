# What the pinned nightly is for, and what it costs

**Date:** September 12, 2026
**Status:** measured; the decisions it produced are [ADR-001](specification/adr/adr-001.md) D1's
and [ADR-005](specification/adr/adr-005.md) D2's corrected evidence paragraphs
**Related:** [ADR-001](specification/adr/adr-001.md) D1 (the pin), [ADR-003](specification/adr/adr-003.md) D1
(`rustc_private` confined to one crate), [ADR-004](specification/adr/adr-004.md) (the bridge backend
that uses it), [ADR-005](specification/adr/adr-005.md) D2 (the Polonius case), [ADR-021](specification/adr/adr-021.md) D9
(the precedent for refusing a backend by name), [`subprocess-cost.md`](subprocess-cost.md)
(the same machine, the same method, the adjacent question)

[ADR-001](specification/adr/adr-001.md) D1 pins one exact nightly and justifies it with two
things: `rustc_private`, and `-Z` flags. This file asks what each is actually for today, what
it buys, and what it costs in the three terms a person installing a compiler feels —
how big, how long, and which tools they must already have.

**The three conclusions, first.**

1. **`rustc_private` is the whole of it.** Two `#![feature(…)]` attributes exist in the
   workspace and both are `rustc_private`; the Rust that Stage 0 emits uses no unstable
   feature at all. Everything except the Bridge-IR backend builds, tests and runs on a
   **stable** toolchain, and does so in CI now.
2. **`-Z` buys nothing, because no `-Z` flag is passed.** `-Zpolonius=next` is named as a
   premise by three records and has never been on a command line. Measured across the
   thirty programs this repository lowers, it changes **zero** verdicts, and it would cost
   **+7.1 %** of the instructions a full build of the largest of them retires.
3. **For a *user*, the nightly costs almost no bytes and all of the flexibility.** A
   minimal stable toolchain is 577 MiB and a minimal pinned nightly is 579 MiB — two
   megabytes apart. What the pin costs a user is not size: it is that a bridge-enabled
   `nikaia` binary links `librustc_driver-<hash>.so` by **absolute path**, so it runs only
   beside that one nightly, installed where the build machine had it.

---

## 1. The machine and the method

Intel Xeon @ 2.80 GHz, 4 vCPU, 15 GB RAM, Linux 6.18.44 x86_64 — a **shared virtual
machine** with no `cpufreq` governor exposed, and the same box
[`subprocess-cost.md`](subprocess-cost.md) used. `rustc 1.94.0-nightly (8d670b93d
2025-12-31)`, which is the `nightly-2026-01-01` of `rust-toolchain.toml`
([ADR-001](specification/adr/adr-001.md) D1); stable is `1.94.1 (e408947bf 2026-03-25)` for
the builds and `1.98.1 (48a229cea 2026-09-01)` for the installs, because an install measures
what a person downloads *today* and a build has to match the rlibs already on disk.

`docs/staging-candidates.md` §2 sets this repository's standard: *wall-clock on a shared
machine is not a measurement; an instruction count is deterministic and diffable*. So §3's
Polonius decision rests on callgrind, and the wall-clock beside it says what those counts
feel like. Where a duration is quoted it is the **median of three runs from nothing** —
`rm -rf target` before each build, a fresh `RUSTUP_HOME` before each install — with the
spread given, because a median of three on this box is worth about ±5 %.

**What "the corpus" means here** is wider than in `subprocess-cost.md`, because a borrow
checker has to be offered everything that has a body. Every `.nika` file the bootstrap
compiler can lower: **ten** of the eleven `examples/*.nika` (all but `fortunes.nika`, whose
`dsl postgres` block it refuses), the three files of `examples/inventory`, the four
`examples/foreign-runtime` programs (`overlaps.nika` and the `crossing`, `serve` and
`smuggled` projects), three `tests/samples/`, `crates/nikaia-std/src/text.nika`, four
`crates/nikaia/tests/fixtures`, and the five of the twenty-six `tests/errors/` files that
get past the parser — **thirty** lowered crates. Of those, 24 reach the borrow checker and
6 stop earlier, at a parse or resolution error, which is what they are in the corpus for.

---

## 2. What nightly is needed for, by name

Two `#![feature(…)]` attributes in the workspace, and both are the same one:

| where | why |
| :--- | :--- |
| `crates/rustc-executor/src/lib.rs` | `#![feature(rustc_private)]` — the crate builds `rustc_ast` and prints it ([ADR-004](specification/adr/adr-004.md) D1, D2) |
| `crates/nikaia/src/main.rs` | the same attribute and `extern crate rustc_driver`, so the binary links the compiler's dylib set rather than a second copy of `std` |

**No `-Z` flag is passed anywhere in a build.** The only one in the tree was
`-Zls=root` in `crates/nikaia/tests/common/mod.rs`, which read an rlib's dependency list to
pick matching copies out of the deps directory — a *test harness* dependency on the
nightly, not a compiler one, and the thing that made a stable-toolchain run of the suite
impossible. The same question is now answered without a flag, by asking which candidate
rlib's filename hash occurs in the anchor's metadata, so the workspace passes no `-Z` at
all.

**The Rust that Stage 0 emits uses no unstable feature.** All thirty lowered crates compile
on the stable toolchain, which `cargo test -p nikaia --no-default-features` on stable is
what proves (§5).

### What the second `#![feature]` really was

`main.rs` carried the attribute unconditionally, and `[[bin]] name = "nikaia"` carried
`required-features = ["rustc-backend"]`, so there was no binary without the bridge and the
whole compiler needed the nightly. Neither was a technical constraint: `rustc_executor` is
named in exactly one place in `main.rs`. Both are now conditional on the feature, the
`required-features` line is gone, and `cargo build -p nikaia --no-default-features`
produces a compiler on stable. `rustc_private` stays confined to `rustc-executor`
([ADR-003](specification/adr/adr-003.md) D1) — the frontend never gained any.

---

## 3. `-Zpolonius=next`: what it would buy, and what it would cost

[ADR-005](specification/adr/adr-005.md) D2 names one situation — conditional return of a
borrow, the `get_or_insert` shape — as rejected by the stable borrow checker and accepted by
`-Zpolonius=next`, and [ADR-001](specification/adr/adr-001.md) D1, ADR-005 §1 Group B.2 and
the [ADR index](specification/adr/README.md) all rest on it. ADR-005 §5 already said the
case is **not built**. What nobody had done is compile something twice and count.

### 3.1 The positive controls, because a harness that finds nothing must first find something

Four hand-written Rust programs, compiled with `--emit metadata` (which runs borrowck) on
the pinned nightly, with the flag and without:

| control | NLL | `-Zpolonius=next` |
| :--- | :--- | :--- |
| conditional return out of a `HashMap` — D2's own `get_or_insert` | `E0502` | **accepted** |
| the same shape on a `Vec` | `E0502` | **accepted** |
| a borrow live on one path and dead on the other, returning the other input | `E0503` | **accepted** |
| `fn last(&mut Node) -> &mut Node`, the linked-list walk | accepted | accepted |

So the flag does what D2 says, on the toolchain D1 pins, and the harness can see it. Three
of the four flip; the fourth is a case NLL already solved, which is worth keeping in the
table because it is the shape people still quote as needing Polonius.

### 3.2 The corpus: thirty programs, zero differences

Every lowered crate from §1, compiled `--crate-type lib --emit metadata` twice:

| | programs |
| :--- | ---: |
| accepted by both | **24** |
| rejected by both, with byte-identical error codes | 6 |
| accepted by `-Zpolonius=next` and rejected by NLL | **0** |
| accepted by NLL and rejected by `-Zpolonius=next` | **0** |

The six rejected by both are rejected before borrowck ever runs — `expected expression,
found ;` three times, an undefined grammar rule, a `until(…)` that runs through its
boundary, and a missing `digit_value` import — which is what four `tests/errors/` files and
two fixtures are in the corpus to do.

**Zero is the finding, and the reason it is zero is in the corpus itself.** `examples/1brc.nika`
and `examples/access-log.nika` both want exactly D2's situation — look a key up, add it if
it is not there, keep using it — and both write it as
`.entry(k).and_modify fn { … }.or_insert_with fn { … }`. That is the **entry-style API**
D2 names as its own fallback ("if the alpha regresses, the fallback is desugaring the known
patterns onto entry-style APIs in the frontend"). The fallback is not a contingency; it is
what the corpus already does, and the language's own `HashMap` surface is what makes it the
natural way to write it.

### 3.3 But the shape is reachable from Nikaia, and that is the other half

A thirteen-line Nikaia program reaches it:

```nikaia
impl Cache {
    fn first_or_push(&mut self, v: i64) -> &i64 {
        let found = self.entries.get(0)
        if found.is_some() {
            return found.unwrap()
        }
        self.entries.push(v)
        return self.entries.get(0).unwrap()
    }
}
```

Stage 0 lowers it, NLL rejects the emitted Rust with `E0502`, and `-Zpolonius=next` accepts
it. So Group B.2 is a real situation in this language and not only in Rust's issue tracker —
it is simply not in anything the repository contains. Two neighbouring shapes are *not*
reachable, and the reasons are worth writing down because they are both defects rather than
design:

* the same thing over a `HashMap[&str, …]` is rejected by **both**, with `E0621`: Stage 0
  elides the key's lifetime in `fn get_or_insert(&mut self, key: &str)` where the field it
  inserts into is `HashMap<&'a str, …>`. The program never reaches the borrow checker's
  flow analysis, because elision gets there first;
* writing the branch as a `match` with `return x` in an arm emits `x` — Stage 0 **drops the
  `return`** from a match arm whose body is a value, turning a control-flow statement into
  the arm's value. Here it surfaces as `E0308` (incompatible arm types); in a function
  returning nothing it would surface as a silently different program. `return` with no
  value in an arm is kept, so the bug is narrow and is not this file's business to fix —
  recorded so it is not found twice.

### 3.4 What it would cost

`valgrind --tool=callgrind`, one run each, deterministic:

| | Ir, NLL | Ir, `-Zpolonius=next` | delta |
| :--- | ---: | ---: | ---: |
| `hello_world.rs` (589 B), `--emit metadata` | 25 838 798 | 25 924 473 | +0.33 % |
| `json.rs` (6 393 B), `--emit metadata` | 818 392 914 | 942 296 839 | **+15.1 %** |
| `json.rs`, **full debug compile** | 1 731 303 418 | 1 854 471 352 | **+7.11 %** |

The full-compile row is the one to quote, and it cross-checks
[`subprocess-cost.md`](subprocess-cost.md) §4, which measured the same file's full debug
compile at 1 719 M Ir — 0.7 % apart, a day and a flag apart.

Wall-clock beside it, median of nine after two warm-ups, milliseconds:

| program | NLL | polonius | delta |
| :--- | ---: | ---: | ---: |
| `hello_world` | 26 | 26 | 0 |
| `tally` | 38 | 38 | 0 |
| `k-nucleotide` | 124 | 130 | +6 |
| `1brc` | 128 | 134 | +6 |
| `json` | 246 | 268 | +22 |

So: **+7.1 % of a build, on every program, to accept a program nobody in this repository has
written.** And it would cost one more thing that is not a percentage: the flag is `-Z`, so
passing it would put *every user's* build on the pinned nightly, where today only the
bridge backend's own compilation is. That is the trade the records should be read against,
and it is why the correction in ADR-005 D2 leaves the decision where D2 put it rather than
reversing it.

---

## 4. What nightly costs, as the three things a person installs

Fresh `RUSTUP_HOME` each time, three runs each, `du -sm` of the whole rustup home.

| what | median install | spread | on disk |
| :--- | ---: | ---: | ---: |
| stable, `--profile minimal` | **11.05 s** | 10.85–11.35 | **577 MiB** |
| the pinned nightly, `--profile minimal` | 11.37 s | 10.80–11.81 | 579 MiB |
| stable, minimal + `rustfmt` + `clippy` | 12.30 s | 11.96–13.51 | 602 MiB |
| the pinned nightly, all four components `rust-toolchain.toml` names | **24.73 s** | 24.72–26.54 | **1 420 MiB** |

The gap between the last two rows is **+12.4 s and +818 MiB**, and it is almost entirely one
component. Measured by summing the on-disk size of the files each component's own manifest
lists:

| component | files | on disk |
| :--- | ---: | ---: |
| `rustc-dev` | 3 085 | **654.3 MB** |
| `rustc` | 41 | 388.6 MB |
| `llvm-tools-preview` | 16 | 194.6 MB |
| `rust-std` | 35 | 174.8 MB |
| `cargo` | 43 | 42.1 MB |
| `clippy-preview` | 5 | 21.2 MB |
| `rustfmt-preview` | 5 | 9.0 MB |

Two notes on that table. **`rustc-dev` is 654 MB and not the 931 MB that had been quoted** —
931 MB is `lib/rustlib/x86_64-unknown-linux-gnu`, which holds `rust-std`'s rlibs too. Of the
654, **41 MB is `rustc-src`**: 2 829 of `rustc-dev`'s 3 085 files are rustc's own sources,
which `rustc-dev` installs and nothing in this workspace reads. And
**`llvm-tools-preview`'s 194.6 MB is almost all `libLLVM.so`, which the `rustc` component
ships anyway** (the two are hardlinks); its own contribution is the fourteen `llvm-*`
binaries, about 8 MB, and nothing in the repository invokes one. It is declared in
`rust-toolchain.toml` and is not obviously earning its line — small enough that this file
records it rather than acting on it.

### 4.1 The compiler's needs and the user's are not the same thing, and this is the crux

**What *builds* `nikaia` needs**, per `rust-toolchain.toml`: 1 420 MiB and 24.7 s, of which
`rustc-dev` is 654 MB — and it is needed only to compile `rustc-executor` against
`rustc_ast`.

**What *running* a `.nika` file needs** is a different list, because the emitted Rust is
ordinary stable Rust and a project build hands it to `cargo`:

| the binary they were given | what the user must have | install | on disk |
| :--- | :--- | ---: | ---: |
| built `--no-default-features` (no bridge) | **any** recent stable toolchain | 11.05 s | 577 MiB |
| built with the bridge | `nightly-2026-01-01`, **at the path the build machine had it** | 11.37 s | 579 MiB |

Two megabytes apart, which is the opposite of the way the question is usually framed. The
real cost is in the second column's small print, and `readelf`/`ldd` are where it shows:

```
$ ldd target/release/nikaia                      # built with the bridge
    librustc_driver-bcf6c5c396c0890f.so => /root/.rustup/toolchains/nightly-2026-01-01-…/lib/…
    libLLVM.so.21.1-rust-1.94.0-nightly          => …
$ readelf -d target/release/nikaia | grep RUNPATH
    Library runpath: [/root/.rustup/toolchains/nightly-2026-01-01-x86_64-unknown-linux-gnu/lib]

$ ldd target/release/nikaia                      # built --no-default-features
    libgcc_s.so.1, libc.so.6, ld-linux-x86-64.so.2
```

So a bridge-enabled `nikaia` is **not a relocatable binary**: the soname carries a
per-build hash (a stable install has `librustc_driver-83018425804cb0fc.so`, which is the
wrong file), and the runpath is an absolute path into whoever built it. A compiler without
the bridge links libc and nothing else, and 4 037 608 bytes of it is the whole thing against
3 830 568 with the bridge — it is *larger*, because it carries its own `std` instead of
borrowing rustc's.

### 4.2 Building the compiler, timed both ways

Clean `release` build of the `nikaia` binary, `rm -rf target` first, three runs:

| | median | spread | binary |
| :--- | ---: | ---: | ---: |
| nightly, with the bridge | 39.13 s | 38.90–43.19 | 3 830 568 B |
| stable, `--no-default-features` | **38.02 s** | 37.76–38.76 | 4 037 608 B |

They are the same build, inside the noise of this box. The first measurement taken put the
stable one at 52.6 s, and repeating it is the only reason this table does not say that the
bridge makes the build faster. **Build time is not a term in this decision**; install size
and toolchain identity are.

---

## 5. A stable-only installation, and what was needed to make one

`[[bin]] name = "nikaia"` had `required-features = ["rustc-backend"]`, so a build without the
bridge produced a library and no compiler. Four changes, none of them in a frontend:

1. `main.rs`'s `#![feature(rustc_private)]` became `#![cfg_attr(feature = "rustc-backend",
   feature(rustc_private))]`, and `extern crate rustc_driver` and the Bridge-IR frontend
   are now behind the same `cfg`;
2. the `required-features` line is gone, and the bridge arm of the backend match is two
   functions, one per `cfg`;
3. the absent one **fails by name** — [ADR-021](specification/adr/adr-021.md) D9's rule about
   `cranelift`, applied to a backend that was configured out rather than never written —
   and the message says which feature, which toolchain component, and roughly what it
   weighs. `crates/nikaia/tests/backend_absent.rs` asserts that, and asserts that the
   `rust` backend still lowers, so the first test cannot pass on a compiler that refuses
   everything;
4. `crates/nikaia/tests/common/mod.rs` no longer passes `-Zls=root` (§2), which is what let
   the suite run at all on stable.

The result, on this box:

| | |
| :--- | :--- |
| `rustup run stable cargo build -p nikaia --no-default-features --release` | 38.02 s, 4 037 608 B, links libc only |
| `rustup run stable cargo test -p nikaia --no-default-features` | **all green**, 0 failures |
| the same `--release` | all green, 0 failures |
| `rustup run stable cargo clippy -p nikaia --no-default-features --all-targets -- -D warnings` | clean |
| `--backend bridge` on that binary | refused by name, exit non-zero, nothing compiled |

The green suite is the corpus passing through a stable build: `examples.rs` lowers every
runnable example at **both** settings of `user_parallelism`, compiles the emitted Rust with
the `rustc` that built the test — here a stable one — runs the binary and compares what it
printed; `one_brc.rs`, `modules.rs`, `project.rs`, `ledger_determinism.rs` and
`foreign_runtime.rs` do the same for their own programs. A second CI leg runs exactly that,
so the claim cannot rot quietly.

What a stable build does **not** have is the bridge backend, which is the default backend.
So `nikaia --input x.nika` with no `--backend` fails there, by name, and the user has to say
`--backend rust`. Whether that is acceptable — whether the default should resolve per build,
or whether the bridge stays the default everywhere and a stable installation is a
`--backend rust` installation — is a decision [ADR-004](specification/adr/adr-004.md) and
[ADR-021](specification/adr/adr-021.md) D9 own and neither makes. It is **not** answered here
and nothing was changed to imply an answer: the default is still `bridge` in both builds.

> **Answered since, by [ADR-004](specification/adr/adr-004.md) D4:** the default is `rust`
> everywhere, and the bridge is optional rather than the backend every installation carries.
> So the paragraph above describes the situation this file measured, not the one that
> followed from it — a stable build is now a whole installation and `nikaia --input x.nika`
> works there with nothing extra typed. The measurements are unchanged; what changed is the
> decision they were handed to.

---

## 6. What this changes in the records, and what it does not

* [ADR-001](specification/adr/adr-001.md) D1 stands — one exact nightly per release, and
  `rust-toolchain.toml` as the single source of truth. What is corrected in it is the
  *reason*: "`-Z` flags and `rustc_private`" is one item and not two, and the pin is paid for
  by the bridge backend alone.
* [ADR-005](specification/adr/adr-005.md) D2 stands. Polonius does what D2 says it does, and
  the shape is reachable from Nikaia. What is corrected is everything written in the present
  tense about a flag nothing passes, and what is added is the price.
* [ADR-004](specification/adr/adr-004.md) was untouched by this file. The bridge backend
  exists, it was still the default when this was written, and nothing here proposed removing
  it — nor does D4, which made it **optional** and not the default while leaving the backend
  and [ADR-003](specification/adr/adr-003.md)'s hub and spoke exactly where they were.
* Unanswered here, and handed on rather than decided — **what the default backend is on a build
  that has no bridge** — and since answered by [ADR-004](specification/adr/adr-004.md) D4: it
  is `rust`, on every build. Also still open from [`subprocess-cost.md`](subprocess-cost.md) §5 and
  adjacent to everything above: whether the executor should invoke the rustup shim or a named
  compiler. The two questions are the same question seen from either end — *which* `rustc`
  a Nikaia installation is entitled to assume.
