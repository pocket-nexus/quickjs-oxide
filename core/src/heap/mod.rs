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
pub mod gc;
pub(crate) mod hash;
pub mod object;
pub mod string;
pub mod symbol;
pub mod values;

use std::fmt;

use self::arena::Arena;
use self::atom::{AtomTable, KEY_INDEX_LIMIT, PropertyKey};
use self::bigint::BigIntRecord;
use self::gc::{GcState, Handles, HeapLimits, RootStack, Roots};
use self::hash::FxHashMap;
use self::object::{
    FunctionBytecode, FunctionRecord, NONE, ObjectHeader, ObjectKind, PayloadArenas, flags,
};
use self::string::{StringRecord, StringRepr};
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
            + (values::class_len(self.values[0]) + values::class_len(self.values[1])) as usize * 8
            + self.extra
    }
}

pub struct Heap {
    pub(crate) objects: Arena<ObjectHeader>,
    pub(crate) shapes: Arena<Shape>,
    pub(crate) strings: Arena<StringRecord>,
    pub(crate) symbols: Arena<SymbolRecord>,
    pub(crate) bigints: Arena<BigIntRecord>,
    pub(crate) cells: Arena<ValidityCell>,
    pub(crate) values: ValueHeap,
    pub(crate) payloads: PayloadArenas,
    pub(crate) atoms: AtomTable,
    /// Transition-tree roots by `(prototype index or NONE, kind)`. Weak.
    pub(crate) shape_roots: FxHashMap<(u32, ObjectKind), ShapeId>,
    /// One-entry cache in front of `shape_roots`; cleared by every collection.
    pub(crate) last_root: Option<((u32, ObjectKind), ShapeId)>,
    /// Weak in key and dependents.
    pub(crate) prototypes: FxHashMap<ObjectId, PrototypeInfo>,
    pub(crate) roots: Roots,
    pub(crate) gc: GcState,
    /// The register file and the frame table (phase 2, section 4.1): fixed
    /// roots, so a safepoint marks the running frames with no parameter.
    pub(crate) vm: VmStack,
    /// The inline caches (phase 3, section 2.1): beside the code store in
    /// spirit, inside the heap so that every collection prunes them.
    pub(crate) ics: IcStore,
}

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
            symbols: Arena::new(),
            bigints: Arena::new(),
            cells: Arena::new(),
            values: ValueHeap::new(),
            payloads: PayloadArenas::default(),
            atoms: AtomTable::default(),
            shape_roots: FxHashMap::default(),
            last_root: None,
            prototypes: FxHashMap::default(),
            roots: Roots::default(),
            gc: GcState::new(limits),
            vm: VmStack::default(),
            ics: IcStore::default(),
        };
        let cell = heap.cells.alloc(ValidityCell { valid: true });
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

    /// Bytes reserved by every arena and the value heap: what the cap limits.
    pub fn bytes(&self) -> usize {
        self.objects.bytes()
            + self.shapes.bytes()
            + self.strings.bytes()
            + self.symbols.bytes()
            + self.bigints.bytes()
            + self.cells.bytes()
            + self.payloads.functions.bytes()
            + self.payloads.bytecode.bytes()
            + self.values.bytes()
    }

    /// Bytes held by live records and live value ranges.
    pub fn used_bytes(&self) -> usize {
        self.objects.used_bytes()
            + self.shapes.used_bytes()
            + self.strings.used_bytes()
            + self.symbols.used_bytes()
            + self.bigints.used_bytes()
            + self.cells.used_bytes()
            + self.payloads.functions.used_bytes()
            + self.payloads.bytecode.used_bytes()
            + self.values.used_slots() * 8
    }

    pub fn ics(&self) -> &IcStore {
        &self.ics
    }

    pub fn value_heap(&self) -> &ValueHeap {
        &self.values
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
            ObjectKind::Ordinary | ObjectKind::Array | ObjectKind::Native => {
                self.alloc_plain(kind, prototype, slot_capacity, 0)
            }
            ObjectKind::Scope | ObjectKind::GlobalObject => {
                self.alloc_object_with(kind, prototype, slot_capacity, 0, NONE, Need::default())
            }
            ObjectKind::Function => panic!("function objects are created by alloc_function"),
            ObjectKind::Free => panic!("ObjectKind::Free is the poison value"),
        }
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
        let slots = self.values.alloc(slot_capacity);
        let elements = self.values.alloc(element_capacity);
        let header = ObjectHeader::new(kind, shape, slots, elements, 0, flags::EXTENSIBLE);
        Ok(ObjectId(self.objects.alloc(header)))
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
        let slots = self.values.alloc(slot_capacity);
        let elements = self.values.alloc(element_capacity);
        if kind == ObjectKind::Function {
            object_flags |= flags::CALLABLE;
        }
        let header = ObjectHeader::new(kind, shape, slots, elements, payload, object_flags);
        Ok(ObjectId(self.objects.alloc(header)))
    }

    pub fn object_kind(&self, object: ObjectId) -> ObjectKind {
        self.objects.get(object.0).kind
    }

    pub fn header(&self, object: ObjectId) -> &ObjectHeader {
        self.objects.get(object.0)
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
    /// holes. An array's length follows. Phase 1 has no sparse fallback: the
    /// heap cap bounds a far index.
    pub fn set_element(
        &mut self,
        object: ObjectId,
        index: u32,
        value: Value,
    ) -> Result<(), HeapError> {
        let header = *self.objects.get(object.0);
        let mut elements = header.elements();
        if index >= elements.len {
            let new_len = (elements.len * 2).max(index + 1).max(4);
            self.safepoint(
                &Need {
                    values: [new_len, 0],
                    ..Need::default()
                },
                &[Value::object(object), value],
            )?;
            elements = self.values.grow(elements, new_len);
            self.objects.get_mut(object.0).set_elements(elements);
        }
        self.values.set(elements, index, value);
        let header = self.objects.get_mut(object.0);
        if header.kind == ObjectKind::Array && index >= header.payload {
            header.payload = index + 1;
        }
        Ok(())
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
        let scope_value = scope.map_or(Value::UNDEFINED, |scope| Value::object(scope.object()));
        let mark = self.roots.stack.mark();
        self.roots.stack.push(scope_value);
        let need = Need {
            functions: 1,
            ..Need::default()
        };
        let result = self.alloc_object_with(ObjectKind::Function, prototype, 0, 0, NONE, need);
        self.roots.stack.truncate(mark);
        let object = result?;
        let record = FunctionRecord {
            scope: scope.map_or(NONE, |scope| scope.0),
            bytecode: NONE,
            native: NONE,
        };
        let payload = self.payloads.functions.alloc(record);
        self.objects.get_mut(object.0).payload = payload;
        Ok(object)
    }

    pub fn function_scope(&self, function: ObjectId) -> Option<ScopeId> {
        let header = self.objects.get(function.0);
        debug_assert_eq!(header.kind, ObjectKind::Function);
        let scope = self.payloads.functions.get(header.payload).scope;
        (scope != NONE).then_some(ScopeId(scope))
    }

    /// A function object over the root of a published code-unit tree, with
    /// one bytecode record per unit (the constant tables and the child links
    /// of section 2). `tree[0]` is the root; `children` index into `tree`.
    /// The constants are rooted for the call.
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
            let constants = self.values.alloc(constant_count);
            self.values.slice_mut(constants)[..template.constants.len()]
                .copy_from_slice(template.constants);
            let record = FunctionBytecode {
                unit: template.unit.0,
                constants,
                constant_count,
                children: Range::EMPTY,
                child_count: 0,
            };
            ids.push(self.payloads.bytecode.alloc(record));
        }
        for (template, &id) in tree.iter().zip(&ids) {
            let child_count = template.children.len() as u32;
            let children = self.values.alloc(child_count);
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

    /// A closure over `bytecode`, for `MakeClosure`. The record must be
    /// reachable through a rooted function (it is a child of the running
    /// function's record), since a record is not a value the root stack can
    /// hold.
    pub fn alloc_closure(
        &mut self,
        prototype: Option<ObjectId>,
        scope: Option<ScopeId>,
        bytecode: BytecodeId,
    ) -> Result<ObjectId, HeapError> {
        let function = self.alloc_function(prototype, scope)?;
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
        let function = self.alloc_function(prototype, None)?;
        let payload = self.objects.get(function.0).payload;
        self.payloads.functions.get_mut(payload).native = native;
        Ok(function)
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
            extra: record.repr.content_bytes(),
            ..Need::default()
        };
        self.safepoint(&need, &[])?;
        Ok(StringId(self.strings.alloc(record)))
    }

    /// A string that is not an atom.
    pub fn alloc_string(&mut self, text: &str) -> Result<StringId, HeapError> {
        self.alloc_string_record(StringRecord::new(StringRepr::from_text(text)))
    }

    pub fn alloc_string_units(&mut self, units: &[u16]) -> Result<StringId, HeapError> {
        self.alloc_string_record(StringRecord::new(StringRepr::from_units(units)))
    }

    pub fn string(&self, id: StringId) -> &StringRecord {
        self.strings.get(id.0)
    }

    pub fn string_text(&self, id: StringId) -> String {
        self.strings.get(id.0).repr.to_rust_string()
    }

    fn intern_repr(&mut self, repr: StringRepr) -> Result<StringId, HeapError> {
        let hash = repr.content_hash();
        if let Some(id) = self.atoms.find(&self.strings, &repr, hash) {
            return Ok(id);
        }
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
        Ok(id)
    }

    /// The atom with this content.
    pub fn intern(&mut self, text: &str) -> Result<StringId, HeapError> {
        self.intern_repr(StringRepr::from_text(text))
    }

    pub fn intern_units(&mut self, units: &[u16]) -> Result<StringId, HeapError> {
        self.intern_repr(StringRepr::from_units(units))
    }

    /// The atom with the content of `string` (itself, if it is one).
    pub fn intern_string(&mut self, string: StringId) -> Result<StringId, HeapError> {
        let record = self.strings.get(string.0);
        if record.is_atom {
            return Ok(string);
        }
        let repr = record.repr.clone();
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
        let id = self.symbols.alloc(record);
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
        Ok(BigIntId(self.bigints.alloc(record)))
    }

    pub fn bigint(&self, id: BigIntId) -> &BigIntRecord {
        self.bigints.get(id.0)
    }
}
