//! Bounded native-stack dispatch for write requests.
use super::{
    Completion, DirectCallTarget, Error, Finish, NativeConversion, Next, Query, Resume,
    ReturnOwner, RunningExecution, Runtime, Step, overflow, runtime_error_to_vm_error,
};

#[inline(never)]
pub(super) fn keys(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    _owner: ReturnOwner,
    _identity: u64,
    query: &mut Query,
    pending: &mut Step,
) -> Result<Next, Error> {
    let step = pending;
    loop {
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_execution_event("dispatch_write.keys.visit");
        let realm = query.realm;
        match &mut *step {
            Step::SnapshotEnumerable {
                object,
                key,
                resume,
            } => {
                let object = object.take().expect("selected Step field");
                let key = key.take().expect("selected Step field");

                if runtime
                    .is_proxy_object(&object)
                    .map_err(runtime_error_to_vm_error)?
                {
                    *step = Step::Descriptor {
                        object: Some(object),
                        key: Some(key),
                        resume: Some(Resume::OwnFlagReply {
                            enumerable: true,
                            resume: Box::new(resume.take().expect("selected Step field")),
                        }),
                    };
                } else {
                    let result = runtime
                        .internal_snapshot_own_property_is_enumerable(realm, &object, &key)
                        .map_err(runtime_error_to_vm_error)?;
                    let resume = resume.take().expect("selected Step field");
                    *step = resume
                        .boolean(runtime, result)
                        .map_err(runtime_error_to_vm_error)?;
                }
                continue;
            }
            Step::OwnFlag {
                object,
                key,
                enumerable,
                resume,
            } => {
                let object = object.take().expect("selected Step field");
                let key = key.take().expect("selected Step field");
                let enumerable = enumerable.take().expect("selected Step field");

                if runtime
                    .is_proxy_object(&object)
                    .map_err(runtime_error_to_vm_error)?
                {
                    *step = Step::Descriptor {
                        object: Some(object),
                        key: Some(key),
                        resume: Some(Resume::OwnFlagReply {
                            enumerable,
                            resume: Box::new(resume.take().expect("selected Step field")),
                        }),
                    };
                } else {
                    let result = if enumerable {
                        runtime.internal_own_property_is_enumerable(realm, &object, &key)
                    } else {
                        runtime.internal_has_own_property(realm, &object, &key)
                    }
                    .map_err(runtime_error_to_vm_error)?;
                    let resume = resume.take().expect("selected Step field");
                    *step = resume
                        .boolean(runtime, result)
                        .map_err(runtime_error_to_vm_error)?;
                }
                continue;
            }
            Step::Keys { object, resume } => {
                let object = object.take().expect("selected Step field");

                if runtime
                    .is_proxy_object(&object)
                    .map_err(runtime_error_to_vm_error)?
                {
                    if !execution
                        .frames
                        .can_push_with_continuations(query.continuation_depth())
                    {
                        let Completion::Throw(value) = overflow(runtime, realm)? else {
                            unreachable!()
                        };
                        let resume = resume.take().expect("selected Step field");
                        *step = resume
                            .keys(runtime, NativeConversion::Throw(value))
                            .map_err(runtime_error_to_vm_error)?;
                        continue;
                    }
                    query
                        .parents
                        .try_reserve(1)
                        .map_err(|_| Error::internal("ownKeys continuation allocation failed"))?;
                    let resume = resume.take().expect("selected Step field");
                    query.parents.push(resume);
                    *step = crate::engine::object::KeysStep::start(runtime, realm, object)
                        .map_err(runtime_error_to_vm_error)?
                        .try_into()?;
                } else {
                    let result = runtime
                        .own_property_keys(&object)
                        .map_err(runtime_error_to_vm_error)?;
                    let resume = resume.take().expect("selected Step field");
                    *step = resume
                        .keys(runtime, NativeConversion::Value(result))
                        .map_err(runtime_error_to_vm_error)?;
                }
                continue;
            }
            Step::KeysComplete(result) => {
                let resume = query
                    .parents
                    .pop()
                    .ok_or_else(|| Error::internal("ownKeys result has no parent"))?;
                let result = result.take().expect("selected Step field");
                *step = resume
                    .keys(runtime, result)
                    .map_err(runtime_error_to_vm_error)?;
                continue;
            }
            Step::ReadValue {
                receiver,
                key,
                resume,
            } => {
                let receiver = receiver.take().expect("selected Step field");
                let key = key.take().expect("selected Step field");

                let read = runtime
                    .prepare_value_property_read_completion(realm, receiver, &key)
                    .map_err(runtime_error_to_vm_error)?;
                let resume = resume.take().expect("selected Step field");
                *step = match read {
                    NativeConversion::Value(read) => Step::PreparedRead {
                        read: Some(read),
                        key: Some(key),
                        resume: Some(resume),
                    },
                    NativeConversion::Throw(reason) => resume
                        .resume(runtime, Completion::Throw(reason))
                        .map_err(runtime_error_to_vm_error)?,
                };
                continue;
            }
            _ => {
                return Ok(Next::Continue);
            }
        }
    }
}

/// Preserve the existing PreparedSet prefix for an actual selected child.
/// The selected progress and parent stay in Step across budget, materialization,
/// Error allocation, and continuation reservation. Return only a real Error
/// publication fact; callers service it after the new reply is armed in Step.
pub(in crate::engine::vm) fn enter_prepared_set_in_state(
    runtime: &Runtime,
    state: &mut crate::engine::heap::runtime::RuntimeState,
    execution: &mut RunningExecution,
    query: &mut Query,
    step: &mut Step,
) -> Result<bool, Error> {
    if !matches!(step, Step::PreparedSetProgress { .. }) {
        return Err(Error::internal(
            "Set child preflight lost selected progress",
        ));
    }
    if !execution
        .frames
        .can_push_with_continuations(query.continuation_depth())
    {
        execution.frames.materialize_in_state(state)?;
        let result = state.new_native_error_from_message(
            &runtime.0.poisoned,
            query.realm,
            crate::engine::api::error::NativeErrorKind::Internal,
            crate::engine::api::error::NativeErrorMessage::from_utf8("stack overflow"),
        );
        if runtime.0.poisoned.get() {
            return Err(runtime_error_to_vm_error(
                crate::engine::api::RuntimeError::Poisoned,
            ));
        }
        let object = result.map_err(runtime_error_to_vm_error)?;
        let mut output = crate::engine::heap::runtime::owned_values::OwnedValueGuard::new(
            state,
            &runtime.0.poisoned,
            crate::engine::value::JsValue::Object(object),
        );
        let (state, output) = output.parts();
        let Step::PreparedSetProgress { progress, resume } = step else {
            unreachable!()
        };
        progress
            .take()
            .expect("selected Set child")
            .retire_in_state(state, &runtime.0.poisoned)
            .map_err(runtime_error_to_vm_error)?;
        *step = Step::SetReply {
            action: Some(crate::engine::object::SetAction::Throw(
                output.take().expect("Set child overflow output"),
            )),
            resume: resume.take(),
        };
        return Ok(true);
    }
    query
        .parents
        .try_reserve(1)
        .map_err(|_| Error::internal("property continuation allocation failed"))?;
    let Step::PreparedSetProgress { progress, resume } = step else {
        unreachable!()
    };
    let progress = progress.take().expect("selected Set child");
    query
        .parents
        .push(resume.take().expect("selected Set parent"));
    *step = Step::SetProgress(Some(progress));
    Ok(false)
}

#[inline(never)]
pub(super) fn set(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    _owner: ReturnOwner,
    _identity: u64,
    query: &mut Query,
    pending: &mut Step,
) -> Result<Next, Error> {
    let step = pending;
    loop {
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_execution_event("dispatch_write.set.visit");
        let realm = query.realm;
        if matches!(step, Step::PreparedSetProgress { .. }) {
            query.ensure_raw_owner_registration(runtime)?;
            let published = {
                let mut state = runtime.0.state.borrow_mut();
                enter_prepared_set_in_state(runtime, &mut state, execution, query, step)?
            };
            runtime.check_poison().map_err(runtime_error_to_vm_error)?;
            if published {
                runtime
                    .collect_if_requested()
                    .map_err(runtime_error_to_vm_error)?;
            }
            continue;
        }
        if matches!(
            step,
            Step::WriteOperands { .. }
                | Step::ValueSet { .. }
                | Step::SetProgress(_)
                | Step::SetReply { .. }
                | Step::WriteError(_)
        ) {
            query.ensure_raw_owner_registration(runtime)?;
            let progress = {
                let mut state = runtime.0.state.borrow_mut();
                query
                    .advance_raw_in_state(runtime, &mut state, step)
                    .map_err(runtime_error_to_vm_error)?
            };
            runtime.check_poison().map_err(runtime_error_to_vm_error)?;
            if progress.cycle_published {
                runtime
                    .collect_if_requested()
                    .map_err(runtime_error_to_vm_error)?;
            }
            match step {
                Step::WriteOperands { .. } => {
                    let mut segment = crate::engine::vm::stack::FrameExecution::admit(
                        execution,
                        _owner.frame()?,
                    )?;
                    let mut state = runtime.0.state.borrow_mut();
                    segment.publish_write_operands_in_state(runtime, &mut state, query, step)?;
                    continue;
                }
                Step::SetProgress(progress) => {
                    let selected = progress.take().expect("selected Set effect");
                    *step = super::request::set::consume_boundary(runtime, query, selected)
                        .map_err(runtime_error_to_vm_error)?;
                    continue;
                }
                Step::SetReply { action, resume } => {
                    let action = action.take().expect("selected Set reply");
                    *step = if let Some(parent) = resume.take() {
                        parent
                            .set(runtime, action.into_boundary(runtime))
                            .map_err(runtime_error_to_vm_error)?
                    } else {
                        let Some(Finish::Write { key, strict, .. }) = query.finish.as_ref() else {
                            action
                                .retire_at_boundary(runtime)
                                .map_err(runtime_error_to_vm_error)?;
                            return Err(Error::internal("Set result has no assignment owner"));
                        };
                        Step::Complete(Some(
                            runtime
                                .finish_property_set(
                                    match action.into_result() {
                                        Ok(result) => result,
                                        Err(action) => {
                                            action
                                                .retire_at_boundary(runtime)
                                                .map_err(runtime_error_to_vm_error)?;
                                            return Err(Error::internal(
                                                "setter result bypassed callback consumer",
                                            ));
                                        }
                                    },
                                    key,
                                    *strict,
                                )
                                .map_err(runtime_error_to_vm_error)?,
                        ))
                    };
                    continue;
                }
                Step::WriteError(error) => {
                    execution.frames.materialize(runtime)?;
                    let completion = super::super::property_driver::throw_error(
                        runtime,
                        realm,
                        error.take().expect("selected write error"),
                    )?;
                    let super::CallStep::Complete(completion) = completion else {
                        return Err(Error::internal("write diagnostic omitted its completion"));
                    };
                    *step = Step::Complete(Some(completion));
                    continue;
                }
                _ => {
                    return super::dispatch_conversion::consume_selected(
                        runtime, execution, _owner, _identity, query, step,
                    );
                }
            }
        }
        match &mut *step {
            Step::PreparedSet {
                step: selected,
                resume,
            } => {
                if !execution
                    .frames
                    .can_push_with_continuations(query.continuation_depth())
                {
                    let Completion::Throw(value) = overflow(runtime, realm)? else {
                        unreachable!()
                    };
                    let mut reply = super::native::NativeStepGuard::new(
                        runtime,
                        Step::Complete(Some(Completion::Throw(value))),
                    );
                    runtime.check_poison().map_err(runtime_error_to_vm_error)?;
                    selected
                        .take()
                        .expect("selected Step field")
                        .release(runtime);
                    runtime.check_poison().map_err(runtime_error_to_vm_error)?;
                    let Step::Complete(completion) = &mut *reply else {
                        unreachable!()
                    };
                    let Some(Completion::Throw(value)) = completion.take() else {
                        unreachable!()
                    };
                    let resume = resume.take().expect("selected Step field");
                    *step = resume
                        .set(
                            runtime,
                            crate::engine::object::operations::PropertySetAction::Throw(value),
                        )
                        .map_err(runtime_error_to_vm_error)?;
                    continue;
                }
                query
                    .parents
                    .try_reserve(1)
                    .map_err(|_| Error::internal("property continuation allocation failed"))?;
                let selected = selected.take().expect("selected Step field");
                let resume = resume.take().expect("selected Step field");
                query.parents.push(resume);
                *step = (*selected).try_into()?;
                continue;
            }
            Step::SetLength { value, resume } => {
                query
                    .parents
                    .try_reserve(1)
                    .map_err(|_| Error::internal("property continuation allocation failed"))?;
                let value = value.take().expect("selected Step field");
                let mut resume = resume.take().expect("selected Step field");
                let initial = resume.take_array_length_initial();
                query.parents.push(Resume::SetLength(resume));
                *step = crate::engine::object::ArrayLengthStep::start_selected(
                    runtime,
                    Some(realm),
                    value,
                    initial,
                )
                .map_err(runtime_error_to_vm_error)?
                .try_into()?;
                continue;
            }
            Step::SetComplete(action) => {
                let action = action.take().expect("selected Step field");

                if let crate::engine::object::operations::PropertySetAction::Call { payload } =
                    action
                {
                    let (setter, receiver, argument) = payload.into_parts();

                    *step = Step::Call {
                        target: Some(DirectCallTarget::Callable(setter)),
                        receiver: Some(receiver),
                        arguments: Some(vec![argument]),
                        resume: Some(Resume::Setter),
                    };
                    continue;
                }
                if let Some(resume) = query.parents.pop() {
                    *step = resume
                        .set(runtime, action)
                        .map_err(runtime_error_to_vm_error)?;
                    continue;
                }
                let Some(Finish::Write { key, strict, .. }) = query.finish.as_ref() else {
                    if let crate::engine::object::operations::PropertySetAction::Throw(value) =
                        action
                    {
                        let _ = runtime.release_jsvalue(value);
                    }
                    return Err(Error::internal("Set result has no assignment owner"));
                };
                *step = Step::Complete(Some(
                    runtime
                        .finish_property_set(
                            super::request::set_result(action)
                                .map_err(runtime_error_to_vm_error)?,
                            key,
                            *strict,
                        )
                        .map_err(runtime_error_to_vm_error)?,
                ));
                continue;
            }
            Step::Set {
                object,
                key,
                value,
                receiver,
                resume,
            } => {
                if !execution
                    .frames
                    .can_push_with_continuations(query.continuation_depth())
                {
                    let Completion::Throw(error) = overflow(runtime, realm)? else {
                        unreachable!()
                    };
                    // Keep the real overflow result and resume armed while
                    // retiring the original value then receiver. Fatal cleanup
                    // wins and never traverses the remaining suffix.
                    let mut reply = super::native::NativeStepGuard::new(
                        runtime,
                        Step::Complete(Some(Completion::Throw(error))),
                    );
                    runtime.check_poison().map_err(runtime_error_to_vm_error)?;
                    runtime
                        .release_jsvalue(value.take().expect("selected Set value"))
                        .map_err(runtime_error_to_vm_error)?;
                    runtime.check_poison().map_err(runtime_error_to_vm_error)?;
                    runtime
                        .release_jsvalue(receiver.take().expect("selected Set receiver"))
                        .map_err(runtime_error_to_vm_error)?;
                    runtime.check_poison().map_err(runtime_error_to_vm_error)?;
                    let Step::Complete(completion) = &mut *reply else {
                        unreachable!()
                    };
                    let Some(Completion::Throw(error)) = completion.take() else {
                        unreachable!()
                    };
                    *step = resume
                        .take()
                        .expect("selected Set parent")
                        .set(
                            runtime,
                            crate::engine::object::operations::PropertySetAction::Throw(error),
                        )
                        .map_err(runtime_error_to_vm_error)?;
                    continue;
                }
                if query.parents.try_reserve(1).is_err() {
                    return Err(Error::internal("property continuation allocation failed"));
                }
                let object = object.take().expect("selected Step field");
                let key = key.take().expect("selected Step field");
                let value = value.take().expect("selected Step field");
                let receiver = receiver.take().expect("selected Step field");
                let resume = resume.take().expect("selected Step field");

                query.parents.push(resume);
                *step = crate::engine::object::SetStep::start(
                    runtime,
                    Some(realm),
                    object,
                    key,
                    value,
                    receiver,
                )
                .map_err(runtime_error_to_vm_error)?
                // The budget check and parent reservation above must precede
                // any storage effects. Only shared no-callback phases advance;
                // selected setters and exotic waits keep their exact state.
                .advance_without_callback(runtime)
                .map_err(runtime_error_to_vm_error)?
                .try_into()?;
                continue;
            }
            Step::SetProxy {
                object,
                key,
                value,
                receiver,
                resume,
            } => {
                if !execution
                    .frames
                    .can_push_with_continuations(query.continuation_depth())
                {
                    let Completion::Throw(error) = overflow(runtime, realm)? else {
                        unreachable!()
                    };
                    // Keep the real overflow result and resume armed while
                    // retiring the original value then receiver. Fatal cleanup
                    // wins and never traverses the remaining suffix.
                    let mut reply = super::native::NativeStepGuard::new(
                        runtime,
                        Step::Complete(Some(Completion::Throw(error))),
                    );
                    runtime.check_poison().map_err(runtime_error_to_vm_error)?;
                    runtime
                        .release_jsvalue(value.take().expect("selected Set value"))
                        .map_err(runtime_error_to_vm_error)?;
                    runtime.check_poison().map_err(runtime_error_to_vm_error)?;
                    runtime
                        .release_jsvalue(receiver.take().expect("selected Set receiver"))
                        .map_err(runtime_error_to_vm_error)?;
                    runtime.check_poison().map_err(runtime_error_to_vm_error)?;
                    let Step::Complete(completion) = &mut *reply else {
                        unreachable!()
                    };
                    let Some(Completion::Throw(error)) = completion.take() else {
                        unreachable!()
                    };
                    *step = resume
                        .take()
                        .expect("selected Set parent")
                        .set(
                            runtime,
                            crate::engine::object::operations::PropertySetAction::Throw(error),
                        )
                        .map_err(runtime_error_to_vm_error)?;
                    continue;
                }
                if query.parents.try_reserve(1).is_err() {
                    return Err(Error::internal("property continuation allocation failed"));
                }
                let object = object.take().expect("selected Step field");
                let key = key.take().expect("selected Step field");
                let value = value.take().expect("selected Step field");
                let receiver = receiver.take().expect("selected Step field");
                let resume = resume.take().expect("selected Step field");

                query.parents.push(resume);
                *step = crate::engine::object::ProxySetStep::start(
                    runtime, realm, object, key, value, receiver,
                )
                .map_err(runtime_error_to_vm_error)?
                .try_into()?;
                continue;
            }
            _ => {
                return Ok(Next::Continue);
            }
        }
    }
}

#[inline(never)]
pub(super) fn define(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    _owner: ReturnOwner,
    _identity: u64,
    query: &mut Query,
    pending: &mut Step,
) -> Result<Next, Error> {
    let step = pending;
    loop {
        #[cfg(feature = "profiling")]
        {
            crate::engine::api::profiling::record_owned_execution_event(
                "dispatch_write.define.visit",
            );
            crate::engine::api::profiling::record_owned_execution_event(match &step {
                Step::Define { .. } => "dispatch_write.define.stage.request",
                Step::DefineOrdinary { .. } => "dispatch_write.define.stage.ordinary",
                Step::Defined { .. } => "dispatch_write.define.stage.reply",
                _ => "dispatch_write.define.stage.leave",
            });
        }
        let realm = query.realm;
        match &mut *step {
            Step::Defined(result) => {
                let resume = query
                    .parents
                    .pop()
                    .ok_or_else(|| Error::internal("Define result has no parent"))?;
                let result = result.take().expect("selected Step field");
                *step = resume
                    .defined(runtime, result)
                    .map_err(runtime_error_to_vm_error)?;
                continue;
            }
            Step::Define {
                object,
                key,
                descriptor,
                resume,
            } => {
                let object = object.take().expect("selected Step field");
                let key = key.take().expect("selected Step field");
                let descriptor = descriptor
                    .take()
                    .expect("selected Step field")
                    .into_owned(runtime)
                    .map_err(runtime_error_to_vm_error)?;

                if runtime
                    .is_proxy_object(&object)
                    .map_err(runtime_error_to_vm_error)?
                {
                    if !execution
                        .frames
                        .can_push_with_continuations(query.continuation_depth())
                    {
                        let Completion::Throw(value) = overflow(runtime, realm)? else {
                            unreachable!()
                        };
                        let resume = resume.take().expect("selected Step field");
                        *step = resume
                            .defined(runtime, NativeConversion::Throw(value))
                            .map_err(runtime_error_to_vm_error)?;
                        continue;
                    }
                    query
                        .parents
                        .try_reserve(1)
                        .map_err(|_| Error::internal("property continuation allocation failed"))?;
                    query
                        .parents
                        .push(resume.take().expect("selected Step field"));
                    *step = crate::engine::object::ProxyDefineStep::start(
                        runtime, realm, object, key, descriptor,
                    )
                    .map_err(runtime_error_to_vm_error)?
                    .try_into()?;
                    continue;
                }
                *step = Step::DefineOrdinary {
                    object: Some(object),
                    key: Some(key),
                    descriptor: Some(descriptor.into()),
                    resume: Some(resume.take().expect("selected Step field")),
                };
                continue;
            }
            Step::DefineOrdinary {
                object,
                key,
                descriptor,
                resume,
            } => {
                let object = object.take().expect("selected Step field");
                let key = key.take().expect("selected Step field");
                let descriptor = descriptor
                    .take()
                    .expect("selected Step field")
                    .into_owned(runtime)
                    .map_err(runtime_error_to_vm_error)?;

                if let Some(length) = runtime
                    .prepare_array_length_definition_owned(Some(realm), &object, &key, &descriptor)
                    .map_err(runtime_error_to_vm_error)?
                {
                    if query.parents.try_reserve(1).is_err() {
                        let mut abandoned: Step = length.try_into()?;
                        abandoned.release_owned(runtime);
                        return Err(Error::internal("property continuation allocation failed"));
                    }
                    query.parents.push(Resume::DefineLength {
                        payload: Box::new(super::request::DefineLengthPayload {
                            object,
                            key,
                            descriptor,
                            resume: Box::new(resume.take().expect("selected Step field")),
                        }),
                    });
                    *step = length.try_into()?;
                    continue;
                }
                if let Some(request) = runtime
                    .prepare_typed_array_definition_owned(&object, &key, &descriptor)
                    .map_err(runtime_error_to_vm_error)?
                {
                    if query.parents.try_reserve(1).is_err() {
                        let mut abandoned: Step = request.try_into()?;
                        abandoned.release_owned(runtime);
                        return Err(Error::internal("property continuation allocation failed"));
                    }
                    query.parents.push(Resume::DefineTyped {
                        payload: Box::new(super::request::DefineTypedPayload {
                            object,
                            _descriptor: descriptor,
                            resume: Box::new(resume.take().expect("selected Step field")),
                        }),
                    });
                    *step = request.try_into()?;
                    continue;
                }
                let accepted = runtime
                    .define_owned_property_after_conversion_selection(&object, &key, &descriptor)
                    .map_err(runtime_error_to_vm_error)?;
                let result = NativeConversion::Value(if accepted {
                    crate::engine::object::operations::InternalDefineResult::Defined
                } else {
                    crate::engine::object::operations::InternalDefineResult::RejectedOrdinary(
                        object,
                    )
                });
                let resume = resume.take().expect("selected Step field");
                *step = resume
                    .defined(runtime, result)
                    .map_err(runtime_error_to_vm_error)?;
                continue;
            }
            _ => {
                return Ok(Next::Continue);
            }
        }
    }
}

#[cfg(test)]
mod local_set_tests {
    use super::super::Parents;
    use super::*;
    use crate::engine::api::Value;
    use crate::engine::value::JsValue;
    use crate::engine::vm::execution::ExecutionLimits;

    #[test]
    fn local_set_dispatch_budget_failure_precedes_array_write() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        let Value::Object(array) = context.eval("[]").unwrap() else {
            panic!("expected array");
        };
        let key = runtime.property_key_for_index(0).unwrap();
        let mut query = Query {
            native_runtime: std::rc::Weak::new(),
            #[cfg(feature = "profiling")]
            had_callback: false,
            #[cfg(feature = "profiling")]
            transport: crate::engine::vm::proxy_get_driver::transport::QueryTransport::default(),
            realm: context.realm,
            parents: Parents::default(),
            natives: Vec::new(),
            saved_native_depth: 0,
            spare_parents: Vec::new(),
            finish: None,
        };
        let mut pending = Step::Set {
            object: Some(array.try_clone().expect("duplicate root")),
            key: Some(key.try_clone().expect("duplicate root")),
            value: Some(JsValue::Int(7)),
            receiver: Some(
                runtime
                    .unroot_value(&Value::Object(array.try_clone().expect("duplicate root")))
                    .unwrap(),
            ),
            resume: Some(Resume::RootSet),
        };
        let mut execution = RunningExecution::new(
            &runtime,
            ExecutionLimits {
                frames: 0,
                slots: 0,
            },
        )
        .unwrap();
        assert!(matches!(
            set(
                &runtime,
                &mut execution,
                ReturnOwner::Root,
                1,
                &mut query,
                &mut pending
            )
            .unwrap(),
            Next::Continue
        ));
        let Step::Complete(Some(Completion::Throw(value))) = pending else {
            panic!("expected an abrupt local set");
        };
        runtime.release_jsvalue(value).unwrap();
        assert!(query.parents.is_empty());
        assert!(runtime.get_own_property(&array, &key).unwrap().is_none());
        assert_eq!(
            runtime.array_length_state_if_genuine(&array).unwrap(),
            Some((0, true))
        );
    }

    #[test]
    fn legacy_set_budget_failure_precedes_selected_callee_maximum() {
        use crate::engine::{
            api::error::NativeErrorKind,
            heap::RawId,
            object::{AccessorValue, DescriptorField, OrdinaryPropertyDescriptor},
        };
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        let Value::Object(setter) = context.eval("(function(v){throw 'must not run'})").unwrap()
        else {
            panic!("setter fixture")
        };
        let object = runtime.new_object(None).unwrap();
        let value = runtime.new_object(None).unwrap();
        let key = runtime.intern_property_key("x").unwrap();
        runtime
            .define_own_property(
                &object,
                &key,
                &OrdinaryPropertyDescriptor {
                    set: DescriptorField::Present(AccessorValue::Callable(
                        runtime.as_callable(&setter).unwrap().unwrap(),
                    )),
                    configurable: DescriptorField::Present(true),
                    ..OrdinaryPropertyDescriptor::new()
                },
            )
            .unwrap();
        let expected = runtime
            .0
            .state
            .borrow()
            .heap
            .context(context.realm)
            .unwrap()
            .native_error_prototypes[NativeErrorKind::Internal.index()]
        .unwrap();
        let mut query = Query {
            native_runtime: std::rc::Weak::new(),
            #[cfg(feature = "profiling")]
            had_callback: false,
            #[cfg(feature = "profiling")]
            transport: crate::engine::vm::proxy_get_driver::transport::QueryTransport::default(),
            realm: context.realm,
            parents: Parents::default(),
            natives: Vec::new(),
            saved_native_depth: 0,
            spare_parents: Vec::new(),
            finish: None,
        };
        let mut pending = Step::Set {
            object: Some(object.try_clone().unwrap()),
            key: Some(key.try_clone().unwrap()),
            value: Some(
                runtime
                    .dup_jsvalue(&JsValue::Object(value.object_id()))
                    .unwrap(),
            ),
            receiver: Some(
                runtime
                    .dup_jsvalue(&JsValue::Object(object.object_id()))
                    .unwrap(),
            ),
            resume: Some(Resume::RootSet),
        };
        let mut execution = RunningExecution::new(
            &runtime,
            ExecutionLimits {
                frames: 0,
                slots: 0,
            },
        )
        .unwrap();
        let count = runtime
            .0
            .state
            .borrow()
            .heap
            .object_strong_count(setter.object_id())
            .unwrap();
        runtime
            .0
            .state
            .borrow_mut()
            .heap
            .set_strong_count_for_test(RawId::Object(setter.object_id()), u32::MAX);
        let result = set(
            &runtime,
            &mut execution,
            ReturnOwner::Root,
            1,
            &mut query,
            &mut pending,
        );
        let maximum = runtime
            .0
            .state
            .borrow()
            .heap
            .object_strong_count(setter.object_id());
        // Restore before assertions can unwind through real accessor owners.
        runtime
            .0
            .state
            .borrow_mut()
            .heap
            .set_strong_count_for_test(RawId::Object(setter.object_id()), count);
        assert!(matches!(result.unwrap(), Next::Continue));
        let Step::Complete(Some(Completion::Throw(error))) = &mut pending else {
            panic!("budget must win the competing callee refusal")
        };
        let error = runtime.root_value(error).unwrap();
        let Value::Object(error) = error else {
            panic!("actual overflow Error")
        };
        assert_eq!(
            runtime
                .get_prototype_of(&error)
                .unwrap()
                .unwrap()
                .object_id(),
            expected
        );
        assert_eq!(maximum, Ok(u32::MAX));
        assert_eq!(
            runtime
                .0
                .state
                .borrow()
                .heap
                .object_strong_count(value.object_id()),
            Ok(1)
        );
        assert_eq!(
            runtime
                .0
                .state
                .borrow()
                .heap
                .object_strong_count(object.object_id()),
            Ok(1)
        );
        assert!(query.parents.is_empty());
        assert_eq!(runtime.0.active_frame_depth.get(), 0);
        assert!(!runtime.is_poisoned());
        pending.release_owned(&runtime);
    }

    #[test]
    fn prepared_raw_set_budget_retires_selected_inputs_before_parent_reply() {
        use crate::engine::object::{
            AccessorValue, DescriptorField, OrdinaryPropertyDescriptor, SetAction, SetProgress,
        };
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        let Value::Object(function) = context.eval("(function(v){throw 'must not run'})").unwrap()
        else {
            panic!("actual selected setter")
        };
        let receiver = runtime.new_object(None).unwrap();
        let argument = runtime.new_object(None).unwrap();
        let key = runtime.intern_property_key("x").unwrap();
        runtime
            .define_own_property(
                &receiver,
                &key,
                &OrdinaryPropertyDescriptor {
                    set: DescriptorField::Present(AccessorValue::Callable(
                        runtime.as_callable(&function).unwrap().unwrap(),
                    )),
                    ..OrdinaryPropertyDescriptor::new()
                },
            )
            .unwrap();
        let input = runtime
            .dup_jsvalue(&JsValue::Object(argument.object_id()))
            .unwrap();
        let this = runtime
            .dup_jsvalue(&JsValue::Object(receiver.object_id()))
            .unwrap();
        let mut query = Query {
            native_runtime: std::rc::Weak::new(),
            #[cfg(feature = "profiling")]
            had_callback: false,
            #[cfg(feature = "profiling")]
            transport: crate::engine::vm::proxy_get_driver::transport::QueryTransport::default(),
            realm: context.realm,
            parents: Parents::default(),
            natives: Vec::new(),
            saved_native_depth: 0,
            spare_parents: Vec::new(),
            finish: None,
        };
        let mut execution = RunningExecution::new(
            &runtime,
            ExecutionLimits {
                frames: 0,
                slots: 0,
            },
        )
        .unwrap();
        let mut state = runtime.0.state.borrow_mut();
        let selected = state
            .start_set_borrowed(
                &runtime.0.poisoned,
                Some(context.realm),
                receiver.object_id(),
                key.atom(),
                input,
                this,
            )
            .unwrap();
        assert!(matches!(
            selected,
            SetProgress::Complete(SetAction::Call { .. })
        ));
        let count = state
            .heap
            .object_strong_count(function.object_id())
            .unwrap();
        assert_eq!(
            count, 3,
            "public, stored accessor, actual selected callback owners"
        );
        let mut step = Step::PreparedSetProgress {
            progress: Some(selected),
            resume: Some(Resume::RootSet),
        };
        let published = enter_prepared_set_in_state(
            &runtime,
            &mut state,
            &mut execution,
            &mut query,
            &mut step,
        )
        .unwrap();
        assert!(published);
        assert!(matches!(
            step,
            Step::SetReply {
                action: Some(SetAction::Throw(_)),
                ..
            }
        ));
        assert_eq!(
            state.heap.object_strong_count(function.object_id()),
            Ok(count - 1)
        );
        assert_eq!(state.heap.object_strong_count(receiver.object_id()), Ok(1));
        assert_eq!(state.heap.object_strong_count(argument.object_id()), Ok(1));
        assert!(query.parents.is_empty());
        assert!(!runtime.is_poisoned());
        step.retire_raw_in_state(&mut state, &runtime.0.poisoned)
            .unwrap();
    }

    #[test]
    fn local_set_dispatch_keeps_setters_proxy_and_array_length_order() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        assert_eq!(
            context
                .eval(
                    r#"(function () {
            var setterCalls = 0, setterLength = -1, stored;
            var proto = Object.create(Array.prototype);
            Object.defineProperty(proto, '0', { set: function (value) {
                setterCalls++; setterLength = this.length; stored = value;
            }});
            var a = []; Object.setPrototypeOf(a, proto);
            if (a.push(7) !== 1 || setterCalls !== 1 || setterLength !== 0 ||
                stored !== 7 || Object.hasOwn(a, '0')) return false;
            var trace = [], target = [];
            var proxy = new Proxy(target, {set: function (t, k, v, r) {
                trace.push(k + ':' + t.length); return Reflect.set(t, k, v, r);
            }});
            if (Array.prototype.push.call(proxy, 9) !== 1 || target[0] !== 9 ||
                trace.join(',') !== '0:0,length:1') return false;
            var b = [];
            Object.defineProperty(b, 'length', {writable: false});
            var rejected = false;
            try { b.push(1); } catch (e) { rejected = e instanceof TypeError; }
            if (!rejected || b.length !== 0 || Object.hasOwn(b, '0')) return false;
            var order = '', length = 0, receiver = {
                get length() { order += 'g'; return length; },
                set length(v) { order += 'l'; length = v; },
                set 0(v) { order += 'a'; },
                set 1(v) { order += 'b'; throw 23; }
            };
            var failure;
            try { Array.prototype.push.call(receiver, 1, 2); } catch(e) { failure = e; }
            return failure === 23 && order === 'gab' && length === 0;
        })()"#
                )
                .unwrap(),
            Value::Bool(true)
        );
        assert!(runtime.0.state.borrow().active_frames.is_empty());
    }
}
