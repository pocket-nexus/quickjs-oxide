//! QuickJS-style builtin property tables and their bootstrap installer.
//!
//! Each intrinsic object publishes `const` tables of [`BuiltinEntry`] values,
//! mirroring QuickJS's `JSCFunctionListEntry` arrays. Consecutive lazy methods
//! are appended as one layout; every other entry is a publication boundary in
//! table order, so own-key order and function identities follow the table.

use super::shape::{PropertyFlags, PropertyStorageKind, ShapeEntry};
use super::{
    AccessorValue, DescriptorField, ObjectRef, OrdinaryPropertyDescriptor, PropertyKey,
    WellKnownSymbol,
};
use crate::engine::api::runtime::Runtime;
use crate::engine::api::runtime_error::RuntimeError;
use crate::engine::atom::AtomIdx;
use crate::engine::builtins::native::NativeFunctionId;
use crate::engine::heap::runtime::{RuntimeState, owned_values::OwnedValueGuard};
use crate::engine::heap::{
    AutoInitProperty, ContextId, HeapError, ObjectId, ObjectPayload, PropertySlot, RawValue,
};
use crate::engine::object::property::PropertyDescriptor;
use crate::engine::value::JsString;
use crate::engine::value::{JsValue, Value};
use crate::engine::vm::Completion;
use std::cell::Cell;
use std::collections::HashSet;

/// One lazy method, keeping its descriptor and callable metadata together.
#[derive(Clone, Copy)]
pub(crate) struct NativeBuiltinProperty {
    pub(crate) target: NativeFunctionId,
    pub(crate) name: &'static str,
    pub(crate) length: u8,
    pub(crate) min_readable_args: u8,
    pub(crate) flags: PropertyFlags,
    /// Symbol-keyed methods use `name` (`"[Symbol.x]"`) only as function name.
    pub(crate) symbol: Option<WellKnownSymbol>,
}

impl NativeBuiltinProperty {
    pub(crate) const fn new(
        target: NativeFunctionId,
        name: &'static str,
        length: u8,
        min_readable_args: u8,
    ) -> Self {
        Self {
            target,
            name,
            length,
            min_readable_args,
            flags: METHOD_FLAGS,
            symbol: None,
        }
    }
}

/// A builtin table property key.
#[derive(Clone, Copy)]
pub(crate) enum BuiltinKey {
    Name(&'static str),
    Symbol(WellKnownSymbol),
}

/// One eagerly created accessor half: its target and function name.
pub(crate) type BuiltinAccessorFunction = Option<(NativeFunctionId, &'static str)>;

/// A constant data property value.
#[derive(Clone, Copy)]
pub(crate) enum BuiltinValue {
    Float(f64),
    Undefined,
    Symbol(WellKnownSymbol),
}

/// One `JSCFunctionListEntry` equivalent.
#[derive(Clone, Copy)]
pub(crate) enum BuiltinEntry {
    /// `JS_CFUNC_DEF`: a lazy (AutoInit) native method.
    Method(NativeBuiltinProperty),
    /// `JS_CGETSET_DEF`: eager getter/setter functions, configurable and
    /// non-enumerable. Getters read no arguments; setters read one.
    Accessor {
        key: BuiltinKey,
        get: BuiltinAccessorFunction,
        set: BuiltinAccessorFunction,
    },
    /// `JS_ALIAS_DEF`: materialize `from` and store that exact function
    /// object as a writable, configurable, non-enumerable data property.
    Alias { key: BuiltinKey, from: BuiltinKey },
    /// `@@toStringTag` as a non-writable, configurable string.
    ToStringTag(&'static str),
    /// A non-enumerable constant data property.
    Value {
        name: &'static str,
        value: BuiltinValue,
        writable: bool,
        configurable: bool,
    },
    /// A lazy writable, configurable, non-enumerable string property.
    LazyString {
        name: &'static str,
        value: &'static str,
    },
}

/// The default writable, configurable, non-enumerable method flags.
pub(crate) const METHOD_FLAGS: PropertyFlags = PropertyFlags::data(true, false, true);

/// A lazy, writable, configurable, non-enumerable method.
pub(crate) const fn method(
    target: NativeFunctionId,
    name: &'static str,
    length: u8,
    min_readable_args: u8,
) -> BuiltinEntry {
    BuiltinEntry::Method(NativeBuiltinProperty::new(
        target,
        name,
        length,
        min_readable_args,
    ))
}

/// A lazy symbol-keyed method whose function is named `name`.
pub(crate) const fn symbol_method(
    symbol: WellKnownSymbol,
    target: NativeFunctionId,
    name: &'static str,
    length: u8,
    flags: PropertyFlags,
) -> BuiltinEntry {
    let mut method = NativeBuiltinProperty::new(target, name, length, length);
    method.flags = flags;
    method.symbol = Some(symbol);
    BuiltinEntry::Method(method)
}

/// A symbol-keyed getter-only accessor.
pub(crate) const fn symbol_getter(
    symbol: WellKnownSymbol,
    target: NativeFunctionId,
    getter_name: &'static str,
) -> BuiltinEntry {
    BuiltinEntry::Accessor {
        key: BuiltinKey::Symbol(symbol),
        get: Some((target, getter_name)),
        set: None,
    }
}

/// The `get [Symbol.species]` accessor of species-aware constructors.
pub(crate) const fn species_getter(target: NativeFunctionId) -> BuiltinEntry {
    symbol_getter(WellKnownSymbol::Species, target, "get [Symbol.species]")
}

pub(crate) const fn alias(key: BuiltinKey, from: BuiltinKey) -> BuiltinEntry {
    BuiltinEntry::Alias { key, from }
}

pub(crate) const fn to_string_tag(value: &'static str) -> BuiltinEntry {
    BuiltinEntry::ToStringTag(value)
}

/// A string-keyed getter-only accessor whose function is named `get <name>`.
macro_rules! builtin_getter {
    ($target:expr, $name:literal) => {
        $crate::engine::object::builtin_properties::BuiltinEntry::Accessor {
            key: $crate::engine::object::builtin_properties::BuiltinKey::Name($name),
            get: Some(($target, concat!("get ", $name))),
            set: None,
        }
    };
}
pub(crate) use builtin_getter;

impl Runtime {
    fn builtin_key(&self, key: BuiltinKey) -> Result<PropertyKey, RuntimeError> {
        match key {
            BuiltinKey::Name(name) => self.intern_property_key(name),
            BuiltinKey::Symbol(symbol) => Ok(PropertyKey::from(self.well_known_symbol(symbol)?)),
        }
    }

    /// Install one builtin table on `object` in table order.
    pub(crate) fn define_builtin_entries(
        &self,
        object: &ObjectRef,
        realm: ContextId,
        entries: &[BuiltinEntry],
    ) -> Result<(), RuntimeError> {
        let mut rest = entries;
        while let Some(entry) = rest.first() {
            if let BuiltinEntry::Method(_) = entry {
                let run = rest
                    .iter()
                    .take_while(|entry| matches!(entry, BuiltinEntry::Method(_)))
                    .count();
                let methods = rest[..run].iter().filter_map(|entry| match entry {
                    BuiltinEntry::Method(method) => Some(*method),
                    _ => None,
                });
                self.define_native_builtin_auto_init_batch(object, realm, methods)?;
                rest = &rest[run..];
                continue;
            }
            match *entry {
                BuiltinEntry::Method(_) => unreachable!("method runs are batched"),
                BuiltinEntry::Accessor { key, get, set } => {
                    let function_prototype = self
                        .0
                        .state
                        .borrow()
                        .heap
                        .context(realm)?
                        .function_prototype;
                    let function_prototype =
                        ObjectRef::from_borrowed_handle(self.clone(), function_prototype)?;
                    let half = |half: BuiltinAccessorFunction, arity: u8| match half {
                        Some((target, name)) => self
                            .new_native_builtin(
                                &function_prototype,
                                realm,
                                target,
                                arity,
                                name,
                                i32::from(arity),
                            )
                            .map(AccessorValue::Callable),
                        None => Ok(AccessorValue::Undefined),
                    };
                    let get = half(get, 0)?;
                    let set = half(set, 1)?;
                    let key = self.builtin_key(key)?;
                    if !self.define_own_property(
                        object,
                        &key,
                        &OrdinaryPropertyDescriptor {
                            get: DescriptorField::Present(get),
                            set: DescriptorField::Present(set),
                            enumerable: DescriptorField::Present(false),
                            configurable: DescriptorField::Present(true),
                            ..OrdinaryPropertyDescriptor::new()
                        },
                    )? {
                        return Err(RuntimeError::Invariant(
                            "native builtin accessor definition was rejected",
                        ));
                    }
                }
                BuiltinEntry::Alias { key, from } => {
                    let from = self.builtin_key(from)?;
                    self.define_builtin_alias(object, realm, object, &from, key)?;
                }
                BuiltinEntry::ToStringTag(value) => self.define_to_string_tag(object, value)?,
                BuiltinEntry::Value {
                    name,
                    value,
                    writable,
                    configurable,
                } => {
                    let value = match value {
                        BuiltinValue::Float(value) => Value::Float(value),
                        BuiltinValue::Undefined => Value::Undefined,
                        BuiltinValue::Symbol(symbol) => {
                            Value::Symbol(self.well_known_symbol(symbol)?)
                        }
                    };
                    self.define_function_data_property(
                        object,
                        name,
                        value,
                        writable,
                        configurable,
                    )?;
                }
                BuiltinEntry::LazyString { name, value } => {
                    self.define_string_auto_init(object, realm, name, value)?;
                }
            }
            rest = &rest[1..];
        }
        Ok(())
    }

    /// QuickJS `JS_ALIAS_DEF`/`JS_ALIAS_BASE_DEF`: read `source[from]`, which
    /// materializes a lazy method, and store that exact function object on
    /// `object` as a writable, configurable, non-enumerable data property.
    pub(crate) fn define_builtin_alias(
        &self,
        object: &ObjectRef,
        realm: ContextId,
        source: &ObjectRef,
        from: &PropertyKey,
        key: BuiltinKey,
    ) -> Result<ObjectId, RuntimeError> {
        let value = match self.get_property_in_realm(realm, source, from)? {
            Completion::Return(JsValue::Object(id)) => {
                ObjectRef::from_owned_handle(self.clone(), id)
            }
            Completion::Return(value) | Completion::Throw(value) => {
                self.release_jsvalue(value)?;
                return Err(RuntimeError::Invariant(
                    "builtin alias target was not a function during bootstrap",
                ));
            }
        };
        let key = self.builtin_key(key)?;
        if !self.define_raw_property(
            object,
            &key,
            &PropertyDescriptor {
                value: Some(RawValue::Object(value.object_id())),
                writable: Some(true),
                enumerable: Some(false),
                configurable: Some(true),
                ..Default::default()
            },
        )? {
            return Err(RuntimeError::Invariant(
                "builtin alias definition was rejected",
            ));
        }
        Ok(value.object_id())
    }

    /// Append an entire table without publishing intermediate layouts.
    ///
    /// This is a bootstrap primitive, not general [[DefineOwnProperty]]. It
    /// accepts only extensible ordinary, native-function, Array, primitive
    /// wrapper and global receivers and new non-index data properties. No
    /// descriptor invokes user code. Validation failures leave the receiver
    /// unchanged. Publication and any subsequent invariant errors retain
    /// `replace_layout`'s existing contract.
    pub(crate) fn define_native_builtin_auto_init_batch(
        &self,
        object: &ObjectRef,
        realm: ContextId,
        methods: impl IntoIterator<Item = NativeBuiltinProperty>,
    ) -> Result<(), RuntimeError> {
        let _operation = self.operation()?;
        // Consume descriptors and intern their keys before borrowing Runtime
        // state. These owning keys outlive the state borrow on every exit.
        let properties = methods
            .into_iter()
            .map(|method| {
                let key = match method.symbol {
                    Some(symbol) => BuiltinKey::Symbol(symbol),
                    None => BuiltinKey::Name(method.name),
                };
                self.builtin_key(key).map(|key| (key, method))
            })
            .collect::<Result<Vec<_>, _>>()?;
        if properties.is_empty() {
            return Ok(());
        }
        self.validate_object_and_key(object, &properties[0].0)?;
        let mut state = self.0.state.borrow_mut();
        state.heap.context(realm)?;
        let object_id = object.object_id();
        let (prototype, mut entries, mut slots) = {
            let object = state.heap.object(object_id)?;
            if !object.extensible
                || !matches!(
                    object.payload,
                    ObjectPayload::Ordinary
                        | ObjectPayload::NativeFunction { .. }
                        | ObjectPayload::Array { .. }
                        | ObjectPayload::Primitive(_)
                        | ObjectPayload::GlobalObject { .. }
                )
            {
                return Err(RuntimeError::Invariant("invalid builtin batch receiver"));
            }
            let shape = state.heap.shape(object.shape)?;
            let mut seen = HashSet::with_capacity(properties.len());
            for (key, method) in &properties {
                if method.flags.storage != PropertyStorageKind::Data
                    || state.atoms.array_index(key.atom())?.is_some()
                    || shape.find(AtomIdx::from_raw(key.atom().raw())).is_some()
                    || !seen.insert(key.atom())
                {
                    return Err(RuntimeError::Invariant(
                        "invalid or duplicate builtin batch property",
                    ));
                }
            }
            (
                shape.prototype(),
                shape.entries().to_vec(),
                object.slots.clone(),
            )
        };
        entries
            .try_reserve(properties.len())
            .map_err(|_| HeapError::Allocation {
                operation: "preparing builtin batch shape entries",
            })?;
        slots
            .try_reserve(properties.len())
            .map_err(|_| HeapError::Allocation {
                operation: "preparing builtin batch property slots",
            })?;
        for (key, method) in &properties {
            entries.push(ShapeEntry {
                atom: AtomIdx::from_raw(key.atom().raw()),
                flags: method.flags,
            });
            slots.push(PropertySlot::auto_init(AutoInitProperty::NativeBuiltin {
                realm,
                target: method.target,
                name: method.name,
                length: method.length,
                min_readable_args: method.min_readable_args,
            }));
        }
        state.replace_layout(object_id, prototype, &entries, slots)
    }
}

impl RuntimeState {
    /// Materialize one lazy `JS_OBJECT_DEF` namespace object in its defining
    /// realm: AutoInit `methods` in table order, then a configurable
    /// `@@toStringTag`, then frozen numeric `constants`.
    pub(crate) fn instantiate_namespace_object(
        &mut self,
        poisoned: &Cell<bool>,
        realm: ContextId,
        methods: &[NativeBuiltinProperty],
        tag: &'static str,
        constants: &[(&'static str, f64)],
    ) -> Result<ObjectId, RuntimeError> {
        let object = self.new_ordinary_object_in_realm(poisoned, realm)?;
        let mut result_owner = OwnedValueGuard::new(self, poisoned, JsValue::Object(object));
        let (state, result_owner) = result_owner.parts();
        for method in methods {
            state.define_native_builtin_auto_init(
                poisoned,
                object,
                realm,
                method.target,
                method.name,
                method.length,
                method.min_readable_args,
            )?;
        }
        let to_string_tag = state.well_known_symbols[&WellKnownSymbol::ToStringTag];
        {
            let string = state.heap.allocate_string(JsString::from_static(tag))?;
            let mut producer = OwnedValueGuard::new(state, poisoned, JsValue::String(string));
            let (state, producer) = producer.parts();
            if !state.define_raw_property_with_poison(
                poisoned,
                object,
                to_string_tag,
                &PropertyDescriptor {
                    value: Some(RawValue::String(string)),
                    writable: Some(false),
                    enumerable: Some(false),
                    configurable: Some(true),
                    ..PropertyDescriptor::new()
                },
            )? {
                return Err(RuntimeError::Invariant(
                    "namespace toStringTag definition was rejected",
                ));
            }
            state.release_owned_jsvalue(
                poisoned,
                producer.take().expect("intrinsic tag producer"),
            )?;
        }
        for &(name, value) in constants {
            let key = state.intern_property_key_js_string(&JsString::try_from_utf8(name)?)?;
            let defined = state.define_fresh_function_data_property(
                poisoned,
                object,
                key,
                &RawValue::Float(value),
                false,
                false,
            );
            if !poisoned.get()
                && let Err(error) = state.atoms.release(key)
            {
                poisoned.set(true);
                return defined.and(Err(error.into()));
            }
            defined?;
        }
        let JsValue::Object(object) = result_owner.take().expect("intrinsic factory result") else {
            unreachable!("intrinsic factory allocated an object")
        };
        Ok(object)
    }
}
