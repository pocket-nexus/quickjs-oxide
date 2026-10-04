//! Bound CALL uses the existing resident callback publisher and reply loop.
use super::*;
use crate::engine::{
    heap::runtime::RuntimeState,
    vm::{
        call::{
            BoundSelection,
            ordinary::{RawCallbackGuard, RawCallbackInputs},
        },
        frame::{ReturnOwner, ReturnTarget, ReturnValue},
        proxy_get_driver::{RawNativeQuery, Resume, Step, resident_query},
    },
};

impl FrameExecution<'_> {
    #[allow(clippy::too_many_arguments)]
    pub(in crate::engine::vm) fn enter_bound_call(
        &mut self,
        runtime: &Runtime,
        state: &mut RuntimeState,
        selected: BoundSelection,
        count: usize,
        method: bool,
        tail: bool,
        fallthrough: crate::engine::vm::execute::FallthroughPc,
        depth: usize,
    ) -> Result<crate::engine::vm::driver::ordinary::Entry, Error> {
        use crate::engine::value::conversion::NativeConversion;
        use crate::engine::vm::exception::runtime_error_to_vm_error;
        // Retain the old ready Call exit's real caller materialization point
        // before Bound promotion and possible diagnostic allocation. It is
        // shared registry work, not a temporary header-root requirement; no
        // Bound frame is created.
        self.execution.frames.materialize_in_state(state)?;
        let parent = self.execution.frames.current_id().expect("Bound caller");
        let realm = self.frame().executable.realm;
        // The payload's genuine checked target/receiver/argv promotions
        // precede caller argument-buffer admission, as in the previous entry.
        let snapshot = selected
            .snapshot_in_state(state, &runtime.0.poisoned)
            .map_err(runtime_error_to_vm_error)?;
        let mut snapshot = RawCallbackGuard::new(state, &runtime.0.poisoned, snapshot);
        let (state, snapshot_inputs) = snapshot.parts();
        let logical_depth = self.execution.frames.logical_active_depth(runtime);
        let (arguments, receiver, function) = self
            .frame()
            .transaction
            .take_validated_native_call_operands_in_state(
                runtime,
                state,
                logical_depth,
                count,
                method,
            )?;
        let return_to = ReturnTarget {
            owner: ReturnOwner::Frame(parent),
            value_use: ReturnValue::Push,
            tail,
            operation: None,
        };
        let query = resident_query(
            &mut self.execution.query_storage,
            realm,
            return_to,
            Some(depth),
        );
        let mut owner = RawNativeQuery::from_query(
            runtime,
            state,
            query,
            Step::RawCall {
                inputs: Some(RawCallbackInputs::new(function, receiver, arguments)),
                resume: Some(Resume::Identity),
            },
        );
        // Consume the exact first selection instead of inspecting this Bound
        // payload again. Later target classifications use the shared loop.
        let Step::RawCall {
            inputs: Some(inputs),
            ..
        } = &mut owner.step
        else {
            unreachable!()
        };
        let result = inputs
            .apply_bound_snapshot_in_state(owner.state, &runtime.0.poisoned, realm, snapshot_inputs)
            .map_err(runtime_error_to_vm_error)?;
        if let NativeConversion::Throw(value) = result {
            let mut value = crate::engine::heap::runtime::owned_values::OwnedValueGuard::new(
                owner.state,
                &runtime.0.poisoned,
                value,
            );
            let (state, value) = value.parts();
            inputs
                .retire(state, &runtime.0.poisoned)
                .map_err(runtime_error_to_vm_error)?;
            owner.step = Step::CyclePublishedPrimitiveReply {
                value: Some(crate::engine::vm::Completion::Throw(
                    value.take().expect("Bound overflow diagnostic"),
                )),
                resume: Some(Resume::Identity),
            };
        }
        let result = self.consume_native_query(&mut owner, return_to, fallthrough)?;
        self.finish_state_call_progress(result, tail, fallthrough, depth)
    }
}
