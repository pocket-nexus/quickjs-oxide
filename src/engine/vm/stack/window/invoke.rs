//! OP_apply's borrowed operands become the same actual Invoke Query.
use super::*;
use crate::engine::{
    heap::runtime::RuntimeState,
    vm::{
        frame::{ReturnOwner, ReturnTarget, ReturnValue},
        proxy_get_driver::{RawNativeQuery, Step, resident_query},
    },
};
impl FrameExecution<'_> {
    pub(in crate::engine::vm) fn enter_apply_in_state(
        &mut self,
        runtime: &Runtime,
        state: &mut RuntimeState,
        kind: crate::engine::code::bytecode::ApplyKind,
        fallthrough: crate::engine::vm::execute::FallthroughPc,
    ) -> Result<crate::engine::vm::driver::ordinary::Entry, Error> {
        use crate::engine::vm::exception::runtime_error_to_vm_error;
        self.materialize_in_state(state)?;
        let parent = self.execution.frames.current_id().expect("Apply caller");
        let turn = self.frame();
        let realm = turn.executable.realm;
        let depth = turn.transaction.window.depth;
        let selected = crate::engine::builtins::InvokeStep::start_spread_in_state(
            state,
            &runtime.0.poisoned,
            realm,
            kind,
            turn.transaction.peek(2)?,
            turn.transaction.peek(1)?,
            turn.transaction.peek(0)?,
        )
        .map_err(runtime_error_to_vm_error)?;
        let return_to = ReturnTarget {
            owner: ReturnOwner::Frame(parent),
            value_use: ReturnValue::Push,
            tail: false,
            operation: None,
        };
        let query = resident_query(
            &mut self.execution.query_storage,
            realm,
            return_to,
            Some(depth),
        );
        let mut owner =
            RawNativeQuery::from_query(runtime, state, query, Step::InvokeProgress(Some(selected)));
        // The semantic request protects all source copies before original
        // operands retire in the old top-to-bottom instruction-prefix order.
        for _ in 0..3 {
            let value = self.frame().transaction.slots().pop()?;
            owner
                .state
                .release_owned_jsvalue(&runtime.0.poisoned, value)
                .map_err(runtime_error_to_vm_error)?;
        }
        let progress = self.consume_native_query(&mut owner, return_to, fallthrough)?;
        self.finish_state_call_progress(progress, false, fallthrough, depth)
    }
}
