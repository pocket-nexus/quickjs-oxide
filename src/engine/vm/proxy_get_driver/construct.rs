//! Constructor dispatch returns owned requests; bytecode bodies use ordinary child frames.
use super::{
    BytecodeCallRequest, CallableExecution, Completion, Error, NativeConversion, Next,
    OperationTarget, Query, Resume, ReturnOwner, ReturnTarget, ReturnValue, RunningExecution,
    Runtime, Step, overflow, runtime_error_to_vm_error,
};
use crate::engine::value::JsValue;
use crate::engine::{
    code::function::metadata::ConstructorKind,
    vm::call::{ConstructNewTarget, ConstructorRef, ConstructorTarget, NormalizedConstructor},
};

// A constructor boundary owns its operands until a child Step/frame accepts them.
struct ConstructorBoundary {
    runtime: Runtime,
    new_target: Option<ConstructNewTarget>,
    arguments: Vec<JsValue>,
    request: Option<Box<BytecodeCallRequest>>,
    receiver: Option<Completion>,
    resume: Option<Resume>,
}
impl ConstructorBoundary {
    fn new(runtime: &Runtime, resume: Resume) -> Self {
        Self {
            runtime: runtime.clone(),
            new_target: None,
            arguments: Vec::new(),
            request: None,
            receiver: None,
            resume: Some(resume),
        }
    }
    fn take_resume(&mut self) -> Resume {
        self.resume.take().expect("constructor continuation")
    }
}
impl Drop for ConstructorBoundary {
    fn drop(&mut self) {
        if let Some(target) = self.new_target.take() {
            let _ = target.release(&self.runtime);
        }
        for value in self.arguments.drain(..) {
            let _ = self.runtime.release_jsvalue(value);
        }
        if let Some(mut request) = self.request.take() {
            let _ = request.release_owned_values(&self.runtime);
        }
        if let Some(Completion::Return(value) | Completion::Throw(value)) = self.receiver.take() {
            let _ = self.runtime.release_jsvalue(value);
        }
        if let Some(resume) = self.resume.take() {
            resume.release_owned();
        }
    }
}

// Keep the return owner, reply identity and original constructor operands explicit at this suspension boundary.
#[allow(clippy::too_many_arguments)]
pub(super) fn start(
    runtime: &Runtime,
    owner: ReturnOwner,
    identity: u64,
    realm: crate::engine::heap::ContextId,
    constructor: ConstructorRef,
    new_target: ConstructNewTarget,
    arguments: Vec<JsValue>,
    resume: Resume,
) -> Result<Step, Error> {
    let mut boundary = ConstructorBoundary::new(runtime, resume);
    let normalized = match runtime
        .normalize_constructor(realm, constructor, new_target, arguments)
        .map_err(runtime_error_to_vm_error)?
    {
        NativeConversion::Value(result) => result,
        NativeConversion::Throw(value) => {
            return boundary
                .take_resume()
                .resume(runtime, Completion::Throw(value))
                .map_err(runtime_error_to_vm_error);
        }
    };
    prepared(
        runtime,
        owner,
        identity,
        realm,
        normalized,
        boundary.take_resume(),
    )
}

pub(super) fn prepared(
    runtime: &Runtime,
    owner: ReturnOwner,
    identity: u64,
    realm: crate::engine::heap::ContextId,
    normalized: NormalizedConstructor,
    resume: Resume,
) -> Result<Step, Error> {
    let NormalizedConstructor {
        target,
        new_target,
        arguments,
    } = normalized;
    let mut boundary = ConstructorBoundary::new(runtime, resume);
    boundary.new_target = Some(new_target);
    boundary.arguments = arguments;
    let (callable, classification) = match target {
        ConstructorTarget::Proxy(target) => {
            return Ok(Step::ConstructProxy {
                target: Some(target),
                new_target: boundary.new_target.take(),
                arguments: Some(std::mem::take(&mut boundary.arguments)),
                resume: Some(boundary.take_resume()),
            });
        }
        ConstructorTarget::Ordinary {
            callable,
            classification,
        } => (callable, classification),
    };
    match classification {
        CallableExecution::Native {
            target,
            realm: defining_realm,
            min_readable_args,
        } => Ok(Step::Native {
            mode: Some(crate::engine::vm::call::NativeInvokeMode::Ordinary),
            callable: Some(callable),
            target: Some(target),
            defining_realm: Some(defining_realm),
            min_readable_args: Some(min_readable_args),
            invocation: Some(crate::engine::vm::call::NativeInvocation::Construct {
                new_target: boundary
                    .new_target
                    .take()
                    .expect("constructor new target")
                    .into_value(),
            }),
            arguments: Some(std::mem::take(&mut boundary.arguments)),
            resume: Some(boundary.take_resume()),
        }),
        CallableExecution::Bytecode {
            bytecode,
            closure_slots,
        } => {
            let kind = runtime
                .0
                .state
                .borrow()
                .heap
                .function_bytecode(bytecode.bytecode_id())
                .map_err(|error| Error::internal(error.to_string()))?
                .metadata
                .constructor_kind;
            let request = Box::new(BytecodeCallRequest {
                callable,
                receiver: JsValue::Undefined,
                new_target: boundary
                    .new_target
                    .take()
                    .expect("constructor new target")
                    .into_value(),
                arguments: std::mem::take(&mut boundary.arguments),
                bytecode,
                closure_slots,
                caller_realm: realm,
                return_to: ReturnTarget {
                    owner,
                    value_use: ReturnValue::Push,
                    tail: false,
                    operation: Some(OperationTarget::PropertyGet(identity)),
                },
            });
            boundary.request = Some(request);
            match kind {
                ConstructorKind::None => Err(Error::internal(
                    "constructor bit disagrees with bytecode constructor metadata",
                )),
                ConstructorKind::Derived => Ok(Step::ConstructorReady {
                    request: boundary.request.take(),
                    receiver: Some(Completion::Return(JsValue::Undefined)),
                    derived: Some(true),
                    resume: Some(boundary.take_resume()),
                }),
                ConstructorKind::Base
                    if matches!(
                        boundary
                            .request
                            .as_ref()
                            .expect("constructor request")
                            .new_target,
                        JsValue::Undefined
                    ) =>
                {
                    prototype(
                        runtime,
                        boundary.request.take().expect("constructor request"),
                        Completion::Return(JsValue::Undefined),
                        boundary.take_resume(),
                    )
                }
                ConstructorKind::Base => {
                    let key = runtime
                        .pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Prototype)
                        .map_err(|error| Error::internal(error.to_string()))?;
                    let receiver = runtime
                        .dup_jsvalue(
                            &boundary
                                .request
                                .as_ref()
                                .expect("constructor request")
                                .new_target,
                        )
                        .map_err(runtime_error_to_vm_error)?;
                    Ok(Step::ReadValue {
                        receiver: Some(receiver),
                        key: Some(key),
                        resume: Some(Resume::ConstructorPrototype {
                            request: boundary.request.take().expect("constructor request"),
                            resume: Box::new(boundary.take_resume()),
                        }),
                    })
                }
            }
        }
        _ => Err(Error::internal("constructor dispatch was not normalized")),
    }
}
pub(super) fn prototype(
    runtime: &Runtime,
    request: Box<BytecodeCallRequest>,
    completion: Completion,
    resume: Resume,
) -> Result<Step, Error> {
    let mut boundary = ConstructorBoundary::new(runtime, resume);
    boundary.request = Some(request);
    let request = boundary.request.as_ref().expect("constructor request");
    let receiver = runtime
        .create_from_constructor_prototype_reply(
            request.caller_realm,
            &request.new_target,
            completion,
        )
        .map_err(runtime_error_to_vm_error)?;
    Ok(Step::ConstructorReady {
        request: boundary.request.take(),
        receiver: Some(receiver),
        derived: Some(false),
        resume: Some(boundary.take_resume()),
    })
}
// The selected Step already owns this boxed request; consume it in place instead of
// moving its payload through the driver stack.
#[allow(clippy::boxed_local)]
pub(super) fn ready(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    query: &Query,
    request: Box<BytecodeCallRequest>,
    receiver: Completion,
    derived: bool,
    resume: Resume,
) -> Result<Result<Next, Step>, Error> {
    let mut boundary = ConstructorBoundary::new(runtime, resume);
    boundary.request = Some(request);
    boundary.receiver = Some(receiver);
    if matches!(boundary.receiver, Some(Completion::Throw(_))) {
        boundary
            .request
            .as_mut()
            .expect("constructor request")
            .release_owned_values(runtime)
            .map_err(runtime_error_to_vm_error)?;
        let completion = boundary.receiver.take().expect("constructor receiver");
        return Ok(Err(boundary
            .take_resume()
            .resume(runtime, completion)
            .map_err(runtime_error_to_vm_error)?));
    }
    if !execution
        .frames
        .can_push_with_continuations(query.continuation_depth())
        || runtime.bytecode_call_would_overflow()
    {
        let realm = boundary
            .request
            .as_ref()
            .expect("constructor request")
            .caller_realm;
        boundary
            .request
            .as_mut()
            .expect("constructor request")
            .release_owned_values(runtime)
            .map_err(runtime_error_to_vm_error)?;
        if let Some(Completion::Return(receiver)) = boundary.receiver.take() {
            runtime
                .release_jsvalue(receiver)
                .map_err(runtime_error_to_vm_error)?;
        }
        let completion = overflow(runtime, realm)?;
        return Ok(Err(boundary
            .take_resume()
            .resume(runtime, completion)
            .map_err(runtime_error_to_vm_error)?));
    }
    let Some(Completion::Return(receiver)) = boundary.receiver.as_ref() else {
        unreachable!()
    };
    boundary
        .request
        .as_mut()
        .expect("constructor request")
        .receiver = runtime
        .dup_jsvalue(receiver)
        .map_err(runtime_error_to_vm_error)?;
    if derived {
        let Some(Completion::Return(receiver)) = boundary.receiver.take() else {
            unreachable!()
        };
        runtime
            .release_jsvalue(receiver)
            .map_err(runtime_error_to_vm_error)?;
    }
    let request = boundary.request.take().expect("constructor request");
    let mut entry = request.prepare(runtime, &mut execution.call_storage)?;
    entry.cold.constructor_return = Some(if derived {
        crate::engine::vm::frame::ConstructorReturn::Derived
    } else {
        let Some(Completion::Return(receiver)) = boundary.receiver.take() else {
            unreachable!()
        };
        crate::engine::vm::frame::ConstructorReturn::Base(receiver)
    });
    Ok(Ok(Next::Call {
        entry: Box::new(entry),
        pc: 0,
        resume: boundary.take_resume(),
    }))
}
