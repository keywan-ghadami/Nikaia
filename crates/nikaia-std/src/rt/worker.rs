//! The I/O threads, and the boundary that keeps user code off them.
//!
//! [ADR-038](../../../../docs/specification/adr/adr-038.md) D4 starts one of
//! these before the first statement the user wrote, so that an operation costs
//! no thread wake-up. [ADR-037](../../../../docs/specification/adr/adr-037.md)
//! D2 says whose thread it is: *the compiler's*, not the user's - which is the
//! only reason it may exist at `user_parallelism = no` at all.
//!
//! ## Where that boundary is, in the code
//!
//! It is [`Op`]. A worker's inbox is a channel of `Op`, `Op` is a **closed
//! enum private to `crate::rt`**, and no variant of it carries a closure, a
//! `Box<dyn FnOnce>` or anything else a program could put its own code into.
//! So "nothing the user wrote runs on the I/O thread" is not a convention that
//! review has to keep: outside this module there is no way to name a value the
//! channel accepts.
//!
//! The other direction is checked at run time, cheaply: [`on_io_worker`] is
//! true only inside a worker's loop, and every public entry point of
//! [`crate::rt::io`] asserts it is false. A `std` function that ended up being
//! called from a worker would be a `std` bug, and it says so rather than
//! deadlocking.

use std::io;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};

/// What a worker may be asked to do.
///
/// Not `pub`: see the module comment. This enum *is* the user-code boundary,
/// and it stays a closed list of operations `std` performs on the program's
/// behalf.
pub(super) enum Op {
    /// A whole file, read on the worker thread with ordinary blocking calls -
    /// D3's fallback, for the machine that has no completion queue.
    Read {
        path: PathBuf,
        reply: Sender<io::Result<Vec<u8>>>,
    },
    /// Wait until a socket can be read or written without blocking - D3's
    /// readiness half. The worker owns the poller *and the descriptor it polls*.
    ///
    /// **A duplicate and not the caller's own**
    /// ([ADR-121](../../../../docs/specification/adr/adr-121.md) D4). While the
    /// only surface was `io::wait`, the caller was blocked for the whole of the
    /// wait and its borrow was the guarantee - `poll_one`'s safety comment said
    /// so. `io::waiting` is the same wait as a **future**, and a future may be
    /// dropped while the operation is still in flight, so the borrow is not
    /// there to be had. A `dup` shares the file description, which is what
    /// readiness is about, and costs one syscall.
    Readiness {
        fd: std::os::fd::OwnedFd,
        interest: Interest,
        timeout: Option<std::time::Duration>,
        reply: Sender<io::Result<bool>>,
    },
    /// All of standard input, read on the worker thread
    /// ([ADR-121](../../../../docs/specification/adr/adr-121.md) D4).
    ///
    /// **Not a second read shape**, which is that decision's own words: a stream
    /// has no size to `stat` and nothing to put on the ring, so this is the
    /// blocking read standard input always had - moved off the thread the
    /// program runs on, now that a worker's reply can wake a park on either
    /// mechanism. The lock `StdinLock` takes is the process's, which is what
    /// makes one reader at a time true here as it is anywhere else.
    Stdin { reply: Sender<io::Result<Vec<u8>>> },
    /// Stop after everything already queued. What shutdown sends (ADR-006 D5).
    Stop,
}

/// Which way a socket has to be ready.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Interest {
    /// There is something to read, or the peer has gone.
    Readable,
    /// A write will not block.
    Writable,
}

thread_local! {
    /// Whether this thread is one of the runtime's I/O workers.
    static IS_IO_WORKER: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Whether the calling thread is an I/O worker.
///
/// The assertion behind ADR-037 D2's line: the I/O thread is the compiler's,
/// so `std`'s user-facing functions are never *on* it.
pub fn on_io_worker() -> bool {
    IS_IO_WORKER.with(std::cell::Cell::get)
}

/// The worker threads, and the queue they share.
pub(super) struct Workers {
    /// `mpsc::Sender` is `Send` but not `Sync`, and the runtime is a
    /// `&'static` every thread reaches - so the handle is behind a lock. At
    /// `user_parallelism = no` there is one user thread and it is uncontended
    /// by construction (ADR-037 D2).
    inbox: Mutex<Sender<Op>>,
    /// Operations queued and not yet answered. Shutdown's deadline watches
    /// this, and it is what makes the drain bounded rather than hopeful
    /// (ADR-006 D5).
    pending: Arc<AtomicUsize>,
    threads: Mutex<Vec<std::thread::JoinHandle<()>>>,
}

impl Workers {
    /// Start `count` of them. At least one, always (D4).
    pub(super) fn start(count: usize) -> Workers {
        let (inbox, outbox) = channel::<Op>();
        let outbox = Arc::new(Mutex::new(outbox));
        let pending = Arc::new(AtomicUsize::new(0));

        let mut threads = Vec::with_capacity(count.max(1));
        for n in 0..count.max(1) {
            let outbox = Arc::clone(&outbox);
            let pending = Arc::clone(&pending);
            let thread = std::thread::Builder::new()
                .name(format!("nikaia-io-{n}"))
                .spawn(move || run(&outbox, &pending))
                .expect("the runtime's I/O thread starts");
            threads.push(thread);
        }

        Workers {
            inbox: Mutex::new(inbox),
            pending,
            threads: Mutex::new(threads),
        }
    }

    /// Queue one operation. `false` if the workers have already stopped, which
    /// is what a caller after shutdown sees.
    pub(super) fn send(&self, op: Op) -> bool {
        self.pending.fetch_add(1, Ordering::SeqCst);
        let inbox = self.inbox.lock().unwrap_or_else(|e| e.into_inner());
        if inbox.send(op).is_err() {
            self.pending.fetch_sub(1, Ordering::SeqCst);
            return false;
        }
        true
    }

    pub(super) fn pending(&self) -> usize {
        self.pending.load(Ordering::SeqCst)
    }

    /// Drain, bounded by `deadline`, then stop (ADR-006 D5).
    ///
    /// Returns how many operations had not finished when the deadline expired,
    /// which is what the warning names. This cannot hang: the timer is the
    /// caller's own clock and does not depend on any operation making
    /// progress.
    pub(super) fn drain(&self, deadline: std::time::Duration) -> usize {
        let until = std::time::Instant::now() + deadline;
        while self.pending() > 0 && std::time::Instant::now() < until {
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        let left = self.pending();

        let threads = {
            let mut held = self.threads.lock().unwrap_or_else(|e| e.into_inner());
            std::mem::take(&mut *held)
        };
        {
            let inbox = self.inbox.lock().unwrap_or_else(|e| e.into_inner());
            for _ in 0..threads.len() {
                self.pending.fetch_add(1, Ordering::SeqCst);
                let _ = inbox.send(Op::Stop);
            }
        }
        if left == 0 {
            for thread in threads {
                let _ = thread.join();
            }
        }
        // With work still outstanding the threads are left to the process's
        // exit rather than joined: joining them is exactly the wait the
        // deadline has already expired on.
        left
    }
}

/// One worker's whole life.
fn run(outbox: &Mutex<Receiver<Op>>, pending: &AtomicUsize) {
    IS_IO_WORKER.with(|flag| flag.set(true));
    loop {
        let op = {
            let held = match outbox.lock() {
                Ok(held) => held,
                Err(poisoned) => poisoned.into_inner(),
            };
            held.recv()
        };
        let Ok(op) = op else { return };
        if matches!(op, Op::Stop) {
            pending.fetch_sub(1, Ordering::SeqCst);
            return;
        }
        perform(op);
        // **The bell first and the count second**, which is the opposite of
        // what this did and the ordering the caller needs.
        //
        // The reply is in its channel by now; the bell says *an answer landed*
        // and `pending` says *something is still outstanding*. A caller that
        // finds neither concludes **nothing can move** — `exec::block_on`
        // panics about a waker nobody arranged, or spins out its
        // `cleanup-deadline` and abandons the task. Dropping the count first
        // opened exactly that window: for the few instructions between the two
        // writes, an operation that had already answered was invisible in both.
        //
        // The other order's argument was that a woken caller should see the
        // finished count rather than the one before it. It costs nothing to be
        // wrong that way round: the caller re-polls, finds the reply, and a
        // `pending` that is briefly one too high only ever makes it **wait**
        // instead of declaring the runtime stuck. Measured before this:
        // `a_future_fed_from_a_worker_finishes_under_block_on` went red about
        // two runs in five of its binary.
        super::ring_the_bell();
        pending.fetch_sub(1, Ordering::SeqCst);
    }
}

/// What a worker actually does. Every arm is `std`'s own code, which is the
/// whole point of `Op` being closed.
fn perform(op: Op) {
    match op {
        Op::Read { path, reply } => {
            let _ = reply.send(std::fs::read(&path));
        }
        Op::Readiness {
            fd,
            interest,
            timeout,
            reply,
        } => {
            let _ = reply.send(poll_one(fd, interest, timeout));
        }
        Op::Stdin { reply } => {
            use std::io::Read;

            let mut bytes = Vec::new();
            let read = std::io::stdin()
                .lock()
                .read_to_end(&mut bytes)
                .map(|_| bytes);
            let _ = reply.send(read);
        }
        Op::Stop => {}
    }
}

/// The blocking write: D3's fallback for a file that leaves the program.
///
/// It runs on the **calling** thread, and there is no `Op` for it. A caller
/// that is going to wait for its own write gains nothing from handing it to
/// another thread: it pays a wake-up and buys no overlap. What would earn the
/// worker is a *batch* of writes, the way [`Op::Read`] earns it for a batch of
/// reads - and no lowering asks for one, so the variant that would carry it is
/// not written until something needs it.
pub(super) fn blocking_write(
    path: &std::path::Path,
    bytes: &[u8],
    append: bool,
    create: bool,
) -> io::Result<()> {
    use std::io::Write;

    if !append && create {
        return std::fs::write(path, bytes);
    }
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .append(append)
        .truncate(!append)
        .create(create)
        .open(path)?;
    file.write_all(bytes)
}

/// Wait for one descriptor to be ready.
///
/// `polling` is the readiness half of D3: a small crate over `epoll`,
/// `kqueue` and IOCP with no runtime, no executor and no opinion about what
/// the program does when the socket is ready. `false` is a timeout rather
/// than a failure, which is what a deadline needs to be able to tell apart.
///
/// The event is one-shot and the poller is built per wait rather than kept:
/// this is the shape a *readiness* answer has, and the socket layer the HTTP
/// server will need (D1, D6) is what turns it into a kept registration. What
/// is here is the mechanism behind one `std` surface, so that change is a
/// `std` change.
fn poll_one(
    fd: std::os::fd::OwnedFd,
    interest: Interest,
    timeout: Option<std::time::Duration>,
) -> io::Result<bool> {
    use polling::{Event, Events, Poller};
    use std::os::fd::AsRawFd;

    let fd = fd.as_raw_fd();
    let poller = Poller::new()?;
    let key = 0usize;
    let event = match interest {
        Interest::Readable => Event::readable(key),
        Interest::Writable => Event::writable(key),
    };
    // SAFETY: the descriptor is the `OwnedFd` this call holds, so it is open
    // for the whole of the wait whatever the caller does - which is what
    // [`Op::Readiness`] is a duplicate for. It is deleted again below, and
    // closed when this function returns.
    unsafe { poller.add(fd, event)? };

    let mut events = Events::new();
    let waited = poller.wait(&mut events, timeout);
    let deleted = poller.delete(unsafe { std::os::fd::BorrowedFd::borrow_raw(fd) });
    waited?;
    deleted?;
    Ok(!events.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The boundary, from the only side a test can see it from: the calling
    /// thread is never an I/O worker, and a worker's own loop always is.
    #[test]
    fn the_calling_thread_is_never_an_io_worker() {
        assert!(!on_io_worker());
        let workers = Workers::start(1);

        // A worker answers, so it ran; and the answer arrived on this thread,
        // which is not one.
        let (reply, answer) = channel();
        let path = std::env::temp_dir().join(format!("nikaia-worker-{}", std::process::id()));
        std::fs::write(&path, "eins").expect("write");
        assert!(workers.send(Op::Read {
            path: path.clone(),
            reply
        }));
        assert_eq!(answer.recv().expect("answered").expect("read"), b"eins");
        assert!(!on_io_worker());

        assert_eq!(workers.drain(std::time::Duration::from_secs(5)), 0);
        std::fs::remove_file(&path).ok();
    }

    /// The drain is bounded by the clock and not by the work, which is the
    /// half of ADR-006 D5 that cannot hang.
    #[test]
    fn the_drain_is_bounded_by_its_deadline() {
        use std::os::fd::AsFd;

        let workers = Workers::start(1);
        let (reply, answer) = channel();
        // A socket nobody writes to: readable never comes, so this operation
        // outlives any deadline.
        let (quiet, _peer) = std::os::unix::net::UnixStream::pair().expect("a socket pair");
        assert!(workers.send(Op::Readiness {
            fd: quiet.as_fd().try_clone_to_owned().expect("a duplicate"),
            interest: Interest::Readable,
            timeout: None,
            reply,
        }));

        let began = std::time::Instant::now();
        let left = workers.drain(std::time::Duration::from_millis(50));
        assert!(began.elapsed() < std::time::Duration::from_secs(2), "hung");
        assert_eq!(left, 1, "the deadline expired with the operation pending");
        drop(answer);
    }

    /// Readiness says "ready" when there is something, and "not yet" when the
    /// timeout runs out first - and the two are told apart rather than both
    /// being a failure.
    #[test]
    fn readiness_tells_ready_from_timed_out() {
        use std::io::Write;
        use std::os::fd::AsFd;

        let (here, there) = std::os::unix::net::UnixStream::pair().expect("a socket pair");
        assert!(
            !poll_one(
                here.as_fd().try_clone_to_owned().expect("a duplicate"),
                Interest::Readable,
                Some(std::time::Duration::from_millis(20))
            )
            .expect("polled"),
            "a socket nobody wrote to is not readable"
        );

        (&there).write_all(b"x").expect("the peer writes");
        assert!(
            poll_one(
                here.as_fd().try_clone_to_owned().expect("a duplicate"),
                Interest::Readable,
                Some(std::time::Duration::from_secs(5))
            )
            .expect("polled"),
            "a socket with a byte waiting is readable"
        );
    }
}
