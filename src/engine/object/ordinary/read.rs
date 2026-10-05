//! One non-reentrant ordinary walk under the consumer's existing state access.
//! Only the promoted data result owns an edge. A cold selection must be
//! consumed while the initial receiver still owns the selected prototype chain.

use crate::engine::{
    api::runtime_error::RuntimeError,
    atom::Atom,
    heap::{ObjectId, runtime::RuntimeState},
    object::ordinary_storage::{OwnReadSelection, SpecialKind},
    value::JsValue,
};

pub(crate) enum ReadBoundary {
    Absent,
    Getter(ObjectId),
    Special { object: ObjectId, kind: SpecialKind },
}

impl RuntimeState {
    /// The rooted initial object keeps every traversed prototype and property
    /// live. No mutation, cleanup, callback, or temporary prototype owner is
    /// needed while walking. A data result is retained exactly once before
    /// returning; a cold boundary is selected without creating a continuation.
    pub(crate) fn select_ordinary_read_in_state(
        &mut self,
        mut object: ObjectId,
        atom: Atom,
        domain_id: u64,
        boundary: &mut Option<ReadBoundary>,
        mut native: Option<&mut Option<crate::engine::object::LinkedNativeSelection>>,
    ) -> Result<Option<JsValue>, RuntimeError> {
        *boundary = None;
        loop {
            #[cfg(feature = "profiling")]
            crate::engine::api::profiling::record_owned_execution_event(
                "ordinary_read.state_probe",
            );
            match self.select_own_read(object, atom, false)? {
                OwnReadSelection::Value(raw) => {
                    let borrowed = JsValue::from_raw(raw).ok_or(RuntimeError::Invariant(
                        "internal sentinel in ordinary data property",
                    ))?;
                    let value = self.dup_jsvalue(&borrowed)?;
                    if let (Some(output), JsValue::Object(function)) = (native.as_mut(), &value)
                        && let Some(data) = self.linked_native_data(*function)
                    {
                        **output = Some(
                            crate::engine::object::LinkedNativeSelection::from_read_payload(
                                domain_id, *function, data,
                            ),
                        );
                    }
                    return Ok(Some(value));
                }
                OwnReadSelection::Getter(None) => {
                    return Ok(Some(JsValue::Undefined));
                }
                OwnReadSelection::Missing(None) => {
                    *boundary = Some(ReadBoundary::Absent);
                    return Ok(None);
                }
                OwnReadSelection::Getter(Some(getter)) => {
                    *boundary = Some(ReadBoundary::Getter(getter));
                    return Ok(None);
                }
                OwnReadSelection::Missing(Some(next)) => object = next,
                OwnReadSelection::Special(kind) => {
                    *boundary = Some(ReadBoundary::Special { object, kind });
                    return Ok(None);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::api::{Runtime, Value};

    #[test]
    fn absence_stays_distinct_from_a_getterless_accessor_for_global_resolution() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        let Value::Object(base) = context
            .eval("Object.defineProperty(Object.create(null), 'x', {get: undefined})")
            .unwrap()
        else {
            panic!("object receiver")
        };
        let absent = runtime.intern_property_key("absent").unwrap();
        let present = runtime.intern_property_key("x").unwrap();
        assert!(matches!(
            runtime
                .prepare_ordinary_read_selected(
                    &base,
                    &absent,
                    &JsValue::Object(base.object_id()),
                    None,
                )
                .unwrap(),
            super::super::OrdinaryRead::Complete(None)
        ));
        assert!(matches!(
            runtime
                .prepare_ordinary_read_selected(
                    &base,
                    &present,
                    &JsValue::Object(base.object_id()),
                    None,
                )
                .unwrap(),
            super::super::OrdinaryRead::Complete(Some(JsValue::Undefined))
        ));
    }

    #[test]
    fn inherited_data_keeps_only_the_result_owner_after_last_receiver_release() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        let Value::Object(base) = context.eval("({__proto__: {x: {marker: 7}}})").unwrap() else {
            panic!("object receiver");
        };
        let key = runtime.intern_property_key("x").unwrap();
        let base = runtime.into_jsvalue(Value::Object(base)).unwrap();
        let JsValue::Object(id) = base else {
            unreachable!()
        };
        let mut state = runtime.0.state.borrow_mut();
        let original_count = state.heap.object_strong_count(id).unwrap();
        let mut boundary = None;
        let value = state
            .select_ordinary_read_in_state(id, key.atom(), runtime.domain_id(), &mut boundary, None)
            .unwrap()
            .unwrap();
        assert!(boundary.is_none());
        assert_eq!(state.heap.object_strong_count(id).unwrap(), original_count);
        let JsValue::Object(child) = value else {
            panic!("object data result")
        };
        assert_eq!(state.heap.object_strong_count(child).unwrap(), 2);
        state.release_jsvalue(JsValue::Object(id)).unwrap();
        assert!(state.heap.object(id).is_err());
        assert_eq!(state.heap.object_strong_count(child).unwrap(), 1);
        state.release_jsvalue(JsValue::Object(child)).unwrap();
        assert!(state.heap.object(child).is_err());
    }

    #[test]
    fn inherited_getter_and_proxy_are_selected_without_calling_or_retaining_them() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        for expression in [
            "({__proto__: {get x(){throw 99}}})",
            "({__proto__: new Proxy({}, {get(){throw 98}})})",
        ] {
            let Value::Object(base) = context.eval(expression).unwrap() else {
                panic!("object receiver");
            };
            let key = runtime.intern_property_key("x").unwrap();
            let mut state = runtime.0.state.borrow_mut();
            let original_count = state.heap.object_strong_count(base.object_id()).unwrap();
            let mut boundary = None;
            assert!(
                state
                    .select_ordinary_read_in_state(
                        base.object_id(),
                        key.atom(),
                        runtime.domain_id(),
                        &mut boundary,
                        None,
                    )
                    .unwrap()
                    .is_none()
            );
            assert_eq!(
                state.heap.object_strong_count(base.object_id()).unwrap(),
                original_count
            );
            match boundary.unwrap() {
                ReadBoundary::Getter(getter) => {
                    assert_eq!(state.heap.object_strong_count(getter).unwrap(), 1);
                }
                ReadBoundary::Special {
                    object,
                    kind: SpecialKind::Proxy,
                } => {
                    assert_eq!(state.heap.object_strong_count(object).unwrap(), 1);
                }
                _ => panic!("unexpected selected boundary"),
            }
        }
    }
}
