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

### Where does an error's `secondary` list live when the channel has no envelope?

**What is blocked.** [ADR-115](specification/adr/adr-115.md) entire —
`docs/open-work.md` §2.25. Its D1 says *every error value has `secondary`*, a
list of the further failures that joined it, and that is what an `overlap`'s
later failing branches become (D2) and what a cleanup error attaches to (D3).
The list is read in a handler (`throw LoadFailed(error, error.secondary)`, D4)
and printed indented by a log and by `nikaia explain`.

**Nothing about the list was hard until the channel got a type.** When the
record was written, every failure travelled in one box and `Raised` is this
compiler's own envelope with room for anything. Four records later the channel
is one of four things, and two of them have nowhere to put a list:

| the channel | what carries the failure | room for a list |
| :--- | :--- | :--- |
| the box | `Raised` — this compiler's envelope | **yes** |
| a type this program declares | `Thrown<E>` — the author's value plus a `Site` | **yes**, on the `Site` |
| a library's type ([159](specification/adr/adr-159.md) D2) | the value itself, **bare** | **no** |
| a set of two ([160](specification/adr/adr-160.md) D1) | a generated sum, per member as above | **only where the member has one** |

And the two without room are the common ones. ADR-115's own context is *three
loads, two of them failing on the same outage* — which is three file reads, and
a function that only reads files has `io::IoError` as its whole set, travelling
bare.

**Why it is the owner's.** [ADR-159](specification/adr/adr-159.md) D2 chose bare
deliberately: *there is no `throw` in this program to have a site*, so a `catch`
binds what the source names and propagation is a plain `?`. An `overlap` that
combines failures is the first time the **language** has something to attach to
a failure it did not raise. Whether that reopens D2 is a question about what an
error is, not about how to build one.

**The options.**

* **A — a bare channel gains an envelope where the body joins.** Where a
  function's body holds an `overlap` that can combine, a library's error travels
  in `Thrown<E>` after all. It is invisible in the source —
  [ADR-157](specification/adr/adr-157.md) D2 hands a handler the error with the
  envelope opened once, so `match error { … }` reads the same either way — but
  the channel then depends on the body rather than on the set, which is a new
  kind of fact in the ledger.
* **B — the list lives only where an envelope does**, and a block whose channel
  is bare keeps dropping the later failures. D1 becomes *every error the
  language raised carries a list*, and ADR-115's own worked case is the one it
  does not cover.
* **C — the list is the block's, not the error's.** `combine<n>` answers
  `Err(first)` and puts the rest somewhere the handler reaches by asking the
  **block** rather than the error. Nothing in Part I says a block can be asked
  anything, so it is a construct as well as a decision.

**What this page recommends: A**, and the reason is that B's exception is the
example the record was written for. A costs one derived fact — *this body joins,
so its failures are enveloped* — which the ledger already derives harder things
than; and it costs nothing in the source, because the handler was already handed
the error rather than the envelope. What it would cost if wrong is a channel
that changes when an unrelated `overlap` is added to a body, which is a real
surprise and the thing to weigh.

**What is not blocked by this**, and is already built: the block **combines**
rather than returning at the first `Err`
([ADR-164](specification/adr/adr-164.md) D1), which is where the list goes
whichever answer this gets.

### A postfix unwrap on a `T?`: does the language have one?

**What is blocked.** [ADR-018](specification/adr/adr-018.md) D3 writes
`lookup(a.query("id")??)` — a **postfix** `??`, which unwraps the nullable
rather than falling back. Part I 3.5 defines `??` as null coalescing, `a ?? b`,
and nothing else; there is no postfix form in that section, in the parser, or
anywhere the specification states a rule. A reader who copies the line gets a
parse error with nothing to look up, because the section it would be defined in
does not mention it. `docs/open-work.md` §3.2 carried the measurement.

**It is one example, and Part III 17.1's copy has already been rewritten** to
`?? "world"` and `?? ""`. An ADR is written once, so ADR-018's stands until
something answers this.

**The options.**

* **A — the language gains a postfix unwrap.** Part I 3.5 gains it, the parser
  follows, and it needs a name for what it does when the value **is** null:
  an abort, which is Part III A.2's territory.
* **B — it does not, and the example is rewritten** against what Part I 3.5
  already has.

**What this page recommends: B**, and it recommends it with a program rather
than with an argument. The three things a postfix unwrap is reached for all have
a spelling already:

* a **fallback value** — `a ?? b` (Part I 3.5);
* a **jump** — `a ?? throw NotFound` or `a ?? return r`, since
  [ADR-138](specification/adr/adr-138.md) D1 made the four jumps expressions;
* reaching a **member** through it — `a?.b`
  ([ADR-052](specification/adr/adr-052.md) D7).

What is left over is *abort where it is null*, and that is the one thing Part I
2.3 is built to make a program say out loud: a `T?` is a type of its own, and
[ADR-066](specification/adr/adr-066.md) D6 refuses a plain `.` on one. A postfix
`??` would be a one-character way to take that back.

**And the replacement is shorter than the example it replaces**, which is the
argument that does not need a rule. ADR-018 D3 asks `query` twice and then
unwraps:

```nika
if a.query("id").is_none() {
    http::Response(status: 400, body: "id is required")
} else {
    lookup(a.query("id")??)
}
```

What the language has today is one binding, and it is a program — measured, not
sketched:

```nika
let id = a.query("id") ?? return http::Response(status: 400, body: "id is required")
lookup(id)
```

**What either direction costs if it is wrong.**

* **B wrong** — a postfix unwrap was worth having: a program writes
  `x ?? throw NotFound` where one character would have done. That is a few words
  for a decision the reader can see, which is the trade Part I 2.3 already made.
* **A wrong**: the language gains an operator whose failure mode is an abort
  written as punctuation, in a language whose `T?` exists so that the null case
  is answered rather than deferred — and an operator is very hard to take back.

## A page with nothing on it
only means no question has been *found* yet, and
the way they are found is by building. The next one belongs here the moment
something comes to rest on it, in the shape this page asks for above: what is
blocked, the options, a recommendation, and what either direction costs if it is
wrong.

---

This file is a notes page: nothing here is normative. A decision taken from it is
written down in [`specification/adr/`](specification/adr).

