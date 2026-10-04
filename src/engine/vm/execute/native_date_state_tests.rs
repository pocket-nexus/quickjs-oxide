//! Every registered read-only Date selector runs through the actual held-State loop.
use crate::engine::{
    api::{Runtime, Value},
    builtins::native::{DateGetFieldKind, DateStringMethod},
    code::{
        bytecode::Instruction,
        function::{UnlinkedFunction, metadata::FunctionMetadata},
    },
    heap::ObjectId,
    host::HostServices,
    value::{JsString, JsValue},
    vm::{
        call::CallableExecution,
        execution::{ExecutionLimits, RunningExecution},
        frame::FrameId,
    },
};
use std::{cell::Cell, rc::Rc};

#[derive(Debug)]
struct DateHost(Rc<Cell<usize>>);
impl HostServices for DateHost {
    fn now_millis(&self) -> i64 {
        self.0.set(self.0.get() + 1);
        42
    }
    fn timezone_offset_minutes(&self, _: i64) -> i32 {
        -90
    }
    fn random_seed(&self) -> u64 {
        1
    }
}

fn execution(
    runtime: &Runtime,
    context: &mut crate::engine::api::Context,
    target: &str,
    tail: bool,
) -> (RunningExecution, FrameId, ObjectId) {
    let mut instructions = vec![Instruction::Undefined; 4];
    instructions.push(if tail {
        Instruction::TailCallMethod(2)
    } else {
        Instruction::CallMethod(2)
    });
    instructions.push(Instruction::Return);
    let bytecode = runtime
        .publish_unlinked_function(
            context.realm,
            UnlinkedFunction::fixture(
                instructions,
                Vec::new(),
                FunctionMetadata {
                    max_stack: 4,
                    ..Default::default()
                },
            ),
        )
        .unwrap();
    let caller = runtime
        .new_bytecode_closure(context.realm, &bytecode)
        .unwrap();
    let CallableExecution::Bytecode {
        bytecode,
        closure_slots,
    } = runtime.bytecode_for_callable(&caller).unwrap()
    else {
        unreachable!()
    };
    let entry = super::super::root_call::prepare_call(
        runtime,
        context.realm,
        &caller,
        JsValue::Undefined,
        JsValue::Undefined,
        Vec::new(),
        bytecode,
        closure_slots,
    )
    .unwrap();
    // Finish all API preparation before entering the execution turn.
    let Value::Object(date) = context.eval("new Date(946684800123)").unwrap() else {
        unreachable!()
    };
    let date_id = date.object_id();
    let receiver = JsValue::Object(date.into_handle());
    let callable = runtime.into_jsvalue(context.eval(target).unwrap()).unwrap();
    let alias = runtime.dup_jsvalue(&receiver).unwrap();
    let ignored = runtime.new_object(None).unwrap().into_handle();
    let mut execution = RunningExecution::new(runtime, ExecutionLimits::default()).unwrap();
    let id = super::super::driver::push_frame(runtime, &mut execution, entry).unwrap();
    let frame = execution.frames.current_mut(id).unwrap();
    frame.resume_pc = 4;
    frame.fault_pc = 4;
    for value in [receiver, callable, alias, JsValue::Object(ignored)] {
        execution.slots.push(&mut frame.window, value).unwrap();
    }
    (execution, id, date_id)
}

#[test]
fn all_twenty_eight_date_selectors_finish_resident_without_runtime_or_host_rc_clones() {
    let mut targets = vec![
        "Date.now".to_owned(),
        "Date.prototype.getTime".to_owned(),
        "Date.prototype.getTimezoneOffset".to_owned(),
    ];
    targets.extend(DateStringMethod::ALL.map(|kind| format!("Date.prototype.{}", kind.name())));
    targets.extend(DateGetFieldKind::ALL.map(|kind| format!("Date.prototype.{}", kind.name())));
    assert_eq!(targets.len(), 28);
    for target in targets {
        for tail in [false, true] {
            let clock = Rc::new(Cell::new(0));
            let runtime = Runtime::new_with_host_services(DateHost(clock.clone()));
            let mut context = runtime.new_context().unwrap();
            let (mut execution, id, date) = execution(&runtime, &mut context, &target, tail);
            let owners = Rc::strong_count(&runtime.0);
            let hosts = Rc::strong_count(&runtime.0.host_services);
            #[cfg(feature = "profiling")]
            let profile = crate::engine::api::profiling::CostProfile::start();
            {
                let mut state = runtime.0.state.borrow_mut();
                assert!(
                    matches!(
                        super::execute_frame_in_state(&runtime, &mut state, &mut execution, id)
                            .unwrap(),
                        super::VmAction::Complete
                    ),
                    "{target} tail={tail}"
                );
                assert!(execution.pending.is_some(), "{target}");
                assert_eq!(state.active_frames.len(), 1);
                assert_eq!(Rc::strong_count(&runtime.0), owners);
                assert_eq!(Rc::strong_count(&runtime.0.host_services), hosts);
                assert!(!runtime.0.deferred_references.has_pending());
                assert!(
                    state.heap.object(date).is_err(),
                    "receiver and aliased ignored argv retired: {target}"
                );
                assert_eq!(clock.get(), usize::from(target == "Date.now"));
            }
            #[cfg(feature = "profiling")]
            {
                let events = profile.snapshot().owned_execution_events;
                assert_eq!(events.get("core.frame_executor_entry"), Some(&1));
                assert_eq!(events.get("core.internal_native_body"), Some(&1));
                assert_eq!(events.get("native_state_body"), Some(&1));
                assert_eq!(events.get("runtime.clone"), None);
                assert_eq!(events.get("runtime.deferred.release"), None);
            }
            drop(execution);
            assert!(runtime.0.state.borrow().active_frames.is_empty());
            assert!(!runtime.is_poisoned());
        }
    }
}

#[test]
fn resident_date_string_result_is_published_before_requested_collection() {
    let runtime = Runtime::new_with_host_services(DateHost(Rc::new(Cell::new(0))));
    let mut context = runtime.new_context().unwrap();
    let Value::Object(garbage) = context
        .eval("(()=>{let o={};o.self=o;return o})()")
        .unwrap()
    else {
        unreachable!()
    };
    let garbage = garbage.into_handle();
    runtime.release_jsvalue(JsValue::Object(garbage)).unwrap();
    let (mut execution, id, date) =
        execution(&runtime, &mut context, "Date.prototype.toISOString", false);
    runtime.0.gc_pressure.remaining.set(0);
    let mut state = runtime.0.state.borrow_mut();
    assert!(matches!(
        super::execute_frame_in_state(&runtime, &mut state, &mut execution, id).unwrap(),
        super::VmAction::Complete
    ));
    let Some(JsValue::String(string)) = execution.pending.as_ref() else {
        panic!("published Date string")
    };
    let string = *string;
    assert_eq!(
        *state.heap.string(string).unwrap(),
        JsString::from_static("2000-01-01T00:00:00.123Z")
    );
    assert!(state.heap.object(garbage).is_err());
    assert!(state.heap.object(date).is_err());
    assert!(!runtime.0.gc_pressure.requested());
    drop(state);
    drop(execution);
    assert!(runtime.0.state.borrow().heap.string(string).is_err());
}

#[test]
fn date_query_consumers_share_state_body_and_preserve_ignored_argv_and_error_backtrace() {
    let runtime = Runtime::new_with_host_services(DateHost(Rc::new(Cell::new(0))));
    let mut context = runtime.new_context().unwrap();
    #[cfg(feature = "profiling")]
    let profile = crate::engine::api::profiling::CostProfile::start();
    assert_eq!(context.eval(r#"(()=>{
        let date=new Date(946684800123), calls=0;
        const ignored={valueOf(){calls++;throw 99;},toString(){calls++;throw 98;}};
        let targets=['getTime','getTimezoneOffset','toString','toDateString','toTimeString',
            'toUTCString','toISOString','toLocaleString','toLocaleDateString','toLocaleTimeString',
            'getYear','getFullYear','getUTCFullYear','getMonth','getUTCMonth','getDate','getUTCDate',
            'getHours','getUTCHours','getMinutes','getUTCMinutes','getSeconds','getUTCSeconds',
            'getMilliseconds','getUTCMilliseconds','getDay','getUTCDay'];
        for (let target of targets) Date.prototype[target].call(date, ignored);
        let now=Date.now.call(ignored, ignored);
        function wrongDateBrand(){return Date.prototype.getTime.call(ignored);}
        let observed=false;
        try {wrongDateBrand();} catch(e) {
            observed=e instanceof TypeError && e.message==='not a Date object'
                && e.stack.includes('wrongDateBrand');
        }
        return now===42 && calls===0 && observed
            && Date.prototype.toGMTString===Date.prototype.toUTCString;
    })()"#).unwrap(), Value::Bool(true));
    #[cfg(feature = "profiling")]
    assert_eq!(
        profile
            .snapshot()
            .owned_execution_events
            .get("native_state_body"),
        Some(&29)
    );
    assert!(runtime.0.state.borrow().active_frames.is_empty());
    assert!(!runtime.is_poisoned());
}
