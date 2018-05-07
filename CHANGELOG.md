## 0.1.0 (TBD)
* Initial release, extracted from body-image crate by the same author,
  with additional changes listed below.

* New `PosRead` trait, `ReadPos` and `ReadSlice` types.

* `GatheringReader` is now generic over `AsRef<[u8]>` (including
  `Bytes`).

* Add benchmarks (cargo bench) of reads from `GatheringReader`,
  chained `std::io::Cursor` and upfront gather with single `Cursor`.

  On my dev host; i7-5600U, rustc 1.27.0-nightly (bd40cbbe1 2018-04-14):
  ``` text
  test gather_chained_cursors   ... bench:     558,877 ns/iter (+/- 90,532)
  test gather_reader            ... bench:      63,256 ns/iter (+/- 2,294)
  test gather_upfront           ... bench:      64,078 ns/iter (+/- 14,701)
  test gather_upfront_read_only ... bench:      40,490 ns/iter (+/- 3,578)
  ```
