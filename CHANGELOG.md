## 1.4.0 (unreleased)

## 1.3.0 (2020-1-3)
* Expand bytes dev dependency to include 0.5.z.

* Minimum supported rust version is now 1.34.0. CI testing is limited to the
  older bytes 0.4.12 release on MSRV.  For the latest bytes 0.5.z, tests
  require rust ≥1.39.0.

* As `AtomicU64` was stabilized in rust 1.34.0, `olio::mem::MemAdvice` and
  `MemHandle` now always uses u64 representation.

## 1.2.0 (2019-9-30)
* Make `olio::mem::MemHandle` a `Sync` type, by replacing the per-handle `Cell`
  (for cache of last advice) with another atomic.

* Fix build.rs for `rustc --version` not including git metadata.

* Update dev dependencies rand (0.7.0) and tempfile (3.1.0).

* Minimum supported rust version is now 1.32.0 (to match above dep updates).

## 1.1.0 (2019-5-13)
* On rust 1.34+, where `AtomicU64` is stable, use u64 representation of
  `olio::mem::MemAdvice` and `MemHandle` internal state, instead of usize, as
  it affords room for 6 advise levels above baseline (currently `Normal`) on
  all supported platforms.

* Narrow various dependencies to avoid future MINOR versions, for reliability.
  We may subsequently make PATCH releases which _broaden_ private or public
  dependencies to include new MINOR releases found _compatible_. Rationale:
  Cargo fails to consider MSRV for dependency resolution (rust-lang/rfcs#2495),
  and MSRV bumps are common in MINOR releases, including as planned here.

* Add build.rs script to fail fast on an attempt to compile with a rustc below
  MSRV, which remains 1.31.0.

## 1.0.1 (2019-3-6)
* Use `AtomicUsize::new`, a `const fn` since rustc 1.24.0. `ATOMIC_USIZE_INIT`
  was deprecated as of rust 1.34.0 (nightly).

## 1.0.0 (2018-12-4)
* Update to the rust 2018 edition, including the changes to pass all 2018 idiom
  lints (anchored paths, anonymous/elided lifetimes, dyn Trait).  _This start
  of the 1.x release series has a minimum supported rust version of 1.31.0, and
  is thus potentially less stable than prior 0.x releases, which will continue
  to be maintained as needed._

* Update (dev dep) to rand 0.6.1, fix deprecation in test.

## 0.5.0 (2018-9-20)
* Use u64 offset with latest *memmap* crate release 0.7.0 in
  `ReadSlice::mem_map`. The *memmap* crate minimum version is now 0.7.0.

* Minimum supported rust version is now 1.27.2.

## 0.4.0 (2018-8-13)
* New `mem::MemHandle` wrapper for `Mmap` or other `Deref` byte buffer types,
  offering concurrent-aware access advice. This is currently limited to \*nix
  `libc::posix_madvise` with a subset of advice flags, and is no-op on other
  platforms.

## 0.3.0 (2018-5-22)
* Make `ReadPos` and `ReadSlice` generic over `PosRead` trait, and owned or
  reference `File` types (#1):
  * Implement `PosRead` trait generically over all `Borrow<File>`.
  * Move `ReadPos` and `ReadSlice` types to `olio::fs` module and make them
    generic over any `PosRead`. In combination with the above change, for
    example, this supports an owned `ReadPos<File>` or shared reference
    `ReadPos<&File>` or `ReadPos<Arc<File>>`.
  * The existing `olio::fs::rc::ReadPos` and `ReadSlice` become type aliases
    of the `Arc<File>` forms of the above, so no breaking change.

* New benchmarks (cargo bench read_all) for sanity checking
  `ReadPos`/`ReadSlice` reads vs direct/raw `File` reads. As expected, the
  differences here are small (within error margins) in comparison to file I/O
  system call cost even with fast SSD/OS-cache. Also, the generic changes did
  not have a measurable effect.

  dev i7-5600U, rustc 1.27.0-nightly (acd3871ba 2018-05-10):
  ``` text
  test read_all_pos   ... bench:   1,716,227 ns/iter (+/- 127,651)
  test read_all_raw   ... bench:   1,721,675 ns/iter (+/- 85,265)
  test read_all_slice ... bench:   1,712,060 ns/iter (+/- 140,824)
  ```

* `GatheringReader` benchmark improvements with latest rust nightly:

  dev i7-5600U, rustc 1.27.0-nightly (acd3871ba 2018-05-10):
  ``` text
  test gather_chained_cursors   ... bench:     540,762 ns/iter (+/- 11,658)
  test gather_reader            ... bench:      34,323 ns/iter (+/- 7,333)
  test gather_upfront           ... bench:      45,337 ns/iter (+/- 1,058)
  test gather_upfront_read_only ... bench:      24,184 ns/iter (+/- 901)
  ```

## 0.2.0 (2018-5-8)
* Add _mmap_ as "meta" feature over _memmap_.

## 0.1.0 (2018-5-7)
* Initial release, extracted from body-image crate by the same author,
  with additional changes listed below.

* New `PosRead` trait, `ReadPos` and `ReadSlice` types.

* `GatheringReader` is now generic over `AsRef<[u8]>` (including
  `Bytes`).

* New benchmarks (cargo bench) of reads from `GatheringReader`,
  chained `std::io::Cursor` and "upfront" gather with a single `Cursor`.

  On my dev host; i7-5600U, rustc 1.27.0-nightly (bd40cbbe1 2018-04-14):
  ``` text
  test gather_chained_cursors   ... bench:     558,877 ns/iter (+/- 90,532)
  test gather_reader            ... bench:      63,256 ns/iter (+/- 2,294)
  test gather_upfront           ... bench:      64,078 ns/iter (+/- 14,701)
  test gather_upfront_read_only ... bench:      40,490 ns/iter (+/- 3,578)
  ```

  Where `gather_chained_cursors` uses standard `Cursor` `Read::chain`
  over each buffer and demonstrates the need for the custom
  `GatheringReader` in `gather_reader`.  Benchmark `gather_upfront`
  includes timing the gather operation before a single Cursor
  based read, and `gather_upfront_read_only` only times the same
  Cursor based read.  Particular CPU/RAM bandwidth, CPU cache size,
  body size and concurrency may all effect the relative performance of
  the `GatheringReader` vs. upfront gather.
