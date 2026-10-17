//! The embedder's entries (phase 6, section 2): the object, property,
//! conversion and value-construction operations the root crate's API
//! exposes, each one engine entry over the internal operations the loop
//! uses.
//!
//! Rooting: every `Value` passed in must be rooted by the caller (a handle
//! from [`Runtime::hold`], a fixed root, or a value the caller just received
//! and holds before its next entry). Results are unrooted; hold them before
//! the next call that can allocate.

use crate::heap::atom::{KeyKind, PropertyKey};
use crate::heap::object::flags;
use crate::runtime::{Runtime, WellKnown};
use crate::value::{Kind, ObjectId, Value};
use crate::vm::object::Descriptor;
use crate::vm::{ErrorKind, Throw, ops};
use crate::{HeapError, ObjectKind};

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

impl Runtime {
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
        ops::is_callable(&self.heap, value)
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

    /// A pending promise of `%Promise.prototype%`.
    pub fn new_promise(&mut self) -> Result<ObjectId, Throw> {
        let prototype = self.realm.intrinsic(crate::Intrinsic::PromisePrototype);
        crate::vm::jobs::new_promise(&mut self.cx(), prototype)
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
        Some(ops::big(&self.heap, value).to_string_radix10())
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
        ops::to_boolean(&self.heap, value)
    }

    /// `ToObject(value)`.
    pub fn to_object(&mut self, value: Value) -> Result<ObjectId, Throw> {
        crate::vm::object::to_object(&mut self.cx(), value)
    }

    /// `IsStrictlyEqual(x, y)`.
    pub fn strict_equals(&self, x: Value, y: Value) -> bool {
        ops::strict_eq(&self.heap, x, y)
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
        ops::same_value_zero_heap(&self.heap, x, y)
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
