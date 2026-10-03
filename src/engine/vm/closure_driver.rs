//! Closure construction and capture temporaries stay in the resident state lease.
use super::bindings::capture::{capture_frame_binding, capture_local_binding, reuse_frame_capture};
use super::closure::ClosureSlots;
use super::exception::runtime_error_to_vm_error;
use super::execute::{FallthroughPc, FrameCursor};
use crate::engine::api::{error::Error, runtime_error::RuntimeError};
use crate::engine::code::function::metadata::ClosureSource;
use crate::engine::code::runtime::PublishedFunctionSnapshot;
use crate::engine::heap::{BytecodeConstant, FunctionBytecodeId, VarRefId, runtime::RuntimeState};
use crate::engine::value::JsValue;
use std::cell::Cell;

/// The same existing capture Vec owns every checked temporary. Normal finish
/// retires captures before bytecode, matching the old rooted local drop order.
struct CapturedEdges<'a> {
    state: &'a mut RuntimeState,
    poisoned: &'a Cell<bool>,
    cells: Vec<VarRefId>,
    bytecode: Option<FunctionBytecodeId>,
}

impl CapturedEdges<'_> {
    fn retire(&mut self) -> Result<(), RuntimeError> {
        let _unwind = crate::engine::api::runtime::RuntimeUnwindGuard::from_flag(self.poisoned);
        for cell in self.cells.drain(..) {
            self.state
                .release_var_ref_handle(cell)
                .inspect_err(|_| self.poisoned.set(true))?;
        }
        if let Some(bytecode) = self.bytecode.take() {
            self.state
                .release_function_bytecode_handle(bytecode)
                .inspect_err(|_| self.poisoned.set(true))?;
        }
        Ok(())
    }
}

impl Drop for CapturedEdges<'_> {
    fn drop(&mut self) {
        if std::thread::panicking() {
            self.poisoned.set(true);
        }
        if !self.poisoned.get() {
            let _ = self.retire();
        }
    }
}

#[inline(never)]
#[allow(clippy::too_many_arguments)]
pub(super) fn instantiate(
    state: &mut RuntimeState,
    poisoned: &Cell<bool>,
    dynamic_import_allowed: bool,
    cursor: &mut FrameCursor<'_>,
    executable: &PublishedFunctionSnapshot,
    environment: &ClosureSlots,
    index: u32,
    fallthrough: FallthroughPc,
) -> Result<(), Error> {
    let Some(BytecodeConstant::Function(child)) = executable.constant(index) else {
        return Err(Error::internal(
            "function-closure opcode referenced a non-function constant",
        ));
    };
    let child = *child;
    let descriptors = state
        .heap
        .function_bytecode(child)
        .map_err(|error| Error::internal(error.to_string()))?
        .closure_variables
        .clone();
    // Preserve the old temporary FunctionBytecodeRef's checked retain, without
    // a Runtime owner. The published parent also owns the child constant.
    state
        .heap
        .retain_function_bytecode(child)
        .map_err(|error| Error::internal(error.to_string()))?;
    let mut captures = CapturedEdges {
        state,
        poisoned,
        cells: Vec::new(),
        bytecode: Some(child),
    };
    captures
        .cells
        .try_reserve_exact(descriptors.len())
        .map_err(|_| Error::internal("closure captures allocation failed"))?;
    for descriptor in descriptors.iter().copied() {
        let cell = match descriptor.source {
            ClosureSource::ParentLocal(index) => cursor.with_slots(|slots| {
                capture_local_binding(
                    captures.state,
                    poisoned,
                    slots.local_mut(index)?,
                    executable.local_definitions[usize::from(index)],
                    descriptor,
                )
                .map_err(runtime_error_to_vm_error)
            })?,
            ClosureSource::ParentArgument(index) => cursor.with_slots(|slots| {
                capture_frame_binding(
                    captures.state,
                    poisoned,
                    slots.parameter_mut(index)?,
                    descriptor,
                )
                .map_err(runtime_error_to_vm_error)
            })?,
            ClosureSource::ParentClosure(index) => {
                let cell = environment.cell_id(usize::from(index)).ok_or_else(|| {
                    Error::internal("captured parent closure index is out of bounds")
                })?;
                reuse_frame_capture(captures.state, cell, descriptor)
                    .map_err(runtime_error_to_vm_error)?
            }
            ClosureSource::ParentGlobal(index) => {
                let cell = environment.cell_id(usize::from(index)).ok_or_else(|| {
                    Error::internal("relayed parent global closure index is out of bounds")
                })?;
                captures
                    .state
                    .heap
                    .retain_var_ref(cell)
                    .map_err(|error| Error::internal(error.to_string()))?;
                cell
            }
            _ => {
                return Err(Error::internal(
                    "child closure attempted to resolve a root descriptor",
                ));
            }
        };
        captures.cells.push(cell);
    }
    // Keep capability rejection after capture preparation, as at the former
    // Runtime factory boundary; a denied child does not replay the prefix.
    captures
        .state
        .ensure_dynamic_import_bytecode_id_tree_authorized(child, dynamic_import_allowed)
        .map_err(runtime_error_to_vm_error)?;
    let function = captures
        .state
        .new_bytecode_closure_with_slots(poisoned, executable.realm, child, captures.cells.clone())
        .map_err(runtime_error_to_vm_error)?;
    cursor.commit_owned(captures.state, JsValue::Object(function))?;
    cursor.advance(fallthrough.index());
    captures.retire().map_err(runtime_error_to_vm_error)
}
