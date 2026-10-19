//! Functions the embedder defines: a native closure whose data holds the
//! embedder's index, called through the runtime's [`HostCallFn`]. The hook
//! receives a [`HostCx`], the call's view of the engine.

use super::{Native, define_name_length, native_data};
use crate::heap::HeapError;
use crate::heap::atom::PropertyKey;
use crate::heap::buffer::{Buffer, MAX_BUFFER_LENGTH, Store};
use crate::heap::gc::Handle;
use crate::heap::object::{InternalData, InternalTag, ObjectKind, flags};
use crate::runtime::Intrinsic;
use crate::value::{ObjectId, Value};
use crate::vm::error::{self as vm_error, ErrorKind};
use crate::vm::native::CallSite;
use crate::vm::{Cx, Throw, ops, typed};

/// The embedder's entry for its functions: `id` is the index the function
/// was made with. An `Err` is the value to throw.
pub type HostCallFn = fn(&mut HostCx<'_, '_>, id: u32) -> Result<Value, Value>;

/// The data slot holding the embedder's index.
const ID: usize = 0;

/// `HostFunction` and `HostConstructor`.
pub(crate) fn call(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let id = site.data(cx, ID).as_int32().unwrap_or(0) as u32;
    let Some(hook) = cx.host.host_call else {
        return Err(vm_error::type_error(cx, "no host function hook"));
    };
    let mut host = HostCx { cx, site };
    hook(&mut host, id).map_err(Throw)
}

/// A function of the embedder named `name` with `length`, calling the hook
/// with `id`; a constructor when `constructor`.
pub(crate) fn new_host_function(
    cx: &mut Cx<'_>,
    id: u32,
    name: &str,
    length: u32,
    constructor: bool,
) -> Result<ObjectId, Throw> {
    let mark = cx.heap.roots().mark();
    let result = (|| {
        let data = native_data(cx, &[Value::int32(id as i32)])?;
        cx.heap.roots().push(Value::object(data));
        let native = if constructor {
            Native::HostConstructor
        } else {
            Native::HostFunction
        };
        let prototype = cx.realm.intrinsic(Intrinsic::FunctionPrototype);
        let function = cx
            .heap
            .alloc_native_closure(Some(prototype), native.index(), Some(data))
            .map_err(|error| vm_error::heap_error(cx, error))?;
        cx.heap.roots().push(Value::object(function));
        if constructor {
            cx.heap.set_object_flag(function, flags::CONSTRUCTOR);
        }
        let name = if name.is_empty() {
            cx.realm.names.empty
        } else {
            cx.heap
                .intern(name)
                .map_err(|error| vm_error::heap_error(cx, error))?
        };
        let names = cx.realm.names;
        define_name_length(cx.heap, &names, function, Value::string(name), length)
            .map_err(|error| vm_error::heap_error(cx, error))?;
        Ok(function)
    })();
    cx.heap.roots().truncate(mark);
    result
}

/// A host function's view of the engine for one call. Values it returns are
/// unrooted: the embedder holds what it keeps before the next allocation.
pub struct HostCx<'r, 'a> {
    cx: &'r mut Cx<'a>,
    site: CallSite,
}

impl HostCx<'_, '_> {
    // ---- the call ----

    pub fn this(&self) -> Value {
        self.site.this(self.cx)
    }

    pub fn argc(&self) -> usize {
        self.site.argc
    }

    pub fn arg(&self, index: usize) -> Value {
        self.site.arg(self.cx, index)
    }

    /// `new.target` when `new` called the function.
    pub fn new_target(&self) -> Option<Value> {
        self.site.construct.then_some(self.site.new_target)
    }

    // ---- handles ----

    pub fn hold(&mut self, value: Value) -> Handle {
        self.cx.heap.handles().create(value)
    }

    pub fn release(&mut self, handle: Handle) {
        self.cx.heap.handles().release(handle);
    }

    // ---- keys and objects ----

    pub fn key_from_units(&mut self, units: &[u16]) -> Result<PropertyKey, HeapError> {
        self.cx.heap.property_key_units(units)
    }

    pub fn key_from_symbol(&self, symbol: Value) -> Option<PropertyKey> {
        symbol.as_symbol().map(PropertyKey::symbol)
    }

    pub fn get(&mut self, object: ObjectId, key: PropertyKey) -> Result<Value, Value> {
        crate::vm::object::get(self.cx, object, key, Value::object(object)).map_err(|t| t.0)
    }

    pub fn set(&mut self, object: ObjectId, key: PropertyKey, value: Value) -> Result<bool, Value> {
        crate::vm::object::set(self.cx, object, key, value, Value::object(object)).map_err(|t| t.0)
    }

    /// `CreateDataPropertyOrThrow(object, key, value)`.
    pub fn create_data_property(
        &mut self,
        object: ObjectId,
        key: PropertyKey,
        value: Value,
    ) -> Result<(), Value> {
        crate::vm::object::create_data_property_or_throw(self.cx, object, key, value)
            .map(|_| ())
            .map_err(|t| t.0)
    }

    pub fn new_object_with_prototype(
        &mut self,
        prototype: Option<ObjectId>,
    ) -> Result<ObjectId, Value> {
        self.cx
            .heap
            .alloc_object(ObjectKind::Ordinary, prototype, 4)
            .map_err(|error| vm_error::heap_error(self.cx, error).0)
    }

    pub fn intrinsic(&self, intrinsic: Intrinsic) -> ObjectId {
        self.cx.realm.intrinsic(intrinsic)
    }

    pub fn global(&self) -> ObjectId {
        self.cx.realm.global()
    }

    /// `CreateArrayFromList(values)`; the values must be rooted.
    pub fn new_array_from(&mut self, values: &[Value]) -> Result<ObjectId, Value> {
        let cx = &mut *self.cx;
        let length = u32::try_from(values.len()).unwrap_or(u32::MAX);
        let prototype = cx.realm.intrinsic(Intrinsic::ArrayPrototype);
        let array = cx
            .heap
            .alloc_array(Some(prototype), length)
            .map_err(|error| vm_error::heap_error(cx, error).0)?;
        let mark = cx.heap.roots().mark();
        cx.heap.roots().push(Value::object(array));
        let mut result = Ok(array);
        for (index, &value) in values.iter().enumerate() {
            let key = crate::vm::object::index_key(cx, index as u32);
            if let Err(error) = key.and_then(|key| {
                crate::vm::object::create_data_property_or_throw(cx, array, key, value)
            }) {
                result = Err(error.0);
                break;
            }
        }
        cx.heap.roots().truncate(mark);
        result
    }

    pub fn new_host_function(
        &mut self,
        id: u32,
        name: &str,
        length: u32,
        constructor: bool,
    ) -> Result<ObjectId, Value> {
        new_host_function(self.cx, id, name, length, constructor).map_err(|t| t.0)
    }

    // ---- bytes ----

    /// The buffer and byte range `value` exposes: an `ArrayBuffer`'s bytes,
    /// or a typed array's or `DataView`'s window on its buffer. `None` for
    /// any other value, a `SharedArrayBuffer` or a view on one, and a
    /// detached buffer or out-of-bounds view.
    fn byte_range(&self, value: Value) -> Option<(ObjectId, usize, usize)> {
        let object = value.as_object()?;
        let heap = &*self.cx.heap;
        match heap.internal_tag(object) {
            InternalTag::ArrayBuffer => {
                let buffer = typed::buffer(heap, object);
                (!buffer.detached).then(|| (object, 0, buffer.len()))
            }
            InternalTag::TypedArray | InternalTag::DataView => {
                let (view, store) = typed::view(heap, object);
                if heap.internal_tag(store) != InternalTag::ArrayBuffer {
                    return None;
                }
                let buffer = typed::buffer(heap, store);
                let size = view.kind.size();
                let length = view.current_length(buffer.len(), buffer.detached, size)?;
                Some((store, view.byte_offset, length * size))
            }
            _ => None,
        }
    }

    /// The bytes `value` exposes (see [`HostCx::bytes_mut`]).
    pub fn bytes(&self, value: Value) -> Option<&[u8]> {
        let (store, offset, len) = self.byte_range(value)?;
        typed::buffer(self.cx.heap, store)
            .owned_slice()?
            .get(offset..offset + len)
    }

    /// The bytes `value` exposes, writable: an `ArrayBuffer`'s, or a typed
    /// array's or `DataView`'s window. `None` for any other value, shared
    /// memory, and a detached buffer or out-of-bounds view. The borrow ends
    /// before the engine runs again, so the buffer cannot change under it.
    pub fn bytes_mut(&mut self, value: Value) -> Option<&mut [u8]> {
        let (store, offset, len) = self.byte_range(value)?;
        typed::buffer_mut(self.cx.heap, store)
            .owned_slice_mut()?
            .get_mut(offset..offset + len)
    }

    /// A new `ArrayBuffer` owning `bytes`, without copying them.
    pub fn new_array_buffer(&mut self, bytes: Vec<u8>) -> Result<ObjectId, Value> {
        let cx = &mut *self.cx;
        if bytes.len() > MAX_BUFFER_LENGTH {
            return Err(vm_error::range_error(cx, "invalid array buffer length").0);
        }
        let prototype = cx.realm.intrinsic(Intrinsic::ArrayBufferPrototype);
        cx.heap
            .charge(bytes.capacity(), &[Value::object(prototype)])
            .map_err(|error| vm_error::heap_error(cx, error).0)?;
        let buffer = Buffer {
            store: Store::Owned(bytes),
            detached: false,
            max: None,
        };
        cx.heap
            .alloc_internal(
                ObjectKind::Builtin,
                InternalTag::ArrayBuffer,
                Some(prototype),
                0,
                &[],
                InternalData::Buffer(Box::new(buffer)),
                0,
            )
            .map_err(|error| vm_error::heap_error(cx, error).0)
    }

    // ---- calls ----

    pub fn call(&mut self, function: Value, this: Value, args: &[Value]) -> Result<Value, Value> {
        crate::vm::call(self.cx, function, this, args).map_err(|t| t.0)
    }

    pub fn construct(
        &mut self,
        function: Value,
        args: &[Value],
        new_target: Value,
    ) -> Result<Value, Value> {
        crate::vm::construct(self.cx, function, args, new_target).map_err(|t| t.0)
    }

    pub fn is_callable(&self, value: Value) -> bool {
        ops::is_callable(self.cx.heap, value)
    }

    // ---- primitives and conversions ----

    pub fn new_string_units(&mut self, units: &[u16]) -> Result<Value, Value> {
        self.cx
            .heap
            .alloc_string_units(units)
            .map(Value::string)
            .map_err(|error| vm_error::heap_error(self.cx, error).0)
    }

    pub fn string_units(&self, value: Value) -> Option<Vec<u16>> {
        value
            .as_string()
            .map(|id| self.cx.heap.string(id).repr().units().collect())
    }

    pub fn bigint_decimal(&self, value: Value) -> Option<String> {
        value.as_bigint()?;
        Some(ops::big(self.cx.heap, value).to_string_radix10())
    }

    pub fn to_string(&mut self, value: Value) -> Result<Value, Value> {
        ops::to_string(self.cx, value).map_err(|t| t.0)
    }

    pub fn to_number(&mut self, value: Value) -> Result<f64, Value> {
        ops::to_number(self.cx, value).map_err(|t| t.0)
    }

    /// A new error object of `kind` with `message`.
    pub fn new_error(&mut self, kind: ErrorKind, message: &str) -> Value {
        match self.cx.heap.alloc_string(message) {
            Ok(text) => vm_error::new_error(self.cx, kind, Value::string(text))
                .unwrap_or_else(|error| vm_error::heap_error(self.cx, error).0),
            Err(error) => vm_error::heap_error(self.cx, error).0,
        }
    }
}
