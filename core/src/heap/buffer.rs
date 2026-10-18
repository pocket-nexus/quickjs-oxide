//! Array buffer storage and typed-array views.
//!
//! An `ArrayBuffer` owns its bytes; a `SharedArrayBuffer`'s bytes are an
//! `Arc` of atomic bytes, allocated at the maximum length for a growable
//! one, so agents on other threads share them in safe Rust and `Atomics`
//! operates on them directly. A typed array or `DataView` is a [`View`]
//! over a buffer object held in its internal slot 0.

use std::sync::Arc;
use std::sync::atomic::{AtomicU8, AtomicUsize, Ordering};

/// The bytes of a shared buffer, which other agents may hold too.
#[derive(Debug)]
pub struct SharedBytes {
    bytes: Box<[AtomicU8]>,
    len: AtomicUsize,
}

impl SharedBytes {
    pub fn new(len: usize, capacity: usize) -> Arc<SharedBytes> {
        Arc::new(SharedBytes {
            bytes: (0..capacity.max(len)).map(|_| AtomicU8::new(0)).collect(),
            len: AtomicUsize::new(len),
        })
    }

    pub fn len(&self) -> usize {
        self.len.load(Ordering::SeqCst)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn capacity(&self) -> usize {
        self.bytes.len()
    }

    /// Grows to `len` (at most the capacity); false if another agent made
    /// it longer meanwhile or `len` is shorter.
    pub fn grow(&self, len: usize) -> bool {
        let current = self.len();
        len >= current
            && len <= self.capacity()
            && self
                .len
                .compare_exchange(current, len, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
    }

    pub fn byte(&self, index: usize) -> &AtomicU8 {
        &self.bytes[index]
    }
}

/// Where a buffer's bytes are.
#[derive(Debug)]
pub enum Store {
    Owned(Vec<u8>),
    Shared(Arc<SharedBytes>),
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
            store: Store::Owned(vec![0; len]),
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

    pub fn is_shared(&self) -> bool {
        matches!(self.store, Store::Shared(_))
    }

    pub fn len(&self) -> usize {
        match &self.store {
            Store::Owned(bytes) => bytes.len(),
            Store::Shared(shared) => shared.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn owned_bytes(&self) -> usize {
        match &self.store {
            Store::Owned(bytes) => bytes.capacity(),
            Store::Shared(shared) => shared.capacity(),
        }
    }

    /// Copies `out.len()` bytes from `offset`.
    #[inline]
    pub fn read(&self, offset: usize, out: &mut [u8]) {
        if out.is_empty() {
            return;
        }
        match &self.store {
            Store::Owned(bytes) => out.copy_from_slice(&bytes[offset..offset + out.len()]),
            Store::Shared(shared) => {
                for (index, byte) in out.iter_mut().enumerate() {
                    *byte = shared.byte(offset + index).load(Ordering::Relaxed);
                }
            }
        }
    }

    /// Writes `data` at `offset`.
    #[inline]
    pub fn write(&mut self, offset: usize, data: &[u8]) {
        if data.is_empty() {
            return;
        }
        match &mut self.store {
            Store::Owned(bytes) => bytes[offset..offset + data.len()].copy_from_slice(data),
            Store::Shared(shared) => {
                for (index, byte) in data.iter().enumerate() {
                    shared.byte(offset + index).store(*byte, Ordering::Relaxed);
                }
            }
        }
    }

    /// Copies `len` bytes within the buffer (`memmove`).
    pub fn copy_within(&mut self, from: usize, to: usize, len: usize) {
        if len == 0 {
            return;
        }
        match &mut self.store {
            Store::Owned(bytes) => bytes.copy_within(from..from + len, to),
            Store::Shared(_) => {
                let mut temporary = vec![0; len];
                self.read(from, &mut temporary);
                self.write(to, &temporary);
            }
        }
    }

    /// The owned bytes, for bulk work (`None` for a shared buffer).
    pub fn owned_slice(&self) -> Option<&[u8]> {
        match &self.store {
            Store::Owned(bytes) => Some(bytes),
            Store::Shared(_) => None,
        }
    }

    pub fn owned_slice_mut(&mut self) -> Option<&mut Vec<u8>> {
        match &mut self.store {
            Store::Owned(bytes) => Some(bytes),
            Store::Shared(_) => None,
        }
    }

    /// The bytes as a copy, for any store.
    pub fn to_vec(&self, offset: usize, len: usize) -> Vec<u8> {
        let mut out = vec![0; len];
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
