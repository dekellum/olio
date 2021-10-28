# olio

[![Rustdoc](https://docs.rs/olio/badge.svg)](https://docs.rs/olio)
[![Change Log](https://img.shields.io/crates/v/olio.svg?maxAge=3600&label=change%20log&color=9cf)](https://github.com/dekellum/olio/blob/main/CHANGELOG.md)
[![crates.io](https://img.shields.io/crates/v/olio.svg?maxAge=3600)](https://crates.io/crates/olio)
[![CI Status](https://github.com/dekellum/olio/workflows/CI/badge.svg?branch=main)](https://github.com/dekellum/olio/actions?query=workflow%3ACI)

Provides I/O-related utilities complimenting the Rust Standard Library
`std::io`, `std::fs`, etc.

* The _fs_ module includes a `PosRead` trait, offering a uniform pread
  for positioned file reads; and a `ReadSlice` supporting multiple
  independent reader instances limited to a fixed start..end range.

* The _io_ module includes a `GatheringReader`, which presents a
  continuous Read interface over N non-contiguous byte buffers.

* The _mem_ module includes a `MemHandle` supporting prioritized
  concurrent memory access advice (e.g. madvise (2) on unix).

## Minimum supported rust version

MSRV := 1.39.0

The crate will fail fast on any lower rustc (via a build.rs version
check) and is also CI tested on this version.

## License

This project is dual licensed under either of following:

* The Apache License, version 2.0 ([LICENSE-APACHE](LICENSE-APACHE)
  or http://www.apache.org/licenses/LICENSE-2.0)

* The MIT License ([LICENSE-MIT](LICENSE-MIT)
  or http://opensource.org/licenses/MIT)

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in olio by you, as defined by the Apache License, shall be dual
licensed as above, without any additional terms or conditions.
