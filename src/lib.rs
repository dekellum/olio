//! This crate provides I/O-related utilities complimenting the Rust Standard
//! Library `std::io`, `std::fs`, etc.
//!
//! ## Optional Features
//!
//! _mmap (default):_ Adds `fs::rc::ReadSlice::mem_map` support for memory
//! mapping.

#[cfg(feature = "mmap")] extern crate memmap;

/// The crate version string.
pub static VERSION: &str = env!("CARGO_PKG_VERSION");

/// Filesystem extensions and utilities.
///
/// The `PosRead` trait offers a uniform `pread` for positioned reads.
pub mod fs {

    mod pos_read;
    pub use fs::pos_read::PosRead;

    /// Shared, reference counted `File` extensions and utilities.
    ///
    /// The `ReadPos` and `ReadSlice` types support multiple independent instance
    /// positions over a shared `File`, without needing a path to open an
    /// independent new `File` instance.  Thus they are compatible with "unnamed"
    /// (not linked) temporary files, and can reduce the number of necessary file
    /// handles.  Note that unix `dup`/`dup2` and the standard `File::try_clone`
    /// do _not_ provide independent file positions.
    pub mod rc {
        mod read;
        pub use fs::rc::read::{ReadPos, ReadSlice};
    }
}

/// I/O extensions and utilities
pub mod io {
    mod gather;
    pub use io::gather::GatheringReader;
}
