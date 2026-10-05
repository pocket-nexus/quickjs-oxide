//! Literal construction transfers frame owners under the held state access.
use super::{
    Error, FallthroughPc, FrameCursor, FrameExecution, FrameTurn, JsValue, Runtime, RuntimeState,
    runtime_error_to_vm_error,
};

// Keep allocation, partial-prefix cleanup and GC out of the dispatch body.
// This is a synchronous transaction; it does not publish a continuation.
#[inline(never)]
pub(super) fn execute(
    runtime: &Runtime,
    state: &mut RuntimeState,
    segment: &mut FrameExecution<'_>,
    count: u16,
    fallthrough: FallthroughPc,
) -> Result<(), Error> {
    segment.materialize_in_state(state)?;
    {
        let FrameTurn {
            executable,
            transaction,
            fault_pc,
            resume_pc,
            ..
        } = segment.frame();
        #[cfg(feature = "profiling")]
        let depth = transaction.operand_depth();
        let mut cursor = FrameCursor::new(transaction, fault_pc, resume_pc, &runtime.0.poisoned);
        let mut values = Vec::new();
        values
            .try_reserve_exact(usize::from(count))
            .map_err(|_| Error::internal("array elements allocation failed"))?;
        let mut values_owner = crate::engine::heap::runtime::owned_values::OwnedValuesGuard::new(
            state,
            &runtime.0.poisoned,
            values,
        );
        let (state, values) = values_owner.parts();
        for _ in 0..count {
            values.push(cursor.move_owned()?);
        }
        values.reverse();
        let array = state
            .new_array_from_values_jsvalue(
                &runtime.0.poisoned,
                executable.realm,
                std::mem::take(values),
            )
            .map_err(runtime_error_to_vm_error)?;
        cursor.commit_owned(state, JsValue::Object(array))?;
        cursor.advance(fallthrough.index());
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_instruction(depth);
    }
    // Every element and the result owner are published before the
    // original allocation checkpoint can observe this Array.
    state
        .collect_if_requested(&runtime.0.gc_pressure, &runtime.0.poisoned)
        .map_err(runtime_error_to_vm_error)?;
    #[cfg(feature = "profiling")]
    crate::engine::api::profiling::record_owned_execution_event("core.internal_array_from");
    Ok(())
}
