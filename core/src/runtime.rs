//! The `Runtime`: the heap plus the realm. Every operation is
//! `fn(&mut Runtime, …)`; there is no shared interior mutability.

use crate::heap::gc::{Handle, HeapLimits};
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
        Ok(Runtime { heap, realm })
    }

    #[inline]
    pub fn heap(&self) -> &Heap {
        &self.heap
    }

    #[inline]
    pub fn heap_mut(&mut self) -> &mut Heap {
        &mut self.heap
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
