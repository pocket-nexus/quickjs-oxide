//! Completion-aware Proxy HasProperty and IsExtensible requests.
use super::{
    RootedProxy,
    method::{MethodResume, MethodStep},
};
use crate::engine::api::{runtime::Runtime, runtime_error::RuntimeError};
use crate::engine::heap::ContextId;
use crate::engine::object::{CompleteOrdinaryPropertyDescriptor, ObjectRef, PropertyKey};
use crate::engine::value::{Value, conversion::NativeConversion};
use crate::engine::vm::{Completion, call::DirectCallTarget};

pub(crate) enum ProxyBooleanKind {
    Has(PropertyKey),
    Extensible,
}
pub(crate) enum ProxyBooleanStep {
    Complete(NativeConversion<bool>),
    Read {
        object: ObjectRef,
        key: PropertyKey,
        receiver: Value,
        resume: ProxyBooleanResume,
    },
    Call {
        target: DirectCallTarget,
        receiver: Value,
        arguments: Vec<Value>,
        resume: ProxyBooleanResume,
    },
    Has {
        object: ObjectRef,
        key: PropertyKey,
        resume: ProxyBooleanResume,
    },
    Extensible {
        object: ObjectRef,
        resume: ProxyBooleanResume,
    },
    Descriptor {
        object: ObjectRef,
        key: PropertyKey,
        resume: ProxyBooleanResume,
    },
}
pub(crate) struct ProxyBooleanResume {
    realm: ContextId,
    phase: Phase,
}
enum Phase {
    Method {
        resume: MethodResume,
        kind: ProxyBooleanKind,
    },
    Forward {
        _rooted: RootedProxy,
    },
    Trap {
        rooted: RootedProxy,
        kind: ProxyBooleanKind,
    },
    HasInvariant {
        rooted: RootedProxy,
    },
    ExtensibleInvariant {
        _rooted: RootedProxy,
        result: bool,
    },
}

impl ProxyBooleanStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        object: ObjectRef,
        kind: ProxyBooleanKind,
    ) -> Result<Self, RuntimeError> {
        let name = match &kind {
            ProxyBooleanKind::Has(key) => {
                runtime.validate_object_and_key(&object, key)?;
                "has"
            }
            ProxyBooleanKind::Extensible => "isExtensible",
        };
        let step = MethodStep::start(runtime, realm, object, name)?;
        method(runtime, realm, kind, step)
    }
}

fn method(
    runtime: &Runtime,
    realm: ContextId,
    kind: ProxyBooleanKind,
    step: MethodStep,
) -> Result<ProxyBooleanStep, RuntimeError> {
    Ok(match step {
        MethodStep::Complete(NativeConversion::Throw(value)) => {
            ProxyBooleanStep::Complete(NativeConversion::Throw(value))
        }
        MethodStep::Read {
            object,
            key,
            receiver,
            resume,
        } => ProxyBooleanStep::Read {
            object,
            key,
            receiver,
            resume: ProxyBooleanResume {
                realm,
                phase: Phase::Method { resume, kind },
            },
        },
        MethodStep::Complete(NativeConversion::Value((rooted, None))) => {
            let object = rooted.target.clone();
            let resume = ProxyBooleanResume {
                realm,
                phase: Phase::Forward { _rooted: rooted },
            };
            match kind {
                ProxyBooleanKind::Has(key) => ProxyBooleanStep::Has {
                    object,
                    key,
                    resume,
                },
                ProxyBooleanKind::Extensible => ProxyBooleanStep::Extensible { object, resume },
            }
        }
        MethodStep::Complete(NativeConversion::Value((rooted, Some(target)))) => {
            let mut arguments = vec![Value::Object(rooted.target.clone())];
            if let ProxyBooleanKind::Has(key) = &kind {
                arguments.push(runtime.property_key_value(key)?);
            }
            ProxyBooleanStep::Call {
                target,
                receiver: Value::Object(rooted.handler.clone()),
                arguments,
                resume: ProxyBooleanResume {
                    realm,
                    phase: Phase::Trap { rooted, kind },
                },
            }
        }
    })
}

impl ProxyBooleanResume {
    pub(crate) fn resume(
        self,
        runtime: &Runtime,
        completion: Completion,
    ) -> Result<ProxyBooleanStep, RuntimeError> {
        let value = match completion {
            Completion::Return(value) => value,
            Completion::Throw(value) => {
                return Ok(ProxyBooleanStep::Complete(NativeConversion::Throw(value)));
            }
        };
        match self.phase {
            Phase::Method { resume, kind } => method(
                runtime,
                self.realm,
                kind,
                resume.resume(runtime, Completion::Return(value))?,
            ),
            Phase::Trap { rooted, kind } => {
                let result = runtime.value_to_boolean(&value)?;
                match kind {
                    ProxyBooleanKind::Has(_) if result => {
                        Ok(ProxyBooleanStep::Complete(NativeConversion::Value(true)))
                    }
                    ProxyBooleanKind::Has(key) => Ok(ProxyBooleanStep::Descriptor {
                        object: rooted.target.clone(),
                        key,
                        resume: Self {
                            realm: self.realm,
                            phase: Phase::HasInvariant { rooted },
                        },
                    }),
                    ProxyBooleanKind::Extensible => Ok(ProxyBooleanStep::Extensible {
                        object: rooted.target.clone(),
                        resume: Self {
                            realm: self.realm,
                            phase: Phase::ExtensibleInvariant {
                                _rooted: rooted,
                                result,
                            },
                        },
                    }),
                }
            }
            _ => Err(RuntimeError::Invariant(
                "Proxy boolean continuation received a value reply",
            )),
        }
    }

    pub(crate) fn boolean(
        self,
        runtime: &Runtime,
        result: NativeConversion<bool>,
    ) -> Result<ProxyBooleanStep, RuntimeError> {
        let value = match result {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => {
                return Ok(ProxyBooleanStep::Complete(NativeConversion::Throw(value)));
            }
        };
        match self.phase {
            Phase::Forward { .. } => Ok(ProxyBooleanStep::Complete(NativeConversion::Value(value))),
            Phase::ExtensibleInvariant { _rooted, result } => {
                if result != value {
                    return Ok(ProxyBooleanStep::Complete(
                        runtime.proxy_invariant_throw(self.realm, "isExtensible")?,
                    ));
                }
                Ok(ProxyBooleanStep::Complete(NativeConversion::Value(result)))
            }
            _ => Err(RuntimeError::Invariant(
                "Proxy value continuation received a boolean reply",
            )),
        }
    }

    pub(crate) fn descriptor(
        self,
        runtime: &Runtime,
        descriptor: NativeConversion<Option<CompleteOrdinaryPropertyDescriptor>>,
    ) -> Result<ProxyBooleanStep, RuntimeError> {
        let Phase::HasInvariant { rooted } = self.phase else {
            return Err(RuntimeError::Invariant(
                "Proxy boolean continuation received a descriptor reply",
            ));
        };
        let descriptor = match descriptor {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => {
                return Ok(ProxyBooleanStep::Complete(NativeConversion::Throw(value)));
            }
        };
        if let Some(target) = descriptor
            && (!target.configurable() || !runtime.raw_extensible_bit(&rooted.target)?)
        {
            return Ok(ProxyBooleanStep::Complete(
                runtime.proxy_invariant_throw(self.realm, "has")?,
            ));
        }
        Ok(ProxyBooleanStep::Complete(NativeConversion::Value(false)))
    }
}

pub(super) fn finish(
    runtime: &Runtime,
    realm: ContextId,
    mut step: ProxyBooleanStep,
) -> Result<NativeConversion<bool>, RuntimeError> {
    loop {
        step = match step {
            ProxyBooleanStep::Complete(result) => return Ok(result),
            ProxyBooleanStep::Read {
                object,
                key,
                receiver,
                resume,
            } => resume.resume(
                runtime,
                runtime.internal_get(realm, &object, &key, receiver)?,
            )?,
            ProxyBooleanStep::Call {
                target,
                receiver,
                arguments,
                resume,
            } => {
                let result = match target {
                    DirectCallTarget::Callable(callable) => {
                        runtime.call_internal(realm, &callable, receiver, &arguments)?
                    }
                    DirectCallTarget::NonCallableProxy(object) => {
                        runtime.call_proxy(realm, &object, receiver, &arguments)?
                    }
                };
                resume.resume(runtime, result)?
            }
            ProxyBooleanStep::Has {
                object,
                key,
                resume,
            } => resume.boolean(
                runtime,
                runtime.internal_has_property(realm, &object, &key)?,
            )?,
            ProxyBooleanStep::Extensible { object, resume } => {
                resume.boolean(runtime, runtime.internal_is_extensible(realm, &object)?)?
            }
            ProxyBooleanStep::Descriptor {
                object,
                key,
                resume,
            } => resume.descriptor(
                runtime,
                runtime.internal_get_own_property(realm, &object, &key)?,
            )?,
        };
    }
}
