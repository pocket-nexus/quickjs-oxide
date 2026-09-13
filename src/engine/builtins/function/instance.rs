//! Instanceof owns method selection and prototype walking across user callbacks.
use crate::engine::{
    api::{
        Error, ErrorKind, error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError,
    },
    builtins::native::NativeFunctionId,
    heap::{ContextId, ObjectPayload},
    object::{CallableRef, ObjectRef, PropertyKey, WellKnownSymbol},
    value::{Value, conversion::NativeConversion},
    vm::{
        Completion,
        call::{NativeArguments, NativeInvocation},
    },
};
pub(crate) enum InstanceStep {
    Complete(Completion),
    Read {
        object: ObjectRef,
        key: PropertyKey,
        resume: InstanceResume,
    },
    Call {
        callable: CallableRef,
        receiver: Value,
        arguments: Vec<Value>,
        delegate: bool,
        resume: InstanceResume,
    },
    Prototype {
        object: ObjectRef,
        resume: InstanceResume,
    },
}
pub(crate) struct InstanceResume {
    realm: ContextId,
    candidate: Value,
    target: ObjectRef,
    phase: Phase,
}
enum Phase {
    Method { delegate: bool },
    Result,
    Prototype,
    Walk(ObjectRef),
}
impl InstanceStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        candidate: Value,
        target: ObjectRef,
    ) -> Result<Self, RuntimeError> {
        Self::method(runtime, realm, candidate, target, false)
    }
    fn method(
        runtime: &Runtime,
        realm: ContextId,
        candidate: Value,
        target: ObjectRef,
        delegate: bool,
    ) -> Result<Self, RuntimeError> {
        Ok(Self::Read {
            object: target.clone(),
            key: PropertyKey::from(runtime.well_known_symbol(WellKnownSymbol::HasInstance)),
            resume: InstanceResume {
                realm,
                candidate,
                target,
                phase: Phase::Method { delegate },
            },
        })
    }
    pub(crate) fn native(
        runtime: &Runtime,
        realm: ContextId,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        let NativeInvocation::Call { this_value } = invocation else {
            return Err(RuntimeError::Invariant(
                "hasInstance requires generic invocation",
            ));
        };
        let target = match this_value {
            Value::Object(target) => runtime.as_callable(target)?,
            _ => None,
        };
        let Some(target) = target else {
            return Ok(Self::Complete(Completion::Return(Value::Bool(false))));
        };
        Self::ordinary(
            runtime,
            realm,
            &target,
            arguments
                .readable
                .first()
                .cloned()
                .unwrap_or(Value::Undefined),
        )
    }
    pub(crate) fn ordinary(
        runtime: &Runtime,
        realm: ContextId,
        target: &CallableRef,
        candidate: Value,
    ) -> Result<Self, RuntimeError> {
        let bound = {
            let state = runtime.0.state.borrow();
            match &state.heap.object(target.as_object().object_id())?.payload {
                ObjectPayload::BoundFunction { target, .. } => Some(*target),
                ObjectPayload::NativeFunction { .. }
                | ObjectPayload::BytecodeFunction { .. }
                | ObjectPayload::Proxy(_) => None,
                _ => {
                    return Err(RuntimeError::Invariant(
                        "ordinary instanceof received a non-callable target",
                    ));
                }
            }
        };
        if let Some(bound) = bound {
            let target = ObjectRef::from_borrowed_handle(runtime.clone(), bound)?;
            return Self::method(runtime, realm, candidate, target, true);
        }
        if !matches!(candidate, Value::Object(_)) {
            return Ok(Self::Complete(Completion::Return(Value::Bool(false))));
        }
        Ok(Self::Read {
            object: target.as_object().clone(),
            key: runtime.intern_property_key("prototype")?,
            resume: InstanceResume {
                realm,
                candidate,
                target: target.as_object().clone(),
                phase: Phase::Prototype,
            },
        })
    }
}
impl InstanceResume {
    pub(crate) fn resume(
        self,
        runtime: &Runtime,
        result: Completion,
    ) -> Result<InstanceStep, RuntimeError> {
        let value = match result {
            Completion::Return(value) => value,
            result @ Completion::Throw(_) => return Ok(InstanceStep::Complete(result)),
        };
        match self.phase {
            Phase::Method { delegate } => {
                if matches!(value, Value::Null | Value::Undefined) {
                    let Some(target) = runtime.as_callable(&self.target)? else {
                        return if delegate {
                            Ok(InstanceStep::Complete(Completion::Throw(
                                runtime.new_native_error(
                                    self.realm,
                                    NativeErrorKind::Type,
                                    "invalid 'instanceof' right operand",
                                )?,
                            )))
                        } else {
                            Err(RuntimeError::Engine(Error::new(
                                ErrorKind::Type,
                                "invalid 'instanceof' right operand",
                            )))
                        };
                    };
                    return InstanceStep::ordinary(runtime, self.realm, &target, self.candidate);
                }
                let callable = match runtime.callable_from_value(value) {
                    Ok(callable) => callable,
                    Err(RuntimeError::Engine(error))
                        if delegate && error.kind() == ErrorKind::Type =>
                    {
                        return Ok(InstanceStep::Complete(Completion::Throw(
                            runtime.new_native_error_from_error(
                                self.realm,
                                NativeErrorKind::Type,
                                &error,
                            )?,
                        )));
                    }
                    Err(error) => return Err(error),
                };
                Ok(InstanceStep::Call {
                    callable,
                    receiver: Value::Object(self.target.clone()),
                    arguments: vec![self.candidate.clone()],
                    delegate,
                    resume: Self {
                        phase: Phase::Result,
                        ..self
                    },
                })
            }
            Phase::Result => Ok(InstanceStep::Complete(Completion::Return(Value::Bool(
                runtime.value_to_boolean(&value)?,
            )))),
            Phase::Prototype => {
                let Value::Object(prototype) = value else {
                    return Ok(InstanceStep::Complete(Completion::Throw(
                        runtime.new_native_error(
                            self.realm,
                            NativeErrorKind::Type,
                            "operand 'prototype' property is not an object",
                        )?,
                    )));
                };
                let Value::Object(candidate) = &self.candidate else {
                    return Err(RuntimeError::Invariant("instanceof lost object candidate"));
                };
                Ok(InstanceStep::Prototype {
                    object: candidate.clone(),
                    resume: Self {
                        phase: Phase::Walk(prototype),
                        ..self
                    },
                })
            }
            Phase::Walk(_) => Err(RuntimeError::Invariant(
                "prototype walk received untyped reply",
            )),
        }
    }
    pub(crate) fn prototype(
        self,
        result: NativeConversion<Option<ObjectRef>>,
    ) -> Result<InstanceStep, RuntimeError> {
        let Phase::Walk(expected) = &self.phase else {
            return Err(RuntimeError::Invariant(
                "instanceof received unexpected prototype",
            ));
        };
        Ok(match result {
            NativeConversion::Throw(value) => InstanceStep::Complete(Completion::Throw(value)),
            NativeConversion::Value(None) => {
                InstanceStep::Complete(Completion::Return(Value::Bool(false)))
            }
            NativeConversion::Value(Some(object)) if &object == expected => {
                InstanceStep::Complete(Completion::Return(Value::Bool(true)))
            }
            NativeConversion::Value(Some(object)) => InstanceStep::Prototype {
                object,
                resume: self,
            },
        })
    }
}
pub(super) fn finish(
    runtime: &Runtime,
    mut realm: ContextId,
    mut step: InstanceStep,
) -> Result<Completion, RuntimeError> {
    // The old consumer retains its bound-chain trampoline and native backtraces.
    let mut frames = Vec::new();
    let result = (|| loop {
        step = match step {
            InstanceStep::Complete(result) => return Ok(result),
            InstanceStep::Read {
                object,
                key,
                resume,
            } => resume.resume(
                runtime,
                runtime.get_property_in_realm(realm, &object, &key)?,
            )?,
            InstanceStep::Prototype { object, resume } => {
                resume.prototype(runtime.internal_get_prototype_of(realm, &object)?)?
            }
            InstanceStep::Call {
                callable,
                receiver,
                arguments,
                delegate,
                resume,
            } => {
                let standard = if delegate {
                    runtime.direct_native_callable_metadata(&callable)?
                } else {
                    None
                };
                if let Some((NativeFunctionId::FunctionPrototypeHasInstance, defining_realm, min)) =
                    standard
                {
                    frames.push(runtime.push_native_active_frame(
                        callable.as_object().clone(),
                        defining_realm,
                        NativeFunctionId::FunctionPrototypeHasInstance,
                        1,
                        1usize.max(usize::from(min)),
                    )?);
                    realm = defining_realm;
                    InstanceStep::native(
                        runtime,
                        realm,
                        &NativeInvocation::Call {
                            this_value: receiver,
                        },
                        &NativeArguments {
                            actual_arg_count: 1,
                            readable: arguments,
                        },
                    )?
                } else {
                    resume.resume(
                        runtime,
                        runtime.call_internal(realm, &callable, receiver, &arguments)?,
                    )?
                }
            }
        };
    })();
    let mut frame_error = None;
    while let Some(frame) = frames.pop() {
        if let Err(error) = frame.finish() {
            frame_error.get_or_insert(error);
        }
    }
    frame_error.map_or(result, Err)
}
