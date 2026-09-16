//! Complete and destroy a frame without a second execution core.
use super::{
    Completion,
    exception::runtime_error_to_vm_error,
    execution::RunningExecution,
    frame::{ConstructorReturn, FrameId, ReturnTarget},
    run::RunExit,
};
use crate::engine::{
    api::{Error, runtime::Runtime},
    value::Value,
};

pub(super) struct FrameExit {
    pub completion: Completion,
    pub return_to: Option<ReturnTarget>,
}

#[inline(never)]
pub(super) fn finish(
    _runtime: &Runtime,
    execution: &mut RunningExecution,
    id: FrameId,
    exit: RunExit,
    forwarded: Option<Completion>,
) -> Result<FrameExit, Error> {
    if exit != RunExit::Complete {
        return Err(Error::internal("driver did not handle a run exit"));
    }
    let mut frame = execution.frames.pop(id)?;
    let return_to = frame.cold.return_to;
    let constructor_return = frame.cold.constructor_return.take();
    let guard = frame.cold.entry_guard.take();
    let completion = match forwarded {
        Some(completion) => completion,
        None => execution
            .pending
            .take()
            .map(Completion::Return)
            .ok_or_else(|| Error::internal("owned completion has no payload"))?,
    };
    execution.slots.clear_frame(frame.window.take())?;
    if let Some(guard) = guard {
        guard.finish().map_err(runtime_error_to_vm_error)?;
    }
    execution.call_storage.recycle(frame.cold);
    let completion = match (completion, constructor_return) {
        (Completion::Return(value), Some(ConstructorReturn::Base(receiver))) => {
            Completion::Return(if matches!(value, Value::Object(_)) {
                value
            } else {
                receiver
            })
        }
        (Completion::Return(value), Some(ConstructorReturn::Derived)) => {
            if !matches!(value, Value::Object(_)) {
                return Err(Error::internal(
                    "derived constructor bytecode returned an unvalidated primitive",
                ));
            }
            Completion::Return(value)
        }
        (completion, _) => completion,
    };
    Ok(FrameExit {
        completion,
        return_to,
    })
}
