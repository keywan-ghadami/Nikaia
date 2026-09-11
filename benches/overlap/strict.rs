//! The `--ordering strict` shape: two reads, one after the other.
//!
//! Reduced from the emitter's output for `TWO_READS` (crates/nikaia/tests/
//! ordering.rs) to plain std, so a stock `rustc` can build it. See run.sh.

use std::fs;
fn once() -> usize {
    let a = match fs::read_to_string("eins.txt") { Ok(v) => v, Err(_) => String::new() };
    let b = match fs::read_to_string("zwei.txt") { Ok(v) => v, Err(_) => String::new() };
    a.len() + b.len()
}
fn main() {
    let n: usize = std::env::args().nth(1).unwrap().parse().unwrap();
    let t = std::time::Instant::now();
    let mut acc = 0usize;
    for _ in 0..n { acc = acc.wrapping_add(once()); }
    println!("strict  {:>10.3} ms  ({acc})", t.elapsed().as_secs_f64()*1000.0);
}
