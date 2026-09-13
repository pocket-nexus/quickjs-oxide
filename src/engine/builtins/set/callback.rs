//! Set.forEach keeps its active record until the callback reply is delivered.
use crate::engine::{
    api::{error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError},
    heap::ContextId,
    object::{CallableRef, ObjectRef},
    value::{Value, conversion::NativeConversion},
    vm::{
        Completion,
        call::{NativeArguments, NativeInvocation},
        frames::{ActiveCollectionRecord, ActiveCollectionRecordGuard},
    },
};
pub(crate) enum EachStep {
    Complete(Completion),
    Call {
        callable: CallableRef,
        receiver: Value,
        arguments: Vec<Value>,
        resume: EachResume,
    },
}
pub(crate) struct EachResume {
    record: Option<ActiveCollectionRecordGuard>,
    set: ObjectRef,
    callback: CallableRef,
    receiver: Value,
    index: usize,
}
impl EachStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        let set = match runtime.set_receiver(realm, invocation.clone(), false)? {
            NativeConversion::Value(set) => set,
            NativeConversion::Throw(value) => return Ok(Self::Complete(Completion::Throw(value))),
        };
        let value = arguments.readable.first().ok_or(RuntimeError::Invariant(
            "Set.prototype.forEach callback argv was not padded",
        ))?;
        let callback = match value {
            Value::Object(object) => runtime.as_callable(object)?,
            _ => None,
        };
        let Some(callback) = callback else {
            return Ok(Self::Complete(Completion::Throw(
                runtime.new_native_error(realm, NativeErrorKind::Type, "not a function")?,
            )));
        };
        EachResume {
            set,
            callback,
            receiver: arguments
                .readable
                .get(1)
                .cloned()
                .unwrap_or(Value::Undefined),
            index: 0,
            record: None,
        }
        .next(runtime)
    }
}
impl EachResume {
    fn next(mut self, runtime: &Runtime) -> Result<EachStep, RuntimeError> {
        let Some((record_index, value)) =
            runtime.next_live_set_record(&self.set, &mut self.index)?
        else {
            return Ok(EachStep::Complete(Completion::Return(Value::Undefined)));
        };
        self.record = Some(
            runtime.push_active_collection_record(ActiveCollectionRecord::Set {
                object: self.set.object_id(),
                index: record_index,
            }),
        );
        Ok(EachStep::Call {
            callable: self.callback.clone(),
            receiver: self.receiver.clone(),
            arguments: vec![value.clone(), value, Value::Object(self.set.clone())],
            resume: self,
        })
    }
    pub(crate) fn resume(
        mut self,
        runtime: &Runtime,
        reply: Completion,
    ) -> Result<EachStep, RuntimeError> {
        if let Some(record) = self.record.take() {
            record.finish()?;
        }
        match reply {
            Completion::Throw(value) => Ok(EachStep::Complete(Completion::Throw(value))),
            Completion::Return(_) => self.next(runtime),
        }
    }
}
pub(crate) fn finish(
    runtime: &Runtime,
    realm: ContextId,
    mut step: EachStep,
) -> Result<Completion, RuntimeError> {
    loop {
        step = match step {
            EachStep::Complete(result) => return Ok(result),
            EachStep::Call {
                callable,
                receiver,
                arguments,
                resume,
            } => resume.resume(
                runtime,
                runtime.call_internal(realm, &callable, receiver, &arguments)?,
            )?,
        };
    }
}
