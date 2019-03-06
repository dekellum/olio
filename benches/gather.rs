//! These benchmarks compare the custom built `GatheringReader`, to a chained
//! cursor approach from `std`, for reading a typical scattered vector of byte
//! buffers.
#![warn(rust_2018_idioms)]

#![feature(test)]
extern crate test; // Still required, see rust-lang/rust#55133

use std::io;
use std::io::{Cursor, Read};

use bytes::{BufMut, Bytes, BytesMut};
use test::Bencher;

use olio::io::GatheringReader;

const CHUNK_SIZE: usize = 8 * 1024;
const CHUNK_COUNT: usize = 40;
const READ_BUFF_SIZE: usize = 101;

#[bench]
fn gather_reader(b: &mut Bencher) {
    let buffers = create_buffers();
    b.iter(move || {
        let len = read_gathered(&buffers).expect("read");
        assert_eq!(CHUNK_SIZE * CHUNK_COUNT, len);
    })
}

#[bench]
fn gather_x_chained_cursors(b: &mut Bencher) {
    let buffers = create_buffers();
    b.iter(move || {
        let len = read_chained(&buffers).expect("read");
        assert_eq!(CHUNK_SIZE * CHUNK_COUNT, len);
    })
}

#[bench]
fn gather_upfront(b: &mut Bencher) {
    let buffers = {
        let mut bufs = Vec::with_capacity(CHUNK_COUNT);
        for b in create_buffers() {
            bufs.push(b)
        }
        bufs
    };
    b.iter(|| {
        let buffers = buffers.clone(); // shallow
        let buf = gather(buffers);
        let cur = Cursor::new(&buf);
        let len = read_to_end(cur).expect("read");
        assert_eq!(CHUNK_SIZE * CHUNK_COUNT, len);
    })
}

#[bench]
fn gather_upfront_read_only(b: &mut Bencher) {
    let buf = {
        let mut buffers = Vec::with_capacity(CHUNK_COUNT);
        for b in create_buffers() {
            buffers.push(b)
        }
        gather(buffers)
    };
    b.iter(|| {
        let cur = Cursor::new(&buf);
        let len = read_to_end(cur).expect("read");
        assert_eq!(CHUNK_SIZE * CHUNK_COUNT, len);
    })
}

fn create_buffers() -> Vec<Bytes> {
    let chunk = vec![65u8; CHUNK_SIZE];
    let mut v = Vec::new();
    for _ in 0..CHUNK_COUNT {
        v.push(chunk.as_slice().into());
    }
    v
}

fn gather(buffers: Vec<Bytes>) -> Bytes {
    let mut newb = BytesMut::with_capacity(CHUNK_SIZE * CHUNK_COUNT);
    for b in buffers {
        newb.put_slice(&b);
        drop::<Bytes>(b); // Ensure ASAP drop
    }
    newb.freeze()
}

fn read_gathered(buffers: &[Bytes]) -> Result<usize, io::Error> {
    let r = GatheringReader::new(buffers);
    read_to_end(r)
}

fn read_chained(buffers: &[Bytes]) -> Result<usize, io::Error> {
    let mut r: Box<dyn Read> = Box::new(Cursor::new(&buffers[0]));
    for b in &buffers[1..] {
        r = Box::new(r.chain(Cursor::new(b)));
    }
    read_to_end(r)
}

fn read_to_end<R: Read>(mut r: R) -> Result<usize, io::Error> {
    let mut buf = [0u8; READ_BUFF_SIZE];
    let mut total = 0;
    loop {
        let len = r.read(&mut buf)?;
        if len == 0 {
            break;
        }
        total += len;
    }
    Ok(total)
}
