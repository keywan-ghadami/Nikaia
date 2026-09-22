//! **A socket in `std`**, driven end to end
//! ([ADR-194](../../../docs/specification/adr/adr-194.md) D1).
//!
//! Its own test binary for `at_yes.rs`'s reason: the runtime is one per process
//! ([ADR-038](../../../docs/specification/adr/adr-038.md) D4), and what is
//! asserted here is that a thread is **given up** rather than held — which a
//! test running after one that asked for something else would measure wrong.

use nikaia_std::net;
use nikaia_std::rt::{self, exec, UserCode};

/// Everything here runs inside the runtime, because the readiness the socket
/// awaits is the runtime's.
fn run<T: Send + 'static>(work: impl std::future::Future<Output = T> + Send + 'static) -> T {
    let started = rt::start(UserCode::Sequential);
    let out = exec::block_on(work);
    started.finish();
    out
}

/// **One connection, both ways**, and the address the listener answers with —
/// which is what a caller that asked for port `0` needs.
#[test]
fn a_listener_accepts_and_the_two_ends_talk() {
    let said = run(async {
        let listener = net::listen("127.0.0.1:0").await.expect("a listener");
        let address = listener.address();
        assert!(address.starts_with("127.0.0.1:"), "{address}");

        let serving = async {
            let mut connection = listener.accept().await.expect("a connection");
            let asked = connection.read().await.expect("what the client said");
            connection
                .write(b"pong")
                .await
                .expect("the answer goes out");
            String::from_utf8_lossy(asked.as_ref()).to_string()
        };
        let asking = async {
            let mut connection = net::connect(&address).await.expect("connected");
            connection.write(b"ping").await.expect("the ask goes out");
            let answer = connection.read().await.expect("the answer");
            String::from_utf8_lossy(answer.as_ref()).to_string()
        };

        futures_join(serving, asking).await
    });

    assert_eq!(said, ("ping".to_string(), "pong".to_string()));
}

/// **An empty read is the peer closing**, which is the one thing a caller has
/// to know to write a loop.
#[test]
fn a_closed_peer_reads_as_nothing() {
    let nothing = run(async {
        let listener = net::listen("127.0.0.1:0").await.expect("a listener");
        let address = listener.address();
        let serving = async {
            let mut connection = listener.accept().await.expect("a connection");
            connection.read().await.expect("a read")
        };
        let asking = async {
            let connection = net::connect(&address).await.expect("connected");
            connection.close();
        };
        let (read, ()) = futures_join(serving, asking).await;
        read.as_ref().len()
    });
    assert_eq!(nothing, 0);
}

/// Two futures to their end, without a dependency on a futures crate.
///
/// `join!` is what this would be; `nikaia-std` does not carry the crate that
/// has one and this is four lines.
async fn futures_join<A, B>(
    a: impl std::future::Future<Output = A>,
    b: impl std::future::Future<Output = B>,
) -> (A, B) {
    use std::pin::pin;
    use std::task::Poll;

    let mut a = pin!(a);
    let mut b = pin!(b);
    let mut done_a = None;
    let mut done_b = None;
    std::future::poll_fn(move |cx| {
        if done_a.is_none() {
            if let Poll::Ready(value) = a.as_mut().poll(cx) {
                done_a = Some(value);
            }
        }
        if done_b.is_none() {
            if let Poll::Ready(value) = b.as_mut().poll(cx) {
                done_b = Some(value);
            }
        }
        match (done_a.is_some(), done_b.is_some()) {
            (true, true) => Poll::Ready((
                done_a.take().expect("polled ready"),
                done_b.take().expect("polled ready"),
            )),
            _ => Poll::Pending,
        }
    })
    .await
}
