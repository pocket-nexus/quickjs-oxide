//! Bounded native-stack dispatch for write requests.
use super::{
    Completion, DirectCallTarget, Error, Finish, NativeConversion, Next, Query, Resume,
    ReturnOwner, RunningExecution, Runtime, Step, overflow, request, runtime_error_to_vm_error,
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
    let mut step = pending.take();
    loop {
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_execution_event("dispatch_write.keys.visit");
        let realm = query.realm;
        match step {
            Step::SnapshotEnumerable {
                object,
                key,
                resume,
            } => {
                if runtime
                    .is_proxy_object(&object)
                    .map_err(runtime_error_to_vm_error)?
                {
                    step = Step::Descriptor {
                        object,
                        key,
                        resume: Resume::OwnFlagReply {
                            enumerable: true,
                            resume: Box::new(resume),
                        },
                    };
                } else {
                    let result = runtime
                        .internal_snapshot_own_property_is_enumerable(realm, &object, &key)
                        .map_err(runtime_error_to_vm_error)?;
                    step = resume
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
                if runtime
                    .is_proxy_object(&object)
                    .map_err(runtime_error_to_vm_error)?
                {
                    step = Step::Descriptor {
                        object,
                        key,
                        resume: Resume::OwnFlagReply {
                            enumerable,
                            resume: Box::new(resume),
                        },
                    };
                } else {
                    let result = if enumerable {
                        runtime.internal_own_property_is_enumerable(realm, &object, &key)
                    } else {
                        runtime.internal_has_own_property(realm, &object, &key)
                    }
                    .map_err(runtime_error_to_vm_error)?;
                    step = resume
                        .boolean(runtime, result)
                        .map_err(runtime_error_to_vm_error)?;
                }
                continue;
            }
            Step::Keys { object, resume } => {
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
                        step = resume
                            .keys(runtime, NativeConversion::Throw(value))
                            .map_err(runtime_error_to_vm_error)?;
                        continue;
                    }
                    query
                        .parents
                        .try_reserve(1)
                        .map_err(|_| Error::internal("ownKeys continuation allocation failed"))?;
                    query.parents.push(resume);
                    step = crate::engine::object::KeysStep::start(runtime, realm, object)
                        .map_err(runtime_error_to_vm_error)?
                        .into();
                } else {
                    let result = runtime
                        .own_property_keys(&object)
                        .map_err(runtime_error_to_vm_error)?;
                    step = resume
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
                step = resume
                    .keys(runtime, result)
                    .map_err(runtime_error_to_vm_error)?;
                continue;
            }
            Step::ReadValue {
                receiver,
                key,
                resume,
            } => {
                step = match runtime
                    .prepare_value_property_read_completion(realm, receiver, &key)
                    .map_err(runtime_error_to_vm_error)?
                {
                    NativeConversion::Value(read) => Step::PreparedRead { read, key, resume },
                    NativeConversion::Throw(reason) => resume
                        .resume(runtime, Completion::Throw(reason))
                        .map_err(runtime_error_to_vm_error)?,
                };
                continue;
            }
            next => {
                *pending = next;
                return Ok(Next::Continue);
            }
        }
    }
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
    let mut step = pending.take();
    loop {
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_execution_event("dispatch_write.set.visit");
        let realm = query.realm;
        match step {
            Step::SetContinue(resume) => {
                step = resume
                    .advance(runtime)
                    .map_err(runtime_error_to_vm_error)?
                    .into();
                continue;
            }
            Step::SetLength { value, resume } => {
                query
                    .parents
                    .try_reserve(1)
                    .map_err(|_| Error::internal("property continuation allocation failed"))?;
                query.parents.push(Resume::SetLength(resume));
                step = crate::engine::object::ArrayLengthStep::start(runtime, Some(realm), value)
                    .map_err(runtime_error_to_vm_error)?
                    .into();
                continue;
            }
            Step::SetSpecial {
                object,
                key,
                value,
                receiver,
                resume,
            } => {
                match runtime
                    .prepare_typed_array_set(&object, &key, &value, &receiver)
                    .map_err(runtime_error_to_vm_error)?
                {
                    None => {
                        step = resume
                            .special(runtime, None)
                            .map_err(runtime_error_to_vm_error)?
                            .into()
                    }
                    Some(request) => {
                        query.parents.try_reserve(1).map_err(|_| {
                            Error::internal("property continuation allocation failed")
                        })?;
                        query.parents.push(Resume::SetTyped(resume));
                        step = request.into();
                    }
                }
                continue;
            }
            Step::SetComplete(action) => {
                if let crate::engine::object::operations::PropertySetAction::Call {
                    setter,
                    receiver,
                    argument,
                } = action
                {
                    step = Step::Call {
                        target: DirectCallTarget::Callable(setter),
                        receiver,
                        arguments: vec![argument],
                        resume: Resume::Setter,
                    };
                    continue;
                }
                if let Some(resume) = query.parents.pop() {
                    step = resume
                        .set(runtime, action)
                        .map_err(runtime_error_to_vm_error)?;
                    continue;
                }
                let Some(Finish::Write { key, strict, .. }) = query.finish.as_ref() else {
                    return Err(Error::internal("Set result has no assignment owner"));
                };
                step = Step::Complete(
                    runtime
                        .finish_property_set(
                            request::set_result(action).map_err(runtime_error_to_vm_error)?,
                            key,
                            *strict,
                        )
                        .map_err(runtime_error_to_vm_error)?,
                );
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
                    let Completion::Throw(value) = overflow(runtime, realm)? else {
                        unreachable!()
                    };
                    step = resume
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
                query.parents.push(resume);
                step = crate::engine::object::SetStep::start(
                    runtime,
                    Some(realm),
                    object,
                    key,
                    value,
                    receiver,
                )
                .map_err(runtime_error_to_vm_error)?
                .into();
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
                    let Completion::Throw(value) = overflow(runtime, realm)? else {
                        unreachable!()
                    };
                    step = resume
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
                query.parents.push(resume);
                step = crate::engine::object::ProxySetStep::start(
                    runtime, realm, object, key, value, receiver,
                )
                .map_err(runtime_error_to_vm_error)?
                .into();
                continue;
            }
            next => {
                *pending = next;
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
    let mut step = pending.take();
    loop {
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_execution_event("dispatch_write.define.visit");
        let realm = query.realm;
        match step {
            Step::Defined(result) => {
                let resume = query
                    .parents
                    .pop()
                    .ok_or_else(|| Error::internal("Define result has no parent"))?;
                step = resume
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
                        step = resume
                            .defined(runtime, NativeConversion::Throw(value))
                            .map_err(runtime_error_to_vm_error)?;
                        continue;
                    }
                    query
                        .parents
                        .try_reserve(1)
                        .map_err(|_| Error::internal("property continuation allocation failed"))?;
                    query.parents.push(resume);
                    step = crate::engine::object::ProxyDefineStep::start(
                        runtime, realm, object, key, descriptor,
                    )
                    .map_err(runtime_error_to_vm_error)?
                    .into();
                    continue;
                }
                step = Step::DefineOrdinary {
                    object,
                    key,
                    descriptor,
                    resume,
                };
                continue;
            }
            Step::DefineOrdinary {
                object,
                key,
                descriptor,
                resume,
            } => {
                if let Some(length) = runtime
                    .prepare_array_length_definition(Some(realm), &object, &key, &descriptor)
                    .map_err(runtime_error_to_vm_error)?
                {
                    query
                        .parents
                        .try_reserve(1)
                        .map_err(|_| Error::internal("property continuation allocation failed"))?;
                    query.parents.push(Resume::DefineLength {
                        object,
                        key,
                        descriptor,
                        resume: Box::new(resume),
                    });
                    step = length.into();
                    continue;
                }
                if let Some(request) = runtime
                    .prepare_typed_array_definition(&object, &key, &descriptor)
                    .map_err(runtime_error_to_vm_error)?
                {
                    query
                        .parents
                        .try_reserve(1)
                        .map_err(|_| Error::internal("property continuation allocation failed"))?;
                    query.parents.push(Resume::DefineTyped {
                        object,
                        _descriptor: descriptor,
                        resume: Box::new(resume),
                    });
                    step = request.into();
                    continue;
                }
                let result = match runtime
                        .define_own_property_in_realm(Some(realm), &object, &key, &descriptor)
                        .map_err(runtime_error_to_vm_error)? {
                        crate::engine::object::operations::PropertyDefineOutcome::Defined(true) => NativeConversion::Value(crate::engine::object::operations::InternalDefineResult::Defined),
                        crate::engine::object::operations::PropertyDefineOutcome::Defined(false) => NativeConversion::Value(crate::engine::object::operations::InternalDefineResult::RejectedOrdinary(object)),
                        crate::engine::object::operations::PropertyDefineOutcome::Throw(value) => NativeConversion::Throw(value),
                    };
                step = resume
                    .defined(runtime, result)
                    .map_err(runtime_error_to_vm_error)?;
                continue;
            }
            next => {
                *pending = next;
                return Ok(Next::Continue);
            }
        }
    }
}
