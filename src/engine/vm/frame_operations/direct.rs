//! Same-frame slot ownership transitions outside the resident RunSlots borrow.
#[cfg(feature = "profiling")]
use crate::engine::api::error::Error;
#[cfg(feature = "profiling")]
use crate::engine::vm::execution::RunningExecution;
#[cfg(feature = "profiling")]
use crate::engine::vm::frame::FrameId;
#[cfg(feature = "profiling")]
use crate::engine::vm::run::RunExit;

#[cfg(feature = "profiling")]
pub(in crate::engine::vm) fn complete(
    runtime: &crate::engine::api::runtime::Runtime,
    execution: &mut RunningExecution,
    id: FrameId,
    exit: RunExit,
) -> Result<bool, Error> {
    if let RunExit::ReleaseOperand { keep_top } = exit {
        let frame = execution.frames.current_mut(id)?;
        // Hot preflight has not changed an owner. Validate both operands before
        // moving either, then release the removed internal edge.
        #[cfg(feature = "profiling")]
        let depth = execution.slots.depth(&frame.window);
        let released = {
            let mut slots = execution.slots.run_window(&mut frame.window)?;
            slots.peek(usize::from(keep_top))?;
            let kept = if keep_top { Some(slots.pop()?) } else { None };
            let released = slots.pop()?;
            if let Some(kept) = kept {
                slots.push(kept)?;
            }
            released
        };
        // Publish the surviving stack before dropping the last temporary root.
        runtime
            .release_jsvalue(released)
            .map_err(crate::engine::vm::exception::runtime_error_to_vm_error)?;
        frame.resume_pc = frame
            .fault_pc
            .checked_add(1)
            .ok_or_else(|| Error::internal("release resume PC overflow"))?;
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_instruction(depth);
        return Ok(true);
    }
    Ok(false)
}
