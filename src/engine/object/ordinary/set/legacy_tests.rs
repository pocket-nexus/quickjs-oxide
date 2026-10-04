#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::object::CompleteOrdinaryPropertyDescriptor;

    fn terminal(runtime: &Runtime, mut step: SetStep) -> PropertySetAction {
        loop {
            match step {
                SetStep::Complete(action) | SetStep::CyclePublishedComplete(action) => {
                    return action;
                }
                selected => step = selected.finish_sync(runtime).unwrap(),
            }
        }
    }

    #[test]
    fn set_preserves_string_handle_for_ordinary_dense_and_sparse_storage() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        let value = runtime
            .into_jsvalue(Value::String(crate::engine::value::JsString::from_static(
                "same arena node",
            )))
            .unwrap();
        let JsValue::String(expected) = &value else {
            unreachable!()
        };
        for (source, name) in [
            ("({})", "x"),
            ("({x: 0})", "x"),
            ("[]", "0"),
            ("[0]", "0"),
            ("[0]", "3"),
        ] {
            let Value::Object(object) = context.eval(source).unwrap() else {
                unreachable!()
            };
            let key = runtime.intern_property_key(name).unwrap();
            let mut step = SetStep::start(
                &runtime,
                Some(context.realm),
                object.try_clone().expect("duplicate root"),
                key.try_clone().expect("duplicate root"),
                runtime.dup_jsvalue(&value).unwrap(),
                runtime
                    .into_jsvalue(Value::Object(object.try_clone().expect("duplicate root")))
                    .unwrap(),
            )
            .unwrap();
            loop {
                if let SetStep::Complete(action) = step {
                    assert!(matches!(action, PropertySetAction::Complete));
                    break;
                }
                step = step.finish_sync(&runtime).unwrap();
            }
            let state = runtime.0.state.borrow();
            let data = state.heap.object(object.object_id()).unwrap();
            let raw = if let crate::engine::heap::ObjectPayload::Array { dense: Some(dense) } =
                &data.payload
            {
                &dense[name.parse::<usize>().unwrap()]
            } else {
                let shape = state.heap.shape(data.shape).unwrap();
                let index = shape
                    .find(crate::engine::atom::AtomIdx::from_raw(key.atom().raw()))
                    .unwrap();
                let crate::engine::heap::PropertySlot::Data(raw) = &data.slots[index as usize]
                else {
                    unreachable!()
                };
                raw
            };
            assert!(
                matches!(raw, crate::engine::heap::RawValue::String(actual) if actual == expected)
            );
        }
        runtime.release_jsvalue(value).unwrap();
    }

    #[test]
    fn canonical_actions_and_selected_typed_effects_match_owned_boundary() {
        for entry in 0..3 {
            let runtime = Runtime::new();
            let mut context = runtime.new_context().expect("create context");
            for (source, key, value, expected) in [
                ("({x:1})", "x", "42", "stored"),
                ("Object.freeze({x:1})", "x", "42", "rejected"),
                ("new Uint8Array(1)", "0", "257", "stored"),
                ("new Uint8Array(1)", "-0", "Symbol()", "throw"),
                ("new BigInt64Array(1)", "1", "1", "throw"),
            ] {
                let Value::Object(object) = context.eval(source).unwrap() else {
                    panic!("expected target");
                };
                let key = runtime.intern_property_key(key).unwrap();
                let value = context.eval(value).unwrap();
                let receiver = Value::Object(object.try_clone().expect("duplicate root"));
                let mut waiting = None;
                let action = if entry == 0 {
                    Some(terminal(
                        &runtime,
                        SetStep::start(
                            &runtime,
                            Some(context.realm),
                            object,
                            key,
                            runtime.into_jsvalue(value).unwrap(),
                            runtime.into_jsvalue(receiver).unwrap(),
                        )
                        .unwrap(),
                    ))
                } else if entry == 2 {
                    drop(object);
                    SetStep::start_receiver_into(
                        &runtime,
                        context.realm,
                        &key,
                        runtime.into_jsvalue(value).unwrap(),
                        runtime.into_jsvalue(receiver).unwrap(),
                        |step| {
                            assert!(waiting.is_none());
                            waiting = Some(step);
                        },
                    )
                    .unwrap()
                } else {
                    SetStep::start_into(
                        &runtime,
                        Some(context.realm),
                        object,
                        key,
                        runtime.into_jsvalue(value).unwrap(),
                        runtime.into_jsvalue(receiver).unwrap(),
                        |step| {
                            assert!(waiting.is_none());
                            waiting = Some(step);
                        },
                    )
                    .unwrap()
                };
                let action =
                    action.unwrap_or_else(|| terminal(&runtime, waiting.expect("selected effect")));
                assert!(matches!(
                    (expected, &action),
                    ("stored", PropertySetAction::Complete)
                        | (
                            "rejected",
                            PropertySetAction::Rejected(PropertySetRejection::ReadOnly)
                        )
                        | ("throw", PropertySetAction::Throw(_))
                ));
                SetStep::Complete(action).release(&runtime);
            }
            assert!(runtime.0.state.borrow().active_frames.is_empty());
        }
    }

    #[test]
    fn initial_waiting_transport_preserves_conversion_throw_and_roots() {
        for entry in 0..3 {
            let runtime = Runtime::new();
            let mut context = runtime.new_context().expect("create context");
            let Value::Object(object) = context.eval("globalThis.trace=''; globalThis.marker={}; globalThis.target=new Uint8Array(1); target").unwrap() else {
                panic!("expected target");
            };
            let value = context
                .eval("({valueOf(){trace+='v';throw marker}})")
                .unwrap();
            let marker = context.eval("marker").unwrap();
            let key = runtime.intern_property_key("0").unwrap();
            let receiver = Value::Object(object.try_clone().expect("duplicate root"));
            let mut step = if entry == 0 {
                SetStep::start(
                    &runtime,
                    Some(context.realm),
                    object,
                    key,
                    runtime.into_jsvalue(value).unwrap(),
                    runtime.into_jsvalue(receiver).unwrap(),
                )
                .unwrap()
            } else {
                let mut waiting = None;
                let sink = |step| {
                    assert!(waiting.is_none(), "waiting sink called twice");
                    waiting = Some(step);
                };
                let action = if entry == 2 {
                    drop(object);
                    SetStep::start_receiver_into(
                        &runtime,
                        context.realm,
                        &key,
                        runtime.into_jsvalue(value).unwrap(),
                        runtime.into_jsvalue(receiver).unwrap(),
                        sink,
                    )
                } else {
                    SetStep::start_into(
                        &runtime,
                        Some(context.realm),
                        object,
                        key,
                        runtime.into_jsvalue(value).unwrap(),
                        runtime.into_jsvalue(receiver).unwrap(),
                        sink,
                    )
                }
                .unwrap();
                assert!(action.is_none());
                waiting.expect("conversion state missing")
            };
            assert_eq!(
                context.eval("trace").unwrap(),
                Value::String(crate::engine::value::JsString::from_static(""))
            );
            runtime.run_gc().unwrap();
            loop {
                match step {
                    SetStep::Complete(PropertySetAction::Throw(value)) => {
                        assert_eq!(runtime.root_and_release_jsvalue(value).unwrap(), marker);
                        break;
                    }
                    SetStep::Complete(_) => panic!("conversion throw was lost"),
                    pending => step = pending.finish_sync(&runtime).unwrap(),
                }
            }
            assert_eq!(
                context.eval("trace==='v' && target[0]===0").unwrap(),
                Value::Bool(true)
            );
            assert!(runtime.0.state.borrow().active_frames.is_empty());
        }
    }

    #[test]
    fn set_boundary_drains_deferred_references_before_waiting_delivery() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        let Value::Object(object) = context.eval("new Proxy({}, {})").unwrap() else {
            panic!("expected proxy");
        };
        let target_id = object.object_id();
        let receiver = Value::Object(object.try_clone().expect("duplicate root"));
        let value = runtime.new_object(None).unwrap();
        let value_id = value.object_id();
        let key = runtime.intern_property_key("x").unwrap();
        let released = runtime.new_object(None).unwrap();
        let released_id = released.object_id();
        let value = runtime.into_jsvalue(Value::Object(value)).unwrap();
        let receiver = runtime.into_jsvalue(receiver).unwrap();
        {
            let _borrow = runtime.0.state.borrow();
            drop(released);
        }
        assert!(runtime.0.deferred_references.has_pending());
        let mut delivered = false;
        let action = SetStep::start_into(
            &runtime,
            Some(context.realm),
            object,
            key,
            value,
            receiver,
            |step| {
                assert!(!delivered);
                delivered = true;
                // The actual public admission drains before selecting and
                // delivering the waiting raw record.
                assert!(!runtime.0.deferred_references.has_pending());
                assert!(runtime.0.state.borrow().heap.object(released_id).is_err());
                runtime.run_gc().unwrap();
                for id in [target_id, value_id] {
                    assert!(runtime.0.state.borrow().heap.object(id).is_ok());
                }
                let step = step.advance_without_callback(&runtime).unwrap();
                assert!(matches!(&step, SetStep::Proxy { .. }));
                step.release(&runtime);
            },
        )
        .unwrap();
        assert!(action.is_none() && delivered);
        runtime.run_gc().unwrap();
        for id in [target_id, value_id] {
            assert!(runtime.0.state.borrow().heap.object(id).is_err());
        }
    }

    #[test]
    fn selected_typed_reply_keeps_throw_root_until_its_action_is_released() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        let Value::Object(object) = context.eval("new Uint8Array(1)").unwrap() else {
            panic!("expected typed array");
        };
        let receiver = Value::Object(object.try_clone().expect("duplicate root"));
        let value = context.eval("Symbol()").unwrap();
        let mut waiting = None;
        let action = SetStep::start_into(
            &runtime,
            Some(context.realm),
            object,
            runtime.intern_property_key("-0").unwrap(),
            runtime.into_jsvalue(value).unwrap(),
            runtime.into_jsvalue(receiver).unwrap(),
            |step| waiting = Some(step),
        )
        .unwrap();
        let action = action
            .unwrap_or_else(|| terminal(&runtime, waiting.expect("selected typed conversion")));
        let PropertySetAction::Throw(JsValue::Object(error)) = &action else {
            panic!("expected rooted TypeError");
        };
        let id = *error;
        runtime.run_gc().unwrap();
        assert!(runtime.0.state.borrow().heap.object(id).is_ok());
        SetStep::Complete(action).release(&runtime);
        runtime.run_gc().unwrap();
        assert!(runtime.0.state.borrow().heap.object(id).is_err());
    }

    #[test]
    fn borrowed_set_validates_target_key_and_value_before_storage() {
        let runtime = Runtime::new();
        let foreign = Runtime::new();
        let context = runtime.new_context().expect("create context");
        for (foreign_target, foreign_key, expected) in [
            (true, true, "object"),
            (false, true, "property key"),
            (false, false, "property value"),
        ] {
            let target_runtime = if foreign_target { &foreign } else { &runtime };
            let key_runtime = if foreign_key { &foreign } else { &runtime };
            let receiver = Value::Object(target_runtime.new_object(None).unwrap());
            let key = key_runtime.intern_property_key("x").unwrap();
            let value = Value::Object(foreign.new_object(None).unwrap());
            let Value::Object(target) = &receiver else {
                unreachable!()
            };
            let result = runtime.prepare_set_property_with_receiver_in_realm(
                Some(context.realm),
                target,
                &key,
                value,
                receiver.try_clone().expect("duplicate root"),
            );
            assert!(matches!(result, Err(RuntimeError::WrongRuntime(role)) if role == expected));
            assert!(!runtime.0.deferred_references.has_pending());
        }
    }

    #[test]
    fn resident_set_array_length_primitive_completion_uses_original_conversion() {
        for (source, expected) in [("2", 2), ("' 2 '", 2), ("true", 1), ("null", 0), ("-0", 0)] {
            let runtime = Runtime::new();
            let mut context = runtime.new_context().expect("create context");
            let Value::Object(array) = context.eval("[1,2,3]").unwrap() else {
                panic!("array");
            };
            let value = context.eval(source).unwrap();
            let mut waiting = None;
            let action = SetStep::start_receiver_into(
                &runtime,
                context.realm,
                &runtime.intern_property_key("length").unwrap(),
                runtime.into_jsvalue(value).unwrap(),
                runtime
                    .into_jsvalue(Value::Object(array.try_clone().expect("duplicate root")))
                    .unwrap(),
                |step| waiting = Some(step),
            )
            .unwrap();
            let action = action.unwrap_or_else(|| {
                terminal(&runtime, waiting.expect("selected length conversion"))
            });
            assert!(matches!(action, PropertySetAction::Complete));
            assert_eq!(runtime.array_length_state(&array).unwrap().0, expected);
        }
    }

    #[test]
    fn resident_set_array_length_keeps_callbacks_partial_shrink_and_readonly_order() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        assert_eq!(context.eval(r#"(()=>{
            let a=[0,1,2,3], trace='', value={valueOf(){trace+='v';return 2}};
            a.length=value;
            if(trace!=='vv'||a.length!==2)return false;
            a=[0,1,2,3];Object.defineProperty(a,'2',{configurable:false});
            if(Reflect.set(a,'length','1')!==false||a.length!==3||3 in a||!(2 in a))return false;
            a=[0,1];Object.defineProperty(a,'length',{writable:false});
            let thrown=false;try{a.length=Symbol()}catch(e){thrown=e instanceof TypeError}
            if(!thrown||a.length!==2)return false;
            a=[0,1,2];trace='';
            value={valueOf(){trace+='x';if(trace.length===2)Object.defineProperty(a,'length',{writable:false});return 1}};
            if(Reflect.set(a,'length',value)!==false||trace!=='xx'||a.length!==3)return false;
            let marker={};trace='';
            try{a.length={valueOf(){trace+='t';throw marker}}}catch(e){if(e!==marker)return false}
            return trace==='t' && a.length===3;
        })()"#).unwrap(), Value::Bool(true));
        assert!(runtime.0.state.borrow().active_frames.is_empty());
    }

    #[test]
    fn selected_missing_set_preserves_prototype_and_distinct_receiver_semantics() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        assert_eq!(context.eval(r#"(() => {
            const symbol = Symbol('slot'), marker = {};
            let trace = '';
            const proto = { writable: 1, set setter(v) { trace += 's'; this.seen = v; } };
            Object.defineProperty(proto, 'readonly', { value: 1 });
            const object = Object.create(proto);
            object.writable = marker; object[symbol] = marker; object.setter = marker;
            if (object.writable !== marker || object[symbol] !== marker || object.seen !== marker || trace !== 's') return false;
            if (Reflect.set(object, 'readonly', marker) || Object.hasOwn(object, 'readonly')) return false;
            try { (function(){ 'use strict'; object.readonly = marker; })(); return false; }
            catch (e) { if (!(e instanceof TypeError)) return false; }
            const sealed = Object.preventExtensions(Object.create(proto));
            if (Reflect.set(sealed, 'newKey', marker)) return false;
            // Receiver's prototype is irrelevant once target has selected data.
            const receiver = Object.create({ set writable(v) { throw 'wrong receiver prototype'; } });
            if (!Reflect.set(proto, 'writable', marker, receiver) || receiver.writable !== marker) return false;
            const proxy = new Proxy({}, {
                set(t,k,v,r) { trace += 'p'; return Reflect.set(t,k,v,r); },
                getOwnPropertyDescriptor(t,k) { trace += 'd'; return Reflect.getOwnPropertyDescriptor(t,k); }
            });
            const child = Object.create(Object.create(proxy));
            child.key = marker;
            if (trace !== 'sp' || child.key !== marker) return false;
            const mutating = new Proxy({}, {set(t,k,v,r) {
                Object.defineProperty(r,k,{value:7,writable:false});
                return Reflect.set(t,k,v,r);
            }});
            const afterBoundary = Object.create(mutating);
            if (Reflect.set(afterBoundary,'key',marker) || afterBoundary.key !== 7) return false;
            return true;
        })()"#).unwrap(), Value::Bool(true));
    }

    #[test]
    fn selected_missing_append_keeps_unique_dictionary_and_shared_shape_isolation() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        assert_eq!(context.eval(r#"(() => {
            const a = {}, b = {};
            for (let i=0; i<12; i++) { a['k'+i]=i; b['k'+i]=i; }
            a.onlyA=12;
            if ('onlyA' in b || Object.keys(b).length !== 12) return false;
            delete a.k1; a.afterDelete=13; a.k1=14;
            const symbol=Symbol(); a[symbol]=15;
            if (a.k0 !== 0 || a.k1 !== 14 || a.afterDelete !== 13 || a[symbol] !== 15) return false;
            if (Object.keys(a).join(',') !== 'k0,k2,k3,k4,k5,k6,k7,k8,k9,k10,k11,onlyA,afterDelete,k1') return false;
            Object.preventExtensions(a);
            return !Reflect.set(a,'rejected',1) && a.k1 === 14 && b.k1 === 1;
        })()"#).unwrap(), Value::Bool(true));
    }

    #[test]
    fn selected_dense_append_preserves_array_permissions_holes_and_special_keys() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        assert_eq!(context.eval(r#"(() => {
            const a = [], marker = {}, symbol = Symbol();
            a[0] = marker; a[1] = 2;
            if (a.length !== 2 || a[0] !== marker) return false;
            a.length = 5; a[2] = 3;
            if (a.length !== 5 || 3 in a || 4 in a) return false;
            Object.defineProperty(a, 'length', { writable: false });
            a[3] = 4;
            if (a[3] !== 4 || Reflect.set(a, '5', 6)) return false;
            a['4294967295'] = 9; a[symbol] = marker;
            if (a.length !== 5 || a['4294967295'] !== 9 || a[symbol] !== marker) return false;
            const b = []; Object.preventExtensions(b);
            if (Reflect.set(b, '0', marker) || b.length !== 0) return false;
            let seen = 0;
            const prototype = Object.create(Array.prototype);
            Object.defineProperty(prototype, '0', { set(v) { seen++; if(v !== marker) throw 'bad'; } });
            const c = []; Object.setPrototypeOf(c, prototype); c[0] = marker;
            if (seen !== 1 || c.length !== 0 || Object.hasOwn(c, '0')) return false;
            const receiver = []; Object.setPrototypeOf(receiver, prototype);
            if (!Reflect.set({0: 1}, '0', marker, receiver) || seen !== 1 || receiver[0] !== marker || receiver.length !== 1) return false;
            return true;
        })()"#).unwrap(), Value::Bool(true));
    }

    #[test]
    fn local_new_property_definition_uses_selected_receiver_and_shared_rejection() {
        for (source, name, rejected) in [
            ("({})", "x", false),
            ("Object.create({x: 9})", "x", false),
            ("Object.create(null)", "__proto__", false),
            ("Object.preventExtensions({})", "x", true),
        ] {
            let runtime = Runtime::new();
            let mut context = runtime.new_context().expect("create context");
            let Value::Object(target) = context.eval(source).unwrap() else {
                panic!("target")
            };
            let value = context.eval("({valueOf(){throw 99}})").unwrap();
            let key = runtime.intern_property_key(name).unwrap();
            let mut deliveries = 0;
            let mut action = None;
            #[cfg(feature = "profiling")]
            let profile = crate::engine::api::profiling::CostProfile::start();
            let initial = SetStep::start_receiver_into(
                &runtime,
                context.realm,
                &key,
                runtime
                    .into_jsvalue(value.try_clone().expect("duplicate root"))
                    .unwrap(),
                runtime
                    .into_jsvalue(Value::Object(target.try_clone().expect("duplicate root")))
                    .unwrap(),
                |step| {
                    deliveries += 1;
                    let SetStep::Complete(completed) =
                        step.advance_without_callback(&runtime).unwrap()
                    else {
                        panic!("new ordinary definition escaped to query");
                    };
                    action = Some(completed);
                },
            )
            .unwrap();
            assert_eq!(deliveries, 0, "ordinary definition stays resident");
            #[cfg(feature = "profiling")]
            {
                let costs = profile.snapshot();
                assert_eq!(
                    costs
                        .owned_execution_events
                        .get("property_storage_set_probe")
                        .copied(),
                    Some(1),
                    "the initial missing selection must not be repeated",
                );
                if !rejected {
                    assert_eq!(
                        costs
                            .owned_execution_events
                            .get("set_missing_committed_from_selection")
                            .copied(),
                        Some(1)
                    );
                    assert!(
                        !costs
                            .owned_execution_events
                            .contains_key("set_state_created")
                    );
                    assert!(
                        !costs
                            .owned_execution_events
                            .contains_key("set_local_define_attempt")
                    );
                    assert!(
                        !costs
                            .owned_execution_events
                            .contains_key("set_owner_clone.PropertyKey")
                    );
                }
                assert!(
                    !costs
                        .owned_execution_events
                        .keys()
                        .any(|key| key.starts_with("set_request_publish."))
                );
                drop(profile);
            }
            let action = initial.or(action);
            if rejected {
                assert!(matches!(
                    action,
                    Some(PropertySetAction::Rejected(
                        PropertySetRejection::NotExtensible
                    ))
                ));
                assert!(runtime.get_own_property(&target, &key).unwrap().is_none());
            } else {
                assert!(matches!(action, Some(PropertySetAction::Complete)));
                let Some(CompleteOrdinaryPropertyDescriptor::Data {
                    value: stored,
                    writable,
                    enumerable,
                    configurable,
                }) = runtime.get_own_property(&target, &key).unwrap()
                else {
                    panic!("own data")
                };
                assert_eq!(stored, value);
                assert!(writable && enumerable && configurable);
            }
        }
    }

    #[test]
    fn local_new_property_definition_preserves_prototype_callbacks_and_key_order() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        #[cfg(feature = "profiling")]
        let profile = crate::engine::api::profiling::CostProfile::start();
        assert_eq!(
            context
                .eval(
                    r#"
            (() => {
                let calls = 0;
                const inherited = Object.create({set x(value) { calls++; this.y = value; }});
                inherited.x = 7;
                const proxy = new Proxy({}, {set(target,key,value,receiver) {
                    calls++; return Reflect.set(target,key,value,receiver);
                }});
                const receiver = Object.create(proxy);
                receiver.a = 8;
                const o = {}, symbol = Symbol();
                o.b = 1; o[3] = 3; o.a = 2; o[1] = 1; o[symbol] = 4;
                const keys = Reflect.ownKeys(o);
                return calls === 2 && inherited.y === 7
                    && !Object.hasOwn(inherited, 'x') && receiver.a === 8
                    && keys.length === 5 && keys[0] === '1' && keys[1] === '3'
                    && keys[2] === 'b' && keys[3] === 'a' && keys[4] === symbol;
            })()
        "#
                )
                .unwrap(),
            Value::Bool(true)
        );
        #[cfg(feature = "profiling")]
        assert!(
            profile
                .snapshot()
                .owned_execution_events
                .get("set_missing_committed_from_selection")
                .copied()
                .unwrap_or(0)
                > 0
        );
        assert!(runtime.0.state.borrow().active_frames.is_empty());
    }

    fn take_descriptor(step: SetStep) -> SetResume {
        let SetStep::Descriptor { resume, .. } = step else {
            panic!("expected receiver descriptor query")
        };
        resume
    }

    #[test]
    fn abandoned_set_receiver_queries_keep_then_release_target_receiver_and_value() {
        for after_descriptor in [false, true] {
            let runtime = Runtime::new();
            let weak = std::rc::Rc::downgrade(&runtime.0);
            let context = runtime.new_context().expect("create context");
            let target = runtime.new_object(None).unwrap();
            let target_id = target.object_id();
            let handler = runtime.new_object(None).unwrap();
            let handler_id = handler.object_id();
            let NativeConversion::Value(receiver) = runtime
                .new_proxy(
                    context.realm,
                    Value::Object(runtime.new_object(None).unwrap()),
                    Value::Object(handler),
                )
                .unwrap()
            else {
                panic!("proxy allocation failed")
            };
            let receiver_id = receiver.object_id();
            let value = runtime.new_object(None).unwrap();
            let value_id = value.object_id();
            let mut step = SetStep::start(
                &runtime,
                Some(context.realm),
                target,
                runtime.intern_property_key("x").unwrap(),
                runtime.into_jsvalue(Value::Object(value)).unwrap(),
                runtime.into_jsvalue(Value::Object(receiver)).unwrap(),
            )
            .unwrap();
            if after_descriptor {
                let resume = take_descriptor(step);
                let owner = (&*resume.0) as *const SetResumeState;
                step = resume
                    .descriptor(&runtime, NativeConversion::Value(None))
                    .unwrap();
                let SetStep::Define { resume } = &step else {
                    panic!("expected define");
                };
                assert_eq!(owner, (&*resume.0) as *const SetResumeState);
            }
            runtime.run_gc().unwrap();
            for id in [target_id, handler_id, receiver_id, value_id] {
                assert!(runtime.0.state.borrow().heap.object(id).is_ok());
            }
            step.release(&runtime);
            runtime.run_gc().unwrap();
            for id in [target_id, handler_id, receiver_id, value_id] {
                assert!(runtime.0.state.borrow().heap.object(id).is_err());
            }
            drop(context);
            drop(runtime);
            assert!(weak.upgrade().is_none());
        }
    }

    #[test]
    fn set_rejects_an_unrelated_reply_before_mutating_the_receiver() {
        let runtime = Runtime::new();
        let context = runtime.new_context().expect("create context");
        let target = runtime.new_object(None).unwrap();
        let NativeConversion::Value(receiver) = runtime
            .new_proxy(
                context.realm,
                Value::Object(runtime.new_object(None).unwrap()),
                Value::Object(runtime.new_object(None).unwrap()),
            )
            .unwrap()
        else {
            panic!("proxy allocation failed")
        };
        let resume = take_descriptor(
            SetStep::start(
                &runtime,
                Some(context.realm),
                target,
                runtime.intern_property_key("x").unwrap(),
                runtime.into_jsvalue(Value::Int(42)).unwrap(),
                runtime.into_jsvalue(Value::Object(receiver)).unwrap(),
            )
            .unwrap(),
        );
        assert!(
            resume
                .defined(
                    &runtime,
                    NativeConversion::Value(InternalDefineResult::Defined)
                )
                .is_err()
        );
        assert_eq!(runtime.0.proxy_method_depth.get(), 0);
    }
}
