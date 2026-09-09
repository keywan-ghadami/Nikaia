# Handoff — open work on error messages and the parser backend

Written at the end of a session that could not finish, because the change it
depends on lived in a repository that session had no push access to. That
change is merged and Nikaia is on it; what is still open is below. Read this
file first.

---

## 1. The blocked thing — landed

The `winnow-grammar` change is **merged upstream** (winnow-grammar#4, `e1b0e33`)
and Nikaia is on it: `Cargo.lock` names that commit and
`tests/errors/EXPECTED.txt` is regenerated against it, so the corpus is green
as checked in and its rows now describe what a user actually gets.

`docs/upstream/0001-expectations-by-requirement.patch` stays here as the record
of what was handed over — it is the first of the five commits that landed:

* the ranking itself (the patch, unchanged but for a clippy lint in its test),
* SYNTAX.md and CHANGELOG.md, which the patch had not touched — the documented
  order of message selection was still progress-then-priority,
* two documentation errors that had `cargo doc` failing on upstream `main`
  since `54acc33`,
* `text(p)`/`dec<T>(p)` inside a `#[frame]` — §4.2 below, now closed,
* the remaining findings, as TODO items §5 and §6 upstream.

Twelve of the twenty-one failing corpus rows moved. The nine that did not are
§3, and they are the open work on messages.

### To test a further backend change against Nikaia

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
what the compiler says, in two columns — before the ranking change and today.
`tests/errors/*.nika` are the inputs and `tests/errors/EXPECTED.txt` is what
they produce against the backend `Cargo.lock` names, so the suite is green as
checked in.

```bash
cargo run -p nikaia --example errors > tests/errors/EXPECTED.txt   # regenerate
```

**A backend bump that changes a message makes `errors.rs` fail, and that is the
point:** the diff is the change in the reader's terms. Read it, then regenerate.

**One row is left.** C2 reports `expected digits` where the reader needs `->`,
and that is a grammar question rather than a message one: `digits` comes from
the built-in name table and the rule really is looking at a repetition there.

What closed the other twenty, upstream, each measured against this corpus:

| change | what it closed |
| :--- | :--- |
| ranking by requirement (winnow-grammar#4) | the whitespace skip out of the headline |
| a rule may name itself (#5) | `expected expression`, `expected type` |
| an element that *began* is a requirement (#6) | A3, F3, and the second expectation in A1, A2, C3, C5, E3 |
| a losing alternative keeps its error (#8) | A4, B1, G1, F1 |

The last one is worth reading before touching this area again. A4, B1 and G1
were filed twice as "trivia wins on progress" and were never that: `alt` drops
what a losing alternative found, so where a *shorter* alternative wins - `let`
and `xs` each parse as an expression statement in `let xs = [1, 2` - nothing
survives at the position the input actually goes wrong except the whitespace
skip. Two repairs were tried against this file and reverted before the third
worked, and `docs/error-corpus.md` keeps all three.

**One tick has a footnote.** F1 names the `"` it is missing, and its position
is still the end of the file rather than the opening quote. Remembering the
opening delimiter is a separate mechanism and is not done.

### The label obstacle, and how it was removed

`expected expression` instead of six token spellings needed `# "…"` on a rule
with many alternatives, and the backend could only label a *single* variant.
Both halves are upstream now (winnow-grammar#5):

* a rule may name itself between its return type and its `=`;
* the leading whitespace skip of a labelled rule is hoisted **outside** the
  label, because `ParseError::labelled` substitutes only when the error is at
  the position the label started at - measured from inside the skip, it never
  is.

Nikaia's own grammar labels `expr`, `unary_expr`, `stmt`, `item` and
`type_ref`; `.nika` grammars can use the same syntax (Part II, 10.6), and
`examples/calc.nika` does. `unary_expr` is labelled as well as `expr` for a
reason worth keeping: `1 + ` fails inside `add_tail`, whose operand is a
`mul_expr`, so the label on `expr` never sees it.

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
2. ~~**`dec(p)` and `text(p)` cannot appear inside a `#[frame]`**~~ — **done**,
   on the upstream branch above. The frame check had no arm for them, so
   `_ => true`; they are now excepted with the reasoning `intern(p)` already
   carried, and `tests/ui/frames.rs` upstream pins that an argument which *can*
   consume the boundary is still rejected. `examples/1brc.nika` can drop its
   `unchecked` once Nikaia is on that backend. Findings note §3.
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

`claude/nikaia-compiler-lowering-zaqa0y` (PR #9), and everything below it is
pushed and green: 28 tests, `cargo fmt --check` clean, clippy clean. This file
is updated on `claude/nika-2-branches-offene-aufgaben-kd77u1`, which is that
branch plus this note.

ADR-009 no longer sits below it: its two commits landed on `main` on their own
(PR #10), so what PR #9 still adds is the Stage 0 lowering and everything
after it.

* `Cargo.lock` is on upstream `024e3d3`. The measurements in ADR-011 §4 were
  taken on `2f0d5da` and say so.
* `examples/1brc.nika` runs and prints what the benchmark asks for.
* The 203 explicit `_sp:skip_ws` bindings are gone (commit `8888c3c`). **That is
  a precondition for the patch to reach `.nika` files**, not a cleanup: an
  explicit `skip_ws` calls `WS` as an ordinary rule, past the mark the code
  generator puts on the whitespace it inserts itself. Do not put them back.
