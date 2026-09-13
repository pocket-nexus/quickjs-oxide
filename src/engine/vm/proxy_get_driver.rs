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

use crate::engine::object::{ProxyPrototypeKind, ProxyPrototypeStep};

mod request;
use request::{Resume, Step};

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
    Write {
        key: PropertyKey,
        strict: bool,
        depth: usize,
    },
    PropertyRead(usize),
    Call {
        depth: usize,
        tail: bool,
    },
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

/// A super lookup retains its frozen base independently of the getter receiver.
#[allow(clippy::too_many_arguments)]
pub(super) fn start_owned_read(
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
    let step = Step::Read {
        object: object.clone(),
        key,
        receiver,
        resume: Resume::ReadOwner(object),
    };
    let result = advance(
        runtime,
        execution,
        frame,
        identity,
        Vec::new(),
        step,
        Finish::PropertyRead(depth),
    );
    match finish_error(runtime, realm, result)? {
        Progress::Call(step) => Ok(step),
        Progress::Conversion(_) => Err(Error::internal("super read returned a conversion")),
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn start_boolean(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    frame: FrameId,
    object: ObjectRef,
    kind: ProxyBooleanKind,
    strict_delete: bool,
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
    let resume = Resume::BooleanResult {
        _object: object.clone(),
        _key: match &kind {
            ProxyBooleanKind::Has(key) | ProxyBooleanKind::Delete(key) => Some(key.clone()),
            _ => None,
        },
        strict_delete,
    };
    let step = match kind {
        ProxyBooleanKind::Has(key) => Step::Has {
            object,
            key,
            resume,
        },
        ProxyBooleanKind::Delete(key) => Step::Delete {
            object,
            key,
            resume,
        },
        ProxyBooleanKind::Extensible => Step::Extensible { object, resume },
        ProxyBooleanKind::PreventExtensions => Step::PreventExtensions { object, resume },
    };
    let result = advance(
        runtime,
        execution,
        frame,
        identity,
        Vec::new(),
        step,
        Finish::PropertyRead(depth),
    );
    match finish_error(runtime, realm, result)? {
        Progress::Call(step) => Ok(step),
        Progress::Conversion(_) => Err(Error::internal("boolean query returned a conversion")),
    }
}

/// The query entry is also used to validate the protocol before native entry migration.
#[cfg(test)]
pub(super) fn start_prototype(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    frame: FrameId,
    object: ObjectRef,
    kind: ProxyPrototypeKind,
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
        let mut parents = Vec::new();
        parents
            .try_reserve(1)
            .map_err(|_| Error::internal("property continuation allocation failed"))?;
        parents.push(Resume::ReadOwner(object.clone()));
        let step = ProxyPrototypeStep::start(runtime, realm, object, kind)
            .map_err(runtime_error_to_vm_error)?;
        advance(
            runtime,
            execution,
            frame,
            identity,
            parents,
            step.into(),
            Finish::PropertyRead(0),
        )
    })();
    match finish_error(runtime, realm, result)? {
        Progress::Call(step) => Ok(step),
        Progress::Conversion(_) => Err(Error::internal("prototype query returned a conversion")),
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

#[allow(clippy::too_many_arguments)]
pub(super) fn start_write(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    frame: FrameId,
    object: ObjectRef,
    key: PropertyKey,
    value: Value,
    receiver: Value,
    strict: bool,
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
        let step = crate::engine::object::SetStep::start(
            runtime,
            Some(realm),
            object,
            key.clone(),
            value,
            receiver,
        )
        .map_err(runtime_error_to_vm_error)?;
        advance(
            runtime,
            execution,
            frame,
            identity,
            Vec::new(),
            step.into(),
            Finish::Write { key, strict, depth },
        )
    })();
    match finish_error(runtime, realm, result)? {
        Progress::Call(step) => Ok(step),
        Progress::Conversion(_) => Err(Error::internal("Set returned a conversion operation")),
    }
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
                let (_depth, push) = match finish {
                    Finish::Write { depth, .. } => (depth, false),
                    Finish::PropertyRead(depth) => (depth, true),
                    Finish::Call { depth, tail } => {
                        if tail {
                            #[cfg(feature = "profiling")]
                            crate::engine::api::profiling::record_owned_instruction(depth);
                            return Ok(Progress::Call(CallStep::Complete(completion)));
                        }
                        (depth, true)
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
                        if push {
                            execution.slots.push(&mut parent.window, value)?;
                        }
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
            Step::SetContinue(resume) => {
                step = resume
                    .advance(runtime)
                    .map_err(runtime_error_to_vm_error)?
                    .into();
                continue;
            }
            Step::SetLength { value, resume } => {
                parents
                    .try_reserve(1)
                    .map_err(|_| Error::internal("property continuation allocation failed"))?;
                parents.push(Resume::SetLength(resume));
                step = crate::engine::object::ArrayLengthStep::start(runtime, Some(realm), value)
                    .map_err(runtime_error_to_vm_error)?
                    .into();
                continue;
            }
            Step::Number { value, resume } => {
                parents
                    .try_reserve(1)
                    .map_err(|_| Error::internal("property continuation allocation failed"))?;
                parents.push(Resume::LengthNumber(resume));
                step = crate::engine::value::conversion::number::NumberStep::start(
                    runtime, realm, value,
                )
                .map_err(runtime_error_to_vm_error)?
                .into();
                continue;
            }
            Step::NumberComplete(result) => {
                let Some(Resume::LengthNumber(resume)) = parents.pop() else {
                    return Err(Error::internal(
                        "ToNumber result has no matching continuation",
                    ));
                };
                step = resume
                    .number(runtime, result)
                    .map_err(runtime_error_to_vm_error)?
                    .into();
                continue;
            }
            Step::LengthComplete(result) => {
                let resume = parents
                    .pop()
                    .ok_or_else(|| Error::internal("Array length result has no parent"))?;
                step = resume
                    .length(runtime, result)
                    .map_err(runtime_error_to_vm_error)?;
                continue;
            }
            Step::SetSpecial {
                object,
                key,
                value,
                receiver,
                resume,
            } => {
                match runtime
                    .prepare_typed_array_set(&object, &key, &value, &receiver)
                    .map_err(runtime_error_to_vm_error)?
                {
                    None => {
                        step = resume
                            .special(runtime, None)
                            .map_err(runtime_error_to_vm_error)?
                            .into()
                    }
                    Some(request) => {
                        parents.try_reserve(1).map_err(|_| {
                            Error::internal("property continuation allocation failed")
                        })?;
                        parents.push(Resume::SetTyped(resume));
                        step = request.into();
                    }
                }
                continue;
            }
            Step::Element {
                element,
                value,
                resume,
            } => {
                parents
                    .try_reserve(1)
                    .map_err(|_| Error::internal("property continuation allocation failed"))?;
                parents.push(Resume::TypedElement(resume));
                step = crate::engine::builtins::ElementStep::start(runtime, realm, element, value)
                    .map_err(runtime_error_to_vm_error)?
                    .into();
                continue;
            }
            Step::ElementComplete(result) => {
                let Some(Resume::TypedElement(resume)) = parents.pop() else {
                    return Err(Error::internal(
                        "element result has no matching continuation",
                    ));
                };
                step = resume
                    .element(runtime, result)
                    .map_err(runtime_error_to_vm_error)?
                    .into();
                continue;
            }
            Step::TypedComplete(result) => {
                let resume = parents
                    .pop()
                    .ok_or_else(|| Error::internal("TypedArray result has no parent"))?;
                step = resume
                    .typed(runtime, result)
                    .map_err(runtime_error_to_vm_error)?;
                continue;
            }
            Step::SetComplete(action) => {
                if let crate::engine::object::operations::PropertySetAction::Call {
                    setter,
                    receiver,
                    argument,
                } = action
                {
                    step = Step::Call {
                        target: DirectCallTarget::Callable(setter),
                        receiver,
                        arguments: vec![argument],
                        resume: Resume::Setter,
                    };
                    continue;
                }
                if let Some(resume) = parents.pop() {
                    step = resume
                        .set(runtime, action)
                        .map_err(runtime_error_to_vm_error)?;
                    continue;
                }
                let Finish::Write { key, strict, .. } = &finish else {
                    return Err(Error::internal("Set result has no assignment owner"));
                };
                step = Step::Complete(
                    runtime
                        .finish_property_set(
                            request::set_result(action).map_err(runtime_error_to_vm_error)?,
                            key,
                            *strict,
                        )
                        .map_err(runtime_error_to_vm_error)?,
                );
                continue;
            }
            Step::Set {
                object,
                key,
                value,
                receiver,
                resume,
            } => {
                if !execution.frames.can_push_with_continuations(parents.len()) {
                    let Completion::Throw(value) = overflow(runtime, realm)? else {
                        unreachable!()
                    };
                    step = resume
                        .set(
                            runtime,
                            crate::engine::object::operations::PropertySetAction::Throw(value),
                        )
                        .map_err(runtime_error_to_vm_error)?;
                    continue;
                }
                parents
                    .try_reserve(1)
                    .map_err(|_| Error::internal("property continuation allocation failed"))?;
                parents.push(resume);
                step = crate::engine::object::SetStep::start(
                    runtime,
                    Some(realm),
                    object,
                    key,
                    value,
                    receiver,
                )
                .map_err(runtime_error_to_vm_error)?
                .into();
                continue;
            }
            Step::SetProxy {
                object,
                key,
                value,
                receiver,
                resume,
            } => {
                if !execution.frames.can_push_with_continuations(parents.len()) {
                    let Completion::Throw(value) = overflow(runtime, realm)? else {
                        unreachable!()
                    };
                    step = resume
                        .set(
                            runtime,
                            crate::engine::object::operations::PropertySetAction::Throw(value),
                        )
                        .map_err(runtime_error_to_vm_error)?;
                    continue;
                }
                parents
                    .try_reserve(1)
                    .map_err(|_| Error::internal("property continuation allocation failed"))?;
                parents.push(resume);
                step = crate::engine::object::ProxySetStep::start(
                    runtime, realm, object, key, value, receiver,
                )
                .map_err(runtime_error_to_vm_error)?
                .into();
                continue;
            }
            Step::Defined(result) => {
                let resume = parents
                    .pop()
                    .ok_or_else(|| Error::internal("Define result has no parent"))?;
                step = resume
                    .defined(runtime, result)
                    .map_err(runtime_error_to_vm_error)?;
                continue;
            }
            Step::Define {
                object,
                key,
                descriptor,
                resume,
            } => {
                if runtime
                    .is_proxy_object(&object)
                    .map_err(runtime_error_to_vm_error)?
                {
                    if !execution.frames.can_push_with_continuations(parents.len()) {
                        let Completion::Throw(value) = overflow(runtime, realm)? else {
                            unreachable!()
                        };
                        step = resume
                            .defined(runtime, NativeConversion::Throw(value))
                            .map_err(runtime_error_to_vm_error)?;
                        continue;
                    }
                    parents
                        .try_reserve(1)
                        .map_err(|_| Error::internal("property continuation allocation failed"))?;
                    parents.push(resume);
                    step = crate::engine::object::ProxyDefineStep::start(
                        runtime, realm, object, key, descriptor,
                    )
                    .map_err(runtime_error_to_vm_error)?
                    .into();
                    continue;
                }
                if let Some(length) = runtime
                    .prepare_array_length_definition(Some(realm), &object, &key, &descriptor)
                    .map_err(runtime_error_to_vm_error)?
                {
                    parents
                        .try_reserve(1)
                        .map_err(|_| Error::internal("property continuation allocation failed"))?;
                    parents.push(Resume::DefineLength {
                        object,
                        key,
                        descriptor,
                        resume: Box::new(resume),
                    });
                    step = length.into();
                    continue;
                }
                if let Some(request) = runtime
                    .prepare_typed_array_definition(&object, &key, &descriptor)
                    .map_err(runtime_error_to_vm_error)?
                {
                    parents
                        .try_reserve(1)
                        .map_err(|_| Error::internal("property continuation allocation failed"))?;
                    parents.push(Resume::DefineTyped {
                        object,
                        _descriptor: descriptor,
                        resume: Box::new(resume),
                    });
                    step = request.into();
                    continue;
                }
                let result = runtime
                    .internal_define_own_property(realm, &object, &key, &descriptor)
                    .map_err(runtime_error_to_vm_error)?;
                step = resume
                    .defined(runtime, result)
                    .map_err(runtime_error_to_vm_error)?;
                continue;
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
            Step::GetPrototype { object, resume } => {
                if runtime
                    .is_proxy_object(&object)
                    .map_err(runtime_error_to_vm_error)?
                {
                    if !execution.frames.can_push_with_continuations(parents.len()) {
                        let Completion::Throw(value) = overflow(runtime, realm)? else {
                            unreachable!()
                        };
                        step = resume
                            .prototype(runtime, NativeConversion::Throw(value))
                            .map_err(runtime_error_to_vm_error)?;
                        continue;
                    }
                    parents
                        .try_reserve(1)
                        .map_err(|_| Error::internal("property continuation allocation failed"))?;
                    parents.push(Resume::PrototypeGetReply(Box::new(resume)));
                    step =
                        ProxyPrototypeStep::start(runtime, realm, object, ProxyPrototypeKind::Get)
                            .map_err(runtime_error_to_vm_error)?
                            .into();
                } else {
                    let result = runtime
                        .get_prototype_of(&object)
                        .map_err(runtime_error_to_vm_error)?;
                    step = resume
                        .prototype(runtime, NativeConversion::Value(result))
                        .map_err(runtime_error_to_vm_error)?;
                }
                continue;
            }
            Step::SetPrototype {
                object,
                prototype,
                resume,
            } => {
                if runtime
                    .is_proxy_object(&object)
                    .map_err(runtime_error_to_vm_error)?
                {
                    if !execution.frames.can_push_with_continuations(parents.len()) {
                        let Completion::Throw(value) = overflow(runtime, realm)? else {
                            unreachable!()
                        };
                        step = resume
                            .boolean(runtime, NativeConversion::Throw(value))
                            .map_err(runtime_error_to_vm_error)?;
                        continue;
                    }
                    parents
                        .try_reserve(1)
                        .map_err(|_| Error::internal("property continuation allocation failed"))?;
                    parents.push(Resume::PrototypeSetReply(Box::new(resume)));
                    step = ProxyPrototypeStep::start(
                        runtime,
                        realm,
                        object,
                        ProxyPrototypeKind::Set(prototype),
                    )
                    .map_err(runtime_error_to_vm_error)?
                    .into();
                } else {
                    let result = runtime
                        .set_prototype_of(&object, prototype.as_ref())
                        .map_err(runtime_error_to_vm_error)?;
                    step = resume
                        .boolean(runtime, NativeConversion::Value(result))
                        .map_err(runtime_error_to_vm_error)?;
                }
                continue;
            }
            Step::Delete {
                object,
                key,
                resume,
            } => {
                if runtime
                    .is_proxy_object(&object)
                    .map_err(runtime_error_to_vm_error)?
                {
                    if !execution.frames.can_push_with_continuations(parents.len()) {
                        let Completion::Throw(value) = overflow(runtime, realm)? else {
                            unreachable!()
                        };
                        step = resume
                            .boolean(runtime, NativeConversion::Throw(value))
                            .map_err(runtime_error_to_vm_error)?;
                        continue;
                    }
                    parents
                        .try_reserve(1)
                        .map_err(|_| Error::internal("property continuation allocation failed"))?;
                    parents.push(resume);
                    step = ProxyBooleanStep::start(
                        runtime,
                        realm,
                        object,
                        ProxyBooleanKind::Delete(key),
                    )
                    .map_err(runtime_error_to_vm_error)?
                    .into();
                } else {
                    let result = runtime
                        .delete_property(&object, &key)
                        .map_err(runtime_error_to_vm_error)?;
                    step = resume
                        .boolean(runtime, NativeConversion::Value(result))
                        .map_err(runtime_error_to_vm_error)?;
                }
                continue;
            }
            Step::PreventExtensions { object, resume } => {
                if runtime
                    .is_proxy_object(&object)
                    .map_err(runtime_error_to_vm_error)?
                {
                    if !execution.frames.can_push_with_continuations(parents.len()) {
                        let Completion::Throw(value) = overflow(runtime, realm)? else {
                            unreachable!()
                        };
                        step = resume
                            .boolean(runtime, NativeConversion::Throw(value))
                            .map_err(runtime_error_to_vm_error)?;
                        continue;
                    }
                    parents
                        .try_reserve(1)
                        .map_err(|_| Error::internal("property continuation allocation failed"))?;
                    parents.push(resume);
                    step = ProxyBooleanStep::start(
                        runtime,
                        realm,
                        object,
                        ProxyBooleanKind::PreventExtensions,
                    )
                    .map_err(runtime_error_to_vm_error)?
                    .into();
                } else {
                    let result = runtime
                        .internal_prevent_extensions(realm, &object)
                        .map_err(runtime_error_to_vm_error)?;
                    step = resume
                        .boolean(runtime, result)
                        .map_err(runtime_error_to_vm_error)?;
                }
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
