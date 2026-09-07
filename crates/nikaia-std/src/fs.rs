//! `std::fs` - files.

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

    std::str::from_utf8(&pages).map_err(|e| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("not UTF-8 at byte {}", e.valid_up_to()),
        )
    })?;

    Ok(Mapped {
        map: Backing::Pages(pages),
    })
}
