# Open decisions — the questions that need the owner

## The shape this page is for

The entries here are written to be *answerable* — what is blocked, the options, a
recommendation, and what either direction costs if it is wrong.

An answer is an [ADR](specification/adr/), and
the moment a question is answered its entry leaves this file rather than
staying with a note on it. What is merely **unbuilt** is in
[`open-work.md`](open-work.md). Each entry says what the
question is, why it is the owner's, and what this file recommends.

**What is here is what has been asked**, which is not the same as what is
unsettled: an entry is here because a piece of work was blocked by it and somebody
noticed, so an empty page means nothing is blocked that anyone has written down.
The way to refill it is the paragraph above — the moment a piece of work is
blocked by a question, the question comes here in that shape.

## Open

### A container of structs holding views that drops entries in a loop

**What is blocked.** `NK2304` refuses a program that keeps structs holding
views of text read inside a loop, in a container that also drops entries — a
parser that reads many files into a list of records and prunes it as it goes:

```nika
struct Record { name: ref String, size: i64 }

fn main() throws {
    let mut kept: Vec[Record] = Vec()
    for path in paths() {
        let text = fs::read_to_string(path, fs::Root::Anywhere)
        kept.push(Record { name: text.trim(), size: text.len() as i64 })
        if kept.len() > 100 { kept.remove(0) }
    }
}
```

For **text** alone this is built ([ADR-209](specification/adr/adr-209.md) D4):
each view carries its own handle on its buffer (`tether::Held`), so a buffer is
freed when its last view leaves. A struct's view fields have no such handle.
Refusing it is correct today; the question is whether the language should hold
it.

**Options.**

1. **Keep the refusal.** The way out stays `.clone()` on the view (a copy per
   field), or keeping text views in the container rather than structs.
2. **A per-view handle in every view field**: a struct used this way is lowered
   a second time with `Held` fields. Reads work through `Held`'s deref, but
   every method and construction site of the struct needs the second form too.
3. **One handle per element** (ADR-209 D3's packed handle, generalised): the
   container holds the struct beside a handle on its buffer, and every read of
   an element goes through `.get()`, which hands out the struct over a lifetime
   no longer than the read. The struct is lowered once; the emitter's
   *shortened* lifetime for tethered task bindings is the same mechanism.

**Recommendation: 3.** One lowering of the struct, one handle per element, and
the read path already exists for tasks. It frees a buffer when its last element
leaves, which is what D4 does for text.

**What it costs if wrong.**

* **1:** every program of this shape pays an allocation per view field, in a
  language whose promise is that a copy is written and never needed for safety.
* **2:** each such struct's code is emitted twice, and a method written for one
  form must be written for the other.
* **3:** the emitted Rust extends a lifetime behind `unsafe`, as `Held` does.
  A read path that forgets `.get()` is a `rustc` error rather than unsoundness,
  as long as the element's inner struct is reachable only through `.get()`.
