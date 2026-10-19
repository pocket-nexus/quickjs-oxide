//! The value heap: one `Vec<Value>` allocated in power-of-two ranges.
//!
//! Every property value, dense element and scope slot lives in a range. Each
//! power-of-two size class keeps a free list. A range's length is always its
//! class size, which lets an object header store it as one `u8` exponent.
//! A collection's sweep records the ranges it frees in a bitmap of free slots
//! and rebuilds the free space from its maximal runs: short runs go on the
//! class lists, long ones become free runs that requests take from in address
//! order, a long free run at the end is cut off the heap and its capacity
//! given back. A request whose class list is empty takes from the current
//! free run, then splits the smallest larger free range. The value heap does not move
//! ranges; [`ValueHeap::fragmentation`] reports what churn leaves behind.

use super::arena::{grown_capacity, step_bytes};
use super::gc::Room;
use crate::value::Value;

const CLASSES: usize = 32;
/// Free runs shorter than this go on the class lists after a collection.
const SHORT_RUN: usize = 64;

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
    /// Bit `k` is set while list `k` may hold a range: set by every push,
    /// cleared when a split finds the list empty.
    listed: u32,
    /// The long free runs `[start, end)` the last collection left, highest
    /// first, each with the longest length among it and the runs above it.
    runs: Vec<(u32, u32, u32)>,
    /// The free run requests take from now.
    current: (u32, u32),
    free_slots: usize,
    /// One bit per slot, set for a free one, during a sweep.
    bits: Vec<u64>,
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
            listed: 0,
            runs: Vec::new(),
            current: (0, 0),
            free_slots: 0,
            bits: Vec::new(),
        }
    }

    /// Allocates a range of at least `len` slots, filled with `Value::EMPTY`.
    /// The returned length is rounded up to the class size. A growth step is
    /// charged to `room`.
    #[inline]
    pub fn alloc(&mut self, len: u32, room: &mut Room) -> Range {
        if len == 0 {
            return Range::EMPTY;
        }
        let len = len.next_power_of_two();
        if let Some(start) = self.free[len.trailing_zeros() as usize].pop() {
            self.free_slots -= len as usize;
            return Range { start, len };
        }
        let (start, end) = self.current;
        if end - start >= len {
            self.current.0 += len;
            self.free_slots -= len as usize;
            return Range { start, len };
        }
        self.alloc_slow(len, room)
    }

    /// A range of class `len` from the current free run, split from the
    /// smallest larger free range, or fresh past the end.
    #[inline(never)]
    fn alloc_slow(&mut self, len: u32, room: &mut Room) -> Range {
        loop {
            let (start, end) = self.current;
            if end - start >= len {
                self.current.0 += len;
                self.free_slots -= len as usize;
                return Range { start, len };
            }
            // The rest of the run is too short: it goes on the class lists,
            // and the next run is taken.
            self.push_run(start as usize, end as usize);
            match self.runs.pop() {
                Some((start, end, _)) => self.current = (start, end),
                None => {
                    self.current = (0, 0);
                    break;
                }
            }
        }
        let class = len.trailing_zeros();
        let mut larger = self.listed & !((2u32 << class) - 1);
        while larger != 0 {
            let source = larger.trailing_zeros();
            larger &= larger - 1;
            let Some(start) = self.free[source as usize].pop() else {
                self.listed &= !(1 << source);
                continue;
            };
            // `[start, start + len)` is returned and the rest of the block
            // goes back as the largest ranges that fit.
            self.push_run((start + len) as usize, (start + (1 << source)) as usize);
            self.free_slots -= len as usize;
            return Range { start, len };
        }
        self.alloc_fresh(len, room)
    }

    #[inline]
    fn push(&mut self, start: u32, class: u32) {
        self.free[class as usize].push(start);
        self.listed |= 1 << class;
    }

    /// Lists the free slots `[start, stop)` as the largest ranges that fit,
    /// from `start`.
    fn push_run(&mut self, mut start: usize, stop: usize) {
        while start < stop {
            let class = (usize::BITS - 1 - (stop - start).leading_zeros()).min(CLASSES as u32 - 1);
            self.push(start as u32, class);
            start += 1 << class;
        }
    }

    fn alloc_fresh(&mut self, len: u32, room: &mut Room) -> Range {
        let start = u32::try_from(self.values.len()).expect("value heap overflow");
        let end = self.values.len() + len as usize;
        assert!(end <= u32::MAX as usize, "value heap overflow");
        if end > self.values.capacity() {
            let before = self.values.capacity();
            let target = grown_capacity(before, end, 8);
            self.values.reserve_exact(target - self.values.len());
            room.take((self.values.capacity() - before) * 8);
        }
        self.values.resize(end, Value::EMPTY);
        Range { start, len }
    }

    /// Bytes the backing vector grows by if a range of `len` is allocated now.
    pub fn growth_bytes(&self, len: u32) -> usize {
        let len = class_len(len);
        let (start, end) = self.current;
        if len == 0
            || end - start >= len
            || self
                .runs
                .last()
                .is_some_and(|&(_, _, longest)| longest >= len)
            || self.free[len.trailing_zeros() as usize..]
                .iter()
                .any(|list| !list.is_empty())
        {
            return 0;
        }
        let end = self.values.len() + len as usize;
        let capacity = self.values.capacity();
        (grown_capacity(capacity, end, 8) - capacity) * 8
    }

    /// Returns a range of at least `new_len` holding `range`'s values: the same
    /// range when it is already large enough, otherwise a new one (alloc, copy,
    /// free).
    pub fn grow(&mut self, range: Range, new_len: u32, room: &mut Room) -> Range {
        if new_len <= range.len {
            return range;
        }
        let new = self.alloc(new_len, room);
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
        self.push(range.start, range.len.trailing_zeros());
        self.free_slots += range.len as usize;
    }

    /// Starts a collection's sweep: the free space moves into the bitmap of
    /// free slots, where [`ValueHeap::release`] records the ranges the sweep
    /// frees, and [`ValueHeap::finish_sweep`] rebuilds it from the bitmap.
    pub fn begin_sweep(&mut self) {
        let mut bits = std::mem::take(&mut self.bits);
        bits.clear();
        bits.resize(self.values.len().div_ceil(64), 0);
        for (class, list) in self.free.iter_mut().enumerate() {
            for start in list.drain(..) {
                set_bits(&mut bits, start as usize, start as usize + (1 << class));
            }
        }
        let (start, end) = std::mem::take(&mut self.current);
        for (start, end, _) in self.runs.drain(..).chain([(start, end, 0)]) {
            if start < end {
                set_bits(&mut bits, start as usize, end as usize);
            }
        }
        self.bits = bits;
        self.listed = 0;
        self.free_slots = 0;
    }

    /// Frees a range during the sweep: poisons it with `EMPTY` and records it
    /// in the bitmap of free slots.
    pub fn release(&mut self, range: Range) {
        if range.len == 0 {
            return;
        }
        let (start, end) = (range.start as usize, (range.start + range.len) as usize);
        self.values[start..end].fill(Value::EMPTY);
        set_bits(&mut self.bits, start, end);
    }

    /// Ends the sweep: rebuilds the free space from the maximal runs of free
    /// slots. A run shorter than [`SHORT_RUN`] goes on the class lists, cut
    /// from its start into the largest power-of-two ranges that fit (a run
    /// of six becomes ranges of four and two); a longer one becomes a free
    /// run. A run that ends the heap and spans more than a quarter of its
    /// capacity is cut off instead, and the capacity past one growth step
    /// goes back to the allocator. Lower ranges and runs are reused first.
    pub fn finish_sweep(&mut self) {
        let len = self.values.len();
        let bits = std::mem::take(&mut self.bits);
        let mut at = 0;
        while let Some(start) = next_bit(&bits, at, true, len) {
            let stop = next_bit(&bits, start, false, len).unwrap_or(len);
            let capacity = self.values.capacity();
            if stop == len && (capacity - start) * 4 > capacity {
                self.values.truncate(start);
                self.values.shrink_to(start + step_bytes(start * 8, 8) / 8);
                break;
            }
            self.free_slots += stop - start;
            if stop - start < SHORT_RUN {
                self.push_run(start, stop);
            } else {
                self.runs.push((start as u32, stop as u32, 0));
            }
            at = stop;
        }
        self.bits = bits;
        self.runs.reverse();
        let mut longest = 0;
        for run in &mut self.runs {
            longest = longest.max(run.1 - run.0);
            run.2 = longest;
        }
        for list in &mut self.free {
            list.reverse();
            if list.capacity() > 2 * list.len() + 16 {
                list.shrink_to(2 * list.len());
            }
        }
    }

    /// Coalesces the free lists outside a collection.
    pub fn coalesce(&mut self) {
        self.begin_sweep();
        self.finish_sweep();
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

    /// The value at `slot` of `range`, or `Value::EMPTY` past its end.
    #[inline]
    pub fn get_or_empty(&self, range: Range, slot: u32) -> Value {
        if slot < range.len {
            self.get(range, slot)
        } else {
            Value::EMPTY
        }
    }

    /// An absolute slot; callers check the range in debug builds.
    #[inline]
    pub fn get_at(&self, index: u32) -> Value {
        self.values[index as usize]
    }

    #[inline]
    pub fn set_at(&mut self, index: u32, value: Value) {
        self.values[index as usize] = value;
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
            + self.runs.capacity() * 12
            + self.bits.capacity() * 8
    }

    /// Every free range of the class lists.
    pub fn free_blocks(&self) -> impl Iterator<Item = Range> + '_ {
        self.free.iter().enumerate().flat_map(|(class, starts)| {
            starts.iter().map(move |&start| Range {
                start,
                len: 1 << class,
            })
        })
    }

    /// The free runs, the current one first, for the stress-mode verifier.
    pub fn free_runs(&self) -> impl Iterator<Item = (u32, u32)> + '_ {
        let (start, end) = self.current;
        (start < end)
            .then_some((start, end))
            .into_iter()
            .chain(self.runs.iter().rev().map(|&(start, end, _)| (start, end)))
    }
}

/// Sets bits `[from, to)`.
fn set_bits(bits: &mut [u64], from: usize, to: usize) {
    let (first, last) = (from / 64, (to - 1) / 64);
    let low = !0u64 << (from % 64);
    let high = !0u64 >> (63 - (to - 1) % 64);
    if first == last {
        bits[first] |= low & high;
        return;
    }
    bits[first] |= low;
    bits[first + 1..last].fill(!0);
    bits[last] |= high;
}

/// The first bit at or after `from`, below `len`, that is set (`set`) or
/// clear.
fn next_bit(bits: &[u64], from: usize, set: bool, len: usize) -> Option<usize> {
    if from >= len {
        return None;
    }
    let flip = if set { 0 } else { !0 };
    let mut word = from / 64;
    let mut current = (bits[word] ^ flip) & (!0u64 << (from % 64));
    loop {
        if current != 0 {
            let index = word * 64 + current.trailing_zeros() as usize;
            return (index < len).then_some(index);
        }
        word += 1;
        if word == bits.len() {
            return None;
        }
        current = bits[word] ^ flip;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classes_round_up_and_reuse() {
        let room = &mut Room::unbounded();
        let mut heap = ValueHeap::new();
        let a = heap.alloc(3, room);
        assert_eq!(a, Range { start: 0, len: 4 });
        assert!(heap.slice(a).iter().all(|v| v.is_empty()));
        heap.set(a, 2, Value::int32(5));
        let b = heap.alloc(1, room);
        assert_eq!(b, Range { start: 4, len: 1 });
        heap.free(a);
        assert_eq!(heap.free_slots(), 4);
        let c = heap.alloc(4, room);
        assert_eq!(c.start, 0);
        assert!(heap.get(c, 2).is_empty(), "freed slots are poisoned");
        assert_eq!(heap.alloc(0, room), Range::EMPTY);
    }

    #[test]
    fn grow_copies_and_frees() {
        let room = &mut Room::unbounded();
        let mut heap = ValueHeap::new();
        let a = heap.alloc(2, room);
        heap.set(a, 0, Value::int32(1));
        heap.set(a, 1, Value::int32(2));
        assert_eq!(heap.grow(a, 2, room), a);
        let b = heap.grow(a, 3, room);
        assert_eq!(b.len, 4);
        assert_eq!(heap.get(b, 0), Value::int32(1));
        assert_eq!(heap.get(b, 1), Value::int32(2));
        assert!(heap.get(b, 2).is_empty());
        assert_eq!(heap.free_blocks().collect::<Vec<_>>(), [a]);
        let large = heap.alloc(1000, room);
        assert_eq!(large.len, 1024);
        heap.free(large);
        assert_eq!(heap.alloc(513, room).start, large.start);
    }

    #[test]
    fn a_request_splits_a_larger_range() {
        let room = &mut Room::unbounded();
        let mut heap = ValueHeap::new();
        let big = heap.alloc(16, room);
        let after = heap.alloc(1, room);
        heap.free(big);
        assert_eq!(heap.alloc(2, room), Range { start: 0, len: 2 });
        // The rest of the block: ranges of 8, 4 and 2.
        assert_eq!(heap.alloc(2, room), Range { start: 14, len: 2 });
        assert_eq!(heap.alloc(8, room), Range { start: 2, len: 8 });
        assert_eq!(heap.alloc(4, room), Range { start: 10, len: 4 });
        assert_eq!(heap.free_slots(), 0);
        assert_eq!(heap.alloc(1, room).start, after.start + 1);
    }

    #[test]
    fn requests_take_from_free_runs_in_address_order() {
        let room = &mut Room::unbounded();
        let mut heap = ValueHeap::new();
        heap.alloc(1, room);
        let big = heap.alloc(1024, room);
        let tail = heap.alloc(1, room);
        heap.free(big);
        heap.coalesce();
        assert_eq!(heap.free_runs().collect::<Vec<_>>(), [(1, 1025)]);
        assert_eq!(heap.alloc(4, room).start, 1);
        assert_eq!(heap.alloc(2, room).start, 5);
        assert_eq!(heap.alloc(512, room).start, 7);
        // 506 slots are left: too few, so they go on the class lists.
        assert_eq!(heap.alloc(512, room).start, tail.start + 1);
        assert_eq!(heap.free_runs().count(), 0);
        assert_eq!(heap.free_slots(), 506);
        assert_eq!(heap.alloc(256, room).start, 519);
    }

    #[test]
    fn coalescing_rebuilds_runs_and_cuts_the_end() {
        let room = &mut Room::unbounded();
        let mut heap = ValueHeap::new();
        let ranges: Vec<Range> = (0..8).map(|_| heap.alloc(2, room)).collect();
        // Free slots 2..8 (a run of six) and 10..16 (the end).
        for &index in &[1, 2, 3, 5, 6, 7] {
            heap.free(ranges[index]);
        }
        heap.coalesce();
        assert_eq!(heap.total_slots(), 10);
        assert_eq!(heap.free_slots(), 6);
        let mut free: Vec<Range> = heap.free_blocks().collect();
        free.sort_by_key(|range| range.start);
        assert_eq!(
            free,
            [Range { start: 2, len: 4 }, Range { start: 6, len: 2 }]
        );
        assert_eq!(heap.alloc(4, room).start, 2);
        assert_eq!(heap.alloc(2, room).start, 6);
        assert_eq!(heap.alloc(2, room).start, 10);
    }

    #[test]
    fn a_short_free_end_stays_listed() {
        let room = &mut Room::unbounded();
        let mut heap = ValueHeap::new();
        let ranges: Vec<Range> = (0..8).map(|_| heap.alloc(2, room)).collect();
        heap.free(ranges[7]);
        heap.coalesce();
        assert_eq!(heap.total_slots(), 16);
        assert_eq!(heap.free_blocks().collect::<Vec<_>>(), [ranges[7]]);
    }

    #[test]
    fn a_long_free_end_gives_back_capacity() {
        let room = &mut Room::unbounded();
        let mut heap = ValueHeap::new();
        let keep = heap.alloc(64, room);
        let large = heap.alloc(1 << 20, room);
        heap.free(large);
        let before = heap.bytes();
        heap.coalesce();
        assert_eq!(heap.total_slots(), 64);
        assert!(heap.bytes() < before / 4);
        assert_eq!(heap.alloc(64, room).start, keep.start + 64);
    }

    #[test]
    fn bit_runs() {
        let mut bits = vec![0u64; 3];
        set_bits(&mut bits, 3, 130);
        assert_eq!(next_bit(&bits, 0, true, 192), Some(3));
        assert_eq!(next_bit(&bits, 3, false, 192), Some(130));
        assert_eq!(next_bit(&bits, 130, true, 192), None);
        set_bits(&mut bits, 150, 151);
        assert_eq!(next_bit(&bits, 130, true, 192), Some(150));
        assert_eq!(next_bit(&bits, 150, false, 140), None);
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
