//! The memory report (phase 6, section 2.4): what every part of a runtime
//! holds, read from the bitmaps, free lists and capacities without a
//! collection. [`MemoryReport::reserved_bytes`] equals [`Heap::bytes`], the
//! total the heap cap compares against.

use std::fmt;

use super::Heap;
use super::arena::{Arena, Record};
use super::gc::GcStats;

/// One typed arena's records and storage.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ArenaReport {
    /// Live records.
    pub live: usize,
    /// Slots, live or free.
    pub capacity: usize,
    /// Bytes held by live records and their out-of-line payloads.
    pub used_bytes: usize,
    /// Bytes reserved: record storage, bitmaps, the free list and payloads.
    pub reserved_bytes: usize,
}

impl ArenaReport {
    fn of<T: Record>(arena: &Arena<T>) -> Self {
        ArenaReport {
            live: arena.live_count(),
            capacity: arena.slot_count(),
            used_bytes: arena.used_bytes(),
            reserved_bytes: arena.bytes(),
        }
    }
}

/// The value heap: the ranges of slots that objects' properties and
/// elements live in.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ValueHeapReport {
    /// Slots held by live ranges.
    pub used_slots: usize,
    /// Free slots by size class: `free_by_class[k]` counts free ranges of
    /// `2^k` slots.
    pub free_by_class: Vec<usize>,
    /// Slots allocated, used or free.
    pub capacity_slots: usize,
    pub reserved_bytes: usize,
}

/// What the code store and its side tables hold. The code store is not on
/// the heap: it is not collected and does not count against the cap.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CodeReport {
    pub units: usize,
    pub instructions: usize,
    pub instruction_bytes: usize,
    /// Bytes of the units' source-position tables.
    pub position_bytes: usize,
    /// Bytes of source text kept for `Function.prototype.toString`.
    pub source_bytes: usize,
    /// Compiled regular-expression programs.
    pub regexp_programs: usize,
}

/// A runtime's memory, part by part.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MemoryReport {
    pub objects: ArenaReport,
    pub shapes: ArenaReport,
    pub strings: ArenaReport,
    pub symbols: ArenaReport,
    pub bigints: ArenaReport,
    pub cells: ArenaReport,
    pub functions: ArenaReport,
    pub bytecode: ArenaReport,
    pub internals: ArenaReport,
    pub values: ValueHeapReport,
    /// Bytes of flattened ropes (the strings arena counts a rope as empty).
    pub rope_bytes: usize,
    /// Interned strings.
    pub atoms: usize,
    pub code: CodeReport,
    /// Registers the register file has room for.
    pub register_capacity: usize,
    /// Registers made addressable so far: the file never shrinks, so this is
    /// its high-water mark.
    pub register_high_water: usize,
    /// Inline-cache entries.
    pub ic_entries: usize,
    pub gc: GcStats,
    /// Nominal bytes allocated since the last collection, and the count at
    /// which the next one runs.
    pub allocated_since_gc: usize,
    pub threshold: usize,
    /// The heap cap in bytes; `usize::MAX` for none.
    pub cap: usize,
}

impl MemoryReport {
    /// The arenas by name, in report order.
    pub fn arenas(&self) -> [(&'static str, &ArenaReport); 9] {
        [
            ("objects", &self.objects),
            ("shapes", &self.shapes),
            ("strings", &self.strings),
            ("symbols", &self.symbols),
            ("bigints", &self.bigints),
            ("cells", &self.cells),
            ("functions", &self.functions),
            ("bytecode", &self.bytecode),
            ("internals", &self.internals),
        ]
    }

    /// Bytes reserved on the heap: what the cap compares against.
    pub fn reserved_bytes(&self) -> usize {
        self.arenas()
            .iter()
            .map(|(_, arena)| arena.reserved_bytes)
            .sum::<usize>()
            + self.values.reserved_bytes
            + self.rope_bytes
    }
}

impl Heap {
    pub(crate) fn report(&self) -> MemoryReport {
        let mut free_by_class = Vec::new();
        for range in self.values.free_blocks() {
            let class = range.len.trailing_zeros() as usize;
            if free_by_class.len() <= class {
                free_by_class.resize(class + 1, 0);
            }
            free_by_class[class] += 1;
        }
        MemoryReport {
            objects: ArenaReport::of(&self.objects),
            shapes: ArenaReport::of(&self.shapes),
            strings: ArenaReport::of(&self.strings),
            symbols: ArenaReport::of(&self.symbols),
            bigints: ArenaReport::of(&self.bigints),
            cells: ArenaReport::of(&self.cells),
            functions: ArenaReport::of(&self.payloads.functions),
            bytecode: ArenaReport::of(&self.payloads.bytecode),
            internals: ArenaReport::of(&self.payloads.internals),
            values: ValueHeapReport {
                used_slots: self.values.used_slots(),
                free_by_class,
                capacity_slots: self.values.used_slots() + self.values.free_slots(),
                reserved_bytes: self.values.bytes(),
            },
            rope_bytes: self.rope_bytes.get(),
            atoms: self.atoms.len(),
            code: CodeReport::default(),
            register_capacity: self.vm.registers.capacity(),
            register_high_water: self.vm.registers.len(),
            ic_entries: self.ics.len(),
            gc: self.gc.stats,
            allocated_since_gc: self.gc.allocated,
            threshold: self.gc.threshold,
            cap: self.gc.limits.max_bytes,
        }
    }
}

fn kib(bytes: usize) -> String {
    format!("{:.1} KiB", bytes as f64 / 1024.0)
}

impl fmt::Display for MemoryReport {
    /// `qjs --memory-report`'s table.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "{:<12}{:>10}{:>10}{:>14}{:>14}",
            "memory", "live", "capacity", "used", "reserved"
        )?;
        for (name, arena) in self.arenas() {
            writeln!(
                f,
                "{name:<12}{:>10}{:>10}{:>14}{:>14}",
                arena.live,
                arena.capacity,
                kib(arena.used_bytes),
                kib(arena.reserved_bytes)
            )?;
        }
        let free: usize = self.values.capacity_slots - self.values.used_slots;
        writeln!(
            f,
            "{:<12}{:>10}{:>10}{:>14}{:>14}",
            "values",
            self.values.used_slots,
            self.values.capacity_slots,
            kib(self.values.used_slots * 8),
            kib(self.values.reserved_bytes)
        )?;
        writeln!(
            f,
            "  free value slots: {free} in {:?} by class",
            self.values.free_by_class
        )?;
        writeln!(f, "ropes: {}; atoms: {}", kib(self.rope_bytes), self.atoms)?;
        writeln!(
            f,
            "heap reserved: {}; threshold {}; cap {}",
            kib(self.reserved_bytes()),
            kib(self.threshold),
            if self.cap == usize::MAX {
                "none".to_owned()
            } else {
                kib(self.cap)
            }
        )?;
        writeln!(
            f,
            "code: {} units, {} instructions ({}), positions {}, source {}, {} regexp programs",
            self.code.units,
            self.code.instructions,
            kib(self.code.instruction_bytes),
            kib(self.code.position_bytes),
            kib(self.code.source_bytes),
            self.code.regexp_programs
        )?;
        writeln!(
            f,
            "registers: {} high-water, {} capacity; inline caches: {} entries",
            self.register_high_water, self.register_capacity, self.ic_entries
        )?;
        writeln!(
            f,
            "gc: {} collections, {} records and {} freed, live {}",
            self.gc.collections,
            self.gc.freed_records,
            kib(self.gc.freed_bytes as usize),
            kib(self.gc.live_bytes)
        )
    }
}
