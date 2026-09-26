//! **A poller that owns what it watches.**
//!
//! `polling::Poller::add` is `unsafe` for one reason: a descriptor must be
//! taken out of the poller before it is closed. This poller makes that
//! impossible to get wrong by holding every watched descriptor itself:
//! [`Poller::add`] takes an `OwnedFd`, [`Poller::remove`] deletes the
//! registration and only then hands the descriptor back, and dropping the
//! poller closes it before any descriptor it held.
//!
//! Every `unsafe` in this crate, and the argument for it, is listed in
//! `README.md`.

#![deny(unsafe_op_in_unsafe_fn)]

use std::collections::HashMap;
use std::io;
use std::os::fd::{AsFd, AsRawFd, OwnedFd};
use std::sync::Mutex;
use std::time::Duration;

pub use polling::{Event, Events};

/// A readiness poller and the descriptors registered with it.
pub struct Poller {
    // Declared first, so the poller is closed before the descriptors are:
    // what it watched is gone from the kernel before anything it names is.
    inner: polling::Poller,
    watched: Mutex<HashMap<usize, OwnedFd>>,
}

impl Poller {
    pub fn new() -> io::Result<Poller> {
        Ok(Poller {
            inner: polling::Poller::new()?,
            watched: Mutex::new(HashMap::new()),
        })
    }

    /// Watch `fd` for `event`, under `event.key`. The poller keeps `fd` until
    /// [`Poller::remove`] hands it back. A key already in use is refused.
    pub fn add(&self, fd: OwnedFd, event: Event) -> io::Result<()> {
        let mut watched = self.watched.lock().unwrap_or_else(|e| e.into_inner());
        if watched.contains_key(&event.key) {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "a descriptor is already watched under this key",
            ));
        }
        // SAFETY: `fd` goes into `watched` below, under the key it is
        // registered with, and leaves it only through `remove` - which deletes
        // the registration first - or when the poller is dropped, after
        // `inner` is closed. So it is open for as long as it is registered.
        unsafe { self.inner.add(fd.as_raw_fd(), event)? };
        watched.insert(event.key, fd);
        Ok(())
    }

    /// Stop watching `key`, and hand back the descriptor it was watching.
    pub fn remove(&self, key: usize) -> Option<OwnedFd> {
        let mut watched = self.watched.lock().unwrap_or_else(|e| e.into_inner());
        let fd = watched.remove(&key)?;
        let _ = self.inner.delete(fd.as_fd());
        Some(fd)
    }

    /// Watch `key`'s descriptor for `event` again (a oneshot registration is
    /// disarmed once it fires).
    pub fn modify(&self, key: usize, event: Event) -> io::Result<()> {
        let watched = self.watched.lock().unwrap_or_else(|e| e.into_inner());
        match watched.get(&key) {
            Some(fd) => self.inner.modify(fd.as_fd(), event),
            None => Err(io::Error::new(
                io::ErrorKind::NotFound,
                "nothing is watched under this key",
            )),
        }
    }

    /// Wait for readiness, at most `timeout`.
    pub fn wait(&self, events: &mut Events, timeout: Option<Duration>) -> io::Result<usize> {
        self.inner.wait(events, timeout)
    }

    /// Wake a `wait` in progress.
    pub fn notify(&self) -> io::Result<()> {
        self.inner.notify()
    }

    /// How many descriptors are watched.
    pub fn len(&self) -> usize {
        self.watched.lock().unwrap_or_else(|e| e.into_inner()).len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(all(test, not(miri)))]
mod tests {
    use super::*;
    use std::io::Write;
    use std::os::unix::net::UnixStream;

    #[test]
    fn a_ready_socket_is_reported_and_its_descriptor_comes_back() {
        let poller = Poller::new().expect("poller");
        let (mut a, b) = UnixStream::pair().expect("pair");
        poller
            .add(OwnedFd::from(b), Event::readable(7))
            .expect("add");
        assert!(
            poller
                .add(
                    OwnedFd::from(UnixStream::pair().unwrap().0),
                    Event::readable(7)
                )
                .is_err()
        );
        a.write_all(b"x").expect("write");
        let mut events = Events::new();
        poller
            .wait(&mut events, Some(Duration::from_secs(5)))
            .expect("wait");
        assert!(events.iter().any(|e| e.key == 7 && e.readable));
        let back = poller.remove(7).expect("it was watched");
        assert!(poller.is_empty());
        drop(back);
        assert!(poller.remove(7).is_none());
    }

    #[test]
    fn a_poller_dropped_with_descriptors_in_it_closes_cleanly() {
        let poller = Poller::new().expect("poller");
        for key in 0..4 {
            let (_a, b) = UnixStream::pair().expect("pair");
            poller
                .add(OwnedFd::from(b), Event::writable(key))
                .expect("add");
        }
        assert_eq!(poller.len(), 4);
        drop(poller);
    }
}
