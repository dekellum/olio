//! This crate provides various I/O utilities arranged in a similar way as the
//! rust std.
//!
//! ## Optional Features
//!
//! _memmap (default):_ Adds `fs::rc::read::ReadSlice::mem_map` support for
//! memory mapping

#[cfg(feature = "memmap")] extern crate memmap;

/// The crate version string.
pub static VERSION: &str = env!("CARGO_PKG_VERSION");

/// I/O extensions and utilities
pub mod io {
    mod gather;
    pub use io::gather::GatheringReader;
}
