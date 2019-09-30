use std::fmt;
use std::io;
use std::ops::Deref;
use std::sync::Arc;
use std::sync::atomic::Ordering::{Acquire, SeqCst};

#[cfg(unix)] use libc;

// Prefer a u64 representation of advice on all platforms, as it affords room
// for 6 advise levels above baseline (currently `Normal`). Of course, usize is
// already 64 bit unsigned on platforms like x86_64. As of rust 1.34
// `AtomicU64` is stable, on all supported platforms. Start using u64 when
// possible, and raise MSRV to 1.34 once the bits are needed.

#[cfg(olio_std_atomic_u64)]
mod types {
    #[allow(non_camel_case_types)]
    pub type uadv = u64;

    pub use std::sync::atomic::AtomicU64 as AtomicUadv;
}

#[cfg(not(olio_std_atomic_u64))]
mod types {
    #[allow(non_camel_case_types)]
    pub type uadv = usize;

    pub use std::sync::atomic::AtomicUsize as AtomicUadv;
}

use self::types::*;

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
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "libc::posix_madvise error return code {}", self.ecode)
    }
}

impl std::error::Error for MemAdviseError {}

/// Memory access pattern advice.
///
/// This encodes a subset of POSIX.1-2001 `madvise` flags, and is intending to
/// be a workable cross platform abstraction. In particular, the values do not
/// correspond to any libc or other lib constants, and are arranged in
/// ascending order of minimal to maximum *priority* in the presence of
/// concurrent interest in the same region.
#[cfg(olio_std_atomic_u64)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u64)]
pub enum MemAdvice {
    Normal     = 0,       // Not counted
    Random     = 0x003FF, // Bits  1-10 mask value
    Sequential = 0xFFC00, // Bits 11-20 mask value
}

/// Memory access pattern advice.
///
/// This encodes a subset of POSIX.1-2001 `madvise` flags, and is intending to
/// be a workable cross platform abstraction. In particular, the values do not
/// correspond to any libc or other lib constants, and are arranged in
/// ascending order of minimal to maximum *priority* in the presence of
/// concurrent interest in the same region.
#[cfg(not(olio_std_atomic_u64))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(usize)]
pub enum MemAdvice {
    Normal     = 0,       // Not counted
    Random     = 0x003FF, // Bits  1-10 mask value
    Sequential = 0xFFC00, // Bits 11-20 mask value
}

impl From<uadv> for MemAdvice {
    fn from(v: uadv) -> Self {
        match v {
            0       => MemAdvice::Normal,
            0x003FF => MemAdvice::Random,
            0xFFC00 => MemAdvice::Sequential,
            _       => unreachable!("not a MemAdvice repr!"),
        }
    }
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
    advice: AtomicUadv,
}

impl<T> MemHandle<T>
    where T: Deref<Target=[u8]>
{
    /// Wrap an owned instance of a byte slice buffer. Additional (atomic)
    /// references to the underlying buffer can then be created by `clone` of
    /// this handle.
    pub fn new(mem: T) -> MemHandle<T> {
        MemHandle {
            mem: Arc::new(Mem::new(mem)),
            advice: AtomicUadv::new(MemAdvice::Normal as uadv)
        }
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
        let prior = self.advice.swap(advice as uadv, SeqCst).into();
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
        MemHandle {
            mem: self.mem.clone(),
            advice: AtomicUadv::new(MemAdvice::Normal as uadv)
        }
    }
}

impl<T> Drop for MemHandle<T>
    where T: Deref<Target=[u8]>
{
    fn drop(&mut self) {
        let advice = self.advice.load(Acquire).into();
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
    advisors: AtomicUadv,
}

impl<T> Mem<T>
    where T: Deref<Target=[u8]>
{
    fn new(mem: T) -> Mem<T> {
        Mem { mem, advisors: AtomicUadv::new(0) }
    }

    fn adjust_advice(&self, prior: MemAdvice, advice: MemAdvice)
        -> Result<MemAdvice, MemAdviseError>
    {
        debug_assert!(prior != advice);
        let mut adv = self.advisors.load(Acquire);
        loop {
            let old_top = top_most(adv);
            let new_adv = decr_advisors(adv, prior);
            let new_adv = incr_advisors(new_adv, advice);
            let new_top = top_most(new_adv);
            match self.advisors.compare_exchange_weak(
                adv, new_adv, SeqCst, Acquire)
            {
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
fn decr_advisors(mut advisors: uadv, prior: MemAdvice) -> uadv {
    if prior != MemAdvice::Normal {
        let mut p = advisors & (prior as uadv);
        advisors -= p;
        if prior == MemAdvice::Sequential { p >>= 10; }
        if p > 0 { p -= 1; }
        if prior == MemAdvice::Sequential { p <<= 10; }
        advisors |= p;
    }
    advisors
}

// Given packed advisors state, and new advice, return incremented state.
fn incr_advisors(mut advisors: uadv, advice: MemAdvice) -> uadv {
    let mut cur = advisors & (advice as uadv);
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
fn top_most(advisors: uadv) -> MemAdvice {
    if (advisors & (MemAdvice::Sequential as uadv)) > 0 {
        MemAdvice::Sequential
    } else if (advisors & (MemAdvice::Random as uadv)) > 0 {
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
    fn is_sync<T: Sync>() -> bool { true }

    #[test]
    fn test_send_sync() {
        assert!(is_send::<MemHandle<Vec<u8>>>());
        assert!(is_sync::<MemHandle<Vec<u8>>>());
    }

    #[cfg(feature = "mmap")]
    mod mmap {
        use std::io::Write;
        use std::thread;

        use tempfile::tempfile;
        use memmap::Mmap;

        use rand::seq::SliceRandom;

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
                    threads.push(thread::spawn(move || {
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
