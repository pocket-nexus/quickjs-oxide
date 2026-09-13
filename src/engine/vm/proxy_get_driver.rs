//! Schedule the object-owned Proxy Get protocol using explicit child frames.
use super::{
    Completion,
    call::{BytecodeCallRequest, CallableExecution, DirectCallTarget},
    driver::{CallStep, push_frame},
    exception::runtime_error_to_vm_error,
    execution::RunningExecution,
    frame::{FrameId, OperationTarget, ReturnTarget, ReturnValue},
};
use crate::engine::api::{Error, runtime::Runtime};
use crate::engine::code::function::metadata::FunctionKind;
use crate::engine::object::{
    CompleteOrdinaryPropertyDescriptor, ObjectRef, OrdinaryRead, PropertyKey, ProxyGetResume,
    ProxyGetStep, ProxyOwnResume, ProxyOwnStep,
};
use crate::engine::object::{
    OrdinaryPropertyDescriptor, PreparedHas, ProxyBooleanKind, ProxyBooleanResume, ProxyBooleanStep,
};
use crate::engine::value::conversion::descriptor::{DescriptorResume, DescriptorStep};
use crate::engine::value::{Value, conversion::NativeConversion};

enum Resume {
    Get(ProxyGetResume),
    Call(crate::engine::object::ProxyCallResume),
    Own(ProxyOwnResume),
    Conversion(DescriptorResume),
    Boolean(ProxyBooleanResume),
}

enum Step {
    Complete(Completion),
    BooleanComplete(NativeConversion<bool>),
    OwnComplete(NativeConversion<Option<CompleteOrdinaryPropertyDescriptor>>),
    Converted(NativeConversion<OrdinaryPropertyDescriptor>),
    Has {
        object: ObjectRef,
        key: PropertyKey,
        resume: Resume,
    },
    Read {
        object: ObjectRef,
        key: PropertyKey,
        receiver: Value,
        resume: Resume,
    },
    Call {
        target: DirectCallTarget,
        receiver: Value,
        arguments: Vec<Value>,
        resume: Resume,
    },
    Descriptor {
        object: ObjectRef,
        key: PropertyKey,
        resume: Resume,
    },
    Extensible {
        object: ObjectRef,
        resume: Resume,
    },
    Convert {
        value: Value,
        resume: ProxyOwnResume,
    },
}

impl From<ProxyGetStep> for Step {
    fn from(step: ProxyGetStep) -> Self {
        match step {
            ProxyGetStep::Complete(result) => Self::Complete(result),
            ProxyGetStep::Read {
                object,
                key,
                receiver,
                resume,
            } => Self::Read {
                object,
                key,
                receiver,
                resume: Resume::Get(resume),
            },
            ProxyGetStep::Call {
                target,
                receiver,
                arguments,
                resume,
            } => Self::Call {
                target,
                receiver,
                arguments,
                resume: Resume::Get(resume),
            },
            ProxyGetStep::Descriptor {
                object,
                key,
                resume,
            } => Self::Descriptor {
                object,
                key,
                resume: Resume::Get(resume),
            },
        }
    }
}

impl From<ProxyOwnStep> for Step {
    fn from(step: ProxyOwnStep) -> Self {
        match step {
            ProxyOwnStep::Complete(result) => Self::OwnComplete(result),
            ProxyOwnStep::Read {
                object,
                key,
                receiver,
                resume,
            } => Self::Read {
                object,
                key,
                receiver,
                resume: Resume::Own(resume),
            },
            ProxyOwnStep::Call {
                target,
                receiver,
                arguments,
                resume,
            } => Self::Call {
                target,
                receiver,
                arguments,
                resume: Resume::Own(resume),
            },
            ProxyOwnStep::Descriptor {
                object,
                key,
                resume,
            } => Self::Descriptor {
                object,
                key,
                resume: Resume::Own(resume),
            },
            ProxyOwnStep::Extensible { object, resume } => Self::Extensible {
                object,
                resume: Resume::Own(resume),
            },
            ProxyOwnStep::Convert { value, resume } => Self::Convert { value, resume },
        }
    }
}

impl From<DescriptorStep> for Step {
    fn from(step: DescriptorStep) -> Self {
        match step {
            DescriptorStep::Complete(result) => Self::Converted(result),
            DescriptorStep::Has {
                object,
                key,
                resume,
            } => Self::Has {
                object,
                key,
                resume: Resume::Conversion(resume),
            },
            DescriptorStep::Read {
                object,
                key,
                receiver,
                resume,
            } => Self::Read {
                object,
                key,
                receiver,
                resume: Resume::Conversion(resume),
            },
        }
    }
}

impl From<ProxyBooleanStep> for Step {
    fn from(step: ProxyBooleanStep) -> Self {
        match step {
            ProxyBooleanStep::Complete(result) => Self::BooleanComplete(result),
            ProxyBooleanStep::Read {
                object,
                key,
                receiver,
                resume,
            } => Self::Read {
                object,
                key,
                receiver,
                resume: Resume::Boolean(resume),
            },
            ProxyBooleanStep::Call {
                target,
                receiver,
                arguments,
                resume,
            } => Self::Call {
                target,
                receiver,
                arguments,
                resume: Resume::Boolean(resume),
            },
            ProxyBooleanStep::Has {
                object,
                key,
                resume,
            } => Self::Has {
                object,
                key,
                resume: Resume::Boolean(resume),
            },
            ProxyBooleanStep::Extensible { object, resume } => Self::Extensible {
                object,
                resume: Resume::Boolean(resume),
            },
            ProxyBooleanStep::Descriptor {
                object,
                key,
                resume,
            } => Self::Descriptor {
                object,
                key,
                resume: Resume::Boolean(resume),
            },
        }
    }
}

impl From<crate::engine::object::ProxyCallStep> for Step {
    fn from(step: crate::engine::object::ProxyCallStep) -> Self {
        use crate::engine::object::ProxyCallStep;
        match step {
            ProxyCallStep::Complete(result) => Self::Complete(result),
            ProxyCallStep::Read {
                object,
                key,
                receiver,
                resume,
            } => Self::Read {
                object,
                key,
                receiver,
                resume: Resume::Call(resume),
            },
            ProxyCallStep::Call {
                target,
                receiver,
                arguments,
                resume,
            } => Self::Call {
                target,
                receiver,
                arguments,
                resume: Resume::Call(resume),
            },
        }
    }
}

impl Resume {
    fn boolean(
        self,
        runtime: &Runtime,
        result: NativeConversion<bool>,
    ) -> Result<Step, crate::engine::api::runtime_error::RuntimeError> {
        match self {
            Self::Own(resume) => resume.extensible(result).map(Into::into),
            Self::Boolean(resume) => resume.boolean(runtime, result).map(Into::into),
            Self::Conversion(resume) => resume.has(runtime, result).map(Into::into),
            Self::Get(_) | Self::Call(_) => {
                Err(crate::engine::api::runtime_error::RuntimeError::Invariant(
                    "Proxy Get received a boolean reply",
                ))
            }
        }
    }

    fn resume(
        self,
        runtime: &Runtime,
        completion: Completion,
    ) -> Result<Step, crate::engine::api::runtime_error::RuntimeError> {
        match self {
            Self::Get(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::Call(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::Own(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::Conversion(resume) => resume.read(runtime, completion).map(Into::into),
            Self::Boolean(resume) => resume.resume(runtime, completion).map(Into::into),
        }
    }
    fn descriptor(
        self,
        runtime: &Runtime,
        result: NativeConversion<Option<CompleteOrdinaryPropertyDescriptor>>,
    ) -> Result<Step, crate::engine::api::runtime_error::RuntimeError> {
        match self {
            Self::Get(resume) => resume.descriptor(runtime, result).map(Into::into),
            Self::Own(resume) => resume.descriptor(runtime, result).map(Into::into),
            Self::Boolean(resume) => resume.descriptor(runtime, result).map(Into::into),
            Self::Conversion(_) | Self::Call(_) => {
                Err(crate::engine::api::runtime_error::RuntimeError::Invariant(
                    "descriptor conversion received an own-property reply",
                ))
            }
        }
    }
}

pub(super) struct PendingProxyGet {
    identity: u64,
    parents: Vec<Resume>,
    resume: Resume,
    finish: Finish,
}

impl PendingProxyGet {
    pub(super) fn continuation_depth(&self) -> usize {
        // The pending reply itself is accounted for by its installed child
        // frame; the parent domain operations have no bytecode frame.
        self.parents.len()
    }
}

enum Finish {
    PropertyRead(usize),
    Call { depth: usize, tail: bool },
    Conversion(super::conversion_driver::ConversionWait),
}

pub(super) enum Progress {
    Call(CallStep),
    Conversion(super::conversion_driver::ConversionTask),
}

#[allow(clippy::too_many_arguments)]
pub(super) fn start(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    frame: FrameId,
    object: ObjectRef,
    key: PropertyKey,
    receiver: Value,
    depth: usize,
) -> Result<CallStep, Error> {
    let parent = execution.frames.current_mut(frame)?;
    let identity = parent
        .cold
        .property_generation
        .checked_add(1)
        .ok_or_else(|| Error::internal("property operation identity exhausted"))?;
    parent.cold.property_generation = identity;
    let realm = parent.executable.realm;
    let result = (|| {
        let step = ProxyGetStep::start(runtime, realm, object, key, receiver)
            .map_err(runtime_error_to_vm_error)?;
        advance(
            runtime,
            execution,
            frame,
            identity,
            Vec::new(),
            step.into(),
            Finish::PropertyRead(depth),
        )
    })();
    match finish_error(runtime, realm, result)? {
        Progress::Call(step) => Ok(step),
        Progress::Conversion(_) => Err(Error::internal("property read returned a conversion")),
    }
}

pub(super) fn start_conversion(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    frame: FrameId,
    object: ObjectRef,
    key: PropertyKey,
    wait: super::conversion_driver::ConversionWait,
) -> Result<Progress, Error> {
    let parent = execution.frames.current_mut(frame)?;
    let identity = parent
        .cold
        .property_generation
        .checked_add(1)
        .ok_or_else(|| Error::internal("property operation identity exhausted"))?;
    parent.cold.property_generation = identity;
    let realm = parent.executable.realm;
    let result = (|| {
        let receiver = Value::Object(object.clone());
        let step = ProxyGetStep::start(runtime, realm, object, key, receiver)
            .map_err(runtime_error_to_vm_error)?;
        advance(
            runtime,
            execution,
            frame,
            identity,
            Vec::new(),
            step.into(),
            Finish::Conversion(wait),
        )
    })();
    finish_error(runtime, realm, result)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn start_call(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    frame: FrameId,
    proxy: ObjectRef,
    receiver: Value,
    arguments: Vec<Value>,
    tail: bool,
    depth: usize,
) -> Result<CallStep, Error> {
    match start_proxy_call(
        runtime,
        execution,
        frame,
        proxy,
        receiver,
        arguments,
        Finish::Call { depth, tail },
    )? {
        Progress::Call(step) => Ok(step),
        Progress::Conversion(_) => Err(Error::internal("Proxy call returned a conversion")),
    }
}

pub(super) fn start_conversion_call(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    frame: FrameId,
    proxy: ObjectRef,
    receiver: Value,
    arguments: Vec<Value>,
    wait: super::conversion_driver::ConversionWait,
) -> Result<Progress, Error> {
    start_proxy_call(
        runtime,
        execution,
        frame,
        proxy,
        receiver,
        arguments,
        Finish::Conversion(wait),
    )
}

fn start_proxy_call(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    frame: FrameId,
    proxy: ObjectRef,
    receiver: Value,
    arguments: Vec<Value>,
    finish: Finish,
) -> Result<Progress, Error> {
    let parent = execution.frames.current_mut(frame)?;
    let identity = parent
        .cold
        .property_generation
        .checked_add(1)
        .ok_or_else(|| Error::internal("property operation identity exhausted"))?;
    parent.cold.property_generation = identity;
    let realm = parent.executable.realm;
    let result = (|| {
        let step =
            crate::engine::object::ProxyCallStep::start(runtime, realm, proxy, receiver, arguments)
                .map_err(runtime_error_to_vm_error)?;
        advance(
            runtime,
            execution,
            frame,
            identity,
            Vec::new(),
            step.into(),
            finish,
        )
    })();
    finish_error(runtime, realm, result)
}

pub(super) fn reply(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    target: ReturnTarget,
    completion: Completion,
) -> Result<Progress, Error> {
    let parent = execution.frames.current_mut(target.frame)?;
    let pending = parent
        .cold
        .property_wait
        .take()
        .ok_or_else(|| Error::internal("property reply has no pending operation"))?;
    if target.operation != Some(OperationTarget::PropertyGet(pending.identity)) {
        return Err(Error::internal(
            "property reply belongs to another operation",
        ));
    }
    let realm = parent.executable.realm;
    let result = (|| {
        let step = pending
            .resume
            .resume(runtime, completion)
            .map_err(runtime_error_to_vm_error)?;
        advance(
            runtime,
            execution,
            target.frame,
            pending.identity,
            pending.parents,
            step,
            pending.finish,
        )
    })();
    finish_error(runtime, realm, result)
}

fn finish_error(
    runtime: &Runtime,
    realm: crate::engine::heap::ContextId,
    result: Result<Progress, Error>,
) -> Result<Progress, Error> {
    match result {
        Ok(step) => Ok(step),
        Err(error) => {
            super::property_driver::throw_error(runtime, realm, error).map(Progress::Call)
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn advance(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    frame: FrameId,
    identity: u64,
    mut parents: Vec<Resume>,
    mut step: Step,
    finish: Finish,
) -> Result<Progress, Error> {
    let realm = execution.frames.current_mut(frame)?.executable.realm;
    loop {
        let (target, receiver, arguments, resume) = match step {
            Step::Complete(completion) => {
                if let Some(parent) = parents.pop() {
                    step = parent
                        .resume(runtime, completion)
                        .map_err(runtime_error_to_vm_error)?;
                    continue;
                }
                let _depth = match finish {
                    Finish::PropertyRead(depth) => depth,
                    Finish::Call { depth, tail } => {
                        if tail {
                            #[cfg(feature = "profiling")]
                            crate::engine::api::profiling::record_owned_instruction(depth);
                            return Ok(Progress::Call(CallStep::Complete(completion)));
                        }
                        depth
                    }
                    Finish::Conversion(wait) => {
                        return super::conversion_driver::ConversionTask::from_wait(
                            runtime, frame, wait, completion,
                        )
                        .map(Progress::Conversion);
                    }
                };
                return match completion {
                    Completion::Return(value) => {
                        let parent = execution.frames.current_mut(frame)?;
                        execution.slots.push(&mut parent.window, value)?;
                        parent.resume_pc = parent
                            .fault_pc
                            .checked_add(1)
                            .ok_or_else(|| Error::internal("property resume PC overflow"))?;
                        #[cfg(feature = "profiling")]
                        crate::engine::api::profiling::record_owned_instruction(_depth);
                        Ok(Progress::Call(CallStep::Entered))
                    }
                    completion => Ok(Progress::Call(CallStep::Complete(completion))),
                };
            }
            Step::OwnComplete(descriptor) => {
                let parent = parents
                    .pop()
                    .ok_or_else(|| Error::internal("descriptor reply has no parent operation"))?;
                step = parent
                    .descriptor(runtime, descriptor)
                    .map_err(runtime_error_to_vm_error)?;
                continue;
            }
            Step::BooleanComplete(result) => {
                let resume = parents
                    .pop()
                    .ok_or_else(|| Error::internal("boolean reply has no parent operation"))?;
                step = resume
                    .boolean(runtime, result)
                    .map_err(runtime_error_to_vm_error)?;
                continue;
            }
            Step::Extensible { object, resume } => {
                if runtime
                    .is_proxy_object(&object)
                    .map_err(runtime_error_to_vm_error)?
                {
                    parents
                        .try_reserve(1)
                        .map_err(|_| Error::internal("property continuation allocation failed"))?;
                    parents.push(resume);
                    step = ProxyBooleanStep::start(
                        runtime,
                        realm,
                        object,
                        ProxyBooleanKind::Extensible,
                    )
                    .map_err(runtime_error_to_vm_error)?
                    .into();
                    continue;
                }
                let result = runtime
                    .is_extensible(&object)
                    .map_err(runtime_error_to_vm_error)?;
                step = resume
                    .boolean(runtime, NativeConversion::Value(result))
                    .map_err(runtime_error_to_vm_error)?;
                continue;
            }
            Step::Convert { value, resume } => {
                parents
                    .try_reserve(1)
                    .map_err(|_| Error::internal("property continuation allocation failed"))?;
                parents.push(Resume::Own(resume));
                step = DescriptorStep::start(runtime, realm, value)
                    .map_err(runtime_error_to_vm_error)?
                    .into();
                continue;
            }
            Step::Converted(result) => {
                let Some(Resume::Own(resume)) = parents.pop() else {
                    return Err(Error::internal(
                        "converted descriptor has no matching property operation",
                    ));
                };
                step = resume
                    .converted(runtime, result)
                    .map_err(runtime_error_to_vm_error)?
                    .into();
                continue;
            }
            Step::Has {
                object,
                key,
                resume,
            } => {
                match runtime
                    .prepare_has_property(&object, &key)
                    .map_err(runtime_error_to_vm_error)?
                {
                    PreparedHas::Complete(value) => {
                        step = resume
                            .boolean(runtime, NativeConversion::Value(value))
                            .map_err(runtime_error_to_vm_error)?
                    }
                    PreparedHas::Proxy(object) => {
                        parents.try_reserve(1).map_err(|_| {
                            Error::internal("property continuation allocation failed")
                        })?;
                        parents.push(resume);
                        step = ProxyBooleanStep::start(
                            runtime,
                            realm,
                            object,
                            ProxyBooleanKind::Has(key),
                        )
                        .map_err(runtime_error_to_vm_error)?
                        .into();
                    }
                }
                continue;
            }
            Step::Read {
                object,
                key,
                receiver,
                resume,
            } => {
                match runtime
                    .prepare_ordinary_read(&object, &key, receiver)
                    .map_err(runtime_error_to_vm_error)?
                {
                    OrdinaryRead::Complete(value) => {
                        step = resume
                            .resume(
                                runtime,
                                Completion::Return(value.unwrap_or(Value::Undefined)),
                            )
                            .map_err(runtime_error_to_vm_error)?;
                        continue;
                    }
                    OrdinaryRead::Call { getter, receiver } => (
                        DirectCallTarget::Callable(getter),
                        receiver,
                        Vec::new(),
                        resume,
                    ),
                    OrdinaryRead::Special {
                        object, receiver, ..
                    } => {
                        if !execution.frames.can_push_with_continuations(parents.len()) {
                            step = resume
                                .resume(runtime, overflow(runtime, realm)?)
                                .map_err(runtime_error_to_vm_error)?;
                            continue;
                        }
                        parents.try_reserve(1).map_err(|_| {
                            Error::internal("property continuation allocation failed")
                        })?;
                        parents.push(resume);
                        step = ProxyGetStep::start(runtime, realm, object, key, receiver)
                            .map_err(runtime_error_to_vm_error)?
                            .into();
                        continue;
                    }
                }
            }
            Step::Call {
                target,
                receiver,
                arguments,
                resume,
            } => (target, receiver, arguments, resume),
            Step::Descriptor {
                object,
                key,
                resume,
            } => {
                if runtime
                    .is_proxy_object(&object)
                    .map_err(runtime_error_to_vm_error)?
                {
                    parents
                        .try_reserve(1)
                        .map_err(|_| Error::internal("property continuation allocation failed"))?;
                    parents.push(resume);
                    step = ProxyOwnStep::start(runtime, realm, object, key)
                        .map_err(runtime_error_to_vm_error)?
                        .into();
                    continue;
                }
                let descriptor = runtime
                    .internal_get_own_property(realm, &object, &key)
                    .map_err(runtime_error_to_vm_error)?;
                step = resume
                    .descriptor(runtime, descriptor)
                    .map_err(runtime_error_to_vm_error)?;
                continue;
            }
        };
        let callable = match target {
            DirectCallTarget::Callable(callable) => callable,
            DirectCallTarget::NonCallableProxy(proxy) => {
                if !execution.frames.can_push_with_continuations(parents.len()) {
                    step = resume
                        .resume(runtime, overflow(runtime, realm)?)
                        .map_err(runtime_error_to_vm_error)?;
                    continue;
                }
                parents
                    .try_reserve(1)
                    .map_err(|_| Error::internal("property continuation allocation failed"))?;
                parents.push(resume);
                step = crate::engine::object::ProxyCallStep::start(
                    runtime, realm, proxy, receiver, arguments,
                )
                .map_err(runtime_error_to_vm_error)?
                .into();
                continue;
            }
        };
        let super::call::NormalizedCallback {
            callable,
            receiver,
            arguments,
            classification,
        } = match super::call::normalize_callback(runtime, realm, callable, receiver, arguments)? {
            NativeConversion::Value(call) => call,
            NativeConversion::Throw(value) => {
                step = resume
                    .resume(runtime, Completion::Throw(value))
                    .map_err(runtime_error_to_vm_error)?;
                continue;
            }
        };
        if matches!(classification, CallableExecution::Proxy) {
            if !execution.frames.can_push_with_continuations(parents.len()) {
                step = resume
                    .resume(runtime, overflow(runtime, realm)?)
                    .map_err(runtime_error_to_vm_error)?;
                continue;
            }
            parents
                .try_reserve(1)
                .map_err(|_| Error::internal("property continuation allocation failed"))?;
            parents.push(resume);
            step = crate::engine::object::ProxyCallStep::start(
                runtime,
                realm,
                callable.as_object().clone(),
                receiver,
                arguments,
            )
            .map_err(runtime_error_to_vm_error)?
            .into();
            continue;
        }
        if let CallableExecution::Bytecode {
            bytecode,
            closure_slots,
        } = classification
        {
            let kind = runtime
                .0
                .state
                .borrow()
                .heap
                .function_bytecode(bytecode.bytecode_id())
                .map_err(|error| Error::internal(error.to_string()))?
                .metadata
                .function_kind;
            if kind == FunctionKind::Normal {
                if !execution.frames.can_push_with_continuations(parents.len())
                    || runtime.bytecode_call_would_overflow()
                {
                    let completion = runtime
                        .bytecode_stack_overflow_completion(realm, &bytecode)
                        .map_err(runtime_error_to_vm_error)?;
                    step = resume
                        .resume(runtime, completion)
                        .map_err(runtime_error_to_vm_error)?;
                    continue;
                }
                let request = BytecodeCallRequest {
                    callable,
                    receiver,
                    arguments,
                    bytecode,
                    closure_slots,
                    new_target: Value::Undefined,
                    caller_realm: realm,
                    return_to: ReturnTarget {
                        frame,
                        value_use: ReturnValue::Push,
                        tail: false,
                        operation: Some(OperationTarget::PropertyGet(identity)),
                    },
                };
                let entry = request.prepare(runtime)?;
                let parent = execution.frames.current_mut(frame)?;
                if parent.cold.property_wait.is_some() {
                    return Err(Error::internal(
                        "property operation overwrote a pending reply",
                    ));
                }
                parent.cold.property_wait = Some(Box::new(PendingProxyGet {
                    identity,
                    parents,
                    resume,
                    finish,
                }));
                push_frame(execution, entry)?;
                return Ok(Progress::Call(CallStep::Entered));
            }
        }
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_sync_call_bridge();
        let completion = runtime
            .call_internal(realm, &callable, receiver, &arguments)
            .map_err(runtime_error_to_vm_error)?;
        step = resume
            .resume(runtime, completion)
            .map_err(runtime_error_to_vm_error)?;
    }
}

fn overflow(runtime: &Runtime, realm: crate::engine::heap::ContextId) -> Result<Completion, Error> {
    Ok(Completion::Throw(
        runtime
            .new_native_error(
                realm,
                crate::engine::api::error::NativeErrorKind::Internal,
                "stack overflow",
            )
            .map_err(runtime_error_to_vm_error)?,
    ))
}
