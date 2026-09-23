//! **HTTP/1.1's text half**, as far as the MVP's protocol goes
//! ([ADR-194](../../../docs/specification/adr/adr-194.md) D5).
//!
//! `GET` and `POST`, bodies by `Content-Length`, `Connection: close`, no
//! chunked transfer and no TLS. That record's own scope, and its own staging:
//! **the parser is Rust here and moving it into a Nikaia grammar is a later
//! step** — self-hosting the protocol is not the MVP, and the route it will
//! take when it moves is [ADR-196](../../../docs/specification/adr/adr-196.md)
//! D2's.
//!
//! **Not named `http`**, and that is not a taste: a program reaches a *package*
//! by that word ([ADR-069](../../../docs/specification/adr/adr-069.md) D1), and
//! a `std` module of the same name would make `http::Response` mean two things.
//! `std`'s own ledger says what happens the day a crate's word collides with a
//! module's — *that is a refusal to write* — and this is the day not to create
//! one.
//!
//! ## Where the line between this and a package runs
//!
//! This module holds **framing**: where a head ends, what its lines say, and
//! how many bytes of body the head promised. A package holds **meaning**: what
//! a path is for, what a handler is, what goes back. `Buffer` is on this side
//! of the line because deciding that a head has not arrived yet is reading
//! HTTP/1.1, not reading a request.
//!
//! It reaches no socket. A program reads bytes with `net` and hands them here,
//! which is why every function below is `sync`: a server calls them between two
//! steps that do pause, and neither of them is this.
//!
//! ## What it refuses, and why a refusal is the feature
//!
//! A head longer than the cap, a `Content-Length` that is not a number, a
//! request line that is not three words. Each is a **400** or a **431** rather
//! than a guess: [ADR-010](../../../docs/specification/adr/adr-010.md) D2 says
//! these bytes are somebody else's, and a parser that guessed at them would be
//! guessing on their behalf.

use crate::io::IoError;

/// The bytes of one request as they arrive, and the questions HTTP/1.1 can ask
/// of them.
///
/// A `Vec[u8]` and not a `Bytes`: this one grows, and what makes a `Bytes` cheap
/// to hand on is that it does not
/// ([ADR-156](../../../docs/specification/adr/adr-156.md) D2). A caller reads a
/// chunk, `take`s it, and asks again.
#[derive(Debug, Default)]
pub struct Buffer {
    bytes: Vec<u8>,
}

impl Buffer {
    /// Nothing read yet. It is written `http1::Buffer()`.
    pub fn new() -> Buffer {
        Buffer { bytes: Vec::new() }
    }

    /// Add what the socket just handed over.
    ///
    /// `impl AsRef<[u8]>` for `net::Connection::write`'s reason: what a caller
    /// has is a `Bytes`, what a test has is text, and a signature that named one
    /// of them would refuse the other.
    pub fn take(&mut self, more: impl AsRef<[u8]>) {
        self.bytes.extend_from_slice(more.as_ref());
    }

    /// How many bytes have arrived.
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    /// Whether none have.
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// Where the head ends, or **`0` for *not yet***.
    ///
    /// Zero and not a missing value, which is a shape chosen for what it lets a
    /// caller write: a read loop over `while ended == 0` is one condition, where
    /// a loop over an absence is a condition plus the unwrapping that follows
    /// it. A head cannot end at byte zero — the shortest one there is is
    /// `GET / HTTP/1.0\r\n\r\n` — so the two readings can never collide.
    ///
    /// The **cap** is answered here rather than in `head`, because it is the
    /// question a *growing* buffer asks and `head` is asked once: a head that
    /// has not ended and is already over the cap never will.
    pub fn head_end(&self, cap: i64) -> Result<i64, IoError> {
        match find(&self.bytes, b"\r\n\r\n") {
            Some(at) => {
                let end = (at + 4) as i64;
                if end > cap {
                    return Err(IoError::Other("431 a request head over the cap".into()));
                }
                Ok(end)
            }
            None => {
                if self.bytes.len() as i64 > cap {
                    return Err(IoError::Other("431 a request head over the cap".into()));
                }
                Ok(0)
            }
        }
    }

    /// What the head says, where `head_end` said there is one.
    ///
    /// It finds the end again rather than being told it, so that a caller
    /// cannot pass a number this did not produce — four bytes over a buffer a
    /// request head long, twice, which is not where a server spends its time.
    pub fn head(&self) -> Result<Head, IoError> {
        let Some(at) = find(&self.bytes, b"\r\n\r\n") else {
            return Err(IoError::Other(
                "400 a request head that has not ended".into(),
            ));
        };
        let end = at + 4;
        let Ok(text) = std::str::from_utf8(&self.bytes[..end]) else {
            return Err(IoError::Other("400 a request head that is not text".into()));
        };
        parse(text, end as i64)
    }

    /// The bytes from `at` on, as text.
    ///
    /// **Owned**, and the copy is not an oversight: this is the body, and a body
    /// goes into a request a handler is given, which owns it. The view this
    /// could hand back instead would be copied there anyway.
    pub fn text_from(&self, at: i64) -> Result<String, IoError> {
        let at = at.max(0) as usize;
        if at > self.bytes.len() {
            return Err(IoError::Other(
                "400 a body that starts past its bytes".into(),
            ));
        }
        match std::str::from_utf8(&self.bytes[at..]) {
            Ok(text) => Ok(text.to_string()),
            Err(_) => Err(IoError::Other("400 a request body that is not text".into())),
        }
    }
}

/// What the head of one request says.
#[derive(Debug)]
pub struct Head {
    method: String,
    path: String,
    length: i64,
    /// How many bytes the head took, so a caller knows where the body starts.
    size: i64,
    /// Whether the client asked to keep the connection open. **Read and not
    /// honoured** in the MVP: `Connection: close` is the scope
    /// ([ADR-194](../../../docs/specification/adr/adr-194.md) D5), and a server
    /// that read the header and ignored it silently would be lying to a client
    /// that asked.
    keep_alive: bool,
}

impl Head {
    /// `GET` or `POST`, as it was written.
    pub fn method(&self) -> &str {
        &self.method
    }

    /// The path, with its query string if there is one. Not decoded: what a
    /// `%20` means is the program's question, and a `std` that decided it would
    /// be deciding a security question on the program's behalf.
    pub fn path(&self) -> &str {
        &self.path
    }

    /// What `Content-Length` said, or `0` where it said nothing.
    pub fn length(&self) -> i64 {
        self.length
    }

    /// How many bytes of the buffer the head took.
    pub fn size(&self) -> i64 {
        self.size
    }

    /// Whether the client asked for the connection to stay open.
    pub fn keep_alive(&self) -> bool {
        self.keep_alive
    }
}

/// The request line and the headers, where the head is already known to be text
/// and `size` bytes long.
fn parse(text: &str, size: i64) -> Result<Head, IoError> {
    let mut lines = text.split("\r\n");
    let request_line = lines.next().unwrap_or("");
    let mut words = request_line.split(' ');
    let (Some(method), Some(path), Some(version)) = (words.next(), words.next(), words.next())
    else {
        return Err(IoError::Other(
            "400 a request line that is not three words".into(),
        ));
    };
    if !version.starts_with("HTTP/1.") {
        return Err(IoError::Other("400 a version this does not speak".into()));
    }
    // **The two methods and no others** (D5). A `PUT` refused by name is a
    // server saying what it is; one quietly routed as a `GET` is a server
    // guessing.
    if method != "GET" && method != "POST" {
        return Err(IoError::Other("400 a method this does not answer".into()));
    }

    let mut length = 0_i64;
    let mut keep_alive = false;
    for line in lines {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        // A header's name does not care about case, and its value's surrounding
        // blanks are not part of it.
        let name = name.trim().to_ascii_lowercase();
        let value = value.trim();
        match name.as_str() {
            "content-length" => match value.parse::<i64>() {
                Ok(n) if n >= 0 => length = n,
                _ => {
                    return Err(IoError::Other(
                        "400 a content-length that is not a count".into(),
                    ))
                }
            },
            "connection" => keep_alive = value.eq_ignore_ascii_case("keep-alive"),
            "transfer-encoding" => {
                // **Refused and not ignored** (D5: no chunked). A server that
                // ignored this would read a chunk header as a body.
                return Err(IoError::Other(
                    "400 a transfer-encoding this does not speak".into(),
                ));
            }
            _ => {}
        }
    }

    Ok(Head {
        method: method.to_string(),
        path: path.to_string(),
        length,
        size,
        keep_alive,
    })
}

/// Where `needle` starts in `haystack`.
///
/// A `memchr` here would be a dependency for a loop over a request head that
/// nobody has measured ([ADR-009](../../../docs/specification/adr/adr-009.md)
/// D4).
fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.len() > haystack.len() {
        return None;
    }
    (0..=haystack.len() - needle.len()).find(|&at| &haystack[at..at + needle.len()] == needle)
}
