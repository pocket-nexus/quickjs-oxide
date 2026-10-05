//! Shared backing access is a real cold boundary, outside State and the hot
//! ready loop. Its word decoder and guards must not widen numeric dispatch.
use crate::engine::{
    api::{Error, Runtime},
    builtins::SharedTypedRead,
    vm::{
        exception::runtime_error_to_vm_error,
        execute::{FallthroughPc, named_read},
        execution::RunningExecution,
        frame::FrameId,
        stack::FrameExecution,
    },
};

#[cold]
#[inline(never)]
pub(in crate::engine::vm) fn finish(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    id: FrameId,
    read: SharedTypedRead,
    keep_receiver: bool,
    fallthrough: FallthroughPc,
) -> Result<(), Error> {
    // read consumes the backing lock before any State access is obtained.
    let (element, bytes) = read.read().map_err(runtime_error_to_vm_error)?;
    let mut state = runtime.0.state.borrow_mut();
    let mut segment = FrameExecution::admit(execution, id)?;
    named_read::finish_shared(
        runtime,
        &mut state,
        &mut segment,
        element,
        bytes,
        keep_receiver,
        fallthrough,
    )
}
