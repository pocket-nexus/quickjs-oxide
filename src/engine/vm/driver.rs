//! Own frames and advance ordinary bytecode calls without native recursion.

use crate::engine::api::error::Error;
use crate::engine::api::runtime::Runtime;
use crate::engine::code::function::metadata::FunctionKind;
use crate::engine::value::Value;
use crate::engine::value::conversion::NativeConversion;
use crate::engine::vm::call::{BytecodeCallRequest, CallableExecution};
use crate::engine::vm::exception::runtime_error_to_vm_error;
use crate::engine::vm::execution::{ExecutionLimits, RunningExecution};
use crate::engine::vm::frame::{Frame, FrameEntry, FrameId, ReturnTarget};
use crate::engine::vm::run::{RunExit, run};
#[cfg(test)]
use crate::engine::vm::stack::FrameStorage;
use crate::engine::vm::{BytecodePc, Completion};

pub(super) fn push_frame(
    execution: &mut RunningExecution,
    entry: FrameEntry,
) -> Result<FrameId, Error> {
    let window = execution
        .slots
        .push_frame(&entry.executable.frame_layout(), entry.storage)?;
    execution.frames.push(Frame {
        executable: entry.executable,
        cold: entry.cold,
        window,
        fault_pc: 0,
        resume_pc: 0,
    })
}

pub(super) fn prepare_captured_reuse(
    frame: &mut Frame,
    slots: &super::stack::SlotStore,
) -> Result<(), Error> {
    if frame.cold.reusable_captured_locals.len() != frame.executable.local_definitions.len() {
        return Err(Error::internal(
            "reusable captured-local flags disagree with the frame",
        ));
    }
    for (index, reusable) in frame.cold.reusable_captured_locals.iter_mut().enumerate() {
        *reusable = matches!(
            slots.local(&frame.window, index as u16)?,
            super::bindings::FrameBinding::Captured(_)
        );
    }
    Ok(())
}

pub(super) enum CallStep {
    Entered,
    Complete(Completion),
    Bridge,
}

/// All classification and allocation occurs after publishing the caller's PC.
/// Unsupported callable kinds leave every operand and the resume PC untouched.
pub(super) fn enter_call(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    id: FrameId,
    count: u16,
    method: bool,
    tail: bool,
) -> Result<CallStep, Error> {
    let frame = execution.frames.current_mut(id)?;
    let window = &mut frame.window;
    let count = usize::from(count);
    execution.slots.peek(window, count + usize::from(method))?;
    let Value::Object(object) = execution.slots.peek(window, count)? else {
        return Ok(CallStep::Bridge);
    };
    if !object.belongs_to(runtime) {
        return Ok(CallStep::Bridge);
    }
    let Some(mut callable) = runtime
        .as_callable(object)
        .map_err(runtime_error_to_vm_error)?
    else {
        return Ok(CallStep::Bridge);
    };
    // Keep the existing rejection order and exception materialization until
    // general call errors join the owned unwind path. Nothing was consumed.
    if method
        && runtime
            .validate_value_domain(execution.slots.peek(window, count + 1)?, "call this value")
            .is_err()
    {
        return Ok(CallStep::Bridge);
    }
    for offset in (0..count).rev() {
        if runtime
            .validate_value_domain(execution.slots.peek(window, offset)?, "call argument")
            .is_err()
        {
            return Ok(CallStep::Bridge);
        }
    }
    let realm = frame.executable.realm;
    let mut bound_arguments = None;
    let mut bound_receiver = None;
    let (bytecode, closure_slots) = loop {
        match runtime
            .bytecode_for_callable(&callable)
            .map_err(runtime_error_to_vm_error)?
        {
            CallableExecution::Bytecode {
                bytecode,
                closure_slots,
            } => {
                break (bytecode, closure_slots);
            }
            CallableExecution::Bound {
                target,
                this_value,
                arguments,
            } => {
                let call_arguments = match bound_arguments.take() {
                    Some(arguments) => arguments,
                    None => {
                        let mut arguments = Vec::new();
                        arguments
                            .try_reserve_exact(count)
                            .map_err(|_| Error::internal("call arguments allocation failed"))?;
                        for offset in (0..count).rev() {
                            arguments.push(execution.slots.peek(window, offset)?.clone());
                        }
                        arguments
                    }
                };
                bound_arguments = Some(
                    match runtime
                        .concatenate_bound_arguments(realm, &arguments, &call_arguments)
                        .map_err(runtime_error_to_vm_error)?
                    {
                        NativeConversion::Value(arguments) => arguments,
                        NativeConversion::Throw(value) => {
                            return Ok(CallStep::Complete(Completion::Throw(value)));
                        }
                    },
                );
                bound_receiver = Some(this_value);
                callable = target;
            }
            _ => {
                return super::call_bridge::prepare(
                    runtime, execution, id, count, method, tail, None,
                );
            }
        }
    };
    let kind = runtime
        .0
        .state
        .borrow()
        .heap
        .function_bytecode(bytecode.bytecode_id())
        .map_err(|error| Error::internal(error.to_string()))?
        .metadata
        .function_kind;
    if kind != FunctionKind::Normal {
        let overflow = (!execution.frames.can_push() || runtime.bytecode_call_would_overflow())
            .then_some(bytecode);
        return super::call_bridge::prepare(runtime, execution, id, count, method, tail, overflow);
    }
    #[cfg(feature = "profiling")]
    let observed_depth = execution.slots.depth(window);
    if !execution.frames.can_push() || runtime.bytecode_call_would_overflow() {
        return runtime
            .bytecode_stack_overflow_completion(realm, &bytecode)
            .map(CallStep::Complete)
            .map_err(runtime_error_to_vm_error);
    }
    let mut arguments = Vec::new();
    arguments
        .try_reserve_exact(count)
        .map_err(|_| Error::internal("call arguments allocation failed"))?;
    let frame = execution.frames.current_mut(id)?;
    for _ in 0..count {
        arguments.push(execution.slots.pop(&mut frame.window)?);
    }
    arguments.reverse();
    let function = execution.slots.pop(&mut frame.window)?;
    let receiver = if method {
        execution.slots.pop(&mut frame.window)?
    } else {
        Value::Undefined
    };
    // The checked callable root now owns the popped callee identity.
    drop(function);
    if let Some(normalized) = bound_arguments {
        arguments = normalized;
    }
    let receiver = bound_receiver.unwrap_or(receiver);
    let request = BytecodeCallRequest {
        callable,
        receiver,
        new_target: Value::Undefined,
        arguments,
        bytecode,
        closure_slots,
        caller_realm: realm,
        return_to: ReturnTarget {
            value_use: crate::engine::vm::frame::ReturnValue::Push,
            frame: id,
            tail,
            operation: None,
        },
    };
    frame.resume_pc = frame
        .fault_pc
        .checked_add(1)
        .ok_or_else(|| Error::internal("call resume PC overflow"))?;
    let entry = request.prepare(runtime)?;
    push_frame(execution, entry)?;
    #[cfg(feature = "profiling")]
    crate::engine::api::profiling::record_owned_instruction(observed_depth);
    Ok(CallStep::Entered)
}

pub(super) fn execute(
    runtime: Runtime,
    entry: FrameEntry,
    limits: ExecutionLimits,
) -> Result<RunningExit, Error> {
    let mut execution = RunningExecution::new(&runtime, limits)?;
    push_frame(&mut execution, entry)?;
    run_frames(&runtime, execution)
}

pub(super) enum RunningExit {
    Complete(Completion),
    RootHandoff(Box<super::frame_exit::RootHandoff>),
    Call(Box<CallContinuation>),
}

pub(super) struct CallContinuation {
    execution: RunningExecution,
    conversion: Option<super::conversion_driver::ConversionTask>,
    next_operation: u64,
}

impl CallContinuation {
    #[inline(never)]
    fn invoke(&mut self, runtime: &Runtime) -> Result<Option<Completion>, Error> {
        let call = self
            .execution
            .pending_call
            .take()
            .ok_or_else(|| Error::internal("call continuation has no request"))?;
        call.invoke(runtime, &mut self.execution)
    }

    #[inline(never)]
    fn resume(
        self: Box<Self>,
        runtime: &Runtime,
        forwarded: Option<Completion>,
    ) -> Result<RunningExit, Error> {
        let Self {
            execution,
            conversion,
            next_operation,
        } = *self;
        run_frames_with_state(runtime, execution, forwarded, conversion, next_operation)
    }
}

impl RunningExit {
    pub(super) fn finish(self, runtime: Runtime) -> Result<Completion, Error> {
        let mut exit = self;
        loop {
            match exit {
                Self::Complete(completion) => return Ok(completion),
                Self::RootHandoff(handoff) => return handoff.execute(runtime),
                Self::Call(mut continuation) => {
                    let forwarded = continuation.invoke(&runtime)?;
                    exit = continuation.resume(&runtime, forwarded)?;
                }
            }
        }
    }
}

#[inline(never)]
fn run_frames(runtime: &Runtime, execution: RunningExecution) -> Result<RunningExit, Error> {
    run_frames_with_state(runtime, execution, None, None, 0)
}

#[inline(never)]
fn run_frames_with_state(
    runtime: &Runtime,
    mut execution: RunningExecution,
    mut forwarded: Option<Completion>,
    mut conversion: Option<super::conversion_driver::ConversionTask>,
    mut next_operation: u64,
) -> Result<RunningExit, Error> {
    loop {
        if execution.pending_call.is_some() {
            if forwarded.is_some() {
                return Err(Error::internal("pending call conflicts with a completion"));
            }
            return Ok(RunningExit::Call(Box::new(CallContinuation {
                execution,
                conversion,
                next_operation,
            })));
        }
        let id = execution
            .frames
            .current_id()
            .ok_or_else(|| Error::internal("driver lost its current frame"))?;
        let mut exit = if let Some(task) = conversion.take() {
            use crate::engine::vm::conversion_driver::Progress;
            #[cfg(feature = "profiling")]
            let operands =
                crate::engine::vm::conversion_driver::ConversionTask::operand_count(&task);
            match crate::engine::vm::conversion_driver::ConversionTask::advance(
                task,
                runtime,
                &mut execution,
            )? {
                Progress::Ready(task) => {
                    conversion = Some(task);
                    continue;
                }
                Progress::Entered => continue,
                Progress::Complete(Completion::Return(value)) => {
                    let frame = execution.frames.current_mut(id)?;
                    #[cfg(feature = "profiling")]
                    let depth = execution.slots.depth(&frame.window) + operands;
                    execution.slots.push(&mut frame.window, value)?;
                    frame.resume_pc = frame
                        .fault_pc
                        .checked_add(1)
                        .ok_or_else(|| Error::internal("conversion resume PC overflow"))?;
                    #[cfg(feature = "profiling")]
                    crate::engine::api::profiling::record_owned_instruction(depth);
                    continue;
                }
                Progress::Complete(completion) => {
                    forwarded = Some(completion);
                    RunExit::Complete
                }
                Progress::PropertyRead(input) => {
                    match super::property_driver::read_converted(
                        runtime,
                        &mut execution,
                        id,
                        input,
                    )? {
                        CallStep::Entered => continue,
                        CallStep::Complete(completion) => {
                            forwarded = Some(completion);
                            RunExit::Complete
                        }
                        CallStep::Bridge => {
                            return Err(Error::internal(
                                "converted property read attempted replay",
                            ));
                        }
                    }
                }
            }
        } else if forwarded.is_some() {
            RunExit::Complete
        } else {
            let result = run(&mut execution, id);
            let frame = execution.frames.current_mut(id)?;
            runtime
                .update_active_bytecode_pc(frame.cold.active_frame, BytecodePc::new(frame.fault_pc))
                .map_err(runtime_error_to_vm_error)?;
            result?
        };
        if let RunExit::Environment(super::environment_driver::Operation::Has { source, name }) =
            exit
        {
            next_operation = next_operation
                .checked_add(1)
                .ok_or_else(|| Error::internal("operation identity exhausted"))?;
            match super::with_driver::start(
                runtime,
                &mut execution,
                id,
                source,
                name,
                next_operation,
            )? {
                CallStep::Entered => continue,
                CallStep::Complete(completion) => {
                    forwarded = Some(completion);
                    exit = RunExit::Complete;
                }
                CallStep::Bridge => exit = RunExit::Bridge,
            }
        }
        if let RunExit::Environment(op) = exit {
            match super::environment_driver::step(runtime, &mut execution, id, op)? {
                CallStep::Entered => continue,
                CallStep::Complete(completion) => {
                    forwarded = Some(completion);
                    exit = RunExit::Complete;
                }
                CallStep::Bridge => exit = RunExit::Bridge,
            }
        }
        if let RunExit::DefineProperty { key, method } = exit {
            match super::construct_driver::define_property(
                runtime,
                &mut execution,
                id,
                key,
                method,
            )? {
                CallStep::Entered => continue,
                CallStep::Complete(completion) => {
                    forwarded = Some(completion);
                    exit = RunExit::Complete;
                }
                CallStep::Bridge => exit = RunExit::Bridge,
            }
        }
        if let RunExit::DefineClass { name, has_heritage } = exit {
            next_operation = next_operation
                .checked_add(1)
                .ok_or_else(|| Error::internal("operation identity exhausted"))?;
            match super::construct_driver::define_class(
                runtime,
                &mut execution,
                id,
                name,
                has_heritage,
                next_operation,
            )? {
                CallStep::Entered => continue,
                CallStep::Complete(completion) => {
                    forwarded = Some(completion);
                    exit = RunExit::Complete;
                }
                CallStep::Bridge => exit = RunExit::Bridge,
            }
        }
        if let RunExit::ClassInitializer(mode) = exit {
            match super::construct_driver::initializer(runtime, &mut execution, id, mode)? {
                CallStep::Entered => continue,
                CallStep::Complete(completion) => {
                    forwarded = Some(completion);
                    exit = RunExit::Complete;
                }
                CallStep::Bridge => {
                    return Err(Error::internal("class initialization attempted replay"));
                }
            }
        }
        if let Some(step) = super::frame_operations::step(runtime, &mut execution, id, exit)? {
            match step {
                CallStep::Entered => continue,
                CallStep::Complete(completion) => {
                    forwarded = Some(completion);
                    exit = RunExit::Complete;
                }
                CallStep::Bridge => exit = RunExit::Bridge,
            }
        }
        if matches!(
            exit,
            RunExit::Construct(_) | RunExit::InitDerivedConstructor | RunExit::Apply(_)
        ) {
            next_operation = next_operation
                .checked_add(1)
                .ok_or_else(|| Error::internal("operation identity exhausted"))?;
            let step = match exit {
                RunExit::Apply(kind) => {
                    super::apply_driver::step(runtime, &mut execution, id, kind, next_operation)?
                }
                RunExit::Construct(count) => super::construct_driver::enter(
                    runtime,
                    &mut execution,
                    id,
                    count,
                    next_operation,
                )?,
                _ => super::construct_driver::enter_default_derived(
                    runtime,
                    &mut execution,
                    id,
                    next_operation,
                )?,
            };
            match step {
                CallStep::Entered => continue,
                CallStep::Complete(completion) => {
                    forwarded = Some(completion);
                    exit = RunExit::Complete;
                }
                CallStep::Bridge => exit = RunExit::Bridge,
            }
        }
        if matches!(
            exit,
            RunExit::ConvertPlus | RunExit::ConvertAdd | RunExit::ConvertPropertyKey
        ) {
            let frame = execution.frames.current_mut(id)?;
            let addition = exit == RunExit::ConvertAdd;
            let mut invalid = false;
            for offset in (0..=usize::from(addition)).rev() {
                invalid |= runtime
                    .validate_value_domain(
                        execution.slots.peek(&frame.window, offset)?,
                        "conversion operand",
                    )
                    .is_err();
            }
            if invalid {
                exit = RunExit::Bridge;
            } else {
                next_operation = next_operation
                    .checked_add(1)
                    .ok_or_else(|| Error::internal("conversion identity exhausted"))?;
                conversion = Some(crate::engine::vm::conversion_driver::ConversionTask::start(
                    runtime,
                    &mut execution,
                    id,
                    next_operation,
                    addition,
                    exit == RunExit::ConvertPropertyKey,
                )?);
                continue;
            }
        }
        if let RunExit::ApplyEval(environment) = exit {
            match super::eval_driver::apply(runtime, &mut execution, id, environment)? {
                CallStep::Entered => continue,
                CallStep::Complete(completion) => {
                    forwarded = Some(completion);
                    exit = RunExit::Complete;
                }
                CallStep::Bridge => exit = RunExit::Bridge,
            }
        }
        if let RunExit::Eval {
            arguments,
            environment,
        } = exit
        {
            match super::eval_driver::step(runtime, &mut execution, id, arguments, environment)? {
                CallStep::Entered => continue,
                CallStep::Complete(completion) => {
                    forwarded = Some(completion);
                    exit = RunExit::Complete;
                }
                CallStep::Bridge => exit = RunExit::Bridge,
            }
        }
        if let RunExit::GetField {
            index,
            keep_receiver,
        } = exit
        {
            match super::property_driver::read(
                runtime,
                &mut execution,
                id,
                super::property_driver::ReadKey::Static(index),
                keep_receiver,
            )? {
                CallStep::Entered => continue,
                CallStep::Complete(completion) => {
                    forwarded = Some(completion);
                    exit = RunExit::Complete;
                }
                CallStep::Bridge => exit = RunExit::Bridge,
            }
        }
        if let RunExit::GetElement {
            keep_receiver,
            keep_key,
        } = exit
        {
            let frame = execution.frames.current_mut(id)?;
            if !matches!(
                execution.slots.peek(&frame.window, 1)?,
                Value::Null | Value::Undefined
            ) && matches!(execution.slots.peek(&frame.window, 0)?, Value::Object(_))
            {
                next_operation = next_operation
                    .checked_add(1)
                    .ok_or_else(|| Error::internal("property conversion identity exhausted"))?;
                conversion = Some(
                    super::conversion_driver::ConversionTask::start_property_read(
                        runtime,
                        &mut execution,
                        id,
                        next_operation,
                        keep_receiver,
                        keep_key,
                    )?,
                );
                continue;
            }
            match super::property_driver::read(
                runtime,
                &mut execution,
                id,
                super::property_driver::ReadKey::Computed { keep_key },
                keep_receiver,
            )? {
                CallStep::Entered => continue,
                CallStep::Complete(completion) => {
                    forwarded = Some(completion);
                    exit = RunExit::Complete;
                }
                CallStep::Bridge => exit = RunExit::Bridge,
            }
        }
        if let RunExit::Call {
            arguments,
            method,
            tail,
        } = exit
        {
            match enter_call(runtime, &mut execution, id, arguments, method, tail)? {
                CallStep::Entered => continue,
                CallStep::Complete(completion) => {
                    forwarded = Some(completion);
                    exit = RunExit::Complete;
                }
                CallStep::Bridge => exit = RunExit::Bridge,
            }
        }
        if matches!(forwarded, Some(Completion::Throw(_))) {
            let Some(Completion::Throw(value)) = forwarded.take() else {
                unreachable!()
            };
            match super::iterator_driver::unwind(runtime, &mut execution, id, value)? {
                CallStep::Entered => continue,
                CallStep::Complete(completion) => forwarded = Some(completion),
                CallStep::Bridge => {
                    return Err(Error::internal("unwind attempted instruction replay"));
                }
            }
        }
        let (completion, return_to) =
            match super::frame_exit::finish(runtime, &mut execution, id, exit, forwarded.take())? {
                super::frame_exit::FrameExit::Complete {
                    completion,
                    return_to,
                } => (completion, return_to),
                super::frame_exit::FrameExit::RootHandoff(handoff) => {
                    drop(execution);
                    return Ok(RunningExit::RootHandoff(handoff));
                }
            };
        let Some(target) = return_to else {
            return Ok(RunningExit::Complete(completion));
        };
        execution.frames.current_mut(target.frame)?;
        if matches!(
            target.operation,
            Some(super::frame::OperationTarget::Constructor(_))
        ) {
            match super::construct_driver::reply(runtime, &mut execution, target, completion)? {
                CallStep::Entered => continue,
                CallStep::Complete(completion) => {
                    forwarded = Some(completion);
                    continue;
                }
                CallStep::Bridge => {
                    return Err(Error::internal(
                        "constructor reply attempted instruction replay",
                    ));
                }
            }
        }
        if matches!(
            target.operation,
            Some(super::frame::OperationTarget::ClassDefinition(_))
        ) {
            match super::construct_driver::reply_class(runtime, &mut execution, target, completion)?
            {
                CallStep::Entered => continue,
                CallStep::Complete(completion) => {
                    forwarded = Some(completion);
                    continue;
                }
                CallStep::Bridge => return Err(Error::internal("class reply attempted replay")),
            }
        }
        if matches!(
            target.operation,
            Some(
                super::frame::OperationTarget::HasBinding(_)
                    | super::frame::OperationTarget::Iterator(_)
            )
        ) {
            match super::environment_driver::reply(runtime, &mut execution, target, completion)? {
                CallStep::Entered => continue,
                CallStep::Complete(completion) => {
                    forwarded = Some(completion);
                    continue;
                }
                CallStep::Bridge => {
                    return Err(Error::internal("HasBinding reply attempted replay"));
                }
            }
        }
        if let Some(super::frame::OperationTarget::Eval(arguments)) = target.operation {
            let parent = execution.frames.current_mut(target.frame)?;
            parent.cold.eval_arguments = None;
            for _ in 0..=arguments {
                execution.slots.pop(&mut parent.window)?;
            }
            match completion {
                Completion::Return(value) => execution.slots.push(&mut parent.window, value)?,
                completion => forwarded = Some(completion),
            }
            continue;
        }
        if target.operation.is_some() {
            conversion = Some(crate::engine::vm::conversion_driver::ConversionTask::reply(
                runtime,
                &mut execution,
                target,
                completion,
            )?);
            continue;
        }
        match completion {
            Completion::Return(value) if !target.tail => {
                let parent = execution.frames.current_mut(target.frame)?;
                if matches!(target.value_use, super::frame::ReturnValue::Push) {
                    execution.slots.push(&mut parent.window, value)?;
                }
            }
            completion => forwarded = Some(completion),
        }
    }
}

#[cfg(all(test, feature = "profiling"))]
mod tests {
    use super::*;
    use crate::engine::api::profiling::CostProfile;
    use crate::engine::vm::frame::FrameCold;

    fn execute(
        runtime: Runtime,
        entry: FrameEntry,
        limits: ExecutionLimits,
    ) -> Result<Completion, Error> {
        super::execute(runtime.clone(), entry, limits)?.finish(runtime)
    }

    fn execute_running(runtime: Runtime, execution: RunningExecution) -> Result<Completion, Error> {
        super::run_frames(&runtime, execution)?.finish(runtime)
    }

    fn entry(
        runtime: &Runtime,
        context: &mut crate::engine::api::Context,
        source: &str,
        arguments: Vec<Value>,
    ) -> FrameEntry {
        let Value::Object(function) = context.eval(source).unwrap() else {
            panic!("expected function");
        };
        let callable = runtime.as_callable(&function).unwrap().unwrap();
        callable_entry(runtime, context, callable, arguments)
    }

    fn callable_entry(
        runtime: &Runtime,
        context: &mut crate::engine::api::Context,
        callable: crate::engine::object::CallableRef,
        arguments: Vec<Value>,
    ) -> FrameEntry {
        let function = callable.as_object().clone();
        let CallableExecution::Bytecode {
            bytecode,
            closure_slots,
        } = runtime.bytecode_for_callable(&callable).unwrap()
        else {
            panic!("expected bytecode");
        };
        let prepared = runtime
            .prepare_bytecode_frame(
                &callable,
                Value::Undefined,
                Value::Undefined,
                &arguments,
                bytecode,
            )
            .unwrap();
        let locals = prepared.locals.len();
        FrameEntry {
            executable: prepared.executable,
            cold: Box::new(FrameCold {
                regions: Vec::new(),
                constructor_wait: None,
                class_wait: None,
                has_binding_wait: None,
                iterator_wait: None,
                iterator_generation: 0,
                eval_arguments: None,
                constructor_return: None,
                conversion: None,
                normalized_this: None,
                return_to: None,
                active_frame: prepared.active_frame.token(),
                entry_guard: Some(prepared.active_frame),
                caller_realm: context.realm,
                function,
                closure_slots,
                reusable_captured_locals: vec![false; locals],
                input: prepared.input,
            }),
            storage: FrameStorage {
                original_arguments: arguments,
                parameters: prepared.arguments,
                locals: prepared.locals,
                operands: Vec::new(),
            },
        }
    }

    #[test]
    fn lexical_initialization_and_tdz_use_owned_slots() {
        for (source, error_message) in [
            (
                "(function(){var sum=0,i=0; while(i<3){let x=i+13; sum=sum+x; i=i+1} return sum})",
                None,
            ),
            (
                "(function(){var i=0; while(i<2){if(i===1)return x; let x=42; i=i+1}})",
                Some("x is not initialized"),
            ),
            ("(function(){let x=40; x=x+1; const y=1; return x+y})", None),
            (
                "(function(){return x; let x=42})",
                Some("x is not initialized"),
            ),
            ("(function(){x=42; let x})", Some("x is not initialized")),
            (
                "(function(){let x=x; return x})",
                Some("x is not initialized"),
            ),
        ] {
            let runtime = Runtime::new();
            let mut context = runtime.new_context();
            let entry = entry(&runtime, &mut context, source, Vec::new());
            let profile = CostProfile::start();
            let completion = execute(runtime.clone(), entry, ExecutionLimits::default()).unwrap();
            let costs = profile.snapshot();
            assert_eq!(costs.legacy_dispatches, 0, "{source}: {costs:?}");
            assert_eq!(costs.owned_bridge_exits, 0, "{source}: {costs:?}");
            match (completion, error_message) {
                (Completion::Return(value), None) => assert_eq!(value, Value::Int(42)),
                (Completion::Throw(Value::Object(error)), Some(message)) => {
                    for (key, expected) in [("name", "ReferenceError"), ("message", message)] {
                        assert_eq!(
                            context
                                .get_property(&error, &runtime.intern_property_key(key).unwrap())
                                .unwrap(),
                            Value::String(crate::engine::value::JsString::from_static(expected))
                        );
                    }
                }
                _ => panic!("unexpected lexical completion: {source}"),
            }
            assert!(runtime.0.state.borrow().active_frames.is_empty());
        }
    }

    #[test]
    fn closure_reads_and_writes_preserve_live_cells_and_tdz() {
        for (source, throws) in [
            (
                "(function(){var x=40;return function(){x=x+1;return x}})()",
                false,
            ),
            (
                "(function(){let x=40;return function(){x=x+1;return x}})()",
                false,
            ),
            (
                "(function(){const x=42;return function(){return x}})()",
                false,
            ),
            ("(function(){return function(){return x};let x})()", true),
            ("(function(){return function(){x=42};let x})()", true),
        ] {
            let runtime = Runtime::new();
            let mut context = runtime.new_context();
            let closure = context.eval(source).unwrap();
            let entry = entry(
                &runtime,
                &mut context,
                "(function(f){f();return f()})",
                vec![closure],
            );
            let profile = CostProfile::start();
            let completion = execute(runtime.clone(), entry, ExecutionLimits::default()).unwrap();
            let costs = profile.snapshot();
            assert_eq!(costs.legacy_dispatches, 0, "{source}: {costs:?}");
            assert_eq!(costs.owned_bridge_exits, 0, "{source}: {costs:?}");
            assert_eq!(costs.owned_storage.maximum_frame_depth, 2);
            match (completion, throws) {
                (Completion::Return(value), false) => assert_eq!(value, Value::Int(42)),
                (Completion::Throw(Value::Object(error)), true) => {
                    for (key, expected) in [
                        ("name", "ReferenceError"),
                        ("message", "x is not initialized"),
                    ] {
                        assert_eq!(
                            context
                                .get_property(&error, &runtime.intern_property_key(key).unwrap())
                                .unwrap(),
                            Value::String(crate::engine::value::JsString::from_static(expected))
                        );
                    }
                }
                _ => panic!("unexpected closure completion: {source}"),
            }
            assert!(runtime.0.state.borrow().active_frames.is_empty());
        }
    }

    #[test]
    fn catch_and_finally_resume_owned_frames_and_survive_handoff() {
        for (source, bridge) in [
            ("(function(){try{throw 40}catch(e){return e+2}})", false),
            ("(function(f){try{return f()}catch(e){return e+2}})", false),
            ("(function(){try{return 1}finally{return 42}})", false),
            (
                "(function(){var x=0;try{try{throw 40}finally{x=2}}catch(e){return e+x}})",
                false,
            ),
            (
                "(function(){try{String(1);throw 40}catch(e){return e+2}})",
                true,
            ),
        ] {
            let runtime = Runtime::new();
            let mut context = runtime.new_context();
            let throwing = context.eval("(function(){throw 40})").unwrap();
            let entry = entry(&runtime, &mut context, source, vec![throwing]);
            let profile = CostProfile::start();
            let completion = execute(runtime.clone(), entry, ExecutionLimits::default()).unwrap();
            let Completion::Return(value) = completion else {
                panic!("expected caught return: {source}")
            };
            assert_eq!(value, Value::Int(42), "{source}");
            let costs = profile.snapshot();
            assert_eq!(costs.owned_bridge_exits != 0, bridge, "{source}: {costs:?}");
            if !bridge {
                assert_eq!(costs.legacy_dispatches, 0, "{source}: {costs:?}");
            }
            assert!(runtime.0.state.borrow().active_frames.is_empty());
        }
    }

    #[test]
    fn created_closures_keep_captured_values_after_scope_and_frame_exit() {
        for source in [
            "(function(){let x=42;return function(){return x}})",
            "(function(x){return function(){return x}})",
            "(function(){var f; {let x=42; f=function(){return x}} return f})",
            "(function(){var f,g,i=41;while(i<43){let x=i;if(i===41)f=function(){return x};else g=function(){return x};i=i+1}return function(){return f()+g()-41}})",
        ] {
            let runtime = Runtime::new();
            let mut context = runtime.new_context();
            let call_entry = entry(&runtime, &mut context, source, vec![Value::Int(42)]);
            let code = call_entry.executable.code.clone();
            let profile = CostProfile::start();
            let completion =
                execute(runtime.clone(), call_entry, ExecutionLimits::default()).unwrap();
            let Completion::Return(Value::Object(closure)) = completion else {
                panic!("expected closure")
            };
            let costs = profile.snapshot();
            assert_eq!(
                costs.owned_bridge_exits, 0,
                "{source}: {costs:?}, code: {code:?}"
            );
            assert_eq!(
                costs.legacy_dispatches, 0,
                "{source}: {costs:?}, code: {code:?}"
            );
            drop(profile);
            let entry = entry(
                &runtime,
                &mut context,
                "(function(f){return f()})",
                vec![Value::Object(closure)],
            );
            let profile = CostProfile::start();
            let completion = execute(runtime.clone(), entry, ExecutionLimits::default()).unwrap();
            let Completion::Return(value) = completion else {
                panic!("expected closure value")
            };
            assert_eq!(value, Value::Int(42));
            assert_eq!(profile.snapshot().legacy_dispatches, 0);
            assert!(runtime.0.state.borrow().active_frames.is_empty());
        }
    }

    #[test]
    fn captured_parent_slots_share_writes_initialization_and_abrupt_reuse() {
        for source in [
            "(function(){let x=40;var f=function(){x=x+1};f();x=x+1;return x})",
            "(function(x){var f=function(){x=x+1};f();x=x+1;return x})",
            "(function(){function f(){return x}let x=42;return f()})",
            "(function(){function f(){return x}try{return x}catch(e){return 42}let x})",
            "(function(){var i=0,f;while(i<2){try{let x=40+i;if(i===0)f=function(){return x};i=i+1;throw 0}catch(e){}}return f()+1})",
        ] {
            let runtime = Runtime::new();
            let mut context = runtime.new_context();
            let entry = entry(&runtime, &mut context, source, vec![Value::Int(40)]);
            let profile = CostProfile::start();
            let completion = execute(runtime.clone(), entry, ExecutionLimits::default()).unwrap();
            let Completion::Return(value) = completion else {
                panic!("expected captured return: {source}")
            };
            assert_eq!(value, Value::Int(42), "{source}");
            let costs = profile.snapshot();
            assert_eq!(costs.legacy_dispatches, 0, "{source}: {costs:?}");
            assert_eq!(costs.owned_bridge_exits, 0, "{source}: {costs:?}");
            assert!(runtime.0.state.borrow().active_frames.is_empty());
        }
    }

    #[test]
    fn arguments_and_rest_preserve_actual_arity_and_parameter_aliasing() {
        for (source, arguments, expected) in [
            (
                "(function(a){'use strict';a=2;return arguments})",
                vec![Value::Int(1)],
                vec![Value::Int(1)],
            ),
            (
                "(function(a){a=2;return arguments})",
                vec![Value::Int(1), Value::Int(40)],
                vec![Value::Int(2), Value::Int(40)],
            ),
            (
                "(function(a,b){return arguments})",
                vec![Value::Int(42)],
                vec![Value::Int(42)],
            ),
            (
                "(function(a,...rest){return rest})",
                vec![Value::Int(1), Value::Int(40), Value::Int(2)],
                vec![Value::Int(40), Value::Int(2)],
            ),
            (
                "(function(a,b,...rest){return rest})",
                vec![Value::Int(1)],
                vec![],
            ),
        ] {
            let runtime = Runtime::new();
            let mut context = runtime.new_context();
            let entry = entry(&runtime, &mut context, source, arguments);
            let profile = CostProfile::start();
            let completion = execute(runtime.clone(), entry, ExecutionLimits::default()).unwrap();
            let Completion::Return(Value::Object(object)) = completion else {
                panic!("expected arguments or rest object: {source}")
            };
            let costs = profile.snapshot();
            assert_eq!(costs.owned_bridge_exits, 0, "{source}: {costs:?}");
            assert_eq!(costs.legacy_dispatches, 0, "{source}: {costs:?}");
            drop(profile);
            assert_eq!(
                context
                    .get_property(&object, &runtime.intern_property_key("length").unwrap())
                    .unwrap(),
                Value::Int(expected.len() as i32)
            );
            for (index, value) in expected.into_iter().enumerate() {
                assert_eq!(
                    context
                        .get_property(
                            &object,
                            &runtime.intern_property_key(&index.to_string()).unwrap()
                        )
                        .unwrap(),
                    value,
                    "{source}: {index}"
                );
            }
            assert!(runtime.0.state.borrow().active_frames.is_empty());
        }
    }

    #[test]
    fn default_parameters_keep_order_tdz_and_supplied_values() {
        for (source, arguments) in [
            (
                "(function(a=missing){return a===null?42:0})",
                vec![Value::Null],
            ),
            (
                "(function(a=missing){return a===false?42:0})",
                vec![Value::Bool(false)],
            ),
            ("(function(a=40,b=a+2){return b})", vec![]),
            ("(function(a=40,b=a+2){return b})", vec![Value::Undefined]),
            (
                "(function(a=40,b=a+2){return b})",
                vec![Value::Int(1), Value::Int(42)],
            ),
            (
                "(function(a=40,b=function(){return a}){a=2;return a+b()})",
                vec![],
            ),
            ("(function(f){try{return f()}catch(e){return e}})", vec![]),
        ] {
            let runtime = Runtime::new();
            let mut context = runtime.new_context();
            let arguments = if source.contains("try{") {
                vec![context.eval("(function(a=b,b=42){return a})").unwrap()]
            } else {
                arguments
            };
            let entry = entry(&runtime, &mut context, source, arguments);
            let profile = CostProfile::start();
            let completion = execute(runtime.clone(), entry, ExecutionLimits::default()).unwrap();
            let Completion::Return(value) = completion else {
                panic!("expected parameter return: {source}")
            };
            let costs = profile.snapshot();
            assert_eq!(costs.owned_bridge_exits, 0, "{source}: {costs:?}");
            assert_eq!(costs.legacy_dispatches, 0, "{source}: {costs:?}");
            drop(profile);
            if source.contains("try{") {
                let Value::Object(error) = value else {
                    panic!("expected TDZ error")
                };
                assert_eq!(
                    context
                        .get_property(&error, &runtime.intern_property_key("name").unwrap())
                        .unwrap(),
                    Value::String(crate::engine::value::JsString::from_static(
                        "ReferenceError"
                    ))
                );
            } else {
                assert_eq!(value, Value::Int(42));
            }
            assert!(runtime.0.state.borrow().active_frames.is_empty());
        }
    }

    #[test]
    fn readonly_binding_errors_preserve_tdz_and_rhs_order() {
        for (source, name, message) in [
            (
                "(function(){const x=1;x=42})",
                "TypeError",
                "'x' is read-only",
            ),
            (
                "(function f(){'use strict';f=42})",
                "TypeError",
                "'f' is read-only",
            ),
            (
                "(function(){const x=1;return function(){x=42}})()",
                "TypeError",
                "'x' is read-only",
            ),
            (
                "(function(){x=42;const x=1})",
                "TypeError",
                "'x' is read-only",
            ),
        ] {
            let runtime = Runtime::new();
            let mut context = runtime.new_context();
            let entry = entry(&runtime, &mut context, source, vec![]);
            let profile = CostProfile::start();
            let completion = execute(runtime.clone(), entry, ExecutionLimits::default()).unwrap();
            let Completion::Throw(Value::Object(error)) = completion else {
                panic!("expected binding error: {source}")
            };
            let costs = profile.snapshot();
            assert_eq!(costs.owned_bridge_exits, 0, "{source}: {costs:?}");
            assert_eq!(costs.legacy_dispatches, 0, "{source}: {costs:?}");
            drop(profile);
            for (key, expected) in [("name", name), ("message", message)] {
                assert_eq!(
                    context
                        .get_property(&error, &runtime.intern_property_key(key).unwrap())
                        .unwrap(),
                    Value::String(crate::engine::value::JsString::from_static(expected))
                );
            }
            assert!(runtime.0.state.borrow().active_frames.is_empty());
        }
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let entry = entry(
            &runtime,
            &mut context,
            "(function(){const x=0;function rhs(){throw 42}try{x=rhs()}catch(e){return e}})",
            vec![],
        );
        let profile = CostProfile::start();
        let result = execute(runtime.clone(), entry, ExecutionLimits::default()).unwrap();
        let Completion::Return(value) = result else {
            panic!("expected RHS throw to win")
        };
        assert_eq!(value, Value::Int(42));
        assert_eq!(profile.snapshot().legacy_dispatches, 0);
    }

    #[test]
    fn private_field_initialization_uses_fresh_identity_in_published_frames() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let mut names = Vec::new();
        for _ in 0..2 {
            let entry = entry(
                &runtime,
                &mut context,
                "(function(){return class {#x=42; read(){return this.#x}}})",
                vec![],
            );
            let pc = entry
                .executable
                .code
                .iter()
                .position(|op| {
                    matches!(
                        op,
                        crate::engine::code::bytecode::Instruction::InitializePrivateName(_)
                    )
                })
                .unwrap();
            let mut execution =
                RunningExecution::new(&runtime, ExecutionLimits::default()).unwrap();
            let id = push_frame(&mut execution, entry).unwrap();
            execution.frames.current_mut(id).unwrap().resume_pc = pc;
            let RunExit::PrivateInitialize { index, kind } = run(&mut execution, id).unwrap()
            else {
                panic!("expected private initialization boundary")
            };
            let frame = execution.frames.current_mut(id).unwrap();
            runtime
                .update_active_bytecode_pc(frame.cold.active_frame, BytecodePc::new(frame.fault_pc))
                .unwrap();
            assert!(
                super::super::private_bindings::step(&runtime, &mut execution, id, index, kind)
                    .unwrap()
                    .is_none()
            );
            let frame = execution.frames.current_mut(id).unwrap();
            let super::super::bindings::FrameBinding::Private(name) =
                execution.slots.local(&frame.window, index).unwrap()
            else {
                panic!("expected private identity owner")
            };
            names.push(name.clone());
            assert_eq!(frame.resume_pc, pc + 1);
            drop(execution);
            assert!(runtime.0.state.borrow().active_frames.is_empty());
        }
        assert_ne!(names[0], names[1]);
    }

    #[test]
    fn private_field_reads_writes_and_membership_use_owned_frames() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let instance = context.eval("new (class {#x=40;#f=function(){return this};bump(){this.#x=this.#x+2;return this.#x}has(o){return #x in o}read(o){return o.#x}call(){return this.#f()}})").unwrap();
        let foreign = context.eval("({})").unwrap();
        for (source, argument, expected) in [
            (
                "(function(o,x){return o.bump()})",
                Value::Undefined,
                Value::Int(42),
            ),
            (
                "(function(o,x){return o.has(x)})",
                instance.clone(),
                Value::Bool(true),
            ),
            (
                "(function(o,x){return o.has(x)})",
                foreign.clone(),
                Value::Bool(false),
            ),
            (
                "(function(o,x){return o.call()})",
                Value::Undefined,
                instance.clone(),
            ),
        ] {
            let entry = entry(
                &runtime,
                &mut context,
                source,
                vec![instance.clone(), argument],
            );
            let profile = CostProfile::start();
            let result = execute(runtime.clone(), entry, ExecutionLimits::default()).unwrap();
            let Completion::Return(value) = result else {
                panic!("expected private result")
            };
            assert_eq!(value, expected);
            let costs = profile.snapshot();
            assert_eq!(costs.owned_bridge_exits, 0, "{source}: {costs:?}");
            assert_eq!(costs.legacy_dispatches, 0, "{source}: {costs:?}");
        }
        let entry = entry(
            &runtime,
            &mut context,
            "(function(o,x){try{return o.read(x)}catch(e){return e}})",
            vec![instance, foreign],
        );
        let profile = CostProfile::start();
        let result = execute(runtime.clone(), entry, ExecutionLimits::default()).unwrap();
        let Completion::Return(Value::Object(error)) = result else {
            panic!("expected private brand error")
        };
        assert_eq!(profile.snapshot().legacy_dispatches, 0);
        drop(profile);
        assert_eq!(
            context
                .get_property(&error, &runtime.intern_property_key("name").unwrap())
                .unwrap(),
            Value::String(crate::engine::value::JsString::from_static("TypeError"))
        );
        assert!(runtime.0.state.borrow().active_frames.is_empty());
    }

    #[test]
    fn private_methods_preserve_identity_receiver_brand_and_readonly() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let instance = context.eval("new (class {#m(){return this}call(){return this.#m()}get(){return this.#m}has(x){return #m in x}read(x){return x.#m}write(){this.#m=1}})").unwrap();
        let foreign = context.eval("({})").unwrap();
        for (source, argument, expected) in [
            (
                "(function(o,x){return o.call()})",
                Value::Undefined,
                instance.clone(),
            ),
            (
                "(function(o,x){return o.get()===o.get()})",
                Value::Undefined,
                Value::Bool(true),
            ),
            (
                "(function(o,x){return o.has(x)})",
                instance.clone(),
                Value::Bool(true),
            ),
            (
                "(function(o,x){return o.has(x)})",
                foreign.clone(),
                Value::Bool(false),
            ),
        ] {
            let entry = entry(
                &runtime,
                &mut context,
                source,
                vec![instance.clone(), argument],
            );
            let profile = CostProfile::start();
            let result = execute(runtime.clone(), entry, ExecutionLimits::default()).unwrap();
            let Completion::Return(value) = result else {
                panic!("expected private method result")
            };
            assert_eq!(value, expected);
            let costs = profile.snapshot();
            assert_eq!(costs.legacy_dispatches, 0, "{source}: {costs:?}");
            assert_eq!(costs.owned_bridge_exits, 0, "{source}: {costs:?}");
        }
        for source in [
            "(function(o,x){try{return o.read(x)}catch(e){return e}})",
            "(function(o,x){try{return o.write()}catch(e){return e}})",
        ] {
            let entry = entry(
                &runtime,
                &mut context,
                source,
                vec![instance.clone(), foreign.clone()],
            );
            let profile = CostProfile::start();
            let result = execute(runtime.clone(), entry, ExecutionLimits::default()).unwrap();
            let Completion::Return(Value::Object(error)) = result else {
                panic!("expected private error")
            };
            assert_eq!(profile.snapshot().legacy_dispatches, 0);
            drop(profile);
            assert_eq!(
                context
                    .get_property(&error, &runtime.intern_property_key("name").unwrap())
                    .unwrap(),
                Value::String(crate::engine::value::JsString::from_static("TypeError"))
            );
        }
        assert!(runtime.0.state.borrow().active_frames.is_empty());
    }

    #[test]
    fn private_accessors_use_child_frames_and_discard_setter_returns() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let instance = context.eval("new (class {#x=40;get #value(){return this.#x}set #value(v){this.#x=v;return this}run(){try{this.#value=this.#value+2}catch(e){throw e}return this.#value}})").unwrap();
        let call_entry = entry(
            &runtime,
            &mut context,
            "(function(o){return o.run()})",
            vec![instance],
        );
        let profile = CostProfile::start();
        let result = execute(runtime.clone(), call_entry, ExecutionLimits::default()).unwrap();
        let Completion::Return(value) = result else {
            panic!("expected accessor result")
        };
        assert_eq!(value, Value::Int(42));
        let costs = profile.snapshot();
        assert_eq!(costs.legacy_dispatches, 0, "{costs:?}");
        assert_eq!(costs.owned_bridge_exits, 0, "{costs:?}");
        assert_eq!(costs.owned_storage.maximum_frame_depth, 3);
        assert_eq!(costs.owned_storage.frames_pushed, 5);
        drop(profile);
        for source in [
            "new (class {get #value(){throw 42}run(){return this.#value}})",
            "new (class {set #value(v){throw v}run(){this.#value=42;return 0}})",
        ] {
            let instance = context.eval(source).unwrap();
            let call_entry = entry(
                &runtime,
                &mut context,
                "(function(o){try{return o.run()}catch(e){return e}})",
                vec![instance],
            );
            let profile = CostProfile::start();
            let result = execute(runtime.clone(), call_entry, ExecutionLimits::default()).unwrap();
            let Completion::Return(value) = result else {
                panic!("expected accessor throw to reach caller")
            };
            assert_eq!(value, Value::Int(42));
            assert_eq!(profile.snapshot().legacy_dispatches, 0);
            assert_eq!(profile.snapshot().owned_bridge_exits, 0);
        }
        assert!(runtime.0.state.borrow().active_frames.is_empty());
    }

    #[test]
    fn private_getter_returned_function_keeps_receiver_and_runs_once() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let instance = context.eval("new (class {#count=0;get #fn(){this.#count=this.#count+1;return function(){return this}}run(){return this.#fn()}count(){return this.#count}})").unwrap();
        let call_entry = entry(
            &runtime,
            &mut context,
            "(function(o){var result=o.run();return result===o?o.count():0})",
            vec![instance],
        );
        let profile = CostProfile::start();
        let result = execute(runtime.clone(), call_entry, ExecutionLimits::default()).unwrap();
        let Completion::Return(value) = result else {
            panic!("expected getter method result")
        };
        assert_eq!(value, Value::Int(1));
        let costs = profile.snapshot();
        assert_eq!(costs.legacy_dispatches, 0, "{costs:?}");
        assert_eq!(costs.owned_bridge_exits, 0, "{costs:?}");
        assert_eq!(costs.owned_storage.frames_pushed, 5);
        assert_eq!(costs.owned_storage.maximum_frame_depth, 3);
        assert!(runtime.0.state.borrow().active_frames.is_empty());
    }

    #[test]
    fn instance_initializers_run_as_owned_children_and_propagate_throws() {
        for (source, wrapper) in [
            (
                "(class {#x=42;#m(){return this.#x}read(){return this.#m()}})",
                "(function(C){return new C().read()})",
            ),
            (
                "(class extends (function B(){}) {#x=42;read(){return this.#x}})",
                "(function(C){return new C().read()})",
            ),
            (
                "(class {#x=(function(){throw 42})()})",
                "(function(C){try{return new C()}catch(e){return e}})",
            ),
        ] {
            let runtime = Runtime::new();
            let mut context = runtime.new_context();
            let constructor = context.eval(source).unwrap();
            let entry = entry(&runtime, &mut context, wrapper, vec![constructor]);
            let profile = CostProfile::start();
            let completion = execute(runtime.clone(), entry, ExecutionLimits::default()).unwrap();
            let Completion::Return(value) = completion else {
                panic!("expected instance initializer result: {source}")
            };
            assert_eq!(value, Value::Int(42), "{source}");
            let costs = profile.snapshot();
            assert_eq!(costs.legacy_dispatches, 0, "{source}: {costs:?}");
            assert_eq!(costs.owned_bridge_exits, 0, "{source}: {costs:?}");
            assert!(costs.owned_storage.maximum_frame_depth >= 3);
            assert!(runtime.0.state.borrow().active_frames.is_empty());
        }
    }

    #[test]
    fn base_class_creation_and_initialization_throw_through_owned_frames() {
        for source in [
            "(function(){try{class C {static{throw 42}}}catch(e){return e}})",
            "(function(){try{class C {#x=(function(){throw 42})()}return new C()}catch(e){return e}})",
            "(function(){class C extends null {}return 42})",
            "(function(){try{class C extends 1 {}}catch(e){return e}})",
        ] {
            let runtime = Runtime::new();
            let mut context = runtime.new_context();
            let entry = entry(&runtime, &mut context, source, vec![]);
            let profile = CostProfile::start();
            let Completion::Return(value) =
                execute(runtime.clone(), entry, ExecutionLimits::default()).unwrap()
            else {
                panic!("expected class result: {source}")
            };
            let costs = profile.snapshot();
            if let Value::Object(error) = value {
                assert!(source.contains("extends 1"), "{source}");
                assert_eq!(
                    context
                        .get_property(&error, &runtime.intern_property_key("name").unwrap())
                        .unwrap(),
                    Value::String(crate::engine::value::JsString::from_static("TypeError"))
                );
            } else {
                assert!(!source.contains("extends 1"), "{source}");
                assert_eq!(value, Value::Int(42), "{source}");
            }
            assert_eq!(costs.legacy_dispatches, 0, "{source}: {costs:?}");
            assert_eq!(costs.owned_bridge_exits, 0, "{source}: {costs:?}");
            assert!(runtime.0.state.borrow().active_frames.is_empty());
        }
    }

    #[test]
    fn class_methods_are_published_and_called_without_handoff() {
        for source in [
            "(function(){class C {#x=42;read(){return this.#x}}return new C().read()})",
            "(function(){class C {get x(){return 42}}return new C().x})",
            "(function(){class B {read(){return 42}}class C extends B {}return new C().read()})",
            "(function(){class C {#x=42;get x(){return this.#x}set x(v){this.#x=v}}return new C().x})",
        ] {
            let runtime = Runtime::new();
            let mut context = runtime.new_context();
            let entry = entry(&runtime, &mut context, source, vec![]);
            let profile = CostProfile::start();
            let Completion::Return(value) =
                execute(runtime.clone(), entry, ExecutionLimits::default()).unwrap()
            else {
                panic!("expected method result: {source}")
            };
            assert_eq!(value, Value::Int(42));
            let costs = profile.snapshot();
            assert_eq!(costs.legacy_dispatches, 0, "{source}: {costs:?}");
            assert_eq!(costs.owned_bridge_exits, 0, "{source}: {costs:?}");
            assert!(runtime.0.state.borrow().active_frames.is_empty());
        }
    }

    #[test]
    fn public_fields_define_own_data_and_skip_inherited_setters() {
        for source in [
            "(function(){class C {x=42}return new C().x})",
            "(function(){class B {set x(v){throw 99}}class C extends B {x=42}return new C().x})",
            "(function(){class C {x=40;y=this.x+2}return new C().y})",
            "(function(){class C {f=function(){return this.x};x=42}return new C().f()})",
            "(function(){try{class C {x=(function(){throw 42})()}return new C()}catch(e){return e}})",
        ] {
            let runtime = Runtime::new();
            let mut context = runtime.new_context();
            let entry = entry(&runtime, &mut context, source, vec![]);
            let profile = CostProfile::start();
            let Completion::Return(value) =
                execute(runtime.clone(), entry, ExecutionLimits::default()).unwrap()
            else {
                panic!("expected field result: {source}")
            };
            assert_eq!(value, Value::Int(42), "{source}");
            let costs = profile.snapshot();
            assert_eq!(costs.legacy_dispatches, 0, "{source}: {costs:?}");
            assert_eq!(costs.owned_bridge_exits, 0, "{source}: {costs:?}");
            assert!(runtime.0.state.borrow().active_frames.is_empty());
        }
    }

    #[test]
    fn computed_class_keys_convert_once_before_definition() {
        for source in [
            "(function(k,count){class C {[k]=41}return new C().x+count()})",
            "(function(k,count){class C {[k](){return 41}}return new C().x()+count()})",
            "(function(k,count){class C {get [k](){return 41}set x(v){throw 99}}return new C().x+count()})",
        ] {
            let runtime = Runtime::new();
            let mut context = runtime.new_context();
            let Value::Object(pair) = context.eval("(function(){var n=0;return {key:{toString:function(){n=n+1;return 'x'},valueOf:function(){throw 99}},count:function(){return n}}})()").unwrap()
            else {panic!("expected key setup")};
            let key = context
                .get_property(&pair, &runtime.intern_property_key("key").unwrap())
                .unwrap();
            let count = context
                .get_property(&pair, &runtime.intern_property_key("count").unwrap())
                .unwrap();
            let entry = entry(&runtime, &mut context, source, vec![key, count]);
            let profile = CostProfile::start();
            let Completion::Return(value) =
                execute(runtime.clone(), entry, ExecutionLimits::default()).unwrap()
            else {
                panic!("expected computed definition: {source}")
            };
            assert_eq!(value, Value::Int(42), "{source}");
            let costs = profile.snapshot();
            assert_eq!(costs.legacy_dispatches, 0, "{source}: {costs:?}");
            assert_eq!(costs.owned_bridge_exits, 0, "{source}: {costs:?}");
            assert!(runtime.0.state.borrow().active_frames.is_empty());
        }
        for (key_source, source) in [
            (
                "Symbol('x')",
                "(function(k){class C {[k]=42}return new C()})",
            ),
            ("1.5", "(function(k){class C {[k]=42}return new C()})"),
            ("-1", "(function(k){class C {[k]=42}return new C()})"),
            (
                "({toString:function(){throw 42}})",
                "(function(k){try{class C {[k]=(function(){throw 99})()}return new C()}catch(e){return e}})",
            ),
        ] {
            let runtime = Runtime::new();
            let mut context = runtime.new_context();
            let key = context.eval(key_source).unwrap();
            let expected_key = if matches!(key, Value::Object(_)) {
                None
            } else {
                let NativeConversion::Value(key) = runtime
                    .native_to_property_key(context.realm, key.clone())
                    .unwrap()
                else {
                    panic!("expected primitive key")
                };
                Some(key)
            };
            let entry = entry(&runtime, &mut context, source, vec![key]);
            let profile = CostProfile::start();
            let Completion::Return(value) =
                execute(runtime.clone(), entry, ExecutionLimits::default()).unwrap()
            else {
                panic!("expected computed result")
            };
            let costs = profile.snapshot();
            if let Some(key) = expected_key {
                let Value::Object(instance) = value else {
                    panic!("expected computed instance")
                };
                assert_eq!(
                    context.get_property(&instance, &key).unwrap(),
                    Value::Int(42)
                );
            } else {
                assert_eq!(value, Value::Int(42));
            }
            assert_eq!(costs.legacy_dispatches, 0, "{source}: {costs:?}");
            assert_eq!(costs.owned_bridge_exits, 0, "{source}: {costs:?}");
            assert!(runtime.0.state.borrow().active_frames.is_empty());
        }
    }

    #[test]
    fn computed_function_field_names_keep_key_identity_and_existing_names() {
        for (key_source, expected, initializer) in [
            ("'x'", "x", "function(){}"),
            ("-1", "-1", "function(){}"),
            ("Symbol('x')", "[x]", "function(){}"),
            ("Symbol()", "", "function(){}"),
            ("Symbol('')", "[]", "function(){}"),
            ("'x'", "keep", "function keep(){}"),
        ] {
            let runtime = Runtime::new();
            let mut context = runtime.new_context();
            let key = context.eval(key_source).unwrap();
            let NativeConversion::Value(property) = runtime
                .native_to_property_key(context.realm, key.clone())
                .unwrap()
            else {
                panic!("expected canonical key")
            };
            let source = format!("(function(k){{class C {{[k]={initializer}}}return new C()}})");
            let entry = entry(&runtime, &mut context, &source, vec![key]);
            let profile = CostProfile::start();
            let Completion::Return(Value::Object(instance)) =
                execute(runtime.clone(), entry, ExecutionLimits::default()).unwrap()
            else {
                panic!("expected instance")
            };
            let costs = profile.snapshot();
            let Value::Object(function) = context.get_property(&instance, &property).unwrap()
            else {
                panic!("expected function field")
            };
            assert_eq!(
                context
                    .get_property(&function, &runtime.intern_property_key("name").unwrap())
                    .unwrap(),
                Value::String(crate::engine::value::JsString::from_static(expected))
            );
            assert_eq!(costs.legacy_dispatches, 0, "{source}: {costs:?}");
            assert_eq!(costs.owned_bridge_exits, 0, "{source}: {costs:?}");
            assert!(runtime.0.state.borrow().active_frames.is_empty());
        }
    }

    #[test]
    fn with_entry_boxes_primitives_and_unwinds_nullish_errors() {
        for value in [
            Value::Int(1),
            Value::Bool(true),
            Value::String(crate::engine::value::JsString::from_static("x")),
            Value::Null,
            Value::Undefined,
        ] {
            let runtime = Runtime::new();
            let mut context = runtime.new_context();
            let nullish = matches!(value, Value::Null | Value::Undefined);
            let entry = entry(
                &runtime,
                &mut context,
                "(function(o){try{with(o){return 42}}catch(e){return e}})",
                vec![value],
            );
            let profile = CostProfile::start();
            let Completion::Return(value) =
                execute(runtime.clone(), entry, ExecutionLimits::default()).unwrap()
            else {
                panic!("expected with completion")
            };
            let costs = profile.snapshot();
            if nullish {
                let Value::Object(error) = value else {
                    panic!("expected nullish error")
                };
                assert_eq!(
                    context
                        .get_property(&error, &runtime.intern_property_key("message").unwrap())
                        .unwrap(),
                    Value::String(crate::engine::value::JsString::from_static(
                        "cannot convert to object"
                    ))
                );
            } else {
                assert_eq!(value, Value::Int(42))
            }
            assert_eq!(costs.legacy_dispatches, 0, "{costs:?}");
            assert_eq!(costs.owned_bridge_exits, 0, "{costs:?}");
            assert!(runtime.0.state.borrow().active_frames.is_empty());
        }
    }

    #[test]
    fn variable_environment_creation_uses_authenticated_null_prototype() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let entry = entry(
            &runtime,
            &mut context,
            "(function(){eval('');return 42})",
            vec![],
        );
        let pc = entry
            .executable
            .code
            .iter()
            .position(|op| {
                matches!(
                    op,
                    crate::engine::code::bytecode::Instruction::VariableEnvironment
                )
            })
            .unwrap();
        let mut execution = RunningExecution::new(&runtime, ExecutionLimits::default()).unwrap();
        let id = push_frame(&mut execution, entry).unwrap();
        execution.frames.current_mut(id).unwrap().resume_pc = pc;
        let op = super::super::environment_driver::Operation::CreateVariable;
        assert_eq!(run(&mut execution, id).unwrap(), RunExit::Environment(op));
        let before = execution.frames.current_mut(id).unwrap().fault_pc;
        assert!(matches!(
            super::super::environment_driver::step(&runtime, &mut execution, id, op).unwrap(),
            CallStep::Entered
        ));
        let frame = execution.frames.current_mut(id).unwrap();
        assert_eq!(frame.resume_pc, before + 1);
        let Value::Object(environment) = execution.slots.peek(&frame.window, 0).unwrap() else {
            panic!("expected variable environment")
        };
        assert!(runtime.get_prototype_of(environment).unwrap().is_none());
        drop(execution);
        assert!(runtime.0.state.borrow().active_frames.is_empty());
    }

    #[test]
    fn dynamic_data_reads_handle_unscopables_and_captured_with_receivers() {
        for (setup, source) in [
            ("({x:42})", "(function(o){with(o){return x}})"),
            (
                "({x:99,[Symbol.unscopables]:{x:true}})",
                "(function(o){let x=42;with(o){return x}})",
            ),
            (
                "({x:42,[Symbol.unscopables]:{x:false}})",
                "(function(o){with(o){return (function(){return x})()}})",
            ),
            (
                "({x:42,m:function(){return this.x}})",
                "(function(o){with(o){return m()}})",
            ),
            ("({})", "(function(o){let x=42;with(o){return x}})"),
        ] {
            let runtime = Runtime::new();
            let mut context = runtime.new_context();
            let object = context.eval(setup).unwrap();
            let entry = entry(&runtime, &mut context, source, vec![object]);
            let profile = CostProfile::start();
            let Completion::Return(value) =
                execute(runtime.clone(), entry, ExecutionLimits::default()).unwrap()
            else {
                panic!("expected dynamic read")
            };
            assert_eq!(value, Value::Int(42), "{source}");
            let costs = profile.snapshot();
            assert_eq!(costs.legacy_dispatches, 0, "{source}: {costs:?}");
            assert_eq!(costs.owned_bridge_exits, 0, "{source}: {costs:?}");
            assert!(runtime.0.state.borrow().active_frames.is_empty());
        }
    }

    #[test]
    fn dynamic_getters_run_once_and_keep_the_with_receiver() {
        for (setup, source) in [
            (
                "(function(){var n=0;return {get x(){n=n+1;return 41},count:function(){return n}}})()",
                "(function(o){with(o){return x+count()}})",
            ),
            (
                "({get x(){throw 42}})",
                "(function(o){try{with(o){return x}}catch(e){return e}})",
            ),
            (
                "(function(){var n=0;return {x:41,get m(){n=n+1;return function(){return this.x+n}}}})()",
                "(function(o){with(o){return m()}})",
            ),
            (
                "({get x(){return 42}})",
                "(function(o){with(o){return (function(){return x})()}})",
            ),
        ] {
            let runtime = Runtime::new();
            let mut context = runtime.new_context();
            let object = context.eval(setup).unwrap();
            let entry = entry(&runtime, &mut context, source, vec![object]);
            let profile = CostProfile::start();
            let Completion::Return(value) =
                execute(runtime.clone(), entry, ExecutionLimits::default()).unwrap()
            else {
                panic!("expected getter result")
            };
            assert_eq!(value, Value::Int(42));
            let costs = profile.snapshot();
            assert_eq!(costs.legacy_dispatches, 0, "{source}: {costs:?}");
            assert_eq!(costs.owned_bridge_exits, 0, "{source}: {costs:?}");
            assert!(costs.owned_storage.maximum_frame_depth >= 2);
            assert!(runtime.0.state.borrow().active_frames.is_empty());
        }
    }

    #[test]
    fn unscopables_getters_resume_in_order_and_propagate_abrupt_completion() {
        for (setup, source, expected) in [
            (
                "(function(){var n=0;var excluded={get x(){n=n*10+2;return false}};return [{get x(){n=n*10+3;return 41},get [Symbol.unscopables](){n=n*10+1;return excluded}},function(){return n}]})()",
                "(function(o,count){var v=(function(){with(o){return x}})();return v+count()})",
                164,
            ),
            (
                "(function(){var n=0;var excluded={get x(){n=n*10+2;return true}};return [{get x(){throw 99},get [Symbol.unscopables](){n=n*10+1;return excluded}},function(){return n}]})()",
                "(function(o,count){let x=41;var v=(function(){with(o){return x}})();return v+count()})",
                53,
            ),
            (
                "[{x:99,get [Symbol.unscopables](){throw 42}},function(){return 0}]",
                "(function(o){try{with(o){return x}}catch(e){return e}})",
                42,
            ),
            (
                "[{x:99,get [Symbol.unscopables](){return {get x(){throw 42}}}},function(){return 0}]",
                "(function(o){try{with(o){return x}}catch(e){return e}})",
                42,
            ),
            (
                "[{get [Symbol.unscopables](){throw 99}},function(){return 0}]",
                "(function(o){let x=42;with(o){return x}})",
                42,
            ),
        ] {
            let runtime = Runtime::new();
            let mut context = runtime.new_context();
            let Value::Object(pair) = context.eval(setup).unwrap() else {
                panic!("expected setup pair")
            };
            let object = context
                .get_property(&pair, &runtime.intern_property_key("0").unwrap())
                .unwrap();
            let count = context
                .get_property(&pair, &runtime.intern_property_key("1").unwrap())
                .unwrap();
            let entry = entry(&runtime, &mut context, source, vec![object, count]);
            let profile = CostProfile::start();
            let Completion::Return(value) =
                execute(runtime.clone(), entry, ExecutionLimits::default()).unwrap()
            else {
                panic!("expected HasBinding result")
            };
            assert_eq!(value, Value::Int(expected), "{setup}");
            let costs = profile.snapshot();
            assert_eq!(costs.legacy_dispatches, 0, "{setup}: {costs:?}");
            assert_eq!(costs.owned_bridge_exits, 0, "{setup}: {costs:?}");
            assert!(runtime.0.state.borrow().active_frames.is_empty());
        }
    }

    #[test]
    fn selected_native_calls_return_or_throw_into_the_owned_caller() {
        for (callee, source) in [
            ("parseInt", "(function(f){let x=2;return f('40',10)+x})"),
            (
                "Reflect.get",
                "(function(f){let x=40;try{f(0,'x')}catch(e){return x+2}})",
            ),
        ] {
            let runtime = Runtime::new();
            let mut context = runtime.new_context();
            let callee = context.eval(callee).unwrap();
            let entry = entry(&runtime, &mut context, source, vec![callee]);
            let profile = CostProfile::start();
            assert!(matches!(
                execute(runtime.clone(), entry, ExecutionLimits::default()).unwrap(),
                Completion::Return(Value::Int(42))
            ));
            let costs = profile.snapshot();
            assert_eq!(costs.legacy_dispatches, 0, "{source}: {costs:?}");
            assert_eq!(costs.owned_bridge_exits, 0, "{source}: {costs:?}");
            assert!(runtime.0.state.borrow().active_frames.is_empty());
        }
    }

    #[test]
    fn selected_native_callback_reentry_preserves_captured_bindings() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let array = context
            .eval("(function(){let xs=[10,20,12];xs.f=Array.prototype.map;return xs})()")
            .unwrap();
        let entry = entry(
            &runtime,
            &mut context,
            "(function(xs){let sum=0;let mapped=xs.f(function(value){sum=sum+value;return value});return sum})",
            vec![array],
        );
        let profile = CostProfile::start();
        assert!(matches!(
            execute(runtime.clone(), entry, ExecutionLimits::default()).unwrap(),
            Completion::Return(Value::Int(42))
        ));
        let costs = profile.snapshot();
        assert_eq!(costs.legacy_dispatches, 0, "{costs:?}");
        assert_eq!(costs.owned_bridge_exits, 0, "{costs:?}");
        assert!(runtime.0.state.borrow().active_frames.is_empty());
        // The map callback still uses the selected synchronous domain boundary;
        // zero frame handoffs do not claim that its S05 continuation has migrated.
        assert_eq!(costs.owned_sync_call_bridges, 1);
    }

    #[test]
    fn named_array_prototype_reads_reach_owned_methods() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        context.eval("Object.defineProperty(Array.prototype,'owned',{value:function(){return 42},configurable:true})").unwrap();
        let array = context.eval("[]").unwrap();
        let entry = entry(
            &runtime,
            &mut context,
            "(function(xs){return xs.owned()})",
            vec![array],
        );
        let profile = CostProfile::start();
        assert!(matches!(
            execute(runtime.clone(), entry, ExecutionLimits::default()).unwrap(),
            Completion::Return(Value::Int(42))
        ));
        let costs = profile.snapshot();
        assert_eq!(costs.legacy_dispatches, 0, "{costs:?}");
        assert_eq!(costs.owned_bridge_exits, 0, "{costs:?}");
        assert!(runtime.0.state.borrow().active_frames.is_empty());
    }

    #[test]
    fn program_global_initialization_and_assignments_use_owned_cells() {
        for source in [
            "let x=40;x=x+2;x",
            "const x=42;x",
            "var x=40;x=x+2;x",
            "function f(){return 42}f()",
            "x=40;x=x+2;x",
        ] {
            let runtime = Runtime::new();
            let mut context = runtime.new_context();
            let profile = CostProfile::start();
            assert_eq!(context.eval(source).unwrap(), Value::Int(42), "{source}");
            let costs = profile.snapshot();
            assert_eq!(costs.legacy_dispatches, 0, "{source}: {costs:?}");
            assert_eq!(costs.owned_bridge_exits, 0, "{source}: {costs:?}");
            assert!(runtime.0.state.borrow().active_frames.is_empty());
        }
    }

    #[test]
    fn global_writes_preserve_tdz_const_and_rhs_order() {
        for (setup, body, message) in [
            ("const x=1", "x=42;return x", Some("'x' is read-only")),
            (
                "throw 0;let x",
                "x=42;return x",
                Some("x is not initialized"),
            ),
            ("throw 0;let x", "x=(function(){throw 42})()", None),
            ("const x=1", "x=(function(){throw 42})()", None),
            (
                "delete globalThis.x",
                "'use strict';x=42",
                Some("'x' is not defined"),
            ),
            (
                "Object.defineProperty(globalThis,'x',{value:1,writable:false,configurable:true})",
                "'use strict';x=42",
                Some("'x' is read-only"),
            ),
            (
                "Object.defineProperty(globalThis,'x',{value:42,writable:false,configurable:true})",
                "x=99;return x",
                None,
            ),
        ] {
            let runtime = Runtime::new();
            let mut context = runtime.new_context();
            if context.eval(setup).is_err() {
                context.take_exception().unwrap();
            }
            let source = format!("(function(){{{body}}})");
            let entry = entry(&runtime, &mut context, &source, vec![]);
            let profile = CostProfile::start();
            let completion = execute(runtime.clone(), entry, ExecutionLimits::default()).unwrap();
            let costs = profile.snapshot();
            if let Some(message) = message {
                let Completion::Throw(Value::Object(error)) = completion else {
                    panic!("expected global binding error: {source}: {completion:?}")
                };
                assert_eq!(
                    context
                        .get_property(&error, &runtime.intern_property_key("message").unwrap())
                        .unwrap(),
                    Value::String(crate::engine::value::JsString::try_from_utf8(message).unwrap())
                );
            } else if body.contains("throw 42") {
                assert!(
                    matches!(completion, Completion::Throw(Value::Int(42))),
                    "{source}: {completion:?}"
                );
            } else {
                assert!(
                    matches!(completion, Completion::Return(Value::Int(42))),
                    "{source}: {completion:?}"
                );
            }
            assert_eq!(costs.legacy_dispatches, 0, "{source}: {costs:?}");
            assert_eq!(costs.owned_bridge_exits, 0, "{source}: {costs:?}");
            assert!(runtime.0.state.borrow().active_frames.is_empty());
        }
    }

    #[test]
    fn global_property_setters_run_once_without_getter_reads() {
        for inherited in [false, true] {
            let runtime = Runtime::new();
            let mut context = runtime.new_context();
            let target = if inherited {
                "Object.getPrototypeOf(globalThis)"
            } else {
                "globalThis"
            };
            context.eval(&format!("(function(){{let stored=0;globalThis.readStored=function(){{return stored}};Object.defineProperty({target},'x',{{get(){{throw 99}},set(value){{if(this!==globalThis)throw 99;stored=stored+value}},configurable:true}})}})()" )).unwrap();
            let entry = entry(
                &runtime,
                &mut context,
                "(function(){'use strict';x=40;x=2;return 42})",
                vec![],
            );
            let profile = CostProfile::start();
            assert!(matches!(
                execute(runtime.clone(), entry, ExecutionLimits::default()).unwrap(),
                Completion::Return(Value::Int(42))
            ));
            let costs = profile.snapshot();
            drop(profile);
            assert_eq!(context.eval("readStored()").unwrap(), Value::Int(42));
            assert_eq!(
                costs.legacy_dispatches, 0,
                "inherited={inherited}: {costs:?}"
            );
            assert_eq!(
                costs.owned_bridge_exits, 0,
                "inherited={inherited}: {costs:?}"
            );
            assert!(costs.owned_storage.maximum_frame_depth >= 2);
            assert!(runtime.0.state.borrow().active_frames.is_empty());
        }
    }

    #[test]
    fn delete_global_binding_probes_presence_without_reading_accessors() {
        for (setup, expected) in [
            ("let x=1", false),
            ("throw 0;let x", false),
            ("var x=1", false),
            ("globalThis.x=1", true),
            ("delete globalThis.x", true),
            (
                "Object.defineProperty(globalThis,'x',{get(){throw 99},configurable:true})",
                true,
            ),
            (
                "Object.setPrototypeOf(globalThis,{get x(){throw 99}})",
                true,
            ),
        ] {
            let runtime = Runtime::new();
            let mut context = runtime.new_context();
            if context.eval(setup).is_err() {
                context.take_exception().unwrap();
            }
            let entry = entry(
                &runtime,
                &mut context,
                "(function(){return delete x})",
                vec![],
            );
            let profile = CostProfile::start();
            assert!(
                matches!(execute(runtime.clone(),entry,ExecutionLimits::default()).unwrap(),Completion::Return(Value::Bool(value)) if value==expected),
                "{setup}"
            );
            let costs = profile.snapshot();
            assert_eq!(costs.legacy_dispatches, 0, "{setup}: {costs:?}");
            assert_eq!(costs.owned_bridge_exits, 0, "{setup}: {costs:?}");
            assert!(runtime.0.state.borrow().active_frames.is_empty());
        }
    }

    #[test]
    fn spread_calls_preserve_receiver_bound_arguments_and_explicit_frames() {
        for (callee, source) in [
            (
                "(function(a,b){return a+b})",
                "(function(f){return f(...[40,2])})",
            ),
            (
                "({base:40,f:function(x){return this.base+x}})",
                "(function(o){return o.f(...[2])})",
            ),
            (
                "(function(a,b){return this.base+a+b}).bind({base:1},40)",
                "(function(f){return f(...[1])})",
            ),
            ("(function(){return 42})", "(function(f){return f(...[])})"),
            (
                "(function(){throw 42})",
                "(function(f){try{return f(...[1])}catch(e){return e}})",
            ),
            (
                "(function loop(n){if(n===0)return 42;return loop(...[n-1])})",
                "(function(f){return f(...[256])})",
            ),
        ] {
            let runtime = Runtime::new();
            let mut context = runtime.new_context();
            let callee = context.eval(callee).unwrap();
            let entry = entry(&runtime, &mut context, source, vec![callee]);
            let profile = CostProfile::start();
            let completion = execute(runtime.clone(), entry, ExecutionLimits::default()).unwrap();
            assert!(
                matches!(completion, Completion::Return(Value::Int(42))),
                "{source}: {completion:?}"
            );
            let costs = profile.snapshot();
            assert_eq!(costs.legacy_dispatches, 0, "{source}: {costs:?}");
            assert_eq!(costs.owned_bridge_exits, 0, "{source}: {costs:?}");
            assert!(runtime.0.state.borrow().active_frames.is_empty());
        }
    }

    #[test]
    fn spread_constructors_and_super_use_existing_constructor_continuations() {
        for (callee, arguments) in [
            (
                "(function Base(a,b){if(new.target===Base)return {value:a+b};throw 99})",
                "[40,2]",
            ),
            ("(function(a,b){return {value:a+b}}).bind(null,40)", "[2]"),
            (
                "(class Derived extends (class Base{constructor(a,b){return {value:a+b}}}) {constructor(...args){super(...args)}})",
                "[40,2]",
            ),
            (
                "(class Derived extends (class Base{constructor(a,b){return {value:a+b}}}) {})",
                "[40,2]",
            ),
        ] {
            let runtime = Runtime::new();
            let mut context = runtime.new_context();
            let callee = context.eval(callee).unwrap();
            let args = context.eval(arguments).unwrap();
            let entry = entry(
                &runtime,
                &mut context,
                "(function(C,args){return (new C(...args)).value})",
                vec![callee, args],
            );
            let profile = CostProfile::start();
            let completion = execute(runtime.clone(), entry, ExecutionLimits::default()).unwrap();
            assert!(
                matches!(completion, Completion::Return(Value::Int(42))),
                "{arguments}: {completion:?}"
            );
            let costs = profile.snapshot();
            assert_eq!(costs.legacy_dispatches, 0, "{costs:?}");
            assert_eq!(costs.owned_bridge_exits, 0, "{costs:?}");
            assert!(runtime.0.state.borrow().active_frames.is_empty());
        }
    }

    #[test]
    fn apply_construct_snapshot_survives_prototype_getter_and_carrier_mutation() {
        use crate::engine::code::bytecode::{ApplyKind, Instruction};
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let constructor = context.eval("(function(value){return value})").unwrap();
        let new_target = context.eval("({get prototype(){return {}}})").unwrap();
        let Value::Object(carrier) = context.eval("[{}]").unwrap() else {
            panic!("expected argument array")
        };
        let argument_id = {
            let values = runtime
                .fast_array_like_values(&carrier, 1)
                .unwrap()
                .unwrap();
            let Value::Object(object) = &values[0] else {
                panic!("expected object argument")
            };
            object.object_id()
        };
        let entry = entry(
            &runtime,
            &mut context,
            "(function(C){return new C(...[])})",
            vec![],
        );
        let pc = entry
            .executable
            .code
            .iter()
            .position(|op| matches!(op, Instruction::Apply(ApplyKind::Construct)))
            .unwrap();
        let mut execution = RunningExecution::new(&runtime, ExecutionLimits::default()).unwrap();
        let id = push_frame(&mut execution, entry).unwrap();
        let frame = execution.frames.current_mut(id).unwrap();
        frame.resume_pc = pc;
        for value in [constructor, new_target, Value::Object(carrier.clone())] {
            execution.slots.push(&mut frame.window, value).unwrap();
        }
        let profile = CostProfile::start();
        assert!(matches!(
            run(&mut execution, id).unwrap(),
            RunExit::Apply(ApplyKind::Construct)
        ));
        assert!(matches!(
            super::super::apply_driver::step(&runtime, &mut execution, id, ApplyKind::Construct, 1)
                .unwrap(),
            CallStep::Entered
        ));
        assert_ne!(execution.frames.current_id(), Some(id));
        assert!(
            runtime
                .delete_property(&carrier, &runtime.intern_property_key("0").unwrap())
                .unwrap()
        );
        assert!(
            runtime
                .0
                .state
                .borrow()
                .heap
                .object_strong_count(argument_id)
                .unwrap()
                > 0
        );
        let Completion::Return(Value::Object(result)) =
            execute_running(runtime.clone(), execution).unwrap()
        else {
            panic!("expected original argument")
        };
        let costs = profile.snapshot();
        assert_eq!(result.object_id(), argument_id);
        drop(result);
        assert_eq!(
            runtime
                .0
                .state
                .borrow()
                .heap
                .object_strong_count(argument_id)
                .unwrap_or(0),
            0
        );
        assert_eq!(costs.legacy_dispatches, 0, "{costs:?}");
        assert_eq!(costs.owned_bridge_exits, 0, "{costs:?}");
        assert!(runtime.0.state.borrow().active_frames.is_empty());
    }

    #[test]
    fn apply_checkpoint_preserves_callability_list_and_constructor_error_order() {
        use crate::engine::code::bytecode::{ApplyKind, Instruction};
        for (function, array, construct, error_name, message) in [
            (
                "0",
                "({get length(){throw 99}})",
                false,
                Some("TypeError"),
                Some("not a function"),
            ),
            (
                "0",
                "new Array(65535)",
                true,
                Some("TypeError"),
                Some("not a function"),
            ),
            (
                "(()=>42)",
                "new Array(65535)",
                true,
                Some("RangeError"),
                None,
            ),
            ("(()=>42)", "[]", true, Some("TypeError"), None),
            (
                "(()=>42)",
                "1",
                false,
                Some("TypeError"),
                Some("not a object"),
            ),
            ("(()=>42)", "null", true, None, None),
            ("(()=>42)", "undefined", false, None, None),
        ] {
            let runtime = Runtime::new();
            let mut context = runtime.new_context();
            let function = context.eval(function).unwrap();
            let array = context.eval(array).unwrap();
            let source = if construct {
                "(function(C){return new C(...[])})"
            } else {
                "(function(f){return f(...[])})"
            };
            let entry = entry(&runtime, &mut context, source, vec![]);
            let kind = if construct {
                ApplyKind::Construct
            } else {
                ApplyKind::Call
            };
            let pc = entry
                .executable
                .code
                .iter()
                .position(|op| matches!(op, Instruction::Apply(found) if *found==kind))
                .unwrap();
            let mut execution =
                RunningExecution::new(&runtime, ExecutionLimits::default()).unwrap();
            let id = push_frame(&mut execution, entry).unwrap();
            let frame = execution.frames.current_mut(id).unwrap();
            frame.resume_pc = pc;
            for value in [function, Value::Undefined, array] {
                execution.slots.push(&mut frame.window, value).unwrap();
            }
            let profile = CostProfile::start();
            let completion = execute_running(runtime.clone(), execution).unwrap();
            let costs = profile.snapshot();
            if let Some(error_name) = error_name {
                let Completion::Throw(Value::Object(error)) = completion else {
                    panic!("expected Apply error: {completion:?}")
                };
                assert_eq!(
                    context
                        .get_property(&error, &runtime.intern_property_key("name").unwrap())
                        .unwrap(),
                    Value::String(
                        crate::engine::value::JsString::try_from_utf8(error_name).unwrap()
                    )
                );
                if let Some(message) = message {
                    assert_eq!(
                        context
                            .get_property(&error, &runtime.intern_property_key("message").unwrap())
                            .unwrap(),
                        Value::String(
                            crate::engine::value::JsString::try_from_utf8(message).unwrap()
                        )
                    );
                }
            } else {
                assert!(
                    matches!(completion, Completion::Return(Value::Int(42))),
                    "{completion:?}"
                );
            }
            assert_eq!(costs.legacy_dispatches, 0, "{costs:?}");
            assert_eq!(costs.owned_bridge_exits, 0, "{costs:?}");
            assert!(runtime.0.state.borrow().active_frames.is_empty());
        }
    }

    #[test]
    fn for_of_owns_records_and_per_iteration_bindings() {
        for source in [
            "(function(xs){let sum=0;for(let x of xs){sum=sum+x}return sum})",
            "(function(xs){let sum=0;for(const x of xs){if(x===40){sum=x;continue}sum=sum+x}return sum})",
            "(function(xs){let f;for(let x of xs){if(x===40)f=function(){return x};else return f()+x}})",
            "(function(xs){let [a,b]=xs;return a+b})",
        ] {
            let runtime = Runtime::new();
            let mut context = runtime.new_context();
            let xs = context.eval("[40,2]").unwrap();
            let entry = entry(&runtime, &mut context, source, vec![xs]);
            let profile = CostProfile::start();
            let result = execute(runtime.clone(), entry, ExecutionLimits::default()).unwrap();
            assert!(
                matches!(result, Completion::Return(Value::Int(42))),
                "{source}: {result:?}"
            );
            let costs = profile.snapshot();
            assert_eq!(costs.legacy_dispatches, 0, "{source}: {costs:?}");
            assert_eq!(costs.owned_bridge_exits, 0, "{source}: {costs:?}");
            assert!(runtime.0.state.borrow().active_frames.is_empty());
        }
    }

    #[test]
    fn for_of_callback_order_uses_one_iterator_get_and_cached_next() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let xs = context.eval(r#"(function(){let trace='',n=0;globalThis.readTrace=function(){return trace};return {
            get [Symbol.iterator](){trace=trace+'i';return function(){trace=trace+'c';return {
                get next(){trace=trace+'n';return function(){trace=trace+'s';n=n+1;return {
                    get done(){trace=trace+'d';return n>2},
                    get value(){trace=trace+'v';if(n>2)throw 99;return n===1?40:2}
                }}}, get return(){throw 99}
            }}}
        }})()"#).unwrap();
        let entry = entry(
            &runtime,
            &mut context,
            "(function(xs){let sum=0;for(let x of xs){sum=sum+x}return sum})",
            vec![xs],
        );
        let profile = CostProfile::start();
        assert!(matches!(
            execute(runtime.clone(), entry, ExecutionLimits::default()).unwrap(),
            Completion::Return(Value::Int(42))
        ));
        let costs = profile.snapshot();
        drop(profile);
        assert_eq!(
            context.eval("readTrace()").unwrap(),
            Value::String(crate::engine::value::JsString::from_static("icnsdvsdvsd"))
        );
        assert_eq!(costs.legacy_dispatches, 0, "{costs:?}");
        assert_eq!(costs.owned_bridge_exits, 0, "{costs:?}");
        assert!(runtime.0.state.borrow().active_frames.is_empty());
    }

    #[test]
    fn for_of_closes_body_abrupt_completions_but_not_next_failures() {
        for (next, close, body, expected, trace) in [
            (
                "next(){trace=trace+'n';return {done:false,value:40}}",
                "return {}",
                "return x+2",
                42,
                "irc",
            ),
            (
                "next(){trace=trace+'n';return {done:false,value:40}}",
                "return {}",
                "break",
                42,
                "irc",
            ),
            (
                "next(){trace=trace+'n';return {done:false,value:40}}",
                "throw 99",
                "throw 42",
                42,
                "irc",
            ),
            (
                "next(){trace=trace+'n';return {done:false,value:40}}",
                "throw 99",
                "return 42",
                99,
                "irc",
            ),
            (
                "next(){trace=trace+'n';return {done:false,value:40}}",
                "return 0",
                "throw 42",
                42,
                "irc",
            ),
            (
                "next(){trace=trace+'n';throw 42}",
                "throw 99",
                "return 0",
                42,
                "i",
            ),
            (
                "next(){trace=trace+'n';return {get done(){throw 42}}}",
                "throw 99",
                "return 0",
                42,
                "i",
            ),
            (
                "next(){trace=trace+'n';return {done:false,get value(){throw 42}}}",
                "throw 99",
                "return 0",
                42,
                "i",
            ),
        ] {
            let runtime = Runtime::new();
            let mut context = runtime.new_context();
            let setup = r#"(function(){let trace='';globalThis.readTrace=function(){return trace};
                let iter={NEXT,get return(){trace=trace+'r';return function(){trace=trace+'c';CLOSE}}};
                return {[Symbol.iterator](){trace=trace+'i';return iter}};
            })()"#.replace("NEXT", next).replace("CLOSE", close);
            let xs = context.eval(&setup).unwrap();
            let source = format!(
                "(function(xs){{try{{for(let x of xs){{{body}}}return 42}}catch(e){{return e}}}})"
            );
            let entry = entry(&runtime, &mut context, &source, vec![xs]);
            let profile = CostProfile::start();
            let result = execute(runtime.clone(), entry, ExecutionLimits::default()).unwrap();
            assert!(
                matches!(result, Completion::Return(Value::Int(value)) if value == expected),
                "{source}: {result:?}"
            );
            let costs = profile.snapshot();
            drop(profile);
            let expected_trace = trace.replacen('i', "in", 1);
            assert_eq!(
                context.eval("readTrace()").unwrap(),
                Value::String(
                    crate::engine::value::JsString::try_from_utf8(&expected_trace).unwrap()
                ),
                "{source}"
            );
            assert_eq!(costs.legacy_dispatches, 0, "{source}: {costs:?}");
            assert_eq!(costs.owned_bridge_exits, 0, "{source}: {costs:?}");
            assert!(runtime.0.state.borrow().active_frames.is_empty());
        }
    }

    #[test]
    fn for_of_normal_close_rejects_primitive_results_and_unwinds_nested_regions() {
        for (source, expected, expected_trace, message) in [
            (
                "(function(xs,ys){try{for(let x of xs){break}}catch(e){return e}})",
                0,
                "xo",
                Some("not an object"),
            ),
            (
                "(function(xs,ys){try{for(let x of xs){for(let y of ys){throw 42}}}catch(e){return e}})",
                42,
                "xyio",
                None,
            ),
            (
                "(function(xs,ys){try{outer:for(let x of xs){for(let y of ys){break outer}}}catch(e){return 42}})",
                42,
                "xyio",
                None,
            ),
            (
                "(function(xs,ys){try{for(let x of xs){try{throw 42}finally{throw 43}}}catch(e){return e}})",
                43,
                "xo",
                None,
            ),
        ] {
            let runtime = Runtime::new();
            let mut context = runtime.new_context();
            let Value::Object(pair) = context.eval(r#"(function(){let trace='';globalThis.readTrace=function(){return trace};return [
                {[Symbol.iterator](){trace=trace+'x';return {next(){return {done:false,value:40}},return(){trace=trace+'o';return 0}}}},
                {[Symbol.iterator](){trace=trace+'y';return {next(){return {done:false,value:2}},return(){trace=trace+'i';return {}}}}}
            ]})()"#).unwrap() else { panic!("expected pair") };
            let arguments = runtime.fast_array_like_values(&pair, 2).unwrap().unwrap();
            let entry = entry(&runtime, &mut context, source, arguments);
            let profile = CostProfile::start();
            let Completion::Return(value) =
                execute(runtime.clone(), entry, ExecutionLimits::default()).unwrap()
            else {
                panic!("expected caught completion")
            };
            let costs = profile.snapshot();
            drop(profile);
            if let Some(message) = message {
                let Value::Object(error) = value else {
                    panic!("expected close error")
                };
                assert_eq!(
                    context
                        .get_property(&error, &runtime.intern_property_key("message").unwrap())
                        .unwrap(),
                    Value::String(crate::engine::value::JsString::try_from_utf8(message).unwrap())
                );
            } else {
                assert_eq!(value, Value::Int(expected), "{source}");
            }
            assert_eq!(
                context.eval("readTrace()").unwrap(),
                Value::String(
                    crate::engine::value::JsString::try_from_utf8(expected_trace).unwrap()
                ),
                "{source}"
            );
            assert_eq!(costs.legacy_dispatches, 0, "{source}: {costs:?}");
            assert_eq!(costs.owned_bridge_exits, 0, "{source}: {costs:?}");
            assert!(runtime.0.state.borrow().active_frames.is_empty());
        }
    }

    #[test]
    fn append_spreads_dense_arrays_strings_and_eval_arguments_in_owned_frames() {
        for (source, argument, expected) in [
            ("(function(xs){return [...xs,2]})", "[40,41]", "[40,41,2]"),
            ("(function(xs){return [...xs]})", "'ab'", "['a','b']"),
            ("(function(xs){return [...xs]})", "[]", "[]"),
        ] {
            let runtime = Runtime::new();
            let mut context = runtime.new_context();
            let argument = context.eval(argument).unwrap();
            let Value::Object(expected) = context.eval(expected).unwrap() else {
                panic!("expected array")
            };
            let len = runtime.array_length_state(&expected).unwrap().0;
            let expected = runtime
                .fast_array_like_values(&expected, len)
                .unwrap()
                .unwrap();
            let entry = entry(&runtime, &mut context, source, vec![argument]);
            let profile = CostProfile::start();
            let Completion::Return(Value::Object(array)) =
                execute(runtime.clone(), entry, ExecutionLimits::default()).unwrap()
            else {
                panic!("expected spread array")
            };
            let costs = profile.snapshot();
            assert_eq!(runtime.array_length_state(&array).unwrap().0, len);
            assert_eq!(
                runtime
                    .fast_array_like_values(&array, len)
                    .unwrap()
                    .unwrap(),
                expected
            );
            assert_eq!(costs.legacy_dispatches, 0, "{source}: {costs:?}");
            assert_eq!(costs.owned_bridge_exits, 0, "{source}: {costs:?}");
            assert!(runtime.0.state.borrow().active_frames.is_empty());
        }
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let entry = entry(
            &runtime,
            &mut context,
            "(function(){return eval(...['40+2'])})",
            vec![],
        );
        let profile = CostProfile::start();
        assert!(matches!(
            execute(runtime.clone(), entry, ExecutionLimits::default()).unwrap(),
            Completion::Return(Value::Int(42))
        ));
        assert_eq!(profile.snapshot().legacy_dispatches, 0);
        assert_eq!(profile.snapshot().owned_bridge_exits, 0);
        assert!(runtime.0.state.borrow().active_frames.is_empty());
    }

    #[test]
    fn append_callbacks_preserve_lookup_order_cache_next_and_skip_done_value() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let iterable = context.eval(r#"(function(){let trace='',n=0;globalThis.readTrace=function(){return trace};return {
                get [Symbol.iterator](){trace=trace+'i';return function(){trace=trace+'c';return {
                    get next(){trace=trace+'n';return function(){trace=trace+'s';n=n+1;
                        if(n===1)return {get done(){trace=trace+'d';return false},get value(){trace=trace+'v';return 40}};
                        return {get done(){trace=trace+'d';return true},get value(){throw 99}};
                    }},
                    get return(){throw 99}
                }}}
            }})()"#).unwrap();
        let entry = entry(
            &runtime,
            &mut context,
            "(function(xs){return [...xs,2]})",
            vec![iterable],
        );
        let profile = CostProfile::start();
        let Completion::Return(Value::Object(array)) =
            execute(runtime.clone(), entry, ExecutionLimits::default()).unwrap()
        else {
            panic!("expected spread array")
        };
        let costs = profile.snapshot();
        drop(profile);
        assert_eq!(
            runtime.fast_array_like_values(&array, 2).unwrap().unwrap(),
            vec![Value::Int(40), Value::Int(2)]
        );
        assert_eq!(
            context.eval("readTrace()").unwrap(),
            Value::String(crate::engine::value::JsString::from_static("iicnsdvsd"))
        );
        assert_eq!(costs.legacy_dispatches, 0, "{costs:?}");
        assert_eq!(costs.owned_bridge_exits, 0, "{costs:?}");
        assert!(costs.owned_storage.maximum_frame_depth >= 2);
        assert!(runtime.0.state.borrow().active_frames.is_empty());
    }

    #[test]
    fn append_closes_iteration_errors_and_preserves_original_throw() {
        for (next, close, expected) in [
            (
                "get next(){trace=trace+'n';return function(){trace=trace+'s';throw 42}}",
                "get return(){trace=trace+'r';throw 99}",
                "iicnsr",
            ),
            (
                "get next(){trace=trace+'n';return function(){trace=trace+'s';throw 42}}",
                "get return(){trace=trace+'r';return function(){trace=trace+'c';throw 99}}",
                "iicnsrc",
            ),
            (
                "get next(){trace=trace+'n';return function(){trace=trace+'s';return {get done(){trace=trace+'d';throw 42}}}}",
                "get return(){trace=trace+'r';return function(){trace=trace+'c';return 0}}",
                "iicnsdrc",
            ),
            (
                "get next(){trace=trace+'n';return function(){trace=trace+'s';return {done:false,get value(){trace=trace+'v';throw 42}}}}",
                "get return(){trace=trace+'r';return 0}",
                "iicnsvr",
            ),
            (
                "get next(){trace=trace+'n';throw 42}",
                "get return(){trace=trace+'r';throw 99}",
                "iicn",
            ),
        ] {
            let runtime = Runtime::new();
            let mut context = runtime.new_context();
            let setup = format!(
                "(function(){{let trace='';globalThis.readTrace=function(){{return trace}};let iter={{{next},{close}}};return {{get [Symbol.iterator](){{trace=trace+'i';return function(){{trace=trace+'c';return iter}}}}}}}})()"
            );
            let iterable = context.eval(&setup).unwrap();
            let entry = entry(
                &runtime,
                &mut context,
                "(function(xs){try{return [...xs]}catch(e){return e}})",
                vec![iterable],
            );
            let profile = CostProfile::start();
            assert!(
                matches!(
                    execute(runtime.clone(), entry, ExecutionLimits::default()).unwrap(),
                    Completion::Return(Value::Int(42))
                ),
                "{setup}"
            );
            let costs = profile.snapshot();
            drop(profile);
            assert_eq!(
                context.eval("readTrace()").unwrap(),
                Value::String(crate::engine::value::JsString::try_from_utf8(expected).unwrap()),
                "{setup}"
            );
            assert_eq!(costs.legacy_dispatches, 0, "{setup}: {costs:?}");
            assert_eq!(costs.owned_bridge_exits, 0, "{setup}: {costs:?}");
            assert!(runtime.0.state.borrow().active_frames.is_empty());
        }
    }

    #[test]
    fn append_reuses_bounded_frame_storage_across_many_iterator_calls() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let iterable = context
            .eval(
                r#"(function(){let n=0;return {
            [Symbol.iterator](){return {next(){n=n+1;return {done:n>2048,value:n}}}}
        }})()"#,
            )
            .unwrap();
        let entry = entry(
            &runtime,
            &mut context,
            "(function(xs){return [...xs]})",
            vec![iterable],
        );
        let profile = CostProfile::start();
        let Completion::Return(Value::Object(array)) = execute(
            runtime.clone(),
            entry,
            ExecutionLimits {
                frames: 2,
                ..ExecutionLimits::default()
            },
        )
        .unwrap() else {
            panic!("expected finite iterable")
        };
        let costs = profile.snapshot();
        assert_eq!(runtime.array_length_state(&array).unwrap().0, 2048);
        assert_eq!(
            context
                .get_property(&array, &runtime.intern_property_key("2047").unwrap())
                .unwrap(),
            Value::Int(2048)
        );
        assert_eq!(costs.owned_storage.maximum_frame_depth, 2);
        assert!(costs.owned_storage.frames_pushed > 2048);
        assert_eq!(costs.legacy_dispatches, 0, "{costs:?}");
        assert_eq!(costs.owned_bridge_exits, 0, "{costs:?}");
        assert!(runtime.0.state.borrow().active_frames.is_empty());
    }

    #[test]
    fn append_checkpoint_closes_failed_writes_and_wraps_its_u32_index() {
        for fail in [false, true] {
            let runtime = Runtime::new();
            let mut context = runtime.new_context();
            let target = context
                .eval(if fail {
                    "Object.preventExtensions([])"
                } else {
                    "[]"
                })
                .unwrap();
            let iterable = context.eval(if fail {
                r#"(function(){let trace='';globalThis.readTrace=function(){return trace};return {
                    [Symbol.iterator](){return {
                        next(){return {done:false,value:40}},
                        get return(){trace=trace+'r';return function(){trace=trace+'c';throw 99}}
                    }}
                }})()"#
            } else { "[40,2]" }).unwrap();
            let entry = entry(
                &runtime,
                &mut context,
                "(function(xs){return [...xs]})",
                vec![],
            );
            let pc = entry
                .executable
                .code
                .iter()
                .position(|op| matches!(op, crate::engine::code::bytecode::Instruction::Append))
                .unwrap();
            let mut execution =
                RunningExecution::new(&runtime, ExecutionLimits::default()).unwrap();
            let id = push_frame(&mut execution, entry).unwrap();
            let frame = execution.frames.current_mut(id).unwrap();
            frame.resume_pc = pc;
            execution.slots.push(&mut frame.window, target).unwrap();
            execution
                .slots
                .push(&mut frame.window, Value::Int(if fail { 0 } else { -1 }))
                .unwrap();
            execution.slots.push(&mut frame.window, iterable).unwrap();
            let profile = CostProfile::start();
            let completion = execute_running(runtime.clone(), execution).unwrap();
            let costs = profile.snapshot();
            drop(profile);
            if fail {
                let Completion::Throw(Value::Object(error)) = completion else {
                    panic!("expected original write error")
                };
                assert_eq!(
                    context
                        .get_property(&error, &runtime.intern_property_key("message").unwrap())
                        .unwrap(),
                    Value::String(crate::engine::value::JsString::from_static(
                        "property is not configurable"
                    ))
                );
                assert_eq!(
                    context.eval("readTrace()").unwrap(),
                    Value::String(crate::engine::value::JsString::from_static("rc"))
                );
            } else {
                let Completion::Return(Value::Object(array)) = completion else {
                    panic!("expected wrapped-index array")
                };
                for (key, expected) in [("4294967295", 40), ("0", 2), ("length", 1)] {
                    assert_eq!(
                        context
                            .get_property(&array, &runtime.intern_property_key(key).unwrap())
                            .unwrap(),
                        Value::Int(expected)
                    );
                }
            }
            assert_eq!(costs.legacy_dispatches, 0, "fail={fail}: {costs:?}");
            assert_eq!(costs.owned_bridge_exits, 0, "fail={fail}: {costs:?}");
            assert!(runtime.0.state.borrow().active_frames.is_empty());
        }
    }

    #[test]
    fn array_element_checkpoint_defines_own_properties_without_setters() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        context
            .eval("Object.defineProperty(Array.prototype,'1',{set(){throw 99},configurable:true})")
            .unwrap();
        let array = context.eval("[]").unwrap();
        let entry = entry(
            &runtime,
            &mut context,
            "(function(){return [...[],40]})",
            vec![],
        );
        let pc = entry
            .executable
            .code
            .iter()
            .position(|op| {
                matches!(
                    op,
                    crate::engine::code::bytecode::Instruction::DefineArrayEl
                )
            })
            .unwrap();
        let mut execution = RunningExecution::new(&runtime, ExecutionLimits::default()).unwrap();
        let id = push_frame(&mut execution, entry).unwrap();
        let frame = execution.frames.current_mut(id).unwrap();
        // Start at the published element write; Append itself is not exercised here.
        frame.resume_pc = pc;
        execution
            .slots
            .push(&mut frame.window, array.clone())
            .unwrap();
        execution
            .slots
            .push(&mut frame.window, Value::Int(1))
            .unwrap();
        execution
            .slots
            .push(&mut frame.window, Value::Int(40))
            .unwrap();
        let profile = CostProfile::start();
        assert!(matches!(
            run(&mut execution, id).unwrap(),
            RunExit::Environment(super::super::environment_driver::Operation::DefineArrayElement)
        ));
        assert!(matches!(
            super::super::environment_driver::step(
                &runtime,
                &mut execution,
                id,
                super::super::environment_driver::Operation::DefineArrayElement
            )
            .unwrap(),
            CallStep::Entered
        ));
        let frame = execution.frames.current_mut(id).unwrap();
        assert_eq!(frame.resume_pc, pc + 1);
        assert_eq!(execution.slots.depth(&frame.window), 2);
        assert_eq!(
            execution.slots.peek(&frame.window, 0).unwrap(),
            &Value::Int(1)
        );
        assert_eq!(execution.slots.peek(&frame.window, 1).unwrap(), &array);
        let Completion::Return(Value::Object(array)) =
            execute_running(runtime.clone(), execution).unwrap()
        else {
            panic!("expected sparse array")
        };
        let costs = profile.snapshot();
        assert_eq!(
            context
                .get_property(&array, &runtime.intern_property_key("length").unwrap())
                .unwrap(),
            Value::Int(2)
        );
        assert!(
            runtime
                .get_own_property(&array, &runtime.intern_property_key("0").unwrap())
                .unwrap()
                .is_none()
        );
        assert!(matches!(
            runtime
                .get_own_property(&array, &runtime.intern_property_key("1").unwrap())
                .unwrap(),
            Some(
                crate::engine::object::CompleteOrdinaryPropertyDescriptor::Data {
                    value: Value::Int(40),
                    writable: true,
                    enumerable: true,
                    configurable: true
                }
            )
        ));
        assert_eq!(costs.legacy_dispatches, 0, "{costs:?}");
        assert_eq!(costs.owned_bridge_exits, 0, "{costs:?}");
        assert!(runtime.0.state.borrow().active_frames.is_empty());
    }

    #[test]
    fn array_literals_preserve_values_and_callee_realm_without_setters() {
        let runtime = Runtime::new();
        let mut caller = runtime.new_context();
        let mut callee = runtime.new_context();
        callee
            .eval("Object.defineProperty(Array.prototype,'0',{set(){throw 99},configurable:true})")
            .unwrap();
        let prototype = callee.eval("Array.prototype").unwrap();
        for (source, expected) in [
            ("(function(){return []})", vec![]),
            (
                "(function(a){return [40,a,2]})",
                vec![Value::Int(40), Value::Int(41), Value::Int(2)],
            ),
            (
                "(function(){return eval('[40,2]')})",
                vec![Value::Int(40), Value::Int(2)],
            ),
        ] {
            let Value::Object(function) = callee.eval(source).unwrap() else {
                panic!("expected function")
            };
            let callable = runtime.as_callable(&function).unwrap().unwrap();
            let entry = callable_entry(&runtime, &mut caller, callable, vec![Value::Int(41)]);
            let profile = CostProfile::start();
            let Completion::Return(Value::Object(array)) =
                execute(runtime.clone(), entry, ExecutionLimits::default()).unwrap()
            else {
                panic!("expected array")
            };
            let costs = profile.snapshot();
            assert_eq!(
                runtime
                    .fast_array_like_values(&array, expected.len() as u32)
                    .unwrap(),
                Some(expected)
            );
            assert_eq!(
                runtime.get_prototype_of(&array).unwrap().map(Value::Object),
                Some(prototype.clone())
            );
            assert_eq!(costs.legacy_dispatches, 0, "{source}: {costs:?}");
            assert_eq!(costs.owned_bridge_exits, 0, "{source}: {costs:?}");
            assert!(runtime.0.state.borrow().active_frames.is_empty());
        }
    }

    #[test]
    fn apply_eval_checkpoint_runs_children_and_cleans_argument_snapshots() {
        for (callee_source, array_source, expected, throws) in [
            ("eval", "['40+2',{}]", 42, false),
            ("eval", "[42,{}]", 42, false),
            ("eval", "[]", 0, false),
            ("eval", "['throw 42',{}]", 42, true),
            ("(function(a,b){return a+b})", "[40,2]", 42, false),
            (
                "(function(a,b){return a+b}).bind(null,40)",
                "[2]",
                42,
                false,
            ),
        ] {
            let runtime = Runtime::new();
            let mut context = runtime.new_context();
            let callee = context.eval(callee_source).unwrap();
            let array = context.eval(array_source).unwrap();
            let entry = entry(
                &runtime,
                &mut context,
                "(function(){'use strict';return eval(...[])})",
                vec![callee.clone()],
            );
            let pc = entry
                .executable
                .code
                .iter()
                .position(|op| {
                    matches!(
                        op,
                        crate::engine::code::bytecode::Instruction::ApplyEval { .. }
                    )
                })
                .unwrap();
            let mut execution =
                RunningExecution::new(&runtime, ExecutionLimits::default()).unwrap();
            let id = push_frame(&mut execution, entry).unwrap();
            let frame = execution.frames.current_mut(id).unwrap();
            frame.resume_pc = pc;
            execution.slots.push(&mut frame.window, callee).unwrap();
            execution
                .slots
                .push(&mut frame.window, array.clone())
                .unwrap();
            let retained = if array_source == "['40+2',{}]" {
                let Value::Object(carrier) = &array else {
                    unreachable!()
                };
                let Value::Object(extra) = context
                    .get_property(carrier, &runtime.intern_property_key("1").unwrap())
                    .unwrap()
                else {
                    panic!("expected extra argument")
                };
                Some(extra.object_id())
            } else {
                None
            };
            let profile = CostProfile::start();
            if let Some(extra) = retained {
                let RunExit::ApplyEval(environment) = run(&mut execution, id).unwrap() else {
                    panic!("expected ApplyEval")
                };
                assert!(matches!(
                    super::super::eval_driver::apply(&runtime, &mut execution, id, environment)
                        .unwrap(),
                    CallStep::Entered
                ));
                assert_ne!(execution.frames.current_id(), Some(id));
                let Value::Object(carrier) = &array else {
                    unreachable!()
                };
                assert!(
                    runtime
                        .delete_property(carrier, &runtime.intern_property_key("1").unwrap())
                        .unwrap()
                );
                assert!(
                    runtime
                        .0
                        .state
                        .borrow()
                        .heap
                        .object_strong_count(extra)
                        .unwrap()
                        > 0,
                    "snapshot must survive carrier mutation"
                );
            }
            let completion = execute_running(runtime.clone(), execution).unwrap();
            let costs = profile.snapshot();
            if let Some(extra) = retained {
                assert_eq!(
                    runtime
                        .0
                        .state
                        .borrow()
                        .heap
                        .object_strong_count(extra)
                        .unwrap_or(0),
                    0,
                    "snapshot must be released after reply"
                );
            }

            if throws {
                assert!(
                    matches!(completion, Completion::Throw(Value::Int(value)) if value == expected)
                );
            } else if array_source == "[]" {
                assert!(matches!(completion, Completion::Return(Value::Undefined)));
            } else {
                assert!(
                    matches!(completion, Completion::Return(Value::Int(value)) if value == expected),
                    "{callee_source}: {completion:?}"
                );
            }
            assert_eq!(costs.legacy_dispatches, 0, "{callee_source}: {costs:?}");
            assert_eq!(costs.owned_bridge_exits, 0, "{callee_source}: {costs:?}");
            assert!(runtime.0.state.borrow().active_frames.is_empty());
        }
    }

    #[test]
    fn direct_eval_keeps_boxed_this_identity_in_bound_callers() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let bound = context
            .eval("(function(){return eval('this')===this?42:0}).bind(7)")
            .unwrap();
        let entry = entry(
            &runtime,
            &mut context,
            "(function(f){return f()})",
            vec![bound],
        );
        let profile = CostProfile::start();
        let Completion::Return(value) =
            execute(runtime.clone(), entry, ExecutionLimits::default()).unwrap()
        else {
            panic!("expected eval this result")
        };
        let costs = profile.snapshot();
        assert_eq!(value, Value::Int(42));
        assert_eq!(costs.legacy_dispatches, 0, "{costs:?}");
        assert_eq!(costs.owned_bridge_exits, 0, "{costs:?}");
        assert!(costs.owned_storage.maximum_frame_depth >= 3, "{costs:?}");
        assert!(runtime.0.state.borrow().active_frames.is_empty());
    }

    #[test]
    fn direct_eval_runs_in_owned_children_and_shares_caller_bindings() {
        for (source, expected, message) in [
            ("(function(){let x=40;return eval('x+2')})", 42, None),
            ("(function(){let x=1;eval('x=42');return x})", 42, None),
            ("(function(){eval('var x=42');return x})", 42, None),
            (
                "(function(){'use strict';let x=40;return eval('x+2')})",
                42,
                None,
            ),
            (
                "(function(){let x=40;return eval('eval(\"x+2\")')})",
                42,
                None,
            ),
            (
                "(function(){try{return eval('throw 42')}catch(e){return e}})",
                42,
                None,
            ),
            (
                "(function(){try{return eval('let =')}catch(e){return 42}})",
                42,
                None,
            ),
            ("(function(){return eval(42)})", 42, None),
            ("(function(){var n=0;return eval(41,n=n+1)+n})", 42, None),
            (
                "(function(){let x=eval('x');return x})",
                0,
                Some("x is not initialized"),
            ),
            (
                "(function(){let eval=function(a,b){return a+b};return eval(40,2)})",
                42,
                None,
            ),
        ] {
            let runtime = Runtime::new();
            let mut context = runtime.new_context();
            let entry = entry(&runtime, &mut context, source, vec![]);
            let profile = CostProfile::start();
            let completion = execute(runtime.clone(), entry, ExecutionLimits::default()).unwrap();
            let costs = profile.snapshot();
            if let Some(message) = message {
                let Completion::Throw(Value::Object(error)) = completion else {
                    panic!("expected eval TDZ error")
                };
                assert_eq!(
                    context
                        .get_property(&error, &runtime.intern_property_key("message").unwrap())
                        .unwrap(),
                    Value::String(crate::engine::value::JsString::from_static(message))
                );
            } else {
                assert!(
                    matches!(completion, Completion::Return(ref value) if value == &Value::Int(expected)),
                    "{source}: {completion:?}"
                );
            }
            assert_eq!(costs.legacy_dispatches, 0, "{source}: {costs:?}");
            assert_eq!(costs.owned_bridge_exits, 0, "{source}: {costs:?}");
            assert!(runtime.0.state.borrow().active_frames.is_empty());
        }
    }

    #[test]
    fn global_reference_accessors_and_new_bindings_use_owned_calls() {
        for (setup, source, expected, stored, error_message) in [
            (
                "delete globalThis.x",
                "(function(o){with(o){return x=(function(){return 42})()}})",
                42,
                Some(42),
                None,
            ),
            (
                "(function(){let n=1;Object.defineProperty(globalThis,'x',{get(){return n},set(v){n=v;return 99},configurable:true})})()",
                "(function(o){with(o){return x+=(function(){return 41})()}})",
                42,
                Some(42),
                None,
            ),
            (
                "(function(){let n=0;globalThis.marker=41;Object.defineProperty(globalThis,'x',{get(){return n},set(v){n=n+v+this.marker;return 99},configurable:true})})()",
                "(function(o){with(o){return x+=(function(){return 1})()}})",
                1,
                Some(42),
                None,
            ),
            (
                "Object.defineProperty(globalThis,'x',{get(){throw 42},configurable:true})",
                "(function(o){try{with(o){x+=(function(){throw 99})()}}catch(e){return e}})",
                42,
                None,
                None,
            ),
            (
                "Object.defineProperty(globalThis,'x',{value:1,writable:false,configurable:true})",
                "(function(o){with(o){return x=(function(){return 42})()}})",
                42,
                Some(1),
                None,
            ),
            (
                "delete globalThis.x",
                "(function(o){with(o){return function(){'use strict';try{x=(function(){return 42})()}catch(e){return e}}}})({})",
                0,
                None,
                Some("'x' is not defined"),
            ),
        ] {
            let runtime = Runtime::new();
            let mut context = runtime.new_context();
            context.eval(setup).unwrap();
            let object = context.eval("({})").unwrap();
            let entry = entry(&runtime, &mut context, source, vec![object]);
            let profile = CostProfile::start();
            let Completion::Return(value) =
                execute(runtime.clone(), entry, ExecutionLimits::default()).unwrap()
            else {
                panic!("expected global access result")
            };
            let costs = profile.snapshot();
            if let Some(message) = error_message {
                let Value::Object(error) = value else {
                    panic!("expected strict unresolved error")
                };
                assert_eq!(
                    context
                        .get_property(&error, &runtime.intern_property_key("message").unwrap())
                        .unwrap(),
                    Value::String(crate::engine::value::JsString::from_static(message))
                );
            } else {
                assert_eq!(value, Value::Int(expected), "{source}");
            }
            if let Some(stored) = stored {
                assert_eq!(
                    context
                        .get_property(
                            &runtime.global_object_for_realm(context.realm).unwrap(),
                            &runtime.intern_property_key("x").unwrap()
                        )
                        .unwrap(),
                    Value::Int(stored)
                );
            }
            assert_eq!(costs.legacy_dispatches, 0, "{source}: {costs:?}");
            assert_eq!(costs.owned_bridge_exits, 0, "{source}: {costs:?}");
            assert!(runtime.0.state.borrow().active_frames.is_empty());
        }
    }

    #[test]
    fn global_reference_checkpoint_checks_presence_without_invoking_getters() {
        for (setup, present) in [
            ("globalThis.x=1", true),
            (
                "Object.defineProperty(globalThis,'x',{get(){throw 99},configurable:true})",
                true,
            ),
            (
                "Object.setPrototypeOf(globalThis,{get x(){throw 99}})",
                true,
            ),
            ("delete globalThis.x", false),
        ] {
            let runtime = Runtime::new();
            let mut context = runtime.new_context();
            context.eval(setup).unwrap();
            let entry = entry(
                &runtime,
                &mut context,
                "(function(o){with(o){x=(function(){return 42})()}})",
                vec![Value::Undefined],
            );
            let pc = entry
                .executable
                .code
                .iter()
                .position(|op| {
                    matches!(
                        op,
                        crate::engine::code::bytecode::Instruction::GlobalReference(_)
                    )
                })
                .unwrap();
            let mut execution =
                RunningExecution::new(&runtime, ExecutionLimits::default()).unwrap();
            let id = push_frame(&mut execution, entry).unwrap();
            execution.frames.current_mut(id).unwrap().resume_pc = pc;
            let RunExit::Environment(
                op @ super::super::environment_driver::Operation::GlobalReference(_),
            ) = run(&mut execution, id).unwrap()
            else {
                panic!("expected global reference")
            };
            assert!(matches!(
                super::super::environment_driver::step(&runtime, &mut execution, id, op).unwrap(),
                CallStep::Entered
            ));
            let frame = execution.frames.current_mut(id).unwrap();
            assert_eq!(frame.resume_pc, pc + 1);
            assert_eq!(execution.slots.depth(&frame.window), 1);
            let expected = if present {
                Value::Object(runtime.global_object_for_realm(context.realm).unwrap())
            } else {
                Value::Undefined
            };
            assert_eq!(execution.slots.peek(&frame.window, 0).unwrap(), &expected);
            drop(execution);
            assert!(runtime.0.state.borrow().active_frames.is_empty());
        }
    }

    #[test]
    fn global_references_prefer_late_lexical_cells_before_evaluating_rhs() {
        for (lexical, is_const, initial, expected_error) in [
            (false, false, Some(Value::Int(1)), None),
            (true, false, Some(Value::Int(1)), None),
            (true, true, Some(Value::Int(1)), Some("'x' is read-only")),
            (true, false, None, Some("x is not initialized")),
        ] {
            let runtime = Runtime::new();
            let mut context = runtime.new_context();
            context.eval("globalThis.x=1").unwrap();
            let object = context.eval("({})").unwrap();
            let source = if expected_error.is_some() {
                "(function(o){try{with(o){return x=(function(){throw 99})()}}catch(e){return e}})"
            } else {
                "(function(o){with(o){return x=(function(){return 42})()}})"
            };
            let entry = entry(&runtime, &mut context, source, vec![object]);
            assert!(entry.executable.code.iter().any(|op| matches!(
                op,
                crate::engine::code::bytecode::Instruction::GlobalReference(_)
            )));
            // Publish against the property first, then introduce a live lexical cell.
            if lexical {
                context
                    .create_global_lexical_for_test("x", is_const, initial)
                    .unwrap();
            }
            let profile = CostProfile::start();
            let Completion::Return(value) =
                execute(runtime.clone(), entry, ExecutionLimits::default()).unwrap()
            else {
                panic!("expected global reference result")
            };
            let costs = profile.snapshot();
            let key = runtime.intern_property_key("x").unwrap();
            if let Some(message) = expected_error {
                let Value::Object(error) = value else {
                    panic!("RHS ran before lexical validation")
                };
                assert_eq!(
                    context
                        .get_property(&error, &runtime.intern_property_key("message").unwrap())
                        .unwrap(),
                    Value::String(crate::engine::value::JsString::from_static(message))
                );
            } else {
                assert_eq!(value, Value::Int(42));
                if lexical {
                    let root = runtime
                        .own_var_ref_root(&context.global_var_object().unwrap(), &key)
                        .unwrap()
                        .unwrap();
                    assert_eq!(runtime.read_var_ref(&root).unwrap(), Value::Int(42));
                }
            }
            let global = runtime.global_object_for_realm(context.realm).unwrap();
            assert_eq!(
                context.get_property(&global, &key).unwrap(),
                Value::Int(if lexical { 1 } else { 42 })
            );
            assert_eq!(costs.legacy_dispatches, 0, "{source}: {costs:?}");
            assert_eq!(costs.owned_bridge_exits, 0, "{source}: {costs:?}");
            assert!(runtime.0.state.borrow().active_frames.is_empty());
        }
    }

    #[test]
    fn reference_read_checkpoint_preserves_cells_and_reports_unresolved_names() {
        use crate::engine::vm::environment_driver::Operation;
        for (initial, unresolved, expected_error) in [
            (Some(Value::Int(42)), false, None),
            (None, false, Some("x is not initialized")),
            (Some(Value::Int(42)), true, Some("'x' is not defined")),
        ] {
            let runtime = Runtime::new();
            let mut context = runtime.new_context();
            context
                .create_global_lexical_for_test("x", false, initial)
                .unwrap();
            let object = context.global_var_object().unwrap();
            let key = runtime.intern_property_key("x").unwrap();
            let root = runtime.own_var_ref_root(&object, &key).unwrap().unwrap();
            let entry = entry(
                &runtime,
                &mut context,
                "(function(o){with(o){x+=(function(){return 1})()}})",
                vec![Value::Object(object.clone())],
            );
            let pc = entry
                .executable
                .code
                .iter()
                .position(|op| {
                    matches!(
                        op,
                        crate::engine::code::bytecode::Instruction::GetRefValue(_)
                            | crate::engine::code::bytecode::Instruction::GetRefValueUndef(_)
                    )
                })
                .expect("expected published reference read");
            let mut execution =
                RunningExecution::new(&runtime, ExecutionLimits::default()).unwrap();
            let id = push_frame(&mut execution, entry).unwrap();
            let frame = execution.frames.current_mut(id).unwrap();
            frame.resume_pc = pc;
            let base = if unresolved {
                Value::Undefined
            } else {
                Value::Object(object.clone())
            };
            execution
                .slots
                .push(&mut frame.window, base.clone())
                .unwrap();
            let RunExit::Environment(op @ Operation::ReadReference { .. }) =
                run(&mut execution, id).unwrap()
            else {
                panic!("expected reference read")
            };
            let profile = CostProfile::start();
            let result =
                super::super::environment_driver::step(&runtime, &mut execution, id, op).unwrap();
            let costs = profile.snapshot();
            if let Some(message) = expected_error {
                let CallStep::Complete(Completion::Throw(Value::Object(error))) = result else {
                    panic!("expected reference error")
                };
                assert_eq!(
                    context
                        .get_property(&error, &runtime.intern_property_key("message").unwrap())
                        .unwrap(),
                    Value::String(crate::engine::value::JsString::from_static(message))
                );
            } else {
                assert!(matches!(result, CallStep::Entered));
                let frame = execution.frames.current_mut(id).unwrap();
                assert_eq!(frame.resume_pc, pc + 1);
                assert_eq!(execution.slots.depth(&frame.window), 2);
                assert_eq!(
                    execution.slots.peek(&frame.window, 0).unwrap(),
                    &Value::Int(42)
                );
                assert_eq!(execution.slots.peek(&frame.window, 1).unwrap(), &base);
            }
            assert_eq!(
                runtime
                    .own_var_ref_root(&object, &key)
                    .unwrap()
                    .unwrap()
                    .id(),
                root.id()
            );
            assert_eq!(costs.legacy_dispatches, 0);
            assert_eq!(costs.owned_bridge_exits, 0);
            drop(execution);
            assert!(runtime.0.state.borrow().active_frames.is_empty());
        }
    }

    #[test]
    fn reference_write_checkpoint_preserves_live_lexical_cells() {
        use crate::engine::vm::environment_driver::{Operation, WriteTarget};
        for (is_const, initial, strict, expected_error) in [
            (false, Some(Value::Int(1)), false, None),
            (false, Some(Value::Int(1)), true, None),
            (true, Some(Value::Int(1)), false, None),
            (true, Some(Value::Int(1)), true, Some("'x' is read-only")),
            (false, None, false, Some("x is not initialized")),
        ] {
            let runtime = Runtime::new();
            let mut context = runtime.new_context();
            context
                .create_global_lexical_for_test("x", is_const, initial.clone())
                .unwrap();
            let object = context.global_var_object().unwrap();
            let key = runtime.intern_property_key("x").unwrap();
            let root = runtime.own_var_ref_root(&object, &key).unwrap().unwrap();
            let entry = entry(
                &runtime,
                &mut context,
                if strict {
                    "(function(o){with(o){return function(){'use strict';x=(function(){return 42})()}}})({})"
                } else {
                    "(function(o){with(o){x=(function(){return 42})()}})"
                },
                vec![Value::Object(object.clone())],
            );
            let (pc, name) = entry
                .executable
                .code
                .iter()
                .enumerate()
                .find_map(|(pc, op)| match op {
                    crate::engine::code::bytecode::Instruction::PutRefValue(name) => {
                        Some((pc, *name))
                    }
                    _ => None,
                })
                .expect("expected published reference write");
            let mut execution =
                RunningExecution::new(&runtime, ExecutionLimits::default()).unwrap();
            let id = push_frame(&mut execution, entry).unwrap();
            let frame = execution.frames.current_mut(id).unwrap();
            frame.resume_pc = pc;
            execution
                .slots
                .push(&mut frame.window, Value::Object(object.clone()))
                .unwrap();
            execution
                .slots
                .push(&mut frame.window, Value::Int(42))
                .unwrap();
            let op = Operation::Put {
                source: WriteTarget::Reference,
                name,
                strict,
                check_presence: true,
            };
            assert_eq!(run(&mut execution, id).unwrap(), RunExit::Environment(op));
            let result = super::super::environment_driver::step(
                &runtime,
                &mut execution,
                id,
                Operation::Put {
                    source: WriteTarget::Reference,
                    name,
                    strict,
                    check_presence: true,
                },
            )
            .unwrap();
            if let Some(message) = expected_error {
                let CallStep::Complete(Completion::Throw(Value::Object(error))) = result else {
                    panic!("expected lexical rejection")
                };
                assert_eq!(
                    context
                        .get_property(&error, &runtime.intern_property_key("message").unwrap())
                        .unwrap(),
                    Value::String(crate::engine::value::JsString::from_static(message))
                );
            } else {
                assert!(matches!(result, CallStep::Entered));
                let frame = execution.frames.current_mut(id).unwrap();
                assert_eq!(frame.resume_pc, pc + 1);
                assert_eq!(execution.slots.depth(&frame.window), 0);
                assert_eq!(
                    runtime.read_var_ref(&root).unwrap(),
                    if is_const {
                        Value::Int(1)
                    } else {
                        Value::Int(42)
                    }
                );
            }
            assert_eq!(
                runtime
                    .own_var_ref_root(&object, &key)
                    .unwrap()
                    .unwrap()
                    .id(),
                root.id()
            );
            drop(execution);
            assert!(runtime.0.state.borrow().active_frames.is_empty());
        }
    }

    #[test]
    fn dynamic_assignments_preserve_setter_receiver_and_rejection_rules() {
        for (setup, body, expected_error) in [
            ("({x:0})", "with(o){x=42;return x}", None),
            ("Object.create({x:0})", "with(o){x=42;return x}", None),
            (
                "(function(){let n=0;return {y:1,get x(){return n},set x(v){n=n+v+this.y;return 99}}})()",
                "with(o){x=41;return x}",
                None,
            ),
            (
                "(function(){let n=0;return Object.create({get x(){return n},set x(v){n=n+v+this.y;return 99}}, {y:{value:1}})})()",
                "with(o){x=41;return x}",
                None,
            ),
            (
                "({set x(v){throw v}})",
                "try{with(o){x=42}}catch(e){return e}",
                None,
            ),
            (
                "Object.defineProperty({},'x',{value:42})",
                "with(o){x=99;return x}",
                None,
            ),
            ("({get x(){return 42}})", "with(o){x=99;return x}", None),
            (
                "({x:1})",
                "with(o){x=(function(){delete x;return 42})()}return o.x",
                None,
            ),
            (
                "Object.defineProperty({},'x',{value:42})",
                "try{with(o){return (function(){'use strict';x=99})()}}catch(e){return e}",
                Some("'x' is read-only"),
            ),
            (
                "({get x(){return 42}})",
                "try{with(o){return (function(){'use strict';x=99})()}}catch(e){return e}",
                Some("no setter for property"),
            ),
        ] {
            let runtime = Runtime::new();
            let mut context = runtime.new_context();
            let object = context.eval(setup).unwrap();
            let entry = entry(
                &runtime,
                &mut context,
                &format!("(function(o){{{body}}})"),
                vec![object],
            );
            let profile = CostProfile::start();
            let Completion::Return(value) =
                execute(runtime.clone(), entry, ExecutionLimits::default()).unwrap()
            else {
                panic!("expected assignment result: {body}")
            };
            let costs = profile.snapshot();
            if let Some(message) = expected_error {
                let Value::Object(error) = value else {
                    panic!("expected error: {body}")
                };
                assert_eq!(
                    context
                        .get_property(&error, &runtime.intern_property_key("message").unwrap())
                        .unwrap(),
                    Value::String(crate::engine::value::JsString::from_static(message))
                );
            } else {
                assert_eq!(value, Value::Int(42), "{body}");
            }
            assert_eq!(costs.legacy_dispatches, 0, "{body}: {costs:?}");
            assert_eq!(costs.owned_bridge_exits, 0, "{body}: {costs:?}");
            assert!(runtime.0.state.borrow().active_frames.is_empty());
        }
    }

    #[test]
    fn published_eval_declarations_and_reads_use_owned_environment() {
        use crate::engine::code::debug::DebugInfoMode;
        use crate::engine::code::function::metadata::{ClosureVariableKind, EvalRootBinding};
        use crate::engine::compiler::{EvalCompileContext, compile_unlinked_eval_with_filename};
        use crate::engine::value::JsString;

        for (source, setup, expected, error_message) in [
            ("var x; x", "Object.create(null)", Value::Undefined, None),
            (
                "var x; x=42; x",
                "Object.create(null)",
                Value::Int(42),
                None,
            ),
            (
                "function x(){return 42} x()",
                "Object.create(null)",
                Value::Int(42),
                None,
            ),
            (
                "var x; function x(){return 42} x()",
                "Object.create(null)",
                Value::Int(42),
                None,
            ),
            (
                "var x; delete x",
                "Object.create(null)",
                Value::Bool(true),
                None,
            ),
            (
                "x",
                "({x:42,get [Symbol.unscopables](){throw 99}})",
                Value::Int(42),
                None,
            ),
            (
                "x",
                "Object.create({get x(){return this.y}}, {y:{value:42}})",
                Value::Int(42),
                None,
            ),
            (
                "try{x}catch(e){e}",
                "({get x(){throw 42}})",
                Value::Int(42),
                None,
            ),
            ("var x; x", "({get x(){throw 99}})", Value::Undefined, None),
            (
                "var x; x",
                "Object.defineProperty({},'x',{value:42})",
                Value::Undefined,
                Some("property is not configurable"),
            ),
        ] {
            let runtime = Runtime::new();
            let mut context = runtime.new_context();
            let eval_context = EvalCompileContext::direct(
                false,
                vec![EvalRootBinding {
                    name: JsString::from_static("<var>"),
                    scope: 0,
                    is_lexical: false,
                    is_const: false,
                    kind: ClosureVariableKind::EvalVariableObject,
                    is_catch_parameter: false,
                }],
            );
            let unlinked = compile_unlinked_eval_with_filename(
                source,
                "<eval>",
                DebugInfoMode::Full,
                eval_context.clone(),
            )
            .unwrap();
            let verified = crate::engine::code::bytecode_publish::VerifiedFunction::eval(
                unlinked,
                crate::engine::api::compile::eval_publication_input(&eval_context),
            )
            .unwrap();
            let bytecode = runtime
                .publish_verified_unlinked_function(context.realm, verified)
                .unwrap();
            let Value::Object(object) = context.eval(setup).unwrap() else {
                panic!("expected eval environment")
            };
            let root = runtime
                .new_var_ref(
                    Value::Object(object),
                    false,
                    false,
                    ClosureVariableKind::EvalVariableObject,
                )
                .unwrap();
            let mut roots = vec![root];
            let snapshot = runtime.snapshot_function_bytecode(&bytecode).unwrap();
            for descriptor in snapshot.closure_variables.iter().skip(1) {
                use crate::engine::code::function::metadata::{ClosureSource, ClosureVariableName};
                assert_eq!(descriptor.source, ClosureSource::Global);
                let ClosureVariableName::Atom(name) = descriptor.name else {
                    panic!("expected global atom")
                };
                roots.push(runtime.resolve_global_var(context.realm, name).unwrap());
            }
            let callable = runtime
                .new_bytecode_closure_with_slots(context.realm, &bytecode, &roots)
                .unwrap();
            let entry = callable_entry(&runtime, &mut context, callable, Vec::new());
            let profile = CostProfile::start();
            let completion = execute(runtime.clone(), entry, ExecutionLimits::default()).unwrap();
            let costs = profile.snapshot();
            if let Some(message) = error_message {
                let Completion::Throw(Value::Object(error)) = completion else {
                    panic!("{source}: expected definition error")
                };
                assert_eq!(
                    context
                        .get_property(&error, &runtime.intern_property_key("message").unwrap())
                        .unwrap(),
                    Value::String(JsString::from_static(message))
                );
            } else {
                assert!(
                    matches!(completion, Completion::Return(ref value) if value == &expected),
                    "{source}: {completion:?}"
                );
            }
            assert_eq!(costs.legacy_dispatches, 0, "{source}: {costs:?}");
            assert_eq!(costs.owned_bridge_exits, 0, "{source}: {costs:?}");
            assert!(runtime.0.state.borrow().active_frames.is_empty());
        }
    }

    #[test]
    fn dynamic_deletion_respects_own_descriptors_and_unscopables() {
        for (setup, expected, own_after) in [
            ("({x:42})", true, false),
            (
                "Object.defineProperty({},'x',{value:42,configurable:false})",
                false,
                true,
            ),
            ("({get x(){throw 99}})", true, false),
            ("Object.create({x:42})", true, false),
            ("({x:42,[Symbol.unscopables]:{x:true}})", false, true),
            ("({})", false, false),
        ] {
            let runtime = Runtime::new();
            let mut context = runtime.new_context();
            let Value::Object(object) = context.eval(setup).unwrap() else {
                panic!("expected delete target")
            };
            let entry = entry(
                &runtime,
                &mut context,
                "(function(o){let x=41;with(o){return delete x}})",
                vec![Value::Object(object.clone())],
            );
            let profile = CostProfile::start();
            let Completion::Return(value) =
                execute(runtime.clone(), entry, ExecutionLimits::default()).unwrap()
            else {
                panic!("expected delete result")
            };
            assert_eq!(value, Value::Bool(expected), "{setup}");
            let costs = profile.snapshot();
            let key = runtime.intern_property_key("x").unwrap();
            assert_eq!(
                runtime.get_own_property(&object, &key).unwrap().is_some(),
                own_after,
                "{setup}"
            );
            assert_eq!(costs.legacy_dispatches, 0, "{setup}: {costs:?}");
            assert_eq!(costs.owned_bridge_exits, 0, "{setup}: {costs:?}");
            assert!(runtime.0.state.borrow().active_frames.is_empty());
        }
    }

    #[test]
    fn class_heritage_getter_replies_once_before_publication() {
        for (getter, expected_error) in [
            ("function(){n=n+1;return prototype}", false),
            ("function(){n=n+1;throw 41}", false),
            ("function(){n=n+1;return 1}", true),
        ] {
            let runtime = Runtime::new();
            let mut context = runtime.new_context();
            let parent = context.eval(&format!("(function(){{var n=0,prototype={{}};var B=(function(){{}}).bind(null);Object.defineProperty(B,'prototype',{{get:{getter}}});return [B,function(){{return n}}]}})()")).unwrap();
            let Value::Object(pair) = parent else {
                panic!("expected parent pair")
            };
            let parent = context
                .get_property(&pair, &runtime.intern_property_key("0").unwrap())
                .unwrap();
            let counter = context
                .get_property(&pair, &runtime.intern_property_key("1").unwrap())
                .unwrap();
            let entry = entry(
                &runtime,
                &mut context,
                "(function(B,count){try{class C extends B {}return 41+count()}catch(e){if(e===41)return e+count();return e}})",
                vec![parent, counter],
            );
            let profile = CostProfile::start();
            let Completion::Return(value) =
                execute(runtime.clone(), entry, ExecutionLimits::default()).unwrap()
            else {
                panic!("expected class heritage completion")
            };
            let costs = profile.snapshot();
            if expected_error {
                let Value::Object(error) = value else {
                    panic!("expected invalid prototype error")
                };
                assert_eq!(
                    context
                        .get_property(&error, &runtime.intern_property_key("name").unwrap())
                        .unwrap(),
                    Value::String(crate::engine::value::JsString::from_static("TypeError"))
                );
            } else {
                assert_eq!(value, Value::Int(42))
            }
            assert_eq!(costs.legacy_dispatches, 0, "{getter}: {costs:?}");
            assert_eq!(costs.owned_bridge_exits, 0, "{getter}: {costs:?}");
            assert_eq!(costs.owned_storage.maximum_frame_depth, 2);
            assert!(runtime.0.state.borrow().active_frames.is_empty());
        }
    }

    #[test]
    fn class_initializer_installation_and_static_calls_do_not_replay() {
        use super::super::construct_driver::{InitializerKind, initializer};
        use crate::engine::code::bytecode::Instruction;
        use crate::engine::code::function::metadata::ClassInitializerKind;
        use crate::engine::code::rooted::FunctionBytecodeRef;
        use crate::engine::heap::BytecodeConstant;

        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let entry = entry(
            &runtime,
            &mut context,
            "(function(){return class {x=42; static {throw 42}}})",
            vec![],
        );
        let pc = entry
            .executable
            .code
            .iter()
            .position(|op| matches!(op, Instruction::RunClassStaticInitializer))
            .unwrap();
        let mut constructor = None;
        let mut static_initializer = None;
        let mut instance_initializer = None;
        for constant in entry.executable.constants.iter() {
            let BytecodeConstant::Function(child) = constant else {
                continue;
            };
            let metadata = runtime
                .0
                .state
                .borrow()
                .heap
                .function_bytecode(*child)
                .unwrap()
                .metadata;
            let bytecode =
                FunctionBytecodeRef::from_borrowed_handle(runtime.clone(), *child).unwrap();
            let callable = runtime
                .new_bytecode_closure(context.realm, &bytecode)
                .unwrap();
            if metadata.class_initializer_kind == Some(ClassInitializerKind::StaticElements) {
                static_initializer = Some(Value::Object(callable.into_object()));
            } else if metadata.class_initializer_kind == Some(ClassInitializerKind::InstanceFields)
            {
                instance_initializer = Some(Value::Object(callable.into_object()));
            } else if metadata.constructor_kind
                != crate::engine::code::function::metadata::ConstructorKind::None
            {
                constructor = Some(Value::Object(callable.into_object()));
            }
        }
        let crate::engine::vm::DefineClassOutcome::Defined {
            constructor,
            prototype,
        } = runtime
            .define_class_pair(
                context.realm,
                Value::Undefined,
                constructor.unwrap(),
                &crate::engine::value::JsString::from_static(""),
                false,
            )
            .unwrap()
        else {
            panic!("expected fresh class pair")
        };
        let static_initializer = static_initializer.unwrap();
        let install_pc = entry
            .executable
            .code
            .iter()
            .position(|op| matches!(op, Instruction::InstallClassInstanceInitializer))
            .unwrap();
        let mut execution = RunningExecution::new(&runtime, ExecutionLimits::default()).unwrap();
        let parent = push_frame(&mut execution, entry).unwrap();
        let frame = execution.frames.current_mut(parent).unwrap();
        frame.resume_pc = install_pc;
        execution
            .slots
            .push(&mut frame.window, constructor.clone())
            .unwrap();
        execution
            .slots
            .push(&mut frame.window, prototype.clone())
            .unwrap();
        execution
            .slots
            .push(&mut frame.window, instance_initializer.unwrap())
            .unwrap();
        let profile = CostProfile::start();
        assert_eq!(
            run(&mut execution, parent).unwrap(),
            RunExit::ClassInitializer(InitializerKind::Install)
        );
        assert!(matches!(
            initializer(&runtime, &mut execution, parent, InitializerKind::Install).unwrap(),
            CallStep::Entered
        ));
        assert_eq!(execution.frames.current_id(), Some(parent));
        let frame = execution.frames.current_mut(parent).unwrap();
        assert_eq!(frame.resume_pc, install_pc + 1);
        assert_eq!(execution.slots.depth(&frame.window), 2);
        assert_eq!(
            execution.slots.peek(&frame.window, 1).unwrap(),
            &constructor
        );
        assert_eq!(execution.slots.pop(&mut frame.window).unwrap(), prototype);
        frame.resume_pc = pc;
        execution
            .slots
            .push(&mut frame.window, static_initializer.clone())
            .unwrap();
        assert_eq!(
            run(&mut execution, parent).unwrap(),
            RunExit::ClassInitializer(InitializerKind::Static)
        );
        assert!(matches!(
            initializer(&runtime, &mut execution, parent, InitializerKind::Static).unwrap(),
            CallStep::Entered
        ));
        let child = execution.frames.current_id().unwrap();
        assert_ne!(child, parent);
        let RunExit::InstantiateClosure(index) = run(&mut execution, child).unwrap() else {
            panic!("expected static block closure")
        };
        super::super::closure_driver::instantiate(&runtime, &mut execution, child, index).unwrap();
        assert_eq!(
            run(&mut execution, child).unwrap(),
            RunExit::ClassInitializer(InitializerKind::Block)
        );
        assert!(matches!(
            initializer(&runtime, &mut execution, child, InitializerKind::Block).unwrap(),
            CallStep::Entered
        ));
        let block = execution.frames.current_id().unwrap();
        assert_ne!(block, child);
        assert_eq!(run(&mut execution, block).unwrap(), RunExit::Throw);
        let frame = execution.frames.current_mut(block).unwrap();
        assert_eq!(
            execution.slots.pop(&mut frame.window).unwrap(),
            Value::Int(42)
        );
        // Preparation is irreversible even when the body throws.
        assert!(
            runtime
                .begin_class_static_initializer(context.realm, constructor, static_initializer)
                .is_err()
        );
        let costs = profile.snapshot();
        assert_eq!(costs.legacy_dispatches, 0, "{costs:?}");
        assert_eq!(costs.owned_bridge_exits, 0, "{costs:?}");
        assert_eq!(costs.owned_storage.maximum_frame_depth, 3);
        drop(execution);
        assert!(runtime.0.state.borrow().active_frames.is_empty());
    }

    #[test]
    fn logical_limit_returns_a_throw_and_unwinds_every_active_frame() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let entry = entry(
            &runtime,
            &mut context,
            "(function f(){return f()})",
            Vec::new(),
        );
        let profile = CostProfile::start();
        let completion = execute(
            runtime.clone(),
            entry,
            ExecutionLimits {
                frames: 8,
                slots: 1024,
            },
        )
        .unwrap();
        let Completion::Throw(Value::Object(error)) = completion else {
            panic!("expected catchable overflow");
        };
        assert_eq!(profile.snapshot().owned_storage.maximum_frame_depth, 8);
        assert_eq!(profile.snapshot().owned_storage.frames_pushed, 8);
        assert_eq!(profile.snapshot().owned_bridge_exits, 0);
        assert_eq!(profile.snapshot().legacy_dispatches, 0);
        assert!(runtime.0.state.borrow().active_frames.is_empty());
        assert_eq!(
            context
                .get_property(&error, &runtime.intern_property_key("message").unwrap())
                .unwrap(),
            Value::String(crate::engine::value::JsString::from_static(
                "stack overflow"
            ))
        );
    }
    #[test]
    fn nested_bound_calls_keep_argument_order() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let callee = context
            .eval("(function(a,b,c){return a*100+b*10+c}).bind(1000,1).bind(9000,2)")
            .unwrap();
        let entry = entry(
            &runtime,
            &mut context,
            "(function root(f){return f(3)})",
            vec![callee],
        );
        let profile = CostProfile::start();
        let result = execute(runtime.clone(), entry, ExecutionLimits::default()).unwrap();
        assert!(matches!(result, Completion::Return(Value::Int(123))));
        let costs = profile.snapshot();
        assert_eq!(costs.owned_storage.maximum_frame_depth, 2);
        assert_eq!(costs.legacy_dispatches, 0);
        assert_eq!(costs.owned_bridge_exits, 0);
        assert!(runtime.0.state.borrow().active_frames.is_empty());
    }

    #[test]
    fn bound_receiver_and_native_fallback_preserve_original_call() {
        for (source, expected, depth) in [
            (
                "(function(a,b,c){'use strict';return this+a*100+b*10+c}).bind(1000,1).bind(9000,2)",
                Value::Int(1123),
                2,
            ),
            (
                "String.fromCharCode.bind(null,65).bind(null,66)",
                Value::String(crate::engine::value::JsString::from_static("ABC")),
                1,
            ),
        ] {
            let runtime = Runtime::new();
            let mut context = runtime.new_context();
            let callee = context.eval(source).unwrap();
            let argument = if depth == 1 { 67 } else { 3 };
            let entry = entry(
                &runtime,
                &mut context,
                "(function root(f,x){return f(x)})",
                vec![callee, Value::Int(argument)],
            );
            let profile = CostProfile::start();
            let result = execute(runtime.clone(), entry, ExecutionLimits::default()).unwrap();
            let Completion::Return(value) = result else {
                panic!("expected return")
            };
            assert_eq!(value, expected);
            assert_eq!(profile.snapshot().owned_storage.maximum_frame_depth, depth);
            assert_eq!(profile.snapshot().owned_bridge_exits, 0);
            assert_eq!(profile.snapshot().legacy_dispatches, 0);
            assert!(runtime.0.state.borrow().active_frames.is_empty());
        }
    }

    #[test]
    fn boxed_this_identity_survives_frame_handoff() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let callee = context
            .eval("(function(){return this === ('x' in {},this)}).bind(3)")
            .unwrap();
        let entry = entry(
            &runtime,
            &mut context,
            "(function root(f){return f()})",
            vec![callee],
        );
        let profile = CostProfile::start();
        let result = execute(runtime.clone(), entry, ExecutionLimits::default()).unwrap();
        assert!(matches!(result, Completion::Return(Value::Bool(true))));
        assert_eq!(profile.snapshot().owned_storage.maximum_frame_depth, 2);
        assert_eq!(profile.snapshot().owned_bridge_exits, 1);
        assert!(runtime.0.state.borrow().active_frames.is_empty());
    }

    #[test]
    fn named_accessors_on_non_proxy_storage_use_owned_getter_frames() {
        for expression in [
            "(function(){return arguments})(7)",
            "new Uint8Array(2)",
            "new String('text')",
            "new Number(7)",
            "new Map()",
        ] {
            let runtime = Runtime::new();
            let mut context = runtime.new_context();
            let object = context.eval(&format!(
                "var hits=0;var object={expression};object.marker=42;Object.defineProperty(object,'x',{{get:function(){{hits++;return this.marker}}}});object"
            )).unwrap();
            let entry = entry(
                &runtime,
                &mut context,
                "(function(o){return o.x})",
                vec![object],
            );
            let profile = CostProfile::start();
            let result = execute(runtime.clone(), entry, ExecutionLimits::default()).unwrap();
            assert!(
                matches!(result, Completion::Return(Value::Int(42))),
                "{expression}"
            );
            let costs = profile.snapshot();
            assert_eq!(costs.owned_storage.frames_pushed, 2, "{expression}");
            assert_eq!(costs.legacy_dispatches, 0, "{expression}");
            assert_eq!(costs.owned_bridge_exits, 0, "{expression}");
            assert_eq!(costs.owned_sync_call_bridges, 0, "{expression}");
            drop(profile);
            assert_eq!(context.eval("hits").unwrap(), Value::Int(1));
        }
    }

    #[test]
    fn computed_reads_keep_receivers_holes_and_typed_index_terminals() {
        for (setup, source, expected, calls) in [
            (
                "var hits=0;var object=[,];object.marker=42;Object.setPrototypeOf(object,{get 0(){hits++;return this.marker}});object",
                "(function(o){return o[0]})",
                Value::Int(42),
                1,
            ),
            (
                "var hits=0;var object=(function(){return arguments})(7);object.marker=42;Object.defineProperty(object,'0',{get:function(){hits++;return function(){return this.marker}}});object",
                "(function(o){return o[0]()})",
                Value::Int(42),
                1,
            ),
            (
                "var hits=0;var object=new Uint8Array([7]);Object.setPrototypeOf(object,{get '-0'(){hits++;throw 99},get '1'(){hits++;throw 98}});object",
                "(function(o){var k='-0';return o[k]===undefined && o[1]===undefined})",
                Value::Bool(true),
                0,
            ),
            (
                "var hits=0;var object={};object[Symbol.iterator]=42;object",
                "(function(o){return o[Symbol.iterator]})",
                Value::Int(42),
                0,
            ),
            (
                "var hits=0;var object={'true':42,'1.5':42};object",
                "(function(o){return o[true] + o[1.5]})",
                Value::Int(84),
                0,
            ),
        ] {
            let runtime = Runtime::new();
            let mut context = runtime.new_context();
            let object = context.eval(setup).unwrap();
            let entry = entry(&runtime, &mut context, source, vec![object]);
            let profile = CostProfile::start();
            let result = execute(runtime.clone(), entry, ExecutionLimits::default()).unwrap();
            let Completion::Return(value) = result else {
                panic!("read threw: {source}");
            };
            assert_eq!(value, expected, "{source}");
            let costs = profile.snapshot();
            assert_eq!(costs.legacy_dispatches, 0, "{source}");
            assert_eq!(costs.owned_bridge_exits, 0, "{source}");
            assert_eq!(costs.owned_sync_call_bridges, 0, "{source}");
            drop(profile);
            assert_eq!(context.eval("hits").unwrap(), Value::Int(calls));
        }
    }

    #[test]
    fn pending_native_call_can_be_abandoned_without_invocation_or_runtime_cycle() {
        let profile = CostProfile::start();
        let (weak, pending) = {
            let runtime = Runtime::new();
            let mut context = runtime.new_context();
            let callee = context.eval("Math.abs").unwrap();
            let entry = entry(
                &runtime,
                &mut context,
                "(function(f){return f(42)})",
                vec![callee],
            );
            let weak = std::rc::Rc::downgrade(&runtime.0);
            let pending =
                super::execute(runtime.clone(), entry, ExecutionLimits::default()).unwrap();
            assert!(matches!(pending, super::RunningExit::Call(_)));
            assert!(!runtime.0.state.borrow().active_frames.is_empty());
            (weak, pending)
        };
        assert!(weak.upgrade().is_some());
        assert_eq!(profile.snapshot().owned_sync_call_bridges, 0);
        drop(pending);
        assert!(weak.upgrade().is_none());
        assert_eq!(profile.snapshot().owned_sync_call_bridges, 0);
    }

    #[test]
    fn computed_update_retains_the_original_key_across_getter_and_write_handoff() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let object = context.eval("var key='x',hits=0,written=0;({get x(){hits++;key='y';return 40},set x(v){written=v},set y(v){throw 99}})").unwrap();
        let entry = entry(
            &runtime,
            &mut context,
            "(function(o){return o[key]++})",
            vec![object],
        );
        let profile = CostProfile::start();
        let result = execute(runtime.clone(), entry, ExecutionLimits::default()).unwrap();
        assert!(matches!(result, Completion::Return(Value::Int(40))));
        let costs = profile.snapshot();
        assert_eq!(costs.owned_storage.maximum_frame_depth, 2);
        // Set is still awaiting its S05 driver path. The completed Get must
        // carry its old canonical key into that handoff, without rereading it.
        assert_eq!(costs.owned_bridge_exits, 1);
        drop(profile);
        assert_eq!(context.eval("hits").unwrap(), Value::Int(1));
        assert_eq!(context.eval("written").unwrap(), Value::Int(41));
        assert_eq!(
            context.eval("key").unwrap(),
            Value::String(crate::engine::value::JsString::from_static("y"))
        );
    }

    #[test]
    fn nullish_computed_reads_reject_before_key_callbacks() {
        for (source, expected) in [
            (
                "(function(o,k){try{return o[k]}catch(e){return e.message}})",
                "cannot read property of null",
            ),
            (
                "(function(o,k){try{return o[k]++}catch(e){return e.message}})",
                "value has no property",
            ),
        ] {
            let runtime = Runtime::new();
            let mut context = runtime.new_context();
            let key = context
                .eval("var hits=0;({toString(){hits++;throw 99}})")
                .unwrap();
            let entry = entry(&runtime, &mut context, source, vec![Value::Null, key]);
            let profile = CostProfile::start();
            let result = execute(runtime.clone(), entry, ExecutionLimits::default()).unwrap();
            let Completion::Return(value) = result else {
                panic!("read escaped catch");
            };
            assert_eq!(
                value,
                Value::String(crate::engine::value::JsString::from_static(expected))
            );
            let costs = profile.snapshot();
            assert_eq!(costs.owned_bridge_exits, 0);
            assert_eq!(costs.legacy_dispatches, 0);
            assert_eq!(costs.owned_sync_call_bridges, 0);
            drop(profile);
            assert_eq!(context.eval("hits").unwrap(), Value::Int(0));
        }
    }

    #[test]
    fn object_property_keys_resume_string_hint_and_late_method_reads() {
        for (setup, expected_events) in [
            (
                "var events='';var target={marker:42,get x(){events+='G';return this.marker}};var key={get [Symbol.toPrimitive](){events+='M';return function(hint){events+=hint;return 'x'}}}",
                "MstringG",
            ),
            (
                "var events='';var later=function(){throw 99};var target={get x(){events+='G';return 42}};var key={toString(){events+='S';later=function(){events+='V';return 'x'};return this},get valueOf(){events+='M';return later}}",
                "SMVG",
            ),
            (
                "var events='';var target={};Object.defineProperty(target,'x',{get:(function(a,b){events+='G';return this.marker+a+b}).bind({marker:39},1).bind({marker:0},2)});var key={toString:(function(k){events+=this.marker;return k}).bind({marker:'B'},'x')}",
                "BG",
            ),
        ] {
            let runtime = Runtime::new();
            let mut context = runtime.new_context();
            context.eval(setup).unwrap();
            let target = context.eval("target").unwrap();
            let key = context.eval("key").unwrap();
            let entry = entry(
                &runtime,
                &mut context,
                "(function(o,k){return o[k]})",
                vec![target, key],
            );
            let profile = CostProfile::start();
            assert!(matches!(
                execute(runtime.clone(), entry, ExecutionLimits::default()).unwrap(),
                Completion::Return(Value::Int(42))
            ));
            let costs = profile.snapshot();
            assert_eq!(costs.owned_storage.maximum_frame_depth, 2);
            assert_eq!(costs.legacy_dispatches, 0, "{setup}");
            assert_eq!(costs.owned_bridge_exits, 0, "{setup}");
            assert_eq!(costs.owned_sync_call_bridges, 0, "{setup}");
            drop(profile);
            assert_eq!(
                context.eval("events").unwrap(),
                Value::String(crate::engine::value::JsString::from_static(expected_events))
            );
        }
    }

    #[test]
    fn converted_property_keys_keep_the_evaluated_base_and_throw_identity() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let key = context.eval("var current={x:42};var replacement={x:99};({toString(){current=replacement;return 'x'}})").unwrap();
        let initial_entry = entry(
            &runtime,
            &mut context,
            "(function(k){return current[k]})",
            vec![key],
        );
        let profile = CostProfile::start();
        assert!(matches!(
            execute(runtime.clone(), initial_entry, ExecutionLimits::default()).unwrap(),
            Completion::Return(Value::Int(42))
        ));
        assert_eq!(profile.snapshot().owned_bridge_exits, 0);
        assert_eq!(profile.snapshot().owned_sync_call_bridges, 0);
        drop(profile);
        assert_eq!(context.eval("current.x").unwrap(), Value::Int(99));
        for key_source in [
            "({get toString(){hits++;throw token}})",
            "({toString(){hits++;throw token}})",
        ] {
            let token = context.eval("var hits=0;var token={};token").unwrap();
            let key = context.eval(key_source).unwrap();
            let target = context.eval("({get x(){throw 98}})").unwrap();
            let entry = entry(
                &runtime,
                &mut context,
                "(function(o,k){return o[k]})",
                vec![target, key],
            );
            let profile = CostProfile::start();
            let Completion::Throw(value) =
                execute(runtime.clone(), entry, ExecutionLimits::default()).unwrap()
            else {
                panic!("key did not throw");
            };
            assert_eq!(value, token);
            assert_eq!(profile.snapshot().owned_bridge_exits, 0);
            assert_eq!(profile.snapshot().owned_sync_call_bridges, 0);
            drop(profile);
            assert_eq!(context.eval("hits").unwrap(), Value::Int(1));
        }
    }

    #[test]
    fn converted_key_crosses_only_the_unresolved_property_step() {
        for target_source in [
            "new Proxy({x:42},{get(t,k,r){traps++;return t[k]}})",
            "Object.defineProperty({},'x',{get:Number.prototype.valueOf.bind(42)})",
        ] {
            let runtime = Runtime::new();
            let mut context = runtime.new_context();
            let key = context
                .eval("var keys=0,traps=0;({toString(){keys++;return 'x'}})")
                .unwrap();
            let target = context.eval(target_source).unwrap();
            let entry = entry(
                &runtime,
                &mut context,
                "(function(o,k){return o[k]})",
                vec![target, key],
            );
            let profile = CostProfile::start();
            assert!(matches!(
                execute(runtime.clone(), entry, ExecutionLimits::default()).unwrap(),
                Completion::Return(Value::Int(42))
            ));
            let costs = profile.snapshot();
            assert_eq!(costs.legacy_dispatches, 0);
            assert_eq!(costs.owned_bridge_exits, 0);
            assert_eq!(costs.owned_sync_call_bridges, 1);
            drop(profile);
            assert_eq!(context.eval("keys").unwrap(), Value::Int(1));
            assert_eq!(
                context.eval("traps").unwrap(),
                Value::Int(i32::from(target_source.starts_with("new Proxy")))
            );
        }
    }

    #[test]
    fn primitive_property_receivers_and_string_units_use_owned_reads() {
        for (setup, source, input, expected) in [
            (
                "Object.defineProperty(Number.prototype,'x',{get:function(){'use strict';return this===42}})",
                "(function(o){return o.x})",
                Value::Int(42),
                Value::Bool(true),
            ),
            (
                "Object.defineProperty(String.prototype,'x',{get:function(){'use strict';return this==='hi'}})",
                "(function(o){return o.x})",
                Value::String(crate::engine::value::JsString::from_static("hi")),
                Value::Bool(true),
            ),
        ] {
            let runtime = Runtime::new();
            let mut context = runtime.new_context();
            context.eval(setup).unwrap();
            let entry = entry(&runtime, &mut context, source, vec![input]);
            let profile = CostProfile::start();
            let Completion::Return(value) =
                execute(runtime.clone(), entry, ExecutionLimits::default()).unwrap()
            else {
                panic!("primitive read threw");
            };
            assert_eq!(value, expected);
            assert_eq!(profile.snapshot().legacy_dispatches, 0);
            assert_eq!(profile.snapshot().owned_bridge_exits, 0);
            assert_eq!(profile.snapshot().owned_sync_call_bridges, 0);
        }
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let entry = entry(
            &runtime,
            &mut context,
            "(function(s){return s[0]+':'+s.length})",
            vec![Value::String(crate::engine::value::JsString::from_static(
                "hi",
            ))],
        );
        let profile = CostProfile::start();
        assert!(
            matches!(execute(runtime.clone(), entry, ExecutionLimits::default()).unwrap(), Completion::Return(Value::String(value)) if value==crate::engine::value::JsString::from_static("h:2"))
        );
        assert_eq!(profile.snapshot().owned_storage.frames_pushed, 1);
        assert_eq!(profile.snapshot().legacy_dispatches, 0);
        assert_eq!(profile.snapshot().owned_bridge_exits, 0);
        assert_eq!(profile.snapshot().owned_sync_call_bridges, 0);
    }

    #[test]
    fn object_key_updates_keep_one_conversion_across_the_remaining_set_bridge() {
        for selected in ["0", "Symbol.iterator"] {
            let runtime = Runtime::new();
            let mut context = runtime.new_context();
            context.eval(&format!("var keys=0,getters=0,written=0;var selected={selected};var target={{}};Object.defineProperty(target,selected,{{get:function(){{getters++;return 41}},set:function(v){{written=v}}}});var key={{toString(){{keys++;return selected}}}};")).unwrap();
            let target = context.eval("target").unwrap();
            let key = context.eval("key").unwrap();
            let entry = entry(
                &runtime,
                &mut context,
                "(function(o,k){return o[k]++})",
                vec![target, key],
            );
            let profile = CostProfile::start();
            assert!(matches!(
                execute(runtime.clone(), entry, ExecutionLimits::default()).unwrap(),
                Completion::Return(Value::Int(41))
            ));
            assert_eq!(profile.snapshot().owned_storage.maximum_frame_depth, 2);
            assert_eq!(profile.snapshot().owned_bridge_exits, 1);
            assert_eq!(profile.snapshot().owned_sync_call_bridges, 0);
            drop(profile);
            assert_eq!(
                context.eval("keys*100+getters*10+written").unwrap(),
                Value::Int(152)
            );
        }
    }

    #[test]
    fn object_key_conversion_failure_uses_the_reading_realm() {
        let runtime = Runtime::new();
        let mut caller = runtime.new_context();
        let mut foreign = runtime.new_context();
        let Value::Object(expected) = caller.eval("TypeError.prototype").unwrap() else {
            panic!("missing error prototype");
        };
        let key = foreign
            .eval("({toString(){return {}},valueOf(){return {}}})")
            .unwrap();
        let target = caller.eval("({})").unwrap();
        let entry = entry(
            &runtime,
            &mut caller,
            "(function(o,k){return o[k]})",
            vec![target, key],
        );
        let profile = CostProfile::start();
        let Completion::Throw(Value::Object(error)) =
            execute(runtime.clone(), entry, ExecutionLimits::default()).unwrap()
        else {
            panic!("key unexpectedly succeeded");
        };
        assert_eq!(profile.snapshot().legacy_dispatches, 0);
        assert_eq!(profile.snapshot().owned_bridge_exits, 0);
        assert_eq!(profile.snapshot().owned_sync_call_bridges, 0);
        drop(profile);
        assert_eq!(runtime.get_prototype_of(&error).unwrap(), Some(expected));
        assert_eq!(
            caller
                .get_property(&error, &runtime.intern_property_key("message").unwrap())
                .unwrap(),
            Value::String(crate::engine::value::JsString::from_static("toPrimitive"))
        );
        assert!(runtime.0.state.borrow().active_frames.is_empty());
    }

    #[test]
    fn ordinary_getter_resumes_in_the_same_execution() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let object = context
            .eval("Object.create({get x(){return this.y}}, {y:{value:41}})")
            .unwrap();
        let entry = entry(
            &runtime,
            &mut context,
            "(function root(o){return o.x+1})",
            vec![object],
        );
        let profile = CostProfile::start();
        let result = execute(runtime.clone(), entry, ExecutionLimits::default()).unwrap();
        assert!(matches!(result, Completion::Return(Value::Int(42))));
        assert_eq!(profile.snapshot().owned_storage.maximum_frame_depth, 2);
        assert_eq!(profile.snapshot().owned_bridge_exits, 0);
        assert_eq!(profile.snapshot().legacy_dispatches, 0);
        assert!(runtime.0.state.borrow().active_frames.is_empty());
    }

    #[test]
    fn getter_method_call_keeps_the_original_receiver() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let object = context
            .eval("({y:52, fn:function(){return this.y}, get m(){return this.fn}})")
            .unwrap();
        let entry = entry(
            &runtime,
            &mut context,
            "(function root(o){return o.m()})",
            vec![object],
        );
        let profile = CostProfile::start();
        let result = execute(runtime.clone(), entry, ExecutionLimits::default()).unwrap();
        assert!(matches!(result, Completion::Return(Value::Int(52))));
        assert_eq!(profile.snapshot().owned_storage.frames_pushed, 3);
        assert_eq!(profile.snapshot().owned_bridge_exits, 0);
        assert_eq!(profile.snapshot().legacy_dispatches, 0);
        assert!(runtime.0.state.borrow().active_frames.is_empty());
    }

    #[test]
    fn plus_conversion_getter_and_nested_value_of_use_explicit_replies() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let object = context.eval("({get valueOf(){return this.method}, method:function(){return +this.inner}, inner:{valueOf:function(){return 41}}})").unwrap();
        let entry = entry(
            &runtime,
            &mut context,
            "(function root(o){return +o+1})",
            vec![object],
        );
        let profile = CostProfile::start();
        let result = execute(runtime.clone(), entry, ExecutionLimits::default()).unwrap();
        assert!(matches!(result, Completion::Return(Value::Int(42))));
        assert_eq!(profile.snapshot().owned_storage.maximum_frame_depth, 3);
        assert_eq!(profile.snapshot().owned_storage.frames_pushed, 4);
        assert_eq!(profile.snapshot().legacy_dispatches, 0);
        assert_eq!(profile.snapshot().owned_bridge_exits, 0);
        assert!(runtime.0.state.borrow().active_frames.is_empty());
    }

    #[test]
    fn plus_conversion_preserves_callback_numeric_tags_and_bigint_diagnostic() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let Value::Object(factory) = context
            .eval("(function(v){return {valueOf:function(){return v}}})")
            .unwrap()
        else {
            panic!("expected factory");
        };
        let factory = runtime.as_callable(&factory).unwrap().unwrap();
        for value in [
            Value::Float(42.0),
            Value::Float(-0.0),
            Value::Float(f64::from_bits(0x7ff8_0000_0000_0042)),
            Value::BigInt(crate::engine::value::bigint::JsBigInt::from(1)),
        ] {
            let object = context
                .call(&factory, Value::Undefined, std::slice::from_ref(&value))
                .unwrap();
            let entry = entry(
                &runtime,
                &mut context,
                "(function root(o){return +o})",
                vec![object],
            );
            let profile = CostProfile::start();
            let completion = execute(runtime.clone(), entry, ExecutionLimits::default()).unwrap();
            let costs = profile.snapshot();
            assert_eq!(costs.owned_storage.frames_pushed, 2);
            assert_eq!(costs.legacy_dispatches, 0);
            assert_eq!(costs.owned_bridge_exits, 0);
            drop(profile);
            match (value, completion) {
                (Value::Float(expected), Completion::Return(Value::Float(actual))) => {
                    assert_eq!(actual.to_bits(), expected.to_bits());
                }
                (Value::BigInt(_), Completion::Throw(Value::Object(error))) => {
                    for (key, expected) in [
                        ("name", "TypeError"),
                        ("message", "bigint argument with unary +"),
                    ] {
                        assert_eq!(
                            context
                                .get_property(&error, &runtime.intern_property_key(key).unwrap())
                                .unwrap(),
                            Value::String(crate::engine::value::JsString::from_static(expected))
                        );
                    }
                }
                _ => panic!("unexpected unary plus completion"),
            }
            assert!(runtime.0.state.borrow().active_frames.is_empty());
        }
    }

    #[test]
    fn conversion_child_handoff_does_not_repeat_getter_or_method() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let object = context
            .eval("var hits=0; ({get valueOf(){hits++;return function(){hits++;return 5}}})")
            .unwrap();
        let entry = entry(
            &runtime,
            &mut context,
            "(function root(o){return +o})",
            vec![object],
        );
        let result = execute(runtime.clone(), entry, ExecutionLimits::default()).unwrap();
        assert!(matches!(result, Completion::Return(Value::Int(5))));
        assert_eq!(context.eval("hits").unwrap(), Value::Int(2));
        assert!(runtime.0.state.borrow().active_frames.is_empty());
    }

    #[test]
    fn conversion_getter_and_method_throws_keep_identity_and_run_once() {
        for source in [
            "({get valueOf(){hits++;throw token}})",
            "({valueOf:function(){hits++;throw token}})",
        ] {
            let runtime = Runtime::new();
            let mut context = runtime.new_context();
            let token = context.eval("var hits=0; var token={}; token").unwrap();
            let object = context.eval(source).unwrap();
            let entry = entry(
                &runtime,
                &mut context,
                "(function root(o){return +o})",
                vec![object],
            );
            let result = execute(runtime.clone(), entry, ExecutionLimits::default()).unwrap();
            let Completion::Throw(value) = result else {
                panic!("expected throw")
            };
            assert_eq!(value, token);
            assert_eq!(context.eval("hits").unwrap(), Value::Int(1));
            assert!(runtime.0.state.borrow().active_frames.is_empty());
        }
    }

    #[test]
    fn addition_keeps_evaluated_left_when_right_reassigns_parameter() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let object = context.eval("({valueOf:function(){return 40}})").unwrap();
        let entry = entry(
            &runtime,
            &mut context,
            "(function root(x){return x+(x=2)})",
            vec![object],
        );
        let profile = CostProfile::start();
        let result = execute(runtime.clone(), entry, ExecutionLimits::default()).unwrap();
        assert!(matches!(result, Completion::Return(Value::Int(42))));
        assert_eq!(profile.snapshot().legacy_dispatches, 0);
        assert_eq!(profile.snapshot().owned_bridge_exits, 0);
        assert!(runtime.0.state.borrow().active_frames.is_empty());
    }

    #[test]
    fn addition_reads_right_conversion_after_left_callback() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let right = context
            .eval("var log='';var right={valueOf:function(){return 2}};right")
            .unwrap();
        let left = context.eval("({valueOf:function(){log+='L';right.valueOf=function(){log+='R';return 9};return 1}})").unwrap();
        let entry = entry(
            &runtime,
            &mut context,
            "(function root(a,b){return a+b})",
            vec![left, right],
        );
        let result = execute(runtime.clone(), entry, ExecutionLimits::default()).unwrap();
        assert!(matches!(result, Completion::Return(Value::Int(10))));
        assert_eq!(
            context.eval("log").unwrap(),
            Value::String(crate::engine::value::JsString::from_static("LR"))
        );
        assert!(runtime.0.state.borrow().active_frames.is_empty());
    }

    #[test]
    fn constructor_primitive_and_object_returns_use_explicit_child_frames() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let constructor = context.eval("(function C(x){return x})").unwrap();
        for argument in [Value::Int(7), context.eval("({marker:42})").unwrap()] {
            let expected = argument.clone();
            let entry = entry(
                &runtime,
                &mut context,
                "(function root(C,x){return new C(x)})",
                vec![constructor.clone(), argument],
            );
            let profile = CostProfile::start();
            let result = execute(runtime.clone(), entry, ExecutionLimits::default()).unwrap();
            let Completion::Return(Value::Object(object)) = result else {
                panic!("expected constructed object")
            };
            if matches!(expected, Value::Object(_)) {
                assert_eq!(Value::Object(object), expected);
            }
            assert_eq!(profile.snapshot().owned_storage.maximum_frame_depth, 2);
            assert_eq!(profile.snapshot().legacy_dispatches, 0);
            assert_eq!(profile.snapshot().owned_bridge_exits, 0);
            assert!(runtime.0.state.borrow().active_frames.is_empty());
        }
    }

    #[test]
    fn bound_constructor_retargets_new_target_to_original_function() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let original = context
            .eval("var C=function(){return new.target};C")
            .unwrap();
        let bound = context.eval("C.bind(null,1).bind(null,2)").unwrap();
        let entry = entry(
            &runtime,
            &mut context,
            "(function root(C){return new C()})",
            vec![bound],
        );
        let profile = CostProfile::start();
        let result = execute(runtime.clone(), entry, ExecutionLimits::default()).unwrap();
        let Completion::Return(value) = result else {
            panic!("expected return")
        };
        assert_eq!(value, original);
        assert_eq!(profile.snapshot().owned_storage.maximum_frame_depth, 2);
        assert_eq!(profile.snapshot().legacy_dispatches, 0);
        assert!(runtime.0.state.borrow().active_frames.is_empty());
    }

    #[test]
    fn derived_return_without_super_preserves_object_and_rejects_primitives() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let constructor = context
            .eval("(class D extends null {constructor(x){return x}})")
            .unwrap();
        for (value, expected_error) in [
            (context.eval("({marker:42})").unwrap(), None),
            (Value::Undefined, Some("ReferenceError")),
            (Value::Int(1), Some("TypeError")),
        ] {
            let expected = value.clone();
            let entry = entry(
                &runtime,
                &mut context,
                "(function root(C,x){return new C(x)})",
                vec![constructor.clone(), value],
            );
            let profile = CostProfile::start();
            let completion = execute(runtime.clone(), entry, ExecutionLimits::default()).unwrap();
            let costs = profile.snapshot();
            assert_eq!(costs.legacy_dispatches, 0, "{costs:?}");
            assert_eq!(costs.owned_bridge_exits, 0, "{costs:?}");
            match (completion, expected_error) {
                (Completion::Return(value), None) => assert_eq!(value, expected),
                (Completion::Throw(Value::Object(error)), Some(name)) => {
                    assert_eq!(
                        context
                            .get_property(&error, &runtime.intern_property_key("name").unwrap())
                            .unwrap(),
                        Value::String(crate::engine::value::JsString::from_static(name))
                    );
                }
                _ => panic!("unexpected derived completion"),
            }
            assert!(runtime.0.state.borrow().active_frames.is_empty());
        }
    }

    #[test]
    fn explicit_super_initializes_this_once_after_calling_the_base() {
        for (source, frames, throws) in [
            (
                "(class D extends (function B(){}) {constructor(){super()}})",
                3,
                false,
            ),
            (
                "(class D extends (function B(){}) {constructor(){super();super()}})",
                4,
                true,
            ),
        ] {
            let runtime = Runtime::new();
            let mut context = runtime.new_context();
            let constructor = context.eval(source).unwrap();
            let entry = entry(
                &runtime,
                &mut context,
                "(function root(C){return new C()})",
                vec![constructor],
            );
            let profile = CostProfile::start();
            let completion = execute(runtime.clone(), entry, ExecutionLimits::default()).unwrap();
            let costs = profile.snapshot();
            assert_eq!(costs.owned_storage.frames_pushed, frames, "{costs:?}");
            assert_eq!(costs.legacy_dispatches, 0, "{costs:?}");
            assert_eq!(costs.owned_bridge_exits, 0, "{costs:?}");
            match (completion, throws) {
                (Completion::Return(Value::Object(_)), false) => {}
                (Completion::Throw(Value::Object(error)), true) => {
                    assert_eq!(
                        context
                            .get_property(&error, &runtime.intern_property_key("message").unwrap())
                            .unwrap(),
                        Value::String(crate::engine::value::JsString::from_static(
                            "'this' can be initialized only once"
                        ))
                    );
                }
                _ => panic!("unexpected super completion"),
            }
            assert!(runtime.0.state.borrow().active_frames.is_empty());
        }
    }

    #[test]
    fn default_derived_forwards_actual_arguments_and_live_super_new_target() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let constructor = context
            .eval("var D=class extends (function B(a,b,c){return c}) {}; D")
            .unwrap();
        let marker = context.eval("({marker:42})").unwrap();
        let call_entry = entry(
            &runtime,
            &mut context,
            "(function root(C,x){return new C(1,2,x)})",
            vec![constructor.clone(), marker.clone()],
        );
        let profile = CostProfile::start();
        let result = execute(runtime.clone(), call_entry, ExecutionLimits::default()).unwrap();
        let Completion::Return(value) = result else {
            panic!("expected return")
        };
        assert_eq!(value, marker);
        assert_eq!(profile.snapshot().owned_storage.maximum_frame_depth, 3);
        assert_eq!(profile.snapshot().legacy_dispatches, 0);
        drop(profile);
        context
            .eval("Object.setPrototypeOf(D,function Replacement(){return new.target})")
            .unwrap();
        let call_entry = entry(
            &runtime,
            &mut context,
            "(function root(C){return new C()})",
            vec![constructor.clone()],
        );
        let profile = CostProfile::start();
        let result = execute(runtime.clone(), call_entry, ExecutionLimits::default()).unwrap();
        let Completion::Return(value) = result else {
            panic!("expected return")
        };
        assert_eq!(value, constructor);
        assert_eq!(profile.snapshot().owned_storage.maximum_frame_depth, 3);
        assert_eq!(profile.snapshot().legacy_dispatches, 0);
        assert_eq!(profile.snapshot().owned_bridge_exits, 0);
        assert!(runtime.0.state.borrow().active_frames.is_empty());
    }

    #[test]
    fn direct_child_call_preserves_foreign_argument_rejection() {
        let runtime = Runtime::new();
        let foreign = Runtime::new();
        let mut context = runtime.new_context();
        let callee = context.eval("(function ignore(x){return 1})").unwrap();
        let object = foreign.new_object(None).unwrap();
        let entry = entry(
            &runtime,
            &mut context,
            "(function root(f,x){return f(x)})",
            vec![callee, Value::Object(object.clone())],
        );
        let result = execute(runtime.clone(), entry, ExecutionLimits::default());
        let Err(error) = result else {
            panic!("expected domain rejection, got {result:?}");
        };
        assert!(error.to_string().contains("call argument"), "{error}");
        assert!(runtime.0.state.borrow().active_frames.is_empty());
        assert!(
            foreign
                .0
                .state
                .borrow()
                .heap
                .object(object.object_id())
                .is_ok()
        );
    }
}
