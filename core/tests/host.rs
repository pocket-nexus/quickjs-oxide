//! Host functions and host objects: the embedder's Rust code as JavaScript
//! functions, its values attached to JavaScript objects. Passes under
//! `--features gc-stress`.

use std::any::Any;
use std::cell::Cell;
use std::rc::Rc;

use oxide_core::heap::gc::HeapLimits;
use oxide_core::vm::native::{HostFn, NativeCall};
use oxide_core::{ErrorKind, NativeCx, Runtime, Throw, Value};

fn runtime() -> Runtime {
    Runtime::new(HeapLimits::default()).unwrap()
}

/// A host function held by a handle.
fn host_function(
    runtime: &mut Runtime,
    name: &str,
    length: u32,
    constructor: bool,
    data: &[Value],
    body: impl Fn(&mut NativeCx<'_>, NativeCall) -> Result<Value, Throw> + 'static,
) -> Value {
    let body: Rc<HostFn> = Rc::new(body);
    let function = runtime
        .native_cx()
        .new_host_function(name, length, constructor, data, body)
        .unwrap();
    let function = Value::object(function);
    runtime.hold(function);
    function
}

fn throw_type_error(cx: &mut NativeCx<'_>, message: &str) -> Throw {
    Throw(cx.new_error(ErrorKind::Type, message).unwrap())
}

fn message(runtime: &mut Runtime, thrown: Value) -> String {
    let summary = runtime.thrown_summary(thrown).unwrap();
    format!(
        "{}: {}",
        summary.name.unwrap_or_default(),
        summary.message.unwrap_or_default()
    )
}

#[test]
fn a_host_function_reads_its_receiver_and_arguments() {
    let mut runtime = runtime();
    let add = host_function(&mut runtime, "add", 2, false, &[], |cx, call| {
        assert_eq!(call.argc(), 3);
        assert!(!call.is_construct());
        assert_eq!(call.new_target(), Value::UNDEFINED);
        assert_eq!(call.arg(cx, 7), Value::UNDEFINED);
        let this = call.this(cx).as_int32().unwrap();
        let sum: i32 = call.args(cx).iter().map(|v| v.as_int32().unwrap()).sum();
        Ok(Value::int32(this * 100 + sum))
    });
    let args = [Value::int32(1), Value::int32(2), Value::int32(3)];
    assert_eq!(
        runtime.call(add, Value::int32(4), &args),
        Ok(Value::int32(406))
    );
    // `name` and `length` are the embedder's.
    let name = runtime.key_from_units(&"name".encode_utf16().collect::<Vec<_>>());
    let length = runtime.key_from_units(&"length".encode_utf16().collect::<Vec<_>>());
    let object = add.as_object().unwrap();
    let name = runtime.get(object, name.unwrap(), add).unwrap();
    assert_eq!(runtime.string_lossy(name).as_deref(), Some("add"));
    assert_eq!(
        runtime.get(object, length.unwrap(), add),
        Ok(Value::int32(2))
    );
    assert_eq!(runtime.type_of(add), "function");
}

#[test]
fn new_calls_a_host_constructor_and_rejects_a_host_function() {
    let mut runtime = runtime();
    let point = host_function(&mut runtime, "Point", 0, true, &[], |cx, call| {
        assert!(call.is_construct());
        assert_eq!(call.new_target(), call.callee());
        let object = cx.new_object().map_err(|_| throw_type_error(cx, "oom"))?;
        Ok(Value::object(object))
    });
    assert!(runtime.is_constructor(point));
    let made = runtime.construct(point, &[], point).unwrap();
    assert!(made.as_object().is_some());

    let plain = host_function(&mut runtime, "plain", 0, false, &[], |_, _| {
        Ok(Value::UNDEFINED)
    });
    assert!(!runtime.is_constructor(plain));
    let Err(Throw(thrown)) = runtime.construct(plain, &[], plain) else {
        panic!("a host function was constructed");
    };
    assert_eq!(
        message(&mut runtime, thrown),
        "TypeError: plain is not a constructor"
    );
}

#[test]
fn captured_values_are_read_and_written_across_calls() {
    let mut runtime = runtime();
    let counter = host_function(
        &mut runtime,
        "counter",
        0,
        false,
        &[Value::int32(10)],
        |cx, call| {
            assert_eq!(call.data_len(cx), 1);
            let next = call.data(cx, 0).as_int32().unwrap() + 1;
            call.set_data(cx, 0, Value::int32(next));
            Ok(Value::int32(next))
        },
    );
    for expected in 11..14 {
        runtime.run_gc();
        assert_eq!(
            runtime.call(counter, Value::UNDEFINED, &[]),
            Ok(Value::int32(expected))
        );
    }
}

#[test]
fn a_thrown_value_propagates_to_the_caller() {
    let mut runtime = runtime();
    let fail = host_function(&mut runtime, "fail", 0, false, &[], |cx, _| {
        Err(throw_type_error(cx, "x is required"))
    });
    let Err(Throw(thrown)) = runtime.call(fail, Value::UNDEFINED, &[]) else {
        panic!("the body's throw was lost");
    };
    assert_eq!(message(&mut runtime, thrown), "TypeError: x is required");
}

#[test]
fn a_body_reenters_its_own_function() {
    let mut runtime = runtime();
    let depth = host_function(&mut runtime, "depth", 1, false, &[], |cx, call| {
        let n = call.arg(cx, 0).as_int32().unwrap();
        if n == 0 {
            return Ok(Value::int32(0));
        }
        let inner = cx.call(call.callee(), Value::UNDEFINED, &[Value::int32(n - 1)])?;
        Ok(Value::int32(inner.as_int32().unwrap() + 1))
    });
    assert_eq!(
        runtime.call(depth, Value::UNDEFINED, &[Value::int32(100)]),
        Ok(Value::int32(100))
    );
}

#[test]
fn unbounded_recursion_is_a_stack_overflow() {
    let mut runtime = runtime();
    let deepest = Rc::new(Cell::new(0u32));
    let seen = Rc::clone(&deepest);
    let recurse = host_function(&mut runtime, "recurse", 1, false, &[], move |cx, call| {
        let n = call.arg(cx, 0).as_int32().unwrap();
        seen.set(seen.get().max(n as u32));
        cx.call(call.callee(), Value::UNDEFINED, &[Value::int32(n + 1)])
    });
    let Err(Throw(thrown)) = runtime.call(recurse, Value::UNDEFINED, &[Value::int32(0)]) else {
        panic!("unbounded recursion returned");
    };
    assert_eq!(
        message(&mut runtime, thrown),
        "InternalError: stack overflow"
    );
    assert!(deepest.get() > 100);
    // The runtime runs normally after the overflow.
    let one = host_function(&mut runtime, "one", 0, false, &[], |_, _| {
        Ok(Value::int32(1))
    });
    assert_eq!(
        runtime.call(one, Value::UNDEFINED, &[]),
        Ok(Value::int32(1))
    );
}

#[test]
fn dropped_host_functions_free_their_records_and_bodies() {
    let mut runtime = runtime();
    runtime.run_gc();
    let before = runtime.memory_report().internals.live;
    let body_count = Rc::new(());
    for _ in 0..10_000 {
        let captured = Rc::clone(&body_count);
        let body: Rc<HostFn> = Rc::new(move |_, _| {
            let _ = &captured;
            Ok(Value::UNDEFINED)
        });
        runtime
            .native_cx()
            .new_host_function("f", 0, false, &[], body)
            .unwrap();
    }
    runtime.run_gc();
    assert_eq!(runtime.memory_report().internals.live, before);
    assert_eq!(Rc::strong_count(&body_count), 1);
}

#[test]
fn a_body_that_allocates_keeps_its_arguments() {
    let mut runtime = runtime();
    let concat = host_function(&mut runtime, "concat", 2, false, &[], |cx, call| {
        let first = cx.string_lossy(call.arg(cx, 0)).unwrap();
        // Allocations between the reads collect under `gc-stress`.
        for _ in 0..8 {
            cx.new_object().map_err(|_| throw_type_error(cx, "oom"))?;
        }
        let second = cx.string_lossy(call.arg(cx, 1)).unwrap();
        cx.new_string(&format!("{first}{second}"))
            .map_err(|_| throw_type_error(cx, "oom"))
    });
    let a = runtime.new_string("ab").unwrap();
    let a_handle = runtime.hold(a);
    let b = runtime.new_string("cd").unwrap();
    let b_handle = runtime.hold(b);
    let result = runtime.call(concat, Value::UNDEFINED, &[a, b]).unwrap();
    assert_eq!(runtime.string_lossy(result).as_deref(), Some("abcd"));
    runtime.release(a_handle);
    runtime.release(b_handle);
}

/// A datum whose drop the test observes through the `Rc`'s count.
struct Watched(#[allow(dead_code)] Rc<()>);

#[test]
fn a_host_object_holds_the_embedders_value() {
    let mut runtime = runtime();
    let prototype = runtime.new_object().unwrap();
    runtime.hold(Value::object(prototype));
    let object = runtime
        .native_cx()
        .new_host_object(Some(prototype), Rc::new(41_u32) as Rc<dyn Any>)
        .unwrap();
    runtime.hold(Value::object(object));
    let datum = runtime.native_cx().host_data(object).unwrap();
    assert_eq!(datum.downcast_ref::<u32>(), Some(&41));
    assert!(datum.downcast_ref::<String>().is_none());
    assert_eq!(runtime.get_prototype_of(object), Ok(Some(prototype)));
    assert_eq!(runtime.type_of(Value::object(object)), "object");
    // An ordinary object has no datum.
    assert!(runtime.native_cx().host_data(prototype).is_none());
}

#[test]
fn a_collected_host_object_drops_its_datum() {
    let mut runtime = runtime();
    let watch = Rc::new(());
    let kept = runtime
        .native_cx()
        .new_host_object(None, Rc::new(Watched(Rc::clone(&watch))))
        .unwrap();
    let handle = runtime.hold(Value::object(kept));
    runtime
        .native_cx()
        .new_host_object(None, Rc::new(Watched(Rc::clone(&watch))))
        .unwrap();
    runtime.run_gc();
    assert_eq!(
        Rc::strong_count(&watch),
        2,
        "only the unreachable datum drops"
    );
    runtime.release(handle);
    runtime.run_gc();
    assert_eq!(Rc::strong_count(&watch), 1);
}

#[test]
fn many_host_objects_survive_while_rooted() {
    let mut runtime = runtime();
    let array = runtime.new_array(0).unwrap();
    runtime.hold(Value::object(array));
    for index in 0..1_000u32 {
        let object = runtime
            .native_cx()
            .new_host_object(None, Rc::new(index))
            .unwrap();
        runtime
            .heap_mut()
            .set_element(array, index, Value::object(object))
            .unwrap();
    }
    runtime.run_gc();
    for index in [0u32, 499, 999] {
        let object = runtime
            .heap()
            .get_element(array, index)
            .as_object()
            .unwrap();
        let datum = runtime.native_cx().host_data(object).unwrap();
        assert_eq!(datum.downcast_ref::<u32>(), Some(&index));
    }
}
