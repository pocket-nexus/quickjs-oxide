//! The value heap: one `Vec<Value>` allocated in power-of-two ranges.
//!
//! Every property value, dense element and scope slot lives in a range. Size
//! classes 1, 2, 4, …, 256 each keep a free list; larger ranges are also
//! power-of-two blocks and use the same per-class lists (the "large-block
//! list" of the plan is the classes above 256). A range's length is always its
//! class size, which lets an object header store it as one `u8` exponent.
//! Phase 1 does not compact; [`ValueHeap::fragmentation`] reports what churn
//! leaves behind.

use crate::value::Value;

const CLASSES: usize = 32;

/// A block of the value heap. `len` is zero or a power of two.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Range {
    pub start: u32,
    pub len: u32,
}

impl Range {
    pub const EMPTY: Range = Range { start: 0, len: 0 };

    /// The `u8` exponent stored in object headers; `u8::MAX` for an empty range.
    #[inline]
    pub const fn log(self) -> u8 {
        if self.len == 0 {
            u8::MAX
        } else {
            self.len.trailing_zeros() as u8
        }
    }

    #[inline]
    pub const fn from_log(start: u32, log: u8) -> Range {
        if log == u8::MAX {
            Range::EMPTY
        } else {
            Range {
                start,
                len: 1 << log,
            }
        }
    }
}

/// Rounds a requested length to its class size.
#[inline]
pub fn class_len(len: u32) -> u32 {
    if len == 0 { 0 } else { len.next_power_of_two() }
}

pub struct ValueHeap {
    values: Vec<Value>,
    free: [Vec<u32>; CLASSES],
    free_slots: usize,
}

impl Default for ValueHeap {
    fn default() -> Self {
        Self::new()
    }
}

impl ValueHeap {
    pub fn new() -> Self {
        ValueHeap {
            values: Vec::new(),
            free: std::array::from_fn(|_| Vec::new()),
            free_slots: 0,
        }
    }

    /// Allocates a range of at least `len` slots, filled with `Value::EMPTY`.
    /// The returned length is rounded up to the class size.
    pub fn alloc(&mut self, len: u32) -> Range {
        let len = class_len(len);
        if len == 0 {
            return Range::EMPTY;
        }
        let class = len.trailing_zeros() as usize;
        if let Some(start) = self.free[class].pop() {
            self.free_slots -= len as usize;
            return Range { start, len };
        }
        let start = u32::try_from(self.values.len()).expect("value heap overflow");
        let end = self.values.len() + len as usize;
        assert!(end <= u32::MAX as usize, "value heap overflow");
        if end > self.values.capacity() {
            let target = end.max(self.values.capacity() * 2);
            self.values.reserve_exact(target - self.values.len());
        }
        self.values.resize(end, Value::EMPTY);
        Range { start, len }
    }

    /// Bytes the backing vector grows by if a range of `len` is allocated now.
    pub fn growth_bytes(&self, len: u32) -> usize {
        let len = class_len(len);
        if len == 0 || !self.free[len.trailing_zeros() as usize].is_empty() {
            return 0;
        }
        let end = self.values.len() + len as usize;
        if end <= self.values.capacity() {
            0
        } else {
            (end.max(self.values.capacity() * 2) - self.values.capacity()) * 8
        }
    }

    /// Returns a range of at least `new_len` holding `range`'s values: the same
    /// range when it is already large enough, otherwise a new one (alloc, copy,
    /// free).
    pub fn grow(&mut self, range: Range, new_len: u32) -> Range {
        if new_len <= range.len {
            return range;
        }
        let new = self.alloc(new_len);
        let (from, to) = (range.start as usize, new.start as usize);
        self.values.copy_within(from..from + range.len as usize, to);
        self.free(range);
        new
    }

    /// Returns a range to its class. The slots are poisoned with `EMPTY`.
    pub fn free(&mut self, range: Range) {
        if range.len == 0 {
            return;
        }
        debug_assert!(range.len.is_power_of_two());
        let start = range.start as usize;
        self.values[start..start + range.len as usize].fill(Value::EMPTY);
        self.free[range.len.trailing_zeros() as usize].push(range.start);
        self.free_slots += range.len as usize;
    }

    #[inline]
    #[track_caller]
    pub fn get(&self, range: Range, slot: u32) -> Value {
        debug_assert!(
            slot < range.len,
            "slot {slot} outside range of {}",
            range.len
        );
        self.values[(range.start + slot) as usize]
    }

    #[inline]
    #[track_caller]
    pub fn set(&mut self, range: Range, slot: u32, value: Value) {
        debug_assert!(
            slot < range.len,
            "slot {slot} outside range of {}",
            range.len
        );
        self.values[(range.start + slot) as usize] = value;
    }

    #[inline]
    pub fn slice(&self, range: Range) -> &[Value] {
        let start = range.start as usize;
        &self.values[start..start + range.len as usize]
    }

    #[inline]
    pub fn slice_mut(&mut self, range: Range) -> &mut [Value] {
        let start = range.start as usize;
        &mut self.values[start..start + range.len as usize]
    }

    /// Slots handed out, free or in use.
    pub fn total_slots(&self) -> usize {
        self.values.len()
    }

    /// Slots sitting in free lists.
    pub fn free_slots(&self) -> usize {
        self.free_slots
    }

    /// Slots held by live ranges.
    pub fn used_slots(&self) -> usize {
        self.values.len() - self.free_slots
    }

    /// Free-list slots over all handed-out slots.
    pub fn fragmentation(&self) -> f64 {
        if self.values.is_empty() {
            0.0
        } else {
            self.free_slots as f64 / self.values.len() as f64
        }
    }

    pub fn bytes(&self) -> usize {
        self.values.capacity() * 8
            + self
                .free
                .iter()
                .map(|list| list.capacity() * 4)
                .sum::<usize>()
    }

    /// Every free block, for the stress-mode verifier.
    pub fn free_blocks(&self) -> impl Iterator<Item = Range> + '_ {
        self.free.iter().enumerate().flat_map(|(class, starts)| {
            starts.iter().map(move |&start| Range {
                start,
                len: 1 << class,
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classes_round_up_and_reuse() {
        let mut heap = ValueHeap::new();
        let a = heap.alloc(3);
        assert_eq!(a, Range { start: 0, len: 4 });
        assert!(heap.slice(a).iter().all(|v| v.is_empty()));
        heap.set(a, 2, Value::int32(5));
        let b = heap.alloc(1);
        assert_eq!(b, Range { start: 4, len: 1 });
        heap.free(a);
        assert_eq!(heap.free_slots(), 4);
        let c = heap.alloc(4);
        assert_eq!(c.start, 0);
        assert!(heap.get(c, 2).is_empty(), "freed slots are poisoned");
        assert_eq!(heap.alloc(0), Range::EMPTY);
    }

    #[test]
    fn grow_copies_and_frees() {
        let mut heap = ValueHeap::new();
        let a = heap.alloc(2);
        heap.set(a, 0, Value::int32(1));
        heap.set(a, 1, Value::int32(2));
        assert_eq!(heap.grow(a, 2), a);
        let b = heap.grow(a, 3);
        assert_eq!(b.len, 4);
        assert_eq!(heap.get(b, 0), Value::int32(1));
        assert_eq!(heap.get(b, 1), Value::int32(2));
        assert!(heap.get(b, 2).is_empty());
        assert_eq!(heap.free_blocks().collect::<Vec<_>>(), [a]);
        let large = heap.alloc(1000);
        assert_eq!(large.len, 1024);
        heap.free(large);
        assert_eq!(heap.alloc(513).start, large.start);
    }

    #[test]
    fn range_log_round_trips() {
        for len in [0, 1, 2, 256, 1 << 20] {
            let range = Range { start: 9, len };
            let decoded = Range::from_log(9, range.log());
            assert_eq!(decoded.len, len);
            assert!(len == 0 || decoded == range);
        }
    }
}
