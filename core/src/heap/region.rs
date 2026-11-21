//! A buffer's bytes: a region the engine allocated itself (`Own`) or adopted
//! from its embedder (`Foreign`).
//!
//! A region is the only place the engine turns an index into a byte, so it is
//! the only module that writes `unsafe`: the pointer arithmetic in
//! [`Region::byte`] and the slice reconstructions in [`Region::as_bytes`] and
//! [`Region::as_bytes_mut`].
//!
//! An `Own` region reallocates when it outgrows its capacity, so its base
//! moves with it. A `Foreign` region never moves: its capacity is what the
//! caller guaranteed, and a view over it keeps the address it was built at for
//! as long as the region lives. Anything a foreign region hands out therefore
//! goes through [`Region::read`], [`Region::write`] and
//! [`Region::copy_within`], one relaxed atomic per byte, never through a
//! `&[u8]` the engine could be holding while another thread writes.

#![allow(unsafe_code)]

use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec;
use core::any::Any;
use core::sync::atomic::{AtomicU8, AtomicUsize, Ordering};
use core::{fmt, ptr, slice};

/// What keeps a region's bytes alive.
pub enum Owner {
    /// Bytes this region allocated, one atomic byte each.
    Own(Box<[AtomicU8]>),
    /// The embedder's bytes: dropping the guard releases them.
    Foreign(Arc<dyn Any + Send + Sync>),
}

/// A length of bytes a buffer records, with the identity `Atomics` waits on.
pub struct Region {
    /// The first byte; never null, and valid for `capacity` bytes.
    base: *mut AtomicU8,
    /// The bytes the region may use: the length of a `Box` it allocated, or
    /// what the caller of [`Region::foreign`] promised.
    capacity: usize,
    /// The length in use, which a growable shared region raises in place.
    len: AtomicUsize,
    /// Identity for `Atomics` waiters: equal for two regions over one base.
    key: u64,
    /// Whether another thread may read or write these bytes.
    shared: bool,
    /// Keeps `base` alive.
    owner: Owner,
}

impl Region {
    /// A region of `len` zero bytes the engine owns outright.
    pub fn owned(len: usize) -> Region {
        Region::from_owned(Region::boxed(len), len, false)
    }

    /// A region of `len` zero bytes in use and `capacity` allocated, which
    /// agents on other threads may reach.
    pub fn owned_shared(len: usize, capacity: usize) -> Region {
        let capacity = capacity.max(len);
        Region::from_owned(Region::boxed(capacity), len, true)
    }

    /// Adopts bytes the caller owns.
    ///
    /// `key` identifies the region to `Atomics` waiters; `0` asks for it to
    /// be derived from `base`, which is what two regions over one base need to
    /// agree on without the caller naming them.
    ///
    /// # Safety
    ///
    /// `base` must point to `capacity` bytes that the engine may read and
    /// write for as long as the returned region lives; `owner` must be what
    /// releases them, and nothing else may release them before it drops. When
    /// `shared` is false, the caller must keep every other thread away from
    /// the bytes: the engine then reaches them without atomics.
    pub unsafe fn foreign(
        base: *mut u8,
        len: usize,
        capacity: usize,
        key: u64,
        shared: bool,
        owner: Box<dyn Any + Send + Sync>,
    ) -> Region {
        assert!(!base.is_null(), "a region needs a base");
        assert!(
            len <= capacity,
            "a region cannot be longer than its capacity"
        );
        let base = base as *mut AtomicU8;
        Region {
            base,
            capacity,
            len: AtomicUsize::new(len),
            key: if key == 0 { base as u64 } else { key },
            shared,
            owner: Owner::Foreign(Arc::from(owner)),
        }
    }

    fn boxed(capacity: usize) -> Box<[AtomicU8]> {
        (0..capacity).map(|_| AtomicU8::new(0)).collect()
    }

    fn from_owned(bytes: Box<[AtomicU8]>, len: usize, shared: bool) -> Region {
        let capacity = bytes.len();
        let base = bytes.as_ptr() as *mut AtomicU8;
        Region {
            base,
            capacity,
            len: AtomicUsize::new(len),
            key: base as u64,
            shared,
            owner: Owner::Own(bytes),
        }
    }

    /// The length in bytes.
    #[inline]
    pub fn len(&self) -> usize {
        self.len.load(Ordering::SeqCst)
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The bytes the region may use.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// The identity `Atomics` waiters key on.
    #[inline]
    pub fn key(&self) -> u64 {
        self.key
    }

    /// Whether another thread may reach these bytes.
    #[inline]
    pub fn is_shared(&self) -> bool {
        self.shared
    }

    /// Whether the engine allocated these bytes and no other thread reaches
    /// them; only then is there a `&[u8]` to hand out.
    #[inline]
    fn is_local(&self) -> bool {
        !self.shared && matches!(self.owner, Owner::Own(_))
    }

    /// The byte at `index`: the one place an index becomes a pointer.
    #[inline]
    pub fn byte(&self, index: usize) -> &AtomicU8 {
        debug_assert!(index < self.capacity, "a byte past the region");
        // SAFETY: `index` is inside the capacity the base was built with, and
        // the owner keeps those bytes alive for as long as `self` lives.
        unsafe { &*self.base.add(index) }
    }

    /// The bytes in use as a slice, for a region the engine owns outright;
    /// `None` for a shared or foreign one, whose bytes only go through
    /// [`Region::read`].
    pub fn as_bytes(&self) -> Option<&[u8]> {
        if !self.is_local() {
            return None;
        }
        let len = self.len();
        // SAFETY: `AtomicU8` has the size and alignment of `u8`, the owner
        // keeps `capacity` of them alive, `len` is at most that, and no other
        // thread reaches them.
        Some(unsafe { slice::from_raw_parts(self.base as *const u8, len) })
    }

    /// The bytes in use as a mutable slice, for a region the engine owns
    /// outright.
    pub fn as_bytes_mut(&mut self) -> Option<&mut [u8]> {
        if !self.is_local() {
            return None;
        }
        let len = self.len();
        // SAFETY: as `as_bytes`, and `&mut self` means no other reference to
        // the region exists to observe the write.
        Some(unsafe { slice::from_raw_parts_mut(self.base as *mut u8, len) })
    }

    /// Raises the length to `len` (at most the capacity); false when it is
    /// shorter than the current length or another agent grew it meanwhile.
    pub fn grow(&self, len: usize) -> bool {
        let current = self.len();
        len >= current
            && len <= self.capacity
            && self
                .len
                .compare_exchange(current, len, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
    }

    /// Makes the length `len`, reallocating a region the engine owns when
    /// `len` outgrows it; false when a caller's region cannot hold that much.
    /// A shared region only ever grows, in place, through [`Region::grow`].
    pub fn resize(&mut self, len: usize) -> bool {
        if len <= self.capacity {
            let before = self.len();
            if self.shared && len < before {
                return false;
            }
            // An `ArrayBuffer` that grows exposes zero bytes, however much of
            // the capacity an earlier shrink left behind.
            for index in before..len {
                self.byte(index).store(0, Ordering::Relaxed);
            }
            self.len.store(len, Ordering::SeqCst);
            return true;
        }
        if !self.is_local() {
            return false;
        }
        let before = self.len();
        let mut bytes = Region::boxed(len);
        for (index, byte) in bytes.iter_mut().enumerate().take(before) {
            byte.store(self.byte(index).load(Ordering::Relaxed), Ordering::Relaxed);
        }
        self.base = bytes.as_ptr() as *mut AtomicU8;
        self.capacity = len;
        self.len.store(len, Ordering::SeqCst);
        self.key = self.base as u64;
        self.owner = Owner::Own(bytes);
        true
    }

    /// Copies `out.len()` bytes from `offset`.
    #[inline]
    pub fn read(&self, offset: usize, out: &mut [u8]) {
        if out.is_empty() {
            return;
        }
        if let Some(bytes) = self.as_bytes() {
            out.copy_from_slice(&bytes[offset..offset + out.len()]);
            return;
        }
        for (index, byte) in out.iter_mut().enumerate() {
            *byte = self.byte(offset + index).load(Ordering::Relaxed);
        }
    }

    /// Writes `data` at `offset`.
    #[inline]
    pub fn write(&self, offset: usize, data: &[u8]) {
        if data.is_empty() {
            return;
        }
        if self.is_local() {
            // SAFETY: the region owns these bytes and no other thread reaches
            // them. No `&[u8]` from `as_bytes` is live here: that borrow is
            // tied to `&Region`, and every call site of `write` holds a
            // mutable borrow of the buffer instead.
            unsafe {
                let target = self.base.add(offset) as *mut u8;
                ptr::copy_nonoverlapping(data.as_ptr(), target, data.len());
            }
            return;
        }
        for (index, byte) in data.iter().enumerate() {
            self.byte(offset + index).store(*byte, Ordering::Relaxed);
        }
    }

    /// Copies `len` bytes within the region (`memmove`).
    pub fn copy_within(&self, from: usize, to: usize, len: usize) {
        if len == 0 {
            return;
        }
        if self.is_local() {
            // SAFETY: as `write`: the bytes are the region's own, no other
            // thread reaches them and no slice of them is borrowed here.
            unsafe {
                let source = self.base.add(from) as *const u8;
                let target = self.base.add(to) as *mut u8;
                ptr::copy(source, target, len);
            }
            return;
        }
        let mut temporary = vec![0; len];
        self.read(from, &mut temporary);
        self.write(to, &temporary);
    }
}

impl fmt::Debug for Region {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Region")
            .field("base", &self.base)
            .field("capacity", &self.capacity)
            .field("len", &self.len())
            .field("key", &self.key)
            .field("shared", &self.shared)
            .field(
                "owner",
                &match self.owner {
                    Owner::Own(_) => "own",
                    Owner::Foreign(_) => "foreign",
                },
            )
            .finish()
    }
}

// SAFETY: the base points into the owner's bytes, which stay where they are
// for as long as the region lives, so moving the region to another thread moves
// the owner with it and leaves every address valid. Every write goes through an
// `AtomicU8`, or through `write`/`copy_within` on a region the engine owns
// outright and no other thread reaches.
unsafe impl Send for Region {}
// SAFETY: as `Send`, and `&Region` only reads bytes atomically or views the
// engine's own unshared ones.
unsafe impl Sync for Region {}

#[cfg(test)]
mod tests {
    use super::*;

    /// The guard an embedder hands over: whatever it holds is released with
    /// the region.
    struct Bytes(#[allow(dead_code)] Box<[AtomicU8]>);

    fn bytes(len: usize) -> (Box<[AtomicU8]>, *mut u8) {
        let allocated: Box<[AtomicU8]> = (0..len).map(|_| AtomicU8::new(0)).collect();
        let base = allocated.as_ptr() as *mut u8;
        (allocated, base)
    }

    #[test]
    fn an_owned_region_reallocates_and_zeroes_the_bytes_it_grows_into() {
        let mut region = Region::owned(4);
        region.write(0, &[1, 2, 3, 4]);
        assert!(region.resize(8));
        assert_eq!(region.len(), 8);
        assert_eq!(region.capacity(), 8);
        assert_eq!(region.as_bytes().unwrap(), [1, 2, 3, 4, 0, 0, 0, 0]);
        // A shrink keeps the allocation, and growing back into it exposes
        // zeros rather than what the bytes held before.
        assert!(region.resize(1));
        region.write(0, &[9]);
        assert_eq!(region.capacity(), 8);
        assert!(region.resize(8));
        assert_eq!(region.as_bytes().unwrap(), [9, 0, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn a_foreign_region_keeps_its_base_and_its_capacity() {
        let (allocated, base) = bytes(8);
        // SAFETY: `allocated` holds eight bytes, they outlive the region
        // through the guard, and only this test reaches them.
        let mut region =
            unsafe { Region::foreign(base, 4, 8, 0, false, Box::new(Bytes(allocated))) };
        assert_eq!(region.key(), base as u64);
        region.write(0, &[1, 2, 3, 4]);
        assert!(region.resize(8), "inside the caller's capacity");
        assert!(!region.resize(9), "past the caller's capacity");
        assert_eq!(region.capacity(), 8);
        assert!(region.as_bytes().is_none(), "a caller's bytes are no slice");
        let mut out = [0u8; 4];
        region.read(0, &mut out);
        assert_eq!(out, [1, 2, 3, 4]);
    }

    #[test]
    fn a_named_key_wins_over_the_one_the_base_gives() {
        let (allocated, base) = bytes(4);
        let derived = unsafe { Region::foreign(base, 4, 4, 0, true, Box::new(Bytes(allocated))) };
        assert_eq!(derived.key(), base as u64);
        let (allocated, base) = bytes(4);
        let named = unsafe { Region::foreign(base, 4, 4, 41, true, Box::new(Bytes(allocated))) };
        assert_eq!(named.key(), 41);
    }
}
