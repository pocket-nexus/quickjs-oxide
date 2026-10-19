//! Code units compiled at run time: `eval`, the `Function` constructors and
//! `$262.evalScript` publish into the heap, which frees a unit once no
//! bytecode record names it.
//!
//! A unit is reached through the `unit` index of its bytecode records, so a
//! collection marks it while marking them: a live function, a running frame
//! (through its function), a suspended generator and a pending job all reach
//! one. A unit published but not yet instantiated has no record; it is
//! pinned until its compilation ends. The loop holds a running unit through
//! an [`Rc`], so freeing a slot never invalidates the code it is executing;
//! by the time a slot is swept no frame names it and its count is one.
//!
//! A live unit marks its atoms ([`CodeUnit::atoms`]); a freed one returns its
//! cache entries to the [`IcStore`](crate::vm::ic::IcStore) and its bytes to
//! the heap's account.

use std::rc::Rc;

use crate::bytecode::{CodeUnit, CodeUnitId, DYNAMIC};

#[derive(Default)]
pub struct DynamicUnits {
    /// By slot: the index of a dynamic [`CodeUnitId`] without its tag.
    slots: Vec<Option<Rc<CodeUnit>>>,
    /// Empty slots, reused lowest first.
    free: Vec<u32>,
    /// Under `gc-stress`, slots freed by a sweep are reusable only after the
    /// next one, so a stale id is not immediately given to another unit.
    #[cfg(feature = "gc-stress")]
    quarantine: Vec<u32>,
    /// Published units not yet instantiated, a stack the compilations
    /// truncate when they end.
    pinned: Vec<u32>,
    marks: Vec<u64>,
    live: usize,
    live_bytes: usize,
}

#[inline]
fn bit(index: u32) -> (usize, u64) {
    ((index / 64) as usize, 1u64 << (index % 64))
}

impl DynamicUnits {
    /// Stores a validated unit whose cache entries are reserved and pins it.
    /// Returns its id and the bytes it holds.
    pub(crate) fn publish(&mut self, mut unit: CodeUnit) -> (CodeUnitId, usize) {
        unit.atoms = unit.key_atoms();
        let bytes = unit.bytes();
        let unit = Some(Rc::new(unit));
        let index = match self.free.pop() {
            Some(index) => {
                self.slots[index as usize] = unit;
                index
            }
            None => {
                let index = u32::try_from(self.slots.len())
                    .ok()
                    .filter(|&index| index < DYNAMIC)
                    .expect("dynamic code overflow");
                self.slots.push(unit);
                if index as usize / 64 == self.marks.len() {
                    self.marks.push(0);
                }
                index
            }
        };
        self.pinned.push(index);
        self.live += 1;
        self.live_bytes += bytes;
        (CodeUnitId(index | DYNAMIC), bytes)
    }

    /// The live unit `id`.
    #[inline]
    #[track_caller]
    pub fn get(&self, id: CodeUnitId) -> Rc<CodeUnit> {
        Rc::clone(self.slot(id).expect("a live dynamic unit"))
    }

    /// The unit `id`, if it is live.
    pub fn lookup(&self, id: CodeUnitId) -> Option<&CodeUnit> {
        self.slot(id).map(|unit| &**unit)
    }

    #[inline]
    fn slot(&self, id: CodeUnitId) -> Option<&Rc<CodeUnit>> {
        debug_assert!(id.is_dynamic());
        self.slots.get((id.0 & !DYNAMIC) as usize)?.as_ref()
    }

    /// The live units with their ids, by slot.
    pub fn units(&self) -> impl Iterator<Item = (CodeUnitId, &CodeUnit)> {
        self.slots.iter().enumerate().filter_map(|(index, unit)| {
            let unit = unit.as_deref()?;
            Some((CodeUnitId(index as u32 | DYNAMIC), unit))
        })
    }

    /// Live units.
    pub fn len(&self) -> usize {
        self.live
    }

    pub fn is_empty(&self) -> bool {
        self.live == 0
    }

    /// Bytes the live units hold.
    pub fn live_bytes(&self) -> usize {
        self.live_bytes
    }

    /// The position a compilation's pins start at.
    pub(crate) fn pin_mark(&self) -> usize {
        self.pinned.len()
    }

    /// Unpins the units published since `mark`: instantiated, they are
    /// reached through their records; abandoned, the next collection frees
    /// them.
    pub(crate) fn unpin(&mut self, mark: usize) {
        self.pinned.truncate(mark);
    }

    pub(crate) fn pinned(&self) -> &[u32] {
        &self.pinned
    }

    pub(crate) fn clear_marks(&mut self) {
        self.marks.fill(0);
    }

    /// Marks unit `id`; the unit if it was not marked yet.
    #[inline]
    pub(crate) fn mark(&mut self, id: CodeUnitId) -> Option<&CodeUnit> {
        let index = id.0 & !DYNAMIC;
        let (word, mask) = bit(index);
        let bits = &mut self.marks[word];
        if *bits & mask != 0 {
            return None;
        }
        *bits |= mask;
        let unit = self.slots[index as usize].as_deref();
        debug_assert!(unit.is_some(), "a record names freed dynamic unit {index}");
        unit
    }

    pub(crate) fn is_marked(&self, index: u32) -> bool {
        let (word, mask) = bit(index);
        self.marks[word] & mask != 0
    }

    /// Frees every unmarked unit, passing each to `dead` before it is
    /// dropped. Returns the bytes freed.
    pub(crate) fn sweep(&mut self, mut dead: impl FnMut(&CodeUnit)) -> usize {
        #[cfg(feature = "gc-stress")]
        let held = {
            let released = std::mem::take(&mut self.quarantine);
            self.free.extend_from_slice(&released);
            self.free.len()
        };
        let mut freed = 0;
        for index in (0..self.slots.len() as u32).rev() {
            if self.is_marked(index) {
                continue;
            }
            let Some(unit) = self.slots[index as usize].take() else {
                continue;
            };
            debug_assert_eq!(
                Rc::strong_count(&unit),
                1,
                "a swept dynamic unit is still running"
            );
            dead(&unit);
            freed += unit.bytes();
            self.live -= 1;
            self.free.push(index);
        }
        self.live_bytes -= freed;
        #[cfg(feature = "gc-stress")]
        self.quarantine.extend(self.free.drain(held..));
        freed
    }

    /// Debug check after marking: every live unpinned unit is marked and
    /// every marked one is live.
    pub(crate) fn verify(&self) {
        for (index, unit) in self.slots.iter().enumerate() {
            let index = index as u32;
            assert_eq!(
                unit.is_some(),
                self.is_marked(index),
                "gc verify: dynamic unit {index} is {}",
                if unit.is_some() {
                    "live but unreachable"
                } else {
                    "reachable but freed"
                }
            );
        }
    }
}
