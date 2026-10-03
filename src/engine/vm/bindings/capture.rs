//! Capture publication owns temporary cells through the current state access.
use super::{FrameBinding, is_private_callable_kind, release_frame_binding_in_state};
use crate::engine::api::runtime_error::RuntimeError;
use crate::engine::code::function::metadata::{
    ClosureVariable, ClosureVariableKind, VariableDefinition,
};
use crate::engine::heap::{VarRefId, runtime::RuntimeState};
use std::cell::Cell;

struct CapturedCellOwner<'a> {
    state: &'a mut RuntimeState,
    poisoned: &'a Cell<bool>,
    cell: Option<VarRefId>,
}

impl Drop for CapturedCellOwner<'_> {
    fn drop(&mut self) {
        if std::thread::panicking() {
            self.poisoned.set(true);
        }
        if self.poisoned.get() {
            return;
        }
        let _unwind = crate::engine::api::runtime::RuntimeUnwindGuard::from_flag(self.poisoned);
        if let Some(cell) = self.cell.take()
            && self.state.release_var_ref_handle(cell).is_err()
        {
            self.poisoned.set(true);
        }
    }
}

fn publish_captured_cell(
    state: &mut RuntimeState,
    poisoned: &Cell<bool>,
    binding: &mut FrameBinding,
    cell: VarRefId,
) -> Result<VarRefId, RuntimeError> {
    let mut owner = CapturedCellOwner {
        state,
        poisoned,
        cell: Some(cell),
    };
    // The frame and returned temporary each own one independent cell edge.
    owner.state.heap.retain_var_ref(cell)?;
    let previous = std::mem::replace(binding, FrameBinding::Captured(cell));
    release_frame_binding_in_state(owner.state, previous).inspect_err(|_| poisoned.set(true))?;
    Ok(owner.cell.take().expect("new capture owner"))
}

pub(in crate::engine::vm) fn capture_frame_binding(
    state: &mut RuntimeState,
    poisoned: &Cell<bool>,
    binding: &mut FrameBinding,
    descriptor: ClosureVariable,
) -> Result<VarRefId, RuntimeError> {
    let cell = match binding {
        FrameBinding::Direct(_) => {
            if descriptor.kind.is_private() {
                return Err(RuntimeError::Invariant(
                    "private-name capture reached an ordinary frame value",
                ));
            }
            let FrameBinding::Direct(value) =
                std::mem::replace(binding, FrameBinding::Uninitialized)
            else {
                unreachable!("direct binding authenticated before the move")
            };
            state.new_var_ref(
                poisoned,
                value,
                descriptor.is_lexical,
                descriptor.is_const,
                descriptor.kind,
            )?
        }
        FrameBinding::Private(index) => {
            if descriptor.kind != ClosureVariableKind::PrivateField
                || !descriptor.is_lexical
                || !descriptor.is_const
            {
                return Err(RuntimeError::Invariant(
                    "private-field frame cell used an incompatible closure descriptor",
                ));
            }
            state.new_private_var_ref_from_index(poisoned, *index)?
        }
        FrameBinding::PrivateCallable(object) => {
            if !is_private_callable_kind(descriptor.kind)
                || !descriptor.is_lexical
                || !descriptor.is_const
            {
                return Err(RuntimeError::Invariant(
                    "private-callable frame cell used an incompatible closure descriptor",
                ));
            }
            state.new_private_callable_var_ref_from_id(*object, descriptor.kind)?
        }
        FrameBinding::Uninitialized => state.new_uninitialized_captured_var_ref(
            descriptor.is_lexical,
            descriptor.is_const,
            descriptor.kind,
        )?,
        FrameBinding::Captured(cell) => return reuse_frame_capture(state, *cell, descriptor),
    };
    publish_captured_cell(state, poisoned, binding, cell)
}

pub(in crate::engine::vm) fn reuse_frame_capture(
    state: &mut RuntimeState,
    cell: VarRefId,
    descriptor: ClosureVariable,
) -> Result<VarRefId, RuntimeError> {
    state.validate_var_ref_metadata(cell, descriptor)?;
    state.heap.retain_var_ref(cell)?;
    Ok(cell)
}

pub(in crate::engine::vm) fn capture_local_binding(
    state: &mut RuntimeState,
    poisoned: &Cell<bool>,
    binding: &mut FrameBinding,
    definition: VariableDefinition,
    descriptor: ClosureVariable,
) -> Result<VarRefId, RuntimeError> {
    if let FrameBinding::Captured(cell) = binding {
        reuse_frame_capture(state, *cell, descriptor)
    } else {
        capture_frame_binding(
            state,
            poisoned,
            binding,
            ClosureVariable {
                is_lexical: definition.is_lexical,
                is_const: definition.is_const,
                kind: definition.kind,
                ..descriptor
            },
        )
    }
}

#[cfg(test)]
mod tests;
