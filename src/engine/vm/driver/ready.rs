//! Reenter the same frame after cold operations which cannot wait or call JS.
//! Opcode semantics remain in run and the shared cold completion helpers.
use super::{CallStep, RunExit};
use crate::engine::api::error::Error;
use crate::engine::api::runtime::Runtime;
use crate::engine::vm::BytecodePc;
use crate::engine::vm::Completion;
use crate::engine::vm::exception::runtime_error_to_vm_error;
use crate::engine::vm::execution::RunningExecution;
use crate::engine::vm::frame::FrameId;

pub(super) enum Boundary {
    Exit(RunExit),
    /// An existing property/query helper scheduled work; revisit the outer
    /// driver's pending-call/frame checks instead of assuming this frame ran.
    Entered,
    /// Operand domains were checked and next_operation was advanced once.
    /// The outer driver constructs the waiting task without repeating either.
    Conversion(RunExit),
    Complete(Completion),
}

#[inline(never)]
pub(super) fn run(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    mut id: FrameId,
    next_operation: &mut u64,
) -> Result<Boundary, Error> {
    loop {
        let result = super::run(execution, id);
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_execution_event(match &result {
            Ok(exit) => exit.diagnostic_name(),
            Err(_) => "run_exit.EngineError",
        });
        // run has dropped RunSlots and materialized the exact frame PCs even
        // on error. Publish before any cold allocation, release or JS error.
        let frame = execution.frames.current_mut(id)?;
        runtime
            .update_active_bytecode_pc(frame.cold.active_frame, BytecodePc::new(frame.fault_pc))
            .map_err(runtime_error_to_vm_error)?;
        let exit = result?;
        match exit {
            RunExit::Call {
                arguments,
                method,
                tail,
            } => {
                if !super::ordinary::enter(runtime, execution, id, arguments, method, tail)? {
                    return Ok(Boundary::Exit(exit));
                }
                id = execution.frames.current_id().unwrap();
            }
            RunExit::Complete => {
                if !super::ordinary::finish(execution, id)? {
                    return Ok(Boundary::Exit(exit));
                }
                id = execution.frames.current_id().unwrap();
            }
            RunExit::ReplaceBinding { .. } | RunExit::ReleaseOperand { .. } => {
                match crate::engine::vm::frame_operations::complete_owned_slot(execution, id, exit)?
                {
                    Some(CallStep::Entered) => {}
                    _ => {
                        return Err(Error::internal(
                            "direct slot completion changed its frame protocol",
                        ));
                    }
                }
            }
            RunExit::Numeric(kind) => {
                let frame = execution.frames.current_mut(id)?;
                // No operand is moved before this guard. Objects keep the
                // outer callback path and cannot replay a partial conversion.
                for offset in 0..if kind.unary() { 1 } else { 2 } {
                    if matches!(
                        execution.slots.peek(&frame.window, offset),
                        Err(_) | Ok(crate::engine::value::Value::Object(_))
                    ) {
                        return Ok(Boundary::Exit(exit));
                    }
                }
                use crate::engine::vm::frame_operations::NumericProgress;
                match crate::engine::vm::frame_operations::complete_numeric(
                    runtime, execution, id, kind,
                )? {
                    NumericProgress::Completed => {
                        #[cfg(feature = "profiling")]
                        crate::engine::api::profiling::record_owned_execution_event(
                            "numeric_completed_in_same_frame",
                        );
                    }
                    NumericProgress::Deferred(CallStep::Entered) => return Ok(Boundary::Entered),
                    NumericProgress::Deferred(CallStep::Complete(completion)) => {
                        return Ok(Boundary::Complete(completion));
                    }
                    NumericProgress::Deferred(CallStep::Bridge) => {
                        return Err(Error::internal("numeric operation attempted replay"));
                    }
                }
            }
            RunExit::ConvertPlus | RunExit::ConvertAdd => {
                let addition = exit == RunExit::ConvertAdd;
                let frame = execution.frames.current_mut(id)?;
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
                    return Ok(Boundary::Exit(RunExit::Bridge));
                }
                *next_operation = next_operation
                    .checked_add(1)
                    .ok_or_else(|| Error::internal("conversion identity exhausted"))?;
                match crate::engine::vm::conversion_driver::complete_primitives(
                    runtime, execution, id, addition,
                )? {
                    Some(CallStep::Entered) => {}
                    Some(CallStep::Complete(completion)) => {
                        return Ok(Boundary::Complete(completion));
                    }
                    Some(CallStep::Bridge) => {
                        return Err(Error::internal("primitive conversion attempted replay"));
                    }
                    None => return Ok(Boundary::Conversion(exit)),
                }
            }
            RunExit::GetField {
                index,
                keep_receiver,
            } => {
                let progress = crate::engine::vm::property_driver::read_progress(
                    runtime,
                    execution,
                    id,
                    crate::engine::vm::property_driver::ReadKey::Static(index),
                    keep_receiver,
                )?;
                if let Some(boundary) = property_boundary(progress) {
                    return Ok(boundary);
                }
            }
            RunExit::GetElement {
                keep_receiver,
                keep_key,
            } => {
                let frame = execution.frames.current_mut(id)?;
                if !matches!(
                    execution.slots.peek(&frame.window, 1)?,
                    crate::engine::value::Value::Null | crate::engine::value::Value::Undefined
                ) && matches!(
                    execution.slots.peek(&frame.window, 0)?,
                    crate::engine::value::Value::Object(_)
                ) {
                    return Ok(Boundary::Exit(exit));
                }
                let progress = crate::engine::vm::property_driver::read_progress(
                    runtime,
                    execution,
                    id,
                    crate::engine::vm::property_driver::ReadKey::Computed { keep_key },
                    keep_receiver,
                )?;
                if let Some(boundary) = property_boundary(progress) {
                    return Ok(boundary);
                }
            }
            RunExit::SetProperty(key) => {
                let frame = execution.frames.current_mut(id)?;
                if key.is_none()
                    && matches!(
                        execution.slots.peek(&frame.window, 1)?,
                        crate::engine::value::Value::Object(_)
                    )
                {
                    return Ok(Boundary::Exit(exit));
                }
                let progress = crate::engine::vm::property_write_driver::write_progress(
                    runtime, execution, id, key,
                )?;
                if let Some(boundary) = property_boundary(progress) {
                    return Ok(boundary);
                }
            }
            _ => return Ok(Boundary::Exit(exit)),
        }
    }
}

fn property_boundary(
    progress: crate::engine::vm::property_driver::PropertyProgress,
) -> Option<Boundary> {
    use crate::engine::vm::property_driver::PropertyProgress;
    match progress {
        PropertyProgress::Completed => None,
        PropertyProgress::Deferred(CallStep::Entered) => Some(Boundary::Entered),
        PropertyProgress::Deferred(CallStep::Complete(completion)) => {
            Some(Boundary::Complete(completion))
        }
        PropertyProgress::Deferred(CallStep::Bridge) => Some(Boundary::Exit(RunExit::Bridge)),
    }
}
