//! The heap: typed arenas, the value heap, the collector and the root set.
//!
//! Every allocating operation takes `&mut Heap` and passes exactly one
//! safepoint before it allocates, rooting its own arguments for that
//! collection. A caller that holds an index in a Rust local across a second
//! allocating call must root it on [`RootStack`](gc::RootStack) first; see
//! `core/README.md`.

pub mod arena;
pub mod atom;
pub mod bigint;
pub mod buffer;
pub mod code;
pub mod gc;
pub(crate) mod hash;
pub mod object;
pub mod report;
pub mod string;
pub mod symbol;
pub mod table;
pub mod values;

use std::fmt;

use self::arena::Arena;
use self::atom::{AtomTable, KEY_INDEX_LIMIT, PropertyKey};
use self::bigint::BigIntRecord;
use self::gc::{GcState, Handles, HeapLimits, RootStack, Roots};
use self::hash::FxHashMap;
use self::object::{
    FunctionBytecode, FunctionRecord, Internal, InternalData, InternalTag, NONE, ObjectHeader,
    ObjectKind, PayloadArenas, flags,
};
use self::string::{AtomKeep, StringRecord, StringRepr};
use self::symbol::SymbolRecord;
use self::values::{Range, ValueHeap};
use crate::bytecode::CodeUnitId;
use crate::shape::{Dictionary, PrototypeInfo, STATIC_CELL, Shape, ValidityCell};
use crate::value::{BigIntId, BytecodeId, ObjectId, ScopeId, ShapeId, StringId, SymbolId, Value};
use crate::vm::VmStack;
use crate::vm::ic::IcStore;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeapError {
    /// The heap cap would be crossed even after a collection. The caller turns
    /// it into the JavaScript `InternalError`.
    OutOfMemory,
    /// `add_property` on a key the object already has.
    DuplicateKey,
    /// `set_prototype` would create a cycle.
    PrototypeCycle,
}

impl fmt::Display for HeapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            HeapError::OutOfMemory => "out of memory",
            HeapError::DuplicateKey => "duplicate property key",
            HeapError::PrototypeCycle => "cyclic prototype chain",
        })
    }
}

impl std::error::Error for HeapError {}

/// One unit of the tree [`Heap::alloc_bytecode_function`] creates.
#[derive(Clone, Copy, Debug)]
pub struct BytecodeTemplate<'a> {
    pub unit: CodeUnitId,
    pub constants: &'a [Value],
    /// Indices into the tree, in `MakeClosure` order.
    pub children: &'a [u32],
}

/// What one operation will allocate, for the trigger and the cap.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Need {
    pub objects: usize,
    pub shapes: usize,
    pub strings: usize,
    pub symbols: usize,
    pub bigints: usize,
    pub cells: usize,
    pub functions: usize,
    pub bytecodes: usize,
    pub internals: usize,
    /// Up to two value-heap ranges.
    pub values: [u32; 2],
    /// Bytes owned outside the arenas (string contents, limbs).
    pub extra: usize,
}

impl Need {
    /// Bytes charged to the collection trigger. Shapes and validity cells are
    /// reserved conservatively (an operation that might create one asks for
    /// one), so they count toward the cap through [`Heap::growth_bytes`] but
    /// not toward the trigger.
    pub fn nominal_bytes(&self) -> usize {
        self.objects * size_of::<ObjectHeader>()
            + self.strings * size_of::<StringRecord>()
            + self.symbols * size_of::<SymbolRecord>()
            + self.bigints * size_of::<BigIntRecord>()
            + self.functions * size_of::<FunctionRecord>()
            + self.bytecodes * size_of::<FunctionBytecode>()
            + self.internals * size_of::<Internal>()
            + (values::class_len(self.values[0]) + values::class_len(self.values[1])) as usize * 8
            + self.extra
    }
}

pub struct Heap {
    pub(crate) objects: Arena<ObjectHeader>,
    pub(crate) shapes: Arena<Shape>,
    pub(crate) strings: Arena<StringRecord>,
    /// `WeakRef` targets kept alive until the current job ends.
    pub(crate) kept: Vec<Value>,
    /// The rejected promises without a handler, in rejection order, while the
    /// embedder tracks rejections. Roots.
    pub(crate) rejections: Vec<Value>,
    /// Arrays that have had a hole: QuickJS prints them by their index
    /// properties even after the hole is gone. Weak.
    pub(crate) holey: crate::heap::hash::FxHashSet<u32>,
    /// Every module record: modules live as long as the runtime.
    pub(crate) modules: Vec<Value>,
    /// Bytes of flattened ropes (the arena counts a rope as empty).
    pub(crate) rope_bytes: std::cell::Cell<usize>,
    /// Each unflattened rope's two halves, by string index (kept out of
    /// [`StringRecord`], which every string pays for).
    pub(crate) ropes: FxHashMap<u32, (StringId, StringId)>,
    /// Whether `$262.IsHTMLDDA` exists, so ToBoolean of an object skips
    /// looking for it otherwise.
    pub(crate) html_dda: bool,
    pub(crate) symbols: Arena<SymbolRecord>,
    pub(crate) bigints: Arena<BigIntRecord>,
    pub(crate) cells: Arena<ValidityCell>,
    pub(crate) values: ValueHeap,
    pub(crate) payloads: PayloadArenas,
    pub(crate) atoms: AtomTable,
    /// What [`Heap::intern`] does with the atoms it returns.
    pub(crate) atom_mode: AtomMode,
    /// The atoms interned in [`AtomMode::Pin`]: roots for the heap's
    /// lifetime.
    pub(crate) pinned_atoms: Vec<StringId>,
    /// The atoms interned in [`AtomMode::Hold`]: roots until the
    /// compilation that interned them ends.
    pub(crate) held_atoms: Vec<StringId>,
    /// The code units compiled at run time.
    pub(crate) code: code::DynamicUnits,
    /// The regular-expression programs, by `RegExp` object payload.
    pub(crate) regexps: crate::vm::regexp::RegExpStore,
    /// Transition-tree roots by `(prototype index or NONE, kind)`. Weak.
    pub(crate) shape_roots: FxHashMap<(u32, ObjectKind), ShapeId>,
    /// One-entry cache in front of `shape_roots`; cleared by every collection.
    pub(crate) last_root: Option<((u32, ObjectKind), ShapeId)>,
    /// Weak in key and dependents.
    pub(crate) prototypes: FxHashMap<ObjectId, PrototypeInfo>,
    pub(crate) roots: Roots,
    pub(crate) gc: GcState,
    /// The register file and the frame table: fixed roots, so a safepoint marks
    /// the running frames with no parameter.
    pub(crate) vm: VmStack,
    /// The inline caches: beside the code store in spirit, inside the heap so
    /// that every collection prunes them.
    pub(crate) ics: IcStore,
    /// The global object's dictionary generation: bumped by every insert,
    /// remove and reflag of its properties, never by a value write. A global
    /// cache entry hits while it is unchanged.
    pub(crate) global_generation: u32,
    /// No object used as a prototype has an index property or is exotic: a
    /// write into an element hole cannot meet an inherited setter or read-only
    /// index, so the loop's dense write may fill it. Cleared for good the first
    /// time that stops holding.
    pub(crate) holes_writable: bool,
    /// The shapes `MakeClosure` gives a new function (`length`, `name`, and
    /// `prototype` for the third), per function kind: learned from the first
    /// closure of each kind and marked as roots.
    pub(crate) closure_shapes: [Option<ShapeId>; CLOSURE_SHAPES],
    /// The shapes `MakeClosure` gives a new function before its properties
    /// are read: the keys of `closure_shapes`, each with the `AUTOINIT` flag,
    /// over no slots. Learned and marked like `closure_shapes`.
    pub(crate) pending_closure_shapes: [Option<ShapeId>; CLOSURE_SHAPES],
    /// The shapes of a new `arguments` object (`length`, `@@iterator`,
    /// `callee`), sloppy then strict: learned from the first of each and
    /// marked as roots.
    pub(crate) arguments_shapes: [Option<ShapeId>; 2],
    /// The job queue: roots until each runs.
    pub(crate) jobs: std::collections::VecDeque<crate::vm::jobs::Job>,
}

/// How long an atom [`Heap::intern`] returns is kept. Atoms are otherwise
/// collected like any string: a property key, a shape, a dynamic unit or a
/// value keeps one alive.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AtomMode {
    /// For the heap's lifetime: the realm's names and the builtins'
    /// properties, interned while the runtime is created, and the keys of
    /// the units published before the program runs.
    Pin,
    /// Until the running compilation ends: the keys, names and string
    /// constants of code compiled at run time, before its units are
    /// published and instantiated.
    Hold,
    /// No longer than its holders.
    Weak,
}

/// The function kinds with a cached closure shape: plain, with a
/// `prototype`, async, generator, async generator.
pub(crate) const CLOSURE_SHAPES: usize = 5;

impl Default for Heap {
    fn default() -> Self {
        Self::new(HeapLimits::default())
    }
}

impl Heap {
    pub fn new(limits: HeapLimits) -> Self {
        let mut heap = Heap {
            objects: Arena::new(),
            shapes: Arena::new(),
            strings: Arena::new(),
            kept: Vec::new(),
            rejections: Vec::new(),
            holey: Default::default(),
            modules: Vec::new(),
            rope_bytes: std::cell::Cell::new(0),
            ropes: FxHashMap::default(),
            html_dda: false,
            symbols: Arena::new(),
            bigints: Arena::new(),
            cells: Arena::new(),
            values: ValueHeap::new(),
            payloads: PayloadArenas::default(),
            atoms: AtomTable::default(),
            atom_mode: AtomMode::Pin,
            pinned_atoms: Vec::new(),
            held_atoms: Vec::new(),
            code: code::DynamicUnits::default(),
            regexps: Default::default(),
            shape_roots: FxHashMap::default(),
            last_root: None,
            prototypes: FxHashMap::default(),
            roots: Roots::default(),
            gc: GcState::new(limits),
            vm: VmStack::default(),
            ics: IcStore::default(),
            global_generation: 0,
            holes_writable: true,
            closure_shapes: [None; CLOSURE_SHAPES],
            pending_closure_shapes: [None; CLOSURE_SHAPES],
            arguments_shapes: [None; 2],
            jobs: std::collections::VecDeque::new(),
        };
        let cell = heap
            .cells
            .alloc(ValidityCell { valid: true }, &mut heap.gc.room);
        debug_assert_eq!(cell, STATIC_CELL.0);
        heap
    }

    // ---- roots ----

    pub fn roots(&mut self) -> &mut RootStack {
        &mut self.roots.stack
    }

    pub fn handles(&mut self) -> &mut Handles {
        &mut self.roots.handles
    }

    pub fn handles_ref(&self) -> &Handles {
        &self.roots.handles
    }

    /// Adds a runtime root; returns its slot.
    pub fn add_fixed_root(&mut self, value: Value) -> usize {
        self.roots.fixed.push(value);
        self.roots.fixed.len() - 1
    }

    pub fn set_fixed_root(&mut self, slot: usize, value: Value) {
        self.roots.fixed[slot] = value;
    }

    pub fn fixed_root(&self, slot: usize) -> Value {
        self.roots.fixed[slot]
    }

    // ---- collector state ----

    pub fn gc(&self) -> &GcState {
        &self.gc
    }

    pub fn gc_mut(&mut self) -> &mut GcState {
        &mut self.gc
    }

    /// Bytes reserved by every arena, the value heap and the tables beside
    /// them (the ropes' halves, the atom table, the inline caches): what the
    /// cap limits and the budget bounds.
    pub fn bytes(&self) -> usize {
        self.objects.bytes()
            + self.shapes.bytes()
            + self.strings.bytes()
            + self.rope_bytes.get()
            + self.symbols.bytes()
            + self.bigints.bytes()
            + self.cells.bytes()
            + self.payloads.functions.bytes()
            + self.payloads.bytecode.bytes()
            + self.payloads.internals.bytes()
            + self.values.bytes()
            + self.code.live_bytes()
            + self.regexps.bytes()
            + self.table_bytes()
    }

    /// Bytes of the tables beside the arenas: the ropes' halves, the atom
    /// table and the inline caches.
    pub fn table_bytes(&self) -> usize {
        self.ropes.capacity() * (size_of::<(u32, (StringId, StringId))>() + 1)
            + self.atoms.bytes()
            + self.ics.bytes()
    }

    /// Bytes held by live records and live value ranges.
    pub fn used_bytes(&self) -> usize {
        self.objects.used_bytes()
            + self.shapes.used_bytes()
            + self.strings.used_bytes()
            + self.rope_bytes.get()
            + self.symbols.used_bytes()
            + self.bigints.used_bytes()
            + self.cells.used_bytes()
            + self.payloads.functions.used_bytes()
            + self.payloads.bytecode.used_bytes()
            + self.payloads.internals.used_bytes()
            + self.values.used_slots() * 8
            + self.code.live_bytes()
            + self.regexps.bytes()
    }

    pub fn ics(&self) -> &IcStore {
        &self.ics
    }

    pub fn ics_mut(&mut self) -> &mut IcStore {
        &mut self.ics
    }

    pub fn global_generation(&self) -> u32 {
        self.global_generation
    }

    /// A new global generation. The value an empty entry holds is skipped:
    /// on wrapping to it every global entry is cleared, so no entry filled
    /// 2^32 generations ago can hit.
    pub(crate) fn bump_global_generation(&mut self) {
        self.global_generation = self.global_generation.wrapping_add(1);
        if self.global_generation == crate::vm::ic::EMPTY_TAG {
            self.global_generation = 0;
            self.ics.clear_kind(crate::vm::ic::IcKind::Global);
        }
    }

    pub fn value_heap(&self) -> &ValueHeap {
        &self.values
    }

    /// The regular-expression programs.
    pub fn regexps(&self) -> &crate::vm::regexp::RegExpStore {
        &self.regexps
    }

    pub fn atom_table(&self) -> &AtomTable {
        &self.atoms
    }

    pub fn live_objects(&self) -> usize {
        self.objects.live_count()
    }

    pub fn live_shapes(&self) -> usize {
        self.shapes.live_count()
    }

    pub fn live_strings(&self) -> usize {
        self.strings.live_count()
    }

    pub fn live_cells(&self) -> usize {
        self.cells.live_count()
    }

    pub fn is_live(&self, value: Value) -> bool {
        if let Some(id) = value.as_object() {
            self.objects.is_live(id.0)
        } else if let Some(id) = value.as_string() {
            self.strings.is_live(id.0)
        } else if let Some(id) = value.as_symbol() {
            self.symbols.is_live(id.0)
        } else if let Some(id) = value.as_bigint() {
            self.bigints.is_live(id.0)
        } else {
            true
        }
    }

    // ---- objects ----

    /// A new object with room for `slot_capacity` named properties (rounded up
    /// to a size class) before its first growth. The global object kind is
    /// created in dictionary mode.
    pub fn alloc_object(
        &mut self,
        kind: ObjectKind,
        prototype: Option<ObjectId>,
        slot_capacity: u32,
    ) -> Result<ObjectId, HeapError> {
        match kind {
            ObjectKind::Ordinary
            | ObjectKind::Array
            | ObjectKind::Native
            | ObjectKind::Arguments
            | ObjectKind::ForInIterator => self.alloc_plain(kind, prototype, slot_capacity, 0),
            // No program until `vm::regexp::set_program` names one.
            ObjectKind::RegExp => {
                self.alloc_object_with(kind, prototype, slot_capacity, 0, NONE, Need::default())
            }
            ObjectKind::Scope | ObjectKind::GlobalObject => {
                self.alloc_object_with(kind, prototype, slot_capacity, 0, NONE, Need::default())
            }
            ObjectKind::Function => panic!("function objects are created by alloc_function"),
            ObjectKind::Free => panic!("ObjectKind::Free is the poison value"),
            _ => panic!("{kind:?} objects are created by alloc_internal"),
        }
    }

    /// An object of a kind with internal slots: an [`Internal`] record holding
    /// `slots` (rooted for the call) and `data`, with room for `slot_capacity`
    /// named properties. `extra_flags` adds `CALLABLE` and `CONSTRUCTOR` for
    /// the callable kinds.
    #[allow(clippy::too_many_arguments)]
    pub fn alloc_internal(
        &mut self,
        kind: ObjectKind,
        tag: InternalTag,
        prototype: Option<ObjectId>,
        slot_capacity: u32,
        slots: &[Value],
        data: InternalData,
        extra_flags: u8,
    ) -> Result<ObjectId, HeapError> {
        debug_assert!(kind.has_internal());
        let mut keep = Vec::with_capacity(slots.len() + 1);
        keep.push(prototype.map_or(Value::UNDEFINED, Value::object));
        keep.extend_from_slice(slots);
        let need = Need {
            objects: 1,
            shapes: 1,
            internals: 1,
            values: [slot_capacity, 0],
            extra: slots.len() * 8,
            ..Need::default()
        };
        self.safepoint(&need, &keep)?;
        let shape = self.root_shape(prototype, kind);
        let range = self.values.alloc_slots(slot_capacity, &mut self.gc.room);
        let payload = self.payloads.internals.alloc(
            Internal {
                tag,
                slots: slots.to_vec(),
                data,
            },
            &mut self.gc.room,
        );
        let header = ObjectHeader::new(
            kind,
            shape,
            range,
            Range::EMPTY,
            payload,
            flags::EXTENSIBLE | extra_flags,
        );
        Ok(ObjectId(self.objects.alloc(header, &mut self.gc.room)))
    }

    /// The internal record of an object of a kind with internal slots.
    #[inline]
    pub fn internal(&self, object: ObjectId) -> &Internal {
        let header = self.objects.get(object.0);
        debug_assert!(header.kind.has_internal(), "{:?}", header.kind);
        self.payloads.internals.get(header.payload)
    }

    #[inline]
    pub fn internal_mut(&mut self, object: ObjectId) -> &mut Internal {
        let header = self.objects.get(object.0);
        debug_assert!(header.kind.has_internal(), "{:?}", header.kind);
        self.payloads.internals.get_mut(header.payload)
    }

    /// The tag of a [`ObjectKind::Builtin`] object, or `None` for any other.
    #[inline]
    pub fn internal_tag(&self, object: ObjectId) -> InternalTag {
        let header = self.objects.get(object.0);
        if header.kind.has_internal() {
            self.payloads.internals.get(header.payload).tag
        } else {
            InternalTag::None
        }
    }

    #[inline]
    pub fn internal_slot(&self, object: ObjectId, slot: usize) -> Value {
        self.internal(object).slots[slot]
    }

    #[inline]
    pub fn set_internal_slot(&mut self, object: ObjectId, slot: usize, value: Value) {
        self.internal_mut(object).slots[slot] = value;
    }

    /// Appends `value` to the internal slots of `object` (a collection's
    /// entries, a promise's reactions), charging the growth to the trigger.
    pub fn push_internal_slot(&mut self, object: ObjectId, value: Value) -> Result<(), HeapError> {
        let internal = self.internal(object);
        if internal.slots.len() == internal.slots.capacity() {
            let extra = internal.slots.capacity().max(4) * 8;
            self.safepoint(
                &Need {
                    extra,
                    ..Need::default()
                },
                &[Value::object(object), value],
            )?;
        }
        self.internal_mut(object).slots.push(value);
        Ok(())
    }

    /// The common path: a transition-tree root shape, no payload.
    #[inline]
    fn alloc_plain(
        &mut self,
        kind: ObjectKind,
        prototype: Option<ObjectId>,
        slot_capacity: u32,
        element_capacity: u32,
    ) -> Result<ObjectId, HeapError> {
        let need = Need {
            objects: 1,
            shapes: 1,
            values: [slot_capacity, element_capacity],
            ..Need::default()
        };
        self.safepoint(&need, &[prototype.map_or(Value::UNDEFINED, Value::object)])?;
        let shape = self.root_shape(prototype, kind);
        let slots = self.values.alloc_slots(slot_capacity, &mut self.gc.room);
        let elements = self.values.alloc(element_capacity, &mut self.gc.room);
        let header = ObjectHeader::new(kind, shape, slots, elements, 0, flags::EXTENSIBLE);
        Ok(ObjectId(self.objects.alloc(header, &mut self.gc.room)))
    }

    #[inline]
    fn alloc_object_with(
        &mut self,
        kind: ObjectKind,
        prototype: Option<ObjectId>,
        slot_capacity: u32,
        element_capacity: u32,
        payload: u32,
        extra: Need,
    ) -> Result<ObjectId, HeapError> {
        debug_assert!(kind != ObjectKind::Free);
        let need = Need {
            objects: 1,
            shapes: 1,
            values: [slot_capacity, element_capacity],
            ..extra
        };
        let proto_value = prototype.map_or(Value::UNDEFINED, Value::object);
        let payload_value = if kind == ObjectKind::Scope && payload != NONE {
            Value::object(ObjectId(payload))
        } else {
            Value::UNDEFINED
        };
        self.safepoint(&need, &[proto_value, payload_value])?;
        let mut object_flags = flags::EXTENSIBLE;
        let shape = if kind == ObjectKind::GlobalObject {
            object_flags |= flags::DICTIONARY;
            self.dictionary_shape(prototype, kind, Dictionary::default())
        } else {
            self.root_shape(prototype, kind)
        };
        let slots = self.values.alloc_slots(slot_capacity, &mut self.gc.room);
        let elements = self.values.alloc(element_capacity, &mut self.gc.room);
        if kind == ObjectKind::Function {
            object_flags |= flags::CALLABLE;
        }
        let header = ObjectHeader::new(kind, shape, slots, elements, payload, object_flags);
        Ok(ObjectId(self.objects.alloc(header, &mut self.gc.room)))
    }

    pub fn object_kind(&self, object: ObjectId) -> ObjectKind {
        self.objects.get(object.0).kind
    }

    pub fn header(&self, object: ObjectId) -> &ObjectHeader {
        self.objects.get(object.0)
    }

    /// An object with room for `slot_capacity` named properties and
    /// `element_capacity` elements, for the kinds without a side record.
    pub fn alloc_object_with_elements(
        &mut self,
        kind: ObjectKind,
        prototype: Option<ObjectId>,
        slot_capacity: u32,
        element_capacity: u32,
    ) -> Result<ObjectId, HeapError> {
        debug_assert!(matches!(
            kind,
            ObjectKind::Ordinary | ObjectKind::Arguments | ObjectKind::ForInIterator
        ));
        self.alloc_plain(kind, prototype, slot_capacity, element_capacity)
    }

    /// Sets the inline payload of an object of a kind that keeps one (a
    /// mapped `arguments` object's scope, a `for-in` cursor, a regular
    /// expression's program). The caller keeps anything it names alive.
    pub(crate) fn set_payload(&mut self, object: ObjectId, payload: u32) {
        self.objects.get_mut(object.0).payload = payload;
    }

    /// A slot of the range itself, below any shape (the raw slots of a
    /// `for-in` iterator).
    pub(crate) fn set_raw_slot(&mut self, object: ObjectId, slot: u32, value: Value) {
        let slots = self.objects.get(object.0).slots();
        self.values.set(slots, slot, value);
    }

    pub(crate) fn raw_slot(&self, object: ObjectId, slot: u32) -> Value {
        self.values.get(self.objects.get(object.0).slots(), slot)
    }

    // ---- arrays and elements ----

    /// A dense array with room for `capacity` elements; length 0.
    pub fn alloc_array(
        &mut self,
        prototype: Option<ObjectId>,
        capacity: u32,
    ) -> Result<ObjectId, HeapError> {
        self.alloc_object_with(
            ObjectKind::Array,
            prototype,
            0,
            capacity,
            0,
            Need::default(),
        )
    }

    pub fn array_length(&self, array: ObjectId) -> u32 {
        let header = self.objects.get(array.0);
        debug_assert_eq!(header.kind, ObjectKind::Array);
        header.payload
    }

    /// The element at `index`: `Value::EMPTY` for a hole or past the end.
    #[inline]
    pub fn get_element(&self, object: ObjectId, index: u32) -> Value {
        let header = self.objects.get(object.0);
        let elements = header.elements();
        if index < elements.len && (header.kind != ObjectKind::Array || index < header.payload) {
            self.values.get(elements, index)
        } else {
            Value::EMPTY
        }
    }

    /// Stores an element, growing the dense range; indices skipped over become
    /// holes. An array's length follows. There is no sparse fallback here: the
    /// heap cap bounds a far index, and an object goes sparse only through
    /// [`Heap::make_sparse`].
    pub fn set_element(
        &mut self,
        object: ObjectId,
        index: u32,
        value: Value,
    ) -> Result<(), HeapError> {
        let header = *self.objects.get(object.0);
        let mut elements = header.elements();
        if index >= elements.len {
            // Doubling until 1,024 elements, then the next size class (1.5×
            // from a power of two, 4/3× from three times one).
            let grown = if elements.len < 1024 {
                elements.len * 2
            } else {
                values::next_class_len(elements.len)
            };
            let new_len = grown.max(index + 1).max(4);
            self.safepoint(
                &Need {
                    values: [new_len, 0],
                    ..Need::default()
                },
                &[Value::object(object), value],
            )?;
            elements = self.values.grow(elements, new_len, &mut self.gc.room);
            self.objects.get_mut(object.0).set_elements(elements);
        }
        // An array that gets a hole stays a "slow" array for the value
        // printer, as QuickJS's does once it leaves the fast form.
        if header.kind == ObjectKind::Array
            && (value.is_empty()
                || index > header.payload
                || (index > 0 && self.values.get(elements, index - 1).is_empty()))
        {
            self.holey.insert(object.0);
        }
        self.values.set(elements, index, value);
        let header = self.objects.get_mut(object.0);
        if header.kind == ObjectKind::Array && index >= header.payload {
            header.payload = index + 1;
        }
        if header.has(flags::USED_AS_PROTOTYPE) {
            self.holes_writable = false;
        }
        Ok(())
    }

    /// Moves every element of `object` into a named property with an index key
    /// and marks it [`flags::SPARSE`], so element attributes and extensibility
    /// follow the ordinary rules. The element range is freed.
    pub fn make_sparse(&mut self, object: ObjectId) -> Result<(), HeapError> {
        let header = *self.objects.get(object.0);
        if header.has(flags::SPARSE) {
            return Ok(());
        }
        let elements = header.elements();
        let count = if header.kind == ObjectKind::Array {
            header.payload.min(elements.len)
        } else {
            elements.len
        };
        let mut index = 0;
        while index < count {
            let value = self
                .values
                .get(self.objects.get(object.0).elements(), index);
            if !value.is_empty() {
                let key = PropertyKey::index(index).ok_or(HeapError::OutOfMemory)?;
                // The element stays in the range (so rooted) until the range
                // is freed below.
                self.add_property(object, key, crate::shape::PropertyFlags::DEFAULT, value)?;
            }
            index += 1;
        }
        let header = self.objects.get_mut(object.0);
        let elements = header.elements();
        header.set_elements(Range::EMPTY);
        header.flags |= flags::SPARSE;
        self.values.free(elements);
        Ok(())
    }

    /// Records that a write into an element hole may meet an inherited index
    /// property from now on (`Heap::holes_writable`).
    #[inline]
    pub(crate) fn note_prototype_index(&mut self, object: ObjectId) {
        if self.holes_writable && self.objects.get(object.0).has(flags::USED_AS_PROTOTYPE) {
            self.holes_writable = false;
        }
    }

    /// Whether a write into an element hole may skip the prototype chain.
    #[inline]
    pub fn holes_writable(&self) -> bool {
        self.holes_writable
    }

    pub fn push_element(&mut self, array: ObjectId, value: Value) -> Result<(), HeapError> {
        let length = self.array_length(array);
        self.set_element(array, length, value)
    }

    /// Sets an array's length. Shrinking clears the cut elements; growing adds
    /// holes without allocating.
    pub fn set_array_length(&mut self, array: ObjectId, length: u32) {
        let header = *self.objects.get(array.0);
        debug_assert_eq!(header.kind, ObjectKind::Array);
        let elements = header.elements();
        let cut = length.min(elements.len)..header.payload.min(elements.len);
        if !cut.is_empty() {
            self.values.slice_mut(elements)[cut.start as usize..cut.end as usize]
                .fill(Value::EMPTY);
        }
        self.objects.get_mut(array.0).payload = length;
    }

    // ---- scopes and functions ----

    /// A scope with `slots` variables (initialized to `Value::EMPTY`, the TDZ
    /// marker) whose parent is `parent`.
    pub fn alloc_scope(
        &mut self,
        parent: Option<ScopeId>,
        slots: u32,
    ) -> Result<ScopeId, HeapError> {
        let payload = parent.map_or(NONE, |scope| scope.0);
        let object =
            self.alloc_object_with(ObjectKind::Scope, None, slots, 0, payload, Need::default())?;
        Ok(ScopeId(object.0))
    }

    pub fn scope_parent(&self, scope: ScopeId) -> Option<ScopeId> {
        let header = self.objects.get(scope.0);
        debug_assert_eq!(header.kind, ObjectKind::Scope);
        (header.payload != NONE).then_some(ScopeId(header.payload))
    }

    #[inline]
    fn scope_at(&self, mut scope: ScopeId, depth: u32) -> &ObjectHeader {
        for _ in 0..depth {
            scope = ScopeId(self.objects.get(scope.0).payload);
        }
        self.objects.get(scope.0)
    }

    /// `LoadScope(depth, index)`: `depth` parent hops and one load.
    #[inline]
    pub fn scope_get(&self, scope: ScopeId, depth: u32, index: u32) -> Value {
        self.values.get(self.scope_at(scope, depth).slots(), index)
    }

    #[inline]
    pub fn scope_set(&mut self, scope: ScopeId, depth: u32, index: u32, value: Value) {
        let slots = self.scope_at(scope, depth).slots();
        self.values.set(slots, index, value);
    }

    /// A function object capturing `scope`.
    pub fn alloc_function(
        &mut self,
        prototype: Option<ObjectId>,
        scope: Option<ScopeId>,
    ) -> Result<ObjectId, HeapError> {
        self.alloc_function_with(prototype, scope, 2)
    }

    /// [`Heap::alloc_function`] with room for `slot_capacity` properties
    /// (`length`, `name` and a constructor's `prototype`).
    pub fn alloc_function_with(
        &mut self,
        prototype: Option<ObjectId>,
        scope: Option<ScopeId>,
        slot_capacity: u32,
    ) -> Result<ObjectId, HeapError> {
        let scope_value = scope.map_or(Value::UNDEFINED, |scope| Value::object(scope.object()));
        let mark = self.roots.stack.mark();
        self.roots.stack.push(scope_value);
        let need = Need {
            functions: 1,
            ..Need::default()
        };
        let result = self.alloc_object_with(
            ObjectKind::Function,
            prototype,
            slot_capacity,
            0,
            NONE,
            need,
        );
        self.roots.stack.truncate(mark);
        let object = result?;
        let record = FunctionRecord {
            scope: scope.map_or(NONE, |scope| scope.0),
            bytecode: NONE,
            native: NONE,
            home_object: NONE,
            fields: NONE,
        };
        let payload = self.payloads.functions.alloc(record, &mut self.gc.room);
        self.objects.get_mut(object.0).payload = payload;
        Ok(object)
    }

    pub(crate) fn function_record_mut(&mut self, function: ObjectId) -> &mut FunctionRecord {
        let header = self.objects.get(function.0);
        debug_assert_eq!(header.kind, ObjectKind::Function);
        self.payloads.functions.get_mut(header.payload)
    }

    /// Sets the captures of a function made without them (a module's root,
    /// whose environment is made after it is compiled).
    pub(crate) fn set_function_scope(&mut self, function: ObjectId, scope: ScopeId) {
        let header = self.objects.get(function.0);
        debug_assert_eq!(header.kind, ObjectKind::Function);
        let payload = header.payload;
        self.payloads.functions.get_mut(payload).scope = scope.0;
    }

    pub fn function_scope(&self, function: ObjectId) -> Option<ScopeId> {
        let header = self.objects.get(function.0);
        debug_assert_eq!(header.kind, ObjectKind::Function);
        let scope = self.payloads.functions.get(header.payload).scope;
        (scope != NONE).then_some(ScopeId(scope))
    }

    /// A function object over the root of a published code-unit tree, with one
    /// bytecode record per unit (the constant tables and the child links).
    /// `tree[0]` is the root; `children` index into `tree`. The constants are
    /// rooted for the call.
    pub fn alloc_bytecode_function(
        &mut self,
        prototype: Option<ObjectId>,
        scope: Option<ScopeId>,
        tree: &[BytecodeTemplate<'_>],
    ) -> Result<ObjectId, HeapError> {
        assert!(!tree.is_empty(), "a bytecode tree has a root");
        let mark = self.roots.stack.mark();
        for template in tree {
            for &constant in template.constants {
                self.roots.stack.push(constant);
            }
        }
        let result = self.alloc_bytecode_function_rooted(prototype, scope, tree);
        self.roots.stack.truncate(mark);
        result
    }

    fn alloc_bytecode_function_rooted(
        &mut self,
        prototype: Option<ObjectId>,
        scope: Option<ScopeId>,
        tree: &[BytecodeTemplate<'_>],
    ) -> Result<ObjectId, HeapError> {
        let function = self.alloc_function(prototype, scope)?;
        let count = |len: usize| u32::try_from(len).map_err(|_| HeapError::OutOfMemory);
        let mut constant_slots = 0u32;
        let mut child_slots = 0u32;
        for template in tree {
            let constants = values::class_len(count(template.constants.len())?);
            let children = values::class_len(count(template.children.len())?);
            constant_slots = constant_slots
                .checked_add(constants)
                .ok_or(HeapError::OutOfMemory)?;
            child_slots = child_slots
                .checked_add(children)
                .ok_or(HeapError::OutOfMemory)?;
        }
        // One safepoint for the whole tree: the records are unreachable until
        // the root is attached below, so nothing between may collect.
        self.safepoint(
            &Need {
                bytecodes: tree.len(),
                values: [constant_slots, child_slots],
                ..Need::default()
            },
            &[Value::object(function)],
        )?;
        let mut ids = Vec::with_capacity(tree.len());
        for template in tree {
            let constant_count = template.constants.len() as u32;
            let constants = self.values.alloc(constant_count, &mut self.gc.room);
            self.values.slice_mut(constants)[..template.constants.len()]
                .copy_from_slice(template.constants);
            let record = FunctionBytecode {
                unit: template.unit.0,
                constants,
                constant_count,
                children: Range::EMPTY,
                child_count: 0,
            };
            ids.push(self.payloads.bytecode.alloc(record, &mut self.gc.room));
        }
        for (template, &id) in tree.iter().zip(&ids) {
            let child_count = template.children.len() as u32;
            let children = self.values.alloc(child_count, &mut self.gc.room);
            for (slot, &child) in template.children.iter().enumerate() {
                let child = Value::int32(ids[child as usize] as i32);
                self.values.set(children, slot as u32, child);
            }
            let record = self.payloads.bytecode.get_mut(id);
            record.children = children;
            record.child_count = child_count;
        }
        let payload = self.objects.get(function.0).payload;
        self.payloads.functions.get_mut(payload).bytecode = ids[0];
        Ok(function)
    }

    /// A closure over `bytecode`, for `MakeClosure`: no slots, its
    /// properties pending until first used. The record must be reachable
    /// through a rooted function (it is a child of the running function's
    /// record), since a record is not a value the root stack can hold.
    pub fn alloc_closure(
        &mut self,
        prototype: Option<ObjectId>,
        scope: Option<ScopeId>,
        bytecode: BytecodeId,
    ) -> Result<ObjectId, HeapError> {
        let function = self.alloc_function_with(prototype, scope, 0)?;
        debug_assert!(self.payloads.bytecode.is_live(bytecode.0));
        let payload = self.objects.get(function.0).payload;
        self.payloads.functions.get_mut(payload).bytecode = bytecode.0;
        Ok(function)
    }

    /// A host function object: `native` indexes the runtime's native table.
    pub fn alloc_native_function(
        &mut self,
        prototype: Option<ObjectId>,
        native: u32,
    ) -> Result<ObjectId, HeapError> {
        self.alloc_native_closure(prototype, native, None)
    }

    /// A host function object with captured state: `data` (an object whose
    /// internal slots the native reads, rooted by the function) is held in
    /// the function record's scope.
    pub fn alloc_native_closure(
        &mut self,
        prototype: Option<ObjectId>,
        native: u32,
        data: Option<ObjectId>,
    ) -> Result<ObjectId, HeapError> {
        let function = self.alloc_function(prototype, data.map(|data| ScopeId(data.0)))?;
        let payload = self.objects.get(function.0).payload;
        self.payloads.functions.get_mut(payload).native = native;
        Ok(function)
    }

    /// Gives a fresh object a cached shape `shape` and its slot values: no
    /// transition lookups (`MakeClosure`'s and `NewArguments`' fast path).
    pub(crate) fn apply_closure_shape(
        &mut self,
        function: ObjectId,
        shape: ShapeId,
        values: &[Value],
    ) {
        let header = self.objects.get_mut(function.0);
        debug_assert_eq!(self.shapes.get(shape.0).width as usize, values.len());
        debug_assert!(header.slots().len as usize >= values.len());
        header.shape = shape;
        let slots = header.slots();
        for (slot, &value) in values.iter().enumerate() {
            self.values.set(slots, slot as u32, value);
        }
    }

    /// Gives a new closure, allocated with no slots, a cached pending shape
    /// (`MakeClosure`'s fast path).
    pub(crate) fn apply_pending_shape(&mut self, function: ObjectId, shape: ShapeId) {
        let header = self.objects.get_mut(function.0);
        debug_assert_eq!(header.slots().len, 0);
        header.shape = shape;
    }

    /// The function record of a `Function` object.
    #[inline]
    pub fn function_record(&self, function: ObjectId) -> FunctionRecord {
        let header = self.objects.get(function.0);
        debug_assert_eq!(header.kind, ObjectKind::Function);
        *self.payloads.functions.get(header.payload)
    }

    pub fn function_bytecode(&self, function: ObjectId) -> Option<BytecodeId> {
        let header = self.objects.get(function.0);
        debug_assert_eq!(header.kind, ObjectKind::Function);
        let bytecode = self.payloads.functions.get(header.payload).bytecode;
        (bytecode != NONE).then_some(BytecodeId(bytecode))
    }

    #[inline]
    pub fn bytecode_unit(&self, bytecode: BytecodeId) -> CodeUnitId {
        CodeUnitId(self.payloads.bytecode.get(bytecode.0).unit)
    }

    pub fn bytecode_constants(&self, bytecode: BytecodeId) -> &[Value] {
        let record = self.payloads.bytecode.get(bytecode.0);
        &self.values.slice(record.constants)[..record.constant_count as usize]
    }

    #[inline]
    pub fn bytecode_constant(&self, bytecode: BytecodeId, index: u32) -> Value {
        let record = self.payloads.bytecode.get(bytecode.0);
        debug_assert!(index < record.constant_count);
        self.values.get(record.constants, index)
    }

    /// The record of child `index` of `bytecode`'s unit.
    #[inline]
    pub fn bytecode_child(&self, bytecode: BytecodeId, index: u32) -> BytecodeId {
        let record = self.payloads.bytecode.get(bytecode.0);
        debug_assert!(index < record.child_count);
        let child = self.values.get(record.children, index);
        BytecodeId(child.as_int32().expect("child link") as u32)
    }

    pub fn live_bytecodes(&self) -> usize {
        self.payloads.bytecode.live_count()
    }

    // ---- strings, atoms, symbols, BigInts ----

    fn alloc_string_record(&mut self, record: StringRecord) -> Result<StringId, HeapError> {
        let need = Need {
            strings: 1,
            extra: record.flat().map_or(0, StringRepr::content_bytes),
            ..Need::default()
        };
        self.safepoint(&need, &[])?;
        Ok(StringId(self.strings.alloc(record, &mut self.gc.room)))
    }

    /// A string with a representation the caller made canonical.
    pub fn alloc_string_repr(&mut self, repr: StringRepr) -> Result<StringId, HeapError> {
        self.alloc_string_record(StringRecord::new(repr))
    }

    /// Sets an object flag (`flags::ERROR`).
    pub(crate) fn set_object_flag(&mut self, object: ObjectId, flag: u8) {
        self.objects.get_mut(object.0).flags |= flag;
    }

    /// Clears an object flag (`flags::EXTENSIBLE`).
    pub(crate) fn clear_object_flag(&mut self, object: ObjectId, flag: u8) {
        self.objects.get_mut(object.0).flags &= !flag;
    }

    /// A string that is not an atom.
    pub fn alloc_string(&mut self, text: &str) -> Result<StringId, HeapError> {
        self.alloc_string_record(StringRecord::new(StringRepr::from_text(text)))
    }

    pub fn alloc_string_units(&mut self, units: &[u16]) -> Result<StringId, HeapError> {
        self.alloc_string_record(StringRecord::new(StringRepr::from_units(units)))
    }

    /// The record of `id`, flattened if it was a rope.
    #[inline]
    pub fn string(&self, id: StringId) -> &StringRecord {
        let record = self.strings.get(id.0);
        if record.flat().is_none() {
            self.flatten(id, record);
        }
        record
    }

    /// The length of `id` without flattening it.
    #[inline]
    pub fn string_len(&self, id: StringId) -> usize {
        self.strings.get(id.0).len()
    }

    /// Whether every unit of `id` fits in a byte, without flattening it.
    #[inline]
    pub fn string_is_one_byte(&self, id: StringId) -> bool {
        self.strings.get(id.0).is_one_byte()
    }

    /// Flattens a rope in place: an explicit stack walks the tree left to
    /// right, so a left-deep rope of any depth flattens without recursion.
    #[cold]
    #[inline(never)]
    fn flatten(&self, id: StringId, record: &StringRecord) {
        let halves = |id: StringId| self.ropes.get(&id.0).copied();
        let (left, right) = halves(id).expect("a rope");
        let mut stack = vec![right, left];
        let repr = if record.is_one_byte() {
            let mut bytes = Vec::with_capacity(record.len());
            while let Some(id) = stack.pop() {
                let part = self.strings.get(id.0);
                match (part.flat(), halves(id)) {
                    (Some(StringRepr::OneByte(flat)), _) => bytes.extend_from_slice(flat),
                    (Some(StringRepr::TwoByte(_)), _) => unreachable!("a one-byte rope"),
                    (None, Some((left, right))) => {
                        stack.push(right);
                        stack.push(left);
                    }
                    (None, None) => unreachable!("a string with neither units nor halves"),
                }
            }
            StringRepr::OneByte(bytes.into())
        } else {
            let mut units = Vec::with_capacity(record.len());
            while let Some(id) = stack.pop() {
                let part = self.strings.get(id.0);
                match (part.flat(), halves(id)) {
                    (Some(flat), _) => units.extend(flat.units()),
                    (None, Some((left, right))) => {
                        stack.push(right);
                        stack.push(left);
                    }
                    (None, None) => unreachable!("a string with neither units nor halves"),
                }
            }
            StringRepr::TwoByte(units.into())
        };
        self.rope_bytes
            .set(self.rope_bytes.get() + repr.content_bytes());
        record.set_flat(repr);
    }

    /// The concatenation of `left` and `right` as a rope (`len` units).
    pub fn alloc_rope(&mut self, left: StringId, right: StringId) -> Result<StringId, HeapError> {
        let len = self.string_len(left) + self.string_len(right);
        let one_byte = self.string_is_one_byte(left) && self.string_is_one_byte(right);
        let need = Need {
            strings: 1,
            ..Need::default()
        };
        self.safepoint(&need, &[Value::string(left), Value::string(right)])?;
        let record = StringRecord::rope(len as u32, one_byte);
        let id = self.strings.alloc(record, &mut self.gc.room);
        self.ropes.insert(id, (left, right));
        Ok(StringId(id))
    }

    pub fn string_text(&self, id: StringId) -> String {
        self.string(id).repr().to_rust_string()
    }

    fn intern_repr(&mut self, repr: StringRepr) -> Result<StringId, HeapError> {
        let hash = repr.content_hash();
        if let Some(id) = self.atoms.find(&self.strings, &repr, hash) {
            self.retain_atom(id);
            return Ok(id);
        }
        self.insert_atom(repr, hash)
    }

    /// A new atom for `repr`, whose content hash is `hash`.
    fn insert_atom(&mut self, repr: StringRepr, hash: u32) -> Result<StringId, HeapError> {
        let array_index = repr.array_index().unwrap_or(NONE);
        let mut record = StringRecord::new(repr);
        record.is_atom = true;
        record.array_index = array_index;
        let id = self.alloc_string_record(record)?;
        if id.0 >= KEY_INDEX_LIMIT {
            self.strings.free(id.0);
            return Err(HeapError::OutOfMemory);
        }
        self.atoms.insert(hash, id);
        self.retain_atom(id);
        Ok(id)
    }

    /// Keeps an interned atom as [`Heap::atom_mode`] says.
    #[inline]
    fn retain_atom(&mut self, id: StringId) {
        match self.atom_mode {
            AtomMode::Weak => {}
            AtomMode::Hold => self.hold_atom(id),
            AtomMode::Pin => self.pin_atom(id),
        }
    }

    /// Keeps atom `id` until the compilation holding it releases it; an atom
    /// already held (or pinned) is not held twice.
    fn hold_atom(&mut self, id: StringId) {
        let record = self.strings.get_mut(id.0);
        if record.keep == AtomKeep::Weak {
            record.keep = AtomKeep::Held;
            self.held_atoms.push(id);
        }
    }

    /// Keeps atom `id` for the heap's lifetime.
    pub fn pin_atom(&mut self, id: StringId) {
        let record = self.strings.get_mut(id.0);
        debug_assert!(record.is_atom, "pinning a string that is not an atom");
        if record.keep != AtomKeep::Pinned {
            record.keep = AtomKeep::Pinned;
            self.pinned_atoms.push(id);
        }
    }

    /// Sets how [`Heap::intern`] keeps its atoms; returns the mode before.
    pub fn set_atom_mode(&mut self, mode: AtomMode) -> AtomMode {
        std::mem::replace(&mut self.atom_mode, mode)
    }

    pub fn atom_mode(&self) -> AtomMode {
        self.atom_mode
    }

    /// The atom with this content, kept for the heap's lifetime: a name the
    /// natives spell as a literal and hold across allocations.
    pub fn intern_pinned(&mut self, text: &str) -> Result<StringId, HeapError> {
        let mode = self.set_atom_mode(AtomMode::Pin);
        let result = self.intern(text);
        self.atom_mode = mode;
        result
    }

    /// [`Heap::property_key`] kept for the heap's lifetime, as
    /// [`Heap::intern_pinned`].
    pub fn property_key_pinned(&mut self, text: &str) -> Result<PropertyKey, HeapError> {
        let mode = self.set_atom_mode(AtomMode::Pin);
        let result = self.property_key(text);
        self.atom_mode = mode;
        result
    }

    /// The position the atoms held for a compilation start at.
    pub(crate) fn held_atoms_mark(&self) -> usize {
        self.held_atoms.len()
    }

    pub(crate) fn release_held_atoms(&mut self, mark: usize) {
        for position in mark..self.held_atoms.len() {
            let id = self.held_atoms[position];
            let record = self.strings.get_mut(id.0);
            if record.keep == AtomKeep::Held {
                record.keep = AtomKeep::Weak;
            }
        }
        self.held_atoms.truncate(mark);
    }

    /// The atom with this content.
    pub fn intern(&mut self, text: &str) -> Result<StringId, HeapError> {
        self.intern_repr(StringRepr::from_text(text))
    }

    pub fn intern_units(&mut self, units: &[u16]) -> Result<StringId, HeapError> {
        self.intern_repr(StringRepr::from_units(units))
    }

    /// The atom of the Latin-1 string `bytes`; it allocates only when the
    /// atom is new.
    pub fn intern_latin1(&mut self, bytes: &[u8]) -> Result<StringId, HeapError> {
        let hash = string::content_hash(bytes.iter().map(|&byte| u16::from(byte)));
        if let Some(id) = self.atoms.find_one_byte(&self.strings, bytes, hash) {
            self.retain_atom(id);
            return Ok(id);
        }
        self.insert_atom(StringRepr::OneByte(bytes.into()), hash)
    }

    /// The atom with the content of `string` (itself, if it is one).
    pub fn intern_string(&mut self, string: StringId) -> Result<StringId, HeapError> {
        let record = self.string(string);
        if record.is_atom {
            self.retain_atom(string);
            return Ok(string);
        }
        let repr = record.repr().clone();
        self.intern_repr(repr)
    }

    fn key_for_repr(&mut self, repr: StringRepr) -> Result<PropertyKey, HeapError> {
        if let Some(key) = repr.array_index().and_then(PropertyKey::index) {
            return Ok(key);
        }
        Ok(PropertyKey::atom(self.intern_repr(repr)?))
    }

    /// The property key spelled by `text`: an immediate index for a canonical
    /// array index below 2^31 (which never enters the atom table), otherwise
    /// an atom.
    pub fn property_key(&mut self, text: &str) -> Result<PropertyKey, HeapError> {
        self.key_for_repr(StringRepr::from_text(text))
    }

    pub fn property_key_units(&mut self, units: &[u16]) -> Result<PropertyKey, HeapError> {
        self.key_for_repr(StringRepr::from_units(units))
    }

    /// [`Self::property_key`] for the Latin-1 string `bytes`.
    pub fn property_key_latin1(&mut self, bytes: &[u8]) -> Result<PropertyKey, HeapError> {
        let units = || bytes.iter().map(|&byte| u16::from(byte));
        if let Some(key) = string::array_index(bytes.len(), units()).and_then(PropertyKey::index) {
            return Ok(key);
        }
        Ok(PropertyKey::atom(self.intern_latin1(bytes)?))
    }

    pub fn alloc_symbol(
        &mut self,
        description: Option<StringId>,
        private: bool,
    ) -> Result<SymbolId, HeapError> {
        let keep = description.map_or(Value::UNDEFINED, Value::string);
        self.safepoint(
            &Need {
                symbols: 1,
                ..Need::default()
            },
            &[keep],
        )?;
        let record = SymbolRecord {
            description: description.map_or(NONE, |id| id.0),
            private,
        };
        let id = self.symbols.alloc(record, &mut self.gc.room);
        if id >= KEY_INDEX_LIMIT {
            self.symbols.free(id);
            return Err(HeapError::OutOfMemory);
        }
        Ok(SymbolId(id))
    }

    pub fn symbol(&self, id: SymbolId) -> &SymbolRecord {
        self.symbols.get(id.0)
    }

    pub fn symbol_description(&self, id: SymbolId) -> Option<StringId> {
        let description = self.symbols.get(id.0).description;
        (description != NONE).then_some(StringId(description))
    }

    pub fn alloc_bigint(&mut self, negative: bool, limbs: &[u64]) -> Result<BigIntId, HeapError> {
        let used = limbs.len() - limbs.iter().rev().take_while(|&&limb| limb == 0).count();
        let limbs: Box<[u64]> = limbs[..used].into();
        let need = Need {
            bigints: 1,
            extra: limbs.len() * 8,
            ..Need::default()
        };
        self.safepoint(&need, &[])?;
        let record = BigIntRecord {
            negative: negative && !limbs.is_empty(),
            limbs,
        };
        Ok(BigIntId(self.bigints.alloc(record, &mut self.gc.room)))
    }

    pub fn bigint(&self, id: BigIntId) -> &BigIntRecord {
        self.bigints.get(id.0)
    }
}
