//! `std::fs` - files.

use std::path::Path;

/// A file's bytes, addressable as text for as long as the value lives.
///
/// Part III, 17.1 calls for a memory mapping, and that is what this type exists
/// to become: the pages *are* the buffer, so a 13 GB input costs no copy and
/// every view a parser yields points straight into the file. Today it reads the
/// file into memory instead - the same interface, the same guarantee about the
/// text outliving the views, and a different cost. `memmap2` is the change, and
/// it is a change to this file alone.
pub struct Mapped {
    text: String,
}

impl std::ops::Deref for Mapped {
    type Target = str;

    fn deref(&self) -> &str {
        &self.text
    }
}

impl AsRef<str> for Mapped {
    fn as_ref(&self) -> &str {
        &self.text
    }
}

/// Open a file and make its contents addressable as text.
pub fn map(path: impl AsRef<Path>) -> Result<Mapped, std::io::Error> {
    Ok(Mapped {
        text: std::fs::read_to_string(path)?,
    })
}
