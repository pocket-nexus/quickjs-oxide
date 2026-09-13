//! Proxy [[DefineOwnProperty]] shares its observable trap/invariant stages.
use super::{
    RootedProxy,
    method::{MethodResume, MethodStep},
    proxy_define_descriptor_is_compatible,
};
use crate::engine::api::{runtime::Runtime, runtime_error::RuntimeError};
use crate::engine::heap::ContextId;
use crate::engine::object::operations::InternalDefineResult;
use crate::engine::object::{
    CompleteOrdinaryPropertyDescriptor, DescriptorField, ObjectRef, OrdinaryPropertyDescriptor,
    PropertyKey,
};
use crate::engine::value::{Value, conversion::NativeConversion};
use crate::engine::vm::{Completion, call::DirectCallTarget};

pub(crate) enum ProxyDefineStep {
    Complete(NativeConversion<InternalDefineResult>),
    Read {
        object: ObjectRef,
        key: PropertyKey,
        receiver: Value,
        resume: ProxyDefineResume,
    },
    Call {
        target: DirectCallTarget,
        receiver: Value,
        arguments: Vec<Value>,
        resume: ProxyDefineResume,
    },
    Define {
        object: ObjectRef,
        key: PropertyKey,
        descriptor: OrdinaryPropertyDescriptor,
        resume: ProxyDefineResume,
    },
    Descriptor {
        object: ObjectRef,
        key: PropertyKey,
        resume: ProxyDefineResume,
    },
}
pub(crate) struct ProxyDefineResume {
    realm: ContextId,
    phase: Phase,
}
enum Phase {
    Method {
        resume: MethodResume,
        key: PropertyKey,
        descriptor: OrdinaryPropertyDescriptor,
    },
    Forward {
        _rooted: RootedProxy,
    },
    Trap {
        rooted: RootedProxy,
        key: PropertyKey,
        descriptor: OrdinaryPropertyDescriptor,
    },
    Invariant {
        rooted: RootedProxy,
        descriptor: OrdinaryPropertyDescriptor,
    },
}
impl ProxyDefineStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        object: ObjectRef,
        key: PropertyKey,
        descriptor: OrdinaryPropertyDescriptor,
    ) -> Result<Self, RuntimeError> {
        runtime.validate_object_and_key(&object, &key)?;
        runtime.validate_descriptor_domains(&descriptor)?;
        let step = MethodStep::start(runtime, realm, object, "defineProperty")?;
        method(runtime, realm, key, descriptor, step)
    }
}
fn method(
    runtime: &Runtime,
    realm: ContextId,
    key: PropertyKey,
    descriptor: OrdinaryPropertyDescriptor,
    step: MethodStep,
) -> Result<ProxyDefineStep, RuntimeError> {
    Ok(match step {
        MethodStep::Complete(NativeConversion::Throw(value)) => {
            ProxyDefineStep::Complete(NativeConversion::Throw(value))
        }
        MethodStep::Read {
            object,
            key: method_key,
            receiver,
            resume,
        } => ProxyDefineStep::Read {
            object,
            key: method_key,
            receiver,
            resume: ProxyDefineResume {
                realm,
                phase: Phase::Method {
                    resume,
                    key,
                    descriptor,
                },
            },
        },
        MethodStep::Complete(NativeConversion::Value((rooted, None))) => ProxyDefineStep::Define {
            object: rooted.target.clone(),
            key,
            descriptor,
            resume: ProxyDefineResume {
                realm,
                phase: Phase::Forward { _rooted: rooted },
            },
        },
        MethodStep::Complete(NativeConversion::Value((rooted, Some(target)))) => {
            let key_value = runtime.property_key_value(&key)?;
            let descriptor_object = runtime.proxy_descriptor_object(realm, &descriptor)?;
            ProxyDefineStep::Call {
                target,
                receiver: Value::Object(rooted.handler.clone()),
                arguments: vec![
                    Value::Object(rooted.target.clone()),
                    key_value,
                    Value::Object(descriptor_object),
                ],
                resume: ProxyDefineResume {
                    realm,
                    phase: Phase::Trap {
                        rooted,
                        key,
                        descriptor,
                    },
                },
            }
        }
    })
}
impl ProxyDefineResume {
    pub(crate) fn resume(
        self,
        runtime: &Runtime,
        completion: Completion,
    ) -> Result<ProxyDefineStep, RuntimeError> {
        let value = match completion {
            Completion::Throw(value) => {
                return Ok(ProxyDefineStep::Complete(NativeConversion::Throw(value)));
            }
            Completion::Return(value) => value,
        };
        match self.phase {
            Phase::Method {
                resume,
                key,
                descriptor,
            } => method(
                runtime,
                self.realm,
                key,
                descriptor,
                resume.resume(runtime, Completion::Return(value))?,
            ),
            Phase::Trap {
                rooted,
                key,
                descriptor,
            } => {
                if !runtime.value_to_boolean(&value)? {
                    return Ok(ProxyDefineStep::Complete(NativeConversion::Value(
                        InternalDefineResult::RejectedProxyTrap,
                    )));
                }
                Ok(ProxyDefineStep::Descriptor {
                    object: rooted.target.clone(),
                    key,
                    resume: Self {
                        realm: self.realm,
                        phase: Phase::Invariant { rooted, descriptor },
                    },
                })
            }
            _ => Err(RuntimeError::Invariant(
                "Proxy Define continuation received a value reply",
            )),
        }
    }
    pub(crate) fn defined(
        self,
        result: NativeConversion<InternalDefineResult>,
    ) -> Result<ProxyDefineStep, RuntimeError> {
        if !matches!(self.phase, Phase::Forward { .. }) {
            return Err(RuntimeError::Invariant(
                "Proxy Define continuation received a Define reply",
            ));
        }
        Ok(ProxyDefineStep::Complete(result))
    }
    pub(crate) fn descriptor(
        self,
        runtime: &Runtime,
        result: NativeConversion<Option<CompleteOrdinaryPropertyDescriptor>>,
    ) -> Result<ProxyDefineStep, RuntimeError> {
        let Phase::Invariant { rooted, descriptor } = self.phase else {
            return Err(RuntimeError::Invariant(
                "Proxy Define continuation received a descriptor reply",
            ));
        };
        let target = match result {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => {
                return Ok(ProxyDefineStep::Complete(NativeConversion::Throw(value)));
            }
        };
        let compatible = if let Some(target) = target.as_ref() {
            proxy_define_descriptor_is_compatible(target, &descriptor)
        } else {
            runtime.raw_extensible_bit(&rooted.target)?
                && !matches!(descriptor.configurable, DescriptorField::Present(false))
        };
        Ok(ProxyDefineStep::Complete(if compatible {
            NativeConversion::Value(InternalDefineResult::Defined)
        } else {
            runtime.proxy_invariant_throw(self.realm, "defineProperty")?
        }))
    }
}
