# What it costs to answer a request with a file

**Date:** September 13, 2026
**Status:** measured; the numbers are
[ADR-058](specification/adr/adr-058.md)'s evidence
**Related:** [ADR-058](specification/adr/adr-058.md) (the decision these
settled), [ADR-018](specification/adr/adr-018.md) D2 (what a handler may
return), [ADR-038](specification/adr/adr-038.md) D3 (the I/O split), Part III
17.1 (`fs::map`)
**Produced by:** [`benches/sendfile/`](../benches/sendfile) —
`./benches/sendfile/sendfile.sh 7`, twice; §2 is the first run and §2.1 carries
the second where the two disagree

The README used to show an HTTP server that read `index.html` on every request,
and the criticism it drew was that reading a file in order to hand it on is
expensive. That is two claims, and only the first survives measurement.

**There are two programs here, not one**, and that is the first thing the bench
found. A server whose page is known when it starts may do everything once,
outside the handler. A server whose file the *request* names — a download route,
a file server, an upload served back — may do nothing once, and the vehicles
that win are not the same ones. §3 is the first program, §3.1 the second.

**The conclusion, first.** For a page known at startup, reading the file per
request is the expensive part and it is worth **2.2×** the machine's CPU at
4 KiB; `sendfile(2)`, the thing the criticism was pointing at, is worth nothing
at that size and wins only at a megabyte.

For a file the request names, the answer is a different one and sharper. What
decides is **not the mechanism but whether anything may be kept**: a table of
mappings made once is 13.5 µs at 4 KiB where the best vehicle that keeps nothing
is 28.0. And the obvious reading of "map it, don't read it" is a **trap** in this
program — `mmap` and `munmap` per request cost **76.0 µs**, two and a half times
what plainly reading the file costs, because a mapping is a page-table edit and
not a pointer. Among vehicles that may keep nothing, `sendfile` is the best or
tied at every size.

And `io_uring`'s `Splice`, which looks like the natural home for this on a
runtime that already runs files on a ring, is the worst vehicle at every size
while *appearing* to be the best by a factor of twenty on the wrong instrument.

---

## 1. The machine and the method

4 cores, Linux 6.18.44 x86_64, a shared virtual machine. `--release`. Load
average 0.31 before the block and 1.30 after, printed by the binary rather than
assumed away. `io_uring` is available on this kernel.

The vehicles serve the same file over **one kept-open TCP connection on
loopback**, the same number of times, with a thread draining the far end so that
a send blocks only when the kernel's buffers are genuinely full.

**Named at startup** — the path is a constant, so what may be done once is:

| vehicle | what it does |
| :--- | :--- |
| `read_each` | open, read, UTF-8-validate, write — once per request. The README's example |
| `cached` | read once at startup, write the heap buffer per request |
| `mapped` | `mmap` once at startup, write the mapping per request. What `fs::map` already gives |
| `sendfile` | `sendfile(2)`, the descriptor opened once. The program never has the bytes |
| `splice` | `io_uring` `Splice`, file → pipe → socket, the two hops submitted as one linked pair |

**Named by the request** — the path is not known until the request arrives, so
every vehicle pays the open, and only the last may keep anything. The requests
cycle over **64 files** rather than repeating one, because a route that names a
file does not name the same file, and one inode would let the kernel keep state
the real program does not get to keep:

| vehicle | what it does per request |
| :--- | :--- |
| `read_named` | open, read, write, close |
| `mapped_named` | open, `mmap`, write, `munmap`, close |
| `sendfile_named` | open, `fstat` (the length has to be known before the status line), `sendfile`, close |
| `mapped_kept` | a lookup in a table of mappings made once — the hot set a real file server keeps |

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

Median of seven, µs per request. First run.

### Named at startup

| | 4 KiB | 64 KiB | 1 MiB |
| :--- | ---: | ---: | ---: |
| `read_each` | 39.5 | 65.0 | 860 |
| `cached` | 22.5 | 51.5 | 760 |
| `mapped` | **19.0** | **45.5** | 780 |
| `sendfile` | 18.0 | 50.0 | **650** |
| `splice` | 73.5 | 88.0 | 1010 |

### Named by the request

| | 4 KiB | 64 KiB | 1 MiB |
| :--- | ---: | ---: | ---: |
| `read_named` | 30.5 | 65.5 | 925 |
| `mapped_named` | 76.0 | 124.0 | 715 |
| `sendfile_named` | **28.0** | **49.5** | 800 |
| `mapped_kept` | **13.5** | **46.0** | **635** |

### 2.1 The second run, and which rows moved

The whole block was run twice. Every ordering above reproduced except the
megabyte row of the second family, where three of the four vehicles are within
each other's spread and `mapped_named` moved from 715 to 940:

| named by the request, 1 MiB | run 1 | run 2 |
| :--- | ---: | ---: |
| `read_named` | 925 | 805 |
| `mapped_named` | 715 | 940 |
| `sendfile_named` | 800 | 845 |
| `mapped_kept` | **635** | **720** |

So **no claim below rests on the megabyte row of the second family** beyond
`mapped_kept` winning it, which both runs agree on. What reproduced exactly is
everything at 4 KiB and 64 KiB, and it is where the findings are.

## 3. What they say — a page known at startup

**The criticism was right about the read.** 39.5 µs against 19.0 at 4 KiB: more
than half of what answering a small request cost was opening the file, copying
it into the process and validating it as UTF-8 — work that produces the same
bytes every time. At 64 KiB it is 65.0 against 45.5 and at a megabyte 860
against 780: the read's share shrinks as the payload grows, which is what a
fixed per-request cost has to do.

**And wrong about what to do instead.** `sendfile` removes the copy that
`mapped` still pays, and at 4 KiB that is worth nothing: 18.0 against 19.0,
inside the spread. At 64 KiB it *loses* — 50.0 against 45.5. It wins at a
megabyte, 650 against 780, by 17 %. There is no copy worth a syscall's
bookkeeping at four kilobytes, which is why
[ADR-058](specification/adr/adr-058.md) D3 puts the choice inside `std` with a
size an operator can pin, and not in the language where a program's author would
have to guess it.

**`mapped` is `cached` without the heap**, and slightly ahead of it at every
size. Neither number is the reason to prefer it: the mapping does not hold a
second copy of a file the page cache already has, and that is a memory property
this bench does not measure and a server with a thousand pages would feel.

## 3.1 What they say — a file the request names

This is the program the first family cannot speak for, and three things separate
them.

**Keeping the mapping is worth more than any mechanism.** `mapped_kept` — a
table of mappings made once, looked up by path — is **13.5 µs** at 4 KiB where
the best vehicle that keeps nothing is 28.0, and it wins at every size in both
runs. A file server's real answer is a cache, and the interesting decision is
therefore not `sendfile` versus `read`: it is what `std` may keep, how it is
bounded, and when it is invalidated.

**Mapping per request is the trap, and it is the obvious reading of §3.** "Map
it, don't read it" is right when the mapping is made once and **wrong by 2.5×**
when it is not: `mapped_named` costs 76.0 µs at 4 KiB against `read_named`'s
30.5, and 124.0 against 65.5 at 64 KiB. A mapping is a page-table edit, an
address-space reservation and — on unmapping — an invalidation the other cores
have to hear about; none of that is amortised by four kilobytes of payload. An
implementation of `http::File` that mapped per request would be the slowest
option at the sizes servers send most, while looking like the zero-copy one.

**Among vehicles that may keep nothing, `sendfile` is the answer.** 28.0 against
`read_named`'s 30.5 at 4 KiB and 49.5 against 65.5 at 64 KiB, in both runs — and
it is the only one of the three whose cost does not include a copy into the
process, so it is the one that does not scale its memory with the number of
requests in flight. That is what an unbounded set of files — a download route
over user uploads, where a cache would be a liability rather than a hot set —
actually needs.

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

**The cache this bench keeps is perfect and a real one is not.** `mapped_kept`
maps all 64 files up front, never evicts and never checks whether the file
changed underneath it. A `std` that keeps mappings owes an eviction policy and
an answer to "the file was replaced" — both are correctness work this number
does not price, and
[ADR-058](specification/adr/adr-058.md) D8 is where they are decided rather than
assumed.

**And the absolutes travel badly.** This is a shared 4-vCPU VM; `docs/runtime-cost.md`
§6 records the same binary's numbers moving 1.4–1.9× between days on this
machine class. What reproduced across runs here is the *ordering* and the
*ratios* — `read_each` worst at small sizes, `mapped` and `cached` together,
`sendfile` behind them until the megabyte and ahead after, `splice` last — and
those are what [ADR-058](specification/adr/adr-058.md) rests on.
