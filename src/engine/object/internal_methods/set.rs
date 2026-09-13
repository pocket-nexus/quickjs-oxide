//! Proxy [[Set]] stages, including the target descriptor invariant.
use super::{
    RootedProxy,
    method::{MethodResume, MethodStep},
};
use crate::engine::api::{runtime::Runtime, runtime_error::RuntimeError};
use crate::engine::heap::ContextId;
use crate::engine::object::operations::InternalSetResult;
use crate::engine::object::{CompleteOrdinaryPropertyDescriptor, ObjectRef, PropertyKey};
use crate::engine::value::{Value, conversion::NativeConversion};
use crate::engine::vm::{Completion, call::DirectCallTarget};

pub(crate) enum ProxySetStep {
    Complete(NativeConversion<InternalSetResult>),
    Read {
        object: ObjectRef,
        key: PropertyKey,
        receiver: Value,
        resume: ProxySetResume,
    },
    Call {
        target: DirectCallTarget,
        receiver: Value,
        arguments: Vec<Value>,
        resume: ProxySetResume,
    },
    Set {
        object: ObjectRef,
        key: PropertyKey,
        value: Value,
        receiver: Value,
        resume: ProxySetResume,
    },
    Descriptor {
        object: ObjectRef,
        key: PropertyKey,
        resume: ProxySetResume,
    },
}
pub(crate) struct ProxySetResume {
    realm: ContextId,
    phase: Phase,
}
enum Phase {
    Method {
        resume: MethodResume,
        key: PropertyKey,
        value: Value,
        receiver: Value,
    },
    Forward {
        _rooted: RootedProxy,
    },
    Trap {
        rooted: RootedProxy,
        key: PropertyKey,
        value: Value,
    },
    Invariant {
        _rooted: RootedProxy,
        value: Value,
    },
}
impl ProxySetStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        object: ObjectRef,
        key: PropertyKey,
        value: Value,
        receiver: Value,
    ) -> Result<Self, RuntimeError> {
        runtime.validate_object_and_key(&object, &key)?;
        runtime.validate_value_domain(&value, "property value")?;
        runtime.validate_value_domain(&receiver, "property receiver")?;
        let step = MethodStep::start(runtime, realm, object, "set")?;
        method(runtime, realm, key, value, receiver, step)
    }
}
fn method(
    runtime: &Runtime,
    realm: ContextId,
    key: PropertyKey,
    value: Value,
    receiver: Value,
    step: MethodStep,
) -> Result<ProxySetStep, RuntimeError> {
    Ok(match step {
        MethodStep::Complete(NativeConversion::Throw(value)) => {
            ProxySetStep::Complete(NativeConversion::Throw(value))
        }
        MethodStep::Read {
            object,
            key: method_key,
            receiver: method_receiver,
            resume,
        } => ProxySetStep::Read {
            object,
            key: method_key,
            receiver: method_receiver,
            resume: ProxySetResume {
                realm,
                phase: Phase::Method {
                    resume,
                    key,
                    value,
                    receiver,
                },
            },
        },
        MethodStep::Complete(NativeConversion::Value((rooted, None))) => ProxySetStep::Set {
            object: rooted.target.clone(),
            key,
            value,
            receiver,
            resume: ProxySetResume {
                realm,
                phase: Phase::Forward { _rooted: rooted },
            },
        },
        MethodStep::Complete(NativeConversion::Value((rooted, Some(target)))) => {
            let key_value = runtime.property_key_value(&key)?;
            ProxySetStep::Call {
                target,
                receiver: Value::Object(rooted.handler.clone()),
                arguments: vec![
                    Value::Object(rooted.target.clone()),
                    key_value,
                    value.clone(),
                    receiver,
                ],
                resume: ProxySetResume {
                    realm,
                    phase: Phase::Trap { rooted, key, value },
                },
            }
        }
    })
}
impl ProxySetResume {
    pub(crate) fn resume(
        self,
        runtime: &Runtime,
        completion: Completion,
    ) -> Result<ProxySetStep, RuntimeError> {
        let result = match completion {
            Completion::Throw(value) => {
                return Ok(ProxySetStep::Complete(NativeConversion::Throw(value)));
            }
            Completion::Return(value) => value,
        };
        match self.phase {
            Phase::Method {
                resume,
                key,
                value,
                receiver,
            } => method(
                runtime,
                self.realm,
                key,
                value,
                receiver,
                resume.resume(runtime, Completion::Return(result))?,
            ),
            Phase::Trap { rooted, key, value } => {
                if !runtime.value_to_boolean(&result)? {
                    return Ok(ProxySetStep::Complete(NativeConversion::Value(
                        InternalSetResult::RejectedProxyTrap,
                    )));
                }
                Ok(ProxySetStep::Descriptor {
                    object: rooted.target.clone(),
                    key,
                    resume: Self {
                        realm: self.realm,
                        phase: Phase::Invariant {
                            _rooted: rooted,
                            value,
                        },
                    },
                })
            }
            _ => Err(RuntimeError::Invariant(
                "Proxy Set continuation received a value reply",
            )),
        }
    }
    pub(crate) fn set(
        self,
        result: NativeConversion<InternalSetResult>,
    ) -> Result<ProxySetStep, RuntimeError> {
        if !matches!(self.phase, Phase::Forward { .. }) {
            return Err(RuntimeError::Invariant(
                "Proxy Set continuation received a Set reply",
            ));
        }
        Ok(ProxySetStep::Complete(result))
    }
    pub(crate) fn descriptor(
        self,
        runtime: &Runtime,
        result: NativeConversion<Option<CompleteOrdinaryPropertyDescriptor>>,
    ) -> Result<ProxySetStep, RuntimeError> {
        let Phase::Invariant { _rooted, value } = self.phase else {
            return Err(RuntimeError::Invariant(
                "Proxy Set continuation received a descriptor reply",
            ));
        };
        let target = match result {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => {
                return Ok(ProxySetStep::Complete(NativeConversion::Throw(value)));
            }
        };
        let invalid = match target {
            Some(CompleteOrdinaryPropertyDescriptor::Data {
                value: target_value,
                writable: false,
                configurable: false,
                ..
            }) => !value.same_value(&target_value),
            Some(CompleteOrdinaryPropertyDescriptor::Accessor {
                set: None,
                configurable: false,
                ..
            }) => true,
            _ => false,
        };
        Ok(ProxySetStep::Complete(if invalid {
            runtime.proxy_invariant_throw(self.realm, "set")?
        } else {
            NativeConversion::Value(InternalSetResult::Accepted)
        }))
    }
}
