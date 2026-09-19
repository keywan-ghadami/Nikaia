//! The one place a Nikaia program runs two things at once.
//!
//! The emitter names what is in this module and nothing else, so which vehicle
//! carries an overlap is this file's decision rather than a shape baked into
//! every generated program (ADR-033 §8.4).
//!
//! **Two things a program can ask for, and they are different questions:**
//!
//! * [`TaskHandle`] is a `spawn` - one piece of work the executor owns, joined
//!   later or not at all ([ADR-055](../../../docs/specification/adr/adr-055.md)
//!   D5).
//! * `overlap2`..`overlap8` are an `overlap { … }` block - every branch in
//!   flight in one pass, one function per arity
//!   ([ADR-050](../../../docs/specification/adr/adr-050.md) D2).
//! * `race2`..`race8` are a `select { … }` block - the same branches in flight,
//!   and the **first** to finish is the one that is kept
//!   ([ADR-148](../../../docs/specification/adr/adr-148.md) D1). The pair D4
//!   names, one module apart from nothing.
//!
//! Both take **futures**, because Rust has no stable `async` closure and a
//! branch that pauses is the case they exist for. What ran on the pool
//! instead — `both`, the closure pair ADR-033 D10 chose for a group the
//! *compiler* put together — went with the automatic grouping itself
//! (ADR-050 D1).

/// **What a `spawn` hands back** (Part I 8.2,
/// [ADR-055](../../../docs/specification/adr/adr-055.md) D5).
///
/// ```nika
/// let handle = spawn fn { process(path) }
/// let result = handle.join()
/// ```
///
/// `.join()` is where two tasks meet again, and it is an `.await`: the task
/// fills a slot and wakes whoever is waiting on it, so joining gives the thread
/// up rather than holding it. That is what makes Part II 11.2's *"uniform API"*
/// true - the same two lines mean the same thing at either setting of
/// `user_parallelism`, and the only difference is how many threads the executor
/// has.
///
/// **A task nobody joins still runs** (D5), which Part I 8.2's own example
/// needs: the executor owns the task, so dropping the handle drops the handle
/// and not the work.
pub struct TaskHandle<T> {
    slot: std::sync::Arc<crate::rt::exec::Slot<T>>,
    asked: std::sync::Arc<Cancelled>,
}

/// **The request a [`TaskHandle::cancel`] makes**
/// ([ADR-148](../../../docs/specification/adr/adr-148.md) D3).
///
/// A flag and the waker of whoever is running the task, which is what makes the
/// request *prompt* rather than eventual: without the waker a task parked on
/// I/O would sit in the queue until that I/O answered, and only then notice it
/// had been cancelled.
struct Cancelled {
    asked: std::sync::atomic::AtomicBool,
    waking: std::sync::Mutex<Option<std::task::Waker>>,
}

impl Cancelled {
    fn nobody_asked() -> std::sync::Arc<Cancelled> {
        std::sync::Arc::new(Cancelled {
            asked: std::sync::atomic::AtomicBool::new(false),
            waking: std::sync::Mutex::new(None),
        })
    }
}

/// A task's body, wrapped in what makes it cancellable.
///
/// **The body is held in an `Option` so that it can be dropped early**, and
/// dropping it is the whole of D2: a future dropped at its suspension point
/// tears its values down, a `cleanup` that pauses is adopted by the runtime and
/// bounded by the `cleanup-deadline`
/// ([ADR-006](../../../docs/specification/adr/adr-006.md) D3), and nobody waits
/// for any of it.
struct Cancellable<F, T> {
    body: Option<std::pin::Pin<Box<F>>>,
    slot: std::sync::Arc<crate::rt::exec::Slot<T>>,
    asked: std::sync::Arc<Cancelled>,
}

impl<F: std::future::Future<Output = T>, T> std::future::Future for Cancellable<F, T> {
    type Output = ();

    fn poll(
        self: std::pin::Pin<&mut Self>,
        context: &mut std::task::Context<'_>,
    ) -> std::task::Poll<()> {
        let me = self.get_mut();
        if me.asked.asked.load(std::sync::atomic::Ordering::Acquire) {
            // **The teardown is the drop**, and it happens here rather than in
            // `cancel` because this is the task's own thread and its pause
            // point.
            me.body = None;
            return std::task::Poll::Ready(());
        }
        let Some(body) = me.body.as_mut() else {
            return std::task::Poll::Ready(());
        };
        match body.as_mut().poll(context) {
            std::task::Poll::Ready(value) => {
                me.body = None;
                me.slot.fill(value);
                std::task::Poll::Ready(())
            }
            std::task::Poll::Pending => {
                *me.asked.waking.lock().unwrap_or_else(|e| e.into_inner()) =
                    Some(context.waker().clone());
                std::task::Poll::Pending
            }
        }
    }
}

impl<T: 'static> TaskHandle<T> {
    /// Start `body` as a task, and hand back the handle to its value.
    ///
    /// **The captures have already moved**, because the emitter writes the body
    /// as an `async move` block - which is Part I 8.3's implicit move and Rust's
    /// `move` meeting at the same place, with `NK2101` in front of it for the
    /// data the parent still wanted.
    pub fn start(body: impl std::future::Future<Output = T> + 'static) -> TaskHandle<T> {
        let slot = crate::rt::exec::Slot::empty();
        let asked = Cancelled::nobody_asked();
        crate::rt::exec::start(Cancellable {
            body: Some(Box::pin(body)),
            slot: slot.clone(),
            asked: asked.clone(),
        });
        TaskHandle { slot, asked }
    }
}

impl<T> TaskHandle<T> {
    /// **Stop the task** ([ADR-148](../../../docs/specification/adr/adr-148.md)
    /// D3), with exactly the semantics losing a `select` has.
    ///
    /// The task stops at its current pause point, its values are torn down, and
    /// a `cleanup` that pauses is adopted by the runtime and finished in the
    /// background. The caller does not wait for any of that, because it should
    /// not pay for it (D2).
    ///
    /// **It takes the handle**, the way `join` does, and that is what makes
    /// §4's open question — *can a cancelled task be observed to have been
    /// cancelled?* — one no program can ask: after this there is no handle to
    /// ask with. It also means a cancelled task is never joined, so nothing
    /// waits for a value that is not coming.
    pub fn cancel(self) {
        self.asked
            .asked
            .store(true, std::sync::atomic::Ordering::Release);
        // **Wake it so that it notices now.** A task parked on I/O has a clear
        // alarm, and the executor polls what its alarm says is ready.
        let waking = self
            .asked
            .waking
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take();
        if let Some(waker) = waking {
            waker.wake();
        }
        // And the thread that is not this one, at `user_parallelism = yes`.
        crate::rt::ring_the_bell();
    }
}

impl<T: Send + 'static> TaskHandle<T> {
    /// The same, on the **pool**, where another thread may pick the task up
    /// ([ADR-055](../../../docs/specification/adr/adr-055.md) §6 step 1's `yes`
    /// half, [ADR-037](../../../docs/specification/adr/adr-037.md) D2).
    ///
    /// **Two functions and not one with a bound**, because the bound is the
    /// difference and it belongs to the build rather than to the language. The
    /// emitter writes this line at `user_parallelism = yes` and [`start`] at
    /// `no`, from one Nikaia `spawn` — so a program at the default is never
    /// asked for a `Send` its setting does not need, which is what keeps
    /// [ADR-061](../../../docs/specification/adr/adr-061.md) D1's plain count
    /// reachable from inside a task.
    ///
    /// [`start`]: TaskHandle::start
    pub fn start_on_pool(
        body: impl std::future::Future<Output = T> + Send + 'static,
    ) -> TaskHandle<T> {
        let slot = crate::rt::exec::Slot::empty();
        let asked = Cancelled::nobody_asked();
        crate::rt::exec::start_on_pool(Cancellable {
            body: Some(Box::pin(body)),
            slot: slot.clone(),
            asked: asked.clone(),
        });
        TaskHandle { slot, asked }
    }

    /// The task's value, once it has one. **A suspension point**, not a wait.
    pub async fn join(self) -> T {
        crate::rt::exec::Waiting::on(self.slot).await
    }
}

/// **Every branch of an `overlap { … }`, all of them in flight**
/// (Part I 8.1.2, [ADR-050](../../../docs/specification/adr/adr-050.md) D2).
///
/// One arity per macro expansion, because the branches have different types and
/// a tuple of futures is what that means in the language below. The emitter
/// writes `task::overlap3(a, b, c).await` and the arity is how many statements
/// the block had.
///
/// **Flat and not nested, which is D6.** A pair joined with a pair -
/// `two(a, two(b, c))` - polls `a` to completion before `b` is ever started, so
/// a block holding a computation and two reads would cost their sum: the naive
/// order D6 names and refuses. Polling every branch in one pass costs `max` instead: a branch that
/// suspends returns `Pending` at its first suspension point and the next branch
/// is started at once.
///
/// **The order it polls in is the order it is given**, and the emitter hands the
/// branches over with the ones that can pause first — which is D6's rule, read
/// off the ledger's `sync` column. The results go back into written order at the
/// call, so this never has to know about it.
///
/// **A failure is the caller's**, not this function's: a branch that can fail
/// hands back a `Result` like any other value, and D5's "the first in written
/// order wins" is the emitter's `?` on the tuple rather than a rule here.
macro_rules! overlapping {
    ($name:ident, $($branch:ident : $result:ident),+) => {
        // One parameter per branch is what an arity *is*, so the argument count
        // is the point rather than a smell: `overlap8` takes eight branches
        // because a block of eight has eight.
        #[allow(non_snake_case, clippy::too_many_arguments)]
        pub async fn $name<$($branch, $result),+>($($branch: $branch),+) -> ($($result),+)
        where
            $($branch: std::future::Future<Output = $result>),+
        {
            $(let mut $branch = Box::pin($branch);)+
            $(let mut $result: Option<$result> = None;)+

            std::future::poll_fn(move |context| {
                $(
                    if $result.is_none() {
                        if let std::task::Poll::Ready(value) = $branch.as_mut().poll(context) {
                            $result = Some(value);
                        }
                    }
                )+
                if $($result.is_some())&&+ {
                    return std::task::Poll::Ready((
                        $($result.take().expect("checked just above")),+
                    ));
                }
                // No waker is rung, for the reason `interleave` gives: what a
                // branch waits for is the I/O, and the executor is the only
                // thing on this thread that parks.
                std::task::Poll::Pending
            })
            .await
        }
    };
}

overlapping!(overlap2, A: RA, B: RB);
overlapping!(overlap3, A: RA, B: RB, C: RC);
overlapping!(overlap4, A: RA, B: RB, C: RC, D: RD);
overlapping!(overlap5, A: RA, B: RB, C: RC, D: RD, E: RE);
overlapping!(overlap6, A: RA, B: RB, C: RC, D: RD, E: RE, F: RF);
overlapping!(overlap7, A: RA, B: RB, C: RC, D: RD, E: RE, F: RF, G: RG);
overlapping!(overlap8, A: RA, B: RB, C: RC, D: RD, E: RE, F: RF, G: RG, H: RH);

/// **Which arm of a `select { … }` won** (Part II 12.4,
/// [ADR-148](../../../docs/specification/adr/adr-148.md) D1).
///
/// One enum per arity, for the reason `overlap` has one function per arity: the
/// arms have different types, and in the language below that is what a sum of
/// them means. The variants are **ordinals** and not letters, so the generated
/// `match` reads as the source does — `Race2::Second(_)` is the second arm of
/// the block (Part III C.1).
///
/// **The losers are dropped**, and that is D2 rather than an implementation
/// detail: a future dropped at a suspension point tears its values down, a
/// `cleanup` that pauses is adopted by the runtime and bounded by the
/// `cleanup-deadline` ([ADR-006](../../../docs/specification/adr/adr-006.md)
/// D3), and the winner does not wait for any of it. The language below drops
/// the losing futures when `race<n>` returns, so the mechanism is the one this
/// runtime already had.
macro_rules! racing {
    ($name:ident, $won:ident, $($branch:ident : $result:ident : $variant:ident),+) => {
        #[derive(Debug)]
        pub enum $won<$($result),+> {
            $($variant($result)),+
        }

        // One parameter per arm is what an arity *is*, the same way an
        // `overlap`'s is.
        #[allow(non_snake_case, clippy::too_many_arguments)]
        pub async fn $name<$($branch, $result),+>($($branch: $branch),+) -> $won<$($result),+>
        where
            $($branch: std::future::Future<Output = $result>),+
        {
            $(let mut $branch = Box::pin($branch);)+

            std::future::poll_fn(move |context| {
                // **In written order, and the first that is ready wins.** Two
                // arms ready in the same pass is a tie, and the written order
                // is what breaks it - which is the same rule
                // [ADR-050](../../../docs/specification/adr/adr-050.md) D5 uses
                // for an `overlap`'s failures, said about a value instead.
                $(
                    if let std::task::Poll::Ready(value) = $branch.as_mut().poll(context) {
                        return std::task::Poll::Ready($won::$variant(value));
                    }
                )+
                // No waker is rung, for the reason `overlap` gives: what a
                // branch waits for is the I/O or the clock, and the executor is
                // the only thing on this thread that parks.
                std::task::Poll::Pending
            })
            .await
        }
    };
}

racing!(race2, Race2, A: RA: First, B: RB: Second);
racing!(race3, Race3, A: RA: First, B: RB: Second, C: RC: Third);
racing!(race4, Race4, A: RA: First, B: RB: Second, C: RC: Third, D: RD: Fourth);
racing!(
    race5, Race5, A: RA: First, B: RB: Second, C: RC: Third, D: RD: Fourth, E: RE: Fifth
);
racing!(
    race6, Race6, A: RA: First, B: RB: Second, C: RC: Third, D: RD: Fourth, E: RE: Fifth,
    F: RF: Sixth
);
racing!(
    race7, Race7, A: RA: First, B: RB: Second, C: RC: Third, D: RD: Fourth, E: RE: Fifth,
    F: RF: Sixth, G: RG: Seventh
);
racing!(
    race8, Race8, A: RA: First, B: RB: Second, C: RC: Third, D: RD: Fourth, E: RE: Fifth,
    F: RF: Sixth, G: RG: Seventh, H: RH: Eighth
);

/// One half of a [`crate::fs::read_both`], finished as `fs::read_to_string`
/// would have finished it.
///
/// `fs::read_to_string` is a read and then a UTF-8 check, and the pair performs
/// the read - so this is the check, on exactly the bytes that came back,
/// reporting exactly the failure the sequential program would have reported. It
/// is `std`'s own function and not a line written twice into every program that
/// reads two files at once.
pub fn as_text(bytes: Result<Vec<u8>, std::io::Error>) -> Result<String, std::io::Error> {
    crate::fs::text(bytes?)
}
