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
