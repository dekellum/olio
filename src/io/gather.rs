use std::io;
use std::io::{Cursor, Read};

/// A specialized reader presenting a continuous (gathered) `Read` interface
/// over N non-contiguous byte buffers.
///
/// This is more efficient than the current implementation of
/// `std::io::Cursor::chain` for many reads over many buffers. See the
/// associated benchmark comparison.
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
            self.read(buf) // recurse
        } else {
            Ok(n)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gather() {
        let bufs: Vec<&[u8]> = vec![b"hello", b" ", b"world"];

        let mut rdr = GatheringReader::new(&bufs);
        let mut obuf = String::new();
        rdr.read_to_string(&mut obuf).unwrap();
        assert_eq!("hello world", &obuf[..]);
    }

    #[test]
    fn test_empty_buf() {
        let bufs: Vec<&[u8]> = vec![b"hello ", b"wor", b"", b"ld"];

        let mut rdr = GatheringReader::new(&bufs);
        let mut obuf = String::new();
        rdr.read_to_string(&mut obuf).unwrap();
        assert_eq!("hello world", &obuf[..]);
    }

    #[test]
    fn test_empty() {
        let bufs: Vec<&[u8]> = vec![];

        let mut rdr = GatheringReader::new(&bufs);
        let mut obuf = String::new();
        rdr.read_to_string(&mut obuf).unwrap();
        assert_eq!("", &obuf[..]);
    }

    fn is_send<T: Send>() -> bool { true }
    fn is_sync<T: Sync>() -> bool { true }

    #[test]
    fn test_send_sync() {
        assert!(is_send::<GatheringReader<Vec<u8>>>());
        assert!(is_sync::<GatheringReader<Vec<u8>>>());
    }
}
