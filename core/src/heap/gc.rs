//! Root set, mark-sweep, trigger policy, stress mode and the heap cap.
//!
//! Non-moving mark-sweep. Mark bits live in each arena's side bitmap; marking
//! uses an explicit stack, never recursion. Sweep is eager and scans every
//! arena. Collection happens only at an allocation safepoint
//! ([`Heap::safepoint`]) or on an explicit [`Heap::collect`].

use std::mem;
use std::time::Duration;

use super::hash::FxHashMap;
use super::object::{InternalTag, NONE, ObjectKind};
use super::{Heap, HeapError, Need};
use crate::shape::STATIC_CELL;
use crate::value::{Kind, ShapeId, Value};
use crate::vm::ic::Record;

#[cfg(test)]
#[path = "gc_tests.rs"]
mod tests;

/// A position on the [`RootStack`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RootMark(usize);

/// Values Rust code holds across an allocation. The only legal way to keep an
/// index alive across a safepoint.
#[derive(Default)]
pub struct RootStack {
    values: Vec<Value>,
}

impl RootStack {
    #[inline]
    pub fn mark(&self) -> RootMark {
        RootMark(self.values.len())
    }

    #[inline]
    pub fn push(&mut self, value: Value) {
        self.values.push(value);
    }

    #[inline]
    /// Drops the last root.
    pub fn pop_one(&mut self) {
        self.values.pop();
    }

    pub fn truncate(&mut self, mark: RootMark) {
        debug_assert!(mark.0 <= self.values.len(), "root mark above the stack");
        self.values.truncate(mark.0);
    }

    #[inline]
    pub fn get(&self, index: usize) -> Value {
        self.values[index]
    }

    #[inline]
    pub fn set(&mut self, index: usize, value: Value) {
        self.values[index] = value;
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn values(&self) -> &[Value] {
        &self.values
    }
}

/// An embedder-held value, released explicitly.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Handle(u32);

#[derive(Default)]
pub struct Handles {
    slots: Vec<Value>,
    free: Vec<u32>,
}

impl Handles {
    pub fn create(&mut self, value: Value) -> Handle {
        debug_assert!(!value.is_empty(), "EMPTY marks a released handle");
        if let Some(index) = self.free.pop() {
            self.slots[index as usize] = value;
            return Handle(index);
        }
        self.slots.push(value);
        Handle(self.slots.len() as u32 - 1)
    }

    #[track_caller]
    pub fn get(&self, handle: Handle) -> Value {
        let value = self.slots[handle.0 as usize];
        debug_assert!(!value.is_empty(), "released handle");
        value
    }

    pub fn set(&mut self, handle: Handle, value: Value) {
        debug_assert!(!self.slots[handle.0 as usize].is_empty(), "released handle");
        self.slots[handle.0 as usize] = value;
    }

    pub fn release(&mut self, handle: Handle) {
        let slot = &mut self.slots[handle.0 as usize];
        assert!(!slot.is_empty(), "handle released twice");
        *slot = Value::EMPTY;
        self.free.push(handle.0);
    }

    pub fn live_count(&self) -> usize {
        self.slots.len() - self.free.len()
    }

    pub fn values(&self) -> impl Iterator<Item = Value> + '_ {
        self.slots.iter().copied().filter(|value| !value.is_empty())
    }
}

/// The root set owned by the heap, so that a safepoint inside `Heap::alloc_*`
/// sees every root without extra parameters.
#[derive(Default)]
pub struct Roots {
    pub stack: RootStack,
    pub handles: Handles,
    /// Runtime roots: the realm's global object and intrinsics, and from later
    /// phases the job queue and module registry.
    pub fixed: Vec<Value>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HeapLimits {
    /// Reserved bytes the heap may not exceed; `usize::MAX` for no cap.
    pub max_bytes: usize,
}

impl Default for HeapLimits {
    fn default() -> Self {
        HeapLimits {
            max_bytes: usize::MAX,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GcStats {
    pub collections: u64,
    pub total_pause: Duration,
    pub max_pause: Duration,
    pub freed_records: u64,
    pub freed_bytes: u64,
    pub live_bytes: usize,
}

/// The trigger's threshold and what growth steps may still add to reserved
/// memory before the budget is reached. Every arena and the value heap charge
/// their growth steps here; a step past the budget zeroes the threshold, so
/// the next safepoint collects.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Room {
    /// Bytes of growth steps left below the budget.
    left: isize,
    /// Nominal bytes that may be allocated before the next collection.
    threshold: usize,
}

impl Room {
    /// No budget and no trigger (`GcPolicy::Manual`, the arenas' tests).
    pub const fn unbounded() -> Room {
        Room {
            left: isize::MAX,
            threshold: usize::MAX,
        }
    }

    /// `left` bytes of growth below the budget and `threshold` nominal bytes
    /// before the next collection.
    pub fn new(left: usize, threshold: usize) -> Room {
        Room {
            left: left.min(isize::MAX as usize) as isize,
            threshold,
        }
    }

    /// Charges a growth step of `bytes`.
    #[inline]
    pub fn take(&mut self, bytes: usize) {
        self.left = self
            .left
            .saturating_sub(bytes.min(isize::MAX as usize) as isize);
        if self.left < 0 {
            self.threshold = 0;
        }
    }

    #[inline]
    pub fn threshold(&self) -> usize {
        self.threshold
    }
}

/// The growth step the budget keeps room for above reserved memory: a
/// quarter of the heap, at least 64 KiB and at most 2 MiB.
pub fn budget_step(reserved: usize) -> usize {
    (reserved / 4).clamp(64 << 10, 2 << 20)
}

pub struct GcState {
    /// Nominal bytes allocated since the last collection.
    pub(crate) allocated: usize,
    pub(crate) room: Room,
    /// Reserved bytes the heap is kept at or below between collections:
    /// `max(min_heap, (1 + growth_factor) × live)` after the last one, raised
    /// to one growth step above reserved memory when that is more.
    pub(crate) budget: usize,
    pub min_heap: usize,
    pub growth_factor: f64,
    pub limits: HeapLimits,
    /// No growth trigger: collections only at the cap and when the embedder
    /// asks (`GcPolicy::Manual` of the API).
    pub(crate) manual: bool,
    pub stats: GcStats,
    /// When set, every pause is appended to `pauses`.
    pub record_pauses: bool,
    pub pauses: Vec<Duration>,
    mark_stack: Vec<Value>,
    shape_stack: Vec<ShapeId>,
    /// Internal records with weak references reached by this collection.
    weak: Vec<u32>,
    /// The values of weak map entries whose key is not yet marked, by key:
    /// marking the key pushes them (the ephemeron fixpoint in one pass).
    ephemerons: FxHashMap<Value, Vec<Value>>,
    bytecode_stack: Vec<u32>,
    #[cfg(feature = "gc-stress")]
    pub stress: bool,
    /// Collect before every N-th allocating operation.
    #[cfg(feature = "gc-stress")]
    pub stress_interval: u32,
    #[cfg(feature = "gc-stress")]
    stress_counter: u32,
}

pub const DEFAULT_MIN_HEAP: usize = 1 << 20;

impl GcState {
    pub fn new(limits: HeapLimits) -> Self {
        GcState {
            allocated: 0,
            room: Room::new(DEFAULT_MIN_HEAP, DEFAULT_MIN_HEAP),
            budget: DEFAULT_MIN_HEAP,
            min_heap: DEFAULT_MIN_HEAP,
            growth_factor: 2.0,
            limits,
            manual: false,
            stats: GcStats::default(),
            record_pauses: false,
            pauses: Vec::new(),
            mark_stack: Vec::new(),
            weak: Vec::new(),
            ephemerons: FxHashMap::default(),
            shape_stack: Vec::new(),
            bytecode_stack: Vec::new(),
            #[cfg(feature = "gc-stress")]
            stress: true,
            #[cfg(feature = "gc-stress")]
            stress_interval: 1,
            #[cfg(feature = "gc-stress")]
            stress_counter: 0,
        }
    }

    /// The trigger of a heap that has not collected yet and reserves
    /// `reserved` bytes: `min_heap` bytes of budget, `growth_factor` for the
    /// budgets after collections.
    pub fn configure(&mut self, min_heap: usize, growth_factor: f64, reserved: usize) {
        self.min_heap = min_heap;
        self.growth_factor = growth_factor;
        self.budget = min_heap;
        self.room = Room::new(min_heap.saturating_sub(reserved), min_heap);
    }

    /// Nominal bytes that may be allocated before the next collection.
    pub fn threshold(&self) -> usize {
        self.room.threshold
    }

    /// The reserved-bytes budget set by the last collection.
    pub fn budget(&self) -> usize {
        self.budget
    }

    pub fn is_manual(&self) -> bool {
        self.manual
    }

    /// Whether every safepoint takes the slow path: under a cap (to check it)
    /// or under stress.
    #[inline]
    fn always_slow(&self) -> bool {
        #[cfg(feature = "gc-stress")]
        if self.stress {
            return true;
        }
        self.limits.max_bytes != usize::MAX
    }

    #[cfg(feature = "gc-stress")]
    fn stress_tick(&mut self) -> bool {
        if !self.stress {
            return false;
        }
        self.stress_counter += 1;
        if self.stress_counter >= self.stress_interval.max(1) {
            self.stress_counter = 0;
            true
        } else {
            false
        }
    }
}

impl Heap {
    /// The allocation safepoint. Collects when the trigger fires (or always
    /// under stress), and enforces the heap cap: an allocation that would cross
    /// it collects first and fails with `OutOfMemory` if that does not free
    /// enough. `keep` are values rooted for the duration of the collection.
    #[inline]
    pub(crate) fn safepoint(&mut self, need: &Need, keep: &[Value]) -> Result<(), HeapError> {
        let nominal = need.nominal_bytes();
        if self.gc.allocated + nominal < self.gc.room.threshold && !self.gc.always_slow() {
            self.gc.allocated += nominal;
            return Ok(());
        }
        self.safepoint_slow(need, keep)
    }

    #[cold]
    #[inline(never)]
    fn safepoint_slow(&mut self, need: &Need, keep: &[Value]) -> Result<(), HeapError> {
        let nominal = need.nominal_bytes();
        #[allow(unused_mut)]
        let mut collect = self.gc.allocated + nominal >= self.gc.room.threshold;
        #[cfg(feature = "gc-stress")]
        {
            collect |= self.gc.stress_tick();
        }
        let capped = self.gc.limits.max_bytes != usize::MAX;
        let over = |heap: &Heap| {
            heap.bytes() + heap.growth_bytes(need) + need.extra > heap.gc.limits.max_bytes
        };
        let over_cap = capped && over(self);
        if collect || over_cap {
            let mark = self.roots.stack.mark();
            for &value in keep {
                self.roots.stack.push(value);
            }
            self.collect();
            self.roots.stack.truncate(mark);
            if capped && over(self) {
                return Err(HeapError::OutOfMemory);
            }
        }
        self.gc.allocated += nominal;
        Ok(())
    }

    /// Bytes the arenas and the value heap grow by to satisfy `need`.
    pub(crate) fn growth_bytes(&self, need: &Need) -> usize {
        // Two ranges are approximated as one of the combined size.
        let values = self.values.growth_bytes(need.values[0] + need.values[1]);
        self.objects.growth_bytes(need.objects)
            + self.shapes.growth_bytes(need.shapes)
            + self.strings.growth_bytes(need.strings)
            + self.symbols.growth_bytes(need.symbols)
            + self.bigints.growth_bytes(need.bigints)
            + self.cells.growth_bytes(need.cells)
            + self.payloads.functions.growth_bytes(need.functions)
            + self.payloads.bytecode.growth_bytes(need.bytecodes)
            + self.payloads.internals.growth_bytes(need.internals)
            + values
    }

    /// A full collection.
    pub fn collect(&mut self) {
        let start = crate::clock::Stopwatch::start();
        let before = self.used_bytes();
        self.mark_from_roots();
        self.process_weak();
        self.prune_caches();
        let freed = self.sweep();
        let live = self.used_bytes();
        self.set_budget(live);
        let gc = &mut self.gc;
        let pause = start.elapsed();
        let stats = &mut gc.stats;
        stats.collections += 1;
        stats.total_pause += pause;
        stats.max_pause = stats.max_pause.max(pause);
        stats.freed_records += freed as u64;
        stats.freed_bytes += before.saturating_sub(live) as u64;
        stats.live_bytes = live;
        if gc.record_pauses {
            gc.pauses.push(pause);
        }
        #[cfg(feature = "gc-stress")]
        self.verify();
    }

    /// Turns the growth trigger off (`manual`) or back on, with the budget
    /// of the live bytes the last collection left.
    pub fn set_gc_manual(&mut self, manual: bool) {
        self.gc.manual = manual;
        let allocated = self.gc.allocated;
        self.set_budget(self.gc.stats.live_bytes);
        self.gc.allocated = allocated;
    }

    /// The budget and the trigger after a collection that left `live` bytes.
    /// The threshold is what can be allocated before the bytes in use reach
    /// the budget; while the free space matches the requests, that is also
    /// when reserved memory reaches it. A growth step past the budget
    /// collects early.
    fn set_budget(&mut self, live: usize) {
        let reserved = self.bytes();
        let gc = &mut self.gc;
        gc.allocated = 0;
        let step = budget_step(reserved);
        let mut budget = gc
            .min_heap
            .max((live as f64 * (1.0 + gc.growth_factor)) as usize);
        // The cap clamps the room for growth steps, not the threshold: a live
        // set within a step of the cap would otherwise collect at every
        // safepoint, where the cap check already collects when it must.
        let threshold = budget - live.min(budget);
        if gc.limits.max_bytes != usize::MAX {
            budget = budget.min(gc.limits.max_bytes.saturating_sub(step));
        }
        // Free space of the wrong shape, or a live set that grew: the heap
        // takes one more step before the next collection.
        gc.budget = budget.max(reserved + step);
        gc.room = if gc.manual {
            Room::unbounded()
        } else {
            Room::new(gc.budget - reserved, threshold)
        };
    }

    fn clear_marks(&mut self) {
        self.objects.clear_marks();
        self.shapes.clear_marks();
        self.strings.clear_marks();
        self.symbols.clear_marks();
        self.bigints.clear_marks();
        self.cells.clear_marks();
        self.payloads.functions.clear_marks();
        self.payloads.bytecode.clear_marks();
        self.payloads.internals.clear_marks();
        self.code.clear_marks();
        self.regexps.clear_marks();
    }

    /// Marks everything reachable from the root set. A reference to a freed
    /// record panics here in debug and `gc-stress` builds.
    fn mark_from_roots(&mut self) {
        self.clear_marks();
        // verify() also marks, without consuming the weak work list. Only
        // weak objects reached by this traversal may contribute ephemerons.
        self.gc.weak.clear();
        let mut stack = mem::take(&mut self.gc.mark_stack);
        stack.extend_from_slice(self.roots.stack.values());
        stack.extend(self.roots.handles.values());
        stack.extend_from_slice(&self.roots.fixed);
        for job in &self.jobs {
            job.trace(&mut stack);
        }
        // Registers above `top` belong to returned frames and may name freed
        // records; a call clears its window before it can be marked again.
        let top = self.vm.top as usize;
        self.vm.stale_from = self.vm.stale_from.min(self.vm.top);
        push_heap_values(&mut stack, &self.vm.registers[..top]);
        stack.extend(
            self.vm
                .frames
                .iter()
                .map(|frame| Value::object(frame.function)),
        );
        for &id in self.pinned_atoms.iter().chain(&self.held_atoms) {
            self.strings.mark(id.0);
        }
        // Units published but not yet instantiated.
        for position in 0..self.code.pinned().len() {
            let index = self.code.pinned()[position];
            let id = crate::bytecode::CodeUnitId(index | crate::bytecode::DYNAMIC);
            if let Some(unit) = self.code.mark(id) {
                stack.extend(unit.atoms.iter().map(|&atom| Value::string(atom)));
            }
        }
        let mut shapes = mem::take(&mut self.gc.shape_stack);
        shapes.extend(self.closure_shapes.iter().flatten());
        shapes.extend(self.pending_closure_shapes.iter().flatten());
        shapes.extend(self.arguments_shapes.iter().flatten());
        self.mark_shapes(&mut shapes, &mut stack);
        self.gc.shape_stack = shapes;
        self.cells.mark(STATIC_CELL.0);
        stack.extend_from_slice(&self.kept);
        stack.extend_from_slice(&self.rejections);
        stack.extend_from_slice(&self.modules);
        self.drain::<false>(&mut stack);
        // Ephemerons: a weak map's value is live once its key is. Each weak
        // map is scanned once: a live key's value is pushed, an unmarked
        // key's waits in `ephemerons` until `drain` marks the key. Draining
        // may reach further weak maps, scanned in the next round.
        let mut scanned = 0;
        loop {
            let mut ephemerons = mem::take(&mut self.gc.ephemerons);
            for position in scanned..self.gc.weak.len() {
                let internal = self.payloads.internals.get(self.gc.weak[position]);
                if internal.tag != InternalTag::WeakMap {
                    continue;
                }
                for entry in internal.slots.chunks(2) {
                    if entry[0].is_empty() || self.value_marked(entry[1]) {
                        continue;
                    }
                    if self.value_marked(entry[0]) {
                        stack.push(entry[1]);
                    } else {
                        ephemerons.entry(entry[0]).or_default().push(entry[1]);
                    }
                }
            }
            scanned = self.gc.weak.len();
            self.gc.ephemerons = ephemerons;
            if stack.is_empty() {
                break;
            }
            self.drain::<true>(&mut stack);
        }
        self.gc.ephemerons.clear();
        // Cells owned by live prototypes; the table itself is weak.
        for (object, info) in &self.prototypes {
            if self.objects.is_marked(object.0)
                && let Some(cell) = info.cell
            {
                self.cells.mark(cell.0);
            }
        }
        stack.clear();
        self.gc.mark_stack = stack;
    }

    /// Marks everything reachable from `stack`. With `EPHEMERONS`, marking
    /// a key with waiting weak map values pushes them; the first pass has
    /// none waiting and skips the check.
    fn drain<const EPHEMERONS: bool>(&mut self, stack: &mut Vec<Value>) {
        let mut shapes = mem::take(&mut self.gc.shape_stack);
        while let Some(value) = stack.pop() {
            match value.kind() {
                Kind::Object => {
                    let index = value.as_object().unwrap().0;
                    if !self.objects.mark(index) {
                        continue;
                    }
                    if EPHEMERONS
                        && !self.gc.ephemerons.is_empty()
                        && let Some(values) = self.gc.ephemerons.remove(&value)
                    {
                        stack.extend(values);
                    }
                    let header = *self.objects.get(index);
                    // Most objects share a shape already marked.
                    if !self.shapes.is_marked(header.shape.0) {
                        shapes.push(header.shape);
                        self.mark_shapes(&mut shapes, stack);
                    }
                    push_heap_values(stack, self.values.slice(header.slots()));
                    push_heap_values(stack, self.values.slice(header.elements()));
                    match header.kind {
                        ObjectKind::Scope | ObjectKind::Arguments if header.payload != NONE => {
                            stack.push(Value::object(crate::ObjectId(header.payload)));
                        }
                        kind if kind.has_internal() => {
                            if self.payloads.internals.mark(header.payload) {
                                let internal = self.payloads.internals.get(header.payload);
                                match internal.tag {
                                    // Ephemerons and weak targets.
                                    InternalTag::WeakMap
                                    | InternalTag::WeakSet
                                    | InternalTag::WeakRef => {
                                        self.gc.weak.push(header.payload);
                                    }
                                    InternalTag::FinalizationRegistry => {
                                        // The callback and each cell's held
                                        // value are strong.
                                        stack.push(internal.slots[0]);
                                        for cell in internal.slots[1..].chunks(3) {
                                            stack.push(cell[1]);
                                        }
                                        self.gc.weak.push(header.payload);
                                    }
                                    _ => push_heap_values(stack, &internal.slots),
                                }
                                // A module namespace's export keys.
                                if let super::object::InternalData::Exports(keys) = &internal.data {
                                    stack.extend(keys.iter().filter_map(|key| key.heap_value()));
                                }
                            }
                        }
                        ObjectKind::Function => {
                            if self.payloads.functions.mark(header.payload) {
                                let record = *self.payloads.functions.get(header.payload);
                                for link in [record.scope, record.home_object, record.fields] {
                                    if link != NONE {
                                        stack.push(Value::object(crate::ObjectId(link)));
                                    }
                                }
                                if record.bytecode != NONE {
                                    self.mark_bytecode(record.bytecode, stack);
                                }
                            }
                        }
                        ObjectKind::RegExp if header.payload != NONE => {
                            if let Some(record) = self.regexps.mark(header.payload) {
                                stack.push(Value::string(record.source));
                                stack.push(Value::string(record.flags));
                            }
                        }
                        _ => {}
                    }
                }
                Kind::String => {
                    let index = value.as_string().unwrap().0;
                    if self.strings.mark(index)
                        && !self.ropes.is_empty()
                        && self.strings.get(index).flat().is_none()
                        && let Some(&(left, right)) = self.ropes.get(&index)
                    {
                        stack.push(Value::string(left));
                        stack.push(Value::string(right));
                    }
                }
                Kind::Symbol => {
                    let index = value.as_symbol().unwrap().0;
                    if self.symbols.mark(index) {
                        if EPHEMERONS
                            && !self.gc.ephemerons.is_empty()
                            && let Some(values) = self.gc.ephemerons.remove(&value)
                        {
                            stack.extend(values);
                        }
                        let description = self.symbols.get(index).description;
                        if description != NONE {
                            self.strings.mark(description);
                        }
                    }
                }
                Kind::BigInt => {
                    self.bigints.mark(value.as_bigint().unwrap().0);
                }
                _ => {}
            }
        }
        self.gc.shape_stack = shapes;
    }

    /// Marks a bytecode record and the records of its children; pushes their
    /// constants.
    fn mark_bytecode(&mut self, root: u32, stack: &mut Vec<Value>) {
        let mut pending = mem::take(&mut self.gc.bytecode_stack);
        pending.push(root);
        while let Some(index) = pending.pop() {
            if !self.payloads.bytecode.mark(index) {
                continue;
            }
            let record = *self.payloads.bytecode.get(index);
            let unit = crate::bytecode::CodeUnitId(record.unit);
            if unit.is_dynamic()
                && let Some(unit) = self.code.mark(unit)
            {
                stack.extend(unit.atoms.iter().map(|&atom| Value::string(atom)));
            }
            push_heap_values(stack, self.values.slice(record.constants));
            let children = &self.values.slice(record.children)[..record.child_count as usize];
            pending.extend(
                children
                    .iter()
                    .map(|child| child.as_int32().expect("child link") as u32),
            );
        }
        self.gc.bytecode_stack = pending;
    }

    /// Marks shapes and their parent chains; pushes the keys and prototypes
    /// they hold.
    fn mark_shapes(&mut self, shapes: &mut Vec<ShapeId>, stack: &mut Vec<Value>) {
        while let Some(mut id) = shapes.pop() {
            loop {
                if !self.shapes.mark(id.0) {
                    break;
                }
                let shape = self.shapes.get(id.0);
                if shape.count > 0
                    && let Some(key) = shape.key.heap_value()
                {
                    stack.push(key);
                }
                if let Some(prototype) = shape.prototype {
                    stack.push(Value::object(prototype));
                }
                if shape.validity.0 != NONE {
                    self.cells.mark(shape.validity.0);
                }
                if let Some(dictionary) = &shape.dictionary {
                    stack.extend(
                        dictionary
                            .entries()
                            .filter_map(|entry| entry.key.heap_value()),
                    );
                }
                match shape.parent {
                    Some(parent) => id = parent,
                    None => break,
                }
            }
        }
    }

    /// The weak-table hook, run after marking: shape roots, transitions and
    /// prototype infos drop entries whose target died.
    /// Whether `value` survives this collection (a primitive always does).
    fn value_marked(&self, value: Value) -> bool {
        match value.kind() {
            Kind::Object => self.objects.is_marked(value.as_object().unwrap().0),
            Kind::Symbol => self.symbols.is_marked(value.as_symbol().unwrap().0),
            Kind::String => self.strings.is_marked(value.as_string().unwrap().0),
            Kind::BigInt => self.bigints.is_marked(value.as_bigint().unwrap().0),
            _ => true,
        }
    }

    /// Clears the weak references to records about to be swept: dead keys
    /// leave weak collections, dead targets leave `WeakRef`s, and a dead
    /// registry target queues its cleanup job.
    fn clear_weak(&mut self) {
        let weak = mem::take(&mut self.gc.weak);
        for &payload in &weak {
            let tag = self.payloads.internals.get(payload).tag;
            match tag {
                InternalTag::WeakMap | InternalTag::WeakSet => {
                    let internal = self.payloads.internals.get(payload);
                    let width = if tag == InternalTag::WeakMap { 2 } else { 1 };
                    let dead: Vec<usize> = internal
                        .slots
                        .chunks(width)
                        .enumerate()
                        .filter(|(_, entry)| !entry[0].is_empty() && !self.value_marked(entry[0]))
                        .map(|(index, _)| index)
                        .collect();
                    let internal = self.payloads.internals.get_mut(payload);
                    for entry in dead {
                        super::table::tombstone(internal, entry);
                    }
                }
                InternalTag::WeakRef => {
                    let target = self.payloads.internals.get(payload).slots[0];
                    if !self.value_marked(target) {
                        self.payloads.internals.get_mut(payload).slots[0] = Value::UNDEFINED;
                    }
                }
                InternalTag::FinalizationRegistry => {
                    let internal = self.payloads.internals.get(payload);
                    let callback = internal.slots[0];
                    let mut jobs = Vec::new();
                    let mut dead_targets = Vec::new();
                    let mut dead_tokens = Vec::new();
                    for (index, cell) in internal.slots[1..].chunks(3).enumerate() {
                        if cell[0].is_empty() {
                            continue;
                        }
                        if !self.value_marked(cell[0]) {
                            dead_targets.push(index);
                            jobs.push(crate::vm::jobs::Job::Call {
                                function: callback,
                                this: Value::UNDEFINED,
                                args: vec![cell[1]],
                            });
                        } else if !cell[2].is_empty() && !self.value_marked(cell[2]) {
                            dead_tokens.push(index);
                        }
                    }
                    let internal = self.payloads.internals.get_mut(payload);
                    for index in dead_targets {
                        for slot in &mut internal.slots[1 + index * 3..4 + index * 3] {
                            *slot = Value::EMPTY;
                        }
                    }
                    for index in dead_tokens {
                        internal.slots[3 + index * 3] = Value::EMPTY;
                    }
                    self.jobs.extend(jobs);
                }
                _ => {}
            }
        }
        self.gc.weak = weak;
        self.gc.weak.clear();
    }

    fn process_weak(&mut self) {
        self.clear_weak();
        // A rope's halves are dropped once it dies or flattens.
        let strings = &self.strings;
        self.ropes
            .retain(|&index, _| strings.is_marked(index) && strings.get(index).flat().is_none());
        let objects = &self.objects;
        self.holey.retain(|&index| objects.is_marked(index));
        if let Some((error, _)) = &self.vm.unpositioned
            && !objects.is_marked(error.0)
        {
            self.vm.unpositioned = None;
        }
        self.prototypes.retain(|object, info| {
            if !objects.is_marked(object.0) {
                return false;
            }
            info.dependents
                .retain(|dependent| objects.is_marked(dependent.0));
            true
        });
        self.last_root = None;
        let shapes = &mut self.shapes;
        self.shape_roots
            .retain(|_, shape| shapes.is_marked(shape.0));
        let live: Vec<u32> = shapes
            .live_indices()
            .filter(|&index| shapes.is_marked(index))
            .collect();
        for index in live {
            if shapes.get(index).transitions.is_empty() {
                continue;
            }
            let mut transitions = mem::take(&mut shapes.get_mut(index).transitions);
            transitions.retain(|child| shapes.is_marked(child.0));
            shapes.get_mut(index).transitions = transitions;
        }
    }

    /// Clears the cache entries that name a record about to be swept, before
    /// its index can be recycled.
    fn prune_caches(&mut self) {
        let (objects, shapes, cells, strings) =
            (&self.objects, &self.shapes, &self.cells, &self.strings);
        self.ics.prune(|record| match record {
            Record::Shape(index) => shapes.is_marked(index),
            Record::Object(index) => objects.is_marked(index),
            Record::Cell(index) => cells.is_marked(index),
            Record::Atom(index) => strings.is_marked(index),
        });
    }

    fn sweep(&mut self) -> usize {
        let values = &mut self.values;
        values.begin_sweep();
        let mut freed = self.objects.sweep(|_, header| {
            values.release(header.slots());
            values.release(header.elements());
        });
        freed += self.payloads.bytecode.sweep(|_, bytecode| {
            values.release(bytecode.constants);
            values.release(bytecode.children);
        });
        values.finish_sweep();
        freed += self.payloads.functions.sweep(|_, _| {});
        freed += self.payloads.internals.sweep(|_, _| {});
        freed += self.shapes.sweep(|_, _| {});
        let ics = &mut self.ics;
        self.code.sweep(|unit| ics.release(unit));
        self.regexps.sweep();
        let (rope_bytes, atoms) = (&self.rope_bytes, &mut self.atoms);
        freed += self.strings.sweep(|index, record| {
            rope_bytes.set(rope_bytes.get() - record.flattened_rope_bytes());
            if record.is_atom {
                atoms.remove(record.repr().content_hash(), crate::value::StringId(index));
            }
        });
        freed += self.symbols.sweep(|_, _| {});
        freed += self.bigints.sweep(|_, _| {});
        freed += self.cells.sweep(|_, _| {});
        freed
    }

    /// Post-collection verification: every live record is reachable and every
    /// reachable record is live; no free slot is marked and every arena's free
    /// count agrees with its bitmap; every value-heap slot has exactly one
    /// owner, a live range or the free space.
    pub fn verify(&mut self) {
        self.mark_from_roots();
        fn check<T: super::arena::Record>(name: &str, arena: &super::arena::Arena<T>) {
            if let Some(index) = arena.marks_equal_live() {
                panic!(
                    "gc verify: {name} {index} is {}",
                    if arena.is_live(index) {
                        "live but unreachable"
                    } else {
                        "reachable but freed"
                    }
                );
            }
            for index in arena.free_indices() {
                assert!(
                    !arena.is_marked(index),
                    "gc verify: free {name} {index} is marked"
                );
            }
            assert!(
                arena.free_count_matches(),
                "gc verify: {name}'s free count or cursor disagrees with its bitmap"
            );
        }
        check("object", &self.objects);
        check("shape", &self.shapes);
        check("string", &self.strings);
        check("symbol", &self.symbols);
        check("bigint", &self.bigints);
        check("cell", &self.cells);
        check("function", &self.payloads.functions);
        check("bytecode", &self.payloads.bytecode);
        check("internal", &self.payloads.internals);
        self.code.verify();
        self.regexps.verify();
        self.ics.verify_free();
        for index in self.payloads.bytecode.live_indices() {
            let unit = crate::bytecode::CodeUnitId(self.payloads.bytecode.get(index).unit);
            assert!(
                !unit.is_dynamic() || self.code.lookup(unit).is_some(),
                "gc verify: bytecode {index} names a freed dynamic unit"
            );
        }

        let mut owner = vec![false; self.values.total_slots()];
        let mut claim = |range: super::values::Range, what: &str| {
            for slot in range.start..range.start + range.len {
                assert!(
                    !owner[slot as usize],
                    "gc verify: value slot {slot} has two owners ({what})"
                );
                owner[slot as usize] = true;
            }
        };
        for index in self.objects.live_indices() {
            let header = self.objects.get(index);
            claim(header.slots(), "object slots");
            claim(header.elements(), "object elements");
        }
        for index in self.payloads.bytecode.live_indices() {
            let record = self.payloads.bytecode.get(index);
            claim(record.constants, "bytecode constants");
            claim(record.children, "bytecode children");
        }
        for block in self.values.free_blocks() {
            claim(block, "free block");
        }
        for (start, end) in self.values.free_runs() {
            claim(
                super::values::Range {
                    start,
                    len: end - start,
                },
                "free run",
            );
        }
        // After a collection's rebuild every slot is owned or free.
        if let Some(slot) = owner.iter().position(|&owned| !owned) {
            panic!("gc verify: value slot {slot} is neither owned nor free");
        }

        // Every filled cache entry names live records.
        for record in self.ics.named_records() {
            let live = match record {
                Record::Shape(index) => self.shapes.is_live(index),
                Record::Object(index) => self.objects.is_live(index),
                Record::Cell(index) => self.cells.is_live(index),
                Record::Atom(index) => self.strings.is_live(index),
            };
            assert!(live, "gc verify: a cache entry names freed {record:?}");
        }
        self.clear_marks();
    }
}

#[inline]
fn push_heap_values(stack: &mut Vec<Value>, values: &[Value]) {
    stack.extend(values.iter().copied().filter(|value| value.is_heap()));
}
