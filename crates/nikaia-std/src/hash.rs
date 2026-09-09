//! The hash a map gets when nobody outside the program chose its keys.
//!
//! [ADR-010](../../../docs/specification/adr/adr-010.md) D5: provenance selects
//! a hash function and nothing else. Untrusted keys get the keyed, randomly
//! seeded hash that `std` gives every map by default; trusted keys get this one
//! - fast, fixed seed, not cryptographic and not trying to be.
//!
//! It is the hash `rustc` uses on its own tables, and it is here rather than
//! behind a dependency for two reasons: it is twenty lines that sit on the hot
//! path of every Nikaia program that aggregates anything, and a program whose
//! *hash function* comes from a crate it did not choose is a program whose
//! iteration order and worst case can change under it.
//!
//! What it is not: a defence against chosen keys. That is exactly why the
//! compiler picks between the two rather than making this the default, and why
//! a barrier in the provenance analysis widens to untrusted rather than to
//! here.

use std::hash::{BuildHasherDefault, Hasher};

/// A `HashMap` for keys the operator chose.
pub type TrustedMap<K, V> = std::collections::HashMap<K, V, BuildHasherDefault<FxHasher>>;

/// A `HashSet` for keys the operator chose.
pub type TrustedSet<K> = std::collections::HashSet<K, BuildHasherDefault<FxHasher>>;

/// The constant `rustc`'s hash multiplies by: the 64-bit odd number closest to
/// `2^64 / φ`, so that the multiply spreads a change in any bit across the
/// whole word.
const SEED: u64 = 0x51_7c_c1_b7_27_22_0a_95;

/// A fast, non-cryptographic hash with a fixed seed.
///
/// One multiply and one rotate per word of input, no allocation, no state
/// beyond the accumulator. Every write folds into the same accumulator, so the
/// order of the parts of a compound key matters - which is what a hash of a
/// tuple has to promise.
#[derive(Default, Clone)]
pub struct FxHasher {
    hash: u64,
}

impl FxHasher {
    #[inline]
    fn add(&mut self, word: u64) {
        self.hash = (self.hash.rotate_left(5) ^ word).wrapping_mul(SEED);
    }
}

impl Hasher for FxHasher {
    #[inline]
    fn write(&mut self, mut bytes: &[u8]) {
        // Whole words while there are whole words, then what is left, widest
        // piece first. A tail read as one 4-, 2- or 1-byte piece is three
        // branches at most, against one `add` per byte.
        while bytes.len() >= 8 {
            self.add(u64::from_ne_bytes(bytes[..8].try_into().expect("eight")));
            bytes = &bytes[8..];
        }
        if bytes.len() >= 4 {
            self.add(u32::from_ne_bytes(bytes[..4].try_into().expect("four")) as u64);
            bytes = &bytes[4..];
        }
        if bytes.len() >= 2 {
            self.add(u16::from_ne_bytes(bytes[..2].try_into().expect("two")) as u64);
            bytes = &bytes[2..];
        }
        if let Some(byte) = bytes.first() {
            self.add(*byte as u64);
        }
    }

    #[inline]
    fn write_u8(&mut self, n: u8) {
        self.add(n as u64);
    }

    #[inline]
    fn write_u16(&mut self, n: u16) {
        self.add(n as u64);
    }

    #[inline]
    fn write_u32(&mut self, n: u32) {
        self.add(n as u64);
    }

    #[inline]
    fn write_u64(&mut self, n: u64) {
        self.add(n);
    }

    #[inline]
    fn write_usize(&mut self, n: usize) {
        self.add(n as u64);
    }

    #[inline]
    fn finish(&self) -> u64 {
        self.hash
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn hash_of(bytes: &[u8]) -> u64 {
        let mut h = FxHasher::default();
        h.write(bytes);
        h.finish()
    }

    /// A hash function is only useful if the same key hashes the same way every
    /// time, in the same process and the next one. That is what "fixed seed"
    /// means and it is the whole difference from the map `std` gives by
    /// default.
    #[test]
    fn the_same_key_hashes_the_same_way() {
        assert_eq!(hash_of(b"Hamburg"), hash_of(b"Hamburg"));
        assert_ne!(hash_of(b"Hamburg"), hash_of(b"Bulawayo"));
    }

    /// Order matters inside a key: `ab` is not `ba`, and a compound key's parts
    /// do not commute.
    #[test]
    fn the_order_of_the_bytes_matters() {
        assert_ne!(hash_of(b"ab"), hash_of(b"ba"));

        let mut first = FxHasher::default();
        first.write_u32(1);
        first.write_u32(2);
        let mut second = FxHasher::default();
        second.write_u32(2);
        second.write_u32(1);
        assert_ne!(first.finish(), second.finish());
    }

    /// Every tail length is reached: a key one byte longer hashes differently,
    /// across the 8-, 4-, 2- and 1-byte pieces the tail is read in.
    ///
    /// Not injectivity - a 64-bit non-cryptographic hash does not promise that
    /// and this one visibly does not have it: the empty key and the single zero
    /// byte both leave the accumulator at zero, because zero rotated and
    /// xor-ed with zero is zero. That costs a bucket comparison and nothing
    /// else, since a map compares keys in full (ADR-010 D5); a length mixed
    /// into every hash would cost a multiply on every lookup to fix it.
    #[test]
    fn every_length_is_covered() {
        let mut seen = std::collections::HashSet::new();
        for len in 1..40usize {
            let key: Vec<u8> = (0..len).map(|i| (i % 251 + 1) as u8).collect();
            assert!(seen.insert(hash_of(&key)), "collision at length {len}");
        }
        assert_eq!(hash_of(b""), hash_of(&[0]), "the documented one");
    }

    /// What it is for: a table that answers, over the keys a data format
    /// actually has.
    #[test]
    fn it_works_as_a_map_hasher() {
        let mut map: TrustedMap<&str, i32> = TrustedMap::default();
        for (i, name) in ["Hamburg", "Bulawayo", "Palembang", "St. John's", "東京"]
            .iter()
            .enumerate()
        {
            map.insert(name, i as i32);
        }
        assert_eq!(map.len(), 5);
        assert_eq!(map["東京"], 4);
        assert_eq!(map.get("nowhere"), None);

        // …and it agrees with the map it stands in for.
        let plain: HashMap<&str, i32> = map.iter().map(|(k, v)| (*k, *v)).collect();
        assert_eq!(plain.len(), map.len());
    }

    /// The keys of a real aggregation, spread over a table of the size one
    /// would have: no bucket may hold a tenth of them.
    #[test]
    fn it_spreads_the_keys_of_an_aggregation() {
        let keys: Vec<String> = (0..413).map(|i| format!("Station{i:03}")).collect();
        let buckets = 1024u64;
        let mut counts = vec![0usize; buckets as usize];
        for key in &keys {
            counts[(hash_of(key.as_bytes()) % buckets) as usize] += 1;
        }
        let worst = counts.iter().copied().max().expect("non-empty");
        assert!(worst < keys.len() / 10, "worst bucket held {worst}");
    }
}
