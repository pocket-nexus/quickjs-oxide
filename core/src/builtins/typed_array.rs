//! `%TypedArray%` and the twelve typed-array constructors (phase 4, section
//! 6.1 row 9). A typed array is an `ObjectKind::TypedArray` record over a
//! buffer (`vm::typed`). `Symbol.species` is not consulted (phase 0): the
//! methods that create a typed array make one of the receiver's kind
//! through its intrinsic constructor.

use std::cmp::Ordering;

use super::bigint::to_index;
use super::buffer::{is_buffer, new_array_buffer};
use super::{Native, accessor, getter, methods};
use crate::heap::HeapError;
use crate::heap::buffer::{ElementKind, View};
use crate::heap::object::{InternalData, InternalTag, ObjectKind};
use crate::runtime::{Intrinsic, Names, WellKnown};
use crate::shape::PropertyFlags;
use crate::value::{ObjectId, Value};
use crate::vm::native::CallSite;
use crate::vm::object as internal;
use crate::vm::{Cx, Throw, error, iter, ops, typed};

const fn prototype_intrinsic(kind: ElementKind) -> Intrinsic {
    match kind {
        ElementKind::Int8 => Intrinsic::Int8ArrayPrototype,
        ElementKind::Uint8 => Intrinsic::Uint8ArrayPrototype,
        ElementKind::Uint8Clamped => Intrinsic::Uint8ClampedArrayPrototype,
        ElementKind::Int16 => Intrinsic::Int16ArrayPrototype,
        ElementKind::Uint16 => Intrinsic::Uint16ArrayPrototype,
        ElementKind::Int32 => Intrinsic::Int32ArrayPrototype,
        ElementKind::Uint32 => Intrinsic::Uint32ArrayPrototype,
        ElementKind::Float16 => Intrinsic::Float16ArrayPrototype,
        ElementKind::Float32 => Intrinsic::Float32ArrayPrototype,
        ElementKind::Float64 => Intrinsic::Float64ArrayPrototype,
        ElementKind::BigInt64 => Intrinsic::BigInt64ArrayPrototype,
        ElementKind::BigUint64 => Intrinsic::BigUint64ArrayPrototype,
    }
}

const fn constructor_intrinsic(kind: ElementKind) -> Intrinsic {
    match kind {
        ElementKind::Int8 => Intrinsic::Int8Array,
        ElementKind::Uint8 => Intrinsic::Uint8Array,
        ElementKind::Uint8Clamped => Intrinsic::Uint8ClampedArray,
        ElementKind::Int16 => Intrinsic::Int16Array,
        ElementKind::Uint16 => Intrinsic::Uint16Array,
        ElementKind::Int32 => Intrinsic::Int32Array,
        ElementKind::Uint32 => Intrinsic::Uint32Array,
        ElementKind::Float16 => Intrinsic::Float16Array,
        ElementKind::Float32 => Intrinsic::Float32Array,
        ElementKind::Float64 => Intrinsic::Float64Array,
        ElementKind::BigInt64 => Intrinsic::BigInt64Array,
        ElementKind::BigUint64 => Intrinsic::BigUint64Array,
    }
}

const fn constructor_native(kind: ElementKind) -> Native {
    match kind {
        ElementKind::Int8 => Native::Int8ArrayConstructor,
        ElementKind::Uint8 => Native::Uint8ArrayConstructor,
        ElementKind::Uint8Clamped => Native::Uint8ClampedArrayConstructor,
        ElementKind::Int16 => Native::Int16ArrayConstructor,
        ElementKind::Uint16 => Native::Uint16ArrayConstructor,
        ElementKind::Int32 => Native::Int32ArrayConstructor,
        ElementKind::Uint32 => Native::Uint32ArrayConstructor,
        ElementKind::Float16 => Native::Float16ArrayConstructor,
        ElementKind::Float32 => Native::Float32ArrayConstructor,
        ElementKind::Float64 => Native::Float64ArrayConstructor,
        ElementKind::BigInt64 => Native::BigInt64ArrayConstructor,
        ElementKind::BigUint64 => Native::BigUint64ArrayConstructor,
    }
}

pub(super) fn install(cx: &mut Cx<'_>) -> Result<(), HeapError> {
    let object_prototype = cx.realm.intrinsic(Intrinsic::ObjectPrototype);
    let prototype = super::plain_object(cx, Some(object_prototype), 48)?;
    cx.set_intrinsic(Intrinsic::TypedArrayPrototype, prototype);
    // QuickJS's `js_typed_array_base_funcs` and
    // `js_typed_array_base_proto_funcs`, in order; `toString` follows the
    // constructor link.
    let constructor = super::native_function(cx, Native::TypedArrayConstructor)?;
    cx.set_intrinsic(Intrinsic::TypedArray, constructor);
    let names = cx.realm.names;
    methods(
        cx,
        constructor,
        &[Native::TypedArrayFrom, Native::TypedArrayOf],
    )?;
    let species = WellKnown::key(cx.realm.symbols.species);
    accessor(
        cx,
        constructor,
        species,
        "[Symbol.species]",
        Some(Native::SpeciesGetter),
        None,
    )?;
    getter(cx, prototype, "length", Native::TypedArrayLength)?;
    methods(
        cx,
        prototype,
        &[Native::TypedArrayAt, Native::TypedArrayWith],
    )?;
    getter(cx, prototype, "buffer", Native::TypedArrayBuffer)?;
    getter(cx, prototype, "byteLength", Native::TypedArrayByteLength)?;
    getter(cx, prototype, "byteOffset", Native::TypedArrayByteOffset)?;
    methods(
        cx,
        prototype,
        &[Native::TypedArraySet, Native::TypedArrayValues],
    )?;
    // `@@iterator` is `values`.
    let values = cx.heap.property_key("values")?;
    let (slot, _) = cx.heap.lookup_own(prototype, values).expect("values");
    let function = cx.heap.get_slot(prototype, slot);
    super::define_key(
        cx,
        prototype,
        WellKnown::key(cx.realm.symbols.iterator),
        function,
        super::hidden(),
    )?;
    methods(
        cx,
        prototype,
        &[Native::TypedArrayKeys, Native::TypedArrayEntries],
    )?;
    let tag = WellKnown::key(cx.realm.symbols.to_string_tag);
    accessor(
        cx,
        prototype,
        tag,
        "[Symbol.toStringTag]",
        Some(Native::TypedArrayToStringTag),
        None,
    )?;
    methods(
        cx,
        prototype,
        &[
            Native::TypedArrayCopyWithin,
            Native::TypedArrayEvery,
            Native::TypedArraySome,
            Native::TypedArrayForEach,
            Native::TypedArrayMap,
            Native::TypedArrayFilter,
            Native::TypedArrayReduce,
            Native::TypedArrayReduceRight,
            Native::TypedArrayFill,
            Native::TypedArrayFind,
            Native::TypedArrayFindIndex,
            Native::TypedArrayFindLast,
            Native::TypedArrayFindLastIndex,
            Native::TypedArrayReverse,
            Native::TypedArrayToReversed,
            Native::TypedArraySlice,
            Native::TypedArraySubarray,
            Native::TypedArraySort,
            Native::TypedArrayToSorted,
            Native::TypedArrayJoin,
            Native::TypedArrayToLocaleString,
            Native::TypedArrayIndexOf,
            Native::TypedArrayLastIndexOf,
            Native::TypedArrayIncludes,
        ],
    )?;
    super::link_constructor(cx, constructor, prototype)?;
    // `toString` is `Array.prototype.toString`.
    let array_prototype = cx.realm.intrinsic(Intrinsic::ArrayPrototype);
    let to_string = Names::key(names.to_string);
    let (slot, _) = cx
        .heap
        .lookup_own(array_prototype, to_string)
        .expect("Array.prototype.toString");
    let function = cx.heap.get_slot(array_prototype, slot);
    super::define_key(cx, prototype, to_string, function, super::hidden())?;

    let constant = PropertyFlags::data(false, false, false);
    for kind in ElementKind::ALL {
        let concrete = super::plain_object(cx, Some(prototype), 4)?;
        cx.set_intrinsic(prototype_intrinsic(kind), concrete);
        let function = super::install_constructor(cx, constructor_native(kind))?;
        cx.set_intrinsic(constructor_intrinsic(kind), function);
        cx.heap.set_prototype(function, Some(constructor))?;
        let size = Value::int32(kind.size() as i32);
        super::define(cx, function, "BYTES_PER_ELEMENT", size, constant)?;
        super::define(cx, concrete, "BYTES_PER_ELEMENT", size, constant)?;
        if kind == ElementKind::Uint8 {
            methods(
                cx,
                function,
                &[Native::Uint8ArrayFromBase64, Native::Uint8ArrayFromHex],
            )?;
            methods(
                cx,
                concrete,
                &[
                    Native::Uint8ArrayToBase64,
                    Native::Uint8ArrayToHex,
                    Native::Uint8ArraySetFromBase64,
                    Native::Uint8ArraySetFromHex,
                ],
            )?;
        }
        super::link_constructor(cx, function, concrete)?;
    }
    Ok(())
}

fn root(cx: &mut Cx<'_>, value: Value) {
    cx.heap.roots().push(value);
}

// ---- creation ----

/// A typed array of `kind` over `buffer`, rooted.
fn alloc_view(
    cx: &mut Cx<'_>,
    kind: ElementKind,
    prototype: ObjectId,
    buffer: ObjectId,
    byte_offset: usize,
    length: Option<usize>,
) -> Result<ObjectId, Throw> {
    let view = View {
        kind,
        byte_offset,
        length,
    };
    let object = cx
        .heap
        .alloc_internal(
            ObjectKind::TypedArray,
            InternalTag::TypedArray,
            Some(prototype),
            0,
            &[Value::object(buffer)],
            InternalData::View(view),
            0,
        )
        .map_err(|error| error::heap_error(cx, error))?;
    root(cx, Value::object(object));
    Ok(object)
}

/// `AllocateTypedArray` with a new zeroed buffer of `length` elements.
fn allocate(
    cx: &mut Cx<'_>,
    kind: ElementKind,
    prototype: ObjectId,
    length: u64,
) -> Result<ObjectId, Throw> {
    let bytes = length.saturating_mul(kind.size() as u64);
    if bytes > crate::heap::buffer::MAX_BUFFER_LENGTH as u64 {
        return Err(error::range_error(cx, "invalid array buffer length"));
    }
    root(cx, Value::object(prototype));
    let buffer = new_array_buffer(cx, bytes as usize)?;
    alloc_view(cx, kind, prototype, buffer, 0, Some(length as usize))
}

/// A new typed array of `kind` with the intrinsic prototype.
pub(crate) fn create(cx: &mut Cx<'_>, kind: ElementKind, length: u64) -> Result<ObjectId, Throw> {
    let prototype = cx.realm.intrinsic(prototype_intrinsic(kind));
    allocate(cx, kind, prototype, length)
}

fn kind_of(cx: &Cx<'_>, object: ObjectId) -> ElementKind {
    typed::view(cx.heap, object).0.kind
}

/// `ValidateTypedArray(O)`: the typed array and its length.
fn validate(cx: &mut Cx<'_>, value: Value) -> Result<(ObjectId, usize), Throw> {
    let Some(object) = value
        .as_object()
        .filter(|&object| cx.heap.object_kind(object) == ObjectKind::TypedArray)
    else {
        return Err(error::type_error(cx, "not a TypedArray"));
    };
    match typed::length(cx.heap, object) {
        Some(length) => Ok((object, length)),
        None => Err(error::type_error(cx, OUT_OF_BOUNDS)),
    }
}

/// QuickJS's `JS_ThrowTypeErrorArrayBufferOOB`.
const OUT_OF_BOUNDS: &str = "ArrayBuffer is detached or resized";

/// [`this_typed`] for `at` and `with`, which QuickJS reports as detached
/// even when the array is only out of bounds.
fn this_typed_detached(cx: &mut Cx<'_>, site: &CallSite) -> Result<(ObjectId, usize), Throw> {
    let this = site.this(cx);
    if let Some(object) = this
        .as_object()
        .filter(|&object| cx.heap.object_kind(object) == ObjectKind::TypedArray)
        && typed::length(cx.heap, object).is_none()
    {
        return Err(error::type_error(cx, "ArrayBuffer is detached"));
    }
    validate(cx, this)
}

fn this_typed(cx: &mut Cx<'_>, site: &CallSite) -> Result<(ObjectId, usize), Throw> {
    let this = site.this(cx);
    validate(cx, this)
}

/// `TypedArrayCreateFromConstructor(C, [length])`, checking the result.
fn create_from_constructor(
    cx: &mut Cx<'_>,
    constructor: Value,
    length: u64,
) -> Result<ObjectId, Throw> {
    if !internal::is_constructor(cx, constructor) {
        return Err(error::construct_error(cx, constructor));
    }
    let result = crate::vm::construct(
        cx,
        constructor,
        &[Value::number(length as f64)],
        constructor,
    )?;
    root(cx, result);
    let (object, actual) = validate(cx, result)?;
    if (actual as u64) < length {
        return Err(error::type_error(cx, "TypedArray length is too small"));
    }
    Ok(object)
}

/// A typed array of the receiver's kind (`TypedArraySpeciesCreate` with the
/// species dropped, `TypedArrayCreateSameType`).
fn same_type(cx: &mut Cx<'_>, exemplar: ObjectId, length: u64) -> Result<ObjectId, Throw> {
    let kind = kind_of(cx, exemplar);
    create(cx, kind, length)
}

fn get(cx: &mut Cx<'_>, object: ObjectId, index: usize) -> Result<Value, Throw> {
    Ok(typed::get_element(cx, object, index as f64)?.unwrap_or(Value::UNDEFINED))
}

fn set(cx: &mut Cx<'_>, object: ObjectId, index: usize, value: Value) -> Result<(), Throw> {
    typed::set_element(cx, object, index as f64, value)
}

// ---- the constructors ----

pub(super) fn abstract_constructor(cx: &mut Cx<'_>, _site: CallSite) -> Result<Value, Throw> {
    Err(error::type_error(cx, "cannot be called"))
}

fn construct(cx: &mut Cx<'_>, site: CallSite, kind: ElementKind) -> Result<Value, Throw> {
    if !site.construct {
        return Err(error::type_error(cx, "must be called with new"));
    }
    let first = site.arg(cx, 0);
    let default = prototype_intrinsic(kind);
    let Some(source) = first.as_object() else {
        let length = to_index(cx, first)?;
        let prototype = internal::prototype_from_constructor(cx, site.new_target, default)?;
        return allocate(cx, kind, prototype, length).map(Value::object);
    };
    let prototype = internal::prototype_from_constructor(cx, site.new_target, default)?;
    root(cx, Value::object(prototype));
    if cx.heap.object_kind(source) == ObjectKind::TypedArray {
        return from_typed_array(cx, kind, prototype, source).map(Value::object);
    }
    if is_buffer(cx, first) {
        let (offset, length) = (site.arg(cx, 1), site.arg(cx, 2));
        return from_buffer(cx, kind, prototype, source, offset, length).map(Value::object);
    }
    let key = WellKnown::key(cx.realm.symbols.iterator);
    let values = match internal::get_method_or(cx, first, key, "not a function")? {
        Some(method) => {
            root(cx, method);
            let mut record = if method == Value::object(cx.realm.intrinsic(Intrinsic::ArrayValues))
                && iter::is_pristine_array(cx, first)
            {
                iter::get_iterator(cx, first)?
            } else {
                iter::from_method(cx, first, method)?
            };
            let mut values = Vec::new();
            while let Some(value) = iter::step(cx, &mut record)? {
                root(cx, value);
                values.push(value);
            }
            values
        }
        None => {
            let length = internal::length_of_array_like(cx, source)?;
            let object = allocate(cx, kind, prototype, length)?;
            for index in 0..length {
                let key = internal::number_key(cx, index as f64)?;
                let value = internal::get_property(cx, source, key)?;
                set(cx, object, index as usize, value)?;
            }
            return Ok(Value::object(object));
        }
    };
    let object = allocate(cx, kind, prototype, values.len() as u64)?;
    for (index, value) in values.into_iter().enumerate() {
        set(cx, object, index, value)?;
    }
    Ok(Value::object(object))
}

/// A copy between BigInt and number content: QuickJS's element store into
/// a `target` of that kind fails.
#[cold]
#[inline(never)]
fn content_type_error(cx: &mut Cx<'_>, target: ElementKind) -> Throw {
    if target.is_bigint() {
        error::type_error(cx, "cannot convert to bigint")
    } else {
        error::type_error(cx, "cannot convert bigint to number")
    }
}

/// `InitializeTypedArrayFromTypedArray`.
fn from_typed_array(
    cx: &mut Cx<'_>,
    kind: ElementKind,
    prototype: ObjectId,
    source: ObjectId,
) -> Result<ObjectId, Throw> {
    let Some(length) = typed::length(cx.heap, source) else {
        return Err(error::type_error(cx, OUT_OF_BOUNDS));
    };
    let source_kind = kind_of(cx, source);
    if source_kind.is_bigint() != kind.is_bigint() {
        return Err(content_type_error(cx, kind));
    }
    let object = allocate(cx, kind, prototype, length as u64)?;
    copy_elements(cx, source, 0, object, 0, length)?;
    Ok(object)
}

/// Copies `count` elements between typed arrays of the same content type,
/// by bytes when the kinds agree (through a copy if they share a buffer).
fn copy_elements(
    cx: &mut Cx<'_>,
    source: ObjectId,
    from: usize,
    target: ObjectId,
    to: usize,
    count: usize,
) -> Result<(), Throw> {
    let (source_view, source_buffer) = typed::view(cx.heap, source);
    let (target_view, target_buffer) = typed::view(cx.heap, target);
    if source_view.kind == target_view.kind {
        let size = source_view.kind.size();
        let bytes = typed::buffer(cx.heap, source_buffer)
            .to_vec(source_view.byte_offset + from * size, count * size);
        typed::buffer_mut(cx.heap, target_buffer)
            .write(target_view.byte_offset + to * size, &bytes);
        return Ok(());
    }
    let mut values = Vec::with_capacity(count);
    for index in 0..count {
        let value = get(cx, source, from + index)?;
        root(cx, value);
        values.push(value);
    }
    for (index, value) in values.into_iter().enumerate() {
        set(cx, target, to + index, value)?;
    }
    Ok(())
}

/// `InitializeTypedArrayFromArrayBuffer`.
fn from_buffer(
    cx: &mut Cx<'_>,
    kind: ElementKind,
    prototype: ObjectId,
    buffer: ObjectId,
    offset: Value,
    length: Value,
) -> Result<ObjectId, Throw> {
    let size = kind.size();
    let offset = to_index(cx, offset)? as usize;
    if !offset.is_multiple_of(size) {
        return Err(error::range_error(cx, "invalid offset"));
    }
    let new_length = if length.is_undefined() {
        None
    } else {
        Some(to_index(cx, length)? as usize)
    };
    let record = typed::buffer(cx.heap, buffer);
    if record.detached {
        return Err(error::type_error(cx, "ArrayBuffer is detached"));
    }
    let buffer_length = record.len();
    // `IsFixedLengthArrayBuffer`: a growable shared buffer is not.
    let fixed = record.max.is_none();
    let length = match new_length {
        None if !fixed => {
            if offset > buffer_length {
                return Err(error::range_error(cx, "invalid offset"));
            }
            None
        }
        None => {
            if offset > buffer_length {
                return Err(error::range_error(cx, "invalid offset"));
            }
            if !buffer_length.is_multiple_of(size) {
                return Err(error::range_error(cx, "invalid length"));
            }
            Some((buffer_length - offset) / size)
        }
        Some(length) => {
            if offset + length * size > buffer_length {
                return Err(error::range_error(cx, "invalid length"));
            }
            Some(length)
        }
    };
    alloc_view(cx, kind, prototype, buffer, offset, length)
}

macro_rules! constructors {
    ($($function:ident => $kind:ident;)*) => {
        $(
            pub(super) fn $function(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
                construct(cx, site, ElementKind::$kind)
            }
        )*
    };
}

constructors! {
    int8_array => Int8;
    uint8_array => Uint8;
    uint8_clamped_array => Uint8Clamped;
    int16_array => Int16;
    uint16_array => Uint16;
    int32_array => Int32;
    uint32_array => Uint32;
    float16_array => Float16;
    float32_array => Float32;
    float64_array => Float64;
    big_int64_array => BigInt64;
    big_uint64_array => BigUint64;
}

// ---- statics ----

/// `%TypedArray%.from(source, mapfn, thisArg)`.
pub(super) fn from(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let constructor = site.this(cx);
    let (source, mapper, this_arg) = (site.arg(cx, 0), site.arg(cx, 1), site.arg(cx, 2));
    // 23.2.2.1 steps 2–3: the constructor, then the mapper.
    if !internal::is_constructor(cx, constructor) {
        return Err(error::construct_error(cx, constructor));
    }
    if !mapper.is_undefined() && !ops::is_callable(cx.heap, mapper) {
        return Err(error::type_error(cx, "not a function"));
    }
    let key = WellKnown::key(cx.realm.symbols.iterator);
    let values = match internal::get_method(cx, source, key)? {
        Some(method) => {
            root(cx, method);
            let mut record = iter::from_method(cx, source, method)?;
            let mut values = Vec::new();
            while let Some(value) = iter::step(cx, &mut record)? {
                root(cx, value);
                values.push(value);
            }
            Some(values)
        }
        None => None,
    };
    let map = |cx: &mut Cx<'_>, value: Value, index: usize| -> Result<Value, Throw> {
        if mapper.is_undefined() {
            Ok(value)
        } else {
            crate::vm::call(cx, mapper, this_arg, &[value, Value::number(index as f64)])
        }
    };
    if let Some(values) = values {
        let target = create_from_constructor(cx, constructor, values.len() as u64)?;
        for (index, value) in values.into_iter().enumerate() {
            let value = map(cx, value, index)?;
            root(cx, value);
            let key = internal::number_key(cx, index as f64)?;
            internal::put(cx, target, key, value, true)?;
        }
        return Ok(Value::object(target));
    }
    let array_like = internal::to_object(cx, source)?;
    root(cx, Value::object(array_like));
    let length = internal::length_of_array_like(cx, array_like)?;
    let target = create_from_constructor(cx, constructor, length)?;
    for index in 0..length {
        let key = internal::number_key(cx, index as f64)?;
        let value = internal::get_property(cx, array_like, key)?;
        let value = map(cx, value, index as usize)?;
        root(cx, value);
        internal::put(cx, target, key, value, true)?;
    }
    Ok(Value::object(target))
}

/// `%TypedArray%.of(...items)`.
pub(super) fn of(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let constructor = site.this(cx);
    if !internal::is_constructor(cx, constructor) {
        return Err(error::construct_error(cx, constructor));
    }
    let target = create_from_constructor(cx, constructor, site.argc as u64)?;
    for index in 0..site.argc {
        let value = site.arg(cx, index);
        let key = internal::number_key(cx, index as f64)?;
        internal::put(cx, target, key, value, true)?;
    }
    Ok(Value::object(target))
}

// ---- accessors ----

fn this_any(cx: &mut Cx<'_>, site: &CallSite) -> Result<ObjectId, Throw> {
    let this = site.this(cx);
    match this.as_object() {
        Some(object) if cx.heap.object_kind(object) == ObjectKind::TypedArray => Ok(object),
        _ => Err(error::type_error(cx, "not a TypedArray")),
    }
}

pub(super) fn buffer_getter(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let object = this_any(cx, &site)?;
    Ok(cx.heap.internal_slot(object, 0))
}

pub(super) fn byte_length(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let object = this_any(cx, &site)?;
    let length = typed::length(cx.heap, object).unwrap_or(0);
    Ok(Value::number((length * kind_of(cx, object).size()) as f64))
}

pub(super) fn byte_offset(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let object = this_any(cx, &site)?;
    if typed::length(cx.heap, object).is_none() {
        return Ok(Value::int32(0));
    }
    Ok(Value::number(
        typed::view(cx.heap, object).0.byte_offset as f64,
    ))
}

pub(super) fn length(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let object = this_any(cx, &site)?;
    Ok(Value::number(
        typed::length(cx.heap, object).unwrap_or(0) as f64
    ))
}

pub(super) fn to_string_tag_getter(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let this = site.this(cx);
    match this.as_object() {
        Some(object) if cx.heap.object_kind(object) == ObjectKind::TypedArray => {
            let name = kind_of(cx, object).name();
            super::string_text(cx, name)
        }
        _ => Ok(Value::UNDEFINED),
    }
}

// ---- Array-like methods ----

fn callback(cx: &mut Cx<'_>, site: &CallSite) -> Result<Value, Throw> {
    let callback = site.arg(cx, 0);
    if !ops::is_callable(cx.heap, callback) {
        return Err(error::type_error(cx, "not a function"));
    }
    Ok(callback)
}

/// A relative index argument, resolved against `length` (`default` when
/// undefined).
fn relative(cx: &mut Cx<'_>, value: Value, length: usize, default: usize) -> Result<usize, Throw> {
    if value.is_undefined() {
        return Ok(default);
    }
    let relative = internal::to_integer_or_infinity(cx, value)?;
    Ok(super::relative(relative, length))
}

pub(super) fn at(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let (object, length) = this_typed_detached(cx, &site)?;
    let index = site.arg(cx, 0);
    let index = internal::to_integer_or_infinity(cx, index)?;
    let index = if index < 0.0 {
        length as f64 + index
    } else {
        index
    };
    if index < 0.0 || index >= length as f64 {
        return Ok(Value::UNDEFINED);
    }
    get(cx, object, index as usize)
}

pub(super) fn copy_within(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let (object, length) = this_typed(cx, &site)?;
    let (target, start, end) = (site.arg(cx, 0), site.arg(cx, 1), site.arg(cx, 2));
    let to = relative(cx, target, length, 0)?;
    let from = relative(cx, start, length, 0)?;
    let last = relative(cx, end, length, length)?;
    let count = last.saturating_sub(from).min(length - to);
    if count > 0 {
        let Some(length) = typed::length(cx.heap, object) else {
            return Err(error::type_error(cx, OUT_OF_BOUNDS));
        };
        let (view, buffer) = typed::view(cx.heap, object);
        let size = view.kind.size();
        // The conversions may have shrunk the array.
        let count = count
            .min(length.saturating_sub(from))
            .min(length.saturating_sub(to));
        if count > 0 {
            typed::buffer_mut(cx.heap, buffer).copy_within(
                view.byte_offset + from * size,
                view.byte_offset + to * size,
                count * size,
            );
        }
    }
    Ok(Value::object(object))
}

pub(super) fn entries(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let (object, _) = this_typed(cx, &site)?;
    super::iterator::create_array_iterator(
        cx,
        Value::object(object),
        super::iterator::ArrayIterKind::Entries,
    )
}

pub(super) fn keys(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let (object, _) = this_typed(cx, &site)?;
    super::iterator::create_array_iterator(
        cx,
        Value::object(object),
        super::iterator::ArrayIterKind::Keys,
    )
}

pub(super) fn values(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let (object, _) = this_typed(cx, &site)?;
    super::iterator::create_array_iterator(
        cx,
        Value::object(object),
        super::iterator::ArrayIterKind::Values,
    )
}

/// What a callback walk does with the results.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Walk {
    ForEach,
    Every,
    Some,
    Find,
    FindIndex,
    FindLast,
    FindLastIndex,
}

fn each(cx: &mut Cx<'_>, site: CallSite, kind: Walk) -> Result<Value, Throw> {
    let (object, length) = this_typed(cx, &site)?;
    let callback = callback(cx, &site)?;
    let this_arg = site.arg(cx, 1);
    let reverse = matches!(kind, Walk::FindLast | Walk::FindLastIndex);
    for step in 0..length {
        let index = if reverse { length - 1 - step } else { step };
        let value = get(cx, object, index)?;
        let mark = cx.heap.roots().mark();
        root(cx, value);
        let result = crate::vm::call(
            cx,
            callback,
            this_arg,
            &[value, Value::number(index as f64), Value::object(object)],
        );
        cx.heap.roots().truncate(mark);
        let truthy = ops::to_boolean(cx.heap, result?);
        match kind {
            Walk::ForEach => {}
            Walk::Every if !truthy => return Ok(Value::FALSE),
            Walk::Some if truthy => return Ok(Value::TRUE),
            Walk::Find | Walk::FindLast if truthy => return Ok(value),
            Walk::FindIndex | Walk::FindLastIndex if truthy => {
                return Ok(Value::number(index as f64));
            }
            _ => {}
        }
    }
    Ok(match kind {
        Walk::ForEach | Walk::Find | Walk::FindLast => Value::UNDEFINED,
        Walk::Every => Value::TRUE,
        Walk::Some => Value::FALSE,
        Walk::FindIndex | Walk::FindLastIndex => Value::int32(-1),
    })
}

pub(super) fn every(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    each(cx, site, Walk::Every)
}

pub(super) fn some(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    each(cx, site, Walk::Some)
}

pub(super) fn for_each(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    each(cx, site, Walk::ForEach)
}

pub(super) fn find(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    each(cx, site, Walk::Find)
}

pub(super) fn find_index(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    each(cx, site, Walk::FindIndex)
}

pub(super) fn find_last(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    each(cx, site, Walk::FindLast)
}

pub(super) fn find_last_index(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    each(cx, site, Walk::FindLastIndex)
}

pub(super) fn fill(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let (object, length) = this_typed(cx, &site)?;
    let kind = kind_of(cx, object);
    let value = site.arg(cx, 0);
    let bytes = typed::convert(cx, kind, value)?;
    let (start, end) = (site.arg(cx, 1), site.arg(cx, 2));
    let first = relative(cx, start, length, 0)?;
    let last = relative(cx, end, length, length)?;
    let Some(length) = typed::length(cx.heap, object) else {
        return Err(error::type_error(cx, OUT_OF_BOUNDS));
    };
    let last = last.min(length);
    let (view, buffer) = typed::view(cx.heap, object);
    let size = kind.size();
    let store = typed::buffer_mut(cx.heap, buffer);
    for index in first..last {
        store.write(view.byte_offset + index * size, &bytes[..size]);
    }
    Ok(Value::object(object))
}

pub(super) fn filter(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let (object, length) = this_typed(cx, &site)?;
    let callback = callback(cx, &site)?;
    let this_arg = site.arg(cx, 1);
    let mut kept = Vec::new();
    for index in 0..length {
        let value = get(cx, object, index)?;
        root(cx, value);
        let selected = crate::vm::call(
            cx,
            callback,
            this_arg,
            &[value, Value::number(index as f64), Value::object(object)],
        )?;
        if ops::to_boolean(cx.heap, selected) {
            kept.push(value);
        }
    }
    let result = same_type(cx, object, kept.len() as u64)?;
    for (index, value) in kept.into_iter().enumerate() {
        set(cx, result, index, value)?;
    }
    Ok(Value::object(result))
}

/// `includes`, `indexOf` and `lastIndexOf`.
fn search(cx: &mut Cx<'_>, site: CallSite, mode: u8) -> Result<Value, Throw> {
    let (object, length) = this_typed(cx, &site)?;
    let not_found = if mode == 0 {
        Value::FALSE
    } else {
        Value::int32(-1)
    };
    if length == 0 {
        return Ok(not_found);
    }
    let target = site.arg(cx, 0);
    let from = site.arg(cx, 1);
    if mode == 2 {
        let start = if site.argc > 1 {
            let relative = internal::to_integer_or_infinity(cx, from)?;
            if relative == f64::NEG_INFINITY {
                return Ok(not_found);
            }
            if relative >= 0.0 {
                relative.min(length as f64 - 1.0)
            } else {
                length as f64 + relative
            }
        } else {
            length as f64 - 1.0
        };
        let mut index = start;
        while index >= 0.0 {
            if let Some(value) = typed::get_element(cx, object, index)?
                && ops::strict_eq(cx.heap, value, target)
            {
                return Ok(Value::number(index));
            }
            index -= 1.0;
        }
        return Ok(not_found);
    }
    let relative = internal::to_integer_or_infinity(cx, from)?;
    if relative == f64::INFINITY {
        return Ok(not_found);
    }
    let mut index = if relative >= 0.0 {
        relative
    } else {
        (length as f64 + relative).max(0.0)
    };
    while index < length as f64 {
        let value = typed::get_element(cx, object, index)?;
        let found = match value {
            Some(value) if mode == 0 => internal::same_value_zero(cx, value, target),
            Some(value) => ops::strict_eq(cx.heap, value, target),
            None => mode == 0 && target.is_undefined(),
        };
        if found {
            return Ok(if mode == 0 {
                Value::TRUE
            } else {
                Value::number(index)
            });
        }
        index += 1.0;
    }
    Ok(not_found)
}

pub(super) fn includes(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    search(cx, site, 0)
}

pub(super) fn index_of(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    search(cx, site, 1)
}

pub(super) fn last_index_of(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    search(cx, site, 2)
}

pub(super) fn join(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let (object, length) = this_typed(cx, &site)?;
    let separator = site.arg(cx, 0);
    let separator: Vec<u16> = if separator.is_undefined() {
        vec![u16::from(b',')]
    } else {
        let separator = super::to_string_id(cx, separator)?;
        ops::units(cx.heap, separator)
    };
    super::array::join_with(cx, object, length as u64, &separator)
}

pub(super) fn map(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let (object, length) = this_typed(cx, &site)?;
    let callback = callback(cx, &site)?;
    let this_arg = site.arg(cx, 1);
    let result = same_type(cx, object, length as u64)?;
    for index in 0..length {
        let value = get(cx, object, index)?;
        root(cx, value);
        let mapped = crate::vm::call(
            cx,
            callback,
            this_arg,
            &[value, Value::number(index as f64), Value::object(object)],
        )?;
        root(cx, mapped);
        set(cx, result, index, mapped)?;
    }
    Ok(Value::object(result))
}

fn reduce_impl(cx: &mut Cx<'_>, site: CallSite, right: bool) -> Result<Value, Throw> {
    let (object, length) = this_typed(cx, &site)?;
    let callback = callback(cx, &site)?;
    let mut indices: Box<dyn Iterator<Item = usize>> = if right {
        Box::new((0..length).rev())
    } else {
        Box::new(0..length)
    };
    let mut accumulator = if site.argc >= 2 {
        site.arg(cx, 1)
    } else {
        match indices.next() {
            Some(index) => get(cx, object, index)?,
            None => return Err(error::type_error(cx, "empty array")),
        }
    };
    root(cx, accumulator);
    for index in indices {
        let value = get(cx, object, index)?;
        root(cx, value);
        accumulator = crate::vm::call(
            cx,
            callback,
            Value::UNDEFINED,
            &[
                accumulator,
                value,
                Value::number(index as f64),
                Value::object(object),
            ],
        )?;
        root(cx, accumulator);
    }
    Ok(accumulator)
}

pub(super) fn reduce(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    reduce_impl(cx, site, false)
}

pub(super) fn reduce_right(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    reduce_impl(cx, site, true)
}

pub(super) fn reverse(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let (object, length) = this_typed(cx, &site)?;
    let (view, buffer) = typed::view(cx.heap, object);
    let size = view.kind.size();
    let store = typed::buffer_mut(cx.heap, buffer);
    let (mut low, mut high) = (0, length);
    let mut a = [0u8; 8];
    let mut b = [0u8; 8];
    while low + 1 < high {
        high -= 1;
        store.read(view.byte_offset + low * size, &mut a[..size]);
        store.read(view.byte_offset + high * size, &mut b[..size]);
        store.write(view.byte_offset + low * size, &b[..size]);
        store.write(view.byte_offset + high * size, &a[..size]);
        low += 1;
    }
    Ok(Value::object(object))
}

/// `set(source, offset)`.
pub(super) fn set_method(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let target = site.this(cx);
    let Some(target) = target
        .as_object()
        .filter(|&object| cx.heap.object_kind(object) == ObjectKind::TypedArray)
    else {
        return Err(error::type_error(cx, "not a TypedArray"));
    };
    let source = site.arg(cx, 0);
    let offset = site.arg(cx, 1);
    let offset = internal::to_integer_or_infinity(cx, offset)?;
    if offset < 0.0 {
        return Err(error::range_error(cx, "invalid array length"));
    }
    let Some(target_length) = typed::length(cx.heap, target) else {
        return Err(error::type_error(cx, OUT_OF_BOUNDS));
    };
    if let Some(source) = source
        .as_object()
        .filter(|&object| cx.heap.object_kind(object) == ObjectKind::TypedArray)
    {
        let Some(source_length) = typed::length(cx.heap, source) else {
            return Err(error::type_error(cx, OUT_OF_BOUNDS));
        };
        if kind_of(cx, source).is_bigint() != kind_of(cx, target).is_bigint() {
            let kind = kind_of(cx, target);
            return Err(content_type_error(cx, kind));
        }
        if offset == f64::INFINITY || source_length as f64 + offset > target_length as f64 {
            return Err(error::range_error(cx, "invalid array length"));
        }
        copy_elements(cx, source, 0, target, offset as usize, source_length)?;
        return Ok(Value::UNDEFINED);
    }
    let source = internal::to_object(cx, source)?;
    root(cx, Value::object(source));
    let source_length = internal::length_of_array_like(cx, source)?;
    if offset == f64::INFINITY || source_length as f64 + offset > target_length as f64 {
        return Err(error::range_error(cx, "invalid array length"));
    }
    for index in 0..source_length {
        let key = internal::number_key(cx, index as f64)?;
        let value = internal::get_property(cx, source, key)?;
        typed::set_element(cx, target, offset + index as f64, value)?;
    }
    Ok(Value::UNDEFINED)
}

pub(super) fn slice(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let (object, length) = this_typed(cx, &site)?;
    let (start, end) = (site.arg(cx, 0), site.arg(cx, 1));
    let first = relative(cx, start, length, 0)?;
    let last = relative(cx, end, length, length)?;
    let count = last.saturating_sub(first);
    let result = same_type(cx, object, count as u64)?;
    if count > 0 {
        let Some(length) = typed::length(cx.heap, object) else {
            return Err(error::type_error(cx, OUT_OF_BOUNDS));
        };
        let last = last.min(length);
        let count = last.saturating_sub(first);
        copy_elements(cx, object, first, result, 0, count)?;
    }
    Ok(Value::object(result))
}

/// The default typed-array order: numeric, `-0` before `+0`, NaN last.
fn default_compare(cx: &Cx<'_>, x: Value, y: Value) -> Ordering {
    if let (Some(a), Some(b)) = (x.as_f64(), y.as_f64()) {
        return match (a.is_nan(), b.is_nan()) {
            (true, true) => Ordering::Equal,
            (true, false) => Ordering::Greater,
            (false, true) => Ordering::Less,
            _ if a == 0.0 && b == 0.0 => b.is_sign_negative().cmp(&a.is_sign_negative()),
            _ => a.partial_cmp(&b).unwrap(),
        };
    }
    let (a, b) = (
        crate::vm::ops::big(cx.heap, x),
        crate::vm::ops::big(cx.heap, y),
    );
    a.cmp(&b)
}

/// The values of a typed array, sorted (`SortIndexedProperties`).
fn sorted(
    cx: &mut Cx<'_>,
    object: ObjectId,
    length: usize,
    comparator: Value,
) -> Result<Vec<Value>, Throw> {
    let mut values = Vec::with_capacity(length);
    for index in 0..length {
        let value = get(cx, object, index)?;
        root(cx, value);
        values.push(value);
    }
    super::array::merge_sort_by(cx, &mut values, |cx, x, y| {
        if comparator.is_undefined() {
            return Ok(default_compare(cx, x, y));
        }
        let result = crate::vm::call(cx, comparator, Value::UNDEFINED, &[x, y])?;
        let number = ops::to_number(cx, result)?;
        Ok(if number < 0.0 {
            Ordering::Less
        } else if number > 0.0 {
            Ordering::Greater
        } else {
            Ordering::Equal
        })
    })?;
    Ok(values)
}

fn comparator(cx: &mut Cx<'_>, site: &CallSite) -> Result<Value, Throw> {
    let comparator = site.arg(cx, 0);
    if !comparator.is_undefined() && !ops::is_callable(cx.heap, comparator) {
        return Err(error::type_error(cx, "not a function"));
    }
    Ok(comparator)
}

pub(super) fn sort(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let comparator = comparator(cx, &site)?;
    let (object, length) = this_typed(cx, &site)?;
    let values = sorted(cx, object, length, comparator)?;
    for (index, value) in values.into_iter().enumerate() {
        typed::set_element(cx, object, index as f64, value)?;
    }
    Ok(Value::object(object))
}

pub(super) fn to_sorted(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    // 23.2.3.33 steps 1–3: the comparator, then the receiver.
    let comparator = comparator(cx, &site)?;
    let (object, length) = this_typed(cx, &site)?;
    let result = same_type(cx, object, length as u64)?;
    let values = sorted(cx, object, length, comparator)?;
    for (index, value) in values.into_iter().enumerate() {
        set(cx, result, index, value)?;
    }
    Ok(Value::object(result))
}

pub(super) fn to_reversed(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let (object, length) = this_typed(cx, &site)?;
    let result = same_type(cx, object, length as u64)?;
    for index in 0..length {
        let value = get(cx, object, length - 1 - index)?;
        set(cx, result, index, value)?;
    }
    Ok(Value::object(result))
}

pub(super) fn with(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let (object, length) = this_typed_detached(cx, &site)?;
    let index = site.arg(cx, 0);
    let relative = internal::to_integer_or_infinity(cx, index)?;
    let actual = if relative >= 0.0 {
        relative
    } else {
        length as f64 + relative
    };
    let value = site.arg(cx, 1);
    let kind = kind_of(cx, object);
    let value = if kind.is_bigint() {
        crate::builtins::to_bigint(cx, value)?
    } else {
        Value::number(ops::to_number(cx, value)?)
    };
    root(cx, value);
    if typed::valid_index(cx.heap, object, actual).is_none() {
        return Err(error::range_error(cx, "invalid array index"));
    }
    let result = same_type(cx, object, length as u64)?;
    for index in 0..length {
        let element = if index as f64 == actual {
            value
        } else {
            get(cx, object, index)?
        };
        set(cx, result, index, element)?;
    }
    Ok(Value::object(result))
}

/// `subarray(start, end)`: a view of the same buffer.
pub(super) fn subarray(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let object = this_any(cx, &site)?;
    let (view, buffer) = typed::view(cx.heap, object);
    let length = typed::length(cx.heap, object).unwrap_or(0);
    let (start, end) = (site.arg(cx, 0), site.arg(cx, 1));
    let first = relative(cx, start, length, 0)?;
    let tracking = view.length.is_none() && end.is_undefined();
    let last = relative(cx, end, length, length)?;
    let size = view.kind.size();
    let begin = view.byte_offset + first * size;
    let mut args = vec![Value::object(buffer), Value::number(begin as f64)];
    if !tracking {
        args.push(Value::number(last.saturating_sub(first) as f64));
    }
    let constructor = Value::object(cx.realm.intrinsic(constructor_intrinsic(view.kind)));
    let result = crate::vm::construct(cx, constructor, &args, constructor)?;
    root(cx, result);
    validate(cx, result).map(|(object, _)| Value::object(object))
}

pub(super) fn to_locale_string(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let (object, length) = this_typed(cx, &site)?;
    let mut joined: Vec<u16> = Vec::new();
    let key = cx
        .heap
        .property_key("toLocaleString")
        .map_err(|error| error::heap_error(cx, error))?;
    for index in 0..length {
        if index > 0 {
            joined.push(u16::from(b','));
        }
        let value = get(cx, object, index)?;
        if value.is_undefined() || value.is_null() {
            continue;
        }
        let text = internal::invoke(cx, value, key, &[])?;
        let text = ops::to_string(cx, text)?;
        joined.extend(ops::units(cx.heap, text.as_string().unwrap()));
    }
    super::string_units(cx, &joined)
}

// ---- Uint8Array base64 and hex ----

/// `ValidateUint8Array`: the type only; each method checks the bounds
/// where its specification does, after reading its options.
fn this_uint8(cx: &mut Cx<'_>, site: &CallSite) -> Result<ObjectId, Throw> {
    let this = site.this(cx);
    match this.as_object() {
        Some(object)
            if cx.heap.object_kind(object) == crate::ObjectKind::TypedArray
                && kind_of(cx, object) == ElementKind::Uint8 =>
        {
            Ok(object)
        }
        _ => Err(error::type_error(cx, "not a Uint8Array")),
    }
}

fn bytes_of(cx: &Cx<'_>, object: ObjectId, length: usize) -> Vec<u8> {
    let (view, buffer) = typed::view(cx.heap, object);
    typed::buffer(cx.heap, buffer).to_vec(view.byte_offset, length)
}

/// A string option of `options` (`alphabet`, `lastChunkHandling`).
fn string_option(cx: &mut Cx<'_>, options: Value, name: &str) -> Result<Option<String>, Throw> {
    let Some(options) = options.as_object() else {
        return Ok(None);
    };
    let key = cx
        .heap
        .property_key(name)
        .map_err(|error| error::heap_error(cx, error))?;
    let value = internal::get_property(cx, options, key)?;
    if value.is_undefined() {
        return Ok(None);
    }
    match value.as_string() {
        Some(string) => Ok(Some(cx.heap.string_text(string))),
        None => Err(error::type_error(
            cx,
            &format!("expected string for {name}"),
        )),
    }
}

fn options_object(cx: &mut Cx<'_>, options: Value) -> Result<Value, Throw> {
    if options.is_undefined() || options.as_object().is_some() {
        Ok(options)
    } else {
        Err(error::type_error(cx, "options must be an object"))
    }
}

fn alphabet(cx: &mut Cx<'_>, options: Value) -> Result<bool, Throw> {
    match string_option(cx, options, "alphabet")?.as_deref() {
        None | Some("base64") => Ok(false),
        Some("base64url") => Ok(true),
        Some(_) => Err(error::type_error(cx, "invalid alphabet")),
    }
}

fn last_chunk(cx: &mut Cx<'_>, options: Value) -> Result<super::base64::LastChunk, Throw> {
    use super::base64::LastChunk;
    match string_option(cx, options, "lastChunkHandling")?.as_deref() {
        None | Some("loose") => Ok(LastChunk::Loose),
        Some("strict") => Ok(LastChunk::Strict),
        Some("stop-before-partial") => Ok(LastChunk::StopBeforePartial),
        Some(_) => Err(error::type_error(cx, "invalid lastChunkHandling option")),
    }
}

fn string_argument(cx: &mut Cx<'_>, value: Value) -> Result<Vec<u16>, Throw> {
    match value.as_string() {
        Some(string) => Ok(ops::units(cx.heap, string)),
        None => Err(error::type_error(cx, "expected string")),
    }
}

pub(super) fn from_base64(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let input = site.arg(cx, 0);
    let input = string_argument(cx, input)?;
    let options = site.arg(cx, 1);
    let options = options_object(cx, options)?;
    let url = alphabet(cx, options)?;
    let last = last_chunk(cx, options)?;
    let decoded = super::base64::decode_base64(&input, url, last, usize::MAX);
    if let Some(message) = decoded.error {
        return Err(error::throw(cx, crate::ErrorKind::Syntax, message));
    }
    let object = create(cx, ElementKind::Uint8, decoded.bytes.len() as u64)?;
    let (_, buffer) = typed::view(cx.heap, object);
    typed::buffer_mut(cx.heap, buffer).write(0, &decoded.bytes);
    Ok(Value::object(object))
}

pub(super) fn from_hex(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let input = site.arg(cx, 0);
    let input = string_argument(cx, input)?;
    let decoded = super::base64::decode_hex(&input, usize::MAX);
    if let Some(message) = decoded.error {
        return Err(error::throw(cx, crate::ErrorKind::Syntax, message));
    }
    let object = create(cx, ElementKind::Uint8, decoded.bytes.len() as u64)?;
    let (_, buffer) = typed::view(cx.heap, object);
    typed::buffer_mut(cx.heap, buffer).write(0, &decoded.bytes);
    Ok(Value::object(object))
}

/// The `{ read, written }` result of the `setFrom*` methods.
fn read_written(cx: &mut Cx<'_>, read: usize, written: usize) -> Result<Value, Throw> {
    let object = super::new_plain(cx, 2)?;
    root(cx, Value::object(object));
    for (name, value) in [("read", read), ("written", written)] {
        let key = cx
            .heap
            .property_key(name)
            .map_err(|error| error::heap_error(cx, error))?;
        internal::create_data_property_or_throw(cx, object, key, Value::number(value as f64))?;
    }
    Ok(Value::object(object))
}

fn write_decoded(cx: &mut Cx<'_>, object: ObjectId, bytes: &[u8]) {
    let (view, buffer) = typed::view(cx.heap, object);
    typed::buffer_mut(cx.heap, buffer).write(view.byte_offset, bytes);
}

pub(super) fn set_from_base64(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let object = this_uint8(cx, &site)?;
    let input = site.arg(cx, 0);
    let input = string_argument(cx, input)?;
    let options = site.arg(cx, 1);
    let options = options_object(cx, options)?;
    let url = alphabet(cx, options)?;
    let last = last_chunk(cx, options)?;
    let Some(length) = typed::length(cx.heap, object) else {
        return Err(error::type_error(cx, OUT_OF_BOUNDS));
    };
    let decoded = super::base64::decode_base64(&input, url, last, length);
    write_decoded(cx, object, &decoded.bytes);
    if let Some(message) = decoded.error {
        return Err(error::throw(cx, crate::ErrorKind::Syntax, message));
    }
    read_written(cx, decoded.read, decoded.bytes.len())
}

pub(super) fn set_from_hex(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let object = this_uint8(cx, &site)?;
    let input = site.arg(cx, 0);
    let input = string_argument(cx, input)?;
    let Some(length) = typed::length(cx.heap, object) else {
        return Err(error::type_error(cx, OUT_OF_BOUNDS));
    };
    let decoded = super::base64::decode_hex(&input, length);
    write_decoded(cx, object, &decoded.bytes);
    if let Some(message) = decoded.error {
        return Err(error::throw(cx, crate::ErrorKind::Syntax, message));
    }
    read_written(cx, decoded.read, decoded.bytes.len())
}

pub(super) fn to_base64(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let object = this_uint8(cx, &site)?;
    let options = site.arg(cx, 0);
    let options = options_object(cx, options)?;
    let url = alphabet(cx, options)?;
    let omit = match options.as_object() {
        Some(options) => {
            let key = cx
                .heap
                .property_key("omitPadding")
                .map_err(|error| error::heap_error(cx, error))?;
            let value = internal::get_property(cx, options, key)?;
            ops::to_boolean(cx.heap, value)
        }
        None => false,
    };
    let Some(length) = typed::length(cx.heap, object) else {
        return Err(error::type_error(cx, OUT_OF_BOUNDS));
    };
    let bytes = bytes_of(cx, object, length);
    let text = super::base64::encode_base64(&bytes, url, omit);
    super::string_text(cx, &text)
}

pub(super) fn to_hex(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let object = this_uint8(cx, &site)?;
    let Some(length) = typed::length(cx.heap, object) else {
        return Err(error::type_error(cx, OUT_OF_BOUNDS));
    };
    let bytes = bytes_of(cx, object, length);
    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        text.push_str(&format!("{byte:02x}"));
    }
    super::string_text(cx, &text)
}
