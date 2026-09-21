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
    value::JsValue,
};

pub(super) struct FrameExit {
    pub completion: Completion,
    pub return_to: Option<ReturnTarget>,
}

#[inline(never)]
pub(super) fn finish(
    runtime: &Runtime,
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
    let guard = frame.cold.entry_guard.take();
    let completion = match forwarded {
        Some(completion) => completion,
        None => execution
            .pending
            .take()
            .map(Completion::Return)
            .ok_or_else(|| Error::internal("owned completion has no payload"))?,
    };
    execution.slots.clear_frame(runtime, frame.window.take())?;
    if let Some(mut pending) = frame.cold.iterator_wait.take() {
        super::iterator_driver::release_wait(runtime, &mut pending)?;
    }
    if let Some(guard) = guard {
        guard.finish().map_err(runtime_error_to_vm_error)?;
    }
    let constructor_return = frame.cold.constructor_return.take();
    execution.call_storage.recycle(frame.cold);
    let completion = match (completion, constructor_return) {
        (Completion::Return(value), Some(ConstructorReturn::Base(receiver))) => {
            Completion::Return(if matches!(value, JsValue::Object(_)) {
                runtime
                    .release_jsvalue(receiver)
                    .map_err(runtime_error_to_vm_error)?;
                value
            } else {
                runtime
                    .release_jsvalue(value)
                    .map_err(runtime_error_to_vm_error)?;
                receiver
            })
        }
        (Completion::Return(value), Some(ConstructorReturn::Derived)) => {
            if !matches!(value, JsValue::Object(_)) {
                runtime
                    .release_jsvalue(value)
                    .map_err(runtime_error_to_vm_error)?;
                return Err(Error::internal(
                    "derived constructor bytecode returned an unvalidated primitive",
                ));
            }
            Completion::Return(value)
        }
        (completion, Some(ConstructorReturn::Base(receiver))) => {
            runtime
                .release_jsvalue(receiver)
                .map_err(runtime_error_to_vm_error)?;
            completion
        }
        (completion, _) => completion,
    };
    Ok(FrameExit {
        completion,
        return_to,
    })
}
