#![allow(clippy::manual_unwrap_or_default)]
//! (The `match` is what the emitter writes for `catch`. Collapsing it to
//! `unwrap_or_default` would measure a program this compiler does not produce.)

//! The `--ordering strict` shape: two reads, one after the other.
//!
//! Reduced from the emitter's output for `TWO_READS` (crates/nikaia/tests/
//! ordering.rs); this is the baseline every other binary here is measured
//! against. See README.md.

use std::fs;
fn once() -> usize {
    let a = match fs::read_to_string("eins.txt") {
        Ok(v) => v,
        Err(_) => String::new(),
    };
    let b = match fs::read_to_string("zwei.txt") {
        Ok(v) => v,
        Err(_) => String::new(),
    };
    a.len() + b.len()
}
fn main() {
    let n: usize = std::env::args().nth(1).unwrap().parse().unwrap();
    let t = std::time::Instant::now();
    let mut acc = 0usize;
    for _ in 0..n {
        acc = acc.wrapping_add(once());
    }
    println!("strict {:.3} {acc}", t.elapsed().as_secs_f64() * 1000.0);
}
