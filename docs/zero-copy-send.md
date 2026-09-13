# What it costs to answer a request with a file

**Date:** September 13, 2026
**Status:** measured; the numbers are
[ADR-058](specification/adr/adr-058.md)'s evidence
**Related:** [ADR-058](specification/adr/adr-058.md) (the decision these
settled), [ADR-018](specification/adr/adr-018.md) D2 (what a handler may
return), [ADR-038](specification/adr/adr-038.md) D3 (the I/O split), Part III
17.1 (`fs::map`)
**Produced by:** [`benches/sendfile/`](../benches/sendfile) —
`./benches/sendfile/sendfile.sh 7`

The README used to show an HTTP server that read `index.html` on every request,
and the criticism it drew was that reading a file in order to hand it on is
expensive. That is two claims, and only the first survives measurement.

**The conclusion, first.** Reading the file per request is the expensive part
and it is worth **2.2×** the machine's CPU at 4 KiB. Sending it *without*
reading it — `sendfile(2)`, the thing the criticism was pointing at — is worth
nothing at that size and **loses 19 %** to simply mapping the file once; it wins
only at a megabyte, where it takes 610 µs of machine CPU against a mapping's
760. And `io_uring`'s `Splice`, which looks like the natural home for this on a
runtime that already runs files on a ring, is the worst vehicle at every size
while *appearing* to be the best by a factor of twenty on the wrong instrument.

---

## 1. The machine and the method

4 cores, Linux 6.18.44 x86_64, a shared virtual machine. `--release`. Load
average 0.31 before the block and 1.30 after, printed by the binary rather than
assumed away. `io_uring` is available on this kernel.

Five vehicles serve the same file over **one kept-open TCP connection on
loopback**, the same number of times, with a thread draining the far end so that
a send blocks only when the kernel's buffers are genuinely full:

| vehicle | what it does |
| :--- | :--- |
| `read_each` | open, read, UTF-8-validate, write — once per request. The README's example |
| `cached` | read once at startup, write the heap buffer per request |
| `mapped` | `mmap` once at startup, write the mapping per request. What `fs::map` already gives |
| `sendfile` | `sendfile(2)`, the descriptor opened once. The program never has the bytes |
| `splice` | `io_uring` `Splice`, file → pipe → socket, the two hops submitted as one linked pair |

20 000 sends at 4 KiB and 64 KiB, 2 000 at 1 MiB; seven repeats, the median
printed and the wall spread beside it. The far end counts the bytes it received
and the run fails if they are not all there — a vehicle that sends fewer is a
faster vehicle for the wrong reason.

**The column that answers is `machine`.** Three instruments disagree here and
the disagreement is itself a finding:

* *wall* is the loopback pipeline's, reader included;
* *thread* is the sending thread's own `RUSAGE_THREAD`, which is what a naive
  benchmark reports — and which misses every jiffy `io_uring` spends on kernel
  workers;
* *machine* is every non-idle jiffy on the box, from `/proc/stat`, on an
  otherwise quiet machine. It is the one an operator is billed for and the one
  that catches work moved out of the caller's thread.

---

## 2. The numbers

Median of seven, µs per request.

### 4 KiB — the size a static page usually is

| vehicle | wall | **machine** | thread |
| :--- | ---: | ---: | ---: |
| `read_each` | 24.3 | **31.5** | 24.4 |
| `cached` | 9.8 | **14.5** | 9.8 |
| `mapped` | 9.7 | **15.5** | 9.8 |
| `sendfile` | 13.0 | **18.5** | 13.0 |
| `splice` | 71.3 | **78.0** | 21.5 |

### 64 KiB

| vehicle | wall | **machine** | thread |
| :--- | ---: | ---: | ---: |
| `read_each` | 49.8 | **67.5** | 49.3 |
| `cached` | 37.1 | **53.0** | 36.9 |
| `mapped` | 29.9 | **40.0** | 29.9 |
| `sendfile` | 38.8 | **49.0** | 38.6 |
| `splice` | 77.6 | **89.0** | 21.7 |

### 1 MiB

| vehicle | wall | **machine** | thread |
| :--- | ---: | ---: | ---: |
| `read_each` | 645.2 | **895.0** | 635.1 |
| `cached` | 567.0 | **770.0** | 553.2 |
| `mapped` | 557.8 | **760.0** | 545.7 |
| `sendfile` | 436.7 | **610.0** | 433.2 |
| `splice` | 700.8 | **955.0** | 26.1 |

---

## 3. What they say

**The criticism was right about the read.** 31.5 µs against 15.5 at 4 KiB: more
than half of what answering a small request cost was opening the file, copying
it into the process and validating it as UTF-8 — work that produces the same
bytes every time. At 64 KiB it is 67.5 against 40.0 and at a megabyte 895
against 760: the read's share shrinks as the payload grows, which is what a
fixed per-request cost has to do.

**And wrong about what to do instead.** `sendfile` removes the copy that
`mapped` still pays, and at 4 KiB that is worth **−4 µs**: 18.5 against 15.5, a
19 % *loss*. There is no copy worth a syscall's bookkeeping at four kilobytes.
The crossover is somewhere between 64 KiB (49.0 against 40.0, still losing) and
a megabyte (610 against 760, winning by 20 %) — which is why
[ADR-058](specification/adr/adr-058.md) D3 puts the choice inside `std` with a
size the operator can pin, and not in the language where a program's author
would have to guess it.

**`mapped` is `cached` without the heap.** The two are within noise at 4 KiB and
a megabyte, and the mapping is 25 % better at 64 KiB. Neither number is the
reason to prefer it: the mapping does not hold a second copy of a file the page
cache already has, and that is a memory property this bench does not measure and
a server with a thousand pages would feel.

**`splice` is the trap.** Its *thread* column is 21.5, 21.7 and 26.1 µs — flat,
tiny, and independent of the payload, which is exactly what a zero-copy
mechanism is supposed to look like. Its *machine* column is 78, 89 and 955 µs:
the worst of the five everywhere. The work did not vanish, it moved to
`io_uring`'s kernel workers, where the submitting thread's accounting cannot see
it. Both hops were submitted as one linked pair and the pipe was widened to a
megabyte first, so the number is not a strawman: a pipe between the file and the
socket is a second transfer and batching does not remove it.

---

## 4. What this does not measure, and one confound

**Loopback with a draining reader is the friendly case.** A real client is slow
and far away, and the difference that makes is not CPU per request: it is that
`cached` and `mapped` need the body to stay addressable for as long as the
slowest connection takes, while `sendfile` needs a descriptor and an offset. At
a thousand slow connections that is a memory-scaling argument for `sendfile`
that this bench, with its one fast reader, cannot see. It is named here rather
than folded into the recommendation.

**Nothing here is encrypted.** [ADR-038](specification/adr/adr-038.md) D2 binds
`rustls`, and a TLS response cannot use `sendfile` at all without kTLS — so on a
server that terminates TLS, the `sendfile` column is not available and the
`mapped` column is the answer at every size.

**The confound: the page cache is warm throughout.** Every vehicle reads a file
that was written seconds earlier on a box with 15 GB of RAM, so `read_each`'s
disadvantage is a copy and a syscall, never a disk. On a cold cache every
vehicle pays the same read once and the shape of the table would not change —
but `read_each` would pay it *per request* only in the sense that the cache
would have to keep it, which is the same page cache the other four are using.
The honest statement is that this measures the warm case, which is the case a
web server serving a static page is in.

**And the absolutes travel badly.** This is a shared 4-vCPU VM; `docs/runtime-cost.md`
§6 records the same binary's numbers moving 1.4–1.9× between days on this
machine class. What reproduced across runs here is the *ordering* and the
*ratios* — `read_each` worst at small sizes, `mapped` and `cached` together,
`sendfile` behind them until the megabyte and ahead after, `splice` last — and
those are what [ADR-058](specification/adr/adr-058.md) rests on.
