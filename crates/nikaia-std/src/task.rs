//! The one place a Nikaia program runs two things at once.
//!
//! The emitter names `task::both` and nothing else, so which vehicle carries
//! an overlap is this file's decision rather than a shape baked into every
//! generated program (ADR-033 §8.4).

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
