//! Raw Date progress preserves host observations, conversion order and caller storage.
use super::*;
use crate::engine::{
    api::Value,
    heap::{HeapError, RawId},
};
use std::{cell::RefCell, rc::Rc};

mod ownership;

#[derive(Debug)]
struct RecordingHost {
    observations: Rc<RefCell<Vec<i64>>>,
    panic_timezone: bool,
}
impl HostServices for RecordingHost {
    fn now_millis(&self) -> i64 {
        0
    }
    fn timezone_offset_minutes(&self, instant: i64) -> i32 {
        self.observations.borrow_mut().push(instant);
        assert!(!self.panic_timezone, "Date timezone interrupted");
        0
    }
    fn random_seed(&self) -> u64 {
        1
    }
}
fn runtime(panic_timezone: bool) -> (Runtime, Rc<RefCell<Vec<i64>>>) {
    let observations = Rc::new(RefCell::new(Vec::new()));
    (
        Runtime::new_with_host_services(RecordingHost {
            observations: observations.clone(),
            panic_timezone,
        }),
        observations,
    )
}
fn date(runtime: &Runtime, context: &mut crate::engine::api::Context) -> ObjectRef {
    let Value::Object(object) = context.eval("new Date(946684800123)").unwrap() else {
        unreachable!()
    };
    assert!(object.belongs_to(runtime));
    object
}
fn start(
    state: &mut RuntimeState,
    runtime: &Runtime,
    realm: ContextId,
    kind: DateNativeKind,
    receiver: JsValue,
    readable: Vec<JsValue>,
    actual: usize,
) -> Result<DatePrototypeStep, RuntimeError> {
    DatePrototypeStep::start_in_state(
        state,
        &runtime.0.poisoned,
        runtime.0.host_services.as_ref(),
        realm,
        kind,
        &NativeInvocation::Call {
            this_value: receiver,
        },
        &NativeArguments {
            readable,
            actual_arg_count: actual,
        },
    )
}

#[test]
fn date_converting_state_all_field_selectors_keep_raw_owners_and_ignore_extra_argv() {
    let (runtime, observations) = runtime(false);
    let mut context = runtime.new_context().unwrap();
    let receiver = date(&runtime, &mut context);
    let ignored = runtime.new_object(None).unwrap();
    let runtime_owners = Rc::strong_count(&runtime.0);
    let host_owners = Rc::strong_count(&runtime.0.host_services);
    let mut state = runtime.0.state.borrow_mut();
    let receiver_count = state
        .heap
        .object_strong_count(receiver.object_id())
        .unwrap();
    state
        .heap
        .set_strong_count_for_test(RawId::Object(ignored.object_id()), u32::MAX);
    for field in DateSetFieldKind::ALL {
        state
            .heap
            .set_date_value(receiver.object_id(), 946684800123.0)
            .unwrap();
        observations.borrow_mut().clear();
        let count = usize::from(field.end_field() - field.first_field());
        let mut readable = (0..count).map(|_| JsValue::Int(7)).collect::<Vec<_>>();
        readable.push(JsValue::Object(ignored.object_id()));
        let mut step = start(
            &mut state,
            &runtime,
            context.realm,
            DateNativeKind::SetField(field),
            JsValue::Object(receiver.object_id()),
            readable,
            count + 1,
        )
        .unwrap();
        for _ in 0..count {
            let DatePrototypeStep::Number { value, resume } = step else {
                panic!("field {field:?} numeric request")
            };
            assert_eq!(value, JsValue::Int(7));
            step = resume
                .number_in_state(
                    &mut state,
                    &runtime.0.poisoned,
                    runtime.0.host_services.as_ref(),
                    NativeConversion::Value(7.0),
                )
                .unwrap();
        }
        let DatePrototypeStep::Complete(Completion::Return(value)) = step else {
            panic!("field {field:?} completion")
        };
        let stored = state.heap.date_value(receiver.object_id()).unwrap();
        assert_eq!(
            value,
            crate::engine::value::number::operations::Number::compact(stored).into()
        );
        let fields = get_date_fields(stored, false, false, |_| unreachable!()).unwrap();
        for index in field.first_field()..field.end_field() {
            assert_eq!(fields[usize::from(index)], 7.0, "{field:?}");
        }
        assert_eq!(
            state.heap.object_strong_count(receiver.object_id()),
            Ok(receiver_count)
        );
        assert_eq!(
            state.heap.object_strong_count(ignored.object_id()),
            Ok(u32::MAX)
        );
        assert_eq!(Rc::strong_count(&runtime.0), runtime_owners);
        assert_eq!(Rc::strong_count(&runtime.0.host_services), host_owners);
        assert_eq!(observations.borrow().is_empty(), !field.uses_local_time());
    }
    state
        .heap
        .set_strong_count_for_test(RawId::Object(ignored.object_id()), 1);
    assert!(!runtime.0.deferred_references.has_pending());
    assert!(!runtime.is_poisoned());
}

#[test]
fn date_converting_vm_every_setter_accepts_object_argv_and_preserves_field_window_order() {
    let (runtime, _) = runtime(false);
    let mut context = runtime.new_context().unwrap();
    #[cfg(feature = "profiling")]
    let profile = crate::engine::api::profiling::CostProfile::start();
    assert_eq!(
        context
            .eval(
                r#"(() => {
        const names = ['setMilliseconds','setUTCMilliseconds','setSeconds','setUTCSeconds',
            'setMinutes','setUTCMinutes','setHours','setUTCHours','setDate','setUTCDate',
            'setMonth','setUTCMonth','setFullYear','setUTCFullYear','setTime','setYear'];
        const widths = [1,1,2,2,3,3,4,4,1,1,2,2,3,3,1,1];
        for (let i=0; i<names.length; i++) {
            let trace = '';
            const values = Array.from({length:widths[i]}, (_,j) => j + 7);
            const objects = values.map((v,j) => ({ get valueOf() {
                trace += 'g' + j; return function() { trace += 'c' + j; return v; };
            }}));
            objects.push({ valueOf() { throw 'extra argv converted'; } });
            const a = new Date(946684800123), b = new Date(946684800123);
            const expected = b[names[i]](...values);
            const actual = a[names[i]](...objects);
            if (actual !== expected || a.getTime() !== b.getTime()) return false;
            let wanted = '';
            for (let j=0; j<widths[i]; j++) wanted += 'g' + j + 'c' + j;
            if (trace !== wanted) return false;
        }
        return true;
    })()"#
            )
            .unwrap(),
        Value::Bool(true)
    );
    assert!(runtime.0.state.borrow().active_frames.is_empty());
    assert_eq!(runtime.0.raw_execution_owners.get(), 0);
    assert!(!runtime.is_poisoned());
    #[cfg(feature = "profiling")]
    {
        let costs = profile.snapshot();
        let count = |name| costs.owned_execution_events.get(name).copied().unwrap_or(0);
        assert_eq!(count("date_prototype_state_start"), 32);
        assert_eq!(count("date_prototype_state_number_reply"), 68);
        assert!(count("ordinary_install.method") >= 34);
    }
}

#[test]
fn date_converting_vm_field_snapshot_precedes_conversion_but_set_year_rereads_receiver() {
    let (runtime, _) = runtime(false);
    let mut context = runtime.new_context().unwrap();
    assert_eq!(
        context
            .eval(
                r#"(() => {
        const d = new Date(0), y = new Date(0);
        const month = d.setUTCMonth({ valueOf() { d.setUTCFullYear(2005); return 1; } });
        const year = y.setYear({ valueOf() { y.setUTCMonth(5); return 2000; } });
        return month === 2678400000 && d.toISOString() === '1970-02-01T00:00:00.000Z' &&
            year === 959817600000 && y.toISOString() === '2000-06-01T00:00:00.000Z';
    })()"#
            )
            .unwrap(),
        Value::Bool(true)
    );
}

#[test]
fn date_converting_vm_forced_ordinary_hints_preserve_order_and_never_call_exotic_method() {
    let (runtime, _) = runtime(false);
    let mut context = runtime.new_context().unwrap();
    assert_eq!(context.eval(r#"(() => {
        let trace = '';
        const value = {
            get [Symbol.toPrimitive]() { throw 'exotic lookup'; },
            get valueOf() { trace += 'vg'; return function() { trace += 'vc'; return 7; }; },
            get toString() { trace += 'sg'; return function() { trace += 'sc'; return 'S'; }; }
        };
        const primitive = Date.prototype[Symbol.toPrimitive];
        if (primitive.call(value,'number') !== 7 || primitive.call(value,'integer') !== 7 ||
            primitive.call(value,'string') !== 'S' || primitive.call(value,'default') !== 'S') return false;
        try { primitive.call(value,{valueOf(){throw 'hint conversion';}}); return false; }
        catch (e) { if (!(e instanceof TypeError) || e.message !== 'invalid hint') return false; }
        try { primitive.call(1,'number'); return false; }
        catch (e) { if (!(e instanceof TypeError) || e.message !== 'not an object') return false; }
        return trace === 'vgvcvgvcsgscsgsc';
    })()"#).unwrap(), Value::Bool(true));
}

#[test]
fn date_converting_vm_generic_json_boxes_all_primitives_and_preserves_real_proxy_effects() {
    let (runtime, _) = runtime(false);
    let mut context = runtime.new_context().unwrap();
    assert_eq!(context.eval(r#"(() => {
        for (const prototype of [Boolean.prototype,Number.prototype,String.prototype,
                BigInt.prototype,Symbol.prototype]) {
            prototype.toISOString = function() { 'use strict';
                return typeof this === 'object' ? this.valueOf() : 'unboxed'; };
        }
        const json = Date.prototype.toJSON;
        const symbol = Symbol('j');
        for (const value of [true,7,'text',1n,123456789012345678901n,symbol]) {
            if (json.call(value) !== value) return false;
        }
        if (json.call(Infinity) !== null || json.call(NaN) !== null) return false;
        let trace = '';
        const base = { [Symbol.toPrimitive](hint) { trace += hint + ';'; return 1; },
            toISOString() { trace += 'call;'; return this === proxy ? 'json' : 'bad-this'; } };
        const proxy = new Proxy(base,{get(t,k,r){ trace += String(k) + ';'; return Reflect.get(t,k,r); }});
        if (json.call(proxy) !== 'json') return false;
        return trace === 'Symbol(Symbol.toPrimitive);number;toISOString;call;';
    })()"#).unwrap(), Value::Bool(true));
    assert_eq!(runtime.0.raw_execution_owners.get(), 0);
    assert!(runtime.0.state.borrow().active_frames.is_empty());
    assert!(!runtime.is_poisoned());
}

#[test]
fn date_converting_vm_json_waits_and_throws_keep_tail_pc_lower_operands_and_error_realm() {
    let (runtime, _) = runtime(false);
    let mut context = runtime.new_context().unwrap();
    assert_eq!(context.eval(r#"(() => {
        let trace = '';
        const json = Date.prototype.toJSON;
        function tail(value) { return json.call(value); }
        const receiver = { valueOf() { trace += 'v;'; return 1; },
            get toISOString() { trace += 'g;'; return function() { trace += 'c;';
                return Math.max({valueOf(){trace += 'n;'; return 3;}},4); }; } };
        if (30 + tail(receiver) + 8 !== 42) return false;
        const bad = {valueOf(){return 1;}, get toISOString(){trace += 'bad;'; throw 9;}};
        try { 99 + tail(bad); return false; } catch (e) { if (e !== 9) return false; }
        try { json.call({valueOf(){return 1;},toISOString:0}); return false; }
        catch (e) { if (!(e instanceof TypeError) || e.message !== 'object needs toISOString method') return false; }
        const date = new Date(0);
        try { date.setUTCMinutes({valueOf(){trace += 'a;';throw 17;}},
            {valueOf(){trace += 'unreachable;';return 0;}}); return false; }
        catch (e) { if (e !== 17 || date.getTime() !== 0) return false; }
        return trace === 'v;g;c;n;bad;a;';
    })()"#).unwrap(), Value::Bool(true));
    runtime.run_gc().unwrap();
    assert!(runtime.0.state.borrow().active_frames.is_empty());
    assert_eq!(runtime.0.raw_execution_owners.get(), 0);
    assert_eq!(runtime.0.active_frame_depth.get(), 0);
    assert!(!runtime.0.deferred_references.has_pending());
}
