//! These benchmarks compare raw file reads vs positioned reads.

#![feature(test)]
extern crate test;
extern crate olio;
extern crate tempfile;

use olio::fs::rc::{ReadPos, ReadSlice};
use test::Bencher;
use std::fs::File;
use std::io;
use std::io::{Read, Seek, SeekFrom, Write};
use std::sync::Arc;
use tempfile::tempfile;

const CHUNK_COUNT: usize = 48;
const CHUNK_SIZE: usize = 8 * 1024;
const READ_BUFF_SIZE: usize = 101;

#[bench]
fn read_all_raw(b: &mut Bencher) {
    let mut file = create_file().expect("create file");
    b.iter( || {
        file.seek(SeekFrom::Start(0)).expect("rewind");
        let len = read_to_end(&mut file).expect("read raw");
        assert_eq!(CHUNK_SIZE * CHUNK_COUNT, len);
    })
}

#[bench]
fn read_all_pos(b: &mut Bencher) {
    let file = Arc::new(create_file().unwrap());
    b.iter( || {
        let mut rdr = ReadPos::new(
            file.clone(),
            (CHUNK_SIZE * CHUNK_COUNT) as u64
        );
        let len = read_to_end(&mut rdr).expect("read pos");
        assert_eq!(CHUNK_SIZE * CHUNK_COUNT, len);
    })
}

#[bench]
fn read_all_slice(b: &mut Bencher) {
    let file = Arc::new(create_file().unwrap());
    b.iter( || {
        let mut rdr = ReadSlice::new(
            file.clone(),
            0,
            (CHUNK_SIZE * CHUNK_COUNT) as u64
        );
        let len = read_to_end(&mut rdr).expect("read slice");
        assert_eq!(CHUNK_SIZE * CHUNK_COUNT, len);
    })
}

fn create_file() -> Result<File, io::Error> {
    let mut file = tempfile()?;
    for c in 0..CHUNK_COUNT {
        let buf = [c as u8; CHUNK_SIZE];
        file.write_all(&buf)?;
    }
    Ok(file)
}

fn read_to_end<R: Read>(r: &mut R) -> Result<usize, io::Error> {
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
