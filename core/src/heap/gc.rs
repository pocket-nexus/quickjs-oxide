//! Root set, mark-sweep, trigger policy, stress mode and the heap cap.
//!
//! Non-moving mark-sweep. Mark bits live in each arena's side bitmap; marking
//! uses an explicit stack, never recursion. Sweep is eager and scans every
//! arena. Collection happens only at an allocation safepoint
//! ([`Heap::safepoint`]) or on an explicit [`Heap::collect`].

use std::mem;
use std::time::{Duration, Instant};

use super::object::{NONE, ObjectKind};
use super::{Heap, HeapError, Need};
use crate::shape::STATIC_CELL;
use crate::value::{Kind, ShapeId, Value};

/// A position on the [`RootStack`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RootMark(usize);

/// Values Rust code holds across an allocation. The only legal way to keep an
/// index alive across a safepoint in phase 1.
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

#[derive(Clone, Copy, Debug)]
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

#[derive(Clone, Copy, Debug, Default)]
pub struct GcStats {
    pub collections: u64,
    pub total_pause: Duration,
    pub max_pause: Duration,
    pub freed_records: u64,
    pub freed_bytes: u64,
    pub live_bytes: usize,
}

pub struct GcState {
    /// Nominal bytes allocated since the last collection.
    pub(crate) allocated: usize,
    pub(crate) threshold: usize,
    pub min_heap: usize,
    pub growth_factor: f64,
    pub limits: HeapLimits,
    pub stats: GcStats,
    /// When set, every pause is appended to `pauses`.
    pub record_pauses: bool,
    pub pauses: Vec<Duration>,
    mark_stack: Vec<Value>,
    shape_stack: Vec<ShapeId>,
    #[cfg(feature = "gc-stress")]
    pub stress: bool,
    /// Collect before every N-th allocating operation.
    #[cfg(feature = "gc-stress")]
    pub stress_interval: u32,
    #[cfg(feature = "gc-stress")]
    stress_counter: u32,
}

pub const DEFAULT_MIN_HEAP: usize = 4 << 20;

impl GcState {
    pub fn new(limits: HeapLimits) -> Self {
        GcState {
            allocated: 0,
            threshold: DEFAULT_MIN_HEAP,
            min_heap: DEFAULT_MIN_HEAP,
            growth_factor: 2.0,
            limits,
            stats: GcStats::default(),
            record_pauses: false,
            pauses: Vec::new(),
            mark_stack: Vec::new(),
            shape_stack: Vec::new(),
            #[cfg(feature = "gc-stress")]
            stress: true,
            #[cfg(feature = "gc-stress")]
            stress_interval: 1,
            #[cfg(feature = "gc-stress")]
            stress_counter: 0,
        }
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
        if self.gc.allocated + nominal < self.gc.threshold && !self.gc.always_slow() {
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
        let mut collect = self.gc.allocated + nominal >= self.gc.threshold;
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
            + values
    }

    /// A full collection.
    pub fn collect(&mut self) {
        let start = Instant::now();
        let before = self.used_bytes();
        self.mark_from_roots();
        self.process_weak();
        let freed = self.sweep();
        let live = self.used_bytes();
        let gc = &mut self.gc;
        gc.allocated = 0;
        gc.threshold = gc.min_heap.max((live as f64 * gc.growth_factor) as usize);
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

    fn clear_marks(&mut self) {
        self.objects.clear_marks();
        self.shapes.clear_marks();
        self.strings.clear_marks();
        self.symbols.clear_marks();
        self.bigints.clear_marks();
        self.cells.clear_marks();
        self.payloads.functions.clear_marks();
        self.payloads.bytecode.clear_marks();
    }

    /// Marks everything reachable from the root set. A reference to a freed
    /// record panics here in debug and `gc-stress` builds.
    fn mark_from_roots(&mut self) {
        self.clear_marks();
        let mut stack = mem::take(&mut self.gc.mark_stack);
        stack.extend_from_slice(self.roots.stack.values());
        stack.extend(self.roots.handles.values());
        stack.extend_from_slice(&self.roots.fixed);
        for id in self.atoms.ids() {
            self.strings.mark(id.0);
        }
        self.cells.mark(STATIC_CELL.0);
        self.drain(&mut stack);
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

    fn drain(&mut self, stack: &mut Vec<Value>) {
        let mut shapes = mem::take(&mut self.gc.shape_stack);
        while let Some(value) = stack.pop() {
            match value.kind() {
                Kind::Object => {
                    let index = value.as_object().unwrap().0;
                    if !self.objects.mark(index) {
                        continue;
                    }
                    let header = *self.objects.get(index);
                    shapes.push(header.shape);
                    self.mark_shapes(&mut shapes, stack);
                    push_heap_values(stack, self.values.slice(header.slots()));
                    push_heap_values(stack, self.values.slice(header.elements()));
                    match header.kind {
                        ObjectKind::Scope if header.payload != NONE => {
                            stack.push(Value::object(crate::ObjectId(header.payload)));
                        }
                        ObjectKind::Function => {
                            if self.payloads.functions.mark(header.payload) {
                                let record = *self.payloads.functions.get(header.payload);
                                if record.scope != NONE {
                                    stack.push(Value::object(crate::ObjectId(record.scope)));
                                }
                                if record.bytecode != NONE
                                    && self.payloads.bytecode.mark(record.bytecode)
                                {
                                    let constants =
                                        self.payloads.bytecode.get(record.bytecode).constants;
                                    push_heap_values(stack, self.values.slice(constants));
                                }
                            }
                        }
                        _ => {}
                    }
                }
                Kind::String => {
                    self.strings.mark(value.as_string().unwrap().0);
                }
                Kind::Symbol => {
                    let index = value.as_symbol().unwrap().0;
                    if self.symbols.mark(index) {
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
    fn process_weak(&mut self) {
        let objects = &self.objects;
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

    fn sweep(&mut self) -> usize {
        let values = &mut self.values;
        let mut freed = self.objects.sweep(|_, header| {
            values.free(header.slots());
            values.free(header.elements());
        });
        freed += self
            .payloads
            .bytecode
            .sweep(|_, bytecode| values.free(bytecode.constants));
        freed += self.payloads.functions.sweep(|_, _| {});
        freed += self.shapes.sweep(|_, _| {});
        freed += self.strings.sweep(|_, _| {});
        freed += self.symbols.sweep(|_, _| {});
        freed += self.bigints.sweep(|_, _| {});
        freed += self.cells.sweep(|_, _| {});
        freed
    }

    /// Post-collection verification (phase 0, section 3): every live record is
    /// reachable and every reachable record is live; no free-list entry is
    /// live or marked; every value-heap range has exactly one owner.
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
                    !arena.is_live(index) && !arena.is_marked(index),
                    "gc verify: free {name} {index} is live or marked"
                );
            }
        }
        check("object", &self.objects);
        check("shape", &self.shapes);
        check("string", &self.strings);
        check("symbol", &self.symbols);
        check("bigint", &self.bigints);
        check("cell", &self.cells);
        check("function", &self.payloads.functions);
        check("bytecode", &self.payloads.bytecode);

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
            claim(
                self.payloads.bytecode.get(index).constants,
                "bytecode constants",
            );
        }
        for block in self.values.free_blocks() {
            claim(block, "free block");
        }
        self.clear_marks();
    }
}

#[inline]
fn push_heap_values(stack: &mut Vec<Value>, values: &[Value]) {
    stack.extend(values.iter().copied().filter(|value| value.is_heap()));
}
