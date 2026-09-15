//! The ordinary call/return loop's only entry and destruction transactions.
use crate::engine::{
    api::{Error, runtime::Runtime},
    vm::{
        call::ordinary::DirectSelection,
        exception::runtime_error_to_vm_error,
        execution::RunningExecution,
        frame::{FrameId, ReturnValue},
    },
};

pub(super) enum Entry {
    Ordinary,
    Native(super::CallStep),
    General,
}

pub(super) fn enter(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    id: FrameId,
    count: u16,
    method: bool,
    tail: bool,
) -> Result<Entry, Error> {
    let frame = execution.frames.current_mut(id)?;
    let count = usize::from(count);
    execution
        .slots
        .peek(&frame.window, count + usize::from(method))?;
    enum Prepared {
        Ordinary(crate::engine::vm::call::ordinary::OrdinaryCall),
        Native(
            crate::engine::object::CallableRef,
            crate::engine::vm::frames::NativeClassification,
        ),
    }
    // End every Result/selection container holding a slot borrow before any
    // frame installation or operand transfer. Only owning facts leave here.
    let prepared = {
        let selection_result =
            DirectSelection::select(runtime, execution.slots.peek(&frame.window, count)?);
        if matches!(selection_result, Ok(DirectSelection::General)) {
            return Ok(Entry::General);
        }
        if !execution
            .slots
            .validate_call_value_domains(&frame.window, runtime, count, method)?
        {
            return Ok(Entry::General);
        }
        let selection = selection_result.map_err(runtime_error_to_vm_error)?;
        match selection {
            DirectSelection::Ordinary(ordinary) => Prepared::Ordinary(
                ordinary
                    .authenticate(runtime)
                    .map_err(runtime_error_to_vm_error)?,
            ),
            DirectSelection::Native(native) => {
                let (callable, selected) =
                    crate::engine::vm::frames::NativeClassification::promote_selected(native);
                Prepared::Native(callable, selected)
            }
            DirectSelection::General => return Ok(Entry::General),
        }
    };
    match prepared {
        Prepared::Ordinary(call) => {
            #[cfg(feature = "profiling")]
            let depth = execution.slots.depth(&frame.window);
            if !execution.frames.can_push() || runtime.bytecode_call_would_overflow() {
                return Ok(Entry::General);
            }
            call.install(runtime, execution, id, count, method, tail)?;
            #[cfg(feature = "profiling")]
            crate::engine::api::profiling::record_owned_instruction(depth);
            Ok(Entry::Ordinary)
        }
        Prepared::Native(callable, mut selected) => {
            let target = selected.target();
            let realm = selected.defining_realm();
            let minimum = selected.minimum();
            let operation = selected.take_operation();
            let depth = execution.slots.depth(&frame.window);
            execution.slots.reserve_native_argument_depth(
                runtime
                    .0
                    .state
                    .borrow()
                    .active_frames
                    .len()
                    .saturating_add(1),
            )?;
            let (arguments, receiver) =
                execution
                    .slots
                    .take_native_call_operands(&mut frame.window, count, method)?;
            super::super::proxy_get_driver::start_native_with_classification(
                runtime,
                execution,
                id,
                callable,
                target,
                realm,
                minimum,
                receiver,
                arguments,
                tail,
                depth,
                Some(selected),
                operation,
            )
            .map(Entry::Native)
        }
    }
}

pub(super) fn finish(execution: &mut RunningExecution, id: FrameId) -> Result<bool, Error> {
    let frame = execution.frames.current_mut(id)?;
    let Some(target) = frame.cold.ordinary_return() else {
        return Ok(false);
    };
    if execution.pending.is_none() {
        return Ok(false);
    }
    // Result ownership precedes window clearing and activation removal.
    let value = execution.pending.take().unwrap();
    let mut frame = execution.frames.pop(id)?;
    let guard = frame.cold.entry_guard.take();
    execution.slots.clear_frame(frame.window)?;
    if let Some(guard) = guard {
        guard.finish().map_err(runtime_error_to_vm_error)?;
    }
    execution.call_storage.recycle(frame.cold);
    let parent = execution.frames.current_mut(target.frame()?)?;
    if matches!(target.value_use, ReturnValue::Push) {
        execution.slots.push(&mut parent.window, value)?;
    }
    #[cfg(feature = "profiling")]
    crate::engine::api::profiling::record_owned_execution_event("ordinary_return_direct");
    Ok(true)
}

#[cfg(test)]
mod layout_tests {
    #[test]
    fn unified_call_entry_keeps_the_ordinary_result_abi_size() {
        // Error already determines the old result's size. Adding the native
        // completion must not enlarge every ordinary Call return transaction.
        assert_eq!(
            std::mem::size_of::<Result<super::Entry, super::Error>>(),
            std::mem::size_of::<Result<bool, super::Error>>(),
        );
    }
}
