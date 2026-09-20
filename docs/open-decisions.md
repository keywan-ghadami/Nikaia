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

### `eprint`: on the prelude's list, or out of the compiler?

**What is blocked.** `eprint` is a name this compiler keys **bare**, so it needs
no `use` today, and [ADR-154](specification/adr/adr-154.md) D1's list does not
have it: the list writes `println`, `print` and `eprintln`. It joined by being
needed once, which is the direction D4 of that record was written against — *a
name joins the list by a record and never by being needed once.*

**It rode with the `Bytes` entry** this file used to carry, and
[ADR-156](specification/adr/adr-156.md) answered that one without answering
this: the two are one question only in that enforcing a list is what made both
visible.

**The options.**

* **A — the list gains it in a sentence.** One more name that is very hard to
  take back, and a list whose fourth printing function is there because a
  program wanted it.
* **B — the entry leaves the compiler**, and `eprint` is written
  `io::eprint` behind a `use std::io` like everything else outside the list.

**What this page recommends: A**, narrowly. `eprintln` is on the list and
`eprint` is the same function without the newline; a list that has one and
refuses the other is a rule a reader cannot state. If that is not reason enough
to write a record, B is the honest alternative and costs one corpus edit.

**What either direction costs if it is wrong.** A wrong: one name in a prelude,
permanently. B wrong: a program writes `use std::io` to print a line without a
newline while `eprintln` needs nothing, which is exactly the inconsistency A
removes.

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

