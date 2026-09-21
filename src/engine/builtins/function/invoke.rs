//! Call forwarding shares validation and argument ownership across both VM consumers.
use super::arguments::{ArgumentsStep, finish as finish_arguments};
use crate::engine::builtins::native::ReflectKind;
use crate::engine::vm::call::{ConstructNewTarget, ConstructorRef, DirectCallTarget};
use crate::engine::{
    api::{error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError},
    builtins::native::NativeFunctionId,
    heap::ContextId,
    value::{JsValue, conversion::NativeConversion},
    vm::{
        Completion,
        call::{NativeArguments, NativeInvocation},
    },
};

#[derive(Clone, Copy)]
pub(crate) enum InvokeKind {
    Call,
    Apply,
    ReflectApply,
    ReflectConstruct,
}
impl InvokeKind {
    pub(crate) fn for_target(target: NativeFunctionId) -> Option<Self> {
        Some(match target {
            NativeFunctionId::FunctionPrototypeCall => Self::Call,
            NativeFunctionId::FunctionPrototypeApply => Self::Apply,
            NativeFunctionId::Reflect(ReflectKind::Apply) => Self::ReflectApply,
            NativeFunctionId::Reflect(ReflectKind::Construct) => Self::ReflectConstruct,
            _ => return None,
        })
    }
}
pub(crate) enum InvokeStep {
    Construct(Box<InvokeConstruct>),
    Complete(Completion),
    Arguments { resume: InvokeResume },
    Call(Box<InvokeCall>),
}
pub(crate) struct InvokeResume(Box<InvokeResumeState>);
impl std::ops::Deref for InvokeResume {
    type Target = InvokeResumeState;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for InvokeResume {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
const _: () = assert!(std::mem::size_of::<InvokeResume>() <= 8);
pub(crate) struct InvokeResumeState {
    runtime: Runtime,
    arguments: Vec<JsValue>,
    pending_effect: InvokeStepPending,
    realm: ContextId,
    target: ForwardTarget,
}
impl Drop for InvokeResumeState {
    fn drop(&mut self) {
        if let Some(value) = self.pending_effect.arguments_value.take() {
            let _ = self.runtime.release_jsvalue(value);
        }
        for value in self.arguments.drain(..) {
            let _ = self.runtime.release_jsvalue(value);
        }
        let _ = self.target.release_pending_new_target(&self.runtime);
    }
}
enum ForwardTarget {
    Call {
        target: DirectCallTarget,
        receiver: JsValue,
    },
    Construct {
        target: JsValue,
        new_target: Option<ConstructNewTarget>,
    },
}
impl InvokeStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        kind: InvokeKind,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        let NativeInvocation::Call { this_value } = invocation else {
            return Err(RuntimeError::Invariant(
                "call forwarding requires a generic invocation",
            ));
        };
        if matches!(kind, InvokeKind::ReflectConstruct) {
            let new_target = if arguments.actual_arg_count > 2 {
                let value = arguments.readable.get(2).ok_or(RuntimeError::Invariant(
                    "Reflect.construct newTarget argv was not readable",
                ))?;
                if !matches!(value, JsValue::Object(_)) {
                    return Ok(Self::Complete(Completion::Throw(
                        runtime.new_not_constructor_error_jsvalue(realm, value)?,
                    )));
                }
                Some(
                    match runtime.constructor_from_jsvalue(realm, runtime.dup_jsvalue(value)?)? {
                        NativeConversion::Value(target) => ConstructNewTarget::Validated(target),
                        NativeConversion::Throw(value) => {
                            return Ok(Self::Complete(Completion::Throw(value)));
                        }
                    },
                )
            } else {
                None
            };
            return Ok({
                let __pending_field_value = runtime.dup_jsvalue(&arguments.readable[1])?;
                let __pending_field_resume = InvokeResume(Box::new(InvokeResumeState {
                    runtime: runtime.clone(),
                    arguments: Vec::new(),
                    pending_effect: InvokeStepPending::default(),
                    realm,
                    target: ForwardTarget::Construct {
                        target: runtime.dup_jsvalue(&arguments.readable[0])?,
                        new_target,
                    },
                }));
                Self::request_arguments(__pending_field_value, __pending_field_resume)
            });
        }
        let (target, receiver, list) = match kind {
            InvokeKind::ReflectConstruct => unreachable!("constructor validation already handled"),
            InvokeKind::Call => {
                let target = match runtime
                    .direct_call_target_from_jsvalue(runtime.dup_jsvalue(this_value)?)
                {
                    Ok(target) => target,
                    Err(RuntimeError::Engine(error))
                        if error.kind() == crate::engine::api::error::ErrorKind::Type =>
                    {
                        return Ok(Self::Complete(Completion::Throw(
                            runtime.new_native_error_from_error_jsvalue(
                                realm,
                                NativeErrorKind::Type,
                                &error,
                            )?,
                        )));
                    }
                    Err(error) => return Err(error),
                };
                let receiver = runtime
                    .dup_jsvalue(arguments.readable.first().unwrap_or(&JsValue::Undefined))?;
                let mut forwarded = Vec::new();
                let result = (|| {
                    forwarded
                        .try_reserve_exact(arguments.actual_arg_count.saturating_sub(1))
                        .map_err(|_| {
                            RuntimeError::Invariant("function.call argv allocation failed")
                        })?;
                    for value in arguments.readable[..arguments.actual_arg_count]
                        .iter()
                        .skip(1)
                    {
                        forwarded.push(runtime.dup_jsvalue(value)?);
                    }
                    Ok::<_, RuntimeError>(())
                })();
                if let Err(error) = result {
                    let _ = runtime.release_jsvalue(receiver);
                    for argument in forwarded {
                        let _ = runtime.release_jsvalue(argument);
                    }
                    return Err(error);
                }
                #[cfg(feature = "profiling")]
                {
                    crate::engine::api::profiling::record_call_buffer_capacity(
                        "function.call_suffix",
                        0,
                        forwarded.capacity(),
                        size_of::<JsValue>(),
                    );
                    crate::engine::api::profiling::record_call_buffer_js_value_copies(
                        "function.call_suffix",
                        &forwarded,
                    );
                }
                return Ok(Self::Call(Box::new(InvokeCall {
                    target,
                    receiver,
                    arguments: forwarded,
                })));
            }
            InvokeKind::Apply => {
                let target = match this_value {
                    JsValue::Object(id) => {
                        let object = crate::engine::object::ObjectRef::from_borrowed_handle(
                            runtime.clone(),
                            *id,
                        )?;
                        runtime.as_callable(&object)?
                    }
                    _ => None,
                };
                let Some(target) = target else {
                    return Ok(Self::Complete(Completion::Throw(
                        runtime.new_native_error_jsvalue(
                            realm,
                            NativeErrorKind::Type,
                            "not a function",
                        )?,
                    )));
                };
                (
                    DirectCallTarget::Callable(target),
                    runtime.dup_jsvalue(&arguments.readable[0])?,
                    runtime.dup_jsvalue(&arguments.readable[1])?,
                )
            }
            InvokeKind::ReflectApply => (
                DirectCallTarget::Callable(runtime.callable_from_jsvalue(&arguments.readable[0])?),
                runtime.dup_jsvalue(&arguments.readable[1])?,
                runtime.dup_jsvalue(&arguments.readable[2])?,
            ),
        };
        if matches!(kind, InvokeKind::Apply) && matches!(list, JsValue::Null | JsValue::Undefined) {
            return Ok(Self::Call(Box::new(InvokeCall {
                target,
                receiver,
                arguments: Vec::new(),
            })));
        }
        Ok({
            let __pending_field_value = list;
            let __pending_field_resume = InvokeResume(Box::new(InvokeResumeState {
                runtime: runtime.clone(),
                arguments: Vec::new(),
                pending_effect: InvokeStepPending::default(),
                realm,
                target: ForwardTarget::Call { target, receiver },
            }));
            Self::request_arguments(__pending_field_value, __pending_field_resume)
        })
    }

    pub(crate) fn start_spread(
        runtime: &Runtime,
        realm: ContextId,
        kind: crate::engine::code::bytecode::ApplyKind,
        target: &JsValue,
        receiver: &JsValue,
        value: &JsValue,
    ) -> Result<Self, RuntimeError> {
        // OP_apply checks callability before inspecting argsList, including construct mode.
        let callable = runtime.callable_from_jsvalue(target)?;
        if matches!(value, JsValue::Null | JsValue::Undefined) {
            return Ok(Self::Call(Box::new(InvokeCall {
                target: DirectCallTarget::Callable(callable),
                receiver: runtime.dup_jsvalue(receiver)?,
                arguments: Vec::new(),
            })));
        }
        let mut owner = InvokeResume(Box::new(InvokeResumeState {
            runtime: runtime.clone(),
            arguments: Vec::new(),
            pending_effect: Default::default(),
            realm,
            target: ForwardTarget::Construct {
                target: JsValue::Undefined,
                new_target: None,
            },
        }));
        owner.target = match kind {
            crate::engine::code::bytecode::ApplyKind::Call => ForwardTarget::Call {
                target: DirectCallTarget::Callable(callable),
                receiver: runtime.dup_jsvalue(receiver)?,
            },
            crate::engine::code::bytecode::ApplyKind::Construct => ForwardTarget::Construct {
                target: runtime.dup_jsvalue(target)?,
                new_target: Some(ConstructNewTarget::Raw(runtime.dup_jsvalue(receiver)?)),
            },
        };
        let value = runtime.dup_jsvalue(value)?;
        Ok(Self::request_arguments(value, owner))
    }
}
impl ForwardTarget {
    /// Release a raw new-target edge still owned by a failing construct
    /// forward.  `ConstructNewTarget::Raw` carries no `Drop`, so every
    /// abandonment path must release it explicitly.
    fn release_pending_new_target(&mut self, runtime: &Runtime) -> Result<(), RuntimeError> {
        match self {
            Self::Call { receiver, .. } => {
                runtime.release_jsvalue(std::mem::replace(receiver, JsValue::Undefined))
            }
            Self::Construct { target, new_target } => {
                runtime.release_jsvalue(std::mem::replace(target, JsValue::Undefined))?;
                if let Some(ConstructNewTarget::Raw(value)) = new_target.take() {
                    runtime.release_jsvalue(value)?;
                }
                Ok(())
            }
        }
    }
}
impl InvokeResume {
    pub(crate) fn arguments(
        mut self,
        runtime: &Runtime,
        result: NativeConversion<Vec<JsValue>>,
    ) -> Result<InvokeStep, RuntimeError> {
        let arguments = match result {
            NativeConversion::Throw(value) => {
                self.0.target.release_pending_new_target(runtime)?;
                return Ok(InvokeStep::Complete(Completion::Throw(value)));
            }
            NativeConversion::Value(arguments) => arguments,
        };
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_call_buffer_observed(
            "invoke.argv_carrier",
            arguments.capacity(),
            size_of::<JsValue>(),
        );
        self.0.arguments = arguments;
        let target = std::mem::replace(
            &mut self.0.target,
            ForwardTarget::Construct {
                target: JsValue::Undefined,
                new_target: None,
            },
        );
        Ok(match target {
            ForwardTarget::Call { target, receiver } => InvokeStep::Call(Box::new(InvokeCall {
                target,
                receiver,
                arguments: std::mem::take(&mut self.0.arguments),
            })),
            ForwardTarget::Construct { target, new_target } => {
                let classified = runtime.constructor_from_jsvalue(self.0.realm, target);
                let target = match classified {
                    Err(error) => {
                        if let Some(ConstructNewTarget::Raw(value)) = new_target {
                            let _ = runtime.release_jsvalue(value);
                        }
                        return Err(error);
                    }
                    Ok(value) => value,
                };
                let target = match target {
                    NativeConversion::Value(target) => target,
                    NativeConversion::Throw(value) => {
                        if let Some(ConstructNewTarget::Raw(new_target)) = new_target {
                            runtime.release_jsvalue(new_target)?;
                        }
                        return Ok(InvokeStep::Complete(Completion::Throw(value)));
                    }
                };
                let new_target =
                    new_target.unwrap_or_else(|| ConstructNewTarget::Validated(target.clone()));
                InvokeStep::Construct(Box::new(InvokeConstruct {
                    target,
                    new_target,
                    arguments: std::mem::take(&mut self.0.arguments),
                }))
            }
        })
    }
}
pub(crate) fn finish(
    runtime: &Runtime,
    realm: ContextId,
    mut step: InvokeStep,
) -> Result<Completion, RuntimeError> {
    loop {
        step = match step {
            InvokeStep::Complete(result) => return Ok(result),
            InvokeStep::Construct(request) => {
                let target = request.target;
                let new_target = request.new_target;
                return runtime.construct_internal_jsvalue(
                    realm,
                    &target,
                    new_target,
                    request.arguments,
                );
            }
            InvokeStep::Arguments { mut resume } => {
                let value = resume.take_arguments_value();
                resume.arguments(
                    runtime,
                    finish_arguments(runtime, realm, ArgumentsStep::start(runtime, realm, value)?)?,
                )?
            }
            InvokeStep::Call(request) => {
                let target = request.target;
                return match target {
                    DirectCallTarget::Callable(target) => runtime.call_internal_jsvalue(
                        realm,
                        &target,
                        request.receiver,
                        request.arguments,
                    ),
                    DirectCallTarget::NonCallableProxy(proxy) => runtime.call_proxy_jsvalue(
                        realm,
                        &proxy,
                        request.receiver,
                        request.arguments,
                    ),
                };
            }
        };
    }
}

#[derive(Default)]
struct InvokeStepPending {
    arguments_value: Option<JsValue>,
}
impl InvokeStep {
    pub(crate) fn request_arguments(value: JsValue, mut resume: InvokeResume) -> Self {
        resume.0.pending_effect.arguments_value = Some(value);
        Self::Arguments { resume }
    }
}
impl InvokeResume {
    pub(crate) fn take_arguments_value(&mut self) -> JsValue {
        self.0
            .pending_effect
            .arguments_value
            .take()
            .expect("InvokeStep Arguments value")
    }
}
const _: () = assert!(std::mem::size_of::<InvokeStep>() <= 64);

pub(crate) struct InvokeCall {
    pub(crate) target: DirectCallTarget,
    pub(crate) receiver: JsValue,
    pub(crate) arguments: Vec<JsValue>,
}

pub(crate) struct InvokeConstruct {
    pub(crate) target: ConstructorRef,
    pub(crate) new_target: ConstructNewTarget,
    pub(crate) arguments: Vec<JsValue>,
}

// S11 all-domain protocol bound; inline completion stays allocation-free.
const _: () = assert!(std::mem::size_of::<InvokeStep>() <= 64);
