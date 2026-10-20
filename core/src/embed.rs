//! The embedder's entries: the object, property, conversion and
//! value-construction operations the root crate's API exposes, each one engine
//! entry over the internal operations the loop uses. They are implemented
//! once, on [`NativeCx`], which a [`Runtime`] lends between entries
//! ([`Runtime::native_cx`]) and the loop lends to a host function while it
//! runs; `Runtime` keeps each as a delegation.
//!
//! Rooting: every `Value` passed in must be rooted by the caller (a handle
//! from [`Runtime::hold`], a fixed root, or a value the caller just received
//! and holds before its next entry). Results are unrooted; hold them before
//! the next call that can allocate.

use std::any::Any;
use std::rc::Rc;

use crate::bytecode::CodeStore;
use crate::heap::atom::{KeyKind, PropertyKey};
use crate::heap::object::{InternalData, InternalTag, flags};
use crate::runtime::{Realm, Runtime, WellKnown};
use crate::value::{Kind, ObjectId, Value};
use crate::vm::native::{HostBody, HostFn};
use crate::vm::object::Descriptor;
use crate::vm::{Cx, ErrorKind, Services, Throw, ops};
use crate::{Heap, HeapError, ObjectKind};

/// A property descriptor: absent fields are `None`. An accessor has `get`
/// or `set` and no `value` or `writable`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PropertyDescriptor {
    pub value: Option<Value>,
    pub get: Option<Value>,
    pub set: Option<Value>,
    pub writable: Option<bool>,
    pub enumerable: Option<bool>,
    pub configurable: Option<bool>,
}

impl PropertyDescriptor {
    pub fn is_accessor(&self) -> bool {
        self.get.is_some() || self.set.is_some()
    }

    fn to_internal(self) -> Descriptor {
        Descriptor {
            value: self.value,
            get: self.get,
            set: self.set,
            writable: self.writable,
            enumerable: self.enumerable,
            configurable: self.configurable,
        }
    }

    fn from_internal(descriptor: Descriptor) -> Self {
        PropertyDescriptor {
            value: descriptor.value,
            get: descriptor.get,
            set: descriptor.set,
            writable: descriptor.writable,
            enumerable: descriptor.enumerable,
            configurable: descriptor.configurable,
        }
    }
}

/// What a property key names, read without running JavaScript.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeyName {
    Index(u32),
    String(Vec<u16>),
    /// A symbol key: the symbol itself (a fixed or otherwise rooted value
    /// whenever the key came from a rooted object).
    Symbol(Value),
}

/// The embedder's view of a running engine: the heap, the realm, the
/// services and the code store, owned (`Runtime::native_cx`) or borrowed
/// (inside a host function).
pub struct NativeCx<'a> {
    pub(crate) heap: &'a mut Heap,
    code: CodeView<'a>,
    pub(crate) realm: &'a Realm,
    pub(crate) host: &'a mut Services,
}

/// The code store a [`NativeCx`] holds: the runtime's own between entries,
/// or the running loop's, shared, inside a host function.
enum CodeView<'a> {
    Owned(&'a mut CodeStore),
    Shared(&'a CodeStore),
}

impl Runtime {
    /// The embedder's view of the runtime between entries.
    #[inline]
    pub fn native_cx(&mut self) -> NativeCx<'_> {
        NativeCx {
            heap: &mut self.heap,
            code: CodeView::Owned(&mut self.code),
            realm: &self.realm,
            host: &mut self.host,
        }
    }
}

/// [`NativeCx::new_host_function`] over the heap and the realm alone (a
/// module loader's compile-time view makes them too).
pub(crate) fn host_function(
    heap: &mut Heap,
    realm: &Realm,
    name: &str,
    length: u32,
    constructor: bool,
    data: &[Value],
    body: Rc<HostFn>,
) -> Result<ObjectId, HeapError> {
    let record = heap.alloc_internal(
        ObjectKind::Builtin,
        InternalTag::NativeData,
        None,
        0,
        data,
        InternalData::HostFn(HostBody(body)),
        0,
    )?;
    let mark = heap.roots().mark();
    heap.roots().push(Value::object(record));
    let result = (|| {
        let native = if constructor {
            crate::builtins::Native::HostConstructor
        } else {
            crate::builtins::Native::HostFunction
        };
        let prototype = realm.intrinsic(crate::Intrinsic::FunctionPrototype);
        let function = heap.alloc_native_closure(Some(prototype), native.index(), Some(record))?;
        heap.roots().push(Value::object(function));
        if constructor {
            heap.set_object_flag(function, flags::CONSTRUCTOR);
        }
        let name = heap.alloc_string(name)?;
        crate::builtins::define_name_length(
            heap,
            &realm.names,
            function,
            Value::string(name),
            length,
        )?;
        Ok(function)
    })();
    heap.roots().truncate(mark);
    result
}

/// [`NativeCx::new_host_object`] over the heap alone.
pub(crate) fn host_object(
    heap: &mut Heap,
    prototype: Option<ObjectId>,
    data: Rc<dyn Any>,
) -> Result<ObjectId, HeapError> {
    heap.alloc_internal(
        ObjectKind::Builtin,
        InternalTag::HostData,
        prototype,
        4,
        &[],
        InternalData::HostData(data),
        0,
    )
}

impl<'a> NativeCx<'a> {
    /// The same view for a shorter borrow.
    pub fn reborrow(&mut self) -> NativeCx<'_> {
        NativeCx {
            heap: self.heap,
            code: match &mut self.code {
                CodeView::Owned(code) => CodeView::Owned(code),
                CodeView::Shared(code) => CodeView::Shared(code),
            },
            realm: self.realm,
            host: self.host,
        }
    }

    /// The view of the loop's context, for a host function.
    pub(crate) fn from_cx<'b>(cx: &'b mut Cx<'_>) -> NativeCx<'b> {
        NativeCx {
            heap: cx.heap,
            code: CodeView::Shared(cx.static_code),
            realm: cx.realm,
            host: cx.host,
        }
    }

    /// The loop's context for one operation.
    #[inline]
    pub(crate) fn cx(&mut self) -> Cx<'_> {
        let static_code: &CodeStore = match &self.code {
            CodeView::Owned(code) => code,
            CodeView::Shared(code) => code,
        };
        Cx {
            heap: self.heap,
            static_code,
            realm: self.realm,
            host: self.host,
        }
    }

    /// The heap, the code store and the realm for a compilation. Its units
    /// go in the code store when the store is owned and `transient` is
    /// false, else in the heap: inside a host function the running loop
    /// borrows the store, so code compiled there is transient whatever
    /// `transient` says.
    pub fn compiler(&mut self, transient: bool) -> crate::vm::compile::Compiler<'_> {
        match &mut self.code {
            CodeView::Owned(code) => {
                if transient {
                    crate::vm::compile::Compiler::new(self.heap, code, self.realm)
                } else {
                    crate::vm::compile::Compiler::owned(self.heap, code, self.realm)
                }
            }
            CodeView::Shared(code) => {
                crate::vm::compile::Compiler::new(self.heap, code, self.realm)
            }
        }
    }

    /// Whether the code store is the runtime's own (between entries), not
    /// the running loop's (inside a host function).
    pub fn owns_code(&self) -> bool {
        matches!(self.code, CodeView::Owned(_))
    }

    #[inline]
    pub fn heap(&self) -> &Heap {
        self.heap
    }

    #[inline]
    pub fn heap_mut(&mut self) -> &mut Heap {
        self.heap
    }

    #[inline]
    pub fn realm(&self) -> &'a Realm {
        self.realm
    }

    /// The bytes of host stack nested re-entries into the loop may use.
    pub fn set_native_stack_budget(&mut self, bytes: usize) {
        self.heap.vm.native_budget = bytes;
    }
}

impl NativeCx<'_> {
    // ---- calls and handles ----

    /// Calls `function`; the result or the thrown value is unrooted. The
    /// caller keeps `function` alive (a handle or a rooted object).
    pub fn call(&mut self, function: Value, this: Value, args: &[Value]) -> Result<Value, Throw> {
        crate::vm::call(&mut self.cx(), function, this, args)
    }

    /// Runs a script function with the global object as `this`.
    pub fn run_script(&mut self, script: ObjectId) -> Result<Value, Throw> {
        let global = Value::object(self.realm.global());
        self.call(Value::object(script), global, &[])
    }

    /// Holds `value` for the embedder until [`NativeCx::release`].
    pub fn hold(&mut self, value: Value) -> crate::Handle {
        self.heap.handles().create(value)
    }

    pub fn held(&self, handle: crate::Handle) -> Value {
        self.heap.handles_ref().get(handle)
    }

    pub fn release(&mut self, handle: crate::Handle) {
        self.heap.handles().release(handle);
    }

    pub fn global_object(&self) -> ObjectId {
        self.realm.global()
    }

    pub fn intrinsic(&self, intrinsic: crate::Intrinsic) -> ObjectId {
        self.realm.intrinsic(intrinsic)
    }

    /// An ordinary object inheriting from `%Object.prototype%`.
    pub fn new_object(&mut self) -> Result<ObjectId, HeapError> {
        let prototype = self.intrinsic(crate::Intrinsic::ObjectPrototype);
        self.heap
            .alloc_object(ObjectKind::Ordinary, Some(prototype), 4)
    }

    // ---- host functions and host objects ----

    /// A host function named `name` with `length`, whose body is `body`
    /// and whose captured values are `data` (read through
    /// [`NativeCall::data`](crate::vm::native::NativeCall::data)). A
    /// `constructor` may be called by `new`; it gets no `prototype`
    /// property (the embedder defines one). `data` must be rooted by the
    /// caller.
    pub fn new_host_function(
        &mut self,
        name: &str,
        length: u32,
        constructor: bool,
        data: &[Value],
        body: Rc<HostFn>,
    ) -> Result<ObjectId, HeapError> {
        host_function(self.heap, self.realm, name, length, constructor, data, body)
    }

    /// An object inheriting from `prototype` (`None` for null) that holds
    /// the embedder's `data`, read back by [`NativeCx::host_data`]. The datum
    /// drops when the object is collected.
    pub fn new_host_object(
        &mut self,
        prototype: Option<ObjectId>,
        data: Rc<dyn Any>,
    ) -> Result<ObjectId, HeapError> {
        host_object(self.heap, prototype, data)
    }

    /// The datum of a host object, whatever its type; `None` for any other
    /// object.
    pub fn host_data(&self, object: ObjectId) -> Option<Rc<dyn Any>> {
        if self.heap.internal_tag(object) != InternalTag::HostData {
            return None;
        }
        match &self.heap.internal(object).data {
            InternalData::HostData(data) => Some(Rc::clone(data)),
            _ => None,
        }
    }

    // ---- keys ----

    /// The property key of the string `units` (an array index when it is
    /// one's canonical form).
    pub fn key_from_units(&mut self, units: &[u16]) -> Result<PropertyKey, HeapError> {
        self.heap.property_key_units(units)
    }

    /// The key of a symbol value; `None` for any other value.
    pub fn key_from_symbol(&self, symbol: Value) -> Option<PropertyKey> {
        symbol.as_symbol().map(PropertyKey::symbol)
    }

    /// `ToPropertyKey(value)`.
    pub fn to_property_key(&mut self, value: Value) -> Result<PropertyKey, Throw> {
        ops::to_property_key(&mut self.cx(), value)
    }

    pub fn key_name(&self, key: PropertyKey) -> KeyName {
        match key.kind() {
            KeyKind::Index(index) => KeyName::Index(index),
            KeyKind::String(id) => KeyName::String(self.heap.string(id).repr().units().collect()),
            KeyKind::Symbol(id) => KeyName::Symbol(Value::symbol(id)),
        }
    }

    // ---- objects ----

    /// `object.[[Get]](key, receiver)`.
    pub fn get(
        &mut self,
        object: ObjectId,
        key: PropertyKey,
        receiver: Value,
    ) -> Result<Value, Throw> {
        crate::vm::object::get(&mut self.cx(), object, key, receiver)
    }

    /// `object.[[Set]](key, value, receiver)`.
    pub fn set(
        &mut self,
        object: ObjectId,
        key: PropertyKey,
        value: Value,
        receiver: Value,
    ) -> Result<bool, Throw> {
        crate::vm::object::set(&mut self.cx(), object, key, value, receiver)
    }

    /// `object.[[GetOwnProperty]](key)`.
    pub fn get_own_property(
        &mut self,
        object: ObjectId,
        key: PropertyKey,
    ) -> Result<Option<PropertyDescriptor>, Throw> {
        Ok(
            crate::vm::object::get_own_property(&mut self.cx(), object, key)?
                .map(PropertyDescriptor::from_internal),
        )
    }

    /// `object.[[DefineOwnProperty]](key, descriptor)`.
    pub fn define_own_property(
        &mut self,
        object: ObjectId,
        key: PropertyKey,
        descriptor: &PropertyDescriptor,
    ) -> Result<bool, Throw> {
        crate::vm::object::define_own_property(
            &mut self.cx(),
            object,
            key,
            &descriptor.to_internal(),
        )
    }

    /// `object.[[Delete]](key)`.
    pub fn delete_property(&mut self, object: ObjectId, key: PropertyKey) -> Result<bool, Throw> {
        crate::vm::object::delete(&mut self.cx(), object, key)
    }

    /// `HasProperty(object, key)`.
    pub fn has_property(&mut self, object: ObjectId, key: PropertyKey) -> Result<bool, Throw> {
        crate::vm::object::has_property(&mut self.cx(), object, key)
    }

    /// `HasOwnProperty(object, key)`.
    pub fn has_own_property(&mut self, object: ObjectId, key: PropertyKey) -> Result<bool, Throw> {
        crate::vm::object::has_own_property(&mut self.cx(), object, key)
    }

    /// `object.[[OwnPropertyKeys]]()`.
    pub fn own_property_keys(&mut self, object: ObjectId) -> Result<Vec<PropertyKey>, Throw> {
        crate::vm::object::own_property_keys(&mut self.cx(), object)
    }

    /// `object.[[GetPrototypeOf]]()`.
    pub fn get_prototype_of(&mut self, object: ObjectId) -> Result<Option<ObjectId>, Throw> {
        Ok(crate::vm::object::get_prototype_of(&mut self.cx(), object)?.as_object())
    }

    /// `object.[[SetPrototypeOf]](prototype)`.
    pub fn set_prototype_of(
        &mut self,
        object: ObjectId,
        prototype: Option<ObjectId>,
    ) -> Result<bool, Throw> {
        let prototype = prototype.map_or(Value::NULL, Value::object);
        crate::vm::object::set_prototype_of(&mut self.cx(), object, prototype)
    }

    /// `object.[[PreventExtensions]]()`.
    pub fn prevent_extensions(&mut self, object: ObjectId) -> Result<bool, Throw> {
        crate::vm::object::prevent_extensions(&mut self.cx(), object)
    }

    /// `Construct(function, args, new_target)`; `new_target` is `function`
    /// itself for a plain `new`.
    pub fn construct(
        &mut self,
        function: Value,
        args: &[Value],
        new_target: Value,
    ) -> Result<Value, Throw> {
        crate::vm::construct(&mut self.cx(), function, args, new_target)
    }

    /// `IsCallable(value)`.
    pub fn is_callable(&self, value: Value) -> bool {
        ops::is_callable(self.heap, value)
    }

    /// `IsConstructor(value)`.
    pub fn is_constructor(&self, value: Value) -> bool {
        value
            .as_object()
            .is_some_and(|object| self.heap.header(object).has(flags::CONSTRUCTOR))
    }

    /// `IsArray(value)`: throws for a revoked proxy.
    pub fn is_array(&mut self, value: Value) -> Result<bool, Throw> {
        crate::vm::object::is_array(&mut self.cx(), value)
    }

    /// Whether `value` is an Error instance (has `[[ErrorData]]`).
    pub fn is_error(&self, value: Value) -> bool {
        value
            .as_object()
            .is_some_and(|object| self.heap.header(object).has(flags::ERROR))
    }

    /// Whether `value` is a promise.
    pub fn is_promise(&mut self, value: Value) -> bool {
        crate::vm::jobs::is_promise(&self.cx(), value)
    }

    pub fn object_kind(&self, object: ObjectId) -> ObjectKind {
        self.heap.object_kind(object)
    }

    /// An ordinary object inheriting from `prototype` (`None` for null).
    pub fn new_object_with_prototype(
        &mut self,
        prototype: Option<ObjectId>,
    ) -> Result<ObjectId, HeapError> {
        self.heap.alloc_object(ObjectKind::Ordinary, prototype, 4)
    }

    /// `CreateArrayFromList(values)`.
    pub fn new_array_from(&mut self, values: &[Value]) -> Result<ObjectId, Throw> {
        let mut cx = self.cx();
        let length = u32::try_from(values.len()).unwrap_or(u32::MAX);
        let prototype = cx.realm.intrinsic(crate::Intrinsic::ArrayPrototype);
        let array = cx
            .heap
            .alloc_array(Some(prototype), length)
            .map_err(|error| crate::vm::error::heap_error(&cx, error))?;
        let mark = cx.heap.roots().mark();
        cx.heap.roots().push(Value::object(array));
        let mut result = Ok(array);
        for (index, &value) in values.iter().enumerate() {
            let key = crate::vm::object::index_key(&mut cx, index as u32);
            if let Err(error) = key.and_then(|key| {
                crate::vm::object::create_data_property_or_throw(&mut cx, array, key, value)
            }) {
                result = Err(error);
                break;
            }
        }
        cx.heap.roots().truncate(mark);
        result
    }

    /// A new error object of `kind` with `message`, a `stack` from the
    /// running frames.
    pub fn new_error(&mut self, kind: ErrorKind, message: &str) -> Result<Value, HeapError> {
        let message = self.heap.alloc_string(message)?;
        crate::vm::error::new_error(&mut self.cx(), kind, Value::string(message))
    }

    /// A new error object located at `file:line:column` (a compile error):
    /// `fileName`, `lineNumber` and `columnNumber`, and a `stack` that
    /// starts there, the running frames after it.
    pub fn new_located_error(
        &mut self,
        kind: ErrorKind,
        message: &str,
        file: &str,
        line: u32,
        column: u32,
    ) -> Result<Value, HeapError> {
        let message = self.heap.alloc_string(message)?;
        let location = crate::vm::error::Location { file, line, column };
        crate::vm::error::new_error_at(
            &mut self.cx(),
            kind,
            Value::string(message),
            Some(location),
            0,
        )
    }

    /// A pending promise of `%Promise.prototype%`.
    pub fn new_promise(&mut self) -> Result<ObjectId, Throw> {
        let prototype = self.realm.intrinsic(crate::Intrinsic::PromisePrototype);
        crate::vm::jobs::new_promise(&mut self.cx(), prototype)
    }

    /// QuickJS's `JS_DetachArrayBuffer`: detaches an ArrayBuffer; anything
    /// else is ignored.
    pub fn detach_array_buffer(&mut self, value: Value) -> Result<(), Throw> {
        let is_buffer = value.as_object().is_some_and(|object| {
            self.heap.internal_tag(object) == crate::heap::object::InternalTag::ArrayBuffer
        });
        if is_buffer {
            crate::builtins::detach_buffer(&mut self.cx(), value)?;
        }
        Ok(())
    }

    // ---- primitives ----

    pub fn new_string(&mut self, text: &str) -> Result<Value, HeapError> {
        self.heap.alloc_string(text).map(Value::string)
    }

    pub fn new_string_units(&mut self, units: &[u16]) -> Result<Value, HeapError> {
        self.heap.alloc_string_units(units).map(Value::string)
    }

    /// A string value's code units; `None` for any other value.
    pub fn string_units(&self, value: Value) -> Option<Vec<u16>> {
        value
            .as_string()
            .map(|id| self.heap.string(id).repr().units().collect())
    }

    /// A string value as Rust text (lone surrogates replaced).
    pub fn string_lossy(&self, value: Value) -> Option<String> {
        value.as_string().map(|id| self.heap.string_text(id))
    }

    /// The BigInt of the decimal text `decimal` (an optional `-`, then
    /// digits); `None` when it does not parse.
    pub fn new_bigint(&mut self, decimal: &str) -> Option<Result<Value, HeapError>> {
        let big = crate::vm::bigint::Big::parse(decimal)?;
        Some(
            self.heap
                .alloc_bigint(big.negative, &big.limbs)
                .map(Value::bigint),
        )
    }

    /// A BigInt value in decimal; `None` for any other value.
    pub fn bigint_decimal(&self, value: Value) -> Option<String> {
        value.as_bigint()?;
        Some(ops::big(self.heap, value).to_string_radix10())
    }

    /// A new symbol with `description` (`Symbol(description)`).
    pub fn new_symbol(&mut self, description: Option<&[u16]>) -> Result<Value, HeapError> {
        let description = match description {
            Some(units) => Some(self.heap.alloc_string_units(units)?),
            None => None,
        };
        self.heap
            .alloc_symbol(description, false)
            .map(Value::symbol)
    }

    /// A symbol's `[[Description]]`; `None` when it is undefined or `value`
    /// is not a symbol.
    pub fn symbol_description(&self, value: Value) -> Option<Vec<u16>> {
        let record = self.heap.symbol(value.as_symbol()?);
        (record.description != crate::heap::object::NONE).then(|| {
            self.heap
                .string(crate::value::StringId(record.description))
                .repr()
                .units()
                .collect()
        })
    }

    /// The well-known symbol `Symbol.<name>` (`"iterator"`, …).
    pub fn well_known_symbol(&self, name: &str) -> Option<Value> {
        self.realm
            .symbols
            .all()
            .into_iter()
            .find(|&(candidate, _)| candidate == name)
            .map(|(_, id)| Value::symbol(id))
    }

    pub fn well_known_symbol_key(&self, name: &str) -> Option<PropertyKey> {
        self.well_known_symbol(name)
            .and_then(|symbol| symbol.as_symbol())
            .map(WellKnown::key)
    }

    // ---- conversions ----

    /// `ToString(value)`.
    pub fn to_string(&mut self, value: Value) -> Result<Value, Throw> {
        ops::to_string(&mut self.cx(), value)
    }

    /// `ToNumber(value)`.
    pub fn to_number(&mut self, value: Value) -> Result<f64, Throw> {
        ops::to_number(&mut self.cx(), value)
    }

    /// `ToBoolean(value)`.
    pub fn to_boolean(&self, value: Value) -> bool {
        ops::to_boolean(self.heap, value)
    }

    /// `ToObject(value)`.
    pub fn to_object(&mut self, value: Value) -> Result<ObjectId, Throw> {
        crate::vm::object::to_object(&mut self.cx(), value)
    }

    /// `IsStrictlyEqual(x, y)`.
    pub fn strict_equals(&self, x: Value, y: Value) -> bool {
        ops::strict_eq(self.heap, x, y)
    }

    /// `IsLooselyEqual(x, y)`.
    pub fn loose_equals(&mut self, x: Value, y: Value) -> Result<bool, Throw> {
        ops::loose_eq(&mut self.cx(), x, y)
    }

    /// `SameValue(x, y)`.
    pub fn same_value(&mut self, x: Value, y: Value) -> bool {
        crate::vm::object::same_value(&self.cx(), x, y)
    }

    /// `SameValueZero(x, y)`.
    pub fn same_value_zero(&self, x: Value, y: Value) -> bool {
        ops::same_value_zero_heap(self.heap, x, y)
    }

    /// `typeof value`.
    pub fn type_of(&mut self, value: Value) -> String {
        let cx = self.cx();
        let text = ops::type_of(&cx, value);
        self.heap
            .string_text(text.as_string().expect("a typeof atom"))
    }

    /// The kind of a value without running JavaScript.
    pub fn kind_of(&self, value: Value) -> Kind {
        value.kind()
    }

    // ---- collection ----

    /// Runs a full collection. Only between entries: the embedder's handles
    /// and the fixed roots are the roots.
    pub fn run_gc(&mut self) {
        self.heap.collect();
    }
}

impl Runtime {
    // ---- keys ----

    /// [`NativeCx::key_from_units`].
    pub fn key_from_units(&mut self, units: &[u16]) -> Result<PropertyKey, HeapError> {
        self.native_cx().key_from_units(units)
    }

    /// The key of a symbol value; `None` for any other value.
    pub fn key_from_symbol(&self, symbol: Value) -> Option<PropertyKey> {
        symbol.as_symbol().map(PropertyKey::symbol)
    }

    /// [`NativeCx::to_property_key`].
    pub fn to_property_key(&mut self, value: Value) -> Result<PropertyKey, Throw> {
        self.native_cx().to_property_key(value)
    }

    pub fn key_name(&self, key: PropertyKey) -> KeyName {
        match key.kind() {
            KeyKind::Index(index) => KeyName::Index(index),
            KeyKind::String(id) => KeyName::String(self.heap.string(id).repr().units().collect()),
            KeyKind::Symbol(id) => KeyName::Symbol(Value::symbol(id)),
        }
    }

    // ---- objects ----

    /// [`NativeCx::get`].
    pub fn get(
        &mut self,
        object: ObjectId,
        key: PropertyKey,
        receiver: Value,
    ) -> Result<Value, Throw> {
        self.native_cx().get(object, key, receiver)
    }

    /// [`NativeCx::set`].
    pub fn set(
        &mut self,
        object: ObjectId,
        key: PropertyKey,
        value: Value,
        receiver: Value,
    ) -> Result<bool, Throw> {
        self.native_cx().set(object, key, value, receiver)
    }

    /// [`NativeCx::get_own_property`].
    pub fn get_own_property(
        &mut self,
        object: ObjectId,
        key: PropertyKey,
    ) -> Result<Option<PropertyDescriptor>, Throw> {
        self.native_cx().get_own_property(object, key)
    }

    /// [`NativeCx::define_own_property`].
    pub fn define_own_property(
        &mut self,
        object: ObjectId,
        key: PropertyKey,
        descriptor: &PropertyDescriptor,
    ) -> Result<bool, Throw> {
        self.native_cx()
            .define_own_property(object, key, descriptor)
    }

    /// [`NativeCx::delete_property`].
    pub fn delete_property(&mut self, object: ObjectId, key: PropertyKey) -> Result<bool, Throw> {
        self.native_cx().delete_property(object, key)
    }

    /// [`NativeCx::has_property`].
    pub fn has_property(&mut self, object: ObjectId, key: PropertyKey) -> Result<bool, Throw> {
        self.native_cx().has_property(object, key)
    }

    /// [`NativeCx::has_own_property`].
    pub fn has_own_property(&mut self, object: ObjectId, key: PropertyKey) -> Result<bool, Throw> {
        self.native_cx().has_own_property(object, key)
    }

    /// [`NativeCx::own_property_keys`].
    pub fn own_property_keys(&mut self, object: ObjectId) -> Result<Vec<PropertyKey>, Throw> {
        self.native_cx().own_property_keys(object)
    }

    /// [`NativeCx::get_prototype_of`].
    pub fn get_prototype_of(&mut self, object: ObjectId) -> Result<Option<ObjectId>, Throw> {
        self.native_cx().get_prototype_of(object)
    }

    /// [`NativeCx::set_prototype_of`].
    pub fn set_prototype_of(
        &mut self,
        object: ObjectId,
        prototype: Option<ObjectId>,
    ) -> Result<bool, Throw> {
        self.native_cx().set_prototype_of(object, prototype)
    }

    /// [`NativeCx::prevent_extensions`].
    pub fn prevent_extensions(&mut self, object: ObjectId) -> Result<bool, Throw> {
        self.native_cx().prevent_extensions(object)
    }

    /// [`NativeCx::construct`].
    pub fn construct(
        &mut self,
        function: Value,
        args: &[Value],
        new_target: Value,
    ) -> Result<Value, Throw> {
        self.native_cx().construct(function, args, new_target)
    }

    /// `IsCallable(value)`.
    pub fn is_callable(&self, value: Value) -> bool {
        ops::is_callable(&self.heap, value)
    }

    /// `IsConstructor(value)`.
    pub fn is_constructor(&self, value: Value) -> bool {
        value
            .as_object()
            .is_some_and(|object| self.heap.header(object).has(flags::CONSTRUCTOR))
    }

    /// [`NativeCx::is_array`].
    pub fn is_array(&mut self, value: Value) -> Result<bool, Throw> {
        self.native_cx().is_array(value)
    }

    /// Whether `value` is an Error instance (has `[[ErrorData]]`).
    pub fn is_error(&self, value: Value) -> bool {
        value
            .as_object()
            .is_some_and(|object| self.heap.header(object).has(flags::ERROR))
    }

    /// [`NativeCx::is_promise`].
    pub fn is_promise(&mut self, value: Value) -> bool {
        self.native_cx().is_promise(value)
    }

    pub fn object_kind(&self, object: ObjectId) -> ObjectKind {
        self.heap.object_kind(object)
    }

    /// [`NativeCx::new_object_with_prototype`].
    pub fn new_object_with_prototype(
        &mut self,
        prototype: Option<ObjectId>,
    ) -> Result<ObjectId, HeapError> {
        self.native_cx().new_object_with_prototype(prototype)
    }

    /// [`NativeCx::new_array_from`].
    pub fn new_array_from(&mut self, values: &[Value]) -> Result<ObjectId, Throw> {
        self.native_cx().new_array_from(values)
    }

    /// [`NativeCx::new_error`].
    pub fn new_error(&mut self, kind: ErrorKind, message: &str) -> Result<Value, HeapError> {
        self.native_cx().new_error(kind, message)
    }

    /// [`NativeCx::new_located_error`].
    pub fn new_located_error(
        &mut self,
        kind: ErrorKind,
        message: &str,
        file: &str,
        line: u32,
        column: u32,
    ) -> Result<Value, HeapError> {
        self.native_cx()
            .new_located_error(kind, message, file, line, column)
    }

    /// [`NativeCx::new_promise`].
    pub fn new_promise(&mut self) -> Result<ObjectId, Throw> {
        self.native_cx().new_promise()
    }

    /// [`NativeCx::detach_array_buffer`].
    pub fn detach_array_buffer(&mut self, value: Value) -> Result<(), Throw> {
        self.native_cx().detach_array_buffer(value)
    }

    // ---- primitives ----

    /// [`NativeCx::new_string`].
    pub fn new_string(&mut self, text: &str) -> Result<Value, HeapError> {
        self.native_cx().new_string(text)
    }

    /// [`NativeCx::new_string_units`].
    pub fn new_string_units(&mut self, units: &[u16]) -> Result<Value, HeapError> {
        self.native_cx().new_string_units(units)
    }

    /// A string value's code units; `None` for any other value.
    pub fn string_units(&self, value: Value) -> Option<Vec<u16>> {
        value
            .as_string()
            .map(|id| self.heap.string(id).repr().units().collect())
    }

    /// A string value as Rust text (lone surrogates replaced).
    pub fn string_lossy(&self, value: Value) -> Option<String> {
        value.as_string().map(|id| self.heap.string_text(id))
    }

    /// [`NativeCx::new_bigint`].
    pub fn new_bigint(&mut self, decimal: &str) -> Option<Result<Value, HeapError>> {
        self.native_cx().new_bigint(decimal)
    }

    /// A BigInt value in decimal; `None` for any other value.
    pub fn bigint_decimal(&self, value: Value) -> Option<String> {
        value.as_bigint()?;
        Some(ops::big(&self.heap, value).to_string_radix10())
    }

    /// [`NativeCx::new_symbol`].
    pub fn new_symbol(&mut self, description: Option<&[u16]>) -> Result<Value, HeapError> {
        self.native_cx().new_symbol(description)
    }

    /// A symbol's `[[Description]]`; `None` when it is undefined or `value`
    /// is not a symbol.
    pub fn symbol_description(&self, value: Value) -> Option<Vec<u16>> {
        let record = self.heap.symbol(value.as_symbol()?);
        (record.description != crate::heap::object::NONE).then(|| {
            self.heap
                .string(crate::value::StringId(record.description))
                .repr()
                .units()
                .collect()
        })
    }

    /// The well-known symbol `Symbol.<name>` (`"iterator"`, …).
    pub fn well_known_symbol(&self, name: &str) -> Option<Value> {
        self.realm
            .symbols
            .all()
            .into_iter()
            .find(|&(candidate, _)| candidate == name)
            .map(|(_, id)| Value::symbol(id))
    }

    pub fn well_known_symbol_key(&self, name: &str) -> Option<PropertyKey> {
        self.well_known_symbol(name)
            .and_then(|symbol| symbol.as_symbol())
            .map(WellKnown::key)
    }

    // ---- conversions ----

    /// [`NativeCx::to_string`].
    pub fn to_string(&mut self, value: Value) -> Result<Value, Throw> {
        self.native_cx().to_string(value)
    }

    /// [`NativeCx::to_number`].
    pub fn to_number(&mut self, value: Value) -> Result<f64, Throw> {
        self.native_cx().to_number(value)
    }

    /// `ToBoolean(value)`.
    pub fn to_boolean(&self, value: Value) -> bool {
        ops::to_boolean(&self.heap, value)
    }

    /// [`NativeCx::to_object`].
    pub fn to_object(&mut self, value: Value) -> Result<ObjectId, Throw> {
        self.native_cx().to_object(value)
    }

    /// `IsStrictlyEqual(x, y)`.
    pub fn strict_equals(&self, x: Value, y: Value) -> bool {
        ops::strict_eq(&self.heap, x, y)
    }

    /// [`NativeCx::loose_equals`].
    pub fn loose_equals(&mut self, x: Value, y: Value) -> Result<bool, Throw> {
        self.native_cx().loose_equals(x, y)
    }

    /// [`NativeCx::same_value`].
    pub fn same_value(&mut self, x: Value, y: Value) -> bool {
        self.native_cx().same_value(x, y)
    }

    /// `SameValueZero(x, y)`.
    pub fn same_value_zero(&self, x: Value, y: Value) -> bool {
        ops::same_value_zero_heap(&self.heap, x, y)
    }

    /// [`NativeCx::type_of`].
    pub fn type_of(&mut self, value: Value) -> String {
        self.native_cx().type_of(value)
    }

    /// The kind of a value without running JavaScript.
    pub fn kind_of(&self, value: Value) -> Kind {
        value.kind()
    }

    // ---- collection ----

    /// [`NativeCx::run_gc`].
    pub fn run_gc(&mut self) {
        self.native_cx().run_gc()
    }
}
