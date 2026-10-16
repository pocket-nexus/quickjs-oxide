//! The `Runtime`: the heap, the code store and the realm. Every operation is
//! `fn(&mut Runtime, …)`; there is no shared interior mutability.

use crate::bytecode::{self, CodeStore, CodeUnit, CodeUnitId, ValidateError};
use crate::heap::BytecodeTemplate;
use crate::heap::atom::KeyKind;
use crate::heap::gc::{Handle, HeapLimits};
use crate::heap::hash::FxHashMap;
use crate::heap::object::ObjectKind;
use crate::heap::{Heap, HeapError};
use crate::value::{ObjectId, ScopeId, Value};

/// Intrinsic objects held by the realm. Phase 1 creates only the prototypes
/// the object kinds need; builtins are ported in phase 4.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Intrinsic {
    ObjectPrototype,
    FunctionPrototype,
    ArrayPrototype,
}

impl Intrinsic {
    pub const COUNT: usize = 3;
}

/// The single realm of a runtime (phase 0, section 2). Its objects are fixed
/// roots of the heap.
pub struct Realm {
    global: ObjectId,
    intrinsics: [ObjectId; Intrinsic::COUNT],
}

pub struct Runtime {
    heap: Heap,
    /// Beside the heap, not in it: the main loop borrows the running unit's
    /// code across heap calls (phase 2, section 2).
    code: CodeStore,
    realm: Realm,
}

impl Runtime {
    pub fn new(limits: HeapLimits) -> Result<Runtime, HeapError> {
        let mut heap = Heap::new(limits);
        let object_prototype = heap.alloc_object(ObjectKind::Ordinary, None, 4)?;
        heap.add_fixed_root(Value::object(object_prototype));
        let function_prototype =
            heap.alloc_object(ObjectKind::Ordinary, Some(object_prototype), 4)?;
        heap.add_fixed_root(Value::object(function_prototype));
        let array_prototype = heap.alloc_object(ObjectKind::Ordinary, Some(object_prototype), 4)?;
        heap.add_fixed_root(Value::object(array_prototype));
        let global = heap.alloc_object(ObjectKind::GlobalObject, Some(object_prototype), 16)?;
        heap.add_fixed_root(Value::object(global));
        let realm = Realm {
            global,
            intrinsics: [object_prototype, function_prototype, array_prototype],
        };
        Ok(Runtime {
            heap,
            code: CodeStore::default(),
            realm,
        })
    }

    #[inline]
    pub fn heap(&self) -> &Heap {
        &self.heap
    }

    #[inline]
    pub fn heap_mut(&mut self) -> &mut Heap {
        &mut self.heap
    }

    #[inline]
    pub fn code(&self) -> &CodeStore {
        &self.code
    }

    /// The heap and the code store borrowed together, so code can be read
    /// while the heap is mutated.
    #[inline]
    pub fn heap_and_code(&mut self) -> (&mut Heap, &CodeStore) {
        (&mut self.heap, &self.code)
    }

    /// Validates `unit` and appends it to the code store. Its children must
    /// be published first.
    pub fn publish(&mut self, unit: CodeUnit) -> Result<CodeUnitId, ValidateError> {
        self.code.publish(unit)
    }

    /// A function object for the published unit `root`, with a bytecode
    /// record for it and every unit below it. `constants` holds one table per
    /// unit, in the pre-order of [`CodeStore::tree`]; each must have the
    /// unit's `constant_count` entries. The constants need no rooting.
    pub fn instantiate(
        &mut self,
        root: CodeUnitId,
        constants: &[&[Value]],
    ) -> Result<ObjectId, HeapError> {
        let order = self.code.tree(root);
        assert_eq!(order.len(), constants.len(), "one constant table per unit");
        let position: FxHashMap<CodeUnitId, u32> = order
            .iter()
            .enumerate()
            .map(|(index, &unit)| (unit, index as u32))
            .collect();
        let children: Vec<Vec<u32>> = order
            .iter()
            .map(|&id| {
                let unit = self.code.get(id);
                unit.children.iter().map(|child| position[child]).collect()
            })
            .collect();
        let tree: Vec<BytecodeTemplate<'_>> = order
            .iter()
            .zip(constants)
            .zip(&children)
            .map(|((&unit, &constants), children)| {
                assert_eq!(
                    constants.len(),
                    self.code.get(unit).constant_count as usize,
                    "constant table length"
                );
                BytecodeTemplate {
                    unit,
                    constants,
                    children,
                }
            })
            .collect();
        let prototype = self.intrinsic(Intrinsic::FunctionPrototype);
        self.heap
            .alloc_bytecode_function(Some(prototype), None, &tree)
    }

    /// The disassembly of `unit`, with atom keys spelled out.
    pub fn disassemble(&self, unit: CodeUnitId) -> String {
        let heap = &self.heap;
        bytecode::disassemble(self.code.get(unit), &mut |key| match key.kind() {
            KeyKind::String(id) => format!(".{}", heap.string_text(id)),
            _ => bytecode::disasm::key_placeholder(key),
        })
    }

    pub fn global_object(&self) -> ObjectId {
        self.realm.global
    }

    pub fn intrinsic(&self, intrinsic: Intrinsic) -> ObjectId {
        self.realm.intrinsics[intrinsic as usize]
    }

    /// An ordinary object inheriting from `%Object.prototype%`.
    pub fn new_object(&mut self) -> Result<ObjectId, HeapError> {
        let prototype = self.intrinsic(Intrinsic::ObjectPrototype);
        self.heap
            .alloc_object(ObjectKind::Ordinary, Some(prototype), 4)
    }

    pub fn new_array(&mut self, capacity: u32) -> Result<ObjectId, HeapError> {
        let prototype = self.intrinsic(Intrinsic::ArrayPrototype);
        self.heap.alloc_array(Some(prototype), capacity)
    }

    pub fn new_function(&mut self, scope: Option<ScopeId>) -> Result<ObjectId, HeapError> {
        let prototype = self.intrinsic(Intrinsic::FunctionPrototype);
        self.heap.alloc_function(Some(prototype), scope)
    }

    pub fn new_scope(&mut self, parent: Option<ScopeId>, slots: u32) -> Result<ScopeId, HeapError> {
        self.heap.alloc_scope(parent, slots)
    }

    /// Holds `value` for the embedder until [`Runtime::release`].
    pub fn hold(&mut self, value: Value) -> Handle {
        self.heap.handles().create(value)
    }

    pub fn held(&self, handle: Handle) -> Value {
        self.heap.handles_ref().get(handle)
    }

    pub fn release(&mut self, handle: Handle) {
        self.heap.handles().release(handle);
    }
}
