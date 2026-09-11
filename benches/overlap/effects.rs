//! The `--ordering effects` shape: the same two reads under `thread::scope`.
//!
//! Reduced from the emitter's output for `TWO_READS` (crates/nikaia/tests/
//! ordering.rs) to plain std, so a stock `rustc` can build it. The scope, the
//! two spawns and the `resume_unwind` on join are what the emitter writes; only
//! `nikaia_std::prelude` and the `catch` arm's binding are gone. See run.sh.

use std::fs;
fn once() -> usize {
    let (a, b) = std::thread::scope(|scope| {
        let first = scope.spawn(|| match fs::read_to_string("eins.txt") { Ok(v) => v, Err(_) => String::new() });
        let second = scope.spawn(|| match fs::read_to_string("zwei.txt") { Ok(v) => v, Err(_) => String::new() });
        (first.join().unwrap_or_else(|p| std::panic::resume_unwind(p)),
         second.join().unwrap_or_else(|p| std::panic::resume_unwind(p)))
    });
    a.len() + b.len()
}
fn main() {
    let n: usize = std::env::args().nth(1).unwrap().parse().unwrap();
    let t = std::time::Instant::now();
    let mut acc = 0usize;
    for _ in 0..n { acc = acc.wrapping_add(once()); }
    println!("effects {:>10.3} ms  ({acc})", t.elapsed().as_secs_f64()*1000.0);
}
