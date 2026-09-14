//! Array Iterator next keeps the raw value/done ABI across callback requests.
use crate::engine::{
    api::{error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError},
    builtins::native::ArrayIteratorKind,
    heap::{ContextId, HeapError},
    object::{ObjectRef, PropertyKey},
    value::{Value, conversion::NativeConversion},
    vm::{
        Completion,
        call::{NativeInvocation, NativeInvokeOutcome},
    },
};
pub(crate) enum ArrayNextStep {
    Complete(NativeInvokeOutcome),
    #[cfg(feature = "stack-vm")]
    PreparedRead {
        read: crate::engine::object::OrdinaryRead,
        key: PropertyKey,
        resume: ArrayNextResume,
    },
    Read {
        object: ObjectRef,
        key: PropertyKey,
        resume: ArrayNextResume,
    },
    Number {
        value: Value,
        resume: ArrayNextResume,
    },
}
pub(crate) struct ArrayNextResume {
    realm: ContextId,
    iterator: ObjectRef,
    source: ObjectRef,
    index: u32,
    kind: ArrayIteratorKind,
    phase: Phase,
}
enum Phase {
    Length,
    Number,
    Value,
}
impl ArrayNextStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        invocation: &NativeInvocation,
    ) -> Result<Self, RuntimeError> {
        let NativeInvocation::Call { this_value } = invocation else {
            return Err(RuntimeError::Invariant(
                "Array Iterator next did not receive an iterator-next invocation",
            ));
        };
        let Value::Object(iterator) = this_value else {
            return Self::wrong_receiver(runtime, realm);
        };
        let state = runtime
            .0
            .state
            .borrow()
            .heap
            .array_iterator_state(iterator.object_id());
        let (source, index, kind) = match state {
            Ok(state) => state,
            Err(HeapError::Invariant(_)) => return Self::wrong_receiver(runtime, realm),
            Err(error) => return Err(error.into()),
        };
        let Some(source) = source else {
            return Ok(Self::Complete(NativeInvokeOutcome::IteratorNextRaw {
                value: Value::Undefined,
                done: true,
            }));
        };
        #[cfg(feature = "stack-vm")]
        if let Some(value) = Self::dense_immediate_next(runtime, iterator, source, index, kind)? {
            #[cfg(feature = "profiling")]
            crate::engine::api::profiling::record_owned_execution_event(
                "array_next_dense_immediate_leaf",
            );
            return Ok(Self::Complete(NativeInvokeOutcome::IteratorNextRaw {
                value,
                done: false,
            }));
        }
        let source = ObjectRef::from_borrowed_handle(runtime.clone(), source)?;
        let resume = ArrayNextResume {
            realm,
            iterator: iterator.clone(),
            source: source.clone(),
            index,
            kind,
            phase: Phase::Length,
        };
        if runtime.typed_array_is_object(&source)? {
            return match runtime.typed_array_validated_length(realm, &source)? {
                NativeConversion::Value(length) => resume.length(runtime, length),
                NativeConversion::Throw(value) => Ok(Self::Complete(
                    NativeInvokeOutcome::Completion(Completion::Throw(value)),
                )),
            };
        }
        Ok(Self::Read {
            object: source,
            key: runtime.intern_property_key("length")?,
            resume,
        })
    }
    fn wrong_receiver(runtime: &Runtime, realm: ContextId) -> Result<Self, RuntimeError> {
        Ok(Self::Complete(NativeInvokeOutcome::Completion(
            Completion::Throw(runtime.new_native_error(
                realm,
                NativeErrorKind::Type,
                "Array Iterator object expected",
            )?),
        )))
    }
}
impl ArrayNextResume {
    pub(crate) fn resume(
        mut self,
        runtime: &Runtime,
        reply: Completion,
    ) -> Result<ArrayNextStep, RuntimeError> {
        let value = match reply {
            Completion::Return(value) => value,
            Completion::Throw(value) => {
                return Ok(ArrayNextStep::Complete(NativeInvokeOutcome::Completion(
                    Completion::Throw(value),
                )));
            }
        };
        match self.phase {
            Phase::Length => {
                self.phase = Phase::Number;
                Ok(ArrayNextStep::Number {
                    value,
                    resume: self,
                })
            }
            Phase::Value => {
                let value = if self.kind == ArrayIteratorKind::KeyAndValue {
                    Value::Object(runtime.new_array_from_values(
                        self.realm,
                        vec![Runtime::array_length_value(self.index), value],
                    )?)
                } else {
                    value
                };
                Ok(ArrayNextStep::Complete(
                    NativeInvokeOutcome::IteratorNextRaw { value, done: false },
                ))
            }
            Phase::Number => Err(RuntimeError::Invariant(
                "Array Iterator number phase received completion",
            )),
        }
    }
    pub(crate) fn number(
        self,
        runtime: &Runtime,
        reply: NativeConversion<f64>,
    ) -> Result<ArrayNextStep, RuntimeError> {
        if !matches!(self.phase, Phase::Number) {
            return Err(RuntimeError::Invariant(
                "Array Iterator numeric reply has wrong phase",
            ));
        }
        match reply {
            NativeConversion::Value(value) => {
                self.length(runtime, Runtime::to_uint32_number(value))
            }
            NativeConversion::Throw(value) => Ok(ArrayNextStep::Complete(
                NativeInvokeOutcome::Completion(Completion::Throw(value)),
            )),
        }
    }
    fn length(mut self, runtime: &Runtime, length: u32) -> Result<ArrayNextStep, RuntimeError> {
        let Some(next_index) = live_next_index(self.index, length) else {
            let mut state = runtime.0.state.borrow_mut();
            let cleanup = state
                .heap
                .finish_array_iterator(self.iterator.object_id())?;
            state.apply_cleanup(cleanup)?;
            return Ok(ArrayNextStep::Complete(
                NativeInvokeOutcome::IteratorNextRaw {
                    value: Value::Undefined,
                    done: true,
                },
            ));
        };
        runtime
            .0
            .state
            .borrow_mut()
            .heap
            .set_array_iterator_index(self.iterator.object_id(), next_index)?;
        if self.kind == ArrayIteratorKind::Key {
            return Ok(ArrayNextStep::Complete(
                NativeInvokeOutcome::IteratorNextRaw {
                    value: Runtime::array_length_value(self.index),
                    done: false,
                },
            ));
        }
        self.phase = Phase::Value;
        Ok(ArrayNextStep::Read {
            object: self.source.clone(),
            key: runtime.property_key_for_index(self.index as u64)?,
            resume: self,
        })
    }
}
// Shared advance/completion decision. An in-range Uint32 index always has a
// representable successor; neither caller can overflow at the last element.
fn live_next_index(index: u32, length: u32) -> Option<u32> {
    (index < length).then(|| index + 1)
}

pub(crate) fn finish(
    runtime: &Runtime,
    realm: ContextId,
    mut step: ArrayNextStep,
) -> Result<NativeInvokeOutcome, RuntimeError> {
    loop {
        step = match step {
            ArrayNextStep::Complete(result) => return Ok(result),
            #[cfg(feature = "stack-vm")]
            ArrayNextStep::PreparedRead { read, key, resume } => {
                let completion = match runtime.finish_prepared_read(realm, &key, read)? {
                    NativeConversion::Value(value) => {
                        Completion::Return(value.unwrap_or(Value::Undefined))
                    }
                    NativeConversion::Throw(value) => Completion::Throw(value),
                };
                resume.resume(runtime, completion)?
            }
            ArrayNextStep::Read {
                object,
                key,
                resume,
            } => resume.resume(
                runtime,
                runtime.get_property_in_realm(realm, &object, &key)?,
            )?,
            ArrayNextStep::Number { value, resume } => {
                resume.number(runtime, runtime.native_to_number(realm, &value)?)?
            }
        };
    }
}

#[cfg(feature = "stack-vm")]
mod local;

#[cfg(all(test, feature = "stack-vm", feature = "profiling"))]
mod tests;
