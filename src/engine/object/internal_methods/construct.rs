//! Proxy [[Construct]] keeps each selected handler and the incoming new.target rooted.
use super::{ProxyMethodStackGuard, RootedProxy};
use crate::engine::{
    api::{
        error::{ErrorKind, NativeErrorKind},
        runtime::Runtime,
        runtime_error::RuntimeError,
    },
    heap::ContextId,
    object::{ObjectRef, PropertyKey},
    value::{Value, conversion::NativeConversion},
    vm::{
        Completion,
        call::{ConstructNewTarget, ConstructorRef, DirectCallTarget},
    },
};
pub(crate) enum ProxyConstructStep {
    Complete(Completion),
    Read {
        object: ObjectRef,
        key: PropertyKey,
        resume: ProxyConstructResume,
    },
    Call {
        target: DirectCallTarget,
        receiver: Value,
        arguments: Vec<Value>,
        resume: ProxyConstructResume,
    },
    Construct {
        target: ConstructorRef,
        new_target: ConstructNewTarget,
        arguments: Vec<Value>,
        resume: ProxyConstructResume,
    },
}
pub(crate) struct ProxyConstructResume {
    phase: Phase,
}
enum Phase {
    Method {
        rooted: RootedProxy,
        search: Search,
    },
    Result {
        realm: ContextId,
        trap: bool,
        _rooted: RootedProxy,
        _guard: ProxyMethodStackGuard,
    },
}
struct Search {
    realm: ContextId,
    key: PropertyKey,
    limit: Option<usize>,
    depth: usize,
    guard: ProxyMethodStackGuard,
    new_target: ConstructNewTarget,
    arguments: Vec<Value>,
}
fn overflow(runtime: &Runtime, realm: ContextId) -> Result<ProxyConstructStep, RuntimeError> {
    Ok(ProxyConstructStep::Complete(Completion::Throw(
        runtime.new_native_error(realm, NativeErrorKind::Internal, "stack overflow")?,
    )))
}
impl ProxyConstructStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        proxy: ConstructorRef,
        new_target: ConstructNewTarget,
        arguments: Vec<Value>,
    ) -> Result<Self, RuntimeError> {
        if runtime.proxy_method_stack_would_overflow() {
            return overflow(runtime, realm);
        }
        Search {
            realm,
            key: runtime.intern_property_key("construct")?,
            limit: runtime.proxy_method_chain_limit("construct"),
            depth: 0,
            guard: ProxyMethodStackGuard::enter(runtime),
            new_target,
            arguments,
        }
        .read(runtime, proxy)
    }
}
impl Search {
    fn read(
        self,
        runtime: &Runtime,
        proxy: ConstructorRef,
    ) -> Result<ProxyConstructStep, RuntimeError> {
        if self.limit.is_some_and(|limit| self.depth == limit) {
            return overflow(runtime, self.realm);
        }
        let data =
            runtime
                .proxy_snapshot_if_any(proxy.as_object())?
                .ok_or(RuntimeError::Invariant(
                    "Proxy construct dispatch reached an ordinary object",
                ))?;
        if data.is_revoked {
            return match runtime.proxy_revoked_throw(self.realm)? {
                NativeConversion::Throw(value) => {
                    Ok(ProxyConstructStep::Complete(Completion::Throw(value)))
                }
                NativeConversion::Value(()) => Err(RuntimeError::Invariant(
                    "revoked Proxy construct returned a value",
                )),
            };
        }
        let rooted = runtime.root_proxy_snapshot(proxy.as_object(), data)?;
        Ok(ProxyConstructStep::Read {
            object: rooted.handler.clone(),
            key: self.key.clone(),
            resume: ProxyConstructResume {
                phase: Phase::Method {
                    rooted,
                    search: self,
                },
            },
        })
    }
}
impl ProxyConstructResume {
    pub(crate) fn resume(
        self,
        runtime: &Runtime,
        completion: Completion,
    ) -> Result<ProxyConstructStep, RuntimeError> {
        let (rooted, mut search) = match self.phase {
            Phase::Method { rooted, search } => (rooted, search),
            Phase::Result { realm, trap, .. } => {
                return Ok(ProxyConstructStep::Complete(match completion {
                    Completion::Return(value) if trap && !matches!(value, Value::Object(_)) => {
                        Completion::Throw(runtime.new_native_error(
                            realm,
                            NativeErrorKind::Type,
                            "not an object",
                        )?)
                    }
                    result => result,
                }));
            }
        };
        let method = match completion {
            Completion::Return(value) => value,
            result @ Completion::Throw(_) => return Ok(ProxyConstructStep::Complete(result)),
        };
        // Validate the immediate target after Get(trap), even for missing traps.
        let target = match runtime
            .constructor_from_value(search.realm, Value::Object(rooted.target.clone()))?
        {
            NativeConversion::Value(target) => target,
            NativeConversion::Throw(value) => {
                return Ok(ProxyConstructStep::Complete(Completion::Throw(value)));
            }
        };
        if matches!(method, Value::Null | Value::Undefined) {
            if runtime.is_proxy_object(target.as_object())? {
                search.depth = search.depth.saturating_add(1);
                return search.read(runtime, target);
            }
            return Ok(ProxyConstructStep::Construct {
                target,
                new_target: search.new_target,
                arguments: search.arguments,
                resume: Self {
                    phase: Phase::Result {
                        realm: search.realm,
                        trap: false,
                        _rooted: rooted,
                        _guard: search.guard,
                    },
                },
            });
        }
        let array = runtime.new_array_from_values(search.realm, search.arguments)?;
        let method = match runtime.direct_call_target_from_value(method) {
            Ok(method) => method,
            Err(RuntimeError::Engine(error)) if error.kind() == ErrorKind::Type => {
                return Ok(ProxyConstructStep::Complete(Completion::Throw(
                    runtime.new_native_error_from_error(
                        search.realm,
                        NativeErrorKind::Type,
                        &error,
                    )?,
                )));
            }
            Err(error) => return Err(error),
        };
        Ok(ProxyConstructStep::Call {
            target: method,
            receiver: Value::Object(rooted.handler.clone()),
            arguments: vec![
                Value::Object(rooted.target.clone()),
                Value::Object(array),
                search.new_target.value(),
            ],
            resume: Self {
                phase: Phase::Result {
                    realm: search.realm,
                    trap: true,
                    _rooted: rooted,
                    _guard: search.guard,
                },
            },
        })
    }
}
pub(super) fn finish(
    runtime: &Runtime,
    realm: ContextId,
    mut step: ProxyConstructStep,
) -> Result<Completion, RuntimeError> {
    loop {
        step = match step {
            ProxyConstructStep::Complete(result) => return Ok(result),
            ProxyConstructStep::Read {
                object,
                key,
                resume,
            } => resume.resume(
                runtime,
                runtime.internal_get(realm, &object, &key, Value::Object(object.clone()))?,
            )?,
            ProxyConstructStep::Call {
                target,
                receiver,
                arguments,
                resume,
            } => {
                let result = match target {
                    DirectCallTarget::Callable(target) => {
                        runtime.call_internal(realm, &target, receiver, &arguments)?
                    }
                    DirectCallTarget::NonCallableProxy(proxy) => {
                        runtime.call_proxy(realm, &proxy, receiver, &arguments)?
                    }
                };
                resume.resume(runtime, result)?
            }
            ProxyConstructStep::Construct {
                target,
                new_target,
                arguments,
                resume,
            } => resume.resume(
                runtime,
                runtime
                    .construct_internal_with_new_target(realm, &target, new_target, &arguments)?,
            )?,
        };
    }
}
