# Does `from(f)` generalise? The worked cases for `throws` and `touches`

[ADR-029](specification/adr/adr-029.md) D3 added a fourth `sync` state to the
ledger, `sync = "from(f)"`, and rested it on one sentence:

> A caller reads it as "this call adds no pausing of its own." That is sound for
> exactly one reason: the lambda runs **during** the call, so its body is part of
> the function that writes it, and that function's own analysis has already
> counted its calls. `from` adds nothing because there is nothing left to add.

The question this page answers: does that argument also hold for `throws`
([ADR-023](specification/adr/adr-023.md) D1) and for `touches`
([ADR-033](specification/adr/adr-033.md) D2), so that "immediate function types
for user code" is **one** inference rule rather than three?

Nothing here is normative. The answer is one sentence per effect in ADR-029 D3;
this is the programs, the ledgers they produced and the reports they printed.

---

## 1. What the argument actually requires

Read closely, D3's sentence has **two** premises, and only the first one is the
one D4 bounds:

1. **Immediacy.** The lambda runs before the call returns, so its body is
   lexically inside the function that writes the call. D4 is this premise, and
   `a_detached_lambda_may_not_use_from` is how `std` is held to it.
2. **Scope match.** The analysis that consumes the contract asks a question
   whose scope is *the whole function body*. Only then does "its calls are
   already counted" mean they are counted **where the contract is being read**.

Premise 2 is never stated in D3, because for `sync` it is invisible: `sync` *is*
a property of a function, so counting an effect anywhere in the body is counting
it in the right place. It is the premise that decides the other two effects.

Mechanically, premise 1 is one match arm. `contracts::sync`'s `visit_expr_blocks`
descends into `Expr::Closure { body }` wherever a closure is an argument, and
skips `spawn`:

```rust
Expr::Block(block) | Expr::Seq(block) | Expr::Closure { body: block, .. } => f(block),
```

`contracts::throws` imports that same walk. `contracts::order` does not have it.
That one difference is the whole answer.

---

## 2. `sync` — the control

The program is ADR-029 §3's own, re-run here so the other two have something to
be compared against.

```nika
fn ordne(xs: Vec[i64]) -> i64 { xs.sort_by_key fn { a } return 1 }
fn im_lock(xs: Vec[i64]) -> i64 sync { return ordne(xs) }
```

`nikaia.contracts`:

```toml
[fn."im_lock"]
sync = true
signature = "(xs: Vec[i64]) -> i64"

[fn."ordne"]
sync = "inferred"
signature = "(xs: Vec[i64]) -> i64"
```

`sort_by_key` says `sync = "from(f)"`, `ordne` earns `sync = "inferred"` through
it, and nothing is reported. Now the negative half, which is what proves the
counting rather than the permission:

```nika
use std::io

fn ordne(xs: Vec[i64]) -> i64 throws {
    xs.sort_by_key fn { io::read_to_string().len() }
    return 1
}
```

```toml
[fn."ordne"]
throws = ["?"]
signature = "(xs: Vec[i64]) -> i64"
```

No `sync` key, which is `Sync::No`: the pausing call **inside the lambda** took
the claim away. `from(f)` did not launder it, because the enclosing function's
own walk reached into the lambda and found `io::read_to_string`. That is D3's
sentence, executing.

---

## 3. `throws` — the same walk, and nothing left to buy

```nika
enum LeereZeile { Leer }

fn pruefe(n: &i64) -> i64 throws {
    if n == 0 { throw LeereZeile::Leer }
    return 1
}

fn sortiere(xs: Vec[i64]) -> i64 throws {
    xs.sort_by_key fn { pruefe(a) }
    return 1
}

fn ohne_lambda(xs: Vec[i64]) -> i64 throws {
    xs.sort_by_key fn { a }
    return 1
}
```

```toml
[fn."ohne_lambda"]
sync = "inferred"
throws = ["?"]
signature = "(xs: Vec[i64]) -> i64"

[fn."pruefe"]
sync = "inferred"
throws = ["LeereZeile"]
signature = "(n: &i64) -> i64"

[fn."sortiere"]
sync = "inferred"
throws = ["?", "LeereZeile"]
signature = "(xs: Vec[i64]) -> i64"
```

`LeereZeile` is in `sortiere`'s set, and `ohne_lambda` — the same call with a
lambda that cannot fail — has only the `"?"`. So the named error came from the
lambda's body and from nowhere else: the caller's own analysis already counts
what the lambda throws, at the point of the call. Both premises hold, for the
same reason they hold for `sync`, and `throws` is a fact about a function in
exactly the way `sync` is.

The `"?"` is the second, independent reason `throws` has nothing to gain.
`contracts::throws` answers **every** method call with `"?"`:

```rust
// A method call, or a call nobody can name. Either can fail, and
// treating "I cannot see it" as "it does not fail" is the one
// direction ADR-010 D1 calls a vulnerability generator.
Some(Reached::Method) | Some(Reached::Opaque(_)) => {
    into.direct.insert(UNNAMED_ERROR.to_string());
}
```

So a higher-order call is already as pessimistic as the column can be. A
`throws = "from(f)"` read as "adds nothing" would make it **less** pessimistic —
it would be the one change that could remove the `"?"` — which is the direction
[ADR-010](specification/adr/adr-010.md) D1 forbids. A `throws` column already
behaves exactly as `from` would want it to and has no key to add.

### The `and_modify` chain, for the precision it loses

[ADR-031](specification/adr/adr-031.md)'s live example, with a failing `add`:

```nika
use std::collections::HashMap

enum ZuVoll { Voll }

pub struct Stats { n: i64 }

impl Stats {
    fn add(&self, v: i64) throws {
        if self.n > 100 { throw ZuVoll::Voll }
    }
}

fn record(m: HashMap[&str, Stats], v: i64) throws {
    m.entry("x").and_modify fn { a.add(v) }
}
```

```toml
[fn."Stats::add"]
sync = "inferred"
throws = ["ZuVoll"]
signature = "(&Stats, v: i64)"

[fn."record"]
sync = "inferred"
throws = ["?"]
signature = "(m: HashMap[&str, Stats], v: i64)"
```

`record` gets `["?"]` and not `["ZuVoll"]`. The lambda **was** walked — that is
where the `"?"` came from — but `a.add(v)` is a *method* call, and
`contracts::throws` does not ask the type checker what a method resolves to the
way `contracts::sync`'s `reach_of` does with ADR-028's `MethodCalls`. So the
chain `HashMap[&str, Stats]` → `fn(&$V)` → `a.add(v)` → `Stats::add` is followed
for `sync` and not for `throws`.

That is a **precision** gap and not a soundness one: `"?"` is the widest claim
the column can make, and a caller reads it as "it fails". It is also orthogonal
to `from` — handing `throws::infer` the same `resolved` map `sync::infer` already
gets would close it, and `from` would still have nothing to add afterwards.

---

## 4. `touches` — where the argument breaks, and it is the polarity

Premise 1 holds here too: `and_modify`'s lambda runs before the call returns.
**Premise 2 does not.** `contracts::order` does not ask "what does this function
reach"; it asks "what does *this statement* reach", because the rule it serves is
about two adjacent statements. A touch set that is complete for the function and
incomplete for the statement is worth nothing to it.

And nothing walks a lambda for an effect. `contracts::order`'s `walk` reduces a
statement's value to the calls it performs, and a closure is not one of the
shapes it reads — so the lambda's body is never reached, and the statement is
refused instead. The file is already half-aware of the asymmetry: `names_in`,
which finds data dependencies, *does* descend into `Expr::Closure`. The names
count; the effects do not.

### The contrast that settles it

A `catch` handler is the other piece of code that runs during a statement, and
ADR-033 §8.3 made its effects part of the statement's touch set. So the two
cases can be put side by side. With `--overlaps --user-parallelism yes`:

```nika
use std::fs

fn main() {
    fs::write("a.txt", "x") catch { println("gescheitert") }
    println("danach")
}
```

```text
main:
    in order  fs::write + println / println - both reach stdout, and one writes it
```

The handler's `println` is **named as a callee of the first statement** —
`fs::write + println` — and `stdout` is what the pair meets on. Counted.

```nika
fn lauf(xs: Vec[i64]) {
    xs.sort_by_key fn { println("aus dem lambda") return a }
    println("danach")
}
```

```text
lauf:
    in order  println - nothing says what `sort_by_key` reaches, so it reaches everything
```

The lambda's `println` appears nowhere. The pair is refused, correctly, but for
**ignorance about `sort_by_key`** and not because anybody counted the write. So:

> Where a handler's effect is *counted into the statement*, a lambda's effect is
> *stepped around*. "Already counted at the call site" is true for `sync` and for
> `throws` and false here.

### The counterfactual, measured

Run the same program against `std`'s ledger with one line added —
`Vec::sort_by_key` claiming `touches = []`, which is the permissive entry a
`from`-style reading would amount to:

```text
lauf:
    in order  println - `xs` is not a literal, and only literals are sent to another thread
```

Still refused, and **not** because of the lambda. Today a trailing lambda on a
method call is refused two or three times over, and the other refusals are the
ones the records call liftable:

| refusal | what would lift it |
| :--- | :--- |
| nothing says what `sort_by_key` reaches | a `touches` line — the thing `from` would be |
| `xs` is not a literal | ADR-033 D9's table: "an implementation limit, not a language question" |
| what `Vec::new` hands back may not cross a thread | a `crosses` line for the type (ADR-005 §1 Group B) |

Each of those is a gap somebody is meant to close. Close all three and the only
thing left between a permissive higher-order entry and an overlap is whether the
analysis reads the lambda. Before this change nothing in the repository said that
it must not; the refusal was a consequence of `walk`'s `_` arm. It is a named arm
now, with the reason written where somebody will be standing, and a test —
`a_lambdas_own_effects_are_not_in_the_statements_touch_set` in
`crates/nikaia/tests/ordering.rs` — that holds it. With a bare lambda statement,
where no other limit fires first, the arm is visible:

```text
lauf:
    in order  println - one of them holds a lambda, whose body this analysis does not read
```

### Why this is a soundness question and not a speed one

`touches` is fail-closed by [ADR-033](specification/adr/adr-033.md) D4 — an
unknown touch set is *everything* — and the reason is
[ADR-010](specification/adr/adr-010.md) D1's: an analysis that fails open is a
vulnerability generator. `sync`'s `from` is a **permissive** reading, and that is
affordable there because the permission it grants is paid for by a count that
really happened somewhere else. Here there is no count, so the permission is
unfunded, and what it buys is not speed:

```nika
xs.sort_by_key fn { println("aus dem lambda") return a }
println("danach")
```

Two touch sets that meet on nothing may be reordered. If `sort_by_key` said
"adds nothing" and the analysis believed it, these two lines would become a
group, and the two lines of output would interleave differently from run to run.
That is ADR-033 D1 broken by a resource nobody checked — the identical shape as
the `stdout`/`stderr` mistake ADR-033 §6 caught (`println` / `eprintln` /
`println` interleaving under `2>&1`) and as the `catch` hole §8.3 closed. **A
soundness hole here changes what a program prints.**

So `touches` does not want `from`'s reading. What it would want, if a Nikaia
function could ever take a lambda, is the **opposite** instruction: *add* what
the lambda reaches to this statement, and where the lambda cannot be read, the
statement reaches everything. That is the polarity `walk_block` already gives a
`catch` handler, and it is a union rather than an omission.

Which also means the spelling must not be reused. One word meaning "skip, it is
counted elsewhere" in the `sync` column and "descend, nothing else counts it" in
the `touches` column is two rules wearing one name, and the `stdoutt` lesson of
`contracts::touch::KINDS` is what a vocabulary whose mistakes are permissions
costs. If the column ever needs it, it needs a word that says descend.

---

## 5. The answer

**One rule, not three — and `touches` is not the third.**

| effect | does the caller's analysis already count the lambda, at the call? | what `from` needs |
| :--- | :--- | :--- |
| `sync` | **yes**, by `visit_expr_blocks`' `Expr::Closure` arm | the rule ADR-029 D3 built |
| `throws` | **yes**, by the same walk — and a method call is already `"?"` | **nothing**, and no key either |
| `touches` | **no** — the consumer asks per *statement*, and no walk reads a lambda for an effect | not `from`'s reading; a *union* instruction under a different word |

The rule, stated so that it says what it covers:

> A ledger may say "the lambda decides, and this call adds nothing of its own"
> for an effect that is (a) carried by an **immediate** parameter, and (b)
> consumed as a fact about a **whole function**. Where the consumer needs the
> effect attributed to a smaller scope than the function — a statement, a pair —
> "already counted" is a claim about the wrong place, and the entry must make the
> caller *add* the lambda's effect instead of omitting it.

So a function type for user code costs **one** inference rule, the one that
exists. `throws` rides on the same walk and needs no entry. `touches` needs no
entry either, as long as nothing ever reads a higher-order call permissively —
which is now a named refusal rather than a `_` arm.

---

## 6. Incidental findings

* **A call that can fail, in a function that does not declare `throws`, is not
  reported by Nikaia.** `fn ruft() -> i64 { return pruefe(0) }` where `pruefe`
  is `throws` lowers without a diagnostic and records `ruft` as unable to fail;
  the emitted Rust is `fn ruft() -> i64 { pruefe(0) }`, which `rustc` rejects.
  `NK2601` and `NK2701` are both about *implicit* calls
  ([ADR-025](specification/adr/adr-025.md) D1), and the explicit case has no
  code. Nothing to do with lambdas — the same program without one behaves the
  same way — but it is the reason §3's programs all declare `throws`.
* **`contracts::throws` does not use ADR-028's method resolution**, where
  `contracts::sync` does. §3's `and_modify` chain is what shows it: `ZuVoll`
  becomes `"?"`. Fail-closed, so not urgent, and a one-argument change.
* **Two leftover mentions of the withdrawn path** outside the files
  `docs/README.md` allows them in: `docs/foreign-runtime.md` (line 42) calls
  `rustc 1.94.0-nightly` "the toolchain this repository named", which it no
  longer is, and [ADR-036](specification/adr/adr-036.md) §2 names the same
  nightly as its measurement machine. Both are records of a measurement rather
  than instructions to install anything, which is why neither is changed here.
