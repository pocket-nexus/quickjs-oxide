//! Final computed read publication on the existing current frame/window.
use super::*;
use crate::engine::{
    heap::runtime::RuntimeState,
    object::{OwnedRead, ReadStep},
    vm::{
        Completion,
        call::ordinary::RawCallbackInputs,
        exception::runtime_error_to_vm_error,
        proxy_get_driver::{
            Finish, RawNativeQuery, StateNativeProgress, Step, computed::ComputedPhase,
        },
    },
};

impl FrameExecution<'_> {
    pub(in crate::engine::vm) fn service_computed_publication(
        &mut self,
        runtime: &Runtime,
        state: &mut RuntimeState,
        query: &mut crate::engine::vm::proxy_get_driver::Query,
    ) -> Result<(), Error> {
        if query.has_computed_publication() {
            self.materialize_in_state(state)?;
            state
                .collect_if_requested(&runtime.0.gc_pressure, &runtime.0.poisoned)
                .map_err(runtime_error_to_vm_error)?;
            query.clear_computed_publication();
        }
        Ok(())
    }
    pub(in crate::engine::vm) fn acquire_read_query(
        &mut self,
        realm: crate::engine::heap::ContextId,
        finish: Finish,
    ) -> crate::engine::vm::proxy_get_driver::Query {
        crate::engine::vm::proxy_get_driver::computed::acquire_query(
            &mut self.execution.query_storage,
            realm,
            finish,
        )
    }
    pub(in crate::engine::vm) fn consume_read_query(
        &mut self,
        owner: &mut RawNativeQuery<'_>,
        fallthrough: crate::engine::vm::execute::FallthroughPc,
    ) -> Result<StateNativeProgress, Error> {
        let return_to = self.named_getter_return_target();
        let result = self.consume_native_query(owner, return_to, fallthrough);
        if result.is_err() && !owner.runtime.0.poisoned.get() {
            owner.retire().map_err(runtime_error_to_vm_error)?;
        }
        result
    }

    pub(in crate::engine::vm) fn prepare_computed_selected(
        &mut self,
        owner: &mut RawNativeQuery<'_>,
    ) -> Result<(), Error> {
        self.prepare_computed_selected_step(
            owner.runtime,
            owner.state,
            owner.query.as_mut().expect("computed query"),
            &mut owner.step,
        )
    }
    pub(in crate::engine::vm) fn prepare_computed_selected_step(
        &mut self,
        runtime: &Runtime,
        state: &mut RuntimeState,
        query: &mut crate::engine::vm::proxy_get_driver::Query,
        step: &mut Step,
    ) -> Result<(), Error> {
        let input = query.computed_read_mut().expect("computed final read");
        let Step::RawRead { read, .. } = step else {
            unreachable!("selected computed read")
        };
        if matches!(read, Some(ReadStep::Shared(_))) {
            return Ok(());
        }
        input.cycle_published |= matches!(read, Some(ReadStep::CyclePublished(_)));
        if matches!(
            read,
            Some(
                ReadStep::Ready(OwnedRead::Proxy { .. })
                    | ReadStep::CyclePublished(OwnedRead::Proxy { .. })
            )
        ) {
            // The converted base is already an owner. The direct path keeps
            // its original independent checked preservation role.
            if input.base.is_none() {
                input.base = Some(
                    state
                        .dup_jsvalue(self.frame().transaction.peek(1)?)
                        .map_err(runtime_error_to_vm_error)?,
                );
            }
            input.phase = ComputedPhase::Proxy;
            return Ok(());
        }
        input
            .retire_atom(state, &runtime.0.poisoned)
            .map_err(runtime_error_to_vm_error)?;
        let selected = match read.take().expect("computed selected read") {
            ReadStep::Ready(read) | ReadStep::CyclePublished(read) => read,
            ReadStep::Shared(_) => unreachable!(),
        };
        match selected {
            OwnedRead::Complete(value) => {
                *self.frame().selected_native = input.native.take();
                input.phase = ComputedPhase::Complete;
                *step = Step::Complete(Some(Completion::Return(
                    value.unwrap_or(JsValue::Undefined),
                )));
            }
            OwnedRead::Getter { function, receiver } => {
                // Arm the selected getter/this before the fallible direct-base
                // promotion. Converted reads move their existing base edge.
                *step = Step::RawCall {
                    inputs: Some(RawCallbackInputs::new(function, receiver, Vec::new())),
                    resume: Some(crate::engine::vm::proxy_get_driver::Resume::Identity),
                };
                if input.base.is_none() {
                    input.base = Some(
                        state
                            .dup_jsvalue(self.frame().transaction.peek(1)?)
                            .map_err(runtime_error_to_vm_error)?,
                    );
                }
                let Step::RawCall {
                    inputs: Some(inputs),
                    ..
                } = &mut *step
                else {
                    unreachable!()
                };
                inputs.preserved_receiver = input.base.take();
                input.phase = ComputedPhase::Getter;
            }
            OwnedRead::Proxy { .. } => unreachable!(),
        }
        Ok(())
    }

    /// Called before Query recycle and before the final allocation checkpoint.
    pub(in crate::engine::vm) fn finish_computed_query(
        &mut self,
        runtime: &Runtime,
        state: &mut RuntimeState,
        query: &mut crate::engine::vm::proxy_get_driver::Query,
        step: &mut Step,
    ) -> Result<Option<StateNativeProgress>, Error> {
        if !matches!(
            query.finish,
            Some(Finish::ComputedRead(_) | Finish::PropertyKeyValue { .. })
        ) {
            return Ok(None);
        }
        let Step::Complete(value) = step else {
            return Err(Error::internal("computed instruction omitted its result"));
        };
        let completion = value.take().expect("computed completion");
        if let Completion::Throw(value) = completion {
            if let Some(Finish::ComputedRead(input)) = query.finish.as_mut()
                && !input.prefix_published
            {
                let FrameTurn {
                    mut transaction,
                    resume_pc,
                    ..
                } = self.frame();
                let fault = *resume_pc;
                // A nullish/direct failure still owns its original base/key
                // slots. Consume that actual prefix under the same guarded
                // publisher before committing the thrown owner; the verifier
                // promises space for the result, not an extra third operand.
                super::super::publish_property_read_result(
                    state,
                    &runtime.0.poisoned,
                    &mut transaction,
                    resume_pc,
                    fault,
                    &mut input.base,
                    &mut None,
                    false,
                    input.consume,
                    value,
                )?;
                input.prefix_published = true;
            } else {
                let FrameTurn {
                    transaction,
                    fault_pc,
                    resume_pc,
                    ..
                } = self.frame();
                let mut cursor = crate::engine::vm::execute::FrameCursor::new(
                    transaction,
                    fault_pc,
                    resume_pc,
                    &runtime.0.poisoned,
                );
                cursor.commit_owned(state, value)?;
            }
            if let Some(finish) = query.finish.as_mut() {
                finish
                    .retire_computed_in_state(state, &runtime.0.poisoned)
                    .map_err(runtime_error_to_vm_error)?;
            }
            if query.has_computed_publication() {
                self.materialize_in_state(state)?;
                state
                    .collect_if_requested(&runtime.0.gc_pressure, &runtime.0.poisoned)
                    .map_err(runtime_error_to_vm_error)?;
            }
            return Ok(Some(StateNativeProgress::PublishedThrow));
        }
        let Completion::Return(value) = completion else {
            unreachable!()
        };
        match query.finish.as_mut().expect("computed final continuation") {
            Finish::ComputedRead(input) if !input.prefix_published => {
                let FrameTurn {
                    mut transaction,
                    resume_pc,
                    ..
                } = self.frame();
                super::super::publish_property_read_result(
                    state,
                    &runtime.0.poisoned,
                    &mut transaction,
                    resume_pc,
                    input.fallthrough.index(),
                    &mut input.base,
                    &mut input.retained_key,
                    input.keep_receiver,
                    input.consume,
                    value,
                )?;
                input.prefix_published = true;
            }
            Finish::ComputedRead(input) => {
                let FrameTurn {
                    transaction,
                    fault_pc,
                    resume_pc,
                    ..
                } = self.frame();
                let mut cursor = crate::engine::vm::execute::FrameCursor::new(
                    transaction,
                    fault_pc,
                    resume_pc,
                    &runtime.0.poisoned,
                );
                cursor.advance(input.fallthrough.index());
                cursor.commit_owned(state, value)?;
            }
            Finish::PropertyKeyValue { fallthrough, .. } => {
                let FrameTurn {
                    transaction,
                    fault_pc,
                    resume_pc,
                    ..
                } = self.frame();
                let mut cursor = crate::engine::vm::execute::FrameCursor::new(
                    transaction,
                    fault_pc,
                    resume_pc,
                    &runtime.0.poisoned,
                );
                cursor.advance(fallthrough.index());
                cursor.commit_owned(state, value)?;
            }
            _ => unreachable!(),
        }
        let cycle_published = query.has_computed_publication();
        #[cfg(feature = "profiling")]
        {
            let (depth, event) = match query.finish.as_ref().expect("computed finish") {
                Finish::ComputedRead(input) => (input.depth, "core.internal_computed_read"),
                Finish::PropertyKeyValue { depth, .. } => {
                    (*depth, "core.internal_property_key_value")
                }
                _ => unreachable!(),
            };
            crate::engine::api::profiling::record_owned_instruction(depth);
            crate::engine::api::profiling::record_owned_execution_event(event);
        }
        if cycle_published {
            self.materialize_in_state(state)?;
            state
                .collect_if_requested(&runtime.0.gc_pressure, &runtime.0.poisoned)
                .map_err(runtime_error_to_vm_error)?;
        }
        Ok(Some(StateNativeProgress::Published))
    }
}
