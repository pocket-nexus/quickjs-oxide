//! `Atomics`. Every operation on shared memory runs inside the host waiter's
//! coordinator, which orders them sequentially consistently across agents;
//! `wait` and `notify` queue on the same coordinator.

use core::time::Duration;

use super::bigint::to_index;
use super::{Native, methods, to_string_tag};
use crate::heap::buffer::{ElementKind, Store};
use crate::heap::object::ObjectKind;
use crate::heap::{Heap, HeapError};
use crate::runtime::Intrinsic;
use crate::value::{ObjectId, Value};
use crate::vm::native::CallSite;
use crate::vm::object as internal;
use crate::vm::waiter::{self, WaitLocation, WaitOutcome};
use crate::vm::{Cx, Throw, error, number, ops, typed};

pub(super) fn install(cx: &mut Cx<'_>) -> Result<(), HeapError> {
    let object_prototype = cx.realm.intrinsic(Intrinsic::ObjectPrototype);
    let atomics = super::plain_object(cx, Some(object_prototype), 16)?;
    methods(
        cx,
        atomics,
        &[
            Native::AtomicsAdd,
            Native::AtomicsAnd,
            Native::AtomicsOr,
            Native::AtomicsSub,
            Native::AtomicsXor,
            Native::AtomicsExchange,
            Native::AtomicsCompareExchange,
            Native::AtomicsLoad,
            Native::AtomicsStore,
            Native::AtomicsIsLockFree,
            Native::AtomicsPause,
            Native::AtomicsWait,
            Native::AtomicsNotify,
        ],
    )?;
    to_string_tag(cx, atomics, "Atomics")?;
    let global = cx.realm.global();
    super::define(
        cx,
        global,
        "Atomics",
        Value::object(atomics),
        super::hidden(),
    )
}

/// `ValidateIntegerTypedArray(ta, waitable)` (25.4.3.1): the typed array,
/// its kind and its length then. ValidateTypedArray comes first (detached
/// or out of bounds: `TypeError`), then the element type; `wait` then
/// requires a shared buffer (DoWait step 3).
fn validate(
    cx: &mut Cx<'_>,
    value: Value,
    waitable: bool,
    wait: bool,
) -> Result<(ObjectId, ElementKind, usize), Throw> {
    let Some(object) = value
        .as_object()
        .filter(|&object| cx.heap.object_kind(object) == ObjectKind::TypedArray)
    else {
        return Err(error::type_error(cx, "integer TypedArray expected"));
    };
    let Some(length) = typed::length(cx.heap, object) else {
        let message = if detached(cx, object) {
            "ArrayBuffer is detached"
        } else {
            "TypedArray is out of bounds"
        };
        return Err(error::type_error(cx, message));
    };
    let kind = typed::view(cx.heap, object).0.kind;
    let valid = if waitable {
        matches!(kind, ElementKind::Int32 | ElementKind::BigInt64)
    } else {
        !matches!(
            kind,
            ElementKind::Uint8Clamped
                | ElementKind::Float16
                | ElementKind::Float32
                | ElementKind::Float64
        )
    };
    if !valid {
        return Err(error::type_error(cx, "integer TypedArray expected"));
    }
    if wait && location(cx.heap, object, 0).is_none() {
        return Err(error::type_error(cx, "not a SharedArrayBuffer TypedArray"));
    }
    Ok((object, kind, length))
}

fn detached(cx: &Cx<'_>, object: ObjectId) -> bool {
    let (_, store) = typed::view(cx.heap, object);
    typed::buffer(cx.heap, store).detached
}

/// `ValidateAtomicAccess` against the length validation saw: the element's
/// byte index in the buffer. The operations that convert a value
/// revalidate after it (RevalidateAtomicAccess); `load` right away.
fn index_in(
    cx: &mut Cx<'_>,
    object: ObjectId,
    length: usize,
    index: Value,
) -> Result<usize, Throw> {
    let index = to_index(cx, index)? as usize;
    if index >= length {
        return Err(error::range_error(cx, "out-of-bound access"));
    }
    let view = typed::view(cx.heap, object).0;
    Ok(view.byte_offset + index * view.kind.size())
}

/// `RevalidateAtomicAccess` after the conversions ran; `message` is the
/// TypeError for an array now out of bounds.
fn revalidate(
    cx: &mut Cx<'_>,
    object: ObjectId,
    byte_index: usize,
    message: &str,
) -> Result<(), Throw> {
    let Some(length) = typed::length(cx.heap, object) else {
        return Err(error::type_error(cx, message));
    };
    let view = typed::view(cx.heap, object).0;
    if byte_index >= view.byte_offset + length * view.kind.size() {
        return Err(error::range_error(cx, "out-of-bound access"));
    }
    Ok(())
}

/// The operand converted for `kind`, as the element's 64-bit pattern.
fn operand(cx: &mut Cx<'_>, kind: ElementKind, value: Value) -> Result<(u64, Value), Throw> {
    if kind.is_bigint() {
        let big = crate::builtins::to_bigint(cx, value)?;
        cx.heap.roots().push(big);
        return Ok((typed::bigint_bits(cx.heap, big), big));
    }
    let integer = internal::to_integer_or_infinity(cx, value)?;
    let int = number::to_int32(integer);
    let bits = match kind {
        ElementKind::Uint32 => u64::from(int as u32),
        _ => int as i64 as u64,
    };
    Ok((bits, number::number(integer)))
}

fn mask(kind: ElementKind) -> u64 {
    match kind.size() {
        8 => u64::MAX,
        size => (1u64 << (size * 8)) - 1,
    }
}

/// Runs `operation` on the element's bits at `byte_index`, in the shared
/// order for shared memory; returns the old bits.
fn modify(
    cx: &mut Cx<'_>,
    object: ObjectId,
    byte_index: usize,
    mut operation: impl FnMut(u64) -> Option<u64>,
) -> u64 {
    let (view, store) = typed::view(cx.heap, object);
    let size = view.kind.size();
    let buffer = typed::buffer_mut(cx.heap, store);
    let shared = buffer.is_shared();
    let mut run = || {
        let mut bytes = [0u8; 8];
        buffer.read(byte_index, &mut bytes[..size]);
        let old = u64::from_le_bytes(bytes);
        if let Some(new) = operation(old) {
            buffer.write(byte_index, &new.to_le_bytes()[..size]);
        }
        old
    };
    if shared {
        waiter::with_seq_cst(cx.host.waiter, run)
    } else {
        run()
    }
}

fn decode(cx: &mut Cx<'_>, kind: ElementKind, bits: u64) -> Result<Value, Throw> {
    typed::decode(cx, kind, &bits.to_le_bytes()[..kind.size()])
}

fn read_modify_write(
    cx: &mut Cx<'_>,
    site: CallSite,
    operation: fn(u64, u64) -> u64,
) -> Result<Value, Throw> {
    let array = site.arg(cx, 0);
    let (object, kind, length) = validate(cx, array, false, false)?;
    let index = site.arg(cx, 1);
    let byte_index = index_in(cx, object, length, index)?;
    let value = site.arg(cx, 2);
    let (bits, _) = operand(cx, kind, value)?;
    revalidate(cx, object, byte_index, "ArrayBuffer is detached")?;
    let mask = mask(kind);
    let old = modify(cx, object, byte_index, |old| {
        Some(operation(old, bits) & mask)
    });
    decode(cx, kind, old)
}

pub(super) fn add(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    read_modify_write(cx, site, u64::wrapping_add)
}

pub(super) fn sub(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    read_modify_write(cx, site, u64::wrapping_sub)
}

pub(super) fn and(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    read_modify_write(cx, site, |a, b| a & b)
}

pub(super) fn or(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    read_modify_write(cx, site, |a, b| a | b)
}

pub(super) fn xor(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    read_modify_write(cx, site, |a, b| a ^ b)
}

pub(super) fn exchange(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    read_modify_write(cx, site, |_, b| b)
}

pub(super) fn compare_exchange(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let array = site.arg(cx, 0);
    let (object, kind, length) = validate(cx, array, false, false)?;
    let index = site.arg(cx, 1);
    let byte_index = index_in(cx, object, length, index)?;
    let (expected, replacement) = (site.arg(cx, 2), site.arg(cx, 3));
    let (expected, _) = operand(cx, kind, expected)?;
    let (replacement, _) = operand(cx, kind, replacement)?;
    revalidate(cx, object, byte_index, "ArrayBuffer is detached")?;
    let mask = mask(kind);
    let old = modify(cx, object, byte_index, |old| {
        (old & mask == expected & mask).then_some(replacement & mask)
    });
    decode(cx, kind, old & mask)
}

pub(super) fn load(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let array = site.arg(cx, 0);
    let (object, kind, length) = validate(cx, array, false, false)?;
    let index = site.arg(cx, 1);
    let byte_index = index_in(cx, object, length, index)?;
    revalidate(cx, object, byte_index, "ArrayBuffer is detached")?;
    let old = modify(cx, object, byte_index, |_| None);
    decode(cx, kind, old & mask(kind))
}

pub(super) fn store(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let array = site.arg(cx, 0);
    let (object, kind, length) = validate(cx, array, false, false)?;
    let index = site.arg(cx, 1);
    let byte_index = index_in(cx, object, length, index)?;
    let value = site.arg(cx, 2);
    let (bits, result) = operand(cx, kind, value)?;
    revalidate(cx, object, byte_index, "ArrayBuffer is detached")?;
    let mask = mask(kind);
    modify(cx, object, byte_index, |_| Some(bits & mask));
    Ok(result)
}

pub(super) fn is_lock_free(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let size = site.arg(cx, 0);
    let size = internal::to_integer_or_infinity(cx, size)?;
    Ok(Value::bool(
        matches!(size as i64, 1 | 2 | 4 | 8) && crate::float::fract(size) == 0.0,
    ))
}

/// The coordinator's key for a shared buffer's byte: the region's own, so two
/// wrappers over one base wait and notify on the same place.
fn location(heap: &Heap, object: ObjectId, byte_index: usize) -> Option<WaitLocation> {
    let (_, store) = typed::view(heap, object);
    match &typed::buffer(heap, store).store {
        Store::Shared(shared) => Some(WaitLocation::new(shared.key(), byte_index)),
        Store::Owned(_) => None,
    }
}

/// `Atomics.wait(typedArray, index, value, timeout)`.
pub(super) fn wait(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let array = site.arg(cx, 0);
    let (object, kind, length) = validate(cx, array, true, true)?;
    let index = site.arg(cx, 1);
    let byte_index = index_in(cx, object, length, index)?;
    let place = location(cx.heap, object, byte_index).expect("a shared buffer");
    let value = site.arg(cx, 2);
    let expected = if kind.is_bigint() {
        operand(cx, kind, value)?.0
    } else {
        let number = ops::to_number(cx, value)?;
        number::to_int32(number) as u32 as u64
    };
    let timeout = site.arg(cx, 3);
    let timeout = ops::to_number(cx, timeout)?;
    let timeout = if timeout.is_nan() || timeout == f64::INFINITY {
        None
    } else {
        // A finite JS timeout can exceed Duration's range. Saturate it rather
        // than panicking, while keeping NaN/+Infinity as untimed waits.
        Some(Duration::try_from_secs_f64(timeout.max(0.0) / 1000.0).unwrap_or(Duration::MAX))
    };
    if !cx.host.can_block || cx.host.waiter.is_none() {
        return Err(error::type_error(cx, "cannot block in this thread"));
    }
    let (view, store) = typed::view(cx.heap, object);
    let size = view.kind.size();
    let mask = mask(kind);
    let buffer = typed::buffer(cx.heap, store);
    let outcome = cx
        .host
        .waiter
        .expect("a blocking waiter")
        .wait(place, timeout, &mut || {
            let mut bytes = [0u8; 8];
            buffer.read(byte_index, &mut bytes[..size]);
            u64::from_le_bytes(bytes) & mask == expected & mask
        });
    let text = match outcome {
        WaitOutcome::NotEqual => "not-equal",
        WaitOutcome::Ok => "ok",
        WaitOutcome::TimedOut => "timed-out",
    };
    super::string_text(cx, text)
}

/// `Atomics.notify(typedArray, index, count)`.
pub(super) fn notify(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let array = site.arg(cx, 0);
    let (object, _, length) = validate(cx, array, true, false)?;
    let index = site.arg(cx, 1);
    let byte_index = index_in(cx, object, length, index)?;
    let count = site.arg(cx, 2);
    let count = if count.is_undefined() {
        usize::MAX
    } else {
        let count = internal::to_integer_or_infinity(cx, count)?;
        if count == f64::INFINITY {
            usize::MAX
        } else {
            count.max(0.0) as usize
        }
    };
    let Some(place) = location(cx.heap, object, byte_index) else {
        return Ok(Value::int32(0));
    };
    let notified = cx
        .host
        .waiter
        .map_or(0, |waiter| waiter.notify(place, count));
    Ok(Value::number(notified as f64))
}

/// `Atomics.pause(N)`.
pub(super) fn pause(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let iterations = site.arg(cx, 0);
    if !iterations.is_undefined() {
        match iterations.as_f64() {
            Some(number) if number.is_finite() && crate::float::fract(number) == 0.0 => {}
            _ => return Err(error::type_error(cx, "not an integral number")),
        }
    }
    core::hint::spin_loop();
    Ok(Value::UNDEFINED)
}

#[cfg(test)]
#[allow(unsafe_code)]
mod tests {
    use alloc::boxed::Box;
    use alloc::sync::Arc;
    use core::sync::atomic::AtomicU8;

    use super::location;
    use crate::heap::gc::HeapLimits;
    use crate::runtime::Intrinsic;
    use crate::{Handle, NativeCx, Runtime, Value};

    /// One reference to the caller's bytes, which the engine drops with the
    /// buffer that holds it.
    struct Memory(#[allow(dead_code)] Arc<[AtomicU8]>);

    /// A region of `len` zero bytes the test owns, and its base.
    fn memory(len: usize) -> (Arc<[AtomicU8]>, *mut u8) {
        let bytes: Arc<[AtomicU8]> = (0..len).map(|_| AtomicU8::new(0)).collect();
        let base = bytes.as_ptr() as *mut u8;
        (bytes, base)
    }

    /// A `Uint8Array` over `buffer`, rendered through `%Uint8Array%`.
    fn view(cx: &mut NativeCx<'_>, buffer: Handle) -> Value {
        let constructor = Value::object(cx.intrinsic(Intrinsic::Uint8Array));
        cx.construct(constructor, &[cx.held(buffer)], constructor)
            .unwrap()
    }

    #[test]
    fn two_shared_buffers_over_one_base_share_the_atomics_key() {
        let mut runtime = Runtime::new(HeapLimits::default()).unwrap();
        let (bytes, base) = memory(64);
        let mut cx = runtime.native_cx();

        let first = unsafe {
            cx.new_shared_array_buffer_over(base, 16, 64, 0, Box::new(Memory(Arc::clone(&bytes))))
        }
        .unwrap();
        let first = cx.hold(Value::object(first));
        let second = unsafe {
            cx.new_shared_array_buffer_over(base, 32, 64, 0, Box::new(Memory(Arc::clone(&bytes))))
        }
        .unwrap();
        let second = cx.hold(Value::object(second));
        let one = view(&mut cx, first);
        let one = cx.hold(one);
        let two = view(&mut cx, second);
        let two = cx.hold(two);

        // Two wrappers over one base are one wait location: nothing else makes
        // the pair differ, so the whole location has to match.
        let here = location(cx.heap(), cx.held(one).as_object().unwrap(), 4);
        let there = location(cx.heap(), cx.held(two).as_object().unwrap(), 4);
        assert_eq!(here, there, "one base, one wait location");

        // The bytes of another region are another location.
        let (other, other_base) = memory(16);
        let third = unsafe {
            cx.new_shared_array_buffer_over(
                other_base,
                16,
                16,
                0,
                Box::new(Memory(Arc::clone(&other))),
            )
        }
        .unwrap();
        let third = cx.hold(Value::object(third));
        let elsewhere = view(&mut cx, third);
        let elsewhere = cx.hold(elsewhere);
        assert_ne!(
            here,
            location(cx.heap(), cx.held(elsewhere).as_object().unwrap(), 4),
            "another base is another location"
        );

        // A caller-supplied key wins over the one derived from the base.
        let named = unsafe {
            cx.new_shared_array_buffer_over(base, 16, 64, 7, Box::new(Memory(Arc::clone(&bytes))))
        }
        .unwrap();
        let named = cx.hold(Value::object(named));
        let named = view(&mut cx, named);
        let named = cx.hold(named);
        assert_ne!(
            here,
            location(cx.heap(), cx.held(named).as_object().unwrap(), 4),
            "a named key is its own location"
        );

        for handle in [named, elsewhere, third, two, one, second, first] {
            cx.release(handle);
        }
    }
}
