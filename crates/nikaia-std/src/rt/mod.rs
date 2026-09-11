//! `std`'s runtime: started before `main`, and the one surface I/O goes
//! through.
//!
//! This module is
//! [ADR-038](../../../../docs/specification/adr/adr-038.md) D3, D4 and D5, and
//! it is deliberately *here* rather than in the compiler. ADR-033 §8.4 gave
//! the reason when it put the statement overlap behind
//! [`crate::task::both`]: the next change of mechanism must be a `std` change
//! and not a compiler change. Nothing about `io_uring`, `epoll`, a thread
//! count or a configuration key appears in a `.nika` file or in the emitted
//! Rust - a program says `fs::read_to_string(p)` and what happens underneath
//! is this file's business (Part I 8.1, Part III 15.x).
//!
//! ## D4 — it is running before the first statement the user wrote
//!
//! An I/O thread that is already running costs nothing per operation; one
//! started per pair of operations costs a wake-up per pair, which is the whole
//! of ADR-033 §8.4's finding. So the emitted `fn main` starts this and then
//! calls the program:
//!
//! ```rust,ignore
//! fn main() {
//!     let runtime = nikaia_std::rt::start(nikaia_std::rt::UserCode::Sequential);
//!     let outcome = nikaia_main();
//!     runtime.finish();
//!     outcome
//! }
//! ```
//!
//! What starts is what [ADR-037](../../../../docs/specification/adr/adr-037.md)
//! D2 allows. At `user_parallelism = no`: the I/O workers, and everything the
//! user wrote on the main thread. At `yes`: a pool for user code as well. The
//! asymmetry is D2's own - the compiler's threads were never bounded by that
//! switch, and the I/O thread is one of the compiler's ([ADR-016](../../../../docs/specification/adr/adr-016.md) D3).
//!
//! ## D3 — the two halves of I/O, behind one surface
//!
//! * **Files complete.** [`uring`] hands the read to the kernel and collects
//!   it; no thread performs it and none is woken for it.
//! * **Sockets signal readiness.** [`worker`]'s poller says when a descriptor
//!   can be read or written without blocking, and the caller does the transfer.
//! * **The fallback is the blocking path on an I/O worker**, for the machine
//!   that has no completion queue: an older kernel, a sandbox that forbids the
//!   syscalls, a target that never had them.
//!
//! **Which one is chosen is decided when the program starts, never when it is
//! compiled.** A binary built on a machine with `io_uring` runs on one without
//! it. The only compile-time gate is on `io_uring`'s *bindings*, which do not
//! exist off Linux at all; on Linux the choice is [`Method::Auto`] probing the
//! kernel, or an operator pinning it (D5).

pub mod config;
pub mod worker;

#[cfg(target_os = "linux")]
mod uring;

pub use config::{Config, Method};
pub use worker::Interest;

use std::path::Path;
use std::sync::{Mutex, OnceLock};

/// Whether the program's own code may run concurrently
/// ([ADR-037](../../../../docs/specification/adr/adr-037.md) D2), as the
/// runtime sees it.
///
/// The emitted `main` says which, because the *compiler* is what knows: it is
/// a build switch, not an operating property, and it is deliberately not one
/// of D5's four settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UserCode {
    /// `user_parallelism = no`. Nothing the user wrote ever runs concurrently,
    /// so no pool for user code starts. The I/O workers still do: they are the
    /// compiler's.
    #[default]
    Sequential,
    /// `user_parallelism = yes`. A pool for user code starts with the runtime.
    Concurrent,
}

/// Which mechanism this process actually got, after detection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Files {
    /// The kernel completes the read (D3's first half).
    Completion,
    /// The blocking path on an I/O worker (D3's fallback).
    Blocking,
}

impl Files {
    pub fn as_str(self) -> &'static str {
        match self {
            Files::Completion => "completion",
            Files::Blocking => "blocking",
        }
    }
}

/// The runtime. One per process, started before `main`.
pub struct Runtime {
    config: Config,
    user_code: UserCode,
    files: Files,
    workers: worker::Workers,
    #[cfg(target_os = "linux")]
    ring: Option<Mutex<uring::Ring>>,
    /// The pool for user code, at `user_parallelism = yes` and not otherwise.
    ///
    /// `None` at `Sequential` is not an optimisation. It is ADR-037 D2 as a
    /// thread count: there is no vehicle, so nothing the user wrote *can* run
    /// concurrently, whatever a later mistake in the emitter asks for.
    user_pool: Option<rayon::ThreadPool>,
}

static RUNTIME: OnceLock<Runtime> = OnceLock::new();

/// Start the runtime, and hand back the guard whose `finish` drains it.
///
/// Called by the emitted `fn main` before the program's first statement (D4).
/// Calling it twice is not an error and does not start a second runtime - a
/// library's test harness and a generated `main` may both reach for it, and
/// the process has one runtime either way.
pub fn start(user_code: UserCode) -> Started {
    let _ =
        RUNTIME.get_or_init(|| Runtime::build(user_code, Config::load().unwrap_or_else(report)));
    Started { _private: () }
}

/// The runtime, starting it if `main` was not one this compiler emitted.
///
/// A `std` function must work in a unit test and in a `build.rs`, neither of
/// which has a generated `main`. That is a *fallback* and not the design: the
/// first operation in such a process pays the start, which is exactly the cost
/// D4 exists to remove.
pub fn handle() -> &'static Runtime {
    RUNTIME
        .get_or_init(|| Runtime::build(UserCode::default(), Config::load().unwrap_or_else(report)))
}

/// A configuration file that does not parse is the operator's to fix, and a
/// program that silently ran on the defaults instead would hide it. The
/// program still starts - refusing to start over a tuning file would be worse
/// - and the reason is on standard error, once.
fn report(why: String) -> Config {
    eprintln!("nikaia: {why}");
    eprintln!("nikaia: the runtime is starting on its default settings");
    Config::default()
}

/// What [`start`] hands back: the drain, as a value a generated `main` holds.
///
/// Not a `Drop` guard, deliberately. ADR-006 D5's drain has an outcome - it
/// names every resource that did not finish - and a destructor is the wrong
/// place for something that reports.
#[must_use = "the runtime is drained by calling `finish`"]
pub struct Started {
    _private: (),
}

impl Started {
    /// Drain, bounded by the configured deadline, and warn about what did not
    /// finish (ADR-006 D5).
    pub fn finish(self) {
        let runtime = handle();
        let deadline = runtime.config.cleanup_deadline;
        if deadline.is_zero() {
            // "0" disables draining, which is ADR-006 D5's own word for it.
            return;
        }
        let left = runtime.workers.drain(deadline);
        if left > 0 {
            eprintln!(
                "nikaia: {left} pending I/O operation(s) did not finish within \
                 the {}s cleanup deadline and were abandoned",
                deadline.as_secs_f64()
            );
        }
    }
}

impl Runtime {
    fn build(user_code: UserCode, config: Config) -> Runtime {
        let workers = worker::Workers::start(config.io_workers);

        #[cfg(target_os = "linux")]
        let ring = match config.io_method {
            // **Run-time detection.** `IoUring::new` is the probe: an older
            // kernel answers `ENOSYS` and a sandbox that forbids the syscall
            // answers `EPERM`, and either way the program runs on the
            // blocking path rather than failing to start.
            Method::Auto => uring::Ring::open().ok().map(Mutex::new),
            Method::Uring => match uring::Ring::open() {
                Ok(ring) => Some(Mutex::new(ring)),
                // Pinned, and not available: a failure to start. The whole
                // point of pinning a mechanism is to find out whether it is
                // there, so falling back silently would defeat it (D5).
                Err(e) => panic!(
                    "nikaia: `io-method = \"uring\"` was pinned in {} and this machine \
                     has no io_uring ({e}); `auto` falls back, `uring` does not",
                    config::FILE
                ),
            },
            Method::Blocking => None,
        };

        #[cfg(target_os = "linux")]
        let files = if ring.is_some() {
            Files::Completion
        } else {
            Files::Blocking
        };
        #[cfg(not(target_os = "linux"))]
        let files = Files::Blocking;

        let user_pool = match user_code {
            UserCode::Sequential => None,
            UserCode::Concurrent => rayon::ThreadPoolBuilder::new()
                // `0` is rayon's own "as many as the machine has", which is
                // the default D5 documents and is not a count somebody typed.
                .num_threads(config.user_pool)
                .thread_name(|n| format!("nikaia-user-{n}"))
                .build()
                .ok(),
        };

        Runtime {
            config,
            user_code,
            files,
            workers,
            #[cfg(target_os = "linux")]
            ring,
            user_pool,
        }
    }

    /// The four settings this process started on.
    pub fn config(&self) -> Config {
        self.config
    }

    /// Which mechanism files actually got, after detection.
    pub fn files(&self) -> Files {
        self.files
    }

    /// What `user_parallelism` said.
    pub fn user_code(&self) -> UserCode {
        self.user_code
    }

    /// The pool for user code, which exists only at `user_parallelism = yes`.
    pub fn user_pool(&self) -> Option<&rayon::ThreadPool> {
        self.user_pool.as_ref()
    }

    /// How many I/O operations are queued and unanswered.
    pub fn pending(&self) -> usize {
        self.workers.pending()
    }

    /// One line naming what started and how, for `--runtime` and for a bug
    /// report that has to say which path it was on.
    pub fn describe(&self) -> String {
        format!(
            "io-workers={} io-method={} (chose {}) user-pool={} cleanup-deadline={:?} user-code={}",
            self.config.io_workers,
            self.config.io_method.as_str(),
            self.files.as_str(),
            match self.user_pool.as_ref() {
                Some(pool) => pool.current_num_threads().to_string(),
                None => "none".to_string(),
            },
            self.config.cleanup_deadline,
            match self.user_code {
                UserCode::Sequential => "sequential",
                UserCode::Concurrent => "concurrent",
            }
        )
    }

    #[cfg(target_os = "linux")]
    fn with_ring<T>(&self, f: impl FnOnce(&mut uring::Ring) -> T) -> Option<T> {
        let ring = self.ring.as_ref()?;
        // A panic inside a file operation poisons this lock and must not lose
        // the ring: the buffers the kernel holds live in it, and dropping them
        // is the one thing `uring`'s soundness rule forbids. Recovering is
        // sound because the *next* operation reconciles the ring before it
        // submits anything.
        let mut held = ring.lock().unwrap_or_else(|e| e.into_inner());
        Some(f(&mut held))
    }
}

/// I/O, as the one surface D3 asks for.
///
/// Every function here is what a Nikaia program reaches through `std::fs` and
/// `std::net`. Which mechanism serves it is decided inside, at startup, and a
/// caller cannot tell - which is what makes a change of mechanism a `std`
/// change.
pub mod io {
    use super::{handle, worker, Files, Interest, Path};
    use std::io::{Error, Result};

    /// The invariant ADR-037 D2 rests on, checked rather than assumed.
    ///
    /// The I/O thread is the compiler's, so `std`'s user-facing functions are
    /// never *on* it: one of them being called from a worker would mean a
    /// worker had been handed something other than an [`worker::Op`], which
    /// the type system already forbids. A `debug_assert` is the belt for the
    /// braces.
    fn off_the_io_thread() {
        debug_assert!(
            !worker::on_io_worker(),
            "a `std` I/O call reached an I/O worker; the worker's inbox takes \
             operations, never code (ADR-037 D2)"
        );
    }

    /// A whole file, as bytes.
    pub fn read(path: &Path) -> Result<Vec<u8>> {
        off_the_io_thread();
        let mut both = read_all(&[path]);
        both.pop().expect("one path, one answer")
    }

    /// Two files, **both in flight at once**, answered in the order they were
    /// asked for.
    ///
    /// This is the shape ADR-033 §8.5 predicted and could not measure: two
    /// operations that meet on nothing, neither waiting for the other, and no
    /// thread started or woken for the pair. `benches/overlap/src/bin/runtime.rs`
    /// is what puts the number on it.
    ///
    /// It is `std`'s own function over `std`'s own operations, which is why it
    /// exists at `user_parallelism = no`: nothing the *user* wrote runs
    /// concurrently here, and the two reads are `std`'s
    /// ([ADR-037](../../../../docs/specification/adr/adr-037.md) D2).
    pub fn read_both(a: &Path, b: &Path) -> (Result<Vec<u8>>, Result<Vec<u8>>) {
        off_the_io_thread();
        let mut both = read_all(&[a, b]);
        let second = both.pop().expect("two paths, two answers");
        let first = both.pop().expect("two paths, two answers");
        (first, second)
    }

    /// Every path, all of them in flight at once. The one place the mechanism
    /// is chosen.
    fn read_all(paths: &[&Path]) -> Vec<Result<Vec<u8>>> {
        let runtime = handle();
        match runtime.files() {
            #[cfg(target_os = "linux")]
            Files::Completion => runtime
                .with_ring(|ring| ring.read_many(paths))
                .expect("`Files::Completion` means there is a ring"),
            #[cfg(not(target_os = "linux"))]
            Files::Completion => unreachable!("no completion queue off Linux"),
            Files::Blocking => blocking_read_all(paths),
        }
    }

    /// D3's fallback, for the machine with no completion queue.
    ///
    /// **The last operation of a batch runs on the calling thread**, and the
    /// rest go to the I/O workers, which are already running - so a pair of
    /// reads is *one* message rather than two, and a lone read is none at all.
    ///
    /// That split is the measurement, not a micro-optimisation. A caller that
    /// is going to block until the answer arrives gains nothing from handing
    /// its own work to another thread and waiting for it: it pays a wake-up
    /// and buys no overlap. Where a *pair* is asked for, the second operation
    /// has something to overlap with and the wake-up buys something - and that
    /// wake-up is exactly the cost ADR-033 §8.4 measured and could not remove,
    /// which is why D3 does not put files here when the kernel can complete
    /// them instead.
    fn blocking_read_all(paths: &[&Path]) -> Vec<Result<Vec<u8>>> {
        let runtime = handle();
        let (queued, here) = paths.split_at(paths.len().saturating_sub(1));

        let mut waiting = Vec::with_capacity(queued.len());
        for path in queued {
            let (reply, answer) = std::sync::mpsc::channel();
            let sent = runtime.workers.send(worker::Op::Read {
                path: path.to_path_buf(),
                reply,
            });
            waiting.push(sent.then_some(answer));
        }

        // Started last and finished first: the operations already in flight
        // are the ones being overlapped with.
        let mine = here.first().map(std::fs::read);

        let mut out: Vec<Result<Vec<u8>>> = waiting
            .into_iter()
            .map(|answer| match answer {
                Some(answer) => answer
                    .recv()
                    .unwrap_or_else(|_| Err(Error::other("the runtime's I/O worker went away"))),
                None => Err(Error::other("the runtime has already been drained")),
            })
            .collect();
        out.extend(mine);
        out
    }

    /// A whole file, written.
    ///
    /// **The copy happens where the kernel needs it and nowhere else.** On the
    /// completion path the buffer handed over has to outlive the submission, so
    /// `std` gives the ring a `Vec` it owns outright - that is `rt::uring`'s
    /// soundness rule and not a convenience, and there is no borrowed buffer to
    /// keep alive because there is no borrowed buffer. On the blocking path the
    /// caller is blocked for the whole of the write, so the bytes are written
    /// straight out of the caller's own slice and nothing is copied.
    pub fn write(path: &Path, bytes: &[u8], append: bool, create: bool) -> Result<()> {
        off_the_io_thread();
        let runtime = handle();
        match runtime.files() {
            #[cfg(target_os = "linux")]
            Files::Completion => runtime
                .with_ring(|ring| ring.write(path, bytes.to_vec(), append, create))
                .expect("`Files::Completion` means there is a ring"),
            #[cfg(not(target_os = "linux"))]
            Files::Completion => unreachable!("no completion queue off Linux"),
            // One operation, and the caller is going to wait for it: a worker
            // would cost a wake-up and buy no overlap.
            Files::Blocking => worker::blocking_write(path, bytes, append, create),
        }
    }

    /// Wait until a socket can be read or written without blocking.
    ///
    /// D3's other half: a socket is *not* a file. `epoll` answers a socket
    /// exactly and answers a file uselessly, which is why the two halves have
    /// different mechanisms behind this one surface. `Ok(false)` is the
    /// timeout expiring, which a deadline has to be able to tell from a
    /// failure.
    ///
    /// The wait happens on an I/O worker, which is already running - so
    /// waiting costs a message rather than a thread.
    pub fn wait(
        socket: &impl std::os::fd::AsFd,
        interest: Interest,
        timeout: Option<std::time::Duration>,
    ) -> Result<bool> {
        use std::os::fd::AsRawFd;

        off_the_io_thread();
        let runtime = handle();
        let (reply, answer) = std::sync::mpsc::channel();
        let queued = runtime.workers.send(worker::Op::Readiness {
            fd: socket.as_fd().as_raw_fd(),
            interest,
            timeout,
            reply,
        });
        if !queued {
            return Err(Error::other("the runtime has already been drained"));
        }
        // `socket` is borrowed for the whole of this call, so the descriptor
        // the worker polls cannot be closed underneath it.
        answer
            .recv()
            .unwrap_or_else(|_| Err(Error::other("the runtime's I/O worker went away")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// D4, from the only angle a test can see it: the runtime a `std` call
    /// finds is already there, and it is the same one every call finds.
    #[test]
    fn there_is_one_runtime_and_it_is_already_running() {
        let first = handle();
        let second = handle();
        assert!(std::ptr::eq(first, second), "two runtimes started");
        assert!(first.config().io_workers >= 1, "D4: one I/O thread always");
    }

    /// ADR-037 D2 as a thread count. Nothing the user wrote can run
    /// concurrently at `Sequential`, because there is nothing to run it on.
    #[test]
    fn a_sequential_build_starts_no_pool_for_user_code() {
        let runtime = Runtime::build(UserCode::Sequential, Config::default());
        assert!(runtime.user_pool().is_none());
        assert_eq!(runtime.pending(), 0);

        // …and the I/O worker started anyway, because it is the compiler's
        // thread and was never bounded by that switch.
        let path = std::env::temp_dir().join(format!("nikaia-rt-seq-{}", std::process::id()));
        std::fs::write(&path, "eins").expect("write");
        assert!(runtime.describe().contains("user-code=sequential"));
        runtime.workers.drain(std::time::Duration::from_secs(5));
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn a_concurrent_build_starts_one() {
        let runtime = Runtime::build(UserCode::Concurrent, Config::default());
        assert!(runtime.user_pool().is_some());
        assert!(runtime.describe().contains("user-code=concurrent"));
        runtime.workers.drain(std::time::Duration::from_secs(5));
    }

    /// The fallback is not a hypothetical: pinning `blocking` produces a
    /// runtime that never touches a ring, and it answers the same bytes.
    #[test]
    fn the_blocking_fallback_reads_what_the_completion_path_reads() {
        let path = std::env::temp_dir().join(format!("nikaia-rt-both-{}", std::process::id()));
        let text = "Hamburg;12.0\n".repeat(10_000);
        std::fs::write(&path, &text).expect("write");

        let blocking = Runtime::build(
            UserCode::Sequential,
            Config {
                io_method: Method::Blocking,
                ..Config::default()
            },
        );
        assert_eq!(blocking.files(), Files::Blocking);

        let auto = Runtime::build(UserCode::Sequential, Config::default());
        let one = read_through(&blocking, &path);
        let other = read_through(&auto, &path);
        assert_eq!(one, text.as_bytes(), "the fallback read the file");
        assert_eq!(other, one, "and the chosen mechanism read the same bytes");

        blocking.workers.drain(std::time::Duration::from_secs(5));
        auto.workers.drain(std::time::Duration::from_secs(5));
        std::fs::remove_file(&path).ok();
    }

    /// A pair through a named runtime, so the test can compare two mechanisms
    /// rather than only whichever one this machine chose.
    fn read_through(runtime: &Runtime, path: &Path) -> Vec<u8> {
        match runtime.files() {
            #[cfg(target_os = "linux")]
            Files::Completion => runtime
                .with_ring(|ring| ring.read_many(&[path]))
                .expect("a ring")
                .pop()
                .expect("one answer")
                .expect("read"),
            #[cfg(not(target_os = "linux"))]
            Files::Completion => unreachable!(),
            Files::Blocking => {
                let (reply, answer) = std::sync::mpsc::channel();
                assert!(runtime.workers.send(worker::Op::Read {
                    path: path.to_path_buf(),
                    reply
                }));
                answer.recv().expect("answered").expect("read")
            }
        }
    }

    /// Both mechanisms answer a pair in the order it was asked for, which is
    /// what makes the surface one surface.
    #[test]
    fn a_pair_is_answered_in_order_on_either_mechanism() {
        let dir = std::env::temp_dir().join(format!("nikaia-rt-pair-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("scratch");
        let (a, b) = (dir.join("eins"), dir.join("zwei"));
        std::fs::write(&a, "eins").expect("write");
        std::fs::write(&b, "zwei").expect("write");

        for method in [Method::Auto, Method::Blocking] {
            let runtime = Runtime::build(
                UserCode::Sequential,
                Config {
                    io_method: method,
                    io_workers: 2,
                    ..Config::default()
                },
            );
            assert_eq!(read_through(&runtime, &a), b"eins", "{method:?}");
            assert_eq!(read_through(&runtime, &b), b"zwei", "{method:?}");
            runtime.workers.drain(std::time::Duration::from_secs(5));
        }
        std::fs::remove_dir_all(&dir).ok();
    }

    /// D3's other half, through the surface rather than through the mechanism:
    /// a socket says when it is readable, and the wait happens on an I/O
    /// worker that was already running.
    ///
    /// `epoll` answers a socket exactly and answers a file uselessly, which is
    /// the whole reason the two halves have different mechanisms behind one
    /// `std` surface - so this is the test that the socket half is reachable
    /// from where a program would reach it.
    #[test]
    fn a_socket_reports_readiness_through_the_std_surface() {
        use std::io::{Read, Write};
        use std::os::unix::net::UnixStream;

        let (here, there) = UnixStream::pair().expect("a socket pair");

        // Nothing sent: not readable, and a timeout is `Ok(false)` rather than
        // a failure - a deadline has to be able to tell those apart.
        assert!(!io::wait(
            &here,
            Interest::Readable,
            Some(std::time::Duration::from_millis(20))
        )
        .expect("polled"));

        // A byte from the peer, and the same call says so.
        (&there).write_all(b"x").expect("the peer writes");
        assert!(io::wait(
            &here,
            Interest::Readable,
            Some(std::time::Duration::from_secs(5))
        )
        .expect("polled"));

        // …and the transfer is the caller's, which is what "readiness" means:
        // the runtime said when, the program says what.
        let mut byte = [0u8; 1];
        (&here).read_exact(&mut byte).expect("read");
        assert_eq!(byte, [b'x']);

        // A socket with room is writable straight away, so both interests are
        // reachable rather than only the one a test happened to need.
        assert!(io::wait(&here, Interest::Writable, None).expect("polled"));
    }

    /// What the `--runtime` line has to be able to say.
    #[test]
    fn the_description_names_the_mechanism_that_was_chosen() {
        let runtime = Runtime::build(UserCode::Sequential, Config::default());
        let said = runtime.describe();
        assert!(said.contains("io-method=auto"), "{said}");
        assert!(
            said.contains("chose completion") || said.contains("chose blocking"),
            "the report has to name which one detection picked: {said}"
        );
        runtime.workers.drain(std::time::Duration::from_secs(5));
    }
}
