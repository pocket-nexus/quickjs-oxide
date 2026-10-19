//! `Arena<T>`: a `Vec<T>` with a free list, a side mark bitmap and a side live
//! bitmap.
//!
//! The live bitmap is what sweep uses to tell free slots from dead records, so
//! it exists in every build. Only debug and `gc-stress` builds assert liveness
//! on access; release builds check nothing.
//!
//! The record vector grows by [`step_bytes`] and gives back a trailing free
//! run of more than a quarter of its capacity after a sweep.

use std::mem;

use super::gc::Room;

/// A record stored in an [`Arena`].
pub trait Record {
    /// The poison value written over a freed slot. It must own no heap memory.
    fn vacant() -> Self;

    /// Bytes owned outside the arena slot (string contents, tables), counted
    /// toward the heap cap.
    fn extra_bytes(&self) -> usize {
        0
    }
}

const CHECK_LIVE: bool = cfg!(any(debug_assertions, feature = "gc-stress"));

/// Below this many bytes a vector doubles. The allocator keeps such blocks
/// in its heaps, where growing one copies it and leaves the old block
/// resident; a doubling makes that happen once per doubling. Larger blocks
/// are each their own mapping and grow in place (`mremap`), so they grow by
/// [`STEP`] and their slack stays below one step.
pub const DOUBLING_LIMIT: usize = 32 << 20;
pub const STEP: usize = 2 << 20;

/// The bytes a vector of `capacity` bytes grows by.
#[inline]
pub fn step_bytes(capacity: usize, unit: usize) -> usize {
    if capacity < DOUBLING_LIMIT {
        capacity.max(4 * unit)
    } else {
        STEP
    }
}

/// The capacity, in elements of `unit` bytes, after growing from `capacity`
/// until it holds `needed`.
pub fn grown_capacity(mut capacity: usize, needed: usize, unit: usize) -> usize {
    while capacity < needed {
        capacity += step_bytes(capacity * unit, unit).div_ceil(unit);
    }
    capacity
}

pub struct Arena<T> {
    records: Vec<T>,
    live: Vec<u64>,
    marks: Vec<u64>,
    /// Free indices. Sweep pushes them in descending order so the lowest index
    /// is reused first; reuse is deterministic.
    free: Vec<u32>,
    /// Under `gc-stress`, indices freed by a sweep are reusable only after the
    /// next sweep, so a stale index is not immediately recycled into a live
    /// record and the liveness assertion catches it.
    #[cfg(feature = "gc-stress")]
    quarantine: Vec<u32>,
    live_count: usize,
    extra: usize,
}

#[inline]
fn bit(index: u32) -> (usize, u64) {
    ((index / 64) as usize, 1u64 << (index % 64))
}

impl<T: Record> Default for Arena<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Record> Arena<T> {
    pub fn new() -> Self {
        Arena {
            records: Vec::new(),
            live: Vec::new(),
            marks: Vec::new(),
            free: Vec::new(),
            #[cfg(feature = "gc-stress")]
            quarantine: Vec::new(),
            live_count: 0,
            extra: 0,
        }
    }

    /// Stores `value`, reusing the free list first. Returns its index. A
    /// growth step is charged to `room`.
    #[inline(always)]
    pub fn alloc(&mut self, value: T, room: &mut Room) -> u32 {
        self.extra += value.extra_bytes();
        self.live_count += 1;
        if let Some(index) = self.free.pop() {
            self.records[index as usize] = value;
            let (word, mask) = bit(index);
            self.live[word] |= mask;
            return index;
        }
        self.push(value, room)
    }

    #[inline(never)]
    fn push(&mut self, value: T, room: &mut Room) -> u32 {
        let index = u32::try_from(self.records.len()).expect("arena index overflow");
        if self.records.len() == self.records.capacity() {
            let before = self.bytes();
            let unit = mem::size_of::<T>().max(1);
            let old = self.records.capacity();
            self.records
                .reserve_exact(grown_capacity(old, old + 1, unit) - old);
            let words = self.records.capacity().div_ceil(64);
            self.live.reserve_exact(words - self.live.len());
            self.marks.reserve_exact(words - self.marks.len());
            room.take(self.bytes() - before);
        }
        self.records.push(value);
        let (word, mask) = bit(index);
        if word == self.live.len() {
            self.live.push(0);
            self.marks.push(0);
        }
        self.live[word] |= mask;
        index
    }

    /// Bytes the arena's record vector grows by if `count` records are
    /// allocated now (zero when the free list covers them).
    pub fn growth_bytes(&self, count: usize) -> usize {
        let fresh = count.saturating_sub(self.free.len());
        // Quarantined indices do not count: they are not reusable yet.
        let needed = self.records.len() + fresh;
        let old = self.records.capacity();
        let unit = mem::size_of::<T>().max(1);
        (grown_capacity(old, needed, unit) - old) * unit
    }

    #[inline]
    pub fn is_live(&self, index: u32) -> bool {
        let (word, mask) = bit(index);
        self.live.get(word).is_some_and(|bits| bits & mask != 0)
    }

    #[inline]
    #[track_caller]
    fn check(&self, index: u32) {
        if CHECK_LIVE {
            assert!(
                self.is_live(index),
                "stale {} index {index}",
                std::any::type_name::<T>()
            );
        }
    }

    #[inline]
    #[track_caller]
    pub fn get(&self, index: u32) -> &T {
        self.check(index);
        &self.records[index as usize]
    }

    #[inline]
    #[track_caller]
    pub fn get_mut(&mut self, index: u32) -> &mut T {
        self.check(index);
        &mut self.records[index as usize]
    }

    /// Frees one record: poisons it, clears its live bit and returns it.
    #[track_caller]
    pub fn free(&mut self, index: u32) -> T {
        assert!(self.is_live(index), "double free of index {index}");
        let record = mem::replace(&mut self.records[index as usize], T::vacant());
        self.extra -= record.extra_bytes();
        self.live_count -= 1;
        let (word, mask) = bit(index);
        self.live[word] &= !mask;
        self.free.push(index);
        record
    }

    /// Records whose extra bytes change in place report it here.
    pub fn adjust_extra(&mut self, old: usize, new: usize) {
        self.extra = self.extra - old + new;
    }

    pub fn live_count(&self) -> usize {
        self.live_count
    }

    /// Number of slots, live or free.
    pub fn slot_count(&self) -> usize {
        self.records.len()
    }

    /// Bytes reserved by the arena: record storage, bitmaps, free list and the
    /// records' extra bytes.
    pub fn bytes(&self) -> usize {
        self.records.capacity() * mem::size_of::<T>()
            + (self.live.capacity() + self.marks.capacity()) * 8
            + self.free.capacity() * 4
            + self.extra
    }

    /// Bytes held by live records.
    pub fn used_bytes(&self) -> usize {
        self.live_count * mem::size_of::<T>() + self.extra
    }

    /// Sets the mark bit; returns true if it was clear.
    #[inline]
    #[track_caller]
    pub fn mark(&mut self, index: u32) -> bool {
        self.check(index);
        let (word, mask) = bit(index);
        let bits = &mut self.marks[word];
        let fresh = *bits & mask == 0;
        *bits |= mask;
        fresh
    }

    #[inline]
    pub fn is_marked(&self, index: u32) -> bool {
        let (word, mask) = bit(index);
        self.marks.get(word).is_some_and(|bits| bits & mask != 0)
    }

    pub fn clear_marks(&mut self) {
        self.marks.fill(0);
    }

    /// Frees every live, unmarked record, highest index first, passing each to
    /// `dead`, then gives back a long trailing free run. Returns the number
    /// freed.
    pub fn sweep(&mut self, mut dead: impl FnMut(u32, T)) -> usize {
        #[cfg(feature = "gc-stress")]
        let held = {
            let released = std::mem::take(&mut self.quarantine);
            self.free.extend_from_slice(&released);
            self.free.len()
        };
        let mut freed = 0;
        for word in (0..self.live.len()).rev() {
            let mut garbage = self.live[word] & !self.marks[word];
            if garbage == 0 {
                continue;
            }
            self.live[word] &= !garbage;
            while garbage != 0 {
                let high = 63 - garbage.leading_zeros();
                garbage &= !(1u64 << high);
                let index = word as u32 * 64 + high;
                let record = mem::replace(&mut self.records[index as usize], T::vacant());
                self.extra -= record.extra_bytes();
                self.free.push(index);
                freed += 1;
                dead(index, record);
            }
        }
        self.live_count -= freed;
        #[cfg(feature = "gc-stress")]
        self.quarantine.extend(self.free.drain(held..));
        self.shrink();
        freed
    }

    /// Truncates a trailing free run of more than a quarter of the capacity
    /// and returns the memory past one growth step above the rest.
    fn shrink(&mut self) {
        let capacity = self.records.capacity();
        #[allow(unused_mut)]
        let mut end = match self.live.iter().rposition(|&bits| bits != 0) {
            Some(word) => word * 64 + 64 - self.live[word].leading_zeros() as usize,
            None => 0,
        };
        // Quarantined slots stay, so a stale index is not reused at once.
        #[cfg(feature = "gc-stress")]
        if let Some(&last) = self.quarantine.iter().max() {
            end = end.max(last as usize + 1);
        }
        if (capacity - end) * 4 <= capacity {
            return;
        }
        self.free.retain(|&index| (index as usize) < end);
        self.records.truncate(end);
        let unit = mem::size_of::<T>().max(1);
        self.records
            .shrink_to(end + step_bytes(end * unit, unit).div_ceil(unit));
        let words = self.records.capacity().div_ceil(64);
        self.live.truncate(words);
        self.marks.truncate(words);
        self.live.shrink_to(words);
        self.marks.shrink_to(words);
        self.free.shrink_to(self.free.len() * 2);
    }

    /// Live indices in ascending order.
    pub fn live_indices(&self) -> impl Iterator<Item = u32> + '_ {
        self.live.iter().enumerate().flat_map(|(word, &bits)| {
            let mut bits = bits;
            std::iter::from_fn(move || {
                if bits == 0 {
                    return None;
                }
                let low = bits.trailing_zeros();
                bits &= bits - 1;
                Some(word as u32 * 64 + low)
            })
        })
    }

    /// Free indices, including quarantined ones.
    pub fn free_indices(&self) -> impl Iterator<Item = u32> + '_ {
        let quarantine: &[u32] = {
            #[cfg(feature = "gc-stress")]
            {
                &self.quarantine
            }
            #[cfg(not(feature = "gc-stress"))]
            {
                &[]
            }
        };
        self.free.iter().chain(quarantine).copied()
    }

    /// Debug check: the live and mark bitmaps agree.
    pub fn marks_equal_live(&self) -> Option<u32> {
        self.live
            .iter()
            .zip(&self.marks)
            .enumerate()
            .find_map(|(word, (live, marks))| {
                let diff = live ^ marks;
                (diff != 0).then(|| word as u32 * 64 + diff.trailing_zeros())
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq)]
    struct Item(u32);

    impl Record for Item {
        fn vacant() -> Self {
            Item(u32::MAX)
        }
    }

    #[test]
    fn free_list_reuse_is_lowest_first_after_sweep() {
        let mut room = Room::unbounded();
        let mut arena = Arena::new();
        for value in 0..10 {
            assert_eq!(arena.alloc(Item(value), &mut room), value);
        }
        for keep in [0, 3, 9] {
            arena.mark(keep);
        }
        let mut dead = Vec::new();
        assert_eq!(arena.sweep(|index, item| dead.push((index, item.0))), 7);
        assert_eq!(dead.first(), Some(&(8, 8)));
        assert_eq!(arena.live_count(), 3);
        assert_eq!(arena.live_indices().collect::<Vec<_>>(), [0, 3, 9]);
        arena.clear_marks();
        #[cfg(feature = "gc-stress")]
        {
            // A second sweep releases the quarantine.
            for keep in [0, 3, 9] {
                arena.mark(keep);
            }
            assert_eq!(arena.sweep(|_, _| {}), 0);
        }
        let reused = (0..7)
            .map(|value| arena.alloc(Item(100 + value), &mut room))
            .collect::<Vec<_>>();
        assert_eq!(reused, [1, 2, 4, 5, 6, 7, 8]);
        assert_eq!(arena.alloc(Item(200), &mut room), 10);
    }

    #[test]
    fn free_poisons_and_growth_is_reported() {
        let mut room = Room::new(1 << 20, 1 << 20);
        let mut arena = Arena::new();
        assert_eq!(arena.growth_bytes(1), 4 * size_of::<Item>());
        let index = arena.alloc(Item(7), &mut room);
        assert_eq!(room.threshold(), 1 << 20);
        assert_eq!(arena.growth_bytes(3), 0);
        assert_eq!(arena.free(index), Item(7));
        assert!(!arena.is_live(index));
        assert_eq!(arena.growth_bytes(4), 0);
        assert_eq!(arena.growth_bytes(5), 4 * size_of::<Item>());
    }

    #[test]
    fn growth_steps_are_bounded() {
        assert_eq!(step_bytes(0, 24), 96);
        assert_eq!(step_bytes(4 << 20, 24), 4 << 20);
        assert_eq!(step_bytes(64 << 20, 24), STEP);
        let mut room = Room::unbounded();
        let mut arena = Arena::new();
        for value in 0..10_000_000 {
            arena.alloc(Item(value), &mut room);
        }
        let capacity = arena.records.capacity() * size_of::<Item>();
        let used = arena.records.len() * size_of::<Item>();
        assert!(capacity - used < STEP, "{capacity} for {used}");
    }

    #[test]
    fn a_trailing_free_run_is_given_back() {
        let mut room = Room::unbounded();
        let mut arena = Arena::new();
        for value in 0..100_000 {
            arena.alloc(Item(value), &mut room);
        }
        let before = arena.bytes();
        for keep in 0..1000 {
            arena.mark(keep);
        }
        arena.sweep(|_, _| {});
        #[cfg(feature = "gc-stress")]
        {
            // The quarantined slots stay until the next sweep releases them.
            assert_eq!(arena.slot_count(), 100_000);
            arena.clear_marks();
            for keep in 0..1000 {
                arena.mark(keep);
            }
            arena.sweep(|_, _| {});
        }
        assert_eq!(arena.slot_count(), 1000);
        assert!(arena.bytes() < before / 4);
        assert_eq!(arena.alloc(Item(5), &mut room), 1000);
    }

    #[test]
    fn a_step_past_the_budget_zeroes_the_threshold() {
        let mut room = Room::new(1 << 10, usize::MAX);
        let mut arena = Arena::new();
        let mut value = 0;
        while room.threshold() != 0 {
            assert!(arena.bytes() <= 1 << 10);
            arena.alloc(Item(value), &mut room);
            value += 1;
        }
        assert!(arena.bytes() > 1 << 10);
    }

    #[test]
    #[cfg(any(debug_assertions, feature = "gc-stress"))]
    #[should_panic(expected = "stale")]
    fn stale_access_panics() {
        let mut arena = Arena::new();
        let index = arena.alloc(Item(1), &mut Room::unbounded());
        arena.free(index);
        let _ = arena.get(index);
    }
}
