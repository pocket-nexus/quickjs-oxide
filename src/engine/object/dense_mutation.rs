//! Guarded endpoint effects. No shape or prototype fact survives the borrow.
use crate::engine::api::{runtime::Runtime, runtime_error::RuntimeError};
use crate::engine::atom::Atom;
use crate::engine::heap::runtime::RuntimeState;
use crate::engine::heap::{ObjectData, ObjectPayload, PropertySlot, RawValue};
use crate::engine::object::{ObjectRef, ordinary_storage::prototypes_allow_dense_append};
use crate::engine::value::JsValue;
#[cfg(test)]
use crate::engine::value::Value;

// Consume the checked length where it is used. An outlined Result here spills
// the array identity and length even when a numeric dense store simply declines.
#[inline(always)]
pub(super) fn writable_dense_length(
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
    pub(crate) fn try_dense_push(
        &self,
        object: &ObjectRef,
        value: &JsValue,
    ) -> Result<Option<JsValue>, RuntimeError> {
        let _operation = self.operation()?;
        if !object.belongs_to(self) {
            return Err(RuntimeError::WrongRuntime("property object"));
        }
        self.0
            .state
            .borrow_mut()
            .try_dense_push(object.object_id(), value)
    }

    pub(crate) fn try_dense_pop(
        &self,
        object: &ObjectRef,
    ) -> Result<Option<JsValue>, RuntimeError> {
        let _operation = self.operation()?;
        if !object.belongs_to(self) {
            return Err(RuntimeError::WrongRuntime("property object"));
        }
        self.0.state.borrow_mut().try_dense_pop(object.object_id())
    }
}

impl RuntimeState {
    /// Get(length), Set(index) and Set(length) use this one access. Every
    /// decline precedes mutation, so the general algorithm can consume inputs.
    pub(crate) fn try_dense_push(
        &mut self,
        id: crate::engine::heap::ObjectId,
        value: &JsValue,
    ) -> Result<Option<JsValue>, RuntimeError> {
        let raw = value.as_raw();
        // Borrow the existing edge; a decline neither materializes nor retains it.
        let prepared = (|| -> Result<Option<u32>, RuntimeError> {
            let data = self.heap.object(id)?;
            let Some(length) = writable_dense_length(self, data)? else {
                return Ok(None);
            };
            if !data.extensible {
                return Ok(None);
            }
            let Some(atom) = Atom::from_immediate_integer(length) else {
                return Ok(None);
            };
            let prototype = self.heap.shape(data.shape)?.prototype();
            if !prototypes_allow_dense_append(self, atom, prototype)? {
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
        let retained = self.retain_raw_value_atoms(std::iter::once(&raw))?;
        let appended = self.heap.append_fresh_array_dense_value(id, raw);
        match appended {
            Ok(()) => {}
            Err(error) => {
                let released = self.release_atoms(retained);
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

    /// Transfer the removed edge from dense storage to the consumer. No retain,
    /// release or public root is needed, including for object and atom tails.
    pub(crate) fn try_dense_pop(
        &mut self,
        id: crate::engine::heap::ObjectId,
    ) -> Result<Option<JsValue>, RuntimeError> {
        let data = self.heap.object(id)?;
        let Some(length) = writable_dense_length(self, data)? else {
            return Ok(None);
        };
        let ObjectPayload::Array { dense: Some(dense) } = &data.payload else {
            unreachable!()
        };
        // Internal sentinels are not public Array elements. Decline before
        // committing either the value or length change.
        let mut value = match dense.last() {
            None => JsValue::Undefined,
            Some(raw) => match JsValue::from_raw(raw.clone()) {
                Some(value) => value,
                None => return Ok(None),
            },
        };
        if length != 0 {
            value = JsValue::from_raw(self.heap.take_last_array_dense_value(id, length)?)
                .expect("validated public dense tail");
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
    fn dense_pop_moves_the_only_heap_edge_under_existing_state_access() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        let Value::Object(array) = context.eval("[{}]").unwrap() else {
            panic!("Array")
        };
        let mut state = runtime.0.state.borrow_mut();
        let ObjectPayload::Array { dense: Some(dense) } =
            &state.heap.object(array.object_id()).unwrap().payload
        else {
            panic!("dense")
        };
        let RawValue::Object(tail) = dense[0] else {
            panic!("object")
        };
        assert_eq!(state.heap.object_strong_count(tail).unwrap(), 1);
        let result = state.try_dense_pop(array.object_id()).unwrap().unwrap();
        assert_eq!(result, JsValue::Object(tail));
        assert_eq!(state.heap.object_strong_count(tail).unwrap(), 1);
        assert_eq!(
            writable_dense_length(&state, state.heap.object(array.object_id()).unwrap()).unwrap(),
            Some(0)
        );
        state.release_jsvalue(result).unwrap();
        assert!(state.heap.object(tail).is_err());
    }

    #[test]
    fn dense_endpoint_guards_decline_before_observable_effects() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
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
            let mut context = runtime.new_context().expect("create context");
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
        let mut context = runtime.new_context().expect("create context");
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
        let mut context = runtime.new_context().expect("create context");
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
        let mut context = runtime.new_context().expect("create context");
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
