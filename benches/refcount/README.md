# What an atomic reference count costs

[ADR-037](../../docs/specification/adr/adr-037.md) D3 expands `Shared` from
`user_parallelism` — `no` gives `Rc`, `yes` gives `Arc` — **per build**, and says
of itself that whether the choice could be made *per value* "is a real question
and is not answered here". This measures what that question is worth, and it is
the evidence [`docs/rc-or-arc.md`](../../docs/rc-or-arc.md) rests on.

One binary, six shapes, each run as a **pair** whose two halves differ in one
thing — the reference count — so the difference is the count and nothing else:

| shape | what it is |
|---|---|
| `clone+drop` | a handle cloned, handed to an opaque call and dropped there |
| `read` | the value read through the handle. **The control**: no count is touched, so the pair must tie |
| `clone+work(k)` | the same, with `k` multiply-adds between them — the count as a share of a loop that does something |
| `counter` | Part II 12.2's `Shared[Locked[i32]]`: `Rc<RefCell<i32>>` against `Arc<Mutex<i32>>` |
| N threads, a handle each | the atomic with no sharing |
| N threads, one handle | the cache line that makes an atomic expensive rather than merely atomic |

```sh
benches/refcount/refcount.sh                    # the table, 50 M ops, 9 repeats
benches/refcount/refcount.sh 20000000 9 4       # …quicker, and with the thread count named
cargo run -p refcount-bench --release --bin refcount -- 20000000 9 4
```

The script prints the machine and the load average before and after, because a
table here cannot be read without them: [`docs/runtime-cost.md`](../../docs/runtime-cost.md)
§6.3 measured absolutes on a box of this class moving 1.4–1.9× from one day to
the next. **Every finding is a ratio, a flatness or a sign.**

## …and whether the two differ in anything observable at all

`cargo test -p refcount-bench` runs six assertions rather than a benchmark: that
the two handles are the same size, that the owner count matches at every step,
that **the value is cleaned up at the same point in the program**, that a weak
handle goes stale at the same point, and that exclusive access and unwrapping
succeed and fail alike. That last set is the gate the whole question had to pass
first — ADR-037 D3's reason for `Shared` being written by hand is that sharing
changes *when a value is cleaned up*, and if that told `Rc` from `Arc` there would
have been nothing to measure.

Two differences are real and are named in `docs/rc-or-arc.md` §2 rather than
asserted here: the **thread a destructor runs on**, and the count-overflow abort
threshold.

## The prototype this pays for

`crates/nikaia/src/contracts/sharing.rs` and `nikaia --sharing` ask the same
question of a value instead of a build. Nothing there emits anything — `Shared`
is unbuilt — and nothing here or there decides ADR-037 D3.
