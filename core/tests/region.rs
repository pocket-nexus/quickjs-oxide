//! Buffers over a byte region the embedder owns: the base stays where the
//! caller put it, the bytes are the caller's, and the guard that keeps them
//! alive drops with the last buffer over them.

#![allow(unsafe_code)]

use std::any::Any;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

use oxide_core::heap::gc::HeapLimits;
use oxide_core::{Intrinsic, NativeCx, ObjectId, Runtime, Value};

/// What keeps a caller's bytes alive: a reference the test can drop, and the
/// flag it sets when it does.
struct Guard {
    /// The caller's reference to the bytes: dropping the guard is what
    /// releases them.
    _bytes: Arc<[AtomicU8]>,
    dropped: Arc<AtomicBool>,
}

impl Drop for Guard {
    fn drop(&mut self) {
        self.dropped.store(true, Ordering::SeqCst);
    }
}

/// `len` zero bytes the test owns, and their base.
fn memory(len: usize) -> (Arc<[AtomicU8]>, *mut u8) {
    let bytes: Arc<[AtomicU8]> = (0..len).map(|_| AtomicU8::new(0)).collect();
    let base = bytes.as_ptr() as *mut u8;
    (bytes, base)
}

/// The guard one buffer over `bytes` holds. Each buffer gets its own, as an
/// embedder would hand out one reference per buffer.
fn guard(bytes: &Arc<[AtomicU8]>, dropped: &Arc<AtomicBool>) -> Box<dyn Any + Send + Sync> {
    Box::new(Guard {
        _bytes: Arc::clone(bytes),
        dropped: Arc::clone(dropped),
    })
}

/// The bytes at `base`, read the way the caller reads its own memory.
fn host(base: *mut u8, len: usize) -> &'static [AtomicU8] {
    unsafe { std::slice::from_raw_parts(base as *const AtomicU8, len) }
}

/// A `Uint8Array` over `buffer`, rendered through `%Uint8Array%`.
fn uint8(cx: &mut NativeCx<'_>, buffer: ObjectId) -> ObjectId {
    let constructor = Value::object(cx.intrinsic(Intrinsic::Uint8Array));
    cx.construct(constructor, &[Value::object(buffer)], constructor)
        .unwrap()
        .as_object()
        .expect("a typed array")
}

/// The property key of a decimal index below ten.
fn index(cx: &mut NativeCx<'_>, digit: u16) -> oxide_core::PropertyKey {
    cx.key_from_units(&[b'0' as u16 + digit]).unwrap()
}

#[test]
fn an_array_buffer_over_a_foreign_region_round_trips() {
    let mut runtime = Runtime::new(HeapLimits::default()).unwrap();
    let dropped = Arc::new(AtomicBool::new(false));
    let (bytes, base) = memory(64);
    let mut cx = runtime.native_cx();
    let three = index(&mut cx, 3);

    let first =
        unsafe { cx.new_array_buffer_over(base, 16, 64, 0, guard(&bytes, &dropped)) }.unwrap();
    let first_handle = cx.hold(Value::object(first));
    assert_eq!(cx.array_buffer_len(first), Some(16));

    // A typed array over it writes into the caller's bytes.
    let view = uint8(&mut cx, first);
    cx.set(view, three, Value::int32(0xab), Value::object(view))
        .unwrap();
    assert_eq!(host(base, 64)[3].load(Ordering::SeqCst), 0xab);
    assert_eq!(cx.array_buffer_len(first), Some(16));

    // The engine's own entry writes and reads them too.
    cx.array_buffer_write(first, 5, &[7, 8]).unwrap();
    assert_eq!(host(base, 64)[5].load(Ordering::SeqCst), 7);
    let mut back = [0u8; 2];
    cx.array_buffer_read(first, 5, &mut back).unwrap();
    assert_eq!(back, [7, 8]);

    // The engine never hands out a slice over bytes another thread may write.
    assert!(cx.array_buffer_bytes(first).is_none());

    // Growing is another buffer over the same base at a new length.
    let second =
        unsafe { cx.new_array_buffer_over(base, 32, 64, 0, guard(&bytes, &dropped)) }.unwrap();
    let second_handle = cx.hold(Value::object(second));
    assert_eq!(cx.array_buffer_len(second), Some(32));
    let mut seen = [0u8; 2];
    cx.array_buffer_read(second, 5, &mut seen).unwrap();
    assert_eq!(seen, [7, 8], "the longer buffer sees the same bytes");
    let longer = uint8(&mut cx, second);
    let four = index(&mut cx, 4);
    cx.set(longer, four, Value::int32(0x5a), Value::object(longer))
        .unwrap();
    assert_eq!(
        host(base, 64)[4].load(Ordering::SeqCst),
        0x5a,
        "the base never moved"
    );
    assert_eq!(
        cx.array_buffer_len(first),
        Some(16),
        "the shorter buffer keeps its length"
    );

    // Detaching the shorter one releases its guard and leaves the bytes alone.
    cx.detach_array_buffer(Value::object(first)).unwrap();
    assert_eq!(cx.array_buffer_len(first), None);
    assert!(
        dropped.load(Ordering::SeqCst),
        "detaching releases the guard"
    );
    assert_eq!(host(base, 64)[3].load(Ordering::SeqCst), 0xab);

    cx.release(first_handle);
    cx.release(second_handle);
    cx.run_gc();
}

#[test]
fn a_foreign_region_is_released_with_its_last_buffer() {
    let mut runtime = Runtime::new(HeapLimits::default()).unwrap();
    let dropped = Arc::new(AtomicBool::new(false));
    let (bytes, base) = memory(8);
    let mut cx = runtime.native_cx();
    let buffer =
        unsafe { cx.new_array_buffer_over(base, 8, 8, 0, guard(&bytes, &dropped)) }.unwrap();
    let handle = cx.hold(Value::object(buffer));

    cx.run_gc();
    assert!(
        !dropped.load(Ordering::SeqCst),
        "a held buffer keeps its region alive"
    );

    cx.release(handle);
    cx.run_gc();
    assert!(
        dropped.load(Ordering::SeqCst),
        "the last buffer over the region releases it"
    );
}

#[test]
fn a_shared_array_buffer_over_a_foreign_region_round_trips() {
    let mut runtime = Runtime::new(HeapLimits::default()).unwrap();
    let dropped = Arc::new(AtomicBool::new(false));
    let (bytes, base) = memory(16);
    let mut cx = runtime.native_cx();
    let buffer =
        unsafe { cx.new_shared_array_buffer_over(base, 16, 16, 0, guard(&bytes, &dropped)) }
            .unwrap();
    let handle = cx.hold(Value::object(buffer));
    assert_eq!(cx.array_buffer_len(buffer), Some(16));

    let view = uint8(&mut cx, buffer);
    let at = index(&mut cx, 6);
    cx.set(view, at, Value::int32(9), Value::object(view))
        .unwrap();
    assert_eq!(host(base, 16)[6].load(Ordering::SeqCst), 9);

    // A shared buffer is still not a slice.
    assert!(cx.array_buffer_bytes(buffer).is_none());

    cx.release(handle);
    cx.run_gc();
    assert!(dropped.load(Ordering::SeqCst));
}
