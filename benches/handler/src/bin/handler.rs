//! What the **boxed future** costs a handler that does not pause
//! ([ADR-122](../../../../docs/specification/adr/adr-122.md) D3).
//!
//! D1 makes a parameter whose type may pause lower to a closure returning a
//! boxed future — `impl Fn(A) -> Pin<Box<dyn Future<Output = R>>>` — whether the
//! callee runs it or keeps it, so that a reader can tell what a signature costs
//! by reading it. A caller who hands such a parameter a lambda that does **not**
//! pause pays a heap allocation and a dynamic call it did not before, and D3
//! says that number is measured rather than assumed.
//!
//! | row | what it is |
//! |---|---|
//! | plain closure | what `fn(i64) -> i64 sync` lowers to: `impl Fn(i64) -> i64` |
//! | boxed future | what `fn(i64) -> i64` lowers to, awaited by the same executor a program uses |
//! | plain closure, twice | the control. It must tie, and it bounds the difference above from below |
//!
//! ```sh
//! cargo run -p handler-bench --release --bin handler
//! ```
//!
//! **What it is not.** It is not a claim about a handler that *does* pause:
//! there the future is the only shape there is, so there is nothing to compare
//! it against. The cost measured here is the one D1 imposes on the case that
//! did not need it.

use std::future::Future;
use std::hint::black_box;
use std::pin::Pin;
use std::time::Instant;

/// The shape `fn(i64) -> i64 sync` lowers to.
fn run_plain(n: i64, f: impl Fn(i64) -> i64) -> i64 {
    let mut total: i64 = 0;
    for i in 0..n {
        total = total.wrapping_add(f(i));
    }
    total
}

/// The shape `fn(i64) -> i64` lowers to, and the call the callee writes for it.
async fn run_future(n: i64, f: impl Fn(i64) -> Pin<Box<dyn Future<Output = i64>>>) -> i64 {
    let mut total: i64 = 0;
    for i in 0..n {
        total = total.wrapping_add(f(i).await);
    }
    total
}

fn nanos_each(took: std::time::Duration, n: i64) -> f64 {
    took.as_secs_f64() * 1e9 / n as f64
}

fn main() {
    // Large enough that the loop dominates the timing call, small enough to
    // stay in cache: the difference being measured is an allocation per call.
    const N: i64 = 2_000_000;
    const REPEATS: usize = 5;

    let mut plain = f64::MAX;
    let mut future = f64::MAX;
    let mut control = f64::MAX;

    for _ in 0..REPEATS {
        let began = Instant::now();
        let got = run_plain(N, |i| black_box(i).wrapping_mul(3));
        black_box(got);
        plain = plain.min(nanos_each(began.elapsed(), N));

        let began = Instant::now();
        let got = nikaia_std::rt::exec::block_on(run_future(N, |i| {
            Box::pin(async move { black_box(i).wrapping_mul(3) })
        }));
        black_box(got);
        future = future.min(nanos_each(began.elapsed(), N));

        let began = Instant::now();
        let got = run_plain(N, |i| black_box(i).wrapping_mul(3));
        black_box(got);
        control = control.min(nanos_each(began.elapsed(), N));
    }

    // The best of the repeats, which is the row least disturbed by whatever
    // else the machine was doing - the same choice `benches/lockfree` makes.
    println!("plain closure          {plain:6.2} ns/call");
    println!(
        "boxed future           {future:6.2} ns/call   ×{:.2}",
        future / plain
    );
    println!("plain closure, twice   {control:6.2} ns/call   (the control: it ties)");
}
