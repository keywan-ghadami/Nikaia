# What an uncontended lock acquisition costs

Part II [12.2](../../docs/specification/20-nikaia-advance.md) expands `Locked[T]`
from `user_parallelism`: at `no` it is "similar to a `RefCell` with a reentrancy
check", at `yes` "a real OS-level **Mutex**". Whether the floor could be a
`Mutex` at *both* settings is open, and it is only worth asking if the `Mutex` is
worth avoiding. This measures that, and it is the evidence
[`docs/mutex-floor.md`](../../docs/mutex-floor.md) rests on.

One binary, five shapes, each run as a **pair** whose two halves differ in one
thing — the lock — so the difference is the lock and nothing else:

| shape | what it is |
|---|---|
| `access` | Part II 12.2's `counter.access fn { a += 1 }` with no reference count in it: `RefCell<i32>` against `Mutex<i32>` |
| `access` twice over | the *same* half run twice. **The control**: it must tie, and it bounds every other difference from below |
| `access` + `k` work | the same acquisition with `k` multiply-adds inside the guard — the lock as a share of a `sync` body that does something |
| reentrancy-checked | a `Mutex` that keeps 12.2's `no` diagnostic against a plain one — what recovering that panic would cost on a `Mutex` floor |
| N threads | one `Mutex` each against one `Mutex` between them |

The `RefCell` half of the last shape does not exist, and that is the point rather
than an omission: `RefCell` is not `Sync`, so two threads cannot reach one. The
row it would occupy is why the floor is a question at all.

```sh
benches/lockfloor/lockfloor.sh                  # the table, 20 M ops, 9 repeats
benches/lockfloor/lockfloor.sh 20000000 9 4     # …with the thread count named
cargo run -p lockfloor-bench --release --bin lockfloor -- 20000000 9 4
```

The script prints the machine and the load average before and after, because a
table here cannot be read without them:
[`docs/runtime-cost.md`](../../docs/runtime-cost.md) §6.3 measured absolutes on a
box of this class moving 1.4–1.9× from one day to the next. **Every finding is a
ratio, a flatness or a sign**, and the one row whose ratio did not survive a
second run is named as such in `docs/mutex-floor.md` §4.2 rather than averaged.

## …and whether the two differ in anything observable

`cargo test -p lockfloor-bench` runs four assertions rather than a benchmark:
that re-entering **panics one way and would block the other**, that a panic
inside the guard **poisons one and not the other**, that only one of the two may
cross a thread, and what each one weighs (`RefCell<i32>` = 16 bytes,
`Mutex<i32>` = 12, on this target).

That set is the mirror image of the gate `docs/rc-or-arc.md` §2 put `Rc` and
`Arc` through. There the answer was "nothing a program can observe", which is
what let that choice be considered as an inference. Here the answer is the
opposite, twice — which is why the lock is *written* and not inferred, and why
the only open question about it is the floor.

## What this does not do

Nothing here emits anything, and nothing here decides `Locked[T]`'s
representation. `Locked` is unbuilt: the compiler accepts the type name, the
backend does not know it, and `docs/mutex-floor.md` §1.1 is the list of what
could therefore not be run.
