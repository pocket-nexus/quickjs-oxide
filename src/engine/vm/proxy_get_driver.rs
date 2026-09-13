//! Schedule typed domain requests and their JavaScript child frames.
//! Domain owners retain algorithms; this driver owns reply routing and roots.
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

mod construct;
mod request;
use request::{Resume, Step};

pub(super) struct PendingProxyGet {
    identity: u64,
    // The innermost domain guard must leave before its native activation.
    resume: Resume,
    query: Query,
}

impl PendingProxyGet {
    pub(super) fn continuation_depth(&self) -> usize {
        self.query.continuation_depth()
    }
}

struct Query {
    realm: crate::engine::heap::ContextId,
    parents: Vec<Resume>,
    natives: Vec<NativeScope>,
    finish: Option<Finish>,
}
struct NativeScope {
    call: super::call::PreparedNativeCall,
    parents: Vec<Resume>,
    resume: Resume,
    parent_realm: crate::engine::heap::ContextId,
}
impl Query {
    fn continuation_depth(&self) -> usize {
        self.natives
            .iter()
            .fold(self.parents.len(), |depth, scope| {
                depth.saturating_add(1 + scope.parents.len())
            })
    }
    fn finish_native(
        &mut self,
        runtime: &Runtime,
        result: Result<Completion, Error>,
    ) -> Result<Step, Error> {
        self.finish_native_outcome(
            runtime,
            result.map(super::call::NativeInvokeOutcome::Completion),
        )
    }
    fn finish_native_outcome(
        &mut self,
        runtime: &Runtime,
        result: Result<super::call::NativeInvokeOutcome, Error>,
    ) -> Result<Step, Error> {
        let scope = self
            .natives
            .pop()
            .ok_or_else(|| Error::internal("native result has no scope"))?;
        while self.parents.pop().is_some() {}
        self.parents = scope.parents;
        self.realm = scope.parent_realm;
        // Ordinary next calls materialize the result while the native frame is active.
        // Raw direct next keeps value/done and uses the calling realm.
        let result = result
            .map_err(crate::engine::api::runtime_error::RuntimeError::Engine)
            .and_then(|result| match (scope.call.activation.mode, result) {
                (
                    super::call::NativeInvokeMode::Ordinary,
                    super::call::NativeInvokeOutcome::IteratorNextRaw { value, done },
                ) => Ok(super::call::NativeInvokeOutcome::Completion(
                    Completion::Return(Value::Object(runtime.new_iterator_result(
                        scope.call.activation.realm,
                        value,
                        done,
                    )?)),
                )),
                (_, result) => Ok(result),
            });
        let result = scope
            .call
            .activation
            .finish(result)
            .map_err(runtime_error_to_vm_error)?;
        scope
            .resume
            .native(runtime, result)
            .map_err(runtime_error_to_vm_error)
    }
}
impl Drop for Query {
    fn drop(&mut self) {
        // Current domain states belong to the innermost native activation.
        // Each saved resume/parent stack belongs to its caller, outside that
        // activation; release them before proceeding to the next outer scope.
        while self.parents.pop().is_some() {}
        while let Some(mut scope) = self.natives.pop() {
            drop(scope.call);
            drop(scope.resume);
            while scope.parents.pop().is_some() {}
        }
    }
}

enum Next {
    Done(Progress),
    Call {
        entry: super::frame::FrameEntry,
        resume: Resume,
    },
}

enum Finish {
    ForIn(usize),
    Class(Box<super::construct_driver::PendingClass>),
    Numeric(usize),
    VmCall(ReturnValue),
    Discard(usize),
    Iterator(Box<super::iterator_driver::PendingIterator>),
    IteratorNext(Box<super::iterator_driver::PendingIterator>),
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

#[allow(clippy::too_many_arguments)]
pub(super) fn start_native_call(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    frame: FrameId,
    callable: crate::engine::object::CallableRef,
    receiver: Value,
    arguments: Vec<Value>,
    tail: bool,
    depth: usize,
) -> Result<CallStep, Error> {
    match start_owned_callback(
        runtime,
        execution,
        frame,
        callable,
        receiver,
        arguments,
        Finish::Call { depth, tail },
    )? {
        Progress::Call(step) => Ok(step),
        Progress::Conversion(_) => Err(Error::internal("native call returned a conversion")),
    }
}

pub(super) fn start_apply(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    frame: FrameId,
    kind: crate::engine::code::bytecode::ApplyKind,
) -> Result<CallStep, Error> {
    let realm = execution.frames.current_mut(frame)?.executable.realm;
    let result = (|| {
        let parent = execution.frames.current_mut(frame)?;
        let step = crate::engine::builtins::InvokeStep::start_spread(
            runtime,
            realm,
            kind,
            execution.slots.peek(&parent.window, 2)?.clone(),
            execution.slots.peek(&parent.window, 1)?.clone(),
            execution.slots.peek(&parent.window, 0)?.clone(),
        )
        .map_err(runtime_error_to_vm_error)?;
        start_instruction(runtime, execution, frame, step.into(), 3)
    })();
    match finish_error(runtime, realm, result)? {
        Progress::Call(step) => Ok(step),
        Progress::Conversion(_) => Err(Error::internal("Apply returned a conversion")),
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn start_construct(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    frame: FrameId,
    target: super::call::ConstructorRef,
    new_target: Value,
    arguments: Vec<Value>,
    operand_count: usize,
) -> Result<CallStep, Error> {
    let realm = execution.frames.current_mut(frame)?.executable.realm;
    let result = start_instruction(
        runtime,
        execution,
        frame,
        Step::Construct {
            target,
            new_target: super::call::ConstructNewTarget::Raw(new_target),
            arguments,
            resume: Resume::Identity,
        },
        operand_count,
    );
    match finish_error(runtime, realm, result)? {
        Progress::Call(step) => Ok(step),
        Progress::Conversion(_) => Err(Error::internal("Construct returned a conversion")),
    }
}

fn start_instruction(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    frame: FrameId,
    step: Step,
    operand_count: usize,
) -> Result<Progress, Error> {
    let parent = execution.frames.current_mut(frame)?;
    let identity = parent
        .cold
        .property_generation
        .checked_add(1)
        .ok_or_else(|| Error::internal("instruction operation identity exhausted"))?;
    parent.cold.property_generation = identity;
    let depth = execution.slots.depth(&parent.window);
    // The request owns every source value before any window owner is released.
    for _ in 0..operand_count {
        execution.slots.pop(&mut parent.window)?;
    }
    advance(
        runtime,
        execution,
        frame,
        identity,
        Vec::new(),
        step,
        Finish::Call { depth, tail: false },
    )
}

pub(super) fn start_native_conversion_call(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    frame: FrameId,
    callable: crate::engine::object::CallableRef,
    receiver: Value,
    arguments: Vec<Value>,
    wait: super::conversion_driver::ConversionWait,
) -> Result<Progress, Error> {
    start_owned_callback(
        runtime,
        execution,
        frame,
        callable,
        receiver,
        arguments,
        Finish::Conversion(wait),
    )
}

fn start_owned_callback(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    frame: FrameId,
    callable: crate::engine::object::CallableRef,
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
    let result = advance(
        runtime,
        execution,
        frame,
        identity,
        Vec::new(),
        Step::Call {
            target: DirectCallTarget::Callable(callable),
            receiver,
            arguments,
            resume: Resume::Identity,
        },
        finish,
    );
    finish_error(runtime, realm, result)
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
    let step = pending
        .resume
        .resume(runtime, completion)
        .map_err(runtime_error_to_vm_error);
    let result = drive(
        runtime,
        execution,
        target.frame,
        pending.identity,
        pending.query,
        step,
    );
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
    parents: Vec<Resume>,
    step: Step,
    finish: Finish,
) -> Result<Progress, Error> {
    let realm = execution.frames.current_mut(frame)?.executable.realm;
    drive(
        runtime,
        execution,
        frame,
        identity,
        Query {
            realm,
            parents,
            natives: Vec::new(),
            finish: Some(finish),
        },
        Ok(step),
    )
}

fn drive(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    frame: FrameId,
    identity: u64,
    mut query: Query,
    mut step: Result<Step, Error>,
) -> Result<Progress, Error> {
    loop {
        let result = step
            .and_then(|step| advance_inner(runtime, execution, frame, identity, &mut query, step));
        match result {
            Ok(Next::Done(result)) => return Ok(result),
            Ok(Next::Call { entry, resume }) => {
                let parent = execution.frames.current_mut(frame)?;
                if parent.cold.property_wait.is_some() {
                    return Err(Error::internal(
                        "property operation overwrote a pending reply",
                    ));
                }
                parent.cold.property_wait = Some(Box::new(PendingProxyGet {
                    identity,
                    query,
                    resume,
                }));
                match push_frame(execution, entry) {
                    Ok(_) => return Ok(Progress::Call(CallStep::Entered)),
                    Err(error) => {
                        let pending = execution
                            .frames
                            .current_mut(frame)?
                            .cold
                            .property_wait
                            .take()
                            .ok_or_else(|| Error::internal("failed child entry lost its query"))?;
                        drop(pending.resume);
                        query = pending.query;
                        step = Err(error);
                    }
                }
            }
            Err(error) if !query.natives.is_empty() => {
                // The native frame and all argv roots are still owned here.
                step = query.finish_native(runtime, Err(error));
            }
            Err(error) => return Err(error),
        }
    }
}

fn advance_inner(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    frame: FrameId,
    identity: u64,
    query: &mut Query,
    mut step: Step,
) -> Result<Next, Error> {
    loop {
        let realm = query.realm;
        let (target, receiver, arguments, resume) = match step {
            Step::Complete(completion) => {
                if let Some(parent) = query.parents.pop() {
                    step = parent
                        .resume(runtime, completion)
                        .map_err(runtime_error_to_vm_error)?;
                    continue;
                }
                if !query.natives.is_empty() {
                    step = query.finish_native(runtime, Ok(completion))?;
                    continue;
                }
                let (_depth, push) = match query
                    .finish
                    .take()
                    .ok_or_else(|| Error::internal("query lost its final continuation"))?
                {
                    Finish::Class(pending) => {
                        return super::construct_driver::finish_class_reply(
                            runtime, execution, *pending, completion,
                        )
                        .map(Progress::Call)
                        .map(Next::Done);
                    }
                    Finish::VmCall(value_use) => {
                        return match completion {
                            Completion::Return(value) => {
                                if matches!(value_use, ReturnValue::Push) {
                                    let parent = execution.frames.current_mut(frame)?;
                                    execution.slots.push(&mut parent.window, value)?;
                                }
                                Ok(Next::Done(Progress::Call(CallStep::Entered)))
                            }
                            completion => {
                                Ok(Next::Done(Progress::Call(CallStep::Complete(completion))))
                            }
                        };
                    }
                    Finish::Iterator(mut pending) => {
                        let action = pending.advance_query(runtime, Some(completion))?;
                        match continue_iterator(runtime, execution, query, pending, action)? {
                            IteratorProgress::Step(next) => {
                                step = next;
                                continue;
                            }
                            IteratorProgress::Done(result) => {
                                return Ok(Next::Done(Progress::Call(result)));
                            }
                        }
                    }
                    Finish::IteratorNext(_) => {
                        return Err(Error::internal("iterator next received untyped completion"));
                    }
                    Finish::ForIn(depth)
                    | Finish::Numeric(depth)
                    | Finish::Write { depth, .. }
                    | Finish::Discard(depth) => (depth, false),
                    Finish::PropertyRead(depth) => (depth, true),
                    Finish::Call { depth, tail } => {
                        if tail {
                            #[cfg(feature = "profiling")]
                            crate::engine::api::profiling::record_owned_instruction(depth);
                            return Ok(Next::Done(Progress::Call(CallStep::Complete(completion))));
                        }
                        (depth, true)
                    }
                    Finish::Conversion(wait) => {
                        return super::conversion_driver::ConversionTask::from_wait(
                            runtime, frame, wait, completion,
                        )
                        .map(Progress::Conversion)
                        .map(Next::Done);
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
                        Ok(Next::Done(Progress::Call(CallStep::Entered)))
                    }
                    completion => Ok(Next::Done(Progress::Call(CallStep::Complete(completion)))),
                };
            }
            Step::Construct {
                target,
                new_target,
                arguments,
                resume,
            } => {
                step = construct::start(
                    runtime, frame, identity, realm, target, new_target, arguments, resume,
                )?;
                continue;
            }
            Step::ConstructProxy {
                target,
                new_target,
                arguments,
                resume,
            } => {
                if !execution
                    .frames
                    .can_push_with_continuations(query.continuation_depth())
                {
                    step = resume
                        .resume(runtime, overflow(runtime, realm)?)
                        .map_err(runtime_error_to_vm_error)?;
                    continue;
                }
                query
                    .parents
                    .try_reserve(1)
                    .map_err(|_| Error::internal("constructor continuation allocation failed"))?;
                query.parents.push(resume);
                step = crate::engine::object::ProxyConstructStep::start(
                    runtime, realm, target, new_target, arguments,
                )
                .map_err(runtime_error_to_vm_error)?
                .into();
                continue;
            }
            Step::ConstructorReady {
                request,
                receiver,
                derived,
                resume,
            } => {
                match construct::ready(
                    runtime, execution, query, request, receiver, derived, resume,
                )? {
                    Ok(next) => return Ok(next),
                    Err(next) => {
                        step = next;
                        continue;
                    }
                }
            }
            Step::Native {
                callable,
                target,
                defining_realm,
                min_readable_args,
                mode,
                invocation,
                arguments,
                resume,
            } => {
                step = native_scope(
                    runtime,
                    execution,
                    query,
                    callable,
                    target,
                    defining_realm,
                    min_readable_args,
                    mode,
                    invocation,
                    arguments,
                    resume,
                )?;
                continue;
            }

            Step::RegExpSpecies { regexp, resume } => {
                query.parents.try_reserve(1).map_err(|_| {
                    Error::internal("RegExp species continuation allocation failed")
                })?;
                query.parents.push(resume);
                step = crate::engine::builtins::RegExpSpeciesStep::start(runtime, realm, regexp)
                    .map_err(runtime_error_to_vm_error)?
                    .into();
                continue;
            }
            Step::RegExpSpeciesComplete(result) => {
                let resume = query
                    .parents
                    .pop()
                    .ok_or_else(|| Error::internal("RegExp species lost parent"))?;
                step = resume
                    .regexp_species(runtime, result)
                    .map_err(runtime_error_to_vm_error)?;
                continue;
            }
            Step::ForInComplete { value, done } => {
                let Some(Finish::ForIn(_depth)) = query.finish.take() else {
                    return Err(Error::internal("for-in result lost its instruction"));
                };
                let parent = execution.frames.current_mut(frame)?;
                execution.slots.push(&mut parent.window, value)?;
                if let Some(done) = done {
                    execution
                        .slots
                        .push(&mut parent.window, Value::Bool(done))?;
                }
                parent.resume_pc = parent
                    .fault_pc
                    .checked_add(1)
                    .ok_or_else(|| Error::internal("for-in resume PC overflow"))?;
                #[cfg(feature = "profiling")]
                crate::engine::api::profiling::record_owned_instruction(_depth);
                return Ok(Next::Done(Progress::Call(CallStep::Entered)));
            }
            Step::NumericComplete { value, previous } => {
                let Some(Finish::Numeric(_depth)) = query.finish.take() else {
                    return Err(Error::internal("numeric result lost its instruction"));
                };
                let parent = execution.frames.current_mut(frame)?;
                if let Some(previous) = previous {
                    execution.slots.push(&mut parent.window, previous)?;
                }
                execution.slots.push(&mut parent.window, value)?;
                parent.resume_pc = parent
                    .fault_pc
                    .checked_add(1)
                    .ok_or_else(|| Error::internal("numeric resume PC overflow"))?;
                #[cfg(feature = "profiling")]
                crate::engine::api::profiling::record_owned_instruction(_depth);
                return Ok(Next::Done(Progress::Call(CallStep::Entered)));
            }
            Step::NumericHtmlDda { value, resume } => {
                step = resume
                    .html_dda(
                        runtime
                            .value_is_html_dda(&value)
                            .map_err(runtime_error_to_vm_error)?,
                    )?
                    .into();
                continue;
            }
            Step::IndirectEval { source, resume } => {
                step = match runtime
                    .prepare_indirect_string_eval(realm, &source)
                    .map_err(runtime_error_to_vm_error)?
                {
                    crate::engine::builtins::DirectEvalPreparation::Complete(completion) => resume
                        .resume(runtime, completion)
                        .map_err(runtime_error_to_vm_error)?,
                    crate::engine::builtins::DirectEvalPreparation::Ready {
                        callable,
                        this_value,
                    } => Step::Call {
                        target: DirectCallTarget::Callable(callable),
                        receiver: this_value,
                        arguments: Vec::new(),
                        resume,
                    },
                };
                continue;
            }
            Step::Aggregate { iterable, resume } => {
                query.parents.try_reserve(1).map_err(|_| {
                    Error::internal("AggregateError continuation allocation failed")
                })?;
                query.parents.push(resume);
                step = crate::engine::builtins::AggregateStep::start(runtime, realm, iterable)
                    .map_err(runtime_error_to_vm_error)?
                    .into();
                continue;
            }
            Step::OrdinaryPrimitive { object, hint } => {
                step = crate::engine::value::conversion::primitive::PrimitiveResume::ordinary(
                    runtime, realm, object, hint,
                )
                .map_err(runtime_error_to_vm_error)?
                .into();
                continue;
            }
            Step::ArraySpecies {
                source,
                length,
                resume,
            } => {
                query
                    .parents
                    .try_reserve(1)
                    .map_err(|_| Error::internal("Array species continuation allocation failed"))?;
                query.parents.push(resume);
                step = crate::engine::builtins::ArraySpeciesStep::start(
                    runtime, realm, &source, length,
                )
                .map_err(runtime_error_to_vm_error)?
                .into();
                continue;
            }
            Step::ArrayPush {
                object,
                value,
                resume,
            } => {
                query
                    .parents
                    .try_reserve(1)
                    .map_err(|_| Error::internal("Array push continuation allocation failed"))?;
                query.parents.push(resume);
                step = crate::engine::builtins::ArrayMutationStep::start_values(
                    runtime,
                    realm,
                    crate::engine::builtins::ArrayMutationKind::Push(
                        crate::engine::builtins::native::ArrayPushKind::Push,
                    ),
                    Value::Object(object),
                    vec![value],
                )
                .map_err(runtime_error_to_vm_error)?
                .into();
                continue;
            }
            Step::IteratorNext {
                iterator,
                method,
                resume,
            } => {
                query
                    .parents
                    .try_reserve(1)
                    .map_err(|_| Error::internal("iterator continuation allocation failed"))?;
                query.parents.push(resume);
                step = crate::engine::builtins::IteratorNextStep::start(
                    runtime, realm, iterator, method,
                )
                .map_err(runtime_error_to_vm_error)?
                .into();
                continue;
            }
            Step::IteratorNextComplete(result) => {
                if let Some(parent) = query.parents.pop() {
                    step = parent
                        .iterator_next(runtime, result)
                        .map_err(runtime_error_to_vm_error)?;
                    continue;
                }
                let Some(Finish::IteratorNext(mut pending)) = query.finish.take() else {
                    return Err(Error::internal("iterator lost its continuation"));
                };
                let action = pending.next_query(runtime, result)?;
                match continue_iterator(runtime, execution, query, pending, action)? {
                    IteratorProgress::Step(next) => {
                        step = next;
                        continue;
                    }
                    IteratorProgress::Done(result) => {
                        return Ok(Next::Done(Progress::Call(result)));
                    }
                }
            }

            Step::IteratorCall {
                callable,
                iterator,
                resume,
            } => {
                let metadata = runtime
                    .direct_native_callable_metadata(&callable)
                    .map_err(runtime_error_to_vm_error)?;
                if let Some((target, defining_realm, min_readable_args)) = metadata
                    && target.descriptor().cproto
                        == crate::engine::builtins::native::NativeCProto::IteratorNext
                {
                    step = Step::Native {
                        callable,
                        target,
                        defining_realm,
                        min_readable_args,
                        mode: super::call::NativeInvokeMode::IteratorNextRaw,
                        invocation: super::call::NativeInvocation::Call {
                            this_value: Value::Object(iterator),
                        },
                        arguments: Vec::new(),
                        resume: Resume::IteratorNext(resume),
                    };
                } else {
                    step = Step::Call {
                        target: DirectCallTarget::Callable(callable),
                        receiver: Value::Object(iterator),
                        arguments: Vec::new(),
                        resume: Resume::IteratorNext(resume),
                    };
                }
                continue;
            }
            Step::NativeRawComplete(result) => {
                if !query.parents.is_empty() {
                    return Err(Error::internal(
                        "raw native result escaped a child operation",
                    ));
                }
                step = query.finish_native_outcome(runtime, Ok(result))?;
                continue;
            }
            Step::IteratorClose {
                iterator,
                completion,
            } => {
                step = crate::engine::builtins::IteratorCloseStep::start(
                    runtime, realm, iterator, completion,
                )
                .map_err(runtime_error_to_vm_error)?
                .into();
                continue;
            }

            Step::String { value, resume } => {
                step = Step::Primitive {
                    value,
                    hint: crate::engine::vm::ToPrimitiveHint::String,
                    resume: Resume::StringValue {
                        realm,
                        resume: Box::new(resume),
                    },
                };
                continue;
            }
            Step::ObjectTag { receiver } => {
                step = crate::engine::builtins::ObjectStringStep::start(
                    runtime,
                    realm,
                    crate::engine::builtins::ObjectStringKind::Tag,
                    &super::call::NativeInvocation::Call {
                        this_value: receiver,
                    },
                )
                .map_err(runtime_error_to_vm_error)?
                .into();
                continue;
            }
            Step::RegExpExec {
                regexp,
                input,
                resume,
            } => {
                query
                    .parents
                    .try_reserve(1)
                    .map_err(|_| Error::internal("RegExp exec continuation allocation failed"))?;
                query.parents.push(resume);
                step = crate::engine::builtins::RegExpExecStep::abstract_exec(
                    runtime, realm, regexp, input,
                )
                .map_err(runtime_error_to_vm_error)?
                .into();
                continue;
            }
            Step::IteratorCloseWithResume {
                iterator,
                completion,
                resume,
            } => {
                query.parents.try_reserve(1).map_err(|_| {
                    Error::internal("iterator close continuation allocation failed")
                })?;
                query.parents.push(resume);
                step = crate::engine::builtins::IteratorCloseStep::start(
                    runtime, realm, iterator, completion,
                )
                .map_err(runtime_error_to_vm_error)?
                .into();
                continue;
            }

            Step::OrdinaryInstance {
                constructor,
                value,
                resume,
            } => {
                query
                    .parents
                    .try_reserve(1)
                    .map_err(|_| Error::internal("instance continuation allocation failed"))?;
                query.parents.push(resume);
                step = crate::engine::builtins::InstanceStep::ordinary(
                    runtime,
                    realm,
                    &constructor,
                    value,
                )
                .map_err(runtime_error_to_vm_error)?
                .into();
                continue;
            }
            Step::ParseIterator { result, resume } => {
                query.parents.try_reserve(1).map_err(|_| {
                    Error::internal("iterator parse continuation allocation failed")
                })?;
                query.parents.push(resume);
                step =
                    crate::engine::builtins::IteratorNextStep::parse_result(runtime, realm, result)
                        .map_err(runtime_error_to_vm_error)?
                        .into();
                continue;
            }
            Step::ArrayCopy {
                object,
                to,
                from,
                count,
                backwards,
                resume,
            } => {
                query
                    .parents
                    .try_reserve(1)
                    .map_err(|_| Error::internal("Array copy continuation allocation failed"))?;
                query.parents.push(resume);
                step = crate::engine::builtins::ArrayCopyStep::start(
                    runtime, realm, object, to, from, count, backwards,
                )
                .map_err(runtime_error_to_vm_error)?
                .into();
                continue;
            }

            Step::ConstructorSource { new_target, resume } => {
                query.parents.try_reserve(1).map_err(|_| {
                    Error::internal("constructor source continuation allocation failed")
                })?;
                query.parents.push(resume);
                step = super::call::prototype::ProtoSourceStep::start(runtime, realm, new_target)
                    .map_err(runtime_error_to_vm_error)?
                    .into();
                continue;
            }
            Step::ConstructorSourceComplete(result) => {
                let parent = query
                    .parents
                    .pop()
                    .ok_or_else(|| Error::internal("constructor source has no parent"))?;
                step = parent
                    .constructor_source(runtime, result)
                    .map_err(runtime_error_to_vm_error)?;
                continue;
            }
            Step::TypedSpeciesView {
                source,
                element,
                buffer,
                offset,
                length,
                resume,
            } => {
                query.parents.try_reserve(1).map_err(|_| {
                    Error::internal("typed species view continuation allocation failed")
                })?;
                query.parents.push(resume);
                step = crate::engine::builtins::TypedSpeciesStep::start_view(
                    runtime, realm, source, element, buffer, offset, length,
                )
                .map_err(runtime_error_to_vm_error)?
                .into();
                continue;
            }
            Step::TypedIteratorMethod { source, resume } => {
                query.parents.try_reserve(1).map_err(|_| {
                    Error::internal("typed iterator method continuation allocation failed")
                })?;
                query.parents.push(resume);
                step =
                    crate::engine::builtins::TypedIteratorMethodStep::start(runtime, realm, source)
                        .map_err(runtime_error_to_vm_error)?
                        .into();
                continue;
            }
            Step::TypedIteratorMethodComplete(result) => {
                let resume = query
                    .parents
                    .pop()
                    .ok_or_else(|| Error::internal("typed iterator method lost parent"))?;
                step = resume
                    .typed_iterator_method(runtime, result)
                    .map_err(runtime_error_to_vm_error)?;
                continue;
            }
            Step::TypedCollect {
                source,
                method,
                element,
                resume,
            } => {
                query.parents.try_reserve(1).map_err(|_| {
                    Error::internal("typed collection continuation allocation failed")
                })?;
                query.parents.push(resume);
                step = crate::engine::builtins::TypedCollectStep::start(
                    realm, source, method, element,
                )
                .into();
                continue;
            }
            Step::TypedCollectComplete(result) => {
                let resume = query
                    .parents
                    .pop()
                    .ok_or_else(|| Error::internal("typed collection lost parent"))?;
                step = resume
                    .typed_collected(runtime, result)
                    .map_err(runtime_error_to_vm_error)?;
                continue;
            }
            Step::TypedCreate {
                constructor,
                length,
                resume,
            } => {
                query.parents.try_reserve(1).map_err(|_| {
                    Error::internal("typed creation continuation allocation failed")
                })?;
                query.parents.push(resume);
                step = crate::engine::builtins::TypedSpeciesStep::create(
                    runtime,
                    realm,
                    constructor,
                    vec![Value::number(length as f64)],
                    Some(length),
                )
                .map_err(runtime_error_to_vm_error)?
                .into();
                continue;
            }
            Step::TypedSpecies {
                source,
                element,
                length,
                resume,
            } => {
                query.parents.try_reserve(1).map_err(|_| {
                    Error::internal("TypedArray species continuation allocation failed")
                })?;
                query.parents.push(resume);
                step = crate::engine::builtins::TypedSpeciesStep::start(
                    runtime, realm, source, element, length,
                )
                .map_err(runtime_error_to_vm_error)?
                .into();
                continue;
            }
            Step::TypedSpeciesComplete(result) => {
                let parent = query
                    .parents
                    .pop()
                    .ok_or_else(|| Error::internal("TypedArray species has no parent"))?;
                step = parent
                    .typed_species(runtime, result)
                    .map_err(runtime_error_to_vm_error)?;
                continue;
            }
            Step::Arguments { value, resume } => {
                query
                    .parents
                    .try_reserve(1)
                    .map_err(|_| Error::internal("argument continuation allocation failed"))?;
                query.parents.push(resume);
                step = crate::engine::builtins::ArgumentsStep::start(runtime, realm, value)
                    .map_err(runtime_error_to_vm_error)?
                    .into();
                continue;
            }
            Step::ArgumentsComplete(result) => {
                let parent = query
                    .parents
                    .pop()
                    .ok_or_else(|| Error::internal("argument list lost its continuation"))?;
                step = parent
                    .arguments(runtime, result)
                    .map_err(runtime_error_to_vm_error)?;
                continue;
            }
            Step::Primitive {
                value,
                hint,
                resume,
            } => {
                query
                    .parents
                    .try_reserve(1)
                    .map_err(|_| Error::internal("primitive continuation allocation failed"))?;
                query.parents.push(resume);
                step = crate::engine::value::conversion::primitive::PrimitiveResume::start(
                    runtime, realm, value, hint,
                )
                .into();
                continue;
            }
            Step::SnapshotEnumerable {
                object,
                key,
                resume,
            } => {
                if runtime
                    .is_proxy_object(&object)
                    .map_err(runtime_error_to_vm_error)?
                {
                    step = Step::Descriptor {
                        object,
                        key,
                        resume: Resume::OwnFlagReply {
                            enumerable: true,
                            resume: Box::new(resume),
                        },
                    };
                } else {
                    let result = runtime
                        .internal_snapshot_own_property_is_enumerable(realm, &object, &key)
                        .map_err(runtime_error_to_vm_error)?;
                    step = resume
                        .boolean(runtime, result)
                        .map_err(runtime_error_to_vm_error)?;
                }
                continue;
            }
            Step::OwnFlag {
                object,
                key,
                enumerable,
                resume,
            } => {
                if runtime
                    .is_proxy_object(&object)
                    .map_err(runtime_error_to_vm_error)?
                {
                    step = Step::Descriptor {
                        object,
                        key,
                        resume: Resume::OwnFlagReply {
                            enumerable,
                            resume: Box::new(resume),
                        },
                    };
                } else {
                    let result = if enumerable {
                        runtime.internal_own_property_is_enumerable(realm, &object, &key)
                    } else {
                        runtime.internal_has_own_property(realm, &object, &key)
                    }
                    .map_err(runtime_error_to_vm_error)?;
                    step = resume
                        .boolean(runtime, result)
                        .map_err(runtime_error_to_vm_error)?;
                }
                continue;
            }
            Step::Keys { object, resume } => {
                if runtime
                    .is_proxy_object(&object)
                    .map_err(runtime_error_to_vm_error)?
                {
                    if !execution
                        .frames
                        .can_push_with_continuations(query.continuation_depth())
                    {
                        let Completion::Throw(value) = overflow(runtime, realm)? else {
                            unreachable!()
                        };
                        step = resume
                            .keys(runtime, NativeConversion::Throw(value))
                            .map_err(runtime_error_to_vm_error)?;
                        continue;
                    }
                    query
                        .parents
                        .try_reserve(1)
                        .map_err(|_| Error::internal("ownKeys continuation allocation failed"))?;
                    query.parents.push(resume);
                    step = crate::engine::object::KeysStep::start(runtime, realm, object)
                        .map_err(runtime_error_to_vm_error)?
                        .into();
                } else {
                    let result = runtime
                        .own_property_keys(&object)
                        .map_err(runtime_error_to_vm_error)?;
                    step = resume
                        .keys(runtime, NativeConversion::Value(result))
                        .map_err(runtime_error_to_vm_error)?;
                }
                continue;
            }
            Step::KeysComplete(result) => {
                let resume = query
                    .parents
                    .pop()
                    .ok_or_else(|| Error::internal("ownKeys result has no parent"))?;
                step = resume
                    .keys(runtime, result)
                    .map_err(runtime_error_to_vm_error)?;
                continue;
            }
            Step::ReadValue {
                receiver,
                key,
                resume,
            } => {
                let read = runtime
                    .prepare_value_property_read(realm, receiver, &key)
                    .map_err(runtime_error_to_vm_error)?;
                step = Step::PreparedRead { read, key, resume };
                continue;
            }
            Step::SetContinue(resume) => {
                step = resume
                    .advance(runtime)
                    .map_err(runtime_error_to_vm_error)?
                    .into();
                continue;
            }
            Step::SetLength { value, resume } => {
                query
                    .parents
                    .try_reserve(1)
                    .map_err(|_| Error::internal("property continuation allocation failed"))?;
                query.parents.push(Resume::SetLength(resume));
                step = crate::engine::object::ArrayLengthStep::start(runtime, Some(realm), value)
                    .map_err(runtime_error_to_vm_error)?
                    .into();
                continue;
            }
            Step::Number { value, resume } => {
                query
                    .parents
                    .try_reserve(1)
                    .map_err(|_| Error::internal("property continuation allocation failed"))?;
                query.parents.push(resume);
                step = crate::engine::value::conversion::number::NumberStep::start(
                    runtime, realm, value,
                )
                .map_err(runtime_error_to_vm_error)?
                .into();
                continue;
            }
            Step::NumberComplete(result) => {
                let Some(resume) = query.parents.pop() else {
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
                let resume = query
                    .parents
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
                        query.parents.try_reserve(1).map_err(|_| {
                            Error::internal("property continuation allocation failed")
                        })?;
                        query.parents.push(Resume::SetTyped(resume));
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
                query
                    .parents
                    .try_reserve(1)
                    .map_err(|_| Error::internal("property continuation allocation failed"))?;
                query.parents.push(resume);
                step = crate::engine::builtins::ElementStep::start(runtime, realm, element, value)
                    .map_err(runtime_error_to_vm_error)?
                    .into();
                continue;
            }
            Step::ElementComplete(result) => {
                let Some(resume) = query.parents.pop() else {
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
                let resume = query
                    .parents
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
                if let Some(resume) = query.parents.pop() {
                    step = resume
                        .set(runtime, action)
                        .map_err(runtime_error_to_vm_error)?;
                    continue;
                }
                let Some(Finish::Write { key, strict, .. }) = query.finish.as_ref() else {
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
                if !execution
                    .frames
                    .can_push_with_continuations(query.continuation_depth())
                {
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
                query
                    .parents
                    .try_reserve(1)
                    .map_err(|_| Error::internal("property continuation allocation failed"))?;
                query.parents.push(resume);
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
                if !execution
                    .frames
                    .can_push_with_continuations(query.continuation_depth())
                {
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
                query
                    .parents
                    .try_reserve(1)
                    .map_err(|_| Error::internal("property continuation allocation failed"))?;
                query.parents.push(resume);
                step = crate::engine::object::ProxySetStep::start(
                    runtime, realm, object, key, value, receiver,
                )
                .map_err(runtime_error_to_vm_error)?
                .into();
                continue;
            }
            Step::Defined(result) => {
                let resume = query
                    .parents
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
                    if !execution
                        .frames
                        .can_push_with_continuations(query.continuation_depth())
                    {
                        let Completion::Throw(value) = overflow(runtime, realm)? else {
                            unreachable!()
                        };
                        step = resume
                            .defined(runtime, NativeConversion::Throw(value))
                            .map_err(runtime_error_to_vm_error)?;
                        continue;
                    }
                    query
                        .parents
                        .try_reserve(1)
                        .map_err(|_| Error::internal("property continuation allocation failed"))?;
                    query.parents.push(resume);
                    step = crate::engine::object::ProxyDefineStep::start(
                        runtime, realm, object, key, descriptor,
                    )
                    .map_err(runtime_error_to_vm_error)?
                    .into();
                    continue;
                }
                step = Step::DefineOrdinary {
                    object,
                    key,
                    descriptor,
                    resume,
                };
                continue;
            }
            Step::DefineOrdinary {
                object,
                key,
                descriptor,
                resume,
            } => {
                if let Some(length) = runtime
                    .prepare_array_length_definition(Some(realm), &object, &key, &descriptor)
                    .map_err(runtime_error_to_vm_error)?
                {
                    query
                        .parents
                        .try_reserve(1)
                        .map_err(|_| Error::internal("property continuation allocation failed"))?;
                    query.parents.push(Resume::DefineLength {
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
                    query
                        .parents
                        .try_reserve(1)
                        .map_err(|_| Error::internal("property continuation allocation failed"))?;
                    query.parents.push(Resume::DefineTyped {
                        object,
                        _descriptor: descriptor,
                        resume: Box::new(resume),
                    });
                    step = request.into();
                    continue;
                }
                let result = match runtime
                        .define_own_property_in_realm(Some(realm), &object, &key, &descriptor)
                        .map_err(runtime_error_to_vm_error)? {
                        crate::engine::object::operations::PropertyDefineOutcome::Defined(true) => NativeConversion::Value(crate::engine::object::operations::InternalDefineResult::Defined),
                        crate::engine::object::operations::PropertyDefineOutcome::Defined(false) => NativeConversion::Value(crate::engine::object::operations::InternalDefineResult::RejectedOrdinary(object)),
                        crate::engine::object::operations::PropertyDefineOutcome::Throw(value) => NativeConversion::Throw(value),
                    };
                step = resume
                    .defined(runtime, result)
                    .map_err(runtime_error_to_vm_error)?;
                continue;
            }
            Step::OwnComplete(descriptor) => {
                let parent = query
                    .parents
                    .pop()
                    .ok_or_else(|| Error::internal("descriptor reply has no parent operation"))?;
                step = parent
                    .descriptor(runtime, descriptor)
                    .map_err(runtime_error_to_vm_error)?;
                continue;
            }
            Step::BooleanComplete(result) => {
                let resume = query
                    .parents
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
                    if !execution
                        .frames
                        .can_push_with_continuations(query.continuation_depth())
                    {
                        let Completion::Throw(value) = overflow(runtime, realm)? else {
                            unreachable!()
                        };
                        step = resume
                            .prototype(runtime, NativeConversion::Throw(value))
                            .map_err(runtime_error_to_vm_error)?;
                        continue;
                    }
                    query
                        .parents
                        .try_reserve(1)
                        .map_err(|_| Error::internal("property continuation allocation failed"))?;
                    query
                        .parents
                        .push(Resume::PrototypeGetReply(Box::new(resume)));
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
                    if !execution
                        .frames
                        .can_push_with_continuations(query.continuation_depth())
                    {
                        let Completion::Throw(value) = overflow(runtime, realm)? else {
                            unreachable!()
                        };
                        step = resume
                            .boolean(runtime, NativeConversion::Throw(value))
                            .map_err(runtime_error_to_vm_error)?;
                        continue;
                    }
                    query
                        .parents
                        .try_reserve(1)
                        .map_err(|_| Error::internal("property continuation allocation failed"))?;
                    query
                        .parents
                        .push(Resume::PrototypeSetReply(Box::new(resume)));
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
                    if !execution
                        .frames
                        .can_push_with_continuations(query.continuation_depth())
                    {
                        let Completion::Throw(value) = overflow(runtime, realm)? else {
                            unreachable!()
                        };
                        step = resume
                            .boolean(runtime, NativeConversion::Throw(value))
                            .map_err(runtime_error_to_vm_error)?;
                        continue;
                    }
                    query
                        .parents
                        .try_reserve(1)
                        .map_err(|_| Error::internal("property continuation allocation failed"))?;
                    query.parents.push(resume);
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
                    if !execution
                        .frames
                        .can_push_with_continuations(query.continuation_depth())
                    {
                        let Completion::Throw(value) = overflow(runtime, realm)? else {
                            unreachable!()
                        };
                        step = resume
                            .boolean(runtime, NativeConversion::Throw(value))
                            .map_err(runtime_error_to_vm_error)?;
                        continue;
                    }
                    query
                        .parents
                        .try_reserve(1)
                        .map_err(|_| Error::internal("property continuation allocation failed"))?;
                    query.parents.push(resume);
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
                    query
                        .parents
                        .try_reserve(1)
                        .map_err(|_| Error::internal("property continuation allocation failed"))?;
                    query.parents.push(resume);
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
                query
                    .parents
                    .try_reserve(1)
                    .map_err(|_| Error::internal("property continuation allocation failed"))?;
                query.parents.push(resume);
                step = DescriptorStep::start(runtime, realm, value)
                    .map_err(runtime_error_to_vm_error)?
                    .into();
                continue;
            }
            Step::Converted(result) => {
                let Some(resume) = query.parents.pop() else {
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
                        query.parents.try_reserve(1).map_err(|_| {
                            Error::internal("property continuation allocation failed")
                        })?;
                        query.parents.push(resume);
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
                let read = runtime
                    .prepare_ordinary_read(&object, &key, receiver)
                    .map_err(runtime_error_to_vm_error)?;
                step = Step::PreparedRead { read, key, resume };
                continue;
            }
            Step::PreparedRead { read, key, resume } => match read {
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
                    if !execution
                        .frames
                        .can_push_with_continuations(query.continuation_depth())
                    {
                        step = resume
                            .resume(runtime, overflow(runtime, realm)?)
                            .map_err(runtime_error_to_vm_error)?;
                        continue;
                    }
                    query
                        .parents
                        .try_reserve(1)
                        .map_err(|_| Error::internal("property continuation allocation failed"))?;
                    query.parents.push(resume);
                    step = ProxyGetStep::start(runtime, realm, object, key, receiver)
                        .map_err(runtime_error_to_vm_error)?
                        .into();
                    continue;
                }
            },
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
                    query
                        .parents
                        .try_reserve(1)
                        .map_err(|_| Error::internal("property continuation allocation failed"))?;
                    query.parents.push(resume);
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
                if !execution
                    .frames
                    .can_push_with_continuations(query.continuation_depth())
                {
                    step = resume
                        .resume(runtime, overflow(runtime, realm)?)
                        .map_err(runtime_error_to_vm_error)?;
                    continue;
                }
                query
                    .parents
                    .try_reserve(1)
                    .map_err(|_| Error::internal("property continuation allocation failed"))?;
                query.parents.push(resume);
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
            if !execution
                .frames
                .can_push_with_continuations(query.continuation_depth())
            {
                step = resume
                    .resume(runtime, overflow(runtime, realm)?)
                    .map_err(runtime_error_to_vm_error)?;
                continue;
            }
            query
                .parents
                .try_reserve(1)
                .map_err(|_| Error::internal("property continuation allocation failed"))?;
            query.parents.push(resume);
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
        if let CallableExecution::Native {
            target,
            realm: defining_realm,
            min_readable_args,
        } = classification
            && crate::engine::builtins::continuation::NativeOperation::for_target(target).is_some()
        {
            step = Step::Native {
                mode: super::call::NativeInvokeMode::Ordinary,
                callable,
                target,
                defining_realm,
                min_readable_args,
                invocation: super::call::NativeInvocation::Call {
                    this_value: receiver,
                },
                arguments,
                resume,
            };
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
                if !execution
                    .frames
                    .can_push_with_continuations(query.continuation_depth())
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
                return Ok(Next::Call { entry, resume });
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

#[cfg(test)]
mod native_scope_tests {
    use super::*;
    use crate::engine::api::{Context, ErrorKind};

    fn prepare(
        runtime: &Runtime,
        context: &mut Context,
        name: &str,
    ) -> super::super::call::PreparedNativeCall {
        let callable = runtime
            .callable_from_value(context.eval(name).unwrap())
            .unwrap();
        let CallableExecution::Native {
            target,
            realm,
            min_readable_args,
        } = runtime.bytecode_for_callable(&callable).unwrap()
        else {
            panic!("expected native")
        };
        runtime
            .prepare_native_invocation(
                &callable,
                realm,
                target,
                min_readable_args,
                super::super::call::NativeInvocation::Call {
                    this_value: Value::Undefined,
                },
                &[],
                super::super::call::NativeInvokeMode::Ordinary,
            )
            .unwrap()
    }

    #[test]
    fn nested_native_scope_errors_keep_frames_until_capture_and_restore_parent_realm() {
        let runtime = Runtime::new();
        let mut caller = runtime.new_context();
        let mut outer = runtime.new_context();
        let mut inner = runtime.new_context();
        let prototype = inner.eval("TypeError.prototype").unwrap();
        let first = prepare(&runtime, &mut outer, "Object.getPrototypeOf");
        let second = prepare(&runtime, &mut inner, "Reflect.setPrototypeOf");
        let mut query = Query {
            realm: inner.realm,
            parents: Vec::new(),
            natives: vec![
                NativeScope {
                    call: first,
                    parents: Vec::new(),
                    resume: Resume::Identity,
                    parent_realm: caller.realm,
                },
                NativeScope {
                    call: second,
                    parents: Vec::new(),
                    resume: Resume::Identity,
                    parent_realm: outer.realm,
                },
            ],
            finish: Some(Finish::PropertyRead(0)),
        };
        assert_eq!(query.continuation_depth(), 2);
        let step = query
            .finish_native(
                &runtime,
                Err(Error::new(ErrorKind::Type, "nested native failure")),
            )
            .unwrap();
        assert_eq!(query.realm, outer.realm);
        assert_eq!(runtime.0.state.borrow().active_frames.len(), 1);
        let Step::Complete(Completion::Throw(Value::Object(error))) = step else {
            panic!("expected captured error")
        };
        assert_eq!(
            runtime.get_prototype_of(&error).unwrap().map(Value::Object),
            Some(prototype)
        );
        let stack = caller
            .get_property(&error, &runtime.intern_property_key("stack").unwrap())
            .unwrap();
        let Value::String(stack) = stack else {
            panic!("expected stack")
        };
        let stack = stack.to_string();
        assert!(stack.contains("getPrototypeOf (native)"), "{stack}");
        assert!(stack.contains("setPrototypeOf (native)"), "{stack}");
        let step = query
            .finish_native(
                &runtime,
                Ok(Completion::Throw(Value::Object(error.clone()))),
            )
            .unwrap();
        assert!(
            matches!(step, Step::Complete(Completion::Throw(Value::Object(value))) if value == error)
        );
        assert_eq!(query.realm, caller.realm);
        assert!(runtime.0.state.borrow().active_frames.is_empty());
    }
}

#[allow(clippy::too_many_arguments)]
fn native_scope(
    runtime: &Runtime,
    execution: &RunningExecution,
    query: &mut Query,
    callable: crate::engine::object::CallableRef,
    target: crate::engine::builtins::native::NativeFunctionId,
    defining_realm: crate::engine::heap::ContextId,
    min_readable_args: u8,
    mode: super::call::NativeInvokeMode,
    invocation: super::call::NativeInvocation,
    arguments: Vec<Value>,
    resume: Resume,
) -> Result<Step, Error> {
    let realm = query.realm;
    let Some(kind) = crate::engine::builtins::continuation::NativeOperation::for_target(target)
    else {
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_sync_call_bridge();
        let native_realm = if matches!(mode, super::call::NativeInvokeMode::IteratorNextRaw)
            || target.uses_calling_realm()
        {
            realm
        } else {
            defining_realm
        };
        let result = runtime
            .invoke_native_function(
                &callable,
                native_realm,
                target,
                min_readable_args,
                invocation,
                &arguments,
                mode,
            )
            .map_err(runtime_error_to_vm_error)?;
        return resume
            .native(runtime, result)
            .map_err(runtime_error_to_vm_error);
    };
    if !execution
        .frames
        .can_push_with_continuations(query.continuation_depth())
        || runtime.host_stack_would_overflow()
    {
        return resume
            .resume(runtime, overflow(runtime, realm)?)
            .map_err(runtime_error_to_vm_error);
    }
    query
        .natives
        .try_reserve(1)
        .map_err(|_| Error::internal("native continuation allocation failed"))?;
    let native_realm = if matches!(mode, super::call::NativeInvokeMode::IteratorNextRaw)
        || target.uses_calling_realm()
    {
        realm
    } else {
        defining_realm
    };
    let mut call = runtime
        .prepare_native_invocation(
            &callable,
            native_realm,
            target,
            min_readable_args,
            invocation,
            &arguments,
            mode,
        )
        .map_err(runtime_error_to_vm_error)?;
    call.activation
        .own_continuation()
        .map_err(runtime_error_to_vm_error)?;
    query.natives.push(NativeScope {
        call,
        parents: std::mem::take(&mut query.parents),
        resume,
        parent_realm: realm,
    });
    query.realm = native_realm;
    let native = query.natives.last().expect("native scope was installed");
    let invocation = runtime
        .adapt_native_invocation(
            target,
            native_realm,
            native.call.invocation.clone(),
            &native.call.activation.arguments,
        )
        .map_err(runtime_error_to_vm_error)?;
    Ok(match invocation {
        super::call::NativeInvocationAdaptation::Complete(result) => Step::Complete(result),
        super::call::NativeInvocationAdaptation::Invoke(invocation) => kind
            .start(
                runtime,
                native_realm,
                &invocation,
                &native.call.activation.arguments,
                &native.call.activation.callable,
            )
            .map_err(runtime_error_to_vm_error)?
            .into(),
    })
}

pub(super) fn start_iterator_read(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    pending: Box<super::iterator_driver::PendingIterator>,
    receiver: Value,
    key: PropertyKey,
) -> Result<CallStep, Error> {
    start_iterator_query(
        runtime,
        execution,
        pending,
        Step::ReadValue {
            receiver,
            key,
            resume: Resume::Identity,
        },
        false,
    )
}
pub(super) fn start_iterator_call(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    pending: Box<super::iterator_driver::PendingIterator>,
    callable: crate::engine::object::CallableRef,
    receiver: Value,
) -> Result<CallStep, Error> {
    start_iterator_query(
        runtime,
        execution,
        pending,
        Step::Call {
            target: DirectCallTarget::Callable(callable),
            receiver,
            arguments: Vec::new(),
            resume: Resume::Identity,
        },
        false,
    )
}
pub(super) fn start_iterator_next(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    pending: Box<super::iterator_driver::PendingIterator>,
    iterator: Value,
    method: crate::engine::object::CallableRef,
) -> Result<CallStep, Error> {
    let Value::Object(iterator) = iterator else {
        return Err(Error::internal("iterator record lost object receiver"));
    };
    let step = crate::engine::builtins::IteratorNextStep::start(
        runtime,
        pending.realm(),
        iterator,
        Value::Object(method.into_object()),
    )
    .map_err(runtime_error_to_vm_error)?;
    start_iterator_query(runtime, execution, pending, step.into(), true)
}
fn start_iterator_query(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    pending: Box<super::iterator_driver::PendingIterator>,
    step: Step,
    next: bool,
) -> Result<CallStep, Error> {
    let frame = pending.frame();
    let realm = pending.realm();
    let parent = execution.frames.current_mut(frame)?;
    let identity = parent
        .cold
        .property_generation
        .checked_add(1)
        .ok_or_else(|| Error::internal("iterator query identity exhausted"))?;
    parent.cold.property_generation = identity;
    let finish = if next {
        Finish::IteratorNext(pending)
    } else {
        Finish::Iterator(pending)
    };
    let result = advance(
        runtime,
        execution,
        frame,
        identity,
        Vec::new(),
        step,
        finish,
    );
    match finish_error(runtime, realm, result)? {
        Progress::Call(step) => Ok(step),
        Progress::Conversion(_) => Err(Error::internal("iterator returned unrelated conversion")),
    }
}
pub(super) fn start_instance(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    frame: FrameId,
    candidate: Value,
    target: ObjectRef,
    depth: usize,
) -> Result<CallStep, Error> {
    let parent = execution.frames.current_mut(frame)?;
    let realm = parent.executable.realm;
    let identity = parent
        .cold
        .property_generation
        .checked_add(1)
        .ok_or_else(|| Error::internal("instance query identity exhausted"))?;
    parent.cold.property_generation = identity;
    let result = (|| {
        let step = crate::engine::builtins::InstanceStep::start(runtime, realm, candidate, target)
            .map_err(runtime_error_to_vm_error)?;
        advance(
            runtime,
            execution,
            frame,
            identity,
            Vec::new(),
            step.into(),
            Finish::Call { depth, tail: false },
        )
    })();
    match finish_error(runtime, realm, result)? {
        Progress::Call(step) => Ok(step),
        Progress::Conversion(_) => Err(Error::internal("instanceof returned conversion")),
    }
}

pub(super) fn start_object_copy(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    frame: FrameId,
    target_depth: usize,
    source_depth: usize,
    excluded_depth: Option<usize>,
) -> Result<CallStep, Error> {
    let parent = execution.frames.current_mut(frame)?;
    let realm = parent.executable.realm;
    let target = execution.slots.peek(&parent.window, target_depth)?.clone();
    let Value::Object(target) = target else {
        return Err(Error::internal(
            "CopyDataProperties target is not an object",
        ));
    };
    let source = execution.slots.peek(&parent.window, source_depth)?.clone();
    let excluded = if let Some(depth) = excluded_depth {
        let Value::Object(object) = execution.slots.peek(&parent.window, depth)?.clone() else {
            return Err(Error::internal(
                "CopyDataProperties exclusion is not an object",
            ));
        };
        Some(object)
    } else {
        None
    };
    let step = crate::engine::builtins::ObjectCopyStep::start(runtime, target, source, excluded)
        .map_err(runtime_error_to_vm_error)?;
    let identity = parent
        .cold
        .property_generation
        .checked_add(1)
        .ok_or_else(|| Error::internal("copy query identity exhausted"))?;
    parent.cold.property_generation = identity;
    let depth = execution.slots.depth(&parent.window);
    if excluded_depth.is_none() {
        execution.slots.pop(&mut parent.window)?;
    }
    let result = advance(
        runtime,
        execution,
        frame,
        identity,
        Vec::new(),
        step.into(),
        Finish::Discard(depth),
    );
    match finish_error(runtime, realm, result)? {
        Progress::Call(step) => Ok(step),
        Progress::Conversion(_) => Err(Error::internal("object copy returned conversion")),
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn start_vm_call(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    frame: FrameId,
    callable: crate::engine::object::CallableRef,
    receiver: Value,
    arguments: Vec<Value>,
    value_use: ReturnValue,
) -> Result<CallStep, Error> {
    match start_owned_callback(
        runtime,
        execution,
        frame,
        callable,
        receiver,
        arguments,
        Finish::VmCall(value_use),
    )? {
        Progress::Call(step) => Ok(step),
        Progress::Conversion(_) => Err(Error::internal("VM callback returned a conversion")),
    }
}

pub(super) fn start_environment(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    frame: FrameId,
    step: super::environment_bindings::operation::EnvironmentStep,
    value_use: ReturnValue,
    depth: usize,
) -> Result<CallStep, Error> {
    let parent = execution.frames.current_mut(frame)?;
    let realm = parent.executable.realm;
    let identity = parent
        .cold
        .property_generation
        .checked_add(1)
        .ok_or_else(|| Error::internal("environment query identity exhausted"))?;
    parent.cold.property_generation = identity;
    let finish = match value_use {
        ReturnValue::Push => Finish::PropertyRead(depth),
        ReturnValue::Discard => Finish::Discard(depth),
    };
    let result = advance(
        runtime,
        execution,
        frame,
        identity,
        Vec::new(),
        step.into(),
        finish,
    );
    match finish_error(runtime, realm, result)? {
        Progress::Call(step) => Ok(step),
        Progress::Conversion(_) => Err(Error::internal("environment returned a conversion")),
    }
}

enum IteratorProgress {
    Step(Step),
    Done(CallStep),
}
fn continue_iterator(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    query: &mut Query,
    pending: Box<super::iterator_driver::PendingIterator>,
    action: super::iterator_driver::IteratorAction,
) -> Result<IteratorProgress, Error> {
    use super::iterator_driver::IteratorAction;
    let (step, next) = match action {
        IteratorAction::Finish => {
            return super::iterator_driver::finish(execution, pending).map(IteratorProgress::Done);
        }
        IteratorAction::Read(receiver, key) => (
            Step::ReadValue {
                receiver,
                key,
                resume: Resume::Identity,
            },
            false,
        ),
        IteratorAction::Call(callable, receiver) => (
            Step::Call {
                target: DirectCallTarget::Callable(callable),
                receiver,
                arguments: Vec::new(),
                resume: Resume::Identity,
            },
            false,
        ),
        IteratorAction::Next(callable, receiver) => {
            let Value::Object(iterator) = receiver else {
                return Err(Error::internal("iterator record lost object receiver"));
            };
            (
                crate::engine::builtins::IteratorNextStep::start(
                    runtime,
                    pending.realm(),
                    iterator,
                    Value::Object(callable.into_object()),
                )
                .map_err(runtime_error_to_vm_error)?
                .into(),
                true,
            )
        }
    };
    query.finish = Some(if next {
        Finish::IteratorNext(pending)
    } else {
        Finish::Iterator(pending)
    });
    Ok(IteratorProgress::Step(step))
}

pub(super) fn start_numeric(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    frame: FrameId,
    step: super::numeric::operation::NumericStep,
    depth: usize,
) -> Result<CallStep, Error> {
    let parent = execution.frames.current_mut(frame)?;
    let realm = parent.executable.realm;
    let identity = parent
        .cold
        .property_generation
        .checked_add(1)
        .ok_or_else(|| Error::internal("numeric query identity exhausted"))?;
    parent.cold.property_generation = identity;
    let result = advance(
        runtime,
        execution,
        frame,
        identity,
        Vec::new(),
        step.into(),
        Finish::Numeric(depth),
    );
    match finish_error(runtime, realm, result)? {
        Progress::Call(step) => Ok(step),
        Progress::Conversion(_) => Err(Error::internal(
            "numeric operation returned conversion task",
        )),
    }
}

pub(super) fn start_class_parent(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    pending: Box<super::construct_driver::PendingClass>,
    parent: ObjectRef,
    realm: crate::engine::heap::ContextId,
    frame: FrameId,
) -> Result<CallStep, Error> {
    let step = Step::ReadValue {
        receiver: Value::Object(parent),
        key: runtime
            .intern_property_key("prototype")
            .map_err(|error| Error::internal(error.to_string()))?,
        resume: Resume::Identity,
    };
    start_instruction_query(
        runtime,
        execution,
        frame,
        realm,
        step,
        Finish::Class(pending),
    )
}
pub(super) fn start_public_field(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    frame: FrameId,
    object: ObjectRef,
    key: PropertyKey,
    value: Value,
    depth: usize,
) -> Result<CallStep, Error> {
    let realm = execution.frames.current_mut(frame)?.executable.realm;
    let step = Step::Define {
        object,
        key,
        descriptor: Runtime::public_class_field_descriptor(value),
        resume: Resume::PublicField,
    };
    start_instruction_query(
        runtime,
        execution,
        frame,
        realm,
        step,
        Finish::Discard(depth),
    )
}
fn start_instruction_query(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    frame: FrameId,
    realm: crate::engine::heap::ContextId,
    step: Step,
    finish: Finish,
) -> Result<CallStep, Error> {
    let parent = execution.frames.current_mut(frame)?;
    let identity = parent
        .cold
        .property_generation
        .checked_add(1)
        .ok_or_else(|| Error::internal("instruction query identity exhausted"))?;
    parent.cold.property_generation = identity;
    let result = advance(
        runtime,
        execution,
        frame,
        identity,
        Vec::new(),
        step,
        finish,
    );
    match finish_error(runtime, realm, result)? {
        Progress::Call(step) => Ok(step),
        Progress::Conversion(_) => Err(Error::internal("instruction returned a conversion task")),
    }
}

pub(super) fn start_for_in_query(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    frame: FrameId,
    step: super::for_in::operation::ForInStep,
    depth: usize,
) -> Result<CallStep, Error> {
    let realm = execution.frames.current_mut(frame)?.executable.realm;
    start_instruction_query(
        runtime,
        execution,
        frame,
        realm,
        step.into(),
        Finish::ForIn(depth),
    )
}

pub(super) fn start_literal_definition(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    frame: FrameId,
    step: crate::engine::object::object_literal::element::LiteralDefinitionStep,
    depth: usize,
) -> Result<CallStep, Error> {
    let realm = execution.frames.current_mut(frame)?.executable.realm;
    start_instruction_query(
        runtime,
        execution,
        frame,
        realm,
        step.into(),
        Finish::Discard(depth),
    )
}
