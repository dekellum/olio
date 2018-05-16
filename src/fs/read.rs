use std::borrow::Borrow;
use std::fs::File;
use std::io;
use std::io::{Error, ErrorKind, Read, Seek, SeekFrom};
use std::marker::PhantomData;

use fs::PosRead;

#[cfg(feature = "mmap")]
use memmap::{Mmap, MmapOptions};

/// Re-implements `Read` and `Seek` over `PosRead` using _only_ positioned
/// reads, and by maintaining an instance independent position.
///
/// The type is generic over any `PosRead` implementation, and a `Borrow`
/// type, so for example, it can use can be an owner via `ReadPos<File, File>`
/// or use a shared reference, as in `ReadPos<File, &File>` or
/// `ReadPos<File, Arc<File>>`.
///
/// A fixed `length` is passed on construction and used solely to interpret
/// `SeekFrom::End`. Reads are not constrained by this length. The length is
/// neither checked nor updated via file metadata, and could deviate from the
/// underlying file length if concurrent writes or truncation is
/// possible. Reads beyond the end of the underlying `File` will return 0
/// length. Seeking past the end is allowed by the platforms for `File`, and
/// is also allowed for `ReadPos`.
#[derive(Debug)]
pub struct ReadPos<P, B>
where P: PosRead, B: Borrow<P>
{
    pos: u64,
    length: u64,
    file: B,
    phantom: PhantomData<fn() -> P>
}

/// Re-implements `Read` and `Seek` over `PosRead` using _only_ positioned
/// reads, and by maintaining instance independent start, end, and position.
///
/// The type is generic over any `PosRead` implementation, and a `Borrow`
/// type, so for example, it can use can be an owner via
/// `ReadSlice<File, File>` or use a shared reference, as in
/// `ReadSlice<File, &File>` or `ReadSlice<File, Arc<File>>`.
///
/// As compared with [`ReadPos`](struct.ReadPos.html), `ReadSlice` adds a
/// general start offset, and limits access to the start..end range. Seeks are
/// relative, so a seek to `SeekFrom::Start(0)` is always the first byte of
/// the slice.
///
/// Fixed `start` and `end` offsets are passed on construction and used to
/// constrain reads and interpret `SeekFrom::Start` and `SeekFrom::End`. These
/// offsets are neither checked nor updated via file metadata, and the end
/// offset could deviate from the underlying file length if concurrent writes
/// or truncation is possible. Reads beyond `end` or the end of the underlying
/// `PosRead` will return 0 length. Seeking past the end is allowed by the
/// platforms for `File`, and is also allowed for `ReadSlice`.
#[derive(Debug)]
pub struct ReadSlice<P, B>
where P: PosRead, B: Borrow<P>
{
    start: u64,
    pos: u64,
    end: u64,
    file: B,
    phantom: PhantomData<fn() -> P>
}

/// Types that can be subsliced to a `ReadSlice`.
pub trait ReadSubSlice
{
    type ReadSliceType;

    /// Return a new and independent `ReadSlice` of the same file, for the
    /// range of byte offsets `start..end`.
    fn subslice(&self, start: u64, end: u64) -> Self::ReadSliceType;
}

impl<P, B> ReadPos<P, B>
where P: PosRead, B: Borrow<P>
{
    /// New instance by `PosRead` reference and fixed file length. The initial
    /// position is the start of the file.
    pub fn new(file: B, length: u64) -> Self {
        ReadPos { pos: 0, length, file, phantom: PhantomData }
    }

    /// Return the length as provided on construction. This may differ from
    /// the underlying file length.
    pub fn len(&self) -> u64 {
        self.length
    }

    /// Return `true` if length is 0.
    pub fn is_empty(&self) -> bool {
        self.length == 0
    }

    /// Return the current instance position. This is a convenience shorthand
    /// for `seek(SeekFrom::Current(0))`, is infallable, and does not require
    /// a mutable reference.
    pub fn tell(&self) -> u64 {
        self.pos
    }

    /// Seek by signed offset from an origin, checking for underflow and
    /// overflow.
    fn seek_from(&mut self, origin: u64, offset: i64) -> io::Result<u64> {
        let checked_pos = if offset < 0 {
            origin.checked_sub((-offset) as u64)
        } else {
            origin.checked_add(offset as u64)
        };

        if let Some(p) = checked_pos {
            self.pos = p;
            Ok(p)
        } else if offset < 0 {
            Err(Error::new(
                ErrorKind::InvalidInput,
                "Attempted seek to a negative absolute position"
            ))
        } else {
            Err(Error::new(
                ErrorKind::Other,
                "Attempted seek would overflow u64 position"
            ))
        }
    }
}

impl<P, B> Clone for ReadPos<P, B>
where P: PosRead, B: Borrow<P> + Clone
{
    /// Return a new, independent `ReadPos` with the same length and file
    /// reference as self, and with position 0 (ignores the current
    /// position of self).
    fn clone(&self) -> Self {
        ReadPos { pos: 0,
                  length: self.length,
                  file: self.file.clone(),
                  phantom: PhantomData }
    }
}

impl<P, B> PosRead for ReadPos<P, B>
where P: PosRead, B: Borrow<P>
{
    #[inline]
    fn pread(&self, buf: &mut [u8], offset: u64) -> io::Result<usize> {
        self.file.borrow().pread(buf, offset)
    }
}

impl<P, B> Read for ReadPos<P, B>
where P: PosRead, B: Borrow<P>
{
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let len = self.pread(buf, self.pos)?;
        self.pos += len as u64;
        Ok(len)
    }
}

impl<P, B> Seek for ReadPos<P, B>
where P: PosRead, B: Borrow<P>
{
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        match from {
            SeekFrom::Start(p) => {
                self.pos = p;
                Ok(p)
            }
            SeekFrom::End(offset) => {
                let origin = self.length;
                self.seek_from(origin, offset)
            }
            SeekFrom::Current(offset) => {
                let origin = self.pos;
                self.seek_from(origin, offset)
            }
        }
    }
}

impl<P, B> ReadSubSlice for ReadPos<P, B>
where P: PosRead, B: Borrow<P> + Clone
{
    type ReadSliceType = ReadSlice<P, B>;

    /// Return a new and independent `ReadSlice` of the same file, for the
    /// range of byte offsets `start..end`. This implementation _panics_ if
    /// start is greater than end. Note that the end parameter is not checked
    /// against the length of self as passed on construction.
    fn subslice(&self, start: u64, end: u64) -> Self::ReadSliceType {
        ReadSlice::new(self.file.clone(), start, end)
    }
}

impl<P, B> ReadSlice<P, B>
where P: PosRead, B: Borrow<P>
{
    /// New instance by `PosRead` reference, fixed start and end offsets. The
    /// initial position is at the start (relative offset 0).
    pub fn new(file: B, start: u64, end: u64) -> Self {
        assert!(start <= end);
        ReadSlice { start, pos: start, end, file, phantom: PhantomData }
    }

    /// Return the total size of the slice in bytes. This is based on the
    /// start and end offsets as constructed and can differ from the
    /// underlying file length.
    pub fn len(&self) -> u64 {
        self.end - self.start
    }

    /// Return `true` if length is 0.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Return the current instance position, relative to the slice. This is a
    /// convenience shorthand for `seek(SeekFrom::Current(0))`, is infallable,
    /// and does not require a mutable reference.
    pub fn tell(&self) -> u64 {
        self.pos - self.start
    }

    /// Like `PosRead::pread`, but using an absolute (internal) position
    /// instead of the external, relative offset.
    fn pread_abs(&self, buf: &mut [u8], abspos: u64) -> io::Result<usize> {
        if abspos < self.end {
            let mlen = self.end - abspos; // positive/no-underflow per above
            if (buf.len() as u64) <= mlen {
                self.file.borrow().pread(buf, abspos)
            } else {
                // safe cast: mlen < buf.len which is already usize
                self.file.borrow().pread(&mut buf[..(mlen as usize)], abspos)
            }
        } else {
            Ok(0)
        }
    }

    /// Seek by signed offset from an (absolute) origin, checking for
    /// underflow and overflow.
    fn seek_from(&mut self, origin: u64, offset: i64) -> io::Result<u64> {
        let checked_pos = if offset < 0 {
            origin.checked_sub((-offset) as u64)
        } else {
            origin.checked_add(offset as u64)
        };

        if let Some(p) = checked_pos {
            self.seek_to(p)
        } else if offset < 0 {
            Err(Error::new(
                ErrorKind::InvalidInput,
                "Attempted seek to a negative position"
            ))
        } else {
            Err(Error::new(
                ErrorKind::Other,
                "Attempted seek would overflow u64 position"
            ))
        }
    }

    /// Seek by absolute position, validated with the start index. Return the
    /// new relative position, or Error if the absolute position is before
    /// start. Like with a regular File, positions beyond end are allowed, and
    /// this is checked on reads.
    fn seek_to(&mut self, abspos: u64) -> io::Result<u64> {
        if abspos < self.start {
            Err(Error::new(
                ErrorKind::InvalidInput,
                "Attempted seek to a negative position"
            ))
        } else {
            self.pos = abspos;
            Ok(abspos - self.start)
        }
    }
}

impl<P, B> Clone for ReadSlice<P, B>
where P: PosRead, B: Borrow<P> + Clone
{
    /// Return a new, independent `ReadSlice` with the same start, end and
    /// file reference as self, and positioned at start (ignores the current
    /// position of self).
    fn clone(&self) -> Self {
        ReadSlice { start: self.start,
                    pos:   self.start,
                    end:   self.end,
                    file:  self.file.clone(),
                    phantom: PhantomData }
    }
}

impl<P, B> PosRead for ReadSlice<P, B>
where P: PosRead, B: Borrow<P>
{
    #[inline]
    fn pread(&self, buf: &mut [u8], offset: u64) -> io::Result<usize> {
        let pos = self.start.saturating_add(offset);
        if pos < self.end {
            self.pread_abs(buf, pos)
        } else {
            Ok(0)
        }
    }
}

impl<P, B> Read for ReadSlice<P, B>
where P: PosRead, B: Borrow<P>
{
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let len = self.pread_abs(buf, self.pos)?;
        self.pos += len as u64;
        Ok(len)
    }
}

impl<P, B> Seek for ReadSlice<P, B>
where P: PosRead, B: Borrow<P>
{
    /// Seek to an offset, in bytes, in a stream. In this implementation,
    /// seeks are relative to the fixed starting offset to underlying File, so
    /// a seek to `SeekFrom::Start(0)` is always the first byte of the slice.
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        match from {
            SeekFrom::Start(p) => {
                if let Some(p) = self.start.checked_add(p) {
                    self.seek_to(p)
                } else {
                    Err(Error::new(
                        ErrorKind::Other,
                        "Attempted seek would overflow u64 position"
                    ))
                }
            },
            SeekFrom::End(offset) => {
                let origin = self.end;
                self.seek_from(origin, offset)
            }
            SeekFrom::Current(offset) => {
                let origin = self.pos;
                self.seek_from(origin, offset)
            }
        }
    }
}

impl<P, B> ReadSubSlice for ReadSlice<P, B>
where P: PosRead, B: Borrow<P> + Clone
{
    type ReadSliceType = Self;

    /// Return a new and independent `ReadSlice` of the same file, for the
    /// range of byte offsets `start..end` which are relative to, and must be
    /// fully contained by self. This implementation _panics_ on overflow, if
    /// start..end is not fully contained, or if start is greater-than end.
    fn subslice(&self, start: u64, end: u64) -> Self {
        let abs_start = self.start.checked_add(start)
            .expect("ReadSlice::subslice start overflow");
        let abs_end = self.start.checked_add(end)
            .expect("ReadSlice::subslice end overflow");
        assert!(abs_start  <= abs_end);
        assert!(self.start <= abs_start);
        assert!(self.end   >= abs_end);

        ReadSlice::new(self.file.clone(), abs_start, abs_end)
    }
}

#[cfg(feature = "mmap")]
impl<B> ReadSlice<File, B>
where B: Borrow<File>
{
    /// Return a new read-only memory map handle `Mmap` for the complete
    /// region of the underlying `File`, from start to end.
    pub fn mem_map(&self) -> Result<Mmap, io::Error> {
        let offset = self.start;
        let len = self.len();
        // See: https://github.com/danburkert/memmap-rs/pull/65
        assert!(offset <= usize::max_value() as u64);
        assert!(len    <= usize::max_value() as u64);
        assert!(len > 0);
        unsafe {
            MmapOptions::new()
                .offset(offset as usize)
                .len(len as usize)
                .map(self.file.borrow())
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::{BufReader, Read, Write};
    use std::sync::Arc;
    use std::thread;
    extern crate tempfile;
    use self::tempfile::tempfile;
    use super::*;

    #[test]
    fn test_seek() {
        let mut f = tempfile().unwrap();
        f.write_all(b"1234567890").unwrap();

        let mut r1 = ReadPos::new(f, 10);
        let mut buf = [0u8; 5];

        let p = r1.seek(SeekFrom::Current(0)).unwrap();
        assert_eq!(0, p);
        let p = r1.seek(SeekFrom::Current(1)).unwrap();
        assert_eq!(1, p);
        r1.read_exact(&mut buf).unwrap();
        assert_eq!(&buf, b"23456");

        let p = r1.seek(SeekFrom::End(-5)).unwrap();
        assert_eq!(5, p);
        let p = r1.seek(SeekFrom::Current(0)).unwrap();
        assert_eq!(5, p);
        r1.read_exact(&mut buf).unwrap();
        assert_eq!(&buf, b"67890");
    }

    #[test]
    fn test_with_buf_reader() {
        let mut f = tempfile().unwrap();
        f.write_all(b"1234567890").unwrap();

        let r0 = ReadPos::<File, _>::new(Arc::new(f), 10);
        let mut r1 = BufReader::with_capacity(0x2000, r0);
        let mut buf = [0u8; 5];

        let p = r1.seek(SeekFrom::Start(1)).unwrap();
        assert_eq!(1, p);
        r1.read_exact(&mut buf).unwrap();
        assert_eq!(&buf, b"23456");

        let mut r0 = r1.into_inner();
        let p = r0.seek(SeekFrom::Current(0)).unwrap();
        assert_eq!(10, p);

        let l = r0.read(&mut buf).unwrap();
        assert_eq!(0, l);
    }

    #[test]
    fn test_interleaved() {
        let mut f = tempfile().unwrap();
        f.write_all(b"1234567890").unwrap();

        let mut r1 = ReadPos::<File, _>::new(Arc::new(f), 10);

        let mut buf = [0u8; 5];
        r1.read_exact(&mut buf).unwrap();
        assert_eq!(&buf, b"12345");

        let mut r2 = r1.clone();
        r2.read_exact(&mut buf).unwrap();
        assert_eq!(&buf, b"12345");

        r1.read_exact(&mut buf).unwrap();
        assert_eq!(&buf, b"67890");

        r2.read_exact(&mut buf).unwrap();
        assert_eq!(&buf, b"67890");
    }

    #[test]
    fn test_concurrent_seek_read() {
        let mut f = tempfile().unwrap();
        let rule = b"1234567890";
        f.write_all(rule).unwrap();
        let f = Arc::new(f);

        let mut threads = Vec::with_capacity(30);
        for i in 0..50 {
            let mut rpc = ReadPos::<File, _>::new(f.clone(), rule.len() as u64);
            threads.push(thread::spawn( move || {
                let p = i % rule.len();
                rpc.seek(SeekFrom::Start(p as u64)).expect("seek");

                thread::yield_now();

                let l = 5.min(rule.len() - p);
                let mut buf = vec![0u8; l];
                rpc.read_exact(&mut buf).expect("read_exact");
                assert_eq!(&buf[..], &rule[p..(p+l)]);
            }))
        }
        for t in threads {
            t.join().unwrap();
        }
    }

    #[test]
    fn test_slice_seek() {
        let mut f = tempfile().unwrap();
        f.write_all(b"1234567890").unwrap();

        let mut r1 = ReadSlice::new(f, 0, 10);
        let mut buf = [0u8; 5];

        let p = r1.seek(SeekFrom::Current(0)).unwrap();
        assert_eq!(0, p);
        let p = r1.seek(SeekFrom::Current(1)).unwrap();
        assert_eq!(1, p);
        r1.read_exact(&mut buf).unwrap();
        assert_eq!(&buf, b"23456");

        let p = r1.seek(SeekFrom::End(-5)).unwrap();
        assert_eq!(5, p);
        let p = r1.seek(SeekFrom::Current(0)).unwrap();
        assert_eq!(5, p);
        r1.read_exact(&mut buf).unwrap();
        assert_eq!(&buf, b"67890");
    }

    #[test]
    fn test_slice_seek_offset() {
        let mut f = tempfile().unwrap();
        f.write_all(b"012345678901").unwrap();

        let r1 = ReadSlice::<File, _>::new(&f, 1, 12);
        let mut r1 = r1.subslice(0, 10);

        let mut buf = [0u8; 5];
        let p = r1.seek(SeekFrom::Current(0)).unwrap();
        assert_eq!(0, p);
        let p = r1.seek(SeekFrom::Current(1)).unwrap();
        assert_eq!(1, p);
        r1.read_exact(&mut buf).unwrap();
        assert_eq!(&buf, b"23456");

        let p = r1.seek(SeekFrom::End(-5)).unwrap();
        assert_eq!(5, p);
        let p = r1.seek(SeekFrom::Current(0)).unwrap();
        assert_eq!(5, p);
        r1.read_exact(&mut buf).unwrap();
        assert_eq!(&buf, b"67890");
    }

    #[test]
    fn test_slice_with_buf_reader() {
        let mut f = tempfile().unwrap();
        f.write_all(b"01234567890").unwrap();

        let r0 = ReadSlice::<File, _>::new(Arc::new(f), 1, 11);
        let mut r1 = BufReader::with_capacity(0x2000, r0);
        let mut buf = [0u8; 5];

        let p = r1.seek(SeekFrom::Start(1)).unwrap();
        assert_eq!(1, p);
        r1.read_exact(&mut buf).unwrap();
        assert_eq!(&buf, b"23456");

        let mut r0 = r1.into_inner();
        let p = r0.seek(SeekFrom::Current(0)).unwrap();
        assert_eq!(10, p);

        let l = r0.read(&mut buf).unwrap();
        assert_eq!(0, l);
    }

    fn is_send<T: Send>() -> bool { true }
    fn is_sync<T: Sync>() -> bool { true }

    #[test]
    fn test_send_sync() {
        assert!(is_send::<ReadPos<File, File>>());
        assert!(is_sync::<ReadPos<File, File>>());
        assert!(is_send::<ReadPos<File, Arc<File>>>());
        assert!(is_sync::<ReadPos<File, Arc<File>>>());
        assert!(is_send::<ReadSlice<File, Arc<File>>>());
        assert!(is_sync::<ReadSlice<File, Arc<File>>>());
    }

    fn is_pos_read<T: PosRead>() -> bool { true }

    #[test]
    fn test_generic_bounds() {
        assert!(is_pos_read::<ReadPos<File, File>>());
        assert!(is_pos_read::<ReadPos<File, Box<File>>>());
        assert!(is_pos_read::<ReadPos<File, &File>>());
    }
}
