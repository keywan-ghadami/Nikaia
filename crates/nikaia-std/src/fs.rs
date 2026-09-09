//! `std::fs` - files.

use rayon::prelude::*;
use std::path::Path;

/// A file's bytes, addressable as text for as long as the value lives.
///
/// Part III, 17.1: a memory mapping. The pages *are* the buffer, so a 13 GB
/// input costs no copy and every view a parser yields points straight into the
/// file. The text is validated as UTF-8 once, when the map is made; after that
/// a view of it is a view of the pages.
///
/// ADR-009 D7 allows a large read-only mapping to be left to the OS at process
/// exit. Nothing here does that yet: the mapping is released when the value is
/// dropped, and the drop is what `main` returning costs.
pub struct Mapped {
    map: Backing,
}

enum Backing {
    Pages(memmap2::Mmap),
    /// A zero-length file cannot be mapped, and an empty input is still an
    /// input.
    Empty,
}

impl std::ops::Deref for Mapped {
    type Target = str;

    fn deref(&self) -> &str {
        match &self.map {
            // SAFETY: `map` validated the whole mapping as UTF-8 before
            // building this value, and the mapping is read-only, so the bytes
            // are the same ones it validated.
            Backing::Pages(pages) => unsafe { std::str::from_utf8_unchecked(pages) },
            Backing::Empty => "",
        }
    }
}

impl AsRef<str> for Mapped {
    fn as_ref(&self) -> &str {
        self
    }
}

/// Map a file and make its contents addressable as text.
///
/// Fails as the file system does, and additionally when the file is not
/// UTF-8 - a parser handed such a mapping would find that out one byte at a
/// time, and the boundary of a frame is a string, not a byte.
pub fn map(path: impl AsRef<Path>) -> Result<Mapped, std::io::Error> {
    let file = std::fs::File::open(path)?;
    if file.metadata()?.len() == 0 {
        return Ok(Mapped {
            map: Backing::Empty,
        });
    }

    // SAFETY: the mapping is read-only and private. The one thing the type
    // system cannot rule out is another process truncating or rewriting the
    // file underneath the mapping while it lives, which is the documented
    // caveat of every memory map and is accepted here as it is everywhere a
    // file is mapped.
    let pages = unsafe { memmap2::Mmap::map(&file)? };

    if let Err(at) = validate(&pages) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("not UTF-8 at byte {at}"),
        ));
    }

    Ok(Mapped {
        map: Backing::Pages(pages),
    })
}

/// Below this, the pool costs more than the check does.
const CHUNKED_ABOVE: usize = 1 << 20;

/// `bytes` as UTF-8, or the offset of the first byte that is not.
///
/// The whole of it is checked, and the check is the thing standing between a
/// mapping and the `&str` views a parser cuts out of it - so it is not
/// skipped, it is divided. Chunks are parsed in parallel and the *earliest*
/// failing offset is the answer, so that a rejected file names the same byte
/// however many cores looked at it.
fn validate(bytes: &[u8]) -> Result<(), usize> {
    if bytes.len() < CHUNKED_ABOVE {
        return std::str::from_utf8(bytes)
            .map(|_| ())
            .map_err(|e| e.valid_up_to());
    }

    let cuts = char_boundaries(bytes, rayon::current_num_threads());
    cuts.par_windows(2)
        .filter_map(|w| {
            std::str::from_utf8(&bytes[w[0]..w[1]])
                .err()
                .map(|e| w[0] + e.valid_up_to())
        })
        .min()
        .map_or(Ok(()), Err)
}

/// `n` cuts through `bytes`, none of them inside a character.
///
/// A continuation byte is `10xxxxxx`, so a cut walks forward off one - at most
/// three times, since a fourth would be malformed and the chunk that holds it
/// says so. Every chunk therefore begins where a character begins, which is
/// what makes checking them separately the same check as checking the whole:
/// no sequence is split, and no chunk can accept a truncated one that the
/// chunk before it left behind.
fn char_boundaries(bytes: &[u8], n: usize) -> Vec<usize> {
    let mut cuts = Vec::with_capacity(n + 1);
    cuts.push(0);
    for k in 1..n {
        let mut at = bytes.len() / n * k;
        for _ in 0..3 {
            if at < bytes.len() && bytes[at] & 0xC0 == 0x80 {
                at += 1;
            }
        }
        cuts.push(at);
    }
    cuts.push(bytes.len());
    cuts.dedup();
    cuts
}

#[cfg(test)]
mod tests {
    use super::{validate, CHUNKED_ABOVE};

    /// What `validate` has to agree with, on every input.
    fn serial(bytes: &[u8]) -> Result<(), usize> {
        std::str::from_utf8(bytes)
            .map(|_| ())
            .map_err(|e| e.valid_up_to())
    }

    /// Big enough to be chunked, and multi-byte throughout so that cuts land
    /// inside characters rather than only between them.
    fn big() -> Vec<u8> {
        let unit = "aä€𝄞b";
        unit.repeat(CHUNKED_ABOVE / unit.len() + 4096).into_bytes()
    }

    #[test]
    fn a_chunked_check_accepts_what_a_serial_one_accepts() {
        let bytes = big();
        assert!(bytes.len() > CHUNKED_ABOVE, "the test input is not chunked");
        assert_eq!(validate(&bytes), Ok(()));
        assert_eq!(validate(&bytes), serial(&bytes));
    }

    #[test]
    fn a_break_anywhere_is_found_where_a_serial_check_finds_it() {
        // Around each cut and at a stride that is coprime with the character
        // width, so the corruption lands on lead bytes and continuation bytes
        // alike - a truncated sequence at a chunk's end is the case chunking
        // could get wrong, and this is where it would show.
        let base = big();
        let cuts = super::char_boundaries(&base, rayon::current_num_threads());

        let mut probes: Vec<usize> = (0..base.len()).step_by(7919).collect();
        for cut in &cuts {
            probes.extend((cut.saturating_sub(8)..(cut + 8).min(base.len())).step_by(1));
        }

        for at in probes {
            let mut bytes = base.clone();
            bytes[at] = 0xFF; // in no position a valid UTF-8 byte
            assert_eq!(
                validate(&bytes),
                serial(&bytes),
                "byte {at} of {}",
                bytes.len()
            );
        }
    }

    #[test]
    fn the_earliest_break_is_the_one_reported() {
        // One break per chunk: whichever core finishes first, the offset is
        // the first one, so a rejected file names the same byte on every
        // machine.
        let mut bytes = big();
        let cuts = super::char_boundaries(&bytes, rayon::current_num_threads());
        assert!(cuts.len() > 2, "one chunk proves nothing here");
        for w in cuts.windows(2) {
            bytes[w[0] + (w[1] - w[0]) / 2] = 0xFF;
        }
        assert_eq!(validate(&bytes), serial(&bytes));
    }

    #[test]
    fn a_small_input_is_checked_in_one_piece() {
        assert_eq!(validate("Hamburg;12.0\n".as_bytes()), Ok(()));
        assert_eq!(validate(b""), Ok(()));
        assert_eq!(validate(b"ok\xFFno"), Err(2));
    }
}
