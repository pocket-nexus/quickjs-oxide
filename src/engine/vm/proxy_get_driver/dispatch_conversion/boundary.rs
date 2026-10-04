//! Consume selected raw effects after the State lease ends. No lookup,
//! authentication, ABI publication or semantic conversion is replayed here.
use super::super::{
    DirectCallTarget, Error, JsValue, Next, Query, Resume, ReturnOwner, RunningExecution, Runtime,
    Step, native, runtime_error_to_vm_error,
};
use crate::engine::{
    object::{CallableRef, ObjectRef, OwnedRead, PropertyKey, ReadStep},
    vm::call::ordinary::CallbackSelection,
};

pub(super) fn consume(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    owner: ReturnOwner,
    identity: u64,
    query: &mut Query,
    pending: &mut Step,
) -> Result<Next, Error> {
    match pending {
        Step::ComputedError(error) => {
            let result = super::super::super::property_driver::throw_error(
                runtime,
                query.realm,
                error.take().expect("selected computed error"),
            )?;
            let super::super::CallStep::Complete(completion) = result else {
                return Err(Error::internal("computed diagnostic attempted a callback"));
            };
            *pending = Step::Complete(Some(completion));
            if let Some(input) = query.computed_read_mut() {
                input.cycle_published = true;
            }
            runtime.check_poison().map_err(runtime_error_to_vm_error)?;
        }
        Step::StringReply { value, resume } => {
            let next = resume
                .take()
                .expect("string reply parent")
                .string(runtime, value.take().expect("string reply"))
                .map_err(runtime_error_to_vm_error)?;
            *pending = next;
        }
        Step::NumberReply { value, resume } => {
            let next = resume
                .take()
                .expect("number reply parent")
                .number(runtime, value.take().expect("number reply"))
                .map_err(runtime_error_to_vm_error)?;
            *pending = next;
        }
        Step::PrimitiveReply { value, resume } => {
            let next = resume
                .take()
                .expect("primitive reply parent")
                .resume(runtime, value.take().expect("primitive reply"))
                .map_err(runtime_error_to_vm_error)?;
            *pending = next;
        }
        Step::RawRead { read, key, resume } => {
            if matches!(read.as_ref(), Some(ReadStep::Shared(_))) {
                // This is the actual selected Arc and bounds, outside State.
                let Some(ReadStep::Shared(word)) = read.take() else {
                    unreachable!()
                };
                let word = word.read().map_err(runtime_error_to_vm_error)?;
                let owned = runtime
                    .0
                    .state
                    .borrow_mut()
                    .own_typed_read_word(word)
                    .map_err(runtime_error_to_vm_error)?;
                *read = Some(ReadStep::Ready(owned));
            } else {
                if !matches!(
                    read.as_ref(),
                    Some(
                        ReadStep::Ready(OwnedRead::Proxy { .. })
                            | ReadStep::CyclePublished(OwnedRead::Proxy { .. })
                    )
                ) {
                    return Err(Error::internal("raw read boundary lost its selected Proxy"));
                }
                // Raw primitive protocols use domain-held intrinsic atoms. Take
                // the PropertyKey's independent role before moving the read.
                let atom = if query.computed_read_mut().is_some_and(|input| {
                    input.phase == super::super::computed::ComputedPhase::Proxy
                }) {
                    let input = query.computed_read_mut().expect("computed Proxy context");
                    if input.atom != Some(*key) {
                        return Err(Error::internal("computed Proxy key changed"));
                    }
                    input.atom.take().expect("computed Proxy atom owner")
                } else {
                    runtime
                        .retain_atom_handle(*key)
                        .map_err(|error| runtime_error_to_vm_error(error.into()))?;
                    *key
                };
                let key = PropertyKey::from_owned_atom(runtime.clone(), atom);
                let selected = match read.take().expect("selected Proxy read") {
                    ReadStep::Ready(read) | ReadStep::CyclePublished(read) => read,
                    ReadStep::Shared(_) => unreachable!(),
                };
                *pending = Step::PreparedRead {
                    read: Some(runtime.adopt_prepared_read(selected)),
                    key: Some(key),
                    resume: resume.take(),
                };
                query.publish_selected_prefix_at_boundary(runtime, execution, owner)?;
            }
        }
        Step::RawCall { inputs, resume } => {
            let inputs = inputs.as_mut().expect("raw callback inputs");
            if inputs.callback_callee.is_some() || inputs.preserved_receiver.is_some() {
                return Err(Error::internal("unselected callback has extra input roles"));
            }
            let callable = CallableRef::from_validated_object(ObjectRef::from_owned_handle(
                runtime.clone(),
                inputs.selected_callee.take().expect("raw callback target"),
            ));
            *pending = Step::Call {
                target: Some(DirectCallTarget::Callable(callable)),
                receiver: inputs.receiver.take(),
                arguments: Some(std::mem::take(&mut inputs.arguments)),
                resume: resume.take(),
            };
            return Ok(Next::Invoke);
        }
        Step::CallbackBoundary(packet) => {
            let packet = packet.as_mut().expect("selected callback boundary");
            if packet.overflow {
                // Consume the rejection selected in the resident frame. Cold
                // host-stack depth must not change that admission decision.
                let ordinary = matches!(packet.selection, CallbackSelection::Ordinary(_));
                {
                    let mut state = runtime.0.state.borrow_mut();
                    if ordinary {
                        if let Some(value) = packet.inputs.receiver.take() {
                            state
                                .release_owned_jsvalue(&runtime.0.poisoned, value)
                                .map_err(runtime_error_to_vm_error)?;
                        }
                    }
                    for value in &mut packet.inputs.arguments {
                        state
                            .release_owned_jsvalue(
                                &runtime.0.poisoned,
                                std::mem::replace(value, JsValue::Undefined),
                            )
                            .map_err(runtime_error_to_vm_error)?;
                    }
                    packet.inputs.arguments.clear();
                    if let Some(value) = packet.inputs.receiver.take() {
                        state
                            .release_owned_jsvalue(&runtime.0.poisoned, value)
                            .map_err(runtime_error_to_vm_error)?;
                    }
                }
                let completion = match &packet.selection {
                    CallbackSelection::Ordinary(call) => {
                        call.executable()
                            .ensure_root(runtime)
                            .map_err(runtime_error_to_vm_error)?;
                        runtime
                            .bytecode_stack_overflow_completion(
                                query.realm,
                                call.executable().root().expect("rooted callback overflow"),
                            )
                            .map_err(runtime_error_to_vm_error)?
                    }
                    CallbackSelection::Native(_) => super::super::overflow(runtime, query.realm)?,
                    CallbackSelection::General => {
                        return Err(Error::internal("general callback overflow lost admission"));
                    }
                };
                let mut reply = native::NativeStepGuard::new(
                    runtime,
                    Step::PrimitiveReply {
                        value: Some(completion),
                        resume: None,
                    },
                );
                packet
                    .inputs
                    .retire_at_boundary(runtime)
                    .map_err(runtime_error_to_vm_error)?;
                let Step::PrimitiveReply { resume, .. } = &mut *reply else {
                    unreachable!()
                };
                *resume = Some(std::mem::replace(&mut packet.resume, Resume::Identity));
                *pending = reply.into_inner();
            } else {
                if matches!(packet.selection, CallbackSelection::General)
                    && query.final_getter_pending()
                {
                    query
                        .computed_read_mut()
                        .expect("computed getter context")
                        .base = packet.inputs.preserved_receiver.take();
                }
                if packet.inputs.callback_callee.is_some()
                    || packet.inputs.preserved_receiver.is_some()
                {
                    return Err(Error::internal(
                        "selected callback has unconsumed input roles",
                    ));
                }
                match std::mem::replace(&mut packet.selection, CallbackSelection::General) {
                    CallbackSelection::Native(selected) => {
                        let target = selected.target();
                        let defining_realm = selected.defining_realm();
                        let minimum = selected.minimum();
                        let callable =
                            CallableRef::from_validated_object(ObjectRef::from_owned_handle(
                                runtime.clone(),
                                packet
                                    .inputs
                                    .selected_callee
                                    .take()
                                    .expect("selected native target"),
                            ));
                        let receiver = packet
                            .inputs
                            .receiver
                            .take()
                            .expect("selected native receiver");
                        let arguments = std::mem::take(&mut packet.inputs.arguments);
                        let resume = std::mem::replace(&mut packet.resume, Resume::Identity);
                        let mut output =
                            native::NativeStepGuard::new(runtime, Step::Complete(None));
                        native::start_selected_into(
                            runtime,
                            execution,
                            query,
                            callable,
                            target,
                            defining_realm,
                            minimum,
                            crate::engine::vm::call::NativeInvokeMode::Ordinary,
                            crate::engine::vm::call::NativeInvocation::Call {
                                this_value: receiver,
                            },
                            arguments,
                            resume,
                            &mut output,
                            Some(selected),
                        )?;
                        *pending = output.into_inner();
                    }
                    CallbackSelection::General => {
                        let callable =
                            CallableRef::from_validated_object(ObjectRef::from_owned_handle(
                                runtime.clone(),
                                packet
                                    .inputs
                                    .selected_callee
                                    .take()
                                    .expect("selected general target"),
                            ));
                        *pending = Step::Call {
                            target: Some(DirectCallTarget::Callable(callable)),
                            receiver: packet.inputs.receiver.take(),
                            arguments: Some(std::mem::take(&mut packet.inputs.arguments)),
                            resume: Some(std::mem::replace(&mut packet.resume, Resume::Identity)),
                        };
                        return super::super::invoke_general_callback(
                            runtime, execution, owner, identity, query, pending,
                        );
                    }
                    CallbackSelection::Ordinary(_) => {
                        return Err(Error::internal(
                            "ordinary callback escaped without overflow",
                        ));
                    }
                }
            }
        }
        Step::PreparedNativeBoundary(_) => {
            native::start_published_boundary(runtime, execution, query, pending)?;
        }
        _ => return Ok(Next::Continue),
    }
    Ok(Next::Continue)
}
