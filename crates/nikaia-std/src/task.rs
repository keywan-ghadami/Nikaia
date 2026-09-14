//! The one place a Nikaia program runs two things at once.
//!
//! The emitter names what is in this module and nothing else, so which vehicle
//! carries an overlap is this file's decision rather than a shape baked into
//! every generated program (ADR-033 §8.4).
//!
//! **Two vehicles, and the difference is whose code is in flight**
//! (ADR-033 D10):
//!
//! * [`both`] puts each statement in a closure and runs the pair on the pool.
//!   Two pieces of **user** code are then in flight, so it exists only at
//!   `user_parallelism = yes` (ADR-037 D2), and it costs a thread wake-up -
//!   ~59 µs a pair, which no user-space vehicle removes (ADR-038 §4.3).
//! * [`read_pair`] hands two reads to the runtime, which is already running.
//!   No thread carries either of them: the kernel performs both reads and
//!   `std` does the waiting. So it exists at **both** settings, and it costs
//!   nothing measurable.
//!
//! Which of the two a pair of statements gets is decided in the emitter, which
//! is where the build switches live; what each one *is* is decided here.

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
        let filling = slot.clone();
        crate::rt::exec::start(async move {
            filling.fill(body.await);
        });
        TaskHandle { slot }
    }

    /// The task's value, once it has one. **A suspension point**, not a wait.
    pub async fn join(self) -> T {
        crate::rt::exec::Waiting::on(self.slot).await
    }
}

/// Run both closures, return both results, keep both panics.
///
/// **Why the pool and not a fresh thread.** `std::thread::scope` reads more
/// obviously and was what the first lowering emitted. It is wrong for the
/// programs ADR-033 exists for: a server that overlaps two reads per request
/// and serves a thousand at once asks the OS for two thousand threads, and
/// nothing in the lowering bounds that. `rayon::join` hands the work to the
/// pool a Nikaia program already has - `fs::map` chunks its UTF-8 check across
/// it and the parallel piece driver runs on it - so the ceiling is the pool's
/// and a program has exactly one answer to "how parallel am I".
///
/// It is also measurably cheaper, though that is the smaller reason:
/// `benches/overlap/` puts the fixed cost of a pair at ~98 µs for
/// `thread::scope` and ~48 µs here. Both are far above a page-cached
/// `fs::read`, which is why ADR-033 §8.2 calls the overlap a pessimisation
/// below roughly a quarter megabyte either way.
///
/// **Panics.** `rayon::join` propagates a panic from either closure into the
/// caller, which is what the sequential program did: the panic reaches the
/// same place it would have without the overlap.
pub fn both<A, B, RA, RB>(a: A, b: B) -> (RA, RB)
where
    A: FnOnce() -> RA + Send,
    B: FnOnce() -> RB + Send,
    RA: Send,
    RB: Send,
{
    rayon::join(a, b)
}

/// **Both futures in flight at once, on this one thread**
/// ([ADR-055](../../../docs/specification/adr/adr-055.md) D1, Part II 11.2).
///
/// [`both`]'s twin for a pair whose halves can **pause**. The pool cannot carry
/// one: `rayon::join` takes closures, Rust has no stable `async` closure, and a
/// plain closure holding an `.await` does not compile. What it takes instead is
/// two futures - which is what an `async` *block* is, and those are stable.
///
/// **And this is Part II 11.2's sentence rather than a way round a limitation.**
/// *"Interleaved on the same thread"* is exactly what happens here: both halves
/// are polled, whichever suspends gives the thread to the other, and neither
/// waits for the other to finish. At `user_parallelism = no` there is no pool to
/// want - and the pair costs no thread wake-up at all, where [`both`]'s costs
/// ~48 µs (`benches/overlap/`).
///
/// Polled in the order they were written, on every round, so a pair whose halves
/// both finish at once finishes in the order the sequential program would have.
///
/// **Panics.** A panic in either half unwinds into the caller, which is where
/// the sequential program's panic went - the same claim [`both`] makes, and here
/// it needs no arrangement: there is no other thread for it to be on.
pub async fn interleave<A, B, RA, RB>(a: A, b: B) -> (RA, RB)
where
    A: std::future::Future<Output = RA>,
    B: std::future::Future<Output = RB>,
{
    // Pinned on the heap rather than with `pin!`, because `poll_fn`'s closure
    // owns them and a stack pin cannot be moved into one. One allocation a
    // pair, against the ~48 µs a pool hand-off costs.
    let mut a = Box::pin(a);
    let mut b = Box::pin(b);
    let mut first: Option<RA> = None;
    let mut second: Option<RB> = None;

    std::future::poll_fn(move |context| {
        if first.is_none() {
            if let std::task::Poll::Ready(value) = a.as_mut().poll(context) {
                first = Some(value);
            }
        }
        if second.is_none() {
            if let std::task::Poll::Ready(value) = b.as_mut().poll(context) {
                second = Some(value);
            }
        }
        match (first.is_some(), second.is_some()) {
            (true, true) => std::task::Poll::Ready((
                first.take().expect("checked just above"),
                second.take().expect("checked just above"),
            )),
            // **No waker is rung here**, and that is deliberate: what the halves
            // are waiting for is the I/O, and the executor is the only thing on
            // this thread that parks - it parks in the I/O and re-arms
            // everything it holds (`rt::io::Reading`, `rt::exec::block_on`).
            _ => std::task::Poll::Pending,
        }
    })
    .await
}

/// **Every branch of an `overlap { … }`, all of them in flight**
/// (Part I 8.1.2, [ADR-050](../../../docs/specification/adr/adr-050.md) D2).
///
/// One arity per macro expansion, because the branches have different types and
/// a tuple of futures is what that means in the language below. The emitter
/// writes `task::overlap3(a, b, c).await` and the arity is how many statements
/// the block had.
///
/// **Flat and not nested, which is D6.** `interleave(a, interleave(b, c))` polls
/// `a` to completion before `b` is ever started, so a block holding a
/// computation and two reads would cost their sum — the naive order D6 names and
/// refuses. Polling every branch in one pass costs `max` instead: a branch that
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

/// Two file reads the **compiler** put together, both in flight where that is
/// free (ADR-033 D10).
///
/// The vehicle for a statement pair at *either* setting of
/// `user_parallelism`, and the reason it is legitimate at `no` is that nothing
/// the user wrote is in flight twice: the two reads are `std`'s own operations,
/// the kernel performs them, and the thread the program is on is the one
/// waiting for both (ADR-037 D2, ADR-038 D3). There is no closure here and
/// therefore nothing of the program's to run anywhere else - which
/// `nikaia_std::rt`'s inbox enforces rather than promises, since no variant of
/// an `Op` can carry code.
///
/// **It overlaps only where overlapping is free.** Where the machine has no
/// completion queue the two reads are performed in the order they were
/// written: the fallback's ~38 µs a pair is the fixed tax ADR-033 §8.2 called
/// a pessimisation, and a pair the program did not ask for may not pay it.
/// `fs::read_both` is the function for a program that *did* ask.
///
/// The answers come back in the order the paths were given, whichever route
/// they took, so a program prints what the sequential program printed (D6).
pub fn read_pair(
    a: impl AsRef<std::path::Path>,
    b: impl AsRef<std::path::Path>,
) -> (
    Result<Vec<u8>, std::io::Error>,
    Result<Vec<u8>, std::io::Error>,
) {
    crate::rt::io::read_pair(a.as_ref(), b.as_ref())
}

/// One half of a [`read_pair`], finished as `fs::read_to_string` would have
/// finished it.
///
/// `fs::read_to_string` is a read and then a UTF-8 check, and the pair above
/// performs the read - so this is the check, on exactly the bytes that came
/// back, reporting exactly the failure the sequential program would have
/// reported. It is `std`'s own function and not a line the emitter writes
/// twice into every program that overlaps two reads.
pub fn as_text(bytes: Result<Vec<u8>, std::io::Error>) -> Result<String, std::io::Error> {
    crate::fs::text(bytes?)
}
