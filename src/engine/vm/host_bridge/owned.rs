//! Temporary S03 ordinary-call entry and one-way handoff to the previous VM.
//! Only this adapter knows both frame representations. Hot execution owns the
//! executable and all slots; unsupported instructions hand back existing owners
//! at their untouched PC. This bridge is removed after S04–S07 cover its exits.

use super::RuntimeVmHost;
use crate::engine::api::error::Error;
use crate::engine::api::runtime::Runtime;
use crate::engine::value::Value;
use crate::engine::vm::frame::{FrameCold, FrameEntry};
use crate::engine::vm::stack::{FrameStorage, copy_value};
use crate::engine::vm::{CallInput, Completion, VmActivation};

pub(super) fn execute(
    host: RuntimeVmHost,
    input: CallInput,
    original_arguments: &[Value],
) -> Result<crate::engine::vm::driver::RunningExit, Error> {
    let (runtime, entry) = prepare(host, input, original_arguments)?;
    crate::engine::vm::driver::execute(
        runtime,
        entry,
        crate::engine::vm::execution::ExecutionLimits::default(),
    )
}

// Preparation does not remain on the native stack during callback reentry.
#[inline(never)]
fn prepare(
    host: RuntimeVmHost,
    input: CallInput,
    original_arguments: &[Value],
) -> Result<(Runtime, FrameEntry), Error> {
    // Preserve the production entry's dynamic authentication, independently
    // of static publication and of dormant-activation resume validation.
    if host.executable.root().is_none() {
        return Err(Error::internal(
            "unpublished host cannot execute published code",
        ));
    }
    if host.closure_slots.len() != usize::from(host.executable.metadata.closure_count) {
        return Err(Error::internal(
            "function object closure slot count does not match bytecode metadata",
        ));
    }
    if host.actual_argument_count != original_arguments.len() {
        return Err(Error::internal(
            "owned original arguments disagree with call arity",
        ));
    }
    let mut original_snapshot = Vec::new();
    original_snapshot
        .try_reserve_exact(original_arguments.len())
        .map_err(|_| Error::internal("original argument snapshot allocation failed"))?;
    for argument in original_arguments {
        original_snapshot.push(copy_value(argument)?);
    }
    let original_arguments = original_snapshot;
    let RuntimeVmHost {
        runtime,
        active_frame_token,
        current_realm: _,
        caller_realm,
        executable,
        current_function,
        actual_argument_count: _,
        closure_slots,
        arguments,
        locals,
        reusable_captured_locals,
    } = host;
    let function = current_function
        .ok_or_else(|| Error::internal("published frame has no current function"))?;
    let entry = FrameEntry {
        executable,
        cold: Box::new(FrameCold {
            regions: Vec::new(),
            constructor_wait: None,
            class_wait: None,
            has_binding_wait: None,
            iterator_wait: None,
            property_wait: None,
            property_generation: 0,
            iterator_generation: 0,
            eval_arguments: None,
            constructor_return: None,
            conversion: None,
            normalized_this: None,
            return_to: None,
            entry_guard: None,
            caller_realm,
            active_frame: active_frame_token,
            function,
            closure_slots,
            reusable_captured_locals,
            input,
        }),
        storage: FrameStorage {
            original_arguments,
            parameters: arguments,
            locals,
            operands: Vec::new(),
        },
    };
    #[cfg(feature = "profiling")]
    crate::engine::api::profiling::record_owned_call_storage(
        size_of::<FrameCold>(),
        entry.cold.reusable_captured_locals.capacity() * size_of::<bool>(),
        entry.storage.original_arguments.capacity() * size_of::<Value>(),
    );
    Ok((runtime, entry))
}

/// Execute only the detached frame supplied by the driver. This adapter does
/// not own or clear the caller's RunningExecution or any parent window.
pub(in crate::engine::vm) fn execute_frame(
    runtime: Runtime,
    entry: FrameEntry,
    resume_pc: usize,
) -> Result<Completion, Error> {
    let FrameEntry {
        executable,
        cold,
        storage,
    } = entry;
    let FrameCold {
        regions,
        constructor_wait,
        class_wait,
        has_binding_wait,
        iterator_wait,
        iterator_generation: _,
        property_wait,
        property_generation: _,
        eval_arguments,
        constructor_return: _,
        conversion,
        normalized_this,
        return_to: _,
        entry_guard: _,
        caller_realm,
        active_frame,
        function,
        closure_slots,
        reusable_captured_locals,
        input,
    } = *cold;
    if property_wait.is_some()
        || conversion.is_some()
        || constructor_wait.is_some()
        || class_wait.is_some()
        || has_binding_wait.is_some()
        || iterator_wait.is_some()
        || eval_arguments.is_some()
    {
        return Err(Error::internal(
            "pending conversion cannot hand off its frame",
        ));
    }
    let mut activation = VmActivation::new_in_realm(
        executable.frame_layout(),
        caller_realm,
        executable.realm,
        function.clone(),
        input.this_value,
        input.new_target,
        input.callee_global,
    );
    activation.regions = regions;
    activation.normalized_this = normalized_this;
    activation.stack = storage.operands;
    activation.pc = resume_pc;
    let mut host = RuntimeVmHost {
        runtime,
        active_frame_token: active_frame,
        current_realm: executable.realm,
        caller_realm,
        executable: executable,
        current_function: Some(function),
        actual_argument_count: storage.original_arguments.len(),
        closure_slots,
        arguments: storage.parameters,
        locals: storage.locals,
        reusable_captured_locals,
    };
    drop(storage.original_arguments);
    let code = host.executable.code.clone();
    activation.execute(&code, &mut host)
}
