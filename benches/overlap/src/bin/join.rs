#![allow(clippy::manual_unwrap_or_default)]
//! (The `match` is what the emitter writes for `catch`. Collapsing it to
//! `unwrap_or_default` would measure a program this compiler does not produce.)

//! What the emitter produces today: `nikaia_std::task::both`, which is
//! `rayon::join` on the pool the program already has.

use std::fs;

fn once() -> usize {
    let (a, b) = rayon::join(
        || match fs::read_to_string("eins.txt") {
            Ok(v) => v,
            Err(_) => String::new(),
        },
        || match fs::read_to_string("zwei.txt") {
            Ok(v) => v,
            Err(_) => String::new(),
        },
    );
    a.len() + b.len()
}

fn main() {
    let n: usize = std::env::args().nth(1).unwrap().parse().unwrap();
    once(); // the pool exists in a running program; starting it is not the cost
    let t = std::time::Instant::now();
    let mut acc = 0usize;
    for _ in 0..n {
        acc = acc.wrapping_add(once());
    }
    println!("join {:.3} {acc}", t.elapsed().as_secs_f64() * 1000.0);
}
