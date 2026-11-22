//! Buffers over a byte region the embedder owns, built from inside a host
//! function: the path an embedding needs to hand a WebAssembly linear memory
//! to JavaScript.

#![allow(unsafe_code)]

use std::any::Any;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

use quickjs_oxide::engine::api::core::{Context, JsString, PropertyKey, Runtime, Scope, Value};

/// What keeps a caller's bytes alive, and the flag it sets when dropped.
struct Guard {
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

fn runtime() -> (Runtime, Context) {
    let runtime = Runtime::new();
    let context = runtime.context();
    (runtime, context)
}

/// Publishes `name` as a global holding `buffer`, so the script keeps it.
fn publish(
    scope: &mut Scope<'_>,
    name: &'static str,
    buffer: Value,
) -> Result<(), quickjs_oxide::engine::api::core::RuntimeError> {
    let key = PropertyKey::from_js_string(JsString::from_static(name));
    let target = scope.global_object()?;
    scope.set_property(&target, &key, buffer)?;
    Ok(())
}

#[test]
fn a_host_function_builds_a_buffer_over_the_callers_bytes() {
    use std::cell::Cell;

    let (_, context) = runtime();
    let dropped = Arc::new(AtomicBool::new(false));
    let (bytes, base) = memory(64);

    let calls = Rc::new(Cell::new(0usize));
    let seen_calls = Rc::clone(&calls);
    let global = context.global_object().unwrap();
    // One guard, handed out on the first call only.
    let kept: Rc<RefCell<Option<Box<dyn Any + Send + Sync>>>> =
        Rc::new(RefCell::new(Some(guard(&bytes, &dropped))));
    context
        .define_function(&global, "make", 0, move |scope: &mut Scope<'_>| {
            let guard = kept.borrow_mut().take();
            if let Some(guard) = guard {
                // SAFETY: `base` is the test's own 64 bytes, alive for the
                // whole test, and the guard is the only thing that releases
                // them.
                let buffer = unsafe { scope.new_array_buffer_over(base, 16, 64, 0, guard) }?;
                publish(scope, "buffer", Value::Object(buffer))?;
            }
            seen_calls.set(seen_calls.get() + 1);
            Ok(Value::Undefined)
        })
        .unwrap();
    drop(global);

    context.eval("make()").unwrap();
    assert_eq!(calls.get(), 1);
    assert!(!dropped.load(Ordering::SeqCst));
    assert_eq!(
        context.eval("buffer.byteLength").unwrap(),
        Value::Float(16.0),
        "the length is the sub-region, not the capacity"
    );
    assert_eq!(
        host(base, 64)[0].load(Ordering::SeqCst),
        0,
        "the bytes are the caller's, untouched"
    );
}

#[test]
fn a_buffer_from_a_host_function_aliases_the_bytes_both_ways() {
    let (_, context) = runtime();
    let dropped = Arc::new(AtomicBool::new(false));
    let (bytes, base) = memory(64);

    let global = context.global_object().unwrap();
    let kept: Rc<RefCell<Option<Box<dyn Any + Send + Sync>>>> =
        Rc::new(RefCell::new(Some(guard(&bytes, &dropped))));
    context
        .define_function(&global, "make", 0, move |scope: &mut Scope<'_>| {
            if let Some(guard) = kept.borrow_mut().take() {
                // SAFETY: as above.
                let buffer = unsafe { scope.new_array_buffer_over(base, 16, 64, 0, guard) }?;
                publish(scope, "buffer", Value::Object(buffer))?;
            }
            Ok(Value::Undefined)
        })
        .unwrap();
    drop(global);
    context.eval("make()").unwrap();

    // JavaScript writes; the caller's bytes change.
    context
        .eval("var u8 = new Uint8Array(buffer); u8[3] = 0xab;")
        .unwrap();
    assert_eq!(host(base, 64)[3].load(Ordering::SeqCst), 0xab);

    // The caller writes; JavaScript sees it.
    host(base, 64)[5].store(0x5a, Ordering::SeqCst);
    assert_eq!(
        context.eval("new Uint8Array(buffer)[5]").unwrap(),
        Value::Int(0x5a)
    );
    assert!(!dropped.load(Ordering::SeqCst));
}

#[test]
fn a_host_function_builds_a_shared_buffer_over_the_callers_bytes() {
    let (_, context) = runtime();
    let dropped = Arc::new(AtomicBool::new(false));
    let (bytes, base) = memory(16);

    let global = context.global_object().unwrap();
    let kept: Rc<RefCell<Option<Box<dyn Any + Send + Sync>>>> =
        Rc::new(RefCell::new(Some(guard(&bytes, &dropped))));
    context
        .define_function(&global, "makeShared", 0, move |scope: &mut Scope<'_>| {
            if let Some(guard) = kept.borrow_mut().take() {
                // SAFETY: as above.
                let buffer = unsafe { scope.new_shared_array_buffer_over(base, 16, 16, 0, guard) }?;
                publish(scope, "shared", Value::Object(buffer))?;
            }
            Ok(Value::Undefined)
        })
        .unwrap();
    drop(global);
    context.eval("makeShared()").unwrap();

    assert_eq!(
        context
            .eval("Object.prototype.toString.call(shared)")
            .unwrap(),
        Value::String(JsString::from_static("[object SharedArrayBuffer]"))
    );
    context
        .eval("var su8 = new Uint8Array(shared); su8[6] = 9;")
        .unwrap();
    assert_eq!(host(base, 16)[6].load(Ordering::SeqCst), 9);
    assert!(!dropped.load(Ordering::SeqCst));
}

#[test]
fn a_buffer_from_a_host_function_releases_its_region_with_the_buffer() {
    let (runtime, context) = runtime();
    let dropped = Arc::new(AtomicBool::new(false));
    let (bytes, base) = memory(8);

    let global = context.global_object().unwrap();
    let kept: Rc<RefCell<Option<Box<dyn Any + Send + Sync>>>> =
        Rc::new(RefCell::new(Some(guard(&bytes, &dropped))));
    context
        .define_function(&global, "makeTransient", 0, move |scope: &mut Scope<'_>| {
            let Some(guard) = kept.borrow_mut().take() else {
                return Ok(Value::Undefined);
            };
            // SAFETY: as above.
            let buffer = unsafe { scope.new_array_buffer_over(base, 8, 8, 0, guard) }?;
            // Returned but never stored: nothing in the script keeps it.
            Ok(Value::Object(buffer))
        })
        .unwrap();
    drop(global);

    context.eval("makeTransient();").unwrap();
    context.eval("void 0").unwrap();
    runtime.run_gc().unwrap();
    assert!(
        dropped.load(Ordering::SeqCst),
        "the last buffer over the region releases it"
    );
}
