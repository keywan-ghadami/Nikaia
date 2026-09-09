# Handoff — open work on error messages and the parser backend

Written at the end of a session that could not finish, because the change it
depends on lives in a repository this session had no push access to. Everything
needed to resume is in this repository. Read this file first.

---

## 1. The blocked thing, and how to unblock it

A change to `winnow-grammar` is **written, tested and committed, but only in a
clone under `/tmp` that dies with the container.** It is preserved here:

    docs/upstream/0001-expectations-by-requirement.patch

542 lines, 7 files, based on upstream `024e3d3`. To resume:

```bash
git clone https://github.com/keywan-ghadami/winnow-grammar.git
cd winnow-grammar && git checkout -b claude/expectations-by-requirement 024e3d3
git am ../Nikaia/docs/upstream/0001-expectations-by-requirement.patch
cargo test --workspace          # 279 tests passed when it was written
```

**The session needs both repositories in its sources.** Without that the git
proxy refuses to inject a credential:

> access denied by the git proxy: keywan-ghadami/winnow-grammar is not in this
> session's authorized repository set

There is no tool to add it from inside a session — it is set when the session is
created.

### To test the patch against Nikaia before it is pushed

Append to `Cargo.toml` (and **remove it again before committing** — it must not
be checked in):

```toml
[patch."https://github.com/keywan-ghadami/winnow-grammar"]
winnow-grammar = { path = "/abs/path/to/winnow-grammar" }
winnow-grammar-macros = { path = "/abs/path/to/winnow-grammar/winnow-grammar-macros" }
winnow-grammar-model = { path = "/abs/path/to/winnow-grammar/winnow-grammar-model" }
```

**Do not edit the vendored checkout under `~/.cargo/git/checkouts/` instead.**
Cargo fingerprints a git dependency on its *commit*, not on file contents, so
the edit is silently inert — `cargo clean -p` does not help either. An hour was
lost to this.

---

## 2. What the patch does, and why it is shaped that way

Nikaia's parse errors named the wrong thing: `expected one of: '//', whitespace`
where `expected ','` was meant. True, useless, and it was **every** syntax error
in the language.

Two axes, and both are needed:

* **A requirement outranks an optional continuation.** The runtime already draws
  the line: `repeat_recording_bounded` *returns* the element's error below the
  minimum and merely *records* it at or above, and `opt_recording` only ever
  records. So `ParseContext::record` is by construction the optional path and
  marks what it stores.
* **Among optional continuations, the implicit whitespace skip ranks last.** The
  first axis cannot separate it: where the entry rule is a repetition
  (`program = item*`) everything is optional. The code generator now calls the
  skip through `rt::skip_trivia`, so what it records is marked — no rule names,
  no declaration, and a `WS` a grammar calls itself stays an ordinary rule.

Nothing is discarded by either axis: the loser becomes a `note: also possible
here: …`. Whitespace alone is left out, and that is the criterion the rest
follows from — the skip is greedy, so more whitespace only moves the same
failure to offset + n.

### Two things that were tried and must not be redone

* **Do not give a `*` repetition's stop-reason a lower priority in general.** It
  is often the useful half: `xs:item* "."` over `1 2 x` needs both the `.` and
  the items. This was rejected on review and the corpus row exists to prove it.
* **Do not take trivia out of the *progress* race** (a second `furthest` slot).
  It looks right and it made case B1 far worse: seventeen expectations in the
  headline, twenty-two in a note, and the position on a token that was correct.
  It was reverted. The three rows it was meant to fix (A4, B1, G1) are still
  open and need a different idea.
* `tests/context_reuse_test.rs` catches an `absorbing` that throws away the
  absorbed error's rule stack — that stack carries `in item 4`, which says
  *which* element failed. Keep it.

---

## 3. The corpus — where to start

`docs/error-corpus.md` is 26 broken inputs, what a reader needs from each, and
what the compiler says, in two columns (shipped, and with the patch above).
`tests/errors/*.nika` are the inputs and `tests/errors/EXPECTED.txt` is what
they produce **today, against the shipped backend**, so the suite is green as
checked in.

```bash
cargo run -p nikaia --example errors > tests/errors/EXPECTED.txt   # regenerate
```

**Applying the patch will make `errors.rs` fail, and that is the point:** the
diff is the improvement, twelve of twenty-one failing rows. Read it, then
regenerate.

The nine rows the patch does not fix sort into three groups, each needing a
different mechanism:

| group | rows | what it needs |
| :--- | :--- | :--- |
| a list where one word belongs | A3, B1, B2, B4, F3 | a label — see below |
| trivia still winning on progress | A4, B1, G1 | unknown; the obvious fix was reverted |
| the position is wrong, not the text | F1, F3 | remember the opening delimiter |

### The label obstacle, already investigated

`expected expression` instead of six token spellings needs `# "…"`. Two
constraints found by reading the backend:

* `ParseError::labelled` substitutes **only when the alternative consumed
  nothing** (`self.offset == start`).
* The code generator wraps a label **only around a single-variant rule**
  (`codegen/variants.rs:137-160`); a multi-variant rule gets a bare `alt((…))`
  with nowhere to hang one.

`primary_expr` has twelve alternatives, so this needs them collapsed into an
inline group — `rule primary_expr -> Expr = (a | b | …) # "expression" -> {…}`.
`parse_group_content` structurally supports it and **no upstream test covers
it**. `type_ref` is already single-variant and can be labelled as it stands,
which is the cheap first experiment (row B5).

### The finding that may be worth more than all of it

**No parse error shows the source line.** Every message is a headline plus
`in <rule>` lines. A *rustc* diagnostic routed through `nikaia --explain` gets a
snippet and a caret (ADR-012, `diagnostics::render`). A reader of
`at line 3, column 5` never sees line 3. This applies to all 26 rows and nothing
above touches it.

---

## 4. Also open, in the order last agreed

Error messages first, performance after — that was the instruction.

1. **The character-class scan threshold.** Upstream `024e3d3` scans a class
   eight bytes at a time (ADR 23). On 1BRC's `digit{1,2}` and `digit` it is a
   loss: `out_mask` costs a fixed ~30 instructions per call against ~4-5 per
   character, so break-even is around six to eight characters. Measured with
   branches and caches simulated, not instructions alone — the scan does buy
   14 067 fewer mispredicts (~0.28 M cycles) for 12.7 M more instructions
   (~3.2 M cycles at IPC 4), about elevenfold against. **Next step:** measure
   where the crossover actually is, and propose a threshold in the code
   generator (`{1,2}` is known at compile time). Detail in
   `docs/upstream/winnow-grammar-findings.md` §2.
2. **`dec(p)` and `text(p)` cannot appear inside a `#[frame]`** — the frame
   check has no arm for them, so `_ => true`. `intern(p)` is already excepted
   with the reasoning that fits all three. Findings note §3.
3. **The intern cache is sized for identifiers** — 512 direct-mapped slots, no
   collision handling. At 413 keys it costs 617 instructions per row against 514
   for a plain fast-hashed map. Making `InternCache::BITS` settable per context
   would close it. Findings note §4.
4. **Input provenance (ADR-010) is specified and not implemented**, and Stage 0
   cannot implement it — choosing a map's hasher needs dataflow the emitter must
   not invent (ADR-011 D2). Costs 22 % of the flagship's instructions.
   ADR-011 §4 has the measurement and why no `.nika` file gains an annotation.

### Do not re-propose these — they were measured and lost

* `dec[i32](digit{1,2})` in 1BRC's `TENTHS`: 693 instructions per row against
  659 for the loop it would replace. `str::parse` does its own scanning, sign
  handling and overflow check.
* Keying the station table by `Symbol` through the intern cache: 617 against 514
  at 413 keys.
* A hardened-vs-fast constructor the user must remember, or a global default
  either way — ADR-010 §4 rejected both before this session started.

---

## 5. The state of the branch

`claude/nikaia-compiler-lowering-zaqa0y`, and everything below it is pushed and
green: 28 tests, `cargo fmt --check` clean, clippy clean.

* `Cargo.lock` is on upstream `024e3d3`. The measurements in ADR-011 §4 were
  taken on `2f0d5da` and say so.
* `examples/1brc.nika` runs and prints what the benchmark asks for.
* The 203 explicit `_sp:skip_ws` bindings are gone (commit `8888c3c`). **That is
  a precondition for the patch to reach `.nika` files**, not a cleanup: an
  explicit `skip_ws` calls `WS` as an ordinary rule, past the mark the code
  generator puts on the whitespace it inserts itself. Do not put them back.
