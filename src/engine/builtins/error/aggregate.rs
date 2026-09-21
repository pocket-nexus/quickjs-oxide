//! AggregateError owns its unpublished errors array and closes after IteratorNext failure.
use crate::engine::{
    api::{error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError},
    builtins::{
        iterator::step::{CloseStep, NextStep, finish_close, finish_next},
        object::ObjectIteratorStep,
    },
    heap::ContextId,
    object::{CallableRef, ObjectRef, PropertyKey, WellKnownSymbol},
    value::JsValue,
    vm::Completion,
};
pub(crate) enum AggregateStep {
    Complete(Completion),
    Read {
        receiver: JsValue,
        key: PropertyKey,
        resume: AggregateResume,
    },
    Call {
        callable: CallableRef,
        receiver: JsValue,
        resume: AggregateResume,
    },
    Next {
        iterator: ObjectRef,
        next: JsValue,
        resume: AggregateResume,
    },
    Close {
        iterator: ObjectRef,
        completion: Completion,
    },
}
enum Phase {
    Method,
    Iterator,
    NextMethod,
    Next,
}
pub(crate) struct AggregateResume(Box<AggregateResumeState>);
impl std::ops::Deref for AggregateResume {
    type Target = AggregateResumeState;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for AggregateResume {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
const _: () = assert!(std::mem::size_of::<AggregateResume>() <= 8);
pub(crate) struct AggregateResumeState {
    runtime: Runtime,
    realm: ContextId,
    phase: Phase,
    iterable: JsValue,
    iterator: Option<ObjectRef>,
    next: JsValue,
    result: Option<ObjectRef>,
    index: u64,
    item: Option<JsValue>,
}
impl Drop for AggregateResumeState {
    /// Release the internal edges still owned when the request is abandoned.
    /// Consumption goes through `std::mem::replace` or a duplicate, so drained
    /// fields are `Undefined` here; releases are defer-safe and nothrow.
    fn drop(&mut self) {
        if let Some(value) = self.item.take() {
            let _ = self.runtime.release_jsvalue(value);
        }
        if !matches!(self.iterable, JsValue::Undefined) {
            let iterable = std::mem::replace(&mut self.iterable, JsValue::Undefined);
            let _ = self.runtime.release_jsvalue(iterable);
        }
        if !matches!(self.next, JsValue::Undefined) {
            let next = std::mem::replace(&mut self.next, JsValue::Undefined);
            let _ = self.runtime.release_jsvalue(next);
        }
    }
}
impl AggregateStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        iterable: JsValue,
    ) -> Result<Self, RuntimeError> {
        if matches!(iterable, JsValue::Null | JsValue::Undefined) {
            return Ok(Self::Complete(Completion::Throw(
                runtime.new_native_error_jsvalue(
                    realm,
                    NativeErrorKind::Type,
                    &format!(
                        "cannot read property 'Symbol.iterator' of {}",
                        if matches!(iterable, JsValue::Null) {
                            "null"
                        } else {
                            "undefined"
                        }
                    ),
                )?,
            )));
        }
        let resume = AggregateResume(Box::new(AggregateResumeState {
            runtime: runtime.clone(),
            realm,
            phase: Phase::Method,
            iterable,
            iterator: None,
            next: JsValue::Undefined,
            result: None,
            index: 0,
            item: None,
        }));
        let receiver = runtime.dup_jsvalue(&resume.0.iterable)?;
        Ok(Self::Read {
            receiver,
            key: PropertyKey::from(runtime.well_known_symbol(WellKnownSymbol::Iterator)),
            resume,
        })
    }
}
impl AggregateResume {
    pub(crate) fn resume(
        mut self,
        runtime: &Runtime,
        result: Completion,
    ) -> Result<AggregateStep, RuntimeError> {
        let value = match result {
            Completion::Return(value) => value,
            Completion::Throw(value) => {
                return Ok(AggregateStep::Complete(Completion::Throw(value)));
            }
        };
        match self.0.phase {
            Phase::Method => {
                let callable = match &value {
                    JsValue::Object(id) => runtime.as_callable_object(*id),
                    _ => Ok(None),
                };
                runtime.release_jsvalue(value)?;
                let callable = callable?;
                let Some(callable) = callable else {
                    return Ok(AggregateStep::Complete(Completion::Throw(
                        runtime.new_native_error_jsvalue(
                            self.0.realm,
                            NativeErrorKind::Type,
                            "value is not iterable",
                        )?,
                    )));
                };
                self.0.phase = Phase::Iterator;
                Ok(AggregateStep::Call {
                    callable,
                    receiver: runtime.dup_jsvalue(&self.0.iterable)?,
                    resume: self,
                })
            }
            Phase::Iterator => {
                let JsValue::Object(iterator) = value else {
                    runtime.release_jsvalue(value)?;
                    return Ok(AggregateStep::Complete(Completion::Throw(
                        runtime.new_native_error_jsvalue(
                            self.0.realm,
                            NativeErrorKind::Type,
                            "not an object",
                        )?,
                    )));
                };
                let iterator = ObjectRef::from_owned_handle(runtime.clone(), iterator);
                let iterable = std::mem::replace(&mut self.0.iterable, JsValue::Undefined);
                runtime.release_jsvalue(iterable)?;
                self.0.iterator = Some(iterator.clone());
                self.0.phase = Phase::NextMethod;
                let key =
                    runtime.pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Next)?;
                Ok(AggregateStep::Read {
                    receiver: JsValue::Object(iterator.into_handle()),
                    key,
                    resume: self,
                })
            }
            Phase::NextMethod => {
                runtime.release_jsvalue(std::mem::replace(&mut self.0.next, value))?;
                self.0.result = Some(runtime.new_array(self.0.realm)?);
                self.next(runtime)
            }
            _ => {
                runtime.release_jsvalue(value)?;
                Err(RuntimeError::Invariant(
                    "AggregateError value phase mismatch",
                ))
            }
        }
    }
    fn next(mut self, runtime: &Runtime) -> Result<AggregateStep, RuntimeError> {
        self.0.phase = Phase::Next;
        Ok(AggregateStep::Next {
            iterator: self
                .0
                .iterator
                .clone()
                .ok_or(RuntimeError::Invariant("AggregateError iterator missing"))?,
            next: runtime.dup_jsvalue(&self.0.next)?,
            resume: self,
        })
    }
    fn close(self, runtime: &Runtime, value: JsValue) -> Result<AggregateStep, RuntimeError> {
        let Some(iterator) = self.0.iterator.clone() else {
            runtime.release_jsvalue(value)?;
            return Err(RuntimeError::Invariant("AggregateError iterator missing"));
        };
        Ok(AggregateStep::Close {
            iterator,
            completion: Completion::Throw(value),
        })
    }
    pub(crate) fn item(
        mut self,
        runtime: &Runtime,
        result: ObjectIteratorStep,
    ) -> Result<AggregateStep, RuntimeError> {
        if !matches!(self.0.phase, Phase::Next) {
            if let ObjectIteratorStep::Yield(value) | ObjectIteratorStep::Throw(value) = result {
                runtime.release_jsvalue(value)?;
            }
            return Err(RuntimeError::Invariant(
                "AggregateError iterator phase mismatch",
            ));
        }
        let value = match result {
            ObjectIteratorStep::Yield(value) => value,
            ObjectIteratorStep::Done => {
                let result = self
                    .0
                    .result
                    .clone()
                    .ok_or(RuntimeError::Invariant("AggregateError result missing"))?;
                return Ok(AggregateStep::Complete(Completion::Return(
                    JsValue::Object(result.into_handle()),
                )));
            }
            ObjectIteratorStep::Throw(value) => return self.close(runtime, value),
        };
        self.0.item = Some(value);
        let result = self
            .0
            .result
            .as_ref()
            .ok_or(RuntimeError::Invariant("AggregateError result missing"))?;
        let key = runtime.intern_property_key(&self.0.index.to_string())?;
        let outcome =
            runtime.define_selected_set_data(result, &key, self.0.item.as_ref().unwrap(), false)?;
        runtime.release_jsvalue(self.0.item.take().unwrap())?;
        match outcome {
            crate::engine::object::operations::PropertyDefineOutcome::Defined(true) => {}
            crate::engine::object::operations::PropertyDefineOutcome::Defined(false) => {
                return Err(RuntimeError::Invariant(
                    "fresh AggregateError Array rejected indexed property",
                ));
            }
            crate::engine::object::operations::PropertyDefineOutcome::Throw(value) => {
                return Ok(AggregateStep::Complete(Completion::Throw(
                    runtime.into_jsvalue(value)?,
                )));
            }
        }
        self.0.index = self.0.index.checked_add(1).ok_or(RuntimeError::Invariant(
            "AggregateError iterable exceeded Uint64 indices",
        ))?;
        self.next(runtime)
    }
}
pub(crate) fn finish(
    runtime: &Runtime,
    realm: ContextId,
    mut step: AggregateStep,
) -> Result<Completion, RuntimeError> {
    loop {
        step = match step {
            AggregateStep::Complete(result) => return Ok(result),
            AggregateStep::Read {
                receiver,
                key,
                resume,
            } => resume.resume(
                runtime,
                runtime.get_value_property_in_realm_jsvalue(realm, receiver, &key)?,
            )?,
            AggregateStep::Call {
                callable,
                receiver,
                resume,
            } => resume.resume(
                runtime,
                runtime.call_internal_jsvalue(realm, &callable, receiver, Vec::new())?,
            )?,
            AggregateStep::Next {
                iterator,
                next,
                resume,
            } => resume.item(
                runtime,
                finish_next(
                    runtime,
                    realm,
                    NextStep::start_jsvalue(runtime, realm, iterator, next)?,
                )?,
            )?,
            AggregateStep::Close {
                iterator,
                completion,
            } => {
                return finish_close(
                    runtime,
                    realm,
                    CloseStep::start(runtime, realm, iterator, completion)?,
                );
            }
        };
    }
}

// S11 all-domain protocol bound; inline completion stays allocation-free.
const _: () = assert!(std::mem::size_of::<AggregateStep>() <= 64);
