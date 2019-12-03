# File Descriptor Reactor

Provides an async reactor for handling file descriptors in a background thread.

The purpose of this crate is to provide a standardized means of creating generic `std::future::Future` types which need to register file descriptors -- and which are independent of a particular async runtime -- as opposed to rolling their own reactors on a background thread. Futures created with this would be universally compatible with both [async-std] and [tokio], and share the same background thread.

[async-std]: https://github.com/async-rs/async-std
[tokio]: https://github.com/tokio-rs/tokio

## Implementation Notes

- The reactor's background thread is spawned on the first time that the reactor handle is fetched.
- Each file descriptor registers an interest to listen for.
- On registering a new file descriptor, a pipe is used to interrupt the poll operation.

## License

Licensed under either of

 * Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

#### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
