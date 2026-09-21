# A fixed map's lookup: `match` or a perfect hash, and where they cross

**Why this file exists.** [ADR-079](specification/adr/adr-079.md) D3 decided that
a map built at build time crosses as a **fixed** map and that *how it is looked
up is the compiler's*. It named no threshold, and
[`staging-candidates.md`](staging-candidates.md) ends on the rule that says one
is needed: *a staging decision enters the compiler only together with a measured
crossover; without one the complexity is certain and the gain is not.* This is
that measurement.

## 1. The three contenders, and which two were already measured

[ADR-073](specification/adr/adr-073.md) §3 measured **two**, on 200 keys:

```
HashMap nachschlagen                  18.65  17.94  18.60  17.85  17.84   ns
match nachschlagen                     9.05   8.95   9.05   9.01   9.06   ns
```

and [ADR-079](specification/adr/adr-079.md) §3 reads that as *"200 keys favour a
`match` 9.0 against 18.0 and a million do not"*. Both are true and neither
settles this page's question, because the third contender was never in the room:
a **perfect hash**, which is what a compiler that owns the key set can build and
what a `HashMap` is not.

## 2. What was measured

A generated Rust program, `-O`, best of five runs of two million lookups each,
for key sets of 8 to 256 short ASCII words of the shape a real table has —
HTTP verbs, SQL keywords, MIME types.

* **`match`** — the arms written out, which is what the emitter would produce.
* **perfect hash** — the `phf` crate's algorithm (CHD): buckets, a displacement
  pair per bucket, one table of `N` slots. The lookup is **one hash, one
  displacement read, one slot read, one string compare**, and the hash is FNV-1a
  rather than SipHash, because a compiler that owns its keys may choose the
  faster one — which is [ADR-010](specification/adr/adr-010.md)'s reasoning one
  construct over.

Both the **hit** and the **miss** path, because a table of keywords is asked *is
this a keyword* far more often than *which keyword is it*, and the two paths are
not the same shape: a `match` rules a miss out by length, a perfect hash pays one
hash either way.

## 3. The trap, and it is this file's most useful line

The first run said the crossover was between **128 and 256** — that a `match`
wins up to a hundred-odd keys. It was wrong, and the reason is the benchmark
rather than the code: the probes cycled through the keys **in order**, so the
branch predictor learned the *benchmark* instead of measuring the *lookup*.

With a pseudo-random probe order the same program says something else entirely.
`staging-candidates.md` §5 warns about picking a benchmark that does not exercise
the change; this is the same mistake one level down, and only a fairness check
found it. **A harness baseline is what says it is not still happening**: a
function that returns the key's length costs **1.78 ns** through the same loop,
so what the rows below measure is the lookup.

## 4. The numbers

Nanoseconds per lookup, best of five, shuffled probe order, misses drawn from the
keys' own alphabet and length range so that a length check cannot reject them for
free.

| N | `match` hit | PHF hit | | `match` miss | PHF miss | |
| ---: | ---: | ---: | :--- | ---: | ---: | :--- |
| **8** | **5.09** | 12.33 | `match` **2.4×** | **2.76** | 11.75 | `match` **4.3×** |
| **16** | 14.62 | **12.07** | PHF 1.21× | 13.72 | **12.93** | PHF 1.06× |
| 24 | 15.44 | **14.59** | PHF 1.06× | 14.82 | **10.35** | PHF 1.43× |
| 32 | 15.02 | **13.33** | PHF 1.13× | 15.81 | **11.60** | PHF 1.36× |
| 48 | 16.34 | **13.69** | PHF 1.19× | 16.17 | **12.12** | PHF 1.33× |
| 64 | 17.50 | **13.79** | PHF 1.27× | 18.22 | **13.02** | PHF 1.40× |
| 128 | 19.93 | **13.78** | PHF 1.45× | 23.58 | **11.34** | PHF 2.08× |
| 256 | 25.40 | **13.76** | PHF 1.85× | 34.98 | **16.63** | PHF 2.10× |

**The perfect hash is flat** — 12 to 14 ns whatever `N` is, which is what one
hash and one compare should look like. **The `match` climbs**, from 5 ns at eight
keys to 25 at two hundred and fifty-six, and faster on the miss path where it has
more to rule out.

## 5. What it concludes

**The crossover is between 8 and 16 keys.** At eight the `match` is not merely
ahead, it is ahead by 2.4× on a hit and 4.3× on a miss — a handful of keys is
ruled out by length and one comparison, and no hash can beat that. At sixteen the
perfect hash is already in front on both paths, and it never gives the lead back.

So a threshold **around 16** is what the numbers say, and a round **20** is a
defensible place to put it: it sits just above the crossing, on the side where
being wrong costs least — a `match` of twenty keys loses about 15 % on a hit,
where a perfect hash of eight loses 140 %.

**What this does not decide** is what a program *writes*. There is no map literal
in this language and no build-time map value: a program builds a map with
`collections::HashMap()` and `insert`, whose bodies are Rust, so the build cannot
see the contents. [ADR-079](specification/adr/adr-079.md) §3 calls the fixed map
*"a type in `std` and not a language rule"*, and nobody has written it. That is
in [`open-decisions.md`](open-decisions.md).

## 6. Reproducing it

The generator and the benchmark are not in the tree: they are ninety lines of
Python that write one Rust file, and the numbers above are what it printed. What
is worth keeping is the method — a shuffled probe order, a baseline through the
same loop, both paths, and a key set shaped like a real one.
