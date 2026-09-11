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

/// A whole file, as text.
///
/// Part III, 17.1. The other half of `map`, and the one to reach for when the
/// file is small or has to outlive the parse: this copies the bytes into a
/// `String` the caller owns, where `map` hands back pages it does not.
///
/// Fails as the file system does, and additionally when the file is not
/// UTF-8 - the same rule `map` follows, for the same reason.
///
/// **The read goes through the runtime**
/// ([ADR-038](../../../docs/specification/adr/adr-038.md) D3): the kernel
/// completes it where the machine has a completion queue, and an I/O worker
/// performs it where it does not. Which one is invisible from here and
/// invisible from a `.nika` file - that is what "one `std` surface" means, and
/// it is why the next change of mechanism is a `std` change rather than a
/// compiler change (ADR-033 §8.4 gave the same reason for `task::both`).
pub fn read_to_string(path: impl AsRef<Path>) -> Result<String, std::io::Error> {
    let bytes = read(path)?;
    // The same check `map` makes, and the same reason: a parser handed bytes
    // that are not text would find that out one view at a time.
    if let Err(at) = validate(&bytes) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("not UTF-8 at byte {at}"),
        ));
    }
    // SAFETY: `validate` checked the whole of `bytes` as UTF-8 just above, and
    // nothing has touched them since.
    Ok(unsafe { String::from_utf8_unchecked(bytes) })
}

/// Two files, **both in flight at once**, answered in the order they were
/// asked for.
///
/// ADR-033 §8.5's prediction, as a function: two operations that meet on
/// nothing and do not wait for each other, with **no thread started or woken
/// for the pair** - the cost §8.4 measured at ~46 µs for `task::both` and
/// could not remove with any user-space vehicle.
///
/// It is available at `user_parallelism = no`, and that is not a loophole: the
/// two reads are `std`'s own operations and nothing the *user* wrote runs
/// concurrently ([ADR-037](../../../docs/specification/adr/adr-037.md) D2,
/// [ADR-016](../../../docs/specification/adr/adr-016.md) D3). The emitter does
/// **not** lower a statement pair onto it - at `no`, `ordering = "effects"`
/// still degrades to `strict` (ADR-033 §8.2b), and changing that is ADR-033's
/// decision to make rather than this one's.
pub fn read_both(
    a: impl AsRef<Path>,
    b: impl AsRef<Path>,
) -> (
    Result<Vec<u8>, std::io::Error>,
    Result<Vec<u8>, std::io::Error>,
) {
    crate::rt::io::read_both(a.as_ref(), b.as_ref())
}

/// A whole file, as bytes.
///
/// Part III 17.1. The half of `read_to_string` that does not check: an input
/// that is not text is not a failure here, because nothing downstream is going
/// to cut a `&str` out of it. Reach for this where the bytes are the point -
/// an image, a checksum, a format with a length prefix.
pub fn read(path: impl AsRef<Path>) -> Result<Vec<u8>, std::io::Error> {
    crate::rt::io::read(path.as_ref())
}

/// A whole file, written.
///
/// Part III 17.1. The file is created if it is not there and **truncated if it
/// is**, which is what `write` means everywhere and is why the specification
/// gives it `create: bool = true` as the default rather than as a decision at
/// the call.
///
/// A whole file, written.
///
/// Part III 17.1, in full:
///
/// ```nika
/// fs::write(path, data)                        // create or truncate
/// fs::write(path, data; append: true)          // add to the end
/// fs::write(path, data; create: false)         // refuse to make a new file
/// ```
///
/// `append` and `create` are Kap 5.1 **options** - after the `;`, named at the
/// call, never positional. The lowering makes them ordinary parameters in
/// declaration order and fills in the defaults a caller left out, so this
/// signature is what a Nikaia call expands to rather than what it looks like.
///
/// The defaults are the ones the specification names, and they are what `write`
/// means everywhere: create the file if it is not there, and truncate it if it
/// is. `append` keeps what is there and adds; `create: false` refuses to make a
/// file that does not exist, which is how a program says it means to overwrite
/// something in particular.
///
/// Takes anything that is bytes, so a Nikaia program hands it a `String`, a
/// `&str` or a buffer without saying which it meant.
///
/// It is not `sync` (Part II, 12.1) and it is not a source (ADR-010 D2): it
/// does I/O, and the bytes travel out of the program rather than in.
pub fn write(
    path: impl AsRef<Path>,
    data: impl AsRef<[u8]>,
    append: bool,
    create: bool,
) -> Result<(), std::io::Error> {
    // Through the runtime, like the reads (ADR-038 D3). The bytes travel as a
    // borrowed slice and the runtime owns the copy only where the *kernel*
    // needs one to outlive the submission - which is the completion path, and
    // is `rt::uring`'s soundness rule rather than a convenience.
    crate::rt::io::write(path.as_ref(), data.as_ref(), append, create)
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

    /// A written file reads back byte for byte, and a second write replaces
    /// what the first one left rather than adding to it.
    #[test]
    fn a_file_is_written_whole_and_replaced_whole() {
        let path = std::env::temp_dir().join(format!("nikaia-write-{}", std::process::id()));
        let _ = std::fs::remove_file(&path);

        super::write(&path, "Hamburg;12.0\n", false, true).expect("write");
        assert_eq!(
            super::read_to_string(&path).expect("read back"),
            "Hamburg;12.0\n"
        );

        super::write(&path, "Bremen;9.5\n", false, true).expect("write again");
        assert_eq!(
            super::read_to_string(&path).expect("read back"),
            "Bremen;9.5\n",
            "a second write truncates rather than appends"
        );

        // Bytes are bytes: what `read` hands back is what `write` was given,
        // with no check in between - which is the difference from
        // `read_to_string` and the reason both exist.
        super::write(&path, [0xFFu8, 0x00, 0xFE], false, true).expect("write bytes");
        assert_eq!(super::read(&path).expect("read bytes"), [0xFF, 0x00, 0xFE]);
        assert!(
            super::read_to_string(&path).is_err(),
            "the same bytes are not text, and the text half says so"
        );

        std::fs::remove_file(&path).expect("clean up");
    }

    /// Failing to write is an ordinary failure, not a panic: a path whose
    /// parent is not there is the commonest one there is.
    #[test]
    fn writing_where_nothing_can_be_written_fails() {
        let path = std::env::temp_dir()
            .join("nikaia-no-such-directory")
            .join("report.html");
        assert!(super::write(&path, "x", false, true).is_err());
    }

    /// The two options Part III 17.1 names, doing what it says they do.
    #[test]
    fn appending_adds_and_create_false_refuses_a_new_file() {
        let path = std::env::temp_dir().join(format!("nikaia-options-{}", std::process::id()));
        let _ = std::fs::remove_file(&path);

        super::write(&path, "one\n", false, true).expect("write");
        super::write(&path, "two\n", true, true).expect("append");
        assert_eq!(
            super::read_to_string(&path).expect("read back"),
            "one\ntwo\n",
            "appending kept what was there"
        );

        // …and `create: false` is how a program says it means to overwrite
        // something in particular, rather than to make a file.
        let missing = std::env::temp_dir().join(format!("nikaia-absent-{}", std::process::id()));
        let _ = std::fs::remove_file(&missing);
        assert!(super::write(&missing, "x", false, false).is_err());
        assert!(!missing.exists(), "`create: false` made the file anyway");

        // On a file that *is* there it writes, and truncates as `write` does.
        super::write(&path, "three\n", false, false).expect("overwrite");
        assert_eq!(super::read_to_string(&path).expect("read back"), "three\n");

        std::fs::remove_file(&path).expect("clean up");
    }

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
