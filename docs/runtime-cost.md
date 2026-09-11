# What an operation costs on a runtime that is already running

**Date:** September 11, 2026
**Status:** measured; the numbers it produced are [ADR-038](specification/adr/adr-038.md) §4.3's evidence
**Related:** [ADR-038](specification/adr/adr-038.md) D3 (the I/O split) and D4 (the runtime starts
before `main`), [ADR-033](specification/adr/adr-033.md) §8.4 (the 46 µs per-pair wake-up this is
measured against) and §8.5 (the prediction this settles),
[ADR-037](specification/adr/adr-037.md) D2 (whose thread the I/O thread is)

[ADR-033](specification/adr/adr-033.md) §8.5 wrote down a prediction and said in the same
sentence that there was nothing to time: *"the sidecar thread is already running; two overlapped
reads are two messages, not two thread starts… **This is a prediction, not a measurement** —
there is no event loop, no sidecar and no async lowering yet."* Two of those three now exist.
This is the measurement, its method, its machine, its spread, and the two things it found that
were not predicted.

**The conclusion, first.** The prediction is right, and it is right for a narrower reason than
it gave. A pair of operations on the pre-started runtime costs **nothing measurable** — but only
on the completion path. The blocking fallback, on the same pre-started threads, pays **~38 µs a
pair**. "The runtime is already running" removes thread *creation*; it does not remove thread
*wake-up*, which is what §8.4 said the floor was. What removes the wake-up is that `io_uring`
involves no thread at all — which is
[ADR-038](specification/adr/adr-038.md) D3's argument, now with a number under it.

---

## 1. The machine and the method

Intel Xeon @ 2.10 GHz, **4 vCPU**, 15 GB RAM, Linux 6.18.44 x86_64 — a shared virtual machine
with no `cpufreq` governor exposed, and a box that other work was using throughout: the load
average was **1.5 to 2.1** before and after every block, and it is printed by the script beside
every table rather than assumed away. `rustc 1.94.0-nightly (8d670b93d 2025-12-31)`, the
`nightly-2026-01-01` of `rust-toolchain.toml`. `--release`. `io_uring` is available on this
kernel and `io-method = "auto"` chose it, which the binary prints before it measures anything.

Same machine class as [ADR-033](specification/adr/adr-033.md) §8.4's "4 vCPU container", so the
46 µs it measured and the 59 µs measured here are comparable rather than merely both being
numbers.

**The program** is `benches/overlap/src/bin/runtime.rs`, run by
`benches/overlap/runtime.sh`. It reads the same two files three ways and times each:

| shape | what it is |
| :--- | :--- |
| `seq` | `fs::read(a)` then `fs::read(b)` — the sequential baseline |
| `join` | `task::both(\|\| fs::read(a), \|\| fs::read(b))`, which is `rayon::join` — §8.4's vehicle |
| `both` | `fs::read_both(a, b)` — both in flight on the pre-started runtime |

**All three go through the same `std::fs`, and therefore through the same mechanism.** That is
what makes the subtraction meaningful: `join − seq` and `both − seq` are the cost of the
*vehicle* and of nothing else, because the reads underneath them are the same reads. The absolute
columns are not compared across mechanisms — see §4 for why that would be wrong.

**The runtime is started once**, by `rt::start`, exactly as the `fn main` the emitter writes
starts it. Nothing timed below pays for starting it; that is the whole of what D4 claims.

**Warm-up** is 64 pairs of each shape before the first repeat, so no measurement includes a cold
page cache, a cold `rayon` pool or the first `io_uring_enter`.

**Repeats.** Nine, each 20 000 pairs (4 000 at 256 KiB, 1 000 at 1 MiB). Every repeat is printed
by the binary; the mean, the range and the standard deviation below are over those nine. Files
are `base64` of `/dev/urandom` truncated to the exact size — incompressible, and warm in the page
cache after the warm-up.

**The two mechanisms are both measured, not one measured and one assumed.** The script runs the
whole table twice: once with `io-method = "auto"` (which chose completion here) and once with
`io-method = "blocking"` pinned, which is the route an older kernel or a sandbox that forbids the
syscalls would take. `io-workers = 2` in both, so the fallback has a thread to overlap a pair
*with*; with one worker a pair on the fallback runs one after the other, which is the honest
answer for a machine that has neither a completion queue nor a thread to spare.

---

## 2. What a vehicle adds per pair

The number that answers D4. Mean over nine repeats, with the range and standard deviation.

### Completion (`io-method = auto`, which chose `io_uring`)

| bytes/file | `seq` | `join − seq` | `both − seq` |
| ---: | ---: | ---: | ---: |
| 5 | 3.34 µs | **+59.17** [57.81, 61.53] sd 1.36 | **−0.25** [−1.29, 0.63] sd 0.65 |
| 4 096 | 3.75 µs | **+60.28** [58.71, 63.29] sd 1.74 | **−0.48** [−1.23, 0.37] sd 0.63 |
| 65 536 | 10.39 µs | +67.26 [58.62, 75.43] sd 4.64 | −0.61 [−2.17, 0.86] sd 1.22 |
| 262 144 | 40.71 µs | +95.85 [82.80, 108.38] sd 7.23 | −0.86 [−7.25, 9.11] sd 4.58 |
| 1 048 576 | 400.66 µs | +291.00 [245.12, 349.67] sd 38.18 | +31.22 [−87.88, 109.02] sd 61.89 |

### Blocking fallback (`io-method = blocking`, pinned)

| bytes/file | `seq` | `join − seq` | `both − seq` |
| ---: | ---: | ---: | ---: |
| 5 | 2.77 µs | +51.91 [43.65, 55.47] sd 3.83 | **+37.87** [36.84, 39.53] sd 0.89 |
| 4 096 | 2.93 µs | +48.15 [41.25, 51.37] sd 2.93 | **+37.72** [31.96, 39.66] sd 2.33 |
| 65 536 | 8.02 µs | +52.66 [49.88, 55.73] sd 1.64 | +37.32 [35.72, 38.53] sd 1.11 |
| 262 144 | 139.91 µs | −56.79 [−71.31, −30.95] sd 12.52 | **−77.90** [−91.70, −55.52] sd 10.90 |
| 1 048 576 | 893.52 µs | −669.46 [−777.54, −592.99] sd 69.59 | −704.48 [−813.83, −572.12] sd 91.63 |

The rows to read are the first two of each table: at 5 bytes and 4 KiB the payload is small
enough that the vehicle *is* the answer. The 1 MiB row of the completion table is the noisiest
thing here (sd 62 µs on a mean of 31) and nothing is concluded from it — at that size a single
read is 200 µs and a neighbour's build moves it.

---

## 3. What the numbers say

**§8.4 reproduces.** `join − seq` is 59 µs where §8.4's `fixed` said 46. The difference is that
`fixed` timed a pair with *nothing in it*; here each closure also performs a read, so the stolen
closure's wake-up sits on the critical path of real work rather than beside it. Same floor, same
shape, a little more of it. The number is stable — sd 1.4 µs over nine repeats of 20 000 pairs —
so it is the vehicle and not the weather.

**D4's claim holds, and the answer is zero.** `both − seq` on the completion path is **negative**
at every size up to a quarter megabyte, with a spread that straddles zero: −0.25, −0.48, −0.61,
−0.86 µs. There is nothing to find. The kernel performs both reads, one `io_uring_enter`
collects them, no thread is started and none is woken — and the ring was already there, because
the runtime started before `main`. ADR-033 §8.5's "none in principle" is "none, measured".

That the mean is very slightly *below* zero is not a discovery, it is arithmetic: two reads in
one `io_uring_enter` is one syscall where `seq` pays two, so `both` saves a syscall and loses
nothing. The saving is smaller than the noise, which is why the honest statement is "zero".

**The finding nobody predicted: the fallback does not inherit D4's zero.** `both − seq` on the
blocking path is **+37.9 µs**, and it is *flat* — 37.87, 37.72, 37.32 µs at 5 bytes, 4 KiB and
64 KiB, sd under 2.5 µs throughout. A fixed cost that does not move with the payload is a
wake-up, and that is what it is: the pair is one message to a thread that is already parked, and
parking and unparking a thread costs what parking and unparking a thread costs however early it
was started.

So "the runtime starts before `main`" is **necessary and not sufficient**. D4 removes the thread
*start*; only D3's completion path removes the thread. Put the two tables side by side and
ADR-038's §1 is no longer an argument from the literature: *that pool **is** the 46 µs* is now a
measurement of this repository's own fallback.

**And the crossover is where ADR-033 put it.** On the fallback `both − seq` turns negative
between 64 KiB (+37 µs) and 256 KiB (−78 µs) — [ADR-033](specification/adr/adr-033.md) §8.2's
"crossover sits near **256 KB** of payload per operation", found again by a different vehicle on
a different mechanism, which is the kind of agreement that makes a number worth trusting. The
completion path has no crossover, because a fixed cost of zero has nothing to amortise.

---

## 4. One confound, named rather than hidden

The `seq` columns of the two tables **are not comparable to each other**, and nothing above
compares them. At 1 MiB the fallback's baseline is 894 µs and the completion path's is 401 µs —
a factor of 2.2 that is far too large to be the mechanism.

Part of it is known and is not about `io_uring` at all: `std::fs::read` sizes its buffer from
`stat` and then calls `read_to_end`, which finds the buffer full, **grows it** and reads again to
see the end of the file — so a 1 MiB read reallocates and copies a megabyte before it returns.
The ring path allocates `size + 1` and so reaches end-of-file without growing. The rest is not
attributed, and on a 4-vCPU box under load it is not worth attributing.

**It does not touch the conclusion**, because every number the conclusion rests on is a
difference *within* one mechanism: `join − seq` and `both − seq` compare vehicles over identical
reads. A cross-mechanism claim would need a quiet machine and an instruction count, which is the
standard `docs/staging-candidates.md` §2 sets and which this measurement deliberately does not
claim to meet. What it claims to meet is the standard `benches/overlap/README.md` sets: the
machine, the method, the repeat count and the spread, all stated.

---

## 5. How to run it again

```sh
benches/overlap/runtime.sh                     # the whole table, both mechanisms
benches/overlap/runtime.sh /tmp/somewhere 9    # …in a named directory, nine repeats

# one size, one mechanism, by hand
cd /tmp/somewhere
printf 'io-method = "blocking"\nio-workers = 2\n' > nikaia-runtime.toml
NIKAIA_RUNTIME_CONFIG=$PWD/nikaia-runtime.toml \
  cargo run -p overlap-bench --release --bin runtime -- 20000 9
```

The script prints the machine, the load average before and after each block, and the runtime's
own report of what it started on — `io-workers=2 io-method=auto (chose completion) user-pool=4
cleanup-deadline=30s user-code=concurrent` — so a table can always be read back to the
configuration that produced it.
