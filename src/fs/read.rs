use std::borrow::Borrow;
use std::fs::File;
use std::io;
use std::io::{Error, ErrorKind, Read, Seek, SeekFrom};

use fs::PosRead;

#[cfg(feature = "mmap")]
use memmap::{Mmap, MmapOptions};

/// Re-implements `Read` and `Seek` over `PosRead` using _only_ positioned
/// reads, and by maintaining an instance independent position.
///
/// [`PosRead`](trait.PosRead.html) is implemented for any `Borrow<File>` so
/// this can own via `ReadPos<File>` or use a shared reference, as in
/// `ReadPos<&File>` or `ReadPos<Arc<File>>`.
///
/// A fixed `length` is passed on construction and used solely to interpret
/// `SeekFrom::End`. Reads are not constrained by this length. The length is
/// neither checked against nor updated from the inner `PosRead` (for example
/// via file metadata) and could deviate if concurrent writes or truncation is
/// possible. Reads beyond the end of the inner `PosRead` will return 0
/// length. Seeking past the end is allowed by the platforms for `File`, and
/// is also allowed for `ReadPos`.
#[derive(Debug)]
pub struct ReadPos<P>
where P: PosRead
{
    pos: u64,
    length: u64,
    pos_read: P,
}

/// Re-implements `Read` and `Seek` over `PosRead` using _only_ positioned
/// reads, and by maintaining instance independent start, end, and position.
///
/// [`PosRead`](trait.PosRead.html) is implemented for any `Borrow<File>` so
/// this can own via `ReadSlice<File>` or use a shared reference, as in
/// `ReadSlice<&File>` or `ReadSlice<Arc<File>>`.
///
/// As compared with [`ReadPos`](struct.ReadPos.html), `ReadSlice` adds a
/// general start offset, and limits access to the start..end range. Seeks are
/// relative, so a seek to `SeekFrom::Start(0)` is always the first byte of
/// the slice.
///
/// Fixed `start` and `end` offsets are passed on construction and used to
/// constrain reads and interpret `SeekFrom::Start` and `SeekFrom::End`. These
/// offsets are neither checked against nor updated from the inner `PosRead`
/// (for example via file metadata) and could deviate if concurrent writes or
/// truncation is possible. Reads beyond `end` or the end of the inner
/// `PosRead` will return 0 length. Seeking past the end is allowed by the
/// platforms for `File`, and is also allowed for `ReadSlice`.
#[derive(Debug)]
pub struct ReadSlice<P>
where P: PosRead
{
    start: u64,
    pos: u64,
    end: u64,
    pos_read: P,
}

/// Types that can be subsliced to a `ReadSlice`.
pub trait ReadSubSlice
{
    type ReadSliceType;

    /// Return a new and independent `ReadSlice` for the range of byte offsets
    /// `start..end`.
    fn subslice(&self, start: u64, end: u64) -> Self::ReadSliceType;
}

impl<P> ReadPos<P>
where P: PosRead
{
    /// New instance for `PosRead` and fixed length. The initial position is
    /// the start (index 0).
    pub fn new(pos_read: P, length: u64) -> Self {
        ReadPos { pos: 0, length, pos_read }
    }

    /// Return the length as provided on construction. This may differ from
    /// the inner `PosRead` length.
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

impl<P> Clone for ReadPos<P>
where P: PosRead + Clone
{
    /// Return a new, independent `ReadPos` by clone of the inner `PosRead`,
    /// with the same length as self, and at position 0.
    fn clone(&self) -> Self {
        ReadPos { pos: 0,
                  length: self.length,
                  pos_read: self.pos_read.clone() }
    }
}

impl<P> PosRead for ReadPos<P>
where P: PosRead
{
    #[inline]
    fn pread(&self, buf: &mut [u8], offset: u64) -> io::Result<usize> {
        self.pos_read.pread(buf, offset)
    }
}

impl<P> Read for ReadPos<P>
where P: PosRead
{
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let len = self.pread(buf, self.pos)?;
        self.pos += len as u64;
        Ok(len)
    }
}

impl<P> Seek for ReadPos<P>
where P: PosRead
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

impl<P> ReadSubSlice for ReadPos<P>
where P: PosRead + Clone
{
    type ReadSliceType = ReadSlice<P>;

    /// Return a new and independent `ReadSlice` by clone of the inner
    /// `PosRead`, for the range of byte offsets `start..end`, and positoned
    /// at start. This implementation _panics_ if start is greater than
    /// end. Note that the end parameter is not checked against the length of
    /// self as passed on construction.
    fn subslice(&self, start: u64, end: u64) -> Self::ReadSliceType {
        ReadSlice::new(self.pos_read.clone(), start, end)
    }
}

impl<P> ReadSlice<P>
where P: PosRead
{
    /// New instance by `PosRead` reference, fixed start and end offsets. The
    /// initial position is at the start (relative offset 0).
    pub fn new(pos_read: P, start: u64, end: u64) -> Self {
        assert!(start <= end);
        ReadSlice { start, pos: start, end, pos_read }
    }

    /// Return the total size of the slice in bytes. This is based on the
    /// start and end offsets as constructed and can differ from the inner
    /// `PosRead` length.
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
                self.pos_read.pread(buf, abspos)
            } else {
                // safe cast: mlen < buf.len which is already usize
                self.pos_read.pread(&mut buf[..(mlen as usize)], abspos)
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

impl<P> Clone for ReadSlice<P>
where P: PosRead + Clone
{
    /// Return a new, independent `ReadSlice` by clone of the inner `PosRead`,
    /// with the same start and end as self, and positioned at start.
    fn clone(&self) -> Self {
        ReadSlice { start: self.start,
                    pos:   self.start,
                    end:   self.end,
                    pos_read:  self.pos_read.clone() }
    }
}

impl<P> PosRead for ReadSlice<P>
where P: PosRead
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

impl<P> Read for ReadSlice<P>
where P: PosRead
{
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let len = self.pread_abs(buf, self.pos)?;
        self.pos += len as u64;
        Ok(len)
    }
}

impl<P> Seek for ReadSlice<P>
where P: PosRead
{
    /// Seek to an offset, in bytes, in a stream. In this implementation,
    /// seeks are relative to the fixed start offset so a seek to
    /// `SeekFrom::Start(0)` is always the first byte of the slice.
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

impl<P> ReadSubSlice for ReadSlice<P>
where P: PosRead + Clone
{
    type ReadSliceType = Self;

    /// Return a new and independent `ReadSlice` by clone of the inner
    /// `PosRead`, for the range of byte offsets `start..end` which are
    /// relative to, and must be fully contained by self. This implementation
    /// _panics_ on overflow, if start..end is not fully contained, or if
    /// start is greater-than end.
    fn subslice(&self, start: u64, end: u64) -> Self {
        let abs_start = self.start.checked_add(start)
            .expect("ReadSlice::subslice start overflow");
        let abs_end = self.start.checked_add(end)
            .expect("ReadSlice::subslice end overflow");
        assert!(abs_start  <= abs_end);
        assert!(self.start <= abs_start);
        assert!(self.end   >= abs_end);

        ReadSlice::new(self.pos_read.clone(), abs_start, abs_end)
    }
}

#[cfg(feature = "mmap")]
impl<P> ReadSlice<P>
where P: PosRead + Borrow<File>
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
                .map(self.pos_read.borrow())
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

        let r0 = ReadPos::new(Arc::new(f), 10);
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

        let mut r1 = ReadPos::new(Arc::new(f), 10);

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
            let mut rpc = ReadPos::new(f.clone(), rule.len() as u64);
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

        let r1 = ReadSlice::new(&f, 1, 12);
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

        let r0 = ReadSlice::new(Arc::new(f), 1, 11);
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
        assert!(is_send::<ReadPos<File>>());
        assert!(is_sync::<ReadPos<File>>());
        assert!(is_send::<ReadPos<Arc<File>>>());
        assert!(is_sync::<ReadPos<Arc<File>>>());
        assert!(is_send::<ReadSlice<Arc<File>>>());
        assert!(is_sync::<ReadSlice<Arc<File>>>());
    }

    fn is_pos_read<T: PosRead>() -> bool { true }

    #[test]
    fn test_generic_bounds() {
        assert!(is_pos_read::<ReadPos<File>>());
        assert!(is_pos_read::<ReadPos<Box<File>>>());
        assert!(is_pos_read::<ReadPos<&File>>());
    }
}
