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
                if !crate::engine::vm::frame_operations::complete_owned_slot(execution, id, exit)? {
                    return Err(Error::internal(
                        "direct slot completion changed its frame protocol",
                    ));
                }
            }

            RunExit::Numeric(kind) => {
                use crate::engine::vm::frame_operations::NumericProgress;
                let Some(progress) =
                    crate::engine::vm::frame_operations::try_complete_primitive_numeric(
                        runtime, execution, id, kind,
                    )?
                else {
                    return Ok(Boundary::Exit(exit));
                };
                match progress {
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
                use crate::engine::vm::conversion_driver::PrimitiveCompletion;
                match crate::engine::vm::conversion_driver::complete_primitives(
                    runtime,
                    execution,
                    id,
                    addition,
                    next_operation,
                )? {
                    PrimitiveCompletion::Completed => {}
                    PrimitiveCompletion::Throw(value) => {
                        return Ok(Boundary::Complete(Completion::Throw(value)));
                    }
                    PrimitiveCompletion::InvalidDomain => {
                        return Ok(Boundary::Exit(RunExit::Bridge));
                    }
                    PrimitiveCompletion::Declined => return Ok(Boundary::Conversion(exit)),
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
