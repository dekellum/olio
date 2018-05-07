//! These benchmarks compare the custom built `GatheringReader`, to a chained
//! cursor approach from `std`, for reading a typical scattered vector of byte
//! buffers.

#![feature(test)]
extern crate test;
extern crate olio;
extern crate bytes;

use olio::io::GatheringReader;
use bytes::Bytes;
use test::Bencher;
use std::io;
use std::io::{Cursor, Read};

const CHUNK_SIZE: usize = 8 * 1024;
const CHUNK_COUNT: usize = 40;
const READ_BUFF_SIZE: usize = 101;

#[bench]
fn gather_reader(b: &mut Bencher) {
    let buffers = create_buffers();
    b.iter( move || {
        let len = read_gathered(&buffers).expect("read");
        assert_eq!(CHUNK_SIZE * CHUNK_COUNT, len);
    })
}

#[bench]
fn gather_chained_cursors(b: &mut Bencher) {
    let buffers = create_buffers();
    b.iter( move || {
        let len = read_chained(&buffers).expect("read");
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

fn read_gathered(buffers: &[Bytes]) -> Result<usize, io::Error> {
    let r = GatheringReader::new(buffers);
    read_to_end(r)
}

fn read_chained(buffers: &[Bytes]) -> Result<usize, io::Error> {
    let mut r: Box<Read> = Box::new(Cursor::new(&buffers[0]));
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
