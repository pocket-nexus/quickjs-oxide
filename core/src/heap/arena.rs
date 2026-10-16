//! `Arena<T>`: a `Vec<T>` with a free list, a side mark bitmap and a side live
//! bitmap.
//!
//! The live bitmap is what sweep uses to tell free slots from dead records, so
//! it exists in every build. Only debug and `gc-stress` builds assert liveness
//! on access; release builds check nothing.

use std::mem;

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

    /// Stores `value`, reusing the free list first. Returns its index.
    #[inline]
    pub fn alloc(&mut self, value: T) -> u32 {
        self.extra += value.extra_bytes();
        self.live_count += 1;
        if let Some(index) = self.free.pop() {
            self.records[index as usize] = value;
            let (word, mask) = bit(index);
            self.live[word] |= mask;
            return index;
        }
        self.push(value)
    }

    #[inline(never)]
    fn push(&mut self, value: T) -> u32 {
        let index = u32::try_from(self.records.len()).expect("arena index overflow");
        if self.records.len() == self.records.capacity() {
            self.records.reserve_exact(self.records.capacity().max(4));
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
        let mut capacity = self.records.capacity();
        let old = capacity;
        while capacity < needed {
            capacity += capacity.max(4);
        }
        (capacity - old) * mem::size_of::<T>()
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
    /// `dead`. Returns the number freed.
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
        freed
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
        let mut arena = Arena::new();
        for value in 0..10 {
            assert_eq!(arena.alloc(Item(value)), value);
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
            .map(|value| arena.alloc(Item(100 + value)))
            .collect::<Vec<_>>();
        assert_eq!(reused, [1, 2, 4, 5, 6, 7, 8]);
        assert_eq!(arena.alloc(Item(200)), 10);
    }

    #[test]
    fn free_poisons_and_growth_is_reported() {
        let mut arena = Arena::new();
        assert_eq!(arena.growth_bytes(1), 4 * size_of::<Item>());
        let index = arena.alloc(Item(7));
        assert_eq!(arena.growth_bytes(3), 0);
        assert_eq!(arena.free(index), Item(7));
        assert!(!arena.is_live(index));
        assert_eq!(arena.growth_bytes(4), 0);
        assert_eq!(arena.growth_bytes(5), 4 * size_of::<Item>());
    }

    #[test]
    #[cfg(any(debug_assertions, feature = "gc-stress"))]
    #[should_panic(expected = "stale")]
    fn stale_access_panics() {
        let mut arena = Arena::new();
        let index = arena.alloc(Item(1));
        arena.free(index);
        let _ = arena.get(index);
    }
}
