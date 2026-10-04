//! Shared actual-argument snapshots, capture preparation and factory consumers.
use super::exception::runtime_error_to_vm_error;
use crate::engine::{
    api::error::Error,
    code::{
        bytecode::ArgumentsKind,
        function::metadata::{
            ClosureSource, ClosureVariable, ClosureVariableKind, ClosureVariableName,
        },
    },
    heap::{
        ObjectId,
        runtime::{
            RuntimeState,
            owned_values::{OwnedValueGuard, OwnedValuesGuard},
        },
    },
    value::JsValue,
};
use std::cell::Cell;

pub(super) fn arguments_in_state(
    state: &mut RuntimeState,
    poisoned: &Cell<bool>,
    executable: &crate::engine::code::runtime::PublishedFunctionSnapshot,
    function: ObjectId,
    slots: &mut super::stack::FrameSlots<'_>,
    kind: ArgumentsKind,
) -> Result<ObjectId, Error> {
    let count = slots.actual_argument_count();
    match kind {
        ArgumentsKind::Unmapped => {
            let values = slots.snapshot_argument_tail_in_state(state, poisoned, 0)?;
            state
                .new_unmapped_arguments_object(poisoned, executable.realm, values)
                .map_err(runtime_error_to_vm_error)
        }
        ArgumentsKind::Mapped => {
            let mapped = count.min(executable.argument_definitions.len());
            let mut captures = super::closure_driver::CapturedEdges::for_arguments(state, poisoned);
            let (state, cells) = captures.parts();
            cells
                .try_reserve_exact(count)
                .map_err(|_| Error::internal("arguments cells allocation failed"))?;
            for index in 0..mapped {
                let index = u16::try_from(index)
                    .map_err(|_| Error::internal("argument index exceeds u16::MAX"))?;
                cells.push(
                    super::bindings::capture::capture_frame_binding(
                        state,
                        poisoned,
                        slots.parameter_mut(index)?,
                        ClosureVariable {
                            source: ClosureSource::ParentArgument(index),
                            name: ClosureVariableName::None,
                            is_lexical: false,
                            is_const: false,
                            kind: ClosureVariableKind::Normal,
                        },
                    )
                    .map_err(runtime_error_to_vm_error)?,
                );
            }
            let values = slots.snapshot_argument_tail_in_state(state, poisoned, mapped)?;
            let mut values = OwnedValuesGuard::new(state, poisoned, values);
            let (state, values) = values.parts();
            for value in values.iter_mut() {
                cells.push(
                    state
                        .new_var_ref(
                            poisoned,
                            std::mem::replace(value, JsValue::Undefined),
                            false,
                            false,
                            ClosureVariableKind::Normal,
                        )
                        .map_err(runtime_error_to_vm_error)?,
                );
            }
            // Preserve the frame's checked legacy function temporary, followed
            // by the factory's distinct checked callable temporary.
            let function_value = state
                .dup_jsvalue(&JsValue::Object(function))
                .map_err(runtime_error_to_vm_error)?;
            let mut function_owner = OwnedValueGuard::new(state, poisoned, function_value);
            let (state, function_owner) = function_owner.parts();
            let callee = state
                .checked_arguments_callee(poisoned, function)
                .map_err(runtime_error_to_vm_error)?;
            let object = state
                .new_mapped_arguments_object(
                    poisoned,
                    executable.realm,
                    callee,
                    std::mem::take(cells),
                )
                .map_err(runtime_error_to_vm_error)?;
            let mut result = OwnedValueGuard::new(state, poisoned, JsValue::Object(object));
            let (state, result) = result.parts();
            state
                .release_owned_jsvalue(
                    poisoned,
                    function_owner.take().expect("arguments callee temporary"),
                )
                .map_err(runtime_error_to_vm_error)?;
            let Some(JsValue::Object(object)) = result.take() else {
                unreachable!()
            };
            Ok(object)
        }
    }
}
pub(super) fn rest_in_state(
    state: &mut RuntimeState,
    poisoned: &Cell<bool>,
    realm: crate::engine::heap::ContextId,
    slots: &mut super::stack::FrameSlots<'_>,
    start: u16,
) -> Result<ObjectId, Error> {
    let values = slots.snapshot_argument_tail_in_state(state, poisoned, usize::from(start))?;
    state
        .new_array_from_values_jsvalue(poisoned, realm, values)
        .map_err(runtime_error_to_vm_error)
}
