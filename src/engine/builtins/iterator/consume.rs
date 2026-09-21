//! Eager iterator consumers share owned iteration, callback and close phases.
use super::{
    ObjectIteratorStep,
    step::{CloseStep, NextStep, finish_close, finish_next},
};
use crate::engine::{
    api::{
        Error, ErrorKind, error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError,
    },
    builtins::native::NativeFunctionId,
    heap::{ContextId, IteratorConsumerKind},
    object::{CallableRef, ObjectRef, PropertyKey},
    value::{JsValue, conversion::NativeConversion},
    vm::{
        Completion,
        call::{NativeArguments, NativeInvocation},
    },
};
#[derive(Clone, Copy)]
pub(crate) enum ConsumeKind {
    Predicate(IteratorConsumerKind),
    Reduce,
    Array,
}
impl ConsumeKind {
    pub(crate) fn for_target(target: NativeFunctionId) -> Option<Self> {
        match target {
            NativeFunctionId::IteratorPrototypeConsume(kind) => Some(Self::Predicate(kind)),
            NativeFunctionId::IteratorPrototypeReduce => Some(Self::Reduce),
            NativeFunctionId::IteratorPrototypeToArray => Some(Self::Array),
            _ => None,
        }
    }
}
pub(crate) enum ConsumeStep {
    Complete(Completion),
    Read {
        resume: ConsumeResume,
    },
    Next {
        resume: ConsumeResume,
    },
    Call {
        resume: ConsumeResume,
    },
    Close {
        iterator: ObjectRef,
        completion: Completion,
    },
}
pub(crate) struct ConsumeResume(Box<ConsumeResumeState>);
impl std::ops::Deref for ConsumeResume {
    type Target = ConsumeResumeState;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for ConsumeResume {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
const _: () = assert!(std::mem::size_of::<ConsumeResume>() <= 8);
pub(crate) struct ConsumeResumeState {
    pending_effect: ConsumeStepPending,
    realm: ContextId,
    kind: ConsumeKind,
    source: ObjectRef,
    next: JsValue,
    callback: Option<CallableRef>,
    accumulator: Option<JsValue>,
    array: Option<ObjectRef>,
    index: i64,
    phase: Phase,
}
impl Drop for ConsumeResumeState {
    fn drop(&mut self) {
        let runtime = self.source.runtime();
        let _ = runtime.release_jsvalue(std::mem::replace(&mut self.next, JsValue::Undefined));
        for value in [
            self.accumulator.take(),
            self.pending_effect.next_method.take(),
        ]
        .into_iter()
        .flatten()
        {
            let _ = runtime.release_jsvalue(value);
        }
        if let Phase::Callback(item) = std::mem::replace(&mut self.phase, Phase::Next) {
            let _ = runtime.release_jsvalue(item);
        }
        for value in self
            .pending_effect
            .call_arguments
            .take()
            .into_iter()
            .flatten()
        {
            let _ = runtime.release_jsvalue(value);
        }
    }
}
enum Phase {
    Method,
    Next,
    Callback(JsValue),
}
impl ConsumeStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        kind: ConsumeKind,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        let source = match runtime.iterator_receiver(realm, invocation)? {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => {
                return Ok(Self::Complete(Completion::Throw(value)));
            }
        };
        let callback = if matches!(kind, ConsumeKind::Array) {
            None
        } else {
            let value = arguments.readable.first().ok_or(RuntimeError::Invariant(
                "Iterator consumer callback was not padded",
            ))?;
            match runtime.iterator_callable_jsvalue(realm, value)? {
                NativeConversion::Value(callback) => Some(callback),
                NativeConversion::Throw(value) => {
                    return Ok(Self::Close {
                        iterator: source,
                        completion: Completion::Throw(value),
                    });
                }
            }
        };
        let key = runtime.pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Next)?;
        let accumulator = if matches!(kind, ConsumeKind::Reduce) && arguments.actual_arg_count > 1 {
            Some(runtime.dup_jsvalue(arguments.readable.get(1).ok_or(
                RuntimeError::Invariant("Iterator reduce initial value disappeared"),
            )?)?)
        } else {
            None
        };
        Ok({
            let __pending_field_object = source.clone();
            let __pending_field_key = key;
            let __pending_field_resume = ConsumeResume(Box::new(ConsumeResumeState {
                pending_effect: ConsumeStepPending::default(),
                realm,
                kind,
                source,
                next: JsValue::Undefined,
                callback,
                accumulator,
                array: None,
                index: 0,
                phase: Phase::Method,
            }));
            Self::request_read(
                __pending_field_object,
                __pending_field_key,
                __pending_field_resume,
            )
        })
    }
}
impl ConsumeResume {
    fn close(self, completion: Completion) -> ConsumeStep {
        ConsumeStep::Close {
            iterator: self.0.source.clone(),
            completion,
        }
    }
    fn next_step(mut self, runtime: &Runtime) -> Result<ConsumeStep, RuntimeError> {
        self.0.phase = Phase::Next;
        {
            let __pending_field_iterator = self.0.source.clone();
            let __pending_field_method = runtime.dup_jsvalue(&self.0.next)?;
            let __pending_field_resume = self;
            Ok(ConsumeStep::request_next(
                __pending_field_iterator,
                __pending_field_method,
                __pending_field_resume,
            ))
        }
    }
    pub(crate) fn resume(
        mut self,
        runtime: &Runtime,
        reply: Completion,
    ) -> Result<ConsumeStep, RuntimeError> {
        let value = match reply {
            Completion::Return(value) => value,
            Completion::Throw(value) => {
                return Ok(
                    if matches!(self.0.phase, Phase::Callback(_))
                        || matches!(self.0.kind, ConsumeKind::Reduce)
                    {
                        self.close(Completion::Throw(value))
                    } else {
                        ConsumeStep::Complete(Completion::Throw(value))
                    },
                );
            }
        };
        match std::mem::replace(&mut self.0.phase, Phase::Next) {
            Phase::Method => {
                runtime.release_jsvalue(std::mem::replace(&mut self.0.next, value))?;
                if matches!(self.0.kind, ConsumeKind::Array) {
                    self.0.array = Some(runtime.new_array(self.0.realm)?);
                }
                Ok(self.next_step(runtime)?)
            }
            Phase::Callback(item) => {
                self.0.index = self.0.index.wrapping_add(1);
                if matches!(self.0.kind, ConsumeKind::Reduce) {
                    runtime.release_jsvalue(item)?;
                    if let Some(old) = self.0.accumulator.replace(value) {
                        runtime.release_jsvalue(old)?;
                    }
                    return self.next_step(runtime);
                }
                let truth = if matches!(
                    self.0.kind,
                    ConsumeKind::Predicate(IteratorConsumerKind::ForEach)
                ) {
                    Ok(false)
                } else {
                    runtime.value_to_boolean_jsvalue(&value)
                };
                runtime.release_jsvalue(value)?;
                let truth = match truth {
                    Ok(truth) => truth,
                    Err(error) => {
                        runtime.release_jsvalue(item)?;
                        return Err(error);
                    }
                };
                let early = match self.0.kind {
                    ConsumeKind::Predicate(IteratorConsumerKind::Find) if truth => Some(item),
                    kind => {
                        runtime.release_jsvalue(item)?;
                        match kind {
                            ConsumeKind::Predicate(IteratorConsumerKind::Every) if !truth => {
                                Some(JsValue::Bool(false))
                            }
                            ConsumeKind::Predicate(IteratorConsumerKind::Some) if truth => {
                                Some(JsValue::Bool(true))
                            }
                            ConsumeKind::Array => {
                                return Err(RuntimeError::Invariant(
                                    "Iterator.toArray received a callback reply",
                                ));
                            }
                            _ => None,
                        }
                    }
                };
                Ok(if let Some(value) = early {
                    self.close(Completion::Return(value))
                } else {
                    self.next_step(runtime)?
                })
            }
            Phase::Next => {
                runtime.release_jsvalue(value)?;
                Err(RuntimeError::Invariant(
                    "Iterator consumer received a completion in step phase",
                ))
            }
        }
    }
    pub(crate) fn next(
        mut self,
        runtime: &Runtime,
        reply: ObjectIteratorStep,
    ) -> Result<ConsumeStep, RuntimeError> {
        if !matches!(self.0.phase, Phase::Next) {
            if let ObjectIteratorStep::Yield(value) | ObjectIteratorStep::Throw(value) = reply {
                runtime.release_jsvalue(value)?;
            }
            return Err(RuntimeError::Invariant(
                "Iterator consumer next has wrong phase",
            ));
        }
        let item = match reply {
            ObjectIteratorStep::Throw(value) => {
                return Ok(ConsumeStep::Complete(Completion::Throw(value)));
            }
            ObjectIteratorStep::Done => {
                let value = match self.0.kind {
                    ConsumeKind::Predicate(IteratorConsumerKind::Every) => JsValue::Bool(true),
                    ConsumeKind::Predicate(IteratorConsumerKind::Some) => JsValue::Bool(false),
                    ConsumeKind::Predicate(
                        IteratorConsumerKind::Find | IteratorConsumerKind::ForEach,
                    ) => JsValue::Undefined,
                    ConsumeKind::Array => JsValue::Object(
                        self.0
                            .array
                            .take()
                            .ok_or(RuntimeError::Invariant("Iterator.toArray result missing"))?
                            .into_handle(),
                    ),
                    ConsumeKind::Reduce => match self.0.accumulator.take() {
                        Some(value) => value,
                        None => {
                            let error = runtime.new_native_error_jsvalue(
                                self.0.realm,
                                NativeErrorKind::Type,
                                "empty iterator",
                            )?;
                            return Ok(self.close(Completion::Throw(error)));
                        }
                    },
                };
                return Ok(ConsumeStep::Complete(Completion::Return(value)));
            }
            ObjectIteratorStep::Yield(value) => value,
        };
        self.0.phase = Phase::Callback(item);
        if matches!(self.0.kind, ConsumeKind::Array) {
            let array = self
                .0
                .array
                .as_ref()
                .ok_or(RuntimeError::Invariant("Iterator.toArray result missing"))?;
            let key = runtime.property_key_for_index(self.0.index as u32 as u64)?;
            let Phase::Callback(ref item) = self.0.phase else {
                unreachable!()
            };
            let defined = runtime.define_selected_set_data(array, &key, item, false)?;
            let Phase::Callback(item) = std::mem::replace(&mut self.0.phase, Phase::Next) else {
                unreachable!()
            };
            runtime.release_jsvalue(item)?;
            match defined {
                crate::engine::object::operations::PropertyDefineOutcome::Defined(true) => {}
                crate::engine::object::operations::PropertyDefineOutcome::Defined(false) => {
                    return Err(RuntimeError::Invariant(
                        "fresh iterator array rejected definition",
                    ));
                }
                crate::engine::object::operations::PropertyDefineOutcome::Throw(value) => {
                    return Ok(ConsumeStep::Complete(Completion::Throw(value)));
                }
            }
            self.0.index = u32::try_from(self.0.index)
                .ok()
                .and_then(|index| index.checked_add(1))
                .map(i64::from)
                .ok_or_else(|| {
                    RuntimeError::Engine(Error::new(ErrorKind::Range, "invalid array length"))
                })?;
            return self.next_step(runtime);
        }
        if matches!(self.0.kind, ConsumeKind::Reduce) && self.0.accumulator.is_none() {
            let Phase::Callback(item) = std::mem::replace(&mut self.0.phase, Phase::Next) else {
                unreachable!()
            };
            self.0.accumulator = Some(item);
            self.0.index = 1;
            return self.next_step(runtime);
        }
        let callable = self.0.callback.clone().ok_or(RuntimeError::Invariant(
            "Iterator consumer callback missing",
        ))?;
        let Phase::Callback(ref item) = self.0.phase else {
            unreachable!()
        };
        let item_argument = runtime.dup_jsvalue(item)?;
        self.0.pending_effect.call_arguments =
            Some(vec![item_argument, JsValue::Float(self.0.index as f64)]);
        if matches!(self.0.kind, ConsumeKind::Reduce) {
            let accumulator = self.0.accumulator.take().ok_or(RuntimeError::Invariant(
                "Iterator reduce accumulator missing",
            ))?;
            self.0
                .pending_effect
                .call_arguments
                .as_mut()
                .unwrap()
                .insert(0, accumulator);
        }
        self.0.pending_effect.call_callable = Some(callable);
        Ok(ConsumeStep::Call { resume: self })
    }
}
pub(crate) fn finish(
    runtime: &Runtime,
    realm: ContextId,
    mut step: ConsumeStep,
) -> Result<Completion, RuntimeError> {
    loop {
        step = match step {
            ConsumeStep::Complete(result) => return Ok(result),
            ConsumeStep::Close {
                iterator,
                completion,
            } => {
                return finish_close(
                    runtime,
                    realm,
                    CloseStep::start(runtime, realm, iterator, completion)?,
                );
            }
            ConsumeStep::Read { mut resume } => {
                let object = resume.take_read_object();
                let key = resume.take_read_key();
                resume.resume(
                    runtime,
                    runtime.internal_get_jsvalue(
                        realm,
                        &object,
                        &key,
                        JsValue::Object(object.clone().into_handle()),
                    )?,
                )?
            }
            ConsumeStep::Call { mut resume } => {
                let callable = resume.take_call_callable();
                let arguments = resume.take_call_arguments();
                resume.resume(
                    runtime,
                    runtime.call_internal_jsvalue(
                        realm,
                        &callable,
                        JsValue::Undefined,
                        arguments,
                    )?,
                )?
            }
            ConsumeStep::Next { mut resume } => {
                let iterator = resume.take_next_iterator();
                let method = resume.take_next_method();
                resume.next(
                    runtime,
                    finish_next(
                        runtime,
                        realm,
                        NextStep::start_jsvalue(runtime, realm, iterator, method)?,
                    )?,
                )?
            }
        };
    }
}

#[derive(Default)]
struct ConsumeStepPending {
    read_object: Option<ObjectRef>,
    read_key: Option<PropertyKey>,
    next_iterator: Option<ObjectRef>,
    next_method: Option<JsValue>,
    call_callable: Option<CallableRef>,
    call_arguments: Option<Vec<JsValue>>,
}
impl ConsumeStep {
    pub(crate) fn request_read(
        object: ObjectRef,
        key: PropertyKey,
        mut resume: ConsumeResume,
    ) -> Self {
        resume.0.pending_effect.read_object = Some(object);
        resume.0.pending_effect.read_key = Some(key);
        Self::Read { resume }
    }
    pub(crate) fn request_next(
        iterator: ObjectRef,
        method: JsValue,
        mut resume: ConsumeResume,
    ) -> Self {
        resume.0.pending_effect.next_iterator = Some(iterator);
        resume.0.pending_effect.next_method = Some(method);
        Self::Next { resume }
    }
}
impl ConsumeResume {
    pub(crate) fn take_read_object(&mut self) -> ObjectRef {
        self.0
            .pending_effect
            .read_object
            .take()
            .expect("ConsumeStep Read object")
    }
    pub(crate) fn take_read_key(&mut self) -> PropertyKey {
        self.0
            .pending_effect
            .read_key
            .take()
            .expect("ConsumeStep Read key")
    }
    pub(crate) fn take_next_iterator(&mut self) -> ObjectRef {
        self.0
            .pending_effect
            .next_iterator
            .take()
            .expect("ConsumeStep Next iterator")
    }
    pub(crate) fn take_next_method(&mut self) -> JsValue {
        self.0
            .pending_effect
            .next_method
            .take()
            .expect("ConsumeStep Next method")
    }
    pub(crate) fn take_call_callable(&mut self) -> CallableRef {
        self.0
            .pending_effect
            .call_callable
            .take()
            .expect("ConsumeStep Call callable")
    }
    pub(crate) fn take_call_arguments(&mut self) -> Vec<JsValue> {
        self.0
            .pending_effect
            .call_arguments
            .take()
            .expect("ConsumeStep Call arguments")
    }
}
const _: () = assert!(std::mem::size_of::<ConsumeStep>() <= 56);

// S11 all-domain protocol bound; inline completion stays allocation-free.
const _: () = assert!(std::mem::size_of::<ConsumeStep>() <= 64);
