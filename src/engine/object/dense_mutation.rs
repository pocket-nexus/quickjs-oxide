//! Guarded endpoint effects. No shape or prototype fact survives the borrow.
use crate::engine::api::{runtime::Runtime, runtime_error::RuntimeError};
use crate::engine::atom::Atom;
use crate::engine::heap::runtime::RuntimeState;
use crate::engine::heap::{ObjectData, ObjectPayload, PropertySlot, RawValue};
use crate::engine::object::{ObjectRef, ordinary_storage::prototypes_allow_dense_append};
use crate::engine::value::JsValue;
#[cfg(test)]
use crate::engine::value::Value;

fn writable_dense_length(
    state: &RuntimeState,
    data: &ObjectData,
) -> Result<Option<u32>, RuntimeError> {
    let ObjectPayload::Array { dense: Some(dense) } = &data.payload else {
        return Ok(None);
    };
    let shape = state.heap.shape(data.shape)?;
    let Some(entry) = shape.entries().first() else {
        return Ok(None);
    };
    if !entry.flags.writable {
        return Ok(None);
    }
    let length = match data.slots.first() {
        Some(PropertySlot::Data(RawValue::Int(n))) if *n >= 0 => *n as u32,
        Some(PropertySlot::Data(RawValue::Float(n)))
            if *n >= 0.0 && *n <= u32::MAX as f64 && n.fract() == 0.0 =>
        {
            *n as u32
        }
        _ => return Ok(None),
    };
    Ok((length as usize == dense.len()).then_some(length))
}

impl Runtime {
    /// A single-element Push's Get(length), Set(index), Set(length) share one
    /// storage borrow. All rejection/callback cases decline before any write.
    pub(crate) fn try_dense_push(
        &self,
        object: &ObjectRef,
        value: &JsValue,
    ) -> Result<Option<JsValue>, RuntimeError> {
        let _operation = self.operation();
        if !object.belongs_to(self) {
            return Err(RuntimeError::WrongRuntime("property object"));
        }
        let raw = value.as_raw();
        // Borrow the existing edge; a decline neither materializes nor retains it.
        let mut state = self.0.state.borrow_mut();
        let id = object.object_id();
        let prepared = (|| -> Result<Option<u32>, RuntimeError> {
            let data = state.heap.object(id)?;
            let Some(length) = writable_dense_length(&state, data)? else {
                return Ok(None);
            };
            if !data.extensible {
                return Ok(None);
            }
            let Some(atom) = Atom::from_immediate_integer(length) else {
                return Ok(None);
            };
            let prototype = state.heap.shape(data.shape)?.prototype();
            if !prototypes_allow_dense_append(&state, atom, prototype)? {
                return Ok(None);
            }
            Ok(Some(length))
        })();
        // Every decline leaves the dense storage and borrowed producer untouched.
        let length = match prepared {
            Ok(Some(length)) => length,
            Ok(None) => return Ok(None),
            Err(error) => return Err(error),
        };
        // The shared allocation/edge kernel commits the element before length.
        // The explicit final Set(length) is a no-op on this writable own slot.
        let retained = state.retain_raw_value_atoms(std::iter::once(&raw))?;
        let appended = state.heap.append_fresh_array_dense_value(id, raw);
        match appended {
            Ok(()) => {}
            Err(error) => {
                let released = state.release_atoms(retained);
                released?;
                return Err(error.into());
            }
        }
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_execution_event("array_mutation_dense_push");
        Ok(Some(
            i32::try_from(length + 1)
                .map(JsValue::Int)
                .unwrap_or_else(|_| JsValue::Float(f64::from(length + 1))),
        ))
    }

    /// Immediate dense tails need no root materialization before deletion.
    /// Reference tails, holes, fixed length and slow Arrays keep the protocol.
    pub(crate) fn try_dense_pop(
        &self,
        object: &ObjectRef,
    ) -> Result<Option<JsValue>, RuntimeError> {
        let _operation = self.operation();
        if !object.belongs_to(self) {
            return Err(RuntimeError::WrongRuntime("property object"));
        }
        let mut state = self.0.state.borrow_mut();
        let id = object.object_id();
        let data = state.heap.object(id)?;
        let Some(length) = writable_dense_length(&state, data)? else {
            return Ok(None);
        };
        let ObjectPayload::Array { dense: Some(dense) } = &data.payload else {
            unreachable!()
        };
        let value = match dense.last() {
            None | Some(RawValue::Undefined) => JsValue::Undefined,
            Some(RawValue::Null) => JsValue::Null,
            Some(RawValue::Bool(v)) => JsValue::Bool(*v),
            Some(RawValue::Int(v)) => JsValue::Int(*v),
            Some(RawValue::Float(v)) => JsValue::Float(*v),
            _ => return Ok(None),
        };
        if length != 0 {
            let cleanup = state.heap.truncate_array_dense(id, length - 1)?;
            state.apply_cleanup(cleanup)?;
            state.replace_property_slot(
                id,
                0,
                PropertySlot::Data(if let Ok(n) = i32::try_from(length - 1) {
                    RawValue::Int(n)
                } else {
                    RawValue::Float(f64::from(length - 1))
                }),
            )?;
        }
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_execution_event("array_mutation_dense_pop");
        Ok(Some(value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dense_endpoint_guards_decline_before_observable_effects() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        for source in [
            "Object.freeze([1])",
            "Object.seal([1])",
            "Object.defineProperty([1], 'length', {writable:false})",
            "Object.assign([,], {length:2})",
            "new Proxy([1], {})",
        ] {
            let Value::Object(object) = context.eval(source).unwrap() else {
                panic!("object")
            };
            assert!(
                runtime
                    .try_dense_push(&object, &JsValue::Int(2))
                    .unwrap()
                    .is_none(),
                "{source}"
            );
            assert!(
                runtime.try_dense_pop(&object).unwrap().is_none(),
                "{source}"
            );
        }
        assert_eq!(
            context
                .eval(
                    r#"
            var log=[];
            var prototype=Object.create(Array.prototype);
            Object.defineProperty(prototype,'0',{set(v){log.push(v)}, configurable:true});
            var a=[]; Object.setPrototypeOf(a,prototype);
            var n=a.push(7);
            n===1 && a.length===1 && !Object.hasOwn(a,'0') && log.join() === '7'
        "#
                )
                .unwrap(),
            Value::Bool(true)
        );
    }

    #[test]
    fn dense_push_decline_and_store_preserve_borrowed_payload() {
        let runtime = Runtime::new();
        let weak = std::rc::Rc::downgrade(&runtime.0);
        {
            let mut context = runtime.new_context();
            for source in ["'borrowed string'", "123456789012345678901234567890n"] {
                let payload = runtime.into_jsvalue(context.eval(source).unwrap()).unwrap();
                let Value::Object(frozen) = context.eval("Object.freeze([])").unwrap() else {
                    panic!("array")
                };
                assert!(runtime.try_dense_push(&frozen, &payload).unwrap().is_none());
                let Value::Object(target) = context.eval("[]").unwrap() else {
                    panic!("array")
                };
                assert!(matches!(
                    runtime.try_dense_push(&target, &payload).unwrap(),
                    Some(JsValue::Int(1))
                ));
                {
                    let state = runtime.0.state.borrow();
                    let ObjectPayload::Array { dense: Some(dense) } =
                        &state.heap.object(target.object_id()).unwrap().payload
                    else {
                        panic!("dense array")
                    };
                    match (dense.first().unwrap(), &payload) {
                        (RawValue::String(stored), JsValue::String(original)) => {
                            assert_eq!(stored, original)
                        }
                        (RawValue::BigInt(stored), JsValue::BigInt(original)) => {
                            assert_eq!(stored, original)
                        }
                        _ => panic!("payload kind changed"),
                    }
                }
                runtime.release_jsvalue(payload).unwrap();
                runtime.run_gc().unwrap();
                assert!(
                    runtime
                        .0
                        .state
                        .borrow()
                        .heap
                        .object(target.object_id())
                        .is_ok()
                );
            }
        }
        drop(runtime);
        assert!(weak.upgrade().is_none());
    }

    #[test]
    fn dense_endpoint_keeps_scalar_representation_and_reference_roots() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        assert_eq!(
            context
                .eval(
                    r#"
            var a=[];
            a.push(-0); var z=a.pop();
            a.push(NaN); var n=a.pop();
            var object={x:42}, symbol=Symbol('s');
            a.push(object); a.push(symbol);
            var s=a.pop(), o=a.pop();
            Object.is(z,-0) && Number.isNaN(n) && s===symbol && o===object && a.length===0
        "#
                )
                .unwrap(),
            Value::Bool(true)
        );
        runtime.run_gc().unwrap();
        assert_eq!(
            context.eval("o.x === 42 && s === symbol").unwrap(),
            Value::Bool(true)
        );
    }

    #[test]
    fn standard_regexp_and_array_sets_complete_without_waiting_owners() {
        use crate::engine::object::{SetStep, operations::PropertySetAction};
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        for (source, name) in [("/a/g", "lastIndex"), ("[]", "length"), ("[]", "name")] {
            let Value::Object(object) = context.eval(source).unwrap() else {
                panic!("object")
            };
            let key = runtime.intern_property_key(name).unwrap();
            let action = SetStep::start_receiver_into(
                &runtime,
                context.realm,
                &key,
                runtime.into_jsvalue(Value::Int(0)).unwrap(),
                runtime.into_jsvalue(Value::Object(object)).unwrap(),
                |_| panic!("standard own Set published a waiting state"),
            )
            .unwrap();
            assert!(matches!(action, Some(PropertySetAction::Complete)));
        }
    }

    #[test]
    fn ordinary_named_slots_keep_exotic_index_and_length_rules() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        assert_eq!(context.eval(r#"
            var a=[]; a.name=1; a['01']=2; a['4294967295']=3;
            var r=/a/g; r.lastIndex=2; r.extra=4;
            Object.defineProperty(r,'lastIndex',{writable:false});
            var rejected=false; try { 'use strict'; (function(){'use strict';r.lastIndex=3})() } catch(e){rejected=e instanceof TypeError}
            var target={}; Reflect.set(r,'extra',9,target);
            a.length===0 && a.name===1 && a['01']===2 && a['4294967295']===3 &&
            r.lastIndex===2 && r.extra===4 && target.extra===9 && rejected
        "#).unwrap(), Value::Bool(true));
    }
}
