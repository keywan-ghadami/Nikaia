//! What `Locked[T]` is in the machine — two shapes behind one surface
//! ([ADR-057](../../../docs/specification/adr/adr-057.md)).
//!
//! **The shape is decided per value and not per build setting** (D1–D3): where
//! `contracts::sharing` proves that nothing crosses a thread with a value, its
//! lock is [`Local`]; where it does not, and at every value that does cross, it
//! is [`Crossing`]. The floor is the one that is safe, exactly as the reference
//! count's floor is the atomic one.
//!
//! **Both say the same thing when a program re-enters one lock** (D4), and that
//! is what the crossing shape pays its owner check for. A borrow flag says so by
//! itself; a bare `Mutex` would hang for ever without saying anything, and at one
//! user thread a hang is the whole program plus
//! [ADR-006](../../../docs/specification/adr/adr-006.md) D5's shutdown deadline.
//! Two behaviours for one written type, picked by an analysis the reader cannot
//! see, is the thing D4 refuses.
//!
//! **Neither is a pause.** `access` takes a lambda that runs straight through
//! ([ADR-005](../../../docs/specification/adr/adr-005.md) D6), so nothing is held
//! across a suspension point and a wait here is bounded by somebody else's
//! compute rather than by somebody else's waiting.

use std::cell::RefCell;
use std::sync::Mutex;

/// The one message, so re-entering reads the same whichever shape a value got.
///
/// `#[track_caller]` out to the `access` that called it, so the location the
/// panic hook is handed is the generated line that wrote the access and not a
/// line of this file — without it
/// [ADR-044](../../../docs/specification/adr/adr-044.md) D1's table has nothing
/// to look up.
#[cold]
#[inline(never)]
#[track_caller]
fn emptied() -> ! {
    panic!("this lock is empty: an `update` on it failed and left nothing behind")
}

/// The one message, so re-entering reads the same whichever shape a value got.
#[cold]
#[inline(never)]
#[track_caller]
fn reentered() -> ! {
    panic!("this lock is already held by the same task: `access` cannot be re-entered")
}

/// A lock for a value the analysis proved never crosses a thread.
///
/// A borrow flag, which is one non-atomic write and a branch. It is what every
/// lock is at `user_parallelism = no` (D2), because nothing a user writes can
/// cross there and the mutex would charge exactly what that switch exists to
/// save.
#[derive(Debug, Default)]
pub struct Local<T> {
    /// **`Option`, so `update` can move the value out and back** without a
    /// `Default` bound and without unsafe code
    /// ([ADR-059](../../../docs/specification/adr/adr-059.md) D2). A lambda that
    /// fails leaves it `None`, and that is reported as what it is rather than
    /// read as something else - the same thing a poisoned mutex says at the
    /// other setting.
    inner: RefCell<Option<T>>,
}

impl<T> Local<T> {
    pub fn new(value: T) -> Self {
        Self {
            inner: RefCell::new(Some(value)),
        }
    }

    /// Part II 12.2's first door: a copy out. No code of the user's runs while
    /// the lock is open, which is why this needs no condition at all.
    #[track_caller]
    pub fn get(&self) -> T
    where
        T: Clone,
    {
        self.read(|value| value.clone())
    }

    /// …and the second: replacing. The new value is computed outside, so the
    /// lock is open for the duration of one store.
    #[track_caller]
    pub fn set(&self, value: T) {
        match self.inner.try_borrow_mut() {
            Ok(mut held) => *held = Some(value),
            Err(_) => reentered(),
        }
    }

    /// **Reading in place** ([ADR-059](../../../docs/specification/adr/adr-059.md)
    /// D1): the lambda is handed the value where it lies and may not change it,
    /// so nothing is copied to answer a question about a large value.
    #[track_caller]
    pub fn access<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        self.read(f)
    }

    /// **Where locked data changes** (D2). The old value is moved out, the
    /// lambda is handed it by value, and what it returns is moved back.
    #[track_caller]
    pub fn update(&self, f: impl FnOnce(T) -> T) {
        let old = match self.inner.try_borrow_mut() {
            Ok(mut held) => held.take().unwrap_or_else(|| emptied()),
            Err(_) => reentered(),
        };
        let new = f(old);
        match self.inner.try_borrow_mut() {
            Ok(mut held) => *held = Some(new),
            Err(_) => reentered(),
        }
    }

    /// **Exclusive even to read**, and that is not an oversight.
    ///
    /// Two reads of one lock do not conflict, so a shared borrow would let a
    /// nested `access` through. The crossing shape cannot: a mutex is exclusive
    /// and its owner check fires. One written type may not behave two ways
    /// ([ADR-057](../../../docs/specification/adr/adr-057.md) D4), and of the two
    /// ways to agree, refusing is the one that keeps Part II 12.2's rule whole -
    /// re-entering a lock is a defect in code that is not well-formed, whichever
    /// door it happens through.
    #[track_caller]
    fn read<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        match self.inner.try_borrow_mut() {
            Ok(held) => match held.as_ref() {
                Some(value) => f(value),
                None => emptied(),
            },
            Err(_) => reentered(),
        }
    }
}

/// A lock for a value that crosses a thread, or that the analysis could not
/// prove does not.
///
/// An OS mutex **with an owner check**, and the check is D4's price rather than
/// an extra: without it this shape hangs where [`Local`] reports, and one written
/// type would mean two behaviours. Measured at +2.0 ns, ×1.15 against a bare
/// `Mutex` (`docs/mutex-floor.md` §4.3).
#[derive(Debug, Default)]
pub struct Crossing<T> {
    /// `Option` for the reason [`Local`]'s is
    /// ([ADR-059](../../../docs/specification/adr/adr-059.md) D2).
    inner: Mutex<Option<T>>,
    /// Which task holds it, or `None`. Read before the acquisition and written
    /// after it, which is why a re-entry is seen *before* the acquisition blocks:
    /// once it blocks there is nothing left to report to.
    held_by: Mutex<Option<u64>>,
}

impl<T> Crossing<T> {
    pub fn new(value: T) -> Self {
        Self {
            inner: Mutex::new(Some(value)),
            held_by: Mutex::new(None),
        }
    }

    /// Part II 12.2's first two doors, which run no code of the user's.
    #[track_caller]
    pub fn get(&self) -> T
    where
        T: Clone,
    {
        self.access(|value| value.clone())
    }

    #[track_caller]
    pub fn set(&self, value: T) {
        self.hold(|slot| *slot = Some(value));
    }

    /// **Reading in place** ([ADR-059](../../../docs/specification/adr/adr-059.md)
    /// D1).
    #[track_caller]
    pub fn access<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        self.hold(|slot| match slot.as_ref() {
            Some(value) => f(value),
            None => emptied(),
        })
    }

    /// **Where locked data changes** (D2): out by value, back by value, and the
    /// lock is held across both so nothing sees the gap.
    #[track_caller]
    pub fn update(&self, f: impl FnOnce(T) -> T) {
        self.hold(|slot| {
            let old = slot.take().unwrap_or_else(|| emptied());
            *slot = Some(f(old));
        });
    }

    /// The owner check and the guard, around whatever the door does with the
    /// slot — written once, because every door needs both and a door that
    /// forgot one would be the hang this shape carries the check to prevent.
    #[track_caller]
    fn hold<R>(&self, f: impl FnOnce(&mut Option<T>) -> R) -> R {
        let me = current_task();
        {
            let mut held = self.held_by.lock().unwrap_or_else(|e| e.into_inner());
            if *held == Some(me) {
                reentered();
            }
            *held = Some(me);
        }

        // **Poisoning is kept.** Part III Appendix A.2's `yes` row says a
        // resource held by a panicking task is poisoned so no other thread reads
        // what a half-finished task left behind, and that is a property of the
        // mutex rather than something written here - so the guard is taken
        // without clearing the poison.
        let result = {
            let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            f(&mut guard)
        };

        *self.held_by.lock().unwrap_or_else(|e| e.into_inner()) = None;
        result
    }
}

/// Who is asking, for the owner check.
///
/// The thread is the unit today, because a task does not move between threads
/// while it holds a lock: `access` takes a lambda that runs straight through, so
/// there is no suspension point between the two lines above for a task to be
/// moved at ([ADR-005](../../../docs/specification/adr/adr-005.md) D6). When a
/// task identity exists this is the one line that changes.
fn current_task() -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    std::thread::current().id().hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The four doors, on both shapes
    /// ([ADR-059](../../../docs/specification/adr/adr-059.md)).
    #[test]
    fn both_shapes_carry_all_four_doors() {
        let local = Local::new(1_i64);
        assert_eq!(local.get(), 1);
        local.set(5);
        local.update(|old| old + 100);
        assert_eq!(local.access(|n| *n), 105);

        let crossing = Crossing::new(1_i64);
        assert_eq!(crossing.get(), 1);
        crossing.set(5);
        crossing.update(|old| old + 100);
        assert_eq!(crossing.access(|n| *n), 105);
    }

    /// **`update` moves, it does not copy**, which is the finding that made
    /// `access` stop needing to write (D2). The list's buffer is the same
    /// allocation after an append, so nothing was copied to add one entry.
    #[test]
    fn update_appends_without_copying_the_value() {
        // Room to spare, so a `push` does not grow the buffer: what is being
        // asked is whether `update` copies, not whether `Vec` reallocates.
        let mut start = Vec::with_capacity(8);
        start.extend([1_i64, 2, 3]);
        let local = Local::new(start);
        let before = local.access(|v| v.as_ptr());
        local.update(|mut v| {
            v.push(4);
            v
        });
        assert_eq!(local.access(|v| v.len()), 4);
        assert_eq!(
            local.access(|v| v.as_ptr()),
            before,
            "the same buffer: an append is a move and not a copy"
        );
    }

    /// And reading in place answers a question about a large value without
    /// taking a copy of it (D1) — the one thing neither `get` nor `update` does.
    #[test]
    fn access_reads_a_large_value_in_place() {
        let local = Local::new((0..10_000_i64).collect::<Vec<_>>());
        let before = local.access(|v| v.as_ptr());
        assert_eq!(local.access(|v| v.len()), 10_000);
        assert_eq!(local.access(|v| v.as_ptr()), before, "nothing was copied");
    }

    /// D4, which is the whole reason the crossing shape carries an owner check:
    /// **one behaviour, two prices.** A bare `Mutex` hangs here.
    #[test]
    fn re_entering_says_the_same_thing_in_both_shapes() {
        let local = Local::new(1_i64);
        let said = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            local.access(|_| local.access(|n| *n))
        }))
        .expect_err("the cheap shape reports");
        assert!(message(&said).contains("already held by the same task"));

        let crossing = Crossing::new(1_i64);
        let said = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            crossing.access(|_| crossing.access(|n| *n))
        }))
        .expect_err("and so does the crossing one, rather than hanging");
        assert!(message(&said).contains("already held by the same task"));
    }

    /// And the crossing shape really does cross, which is the capability the
    /// cheap one cannot have: `Arc<Local<T>>` is not `Send` and this is.
    #[test]
    fn the_crossing_shape_crosses() {
        let value = std::sync::Arc::new(Crossing::new(0_i64));
        let mut threads = Vec::new();
        for _ in 0..4 {
            let value = std::sync::Arc::clone(&value);
            threads.push(std::thread::spawn(move || {
                for _ in 0..1000 {
                    value.update(|n| n + 1);
                }
            }));
        }
        for thread in threads {
            thread.join().expect("no task failed");
        }
        assert_eq!(value.access(|n| *n), 4000);
    }

    fn message(said: &Box<dyn std::any::Any + Send>) -> &str {
        said.downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| said.downcast_ref::<&str>().copied())
            .unwrap_or("")
    }
}

/// One lock, whichever shape it got — what a door over **several** of them needs
/// ([ADR-065](../../../docs/specification/adr/adr-065.md)).
///
/// A door over two locks cannot be written against `Local` or `Crossing` by name:
/// which shape a value gets is decided per value, so one program can hold both,
/// and the two values a transfer names need not have got the same answer. What
/// the door needs of each is the same three things whichever shape it is, and
/// this says which three.
pub trait Door {
    /// What the lock holds.
    type Held;

    /// **Where this lock stands in the one order everybody takes them in.**
    ///
    /// Its address. Two threads that both want A and B take them in the same
    /// order because they compute the same two numbers, which is what makes a
    /// cycle impossible rather than unlikely (Part II, 12.3).
    fn ordering(&self) -> usize;

    /// Read in place, the way `access` does.
    fn reading<R>(&self, f: impl FnOnce(&Self::Held) -> R) -> R;

    /// Take the value out and put one back, the way `update` does — and hand a
    /// result outward, which is what lets two of these nest.
    fn taking<R>(&self, f: impl FnOnce(Self::Held) -> (Self::Held, R)) -> R;
}

impl<T> Door for Local<T> {
    type Held = T;

    fn ordering(&self) -> usize {
        self as *const Self as usize
    }

    #[track_caller]
    fn reading<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        self.read(f)
    }

    #[track_caller]
    fn taking<R>(&self, f: impl FnOnce(T) -> (T, R)) -> R {
        let old = match self.inner.try_borrow_mut() {
            Ok(mut held) => held.take().unwrap_or_else(|| emptied()),
            Err(_) => reentered(),
        };
        let (new, out) = f(old);
        match self.inner.try_borrow_mut() {
            Ok(mut held) => *held = Some(new),
            Err(_) => reentered(),
        }
        out
    }
}

impl<T> Door for Crossing<T> {
    type Held = T;

    fn ordering(&self) -> usize {
        self as *const Self as usize
    }

    #[track_caller]
    fn reading<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        self.access(f)
    }

    #[track_caller]
    fn taking<R>(&self, f: impl FnOnce(T) -> (T, R)) -> R {
        self.hold(|slot| {
            let old = slot.take().unwrap_or_else(|| emptied());
            let (new, out) = f(old);
            *slot = Some(new);
            out
        })
    }
}

/// **A handle on a lock is a lock**, so a door takes one without being told which
/// it was given ([ADR-065](../../../docs/specification/adr/adr-065.md) D1).
///
/// `SharedMut[T]` is a count around a lock and `Locked[T]` is the lock in a
/// field; the door is written once and both reach it. The **order** delegates to
/// the lock inside rather than to the handle, because two handles on one
/// allocation must compute the same number or the order they promise is not one.
impl<D: Door> Door for std::rc::Rc<D> {
    type Held = D::Held;

    fn ordering(&self) -> usize {
        (**self).ordering()
    }

    #[track_caller]
    fn reading<R>(&self, f: impl FnOnce(&Self::Held) -> R) -> R {
        (**self).reading(f)
    }

    #[track_caller]
    fn taking<R>(&self, f: impl FnOnce(Self::Held) -> (Self::Held, R)) -> R {
        (**self).taking(f)
    }
}

impl<D: Door> Door for std::sync::Arc<D> {
    type Held = D::Held;

    fn ordering(&self) -> usize {
        (**self).ordering()
    }

    #[track_caller]
    fn reading<R>(&self, f: impl FnOnce(&Self::Held) -> R) -> R {
        (**self).reading(f)
    }

    #[track_caller]
    fn taking<R>(&self, f: impl FnOnce(Self::Held) -> (Self::Held, R)) -> R {
        (**self).taking(f)
    }
}

/// **Both locks read, both held at once, and taken in one global order**
/// (Part II 12.3, [ADR-065](../../../docs/specification/adr/adr-065.md) D2).
///
/// The order is by address and not by the order they are written, which is the
/// whole point: `access_all(a, b)` in one task and `access_all(b, a)` in another
/// take them the same way round, so there is no cycle to deadlock in. Nesting
/// them by hand is what the language refuses (Part II, 12.3).
#[track_caller]
pub fn access_all<A, B, R>(a: &A, b: &B, f: impl FnOnce(&A::Held, &B::Held) -> R) -> R
where
    A: Door,
    B: Door,
{
    match a.ordering() <= b.ordering() {
        true => a.reading(|x| b.reading(|y| f(x, y))),
        false => b.reading(|y| a.reading(|x| f(x, y))),
    }
}

/// **Both locks written, one new value each**
/// ([ADR-065](../../../docs/specification/adr/adr-065.md) D2).
///
/// `update`'s rule, widened: the old values go in by value and the new ones come
/// back as a pair, so no lambda is handed anything it may change and nothing sees
/// either lock between the two writes. The same address order as above.
#[track_caller]
pub fn update_all<A, B>(a: &A, b: &B, f: impl FnOnce(A::Held, B::Held) -> (A::Held, B::Held))
where
    A: Door,
    B: Door,
{
    match a.ordering() <= b.ordering() {
        true => a.taking(|x| {
            let held = b.taking(|y| {
                let (x, y) = f(x, y);
                (y, x)
            });
            (held, ())
        }),
        false => b.taking(|y| {
            let held = a.taking(|x| {
                let (x, y) = f(x, y);
                (x, y)
            });
            (held, ())
        }),
    }
}

#[cfg(test)]
mod doors {
    use super::*;

    /// Chapter 12's own transfer, which the language could not write before
    /// ([ADR-065](../../../docs/specification/adr/adr-065.md)).
    #[test]
    fn a_transfer_moves_between_two_locks() {
        let a = Local::new(100i64);
        let b = Local::new(5i64);
        update_all(&a, &b, |from, to| (from - 30, to + 30));
        assert_eq!(a.get(), 70);
        assert_eq!(b.get(), 35);
    }

    /// **The order is the addresses', not the arguments'**, which is what makes
    /// two tasks that name them the other way round safe.
    #[test]
    fn the_two_orders_are_the_same_order() {
        let a = Crossing::new(1i64);
        let b = Crossing::new(2i64);
        assert_eq!(access_all(&a, &b, |x, y| *x * 10 + *y), 12);
        assert_eq!(access_all(&b, &a, |y, x| *x * 10 + *y), 12);
    }

    /// And two threads doing the transfer both ways round neither deadlock nor
    /// lose a penny, which is the property `access_all` exists for.
    ///
    /// **Deliberately short.** A deadlock is a hang whatever the count, and the
    /// total is exact at any of them - so the number is chosen to prove the
    /// property without loading the machine, because two timing-sensitive I/O
    /// tests share this binary and a stress test beside them is a flake they did
    /// not have.
    #[test]
    fn two_threads_transferring_both_ways_keep_the_total() {
        let a = std::sync::Arc::new(Crossing::new(1_000i64));
        let b = std::sync::Arc::new(Crossing::new(1_000i64));
        let (one, two) = (std::sync::Arc::clone(&a), std::sync::Arc::clone(&b));
        let left = std::thread::spawn(move || {
            for _ in 0..400 {
                update_all(&*one, &*two, |x, y| (x - 1, y + 1));
            }
        });
        let (one, two) = (std::sync::Arc::clone(&a), std::sync::Arc::clone(&b));
        let right = std::thread::spawn(move || {
            for _ in 0..400 {
                update_all(&*two, &*one, |y, x| (y - 1, x + 1));
            }
        });
        left.join().expect("the left task");
        right.join().expect("the right task");
        assert_eq!(a.get() + b.get(), 2_000, "nothing is created or lost");
    }

    /// A lock of one type beside a lock of another, which is the case a door
    /// written against one shape could not have.
    #[test]
    fn the_two_locks_need_not_hold_the_same_type() {
        let name = Local::new("kasse".to_string());
        let count = Local::new(3i64);
        let said = access_all(&name, &count, |n, c| format!("{n} {c}"));
        assert_eq!(said, "kasse 3");
    }
}
