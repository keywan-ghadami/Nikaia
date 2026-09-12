//! What an always-`Mutex` floor for `Locked[T]` costs, and what it changes.
//!
//! Part II 12.2 expands `Locked[T]` from `user_parallelism`: at `no` it is
//! "similar to a `RefCell` with a reentrancy check", at `yes` "a real OS-level
//! **Mutex**". Whether the floor could be a `Mutex` at both settings is open,
//! and it has two halves — a cost, and a difference a program can see. This
//! binary measures the first and asserts the second, which is the shape
//! [`docs/rc-or-arc.md`](../../../../docs/rc-or-arc.md) used for `Rc` against
//! `Arc`.
//!
//! Five shapes, each a **pair** whose two halves differ in one thing, so the
//! difference is that thing and nothing else:
//!
//! | shape | what it is |
//! |---|---|
//! | `access` | Part II 12.2's `counter.access fn { a += 1 }` with no reference count in it: `RefCell<i32>` against `Mutex<i32>` |
//! | `access` twice over | the *same* half run twice. **The control**: it must tie, and it bounds every other difference from below |
//! | `access` + `k` work | the same acquisition with `k` multiply-adds inside the guard — the lock as a share of a `sync` body that does something |
//! | reentrancy-checked | a `Mutex` that keeps 12.2's `no` diagnostic (an owner check before the acquisition) against a plain one — what recovering the panic would cost on a `Mutex` floor |
//! | N threads | one `Mutex` each against one `Mutex` between them — what the floor costs at `yes` when it is not contended, and when it is |
//!
//! The `RefCell` half of the threaded shape does not exist, and that is the
//! point: `RefCell` is not `Sync`, so it cannot be reached from two threads at
//! all. The row it would occupy is why the floor is a question.
//!
//! ```sh
//! benches/lockfloor/lockfloor.sh                  # the table, 9 repeats
//! cargo run -p lockfloor-bench --release --bin lockfloor -- 20000000 9 4
//! ```
//!
//! Every repeat is printed, because a single number without its spread is not
//! a measurement, and
//! [`docs/runtime-cost.md`](../../../../docs/runtime-cost.md) §6.3 is why only
//! the ratios and the signs here are load-bearing: absolutes on a box of this
//! class moved 1.4–1.9× from one day to the next.

use std::cell::RefCell;
use std::hint::black_box;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

/// Nanoseconds per iteration, and whatever the loop accumulated so nothing in
/// it can be optimised away.
fn per_op(n: usize, mut shape: impl FnMut() -> usize) -> (f64, usize) {
    let began = std::time::Instant::now();
    let mut acc = 0usize;
    for _ in 0..n {
        acc = acc.wrapping_add(shape());
    }
    (began.elapsed().as_secs_f64() * 1e9 / n as f64, acc)
}

/// Mean, minimum, maximum and standard deviation over the repeats.
fn stats(xs: &[f64]) -> (f64, f64, f64, f64) {
    let n = xs.len() as f64;
    let mean = xs.iter().sum::<f64>() / n;
    let var = xs.iter().map(|x| (x - mean) * (x - mean)).sum::<f64>() / n;
    let lo = xs.iter().cloned().fold(f64::INFINITY, f64::min);
    let hi = xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    (mean, lo, hi, var.sqrt())
}

/// `k` multiply-adds, which is a body that is not a lock.
///
/// `seed` varies with the iteration in every caller: a loop-invariant argument
/// is hoisted out and then nothing is being measured (`docs/rc-or-arc.md` §8
/// is the run where that happened).
#[inline(never)]
fn work(k: usize, seed: usize) -> usize {
    let mut acc = seed;
    for i in 0..k {
        acc = acc.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(i);
    }
    acc
}

/// A `Mutex` that keeps Part II 12.2's `no` diagnostic.
///
/// The floor's one lost guarantee at `user_parallelism = no` is the reentrancy
/// *panic*: a `RefCell` says "already borrowed" where a `Mutex` deadlocks. An
/// owner field restores the panic — this is what it costs, not a proposal that
/// it be built.
struct Checked<T> {
    owner: AtomicU64,
    inner: Mutex<T>,
}

impl<T> Checked<T> {
    fn new(value: T) -> Checked<T> {
        Checked {
            owner: AtomicU64::new(0),
            inner: Mutex::new(value),
        }
    }

    /// The acquisition, with the check in front of it.
    fn access<R>(&self, me: u64, f: impl FnOnce(&mut T) -> R) -> R {
        if self.owner.load(Ordering::Relaxed) == me {
            panic!("this task already holds the lock");
        }
        let mut held = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        self.owner.store(me, Ordering::Relaxed);
        let out = f(&mut held);
        self.owner.store(0, Ordering::Relaxed);
        out
    }
}

/// One shape's repeats, printed as they are taken.
struct Series {
    name: &'static str,
    ns: Vec<f64>,
}

impl Series {
    fn take(
        name: &'static str,
        n: usize,
        repeats: usize,
        mut shape: impl FnMut(usize) -> (f64, usize),
    ) -> Self {
        // Warm-up: a tenth of a run, so no repeat pays for a cold branch
        // predictor.
        let _ = shape(n / 10 + 1);
        let mut ns = Vec::with_capacity(repeats);
        print!("  {name:<34}");
        for _ in 0..repeats {
            let (t, acc) = shape(n);
            black_box(acc);
            print!(" {t:7.3}");
            ns.push(t);
        }
        println!();
        Series { name, ns }
    }

    fn stats(&self) -> (f64, f64, f64, f64) {
        stats(&self.ns)
    }
}

/// A pair of series differing in one thing, and what the difference is.
fn compare(plain: &Series, locked: &Series) {
    let (pm, plo, phi, psd) = plain.stats();
    let (lm, llo, lhi, lsd) = locked.stats();
    println!(
        "  {:<34} {pm:7.3} [{plo:.3}, {phi:.3}] sd {psd:.3}",
        plain.name
    );
    println!(
        "  {:<34} {lm:7.3} [{llo:.3}, {lhi:.3}] sd {lsd:.3}",
        locked.name
    );
    println!(
        "  {:<34} {:+7.3} ns   ×{:.2}",
        "difference / ratio",
        lm - pm,
        lm / pm
    );
    println!();
}

/// Per-operation-per-thread time for `threads` threads each doing `n`
/// acquisitions of whatever `one` hands them.
fn threaded(
    threads: usize,
    n: usize,
    one: impl Fn(usize) -> &'static Mutex<i32> + Sync,
) -> (f64, usize) {
    let began = std::time::Instant::now();
    let acc: usize = std::thread::scope(|s| {
        let hs: Vec<_> = (0..threads)
            .map(|t| {
                let lock = one(t);
                s.spawn(move || {
                    let mut acc = 0usize;
                    for _ in 0..n {
                        let mut held = lock.lock().unwrap();
                        *held += 1;
                        acc = acc.wrapping_add(1);
                    }
                    acc
                })
            })
            .collect();
        hs.into_iter().map(|h| h.join().unwrap()).sum()
    });
    let total = threads * n;
    (began.elapsed().as_secs_f64() * 1e9 / total as f64, acc)
}

fn main() {
    let mut args = std::env::args().skip(1);
    let n: usize = args
        .next()
        .unwrap_or_else(|| "20000000".into())
        .parse()
        .unwrap();
    let repeats: usize = args.next().unwrap_or_else(|| "9".into()).parse().unwrap();
    let threads: usize = args
        .next()
        .unwrap_or_else(|| {
            std::thread::available_parallelism()
                .unwrap()
                .get()
                .to_string()
        })
        .parse()
        .unwrap();

    println!("ns per operation, {repeats} repeats of {n} operations; every repeat printed");
    println!();

    let cell = RefCell::new(0i32);
    let lock = Mutex::new(0i32);

    // ---- the acquisition and nothing else. -------------------------------
    println!("access fn {{ a += 1 }} — uncontended, one thread");
    let plain = Series::take("RefCell<i32>", n, repeats, |n| {
        per_op(n, || {
            let c = black_box(&cell);
            *c.borrow_mut() += 1;
            1
        })
    });
    let locked = Series::take("Mutex<i32>", n, repeats, |n| {
        per_op(n, || {
            let m = black_box(&lock);
            *m.lock().unwrap() += 1;
            1
        })
    });
    compare(&plain, &locked);

    // ---- the same half twice: the control. -------------------------------
    println!("the same RefCell half, twice over (the control — must tie)");
    let first = Series::take("RefCell<i32>, run A", n, repeats, |n| {
        per_op(n, || {
            let c = black_box(&cell);
            *c.borrow_mut() += 1;
            1
        })
    });
    let second = Series::take("RefCell<i32>, run B", n, repeats, |n| {
        per_op(n, || {
            let c = black_box(&cell);
            *c.borrow_mut() += 1;
            1
        })
    });
    compare(&first, &second);

    // ---- the lock as a share of a body that computes. --------------------
    for k in [4usize, 40, 400] {
        println!("access with {k} multiply-adds inside the guard");
        let each = (n / (1 + k)).max(100_000);
        let plain = Series::take("RefCell<i32>", each, repeats, |n| {
            let mut i = 0usize;
            per_op(n, || {
                i = i.wrapping_add(1);
                let c = black_box(&cell);
                let mut held = c.borrow_mut();
                *held = held.wrapping_add(1);
                work(k, i)
            })
        });
        let locked = Series::take("Mutex<i32>", each, repeats, |n| {
            let mut i = 0usize;
            per_op(n, || {
                i = i.wrapping_add(1);
                let m = black_box(&lock);
                let mut held = m.lock().unwrap();
                *held = held.wrapping_add(1);
                work(k, i)
            })
        });
        compare(&plain, &locked);
    }

    // ---- keeping 12.2's `no` diagnostic on a `Mutex` floor. --------------
    println!("a Mutex floor that keeps 12.2's reentrancy panic");
    let checked = Checked::new(0i32);
    let plain = Series::take("Mutex<i32>, no check", n, repeats, |n| {
        per_op(n, || {
            let m = black_box(&lock);
            *m.lock().unwrap() += 1;
            1
        })
    });
    let guarded = Series::take("Mutex<i32> + owner check", n, repeats, |n| {
        per_op(n, || {
            let c = black_box(&checked);
            c.access(1, |v| {
                *v = v.wrapping_add(1);
                1
            })
        })
    });
    compare(&plain, &guarded);

    // ---- what the floor costs at `yes`. ----------------------------------
    // `RefCell` has no row here, and that is the finding rather than an
    // omission: it is not `Sync`, so two threads cannot reach one.
    println!("access on {threads} threads — ns per operation per thread");
    let each = (n / 4).max(100_000);
    let own: &'static Vec<Mutex<i32>> =
        Box::leak(Box::new((0..threads).map(|_| Mutex::new(0i32)).collect()));
    let shared: &'static Mutex<i32> = Box::leak(Box::new(Mutex::new(0i32)));
    let apart = Series::take("Mutex, one each", each, repeats, |n| {
        threaded(threads, n, |t| &own[t])
    });
    let together = Series::take("Mutex, one between them", each, repeats, |n| {
        threaded(threads, n, |_| shared)
    });
    compare(&apart, &together);
}

/// Whether `RefCell` and `Mutex` differ in anything a program can observe —
/// the question the cost above is only worth asking if the answer is yes.
///
/// `docs/rc-or-arc.md` §2 asked this of `Rc` against `Arc` and found nothing,
/// which is what let that inference be considered at all. Here the answer is
/// the opposite, and these are the assertions that say so rather than the
/// argument that says so. Two of them are the ones Part II 12.2 names as its
/// per-setting safety nets: the reentrancy check and poisoning.
#[cfg(test)]
mod observable {
    use super::*;
    use std::sync::Arc;

    /// Part II 12.2's `no` bullet: "Task A locks data, waits for network, Task
    /// B tries to lock same data -> Panic!". A `RefCell` panics; a `Mutex`
    /// would block, which on one user thread is the whole program.
    ///
    /// The `Mutex` half is asserted with `try_lock` rather than `lock`,
    /// because `lock` here is the hang this test would otherwise be.
    #[test]
    fn reentering_panics_one_way_and_blocks_the_other() {
        let cell = RefCell::new(0i32);
        let held = cell.borrow_mut();
        assert!(cell.try_borrow_mut().is_err());
        drop(held);

        let lock = Mutex::new(0i32);
        let held = lock.lock().unwrap();
        assert!(
            matches!(lock.try_lock(), Err(std::sync::TryLockError::WouldBlock)),
            "a second acquisition on the same thread would block, not fail"
        );
        drop(held);
    }

    /// Part II 12.2's `yes` safety net: a panic inside `access` leaves the
    /// lock poisoned there and merely released here, so the *next* `access`
    /// behaves differently. That is a difference in a well-formed program's
    /// observable behaviour after an unrelated task has crashed.
    #[test]
    fn a_panic_inside_the_guard_poisons_one_and_not_the_other() {
        let lock = Arc::new(Mutex::new(0i32));
        let mine = Arc::clone(&lock);
        let crashed = std::thread::spawn(move || {
            let _held = mine.lock().unwrap();
            panic!("the task that held it");
        })
        .join();
        assert!(crashed.is_err());
        assert!(
            lock.lock().is_err(),
            "the mutex is poisoned for every later acquisition"
        );
        assert!(lock.is_poisoned());

        // The same script with a borrow flag: the flag is released by the
        // unwind and the next borrow succeeds, with nothing recorded.
        let flag = RefCell::new(0i32);
        let crashed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _held = flag.borrow_mut();
            panic!("the code that held it");
        }));
        assert!(crashed.is_err());
        assert!(
            flag.try_borrow_mut().is_ok(),
            "a borrow flag records nothing about the panic"
        );
    }

    /// And the one that makes the floor a question at all: a `Mutex` may be
    /// reached from another thread and a `RefCell` may not. `Arc<RefCell<T>>`
    /// is not `Send`, which is what `contracts/send.rs` already encodes and
    /// what a commented line here would not compile to say.
    #[test]
    fn only_one_of_the_two_may_cross_a_thread() {
        fn crossable<T: Send>(_: &T) -> bool {
            true
        }
        assert!(crossable(&Arc::new(Mutex::new(0i32))));
    }

    /// What each one weighs, since the floor puts one of them in every
    /// `Locked[T]` a program has.
    #[test]
    fn what_each_one_weighs() {
        // Not an equality: the point is that the two differ and by how much,
        // and a number asserted here would be this platform's rather than a
        // property of the language.
        let cell = std::mem::size_of::<RefCell<i32>>();
        let lock = std::mem::size_of::<Mutex<i32>>();
        assert!(cell > 0 && lock > 0);
        println!("RefCell<i32> = {cell} bytes, Mutex<i32> = {lock} bytes");
    }
}
