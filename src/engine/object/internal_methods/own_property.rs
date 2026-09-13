//! Proxy [[GetOwnProperty]] and its observable invariant-query order.
use super::{
    RootedProxy,
    method::{MethodResume, MethodStep},
    proxy_gopd_descriptor_is_compatible,
};
use crate::engine::api::{runtime::Runtime, runtime_error::RuntimeError};
use crate::engine::heap::ContextId;
use crate::engine::object::operations::{
    descriptor_to_validation_record, validation_record_to_complete,
};
use crate::engine::object::property::validate_and_apply_property_descriptor;
use crate::engine::object::{
    CompleteOrdinaryPropertyDescriptor, ObjectRef, OrdinaryPropertyDescriptor, PropertyKey,
};
use crate::engine::value::{Value, conversion::NativeConversion};
use crate::engine::vm::{Completion, call::DirectCallTarget};

type Descriptor = Option<CompleteOrdinaryPropertyDescriptor>;

pub(crate) enum ProxyOwnStep {
    Complete(NativeConversion<Descriptor>),
    Read {
        object: ObjectRef,
        key: PropertyKey,
        receiver: Value,
        resume: ProxyOwnResume,
    },
    Call {
        target: DirectCallTarget,
        receiver: Value,
        arguments: Vec<Value>,
        resume: ProxyOwnResume,
    },
    Descriptor {
        object: ObjectRef,
        key: PropertyKey,
        resume: ProxyOwnResume,
    },
    Extensible {
        object: ObjectRef,
        resume: ProxyOwnResume,
    },
    Convert {
        value: Value,
        resume: ProxyOwnResume,
    },
}

pub(crate) struct ProxyOwnResume {
    realm: ContextId,
    phase: Phase,
}

enum Phase {
    Method {
        resume: MethodResume,
        key: PropertyKey,
    },
    Forward {
        _rooted: RootedProxy,
    },
    Trap {
        rooted: RootedProxy,
        key: PropertyKey,
    },
    Target {
        rooted: RootedProxy,
        result: Value,
    },
    Extensible {
        rooted: RootedProxy,
        result: Value,
        target: Descriptor,
    },
    Converted {
        _rooted: RootedProxy,
        target: Descriptor,
        extensible: bool,
    },
}

impl ProxyOwnStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        object: ObjectRef,
        key: PropertyKey,
    ) -> Result<Self, RuntimeError> {
        runtime.validate_object_and_key(&object, &key)?;
        let step = MethodStep::start(runtime, realm, object, "getOwnPropertyDescriptor")?;
        method(runtime, realm, key, step)
    }
}

fn method(
    runtime: &Runtime,
    realm: ContextId,
    key: PropertyKey,
    step: MethodStep,
) -> Result<ProxyOwnStep, RuntimeError> {
    Ok(match step {
        MethodStep::Complete(NativeConversion::Throw(value)) => {
            ProxyOwnStep::Complete(NativeConversion::Throw(value))
        }
        MethodStep::Complete(NativeConversion::Value((rooted, None))) => ProxyOwnStep::Descriptor {
            object: rooted.target.clone(),
            key,
            resume: ProxyOwnResume {
                realm,
                phase: Phase::Forward { _rooted: rooted },
            },
        },
        MethodStep::Complete(NativeConversion::Value((rooted, Some(target)))) => {
            let key_value = runtime.property_key_value(&key)?;
            ProxyOwnStep::Call {
                target,
                receiver: Value::Object(rooted.handler.clone()),
                arguments: vec![Value::Object(rooted.target.clone()), key_value],
                resume: ProxyOwnResume {
                    realm,
                    phase: Phase::Trap { rooted, key },
                },
            }
        }
        MethodStep::Read {
            object,
            key: method_key,
            receiver,
            resume,
        } => ProxyOwnStep::Read {
            object,
            key: method_key,
            receiver,
            resume: ProxyOwnResume {
                realm,
                phase: Phase::Method { resume, key },
            },
        },
    })
}

impl ProxyOwnResume {
    pub(crate) fn resume(
        self,
        runtime: &Runtime,
        completion: Completion,
    ) -> Result<ProxyOwnStep, RuntimeError> {
        let value = match completion {
            Completion::Throw(value) => {
                return Ok(ProxyOwnStep::Complete(NativeConversion::Throw(value)));
            }
            Completion::Return(value) => value,
        };
        match self.phase {
            Phase::Method { resume, key } => method(
                runtime,
                self.realm,
                key,
                resume.resume(runtime, Completion::Return(value))?,
            ),
            Phase::Trap { rooted, key } => {
                if !matches!(value, Value::Undefined | Value::Object(_)) {
                    return Ok(ProxyOwnStep::Complete(
                        runtime.proxy_invariant_throw(self.realm, "getOwnPropertyDescriptor")?,
                    ));
                }
                Ok(ProxyOwnStep::Descriptor {
                    object: rooted.target.clone(),
                    key,
                    resume: Self {
                        realm: self.realm,
                        phase: Phase::Target {
                            rooted,
                            result: value,
                        },
                    },
                })
            }
            _ => Err(RuntimeError::Invariant(
                "Proxy descriptor continuation received a value reply",
            )),
        }
    }

    pub(crate) fn descriptor(
        self,
        runtime: &Runtime,
        descriptor: NativeConversion<Descriptor>,
    ) -> Result<ProxyOwnStep, RuntimeError> {
        let target = match descriptor {
            NativeConversion::Value(target) => target,
            NativeConversion::Throw(value) => {
                return Ok(ProxyOwnStep::Complete(NativeConversion::Throw(value)));
            }
        };
        match self.phase {
            Phase::Forward { .. } => Ok(ProxyOwnStep::Complete(NativeConversion::Value(target))),
            Phase::Target { rooted, result } => {
                if matches!(result, Value::Undefined) {
                    if let Some(target) = target
                        && (!target.configurable()
                            || !runtime.raw_extensible_bit(&rooted.target)?)
                    {
                        return Ok(ProxyOwnStep::Complete(
                            runtime
                                .proxy_invariant_throw(self.realm, "getOwnPropertyDescriptor")?,
                        ));
                    }
                    return Ok(ProxyOwnStep::Complete(NativeConversion::Value(None)));
                }
                // QuickJS queries target extensibility before reading any
                // fields from the descriptor returned by the trap.
                Ok(ProxyOwnStep::Extensible {
                    object: rooted.target.clone(),
                    resume: Self {
                        realm: self.realm,
                        phase: Phase::Extensible {
                            rooted,
                            result,
                            target,
                        },
                    },
                })
            }
            _ => Err(RuntimeError::Invariant(
                "Proxy value continuation received a descriptor reply",
            )),
        }
    }

    pub(crate) fn extensible(
        self,
        result: NativeConversion<bool>,
    ) -> Result<ProxyOwnStep, RuntimeError> {
        let Phase::Extensible {
            rooted,
            result: value,
            target,
        } = self.phase
        else {
            return Err(RuntimeError::Invariant(
                "Proxy descriptor continuation received an extensibility reply",
            ));
        };
        match result {
            NativeConversion::Throw(value) => {
                Ok(ProxyOwnStep::Complete(NativeConversion::Throw(value)))
            }
            NativeConversion::Value(extensible) => Ok(ProxyOwnStep::Convert {
                value,
                resume: Self {
                    realm: self.realm,
                    phase: Phase::Converted {
                        _rooted: rooted,
                        target,
                        extensible,
                    },
                },
            }),
        }
    }

    pub(crate) fn converted(
        self,
        runtime: &Runtime,
        result: NativeConversion<OrdinaryPropertyDescriptor>,
    ) -> Result<ProxyOwnStep, RuntimeError> {
        let Phase::Converted {
            _rooted,
            target,
            extensible,
        } = self.phase
        else {
            return Err(RuntimeError::Invariant(
                "Proxy descriptor continuation received a conversion reply",
            ));
        };
        let result = match result {
            NativeConversion::Throw(value) => {
                return Ok(ProxyOwnStep::Complete(NativeConversion::Throw(value)));
            }
            NativeConversion::Value(result) => result,
        };
        let result = descriptor_to_validation_record(&result);
        let complete = validate_and_apply_property_descriptor(
            true,
            &result,
            None,
            &Value::Undefined,
            Value::same_value,
        )
        .map_err(|_| {
            RuntimeError::Invariant("validated Proxy descriptor could not be completed")
        })?;
        let result = validation_record_to_complete(complete)?;
        if !proxy_gopd_descriptor_is_compatible(target.as_ref(), &result, extensible) {
            return Ok(ProxyOwnStep::Complete(
                runtime.proxy_invariant_throw(self.realm, "getOwnPropertyDescriptor")?,
            ));
        }
        Ok(ProxyOwnStep::Complete(NativeConversion::Value(Some(
            result,
        ))))
    }
}
