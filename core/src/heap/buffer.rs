//! Array buffer storage and typed-array views.
//!
//! An `ArrayBuffer`'s bytes live in a [`Region`], either one the engine
//! allocated or one it adopted from its embedder; a `SharedArrayBuffer`'s live
//! in a [`SharedBytes`], an `Arc` of a region other agents may hold too, which
//! `Atomics` operates on directly. A typed array or `DataView` is a [`View`]
//! over a buffer object held in its internal slot 0.

use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::AtomicU8;

use super::region::Region;

/// The bytes of a shared buffer, which other agents may hold too.
#[derive(Debug)]
pub struct SharedBytes {
    region: Region,
}

impl SharedBytes {
    pub fn new(len: usize, capacity: usize) -> Arc<SharedBytes> {
        SharedBytes::over(Region::owned_shared(len, capacity))
    }

    /// A shared buffer over a region that already exists: the caller's, or one
    /// the constructor just allocated.
    pub fn over(region: Region) -> Arc<SharedBytes> {
        Arc::new(SharedBytes { region })
    }

    pub fn region(&self) -> &Region {
        &self.region
    }

    pub fn len(&self) -> usize {
        self.region.len()
    }

    pub fn is_empty(&self) -> bool {
        self.region.is_empty()
    }

    pub fn capacity(&self) -> usize {
        self.region.capacity()
    }

    /// The identity `Atomics` waiters key on, equal for two wrappers over one
    /// base.
    pub fn key(&self) -> u64 {
        self.region.key()
    }

    /// Grows to `len` (at most the capacity); false if another agent made
    /// it longer meanwhile or `len` is shorter.
    pub fn grow(&self, len: usize) -> bool {
        self.region.grow(len)
    }

    pub fn byte(&self, index: usize) -> &AtomicU8 {
        self.region.byte(index)
    }
}

/// Where a buffer's bytes are.
#[derive(Debug)]
pub enum Store {
    /// Bytes this buffer owns outright, which may be an embedder's region.
    Owned(Region),
    Shared(Arc<SharedBytes>),
}

impl Store {
    /// The bytes, whichever arm holds them.
    pub fn region(&self) -> &Region {
        match self {
            Store::Owned(region) => region,
            Store::Shared(shared) => shared.region(),
        }
    }

    /// The region itself, for a store that is not shared: an `Arc` cannot be
    /// borrowed mutably, and a shared region only ever grows in place.
    pub fn owned_region_mut(&mut self) -> Option<&mut Region> {
        match self {
            Store::Owned(region) => Some(region),
            Store::Shared(_) => None,
        }
    }
}

/// An `ArrayBuffer` or `SharedArrayBuffer` record.
#[derive(Debug)]
pub struct Buffer {
    pub store: Store,
    pub detached: bool,
    /// `maxByteLength` of a resizable or growable buffer.
    pub max: Option<usize>,
}

impl Buffer {
    pub fn owned(len: usize, max: Option<usize>) -> Buffer {
        Buffer {
            store: Store::Owned(Region::owned(len)),
            detached: false,
            max,
        }
    }

    pub fn shared(len: usize, max: Option<usize>) -> Buffer {
        Buffer {
            store: Store::Shared(SharedBytes::new(len, max.unwrap_or(len))),
            detached: false,
            max,
        }
    }

    /// A buffer over a region that already exists, of the region's length and
    /// not resizable: an embedder grows one by making another over the same
    /// base.
    pub fn over(region: Region, shared: bool) -> Buffer {
        let store = if shared {
            Store::Shared(SharedBytes::over(region))
        } else {
            Store::Owned(region)
        };
        Buffer {
            store,
            detached: false,
            max: None,
        }
    }

    pub fn is_shared(&self) -> bool {
        matches!(self.store, Store::Shared(_))
    }

    pub fn len(&self) -> usize {
        self.store.region().len()
    }

    pub fn is_empty(&self) -> bool {
        self.store.region().is_empty()
    }

    pub fn owned_bytes(&self) -> usize {
        self.store.region().capacity()
    }

    /// Makes the length `len`, keeping the bytes; false when the region holds
    /// no more than it already does (a caller's region, or a shared one).
    pub fn resize_len(&mut self, len: usize) -> bool {
        match &mut self.store {
            Store::Owned(region) => region.resize(len),
            Store::Shared(shared) => shared.grow(len),
        }
    }

    /// Copies `out.len()` bytes from `offset`.
    #[inline]
    pub fn read(&self, offset: usize, out: &mut [u8]) {
        self.store.region().read(offset, out);
    }

    /// Writes `data` at `offset`.
    #[inline]
    pub fn write(&mut self, offset: usize, data: &[u8]) {
        self.store.region().write(offset, data);
    }

    /// Copies `len` bytes within the buffer (`memmove`).
    pub fn copy_within(&mut self, from: usize, to: usize, len: usize) {
        self.store.region().copy_within(from, to, len);
    }

    /// The bytes as one slice, for bulk work: `None` for a shared buffer and
    /// for a region the engine only adopted, whose bytes another thread may
    /// write.
    pub fn owned_slice(&self) -> Option<&[u8]> {
        match &self.store {
            Store::Owned(region) => region.as_bytes(),
            Store::Shared(_) => None,
        }
    }

    pub fn owned_slice_mut(&mut self) -> Option<&mut [u8]> {
        match &mut self.store {
            Store::Owned(region) => region.as_bytes_mut(),
            Store::Shared(_) => None,
        }
    }

    /// The bytes as a copy, for any store.
    pub fn to_vec(&self, offset: usize, len: usize) -> Vec<u8> {
        let mut out = alloc::vec![0; len];
        self.read(offset, &mut out);
        out
    }
}

/// A typed array's element type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ElementKind {
    Int8,
    Uint8,
    Uint8Clamped,
    Int16,
    Uint16,
    Int32,
    Uint32,
    Float16,
    Float32,
    Float64,
    BigInt64,
    BigUint64,
}

impl ElementKind {
    pub const ALL: [ElementKind; 12] = [
        ElementKind::Int8,
        ElementKind::Uint8,
        ElementKind::Uint8Clamped,
        ElementKind::Int16,
        ElementKind::Uint16,
        ElementKind::Int32,
        ElementKind::Uint32,
        ElementKind::Float16,
        ElementKind::Float32,
        ElementKind::Float64,
        ElementKind::BigInt64,
        ElementKind::BigUint64,
    ];

    #[inline]
    pub const fn size(self) -> usize {
        match self {
            ElementKind::Int8 | ElementKind::Uint8 | ElementKind::Uint8Clamped => 1,
            ElementKind::Int16 | ElementKind::Uint16 | ElementKind::Float16 => 2,
            ElementKind::Int32 | ElementKind::Uint32 | ElementKind::Float32 => 4,
            ElementKind::Float64 | ElementKind::BigInt64 | ElementKind::BigUint64 => 8,
        }
    }

    #[inline]
    pub const fn is_bigint(self) -> bool {
        matches!(self, ElementKind::BigInt64 | ElementKind::BigUint64)
    }

    pub const fn name(self) -> &'static str {
        match self {
            ElementKind::Int8 => "Int8Array",
            ElementKind::Uint8 => "Uint8Array",
            ElementKind::Uint8Clamped => "Uint8ClampedArray",
            ElementKind::Int16 => "Int16Array",
            ElementKind::Uint16 => "Uint16Array",
            ElementKind::Int32 => "Int32Array",
            ElementKind::Uint32 => "Uint32Array",
            ElementKind::Float16 => "Float16Array",
            ElementKind::Float32 => "Float32Array",
            ElementKind::Float64 => "Float64Array",
            ElementKind::BigInt64 => "BigInt64Array",
            ElementKind::BigUint64 => "BigUint64Array",
        }
    }

    pub fn from_index(index: u8) -> ElementKind {
        ElementKind::ALL[index as usize]
    }
}

/// A typed array's or a `DataView`'s window on its buffer.
#[derive(Clone, Copy, Debug)]
pub struct View {
    pub kind: ElementKind,
    pub byte_offset: usize,
    /// The length in elements (bytes for a `DataView`); `None` tracks the
    /// buffer's length.
    pub length: Option<usize>,
}

impl View {
    /// The current length in elements, or `None` when the view is out of
    /// bounds of a buffer `buffer_len` bytes long (or detached).
    #[inline]
    pub fn current_length(
        &self,
        buffer_len: usize,
        detached: bool,
        element: usize,
    ) -> Option<usize> {
        if detached || self.byte_offset > buffer_len {
            return None;
        }
        match self.length {
            Some(length) => (self.byte_offset + length * element <= buffer_len).then_some(length),
            None => Some((buffer_len - self.byte_offset) / element),
        }
    }
}

/// QuickJS's limit on a buffer's length (`INT32_MAX`).
pub const MAX_BUFFER_LENGTH: usize = i32::MAX as usize;

impl super::Heap {
    /// Charges `extra` bytes about to be allocated outside the arenas to the
    /// trigger; `keep` survives the safepoint.
    pub(crate) fn charge(
        &mut self,
        extra: usize,
        keep: &[crate::value::Value],
    ) -> Result<(), super::HeapError> {
        self.safepoint(
            &super::Need {
                extra,
                ..Default::default()
            },
            keep,
        )
    }
}
