//! Native operand publication shared by Call and selected callbacks.
use super::*;
use crate::engine::vm::exception::runtime_error_to_vm_error;
use crate::engine::{
    heap::runtime::RuntimeState,
    vm::{
        call::{NativeInvocation, NativeInvokeMode, NativeStateGuard, ordinary::RawCallbackInputs},
        frames::NativeClassification,
    },
};

/// Concrete source projections, never fabricated operand slots. Callback
/// fields already own admitted edges and stay under their existing guard.
pub(in crate::engine::vm) enum NativeInputSource<'a> {
    CallerWindow {
        count: usize,
        method: bool,
    },
    Callback {
        inputs: &'a mut RawCallbackInputs,
        calling_realm: crate::engine::heap::ContextId,
        read_commit: Option<super::super::ReadOperandCommit<'a>>,
    },
}
impl FrameExecution<'_> {
    #[allow(clippy::too_many_arguments)]
    pub(in crate::engine::vm) fn publish_state_native_inputs<'s>(
        &mut self,
        runtime: &'s Runtime,
        state: &'s mut RuntimeState,
        selected: &NativeClassification,
        mut source: NativeInputSource<'_>,
    ) -> Result<NativeStateGuard<'s>, Error> {
        let caller_realm = match &source {
            NativeInputSource::CallerWindow { .. } => self.frame().executable.realm,
            NativeInputSource::Callback { calling_realm, .. } => *calling_realm,
        };
        let logical_depth = self.execution.frames.logical_active_depth(runtime);
        let (arguments, receiver, function) = match &mut source {
            NativeInputSource::CallerWindow { count, method } => self
                .frame()
                .transaction
                .take_validated_native_call_operands_in_state(
                    runtime,
                    state,
                    logical_depth,
                    *count,
                    *method,
                )?,
            NativeInputSource::Callback { inputs, .. } => {
                // Preserve the existing callback reserve before owner transfer.
                self.execution
                    .slots
                    .reserve_native_argument_depth(logical_depth)?;
                (
                    std::mem::take(&mut inputs.arguments),
                    inputs.receiver.take().expect("native callback receiver"),
                    inputs
                        .selected_callee
                        .take()
                        .expect("native callback callee"),
                )
            }
        };
        let target = selected.target();
        let realm = if target.uses_calling_realm() {
            caller_realm
        } else {
            selected.defining_realm()
        };
        let mut owner = NativeStateGuard::from_operands(
            state,
            &runtime.0.poisoned,
            function,
            realm,
            target,
            NativeInvokeMode::Ordinary,
            NativeInvocation::Call {
                this_value: receiver,
            },
            arguments,
        );
        {
            let (state, call) = owner.parts();
            let NativeInvocation::Call { this_value } = &call.invocation else {
                unreachable!("ordinary native receiver")
            };
            if runtime.0.deferred_references.has_pending()
                || crate::engine::vm::driver::ordinary::native_observes_activation_in_state(
                    state,
                    target,
                    this_value,
                    &call.activation.arguments.readable,
                )
            {
                self.execution.frames.materialize_in_state(state)?;
            } else {
                #[cfg(feature = "profiling")]
                crate::engine::api::profiling::record_owned_execution_event(
                    "native_unobserved_entry",
                );
            }
        }
        owner
            .publish(
                runtime.domain_id(),
                selected.minimum(),
                true,
                Some(selected),
            )
            .map_err(runtime_error_to_vm_error)?;
        if let NativeInputSource::Callback {
            inputs,
            read_commit: Some(commit),
            ..
        } = source
        {
            let (state, _) = owner.parts();
            // The activation protects the selected this role before the
            // original base and preserved role can perform destructive cleanup.
            self.commit_property_read_operands(
                state,
                &runtime.0.poisoned,
                &mut inputs.preserved_receiver,
                commit,
            )?;
        }
        Ok(owner)
    }
    /// One native publication/body/finish/wait consumer for caller windows,
    /// named getters and raw conversion callbacks.
    #[allow(clippy::too_many_arguments)]
    pub(in crate::engine::vm) fn enter_selected_native_state(
        &mut self,
        runtime: &Runtime,
        state: &mut RuntimeState,
        selected: NativeClassification,
        source: NativeInputSource<'_>,
        return_to: crate::engine::vm::frame::ReturnTarget,
        fallthrough: crate::engine::vm::execute::FallthroughPc,
        instruction_depth: Option<usize>,
    ) -> Result<crate::engine::vm::proxy_get_driver::StateNativeProgress, Error> {
        use crate::engine::vm::proxy_get_driver::{
            RawNativeQuery, Resume, StateNativeProgress, Step,
        };
        if let NativeInputSource::Callback {
            inputs,
            calling_realm,
            read_commit,
        } = source
        {
            if !self.execution.frames.can_push_with_continuations(0)
                || runtime.host_stack_would_overflow()
            {
                let query = crate::engine::vm::proxy_get_driver::resident_query(
                    &mut self.execution.query_storage,
                    calling_realm,
                    return_to,
                    instruction_depth,
                );
                let mut owner =
                    RawNativeQuery::from_query(runtime, state, query, Step::Complete(None));
                // Protect all selected roles before retiring the original base.
                let carried = RawCallbackInputs {
                    selected_callee: inputs.selected_callee.take(),
                    callback_callee: inputs.callback_callee.take(),
                    receiver: inputs.receiver.take(),
                    arguments: std::mem::take(&mut inputs.arguments),
                    preserved_receiver: inputs.preserved_receiver.take(),
                };
                owner.step = Step::CallbackBoundary(Some(Box::new(
                    crate::engine::vm::proxy_get_driver::SelectedRawCallback {
                        inputs: carried,
                        selection: crate::engine::vm::call::ordinary::CallbackSelection::Native(
                            selected,
                        ),
                        overflow: true,
                        resume: Resume::Identity,
                    },
                )));
                if let Some(commit) = read_commit {
                    let Step::CallbackBoundary(Some(boundary)) = &mut owner.step else {
                        unreachable!()
                    };
                    self.commit_property_read_operands(
                        owner.state,
                        &runtime.0.poisoned,
                        &mut boundary.inputs.preserved_receiver,
                        commit,
                    )?;
                }
                self.publish_native_boundary(&mut owner, return_to.owner, fallthrough)?;
                return Ok(StateNativeProgress::Boundary);
            }
            return self.enter_selected_native_state_admitted(
                runtime,
                state,
                selected,
                NativeInputSource::Callback {
                    inputs,
                    calling_realm,
                    read_commit,
                },
                return_to,
                fallthrough,
                instruction_depth,
            );
        }
        self.enter_selected_native_state_admitted(
            runtime,
            state,
            selected,
            source,
            return_to,
            fallthrough,
            instruction_depth,
        )
    }
    #[allow(clippy::too_many_arguments)]
    fn enter_selected_native_state_admitted(
        &mut self,
        runtime: &Runtime,
        state: &mut RuntimeState,
        mut selected: NativeClassification,
        source: NativeInputSource<'_>,
        return_to: crate::engine::vm::frame::ReturnTarget,
        fallthrough: crate::engine::vm::execute::FallthroughPc,
        instruction_depth: Option<usize>,
    ) -> Result<crate::engine::vm::proxy_get_driver::StateNativeProgress, Error> {
        use crate::engine::vm::proxy_get_driver::{
            RawNativeQuery, Resume, StateNativeProgress, Step,
        };
        let target = selected.target();
        let mut owner = self.publish_state_native_inputs(runtime, state, &selected, source)?;
        let realm = owner.parts().1.activation.realm;
        if !RuntimeState::has_state_native_body(target) {
            let kind = selected
                .take_operation()
                .unwrap_or(crate::engine::builtins::continuation::NativeOperation::Pure(target));
            let call = owner.into_inner();
            let mut query = RawNativeQuery::for_call(
                runtime,
                state,
                call,
                &mut self.execution.query_storage,
                Step::Complete(None),
                return_to,
                instruction_depth,
            );
            let call = query.call.take().expect("published bridge activation");
            query.step = Step::PreparedNativeBoundary(Some(Box::new(
                crate::engine::vm::proxy_get_driver::PreparedNativeBoundary {
                    call,
                    kind,
                    resume: Resume::Identity,
                },
            )));
            self.publish_native_boundary(&mut query, return_to.owner, fallthrough)?;
            return Ok(StateNativeProgress::Boundary);
        }
        let started = {
            let (state, call) = owner.parts();
            state.invoke_state_native_body(
                &runtime.0.poisoned,
                runtime.0.host_services.as_ref(),
                target,
                realm,
                &call.invocation,
                &call.activation.arguments,
            )
        };
        let started = match started {
            Ok(step) => step,
            Err(error) => {
                let call = owner.into_inner();
                let (result, arguments) =
                    call.finish_completion_reusing(state, &runtime.0.poisoned, Err(error));
                self.execution
                    .slots
                    .recycle_native_argument_buffer(arguments);
                return Err(runtime_error_to_vm_error(
                    result.expect_err("failed body cannot complete"),
                ));
            }
        };
        if let crate::engine::builtins::continuation::NativeStep::Complete(completion) = started {
            let call = owner.into_inner();
            let (result, arguments) =
                call.finish_completion_reusing(state, &runtime.0.poisoned, Ok(completion));
            self.execution
                .slots
                .recycle_native_argument_buffer(arguments);
            return result
                .map(StateNativeProgress::Complete)
                .map_err(runtime_error_to_vm_error);
        }
        let step = Step::try_from(started).map_err(runtime_error_to_vm_error)?;
        let call = owner.into_inner();
        let mut query = RawNativeQuery::for_call(
            runtime,
            state,
            call,
            &mut self.execution.query_storage,
            step,
            return_to,
            instruction_depth,
        );
        self.consume_native_query(&mut query, return_to, fallthrough)
    }

    fn publish_native_boundary(
        &mut self,
        owner: &mut crate::engine::vm::proxy_get_driver::RawNativeQuery<'_>,
        return_owner: crate::engine::vm::frame::ReturnOwner,
        fallthrough: crate::engine::vm::execute::FallthroughPc,
    ) -> Result<(), Error> {
        if self.execution.selected_native_query.is_some() {
            return Err(Error::internal("resident native boundary already occupied"));
        }
        owner.register()?;
        let identity = {
            let turn = self.frame();
            turn.property_generation
                .checked_add(1)
                .ok_or_else(|| Error::internal("property operation identity exhausted"))?
        };
        // Every recoverable check precedes owner transfer. The existing Query
        // protects the selected Step across this actual boundary; no new lookup.
        let query = owner.take_query();
        let step = owner.take_step();
        let turn = self.frame();
        *turn.property_generation = identity;
        *turn.resume_pc = fallthrough.index();
        self.execution.selected_native_query = Some(Box::new(
            crate::engine::vm::proxy_get_driver::ResidentQueryBoundary {
                query: Some(query),
                step: Some(step),
                owner: return_owner,
                identity,
            },
        ));
        Ok(())
    }
    #[allow(clippy::too_many_arguments)]
    pub(super) fn enter_state_native(
        &mut self,
        runtime: &Runtime,
        state: &mut RuntimeState,
        selected: NativeClassification,
        count: usize,
        method: bool,
        tail: bool,
        fallthrough: crate::engine::vm::execute::FallthroughPc,
        depth: usize,
    ) -> Result<crate::engine::vm::driver::ordinary::Entry, Error> {
        #[cfg(feature = "profiling")]
        use crate::engine::vm::proxy_get_driver::StateNativeProgress;
        use crate::engine::vm::{
            driver::ordinary::Entry,
            frame::{ReturnOwner, ReturnTarget, ReturnValue},
        };
        if !self.execution.frames.can_push_with_continuations(0)
            || runtime.host_stack_would_overflow()
        {
            self.execution.selected_native = Some(
                crate::engine::object::LinkedNativeSelection::from_classified_parts(
                    selected.into_linked_parts(),
                ),
            );
            return Ok(Entry::General);
        }
        let parent = self.execution.frames.current_id().expect("native caller");
        let result = self.enter_selected_native_state(
            runtime,
            state,
            selected,
            NativeInputSource::CallerWindow { count, method },
            ReturnTarget {
                owner: ReturnOwner::Frame(parent),
                value_use: ReturnValue::Push,
                tail,
                operation: None,
            },
            fallthrough,
            Some(depth),
        )?;
        #[cfg(feature = "profiling")]
        if matches!(result, StateNativeProgress::Complete(_)) {
            crate::engine::api::profiling::record_owned_execution_event(
                "core.internal_native_body",
            );
        }
        self.finish_state_call_progress(result, tail, fallthrough, depth)
    }

    pub(super) fn finish_state_call_progress(
        &mut self,
        result: crate::engine::vm::proxy_get_driver::StateNativeProgress,
        tail: bool,
        fallthrough: crate::engine::vm::execute::FallthroughPc,
        depth: usize,
    ) -> Result<crate::engine::vm::driver::ordinary::Entry, Error> {
        use crate::engine::vm::{
            Completion, driver::ordinary::Entry, proxy_get_driver::StateNativeProgress,
        };
        match result {
            StateNativeProgress::Published | StateNativeProgress::PublishedThrow => {
                Err(Error::internal("native Call used computed publication"))
            }
            StateNativeProgress::Entered => Ok(Entry::Ordinary),
            StateNativeProgress::Boundary => Ok(Entry::NativeBoundary),
            StateNativeProgress::Complete(completion) => {
                let (value, entry) = match completion {
                    Completion::Return(value) => (
                        value,
                        if tail {
                            Entry::NativeComplete
                        } else {
                            Entry::NativeReady
                        },
                    ),
                    Completion::Throw(value) => (value, Entry::NativeThrow),
                };
                self.execution.pending = Some(value);
                if !matches!(entry, Entry::NativeComplete) {
                    let mut turn = self.frame();
                    turn.transaction.slots().push_pending(turn.pending)?;
                    if matches!(entry, Entry::NativeReady) {
                        *turn.resume_pc = fallthrough.index();
                    }
                }
                #[cfg(feature = "profiling")]
                {
                    crate::engine::api::profiling::record_owned_instruction(depth);
                }
                #[cfg(not(feature = "profiling"))]
                let _ = depth;
                Ok(entry)
            }
        }
    }

    pub(super) fn consume_native_query(
        &mut self,
        owner: &mut crate::engine::vm::proxy_get_driver::RawNativeQuery<'_>,
        return_to: crate::engine::vm::frame::ReturnTarget,
        fallthrough: crate::engine::vm::execute::FallthroughPc,
    ) -> Result<crate::engine::vm::proxy_get_driver::StateNativeProgress, Error> {
        use crate::engine::vm::{
            call::ordinary::CallbackSelection,
            frame::{OperationTarget, ReturnTarget, ReturnValue},
            proxy_get_driver::{StateEffect, StateNativeProgress, Step},
        };
        let runtime = owner.runtime;
        loop {
            let progress = owner.advance().map_err(runtime_error_to_vm_error)?;
            if progress.cycle_published
                && !owner
                    .query
                    .as_mut()
                    .expect("resident query")
                    .carry_computed_publication()
            {
                // Other native queries already own the real completion or
                // callback edges in Step; trial deletion treats them as roots.
                owner
                    .state
                    .collect_if_requested(&runtime.0.gc_pressure, &runtime.0.poisoned)
                    .map_err(runtime_error_to_vm_error)?;
            }
            match progress.effect {
                StateEffect::Diagnostic => {
                    self.materialize_in_state(owner.state)?;
                    let Step::ComputedError(error) = &mut owner.step else {
                        unreachable!("selected computed diagnostic")
                    };
                    let error = error.take().expect("selected computed diagnostic");
                    let Some(kind) =
                        crate::engine::api::error::NativeErrorKind::from_javascript_error(
                            error.kind(),
                        )
                    else {
                        return Err(error);
                    };
                    let message = error.native_message().cloned().unwrap_or_else(|| {
                        crate::engine::api::error::NativeErrorMessage::from_utf8(error.message())
                    });
                    let realm = owner.realm();
                    let object = owner
                        .state
                        .new_native_error_from_message(&runtime.0.poisoned, realm, kind, message)
                        .map_err(runtime_error_to_vm_error)?;
                    // The real allocation owner enters Step before the common
                    // fact consumer can collect or retire another role.
                    owner.step =
                        Step::CyclePublishedComplete(Some(crate::engine::vm::Completion::Throw(
                            crate::engine::value::JsValue::Object(object),
                        )));
                    continue;
                }
                StateEffect::PropertyRead => {
                    self.prepare_computed_selected(owner)?;
                    continue;
                }
                StateEffect::Complete => {
                    if owner.has_native_scope() {
                        owner
                            .finish_native_scope(&mut self.execution.slots)
                            .map_err(runtime_error_to_vm_error)?;
                        continue;
                    }
                    if let Some(result) = self.finish_computed_query(
                        runtime,
                        owner.state,
                        owner.query.as_mut().expect("computed query"),
                        &mut owner.step,
                    )? {
                        let query = owner.take_query();
                        crate::engine::vm::proxy_get_driver::recycle_resident_query(
                            runtime,
                            owner.state,
                            &mut self.execution.query_storage,
                            query,
                        )
                        .map_err(runtime_error_to_vm_error)?;
                        return Ok(result);
                    }
                    let completion = owner
                        .finish_outer(&mut self.execution.slots)
                        .map_err(runtime_error_to_vm_error)?;
                    let query = owner.take_query();
                    crate::engine::vm::proxy_get_driver::recycle_resident_query(
                        runtime,
                        owner.state,
                        &mut self.execution.query_storage,
                        query,
                    )
                    .map_err(runtime_error_to_vm_error)?;
                    return Ok(StateNativeProgress::Complete(completion));
                }
                StateEffect::Boundary => {
                    owner
                        .publish_outer_scope()
                        .map_err(runtime_error_to_vm_error)?;
                    self.publish_native_boundary(owner, return_to.owner, fallthrough)?;
                    return Ok(StateNativeProgress::Boundary);
                }
                StateEffect::Callback => {}
            }
            owner
                .publish_outer_scope()
                .map_err(runtime_error_to_vm_error)?;
            let callback_realm = owner.realm();
            let selection = {
                let Step::RawCall {
                    inputs: Some(inputs),
                    ..
                } = &mut owner.step
                else {
                    unreachable!("raw callback effect")
                };
                inputs
                    .select_callback_in_state(runtime, owner.state, callback_realm)
                    .map_err(runtime_error_to_vm_error)?
            };
            let selection = match selection {
                crate::engine::value::conversion::NativeConversion::Value(selected) => selected,
                crate::engine::value::conversion::NativeConversion::Throw(value) => {
                    // Keep the diagnostic armed before retiring the normalized
                    // callee/receiver suffix. Fatal cleanup wins this reply.
                    let mut value =
                        crate::engine::heap::runtime::owned_values::OwnedValueGuard::new(
                            owner.state,
                            &runtime.0.poisoned,
                            value,
                        );
                    let (state, value) = value.parts();
                    let Step::RawCall { inputs, resume } = &mut owner.step else {
                        unreachable!()
                    };
                    inputs
                        .as_mut()
                        .expect("overflow callback inputs")
                        .retire(state, &runtime.0.poisoned)
                        .map_err(runtime_error_to_vm_error)?;
                    let parent = resume.take();
                    owner.step = Step::CyclePublishedPrimitiveReply {
                        value: Some(crate::engine::vm::Completion::Throw(
                            value.take().expect("Bound overflow diagnostic"),
                        )),
                        resume: parent,
                    };
                    continue;
                }
            };
            match selection {
                CallbackSelection::General => {
                    owner.callback_boundary(CallbackSelection::General, false);
                }
                CallbackSelection::Ordinary(call) => {
                    if !self
                        .execution
                        .frames
                        .can_push_with_continuations(owner.depth())
                        || runtime.bytecode_call_would_overflow()
                    {
                        owner.callback_boundary(CallbackSelection::Ordinary(call), true);
                        continue;
                    }
                    let parent = return_to.owner.frame()?;
                    let frame = self.execution.frames.current_mut(parent)?;
                    if frame.cold.has_pending_query() {
                        return Err(Error::internal("request overwrote a pending reply"));
                    }
                    let identity = frame
                        .property_generation
                        .checked_add(1)
                        .ok_or_else(|| Error::internal("property operation identity exhausted"))?;
                    frame.property_generation = identity;
                    owner.register()?;
                    let Step::RawCall {
                        inputs: Some(inputs),
                        resume,
                    } = &mut owner.step
                    else {
                        unreachable!()
                    };
                    inputs
                        .retire_selection(owner.state, &runtime.0.poisoned)
                        .map_err(runtime_error_to_vm_error)?;
                    let resume = resume.take().expect("callback parent");
                    let query = owner.take_query();
                    let mut pending = Some(
                        self.execution
                            .query_storage
                            .pending(identity, query, resume),
                    );
                    let Step::RawCall {
                        inputs: Some(inputs),
                        ..
                    } = &mut owner.step
                    else {
                        unreachable!()
                    };
                    let result = self.install_raw_property_callback(
                        runtime,
                        owner.state,
                        call,
                        inputs,
                        ReturnTarget {
                            owner: return_to.owner,
                            value_use: ReturnValue::Push,
                            tail: false,
                            operation: Some(OperationTarget::PropertyGet(identity)),
                        },
                        fallthrough,
                        &mut pending,
                    );
                    if let Err(error) = result {
                        // Before child publication the same local pending record
                        // still owns every Query/keeper. After publication it is
                        // installed on the parent and execution cleanup owns it.
                        if let Some(pending) = pending.take() {
                            let (_, query, resume) =
                                self.execution.query_storage.release_pending(pending);
                            owner.query = Some(query);
                            let Step::RawCall { resume: slot, .. } = &mut owner.step else {
                                unreachable!()
                            };
                            *slot = Some(resume);
                        }
                        return Err(error);
                    }
                    return Ok(StateNativeProgress::Entered);
                }
                CallbackSelection::Native(mut selected) => {
                    if !self
                        .execution
                        .frames
                        .can_push_with_continuations(owner.depth())
                        || runtime.host_stack_would_overflow()
                    {
                        owner.callback_boundary(CallbackSelection::Native(selected), true);
                        let Step::CallbackBoundary(Some(packet)) = &mut owner.step else {
                            unreachable!()
                        };
                        if let Some(input) = owner.query.as_mut().and_then(|query| query.computed_read_mut()).filter(|input| !input.prefix_published && input.phase == crate::engine::vm::proxy_get_driver::computed::ComputedPhase::Getter) {
                            self.commit_property_read_operands(owner.state, &runtime.0.poisoned, &mut packet.inputs.preserved_receiver, input.callback_commit())?;
                        }
                        continue;
                    }
                    let calling_realm = owner.realm();
                    let Step::RawCall {
                        inputs: Some(inputs),
                        ..
                    } = &mut owner.step
                    else {
                        unreachable!()
                    };
                    let target = selected.target();
                    let mut native = self.publish_state_native_inputs(
                        runtime,
                        owner.state,
                        &selected,
                        NativeInputSource::Callback {
                            inputs,
                            calling_realm,
                            read_commit: owner.query.as_mut().and_then(|query| query.computed_read_mut()).filter(|input| !input.prefix_published && input.phase == crate::engine::vm::proxy_get_driver::computed::ComputedPhase::Getter).map(|input| input.callback_commit()),
                        },
                    )?;
                    {
                        let (state, _) = native.parts();
                        self.service_computed_publication(
                            runtime,
                            state,
                            owner.query.as_mut().expect("native callback query"),
                        )?;
                    }
                    let realm = native.parts().1.activation.realm;
                    if !RuntimeState::has_state_native_body(target) {
                        let kind = selected.take_operation().unwrap_or(
                            crate::engine::builtins::continuation::NativeOperation::Pure(target),
                        );
                        let call = native.into_inner();
                        owner.native_boundary(call, kind);
                        continue;
                    }
                    let result = {
                        let (state, call) = native.parts();
                        state.invoke_state_native_body(
                            &runtime.0.poisoned,
                            runtime.0.host_services.as_ref(),
                            target,
                            realm,
                            &call.invocation,
                            &call.activation.arguments,
                        )
                    };
                    match result {
                        Ok(crate::engine::builtins::continuation::NativeStep::Complete(
                            completion,
                        )) => {
                            let call = native.into_inner();
                            let (result, arguments) = call.finish_completion_reusing(
                                owner.state,
                                &runtime.0.poisoned,
                                Ok(completion),
                            );
                            self.execution
                                .slots
                                .recycle_native_argument_buffer(arguments);
                            let completion = result.map_err(runtime_error_to_vm_error)?;
                            let Step::RawCall { resume, .. } = &mut owner.step else {
                                unreachable!()
                            };
                            let resume = resume.take().expect("native callback parent");
                            owner.step = resume
                                .resume_in_state(owner.state, &runtime.0.poisoned, completion)
                                .map_err(runtime_error_to_vm_error)?;
                        }
                        Ok(step) => {
                            owner.pending_step =
                                Some(Step::try_from(step).map_err(runtime_error_to_vm_error)?);
                            owner.pending_call = Some(native.into_inner());
                            owner
                                .install_pending_native_scope()
                                .map_err(runtime_error_to_vm_error)?;
                        }
                        Err(error) => {
                            let call = native.into_inner();
                            let (result, arguments) = call.finish_completion_reusing(
                                owner.state,
                                &runtime.0.poisoned,
                                Err(error),
                            );
                            self.execution
                                .slots
                                .recycle_native_argument_buffer(arguments);
                            return Err(runtime_error_to_vm_error(
                                result.expect_err("failed body cannot complete"),
                            ));
                        }
                    }
                }
            }
        }
    }
}

impl FrameExecution<'_> {
    pub(super) fn resume_native_query_after_return(
        &mut self,
        runtime: &Runtime,
        state: &mut RuntimeState,
        target: crate::engine::vm::frame::ReturnTarget,
    ) -> Result<crate::engine::vm::driver::ordinary::Entry, Error> {
        use crate::engine::vm::{
            Completion,
            driver::ordinary::Entry,
            frame::{ReturnTarget, ReturnValue},
            proxy_get_driver::{RawNativeQuery, StateNativeProgress, Step},
        };
        let parent = target.owner.frame()?;
        let pending = self.execution.frames.take_pending(parent)?;
        if !pending.can_resume_raw_in_state(target.operation) {
            // Checked before retirement by the same exclusive frame lease.
            self.execution
                .frames
                .put_pending(parent, pending)
                .expect("reply restores its empty parent slot");
            return Err(Error::internal(
                "native reply no longer matches its selected query",
            ));
        }
        let (_, query, resume) = self.execution.query_storage.release_pending(pending);
        let step = Step::PrimitiveReply {
            value: Some(Completion::Return(
                self.execution
                    .pending
                    .take()
                    .expect("registered callback result"),
            )),
            resume: Some(resume),
        };
        let mut owner = RawNativeQuery::from_query(runtime, state, query, step);
        let fallthrough =
            owner.commit_reply_continuation(self.execution.frames.current_mut(parent)?)?;
        let (tail, value_use, depth) = owner.return_placement();
        let result = self.consume_native_query(
            &mut owner,
            ReturnTarget {
                owner: target.owner,
                value_use,
                tail,
                operation: None,
            },
            fallthrough,
        )?;
        match result {
            StateNativeProgress::Published => Ok(Entry::NativeReady),
            StateNativeProgress::PublishedThrow => Ok(Entry::NativeThrow),
            StateNativeProgress::Entered => Ok(Entry::Ordinary),
            StateNativeProgress::Boundary => Ok(Entry::NativeBoundary),
            StateNativeProgress::Complete(completion) => {
                let (value, entry) = match completion {
                    Completion::Return(value) => (
                        value,
                        if tail {
                            Entry::NativeComplete
                        } else {
                            Entry::NativeReady
                        },
                    ),
                    Completion::Throw(value) => (value, Entry::NativeThrow),
                };
                self.execution.pending = Some(value);
                if !matches!(entry, Entry::NativeComplete) {
                    if matches!(value_use, ReturnValue::Push) || matches!(entry, Entry::NativeThrow)
                    {
                        let mut turn = self.frame();
                        turn.transaction.slots().push_pending(turn.pending)?;
                    } else {
                        let value = self
                            .execution
                            .pending
                            .take()
                            .expect("discarded native result");
                        owner
                            .state
                            .release_owned_jsvalue(&runtime.0.poisoned, value)
                            .map_err(runtime_error_to_vm_error)?;
                    }
                }
                #[cfg(feature = "profiling")]
                if let Some(depth) = depth {
                    crate::engine::api::profiling::record_owned_instruction(depth);
                }
                #[cfg(not(feature = "profiling"))]
                let _ = depth;
                Ok(entry)
            }
        }
    }
}
