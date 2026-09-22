# Open decisions — the questions that need the owner

## The shape this page is for
The
entries here are written to be *answerable* — what is blocked, the options, a
recommendation, and what either direction costs if it is wrong.

An answer is an [ADR](specification/adr/), and
the moment a question is answered its entry leaves this file rather than
staying with a note on it. What is merely **unbuilt** is in
[`open-work.md`](open-work.md) — an ADR said what happens and the compiler does
not do it yet, which needs work and not a ruling. Each entry says what the
question is, why it is the owner's, and what this file recommends.

## Open

### Does a **described** foreign function say whether it puts its argument on a thread?

**What is blocked.** [`open-work.md`](open-work.md)'s *a described foreign call
is not asked whether it threads*, whose last paragraph says a column for this
*is a question and belongs here when somebody asks it*.

**Measured.** `NK2502` asks its question of a call **nothing** describes, which
is [ADR-038](specification/adr/adr-038.md) D7's own wording. A *described*
foreign call is not asked, because no column says whether it threads —
`crates/nikaia/tests/send.rs` holds that silence on purpose so nobody
rediscovers it. `examples/foreign-runtime/crossing` is exactly that shape: a
handle whose description says `crosses = false`, handed to
`hyper_shim::across_a_thread`, which the description names. The build fails —
**against the `.nika` line**, which is [Part III C.1](specification/30-nikaia-tooling.md)'s
rule kept — with `rustc`'s words:

```text
`Rc<String>` cannot be sent between threads safely
```

[ADR-005](specification/adr/adr-005.md) D7 records the *text* as the open half,
and this is it.

**Why it is the owner's.** A column is a claim somebody writes by hand and a
reviewer checks, and *does this function put what it is given on a thread* is
**not visible in a signature**. That is the same difficulty `crosses` has, and
[ADR-123](specification/adr/adr-123.md) D2 answered it with *written by hand and
never inferred*. Whether this language wants a second such claim is a question
about how much a description is trusted to say.

**The options.**

* **A — a column**, `threads`, hand-written like `crosses` and reviewed with the
  rest of the description. `NK2502` then asks a described call too, and the
  refusal is in this compiler's words.
* **B — no column.** The answer stays `rustc`'s `Send` bound on the right line,
  and [C.2](specification/30-nikaia-tooling.md)'s *in the compiler's own words*
  is knowingly not met for this one case. This is today.
* **C — the describer derives it.** ~~Out of reach~~ — **the reason this option
  gave was false**, and finding that out is what added D below. It read:
  *`nikaia describe` reads rustdoc JSON, which carries signatures and not
  bodies*. It does not. [ADR-104](specification/adr/adr-104.md) D4 makes
  rustdoc-JSON the **future** better half and says so outright — *a stable-only
  toolchain ([ADR-001](specification/adr/adr-001.md) D1) is not given up for it,
  so the source parser is what runs today*. `nikaia describe` reads the crate's
  own `.rs` **text**. What is out of reach is narrower and different: it is a
  **signature scraper and not a Rust parser**, by its own module header.
* **D — A, and the describer *proposes*.** The column stays hand-written and the
  tool stops making the author hunt for the answer. Where the scraper sees
  evidence, it writes it as a reviewable note beside the absent column —
  *`spawn_worker`: the parameter `f` is bound `Send + 'static`; does this put it
  on a thread?* — and the person writes `threads`, or does not.

**Why D is the shape rather than a fourth answer.** It is
[ADR-123](specification/adr/adr-123.md) D2's own pattern one column over, and
that one is **built**: `nikaia describe` already writes `crosses = false` for a
type whose fields hold an `Rc` or a raw pointer, `true` where every field is
sendable, and **nothing** where it cannot tell.

**But the license for D2 is soundness, not a good hit rate**, and that is the
line D has to stay on the right side of. A field holding an `Rc` *entails* not
sendable — it is a structural read, not a guess. The indicators here do not
entail:

* **`F: FnOnce() + Send + 'static` on a parameter** is a very good sign and not a
  fact: the bound says the callee *may* send it, which is usually `spawn` and is
  sometimes an API keeping a door open. Written as a **claim** it can refuse a
  correct program ([C.4](specification/30-nikaia-tooling.md)); written as a
  **note** it is the most actionable sentence the tool could produce.
* **`std::thread::spawn`, `tokio::spawn`, `rayon::spawn`, `Sender::send` in the
  body** are the same kind of sign one layer deeper, and they cost more to see:
  the text is in the file the scraper already opens, but knowing *which
  function's body* a line sits in is brace tracking — more than a signature
  scraper does and less than a Rust parser. A step, not a rewrite.
* **A leaf function is the one indicator that must not be built as proposed.**
  *Pure computation, no external calls* would be the ground for `threads =
  false`, and that is the dangerous direction: a false silence, which
  [ADR-010](specification/adr/adr-010.md) D1 calls a vulnerability generator.
  Worse, a signature scraper cannot establish leafness at all — it would need
  the body **and** every callee. **Nothing a signature can show entails
  `false`**, because a function may spawn something it built itself.

**So the asymmetry is the design.** The describer may propose `true`, must
propose `false` never, and stays silent wherever it cannot see — which is the
same *absence is nobody said* the column already rests on.

**What D would take, step by step, and what each step is worth.** The owner's
shape, checked against
[`examples/foreign-runtime/shim`](../examples/foreign-runtime/shim) — which is
the repository's own answer, because it was written to study this exact hole and
has **both** shapes in it side by side:

```rust
across_a_thread<T: Describe + Send + 'static>      → on_one_worker(move || value.describe())
across_a_thread_unchecked<T: Describe + 'static>   → on_one_worker(move || smuggled.describe())
                                                       // the bound is gone: `unsafe impl Send for Smuggled<T>`
on_one_worker<F: FnOnce() -> String + Send + 'static> → tokio::spawn(…)
```

1. **Cargo metadata to find the crate's files.** Yes, and it replaces a guess
   with an answer: the resolved version's actual sources, path and git
   dependencies included. It is a subprocess, and this is a command a person
   runs rather than a build step, so the cost is not the question.
2. **A real parser (`syn`) instead of the scraper.** Yes, and it is the step the
   rest rests on. [ADR-104](specification/adr/adr-104.md) D4 said *the crate's
   sources are parsed* without naming a parser, so nothing is being
   contradicted. Of the scraper's **three** named limits, in its own order:

   * *an item a macro generates is not in the text* — **stays, and not because
     of `syn`.** Expanding a macro needs `rustc -Zunpretty=expanded`, which is
     nightly: measured on this toolchain, *the option `Z` is only accepted on
     the nightly compiler*. So macro-generated items are behind **the same
     [ADR-001](specification/adr/adr-001.md) D1 wall that keeps rustdoc-JSON
     out** — one decision, two consequences, and no parser choice moves either.
     What it hides is narrower than it sounds for *this* tool: a `derive`
     generates `impl`s and `#[tokio::main]` rewrites a body, neither of which is
     the `pub fn` signature a description is made of. A declarative macro that
     generates API surface is the case that bites.
   * *a signature this cannot translate is written `?`* — **narrowed.** A parser
     reads generics, `where` clauses and paths that a line scraper gives up on,
     which is also what step 4 needs in order to see a bound at all.
   * *a `pub` item inside a `mod` block is read as the crate's own* — **closed
     outright**, and the header already says why: *the module path a caller
     writes is a thing only a real parser knows*.
3. **An intra-crate call graph.** Yes — and for a reason sharper than *more
   reach*. In **safe** Rust the signature scan needs no call graph, because the
   bound is not a heuristic there at all: the shim's own doc comment says why —
   *every safe way of reaching another thread carries it … so a foreign crate
   that takes a value across a thread boundary in safe Rust demands it of its
   caller too*. Rust's type system does the propagation and the answer surfaces
   in the signature. **What the call graph is for is the other row**:
   `across_a_thread_unchecked` has no bound, because an `unsafe impl Send` on a
   wrapper took it away — and only following the calls reaches `tokio::spawn`.
   That is the shape the shim says *nothing anywhere complains about*.
4. **The pattern and sink matcher.** Yes, with two things named rather than
   discovered: a per-file **`use` table**, or `use tokio::spawn; spawn(x)` is a
   different string from `tokio::spawn(x)` and the matcher misses it; and the
   knowledge that a sink reached through a call says *this function threads
   something*, never *this function threads your argument* — connecting the two
   is dataflow through a wrapper and a closure capture, which is a third tool.
   So the call graph's output is a **note that names the path**, not a claim.
5. **A crate summary cache.** **Not yet**, and the rule is this project's own:
   [`staging-candidates.md`](staging-candidates.md) — *a staging decision enters
   the compiler only together with a measured crossover; without one the
   complexity is certain and the gain is not*. Two things say the crossover is
   unlikely to be there: `nikaia describe` is a command a person runs when a
   program first reaches into a crate, not a build step; and the committed
   `.contracts` file **is** already the artifact that keeps the answer
   ([ADR-100](specification/adr/adr-100.md)). A second cache with a key of its
   own is the shape [ADR-021](specification/adr/adr-021.md) D5 and D9 were
   written about.

**And one step the list does not have, which may be worth more than three of
them: flag `unsafe impl Send` and `unsafe impl Sync` in the described crate.**
It is one syntactic pattern, it is sound — the item is in the text or it is not
— and it is the single most useful sentence this tool could write about a
foreign crate: *this crate makes a promise the toolchain cannot check*. The shim
is the worked example, and the hole it opens on purpose is invisible to every
other step above.

**What none of it can do, and the note must say so.** `unsafe impl Send` is a
promise a crate makes about its own type. A tool can see that the promise was
made; it cannot see whether it is true. That is the line between *a rule the
toolchain enforces and a rule it inherits* — the shim's own words — and it is
why every output here is a note for a reviewer and the column stays a person's.

**What this page recommends: A with D, and the third value.** The precedent is exact —
`crosses = false` is already a hand-written claim about something a signature
cannot show — and the description is reviewed like code
([ADR-104](specification/adr/adr-104.md)). But it must have **three** values and
not two, for the reason `crosses` has three: the absence of the word is *nobody
said*, never *it does not*. A `threads = false` written by a hopeful hand is a
false **silence**, which is the polarity
[ADR-010](specification/adr/adr-010.md) D1 calls a vulnerability generator, and
the refusal must fire on the claim rather than on its absence.

**And D is what answers the cost of A**, rather than a second thing to build:
the objection to A is that it is *one more line a describer has to get right, on
a surface where getting it wrong is silent*. A note that says **where to look**
does not make the claim safer to get wrong — it makes it less likely to be
skipped, which is the failure mode a hand-written column actually has. The
describer proposes; the person disposes; `NK2502` fires on the claim.

**What it costs if wrong**: A's cost is the line above, unchanged. D's own cost
is a note that cries wolf — a `Send + 'static` bound on an API that never
spawns, read by a reader who then stops reading the notes. That is a reason to
keep the note **specific** (name the parameter and the bound, never *this might
thread*) and to write none at all where the evidence is weaker than a bound; it
is not a reason to skip D, because the alternative is that nobody looks.
