//! Source-prepared witnesses for all constructor/static raw phases.
use super::*;
use crate::engine::{
    api::Value,
    heap::{HeapError, RawId},
    object::ObjectRef,
};
use std::{cell::RefCell, rc::Rc};
mod consumers;
mod ownership;

#[derive(Debug)]
struct RecordingHost {
    clock: Rc<Cell<usize>>,
    zone: Rc<RefCell<Vec<i64>>>,
    panic_zone: bool,
}
impl HostServices for RecordingHost {
    fn now_millis(&self) -> i64 {
        self.clock.set(self.clock.get() + 1);
        42
    }
    fn timezone_offset_minutes(&self, instant: i64) -> i32 {
        self.zone.borrow_mut().push(instant);
        assert!(!self.panic_zone, "constructor timezone interrupted");
        0
    }
    fn random_seed(&self) -> u64 {
        1
    }
}
type RuntimeFixture = (Runtime, Rc<Cell<usize>>, Rc<RefCell<Vec<i64>>>);
fn runtime(panic_zone: bool) -> RuntimeFixture {
    let clock = Rc::new(Cell::new(0));
    let zone = Rc::new(RefCell::new(Vec::new()));
    (
        Runtime::new_with_host_services(RecordingHost {
            clock: clock.clone(),
            zone: zone.clone(),
            panic_zone,
        }),
        clock,
        zone,
    )
}
fn start(
    state: &mut RuntimeState,
    runtime: &Runtime,
    realm: ContextId,
    kind: DateNativeKind,
    target: JsValue,
    args: Vec<JsValue>,
) -> Result<DateConstructorStep, RuntimeError> {
    let invocation = if kind == DateNativeKind::Constructor {
        NativeInvocation::Construct { new_target: target }
    } else {
        NativeInvocation::Call { this_value: target }
    };
    let count = args.len();
    DateConstructorStep::start_in_state(
        state,
        &runtime.0.poisoned,
        runtime.0.host_services.as_ref(),
        realm,
        kind,
        &invocation,
        &NativeArguments {
            readable: args,
            actual_arg_count: count,
        },
    )
}
fn object(context: &mut crate::engine::api::Context, source: &str) -> ObjectRef {
    let Value::Object(object) = context.eval(source).unwrap() else {
        panic!("object fixture")
    };
    object
}
fn retired(step: DateConstructorStep, state: &mut RuntimeState, runtime: &Runtime) {
    step.retire_in_state(state, &runtime.0.poisoned).unwrap();
}

#[test]
fn date_constructor_function_ignores_saturated_argv_without_snapshot_or_clock_repetition() {
    let (runtime, clock, zone) = runtime(false);
    let context = runtime.new_context().unwrap();
    let ignored = runtime.new_object(None).unwrap();
    let mut state = runtime.0.state.borrow_mut();
    let count = state.heap.object_strong_count(ignored.object_id()).unwrap();
    state
        .heap
        .set_strong_count_for_test(RawId::Object(ignored.object_id()), u32::MAX);
    let step = start(
        &mut state,
        &runtime,
        context.realm,
        DateNativeKind::Constructor,
        JsValue::Undefined,
        vec![JsValue::Object(ignored.object_id())],
    )
    .unwrap();
    assert!(matches!(
        step,
        DateConstructorStep::Complete(Completion::Return(JsValue::String(_)))
    ));
    assert_eq!(clock.get(), 1);
    assert_eq!(zone.borrow().as_slice(), &[42]);
    retired(step, &mut state, &runtime);
    state
        .heap
        .set_strong_count_for_test(RawId::Object(ignored.object_id()), count);
}
#[test]
fn date_constructor_raw_snapshots_are_owned_without_runtime_or_host_clones() {
    let (runtime, _, _) = runtime(false);
    let context = runtime.new_context().unwrap();
    let target = runtime.new_object(None).unwrap();
    let arg = runtime.new_object(None).unwrap();
    let runtime_count = Rc::strong_count(&runtime.0);
    let host_count = Rc::strong_count(&runtime.0.host_services);
    let mut state = runtime.0.state.borrow_mut();
    let step = start(
        &mut state,
        &runtime,
        context.realm,
        DateNativeKind::Constructor,
        JsValue::Object(target.object_id()),
        vec![JsValue::Object(arg.object_id())],
    )
    .unwrap();
    assert!(matches!(step, DateConstructorStep::Primitive { .. }));
    assert_eq!(
        state.heap.object_strong_count(target.object_id()).unwrap(),
        2
    );
    assert_eq!(state.heap.object_strong_count(arg.object_id()).unwrap(), 2);
    assert_eq!(Rc::strong_count(&runtime.0), runtime_count);
    assert_eq!(Rc::strong_count(&runtime.0.host_services), host_count);
    retired(step, &mut state, &runtime);
    assert_eq!(
        state.heap.object_strong_count(target.object_id()).unwrap(),
        1
    );
    assert_eq!(state.heap.object_strong_count(arg.object_id()).unwrap(), 1);
}
#[test]
fn date_utc_converts_all_seven_after_nan_and_ignores_eighth_saturated_edge() {
    let (runtime, clock, zone) = runtime(false);
    let context = runtime.new_context().unwrap();
    let ignored = runtime.new_object(None).unwrap();
    let mut state = runtime.0.state.borrow_mut();
    let saved = state.heap.object_strong_count(ignored.object_id()).unwrap();
    state
        .heap
        .set_strong_count_for_test(RawId::Object(ignored.object_id()), u32::MAX);
    let mut args = vec![JsValue::Float(f64::NAN)];
    args.extend((1..7).map(JsValue::Int));
    args.push(JsValue::Object(ignored.object_id()));
    let mut step = start(
        &mut state,
        &runtime,
        context.realm,
        DateNativeKind::Utc,
        JsValue::Undefined,
        args,
    )
    .unwrap();
    for index in 0..7 {
        let DateConstructorStep::Number { value, resume } = step else {
            panic!("field request {index}")
        };
        let number = match value {
            JsValue::Float(value) => value,
            JsValue::Int(value) => f64::from(value),
            _ => panic!("field input"),
        };
        step = resume
            .number_in_state(
                &mut state,
                &runtime.0.poisoned,
                runtime.0.host_services.as_ref(),
                NativeConversion::Value(number),
            )
            .unwrap();
    }
    assert!(
        matches!(step, DateConstructorStep::Complete(Completion::Return(JsValue::Float(value))) if value.is_nan())
    );
    assert_eq!(clock.get(), 0);
    assert!(zone.borrow().is_empty());
    retired(step, &mut state, &runtime);
    state
        .heap
        .set_strong_count_for_test(RawId::Object(ignored.object_id()), saved);
}
#[test]
fn date_parse_independent_request_retain_overflow_preserves_immortal_snapshot_sentinel() {
    let (runtime, _, _) = runtime(false);
    let context = runtime.new_context().unwrap();
    let arg = runtime.new_object(None).unwrap();
    let mut state = runtime.0.state.borrow_mut();
    let saved = state.heap.object_strong_count(arg.object_id()).unwrap();
    state
        .heap
        .set_strong_count_for_test(RawId::Object(arg.object_id()), u32::MAX - 1);
    let result = start(
        &mut state,
        &runtime,
        context.realm,
        DateNativeKind::Parse,
        JsValue::Undefined,
        vec![JsValue::Object(arg.object_id())],
    );
    assert!(matches!(
        result,
        Err(RuntimeError::Heap(HeapError::Overflow { .. }))
    ));
    assert!(!runtime.is_poisoned());
    assert_eq!(
        state.heap.object_strong_count(arg.object_id()).unwrap(),
        u32::MAX
    );
    state
        .heap
        .set_strong_count_for_test(RawId::Object(arg.object_id()), saved);
}
#[test]
fn date_constructor_genuine_date_bypasses_primitive_hooks_and_clips_only_payload_write() {
    let (runtime, clock, zone) = runtime(false);
    let mut context = runtime.new_context().unwrap();
    let date = object(&mut context, "new Date(123)");
    let target = runtime.new_object(None).unwrap();
    let mut state = runtime.0.state.borrow_mut();
    let step = start(
        &mut state,
        &runtime,
        context.realm,
        DateNativeKind::Constructor,
        JsValue::Object(target.object_id()),
        vec![JsValue::Object(date.object_id())],
    )
    .unwrap();
    let DateConstructorStep::Read {
        receiver, resume, ..
    } = step
    else {
        panic!("genuine Date bypasses Primitive")
    };
    state
        .release_owned_jsvalue(&runtime.0.poisoned, receiver)
        .unwrap();
    let step = resume
        .resume_in_state(
            &mut state,
            &runtime.0.poisoned,
            Completion::Return(JsValue::Null),
        )
        .unwrap();
    let DateConstructorStep::CyclePublished(Completion::Return(value)) = step else {
        panic!("fresh Date fact")
    };
    assert_eq!(state.genuine_date_value(&value).unwrap(), Some(123.0));
    state
        .release_owned_jsvalue(&runtime.0.poisoned, value)
        .unwrap();
    assert_eq!(clock.get(), 0);
    assert!(zone.borrow().is_empty());
    assert_eq!(state.heap.object_strong_count(date.object_id()).unwrap(), 1);
}
#[test]
fn date_constructor_propagated_child_throw_has_no_fresh_publication_fact() {
    let (runtime, _, _) = runtime(false);
    let context = runtime.new_context().unwrap();
    let input = runtime.new_object(None).unwrap();
    let exception = runtime.new_object(None).unwrap().into_handle();
    let mut state = runtime.0.state.borrow_mut();
    let step = start(
        &mut state,
        &runtime,
        context.realm,
        DateNativeKind::Parse,
        JsValue::Undefined,
        vec![JsValue::Object(input.object_id())],
    )
    .unwrap();
    let DateConstructorStep::String { value, resume } = step else {
        panic!("parse request")
    };
    state
        .release_owned_jsvalue(&runtime.0.poisoned, value)
        .unwrap();
    let step = resume
        .string_in_state(
            &mut state,
            &runtime.0.poisoned,
            runtime.0.host_services.as_ref(),
            NativeConversion::Throw(JsValue::Object(exception)),
        )
        .unwrap();
    assert!(
        matches!(step, DateConstructorStep::Complete(Completion::Throw(JsValue::Object(id))) if id==exception)
    );
    assert_eq!(
        state.heap.object_strong_count(input.object_id()).unwrap(),
        1
    );
    retired(step, &mut state, &runtime);
    assert!(state.heap.object(exception).is_err());
}
#[test]
fn date_constructor_wrong_reply_phase_retires_input_and_pending_owners() {
    let (runtime, _, _) = runtime(false);
    let context = runtime.new_context().unwrap();
    let pending = runtime.new_object(None).unwrap();
    let incoming = runtime.new_object(None).unwrap().into_handle();
    let mut state = runtime.0.state.borrow_mut();
    let step = start(
        &mut state,
        &runtime,
        context.realm,
        DateNativeKind::Parse,
        JsValue::Undefined,
        vec![JsValue::Object(pending.object_id())],
    )
    .unwrap();
    let DateConstructorStep::String { value, resume } = step else {
        panic!("parse request")
    };
    state
        .release_owned_jsvalue(&runtime.0.poisoned, value)
        .unwrap();
    assert!(matches!(
        resume.primitive_in_state(
            &mut state,
            &runtime.0.poisoned,
            runtime.0.host_services.as_ref(),
            Completion::Return(JsValue::Object(incoming))
        ),
        Err(RuntimeError::Invariant("Date primitive phase mismatch"))
    ));
    assert!(state.heap.object(incoming).is_err());
    assert_eq!(
        state.heap.object_strong_count(pending.object_id()).unwrap(),
        1
    );
    assert!(!runtime.is_poisoned());
}
#[test]
fn date_constructor_host_panic_quarantines_raw_resume_before_retiring_edges() {
    let (runtime, _, _) = runtime(true);
    let context = runtime.new_context().unwrap();
    let input = runtime.new_object(None).unwrap();
    let mut state = runtime.0.state.borrow_mut();
    let step = start(
        &mut state,
        &runtime,
        context.realm,
        DateNativeKind::Parse,
        JsValue::Undefined,
        vec![JsValue::Object(input.object_id())],
    )
    .unwrap();
    let DateConstructorStep::String { value, resume } = step else {
        panic!("parse request")
    };
    state
        .release_owned_jsvalue(&runtime.0.poisoned, value)
        .unwrap();
    let before = state.heap.object_strong_count(input.object_id()).unwrap();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        resume.string_in_state(
            &mut state,
            &runtime.0.poisoned,
            runtime.0.host_services.as_ref(),
            NativeConversion::Value(JsString::from_static("2000-01-01T00:00:00")),
        )
    }));
    assert!(result.is_err());
    assert!(runtime.is_poisoned());
    assert_eq!(
        state.heap.object_strong_count(input.object_id()).unwrap(),
        before
    );
}
