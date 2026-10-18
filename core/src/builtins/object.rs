//! `Object` (phase 4, section 6.1 row 1): the constructor and its static
//! functions, and `Object.prototype` with the Annex B accessors.

use super::{
    Native, accessor, install_constructor, link_constructor, methods, string_text, this_object,
};
use crate::heap::HeapError;
use crate::heap::atom::{KeyKind, PropertyKey};
use crate::heap::object::{ObjectKind, flags};
use crate::runtime::{Intrinsic, Names, WellKnown};
use crate::value::{Kind, ObjectId, Value};
use crate::vm::native::CallSite;
use crate::vm::object::{self as internal, Descriptor};
use crate::vm::{Cx, Throw, error, iter, ops};

pub(super) fn install(cx: &mut Cx<'_>) -> Result<(), HeapError> {
    let prototype = cx.realm.intrinsic(Intrinsic::ObjectPrototype);
    // QuickJS's `js_object_funcs` and `js_object_proto_funcs`, in order.
    let object = install_constructor(cx, Native::ObjectConstructor)?;
    cx.set_intrinsic(Intrinsic::Object, object);
    methods(
        cx,
        object,
        &[
            Native::ObjectCreate,
            Native::ObjectGetPrototypeOf,
            Native::ObjectSetPrototypeOf,
            Native::ObjectDefineProperty,
            Native::ObjectDefineProperties,
            Native::ObjectGetOwnPropertyNames,
            Native::ObjectGetOwnPropertySymbols,
            Native::ObjectGroupBy,
            Native::ObjectKeys,
            Native::ObjectValues,
            Native::ObjectEntries,
            Native::ObjectIsExtensible,
            Native::ObjectPreventExtensions,
            Native::ObjectGetOwnPropertyDescriptor,
            Native::ObjectGetOwnPropertyDescriptors,
            Native::ObjectIs,
            Native::ObjectAssign,
            Native::ObjectSeal,
            Native::ObjectFreeze,
            Native::ObjectIsSealed,
            Native::ObjectIsFrozen,
            Native::ObjectFromEntries,
            Native::ObjectHasOwn,
        ],
    )?;
    methods(
        cx,
        prototype,
        &[
            Native::ObjectToString,
            Native::ObjectToLocaleString,
            Native::ObjectValueOf,
            Native::ObjectHasOwnProperty,
            Native::ObjectIsPrototypeOf,
            Native::ObjectPropertyIsEnumerable,
        ],
    )?;
    let proto = Names::key(cx.realm.names.proto);
    accessor(
        cx,
        prototype,
        proto,
        "__proto__",
        Some(Native::ObjectProtoGetter),
        Some(Native::ObjectProtoSetter),
    )?;
    methods(
        cx,
        prototype,
        &[
            Native::ObjectDefineGetter,
            Native::ObjectDefineSetter,
            Native::ObjectLookupGetter,
            Native::ObjectLookupSetter,
        ],
    )?;
    link_constructor(cx, object, prototype)
}

fn root(cx: &mut Cx<'_>, value: Value) {
    cx.heap.roots().push(value);
}

/// The argument as an object, or a `TypeError`.
fn object_arg(cx: &mut Cx<'_>, value: Value, _what: &str) -> Result<ObjectId, Throw> {
    match value.as_object() {
        Some(object) => Ok(object),
        None => Err(error::type_error(cx, "not an object")),
    }
}

/// A refused `[[SetPrototypeOf]]` on `object`, with QuickJS's reason.
#[cold]
#[inline(never)]
fn prototype_refused(cx: &mut Cx<'_>, object: ObjectId) -> Throw {
    let message = if cx.heap.object_kind(object) == ObjectKind::Proxy {
        "proxy: bad prototype"
    } else if object == cx.realm.intrinsic(Intrinsic::ObjectPrototype) {
        "prototype is immutable"
    } else if !cx
        .heap
        .header(object)
        .has(crate::heap::object::flags::EXTENSIBLE)
    {
        "object is not extensible"
    } else {
        "circular prototype chain"
    };
    error::type_error(cx, message)
}

/// `Object(value)` and `new Object(value)`: a new object for `undefined`
/// and `null` (from `new.target` when a subclass constructs), the value
/// itself for an object, a wrapper for a primitive.
pub(super) fn constructor(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    if site.construct && site.new_target != site.function {
        let prototype =
            internal::prototype_from_constructor(cx, site.new_target, Intrinsic::ObjectPrototype)?;
        return cx
            .heap
            .alloc_object(ObjectKind::Ordinary, Some(prototype), 4)
            .map(Value::object)
            .map_err(|error| error::heap_error(cx, error));
    }
    let value = site.arg(cx, 0);
    match value.kind() {
        Kind::Undefined | Kind::Null => super::new_plain(cx, 4).map(Value::object),
        _ => internal::to_object(cx, value).map(Value::object),
    }
}

/// `Object.prototype.toString`: `[object Tag]`, `Symbol.toStringTag` first.
pub(super) fn to_string(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let this = site.this(cx);
    if this.is_undefined() {
        return string_text(cx, "[object Undefined]");
    }
    if this.is_null() {
        return string_text(cx, "[object Null]");
    }
    let object = this_object(cx, &site)?;
    let builtin = if internal::is_array(cx, Value::object(object))? {
        "Array"
    } else {
        let header = cx.heap.header(object);
        match header.kind {
            ObjectKind::Arguments => "Arguments",
            _ if header.has(flags::CALLABLE) => "Function",
            _ if header.has(flags::ERROR) => "Error",
            ObjectKind::BooleanWrapper => "Boolean",
            ObjectKind::NumberWrapper => "Number",
            ObjectKind::StringWrapper => "String",
            // `%String.prototype%` has `[[StringData]]` "" (it stays an
            // ordinary object here, so the string-receiver cache keeps it).
            _ if object == cx.realm.intrinsic(Intrinsic::StringPrototype) => "String",
            ObjectKind::Date => "Date",
            ObjectKind::RegExp => "RegExp",
            _ => "Object",
        }
    };
    let key = WellKnown::key(cx.realm.symbols.to_string_tag);
    let tag = internal::get_property(cx, object, key)?;
    let mut text: Vec<u16> = "[object ".encode_utf16().collect();
    match tag.as_string() {
        Some(tag) => text.extend(cx.heap.string(tag).repr().units()),
        None => text.extend(builtin.encode_utf16()),
    }
    text.push(u16::from(b']'));
    super::string_units(cx, &text)
}

/// `Object.prototype.toLocaleString`: `this.toString()`.
pub(super) fn to_locale_string(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let this = site.this(cx);
    let key = Names::key(cx.realm.names.to_string);
    internal::invoke(cx, this, key, &[])
}

/// `Object.prototype.valueOf`: `ToObject(this)`.
pub(super) fn value_of(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    this_object(cx, &site).map(Value::object)
}

/// `Object.prototype.hasOwnProperty(key)`: the key converts before the
/// receiver.
pub(super) fn has_own_property(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let key = site.arg(cx, 0);
    let key = ops::to_property_key(cx, key)?;
    root(cx, key.to_value());
    let object = this_object(cx, &site)?;
    internal::has_own_property(cx, object, key).map(Value::bool)
}

/// `Object.prototype.isPrototypeOf(value)`.
pub(super) fn is_prototype_of(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let value = site.arg(cx, 0);
    let Some(mut current) = value.as_object() else {
        return Ok(Value::FALSE);
    };
    let object = this_object(cx, &site)?;
    loop {
        match internal::get_prototype_of(cx, current)?.as_object() {
            None => return Ok(Value::FALSE),
            Some(next) if next == object => return Ok(Value::TRUE),
            Some(next) => current = next,
        }
    }
}

/// `Object.prototype.propertyIsEnumerable(key)`.
pub(super) fn property_is_enumerable(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let key = site.arg(cx, 0);
    let key = ops::to_property_key(cx, key)?;
    root(cx, key.to_value());
    let object = this_object(cx, &site)?;
    let descriptor = internal::get_own_property(cx, object, key)?;
    Ok(Value::bool(descriptor.is_some_and(|descriptor| {
        descriptor.enumerable == Some(true)
    })))
}

/// `get Object.prototype.__proto__`.
pub(super) fn proto_getter(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let object = this_object(cx, &site)?;
    internal::get_prototype_of(cx, object)
}

/// `set Object.prototype.__proto__`.
pub(super) fn proto_setter(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let this = site.this(cx);
    if this.is_undefined() || this.is_null() {
        return Err(error::type_error(cx, "not an object"));
    }
    let prototype = site.arg(cx, 0);
    if prototype.as_object().is_none() && !prototype.is_null() {
        return Ok(Value::UNDEFINED);
    }
    let Some(object) = this.as_object() else {
        return Ok(Value::UNDEFINED);
    };
    if !internal::set_prototype_of(cx, object, prototype)? {
        return Err(prototype_refused(cx, object));
    }
    Ok(Value::UNDEFINED)
}

fn define_accessor_half(cx: &mut Cx<'_>, site: CallSite, setter: bool) -> Result<Value, Throw> {
    let object = this_object(cx, &site)?;
    let function = site.arg(cx, 1);
    if !ops::is_callable(cx.heap, function) {
        return Err(error::type_error(cx, "not a function"));
    }
    let key = site.arg(cx, 0);
    let key = ops::to_property_key(cx, key)?;
    root(cx, key.to_value());
    let descriptor = Descriptor {
        get: (!setter).then_some(function),
        set: setter.then_some(function),
        enumerable: Some(true),
        configurable: Some(true),
        ..Descriptor::default()
    };
    internal::define_property_or_throw(cx, object, key, &descriptor)?;
    Ok(Value::UNDEFINED)
}

pub(super) fn define_getter(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    define_accessor_half(cx, site, false)
}

pub(super) fn define_setter(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    define_accessor_half(cx, site, true)
}

fn lookup_accessor_half(cx: &mut Cx<'_>, site: CallSite, setter: bool) -> Result<Value, Throw> {
    let mut object = this_object(cx, &site)?;
    let key = site.arg(cx, 0);
    let key = ops::to_property_key(cx, key)?;
    root(cx, key.to_value());
    loop {
        if let Some(descriptor) = internal::get_own_property(cx, object, key)? {
            if descriptor.is_accessor() {
                let half = if setter {
                    descriptor.set
                } else {
                    descriptor.get
                };
                return Ok(half.unwrap_or(Value::UNDEFINED));
            }
            return Ok(Value::UNDEFINED);
        }
        match internal::get_prototype_of(cx, object)?.as_object() {
            Some(next) => object = next,
            None => return Ok(Value::UNDEFINED),
        }
    }
}

pub(super) fn lookup_getter(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    lookup_accessor_half(cx, site, false)
}

pub(super) fn lookup_setter(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    lookup_accessor_half(cx, site, true)
}

// ---- static functions ----

/// `Object.assign(target, ...sources)`.
pub(super) fn assign(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let target = site.arg(cx, 0);
    let target = internal::to_object(cx, target)?;
    root(cx, Value::object(target));
    for index in 1..site.argc {
        let source = site.arg(cx, index);
        if source.is_undefined() || source.is_null() {
            continue;
        }
        let from = internal::to_object(cx, source)?;
        root(cx, Value::object(from));
        internal::copy_own_enumerable(cx, from, &[], |cx, key, value| {
            internal::put(cx, target, key, value, true)
        })?;
    }
    Ok(Value::object(target))
}

/// `Object.create(prototype, properties)`.
pub(super) fn create(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let prototype = site.arg(cx, 0);
    if prototype.as_object().is_none() && !prototype.is_null() {
        return Err(error::type_error(cx, "not a prototype"));
    }
    let object = cx
        .heap
        .alloc_object(ObjectKind::Ordinary, prototype.as_object(), 4)
        .map_err(|error| error::heap_error(cx, error))?;
    root(cx, Value::object(object));
    let properties = site.arg(cx, 1);
    if !properties.is_undefined() {
        define_properties_from(cx, object, properties)?;
    }
    Ok(Value::object(object))
}

/// `ObjectDefineProperties(O, Properties)`.
fn define_properties_from(
    cx: &mut Cx<'_>,
    object: ObjectId,
    properties: Value,
) -> Result<(), Throw> {
    let properties = internal::to_object(cx, properties)?;
    root(cx, Value::object(properties));
    // ObjectDefineProperties (20.1.2.3.1): every key's descriptor is read
    // and converted before any is defined.
    let mut descriptors = Vec::new();
    for key in internal::own_property_keys(cx, properties)? {
        root(cx, key.to_value());
        let Some(own) = internal::get_own_property(cx, properties, key)? else {
            continue;
        };
        if own.enumerable != Some(true) {
            continue;
        }
        let value = internal::get(cx, properties, key, Value::object(properties))?;
        root(cx, value);
        let descriptor = internal::to_property_descriptor(cx, value)?;
        for field in [descriptor.value, descriptor.get, descriptor.set]
            .into_iter()
            .flatten()
        {
            root(cx, field);
        }
        descriptors.push((key, descriptor));
    }
    for (key, descriptor) in &descriptors {
        internal::define_property_or_throw(cx, object, *key, descriptor)?;
    }
    Ok(())
}

/// `Object.defineProperties(O, Properties)`.
pub(super) fn define_properties(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let object = site.arg(cx, 0);
    let target = object_arg(cx, object, "Object.defineProperties")?;
    let properties = site.arg(cx, 1);
    define_properties_from(cx, target, properties)?;
    Ok(object)
}

/// `Object.defineProperty(O, P, Attributes)`.
pub(super) fn define_property(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let object = site.arg(cx, 0);
    let target = object_arg(cx, object, "Object.defineProperty")?;
    let key = site.arg(cx, 1);
    let key = ops::to_property_key(cx, key)?;
    root(cx, key.to_value());
    let attributes = site.arg(cx, 2);
    let descriptor = internal::to_property_descriptor(cx, attributes)?;
    internal::define_property_or_throw(cx, target, key, &descriptor)?;
    Ok(object)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Enumerate {
    Keys,
    Values,
    Entries,
}

/// `EnumerableOwnProperties(O, kind)` as an array.
fn enumerable_own(cx: &mut Cx<'_>, site: CallSite, kind: Enumerate) -> Result<Value, Throw> {
    let value = site.arg(cx, 0);
    let object = internal::to_object(cx, value)?;
    root(cx, Value::object(object));
    let mut list = Vec::new();
    for key in internal::own_property_keys(cx, object)? {
        if matches!(key.kind(), KeyKind::Symbol(_)) {
            continue;
        }
        let Some(descriptor) = internal::get_own_property(cx, object, key)? else {
            continue;
        };
        if descriptor.enumerable != Some(true) {
            continue;
        }
        let key_value = key_string(cx, key)?;
        root(cx, key_value);
        let entry = match kind {
            Enumerate::Keys => key_value,
            Enumerate::Values | Enumerate::Entries => {
                let value = internal::get(cx, object, key, Value::object(object))?;
                if kind == Enumerate::Values {
                    value
                } else {
                    root(cx, value);
                    Value::object(internal::array_from_list(cx, &[key_value, value])?)
                }
            }
        };
        root(cx, entry);
        list.push(entry);
    }
    internal::array_from_list(cx, &list).map(Value::object)
}

/// A key as the string a reflective function returns.
pub(crate) fn key_string(cx: &mut Cx<'_>, key: PropertyKey) -> Result<Value, Throw> {
    match key.kind() {
        KeyKind::Index(index) => ops::to_string(cx, Value::number(f64::from(index))),
        _ => Ok(key.to_value()),
    }
}

pub(super) fn keys(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    enumerable_own(cx, site, Enumerate::Keys)
}

pub(super) fn values(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    enumerable_own(cx, site, Enumerate::Values)
}

pub(super) fn entries(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    enumerable_own(cx, site, Enumerate::Entries)
}

pub(super) fn freeze(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    integrity(cx, site, true)
}

pub(super) fn seal(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    integrity(cx, site, false)
}

fn integrity(cx: &mut Cx<'_>, site: CallSite, frozen: bool) -> Result<Value, Throw> {
    let value = site.arg(cx, 0);
    let Some(object) = value.as_object() else {
        return Ok(value);
    };
    if !internal::set_integrity_level(cx, object, frozen)? {
        return Err(error::type_error(
            cx,
            "proxy preventExtensions handler returned false",
        ));
    }
    Ok(value)
}

pub(super) fn is_frozen(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let value = site.arg(cx, 0);
    match value.as_object() {
        Some(object) => internal::test_integrity_level(cx, object, true).map(Value::bool),
        None => Ok(Value::TRUE),
    }
}

pub(super) fn is_sealed(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let value = site.arg(cx, 0);
    match value.as_object() {
        Some(object) => internal::test_integrity_level(cx, object, false).map(Value::bool),
        None => Ok(Value::TRUE),
    }
}

pub(super) fn is_extensible(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let value = site.arg(cx, 0);
    match value.as_object() {
        Some(object) => internal::is_extensible(cx, object).map(Value::bool),
        None => Ok(Value::FALSE),
    }
}

pub(super) fn prevent_extensions(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let value = site.arg(cx, 0);
    if let Some(object) = value.as_object()
        && !internal::prevent_extensions(cx, object)?
    {
        return Err(error::type_error(
            cx,
            "proxy preventExtensions handler returned false",
        ));
    }
    Ok(value)
}

/// `Object.fromEntries(iterable)`.
pub(super) fn from_entries(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let iterable = site.arg(cx, 0);
    if iterable.is_undefined() || iterable.is_null() {
        return Err(error::not_iterable_nullish(cx, iterable));
    }
    let object = super::new_plain(cx, 4)?;
    root(cx, Value::object(object));
    // AddEntriesFromIterable: GetIterator and IteratorStepValue throw
    // without closing; only an entry's reads and the adder close.
    let mut entries = iter::get_iterator(cx, iterable)?;
    while let Some(entry) = iter::step(cx, &mut entries)? {
        let mark = cx.heap.roots().mark();
        root(cx, entry);
        let result = (|| {
            let Some(entry) = entry.as_object() else {
                return Err(error::type_error(cx, "not an object"));
            };
            let key = internal::get_property(cx, entry, PropertyKey::index(0).unwrap())?;
            root(cx, key);
            let value = internal::get_property(cx, entry, PropertyKey::index(1).unwrap())?;
            root(cx, value);
            let key = ops::to_property_key(cx, key)?;
            root(cx, key.to_value());
            internal::create_data_property_or_throw(cx, object, key, value)
        })();
        cx.heap.roots().truncate(mark);
        if let Err(throw) = result {
            iter::close(cx, &entries, Err(throw))?;
        }
    }
    Ok(Value::object(object))
}

/// `Object.getOwnPropertyDescriptor(O, P)`.
pub(super) fn get_own_property_descriptor(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let value = site.arg(cx, 0);
    let object = internal::to_object(cx, value)?;
    root(cx, Value::object(object));
    let key = site.arg(cx, 1);
    let key = ops::to_property_key(cx, key)?;
    root(cx, key.to_value());
    match internal::get_own_property(cx, object, key)? {
        Some(descriptor) => internal::from_property_descriptor(cx, &descriptor),
        None => Ok(Value::UNDEFINED),
    }
}

/// `Object.getOwnPropertyDescriptors(O)`.
pub(super) fn get_own_property_descriptors(
    cx: &mut Cx<'_>,
    site: CallSite,
) -> Result<Value, Throw> {
    let value = site.arg(cx, 0);
    let object = internal::to_object(cx, value)?;
    root(cx, Value::object(object));
    let result = super::new_plain(cx, 4)?;
    root(cx, Value::object(result));
    for key in internal::own_property_keys(cx, object)? {
        if let Some(descriptor) = internal::get_own_property(cx, object, key)? {
            let descriptor = internal::from_property_descriptor(cx, &descriptor)?;
            internal::create_data_property_or_throw(cx, result, key, descriptor)?;
        }
    }
    Ok(Value::object(result))
}

fn own_keys_of(cx: &mut Cx<'_>, site: CallSite, symbols: bool) -> Result<Value, Throw> {
    let value = site.arg(cx, 0);
    let object = internal::to_object(cx, value)?;
    root(cx, Value::object(object));
    let mut list = Vec::new();
    for key in internal::own_property_keys(cx, object)? {
        let is_symbol = matches!(key.kind(), KeyKind::Symbol(_));
        if is_symbol != symbols {
            continue;
        }
        let value = key_string(cx, key)?;
        root(cx, value);
        list.push(value);
    }
    internal::array_from_list(cx, &list).map(Value::object)
}

pub(super) fn get_own_property_names(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    own_keys_of(cx, site, false)
}

pub(super) fn get_own_property_symbols(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    own_keys_of(cx, site, true)
}

pub(super) fn get_prototype_of(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let value = site.arg(cx, 0);
    if value.is_undefined() || value.is_null() {
        return Err(error::type_error(cx, "not an object"));
    }
    let object = internal::to_object(cx, value)?;
    root(cx, Value::object(object));
    internal::get_prototype_of(cx, object)
}

/// `Object.setPrototypeOf(O, proto)`.
pub(super) fn set_prototype_of(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let value = site.arg(cx, 0);
    if value.is_undefined() || value.is_null() {
        return Err(error::type_error(cx, "not an object"));
    }
    let prototype = site.arg(cx, 1);
    if prototype.as_object().is_none() && !prototype.is_null() {
        return Err(error::type_error(cx, "not an object"));
    }
    let Some(object) = value.as_object() else {
        return Ok(value);
    };
    if !internal::set_prototype_of(cx, object, prototype)? {
        return Err(prototype_refused(cx, object));
    }
    Ok(value)
}

/// `Object.hasOwn(O, P)`.
pub(super) fn has_own(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let value = site.arg(cx, 0);
    let object = internal::to_object(cx, value)?;
    root(cx, Value::object(object));
    let key = site.arg(cx, 1);
    let key = ops::to_property_key(cx, key)?;
    root(cx, key.to_value());
    internal::has_own_property(cx, object, key).map(Value::bool)
}

/// `Object.is(a, b)`: `SameValue`.
pub(super) fn is(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let (a, b) = (site.arg(cx, 0), site.arg(cx, 1));
    Ok(Value::bool(internal::same_value(cx, a, b)))
}

/// `Object.groupBy(items, callback)`.
pub(super) fn group_by(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let items = site.arg(cx, 0);
    let callback = site.arg(cx, 1);
    group(cx, items, callback, true, |cx| {
        cx.heap
            .alloc_object(ObjectKind::Ordinary, None, 4)
            .map_err(|error| error::heap_error(cx, error))
    })
}

/// GroupBy (7.3.35) and the groups' object or map from `create`: the
/// callback runs with `undefined` as `this`, each group is a List made into
/// an array at the end (CreateArrayFromList). `property` keys are property
/// keys, otherwise map keys (with -0 normalized).
pub(crate) fn group(
    cx: &mut Cx<'_>,
    items: Value,
    callback: Value,
    property: bool,
    create: impl FnOnce(&mut Cx<'_>) -> Result<ObjectId, Throw>,
) -> Result<Value, Throw> {
    if items.is_undefined() || items.is_null() {
        return Err(error::not_iterable_nullish(cx, items));
    }
    if !ops::is_callable(cx.heap, callback) {
        return Err(error::type_error(cx, "not a function"));
    }
    let mut iterator = iter::get_iterator(cx, items)?;
    // The groups in order of their first member; `index` maps a key to its
    // group, by SameValue of the canonical property key or SameValueZero.
    let index = super::collection::new_collection(
        cx,
        crate::heap::object::InternalTag::Map,
        cx.realm.intrinsic(Intrinsic::MapPrototype),
    )?;
    root(cx, Value::object(index));
    let mut groups: Vec<(Value, Vec<Value>)> = Vec::new();
    let mut k = 0u64;
    while let Some(value) = iter::step(cx, &mut iterator)? {
        root(cx, value);
        let key = crate::vm::call(
            cx,
            callback,
            Value::UNDEFINED,
            &[value, Value::number(k as f64)],
        );
        let key = match key.and_then(|key| {
            if property {
                ops::to_property_key(cx, key).map(PropertyKey::to_value)
            } else if key.as_f64() == Some(0.0) {
                Ok(Value::int32(0))
            } else {
                Ok(key)
            }
        }) {
            Ok(key) => key,
            Err(throw) => {
                iter::close(cx, &iterator, Err(throw))?;
                unreachable!("close rethrows");
            }
        };
        root(cx, key);
        match cx
            .heap
            .table_get(index, key)
            .and_then(|slot| slot.as_int32())
        {
            Some(slot) => groups[slot as usize].1.push(value),
            None => {
                super::collection::set_entry(cx, index, key, Value::int32(groups.len() as i32))?;
                groups.push((key, vec![value]));
            }
        }
        k += 1;
    }
    let result = create(cx)?;
    root(cx, Value::object(result));
    for (key, elements) in groups {
        let array = internal::array_create(cx, 0)?;
        root(cx, Value::object(array));
        for (position, element) in elements.into_iter().enumerate() {
            let element_key = internal::index_key(cx, position as u32)?;
            internal::create_data_property(cx, array, element_key, element)?;
        }
        if property {
            let key = ops::to_property_key(cx, key)?;
            internal::create_data_property(cx, result, key, Value::object(array))?;
        } else {
            super::collection::set_entry(cx, result, key, Value::object(array))?;
        }
    }
    Ok(Value::object(result))
}
