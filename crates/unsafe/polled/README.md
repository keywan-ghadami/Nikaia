# polled

A readiness poller (over `polling`) that **owns every descriptor it watches**.
`polling::Poller::add` is `unsafe` for one reason: a descriptor must be taken
out of the poller before it is closed. Here that cannot go wrong:

* `add` takes an `OwnedFd`;
* `remove` deletes the registration first, and only then hands the descriptor
  back;
* dropping the poller closes it before any descriptor it held.

## Every `unsafe`, and why it is sound

| where | what | why it holds |
| :--- | :--- | :--- |
| `Poller::add` | `polling::Poller::add` | the descriptor goes into the poller's own map under the key it is registered with, and leaves it only through `remove`, which deletes the registration first, or when the poller is dropped, after the poller itself is closed |

## How it is checked

```sh
cargo test
cargo clippy --all-targets -- -D warnings
```

The tests run on the real kernel. **Miri cannot run them**, since it has no
`epoll`, and it could not judge this `unsafe` anyway: it is a contract with
the operating system (a descriptor stays open while it is registered), not a
question about memory. What upholds it is the ownership structure above.
