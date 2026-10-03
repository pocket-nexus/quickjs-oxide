//! Complete and destroy a frame without a second execution core.
use super::{
    Completion,
    exception::runtime_error_to_vm_error,
    execute::VmAction,
    execution::RunningExecution,
    frame::{ConstructorReturn, FrameId, ReturnTarget},
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
    exit: VmAction,
    forwarded: Option<Completion>,
) -> Result<FrameExit, Error> {
    // Admit the completion into reachable execution storage before any
    // fallible window or frame operation can abandon retirement.
    if let Some(completion) = forwarded {
        debug_assert!(execution.pending_completion.is_none());
        execution.pending_completion = Some(completion);
    }
    if exit != VmAction::Complete {
        return Err(Error::internal("driver did not handle a run exit"));
    }
    if execution.pending_completion.is_none() {
        execution.pending_completion = Some(
            execution
                .pending
                .take()
                .map(Completion::Return)
                .ok_or_else(|| Error::internal("owned completion has no payload"))?,
        );
    }
    let frame = execution.frames.pop(id)?;
    let mut frame = super::frame::RetiredFrame::new(runtime, &mut execution.slots, frame);
    let return_to = frame.cold.return_to;
    frame.clear_window()?;
    if let Some(mut pending) = frame.cold.iterator_wait.take() {
        super::iterator_driver::release_wait(runtime, &mut pending)?;
    }
    // Keep the completion and constructor receiver in their registered
    // stores while normalization can still fail. Only the selected result
    // leaves execution storage after retirement has completed.
    let returns_object = matches!(
        execution.pending_completion,
        Some(Completion::Return(JsValue::Object(_)))
    );
    let returns_value = matches!(execution.pending_completion, Some(Completion::Return(_)));
    if matches!(
        frame.cold.constructor_return,
        Some(ConstructorReturn::Base(_))
    ) {
        if returns_value && !returns_object {
            let Some(Completion::Return(value)) = execution.pending_completion.take() else {
                unreachable!("checked return completion")
            };
            if let Err(error) = runtime.release_jsvalue(value) {
                runtime.0.poisoned.set(true);
                return Err(runtime_error_to_vm_error(error));
            }
            let Some(ConstructorReturn::Base(receiver)) = frame.cold.constructor_return.take()
            else {
                unreachable!("checked base constructor receiver")
            };
            execution.pending_completion = Some(Completion::Return(receiver));
        } else if let Err(error) = frame
            .cold
            .release_constructor_return(&mut runtime.0.state.borrow_mut())
        {
            runtime.0.poisoned.set(true);
            return Err(runtime_error_to_vm_error(error));
        }
    } else if returns_value
        && !returns_object
        && matches!(
            frame.cold.constructor_return,
            Some(ConstructorReturn::Derived)
        )
    {
        let Some(Completion::Return(value)) = execution.pending_completion.take() else {
            unreachable!("checked return completion")
        };
        if let Err(error) = runtime.release_jsvalue(value) {
            runtime.0.poisoned.set(true);
            return Err(runtime_error_to_vm_error(error));
        }
        return Err(Error::internal(
            "derived constructor bytecode returned an unvalidated primitive",
        ));
    }
    frame
        .recycle(&mut execution.call_storage)
        .map_err(runtime_error_to_vm_error)?;
    let completion = execution
        .pending_completion
        .take()
        .expect("retired frame completion");
    Ok(FrameExit {
        completion,
        return_to,
    })
}
