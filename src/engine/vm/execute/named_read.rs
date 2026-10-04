//! Whole named Get selection and resident completion on the current lease.
use super::{FallthroughPc, FrameCursor};
use crate::engine::{
    api::{Error, RuntimeError, error::ErrorKind, runtime::Runtime},
    heap::runtime::{RuntimeState, owned_values::OwnedValueGuard},
    object::{OwnedRead, ReadStep},
    value::JsValue,
    vm::{
        Completion,
        call::ordinary::{CallbackSelection, RawCallbackGuard, RawCallbackInputs},
        exception::runtime_error_to_vm_error,
        property_driver::SelectedNamedRead,
        stack::{FrameExecution, FrameTurn, NativeInputSource, StateNativeProgress},
    },
};

pub(in crate::engine::vm) enum Progress {
    Completed,
    CyclePublished,
    Entered,
    Boundary,
    NativeBoundary,
    Throw,
}

pub(super) fn complete(
    runtime: &Runtime,
    state: &mut RuntimeState,
    segment: &mut FrameExecution<'_>,
    index: u32,
    keep_receiver: bool,
    fallthrough: FallthroughPc,
) -> Result<Progress, Error> {
    let step = {
        let turn = segment.frame();
        let atom = turn
            .executable
            .property_key_atoms
            .as_ref()
            .and_then(|atoms| atoms.get(index as usize))
            .copied()
            .filter(|atom| !atom.is_null())
            .ok_or_else(|| Error::internal("property read has no linked key"))?;
        match turn.selected_named_read.take() {
            Some(SelectedNamedRead::Getter(read)) => ReadStep::Ready(read.into_owned_read()),
            Some(SelectedNamedRead::Prepared(read) | SelectedNamedRead::SharedReady(read)) => {
                ReadStep::Ready(read)
            }
            Some(SelectedNamedRead::Shared(word)) => ReadStep::Shared(word),
            Some(error @ SelectedNamedRead::LookupError(_)) => {
                *turn.selected_named_read = Some(error);
                return Ok(Progress::Boundary);
            }
            Some(
                read @ (SelectedNamedRead::RawGetter(_) | SelectedNamedRead::OrdinaryOverflow(_)),
            ) => {
                *turn.selected_named_read = Some(read);
                return Ok(Progress::Boundary);
            }
            None => {
                let mut native = None;
                let receiver = turn.transaction.peek(0)?;
                let nullish = matches!(receiver, JsValue::Null | JsValue::Undefined);
                let step = match state.prepare_value_read_in_state(
                    &runtime.0.poisoned,
                    runtime.domain_id(),
                    turn.executable.realm,
                    receiver,
                    atom,
                    Some(&mut native),
                ) {
                    Ok(step) => step,
                    Err(RuntimeError::Engine(error))
                        if nullish && error.kind() == ErrorKind::Type =>
                    {
                        // Error construction observes the published caller PC
                        // after the lease ends. Carry this selected error to
                        // the existing catchable read boundary without lookup.
                        *turn.selected_named_read = Some(SelectedNamedRead::LookupError(error));
                        return Ok(Progress::Boundary);
                    }
                    Err(error) => return Err(runtime_error_to_vm_error(error)),
                };
                *turn.selected_native = native;
                step
            }
        }
    };
    finish_selected(runtime, state, segment, step, keep_receiver, fallthrough)
}

/// Finish the selected owned effect on its actual current receiver window.
/// Key conversion/ownership belongs to the instruction consumer; this body
/// neither selects properties again nor creates a public callee root.
pub(in crate::engine::vm) fn finish_selected(
    runtime: &Runtime,
    state: &mut RuntimeState,
    segment: &mut FrameExecution<'_>,
    step: ReadStep,
    keep_receiver: bool,
    fallthrough: FallthroughPc,
) -> Result<Progress, Error> {
    let (read, cycle_published) = match step {
        ReadStep::Ready(read) => (read, false),
        ReadStep::CyclePublished(read) => (read, true),
        ReadStep::Shared(word) => {
            *segment.frame().selected_named_read = Some(SelectedNamedRead::Shared(word));
            return Ok(Progress::Boundary);
        }
    };
    match read {
        OwnedRead::Complete(value) => {
            publish_complete(
                runtime,
                state,
                segment,
                value.unwrap_or(JsValue::Undefined),
                keep_receiver,
                fallthrough,
            )?;
            #[cfg(feature = "profiling")]
            crate::engine::api::profiling::record_owned_execution_event("core.internal_named_read");
            Ok(if cycle_published {
                Progress::CyclePublished
            } else {
                Progress::Completed
            })
        }
        OwnedRead::Getter { function, receiver } => {
            // The selected record has one checked getter/receiver edge. Keep
            // its original independent preservation role before authentication.
            let mut owner = RawCallbackGuard::new(
                state,
                &runtime.0.poisoned,
                RawCallbackInputs::new(function, receiver, Vec::new()),
            );
            let mut native_reply = None;
            let result = (|| {
                let (state, inputs) = owner.parts();
                inputs.preserved_receiver = Some(
                    state
                        .dup_jsvalue(segment.frame().transaction.peek(0)?)
                        .map_err(runtime_error_to_vm_error)?,
                );
                // Calling user code is an actual observation boundary. Use the one
                // existing virtual-frame registry algorithm under this same lease.
                segment.materialize_in_state(state)?;
                let selected = inputs
                    .select_callback_in_state(runtime, state)
                    .map_err(runtime_error_to_vm_error)?;
                let call = match selected {
                    CallbackSelection::Ordinary(call) => call,
                    CallbackSelection::General => {
                        *segment.frame().selected_named_read =
                            Some(SelectedNamedRead::RawGetter(owner.take()));
                        return Ok(Progress::Boundary);
                    }
                    CallbackSelection::Native(selected) => {
                        let calling_realm = segment.frame().executable.realm;
                        let return_to = segment.named_getter_return_target();
                        let native = segment.enter_selected_native_state(
                            runtime,
                            state,
                            selected,
                            NativeInputSource::Callback {
                                inputs,
                                named_keep_receiver: Some(keep_receiver),
                                calling_realm,
                            },
                            return_to,
                            fallthrough,
                            None,
                        )?;
                        // The native consumer transferred every callback edge
                        // into its activation/query/frame before replying.
                        native_reply = Some(native);
                        return Ok(Progress::Completed);
                    }
                };
                if !segment.can_push_ordinary_callback() || runtime.bytecode_call_would_overflow() {
                    *segment.frame().selected_named_read =
                        Some(SelectedNamedRead::OrdinaryOverflow(Box::new(
                            crate::engine::vm::property_driver::NamedGetterOverflow {
                                call,
                                inputs: owner.take(),
                            },
                        )));
                    return Ok(Progress::Boundary);
                }
                let (state, inputs) = owner.parts();
                inputs
                    .retire_selection(state, &runtime.0.poisoned)
                    .map_err(runtime_error_to_vm_error)?;
                segment.install_named_ordinary_getter(
                    runtime,
                    state,
                    call,
                    inputs,
                    keep_receiver,
                    fallthrough,
                )?;
                owner.retire().map_err(runtime_error_to_vm_error)?;
                #[cfg(feature = "profiling")]
                crate::engine::api::profiling::record_owned_execution_event(
                    "core.internal_named_getter",
                );
                Ok(Progress::Entered)
            })();
            if result.is_err() && !runtime.0.poisoned.get() {
                // A normal rejection must observe retirement's first failure.
                // Drop is reserved for unwind/abandonment; poisoned suffixes
                // never traverse State after a destructive error.
                owner.retire().map_err(runtime_error_to_vm_error)?;
            }
            if let Some(native) = native_reply {
                // This retires an empty source record; the native reply owner
                // already survived canonical activation retirement.
                owner.retire().map_err(runtime_error_to_vm_error)?;
                drop(owner);
                finish_native_reply(runtime, state, segment, native, fallthrough)
            } else {
                result
            }
        }
        read @ OwnedRead::Proxy { .. } => {
            *segment.frame().selected_named_read = Some(SelectedNamedRead::Prepared(read));
            Ok(Progress::Boundary)
        }
    }
}

fn finish_native_reply(
    runtime: &Runtime,
    state: &mut RuntimeState,
    segment: &mut FrameExecution<'_>,
    progress: StateNativeProgress,
    fallthrough: FallthroughPc,
) -> Result<Progress, Error> {
    match progress {
        StateNativeProgress::Complete(Completion::Return(value)) => {
            let FrameTurn {
                transaction,
                fault_pc,
                resume_pc,
                ..
            } = segment.frame();
            let mut cursor =
                FrameCursor::new(transaction, fault_pc, resume_pc, &runtime.0.poisoned);
            cursor.advance(fallthrough.index());
            cursor.commit_owned(state, value)?;
            Ok(Progress::Completed)
        }
        StateNativeProgress::Complete(Completion::Throw(value)) => {
            let FrameTurn {
                transaction,
                fault_pc,
                resume_pc,
                ..
            } = segment.frame();
            let mut cursor =
                FrameCursor::new(transaction, fault_pc, resume_pc, &runtime.0.poisoned);
            // Throw consumes its operand through the common cold handler.
            // Preserve lower slots and the getter's fault PC until unwind.
            cursor.commit_owned(state, value)?;
            Ok(Progress::Throw)
        }
        StateNativeProgress::Entered => Ok(Progress::Entered),
        StateNativeProgress::Boundary => Ok(Progress::NativeBoundary),
    }
}

fn publish_complete(
    runtime: &Runtime,
    state: &mut RuntimeState,
    segment: &mut FrameExecution<'_>,
    value: JsValue,
    keep_receiver: bool,
    fallthrough: FallthroughPc,
) -> Result<(), Error> {
    let FrameTurn {
        transaction,
        fault_pc,
        resume_pc,
        ..
    } = segment.frame();
    #[cfg(feature = "profiling")]
    let depth = transaction.operand_depth();
    let mut cursor = FrameCursor::new(transaction, fault_pc, resume_pc, &runtime.0.poisoned);
    if keep_receiver {
        // The original base already occupies the preserved receiver slot.
        // Match the existing descriptor completion's next-PC publication
        // before the potentially failing result push.
        cursor.advance(fallthrough.index());
        cursor.commit_owned(state, value)?;
    } else {
        // Guard the produced result while moving the original base. A failed
        // commit retires its output first; destructive cleanup stops the suffix.
        let mut output = OwnedValueGuard::new(state, &runtime.0.poisoned, value);
        let (state, value) = output.parts();
        let original = cursor.move_owned()?;
        let mut base = OwnedValueGuard::new(state, &runtime.0.poisoned, original);
        let (state, original) = base.parts();
        cursor.advance(fallthrough.index());
        let result = cursor.commit_owned(state, value.take().expect("owned named read output"));
        if !runtime.0.poisoned.get() {
            state
                .release_owned_jsvalue(
                    &runtime.0.poisoned,
                    original.take().expect("named read base"),
                )
                .map_err(runtime_error_to_vm_error)?;
        }
        result?;
    }
    #[cfg(feature = "profiling")]
    crate::engine::api::profiling::record_owned_instruction(depth);
    Ok(())
}
