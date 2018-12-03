//! This crate provides I/O-related utilities complimenting the Rust Standard
//! Library `std::io`, `std::fs`, etc.
//!
//! ## Overview
//!
//! The [_fs_ module](fs/index.html) includes a `PosRead` trait, offering a
//! uniform `pread` for positioned file reads, and a `ReadSlice` supporting
//! multiple independent reader instances limited to a fixed
//! start..end range.
//!
//! The [_io_ module](io/index.html) includes a `GatheringReader`, which
//! presents a continuous `Read` interface over N non-contiguous byte buffers.
//!
//! The [_mem_ module](mem/index.html) includes a `MemHandle` supporting
//! prioritized concurrent memory access advice (e.g. madvise (2) on unix).
//!
//! ## Optional Features
//!
//! _mmap (default):_ Adds `fs::ReadSlice::<File>::mem_map` support for memory
//! mapping.
#![warn(rust_2018_idioms)]

/// The crate version string.
pub static VERSION: &str = env!("CARGO_PKG_VERSION");

/// Filesystem extensions and utilities.
///
/// The `PosRead` trait offers a uniform `pread` for positioned reads.
///
/// The `ReadPos` and `ReadSlice` types re-implement `Read` and `Seek` over
/// any `Borrow` of a `PosRead` type. For `File` in particular, this enables
/// multiple independent reader instances, without needing a path to open an
/// independent new `File` instance.  Thus these types are compatible with
/// "unnamed" (not linked) temporary files, and can reduce the number of
/// necessary file handles.  Note that unix `dup`/`dup2` and the standard
/// `File::try_clone` do _not_ provide independent file positions.
///
/// ## Example
///
/// ``` rust
/// extern crate olio;
/// extern crate tempfile;
///
/// # use std::io;
/// use std::fs::File;
/// use std::io::{Read, Write};
/// use olio::fs::{ReadPos, ReadSlice};
/// use tempfile::tempfile;
///
/// # fn run() -> Result<(), io::Error> {
/// let mut file = tempfile()?;
/// file.write_all(b"0123456789")?;
///
/// // ReadPos by &File so that we can subslice by shared reference
/// let mut rpos = ReadPos::new(&file, 10);
///
/// // Read the first half
/// let mut buf = [0u8; 5];
/// rpos.read_exact(&mut buf)?;
/// assert_eq!(&buf, b"01234");
///
/// // Create an independent ReadSlice and read to end
/// let mut rslice = rpos.subslice(2, 7);
/// let mut buf = Vec::new();
/// rslice.read_to_end(&mut buf)?;
/// assert_eq!(&buf, b"23456");
///
/// // Read the second half from the original ReadPos
/// assert_eq!(rpos.tell(), 5);
/// let mut buf = [0u8; 5];
/// rpos.read_exact(&mut buf)?;
/// assert_eq!(&buf, b"56789");
/// # Ok(())
/// # }
/// # run().unwrap();
/// ```
pub mod fs {
    mod pos_read;
    pub use crate::fs::pos_read::PosRead;

    mod read;
    pub use crate::fs::read::{ReadPos, ReadSlice};

    /// Compatibility type aliases.
    pub mod rc {
        use std::fs::File;
        use std::sync::Arc;

        pub type ReadPos = super::ReadPos<Arc<File>>;
        pub type ReadSlice = super::ReadSlice<Arc<File>>;
    }
}

/// I/O extensions and utilities
pub mod io {
    mod gather;
    pub use crate::io::gather::GatheringReader;
}

/// Random access memory utilities
pub mod mem {
    mod handle;
    pub use crate::mem::handle::{MemAdviseError, MemHandle, MemAdvice};
}
