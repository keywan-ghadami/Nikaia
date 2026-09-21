//! The table a `comptime` map crosses as
//! ([ADR-176](../../../docs/specification/adr/adr-176.md) D2, D3).
//!
//! **Two shapes under one type**, and which one a table gets was decided while
//! the program was built rather than by a branch in `std`:
//!
//! * **Under twelve keys**, no displacements. `Fixed::get` walks the keys with a
//!   length check, which [`docs/fixed-map-lookup.md`](../../../docs/fixed-map-lookup.md)
//!   §6 measured within 0 to 17 % of a generated `match` — past the point where
//!   the hash has already won, so the `match` this would otherwise have emitted
//!   buys nothing.
//! * **From twelve**, CHD: buckets by one part of the hash, a displacement pair
//!   per bucket, one slot per key. The lookup is one hash, one displacement, one
//!   slot and one compare.
//!
//! Twelve is where the two cross in that file, and the band is forgiving —
//! anywhere from eight to sixteen is within 30 % on the wrong side — which is
//! worth saying so nobody re-measures this to move it by two.

use std::collections::BTreeMap;

/// A key's place in an attempt: where it was written, and the two halves of its
/// hash the displacement works on.
type Placed = (usize, u32, u32);

/// Where a scan stops being as good as a hash
/// ([ADR-176](../../../docs/specification/adr/adr-176.md) D3).
pub const HASHED_FROM: usize = 12;

/// A table, ready to be written into a `const`.
pub struct Table {
    pub seed: u64,
    /// Empty for a small table, which is read by walking it.
    pub disps: Vec<(u32, u32)>,
    /// In slot order where there are displacements, in written order where
    /// there are not.
    pub keys: Vec<String>,
    /// Beside `keys`, element for element.
    pub order: Vec<usize>,
}

/// The same hash `nikaia_std::fixed` computes, over the same bytes.
///
/// **Two implementations of one function**, which is the thing `open-work.md`
/// §2.9 argues against one construct over — so what holds them together is not
/// care but `crates/nikaia/tests/fixed_map.rs`, which **runs** a program over
/// both table shapes. A generator that agreed with itself would prove nothing.
pub fn fnv(key: &str, seed: u64) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64 ^ seed;
    for byte in key.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

fn displace(f1: u32, f2: u32, d1: u32, d2: u32) -> u32 {
    d2.wrapping_add(f1.wrapping_mul(d1)).wrapping_add(f2)
}

/// Build the table for these keys, in the order they were written.
///
/// `None` where no seed worked, which CHD's own literature says does not happen
/// for a sane bucket count — and which this reports rather than looping, because
/// a build that hangs is worse than one that says it could not.
pub fn build(keys: &[String]) -> Option<Table> {
    if keys.len() < HASHED_FROM {
        return Some(Table {
            seed: 0,
            disps: Vec::new(),
            keys: keys.to_vec(),
            order: (0..keys.len()).collect(),
        });
    }
    let n = keys.len();
    // λ ≈ 5 keys per bucket, which is what the `phf` crate uses.
    let buckets = n.div_ceil(5).max(1);

    for seed in 0..200u64 {
        if let Some(table) = attempt(keys, seed, buckets, n) {
            return Some(table);
        }
    }
    None
}

fn attempt(keys: &[String], seed: u64, buckets: usize, n: usize) -> Option<Table> {
    let mut by_bucket: BTreeMap<usize, Vec<Placed>> = BTreeMap::new();
    for (at, key) in keys.iter().enumerate() {
        let hash = fnv(key, seed);
        let (g, f1, f2) = ((hash >> 32) as u32, hash as u32, (hash >> 16) as u32);
        by_bucket
            .entry((g as usize) % buckets)
            .or_default()
            .push((at, f1, f2));
    }

    // **The fullest bucket first**, which is what makes CHD terminate: the hard
    // placements happen while the table is empty.
    let mut order: Vec<(usize, Vec<Placed>)> = by_bucket.into_iter().collect();
    order.sort_by_key(|(bucket, held)| (std::cmp::Reverse(held.len()), *bucket));

    let mut disps = vec![(0u32, 0u32); buckets];
    let mut slots: Vec<Option<usize>> = vec![None; n];
    for (bucket, held) in order {
        let mut placed = false;
        'search: for d1 in 0..n as u32 {
            for d2 in 0..n as u32 {
                let mut taken = Vec::with_capacity(held.len());
                let mut fits = true;
                for (at, f1, f2) in &held {
                    let slot = (displace(*f1, *f2, d1, d2) as usize) % n;
                    if slots[slot].is_some() || taken.iter().any(|(s, _)| *s == slot) {
                        fits = false;
                        break;
                    }
                    taken.push((slot, *at));
                }
                if fits {
                    disps[bucket] = (d1, d2);
                    for (slot, at) in taken {
                        slots[slot] = Some(at);
                    }
                    placed = true;
                    break 'search;
                }
            }
        }
        if !placed {
            return None;
        }
    }

    // **Every slot holds a key, because the table is minimal**: `n` keys into
    // `n` slots, each in one of its own. That is not a detail — an empty slot
    // would need a key to compare against, and there is no `&str` that no
    // lookup can equal. Writing `""` there would answer `table.get("")` with
    // whatever value sat beside it, which is a wrong answer rather than a
    // missing feature.
    //
    // So a gap is a failed attempt and the next seed is tried, rather than a
    // hole papered over.
    let mut written = Vec::with_capacity(n);
    let mut at_of = Vec::with_capacity(n);
    for slot in &slots {
        let at = (*slot)?;
        written.push(keys[at].clone());
        at_of.push(at);
    }
    Some(Table {
        seed,
        disps,
        keys: written,
        order: at_of,
    })
}
