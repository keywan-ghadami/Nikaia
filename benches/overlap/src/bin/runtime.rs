//! What a pair of operations costs on the runtime that is already running.
//!
//! This is the measurement [ADR-038](../../../../docs/specification/adr/adr-038.md)
//! D4 predicts and [ADR-033](../../../../docs/specification/adr/adr-033.md) §8.5
//! wrote down as a prediction it could not time: an I/O thread that is already
//! running costs nothing per operation, where the vehicle §8.4 measured costs
//! a thread wake-up per pair - **46 µs, and no user-space vehicle removes it**.
//!
//! Three shapes over the same two files, so the difference between them is the
//! vehicle and nothing else:
//!
//! | shape | what it is |
//! |---|---|
//! | `seq` | one read, then the other, both through `std::fs` - the baseline |
//! | `join` | `nikaia_std::task::both`, which is `rayon::join`: §8.4's vehicle |
//! | `both` | `nikaia_std::fs::read_both`: both in flight on the pre-started runtime |
//!
//! The number that answers D4 is **`join - seq` against `both - seq`**: what
//! each vehicle adds for overlapping a pair whose payload is small enough that
//! the payload is not the answer. Every shape reads the same bytes through the
//! same `std`, so nothing but the vehicle differs.
//!
//! ```sh
//! benches/overlap/runtime.sh              # the table, over five file sizes
//! cargo run -p overlap-bench --release --bin runtime -- 5000 7
//! ```
//!
//! The second argument is how many times the whole measurement is repeated;
//! every repeat is printed, because a single number without its spread is not
//! a measurement (`benches/overlap/README.md`).

use nikaia_std::{fs, task};

const A: &str = "eins.txt";
const B: &str = "zwei.txt";

/// One read, then the other. What `--ordering strict` emits.
fn sequential() -> usize {
    let a = fs::read(A).unwrap_or_default();
    let b = fs::read(B).unwrap_or_default();
    a.len() + b.len()
}

/// ADR-033 §8.4's vehicle: two closures on the pool, one wake-up per pair.
fn joined() -> usize {
    let (a, b) = task::both(
        || fs::read(A).unwrap_or_default(),
        || fs::read(B).unwrap_or_default(),
    );
    a.len() + b.len()
}

/// ADR-038 D4's: both operations in flight on the runtime that was already
/// running, results collected in the order they were written.
fn both() -> usize {
    let (a, b) = fs::read_both(A, B);
    a.unwrap_or_default().len() + b.unwrap_or_default().len()
}

fn per_pair(n: usize, mut shape: impl FnMut() -> usize) -> (f64, usize) {
    let began = std::time::Instant::now();
    let mut acc = 0usize;
    for _ in 0..n {
        acc = acc.wrapping_add(shape());
    }
    (began.elapsed().as_secs_f64() * 1e6 / n as f64, acc)
}

fn main() {
    let mut args = std::env::args().skip(1);
    let n: usize = args
        .next()
        .unwrap_or_else(|| "5000".into())
        .parse()
        .unwrap();
    let repeats: usize = args.next().unwrap_or_else(|| "7".into()).parse().unwrap();

    // The runtime is started here, once, exactly as the `fn main` the emitter
    // writes starts it (D4). Nothing below pays for starting it - which is the
    // whole of what is being measured.
    let runtime = nikaia_std::rt::start(nikaia_std::rt::UserCode::Concurrent);
    println!("runtime: {}", nikaia_std::rt::handle().describe());

    // A warm pool and a warm page cache for every shape, so the first repeat
    // is not measuring a cold one.
    for _ in 0..64 {
        sequential();
        joined();
        both();
    }

    println!("repeat  seq(µs)  join(µs)  both(µs)  join-seq  both-seq");
    for repeat in 1..=repeats {
        let (seq, one) = per_pair(n, sequential);
        let (join, two) = per_pair(n, joined);
        let (pair, three) = per_pair(n, both);
        assert_eq!(one, two, "the shapes must read the same bytes");
        assert_eq!(two, three, "the shapes must read the same bytes");
        println!(
            "{repeat:>6}  {seq:>7.2}  {join:>8.2}  {pair:>8.2}  {:>8.2}  {:>8.2}",
            join - seq,
            pair - seq
        );
    }

    runtime.finish();
}
