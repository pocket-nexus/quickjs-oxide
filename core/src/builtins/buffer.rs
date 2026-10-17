//! `ArrayBuffer`, `SharedArrayBuffer` and `DataView` (phase 4, section 6.1
//! row 9). A buffer is an internal record (tags `ArrayBuffer`,
//! `SharedArrayBuffer`) holding a [`Buffer`]; a `DataView` (tag `DataView`)
//! holds the buffer in slot 0 and a byte [`View`]. `Symbol.species` is not
//! consulted (phase 0): `slice` makes an intrinsic buffer.

use super::bigint::to_index;
use super::{
    Native, accessor, getter, install_constructor, link_constructor, methods, to_string_tag,
};
use crate::heap::HeapError;
use crate::heap::buffer::{Buffer, ElementKind, MAX_BUFFER_LENGTH, Store, View};
use crate::heap::object::{InternalData, InternalTag, ObjectKind};
use crate::runtime::{Intrinsic, WellKnown};
use crate::value::{ObjectId, Value};
use crate::vm::native::CallSite;
use crate::vm::object as internal;
use crate::vm::{Cx, Throw, error, ops, typed};

pub(super) fn install(cx: &mut Cx<'_>) -> Result<(), HeapError> {
    let object_prototype = cx.realm.intrinsic(Intrinsic::ObjectPrototype);
    let species = WellKnown::key(cx.realm.symbols.species);

    let prototype = super::plain_object(cx, Some(object_prototype), 12)?;
    cx.set_intrinsic(Intrinsic::ArrayBufferPrototype, prototype);
    let constructor = install_constructor(cx, Native::ArrayBufferConstructor)?;
    cx.set_intrinsic(Intrinsic::ArrayBuffer, constructor);
    methods(cx, constructor, &[Native::ArrayBufferIsView])?;
    accessor(
        cx,
        constructor,
        species,
        "[Symbol.species]",
        Some(Native::SpeciesGetter),
        None,
    )?;
    getter(cx, prototype, "byteLength", Native::ArrayBufferByteLength)?;
    getter(
        cx,
        prototype,
        "maxByteLength",
        Native::ArrayBufferMaxByteLength,
    )?;
    getter(cx, prototype, "resizable", Native::ArrayBufferResizable)?;
    getter(cx, prototype, "detached", Native::ArrayBufferDetached)?;
    methods(
        cx,
        prototype,
        &[
            Native::ArrayBufferResize,
            Native::ArrayBufferSlice,
            Native::ArrayBufferTransfer,
            Native::ArrayBufferTransferToFixedLength,
        ],
    )?;
    to_string_tag(cx, prototype, "ArrayBuffer")?;
    link_constructor(cx, constructor, prototype)?;

    let prototype = super::plain_object(cx, Some(object_prototype), 8)?;
    cx.set_intrinsic(Intrinsic::SharedArrayBufferPrototype, prototype);
    let constructor = install_constructor(cx, Native::SharedArrayBufferConstructor)?;
    accessor(
        cx,
        constructor,
        species,
        "[Symbol.species]",
        Some(Native::SpeciesGetter),
        None,
    )?;
    getter(
        cx,
        prototype,
        "byteLength",
        Native::SharedArrayBufferByteLength,
    )?;
    getter(
        cx,
        prototype,
        "maxByteLength",
        Native::SharedArrayBufferMaxByteLength,
    )?;
    getter(cx, prototype, "growable", Native::SharedArrayBufferGrowable)?;
    methods(
        cx,
        prototype,
        &[
            Native::SharedArrayBufferGrow,
            Native::SharedArrayBufferSlice,
        ],
    )?;
    to_string_tag(cx, prototype, "SharedArrayBuffer")?;
    link_constructor(cx, constructor, prototype)?;

    let prototype = super::plain_object(cx, Some(object_prototype), 28)?;
    cx.set_intrinsic(Intrinsic::DataViewPrototype, prototype);
    let constructor = install_constructor(cx, Native::DataViewConstructor)?;
    getter(cx, prototype, "buffer", Native::DataViewBuffer)?;
    getter(cx, prototype, "byteLength", Native::DataViewByteLength)?;
    getter(cx, prototype, "byteOffset", Native::DataViewByteOffset)?;
    methods(
        cx,
        prototype,
        &[
            Native::DataViewGetInt8,
            Native::DataViewGetUint8,
            Native::DataViewGetInt16,
            Native::DataViewGetUint16,
            Native::DataViewGetInt32,
            Native::DataViewGetUint32,
            Native::DataViewGetBigInt64,
            Native::DataViewGetBigUint64,
            Native::DataViewGetFloat16,
            Native::DataViewGetFloat32,
            Native::DataViewGetFloat64,
            Native::DataViewSetInt8,
            Native::DataViewSetUint8,
            Native::DataViewSetInt16,
            Native::DataViewSetUint16,
            Native::DataViewSetInt32,
            Native::DataViewSetUint32,
            Native::DataViewSetBigInt64,
            Native::DataViewSetBigUint64,
            Native::DataViewSetFloat16,
            Native::DataViewSetFloat32,
            Native::DataViewSetFloat64,
        ],
    )?;
    to_string_tag(cx, prototype, "DataView")?;
    link_constructor(cx, constructor, prototype)
}

// ---- allocation ----

/// A new buffer object (`AllocateArrayBuffer` / `AllocateSharedArrayBuffer`
/// after the prototype lookup), rooted.
pub(crate) fn alloc_buffer(
    cx: &mut Cx<'_>,
    prototype: ObjectId,
    len: usize,
    max: Option<usize>,
    shared: bool,
) -> Result<ObjectId, Throw> {
    if len > MAX_BUFFER_LENGTH {
        return Err(error::range_error(cx, "invalid array buffer length"));
    }
    if max.is_some_and(|max| max > MAX_BUFFER_LENGTH) {
        return Err(error::range_error(cx, "invalid max array buffer length"));
    }
    let reserve = max.filter(|_| shared).unwrap_or(len);
    cx.heap
        .charge(reserve, &[Value::object(prototype)])
        .map_err(|error| error::heap_error(cx, error))?;
    let (tag, buffer) = if shared {
        (InternalTag::SharedArrayBuffer, Buffer::shared(len, max))
    } else {
        (InternalTag::ArrayBuffer, Buffer::owned(len, max))
    };
    let object = cx
        .heap
        .alloc_internal(
            ObjectKind::Builtin,
            tag,
            Some(prototype),
            0,
            &[],
            InternalData::Buffer(Box::new(buffer)),
            0,
        )
        .map_err(|error| error::heap_error(cx, error))?;
    cx.heap.roots().push(Value::object(object));
    Ok(object)
}

/// An `%ArrayBuffer%` of `len` zero bytes, rooted.
pub(crate) fn new_array_buffer(cx: &mut Cx<'_>, len: usize) -> Result<ObjectId, Throw> {
    let prototype = cx.realm.intrinsic(Intrinsic::ArrayBufferPrototype);
    alloc_buffer(cx, prototype, len, None, false)
}

/// Whether `value` is a buffer object (of either kind).
pub(crate) fn is_buffer(cx: &Cx<'_>, value: Value) -> bool {
    value.as_object().is_some_and(|object| {
        matches!(
            cx.heap.internal_tag(object),
            InternalTag::ArrayBuffer | InternalTag::SharedArrayBuffer
        )
    })
}

/// `$262.detachArrayBuffer` / `DetachArrayBuffer`.
pub(crate) fn detach(cx: &mut Cx<'_>, value: Value) -> Result<(), Throw> {
    match value.as_object() {
        Some(object) if cx.heap.internal_tag(object) == InternalTag::ArrayBuffer => {
            let buffer = typed::buffer_mut(cx.heap, object);
            buffer.store = Store::Owned(Vec::new());
            buffer.detached = true;
            Ok(())
        }
        _ => Err(error::type_error(cx, "not an ArrayBuffer")),
    }
}

/// `GetArrayBufferMaxByteLengthOption(options)`.
fn max_option(cx: &mut Cx<'_>, options: Value) -> Result<Option<usize>, Throw> {
    let Some(options) = options.as_object() else {
        return Ok(None);
    };
    let key = cx
        .heap
        .property_key("maxByteLength")
        .map_err(|error| error::heap_error(cx, error))?;
    let max = internal::get_property(cx, options, key)?;
    if max.is_undefined() {
        return Ok(None);
    }
    Ok(Some(to_index(cx, max)? as usize))
}

fn construct(cx: &mut Cx<'_>, site: CallSite, shared: bool) -> Result<Value, Throw> {
    if !site.construct {
        return Err(error::type_error(cx, "must be called with new"));
    }
    let length = site.arg(cx, 0);
    let len = to_index(cx, length)?;
    let options = site.arg(cx, 1);
    let max = max_option(cx, options)?;
    if max.is_some_and(|max| len as usize > max) {
        return Err(error::range_error(cx, "invalid array buffer max length"));
    }
    let default = if shared {
        Intrinsic::SharedArrayBufferPrototype
    } else {
        Intrinsic::ArrayBufferPrototype
    };
    let prototype = internal::prototype_from_constructor(cx, site.new_target, default)?;
    if len > MAX_BUFFER_LENGTH as u64 {
        return Err(error::range_error(cx, "invalid array buffer length"));
    }
    alloc_buffer(cx, prototype, len as usize, max, shared).map(Value::object)
}

pub(super) fn array_buffer_constructor(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    construct(cx, site, false)
}

pub(super) fn shared_array_buffer_constructor(
    cx: &mut Cx<'_>,
    site: CallSite,
) -> Result<Value, Throw> {
    construct(cx, site, true)
}

pub(super) fn is_view(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let value = site.arg(cx, 0);
    Ok(Value::bool(value.as_object().is_some_and(|object| {
        cx.heap.object_kind(object) == ObjectKind::TypedArray
            || cx.heap.internal_tag(object) == InternalTag::DataView
    })))
}

// ---- ArrayBuffer.prototype ----

fn this_buffer(cx: &mut Cx<'_>, site: &CallSite, tag: InternalTag) -> Result<ObjectId, Throw> {
    let this = site.this(cx);
    match this.as_object() {
        Some(object) if cx.heap.internal_tag(object) == tag => Ok(object),
        _ => Err(error::object_expected(
            cx,
            if tag == InternalTag::ArrayBuffer {
                "ArrayBuffer"
            } else {
                "SharedArrayBuffer"
            },
        )),
    }
}

pub(super) fn byte_length(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let object = this_buffer(cx, &site, InternalTag::ArrayBuffer)?;
    Ok(Value::number(typed::buffer(cx.heap, object).len() as f64))
}

pub(super) fn max_byte_length(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let object = this_buffer(cx, &site, InternalTag::ArrayBuffer)?;
    let buffer = typed::buffer(cx.heap, object);
    let max = if buffer.detached {
        0
    } else {
        buffer.max.unwrap_or(buffer.len())
    };
    Ok(Value::number(max as f64))
}

pub(super) fn resizable(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let object = this_buffer(cx, &site, InternalTag::ArrayBuffer)?;
    Ok(Value::bool(typed::buffer(cx.heap, object).max.is_some()))
}

pub(super) fn detached(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let object = this_buffer(cx, &site, InternalTag::ArrayBuffer)?;
    Ok(Value::bool(typed::buffer(cx.heap, object).detached))
}

pub(super) fn resize(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let object = this_buffer(cx, &site, InternalTag::ArrayBuffer)?;
    if typed::buffer(cx.heap, object).max.is_none() {
        // QuickJS tests detachment first.
        if typed::buffer(cx.heap, object).detached {
            return Err(error::type_error(cx, "ArrayBuffer is detached"));
        }
        return Err(error::type_error(cx, "array buffer is not resizable"));
    }
    let length = site.arg(cx, 0);
    let len = to_index(cx, length)?;
    let buffer = typed::buffer(cx.heap, object);
    if buffer.detached {
        return Err(error::type_error(cx, "ArrayBuffer is detached"));
    }
    if len > buffer.max.unwrap() as u64 {
        return Err(error::range_error(cx, "invalid array buffer length"));
    }
    let grow = (len as usize).saturating_sub(buffer.len());
    cx.heap
        .charge(grow, &[Value::object(object)])
        .map_err(|error| error::heap_error(cx, error))?;
    if let Some(bytes) = typed::buffer_mut(cx.heap, object).owned_slice_mut() {
        bytes.resize(len as usize, 0);
    }
    Ok(Value::UNDEFINED)
}

/// `start`/`end` arguments resolved against `len` (the slice methods).
fn range(cx: &mut Cx<'_>, site: &CallSite, len: usize) -> Result<(usize, usize), Throw> {
    let start = site.arg(cx, 0);
    let start = internal::to_integer_or_infinity(cx, start)?;
    let first = super::relative(start, len);
    let end = site.arg(cx, 1);
    let last = if end.is_undefined() {
        len
    } else {
        let end = internal::to_integer_or_infinity(cx, end)?;
        super::relative(end, len)
    };
    Ok((first, last.max(first)))
}

pub(super) fn slice(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let object = this_buffer(cx, &site, InternalTag::ArrayBuffer)?;
    if typed::buffer(cx.heap, object).detached {
        return Err(error::type_error(cx, "ArrayBuffer is detached"));
    }
    let len = typed::buffer(cx.heap, object).len();
    let (first, last) = range(cx, &site, len)?;
    let result = new_array_buffer(cx, last - first)?;
    let source = typed::buffer(cx.heap, object);
    if source.detached {
        return Err(error::type_error(cx, "ArrayBuffer is detached"));
    }
    // The source may have shrunk during the conversions.
    let available = source.len().saturating_sub(first).min(last - first);
    let bytes = source.to_vec(first, available);
    typed::buffer_mut(cx.heap, result).write(0, &bytes);
    Ok(Value::object(result))
}

/// `ArrayBufferCopyAndDetach(O, newLength, preserveResizability)`.
fn transfer_buffer(cx: &mut Cx<'_>, site: CallSite, preserve: bool) -> Result<Value, Throw> {
    let object = this_buffer(cx, &site, InternalTag::ArrayBuffer)?;
    let length = site.arg(cx, 0);
    let len = if length.is_undefined() {
        typed::buffer(cx.heap, object).len() as u64
    } else {
        to_index(cx, length)?
    };
    let buffer = typed::buffer(cx.heap, object);
    if buffer.detached {
        return Err(error::type_error(cx, "ArrayBuffer is detached"));
    }
    let max = if preserve { buffer.max } else { None };
    // QuickJS: past the maximum is a TypeError, past the limit a RangeError.
    if max.is_some_and(|max| len > max as u64) {
        return Err(error::type_error(cx, "invalid array buffer length"));
    }
    if len > MAX_BUFFER_LENGTH as u64 {
        return Err(error::range_error(cx, "invalid array buffer length"));
    }
    let prototype = cx.realm.intrinsic(Intrinsic::ArrayBufferPrototype);
    let result = alloc_buffer(cx, prototype, 0, max, false)?;
    let source = typed::buffer_mut(cx.heap, object);
    let mut bytes = match std::mem::replace(&mut source.store, Store::Owned(Vec::new())) {
        Store::Owned(bytes) => bytes,
        Store::Shared(_) => unreachable!("an ArrayBuffer"),
    };
    source.detached = true;
    bytes.resize(len as usize, 0);
    typed::buffer_mut(cx.heap, result).store = Store::Owned(bytes);
    Ok(Value::object(result))
}

pub(super) fn transfer(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    transfer_buffer(cx, site, true)
}

pub(super) fn transfer_to_fixed_length(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    transfer_buffer(cx, site, false)
}

// ---- SharedArrayBuffer.prototype ----

pub(super) fn shared_byte_length(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let object = this_buffer(cx, &site, InternalTag::SharedArrayBuffer)?;
    Ok(Value::number(typed::buffer(cx.heap, object).len() as f64))
}

pub(super) fn growable(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let object = this_buffer(cx, &site, InternalTag::SharedArrayBuffer)?;
    Ok(Value::bool(typed::buffer(cx.heap, object).max.is_some()))
}

pub(super) fn shared_max_byte_length(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let object = this_buffer(cx, &site, InternalTag::SharedArrayBuffer)?;
    let buffer = typed::buffer(cx.heap, object);
    Ok(Value::number(buffer.max.unwrap_or(buffer.len()) as f64))
}

pub(super) fn grow(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let object = this_buffer(cx, &site, InternalTag::SharedArrayBuffer)?;
    if typed::buffer(cx.heap, object).max.is_none() {
        return Err(error::type_error(cx, "array buffer is not resizable"));
    }
    let length = site.arg(cx, 0);
    let len = to_index(cx, length)?;
    let buffer = typed::buffer(cx.heap, object);
    let Store::Shared(shared) = &buffer.store else {
        unreachable!("a shared buffer");
    };
    if len > buffer.max.unwrap() as u64
        || (len as usize) < shared.len()
        || !shared.grow(len as usize)
    {
        return Err(error::range_error(cx, "invalid array buffer length"));
    }
    Ok(Value::UNDEFINED)
}

pub(super) fn shared_slice(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let object = this_buffer(cx, &site, InternalTag::SharedArrayBuffer)?;
    let len = typed::buffer(cx.heap, object).len();
    let (first, last) = range(cx, &site, len)?;
    let prototype = cx.realm.intrinsic(Intrinsic::SharedArrayBufferPrototype);
    let result = alloc_buffer(cx, prototype, last - first, None, true)?;
    let bytes = typed::buffer(cx.heap, object).to_vec(first, last - first);
    typed::buffer_mut(cx.heap, result).write(0, &bytes);
    Ok(Value::object(result))
}

// ---- DataView ----

/// `new DataView(buffer, byteOffset, byteLength)`.
pub(super) fn data_view_constructor(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    if !site.construct {
        return Err(error::type_error(cx, "must be called with new"));
    }
    let buffer = site.arg(cx, 0);
    if !is_buffer(cx, buffer) {
        return Err(error::object_expected(cx, "ArrayBuffer"));
    }
    let store = buffer.as_object().unwrap();
    let offset = site.arg(cx, 1);
    let offset = to_index(cx, offset)? as usize;
    if typed::buffer(cx.heap, store).detached {
        return Err(error::type_error(cx, "ArrayBuffer is detached"));
    }
    let len = typed::buffer(cx.heap, store).len();
    if offset > len {
        return Err(error::range_error(cx, "invalid byteOffset"));
    }
    let fixed = {
        let buffer = typed::buffer(cx.heap, store);
        buffer.max.is_none() || buffer.is_shared()
    };
    let byte_length = site.arg(cx, 2);
    let length = if byte_length.is_undefined() {
        fixed.then_some(len - offset)
    } else {
        let length = to_index(cx, byte_length)? as usize;
        if offset + length > len {
            return Err(error::range_error(cx, "invalid byteLength"));
        }
        Some(length)
    };
    let prototype =
        internal::prototype_from_constructor(cx, site.new_target, Intrinsic::DataViewPrototype)?;
    // The prototype lookup may have detached or shrunk the buffer.
    let current = typed::buffer(cx.heap, store);
    if current.detached {
        return Err(error::type_error(cx, "ArrayBuffer is detached"));
    }
    let len = current.len();
    if offset > len || length.is_some_and(|length| offset + length > len) {
        return Err(error::range_error(cx, "invalid byteOffset or byteLength"));
    }
    let view = View {
        kind: ElementKind::Uint8,
        byte_offset: offset,
        length,
    };
    cx.heap
        .alloc_internal(
            ObjectKind::Builtin,
            InternalTag::DataView,
            Some(prototype),
            0,
            &[buffer],
            InternalData::View(view),
            0,
        )
        .map(Value::object)
        .map_err(|error| error::heap_error(cx, error))
}

fn this_view(cx: &mut Cx<'_>, site: &CallSite) -> Result<ObjectId, Throw> {
    let this = site.this(cx);
    match this.as_object() {
        Some(object) if cx.heap.internal_tag(object) == InternalTag::DataView => Ok(object),
        _ => Err(error::type_error(cx, "not a DataView")),
    }
}

/// The view's byte length, or a `TypeError` when it is out of bounds.
fn view_length(cx: &mut Cx<'_>, object: ObjectId) -> Result<usize, Throw> {
    let (view, store) = typed::view(cx.heap, object);
    let buffer = typed::buffer(cx.heap, store);
    match view.current_length(buffer.len(), buffer.detached, 1) {
        Some(length) => Ok(length),
        None => Err(error::type_error(cx, "ArrayBuffer is detached or resized")),
    }
}

/// The `get`/`set` methods' `this` (QuickJS's `JS_GetOpaque2`).
fn this_view_method(cx: &mut Cx<'_>, site: &CallSite) -> Result<ObjectId, Throw> {
    let this = site.this(cx);
    match this.as_object() {
        Some(object) if cx.heap.internal_tag(object) == InternalTag::DataView => Ok(object),
        _ => Err(error::object_expected(cx, "DataView")),
    }
}

/// [`view_length`] for `get`/`set` of `size` bytes at `index`, with QuickJS's
/// errors: `ArrayBuffer is detached`, then a `RangeError` past the view's
/// recorded length (a length-tracking view out of bounds has none), else a
/// `TypeError`.
fn access_length(
    cx: &mut Cx<'_>,
    object: ObjectId,
    index: usize,
    size: usize,
) -> Result<usize, Throw> {
    let (view, store) = typed::view(cx.heap, object);
    let buffer = typed::buffer(cx.heap, store);
    if let Some(length) = view.current_length(buffer.len(), buffer.detached, 1) {
        return Ok(length);
    }
    if buffer.detached {
        return Err(error::type_error(cx, "ArrayBuffer is detached"));
    }
    if index + size > view.length.unwrap_or(0) {
        return Err(error::range_error(cx, "out of bound"));
    }
    Err(error::type_error(cx, "out of bound"))
}

pub(super) fn data_view_buffer(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let object = this_view(cx, &site)?;
    Ok(cx.heap.internal_slot(object, 0))
}

pub(super) fn data_view_byte_length(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let object = this_view(cx, &site)?;
    view_length(cx, object).map(|length| Value::number(length as f64))
}

pub(super) fn data_view_byte_offset(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let object = this_view(cx, &site)?;
    view_length(cx, object)?;
    let (view, _) = typed::view(cx.heap, object);
    Ok(Value::number(view.byte_offset as f64))
}

/// `GetViewValue(view, requestIndex, isLittleEndian, type)`.
fn get_value(cx: &mut Cx<'_>, site: CallSite, kind: ElementKind) -> Result<Value, Throw> {
    let object = this_view_method(cx, &site)?;
    let index = site.arg(cx, 0);
    let index = to_index(cx, index)? as usize;
    let little = site.arg(cx, 1);
    let little = ops::to_boolean(cx.heap, little);
    let size = kind.size();
    let length = access_length(cx, object, index, size)?;
    if index + size > length {
        return Err(error::range_error(cx, "out of bound"));
    }
    let (view, store) = typed::view(cx.heap, object);
    let mut bytes = [0u8; 8];
    typed::buffer(cx.heap, store).read(view.byte_offset + index, &mut bytes[..size]);
    if !little {
        bytes[..size].reverse();
    }
    typed::decode(cx, kind, &bytes[..size])
}

/// `SetViewValue(view, requestIndex, isLittleEndian, type, value)`.
fn set_value(cx: &mut Cx<'_>, site: CallSite, kind: ElementKind) -> Result<Value, Throw> {
    let object = this_view_method(cx, &site)?;
    let index = site.arg(cx, 0);
    let index = to_index(cx, index)? as usize;
    let value = site.arg(cx, 1);
    let mut bytes = typed::convert(cx, kind, value)?;
    let little = site.arg(cx, 2);
    let little = ops::to_boolean(cx.heap, little);
    let size = kind.size();
    let length = access_length(cx, object, index, size)?;
    if index + size > length {
        return Err(error::range_error(cx, "out of bound"));
    }
    if !little {
        bytes[..size].reverse();
    }
    let (view, store) = typed::view(cx.heap, object);
    typed::buffer_mut(cx.heap, store).write(view.byte_offset + index, &bytes[..size]);
    Ok(Value::UNDEFINED)
}

macro_rules! view_methods {
    ($($get:ident, $set:ident => $kind:ident;)*) => {
        $(
            pub(super) fn $get(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
                get_value(cx, site, ElementKind::$kind)
            }
            pub(super) fn $set(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
                set_value(cx, site, ElementKind::$kind)
            }
        )*
    };
}

view_methods! {
    get_int8, set_int8 => Int8;
    get_uint8, set_uint8 => Uint8;
    get_int16, set_int16 => Int16;
    get_uint16, set_uint16 => Uint16;
    get_int32, set_int32 => Int32;
    get_uint32, set_uint32 => Uint32;
    get_float16, set_float16 => Float16;
    get_float32, set_float32 => Float32;
    get_float64, set_float64 => Float64;
    get_big_int64, set_big_int64 => BigInt64;
    get_big_uint64, set_big_uint64 => BigUint64;
}
