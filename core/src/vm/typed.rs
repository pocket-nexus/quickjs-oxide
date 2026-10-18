//! Typed-array elements: the integer-indexed exotic object's element reads and
//! writes, the element conversions, and the loop's inline paths.

use super::{Cx, Throw, error, number, ops};
use crate::heap::Heap;
use crate::heap::buffer::{Buffer, ElementKind, View};
use crate::heap::object::InternalData;
use crate::value::{ObjectId, Value};
use crate::vm::bigint::Big;
use crate::{KeyKind, PropertyKey};

/// The view record and buffer object of a typed array or `DataView`.
#[inline]
pub(crate) fn view(heap: &Heap, object: ObjectId) -> (View, ObjectId) {
    let internal = heap.internal(object);
    let InternalData::View(view) = internal.data else {
        unreachable!("a view");
    };
    (view, internal.slots[0].as_object().unwrap())
}

#[inline]
pub(crate) fn buffer(heap: &Heap, buffer: ObjectId) -> &Buffer {
    match &heap.internal(buffer).data {
        InternalData::Buffer(buffer) => buffer,
        _ => unreachable!("a buffer"),
    }
}

#[inline]
pub(crate) fn buffer_mut(heap: &mut Heap, buffer: ObjectId) -> &mut Buffer {
    match &mut heap.internal_mut(buffer).data {
        InternalData::Buffer(buffer) => buffer,
        _ => unreachable!("a buffer"),
    }
}

/// A typed array's length, or `None` when it is out of bounds or its
/// buffer is detached (`IsTypedArrayOutOfBounds`).
#[inline]
pub(crate) fn length(heap: &Heap, object: ObjectId) -> Option<usize> {
    let (view, store) = view(heap, object);
    let buffer = buffer(heap, store);
    view.current_length(buffer.len(), buffer.detached, view.kind.size())
}

/// `IsTypedArrayFixedLength(O)`.
pub(crate) fn fixed_length(heap: &Heap, object: ObjectId) -> bool {
    let (view, store) = view(heap, object);
    let buffer = buffer(heap, store);
    view.length.is_some() && (buffer.max.is_none() || buffer.is_shared())
}

/// `CanonicalNumericIndexString` of a key: the number it names, if any.
pub(crate) fn numeric_index(heap: &Heap, key: PropertyKey) -> Option<f64> {
    match key.kind() {
        KeyKind::Index(index) => Some(f64::from(index)),
        KeyKind::Symbol(_) => None,
        KeyKind::String(id) => {
            let record = heap.string(id);
            let repr = record.repr();
            if repr.is_empty() {
                return None;
            }
            let first = repr.unit(0);
            if !(first == u16::from(b'-')
                || first == u16::from(b'I')
                || first == u16::from(b'N')
                || (u16::from(b'0')..=u16::from(b'9')).contains(&first))
            {
                return None;
            }
            let units: Vec<u16> = repr.units().collect();
            if units == "-0".encode_utf16().collect::<Vec<_>>() {
                return Some(-0.0);
            }
            let value = number::parse(&units);
            let text = number::to_string(value);
            (text.encode_utf16().eq(units.iter().copied())).then_some(value)
        }
    }
}

/// `IsValidIntegerIndex(O, index)`: the element position.
#[inline]
pub(crate) fn valid_index(heap: &Heap, object: ObjectId, index: f64) -> Option<usize> {
    if index.fract() != 0.0 || (index == 0.0 && index.is_sign_negative()) || index < 0.0 {
        return None;
    }
    let length = length(heap, object)?;
    (index < length as f64).then_some(index as usize)
}

/// Decodes element bytes (little-endian).
#[inline]
pub(crate) fn decode(cx: &mut Cx<'_>, kind: ElementKind, bytes: &[u8]) -> Result<Value, Throw> {
    let mut raw = [0u8; 8];
    raw[..bytes.len()].copy_from_slice(bytes);
    Ok(match kind {
        ElementKind::Int8 => Value::int32(i32::from(raw[0] as i8)),
        ElementKind::Uint8 | ElementKind::Uint8Clamped => Value::int32(i32::from(raw[0])),
        ElementKind::Int16 => Value::int32(i32::from(i16::from_le_bytes([raw[0], raw[1]]))),
        ElementKind::Uint16 => Value::int32(i32::from(u16::from_le_bytes([raw[0], raw[1]]))),
        ElementKind::Int32 => Value::int32(i32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]])),
        ElementKind::Uint32 => number::number(f64::from(u32::from_le_bytes([
            raw[0], raw[1], raw[2], raw[3],
        ]))),
        ElementKind::Float16 => number::number(number::from_float16_bits(u16::from_le_bytes([
            raw[0], raw[1],
        ]))),
        ElementKind::Float32 => Value::number(f64::from(f32::from_le_bytes([
            raw[0], raw[1], raw[2], raw[3],
        ]))),
        ElementKind::Float64 => Value::number(f64::from_le_bytes(raw)),
        ElementKind::BigInt64 => {
            let value = i64::from_le_bytes(raw);
            return alloc_big(cx, Big::from_i64(value));
        }
        ElementKind::BigUint64 => {
            let value = u64::from_le_bytes(raw);
            return alloc_big(cx, Big::from_u64(value));
        }
    })
}

fn alloc_big(cx: &mut Cx<'_>, big: Big) -> Result<Value, Throw> {
    cx.heap
        .alloc_bigint(big.negative, &big.limbs)
        .map(Value::bigint)
        .map_err(|error| error::heap_error(cx, error))
}

/// The low 64 bits of a BigInt, two's complement.
pub(crate) fn bigint_bits(heap: &Heap, value: Value) -> u64 {
    let record = heap.bigint(value.as_bigint().unwrap());
    let low = record.limbs.first().copied().unwrap_or(0);
    if record.negative {
        low.wrapping_neg()
    } else {
        low
    }
}

/// `ToUint8Clamp`.
fn uint8_clamp(value: f64) -> u8 {
    if value.is_nan() || value <= 0.0 {
        return 0;
    }
    if value >= 255.0 {
        return 255;
    }
    let floor = value.floor();
    let half = floor + 0.5;
    if value < half {
        floor as u8
    } else if value > half {
        floor as u8 + 1
    } else if (floor as u8).is_multiple_of(2) {
        floor as u8
    } else {
        floor as u8 + 1
    }
}

/// Encodes a number (or for the BigInt kinds the 64-bit pattern in
/// `bits`) as element bytes; returns the byte count.
#[inline]
pub(crate) fn encode_number(kind: ElementKind, value: f64, out: &mut [u8; 8]) -> usize {
    match kind {
        ElementKind::Int8 | ElementKind::Uint8 => out[0] = number::to_int32(value) as u8,
        ElementKind::Uint8Clamped => out[0] = uint8_clamp(value),
        ElementKind::Int16 | ElementKind::Uint16 => {
            out[..2].copy_from_slice(&(number::to_int32(value) as u16).to_le_bytes())
        }
        ElementKind::Int32 | ElementKind::Uint32 => {
            out[..4].copy_from_slice(&number::to_int32(value).to_le_bytes())
        }
        ElementKind::Float16 => {
            out[..2].copy_from_slice(&number::to_float16_bits(value).to_le_bytes())
        }
        ElementKind::Float32 => out[..4].copy_from_slice(&(value as f32).to_le_bytes()),
        ElementKind::Float64 => out.copy_from_slice(&value.to_le_bytes()),
        ElementKind::BigInt64 | ElementKind::BigUint64 => unreachable!("a number kind"),
    }
    kind.size()
}

/// Converts `value` for storing in a `kind` element: `ToNumber` or
/// `ToBigInt`, which may run JavaScript. Returns the bytes.
pub(crate) fn convert(cx: &mut Cx<'_>, kind: ElementKind, value: Value) -> Result<[u8; 8], Throw> {
    let mut out = [0u8; 8];
    if kind.is_bigint() {
        let big = crate::builtins::to_bigint(cx, value)?;
        out.copy_from_slice(&bigint_bits(cx.heap, big).to_le_bytes());
    } else {
        let number = ops::to_number(cx, value)?;
        encode_number(kind, number, &mut out);
    }
    Ok(out)
}

/// `TypedArrayGetElement(O, index)`.
pub(crate) fn get_element(
    cx: &mut Cx<'_>,
    object: ObjectId,
    index: f64,
) -> Result<Option<Value>, Throw> {
    let Some(position) = valid_index(cx.heap, object, index) else {
        return Ok(None);
    };
    let (view, store) = view(cx.heap, object);
    let size = view.kind.size();
    let mut bytes = [0u8; 8];
    buffer(cx.heap, store).read(view.byte_offset + position * size, &mut bytes[..size]);
    decode(cx, view.kind, &bytes[..size]).map(Some)
}

/// `TypedArraySetElement(O, index, value)`: converts first, then writes if
/// the index is (still) valid.
pub(crate) fn set_element(
    cx: &mut Cx<'_>,
    object: ObjectId,
    index: f64,
    value: Value,
) -> Result<(), Throw> {
    let (view, _) = view(cx.heap, object);
    let bytes = convert(cx, view.kind, value)?;
    write_converted(cx.heap, object, index, &bytes);
    Ok(())
}

/// Writes already converted bytes if `index` is valid.
pub(crate) fn write_converted(heap: &mut Heap, object: ObjectId, index: f64, bytes: &[u8; 8]) {
    let Some(position) = valid_index(heap, object, index) else {
        return;
    };
    let (view, store) = view(heap, object);
    let size = view.kind.size();
    buffer_mut(heap, store).write(view.byte_offset + position * size, &bytes[..size]);
}

// ---- the loop's inline paths ----

/// `a[i]` for a typed array and an int32 index: the element, or `None` to
/// take the slow path (BigInt kinds, out-of-range or detached reads, which
/// give `undefined` there).
#[inline(never)]
pub(crate) fn fast_get(heap: &Heap, object: ObjectId, index: u32) -> Option<Value> {
    let internal = heap.internal(object);
    let InternalData::View(view) = internal.data else {
        return None;
    };
    let store = buffer(heap, internal.slots[0].as_object()?);
    let size = view.kind.size();
    let length = view.current_length(store.len(), store.detached, size)?;
    if index as usize >= length {
        return None;
    }
    let at = view.byte_offset + index as usize * size;
    let bytes = &store.owned_slice()?[at..at + size];
    Some(match view.kind {
        ElementKind::Int8 => Value::int32(i32::from(bytes[0] as i8)),
        ElementKind::Uint8 | ElementKind::Uint8Clamped => Value::int32(i32::from(bytes[0])),
        ElementKind::Int16 => Value::int32(i32::from(i16::from_le_bytes([bytes[0], bytes[1]]))),
        ElementKind::Uint16 => Value::int32(i32::from(u16::from_le_bytes([bytes[0], bytes[1]]))),
        ElementKind::Int32 => {
            Value::int32(i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
        }
        ElementKind::Uint32 => number::number(f64::from(u32::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3],
        ]))),
        ElementKind::Float32 => Value::number(f64::from(f32::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3],
        ]))),
        ElementKind::Float64 => Value::number(f64::from_le_bytes(bytes.try_into().ok()?)),
        ElementKind::Float16 => number::number(number::from_float16_bits(u16::from_le_bytes([
            bytes[0], bytes[1],
        ]))),
        ElementKind::BigInt64 | ElementKind::BigUint64 => return None,
    })
}

/// `a[i] = v` for a typed array, an int32 index and a number: whether it
/// was done (otherwise the slow path converts and writes).
#[inline(never)]
pub(crate) fn fast_set(heap: &mut Heap, object: ObjectId, index: u32, value: Value) -> bool {
    let Some(number) = value.as_f64() else {
        return false;
    };
    let internal = heap.internal(object);
    let InternalData::View(view) = internal.data else {
        return false;
    };
    if view.kind.is_bigint() {
        return false;
    }
    let Some(store) = internal.slots[0].as_object() else {
        return false;
    };
    let buffer = buffer_mut(heap, store);
    let size = view.kind.size();
    let Some(length) = view.current_length(buffer.len(), buffer.detached, size) else {
        // A write to an out-of-bounds view is dropped, after `ToNumber`,
        // which a number needs none of.
        return true;
    };
    if index as usize >= length {
        return true;
    }
    let at = view.byte_offset + index as usize * size;
    let Some(bytes) = buffer.owned_slice_mut() else {
        return false;
    };
    match view.kind {
        ElementKind::Uint8 | ElementKind::Int8 => {
            bytes[at] = match value.as_int32() {
                Some(int) => int as u8,
                None => number::to_int32(number) as u8,
            }
        }
        ElementKind::Int32 | ElementKind::Uint32 => {
            let int = value.as_int32().unwrap_or_else(|| number::to_int32(number));
            bytes[at..at + 4].copy_from_slice(&int.to_le_bytes());
        }
        ElementKind::Float64 => bytes[at..at + 8].copy_from_slice(&number.to_le_bytes()),
        ElementKind::Float32 => bytes[at..at + 4].copy_from_slice(&(number as f32).to_le_bytes()),
        kind => {
            let mut out = [0u8; 8];
            let size = encode_number(kind, number, &mut out);
            bytes[at..at + size].copy_from_slice(&out[..size]);
        }
    }
    true
}
