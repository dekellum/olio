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

use std::io;
use std::io::{Cursor, Read};

/// A specialized reader for `BodyImage` in `Ram`, presenting a continuous
/// (gathered) `Read` interface over N non-contiguous byte buffers.
pub struct GatheringReader<'a, T: AsRef<[u8]> + 'a> {
    current: Cursor<&'a [u8]>,
    remainder: &'a [T]
}

impl<'a, T: AsRef<[u8]> + 'a> GatheringReader<'a, T> {
    pub fn new(buffers: &'a [T]) -> Self {
        match buffers.split_first() {
            Some((b, remainder)) => {
                GatheringReader { current: Cursor::new(b.as_ref()), remainder }
            }
            None => {
                GatheringReader { current: Cursor::new(&[]), remainder: &[] }
            }
        }
    }

    fn pop(&mut self) -> bool {
        match self.remainder.split_first() {
            Some((b, rem)) => {
                self.current = Cursor::new(b.as_ref());
                self.remainder = rem;
                true
            }
            None => false
        }
    }
}

impl<'a, T: AsRef<[u8]> + 'a> Read for GatheringReader<'a, T> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.current.read(buf)?;
        if n == 0 && !buf.is_empty() && self.pop() {
            return self.read(buf); // recurse
        }
        Ok(n)
    }
}
