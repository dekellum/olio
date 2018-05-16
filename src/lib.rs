//! This crate provides I/O-related utilities complimenting the Rust Standard
//! Library `std::io`, `std::fs`, etc.
//!
//! ## Optional Features
//!
//! _mmap (default):_ Adds `fs::ReadSlice::mem_map` support for memory
//! mapping.

#[cfg(feature = "mmap")] extern crate memmap;

/// The crate version string.
pub static VERSION: &str = env!("CARGO_PKG_VERSION");

/// Filesystem extensions and utilities.
///
/// The `PosRead` trait offers a uniform `pread` for positioned reads.
///
/// The `ReadPos` and `ReadSlice` types support multiple independent instance
/// positions over a `Borrow` of `File` (or other `PosRead` type), without
/// needing a path to open an independent new `File` instance.  Thus they are
/// compatible with "unnamed" (not linked) temporary files, and can reduce the
/// number of necessary file handles.  Note that unix `dup`/`dup2` and the
/// standard `File::try_clone` do _not_ provide independent file positions.
pub mod fs {
    mod pos_read;
    pub use fs::pos_read::PosRead;

    mod read;
    pub use fs::read::{ReadPos, ReadSlice, ReadSubSlice};

    /// Compatibility type aliases.
    pub mod rc {
        use std::fs::File;
        use std::sync::Arc;

        /// Use the full generic form instead.
        #[deprecated]
        pub type ReadPos = super::ReadPos<File, Arc<File>>;

        /// Use the full generic form instead.
        #[deprecated]
        pub type ReadSlice = super::ReadSlice<File, Arc<File>>;
    }
}

/// I/O extensions and utilities
pub mod io {
    mod gather;
    pub use io::gather::GatheringReader;
}
