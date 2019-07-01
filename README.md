# olio

[![crates.io](https://img.shields.io/crates/v/olio.svg?maxAge=3600)](https://crates.io/crates/olio)
[![Rustdoc](https://docs.rs/olio/badge.svg)](https://docs.rs/olio)
[![Travis CI Build](https://travis-ci.org/dekellum/olio.svg?branch=master)](https://travis-ci.org/dekellum/olio)
[![Appveyor CI Build](https://ci.appveyor.com/api/projects/status/x5tf8nomocbl787w/branch/master?svg=true)](https://ci.appveyor.com/project/dekellum/olio)
[![deps status](https://deps.rs/repo/github/dekellum/olio/status.svg)](https://deps.rs/repo/github/dekellum/olio)

Provides I/O-related utilities complimenting the Rust Standard Library
`std::io`, `std::fs`, etc.

* The _fs_ module includes a `PosRead` trait, offering a uniform pread
  for positioned file reads; and a `ReadSlice` supporting multiple
  independent reader instances limited to a fixed start..end range.

* The _io_ module includes a `GatheringReader`, which presents a
  continuous Read interface over N non-contiguous byte buffers.

* The _mem_ module includes a `MemHandle` supporting prioritized
  concurrent memory access advice (e.g. madvise (2) on unix).

## Minimum supported rust version (MSRV)

1.32.0

The project will fail fast on any lower rustc (via a build.rs version
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
