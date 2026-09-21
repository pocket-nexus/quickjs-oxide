//! Helper creation owns count conversion and cached-next acquisition.
use super::{
    quickjs_to_int64_free,
    step::{CloseStep, finish_close},
};
use crate::engine::{
    api::{error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError},
    heap::{ContextId, IteratorHelperKind},
    object::{ObjectRef, PropertyKey},
    value::{JsValue, conversion::NativeConversion},
    vm::{
        Completion,
        call::{NativeArguments, NativeInvocation},
    },
};
pub(crate) enum CreateStep {
    Complete(Completion),
    Number {
        value: JsValue,
        resume: CreateResume,
    },
    Read {
        object: ObjectRef,
        key: PropertyKey,
        resume: CreateResume,
    },
    CloseInvalidCount {
        iterator: ObjectRef,
        resume: CreateResume,
    },
    Close {
        iterator: ObjectRef,
        completion: Completion,
    },
}
pub(crate) struct CreateResume(Box<CreateResumeState>);
impl std::ops::Deref for CreateResume {
    type Target = CreateResumeState;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for CreateResume {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
const _: () = assert!(std::mem::size_of::<CreateResume>() <= 8);
pub(crate) struct CreateResumeState {
    realm: ContextId,
    source: ObjectRef,
    kind: IteratorHelperKind,
    callback: JsValue,
    count: i64,
}
impl Drop for CreateResumeState {
    fn drop(&mut self) {
        let value = std::mem::replace(&mut self.callback, JsValue::Undefined);
        let _ = self.source.runtime().release_jsvalue(value);
    }
}
impl CreateStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        kind: IteratorHelperKind,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        let source = match runtime.iterator_receiver(realm, invocation)? {
            NativeConversion::Value(source) => source,
            NativeConversion::Throw(value) => {
                return Ok(Self::Complete(Completion::Throw(
                    runtime.into_jsvalue(value)?,
                )));
            }
        };
        let argument = runtime.dup_jsvalue(arguments.readable.first().ok_or(
            RuntimeError::Invariant("Iterator helper argument was not padded"),
        )?)?;
        let mut resume = CreateResume(Box::new(CreateResumeState {
            realm,
            source,
            kind,
            callback: JsValue::Undefined,
            count: 0,
        }));
        if matches!(kind, IteratorHelperKind::Drop | IteratorHelperKind::Take) {
            return Ok(Self::Number {
                value: argument,
                resume,
            });
        }
        resume.callback = argument;
        if let NativeConversion::Throw(value) =
            runtime.iterator_callable_jsvalue(realm, &resume.callback)?
        {
            return resume.close(runtime, runtime.into_jsvalue(value)?);
        }
        resume.read(runtime)
    }
}
impl CreateResume {
    fn close(self, _runtime: &Runtime, value: JsValue) -> Result<CreateStep, RuntimeError> {
        Ok(CreateStep::Close {
            iterator: self.0.source.clone(),
            completion: Completion::Throw(value),
        })
    }
    fn read(self, runtime: &Runtime) -> Result<CreateStep, RuntimeError> {
        Ok(CreateStep::Read {
            object: self.0.source.clone(),
            key: runtime.pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Next)?,
            resume: self,
        })
    }
    pub(crate) fn number(
        mut self,
        runtime: &Runtime,
        reply: NativeConversion<f64>,
    ) -> Result<CreateStep, RuntimeError> {
        let number = match reply {
            NativeConversion::Value(number) => number,
            NativeConversion::Throw(value) => {
                return self.close(runtime, runtime.into_jsvalue(value)?);
            }
        };
        let count = if number == f64::INFINITY {
            (1_i64 << 53) - 1
        } else {
            quickjs_to_int64_free(number.trunc())
        };
        if number.is_nan() || number == f64::NEG_INFINITY || count < 0 {
            return Ok(CreateStep::CloseInvalidCount {
                iterator: self.0.source.clone(),
                resume: self,
            });
        }
        self.0.count = count;
        self.read(runtime)
    }
    pub(crate) fn invalid_count(
        self,
        runtime: &Runtime,
        reply: Completion,
    ) -> Result<CreateStep, RuntimeError> {
        let (Completion::Return(value) | Completion::Throw(value)) = reply;
        runtime.release_jsvalue(value)?;
        Ok(CreateStep::Complete(Completion::Throw(
            runtime.new_native_error_jsvalue(
                self.0.realm,
                NativeErrorKind::Range,
                "must be positive",
            )?,
        )))
    }
    pub(crate) fn resume(
        self,
        runtime: &Runtime,
        reply: Completion,
    ) -> Result<CreateStep, RuntimeError> {
        match reply {
            Completion::Throw(value) => self.close(runtime, value),
            Completion::Return(next) => {
                let result = runtime.new_iterator_helper(
                    self.0.realm,
                    &self.0.source,
                    &next,
                    &self.0.callback,
                    self.0.count,
                    self.0.kind,
                );
                runtime.release_jsvalue(next)?;
                Ok(CreateStep::Complete(Completion::Return(JsValue::Object(
                    result?.into_handle(),
                ))))
            }
        }
    }
}
pub(crate) fn finish(
    runtime: &Runtime,
    realm: ContextId,
    mut step: CreateStep,
) -> Result<Completion, RuntimeError> {
    loop {
        step = match step {
            CreateStep::Complete(result) => return Ok(result),
            CreateStep::Number { value, resume } => {
                resume.number(runtime, runtime.native_to_number_jsvalue(realm, value)?)?
            }
            CreateStep::Read {
                object,
                key,
                resume,
            } => resume.resume(
                runtime,
                runtime.internal_get_jsvalue(
                    realm,
                    &object,
                    &key,
                    JsValue::Object(object.clone().into_handle()),
                )?,
            )?,
            CreateStep::CloseInvalidCount { iterator, resume } => {
                let result = finish_close(
                    runtime,
                    realm,
                    CloseStep::start(
                        runtime,
                        realm,
                        iterator,
                        Completion::Throw(JsValue::Undefined),
                    )?,
                )?;
                resume.invalid_count(runtime, result)?
            }
            CreateStep::Close {
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
const _: () = assert!(std::mem::size_of::<CreateStep>() <= 64);
