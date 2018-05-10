use std::fs::File;
use std::io;

#[cfg(unix)]
use std::os::unix::fs::FileExt;

#[cfg(windows)]
use std::os::windows::fs::FileExt;

/// Trait offering a uniform `pread` for positioned reads, with platform
/// dependent side-effects.
pub trait PosRead {
    /// Read some bytes, starting at the specified offset, into the specified
    /// buffer and return the number of bytes read. The offset is from the
    /// start of the underlying file or file range.  The position of the
    /// underlying file pointer (aka cursor) is not used. It is platform
    /// dependent whether the underlying file pointer is modified by this
    /// operation.
    fn pread(&self, buf: &mut [u8], offset: u64) -> io::Result<usize>;
}

impl PosRead for File {
    #[cfg(unix)]
    #[inline]
    fn pread(&self, buf: &mut [u8], offset: u64) -> io::Result<usize> {
        self.read_at(buf, offset)
    }

    #[cfg(windows)]
    #[inline]
    fn pread(&self, buf: &mut [u8], offset: u64) -> io::Result<usize> {
        self.seek_read(buf, offset)
    }
}
