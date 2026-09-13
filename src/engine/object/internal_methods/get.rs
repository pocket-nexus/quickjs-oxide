//! Proxy [[Get]] phases shared by synchronous entry and the owned VM driver.
//! Each continuation owns the target/handler selected before observable work.
use super::{
    RootedProxy,
    method::{MethodResume, MethodStep},
};
use crate::engine::api::{error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError};
use crate::engine::heap::ContextId;
use crate::engine::object::{CompleteOrdinaryPropertyDescriptor, ObjectRef, PropertyKey};
use crate::engine::value::{Value, conversion::NativeConversion};
use crate::engine::vm::{Completion, call::DirectCallTarget};

pub(crate) enum ProxyGetStep {
    Complete(Completion),
    Read {
        object: ObjectRef,
        key: PropertyKey,
        receiver: Value,
        resume: ProxyGetResume,
    },
    Call {
        target: DirectCallTarget,
        receiver: Value,
        arguments: Vec<Value>,
        resume: ProxyGetResume,
    },
    Descriptor {
        object: ObjectRef,
        key: PropertyKey,
        resume: ProxyGetResume,
    },
}

pub(crate) struct ProxyGetResume {
    realm: ContextId,
    phase: Phase,
}

enum Phase {
    Method {
        resume: MethodResume,
        key: PropertyKey,
        receiver: Value,
    },
    Forward {
        _rooted: RootedProxy,
    },
    Trap {
        rooted: RootedProxy,
        key: PropertyKey,
    },
    Invariant {
        _rooted: RootedProxy,
        result: Value,
    },
}

impl ProxyGetStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        proxy: ObjectRef,
        key: PropertyKey,
        receiver: Value,
    ) -> Result<Self, RuntimeError> {
        runtime.validate_object_and_key(&proxy, &key)?;
        runtime.validate_value_domain(&receiver, "property receiver")?;
        let step = MethodStep::start(runtime, realm, proxy, "get")?;
        method(runtime, realm, key, receiver, step)
    }
}

fn method(
    runtime: &Runtime,
    realm: ContextId,
    key: PropertyKey,
    receiver: Value,
    step: MethodStep,
) -> Result<ProxyGetStep, RuntimeError> {
    Ok(match step {
        MethodStep::Complete(NativeConversion::Throw(value)) => {
            ProxyGetStep::Complete(Completion::Throw(value))
        }
        MethodStep::Complete(NativeConversion::Value((rooted, None))) => ProxyGetStep::Read {
            object: rooted.target.clone(),
            key,
            receiver,
            resume: ProxyGetResume {
                realm,
                phase: Phase::Forward { _rooted: rooted },
            },
        },
        MethodStep::Complete(NativeConversion::Value((rooted, Some(target)))) => {
            let key_value = runtime.property_key_value(&key)?;
            ProxyGetStep::Call {
                target,
                receiver: Value::Object(rooted.handler.clone()),
                arguments: vec![Value::Object(rooted.target.clone()), key_value, receiver],
                resume: ProxyGetResume {
                    realm,
                    phase: Phase::Trap { rooted, key },
                },
            }
        }
        MethodStep::Read {
            object,
            key: method_key,
            receiver: method_receiver,
            resume,
        } => ProxyGetStep::Read {
            object,
            key: method_key,
            receiver: method_receiver,
            resume: ProxyGetResume {
                realm,
                phase: Phase::Method {
                    resume,
                    key,
                    receiver,
                },
            },
        },
    })
}

impl ProxyGetResume {
    pub(crate) fn resume(
        self,
        runtime: &Runtime,
        completion: Completion,
    ) -> Result<ProxyGetStep, RuntimeError> {
        let Completion::Return(value) = completion else {
            return Ok(ProxyGetStep::Complete(completion));
        };
        let realm = self.realm;
        match self.phase {
            Phase::Method {
                resume,
                key,
                receiver,
            } => method(
                runtime,
                realm,
                key,
                receiver,
                resume.resume(runtime, Completion::Return(value))?,
            ),
            Phase::Forward { .. } => Ok(ProxyGetStep::Complete(Completion::Return(value))),
            Phase::Trap { rooted, key } => Ok(ProxyGetStep::Descriptor {
                object: rooted.target.clone(),
                key,
                resume: Self {
                    realm,
                    phase: Phase::Invariant {
                        _rooted: rooted,
                        result: value,
                    },
                },
            }),
            Phase::Invariant { .. } => Err(RuntimeError::Invariant(
                "Proxy Get descriptor continuation received a value reply",
            )),
        }
    }

    pub(crate) fn descriptor(
        self,
        runtime: &Runtime,
        descriptor: NativeConversion<Option<CompleteOrdinaryPropertyDescriptor>>,
    ) -> Result<ProxyGetStep, RuntimeError> {
        let Phase::Invariant { _rooted, result } = self.phase else {
            return Err(RuntimeError::Invariant(
                "Proxy Get value continuation received a descriptor reply",
            ));
        };
        let descriptor = match descriptor {
            NativeConversion::Value(descriptor) => descriptor,
            NativeConversion::Throw(value) => {
                return Ok(ProxyGetStep::Complete(Completion::Throw(value)));
            }
        };
        let inconsistent = match descriptor {
            Some(CompleteOrdinaryPropertyDescriptor::Data {
                value,
                writable: false,
                configurable: false,
                ..
            }) => !result.same_value(&value),
            Some(CompleteOrdinaryPropertyDescriptor::Accessor {
                get: None,
                configurable: false,
                ..
            }) => !matches!(result, Value::Undefined),
            _ => false,
        };
        Ok(ProxyGetStep::Complete(if inconsistent {
            Completion::Throw(runtime.new_native_error(
                self.realm,
                NativeErrorKind::Type,
                "proxy: inconsistent get",
            )?)
        } else {
            Completion::Return(result)
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn abandoned_proxy_method_releases_roots_and_depth_guard() {
        let runtime = Runtime::new();
        let weak = std::rc::Rc::downgrade(&runtime.0);
        let context = runtime.new_context();
        let target = runtime.new_object(None).unwrap();
        let handler = runtime.new_object(None).unwrap();
        let target_id = target.object_id();
        let handler_id = handler.object_id();
        let NativeConversion::Value(proxy) = runtime
            .new_proxy(context.realm, Value::Object(target), Value::Object(handler))
            .unwrap()
        else {
            panic!("proxy allocation failed");
        };
        let step = ProxyGetStep::start(
            &runtime,
            context.realm,
            proxy,
            runtime.intern_property_key("x").unwrap(),
            Value::Undefined,
        )
        .unwrap();
        assert_eq!(runtime.0.proxy_method_depth.get(), 1);
        runtime.run_gc().unwrap();
        assert!(runtime.0.state.borrow().heap.object(target_id).is_ok());
        assert!(runtime.0.state.borrow().heap.object(handler_id).is_ok());
        drop(step);
        assert_eq!(runtime.0.proxy_method_depth.get(), 0);
        runtime.run_gc().unwrap();
        assert!(runtime.0.state.borrow().heap.object(target_id).is_err());
        assert!(runtime.0.state.borrow().heap.object(handler_id).is_err());
        drop(context);
        drop(runtime);
        assert!(weak.upgrade().is_none());
    }

    #[test]
    fn mismatched_proxy_reply_rejects_and_releases_the_method_guard() {
        let runtime = Runtime::new();
        let context = runtime.new_context();
        let NativeConversion::Value(proxy) = runtime
            .new_proxy(
                context.realm,
                Value::Object(runtime.new_object(None).unwrap()),
                Value::Object(runtime.new_object(None).unwrap()),
            )
            .unwrap()
        else {
            panic!("proxy allocation failed");
        };
        let ProxyGetStep::Read { resume, .. } = ProxyGetStep::start(
            &runtime,
            context.realm,
            proxy,
            runtime.intern_property_key("x").unwrap(),
            Value::Undefined,
        )
        .unwrap() else {
            panic!("expected method read");
        };
        assert_eq!(runtime.0.proxy_method_depth.get(), 1);
        assert!(
            resume
                .descriptor(&runtime, NativeConversion::Value(None))
                .is_err()
        );
        assert_eq!(runtime.0.proxy_method_depth.get(), 0);
    }
}
