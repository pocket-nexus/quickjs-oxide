//! Host observations, brands and producers remain complete under one State lease.
use super::*;
use crate::engine::{
    api::{Runtime, Value},
    atom::{AtomIdx, pinned::PinnedAtom},
    builtins::native::DateGetFieldKind,
    heap::{HeapError, PropertySlot, RawId, RawValue},
};
use std::{cell::RefCell, rc::Rc};

#[derive(Debug)]
struct RecordingHost {
    clock: Rc<Cell<usize>>,
    timezone: Rc<RefCell<Vec<i64>>>,
    panic_timezone: bool,
}
impl HostServices for RecordingHost {
    fn now_millis(&self) -> i64 {
        self.clock.set(self.clock.get() + 1);
        42
    }
    fn timezone_offset_minutes(&self, instant: i64) -> i32 {
        self.timezone.borrow_mut().push(instant);
        assert!(!self.panic_timezone, "timezone interrupted");
        -90
    }
    fn random_seed(&self) -> u64 {
        1
    }
}

type DateTestRuntime = (Runtime, Rc<Cell<usize>>, Rc<RefCell<Vec<i64>>>);

fn runtime(panic_timezone: bool) -> DateTestRuntime {
    let clock = Rc::new(Cell::new(0));
    let timezone = Rc::new(RefCell::new(Vec::new()));
    (
        Runtime::new_with_host_services(RecordingHost {
            clock: clock.clone(),
            timezone: timezone.clone(),
            panic_timezone,
        }),
        clock,
        timezone,
    )
}

fn message(state: &RuntimeState, error: &JsValue) -> JsString {
    let JsValue::Object(error) = error else {
        panic!("Date error object")
    };
    let data = state.heap.object(*error).unwrap();
    let key = state.pinned_atoms.get(PinnedAtom::Message);
    let slot = state
        .heap
        .shape(data.shape)
        .unwrap()
        .find(AtomIdx::from_raw(key.raw()))
        .unwrap() as usize;
    let PropertySlot::Data(RawValue::String(string)) = data.slots[slot] else {
        panic!("Date error message");
    };
    state.heap.string(string).unwrap().clone()
}

fn call(
    state: &mut RuntimeState,
    runtime: &Runtime,
    realm: ContextId,
    kind: DateNativeKind,
    receiver: JsValue,
) -> Completion {
    state
        .call_date_readonly_native(
            &runtime.0.poisoned,
            runtime.0.host_services.as_ref(),
            realm,
            kind,
            &NativeInvocation::Call {
                this_value: receiver,
            },
        )
        .unwrap()
}

fn returned(completion: Completion) -> JsValue {
    let Completion::Return(value) = completion else {
        panic!("Date return expected")
    };
    value
}

#[test]
fn readonly_date_all_strings_and_fields_preserve_exact_host_queries_and_results() {
    let (runtime, clock, timezone) = runtime(false);
    let mut context = runtime.new_context().unwrap();
    let Value::Object(date) = context.eval("new Date(946684800123)").unwrap() else {
        unreachable!()
    };
    let id = date.object_id();
    let runtime_owners = Rc::strong_count(&runtime.0);
    let host_owners = Rc::strong_count(&runtime.0.host_services);
    let mut state = runtime.0.state.borrow_mut();
    assert_eq!(
        returned(call(
            &mut state,
            &runtime,
            context.realm,
            DateNativeKind::Now,
            JsValue::Null
        )),
        JsValue::Int(42)
    );
    assert_eq!(clock.get(), 1);
    assert!(timezone.borrow().is_empty());
    assert_eq!(
        returned(call(
            &mut state,
            &runtime,
            context.realm,
            DateNativeKind::TimeValue,
            JsValue::Object(id)
        )),
        JsValue::Float(946684800123.0)
    );
    for (kind, expected, local) in [
        (
            DateStringMethod::String,
            "Sat Jan 01 2000 01:30:00 GMT+0130",
            true,
        ),
        (DateStringMethod::DateString, "Sat Jan 01 2000", true),
        (DateStringMethod::TimeString, "01:30:00 GMT+0130", true),
        (
            DateStringMethod::UtcString,
            "Sat, 01 Jan 2000 00:00:00 GMT",
            false,
        ),
        (
            DateStringMethod::IsoString,
            "2000-01-01T00:00:00.123Z",
            false,
        ),
        (
            DateStringMethod::LocaleString,
            "01/01/2000, 01:30:00 AM",
            true,
        ),
        (DateStringMethod::LocaleDateString, "01/01/2000", true),
        (DateStringMethod::LocaleTimeString, "01:30:00 AM", true),
    ] {
        timezone.borrow_mut().clear();
        let Completion::Return(JsValue::String(string)) = call(
            &mut state,
            &runtime,
            context.realm,
            DateNativeKind::String(kind),
            JsValue::Object(id),
        ) else {
            panic!("Date string {kind:?}")
        };
        assert_eq!(
            *state.heap.string(string).unwrap(),
            JsString::from_static(expected)
        );
        assert_eq!(
            &*timezone.borrow(),
            if local { &[946684800123][..] } else { &[] }
        );
        assert_eq!(state.heap.object_strong_count(id), Ok(1));
        state
            .release_owned_jsvalue(&runtime.0.poisoned, JsValue::String(string))
            .unwrap();
        assert!(state.heap.string(string).is_err());
    }
    for (kind, expected) in [
        (DateGetFieldKind::Year, 100),
        (DateGetFieldKind::FullYear, 2000),
        (DateGetFieldKind::UtcFullYear, 2000),
        (DateGetFieldKind::Month, 0),
        (DateGetFieldKind::UtcMonth, 0),
        (DateGetFieldKind::Date, 1),
        (DateGetFieldKind::UtcDate, 1),
        (DateGetFieldKind::Hours, 1),
        (DateGetFieldKind::UtcHours, 0),
        (DateGetFieldKind::Minutes, 30),
        (DateGetFieldKind::UtcMinutes, 0),
        (DateGetFieldKind::Seconds, 0),
        (DateGetFieldKind::UtcSeconds, 0),
        (DateGetFieldKind::Milliseconds, 123),
        (DateGetFieldKind::UtcMilliseconds, 123),
        (DateGetFieldKind::Day, 6),
        (DateGetFieldKind::UtcDay, 6),
    ] {
        timezone.borrow_mut().clear();
        assert_eq!(
            returned(call(
                &mut state,
                &runtime,
                context.realm,
                DateNativeKind::GetField(kind),
                JsValue::Object(id)
            )),
            JsValue::Int(expected)
        );
        assert_eq!(
            &*timezone.borrow(),
            if kind.uses_local_time() {
                &[946684800123][..]
            } else {
                &[]
            }
        );
        assert_eq!(state.heap.object_strong_count(id), Ok(1));
    }
    timezone.borrow_mut().clear();
    assert_eq!(
        returned(call(
            &mut state,
            &runtime,
            context.realm,
            DateNativeKind::TimezoneOffset,
            JsValue::Object(id)
        )),
        JsValue::Int(-90)
    );
    assert_eq!(&*timezone.borrow(), &[946684800123]);
    assert_eq!(Rc::strong_count(&runtime.0), runtime_owners);
    assert_eq!(Rc::strong_count(&runtime.0.host_services), host_owners);
    assert!(!runtime.0.deferred_references.has_pending());
}

#[test]
fn readonly_date_nan_preserves_iso_error_and_never_queries_timezone() {
    let (runtime, _, timezone) = runtime(false);
    let mut context = runtime.new_context().unwrap();
    let Value::Object(date) = context.eval("new Date(NaN)").unwrap() else {
        unreachable!()
    };
    let id = date.object_id();
    let mut state = runtime.0.state.borrow_mut();
    for kind in DateStringMethod::ALL {
        let completion = call(
            &mut state,
            &runtime,
            context.realm,
            DateNativeKind::String(kind),
            JsValue::Object(id),
        );
        let value = match completion {
            Completion::Throw(error) if kind == DateStringMethod::IsoString => {
                assert_eq!(
                    message(&state, &error),
                    JsString::from_static("Date value is NaN")
                );
                error
            }
            Completion::Return(JsValue::String(string)) if kind != DateStringMethod::IsoString => {
                assert_eq!(
                    *state.heap.string(string).unwrap(),
                    JsString::from_static("Invalid Date")
                );
                JsValue::String(string)
            }
            _ => panic!("invalid Date string {kind:?}"),
        };
        state
            .release_owned_jsvalue(&runtime.0.poisoned, value)
            .unwrap();
    }
    for kind in DateGetFieldKind::ALL
        .into_iter()
        .map(DateNativeKind::GetField)
        .chain([DateNativeKind::TimeValue, DateNativeKind::TimezoneOffset])
    {
        assert!(
            matches!(call(&mut state, &runtime, context.realm, kind, JsValue::Object(id)),
            Completion::Return(JsValue::Float(value)) if value.is_nan())
        );
        assert_eq!(state.heap.object_strong_count(id), Ok(1));
    }
    assert!(timezone.borrow().is_empty());
    assert!(!runtime.is_poisoned());
}

#[test]
fn readonly_date_wrong_brands_never_coerce_or_observe_host() {
    let (runtime, _, timezone) = runtime(false);
    let mut context = runtime.new_context().unwrap();
    let mut receivers = vec![
        JsValue::Undefined,
        JsValue::Null,
        JsValue::Bool(true),
        JsValue::Int(0),
        JsValue::Float(1.5),
        JsValue::ShortBigInt(2),
    ];
    for source in [
        "'text'",
        "123456789012345678901234567890n",
        "Symbol('x')",
        "Date.prototype",
        "globalThis.dateBrandCalls=0;({valueOf(){dateBrandCalls++;throw 99;}})",
        "new Proxy(new Date(0),{get(){dateBrandCalls++;throw 99;}})",
    ] {
        receivers.push(runtime.into_jsvalue(context.eval(source).unwrap()).unwrap());
    }
    let mut state = runtime.0.state.borrow_mut();
    for receiver in &receivers {
        for kind in [DateNativeKind::TimeValue, DateNativeKind::TimezoneOffset]
            .into_iter()
            .chain(DateStringMethod::ALL.map(DateNativeKind::String))
            .chain(DateGetFieldKind::ALL.map(DateNativeKind::GetField))
        {
            let Completion::Throw(error) = call(
                &mut state,
                &runtime,
                context.realm,
                kind,
                JsValue::from_raw(receiver.as_raw()).unwrap(),
            ) else {
                panic!("wrong Date brand")
            };
            assert_eq!(
                message(&state, &error),
                JsString::from_static("not a Date object")
            );
            state
                .release_owned_jsvalue(&runtime.0.poisoned, error)
                .unwrap();
        }
    }
    for receiver in receivers {
        state
            .release_owned_jsvalue(&runtime.0.poisoned, receiver)
            .unwrap();
    }
    assert!(timezone.borrow().is_empty());
    drop(state);
    assert_eq!(context.eval("dateBrandCalls").unwrap(), Value::Int(0));
}

#[test]
fn readonly_date_temporary_checked_retain_overflow_precedes_host_and_preserves_owner() {
    let (runtime, _, timezone) = runtime(false);
    let mut context = runtime.new_context().unwrap();
    let Value::Object(date) = context.eval("new Date(0)").unwrap() else {
        unreachable!()
    };
    let id = date.object_id();
    let mut state = runtime.0.state.borrow_mut();
    let saved = state.heap.object_strong_count(id).unwrap();
    state
        .heap
        .set_strong_count_for_test(RawId::Object(id), u32::MAX);
    let result = state.call_date_readonly_native(
        &runtime.0.poisoned,
        runtime.0.host_services.as_ref(),
        context.realm,
        DateNativeKind::TimezoneOffset,
        &NativeInvocation::Call {
            this_value: JsValue::Object(id),
        },
    );
    let count = state.heap.object_strong_count(id);
    state
        .heap
        .set_strong_count_for_test(RawId::Object(id), saved);
    assert!(matches!(
        result,
        Err(RuntimeError::Heap(HeapError::Overflow { .. }))
    ));
    assert_eq!(count, Ok(u32::MAX));
    assert!(timezone.borrow().is_empty());
    assert!(!runtime.is_poisoned());
    assert_eq!(
        returned(call(
            &mut state,
            &runtime,
            context.realm,
            DateNativeKind::TimezoneOffset,
            JsValue::Object(id)
        )),
        JsValue::Int(-90)
    );
}

#[test]
fn readonly_date_brand_error_factory_failure_releases_checked_receiver_temporary() {
    let (runtime, _, timezone) = runtime(false);
    let context = runtime.new_context().unwrap();
    let receiver = runtime.new_object(None).unwrap();
    let mut state = runtime.0.state.borrow_mut();
    let prototype = state
        .heap
        .context(context.realm)
        .unwrap()
        .native_error_prototypes[NativeErrorKind::Type.index()]
    .unwrap();
    let saved = state.heap.object_strong_count(prototype).unwrap();
    state
        .heap
        .set_strong_count_for_test(RawId::Object(prototype), u32::MAX);
    let result = state.call_date_readonly_native(
        &runtime.0.poisoned,
        runtime.0.host_services.as_ref(),
        context.realm,
        DateNativeKind::TimeValue,
        &NativeInvocation::Call {
            this_value: JsValue::Object(receiver.object_id()),
        },
    );
    state
        .heap
        .set_strong_count_for_test(RawId::Object(prototype), saved);
    assert!(matches!(
        result,
        Err(RuntimeError::Heap(HeapError::Overflow { .. }))
    ));
    assert_eq!(state.heap.object_strong_count(receiver.object_id()), Ok(1));
    assert!(timezone.borrow().is_empty());
    assert!(!runtime.is_poisoned());
}

#[test]
fn readonly_date_host_panic_quarantines_after_the_original_temporary_release() {
    let (runtime, _, timezone) = runtime(true);
    let mut context = runtime.new_context().unwrap();
    let Value::Object(date) = context.eval("new Date(0)").unwrap() else {
        unreachable!()
    };
    let id = date.object_id();
    let mut state = runtime.0.state.borrow_mut();
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        call(
            &mut state,
            &runtime,
            context.realm,
            DateNativeKind::TimezoneOffset,
            JsValue::Object(id),
        );
    }));
    assert!(caught.is_err());
    assert!(runtime.is_poisoned());
    assert_eq!(
        state.heap.object_strong_count(id),
        Ok(1),
        "temporary retired before the host observation"
    );
    assert_eq!(&*timezone.borrow(), &[0]);
    assert!(!runtime.0.deferred_references.has_pending());
}
