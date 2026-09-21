//! Map callbacks preserve active record guards and callback-created insertion order.
use crate::engine::{
    api::{error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError},
    heap::ContextId,
    object::{CallableRef, ObjectRef},
    value::{JsValue, conversion::NativeConversion},
    vm::{
        Completion,
        call::{NativeArguments, NativeInvocation},
        frames::{ActiveCollectionRecord, ActiveCollectionRecordGuard},
    },
};
#[derive(Clone, Copy)]
pub(crate) enum CallbackKind {
    Each,
    Insert { computed: bool },
}
pub(crate) enum CallbackStep {
    Complete(Completion),
    Call { resume: CallbackResume },
}
pub(crate) struct CallbackResume(Box<CallbackResumeState>);
impl std::ops::Deref for CallbackResume {
    type Target = CallbackResumeState;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for CallbackResume {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
const _: () = assert!(std::mem::size_of::<CallbackResume>() <= 8);
pub(crate) struct CallbackResumeState {
    runtime: Runtime,
    pending_effect: CallbackStepPending,
    phase: Phase,
    map: ObjectRef,
}
impl Drop for CallbackResumeState {
    /// Release the internal edges the pending effect and phase still own when
    /// the request is abandoned. Consumption goes through `Option::take` or
    /// `mem::replace`, so drained fields are inert here; releases are
    /// defer-safe and nothrow.
    fn drop(&mut self) {
        if let Some(value) = self.pending_effect.call_receiver.take() {
            let _ = self.runtime.release_jsvalue(value);
        }
        if let Some(values) = self.pending_effect.call_arguments.take() {
            for value in values {
                let _ = self.runtime.release_jsvalue(value);
            }
        }
        match &mut self.phase {
            Phase::Each { receiver, .. } => {
                let value = std::mem::replace(receiver, JsValue::Undefined);
                let _ = self.runtime.release_jsvalue(value);
            }
            Phase::Insert(key) => {
                let value = std::mem::replace(key, JsValue::Undefined);
                let _ = self.runtime.release_jsvalue(value);
            }
        }
    }
}
enum Phase {
    Each {
        callback: CallableRef,
        receiver: JsValue,
        index: usize,
        record: Option<ActiveCollectionRecordGuard>,
    },
    Insert(JsValue),
}
impl CallbackStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        kind: CallbackKind,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        let map = match runtime.map_receiver(realm, invocation, false)? {
            NativeConversion::Value(map) => map,
            NativeConversion::Throw(value) => {
                return Ok(Self::Complete(Completion::Throw(value)));
            }
        };
        if let CallbackKind::Insert { computed } = kind {
            let key = Runtime::normalized_map_key(runtime.dup_jsvalue(
                arguments.readable.first().ok_or(RuntimeError::Invariant(
                    "Map getOrInsert key argv was not padded",
                ))?,
            )?);
            let second = runtime.dup_jsvalue(arguments.readable.get(1).ok_or(
                RuntimeError::Invariant("Map getOrInsert value argv was not padded"),
            )?)?;
            let callback = if computed {
                match callable(runtime, realm, &second)? {
                    NativeConversion::Value(value) => Some(value),
                    NativeConversion::Throw(value) => {
                        runtime.release_jsvalue(key)?;
                        runtime.release_jsvalue(second)?;
                        return Ok(Self::Complete(Completion::Throw(value)));
                    }
                }
            } else {
                None
            };
            if let Some((_, value)) = runtime.find_map_record(&map, &key)? {
                runtime.release_jsvalue(key)?;
                runtime.release_jsvalue(second)?;
                return Ok(Self::Complete(Completion::Return(runtime.dup_jsvalue(
                    &JsValue::from_raw(value.clone()).ok_or(RuntimeError::Invariant(
                        "stored collection value is uninitialized",
                    ))?,
                )?)));
            }
            if let Some(callable) = callback {
                runtime.release_jsvalue(second)?;
                let call_key = runtime.dup_jsvalue(&key)?;
                return Ok(Self::request_call(
                    callable,
                    JsValue::Undefined,
                    vec![call_key],
                    CallbackResume(Box::new(CallbackResumeState {
                        runtime: runtime.clone(),
                        pending_effect: CallbackStepPending::default(),
                        map: map.clone(),
                        phase: Phase::Insert(key),
                    })),
                ));
            }
            let result = runtime.dup_jsvalue(&second);
            runtime.set_map_record(&map, key, second)?;
            return Ok(Self::Complete(Completion::Return(result?)));
        }
        let value = arguments.readable.first().ok_or(RuntimeError::Invariant(
            "Map.prototype.forEach callback argv was not padded",
        ))?;
        let callback = match callable(runtime, realm, value)? {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => {
                return Ok(Self::Complete(Completion::Throw(value)));
            }
        };
        CallbackResume(Box::new(CallbackResumeState {
            runtime: runtime.clone(),
            pending_effect: CallbackStepPending::default(),
            map: map.clone(),
            phase: Phase::Each {
                callback,
                receiver: match arguments.readable.get(1) {
                    Some(value) => runtime.dup_jsvalue(value)?,
                    None => JsValue::Undefined,
                },
                index: 0,
                record: None,
            },
        }))
        .next(runtime)
    }
}
fn callable(
    runtime: &Runtime,
    realm: ContextId,
    value: &JsValue,
) -> Result<NativeConversion<CallableRef>, RuntimeError> {
    let result = match value {
        JsValue::Object(id) => {
            runtime.as_callable(&ObjectRef::from_borrowed_handle(runtime.clone(), *id)?)?
        }
        _ => None,
    };
    Ok(match result {
        Some(value) => NativeConversion::Value(value),
        None => NativeConversion::Throw(runtime.new_native_error_jsvalue(
            realm,
            NativeErrorKind::Type,
            "not a function",
        )?),
    })
}
impl CallbackResume {
    fn next(mut self, runtime: &Runtime) -> Result<CallbackStep, RuntimeError> {
        let Phase::Each {
            callback,
            receiver,
            index,
            record,
        } = &mut self.0.phase
        else {
            return Err(RuntimeError::Invariant("Map callback next phase mismatch"));
        };
        let entry = {
            runtime
                .0
                .state
                .borrow()
                .heap
                .map_records(self.0.map.object_id())?
                .next_at_or_after(*index)
                .map(|(id, entry)| (id, entry.key.clone(), entry.value.clone()))
        };
        let Some((record_index, key, value)) = entry else {
            return Ok(CallbackStep::Complete(Completion::Return(
                JsValue::Undefined,
            )));
        };
        *index = record_index.checked_add(1).ok_or(RuntimeError::Invariant(
            "Map forEach record index overflowed",
        ))?;
        let key = runtime.dup_jsvalue(&JsValue::from_raw(key.clone()).ok_or(
            RuntimeError::Invariant("stored collection value is uninitialized"),
        )?)?;
        let value = (|| {
            runtime.dup_jsvalue(&JsValue::from_raw(value.clone()).ok_or(
                RuntimeError::Invariant("stored collection value is uninitialized"),
            )?)
        })();
        let value = match value {
            Ok(value) => value,
            Err(error) => {
                let _ = runtime.release_jsvalue(key);
                return Err(error);
            }
        };
        let receiver = match runtime.dup_jsvalue(receiver) {
            Ok(receiver) => receiver,
            Err(error) => {
                let _ = runtime.release_jsvalue(value);
                let _ = runtime.release_jsvalue(key);
                return Err(error);
            }
        };
        *record = Some(
            runtime.push_active_collection_record(ActiveCollectionRecord::Map {
                object: self.0.map.object_id(),
                index: record_index,
            }),
        );
        Ok(CallbackStep::request_call(
            callback.clone(),
            receiver,
            vec![
                value,
                key,
                JsValue::Object(self.0.map.clone().into_handle()),
            ],
            self,
        ))
    }
    pub(crate) fn resume(
        mut self,
        runtime: &Runtime,
        reply: Completion,
    ) -> Result<CallbackStep, RuntimeError> {
        if let Phase::Each { record, .. } = &mut self.0.phase
            && let Some(record) = record.take()
        {
            record.finish()?;
        }
        let value = match reply {
            Completion::Return(value) => value,
            Completion::Throw(value) => {
                return Ok(CallbackStep::Complete(Completion::Throw(value)));
            }
        };
        if matches!(self.0.phase, Phase::Each { .. }) {
            return self.next(runtime);
        }
        let key = match &mut self.0.phase {
            Phase::Insert(key) => std::mem::replace(key, JsValue::Undefined),
            Phase::Each { .. } => unreachable!("Map callback phase changed during resume"),
        };
        let result = runtime.dup_jsvalue(&value)?;
        runtime.delete_map_record(&self.0.map, runtime.dup_jsvalue(&key)?)?;
        runtime.set_map_record(&self.0.map, key, value)?;
        Ok(CallbackStep::Complete(Completion::Return(result)))
    }
}
pub(crate) fn finish(
    runtime: &Runtime,
    realm: ContextId,
    mut step: CallbackStep,
) -> Result<Completion, RuntimeError> {
    loop {
        step = match step {
            CallbackStep::Complete(result) => return Ok(result),
            CallbackStep::Call { mut resume } => {
                let callable = resume.take_call_callable();
                let receiver = resume.take_call_receiver();
                let arguments = resume.take_call_arguments();
                resume.resume(
                    runtime,
                    runtime.call_internal_jsvalue(realm, &callable, receiver, arguments)?,
                )?
            }
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::value::Value;

    #[test]
    fn abandoned_each_requests_release_records_and_collection_roots_in_lifo_order() {
        let runtime = Runtime::new();
        let weak = std::rc::Rc::downgrade(&runtime.0);
        let mut context = runtime.new_context();
        let Value::Object(map) = context.eval("new Map([[{}, {}]])").unwrap() else {
            panic!("map expected")
        };
        let Value::Object(set) = context.eval("new Set([{}])").unwrap() else {
            panic!("set expected")
        };
        let map_id = map.object_id();
        let set_id = set.object_id();
        let arguments = NativeArguments {
            actual_arg_count: 1,
            readable: vec![
                runtime
                    .unroot_value(&context.eval("(function () {})").unwrap())
                    .unwrap(),
            ],
        };
        let map_invocation = NativeInvocation::Call {
            this_value: runtime.unroot_value(&Value::Object(map)).unwrap(),
        };
        let set_invocation = NativeInvocation::Call {
            this_value: runtime.unroot_value(&Value::Object(set)).unwrap(),
        };
        let map_step = CallbackStep::start(
            &runtime,
            context.realm,
            CallbackKind::Each,
            &map_invocation,
            &arguments,
        )
        .unwrap();
        let set_step = crate::engine::builtins::set::callback::EachStep::start(
            &runtime,
            context.realm,
            &set_invocation,
            &arguments,
        )
        .unwrap();
        assert!(matches!(map_step, CallbackStep::Call { .. }));
        assert!(matches!(
            set_step,
            crate::engine::builtins::set::callback::EachStep::Call { .. }
        ));
        {
            let NativeInvocation::Call { this_value } = map_invocation else {
                unreachable!()
            };
            runtime.release_jsvalue(this_value).unwrap();
            let NativeInvocation::Call { this_value } = set_invocation else {
                unreachable!()
            };
            runtime.release_jsvalue(this_value).unwrap();
            for value in arguments.readable {
                runtime.release_jsvalue(value).unwrap();
            }
        }
        runtime.run_gc().unwrap();
        {
            let state = runtime.0.state.borrow();
            assert_eq!(state.active_collection_records.len(), 2);
            assert!(state.heap.object(map_id).is_ok());
            assert!(state.heap.object(set_id).is_ok());
        }
        drop(set_step);
        runtime.run_gc().unwrap();
        {
            let state = runtime.0.state.borrow();
            assert_eq!(state.active_collection_records.len(), 1);
            assert!(state.heap.object(map_id).is_ok());
            assert!(state.heap.object(set_id).is_err());
        }
        drop(map_step);
        runtime.run_gc().unwrap();
        {
            let state = runtime.0.state.borrow();
            assert!(state.active_collection_records.is_empty());
            assert!(state.heap.object(map_id).is_err());
        }
        drop(context);
        drop(runtime);
        assert!(weak.upgrade().is_none());
    }
}

#[derive(Default)]
struct CallbackStepPending {
    call_callable: Option<CallableRef>,
    call_receiver: Option<JsValue>,
    call_arguments: Option<Vec<JsValue>>,
}
impl CallbackStep {
    pub(crate) fn request_call(
        callable: CallableRef,
        receiver: JsValue,
        arguments: Vec<JsValue>,
        mut resume: CallbackResume,
    ) -> Self {
        resume.0.pending_effect.call_callable = Some(callable);
        resume.0.pending_effect.call_receiver = Some(receiver);
        resume.0.pending_effect.call_arguments = Some(arguments);
        Self::Call { resume }
    }
}
impl CallbackResume {
    pub(crate) fn take_call_callable(&mut self) -> CallableRef {
        self.0
            .pending_effect
            .call_callable
            .take()
            .expect("CallbackStep Call callable")
    }
    pub(crate) fn take_call_receiver(&mut self) -> JsValue {
        self.0
            .pending_effect
            .call_receiver
            .take()
            .expect("CallbackStep Call receiver")
    }
    pub(crate) fn take_call_arguments(&mut self) -> Vec<JsValue> {
        self.0
            .pending_effect
            .call_arguments
            .take()
            .expect("CallbackStep Call arguments")
    }
}
const _: () = assert!(std::mem::size_of::<CallbackStep>() <= 64);

// S11 all-domain protocol bound; inline completion stays allocation-free.
const _: () = assert!(std::mem::size_of::<CallbackStep>() <= 64);
