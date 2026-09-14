//! Schedule typed domain requests and their JavaScript child frames.
//! Domain owners retain algorithms; this driver owns reply routing and roots.
use super::{
    Completion,
    call::{BytecodeCallRequest, CallableExecution, DirectCallTarget},
    driver::{CallStep, push_frame},
    exception::runtime_error_to_vm_error,
    execution::RunningExecution,
    frame::{FrameId, OperationTarget, ReturnOwner, ReturnTarget, ReturnValue},
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
mod dispatch_conversion;
mod dispatch_execution;
mod dispatch_iteration;
mod dispatch_read;
mod dispatch_write;

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

#[derive(Default)]
struct Parents(Vec<Resume>);
impl Parents {
    fn len(&self) -> usize {
        self.0.len()
    }
    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    fn try_reserve(&mut self, additional: usize) -> Result<(), std::collections::TryReserveError> {
        self.0.try_reserve(additional)
    }
    fn push(&mut self, resume: Resume) {
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_execution_event("parent_push");
        self.0.push(resume);
    }
    fn pop(&mut self) -> Option<Resume> {
        let result = self.0.pop();
        #[cfg(feature = "profiling")]
        if result.is_some() {
            crate::engine::api::profiling::record_owned_execution_event("parent_pop");
        }
        result
    }
}

struct Query {
    #[cfg(feature = "profiling")]
    had_callback: bool,
    realm: crate::engine::heap::ContextId,
    parents: Parents,
    natives: Vec<NativeScope>,
    finish: Option<Finish>,
}
struct NativeScope {
    call: super::call::PreparedNativeCall,
    parents: Parents,
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
    Continue,
    Invoke {
        target: DirectCallTarget,
        receiver: Value,
        arguments: Vec<Value>,
        resume: Resume,
    },
    Done(Progress),
    Call {
        entry: super::frame::FrameEntry,
        pc: usize,
        resume: Resume,
    },
}

enum Finish {
    Root,
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

fn finish_numeric(
    execution: &mut RunningExecution,
    frame: FrameId,
    value: Value,
    previous: Option<Value>,
    _depth: usize,
) -> Result<CallStep, Error> {
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
    Ok(CallStep::Entered)
}

fn finish_instruction(
    execution: &mut RunningExecution,
    owner: ReturnOwner,
    completion: Completion,
    push: bool,
    _depth: usize,
) -> Result<Progress, Error> {
    match completion {
        Completion::Return(value) => {
            let parent = execution.frames.current_mut(owner.frame()?)?;
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
    }
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
pub(super) fn start_callback_call(
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
        let step = step
            .advance_without_callback(runtime)
            .map_err(runtime_error_to_vm_error)?;
        if let crate::engine::object::SetStep::Complete(action) = step {
            if !matches!(
                action,
                crate::engine::object::operations::PropertySetAction::Call { .. }
            ) {
                let completion = runtime
                    .finish_property_set(
                        request::set_result(action).map_err(runtime_error_to_vm_error)?,
                        &key,
                        strict,
                    )
                    .map_err(runtime_error_to_vm_error)?;
                #[cfg(feature = "profiling")]
                crate::engine::api::profiling::record_owned_execution_event(
                    "write_completed_without_query",
                );
                return finish_instruction(
                    execution,
                    ReturnOwner::Frame(frame),
                    completion,
                    false,
                    depth,
                );
            }
            return advance(
                runtime,
                execution,
                frame,
                identity,
                Vec::new(),
                Step::SetComplete(action),
                Finish::Write { key, strict, depth },
            );
        }

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
    reply_outcome(
        runtime,
        execution,
        target,
        super::suspend::VmRunOutcome::Complete(completion),
    )
}

pub(super) fn reply_suspended(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    target: ReturnTarget,
    outcome: super::suspend::VmRunOutcome,
) -> Result<Progress, Error> {
    reply_outcome(runtime, execution, target, outcome)
}

fn take_pending(
    execution: &mut RunningExecution,
    owner: ReturnOwner,
) -> Result<Box<PendingProxyGet>, Error> {
    let pending = match owner {
        ReturnOwner::Frame(frame) => execution
            .frames
            .current_mut(frame)?
            .cold
            .property_wait
            .take(),
        ReturnOwner::Root => execution.root_query.take(),
    };
    pending.ok_or_else(|| Error::internal("request reply has no pending operation"))
}
fn put_pending(
    execution: &mut RunningExecution,
    owner: ReturnOwner,
    pending: Box<PendingProxyGet>,
) -> Result<(), Error> {
    let slot = match owner {
        ReturnOwner::Frame(frame) => &mut execution.frames.current_mut(frame)?.cold.property_wait,
        ReturnOwner::Root => &mut execution.root_query,
    };
    if slot.is_some() {
        return Err(Error::internal("request overwrote a pending reply"));
    }
    *slot = Some(pending);
    Ok(())
}

fn reply_outcome(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    target: ReturnTarget,
    outcome: super::suspend::VmRunOutcome,
) -> Result<Progress, Error> {
    let pending = take_pending(execution, target.owner)?;
    if target.operation != Some(OperationTarget::PropertyGet(pending.identity)) {
        return Err(Error::internal(
            "request reply belongs to another operation",
        ));
    }
    let realm = match target.owner {
        ReturnOwner::Root => pending.query.realm,
        ReturnOwner::Frame(id) => execution.frames.current_mut(id)?.executable.realm,
    };
    let step = match outcome {
        super::suspend::VmRunOutcome::Complete(completion) => {
            pending.resume.resume(runtime, completion)
        }
        outcome => pending.resume.suspended(runtime, outcome),
    }
    .map_err(runtime_error_to_vm_error);
    let result = drive(
        runtime,
        execution,
        target.owner,
        pending.identity,
        pending.query,
        step,
    );
    finish_error(runtime, realm, result)
}

pub(super) fn start_root(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    realm: crate::engine::heap::ContextId,
    operation: super::driver::RootOperation,
) -> Result<Progress, Error> {
    let step: Step = match operation {
        super::driver::RootOperation::Call {
            callable,
            receiver,
            arguments,
        } => Step::Call {
            target: DirectCallTarget::Callable(callable),
            receiver,
            arguments,
            resume: Resume::Identity,
        },
        super::driver::RootOperation::Construct(normalized) => construct::prepared(
            runtime,
            ReturnOwner::Root,
            1,
            realm,
            normalized,
            Resume::Identity,
        )?,
        super::driver::RootOperation::Get {
            object,
            key,
            receiver,
        } => Step::Read {
            object,
            key,
            receiver,
            resume: Resume::Identity,
        },
        super::driver::RootOperation::Own { object, key } => Step::Descriptor {
            object,
            key,
            resume: Resume::RootDescriptor,
        },
        super::driver::RootOperation::Define {
            object,
            key,
            descriptor,
        } => Step::Define {
            object,
            key,
            descriptor,
            resume: Resume::RootDefine,
        },
        super::driver::RootOperation::Set {
            object,
            key,
            value,
            receiver,
        } => Step::Set {
            object,
            key,
            value,
            receiver,
            resume: Resume::RootSet,
        },

        super::driver::RootOperation::ModuleCallback(step) => step.into(),
        super::driver::RootOperation::ModuleEvaluation(step) => step.into(),
        super::driver::RootOperation::ModuleLink(step) => step.into(),
        super::driver::RootOperation::FromSync(step) => step.into(),
        super::driver::RootOperation::AsyncGenerator(step) => step.into(),
        super::driver::RootOperation::Promise(step) => step.into(),
        super::driver::RootOperation::Async(step) => step.into(),
    };
    let result = drive(
        runtime,
        execution,
        ReturnOwner::Root,
        1,
        Query {
            #[cfg(feature = "profiling")]
            had_callback: false,
            realm,
            parents: Parents::default(),
            natives: Vec::new(),
            finish: Some(Finish::Root),
        },
        Ok(step),
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
        ReturnOwner::Frame(frame),
        identity,
        Query {
            #[cfg(feature = "profiling")]
            had_callback: false,
            realm,
            parents: Parents(parents),
            natives: Vec::new(),
            finish: Some(finish),
        },
        Ok(step),
    )
}

fn drive(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    owner: ReturnOwner,
    identity: u64,
    mut query: Query,
    mut step: Result<Step, Error>,
) -> Result<Progress, Error> {
    #[cfg(feature = "profiling")]
    {
        use crate::engine::api::profiling::record_owned_execution_layout as layout;
        layout::<Step>("Step");
        layout::<Resume>("Resume");
        layout::<Next>("Next");
        layout::<super::conversion_driver::ConversionTask>("ConversionTask");
        layout::<super::frame::FrameCold>("FrameCold");
    }
    loop {
        let result = step
            .and_then(|step| advance_inner(runtime, execution, owner, identity, &mut query, step));
        match result {
            Ok(Next::Done(result)) => {
                #[cfg(feature = "profiling")]
                crate::engine::api::profiling::record_owned_execution_event(
                    if query.had_callback {
                        "query_completed_after_callback"
                    } else {
                        "query_completed_without_callback"
                    },
                );
                return Ok(result);
            }
            Ok(Next::Call { entry, pc, resume }) => {
                #[cfg(feature = "profiling")]
                let had_callback = std::mem::replace(&mut query.had_callback, true);
                put_pending(
                    execution,
                    owner,
                    Box::new(PendingProxyGet {
                        identity,
                        query,
                        resume,
                    }),
                )?;
                match push_frame(execution, entry) {
                    Ok(id) => {
                        #[cfg(feature = "profiling")]
                        crate::engine::api::profiling::record_owned_execution_event(
                            "query_bytecode_callback",
                        );
                        let child = execution.frames.current_mut(id)?;
                        child.resume_pc = pc;
                        child.fault_pc = pc.saturating_sub(1);
                        return Ok(Progress::Call(CallStep::Entered));
                    }
                    Err(error) => {
                        let pending = take_pending(execution, owner)?;
                        drop(pending.resume);
                        query = pending.query;
                        #[cfg(feature = "profiling")]
                        {
                            query.had_callback = had_callback;
                        }
                        step = Err(error);
                    }
                }
            }
            Ok(Next::Continue | Next::Invoke { .. }) => {
                return Err(Error::internal(
                    "query dispatch escaped without a terminal step",
                ));
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
    owner: ReturnOwner,
    identity: u64,
    query: &mut Query,
    mut step: Step,
) -> Result<Next, Error> {
    loop {
        // Keep domain dispatch frames bounded on the existing 256 KiB host stack.
        // Each helper returns before another request category is dispatched.
        let dispatch: fn(
            &Runtime,
            &mut RunningExecution,
            ReturnOwner,
            u64,
            &mut Query,
            &mut Step,
        ) -> Result<Next, Error> = match &step {
            Step::RootDescriptor(..)
            | Step::Complete { .. }
            | Step::ForInComplete { .. }
            | Step::NumericComplete { .. }
            | Step::NativeRawComplete { .. } => dispatch_execution::finish,
            Step::ResumeFrame { .. } | Step::ConstructorReady { .. } | Step::Native { .. } => {
                dispatch_execution::activation
            }
            Step::ModuleCallbackOperation { .. }
            | Step::ModuleBodyOperation { .. }
            | Step::ModuleLink { .. }
            | Step::PromiseOperation { .. }
            | Step::IntrinsicPromiseResolve { .. }
            | Step::Construct { .. }
            | Step::ConstructProxy { .. }
            | Step::IndirectEval { .. }
            | Step::NumericHtmlDda { .. } => dispatch_execution::prepare,
            Step::RegExpSpecies { .. }
            | Step::RegExpSpeciesComplete { .. }
            | Step::Aggregate { .. }
            | Step::ArraySpecies { .. }
            | Step::ArrayPush { .. }
            | Step::IteratorNext { .. }
            | Step::IteratorNextComplete { .. }
            | Step::IteratorCall { .. }
            | Step::IteratorClose { .. }
            | Step::ObjectTag { .. }
            | Step::RegExpExec { .. }
            | Step::IteratorCloseWithResume { .. }
            | Step::OrdinaryInstance { .. }
            | Step::ParseIterator { .. }
            | Step::ArrayCopy { .. } => dispatch_iteration::advance,
            Step::String { .. }
            | Step::OrdinaryPrimitive { .. }
            | Step::Arguments { .. }
            | Step::ArgumentsComplete { .. }
            | Step::Primitive { .. }
            | Step::Number { .. }
            | Step::NumberComplete { .. }
            | Step::LengthComplete { .. }
            | Step::Element { .. }
            | Step::ElementComplete { .. }
            | Step::TypedComplete { .. } => dispatch_conversion::primitive,
            Step::ConstructorSource { .. }
            | Step::ConstructorSourceComplete { .. }
            | Step::TypedSpeciesView { .. }
            | Step::TypedIteratorMethod { .. }
            | Step::TypedIteratorMethodComplete { .. }
            | Step::TypedCollect { .. }
            | Step::TypedCollectComplete { .. }
            | Step::TypedCreate { .. }
            | Step::TypedSpecies { .. }
            | Step::TypedSpeciesComplete { .. } => dispatch_conversion::constructor,
            Step::SnapshotEnumerable { .. }
            | Step::OwnFlag { .. }
            | Step::Keys { .. }
            | Step::KeysComplete { .. }
            | Step::ReadValue { .. } => dispatch_write::keys,
            Step::SetContinue { .. }
            | Step::SetLength { .. }
            | Step::SetSpecial { .. }
            | Step::SetComplete { .. }
            | Step::Set { .. }
            | Step::SetProxy { .. } => dispatch_write::set,
            Step::Defined { .. } | Step::Define { .. } | Step::DefineOrdinary { .. } => {
                dispatch_write::define
            }
            Step::OwnComplete { .. }
            | Step::BooleanComplete { .. }
            | Step::GetPrototype { .. }
            | Step::SetPrototype { .. } => dispatch_read::prototype,
            Step::Delete { .. } | Step::PreventExtensions { .. } | Step::Extensible { .. } => {
                dispatch_read::attributes
            }
            Step::Convert { .. }
            | Step::Converted { .. }
            | Step::Has { .. }
            | Step::Read { .. }
            | Step::PreparedRead { .. }
            | Step::Call { .. }
            | Step::Descriptor { .. } => dispatch_read::get,
        };
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_execution_event("query_dispatch");
        let next = dispatch(runtime, execution, owner, identity, query, &mut step)?;
        let (target, receiver, arguments, resume) = match next {
            Next::Continue => {
                #[cfg(feature = "profiling")]
                crate::engine::api::profiling::record_owned_execution_event("query_continue");
                continue;
            }
            Next::Invoke {
                target,
                receiver,
                arguments,
                resume,
            } => (target, receiver, arguments, resume),
            next => return Ok(next),
        };
        match invoke(
            runtime, execution, owner, identity, query, target, receiver, arguments, resume,
            &mut step,
        )? {
            Next::Continue => {}
            next => return Ok(next),
        }
    }
}

#[inline(never)]
fn invoke(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    owner: ReturnOwner,
    identity: u64,
    query: &mut Query,
    target: DirectCallTarget,
    receiver: Value,
    arguments: Vec<Value>,
    resume: Resume,
    next_step: &mut Step,
) -> Result<Next, Error> {
    let realm = query.realm;
    let step;
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
                *next_step = step;
                return Ok(Next::Continue);
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
            *next_step = step;
            return Ok(Next::Continue);
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
            *next_step = step;
            return Ok(Next::Continue);
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
            *next_step = step;
            return Ok(Next::Continue);
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
        *next_step = step;
        return Ok(Next::Continue);
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
        *next_step = step;
        return Ok(Next::Continue);
    }
    if let CallableExecution::Bytecode {
        bytecode,
        closure_slots,
    } = classification
    {
        let metadata = runtime
            .0
            .state
            .borrow()
            .heap
            .function_bytecode(bytecode.bytecode_id())
            .map_err(|error| Error::internal(error.to_string()))?
            .metadata;
        let kind = metadata.function_kind;
        let module_link = metadata.is_module && receiver == Value::Bool(true);
        {
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
                *next_step = step;
                return Ok(Next::Continue);
            }
            let resume = if matches!(kind, FunctionKind::Normal | FunctionKind::Async) {
                resume
            } else {
                query.parents.try_reserve(1).map_err(|_| {
                    Error::internal("generator creation continuation allocation failed")
                })?;
                query.parents.push(resume);
                Resume::GeneratorCreate(super::suspend::creation::GeneratorCreation {
                    realm,
                    callable: callable.clone(),
                    asynchronous: kind == FunctionKind::AsyncGenerator,
                })
            };
            let request = BytecodeCallRequest {
                callable,
                receiver,
                arguments,
                bytecode,
                closure_slots,
                new_target: Value::Undefined,
                caller_realm: realm,
                return_to: ReturnTarget {
                    owner,
                    value_use: ReturnValue::Push,
                    tail: false,
                    operation: Some(OperationTarget::PropertyGet(identity)),
                },
            };
            let entry = request.prepare(runtime)?;
            let resume = if kind == FunctionKind::Async && !module_link {
                query
                    .parents
                    .try_reserve(1)
                    .map_err(|_| Error::internal("async body continuation allocation failed"))?;
                query.parents.push(resume);
                Resume::Async(
                    super::async_function::AsyncResume::start(runtime, realm)
                        .map_err(runtime_error_to_vm_error)?,
                )
            } else {
                resume
            };
            return Ok(Next::Call {
                entry,
                pc: 0,
                resume,
            });
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
    *next_step = step;
    Ok(Next::Continue)
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
            #[cfg(feature = "profiling")]
            had_callback: false,
            realm: inner.realm,
            parents: Parents::default(),
            natives: vec![
                NativeScope {
                    call: first,
                    parents: Parents::default(),
                    resume: Resume::Identity,
                    parent_realm: caller.realm,
                },
                NativeScope {
                    call: second,
                    parents: Parents::default(),
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
pub(super) fn start_iterator_invoke(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    pending: Box<super::iterator_driver::PendingIterator>,
    target: DirectCallTarget,
    receiver: Value,
    arguments: Vec<Value>,
) -> Result<CallStep, Error> {
    start_iterator_query(
        runtime,
        execution,
        pending,
        Step::Call {
            target,
            receiver,
            arguments,
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
        IteratorAction::Invoke(target, receiver, arguments) => (
            Step::Call {
                target,
                receiver,
                arguments,
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
    let step = match step {
        super::numeric::operation::NumericStep::Complete { value, previous } => {
            #[cfg(feature = "profiling")]
            crate::engine::api::profiling::record_owned_execution_event(
                "numeric_completed_without_query",
            );
            return finish_numeric(execution, frame, value, previous, depth);
        }
        super::numeric::operation::NumericStep::Throw(value) => {
            return Ok(CallStep::Complete(Completion::Throw(value)));
        }
        step => step,
    };
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

pub(super) fn start_import(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    frame: FrameId,
) -> Result<CallStep, Error> {
    let parent = execution.frames.current_mut(frame)?;
    let realm = parent.executable.realm;
    let options = execution.slots.peek(&parent.window, 0)?.clone();
    let specifier = execution.slots.peek(&parent.window, 1)?.clone();
    let result = crate::engine::modules::import::ImportStep::start(
        runtime,
        realm,
        parent.executable.root(),
        specifier,
        options,
    )
    .map_err(runtime_error_to_vm_error)
    .and_then(|step| start_instruction(runtime, execution, frame, step.into(), 2));
    match finish_error(runtime, realm, result)? {
        Progress::Call(step) => Ok(step),
        Progress::Conversion(_) => Err(Error::internal(
            "dynamic import returned an untyped conversion",
        )),
    }
}
