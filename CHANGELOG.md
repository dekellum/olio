## TBD

* `GatheringReader` benchmark improvements with latest rust nightly:

  dev host; i7-5600U, rustc 1.27.0-nightly (acd3871ba 2018-05-10):
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
