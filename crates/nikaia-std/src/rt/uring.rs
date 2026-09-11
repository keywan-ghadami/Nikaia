//! Files, completed by the kernel
//! ([ADR-038](../../../../docs/specification/adr/adr-038.md) D3).
//!
//! `epoll` cannot read a file at all: a regular file is always "ready" and the
//! read blocks in the kernel anyway, which is why every readiness-based
//! runtime serves file I/O from a thread pool - and that pool *is*
//! [ADR-033](../../../../docs/specification/adr/adr-033.md) §8.4's 46 µs.
//! `io_uring` removes it, not by being a faster pool but because no thread is
//! involved: the kernel performs the read and reports when it is done.
//!
//! ## The soundness rule, and how it is met
//!
//! A buffer handed to the kernel must stay alive and unmoved until the
//! completion for it arrives. The rule this file follows is one sentence:
//!
//! > **A slot, and the buffer in it, is freed only by the code that has
//! > reaped its completion.**
//!
//! Three things make that hold rather than hope for it.
//!
//! 1. **The buffers are not on any caller's stack.** A [`Job`] lives in
//!    `Ring::jobs`, and the `Ring` lives in the process-global runtime
//!    ([`crate::rt`]), which outlives every frame in the program. An unwind
//!    through `read_many` therefore cannot drop a buffer the kernel holds -
//!    there is nothing on that stack to drop.
//! 2. **A slot is released on a count, not on a return.** `Job::outstanding`
//!    is the number of submissions for that slot whose completion has not been
//!    reaped. The slot is cleared when it reaches zero and at no other moment,
//!    so an operation abandoned half-way leaves its buffer where the kernel
//!    left it.
//! 3. **The next operation reconciles before it submits.** [`Ring::reconcile`]
//!    reaps whatever an abandoned call left behind, which is what makes a slot
//!    reusable *after* its completion rather than after its caller gave up.
//!
//! What the language contributes is that there is never a borrowed buffer to
//! hand over in the first place. `fs::read` and `fs::read_to_string` return
//! **owned** values, so `std` allocates the buffer, moves it into the slot,
//! and moves the finished bytes out again - and `fs::map`, the one function
//! here that hands back memory it does not own, is a mapping and never
//! reaches the ring. Ownership is what
//! [ADR-005](../../../../docs/specification/adr/adr-005.md) and
//! [ADR-008](../../../../docs/specification/adr/adr-008.md)'s tether lattice
//! infer, and it is what the kernel is handed.
//!
//! ## What is *not* completed here
//!
//! `open` and `stat` are ordinary blocking syscalls. They are a path walk in
//! the kernel with no data transfer, they are what the measurement's baseline
//! pays too, and putting them on the ring would buy a syscall rather than a
//! thread. The *read* is what ADR-033 §8.4 priced, and the read is what
//! completes.

use std::fs::File;
use std::io;
use std::os::unix::io::AsRawFd;
use std::path::Path;

use io_uring::{opcode, types, IoUring};

/// How many submissions the ring holds. Two is the pair ADR-033 is about; this
/// leaves room for a handful of jobs in flight without a resubmit.
const ENTRIES: u32 = 64;

/// How much a read grows its buffer by when the file turned out longer than
/// `stat` said - a file being appended to while it is read.
const GROW: usize = 64 * 1024;

/// One file operation, and the buffer the kernel is writing into or reading
/// out of.
///
/// The buffer is owned here and nowhere else. See the module comment for why
/// that is the whole of the soundness argument.
struct Job {
    /// Kept open for as long as the kernel may touch it, for the same reason
    /// the buffer is: a closed descriptor with a submission outstanding is the
    /// other half of the same bug.
    file: File,
    buf: Vec<u8>,
    /// How much of `buf` the kernel has filled (a read) or consumed (a write).
    at: usize,
    /// Submissions for this slot whose completion has not been reaped. The
    /// slot may be cleared when, and only when, this is zero.
    outstanding: usize,
    /// `None` while the job is running.
    outcome: Option<io::Result<()>>,
    kind: Kind,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Read,
    Write,
}

/// The ring, and everything in flight on it.
///
/// One per runtime, behind the runtime's own lock. Two threads submitting into
/// one submission queue is not sound and no lock-free trick here would be
/// worth the reasoning - and at `user_parallelism = no` there is exactly one
/// user thread, so the lock is uncontended by construction
/// ([ADR-037](../../../../docs/specification/adr/adr-037.md) D2).
pub struct Ring {
    ring: IoUring,
    /// Slot index is the `user_data` the kernel reports back.
    jobs: Vec<Option<Job>>,
    /// Completions submitted and not yet reaped, over all slots.
    unreaped: usize,
}

impl Ring {
    /// A ring, or the reason there is none.
    ///
    /// **Feature detection, at run time and never at compile time.** A binary
    /// built on a machine with `io_uring` must still run on one without: an
    /// older kernel answers `ENOSYS`, a sandbox that forbids the syscalls
    /// answers `EPERM`, and both arrive here as an ordinary `Err` that the
    /// caller turns into the blocking path (D3).
    pub fn open() -> io::Result<Ring> {
        let ring = IoUring::new(ENTRIES)?;
        Ok(Ring {
            ring,
            jobs: Vec::new(),
            unreaped: 0,
        })
    }

    /// Read every one of `paths` whole, with all of them in flight at once.
    ///
    /// This is the shape ADR-033 §8.5 predicted and could not measure: several
    /// operations in flight, their results collected in the order they were
    /// written, and **no thread started or woken for the pair** - the kernel
    /// does the reads and one `io_uring_enter` collects them.
    pub fn read_many(&mut self, paths: &[&Path]) -> Vec<io::Result<Vec<u8>>> {
        self.reconcile();

        // Open and size first, so that a path that does not exist fails
        // without a submission - and so that every buffer exists before
        // anything is handed to the kernel.
        let mut slots: Vec<Result<usize, io::Error>> = Vec::with_capacity(paths.len());
        for path in paths {
            slots.push(self.begin(path, Kind::Read, Vec::new()));
        }

        self.drive();

        slots
            .into_iter()
            .map(|slot| match slot {
                Err(e) => Err(e),
                Ok(slot) => self.finish(slot).map(|(buf, _)| buf),
            })
            .collect()
    }

    /// Write `bytes` to `path`, the kernel doing the transfer.
    ///
    /// `bytes` arrives by value, which is not a convenience: it is the
    /// soundness rule. There is no borrowed buffer to keep alive because there
    /// is no borrowed buffer.
    pub fn write(
        &mut self,
        path: &Path,
        bytes: Vec<u8>,
        append: bool,
        create: bool,
    ) -> io::Result<()> {
        self.reconcile();
        let slot = self.open_for_write(path, bytes, append, create)?;
        self.drive();
        self.finish(slot).map(|_| ())
    }

    /// Open `path`, size it, and take a slot with a buffer the kernel can use.
    fn begin(&mut self, path: &Path, kind: Kind, bytes: Vec<u8>) -> io::Result<usize> {
        let file = File::open(path)?;
        let size = file.metadata()?.len() as usize;
        let buf = match kind {
            // `stat`'s answer is a hint and not a contract - `/proc` reports
            // zero and a growing file reports the old length - so the loop in
            // `drive` is what decides where the file ends. One extra byte
            // makes an exactly-`size` file take one more turn and report EOF,
            // rather than being silently truncated at the hint.
            Kind::Read => vec![0u8; size + 1],
            Kind::Write => bytes,
        };
        Ok(self.take_slot(Job {
            file,
            buf,
            at: 0,
            outstanding: 0,
            outcome: None,
            kind,
        }))
    }

    fn open_for_write(
        &mut self,
        path: &Path,
        bytes: Vec<u8>,
        append: bool,
        create: bool,
    ) -> io::Result<usize> {
        let file = std::fs::OpenOptions::new()
            .write(true)
            .append(append)
            .truncate(!append)
            .create(create)
            .open(path)?;
        Ok(self.take_slot(Job {
            file,
            buf: bytes,
            at: 0,
            outstanding: 0,
            outcome: None,
            kind: Kind::Write,
        }))
    }

    /// The first slot that is free. A slot with a submission outstanding is
    /// **not** free, however long ago its caller gave up on it - that is rule
    /// 2 of the module comment, and it is the line between this being sound
    /// and being usually sound.
    fn take_slot(&mut self, job: Job) -> usize {
        if let Some(at) = self.jobs.iter().position(Option::is_none) {
            self.jobs[at] = Some(job);
            return at;
        }
        self.jobs.push(Some(job));
        self.jobs.len() - 1
    }

    /// Push one submission for `slot`, if it has more to do.
    ///
    /// Returns whether anything was submitted.
    fn submit_one(&mut self, slot: usize) -> bool {
        let job = match self.jobs.get_mut(slot).and_then(Option::as_mut) {
            Some(job) if job.outcome.is_none() => job,
            _ => return false,
        };

        if job.kind == Kind::Read && job.at == job.buf.len() {
            job.buf.resize(job.at + GROW, 0);
        }
        if job.kind == Kind::Write && job.at == job.buf.len() {
            job.outcome = Some(Ok(()));
            return false;
        }

        let fd = types::Fd(job.file.as_raw_fd());
        let at = job.at;
        let rest = (job.buf.len() - at) as u32;
        // SAFETY: the pointer is into `job.buf`, which lives in `self.jobs`
        // and is released only by `finish` once `outstanding` reaches zero -
        // so the kernel's view of it outlives every submission made for it.
        // The slice is disjoint from every other in-flight submission because
        // each slot has its own buffer and each submission covers `at..len`
        // of exactly one slot.
        let entry = unsafe {
            match job.kind {
                Kind::Read => opcode::Read::new(fd, job.buf.as_mut_ptr().add(at), rest)
                    .offset(at as u64)
                    .build()
                    .user_data(slot as u64),
                Kind::Write => opcode::Write::new(fd, job.buf.as_ptr().add(at), rest)
                    // `u64::MAX` is "wherever the descriptor is", which is
                    // what `O_APPEND` needs and what a truncating write wants
                    // after the first turn.
                    .offset(u64::MAX)
                    .build()
                    .user_data(slot as u64),
            }
        };

        loop {
            // SAFETY: `entry` names a buffer that outlives the submission, per
            // the comment above; that is the whole of this call's contract.
            if unsafe { self.ring.submission().push(&entry) }.is_ok() {
                break;
            }
            // A full submission queue: hand what is there to the kernel and
            // try again. `submit` cannot fail for lack of room.
            if let Err(e) = self.ring.submit() {
                if let Some(job) = self.jobs[slot].as_mut() {
                    job.outcome = Some(Err(e));
                }
                return false;
            }
        }
        self.jobs[slot]
            .as_mut()
            .expect("a slot just submitted for")
            .outstanding += 1;
        self.unreaped += 1;
        true
    }

    /// Run every unfinished slot to its end, keeping all of them in flight.
    fn drive(&mut self) {
        loop {
            let mut submitted = 0;
            for slot in 0..self.jobs.len() {
                if self.submit_one(slot) {
                    submitted += 1;
                }
            }
            if submitted == 0 {
                return;
            }
            if let Err(e) = self.ring.submit_and_wait(submitted) {
                // Nothing was completed, so nothing may be freed. Every
                // outstanding count stays as it is and `reconcile` will find
                // them; the jobs are failed so their callers stop waiting.
                self.fail_running(e);
                return;
            }
            self.reap();
        }
    }

    /// Take every completion the kernel has posted, and account for it.
    fn reap(&mut self) {
        let mut done = Vec::new();
        for cqe in self.ring.completion() {
            done.push((cqe.user_data() as usize, cqe.result()));
        }
        for (slot, result) in done {
            self.unreaped -= 1;
            let Some(job) = self.jobs.get_mut(slot).and_then(Option::as_mut) else {
                // A completion for a slot nobody is waiting on: the buffer was
                // already released because `outstanding` had reached zero.
                // Nothing to do, and nothing to free.
                continue;
            };
            job.outstanding -= 1;
            match result {
                n if n < 0 => job.outcome = Some(Err(io::Error::from_raw_os_error(-n))),
                0 => {
                    // End of file for a read; a write that moved nothing is
                    // also finished, and both mean "no more turns".
                    job.buf.truncate(job.at);
                    job.outcome.get_or_insert(Ok(()));
                }
                n => {
                    job.at += n as usize;
                    if job.kind == Kind::Write && job.at == job.buf.len() {
                        job.outcome.get_or_insert(Ok(()));
                    }
                }
            }
        }
    }

    /// Mark every running job failed, without touching a single outstanding
    /// count. Used when the kernel refused the submission itself.
    fn fail_running(&mut self, error: io::Error) {
        for job in self.jobs.iter_mut().flatten() {
            if job.outcome.is_none() {
                job.outcome = Some(Err(io::Error::from(error.kind())));
            }
        }
    }

    /// The bytes, and the slot released - if and only if its completions have
    /// all arrived.
    fn finish(&mut self, slot: usize) -> io::Result<(Vec<u8>, usize)> {
        let Some(job) = self.jobs.get(slot).and_then(Option::as_ref) else {
            return Err(io::Error::other("the runtime lost a file operation's slot"));
        };
        let outcome = match &job.outcome {
            Some(Ok(())) => Ok(()),
            Some(Err(e)) => Err(io::Error::from(e.kind())),
            None => Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "the file operation did not complete",
            )),
        };
        if job.outstanding > 0 {
            // Rule 2. The buffer stays exactly where the kernel left it, until
            // `reconcile` has seen the last completion for it - so there are no
            // bytes to hand back here, and a truncated success would be a
            // silent wrong answer. It is a failure instead.
            outcome?;
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "the file operation still has a submission with the kernel",
            ));
        }
        let job = self.jobs[slot].take().expect("checked just above");
        let at = job.at;
        outcome.map(|()| (job.buf, at))
    }

    /// Reap what an abandoned call left behind, and release the slots whose
    /// last completion has now arrived.
    ///
    /// This is what makes a slot reusable after its *completion* rather than
    /// after its caller returned, and it is why a panic between submit and
    /// reap cannot hand a freed buffer to the kernel.
    fn reconcile(&mut self) {
        if self.unreaped > 0 {
            // Whatever is already posted, taken now; and if the kernel has
            // posted nothing yet, wait for one so progress is made rather than
            // spun on.
            let _ = self.ring.submit_and_wait(1);
            self.reap();
        }
        for slot in 0..self.jobs.len() {
            let free = matches!(
                self.jobs[slot].as_ref(),
                Some(job) if job.outstanding == 0 && job.outcome.is_some()
            );
            if free {
                self.jobs[slot] = None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ring() -> Option<Ring> {
        Ring::open().ok()
    }

    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("nikaia-uring-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("scratch");
        dir.join(name)
    }

    /// The kernel reads the file, and the bytes are the file's.
    #[test]
    fn a_file_is_read_whole() {
        let Some(mut ring) = ring() else {
            return; // No ring here; `rt`'s own tests cover the fallback.
        };
        let path = scratch("whole");
        let text = "Hamburg;12.0\nBremen;9.5\n".repeat(4096);
        std::fs::write(&path, &text).expect("write");

        let read = ring.read_many(&[&path]);
        assert_eq!(read.len(), 1);
        assert_eq!(read[0].as_ref().expect("read"), text.as_bytes());
    }

    /// An empty file is an input, and `stat` reporting zero must not be read as
    /// a failure.
    #[test]
    fn an_empty_file_reads_empty() {
        let Some(mut ring) = ring() else { return };
        let path = scratch("empty");
        std::fs::write(&path, "").expect("write");
        assert_eq!(ring.read_many(&[&path])[0].as_ref().expect("read"), &[]);
    }

    /// Both reads in flight at once, and both answers in the order they were
    /// asked for - which is ADR-033 §8.5's shape.
    #[test]
    fn two_reads_are_both_in_flight_and_answered_in_order() {
        let Some(mut ring) = ring() else { return };
        let (a, b) = (scratch("pair-a"), scratch("pair-b"));
        std::fs::write(&a, "eins").expect("write");
        std::fs::write(&b, "zwei").expect("write");

        let read = ring.read_many(&[&a, &b]);
        assert_eq!(read[0].as_ref().expect("a"), b"eins");
        assert_eq!(read[1].as_ref().expect("b"), b"zwei");

        // And the slots are free again, so the next pair reuses them rather
        // than growing the ring's bookkeeping without bound.
        let read = ring.read_many(&[&b, &a]);
        assert_eq!(read[0].as_ref().expect("b"), b"zwei");
        assert_eq!(read[1].as_ref().expect("a"), b"eins");
        assert_eq!(ring.jobs.len(), 2, "slots are reused, not accumulated");
    }

    /// A path that is not there fails before anything is submitted, so a
    /// failure cannot leave a buffer with the kernel.
    #[test]
    fn a_missing_path_fails_without_a_submission() {
        let Some(mut ring) = ring() else { return };
        let read = ring.read_many(&[Path::new("/nikaia/no/such/file")]);
        assert!(read[0].is_err());
        assert_eq!(ring.unreaped, 0);
        assert!(ring.jobs.iter().all(Option::is_none));
    }

    /// Write, then read back through the ring: both halves, and they agree.
    #[test]
    fn a_write_completes_and_reads_back() {
        let Some(mut ring) = ring() else { return };
        let path = scratch("written");
        let _ = std::fs::remove_file(&path);

        let bytes = "eins\n".repeat(20_000).into_bytes();
        ring.write(&path, bytes.clone(), false, true)
            .expect("write");
        assert_eq!(ring.read_many(&[&path])[0].as_ref().expect("read"), &bytes);

        ring.write(&path, b"zwei\n".to_vec(), true, true)
            .expect("append");
        let read = ring.read_many(&[&path]);
        let back = read[0].as_ref().expect("read");
        assert_eq!(back.len(), bytes.len() + 5);
        assert!(back.ends_with(b"zwei\n"), "the append went to the end");
    }

    /// Rule 2 of the module comment, exercised on the bookkeeping: a slot with
    /// a submission outstanding is not free, so its buffer cannot be handed
    /// out to another operation while the kernel may still be writing it.
    #[test]
    fn a_slot_with_a_submission_outstanding_is_never_reused() {
        let Some(mut ring) = ring() else { return };
        let path = scratch("outstanding");
        std::fs::write(&path, "x".repeat(4096)).expect("write");

        let slot = ring.begin(&path, Kind::Read, Vec::new()).expect("opened");
        assert!(ring.submit_one(slot), "submitted");
        assert_eq!(ring.jobs[slot].as_ref().expect("held").outstanding, 1);

        // A second operation while the first is outstanding takes a *different*
        // slot, whatever the first one's caller did.
        let other = ring.begin(&path, Kind::Read, Vec::new()).expect("opened");
        assert_ne!(other, slot, "the outstanding slot was handed out again");

        // And the outstanding one is only released once its completion has
        // been accounted for.
        ring.reconcile();
        ring.drive();
        assert!(ring.jobs[slot].is_none() || ring.jobs[slot].as_ref().unwrap().outstanding == 0);
    }
}
