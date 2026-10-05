//! General named Get consumes current State. The warm IC stays in dispatch;
//! only a selected callback or shared backing access leaves this segment.
use super::{
    Error, FallthroughPc, FrameCursor, FrameExecution, FrameTurn, JsValue, Runtime, RuntimeState,
    runtime_error_to_vm_error,
};
use crate::engine::{
    api::{
        error::{NativeErrorKind, NativeErrorMessage},
        runtime_error::RuntimeError,
    },
    heap::runtime::owned_values::OwnedValueGuard,
    object::ReadBoundary,
    vm::property_driver::{OwnedGetterSelection, OwnedSpecialSelection, SelectedNamedRead},
};

pub(super) enum Progress {
    Completed,
    Boundary,
    Throw,
}

#[inline(never)]
pub(super) fn execute(
    runtime: &Runtime,
    state: &mut RuntimeState,
    segment: &mut FrameExecution<'_>,
    index: u32,
    keep_receiver: bool,
    fallthrough: FallthroughPc,
) -> Result<Progress, Error> {
    // A cache-selected getter already owns its exact callee/receiver. Never
    // redo its lookup. The next batch replaces the remaining effect adapter.
    if segment.frame().selected_named_read.is_some() {
        return Ok(Progress::Boundary);
    }
    // Lazy materialization and String/BigInt publication may allocate. Publish
    // fault frames before selection; all input edges remain in their slots.
    segment.materialize_in_state(state)?;
    let FrameTurn {
        executable,
        transaction,
        fault_pc,
        resume_pc,
        selected_named_read,
        selected_native,
        pending,
        ..
    } = segment.frame();
    #[cfg(feature = "profiling")]
    let depth = transaction.operand_depth();
    let atom = executable
        .property_key_atoms
        .as_ref()
        .and_then(|atoms| atoms.get(index as usize))
        .copied()
        .filter(|atom| !atom.is_null())
        .ok_or_else(|| Error::internal("named read has no linked atom"))?;
    let mut cursor = FrameCursor::new(transaction, fault_pc, resume_pc, &runtime.0.poisoned);
    let mut boundary = None;
    let mut native = None;
    let read = cursor.with_slots(|slots| {
        Ok(state.select_value_read_in_state(
            &runtime.0.poisoned,
            executable.realm,
            slots.peek(0)?,
            atom,
            runtime.domain_id(),
            &mut boundary,
            Some(&mut native),
        ))
    })?;
    let value = match read {
        Ok(value) => value,
        Err(RuntimeError::Engine(error))
            if NativeErrorKind::from_javascript_error(error.kind()).is_some() =>
        {
            let message = error
                .native_message()
                .cloned()
                .unwrap_or_else(|| NativeErrorMessage::from_utf8(error.message()));
            let kind = NativeErrorKind::from_javascript_error(error.kind())
                .expect("JavaScript error kind");
            let error = state
                .new_native_error_from_message(&runtime.0.poisoned, executable.realm, kind, message)
                .map_err(runtime_error_to_vm_error)?;
            debug_assert!(pending.is_none());
            *pending = Some(JsValue::Object(error));
            return Ok(Progress::Throw);
        }
        Err(error) => return Err(runtime_error_to_vm_error(error)),
    };
    *selected_native = native;
    match (value, boundary) {
        (Some(value), None) => complete(
            state,
            &runtime.0.poisoned,
            &mut cursor,
            value,
            keep_receiver,
            fallthrough,
        )?,
        (None, Some(ReadBoundary::Absent)) => complete(
            state,
            &runtime.0.poisoned,
            &mut cursor,
            JsValue::Undefined,
            keep_receiver,
            fallthrough,
        )?,
        (None, Some(ReadBoundary::Getter(getter))) => {
            let selected = cursor.with_slots(|slots| {
                OwnedGetterSelection::prepare(state, &runtime.0.poisoned, slots.peek(0)?, getter)
            })?;
            *selected_named_read = Some(SelectedNamedRead::Getter(selected));
            return Ok(Progress::Boundary);
        }
        (None, Some(ReadBoundary::Shared(read))) => {
            *selected_named_read = Some(SelectedNamedRead::Shared(read));
            return Ok(Progress::Boundary);
        }
        (None, Some(ReadBoundary::Special { object, kind })) => {
            let selected = cursor.with_slots(|slots| {
                OwnedSpecialSelection::prepare(
                    state,
                    &runtime.0.poisoned,
                    object,
                    slots.peek(0)?,
                    kind,
                )
            })?;
            *selected_named_read = Some(SelectedNamedRead::Special(selected));
            return Ok(Progress::Boundary);
        }
        _ => {
            return Err(Error::internal(
                "named read returned an inconsistent selection",
            ));
        }
    }
    #[cfg(feature = "profiling")]
    {
        crate::engine::api::profiling::record_owned_instruction(depth);
        crate::engine::api::profiling::record_owned_execution_event("named_read.state_completed");
    }
    state
        .collect_if_requested(&runtime.0.gc_pressure, &runtime.0.poisoned)
        .map_err(runtime_error_to_vm_error)?;
    Ok(Progress::Completed)
}

/// The caller finished the shared word lock before taking this State access.
pub(in crate::engine::vm) fn finish_shared(
    runtime: &Runtime,
    state: &mut RuntimeState,
    segment: &mut FrameExecution<'_>,
    element: crate::engine::builtins::native::TypedArrayElementKind,
    bytes: [u8; 8],
    keep_receiver: bool,
    fallthrough: FallthroughPc,
) -> Result<(), Error> {
    let value = state
        .decode_typed_index(element, bytes)
        .map_err(runtime_error_to_vm_error)?;
    let FrameTurn {
        transaction,
        fault_pc,
        resume_pc,
        ..
    } = segment.frame();
    let mut cursor = FrameCursor::new(transaction, fault_pc, resume_pc, &runtime.0.poisoned);
    complete(
        state,
        &runtime.0.poisoned,
        &mut cursor,
        value,
        keep_receiver,
        fallthrough,
    )
}

fn complete(
    state: &mut RuntimeState,
    poisoned: &std::cell::Cell<bool>,
    cursor: &mut FrameCursor<'_>,
    value: JsValue,
    keep_receiver: bool,
    fallthrough: FallthroughPc,
) -> Result<(), Error> {
    let mut value = OwnedValueGuard::new(state, poisoned, value);
    let (state, value) = value.parts();
    if keep_receiver {
        cursor.advance(fallthrough.index());
        cursor.with_slots(|slots| slots.push_pending(value))?;
    } else {
        let receiver = cursor.move_owned()?;
        let mut receiver = OwnedValueGuard::new(state, poisoned, receiver);
        let (state, receiver) = receiver.parts();
        cursor.advance(fallthrough.index());
        cursor.with_slots(|slots| slots.push_pending(value))?;
        state
            .release_owned_jsvalue(poisoned, receiver.take().expect("read receiver owner"))
            .map_err(runtime_error_to_vm_error)?;
    }
    Ok(())
}
