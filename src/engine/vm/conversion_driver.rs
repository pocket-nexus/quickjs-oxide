//! Scheduling for a pending addition or unary-plus conversion. Domain phases remain in
//! value/conversion; only frame installation and reply routing live here.
mod local_add;
use crate::engine::api::{error::Error, runtime::Runtime};
use crate::engine::code::function::metadata::FunctionKind;
use crate::engine::object::{CallableRef, OrdinaryRead};
use crate::engine::value::Value;
use crate::engine::value::conversion::primitive::{PrimitiveResume, PrimitiveStep};
use crate::engine::vm::call::{BytecodeCallRequest, CallableExecution};
use crate::engine::vm::exception::runtime_error_to_vm_error;
use crate::engine::vm::execution::RunningExecution;
use crate::engine::vm::frame::{FrameId, ReturnTarget};
use crate::engine::vm::{Completion, ToPrimitiveHint};
pub(super) use local_add::complete_local_add;

enum Finish {
    Predicate(Box<super::predicate_driver::Input>),
    SuperProperty(Box<super::super_property_driver::Input>),
    Plus,
    PropertyKey,
    PropertyWrite {
        base: Value,
        value: Value,
    },
    PropertyRead {
        base: Value,
        keep_receiver: bool,
        keep_key: bool,
    },
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
    Predicate(Box<super::predicate_driver::Input>),
    SuperProperty(Box<super::super_property_driver::Input>),
    Ready(ConversionTask),
    Entered,
    Complete(Completion),
    PropertyRead(Box<super::property_driver::ConvertedRead>),
    PropertyWrite(Box<super::property_write_driver::ConvertedWrite>),
}

fn add_completion(
    runtime: &Runtime,
    realm: crate::engine::heap::ContextId,
    left: Value,
    right: Value,
) -> Result<Completion, Error> {
    match super::numeric::add_primitives(left, right) {
        Ok(value) => Ok(Completion::Return(value)),
        Err(error) => {
            let Some(kind) =
                crate::engine::api::error::NativeErrorKind::from_javascript_error(error.kind())
            else {
                return Err(error);
            };
            Ok(Completion::Throw(
                runtime
                    .new_native_error_from_error(realm, kind, &error)
                    .map_err(runtime_error_to_vm_error)?,
            ))
        }
    }
}

pub(super) enum PrimitiveCompletion {
    Completed,
    Throw(Value),
    Declined,
    InvalidDomain,
}

/// The fault PC is published before entry. The input borrow owns no values
/// across parsing, allocation, error materialization or final-owner release.
pub(super) fn complete_primitives(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    id: FrameId,
    addition: bool,
    next_operation: &mut u64,
) -> Result<PrimitiveCompletion, Error> {
    let frame = execution.frames.current_mut(id)?;
    let realm = frame.executable.realm;
    #[cfg(feature = "profiling")]
    let depth = execution.slots.depth(&frame.window);
    let mut transaction = execution.slots.frame_transaction(&mut frame.window)?;
    let (left, right, store) = {
        let mut slots = transaction.slots();
        // Preserve left-to-right domain validation, including checking a later
        // malformed slot after an earlier invalid domain, before identity issue.
        let mut invalid = false;
        for offset in (0..=usize::from(addition)).rev() {
            invalid |= runtime
                .validate_value_domain(slots.peek(offset)?, "conversion operand")
                .is_err();
        }
        if invalid {
            return Ok(PrimitiveCompletion::InvalidDomain);
        }
        *next_operation = next_operation
            .checked_add(1)
            .ok_or_else(|| Error::internal("conversion identity exhausted"))?;
        let store = if addition && frame.executable.fusion.add_store(frame.fault_pc) {
            use crate::engine::code::bytecode::Instruction;
            match frame.executable.code.get(frame.fault_pc + 1) {
                Some(
                    Instruction::PutLocal(index)
                    | Instruction::PutLocalCheck(index)
                    | Instruction::SetLocal(index)
                    | Instruction::SetLocalCheck(index),
                ) if matches!(
                    slots.local(*index)?,
                    super::bindings::FrameBinding::Direct(_)
                ) =>
                {
                    Some((
                        *index,
                        frame.executable.fusion.add_store_span(frame.fault_pc),
                        matches!(
                            slots.local(*index)?,
                            super::bindings::FrameBinding::Direct(
                                Value::Object(_) | Value::Symbol(_)
                            )
                        ),
                    ))
                }
                _ => None,
            }
        } else {
            None
        };
        for offset in 0..=usize::from(addition) {
            if matches!(slots.peek(offset)?, Value::Object(_)) {
                return Ok(PrimitiveCompletion::Declined);
            }
        }
        let right = slots.pop().expect("validated primitive conversion operand");
        let left = if addition {
            Some(
                slots
                    .pop()
                    .expect("validated primitive conversion left operand"),
            )
        } else {
            None
        };
        (left, right, store)
    };
    // End the authenticated window before String/BigInt allocation or release.
    let completion = if let Some(left) = left {
        add_completion(runtime, realm, left, right)?
    } else {
        match super::numeric::unary_plus_primitive(right) {
            Ok(value) => Completion::Return(value),
            Err(error) => {
                let Some(kind) =
                    crate::engine::api::error::NativeErrorKind::from_javascript_error(error.kind())
                else {
                    return Err(error);
                };
                Completion::Throw(
                    runtime
                        .new_native_error_from_error(realm, kind, &error)
                        .map_err(runtime_error_to_vm_error)?,
                )
            }
        }
    };
    #[cfg(feature = "profiling")]
    crate::engine::api::profiling::record_owned_execution_event(
        "conversion_completed_without_task",
    );
    Ok(match completion {
        Completion::Return(value) => {
            if let Some((index, span, observable_release)) = store {
                // Scalar/String/BigInt release cannot observe the runtime PC.
                // Object/Symbol release retains the canonical store publication
                // before replacement. No RunSlots borrow crosses either release.
                let store_pc = frame
                    .fault_pc
                    .checked_add(1)
                    .ok_or_else(|| Error::internal("conversion resume PC overflow"))?;
                if observable_release {
                    (frame.fault_pc, frame.resume_pc) = (store_pc, store_pc);
                    runtime
                        .update_active_bytecode_pc(
                            frame.active_frame,
                            super::BytecodePc::new(frame.fault_pc),
                        )
                        .map_err(runtime_error_to_vm_error)?;
                }
                let mut pending = Some(super::bindings::FrameBinding::Direct(value));
                let old = {
                    let mut slots = transaction.slots();
                    slots.replace_local_pending(index, &mut pending)
                };
                let old = match old {
                    Ok(old) => old,
                    Err(error) => {
                        if !observable_release {
                            (frame.fault_pc, frame.resume_pc) = (store_pc, store_pc);
                            runtime
                                .update_active_bytecode_pc(
                                    frame.active_frame,
                                    super::BytecodePc::new(frame.fault_pc),
                                )
                                .map_err(runtime_error_to_vm_error)?;
                        }
                        return Err(error);
                    }
                };
                drop(old);
                // The optional Drop only removes the assignment result while
                // the local keeps the value; it has no observable owner drain.
                let resume = store_pc
                    .checked_add(span - 1)
                    .ok_or_else(|| Error::internal("binding release resume PC overflow"))?;
                (frame.fault_pc, frame.resume_pc) = (resume - 1, resume);
                #[cfg(feature = "profiling")]
                {
                    crate::engine::api::profiling::record_owned_instruction(depth);
                    crate::engine::api::profiling::record_owned_instruction(depth - 1);
                    if span == 3 {
                        crate::engine::api::profiling::record_owned_instruction(depth - 1);
                    }
                    crate::engine::api::profiling::record_owned_execution_event(
                        "primitive_add_store_fused",
                    );
                }
            } else {
                let mut pending = Some(value);
                {
                    let mut slots = transaction.slots();
                    slots.push_pending(&mut pending)?;
                }
                frame.resume_pc = frame
                    .fault_pc
                    .checked_add(1)
                    .ok_or_else(|| Error::internal("conversion resume PC overflow"))?;
                #[cfg(feature = "profiling")]
                crate::engine::api::profiling::record_owned_instruction(depth);
            }
            PrimitiveCompletion::Completed
        }
        Completion::Throw(value) => PrimitiveCompletion::Throw(value),
    })
}

impl ConversionTask {
    #[cfg(feature = "profiling")]
    pub(super) fn operand_count(&self) -> usize {
        match &self.finish {
            Finish::SuperProperty(input) => input.operand_count(),
            Finish::Plus | Finish::PropertyKey => 1,
            Finish::PropertyWrite { .. } => 3,
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

    pub(super) fn start_predicate(
        runtime: &Runtime,
        execution: &mut RunningExecution,
        frame: FrameId,
        identity: u64,
        input: Box<super::predicate_driver::Input>,
    ) -> Result<Self, Error> {
        let realm = execution.frames.current_mut(frame)?.executable.realm;
        let step =
            PrimitiveResume::start(runtime, realm, input.key.clone(), ToPrimitiveHint::String);
        Ok(Self {
            finish: Finish::Predicate(input),
            frame,
            identity,
            step,
        })
    }

    pub(super) fn start_super_property(
        runtime: &Runtime,
        execution: &mut RunningExecution,
        frame: FrameId,
        identity: u64,
        input: Box<super::super_property_driver::Input>,
    ) -> Result<Self, Error> {
        let realm = execution.frames.current_mut(frame)?.executable.realm;
        let step =
            PrimitiveResume::start(runtime, realm, input.key.clone(), ToPrimitiveHint::String);
        Ok(Self {
            finish: Finish::SuperProperty(input),
            frame,
            identity,
            step,
        })
    }

    pub(super) fn start_property_write(
        runtime: &Runtime,
        execution: &mut RunningExecution,
        frame: FrameId,
        identity: u64,
    ) -> Result<Self, Error> {
        let parent = execution.frames.current_mut(frame)?;
        for (offset, label) in [
            (2, "property receiver"),
            (1, "property key"),
            (0, "property value"),
        ] {
            runtime
                .validate_value_domain(execution.slots.peek(&parent.window, offset)?, label)
                .map_err(runtime_error_to_vm_error)?;
        }
        let value = execution.slots.pop(&mut parent.window)?;
        let key = execution.slots.pop(&mut parent.window)?;
        let base = execution.slots.pop(&mut parent.window)?;
        Ok(Self {
            finish: Finish::PropertyWrite { base, value },
            frame,
            identity,
            step: PrimitiveResume::start(
                runtime,
                parent.executable.realm,
                key,
                ToPrimitiveHint::String,
            ),
        })
    }

    pub(super) fn start_property_read(
        runtime: &Runtime,
        execution: &mut RunningExecution,
        frame: FrameId,
        identity: u64,
        keep_receiver: bool,
        keep_key: bool,
    ) -> Result<Self, Error> {
        let parent = execution.frames.current_mut(frame)?;
        runtime
            .validate_value_domain(
                execution.slots.peek(&parent.window, 1)?,
                "property receiver",
            )
            .map_err(runtime_error_to_vm_error)?;
        runtime
            .validate_value_domain(execution.slots.peek(&parent.window, 0)?, "property key")
            .map_err(runtime_error_to_vm_error)?;
        let key = execution.slots.pop(&mut parent.window)?;
        let base = execution.slots.pop(&mut parent.window)?;
        Ok(Self {
            finish: Finish::PropertyRead {
                base,
                keep_receiver,
                keep_key,
            },
            frame,
            identity,
            step: PrimitiveResume::start(
                runtime,
                parent.executable.realm,
                key,
                ToPrimitiveHint::String,
            ),
        })
    }

    pub(super) fn reply(
        runtime: &Runtime,
        execution: &mut RunningExecution,
        target: ReturnTarget,
        completion: Completion,
    ) -> Result<Self, Error> {
        let parent = execution.frames.current_mut(target.frame()?)?;
        let wait = parent
            .cold
            .conversion
            .take()
            .ok_or_else(|| Error::internal("conversion reply has no pending owner"))?;
        if target.operation != Some(super::frame::OperationTarget::Conversion(wait.identity)) {
            return Err(Error::internal("conversion reply identity mismatch"));
        }
        Self::from_wait(runtime, target.frame()?, wait, completion)
    }

    pub(super) fn from_wait(
        runtime: &Runtime,
        frame: FrameId,
        wait: ConversionWait,
        completion: Completion,
    ) -> Result<Self, Error> {
        Ok(Self {
            finish: wait.finish,
            frame,
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
                                if !matches!(right, Value::Object(_)) {
                                    #[cfg(feature = "profiling")]
                                    crate::engine::api::profiling::record_owned_execution_event(
                                        "add_completed_with_primitive_rhs",
                                    );
                                    return Ok(Progress::Complete(add_completion(
                                        runtime, realm, value, right,
                                    )?));
                                }
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
                            Finish::AddRight(left) => add_completion(runtime, realm, left, value)?,
                            Finish::Predicate(mut input) => {
                                input.key = value;
                                return Ok(Progress::Predicate(input));
                            }
                            Finish::SuperProperty(mut input) => {
                                input.key = value;
                                return Ok(Progress::SuperProperty(input));
                            }
                            Finish::PropertyWrite {
                                base,
                                value: assigned,
                            } => {
                                return Ok(Progress::PropertyWrite(Box::new(
                                    super::property_write_driver::ConvertedWrite {
                                        base,
                                        key: value,
                                        value: assigned,
                                    },
                                )));
                            }
                            Finish::PropertyRead {
                                base,
                                keep_receiver,
                                keep_key,
                            } => {
                                return Ok(Progress::PropertyRead(Box::new(
                                    super::property_driver::ConvertedRead {
                                        base,
                                        key: value,
                                        keep_receiver,
                                        keep_key,
                                    },
                                )));
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
                            Finish::Plus => match super::numeric::unary_plus_primitive(value) {
                                Ok(value) => Completion::Return(value),
                                Err(error) => {
                                    let Some(kind) = crate::engine::api::error::NativeErrorKind::from_javascript_error(error.kind()) else { return Err(error); };
                                    Completion::Throw(
                                        runtime
                                            .new_native_error_from_error(realm, kind, &error)
                                            .map_err(runtime_error_to_vm_error)?,
                                    )
                                }
                            },
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
                        match super::proxy_get_driver::start_conversion(
                            runtime,
                            execution,
                            frame,
                            object,
                            key,
                            ConversionWait {
                                finish,
                                identity,
                                resume,
                            },
                        )? {
                            super::proxy_get_driver::Progress::Conversion(task) => {
                                Ok(Progress::Ready(task))
                            }
                            super::proxy_get_driver::Progress::Call(
                                super::driver::CallStep::Entered,
                            ) => Ok(Progress::Entered),
                            super::proxy_get_driver::Progress::Call(
                                super::driver::CallStep::Complete(completion),
                            ) => Ok(Progress::Complete(completion)),
                            super::proxy_get_driver::Progress::Call(
                                super::driver::CallStep::Bridge,
                            ) => Err(Error::internal(
                                "conversion property query attempted replay",
                            )),
                        }
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
    let super::call::NormalizedCallback {
        callable,
        receiver,
        arguments,
        classification,
    } = match super::call::normalize_callback(runtime, realm, callable, receiver, arguments)? {
        crate::engine::value::conversion::NativeConversion::Value(call) => call,
        crate::engine::value::conversion::NativeConversion::Throw(value) => {
            return Ok(Progress::Ready(ConversionTask {
                finish,
                frame,
                identity,
                step: resume
                    .resume(runtime, Completion::Throw(value))
                    .map_err(runtime_error_to_vm_error)?,
            }));
        }
    };
    let is_proxy = matches!(classification, CallableExecution::Proxy);
    let is_owned_native = matches!(&classification, CallableExecution::Native { .. }
        if super::frames::native_operation(runtime, &callable).map_err(runtime_error_to_vm_error)?.is_some());
    let is_resumable = if let CallableExecution::Bytecode { bytecode, .. } = &classification {
        runtime
            .0
            .state
            .borrow()
            .heap
            .function_bytecode(bytecode.bytecode_id())
            .map_err(|error| Error::internal(error.to_string()))?
            .metadata
            .function_kind
            != FunctionKind::Normal
    } else {
        false
    };
    if is_proxy || is_owned_native || is_resumable {
        let wait = ConversionWait {
            finish,
            identity,
            resume,
        };
        let progress = if is_proxy {
            super::proxy_get_driver::start_conversion_call(
                runtime,
                execution,
                frame,
                callable.as_object().clone(),
                receiver,
                arguments,
                wait,
            )?
        } else {
            super::proxy_get_driver::start_native_conversion_call(
                runtime, execution, frame, callable, receiver, arguments, wait,
            )?
        };
        return match progress {
            super::proxy_get_driver::Progress::Conversion(task) => Ok(Progress::Ready(task)),
            super::proxy_get_driver::Progress::Call(super::driver::CallStep::Entered) => {
                Ok(Progress::Entered)
            }
            super::proxy_get_driver::Progress::Call(super::driver::CallStep::Complete(
                completion,
            )) => Ok(Progress::Complete(completion)),
            super::proxy_get_driver::Progress::Call(super::driver::CallStep::Bridge) => {
                Err(Error::internal("conversion callback attempted replay"))
            }
        };
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
                    owner: crate::engine::vm::frame::ReturnOwner::Frame(frame),
                    tail: false,
                    operation: Some(super::frame::OperationTarget::Conversion(identity)),
                },
            };
            let entry = request.prepare(runtime, &mut execution.call_storage)?;
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
    #[cfg(feature = "profiling")]
    crate::engine::api::profiling::record_owned_sync_call_bridge();
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

#[cfg(all(test, feature = "profiling"))]
mod primitive_store_tests {
    use crate::engine::api::profiling::CostProfile;
    use crate::engine::api::{Runtime, Value};

    #[test]
    fn primitive_store_keeps_conversion_capture_and_throw_observations() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let profile = CostProfile::start();
        assert_eq!(
            context
                .eval(
                    r#"(()=>{
            let s='', b=1n;
            for(let i=0;i<20;i++){ s+='x'; b+=2n; }
            let capture=()=>s;
            s+='y';
            let old=s,trace='';
            try { s+=Symbol(); } catch(e) { if(e instanceof TypeError)trace+='throw'; }
            finally { trace+='finally'; }
            let a='left';
            a+= {valueOf(){a='changed';return 'right';}};
            let constant='a', read=constant;
            try { const x='a'; x+='b'; } catch(e) { if(e instanceof TypeError)read+='!'; }
            return s===old && capture()===old && s.length===21 && b===41n
                && trace==='throwfinally' && a==='leftright' && read==='a!';
        })()"#
                )
                .unwrap(),
            Value::Bool(true)
        );
        assert!(
            profile
                .snapshot()
                .owned_execution_events
                .get("primitive_add_store_fused")
                .copied()
                .unwrap_or(0)
                > 0
        );
    }
}
