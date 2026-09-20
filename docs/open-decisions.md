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

### Does a `for` iterate something whose step can pause?

**What is blocked.** [`open-work.md`](open-work.md) §2.2, which says outright
that what is left of it *waits on a ruling about the `for`, not work*.

**Measured.** `for line in io::lines()` lowers to this:

```rust
for line in io::lines().await {
    let line = line?;
    …
}
```

The **call** awaits — `io::lines` is not `sync`, so opening the stream
suspends. The **step** does not: `Lines::next` is an `Iterator::next` over a
`BufRead`, which blocks the thread it is on for as long as the line takes.
So a `for` over standard input holds its thread for the whole loop, and at
`user_parallelism = yes` that is a thread the pool could have had.

**Why it is the owner's.** It is a question about what a `for` **is** in this
language, not about how to build one. Rust has no stable `Stream`, so a step
that pauses is `while let Some(x) = s.next().await` below — and deciding that a
`for` may mean that is a decision about the construct.

**The options.**

* **A — a `for` may iterate something whose step pauses.** The type says so in
  the ledger, exactly as [ADR-025](specification/adr/adr-025.md) D6's
  `iterates = "throws"` already says a step can **fail**, and the emitter writes
  the awaiting form for it. It needs a stream-shaped trait of `std`'s own,
  because Rust's is unstable — one trait, not a language feature.
* **B — a stream is not a `for`.** `io::lines()` keeps its blocking step, and a
  program that wants to give the thread up writes the loop itself. Cheap, and it
  makes `io::lines`' own doc false: *a stream larger than memory is a `for` over
  this*.
* **C — leave it.** The step blocks, the cost is one thread at `yes`, and
  nothing is written down about it. This is today.

**What this page recommends: A**, and the reason is that the **shape already
exists**. [ADR-025](specification/adr/adr-025.md) D6 put a column on a *type*
that changes how the emitter writes a `for`'s step; a second column on the same
type, answering *can it pause* instead of *can it fail*, is one more of the same
thing rather than a new idea. And a `for` that cannot pause makes
`user_parallelism = yes` a promise the standard library itself breaks.

**Asked and measured: would `tokio`'s `Stream` do for A?** Three answers, and
they are not the same question.

* **`tokio` the runtime is already rejected**, and not by this page.
  [ADR-055](specification/adr/adr-055.md) §5 says binding it *"brings an
  executor and an I/O layer, and [ADR-038](specification/adr/adr-038.md) built
  the second one already. Two event loops in one process is the state D7 treats
  as a hazard."* Nothing in the `for` question reopens that.
* **`tokio_stream::Stream` is not tokio's own trait.** It is a re-export of
  `futures_core::Stream` — so the dependency actually under discussion is
  `futures-core`: a trait definition, `no_std`-friendly, no executor. A far
  smaller thing than the runtime, and still the wrong place for it, below.
* **A needs no trait on the day it is built.** Of `Seq[T]`'s six producers in
  [`std.contracts`](../crates/nikaia-std/std.contracts) — `io::lines`,
  `HashMap::keys`, `HashMap::values`, `String::chars`, `Vec::drain`,
  `HashMap::drain` — five are marked `sync` and **exactly one** can ever pause.
  The emitted `while let Some(x) = s.next().await` wants an inherent
  `async fn next` on one concrete type, not a trait over many. A trait is owed
  when a second pausing producer exists.

**Where A's cost really lands** is `Seq[T]`'s consumers — `collect`, `count`,
`nth`, `join`, `map` and `filter` — which are `Iterator`'s below and need
pausing twins. That work is the same whichever trait sits on top, and
`futures-core` does not shorten it; `futures::StreamExt` would, at the price of
`Pin`/`Poll` in our signatures and an [ADR-119](specification/adr/adr-119.md)
D2 `no_std` target we no longer control.

So if A is chosen: a minimal stream-shaped trait of `std`'s own, as the option
already says — the same move `Joined` made in
[ADR-170](specification/adr/adr-170.md). `futures_core::Stream` earns its place
only **at a foreign boundary**, where a described crate hands us a stream and
its trait is the lingua franca, never in the core.

**What it costs if wrong**: a trait in `std` that a future Rust may make
redundant. That is the smaller risk of the two — B's cost is a page that says
one thing and a library that does another, which is the state
`docs/README.md` §1 calls a defect in its own right.

### Does a **described** foreign function say whether it puts its argument on a thread?

**What is blocked.** [`open-work.md`](open-work.md) §2.31, whose last paragraph
says a column for this *is a question and belongs here when somebody asks it*.

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
* **C — the describer derives it.** Out of reach and worth saying so:
  `nikaia describe` reads rustdoc JSON, which carries signatures and not bodies,
  and threading is a fact about a body.

**What this page recommends: A, with the third value.** The precedent is exact —
`crosses = false` is already a hand-written claim about something a signature
cannot show — and the description is reviewed like code
([ADR-104](specification/adr/adr-104.md)). But it must have **three** values and
not two, for the reason `crosses` has three: the absence of the word is *nobody
said*, never *it does not*. A `threads = false` written by a hopeful hand is a
false **silence**, which is the polarity
[ADR-010](specification/adr/adr-010.md) D1 calls a vulnerability generator, and
the refusal must fire on the claim rather than on its absence.

**What it costs if wrong**: one more line a describer has to get right, on a
surface where getting it wrong is silent. That is the argument for B, and it is
a real one — which is why the recommendation carries the three-value shape
rather than the column alone.
