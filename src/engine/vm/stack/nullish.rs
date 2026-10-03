//! Nullish comparison ownership and HTMLDDA behavior.
use super::{JsValue, Runtime};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::vm::stack::{FrameStorage, FrameWindow, SlotStore};
    use crate::engine::{code::runtime::PublishedFunctionSnapshot, heap::RawId, value::Value};

    fn frame(runtime: &Runtime, values: Vec<JsValue>) -> (SlotStore, FrameWindow) {
        let context = runtime.new_context().expect("create context");
        let mut executable = PublishedFunctionSnapshot::empty_for_test(context.realm);
        executable.metadata.max_stack = 2;
        let mut store = SlotStore::new(20);
        let window = store
            .push_frame(
                runtime,
                &executable.frame_layout(),
                FrameStorage {
                    original_arguments: vec![],
                    parameters: vec![],
                    locals: vec![],
                    operands: values,
                },
            )
            .unwrap();
        (store, window)
    }

    #[test]
    fn nullish_slot_completion_preserves_retained_identity_and_html_dda() {
        for dda in [false, true] {
            for reverse in [false, true] {
                for null in [JsValue::Null, JsValue::Undefined] {
                    let runtime = Runtime::new();
                    let object = runtime.new_object(None).unwrap();
                    if dda {
                        #[cfg(feature = "test262-host")]
                        runtime.set_object_is_html_dda(&object).unwrap();
                        #[cfg(not(feature = "test262-host"))]
                        continue;
                    }
                    let id = object.object_id();
                    let value = runtime.dup_jsvalue(&JsValue::Object(id)).unwrap();
                    let values = if reverse {
                        vec![null, value]
                    } else {
                        vec![value, null]
                    };
                    let (mut store, mut window) = frame(&runtime, values);
                    assert_eq!(
                        store
                            .borrow_frame_slots(&mut window)
                            .unwrap()
                            .nullish_equality_in_state(
                                &mut runtime.0.state.borrow_mut(),
                                &runtime.0.poisoned
                            )
                            .unwrap(),
                        Some(dda)
                    );
                    assert_eq!(window.depth, 0);
                    assert_eq!(
                        runtime
                            .0
                            .state
                            .borrow()
                            .heap
                            .object_strong_count(id)
                            .unwrap(),
                        1
                    );
                    store.clear_frame(&runtime, window).unwrap();
                }
            }
        }
    }

    #[test]
    fn nullish_slot_declines_saturated_owners_and_preserves_external_cleanup() {
        for count in [u32::MAX - 1, u32::MAX] {
            let runtime = Runtime::new();
            let id = runtime.new_object(None).unwrap().into_handle();
            let (mut store, mut window) = frame(&runtime, vec![JsValue::Object(id), JsValue::Null]);
            runtime
                .0
                .state
                .borrow_mut()
                .heap
                .set_strong_count_for_test(RawId::Object(id), count);
            assert_eq!(
                store
                    .borrow_frame_slots(&mut window)
                    .unwrap()
                    .nullish_equality_in_state(
                        &mut runtime.0.state.borrow_mut(),
                        &runtime.0.poisoned
                    )
                    .unwrap(),
                None
            );
            assert_eq!(window.depth, 2);
            assert_eq!(
                runtime
                    .0
                    .state
                    .borrow()
                    .heap
                    .object_strong_count(id)
                    .unwrap(),
                count
            );
            runtime
                .0
                .state
                .borrow_mut()
                .heap
                .set_strong_count_for_test(RawId::Object(id), 1);
            store.clear_frame(&runtime, window).unwrap();
        }
        let runtime = Runtime::new();
        let object = runtime.new_object(None).unwrap();
        let value = runtime
            .dup_jsvalue(&JsValue::Object(object.object_id()))
            .unwrap();
        let (mut store, mut window) = frame(&runtime, vec![value, JsValue::Null]);
        let pending = runtime.new_object(None).unwrap();
        {
            let _borrow = runtime.0.state.borrow();
            drop(pending);
        }
        assert_eq!(
            store
                .borrow_frame_slots(&mut window)
                .unwrap()
                .nullish_equality_in_state(&mut runtime.0.state.borrow_mut(), &runtime.0.poisoned)
                .unwrap(),
            Some(false)
        );
        assert_eq!(window.depth, 0);
        assert!(runtime.0.deferred_references.has_pending());
        store.clear_frame(&runtime, window).unwrap();
    }

    #[test]
    fn nullish_value_branch_proxy_and_conversion_paths_agree() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        assert_eq!(context.eval(r#"(() => {
            let calls = 0;
            let obj = {valueOf() { calls++; return 1; }};
            let proxy = new Proxy(obj, {get() { calls++; throw 42; }});
            function check(x) {
                if (x == null || null == x || x == undefined || undefined == x) return false;
                if (!(x != null && null != x && x != undefined && undefined != x)) return false;
                return [x == null, null == x, x != null, null != x].join() === 'false,false,true,true';
            }
            if (!check(obj) || !check(proxy) || calls !== 0) return false;
            if (null != undefined || undefined != null || !(null == undefined)) return false;
            if (!(obj == 1) || calls !== 1) return false;
            try { proxy == 1; } catch(e) { return e === 42 && calls === 2; }
            return false;
        })()"#).unwrap(), Value::Bool(true));
    }
}
