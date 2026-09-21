//! String split retains its fresh result through limit and separator coercion.
use crate::engine::{
    api::{error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError},
    heap::ContextId,
    object::{ObjectRef, PropertyKey, WellKnownSymbol},
    value::{JsString, JsValue, Value, conversion::NativeConversion},
    vm::{
        Completion, ToPrimitiveHint,
        call::{DirectCallTarget, NativeArguments, NativeInvocation},
    },
};
pub(crate) enum StringSplitStep {
    Complete(Completion),
    Read { resume: StringSplitResume },
    Primitive { resume: StringSplitResume },
    Call { resume: StringSplitResume },
}
pub(crate) struct StringSplitResume(Box<StringSplitResumeState>);
impl std::ops::Deref for StringSplitResume {
    type Target = StringSplitResumeState;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for StringSplitResume {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
const _: () = assert!(std::mem::size_of::<StringSplitResume>() <= 8);
pub(crate) struct StringSplitResumeState {
    step_pending: StringSplitStepPending,
    runtime: Runtime,
    realm: ContextId,
    receiver: JsValue,
    separator: JsValue,
    limit: JsValue,
    phase: SplitPhase,
}
impl Drop for StringSplitResumeState {
    /// Release the internal edges still owned when the request is abandoned.
    /// Drained fields are `Undefined` here; releases are defer-safe.
    fn drop(&mut self) {
        for value in [
            std::mem::replace(&mut self.receiver, JsValue::Undefined),
            std::mem::replace(&mut self.separator, JsValue::Undefined),
            std::mem::replace(&mut self.limit, JsValue::Undefined),
        ] {
            let _ = self.runtime.release_jsvalue(value);
        }
        self.step_pending.release_owned(&self.runtime);
    }
}
enum SplitPhase {
    Method,
    Called,
    Source,
    Limit {
        source: JsString,
        result: ObjectRef,
    },
    Separator {
        source: JsString,
        result: ObjectRef,
        limit: u32,
    },
}
impl StringSplitStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        let NativeInvocation::Call { this_value } = invocation else {
            return Err(RuntimeError::Invariant(
                "String split did not receive a generic invocation",
            ));
        };
        let this_value = runtime.dup_jsvalue(this_value)?;
        if matches!(this_value, JsValue::Undefined | JsValue::Null) {
            return Ok(Self::Complete(Completion::Throw(
                runtime.new_native_error_jsvalue(
                    realm,
                    NativeErrorKind::Type,
                    "cannot convert to object",
                )?,
            )));
        }
        let separator = runtime.dup_jsvalue(arguments.readable.first().ok_or(
            RuntimeError::Invariant("String split separator argv was not padded"),
        )?)?;
        let limit = runtime.dup_jsvalue(arguments.readable.get(1).ok_or(
            RuntimeError::Invariant("String split limit argv was not padded"),
        )?)?;
        let resume = StringSplitResume(Box::new(StringSplitResumeState {
            step_pending: StringSplitStepPending::default(),
            runtime: runtime.clone(),
            realm,
            receiver: this_value,
            separator,
            limit,
            phase: SplitPhase::Method,
        }));
        if let JsValue::Object(id) = &resume.separator {
            let object = ObjectRef::from_borrowed_handle(runtime.clone(), *id)?;
            Ok(Self::make_read(
                object,
                PropertyKey::from(runtime.well_known_symbol(WellKnownSymbol::Split)),
                resume,
            ))
        } else {
            resume.source(runtime)
        }
    }
}
impl StringSplitResume {
    fn source(mut self, runtime: &Runtime) -> Result<StringSplitStep, RuntimeError> {
        Ok(StringSplitStep::make_primitive(
            runtime.dup_jsvalue(&self.0.receiver)?,
            ToPrimitiveHint::String,
            {
                let updated_0 = SplitPhase::Source;
                self.0.phase = updated_0;
                self
            },
        ))
    }
    fn separator(
        mut self,
        runtime: &Runtime,
        source: JsString,
        result: ObjectRef,
        limit: u32,
    ) -> Result<StringSplitStep, RuntimeError> {
        Ok(StringSplitStep::make_primitive(
            runtime.dup_jsvalue(&self.0.separator)?,
            ToPrimitiveHint::String,
            {
                let updated_0 = SplitPhase::Separator {
                    source,
                    result,
                    limit,
                };
                self.0.phase = updated_0;
                self
            },
        ))
    }
    pub(crate) fn resume(
        mut self,
        runtime: &Runtime,
        result: Completion,
    ) -> Result<StringSplitStep, RuntimeError> {
        let value = match result {
            Completion::Return(value) => runtime.root_and_release_jsvalue(value)?,
            Completion::Throw(value) => {
                return Ok(StringSplitStep::Complete(Completion::Throw(value)));
            }
        };
        let realm = self.0.realm;
        let phase = std::mem::replace(&mut self.0.phase, SplitPhase::Method);
        match phase {
            SplitPhase::Method => {
                if matches!(value, Value::Undefined | Value::Null) {
                    return self.source(runtime);
                }
                let callable = match value {
                    Value::Object(object) => runtime.as_callable(&object)?,
                    _ => None,
                };
                let Some(callable) = callable else {
                    return Ok(StringSplitStep::Complete(Completion::Throw(
                        runtime.new_native_error_jsvalue(
                            realm,
                            NativeErrorKind::Type,
                            "not a function",
                        )?,
                    )));
                };
                let mut arguments = Vec::new();
                if arguments.try_reserve_exact(2).is_err() {
                    return Ok(StringSplitStep::Complete(Completion::Throw(
                        runtime.new_native_error_jsvalue(
                            realm,
                            NativeErrorKind::Internal,
                            "out of memory",
                        )?,
                    )));
                }
                arguments.push(runtime.dup_jsvalue(&self.0.receiver)?);
                arguments.push(runtime.dup_jsvalue(&self.0.limit)?);
                Ok(StringSplitStep::make_call(
                    DirectCallTarget::Callable(callable),
                    runtime.dup_jsvalue(&self.0.separator)?,
                    arguments,
                    {
                        let updated_0 = SplitPhase::Called;
                        self.0.phase = updated_0;
                        self
                    },
                ))
            }
            SplitPhase::Called => Ok(StringSplitStep::Complete(Completion::Return(
                runtime.into_jsvalue(value)?,
            ))),
            SplitPhase::Source => {
                if matches!(value, Value::Object(_)) {
                    return Err(RuntimeError::Invariant(
                        "String split source conversion returned an object",
                    ));
                }
                let source = match runtime.native_to_js_string(realm, &value)? {
                    NativeConversion::Value(value) => value.linearize(),
                    NativeConversion::Throw(value) => {
                        return Ok(StringSplitStep::Complete(Completion::Throw(
                            runtime.into_jsvalue(value)?,
                        )));
                    }
                };
                let result = runtime.new_array(realm)?;
                if matches!(self.0.limit, JsValue::Undefined) {
                    self.separator(runtime, source, result, u32::MAX)
                } else {
                    Ok(StringSplitStep::make_primitive(
                        runtime.dup_jsvalue(&self.0.limit)?,
                        ToPrimitiveHint::Number,
                        {
                            let updated_0 = SplitPhase::Limit { source, result };
                            self.0.phase = updated_0;
                            self
                        },
                    ))
                }
            }
            SplitPhase::Limit { source, result } => {
                if matches!(value, Value::Object(_)) {
                    return Err(RuntimeError::Invariant(
                        "String split limit conversion returned an object",
                    ));
                }
                let limit = match runtime.native_to_number(realm, &value)? {
                    NativeConversion::Value(value) => Runtime::to_uint32_number(value),
                    NativeConversion::Throw(value) => {
                        return Ok(StringSplitStep::Complete(Completion::Throw(
                            runtime.into_jsvalue(value)?,
                        )));
                    }
                };
                {
                    let updated_0 = SplitPhase::Called;
                    self.0.phase = updated_0;
                    self
                }
                .separator(runtime, source, result, limit)
            }
            SplitPhase::Separator {
                source,
                result,
                limit,
            } => {
                if matches!(value, Value::Object(_)) {
                    return Err(RuntimeError::Invariant(
                        "String split separator conversion returned an object",
                    ));
                }
                let separator = match runtime.native_to_js_string(realm, &value)? {
                    NativeConversion::Value(value) => value.linearize(),
                    NativeConversion::Throw(value) => {
                        return Ok(StringSplitStep::Complete(Completion::Throw(
                            runtime.into_jsvalue(value)?,
                        )));
                    }
                };
                Ok(StringSplitStep::Complete(runtime.finish_string_split(
                    realm,
                    source,
                    result,
                    &self.0.separator,
                    separator,
                    limit,
                )?))
            }
        }
    }
}
pub(super) fn finish(
    runtime: &Runtime,
    realm: ContextId,
    mut step: StringSplitStep,
) -> Result<Completion, RuntimeError> {
    loop {
        step = match step {
            StringSplitStep::Complete(result) => return Ok(result),
            StringSplitStep::Read { mut resume } => {
                let object = resume.take_read_object();
                let key = resume.take_read_key();
                resume.resume(
                    runtime,
                    runtime.get_property_in_realm(realm, &object, &key)?,
                )?
            }
            StringSplitStep::Primitive { mut resume } => {
                let value = resume.take_primitive_value();
                let hint = resume.take_primitive_hint();
                {
                    let result = if matches!(value, JsValue::Object(_)) {
                        runtime.to_primitive_jsvalue(realm, value, hint)?
                    } else {
                        Completion::Return(value)
                    };
                    resume.resume(runtime, result)?
                }
            }
            StringSplitStep::Call { mut resume } => {
                let target = resume.take_call_target();
                let receiver = runtime.root_and_release_jsvalue(resume.take_call_receiver())?;
                let arguments = resume
                    .take_call_arguments()
                    .into_iter()
                    .map(|value| runtime.root_and_release_jsvalue(value))
                    .collect::<Result<Vec<_>, _>>()?;
                {
                    let DirectCallTarget::Callable(callable) = target else {
                        return Err(RuntimeError::Invariant(
                            "String split requested an invalid call target",
                        ));
                    };
                    resume.resume(
                        runtime,
                        runtime.call_internal(realm, &callable, receiver, &arguments)?,
                    )?
                }
            }
        };
    }
}

#[derive(Default)]
pub(crate) struct StringSplitStepPending {
    object: Option<ObjectRef>,
    key: Option<PropertyKey>,
    value: Option<JsValue>,
    hint: Option<ToPrimitiveHint>,
    target: Option<DirectCallTarget>,
    receiver: Option<JsValue>,
    arguments: Option<Vec<JsValue>>,
}
impl StringSplitStepPending {
    /// Release every edge that was not consumed by a completed step.
    fn release_owned(&mut self, runtime: &Runtime) {
        let _ = self.object.take();
        let _ = self.key.take();
        let _ = self.hint.take();
        let _ = self.target.take();
        for value in [self.value.take(), self.receiver.take()]
            .into_iter()
            .flatten()
        {
            let _ = runtime.release_jsvalue(value);
        }
        for value in self.arguments.take().into_iter().flatten() {
            let _ = runtime.release_jsvalue(value);
        }
    }
}
impl StringSplitStep {
    pub(crate) fn make_read(
        object: ObjectRef,
        key: PropertyKey,
        mut resume: StringSplitResume,
    ) -> Self {
        resume.0.step_pending.object = Some(object);
        resume.0.step_pending.key = Some(key);
        Self::Read { resume }
    }
    pub(crate) fn make_primitive(
        value: JsValue,
        hint: ToPrimitiveHint,
        mut resume: StringSplitResume,
    ) -> Self {
        resume.0.step_pending.value = Some(value);
        resume.0.step_pending.hint = Some(hint);
        Self::Primitive { resume }
    }
    pub(crate) fn make_call(
        target: DirectCallTarget,
        receiver: JsValue,
        arguments: Vec<JsValue>,
        mut resume: StringSplitResume,
    ) -> Self {
        resume.0.step_pending.target = Some(target);
        resume.0.step_pending.receiver = Some(receiver);
        resume.0.step_pending.arguments = Some(arguments);
        Self::Call { resume }
    }
}
impl StringSplitResume {
    pub(crate) fn take_read_object(&mut self) -> ObjectRef {
        self.0
            .step_pending
            .object
            .take()
            .expect("StringSplitStep::Read lost object")
    }
    pub(crate) fn take_read_key(&mut self) -> PropertyKey {
        self.0
            .step_pending
            .key
            .take()
            .expect("StringSplitStep::Read lost key")
    }

    pub(crate) fn take_primitive_value(&mut self) -> JsValue {
        self.0
            .step_pending
            .value
            .take()
            .expect("StringSplitStep::Primitive lost value")
    }
    pub(crate) fn take_primitive_hint(&mut self) -> ToPrimitiveHint {
        self.0
            .step_pending
            .hint
            .take()
            .expect("StringSplitStep::Primitive lost hint")
    }

    pub(crate) fn take_call_target(&mut self) -> DirectCallTarget {
        self.0
            .step_pending
            .target
            .take()
            .expect("StringSplitStep::Call lost target")
    }
    pub(crate) fn take_call_receiver(&mut self) -> JsValue {
        self.0
            .step_pending
            .receiver
            .take()
            .expect("StringSplitStep::Call lost receiver")
    }
    pub(crate) fn take_call_arguments(&mut self) -> Vec<JsValue> {
        self.0
            .step_pending
            .arguments
            .take()
            .expect("StringSplitStep::Call lost arguments")
    }
}

const _: () = assert!(std::mem::size_of::<StringSplitStep>() <= 64);

// S11 all-domain protocol bound; inline completion stays allocation-free.
const _: () = assert!(std::mem::size_of::<StringSplitStep>() <= 64);
