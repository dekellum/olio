use ::std;
use std::cell::Cell;
use std::fmt;
use std::io;
use std::ops::Deref;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, ATOMIC_USIZE_INIT};
use std::sync::atomic::Ordering::{SeqCst, Relaxed};

#[cfg(unix)] use libc;

/// Possible error with `libc::(posix_)madvise()`, or other platform
/// equivalent.
///
/// Implements `std::error::Error` and may be converted to an
/// `io::Error(Other)`.
#[derive(Debug)]
pub struct MemAdviseError {
    ecode: i32,
}

impl From<MemAdviseError> for io::Error {
    fn from(me: MemAdviseError) -> io::Error {
        io::Error::new(io::ErrorKind::Other, me)
    }
}

impl fmt::Display for MemAdviseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "libc::posix_madvise error return code {}", self.ecode)
    }
}

impl std::error::Error for MemAdviseError {
    fn description(&self) -> &str { "MemAdviseError" }
    fn cause(&self) -> Option<&dyn std::error::Error> { None }
}

/// Memory access pattern advice.
///
/// This encodes a subset of POSIX.1-2001 `madvise` flags, and is intending to
/// be a workable cross platform abstraction. In particular, the values do not
/// correspond to any libc or other lib constants, and are arranged in
/// ascending order of minimal to maximum *priority* in the presence of
/// concurrent interest in the same region.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(usize)]
pub enum MemAdvice {
    Normal     = 0,       // Not counted
    Random     = 0x003FF, // Bits  1-10 mask value
    Sequential = 0xFFC00, // Bits 11-20 mask value
}

/// Wrapper over a byte buffer, supporting concurrent memory access advice,
/// where the highest priority advice wins.
///
/// This is likely to only be useful for memory mapped regions and memory
/// mapped files in particular.
///
/// Uses an internal `Arc` over shared state, so cloning the handle is
/// inexpensive. Each new and cloned handle starts with the implicit
/// `MemAdvice::Normal`. The shared state, a list of advisor interest counts,
/// is maintained as a single atomic integer, for minimal overhead. Each
/// `MemAdvice` level is allocated 10-bits or up to 1,023 MemHandle
/// advisors. Any advice beyond this capacity or after an error is returned
/// from `advise`, may be ignored, favoring the prior highest priority
/// advice.
#[derive(Debug)]
pub struct MemHandle<T>
where T: Deref<Target=[u8]>
{
    mem: Arc<Mem<T>>,
    advice: Cell<MemAdvice>,
}

impl<T> MemHandle<T>
where T: Deref<Target=[u8]>
{
    /// Wrap an owned instance of a byte slice buffer. Additional (atomic)
    /// references to the underlying buffer can then be created by `clone` of
    /// this handle.
    pub fn new(mem: T) -> MemHandle<T> {
        let mem = Arc::new(Mem { mem, advisors: ATOMIC_USIZE_INIT });
        MemHandle { mem, advice: Cell::new(MemAdvice::Normal) }
    }

    /// Advise on access plans for the underlying memory. There may be
    /// multiple cloned handles to the same memory region, so the advice is
    /// only relayed to the operating system if it has greater priority than
    /// any other advice made via another surviving handle. On success,
    /// returns the MemAdvice as relayed, or a snapshot of the current,
    /// highest priority advice. Returns an error if the underlying system
    /// call fails.
    pub fn advise(&self, advice: MemAdvice)
        -> Result<MemAdvice, MemAdviseError>
    {
        let prior = self.advice.replace(advice);
        if advice == prior {
            Ok(prior)
        } else {
            self.mem.adjust_advice(prior, advice)
        }
    }
}

impl<T> Clone for MemHandle<T>
where T: Deref<Target=[u8]>
{
    fn clone(&self) -> MemHandle<T> {
        MemHandle { mem: self.mem.clone(), advice: Cell::new(MemAdvice::Normal) }
    }
}

impl<T> Drop for MemHandle<T>
where T: Deref<Target=[u8]>
{
    fn drop(&mut self) {
        let advice = self.advice.get();
        if advice != MemAdvice::Normal {
            self.mem.adjust_advice(advice, MemAdvice::Normal).ok();
        }
    }
}

impl<T> Deref for MemHandle<T>
where T: Deref<Target=[u8]>
{
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        &self.mem
    }
}

#[derive(Debug)]
struct Mem<T>
where T: Deref<Target=[u8]>
{
    mem: T,
    advisors: AtomicUsize,
}

impl<T> Mem<T>
where T: Deref<Target=[u8]>
{
    fn adjust_advice(&self, prior: MemAdvice, advice: MemAdvice)
        -> Result<MemAdvice, MemAdviseError>
    {
        debug_assert!(prior != advice);
        let mut adv = self.advisors.load(Relaxed);
        loop {
            let old_top = top_most(adv);
            let new_adv = decr_advisors(adv, prior);
            let new_adv = incr_advisors(new_adv, advice);
            let new_top = top_most(new_adv);
            match self.advisors.compare_exchange_weak(
                adv, new_adv, SeqCst, Relaxed
            ) {
                Ok(_) => {
                    if new_top != old_top {
                        // Note, may fail after adjustments
                        advise(&self.mem, new_top)?;
                        return Ok(new_top);
                    }
                    return Ok(new_top);
                }
                Err(x) => adv = x
            }
        }
    }
}

impl<T> Deref for Mem<T>
where T: Deref<Target=[u8]>
{
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        &self.mem
    }
}

// Given packed advisors state, and prior advice, return decremented state.
fn decr_advisors(mut advisors: usize, prior: MemAdvice) -> usize {
    if prior != MemAdvice::Normal {
        let mut p = advisors & (prior as usize);
        advisors -= p;
        if prior == MemAdvice::Sequential { p >>= 10; }
        if p > 0 { p -= 1; }
        if prior == MemAdvice::Sequential { p <<= 10; }
        advisors |= p;
    }
    advisors
}

// Given packed advisors state, and new advice, return incremented state.
fn incr_advisors(mut advisors: usize, advice: MemAdvice) -> usize {
    let mut cur = advisors & (advice as usize);
    advisors -= cur;
    match advice {
        MemAdvice::Normal => {
            advisors
        }
        MemAdvice::Random => {
            if cur < 0x3FF { cur += 1; }
            advisors | cur
        }
        MemAdvice::Sequential => {
            cur >>= 10;
            if cur < 0x3FF { cur += 1; }
            cur <<= 10;
            advisors | cur
        }
    }
}

// Return top most advice from advisors state.
fn top_most(advisors: usize) -> MemAdvice {
    if (advisors & (MemAdvice::Sequential as usize)) > 0 {
        MemAdvice::Sequential
    } else if (advisors & (MemAdvice::Random as usize)) > 0 {
        MemAdvice::Random
    } else {
        MemAdvice::Normal
    }
}

// Advise the \*nix OS about memory access plans.
#[cfg(unix)]
fn advise<T>(mem: &T, advice: MemAdvice) -> Result<(), MemAdviseError>
    where T: Deref<Target=[u8]>
{
    let flags: libc::c_int = match advice {
        MemAdvice::Normal       => libc::POSIX_MADV_NORMAL,
        MemAdvice::Random       => libc::POSIX_MADV_RANDOM,
        MemAdvice::Sequential   => libc::POSIX_MADV_SEQUENTIAL,
    };

    let ptr = &(mem[0]) as *const u8 as *mut libc::c_void;
    let res = unsafe { libc::posix_madvise(ptr, mem.len(), flags) };
    if res == 0 {
        Ok(())
    } else {
        Err(MemAdviseError { ecode: res })
    }
}

// RAM access advice, currently a no-op for non-\*nix OS
#[cfg(not(unix))]
fn advise<T>(_mem: &T, _advice: MemAdvice) -> Result<(), MemAdviseError>
    where T: Deref<Target=[u8]>
{
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::mem::MemHandle;

    #[test]
    fn test_with_any_deref() {
        let _m = MemHandle::new(vec![0u8; 1024]);
        // Note the would typically fail for any actual use of advise (not
        // properly aligned, not memory mapped, etc.
    }

    fn is_send<T: Send>() -> bool { true }

    #[test]
    fn test_send_sync() {
        assert!(is_send::<MemHandle<Vec<u8>>>());
    }

    #[cfg(feature = "mmap")]
    mod mmap {
        extern crate tempfile;
        extern crate rand;

        use std::io::Write;
        use std::thread;

        use self::tempfile::tempfile;
        use memmap::Mmap;

        use self::rand::seq::SliceRandom;

        use crate::mem::MemHandle;
        use crate::mem::MemAdvice::*;

        #[test]
        fn test_advise_one() {
            let map = {
                let mut f = tempfile().unwrap();
                f.write_all(&vec![1u8; 256 * 1024]).unwrap();
                unsafe { Mmap::map(&f) }.unwrap()
            };
            let mem = MemHandle::new(map);
            assert_eq!(mem.advise(Normal).unwrap(),     Normal);
            assert_eq!(mem.advise(Random).unwrap(),     Random);
            assert_eq!(mem.advise(Random).unwrap(),     Random);
            assert_eq!(1u8, mem[0]);
            assert_eq!(mem.advise(Sequential).unwrap(), Sequential);
            assert_eq!(1u8, mem[128*1024-1]);
            assert_eq!(mem.advise(Random).unwrap(),     Random);
            assert_eq!(1u8, mem[256*1024-1]);
            assert_eq!(mem.advise(Normal).unwrap(),     Normal);
        }

        #[test]
        fn test_advise_two_random() {
            let map = {
                let mut f = tempfile().unwrap();
                f.write_all(&vec![1u8; 256 * 1024]).unwrap();
                unsafe { Mmap::map(&f) }.unwrap()
            };
            let h1 = MemHandle::new(map);
            let h2 = h1.clone();
            assert_eq!(h1.advise(Sequential).unwrap(), Sequential);
            assert_eq!(1u8, h1[0]);
            assert_eq!(h2.advise(Random).unwrap(), Sequential);
            assert_eq!(1u8, h2[128*1024-1]);
            drop(h1);
            assert_eq!(h2.advise(Random).unwrap(), Random);
        }

        #[test]
        fn test_advise_two_normal() {
            let map = {
                let mut f = tempfile().unwrap();
                f.write_all(&vec![1u8; 256 * 1024]).unwrap();
                unsafe { Mmap::map(&f) }.unwrap()
            };
            let h1 = MemHandle::new(map);
            let h2 = h1.clone();
            assert_eq!(h1.advise(Sequential).unwrap(), Sequential);
            drop(h1);
            assert_eq!(h2.advise(Normal).unwrap(), Normal);
        }

        #[test]
        fn test_advise_three() {
            let map = {
                let mut f = tempfile().unwrap();
                f.write_all(&vec![1u8; 256 * 1024]).unwrap();
                unsafe { Mmap::map(&f) }.unwrap()
            };
            let h1 = MemHandle::new(map);
            let h2 = h1.clone();
            let h3 = h2.clone();
            assert_eq!(h1.advise(Sequential).unwrap(), Sequential);
            assert_eq!(h2.advise(Random).unwrap(),     Sequential);
            assert_eq!(h3.advise(Random).unwrap(),     Sequential);
            drop(h1); //after which h2 (+h3) wins, now Random
            assert_eq!(h3.advise(Normal).unwrap(),     Random); //h2 remains
        }

        #[test]
        fn test_advise_threaded() {
            let mut rng = rand::thread_rng();
            let one_of = vec![Normal, Random, Sequential];
            let map = {
                let mut f = tempfile().unwrap();
                f.write_all(&vec![2u8; 64 * 1024]).unwrap();
                unsafe { Mmap::map(&f) }.unwrap()
            };
            let h0 = MemHandle::new(map);
            for _ in 0..47 {
                let mut threads = Vec::with_capacity(100);
                let advices = (0..13).map(|_| {
                    *one_of.choose(&mut rng).unwrap()
                });
                for advice in advices {
                    let hc = h0.clone();
                    threads.push(thread::spawn( move || {
                        let res = hc.advise(advice).expect("advise");
                        // Effective advice is always at least what is asked
                        // for, regardless of ordering and handle lifetime.
                        assert!(res >= advice);
                    }));
                }
                for t in threads {
                    t.join().unwrap();
                }
            }
        }
    }
}
