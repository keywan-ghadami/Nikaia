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
    inner: RefCell<T>,
}

impl<T> Local<T> {
    pub fn new(value: T) -> Self {
        Self {
            inner: RefCell::new(value),
        }
    }

    /// Part II 12.2: lock, compute, unlock — and the lambda runs straight
    /// through, which is checked before this is ever reached.
    #[track_caller]
    pub fn access<R>(&self, f: impl FnOnce(&mut T) -> R) -> R {
        match self.inner.try_borrow_mut() {
            Ok(mut held) => f(&mut held),
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
    inner: Mutex<T>,
    /// Which task holds it, or `None`. Read before the acquisition and written
    /// after it, which is why a re-entry is seen *before* the acquisition blocks:
    /// once it blocks there is nothing left to report to.
    held_by: Mutex<Option<u64>>,
}

impl<T> Crossing<T> {
    pub fn new(value: T) -> Self {
        Self {
            inner: Mutex::new(value),
            held_by: Mutex::new(None),
        }
    }

    #[track_caller]
    pub fn access<R>(&self, f: impl FnOnce(&mut T) -> R) -> R {
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

    #[test]
    fn both_shapes_hold_and_release() {
        let local = Local::new(1_i64);
        assert_eq!(local.access(|n| *n + 1), 2);
        local.access(|n| *n = 7);
        assert_eq!(local.access(|n| *n), 7);

        let crossing = Crossing::new(1_i64);
        assert_eq!(crossing.access(|n| *n + 1), 2);
        crossing.access(|n| *n = 7);
        assert_eq!(crossing.access(|n| *n), 7);
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
                    value.access(|n| *n += 1);
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
