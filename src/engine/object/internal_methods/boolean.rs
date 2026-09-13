//! Completion-aware Proxy boolean internal methods and their distinct invariants.
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
    Delete(PropertyKey),
    Extensible,
    PreventExtensions,
}
pub(crate) enum ProxyBooleanStep {
    Delete {
        object: ObjectRef,
        key: PropertyKey,
        resume: ProxyBooleanResume,
    },
    PreventExtensions {
        object: ObjectRef,
        resume: ProxyBooleanResume,
    },
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
        _key: Option<PropertyKey>,
    },
    Trap {
        rooted: RootedProxy,
        kind: ProxyBooleanKind,
    },
    DeleteInvariant {
        rooted: RootedProxy,
        key: PropertyKey,
    },
    RequiredExtensibility {
        _rooted: RootedProxy,
        _key: Option<PropertyKey>,
        name: &'static str,
        expected: bool,
    },
    HasInvariant {
        rooted: RootedProxy,
        key: PropertyKey,
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
            ProxyBooleanKind::Delete(key) => {
                runtime.validate_object_and_key(&object, key)?;
                "deleteProperty"
            }
            ProxyBooleanKind::Extensible => "isExtensible",
            ProxyBooleanKind::PreventExtensions => "preventExtensions",
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
                phase: Phase::Forward {
                    _rooted: rooted,
                    _key: match &kind {
                        ProxyBooleanKind::Has(key) | ProxyBooleanKind::Delete(key) => {
                            Some(key.clone())
                        }
                        _ => None,
                    },
                },
            };
            match kind {
                ProxyBooleanKind::Has(key) => ProxyBooleanStep::Has {
                    object,
                    key,
                    resume,
                },
                ProxyBooleanKind::Extensible => ProxyBooleanStep::Extensible { object, resume },
                ProxyBooleanKind::Delete(key) => ProxyBooleanStep::Delete {
                    object,
                    key,
                    resume,
                },
                ProxyBooleanKind::PreventExtensions => {
                    ProxyBooleanStep::PreventExtensions { object, resume }
                }
            }
        }
        MethodStep::Complete(NativeConversion::Value((rooted, Some(target)))) => {
            let mut arguments = vec![Value::Object(rooted.target.clone())];
            if let ProxyBooleanKind::Has(key) | ProxyBooleanKind::Delete(key) = &kind {
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
                        key: key.clone(),
                        resume: Self {
                            realm: self.realm,
                            phase: Phase::HasInvariant { rooted, key },
                        },
                    }),
                    ProxyBooleanKind::Delete(_) | ProxyBooleanKind::PreventExtensions
                        if !result =>
                    {
                        Ok(ProxyBooleanStep::Complete(NativeConversion::Value(false)))
                    }
                    ProxyBooleanKind::Delete(key) => Ok(ProxyBooleanStep::Descriptor {
                        object: rooted.target.clone(),
                        key: key.clone(),
                        resume: Self {
                            realm: self.realm,
                            phase: Phase::DeleteInvariant { rooted, key },
                        },
                    }),
                    ProxyBooleanKind::PreventExtensions => Ok(ProxyBooleanStep::Extensible {
                        object: rooted.target.clone(),
                        resume: Self {
                            realm: self.realm,
                            phase: Phase::RequiredExtensibility {
                                _rooted: rooted,
                                name: "preventExtensions",
                                expected: false,
                                _key: None,
                            },
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
            Phase::RequiredExtensibility {
                _rooted,
                name,
                expected,
                _key,
            } => Ok(ProxyBooleanStep::Complete(if value != expected {
                runtime.proxy_invariant_throw(self.realm, name)?
            } else {
                NativeConversion::Value(true)
            })),
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
        let (rooted, key, deleting) = match self.phase {
            Phase::HasInvariant { rooted, key } => (rooted, key, false),
            Phase::DeleteInvariant { rooted, key } => (rooted, key, true),
            _ => {
                return Err(RuntimeError::Invariant(
                    "Proxy boolean continuation received a descriptor reply",
                ));
            }
        };
        let descriptor = match descriptor {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => {
                return Ok(ProxyBooleanStep::Complete(NativeConversion::Throw(value)));
            }
        };
        if deleting {
            let Some(target) = descriptor else {
                return Ok(ProxyBooleanStep::Complete(NativeConversion::Value(true)));
            };
            if !target.configurable() {
                return Ok(ProxyBooleanStep::Complete(
                    runtime.proxy_invariant_throw(self.realm, "deleteProperty")?,
                ));
            }
            // Delete consults nested [[IsExtensible]], unlike Has's pinned raw bit.
            return Ok(ProxyBooleanStep::Extensible {
                object: rooted.target.clone(),
                resume: Self {
                    realm: self.realm,
                    phase: Phase::RequiredExtensibility {
                        _rooted: rooted,
                        name: "deleteProperty",
                        expected: true,
                        _key: Some(key),
                    },
                },
            });
        }
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
            ProxyBooleanStep::Delete {
                object,
                key,
                resume,
            } => resume.boolean(
                runtime,
                runtime.internal_delete_property(realm, &object, &key)?,
            )?,
            ProxyBooleanStep::PreventExtensions { object, resume } => resume.boolean(
                runtime,
                runtime.internal_prevent_extensions(realm, &object)?,
            )?,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn take_read(step: ProxyBooleanStep) -> ProxyBooleanResume {
        let ProxyBooleanStep::Read { resume, .. } = step else {
            panic!("expected method read")
        };
        resume
    }
    fn take_call(step: ProxyBooleanStep) -> ProxyBooleanResume {
        let ProxyBooleanStep::Call { resume, .. } = step else {
            panic!("expected trap call")
        };
        resume
    }
    fn take_descriptor(step: ProxyBooleanStep) -> ProxyBooleanResume {
        let ProxyBooleanStep::Descriptor { resume, .. } = step else {
            panic!("expected descriptor query")
        };
        resume
    }
    fn take_extensible(step: ProxyBooleanStep) -> ProxyBooleanResume {
        let ProxyBooleanStep::Extensible { resume, .. } = step else {
            panic!("expected extensibility query")
        };
        resume
    }

    #[test]
    fn delete_keeps_symbol_key_across_both_invariant_queries_and_abandonment() {
        for after_descriptor in [false, true] {
            let runtime = Runtime::new();
            let weak = std::rc::Rc::downgrade(&runtime.0);
            let mut context = runtime.new_context();
            let Value::Object(proxy) = context.eval("new Proxy({}, {})").unwrap() else {
                panic!("expected Proxy")
            };
            let callable = context.eval("(function(){return true})").unwrap();
            let symbol = runtime.new_symbol(None).unwrap();
            let key = PropertyKey::from(symbol);
            let atom = key.atom();
            let resume = take_read(
                ProxyBooleanStep::start(
                    &runtime,
                    context.realm,
                    proxy,
                    ProxyBooleanKind::Delete(key),
                )
                .unwrap(),
            );
            let resume = take_call(
                resume
                    .resume(&runtime, Completion::Return(callable))
                    .unwrap(),
            );
            let mut resume = take_descriptor(
                resume
                    .resume(&runtime, Completion::Return(Value::Bool(true)))
                    .unwrap(),
            );
            if after_descriptor {
                resume = take_extensible(
                    resume
                        .descriptor(
                            &runtime,
                            NativeConversion::Value(Some(
                                CompleteOrdinaryPropertyDescriptor::Data {
                                    value: Value::Int(1),
                                    writable: true,
                                    enumerable: true,
                                    configurable: true,
                                },
                            )),
                        )
                        .unwrap(),
                );
            }
            runtime.run_gc().unwrap();
            assert!(runtime.0.state.borrow().atoms.is_live(atom));
            drop(resume);
            runtime.run_gc().unwrap();
            assert!(!runtime.0.state.borrow().atoms.is_live(atom));
            drop(context);
            drop(runtime);
            assert!(weak.upgrade().is_none());
        }
    }
}
