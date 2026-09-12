# What `std`'s build graph cost, and how it was measured

Laboratory notes for [ADR-002](specification/adr/adr-002.md) D4. Nothing here is
normative; the decision and the numbers that settled it are in the record, and
this is the method, the machine, and the two things that turned out differently
from what the shape of the problem suggested.

## 1. The machine

A shared virtual machine, `x86_64-unknown-linux-gnu`, 4 cores visible, on the
toolchain this repository named at the time — `nightly-2026-01-01`
(`rustc 1.94.0-nightly 8d670b93d`) with `rustc-dev`. Cargo's package cache warm in every measurement — the question is
what is *compiled*, not what is downloaded, and a cold registry would add a
constant to both sides.

Every "before" figure here was taken on this machine from the commit before the
change, and every "after" figure from the commit that made it, so the comparison
carries no machine difference. The 97 / 58 / 29 s that **decided** the question
was measured earlier on a different box; it is quoted in the record as the reason
and is not re-derived here.

## 2. Method

One hello-world project, built from nothing:

```toml
# nikaia.toml
[package]
name = "hello"
version = "0.1.0"
```

```nika
fn main() {
    println("hello")
}
```

```sh
time nikaia build --project .
```

with a `--release` compiler (`cargo build --release -p nikaia`, 39.4 s on this
box, which is the 40 s the record quotes). Three figures come out of it:

* **packages resolved** — `grep -c '^\[\[package\]\]'` over the generated
  `target/nikaia/build/Cargo.lock`. This counts the lockfile, not the platform
  graph, which is why it is larger than what `cargo metadata
  --filter-platform` reports (83 before). The lockfile is the honest number for
  this question: it is what a reviewer sees and what has to be resolved.
* **how many of them exist only to build `std`** — `cargo metadata` for the
  generated package, reachability from the root twice, once following every
  dependency edge and once skipping edges whose only kind is `build`. The
  difference is the set. It is computed rather than eyeballed because the set is
  58 names long and half of them are `windows_*` crates nobody would guess at.
* **units compiled** — `grep -c '^   Compiling'` over Cargo's own output.

## 3. What it cost, before and after

| | packages in the lockfile | only to build `std` | units compiled | cold build |
| :--- | ---: | ---: | ---: | ---: |
| before | 103 | 58 | 79 | 25.8 s |
| after | 46 | 1 | 38 | 16.0 s |

The **58 is identical** to the figure measured on the other machine, which is the
useful part of re-measuring: the package count moved (103 against 97, five months
of version drift in the transitive graph) and the size of the defect did not,
because it is a property of the dependency edge rather than of the registry.

The whole 58 is listed here once, because "58 packages" is easy to read as an
accounting artefact and the names are not:

> anstream, anstyle, anstyle-parse, anstyle-query, anstyle-wincon, anyhow,
> block-buffer, **bridge-ir**, **bridge-orchestrator**, clap, clap_builder,
> clap_derive, clap_lex, colorchoice, cpufeatures, crypto-common, digest,
> equivalent, generic-array, hashbrown, heck, home, indexmap,
> is_terminal_polyfill, itoa, linux-raw-sys, **nikaia**, once_cell_polyfill,
> rustix, serde, serde_core, serde_derive, serde_json, serde_spanned, sha2,
> strsim, syn, toml, toml_datetime, toml_parser, toml_writer, typenum,
> utf8parse, version_check, which, windows-sys, windows-targets, and the eight
> `windows_*` target crates, winnow, winsafe, zmij

`nikaia`, `bridge-ir` and `bridge-orchestrator` are the compiler, under the names
its crates had when this was measured — `bridge-ir` has since been withdrawn and
`bridge-orchestrator` is now `orchestrator`. It was being built inside the
project's `target/` by the project's own build — while the installed compiler was
running, and doing the very lowering that build script existed to do.

The **1** that is left is `version_check`, the build script of a crate `std`
genuinely depends on. It is not a residue of the defect and there is nothing to
remove: a build-dependency that belongs to a runtime dependency is what a build
dependency is for.

## 4. The cross-project figure, which is the one the cache is for

Three builds of the same hello-world, each from an empty project directory:

| | cold build | units compiled |
| :--- | ---: | ---: |
| first project, empty rlib cache | 16.0 s | 38 |
| second project, same cache | **1.65 s** | 1 |
| a project with the cache switched off (`CARGO_TARGET_DIR` set) | 16.0 s | 38 |

The third row is what every project paid before, and it is also the documented
way out, which is why it is measured rather than assumed: setting
`CARGO_TARGET_DIR` has to keep working and has to cost exactly what it costs.

`crates/nikaia/tests/project.rs::a_second_project_links_the_std_the_first_one_built`
asserts the middle row, and it asserts it by the rlib's **modification time**
rather than by the absence of a `Compiling nikaia-std` line in a log. An absent
log line says Cargo was quiet; an unmoved mtime says the file was not written.

## 5. Two things that were expected to matter and did not

**Installation size cannot decide where `std` comes from.** The toolchain this
was measured on — the pinned nightly with `rustc-dev` — is 1.4 GB, of which
`rustc-dev` alone is 654 MB (the 931 MB this note first quoted was
`lib/rustlib/<triple>`, which holds `rust-std`'s rlibs too — `nightly-cost.md`
§4 measured it properly afterwards). `nikaia-std`'s rlib is 1.9 MB. (That nightly is
withdrawn; against the 602 MiB a stable toolchain weighs the conclusion is
unchanged, two and a half orders of magnitude instead of three.) Whether `std`
ships pre-built, ships as sources, or is built locally moves something orders of
magnitude below what already dominates a Nikaia installation — so the argument had to be made on build
behaviour, and the size question is simply not in it. This is worth writing down
because "ship it pre-built, it's only a few megabytes" is the intuitive answer and
it optimises a rounding error.

**Building the compiler from source is cheap.** 40 s and 209 MB
(`cargo build --release -p nikaia`). That is what makes a from-source install an
ordinary option rather than a fallback, and it is what makes shipping pre-built
artifacts per platform buy little: the matrix costs more to maintain than the
40 s it saves.

## 6. What was verified rather than assumed

**`std` is one build, not one per build switch.** The claim the rlib cache's key
rests on is that `user_parallelism` never reaches `std` as a compile-time
condition. Checked two ways: `crates/nikaia-std/Cargo.toml` declares no
`[features]` at all, and every `#[cfg]` in `crates/nikaia-std/src/` is
`target_os` (13 of them, all in `rt/mod.rs`, all about whether `io_uring` exists).
What the switch does instead is travel as a value — the emitter writes
`rt::start(rt::UserCode::Sequential)` or `::Concurrent` into the generated
`fn main`, and the runtime decides what to start from that argument.

So the key carries the `target` and not `user_parallelism`. If a feature or a
`cfg` on the switch ever appears in `std`, that sentence stops being true and the
key is wrong in D7's "too little" direction — which is the failure that looks
like a miscompilation.

**The pre-lowering is switch-insensitive today, and it is a test.** Every `.nika`
file in `std` is lowered at `user_parallelism = no` and at `yes` and the bytes
compared (`crates/nikaia/tests/sysroot.rs`). With one file and one function in it
this is nearly free and nearly trivial — and it is the only thing standing between
"lowered at one setting" and a `Shared` lowered to `Rc` inside a program built at
`yes`, which is what [ADR-037](specification/adr/adr-037.md) D3 would produce.

## 7. One thing left for the owner

The rlib cache is a Cargo target directory whose *path* is our key, so the
project's own crate is built inside it too, not only `std`. That is what makes the
second project cost 1.65 s with no linking surgery, and it has two consequences
worth a decision rather than a note:

* `rm -rf target` no longer gives a cold build. The state that survives is in the
  user's cache directory, and [ADR-021](specification/adr/adr-021.md) §3 already
  owes an eviction policy for the other store there.
* Two builds that share an entry serialise on Cargo's lock on that directory.
  Two projects with their own `target/` did not, so a developer building two
  projects at once now waits where they did not before. Unmeasured, because the
  cost is Cargo's lock and not ours.
