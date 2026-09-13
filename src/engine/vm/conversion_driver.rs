//! Scheduling for a pending addition or unary-plus conversion. Domain phases remain in
//! value/conversion; only frame installation and reply routing live here.
use crate::engine::api::{error::Error, runtime::Runtime};
use crate::engine::code::function::metadata::FunctionKind;
use crate::engine::object::{CallableRef, OrdinaryRead};
use crate::engine::value::Value;
use crate::engine::value::conversion::{
    NativeConversion,
    primitive::{PrimitiveResume, PrimitiveStep},
};
use crate::engine::vm::call::{BytecodeCallRequest, CallableExecution};
use crate::engine::vm::exception::runtime_error_to_vm_error;
use crate::engine::vm::execution::RunningExecution;
use crate::engine::vm::frame::{FrameId, ReturnTarget};
use crate::engine::vm::{Completion, ToPrimitiveHint};

enum Finish {
    Plus,
    PropertyKey,
    AddLeft(Value),
    AddRight(Value),
}

pub(super) struct ConversionWait {
    finish: Finish,
    identity: u64,
    resume: PrimitiveResume,
}

pub(super) struct ConversionTask {
    finish: Finish,
    frame: FrameId,
    identity: u64,
    step: PrimitiveStep,
}

pub(super) enum Progress {
    Ready(ConversionTask),
    Entered,
    Complete(Completion),
}

impl ConversionTask {
    #[cfg(feature = "profiling")]
    pub(super) fn operand_count(&self) -> usize {
        match self.finish {
            Finish::Plus | Finish::PropertyKey => 1,
            _ => 2,
        }
    }

    pub(super) fn start(
        runtime: &Runtime,
        execution: &mut RunningExecution,
        frame: FrameId,
        identity: u64,
        addition: bool,
        property_key: bool,
    ) -> Result<Self, Error> {
        let parent = execution.frames.current_mut(frame)?;
        let right = execution.slots.pop(&mut parent.window)?;
        let (value, finish, hint) = if addition {
            (
                execution.slots.pop(&mut parent.window)?,
                Finish::AddLeft(right),
                ToPrimitiveHint::Default,
            )
        } else if property_key {
            (right, Finish::PropertyKey, ToPrimitiveHint::String)
        } else {
            (right, Finish::Plus, ToPrimitiveHint::Number)
        };
        Ok(Self {
            finish,
            frame,
            identity,
            step: PrimitiveResume::start(runtime, parent.executable.realm, value, hint),
        })
    }

    pub(super) fn reply(
        runtime: &Runtime,
        execution: &mut RunningExecution,
        target: ReturnTarget,
        completion: Completion,
    ) -> Result<Self, Error> {
        let parent = execution.frames.current_mut(target.frame)?;
        let wait = parent
            .cold
            .conversion
            .take()
            .ok_or_else(|| Error::internal("conversion reply has no pending owner"))?;
        if target.operation != Some(super::frame::OperationTarget::Conversion(wait.identity)) {
            return Err(Error::internal("conversion reply identity mismatch"));
        }
        Ok(Self {
            finish: wait.finish,
            frame: target.frame,
            identity: wait.identity,
            step: wait
                .resume
                .resume(runtime, completion)
                .map_err(runtime_error_to_vm_error)?,
        })
    }

    pub(super) fn advance(
        self,
        runtime: &Runtime,
        execution: &mut RunningExecution,
    ) -> Result<Progress, Error> {
        let Self {
            finish,
            frame,
            identity,
            step,
        } = self;
        let realm = execution.frames.current_mut(frame)?.executable.realm;
        match step {
            PrimitiveStep::Complete(completion) => {
                let completion = match completion {
                    Completion::Throw(value) => Completion::Throw(value),
                    Completion::Return(value) => {
                        // Domain completion guarantees a primitive: this call
                        // cannot recursively perform another ToPrimitive.
                        if matches!(value, Value::Object(_)) {
                            return Err(Error::internal("conversion returned an object"));
                        }
                        match finish {
                            Finish::AddLeft(right) => {
                                return Ok(Progress::Ready(Self {
                                    frame,
                                    identity,
                                    finish: Finish::AddRight(value),
                                    step: PrimitiveResume::start(
                                        runtime,
                                        realm,
                                        right,
                                        ToPrimitiveHint::Default,
                                    ),
                                }));
                            }
                            Finish::AddRight(left) => {
                                match super::numeric::add_primitives(left, value) {
                                    Ok(value) => Completion::Return(value),
                                    Err(error) => {
                                        let Some(kind) = crate::engine::api::error::NativeErrorKind::from_javascript_error(error.kind()) else { return Err(error); };
                                        Completion::Throw(
                                            runtime
                                                .new_native_error_from_error(realm, kind, &error)
                                                .map_err(runtime_error_to_vm_error)?,
                                        )
                                    }
                                }
                            }
                            Finish::PropertyKey => {
                                let value = match value {
                                    Value::Symbol(symbol) => {
                                        if !symbol.belongs_to(runtime) {
                                            return Err(Error::internal(
                                                "computed property symbol belongs to another runtime",
                                            ));
                                        }
                                        Value::Symbol(symbol)
                                    }
                                    Value::String(string) => Value::String(string),
                                    primitive => Value::String(primitive.to_js_string()?),
                                };
                                Completion::Return(value)
                            }
                            Finish::Plus => {
                                match runtime
                                    .native_to_number(realm, &value)
                                    .map_err(runtime_error_to_vm_error)?
                                {
                                    NativeConversion::Value(number) => {
                                        Completion::Return(Value::number(number))
                                    }
                                    NativeConversion::Throw(value) => Completion::Throw(value),
                                }
                            }
                        }
                    }
                };
                Ok(Progress::Complete(completion))
            }
            PrimitiveStep::Get {
                object,
                key,
                resume,
            } => {
                let read = runtime
                    .prepare_ordinary_read(&object, &key, Value::Object(object.clone()))
                    .map_err(runtime_error_to_vm_error)?;
                match read {
                    OrdinaryRead::Call { getter, receiver } => invoke(
                        runtime,
                        execution,
                        frame,
                        identity,
                        finish,
                        getter,
                        receiver,
                        Vec::new(),
                        resume,
                    ),
                    OrdinaryRead::Complete(value) => Ok(Progress::Ready(Self {
                        finish,
                        frame,
                        identity,
                        step: resume
                            .resume(
                                runtime,
                                Completion::Return(value.unwrap_or(Value::Undefined)),
                            )
                            .map_err(runtime_error_to_vm_error)?,
                    })),
                    OrdinaryRead::Special { .. } => {
                        // Only this unresolved read crosses the transitional
                        // boundary. Earlier getters and calls are never replayed.
                        let completion = runtime
                            .get_property_in_realm(realm, &object, &key)
                            .map_err(runtime_error_to_vm_error)?;
                        Ok(Progress::Ready(Self {
                            finish,
                            frame,
                            identity,
                            step: resume
                                .resume(runtime, completion)
                                .map_err(runtime_error_to_vm_error)?,
                        }))
                    }
                }
            }
            PrimitiveStep::Call {
                callable,
                receiver,
                arguments,
                resume,
            } => invoke(
                runtime, execution, frame, identity, finish, callable, receiver, arguments, resume,
            ),
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn invoke(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    frame: FrameId,
    identity: u64,
    finish: Finish,
    callable: CallableRef,
    receiver: Value,
    arguments: Vec<Value>,
    resume: PrimitiveResume,
) -> Result<Progress, Error> {
    let realm = execution.frames.current_mut(frame)?.executable.realm;
    if let CallableExecution::Bytecode {
        bytecode,
        closure_slots,
    } = runtime
        .bytecode_for_callable(&callable)
        .map_err(runtime_error_to_vm_error)?
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
            if !execution.frames.can_push() || runtime.bytecode_call_would_overflow() {
                let completion = runtime
                    .bytecode_stack_overflow_completion(realm, &bytecode)
                    .map_err(runtime_error_to_vm_error)?;
                return Ok(Progress::Ready(ConversionTask {
                    finish,
                    frame,
                    identity,
                    step: resume
                        .resume(runtime, completion)
                        .map_err(runtime_error_to_vm_error)?,
                }));
            }
            let request = BytecodeCallRequest {
                callable,
                receiver,
                arguments,
                new_target: Value::Undefined,
                bytecode,
                closure_slots,
                caller_realm: realm,
                return_to: ReturnTarget {
                    value_use: crate::engine::vm::frame::ReturnValue::Push,
                    frame,
                    tail: false,
                    operation: Some(super::frame::OperationTarget::Conversion(identity)),
                },
            };
            let entry = request.prepare(runtime)?;
            let parent = execution.frames.current_mut(frame)?;
            if parent.cold.conversion.is_some() {
                return Err(Error::internal(
                    "conversion overwrote an unanswered request",
                ));
            }
            parent.cold.conversion = Some(ConversionWait {
                identity,
                resume,
                finish,
            });
            super::driver::push_frame(execution, entry)?;
            return Ok(Progress::Entered);
        }
    }
    let completion = runtime
        .call_internal(realm, &callable, receiver, &arguments)
        .map_err(runtime_error_to_vm_error)?;
    Ok(Progress::Ready(ConversionTask {
        finish,
        frame,
        identity,
        step: resume
            .resume(runtime, completion)
            .map_err(runtime_error_to_vm_error)?,
    }))
}
