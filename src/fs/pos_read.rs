use std::borrow::Borrow;
use std::fs::File;
use std::io;

#[cfg(unix)]
use std::os::unix::fs::FileExt;

#[cfg(windows)]
use std::os::windows::fs::FileExt;

/// Trait offering a uniform `pread` for positioned reads, with platform
/// dependent side-effects.
///
/// For `File` (and any `Borrow<File>`), this is implemented using the
/// platform dependent standard `FileExt` traits.  To maintain portability and
/// consistency on all platforms, the user is advised to avoid concurrent,
/// direct reads or writes on a `File` (via its own `Read`/`Write`
/// implementation) while any instances of this interface are in use for the
/// same `File`, and to re-`seek` to a known file position after such use.
pub trait PosRead {
    /// Read bytes, starting at the specified offset, into the specified
    /// buffer and return the number of bytes read. The offset is from the
    /// start of the underlying file or file range.  Reads beyond the end of
    /// available bytes will return 0 length. The position of the underlying
    /// file pointer (aka cursor) is not used. It is platform dependent
    /// whether the underlying file pointer is modified by this operation.
    fn pread(&self, buf: &mut [u8], offset: u64) -> io::Result<usize>;
}

impl<B> PosRead for B
    where B: Borrow<File>
{
    #[cfg(unix)]
    #[inline]
    fn pread(&self, buf: &mut [u8], offset: u64) -> io::Result<usize> {
        self.borrow().read_at(buf, offset)
    }

    #[cfg(windows)]
    #[inline]
    fn pread(&self, buf: &mut [u8], offset: u64) -> io::Result<usize> {
        self.borrow().seek_read(buf, offset)
    }
}
