//! Proxy prototype requests share their ordered extensibility/identity checks.
use super::{
    RootedProxy,
    method::{MethodResume, MethodStep},
};
use crate::engine::api::{runtime::Runtime, runtime_error::RuntimeError};
use crate::engine::heap::ContextId;
use crate::engine::object::{ObjectRef, PropertyKey};
use crate::engine::value::{Value, conversion::NativeConversion};
use crate::engine::vm::{Completion, call::DirectCallTarget};

pub(crate) enum ProxyPrototypeKind {
    Get,
    Set(Option<ObjectRef>),
}
pub(crate) enum ProxyPrototypeStep {
    Complete(Completion),
    Read {
        object: ObjectRef,
        key: PropertyKey,
        receiver: Value,
        resume: ProxyPrototypeResume,
    },
    Call {
        target: DirectCallTarget,
        receiver: Value,
        arguments: Vec<Value>,
        resume: ProxyPrototypeResume,
    },
    Get {
        object: ObjectRef,
        resume: ProxyPrototypeResume,
    },
    Set {
        object: ObjectRef,
        prototype: Option<ObjectRef>,
        resume: ProxyPrototypeResume,
    },
    Extensible {
        object: ObjectRef,
        resume: ProxyPrototypeResume,
    },
}
pub(crate) struct ProxyPrototypeResume {
    realm: ContextId,
    phase: Phase,
}
enum Phase {
    Method {
        resume: MethodResume,
        kind: ProxyPrototypeKind,
    },
    Forward {
        _rooted: RootedProxy,
        kind: ProxyPrototypeKind,
    },
    Trap {
        rooted: RootedProxy,
        kind: ProxyPrototypeKind,
    },
    Extensible {
        rooted: RootedProxy,
        prototype: Option<ObjectRef>,
        setting: bool,
    },
    Compare {
        _rooted: RootedProxy,
        prototype: Option<ObjectRef>,
        setting: bool,
    },
}
impl ProxyPrototypeStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        object: ObjectRef,
        kind: ProxyPrototypeKind,
    ) -> Result<Self, RuntimeError> {
        let name = match &kind {
            ProxyPrototypeKind::Get => "getPrototypeOf",
            ProxyPrototypeKind::Set(prototype) => {
                if prototype
                    .as_ref()
                    .is_some_and(|object| !object.belongs_to(runtime))
                {
                    return Err(RuntimeError::WrongRuntime("Proxy prototype"));
                }
                "setPrototypeOf"
            }
        };
        method(
            runtime,
            realm,
            kind,
            MethodStep::start(runtime, realm, object, name)?,
        )
    }
}
fn method(
    _runtime: &Runtime,
    realm: ContextId,
    kind: ProxyPrototypeKind,
    step: MethodStep,
) -> Result<ProxyPrototypeStep, RuntimeError> {
    Ok(match step {
        MethodStep::Complete(NativeConversion::Throw(value)) => {
            ProxyPrototypeStep::Complete(Completion::Throw(value))
        }
        MethodStep::Read {
            object,
            key,
            receiver,
            resume,
        } => ProxyPrototypeStep::Read {
            object,
            key,
            receiver,
            resume: ProxyPrototypeResume {
                realm,
                phase: Phase::Method { resume, kind },
            },
        },
        MethodStep::Complete(NativeConversion::Value((rooted, None))) => {
            let object = rooted.target.clone();
            let prototype = match &kind {
                ProxyPrototypeKind::Set(prototype) => Some(prototype.clone()),
                _ => None,
            };
            let resume = ProxyPrototypeResume {
                realm,
                phase: Phase::Forward {
                    _rooted: rooted,
                    kind,
                },
            };
            match prototype {
                Some(prototype) => ProxyPrototypeStep::Set {
                    object,
                    prototype,
                    resume,
                },
                None => ProxyPrototypeStep::Get { object, resume },
            }
        }
        MethodStep::Complete(NativeConversion::Value((rooted, Some(target)))) => {
            let mut arguments = vec![Value::Object(rooted.target.clone())];
            if let ProxyPrototypeKind::Set(prototype) = &kind {
                arguments.push(prototype.clone().map_or(Value::Null, Value::Object));
            }
            ProxyPrototypeStep::Call {
                target,
                receiver: Value::Object(rooted.handler.clone()),
                arguments,
                resume: ProxyPrototypeResume {
                    realm,
                    phase: Phase::Trap { rooted, kind },
                },
            }
        }
    })
}
fn completed(prototype: Option<ObjectRef>, setting: bool) -> ProxyPrototypeStep {
    ProxyPrototypeStep::Complete(Completion::Return(if setting {
        Value::Bool(true)
    } else {
        prototype.map_or(Value::Null, Value::Object)
    }))
}
fn inconsistent(runtime: &Runtime, realm: ContextId) -> Result<ProxyPrototypeStep, RuntimeError> {
    Ok(ProxyPrototypeStep::Complete(
        match runtime.proxy_invariant_throw::<Value>(realm, "prototype")? {
            NativeConversion::Throw(value) => Completion::Throw(value),
            NativeConversion::Value(_) => {
                return Err(RuntimeError::Invariant(
                    "Proxy invariant rejection returned a value",
                ));
            }
        },
    ))
}
impl ProxyPrototypeResume {
    pub(crate) fn resume(
        self,
        runtime: &Runtime,
        completion: Completion,
    ) -> Result<ProxyPrototypeStep, RuntimeError> {
        let value = match completion {
            Completion::Return(value) => value,
            Completion::Throw(value) => {
                return Ok(ProxyPrototypeStep::Complete(Completion::Throw(value)));
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
                let (prototype, setting) = match kind {
                    ProxyPrototypeKind::Get => (
                        match value {
                            Value::Object(object) => Some(object),
                            Value::Null => None,
                            _ => return inconsistent(runtime, self.realm),
                        },
                        false,
                    ),
                    ProxyPrototypeKind::Set(prototype) => {
                        if !runtime.value_to_boolean(&value)? {
                            return Ok(ProxyPrototypeStep::Complete(Completion::Return(
                                Value::Bool(false),
                            )));
                        }
                        (prototype, true)
                    }
                };
                Ok(ProxyPrototypeStep::Extensible {
                    object: rooted.target.clone(),
                    resume: Self {
                        realm: self.realm,
                        phase: Phase::Extensible {
                            rooted,
                            prototype,
                            setting,
                        },
                    },
                })
            }
            _ => Err(RuntimeError::Invariant(
                "Proxy prototype continuation received a value reply",
            )),
        }
    }
    pub(crate) fn boolean(
        self,
        _runtime: &Runtime,
        result: NativeConversion<bool>,
    ) -> Result<ProxyPrototypeStep, RuntimeError> {
        let value = match result {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => {
                return Ok(ProxyPrototypeStep::Complete(Completion::Throw(value)));
            }
        };
        match self.phase {
            Phase::Forward {
                kind: ProxyPrototypeKind::Set(_),
                ..
            } => Ok(ProxyPrototypeStep::Complete(Completion::Return(
                Value::Bool(value),
            ))),
            Phase::Extensible {
                rooted,
                prototype,
                setting,
            } => {
                if value {
                    return Ok(completed(prototype, setting));
                }
                Ok(ProxyPrototypeStep::Get {
                    object: rooted.target.clone(),
                    resume: Self {
                        realm: self.realm,
                        phase: Phase::Compare {
                            _rooted: rooted,
                            prototype,
                            setting,
                        },
                    },
                })
            }
            _ => Err(RuntimeError::Invariant(
                "Proxy prototype continuation received a boolean reply",
            )),
        }
    }
    pub(crate) fn prototype(
        self,
        runtime: &Runtime,
        result: NativeConversion<Option<ObjectRef>>,
    ) -> Result<ProxyPrototypeStep, RuntimeError> {
        let value = match result {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => {
                return Ok(ProxyPrototypeStep::Complete(Completion::Throw(value)));
            }
        };
        match self.phase {
            Phase::Forward {
                kind: ProxyPrototypeKind::Get,
                ..
            } => Ok(completed(value, false)),
            Phase::Compare {
                _rooted,
                prototype,
                setting,
            } => {
                if value != prototype {
                    return inconsistent(runtime, self.realm);
                }
                Ok(completed(prototype, setting))
            }
            _ => Err(RuntimeError::Invariant(
                "Proxy prototype continuation received a prototype reply",
            )),
        }
    }
}
pub(super) fn finish(
    runtime: &Runtime,
    realm: ContextId,
    mut step: ProxyPrototypeStep,
) -> Result<Completion, RuntimeError> {
    loop {
        step = match step {
            ProxyPrototypeStep::Complete(result) => return Ok(result),
            ProxyPrototypeStep::Read {
                object,
                key,
                receiver,
                resume,
            } => resume.resume(
                runtime,
                runtime.internal_get(realm, &object, &key, receiver)?,
            )?,
            ProxyPrototypeStep::Call {
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
            ProxyPrototypeStep::Get { object, resume } => {
                resume.prototype(runtime, runtime.internal_get_prototype_of(realm, &object)?)?
            }
            ProxyPrototypeStep::Set {
                object,
                prototype,
                resume,
            } => resume.boolean(
                runtime,
                runtime.internal_set_prototype_of(realm, &object, prototype.as_ref())?,
            )?,
            ProxyPrototypeStep::Extensible { object, resume } => {
                resume.boolean(runtime, runtime.internal_is_extensible(realm, &object)?)?
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn take_read(step: ProxyPrototypeStep) -> ProxyPrototypeResume {
        let ProxyPrototypeStep::Read { resume, .. } = step else {
            panic!("expected read")
        };
        resume
    }
    fn take_call(step: ProxyPrototypeStep) -> ProxyPrototypeResume {
        let ProxyPrototypeStep::Call { resume, .. } = step else {
            panic!("expected call")
        };
        resume
    }
    fn take_extensible(step: ProxyPrototypeStep) -> ProxyPrototypeResume {
        let ProxyPrototypeStep::Extensible { resume, .. } = step else {
            panic!("expected extensibility")
        };
        resume
    }
    fn take_get(step: ProxyPrototypeStep) -> ProxyPrototypeResume {
        let ProxyPrototypeStep::Get { resume, .. } = step else {
            panic!("expected prototype")
        };
        resume
    }
    #[test]
    fn prototypes_are_owned_across_requests_and_released_on_abandonment() {
        for setting in [false, true] {
            for compare in [false, true] {
                let runtime = Runtime::new();
                let weak = std::rc::Rc::downgrade(&runtime.0);
                let mut context = runtime.new_context();
                let Value::Object(proxy) = context.eval("new Proxy({}, {})").unwrap() else {
                    panic!("expected Proxy")
                };
                let rooted = runtime.proxy_snapshot_if_any(&proxy).unwrap().unwrap();
                let ids = [proxy.object_id(), rooted.target, rooted.handler];
                let prototype = runtime.new_object(None).unwrap();
                let prototype_id = prototype.object_id();
                let callable = context.eval("(function(){return true})").unwrap();
                let kind = if setting {
                    ProxyPrototypeKind::Set(Some(prototype.clone()))
                } else {
                    ProxyPrototypeKind::Get
                };
                let resume = take_read(
                    ProxyPrototypeStep::start(&runtime, context.realm, proxy, kind).unwrap(),
                );
                let resume = take_call(
                    resume
                        .resume(&runtime, Completion::Return(callable))
                        .unwrap(),
                );
                let reply = if setting {
                    drop(prototype);
                    Value::Bool(true)
                } else {
                    Value::Object(prototype)
                };
                let mut resume =
                    take_extensible(resume.resume(&runtime, Completion::Return(reply)).unwrap());
                if compare {
                    resume = take_get(
                        resume
                            .boolean(&runtime, NativeConversion::Value(false))
                            .unwrap(),
                    );
                }
                runtime.run_gc().unwrap();
                for id in ids.into_iter().chain([prototype_id]) {
                    assert!(runtime.0.state.borrow().heap.object(id).is_ok());
                }
                drop(resume);
                runtime.run_gc().unwrap();
                for id in ids.into_iter().chain([prototype_id]) {
                    assert!(runtime.0.state.borrow().heap.object(id).is_err());
                }
                assert_eq!(runtime.0.proxy_method_depth.get(), 0);
                drop(context);
                drop(runtime);
                assert!(weak.upgrade().is_none());
            }
        }
    }
}
