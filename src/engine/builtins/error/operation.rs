//! Error construction and stringification retain intermediate values in observable order.
#[cfg(feature = "stack-vm")]
use crate::engine::builtins::native::NativeFunctionId;
use crate::engine::{
    api::{error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError},
    builtins::native::ErrorConstructorKind,
    heap::ContextId,
    object::{ObjectRef, PropertyKey},
    value::{JsString, Value, conversion::NativeConversion},
    vm::{
        Completion,
        call::{NativeArguments, NativeInvocation},
    },
};
#[derive(Clone, Copy)]
pub(crate) enum ErrorKind {
    Constructor(ErrorConstructorKind),
    ToString,
}
impl ErrorKind {
    #[cfg(feature = "stack-vm")]
    pub(crate) fn for_target(target: NativeFunctionId) -> Option<Self> {
        match target {
            NativeFunctionId::ErrorConstructor(kind) => Some(Self::Constructor(kind)),
            NativeFunctionId::ErrorPrototypeToString => Some(Self::ToString),
            _ => None,
        }
    }
}
pub(crate) enum ErrorStep {
    Complete(Completion),
    Read {
        receiver: Value,
        key: PropertyKey,
        resume: ErrorResume,
    },
    String {
        value: Value,
        resume: ErrorResume,
    },
    Has {
        object: ObjectRef,
        key: PropertyKey,
        resume: ErrorResume,
    },
    Aggregate {
        iterable: Value,
        resume: ErrorResume,
    },
}
enum Phase {
    Prototype,
    Message,
    CauseHas,
    Cause,
    Aggregate,
    NameRead,
    Name,
    TextRead,
    Text,
}
pub(crate) struct ErrorResume {
    realm: ContextId,
    kind: ErrorKind,
    phase: Phase,
    object: Option<ObjectRef>,
    new_target: Value,
    arguments: Vec<Value>,
    actual: usize,
    name: JsString,
}
impl ErrorStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        kind: ErrorKind,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        let mut resume = ErrorResume {
            realm,
            kind,
            phase: Phase::Prototype,
            object: None,
            new_target: Value::Undefined,
            arguments: arguments.readable.clone(),
            actual: arguments.actual_arg_count,
            name: JsString::from_static("Error"),
        };
        match kind {
            ErrorKind::Constructor(_) => {
                let NativeInvocation::Construct { new_target } = invocation else {
                    return Err(RuntimeError::Invariant(
                        "Error constructor requires constructor-or-function invocation",
                    ));
                };
                resume.new_target = if matches!(new_target, Value::Undefined) {
                    Value::Object(runtime.active_function()?)
                } else {
                    new_target.clone()
                };
                Ok(Self::Read {
                    receiver: resume.new_target.clone(),
                    key: runtime.intern_property_key("prototype")?,
                    resume,
                })
            }
            ErrorKind::ToString => {
                let NativeInvocation::Call { this_value } = invocation else {
                    return Err(RuntimeError::Invariant(
                        "Error string requires generic invocation",
                    ));
                };
                let Value::Object(object) = this_value else {
                    return Ok(Self::Complete(Completion::Throw(
                        runtime.new_native_error(realm, NativeErrorKind::Type, "not an object")?,
                    )));
                };
                resume.object = Some(object.clone());
                resume.phase = Phase::NameRead;
                Ok(Self::Read {
                    receiver: this_value.clone(),
                    key: runtime.intern_property_key("name")?,
                    resume,
                })
            }
        }
    }
}
impl ErrorResume {
    fn object(&self) -> Result<ObjectRef, RuntimeError> {
        self.object
            .clone()
            .ok_or(RuntimeError::Invariant("Error operation object missing"))
    }
    fn aggregate_kind(&self) -> bool {
        matches!(
            self.kind,
            ErrorKind::Constructor(ErrorConstructorKind::Native(NativeErrorKind::Aggregate))
        )
    }
    pub(crate) fn resume(
        mut self,
        runtime: &Runtime,
        result: Completion,
    ) -> Result<ErrorStep, RuntimeError> {
        let value = match result {
            Completion::Return(value) => value,
            Completion::Throw(value) => return Ok(ErrorStep::Complete(Completion::Throw(value))),
        };
        match self.phase {
            Phase::Prototype => {
                let ErrorKind::Constructor(kind) = self.kind else {
                    return Err(RuntimeError::Invariant("Error prototype kind mismatch"));
                };
                let prototype = if let Value::Object(object) = value {
                    object
                } else {
                    let realm =
                        match runtime.function_realm_from_value(self.realm, &self.new_target)? {
                            NativeConversion::Value(realm) => realm,
                            NativeConversion::Throw(value) => {
                                return Ok(ErrorStep::Complete(Completion::Throw(value)));
                            }
                        };
                    let prototype = {
                        let state = runtime.0.state.borrow();
                        let context = state.heap.context(realm)?;
                        match kind {
                            ErrorConstructorKind::Error => context
                                .error_prototype
                                .ok_or(RuntimeError::Invariant("realm has no Error prototype"))?,
                            ErrorConstructorKind::Native(kind) => {
                                context.native_error_prototypes[kind.index()].ok_or(
                                    RuntimeError::Invariant("realm has no native Error prototype"),
                                )?
                            }
                        }
                    };
                    ObjectRef::from_borrowed_handle(runtime.clone(), prototype)?
                };
                self.object = Some(runtime.new_error_object(&prototype)?);
                let message = self
                    .arguments
                    .get(usize::from(self.aggregate_kind()))
                    .cloned()
                    .ok_or(RuntimeError::Invariant("Error message argv missing"))?;
                if matches!(message, Value::Undefined) {
                    self.cause(runtime)
                } else {
                    self.phase = Phase::Message;
                    Ok(ErrorStep::String {
                        value: message,
                        resume: self,
                    })
                }
            }
            Phase::Cause => {
                runtime.define_function_data_property(
                    &self.object()?,
                    "cause",
                    value,
                    true,
                    true,
                )?;
                self.aggregate(runtime)
            }
            Phase::Aggregate => {
                if !matches!(value, Value::Object(_)) {
                    return Err(RuntimeError::Invariant(
                        "AggregateError iterable returned non-object",
                    ));
                }
                runtime.define_function_data_property(
                    &self.object()?,
                    "errors",
                    value,
                    true,
                    true,
                )?;
                self.finish(runtime)
            }
            Phase::NameRead => {
                if matches!(value, Value::Undefined) {
                    self.text(runtime)
                } else {
                    self.phase = Phase::Name;
                    Ok(ErrorStep::String {
                        value,
                        resume: self,
                    })
                }
            }
            Phase::TextRead => {
                self.phase = Phase::Text;
                if matches!(value, Value::Undefined) {
                    self.string(runtime, NativeConversion::Value(JsString::from_static("")))
                } else {
                    Ok(ErrorStep::String {
                        value,
                        resume: self,
                    })
                }
            }
            _ => Err(RuntimeError::Invariant("Error value reply phase mismatch")),
        }
    }
    pub(crate) fn string(
        mut self,
        runtime: &Runtime,
        result: NativeConversion<JsString>,
    ) -> Result<ErrorStep, RuntimeError> {
        let value = match result {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => {
                return Ok(ErrorStep::Complete(Completion::Throw(value)));
            }
        };
        match self.phase {
            Phase::Message => {
                runtime.define_function_data_property(
                    &self.object()?,
                    "message",
                    Value::String(value),
                    true,
                    true,
                )?;
                self.cause(runtime)
            }
            Phase::Name => {
                self.name = value;
                self.text(runtime)
            }
            Phase::Text => {
                let value = if self.name.is_empty() {
                    value
                } else if value.is_empty() {
                    self.name
                } else {
                    self.name
                        .try_concat(&JsString::from_static(": "))?
                        .try_concat(&value)?
                };
                Ok(ErrorStep::Complete(Completion::Return(Value::String(
                    value,
                ))))
            }
            _ => Err(RuntimeError::Invariant("Error string reply phase mismatch")),
        }
    }
    fn text(mut self, runtime: &Runtime) -> Result<ErrorStep, RuntimeError> {
        self.phase = Phase::TextRead;
        Ok(ErrorStep::Read {
            receiver: Value::Object(self.object()?),
            key: runtime.intern_property_key("message")?,
            resume: self,
        })
    }
    fn cause(mut self, runtime: &Runtime) -> Result<ErrorStep, RuntimeError> {
        let index = usize::from(self.aggregate_kind()) + 1;
        if self.actual > index
            && let Some(Value::Object(options)) = self.arguments.get(index)
        {
            let object = options.clone();
            self.phase = Phase::CauseHas;
            return Ok(ErrorStep::Has {
                object,
                key: runtime.intern_property_key("cause")?,
                resume: self,
            });
        }
        self.aggregate(runtime)
    }
    pub(crate) fn boolean(
        mut self,
        runtime: &Runtime,
        result: NativeConversion<bool>,
    ) -> Result<ErrorStep, RuntimeError> {
        if !matches!(self.phase, Phase::CauseHas) {
            return Err(RuntimeError::Invariant(
                "Error cause boolean phase mismatch",
            ));
        }
        let value = match result {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => {
                return Ok(ErrorStep::Complete(Completion::Throw(value)));
            }
        };
        if !value {
            return self.aggregate(runtime);
        }
        let receiver = self.arguments[usize::from(self.aggregate_kind()) + 1].clone();
        self.phase = Phase::Cause;
        Ok(ErrorStep::Read {
            receiver,
            key: runtime.intern_property_key("cause")?,
            resume: self,
        })
    }
    fn aggregate(mut self, runtime: &Runtime) -> Result<ErrorStep, RuntimeError> {
        if self.aggregate_kind() {
            self.phase = Phase::Aggregate;
            Ok(ErrorStep::Aggregate {
                iterable: self
                    .arguments
                    .first()
                    .cloned()
                    .ok_or(RuntimeError::Invariant(
                        "AggregateError errors argv missing",
                    ))?,
                resume: self,
            })
        } else {
            self.finish(runtime)
        }
    }
    fn finish(self, runtime: &Runtime) -> Result<ErrorStep, RuntimeError> {
        let value = Value::Object(self.object()?);
        runtime.ensure_error_backtrace(&value, true, None)?;
        Ok(ErrorStep::Complete(Completion::Return(value)))
    }
}
pub(crate) fn finish(
    runtime: &Runtime,
    realm: ContextId,
    mut step: ErrorStep,
) -> Result<Completion, RuntimeError> {
    loop {
        step = match step {
            ErrorStep::Complete(result) => return Ok(result),
            ErrorStep::Read {
                receiver,
                key,
                resume,
            } => resume.resume(
                runtime,
                runtime.get_value_property_in_realm(realm, receiver, &key)?,
            )?,
            ErrorStep::String { value, resume } => {
                resume.string(runtime, runtime.native_to_js_string(realm, &value)?)?
            }
            ErrorStep::Has {
                object,
                key,
                resume,
            } => resume.boolean(
                runtime,
                runtime.internal_has_property(realm, &object, &key)?,
            )?,
            ErrorStep::Aggregate { iterable, resume } => resume.resume(
                runtime,
                super::aggregate::finish(
                    runtime,
                    realm,
                    super::aggregate::AggregateStep::start(runtime, realm, iterable)?,
                )?,
            )?,
        };
    }
}
