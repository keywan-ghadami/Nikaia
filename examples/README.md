# Nikaia Examples

Six of the seven programs here compile, run, and are checked by `cargo test`. The seventh is
written at specification level — it shows what Nikaia 0.0.7 is meant to look like, and what it
needs is listed under *Gaps* below.

| | what it is | runs |
| :--- | :--- | :--- |
| [`1brc.nika`](1brc.nika) | the One Billion Row Challenge: a frame, a parallel fold, a billion rows | ✅ `crates/nikaia/tests/one_brc.rs` |
| [`calc.nika`](calc.nika) | a four-function calculator: the grammar protocol at its smallest | ✅ `crates/nikaia/tests/examples.rs` |
| [`access-log.nika`](access-log.nika) | a web log summarised: several fields per line, a report at the end | ✅ `crates/nikaia/tests/examples.rs` |
| [`config.nika`](config.nika) | an INI file with comments: a grammar that defines its own whitespace | ✅ `crates/nikaia/tests/examples.rs` |
| [`json.nika`](json.nika) | a JSON document: a tree of unbounded depth, and where zero-copy stops | ✅ `crates/nikaia/tests/examples.rs` |
| [`n-body.nika`](n-body.nika) | the CLBG benchmark: arithmetic in a loop, and no grammar at all | ✅ `crates/nikaia/tests/examples.rs` |
| [`fortunes.nika`](fortunes.nika) | the TechEmpower benchmark: a SQL DSL and an HTML template DSL in one handler | ❌ needs G6 and G7 |

Each of the six is compiled and run **under both profiles**, and their output must be
identical — that is the claim the profiles rest on, and a test is where it belongs rather than
in a paragraph. Each is the real file: the tests read `examples/*.nika` rather than a copy, so
an example cannot drift from what is checked.

They are deliberately different shapes.

* **`1brc.nika`** is the protocol at scale — `@frame`, `par_fold`, a memory-mapped file, one
  accumulator per core ([ADR-014](../docs/specification/adr/adr-014.md): 8 million lines, 4
  cores, 0.52 s → 0.14 s, identical output), and one function it calls for every digit is
  written in Nikaia and compiled into `std` by the compiler itself.
* **`calc.nika`** is the same protocol with none of that: no frame, no fold, no I/O, one
  `pub rule` that returns a number — recursion and precedence, which is what a grammar can say
  and a chain of combinators cannot.
* **`access-log.nika`** is the shape most real work has: a line with several fields of
  different kinds, a record built from them, a report at the end — including what a *rejected*
  line looks like.
* **`config.nika`** is the one whose result is a *tree* rather than a number or a tally, and
  the one that defines its own `WS` so that `#` comments are legal everywhere a blank is
  without another rule mentioning them. It is also the counterpart to 1BRC's fold: `setting*`
  collects, which is right for a configuration file and wrong at a billion rows.
* **`n-body.nika`** is the one with **no grammar in it at all**. Five programs in a row that
  all begin with a DSL would say Nikaia is a parser generator; this one is arithmetic in a
  loop — `sync` methods, `&mut self`, indices and floats — and it is the only example here
  whose numbers are directly comparable against other languages, because the CLBG publishes
  the same program in some thirty of them with an exact expected output. Ours matches it to
  the digit.
* **`json.nika`** is the first input whose shape the *grammar* does not fix. `config.nika`'s
  tree is two levels deep because the grammar says so; a JSON value contains values, to
  whatever depth the document happens to go. An `enum` is that tree — six variants, and every
  walk over it is a `match` the compiler checks for completeness — and this is also the example
  where zero-copy stops and says so: a JSON string is not a slice of the input, so `Text` holds
  the **raw** body and decoding waits until a program asks about it.

An example that is added has to be declared: either it runs and says what it prints, or it is
specification-level and its gaps are here. `crates/nikaia/tests/examples.rs` fails on a file
that is neither.

What the bootstrap compiler handles: functions and methods, `impl` blocks, `struct` and `use`
items, `let`, assignment, `for`, `if`, `return`, calls, field access, indexing, casts, struct
literals and constructors, lambdas, operators, `throws`/`catch`/`??`, string interpolation — and,
since [ADR-011](../docs/specification/adr/adr-011.md), the whole `grammar` construct: rules,
patterns, `@frame`, `fold`/`par_fold`, and `dsl … from …` with the driver the profile asks for.
Errors are reported on the `.nika` line that caused them
([ADR-012](../docs/specification/adr/adr-012.md)). There is no type checker.

That is deliberate, and it is what these files are *for*. Writing a real program against the
spec is the cheapest way to find out which parts of the spec are underspecified. The gaps each
example exposed are listed below; they read as a roadmap.

`tests/samples/` holds smaller programs that only have to *parse* — one construct each, checked
by `crates/nikaia/tests/samples.rs`. An example graduates the other way: when the compiler
catches up with one, it stays here and gains a row in `RUNNABLE`, because what makes it worth
having is that it is a whole program.

---

## Language benchmarks worth targeting

There are three established suites plus one modern challenge that fit a systems language.
They test different things, and only one of them matches what Nikaia currently claims to be
good at.

### 1. One Billion Row Challenge (1BRC) — **recommended first target**

Read a 13 GB text file of `station;temperature` lines, aggregate min/mean/max per station,
print them sorted. One input file, no dependencies, a one-paragraph spec.

It is the best fit because it stresses exactly the three things 0.0.7 asserts:

* **Scannerless parsing** (ADR-007 D1) — a billion lines of a tiny custom format.
* **Zero-copy / tethered slices** (Part II, 10.6) — station names must point into the input
  buffer; allocating a billion strings loses by an order of magnitude.
* **`par_iter` and the `sync` rule** (12.1, 12.6) — the aggregation is pure computation, so
  the Advanced profile can use every core, and the compiler can prove no task pauses.

If Nikaia is slow here, the grammar protocol's performance argument is wrong. That makes it a
useful benchmark rather than a demo. See `1brc.nika`.

### 2. Computer Language Benchmarks Game (CLBG) — **best for comparable numbers**

Ten programs with exact output specifications and reference implementations in ~30 languages,
so results are directly comparable against Rust, C and Go.

| Program | What it exercises in Nikaia |
| :--- | :--- |
| `n-body`, `spectral-norm`, `mandelbrot` | `sync` functions, `par_iter`, Advanced profile |
| `binary-trees` | allocation and deterministic teardown (`Drop` / `Cleanup`, ADR-006) |
| `fannkuch-redux` | scoped tasks (12.7) |
| `reverse-complement`, `k-nucleotide` | **stdin/stdout IO** plus hashing |
| `regex-redux` | the `grammar` construct against a real regex workload |
| `pidigits` | bignum FFI (Chapter 15) |

Small, self-contained, no external services. `n-body` is the usual first one because it is
~100 lines and purely numeric.

### 3. TechEmpower Web Framework Benchmarks — **the Lite profile's actual thesis**

`plaintext`, `json`, `db`, `queries`, `fortunes`, `updates`, `cached-queries`.

This is the benchmark that matches Nikaia's pitch: implicit async, IO density, share-nothing,
WASM-compatible Lite profile. `fortunes` in particular exercises the 0.0.7 DSL protocol
end-to-end — a SQL DSL with deferred parameters *and* an HTML template DSL with capture holes,
in one request handler. See `fortunes.nika`.

The cost is real: it needs an HTTP server, a Postgres driver and the Docker harness. This is a
target for after self-hosting, not before.

### 4. Are We Fast Yet? — **for judging codegen later**

Marr et al.'s cross-language suite (DeltaBlue, Richards, Havlak, CD, Json). Deliberately
written using only constructs every language shares, so it measures the *compiler* rather than
library tricks. No IO. Worth revisiting once there is a real backend to judge.

**Not applicable:** DaCapo and Renaissance (JVM-only), SPEC CPU (licensed, C/Fortran),
CoreMark (embedded C microbenchmark).

### Suggested order

1. `1brc` — validates the 0.0.7 claims, needs only file IO. ✅ [`1brc.nika`](1brc.nika)
2. CLBG `n-body` — first comparable number against other languages. ✅
   [`n-body.nika`](n-body.nika), matching the published output for n = 1000 to the digit.
3. CLBG `reverse-complement` and `k-nucleotide` — stdin/stdout IO with exact expected output.
4. TechEmpower `fortunes` — once an HTTP stack and a DB driver exist.

---

## Gaps these examples exposed

Writing the two programs surfaced spec questions that a real implementation must answer.

### Resolved

**G1 — `std::fs` named no functions.** The module was described ("looks blocking, is async")
but had no surface. Now specified in Part III, 17.1: whole-file (`read`, `read_to_string`,
`write`), streaming (`lines`, `bytes`), handles (`open` → `File`, which implements `Cleanup`
so a failed flush surfaces as an error instead of being swallowed), memory mapping (`map`,
a **compile error under Lite** because WASM has no mmap and degrading it to a full read would
turn a constant-memory program into one that allocates its whole input), metadata and
directory calls, and a per-profile availability table.

**G2 — a grammar rule that folds instead of collecting.** An earlier draft of
this note claimed a gap around "applying a grammar per line". That was wrong,
and it misread what the language offers: you do not run a grammar per row. You
write down what the *file* looks like and get a parser for it, which drives
itself over the bytes — paying the DSL protocol once instead of a billion times.

The real gap was narrower and only appeared at the entry rule:
`rule file -> Vec[Reading] = measurement*` collects, and a billion `Reading`s do
not fit in memory. `fold` now exists in `winnow-grammar` (upstream, with
`SYNTAX.md` documentation and tests), so the entry rule threads an accumulator
and never builds a collection:

```nika
pub rule file -> Summary =
    fold(measurement, Summary::new, fn(acc, m) { acc.record(m) })
```

**G3 — tethered slices in user structs.** Specified for tokens a parser yields (Part II,
10.6), but not for a slice stored in a struct of one's own. `Reading.name` and the `HashMap`
key in `1brc.nika` are exactly that, and it is the difference between one allocation and a
billion.

Writing the example did more than expose the gap — it showed the existing rule was stated over
the wrong event. "Stored in a struct ⇒ tethered" (Part I 6.6, as of 0.0.7) puts a shared handle
on `Reading`, which this program builds a billion times; under the Advanced profile that is a
billion atomic increment/decrement pairs on one refcount word shared by every worker. Nothing
in the program actually escapes, so the right answer costs nothing at all.

[ADR-008](../docs/specification/adr/adr-008.md) settles it: view types stay (`&str` is a view
marker, not a lifetime), the rule is restated over **escape** rather than storage, a view has
three inferred states (Borrowed ⊑ Tethered ⊑ Owned), the shared handle sits on the *container*
rather than on each slice, and `.to_owned()` is never inserted for you. `@borrowed` turns "this
stays a plain reference" into a compile-time assertion for hot structs — used in `1brc.nika` on
`Reading`. Under those rules the program allocates nothing per row and does no refcount work in
the parallel section.

**G4 — chunking a buffer for `par_iter`.** `par_iter` is specified over a collection.
Splitting a buffer at line boundaries into one chunk per core, with each chunk a slice of the
original, had no spelled-out form.

[ADR-009](../docs/specification/adr/adr-009.md) closes it by **dissolving** it rather than
specifying `split_aligned`. A chunking helper asks the user to restate as an argument what the
grammar already says — that a measurement ends at `"\n"` — and then to hand-write the
split/parallel/merge pipeline that follows from it. Instead the grammar carries both halves:
`@frame` marks a rule as a resynchronization unit — and the compiler *verifies* it, and never
rewrites the grammar to make it so: what a frame can reach is either safe or rejected with the
rule and pattern named (a literal containing the boundary — CSV with quoted newlines — `any`,
`multispace0`, a syntactic rule whose implicit whitespace eats newlines, an `until` that does not
cover the boundary, `recover`). The flagship `NAME` therefore *says* `until(";" | frame_end)`,
where `frame_end` names the boundary of the enclosing frame; an intermediate design that silently
bounded `until(";")` was rejected in review because the same rule text then parsed differently
depending on what reached it. `par_fold(rule, init, step, merge)` supplies the monoid, and its
parser skips no whitespace at its entry, so pieces and the sequential parse agree on every input,
rejections included. All of it is in `winnow-grammar` `main` (`#[frame(boundary = …)]`,
`frame_end`, `par_fold`, `unchecked` for the formats a byte-string boundary cannot cut — its
ADR 16 names them), with `frames_<RULE>`, `merge_<RULE>` and the driver
`parse_<RULE>_pieces(input, ctx, Parallelism)` generated; Nikaia chooses the `Parallelism` from
the profile and the executor. The blind split, the
seam repair, the per-core accumulators and the reduce are then generated; `1brc.nika`'s `main`
is down to `let totals = dsl Measurements from data`.

The same ADR settles what the compiler may then do with the format the grammar states:
word-at-a-time scanning as a specified complexity rather than a hoped-for optimization
(portable SWAR baseline, SIMD only as a target-gated layer, so Lite and `wasm32` keep the same
story), hashing a view from its first bytes while equality stays full-content, and skipping the
unmap of a large read-only mapping at process exit. Parallelism stays opt-in: a plain `fold` is
never parallelised behind your back, because a merge over floats would make the answer depend on
the core count.

**G8 — the default hasher.** Raised by ADR-009, which framed it as a profile question and was
wrong to. The profile answers "which runtime", never "who supplied these bytes": a single-threaded
Lite server hashing attacker-supplied header names is exactly as vulnerable as an Advanced one,
and a compute job over operator-chosen data has no adversary in either.

[ADR-010](../docs/specification/adr/adr-010.md) decides it on the axis that matters, **provenance**,
and at the level where the user actually knows the answer — the place the input enters. Sources are
classified by `std` (network, IPC and database rows untrusted; files, argv, env and compile-time data
trusted), the state travels the edges ADR-008 already tracks and lands in the same Ledger, joins
conservatively, and fails safe at `dyn`/FFI barriers. The user overrides it at the source
(`fs::map(path; trusted: false)`) and a DSL for a wire format can pin an `@untrusted` floor its
callers cannot lower. Only then does the compiler pick an implementation: keyed hash with a random
seed for untrusted keys, fast hash for trusted ones — the profile enters as *how*, never as
*whether*. 1BRC keeps the fast path without a word about hashing; `fortunes` gets hardened without
anyone remembering to ask.

**G9 — bounded repetition (`digit{1,2}`) in the grammar protocol.** ADR-009 D5: fixed-width
numeric parsing is only sound where the grammar states the width bound. Delivered upstream —
`p{n}`, `p{n,}`, `p{n,m}`, together with a single-digit `digit` terminal that turned out to be
missing too (only the greedy `digit1` existed, which would have swallowed the run before a bound
could count anything). `1brc.nika`'s `TENTHS` now states its width and rejects a three-digit
temperature.

**G5 — ordered iteration over a map.** 1BRC's output must be sorted by station name, and
`access-log.nika` wants its paths by hit count; neither an ordered map nor a way to say an order
over a list was specified. ADR-010 D6 is why it cannot be left to the map: an untrusted one is
seeded randomly, so its iteration order differs between runs. The answer is the explicit form,
and now it is written down — Part I 4.5 specifies `sort()` and `sort_by_key`, and says both are
**stable**, which is what lets two passes state a compound order without a comparator:
`names.sort()` then `names.sort_by_key fn: -hits` is hits descending, ties by name.
`access-log.nika` prints that way.

**G10 — no tuple.** Found by `calc.nika`, which has to carry an operator alongside the operand
it applies to (`3 * 4`, where the `*` must survive until the fold). A pair of values of
different types with no name for the group is a *tuple*, and Stage 0 had no way to write one:
a type was a name with optional generic arguments, so the example declared a two-field
`struct Step` to say what one line of a tail rule should have said — a struct pretending to be
a type. Tuples now exist: `(A, B)` as a type, `(a, b)` as a value, `t.0` to read a part
(Part I, 4.5). The parts live where a named type's arguments live, so everything that already
walked a type's arguments — the view analysis of ADR-008 among them — walks a tuple's parts
without knowing about tuples. `mul_tail` yields `(&str, i64)` and `struct Step` is gone.

**G12 — no sum type.** `calc.nika` has exactly two operators to carry, and an `enum` is what
says that. Part I 4.4 specified enums and the bootstrap compiler did not lower them, so the
example carried the operator as a `&str` and compared it — one typo away from a bug the
compiler cannot see. Enums and `match` now lower: the three variant shapes (`Quit`,
`Write(String)`, `Move { x, y }`), and five pattern shapes, each one the language below spells
the same way so the lowering stays a transcription. `calc.nika`'s `mul_tail` yields
`(Op, i64)`. An enum that carries a view takes the input lifetime exactly as a struct does,
which is what lets one appear in a grammar rule's return type.

**G14 — arithmetic did not read like arithmetic.** Found by `n-body.nika`, which is nothing
but arithmetic and so found four things at once. **A range was not an expression**: Part I 3.3
shows `for i in 0..5` and the bootstrap compiler could not parse it, so an index loop had no
way to be written. `..=` comes with it, and a range binds looser than the arithmetic in it —
`0..n - 1` ends at `n - 1`. **A float could not carry an exponent**, so
`9.54791938424326609e-04` had to be spelled out in zeroes, which is how a digit gets lost.

The two beneath those were bugs rather than gaps, and both were **silently wrong**. A group is
not a node — the parser drops it, because that is how the tree was written rather than part of
it — so `(a as f64).sqrt()` came out as `a as f64.sqrt()`, a cast to a type nobody named. And
the compiler's identifier accepted a **leading digit**, because the backend's `ident` does and
a grammar that wants otherwise has to say so: `1.5` parsed as the field `5` of a variable
called `1`. That one printed back identically, which is exactly why it survived — the emitted
text read the same right up until there was more after it, and `1.5e-4` was where it stopped.

**G13 — no character literal, and three things that followed.** Found by `json.nika`, whose
`unescape` has to ask what a character is: `'n'` was not an expression the language had, and
`char` was not in the table of primitive types (Part I, 2.2) either — the type was reachable
only because a type is a name. Decoding an escape is exactly the shape a `match` over
characters has, so the literal is a **pattern** as well as an expression (3.4), and the body is
kept **as written**: the language below spells `'\n'` the same way, so nothing decides twice
what it means.

Writing the same function found the two beside it. A string could not hold a `\u{…}` escape —
the interpolation scanner read the `{` as a hole and emitted a `format!` with an argument
nobody wrote — because a string's body reaches the emitter as it was typed and the scanner did
not know an escape when it saw one. And output could not be composed piece by piece: `println`
was the only way to write, so a pretty-printer that indents a tree had no way to put a fragment
on a line without ending it. `print` and `eprint` are now what `println` and `eprintln` always
were, minus the newline (Part III, 17.1). Part I gains **2.5** while it is at it: string
interpolation was in every example and in no chapter.

**G11 — a rejected parse said what was expected and never where.** Found by writing
`access-log.nika`, whose `catch` prints the failure: the message read
``expected a digit; found unexpected token ` ` `` with the rule stack under it, and no
position at all. A `ParseError` carries an offset and not the text it came from, so only the
code that has both can turn 1042 into *line 2, column 26* — and that is the driver
`dsl … from …` lowers to, which is the one place holding the input and the error together. It
now binds the input and renders against it, so the same message reads
``expected a digit; found unexpected token ` ` at line 2, column 26``.

Worth keeping apart from the two things it resembles. `docs/error-corpus.md` is about the
compiler's messages for `.nika` source, which have carried a line and a column all along; its
closing finding is that none of them shows the *line itself*, with a caret, the way a rustc
diagnostic routed through `--explain` does. This was smaller and worse: a program Nikaia
generates had no position at all.

The same example found the bug beside it: `dsl … from …` emitted its own `?`, so a `catch`
next to it was handed the value it was meant to inspect and nothing compiled. Both are pinned
in `crates/nikaia/tests/grammar_lowering.rs`.

### Open

**G6 — the HTTP handler cannot see the request.** *Decided*
([ADR-018](../docs/specification/adr/adr-018.md)), *not yet implemented* — it waits on the runtime
binding. The request is the handler's **first implicit argument**, under the rule Part I 5.3
already has: a lambda takes as many implicit arguments as its body reaches for, so
`fn: "Hello World"` keeps working unchanged and `fn: a.query("name")` reads one. Nothing is added
to the language. A handler *returns* what answers the request — a `String` is 200 text/plain, an
`html::Raw` is 200 text/html (ADR-017 D2 read from the other end: the type that says "this is
markup" is the type that may be sent as markup), a `Response` is itself, and a `throws` that fails
is 500 with a **generic** body and the error in the log, because an error message is written for
the operator. The request's strings are views into the connection buffer (ADR-008), so a parameter
used inside the request's scope costs nothing and one kept past it has to be owned.

**G7 — HTML escaping belongs in the template grammar's contract.** *Decided*
([ADR-017](../docs/specification/adr/adr-017.md)), *not yet enforced*. Every hole is escaped,
unconditionally — no flag at the hole, and no exemption for "trusted" data, because provenance is
evidence about where bytes came from and one wrong `trusted: true` upstream becomes an XSS hole
downstream (ADR-010 D8). The one way to say "this is already markup" is the type `html::Raw`,
because a flag is a property of the call site while a type travels with the value and `Raw::new`
is one line to grep for. And a hole is only legal in a position the grammar can escape *for*: a
hole inside `<script>` or in a URL is a compile error naming the position, because a promise that
holds only in some positions would have to be qualified everywhere.

`std::html::escape` exists and is tested (`crates/nikaia-std/src/html.rs`) — it returns its input
unallocated when nothing needs escaping, so the contract costs a scan rather than a copy. What is
left is the `html` grammar and the per-hole position check.


