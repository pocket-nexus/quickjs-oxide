//! Canonical raw conversion phases. Only selected effects leave the State lease.
use super::{Completion, Query, Resume, Runtime, Step};
use crate::engine::{
    api::runtime_error::RuntimeError,
    heap::runtime::{RuntimeState, owned_values::OwnedValueGuard},
    object::{OwnedRead, ReadStep},
    value::{
        JsValue,
        conversion::{
            NativeConversion,
            number::NumberStep,
            primitive::{PrimitiveResume, PrimitiveStep},
        },
    },
    vm::call::ordinary::RawCallbackInputs,
};

/// A callback still owns its inputs in Step::RawCall. Boundary likewise leaves
/// the actual selected read/reply in Step, so no producer or lookup is replayed.
pub(in crate::engine::vm) enum StateEffect {
    Complete,
    Callback,
    PropertyRead,
    Diagnostic,
    Boundary,
}
pub(in crate::engine::vm) struct StateProgress {
    pub(in crate::engine::vm) effect: StateEffect,
    pub(in crate::engine::vm) cycle_published: bool,
}

impl Query {
    /// All Query consumers use these semantic transitions. The caller supplies
    /// one admitted lease and keeps the request armed across every fallible phase.
    pub(in crate::engine::vm) fn advance_raw_in_state(
        &mut self,
        runtime: &Runtime,
        state: &mut RuntimeState,
        step: &mut Step,
    ) -> Result<StateProgress, RuntimeError> {
        let poisoned = &runtime.0.poisoned;
        let mut cycle_published = false;
        loop {
            match step {
                Step::ComputedError(_) => {
                    // Frame publication belongs to the executor. Keep the
                    // selected diagnostic armed until it supplies that fact.
                    return Ok(StateProgress {
                        effect: StateEffect::Diagnostic,
                        cycle_published,
                    });
                }
                Step::CyclePublishedStringReply { value, resume } => {
                    cycle_published = true;
                    *step = Step::StringReply {
                        value: value.take(),
                        resume: resume.take(),
                    };
                }
                Step::Arguments { value, resume } => {
                    let next = crate::engine::builtins::ArgumentsStep::start_in_state(
                        state,
                        poisoned,
                        self.realm,
                        value.take().expect("argument-list input"),
                    )?;
                    cycle_published |= matches!(
                        next,
                        crate::engine::builtins::ArgumentsStep::CyclePublished(_)
                    );
                    match next {
                        crate::engine::builtins::ArgumentsStep::Complete(result)
                        | crate::engine::builtins::ArgumentsStep::CyclePublished(result) => {
                            // The fresh fact is read before consuming the enum.
                            *step = Step::ArgumentsReply {
                                value: Some(result),
                                resume: resume.take(),
                            };
                        }
                        next => {
                            if self.parents.try_reserve(1).is_err() {
                                next.retire_in_state(state, poisoned)?;
                                return Err(RuntimeError::Invariant(
                                    "argument continuation allocation failed",
                                ));
                            }
                            self.parents
                                .push(resume.take().expect("argument-list parent"));
                            *step = Step::ArgumentsProgress(Some(next));
                        }
                    }
                }
                Step::ArgumentsReply { value, resume } => {
                    if !resume
                        .as_ref()
                        .expect("argument reply parent")
                        .can_arguments_in_state()
                    {
                        break;
                    }
                    let parent = resume.take().expect("argument reply parent");
                    *step = parent.arguments_in_state(
                        state,
                        poisoned,
                        value.take().expect("argument reply"),
                    )?;
                }
                Step::ArgumentsComplete(value) => {
                    if !self
                        .parents
                        .0
                        .last()
                        .is_some_and(Resume::can_arguments_in_state)
                    {
                        break;
                    }
                    let parent = self.parents.pop().expect("argument-list parent");
                    *step = parent.arguments_in_state(
                        state,
                        poisoned,
                        value.take().expect("argument reply"),
                    )?;
                }
                Step::InvokeProgress(progress) => {
                    use crate::engine::builtins::{InvokeCallTarget, InvokeStep};
                    match progress.as_ref().expect("Invoke progress") {
                        InvokeStep::Construct(_) => break,
                        InvokeStep::Call(request)
                            if matches!(request.target, InvokeCallTarget::NonCallableProxy(_)) =>
                        {
                            break;
                        }
                        _ => {}
                    }
                    match progress.take().expect("Invoke progress") {
                        InvokeStep::Complete(value) => *step = Step::Complete(Some(value)),
                        InvokeStep::CyclePublished(value) => {
                            *step = Step::CyclePublishedPrimitiveReply {
                                value: Some(value),
                                resume: Some(Resume::Identity),
                            }
                        }
                        InvokeStep::Arguments { mut resume } => {
                            *step = Step::Arguments {
                                value: Some(resume.take_arguments_value()),
                                resume: Some(Resume::Invoke(resume)),
                            }
                        }
                        InvokeStep::Call(request) => {
                            let InvokeCallTarget::Callable(callee) = request.target else {
                                unreachable!()
                            };
                            *step = Step::RawCall {
                                inputs: Some(RawCallbackInputs::new(
                                    callee,
                                    request.receiver,
                                    request.arguments,
                                )),
                                resume: Some(Resume::Identity),
                            };
                        }
                        InvokeStep::Construct(_) => unreachable!(),
                    }
                }
                Step::ArgumentsProgress(progress) => {
                    use crate::engine::builtins::ArgumentsStep;
                    match progress.as_mut().expect("argument-list progress") {
                        ArgumentsStep::Complete(_) | ArgumentsStep::CyclePublished(_) => {
                            let next = progress.take().expect("argument-list progress");
                            cycle_published |= matches!(next, ArgumentsStep::CyclePublished(_));
                            let (ArgumentsStep::Complete(value)
                            | ArgumentsStep::CyclePublished(value)) = next
                            else {
                                unreachable!()
                            };
                            *step = Step::ArgumentsComplete(Some(value));
                        }
                        ArgumentsStep::Number { .. } => {
                            let ArgumentsStep::Number { mut resume } =
                                progress.take().expect("argument-list progress")
                            else {
                                unreachable!()
                            };
                            *step = Step::Number {
                                value: Some(resume.take_number_value()),
                                resume: Some(Resume::Arguments(resume)),
                            };
                        }
                        ArgumentsStep::Read { resume } => {
                            let (object, key) = resume.read_in_state();
                            let receiver = state.dup_jsvalue(&JsValue::Object(object))?;
                            let mut receiver = OwnedValueGuard::new(state, poisoned, receiver);
                            let (state, receiver_value) = receiver.parts();
                            let selected = state.prepare_ordinary_read_in_state(
                                poisoned,
                                runtime.domain_id(),
                                object,
                                key,
                                receiver_value.as_ref().expect("argument Get receiver"),
                                false,
                                None,
                            );
                            if let Err(error) = selected {
                                if poisoned.get() {
                                    return Err(RuntimeError::Poisoned);
                                }
                                state.release_owned_jsvalue(
                                    poisoned,
                                    receiver_value.take().expect("argument Get receiver"),
                                )?;
                                let (object, _) = resume.take_read_in_state();
                                state.release_owned_jsvalue(poisoned, JsValue::Object(object))?;
                                return Err(error);
                            }
                            let read = selected.expect("selected argument read");
                            let ArgumentsStep::Read { resume } =
                                progress.take().expect("argument-list progress")
                            else {
                                unreachable!()
                            };
                            *step = Step::RawRead {
                                read: Some(read),
                                key,
                                resume: Some(Resume::Arguments(resume)),
                            };
                            state.release_owned_jsvalue(
                                poisoned,
                                receiver_value.take().expect("argument Get receiver"),
                            )?;
                            let Step::RawRead {
                                resume: Some(Resume::Arguments(resume)),
                                ..
                            } = step
                            else {
                                unreachable!()
                            };
                            let (object, _) = resume.take_read_in_state();
                            state.release_owned_jsvalue(poisoned, JsValue::Object(object))?;
                        }
                    }
                }
                Step::CyclePublishedPrimitiveReply { value, resume } => {
                    cycle_published = true;
                    *step = Step::PrimitiveReply {
                        value: value.take(),
                        resume: resume.take(),
                    };
                }
                Step::CyclePublishedNumber(value) => {
                    cycle_published = true;
                    *step = Step::NumberComplete(value.take());
                }
                Step::CyclePublishedElement(value) => {
                    cycle_published = true;
                    *step = Step::ElementComplete(value.take());
                }
                Step::String { value, resume } => {
                    *step = Step::Primitive {
                        value: value.take(),
                        hint: Some(crate::engine::vm::ToPrimitiveHint::String),
                        resume: Some(Resume::StringValue {
                            realm: self.realm,
                            resume: Box::new(resume.take().expect("string parent")),
                        }),
                    };
                }
                Step::StringReply { value, resume } => {
                    if !resume
                        .as_ref()
                        .expect("string reply parent")
                        .can_string_in_state()
                    {
                        break;
                    }
                    let resume = resume.take().expect("string reply parent");
                    *step = resume.string_in_state(
                        state,
                        poisoned,
                        runtime.0.host_services.as_ref(),
                        value.take().expect("string reply"),
                    )?;
                }
                Step::Number { value, resume } => {
                    let next = NumberStep::start_jsvalue_in_state(
                        state,
                        poisoned,
                        self.realm,
                        value.take().expect("number input"),
                    )?;
                    match next {
                        NumberStep::Complete(result) => {
                            *step = Step::NumberReply {
                                value: Some(result),
                                resume: resume.take(),
                            };
                        }
                        next => {
                            if self.parents.try_reserve(1).is_err() {
                                next.retire_in_state(state, poisoned)?;
                                return Err(RuntimeError::Invariant(
                                    "property continuation allocation failed",
                                ));
                            }
                            self.parents.push(resume.take().expect("number parent"));
                            *step = Step::try_from(next)?;
                        }
                    }
                }
                Step::Primitive {
                    value,
                    hint,
                    resume,
                } => {
                    let next = PrimitiveResume::start_in_state(
                        state,
                        poisoned,
                        self.realm,
                        value.take().expect("primitive input"),
                        hint.take().expect("primitive hint"),
                    )?;
                    match next {
                        PrimitiveStep::Complete(result) => {
                            *step = Step::PrimitiveReply {
                                value: Some(result),
                                resume: resume.take(),
                            };
                        }
                        next => {
                            if self.parents.try_reserve(1).is_err() {
                                next.retire_in_state(state, poisoned)?;
                                return Err(RuntimeError::Invariant(
                                    "primitive continuation allocation failed",
                                ));
                            }
                            self.parents.push(resume.take().expect("primitive parent"));
                            *step = Step::try_from(next)?;
                        }
                    }
                }
                Step::NumberReply { value, resume } => {
                    if !resume
                        .as_ref()
                        .expect("number reply parent")
                        .can_number_in_state()
                    {
                        break;
                    }
                    let resume = resume.take().expect("number reply parent");
                    *step = resume.number_in_state(
                        state,
                        poisoned,
                        runtime.0.host_services.as_ref(),
                        value.take().expect("number reply"),
                    )?;
                }
                Step::PrimitiveReply { value, resume } => {
                    if matches!(resume, Some(Resume::ComputedKey)) {
                        let _ = resume.take();
                        *step = self.computed_key_reply(
                            runtime,
                            state,
                            value.take().expect("computed key reply"),
                        )?;
                        continue;
                    }
                    if matches!(resume, Some(Resume::StringValue { .. })) {
                        *step = resume
                            .as_mut()
                            .expect("string wrapper")
                            .finish_string_value_in_state(
                                state,
                                poisoned,
                                value.take().expect("string primitive reply"),
                            )?;
                        continue;
                    }
                    if !resume
                        .as_ref()
                        .expect("primitive reply parent")
                        .can_resume_in_state()
                    {
                        break;
                    }
                    let resume = resume.take().expect("primitive reply parent");
                    *step = resume.resume_in_state(
                        state,
                        poisoned,
                        runtime.0.host_services.as_ref(),
                        value.take().expect("primitive reply"),
                    )?;
                }
                Step::NumberComplete(value) => {
                    if !self
                        .parents
                        .0
                        .last()
                        .is_some_and(Resume::can_number_in_state)
                    {
                        break;
                    }
                    let resume = self.parents.pop().expect("number parent");
                    *step = resume.number_in_state(
                        state,
                        poisoned,
                        runtime.0.host_services.as_ref(),
                        value.take().expect("number reply"),
                    )?;
                }
                Step::Complete(value) => {
                    if matches!(self.parents.0.last(), Some(Resume::ComputedKey)) {
                        let _ = self.parents.pop();
                        *step = self.computed_key_reply(
                            runtime,
                            state,
                            value.take().expect("computed key reply"),
                        )?;
                        continue;
                    }
                    if self
                        .parents
                        .0
                        .last()
                        .is_some_and(|parent| matches!(parent, Resume::StringValue { .. }))
                    {
                        let next = self
                            .parents
                            .0
                            .last_mut()
                            .expect("string wrapper")
                            .finish_string_value_in_state(
                                state,
                                poisoned,
                                value.take().expect("string primitive reply"),
                            )?;
                        // Success replaced only the wrapper with Identity; on
                        // failure its outer parent remained in the Query.
                        let _ = self.parents.pop();
                        *step = next;
                        continue;
                    }
                    if self.parents.is_empty() {
                        return Ok(StateProgress {
                            effect: StateEffect::Complete,
                            cycle_published,
                        });
                    }
                    if !self
                        .parents
                        .0
                        .last()
                        .is_some_and(Resume::can_resume_in_state)
                    {
                        break;
                    }
                    let resume = self.parents.pop().expect("completion parent");
                    *step = resume.resume_in_state(
                        state,
                        poisoned,
                        runtime.0.host_services.as_ref(),
                        value.take().expect("completion reply"),
                    )?;
                }
                Step::PrimitiveProgress(progress) => {
                    match progress.as_mut().expect("primitive progress") {
                        PrimitiveStep::CyclePublished(_) => {
                            let PrimitiveStep::CyclePublished(value) =
                                progress.take().expect("published primitive progress")
                            else {
                                unreachable!()
                            };
                            *step = Step::CyclePublishedComplete(Some(value));
                        }
                        PrimitiveStep::Complete(_) => {
                            let PrimitiveStep::Complete(value) =
                                progress.take().expect("primitive progress")
                            else {
                                unreachable!()
                            };
                            *step = Step::Complete(Some(value));
                        }
                        PrimitiveStep::Get { resume } => {
                            let (object, key) = resume.get_in_state();
                            let receiver = state.dup_jsvalue(&JsValue::Object(object))?;
                            let PrimitiveStep::Get { mut resume } =
                                progress.take().expect("primitive progress")
                            else {
                                unreachable!()
                            };
                            let _ = resume.take_get_in_state();
                            *step = Step::RawReadRequest {
                                selected: None,
                                object: Some(object),
                                key,
                                receiver: Some(receiver),
                                resume: Some(Resume::Primitive(resume)),
                            };
                        }
                        PrimitiveStep::Call { .. } => {
                            let PrimitiveStep::Call { mut resume } =
                                progress.take().expect("primitive progress")
                            else {
                                unreachable!()
                            };
                            let inputs = RawCallbackInputs::new(
                                resume.take_callable_in_state(),
                                resume.take_receiver(),
                                resume.take_arguments(),
                            );
                            *step = Step::RawCall {
                                inputs: Some(inputs),
                                resume: Some(Resume::Primitive(resume)),
                            };
                        }
                    }
                }
                Step::NumberProgress(progress) => {
                    match progress.as_mut().expect("number progress") {
                        NumberStep::CyclePublished(_) => {
                            let NumberStep::CyclePublished(value) =
                                progress.take().expect("published number progress")
                            else {
                                unreachable!()
                            };
                            *step = Step::CyclePublishedNumber(Some(value));
                        }
                        NumberStep::Complete(_) => {
                            let NumberStep::Complete(value) =
                                progress.take().expect("number progress")
                            else {
                                unreachable!()
                            };
                            *step = Step::NumberComplete(Some(value));
                        }
                        NumberStep::Read { resume } => {
                            let (object, key) = resume.read_in_state();
                            let receiver = state.dup_jsvalue(&JsValue::Object(object))?;
                            let NumberStep::Read { mut resume } =
                                progress.take().expect("number progress")
                            else {
                                unreachable!()
                            };
                            let _ = resume.take_read_in_state();
                            *step = Step::RawReadRequest {
                                selected: None,
                                object: Some(object),
                                key,
                                receiver: Some(receiver),
                                resume: Some(Resume::Number(resume)),
                            };
                        }
                        NumberStep::Call { .. } => {
                            let NumberStep::Call { mut resume } =
                                progress.take().expect("number progress")
                            else {
                                unreachable!()
                            };
                            let (function, receiver, arguments) = resume.take_call_in_state();
                            let inputs = RawCallbackInputs::new(function, receiver, arguments);
                            *step = Step::RawCall {
                                inputs: Some(inputs),
                                resume: Some(Resume::Number(resume)),
                            };
                        }
                    }
                }
                Step::CyclePublishedComplete(value) => {
                    cycle_published = true;
                    *step = Step::Complete(value.take());
                }
                Step::CyclePublishedPrimitive {
                    value,
                    hint,
                    resume,
                } => {
                    cycle_published = true;
                    *step = Step::Primitive {
                        value: value.take(),
                        hint: hint.take(),
                        resume: resume.take(),
                    };
                }
                Step::OrdinaryPrimitive { object, hint } => {
                    *step = Step::try_from(PrimitiveResume::ordinary_in_state(
                        state,
                        poisoned,
                        self.realm,
                        object.take().expect("ordinary primitive receiver"),
                        hint.take().expect("ordinary primitive hint"),
                    )?)?;
                }
                Step::RawValueReadRequest {
                    realm,
                    selected,
                    key,
                    receiver,
                    resume,
                } => {
                    if selected.is_none() {
                        *selected = Some(state.prepare_value_read_in_state(
                            poisoned,
                            runtime.domain_id(),
                            *realm,
                            receiver.as_ref().expect("raw value read receiver"),
                            *key,
                            None,
                        )?);
                    }
                    // The actual selection owns its output before input retirement.
                    state.release_owned_jsvalue(
                        poisoned,
                        receiver.take().expect("raw value read receiver"),
                    )?;
                    *step = Step::RawRead {
                        read: selected.take(),
                        key: *key,
                        resume: resume.take(),
                    };
                }
                Step::RawReadRequest {
                    selected,
                    object,
                    key,
                    receiver,
                    resume,
                } => {
                    if selected.is_none() {
                        let read = state.prepare_ordinary_read_in_state(
                            poisoned,
                            runtime.domain_id(),
                            object.expect("raw read object"),
                            *key,
                            receiver.as_ref().expect("raw read receiver"),
                            false,
                            None,
                        );
                        match read {
                            Ok(read) => *selected = Some(read),
                            Err(error) => {
                                if poisoned.get() {
                                    return Err(RuntimeError::Poisoned);
                                }
                                state.release_owned_jsvalue(
                                    poisoned,
                                    receiver.take().expect("raw read receiver"),
                                )?;
                                state.release_owned_jsvalue(
                                    poisoned,
                                    JsValue::Object(object.take().expect("raw read object")),
                                )?;
                                return Err(error);
                            }
                        }
                    }
                    // Selection is published before cleanup, and each unconsumed
                    // request role stays in this record until its actual retirement.
                    state.release_owned_jsvalue(
                        poisoned,
                        receiver.take().expect("raw read receiver"),
                    )?;
                    state.release_owned_jsvalue(
                        poisoned,
                        JsValue::Object(object.take().expect("raw read object")),
                    )?;
                    *step = Step::RawRead {
                        read: selected.take(),
                        key: *key,
                        resume: resume.take(),
                    };
                }
                Step::RawRead { read, resume, .. } => {
                    if self.final_read_pending() && !matches!(read, Some(ReadStep::Shared(_))) {
                        return Ok(StateProgress {
                            effect: StateEffect::PropertyRead,
                            cycle_published,
                        });
                    }
                    // Preserve the allocation fact even when the selected effect
                    // itself must leave this lease (for example, a Proxy).
                    cycle_published |= matches!(read, Some(ReadStep::CyclePublished(_)));
                    match read.as_ref().expect("selected read") {
                        ReadStep::Shared(_)
                        | ReadStep::Ready(OwnedRead::Proxy { .. })
                        | ReadStep::CyclePublished(OwnedRead::Proxy { .. }) => break,
                        _ => {}
                    }
                    let selected = read.take().expect("selected read");
                    let selected = match selected {
                        ReadStep::Ready(read) => read,
                        ReadStep::CyclePublished(read) => {
                            cycle_published = true;
                            read
                        }
                        ReadStep::Shared(_) => unreachable!(),
                    };
                    let resume = resume.take().expect("read parent");
                    *step = match selected {
                        OwnedRead::Complete(value) => Step::PrimitiveReply {
                            value: Some(Completion::Return(value.unwrap_or(JsValue::Undefined))),
                            resume: Some(resume),
                        },
                        OwnedRead::Getter { function, receiver } => Step::RawCall {
                            inputs: Some(RawCallbackInputs::new(function, receiver, Vec::new())),
                            resume: Some(resume),
                        },
                        OwnedRead::Proxy { .. } => unreachable!(),
                    };
                }
                Step::RawCall { .. } => {
                    return Ok(StateProgress {
                        effect: StateEffect::Callback,
                        cycle_published,
                    });
                }
                _ => break,
            }
        }
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_execution_event(match step {
            Step::Complete(_) => "native_query.boundary.legacy_completion",
            Step::NumberReply { .. } | Step::NumberComplete(_) => {
                "native_query.boundary.legacy_number_reply"
            }
            Step::PrimitiveReply { .. } => "native_query.boundary.legacy_primitive_reply",
            Step::StringReply { .. } => "native_query.boundary.legacy_string_reply",
            Step::RawRead { read, .. } => match read {
                Some(ReadStep::Shared(_)) => "native_query.boundary.shared_read",
                Some(ReadStep::Ready(OwnedRead::Proxy { .. }))
                | Some(ReadStep::CyclePublished(OwnedRead::Proxy { .. })) => {
                    "native_query.boundary.proxy_read"
                }
                _ => "native_query.boundary.other_read",
            },
            Step::CallbackBoundary(_) => "native_query.boundary.general_callback",
            Step::PreparedNativeBoundary(_) => "native_query.boundary.native_body",
            _ => "native_query.boundary.other_step",
        });
        Ok(StateProgress {
            effect: StateEffect::Boundary,
            cycle_published,
        })
    }
}

impl Step {
    /// Resident rollback retires raw records before Query's boundary Drop.
    /// Legacy wrappers remain in place until the caller ends its lease.
    pub(in crate::engine::vm) fn retire_raw_in_state(
        &mut self,
        state: &mut RuntimeState,
        poisoned: &std::cell::Cell<bool>,
    ) -> Result<(), RuntimeError> {
        let completion = |state: &mut RuntimeState, value: Completion| {
            let (Completion::Return(value) | Completion::Throw(value)) = value;
            state.release_owned_jsvalue(poisoned, value)
        };
        match self {
            Self::Complete(value) | Self::CyclePublishedComplete(value) => {
                if let Some(value) = value.take() {
                    completion(state, value)?;
                }
            }
            Self::CyclePublishedNumber(value) => {
                if let Some(NativeConversion::Throw(value)) = value.take() {
                    state.release_owned_jsvalue(poisoned, value)?;
                }
            }
            Self::CyclePublishedElement(value) => {
                if let Some(NativeConversion::Throw(value)) = value.take() {
                    state.release_owned_jsvalue(poisoned, value)?;
                }
            }
            Self::ArgumentsProgress(value) => {
                if let Some(value) = value.take() {
                    value.retire_in_state(state, poisoned)?;
                }
            }
            Self::InvokeProgress(value) => {
                if let Some(value) = value.take() {
                    value.retire_in_state(state, poisoned)?;
                }
            }
            Self::ArgumentsReply { value, resume } => {
                if let Some(value) = value.take() {
                    match value {
                        NativeConversion::Value(values) => {
                            for value in values {
                                state.release_owned_jsvalue(poisoned, value)?;
                            }
                        }
                        NativeConversion::Throw(value) => {
                            state.release_owned_jsvalue(poisoned, value)?
                        }
                    }
                }
                if let Some(resume) = resume {
                    resume.retire_raw_in_state(state, poisoned)?;
                }
            }
            Self::ArgumentsComplete(value) => {
                if let Some(value) = value.take() {
                    match value {
                        NativeConversion::Value(values) => {
                            for value in values {
                                state.release_owned_jsvalue(poisoned, value)?;
                            }
                        }
                        NativeConversion::Throw(value) => {
                            state.release_owned_jsvalue(poisoned, value)?
                        }
                    }
                }
            }
            Self::StringReply { value, resume }
            | Self::CyclePublishedStringReply { value, resume } => {
                if let Some(NativeConversion::Throw(value)) = value.take() {
                    state.release_owned_jsvalue(poisoned, value)?;
                }
                if let Some(resume) = resume {
                    resume.retire_raw_in_state(state, poisoned)?;
                }
            }
            Self::NumberReply { value, resume } => {
                if let Some(NativeConversion::Throw(value)) = value.take() {
                    state.release_owned_jsvalue(poisoned, value)?;
                }
                if let Some(resume) = resume {
                    resume.retire_raw_in_state(state, poisoned)?;
                }
            }
            Self::PrimitiveReply { value, resume }
            | Self::CyclePublishedPrimitiveReply { value, resume } => {
                if let Some(value) = value.take() {
                    completion(state, value)?;
                }
                if let Some(resume) = resume {
                    resume.retire_raw_in_state(state, poisoned)?;
                }
            }
            Self::NumberComplete(value) => {
                if let Some(NativeConversion::Throw(value)) = value.take() {
                    state.release_owned_jsvalue(poisoned, value)?;
                }
            }
            Self::PrimitiveProgress(value) => {
                if let Some(value) = value.take() {
                    value.retire_in_state(state, poisoned)?;
                }
            }
            Self::NumberProgress(value) => {
                if let Some(value) = value.take() {
                    value.retire_in_state(state, poisoned)?;
                }
            }
            Self::Arguments { value, resume }
            | Self::String { value, resume }
            | Self::Number { value, resume }
            | Self::Primitive { value, resume, .. }
            | Self::CyclePublishedPrimitive { value, resume, .. } => {
                if let Some(value) = value.take() {
                    state.release_owned_jsvalue(poisoned, value)?;
                }
                if let Some(resume) = resume {
                    resume.retire_raw_in_state(state, poisoned)?;
                }
            }
            Self::CallbackBoundary(Some(value)) => {
                value.inputs.retire(state, poisoned)?;
                value.resume.retire_raw_in_state(state, poisoned)?;
            }
            Self::PreparedNativeBoundary(Some(value)) => {
                value.call.abandon(state, poisoned);
                if poisoned.get() {
                    return Err(RuntimeError::Poisoned);
                }
                value.resume.retire_raw_in_state(state, poisoned)?;
            }
            Self::OrdinaryPrimitive { object, .. } => {
                if let Some(object) = object.take() {
                    state.release_owned_jsvalue(poisoned, JsValue::Object(object))?;
                }
            }
            Self::RawValueReadRequest {
                selected,
                receiver,
                resume,
                ..
            } => {
                if let Some(ReadStep::Ready(read) | ReadStep::CyclePublished(read)) =
                    selected.take()
                {
                    read.retire(state, poisoned)?;
                }
                if let Some(receiver) = receiver.take() {
                    state.release_owned_jsvalue(poisoned, receiver)?;
                }
                if let Some(resume) = resume {
                    resume.retire_raw_in_state(state, poisoned)?;
                }
            }
            Self::RawReadRequest {
                selected,
                object,
                receiver,
                resume,
                ..
            } => {
                if let Some(ReadStep::Ready(read) | ReadStep::CyclePublished(read)) =
                    selected.take()
                {
                    read.retire(state, poisoned)?;
                }
                if let Some(receiver) = receiver.take() {
                    state.release_owned_jsvalue(poisoned, receiver)?;
                }
                if let Some(object) = object.take() {
                    state.release_owned_jsvalue(poisoned, JsValue::Object(object))?;
                }
                if let Some(resume) = resume {
                    resume.retire_raw_in_state(state, poisoned)?;
                }
            }
            Self::RawRead { read, resume, .. } => {
                if let Some(ReadStep::Ready(read) | ReadStep::CyclePublished(read)) = read.take() {
                    read.retire(state, poisoned)?;
                }
                if let Some(resume) = resume {
                    resume.retire_raw_in_state(state, poisoned)?;
                }
            }
            Self::RawCall { inputs, resume } => {
                if let Some(inputs) = inputs {
                    inputs.retire(state, poisoned)?;
                }
                if let Some(resume) = resume {
                    resume.retire_raw_in_state(state, poisoned)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
}
impl Query {
    pub(in crate::engine::vm) fn retire_raw_in_state(
        &mut self,
        runtime: &Runtime,
        state: &mut RuntimeState,
    ) -> Result<(), RuntimeError> {
        let poisoned = &runtime.0.poisoned;
        for resume in self.parents.0.iter_mut().rev() {
            resume.retire_raw_in_state(state, poisoned)?;
        }
        for scope in self.natives.iter_mut().rev() {
            scope.call.abandon(state, poisoned);
            if poisoned.get() {
                return Err(RuntimeError::Poisoned);
            }
            scope.resume.retire_raw_in_state(state, poisoned)?;
            for resume in scope.parents.0.iter_mut().rev() {
                resume.retire_raw_in_state(state, poisoned)?;
            }
        }
        if let Some(finish) = self.finish.as_mut() {
            finish.retire_computed_in_state(state, poisoned)?;
        }
        Ok(())
    }
}

/// This concrete owner covers a native activation and its semantic Query
/// while the existing State lease is held. It creates no Runtime Rc/Weak;
/// Query registers its one boundary capability only when it acquires raw work.
pub(in crate::engine::vm) struct RawNativeQuery<'a> {
    pub(in crate::engine::vm) runtime: &'a Runtime,
    pub(in crate::engine::vm) state: &'a mut RuntimeState,
    pub(in crate::engine::vm) call: Option<crate::engine::vm::call::PreparedNativeCall>,
    pub(in crate::engine::vm) query: Option<Query>,
    pub(in crate::engine::vm) step: Step,
    pub(in crate::engine::vm) pending_step: Option<Step>,
    pub(in crate::engine::vm) pending_call: Option<crate::engine::vm::call::PreparedNativeCall>,
}
impl<'a> RawNativeQuery<'a> {
    pub(in crate::engine::vm) fn new(
        runtime: &'a Runtime,
        state: &'a mut RuntimeState,
        call: crate::engine::vm::call::PreparedNativeCall,
        query: Query,
        step: Step,
    ) -> Self {
        Self {
            runtime,
            state,
            call: Some(call),
            query: Some(query),
            step,
            pending_step: None,
            pending_call: None,
        }
    }
    pub(in crate::engine::vm) fn advance(&mut self) -> Result<StateProgress, RuntimeError> {
        self.query
            .as_mut()
            .expect("resident native query")
            .advance_raw_in_state(self.runtime, self.state, &mut self.step)
    }
    /// Only a real effect transports the active outer native into Query.
    pub(in crate::engine::vm) fn publish_outer_scope(&mut self) -> Result<(), RuntimeError> {
        if self.call.is_none() {
            return Ok(());
        }
        let query = self.query.as_mut().expect("resident native query");
        super::storage::reserve(&mut query.natives, 1, "query.native_scopes")
            .map_err(|_| RuntimeError::Invariant("native continuation allocation failed"))?;
        super::storage::reserve(&mut query.spare_parents, 1, "query.spare_parents")
            .map_err(|_| RuntimeError::Invariant("native parent storage allocation failed"))?;
        query.natives.push(super::NativeScope {
            call: self.call.take().expect("resident outer native"),
            parents: super::Parents::default(),
            resume: Resume::Identity,
            parent_realm: query.realm,
        });
        query.saved_native_depth += 1;
        Ok(())
    }
    pub(in crate::engine::vm) fn take_query(&mut self) -> Query {
        self.query.take().expect("resident native query")
    }
    pub(in crate::engine::vm) fn take_step(&mut self) -> Step {
        std::mem::replace(&mut self.step, Step::Complete(None))
    }
}
impl RawNativeQuery<'_> {
    /// Normal rejection must observe the first direct retirement failure.
    pub(in crate::engine::vm) fn retire(&mut self) -> Result<(), RuntimeError> {
        if self.runtime.0.poisoned.get() {
            return Ok(());
        }
        if let Some(step) = &mut self.pending_step {
            step.retire_raw_in_state(self.state, &self.runtime.0.poisoned)?;
        }
        if let Some(call) = &mut self.pending_call {
            call.abandon(self.state, &self.runtime.0.poisoned);
            if self.runtime.0.poisoned.get() {
                return Err(RuntimeError::Poisoned);
            }
        }
        self.step
            .retire_raw_in_state(self.state, &self.runtime.0.poisoned)?;
        if let Some(query) = &mut self.query {
            query.retire_raw_in_state(self.runtime, self.state)?;
        }
        if let Some(call) = &mut self.call {
            call.abandon(self.state, &self.runtime.0.poisoned);
            if self.runtime.0.poisoned.get() {
                return Err(RuntimeError::Poisoned);
            }
        }
        Ok(())
    }
}
impl Drop for RawNativeQuery<'_> {
    fn drop(&mut self) {
        if std::thread::panicking() {
            self.runtime.0.poisoned.set(true);
        }
        let _ = self.retire();
        // Successful cleanup or destructive quarantine finishes every raw role.
        // Query's Weak fallback is never invoked under this admitted lease.
        if let Some(query) = &mut self.query
            && query.native_runtime.strong_count() != 0
        {
            let _ = std::mem::take(&mut query.native_runtime);
            self.runtime.unregister_raw_execution_owner();
        }
    }
}

/// The shared native consumer returns only semantic progress. Persistent
/// owners already reside in the existing pending Query or rare boundary slot.
pub(in crate::engine::vm) enum StateNativeProgress {
    Complete(Completion),
    Published,
    PublishedThrow,
    Entered,
    Boundary,
}
pub(in crate::engine::vm) struct ResidentQueryBoundary {
    pub(in crate::engine::vm) query: Option<Query>,
    pub(in crate::engine::vm) step: Option<Step>,
    pub(in crate::engine::vm) owner: crate::engine::vm::frame::ReturnOwner,
    pub(in crate::engine::vm) identity: u64,
}
impl ResidentQueryBoundary {
    pub(in crate::engine::vm) fn into_parts(
        mut self,
    ) -> (Query, Step, crate::engine::vm::frame::ReturnOwner, u64) {
        (
            self.query.take().expect("selected boundary query"),
            self.step.take().expect("selected boundary step"),
            self.owner,
            self.identity,
        )
    }
}
impl Drop for ResidentQueryBoundary {
    fn drop(&mut self) {
        let Some(query) = &self.query else {
            return;
        };
        let runtime = query.native_runtime.upgrade().map(Runtime);
        if let Some(runtime) = runtime
            && let Some(step) = &mut self.step
        {
            step.release_owned(&runtime);
        }
    }
}

impl RawNativeQuery<'_> {
    pub(in crate::engine::vm) fn register(&mut self) -> Result<(), crate::engine::api::Error> {
        self.query
            .as_mut()
            .expect("resident query")
            .ensure_raw_owner_registration(self.runtime)
    }
    pub(in crate::engine::vm) fn realm(&self) -> crate::engine::heap::ContextId {
        self.query.as_ref().expect("resident query").realm
    }
    pub(in crate::engine::vm) fn depth(&self) -> usize {
        self.query
            .as_ref()
            .expect("resident query")
            .continuation_depth()
    }
    pub(in crate::engine::vm) fn has_native_scope(&self) -> bool {
        !self
            .query
            .as_ref()
            .expect("resident query")
            .natives
            .is_empty()
    }
    fn add_native_scope(
        &mut self,
        call: crate::engine::vm::call::PreparedNativeCall,
        resume: Resume,
    ) {
        let query = self.query.as_mut().expect("resident query");
        debug_assert!(query.natives.len() < query.natives.capacity());
        debug_assert!(query.spare_parents.len() < query.spare_parents.capacity());
        let parents = std::mem::replace(
            &mut query.parents,
            query.spare_parents.pop().unwrap_or_default(),
        );
        let parent_realm = query.realm;
        query.realm = call.activation.realm;
        query.saved_native_depth += 1 + parents.len() as u128;
        query.natives.push(super::NativeScope {
            call,
            parents,
            resume,
            parent_realm,
        });
    }
    pub(in crate::engine::vm) fn reserve_native_scope(&mut self) -> Result<(), RuntimeError> {
        let query = self.query.as_mut().expect("resident query");
        super::storage::reserve(&mut query.natives, 1, "query.native_scopes")
            .map_err(|_| RuntimeError::Invariant("native continuation allocation failed"))?;
        super::storage::reserve(&mut query.spare_parents, 1, "query.spare_parents")
            .map_err(|_| RuntimeError::Invariant("native parent storage allocation failed"))?;
        Ok(())
    }
    pub(in crate::engine::vm) fn finish_native_scope(
        &mut self,
        slots: &mut crate::engine::vm::stack::SlotStore,
    ) -> Result<(), RuntimeError> {
        let Step::Complete(value) = &mut self.step else {
            return Err(RuntimeError::Invariant("native completion omitted result"));
        };
        let completion = value.take().expect("native completion");
        let query = self.query.as_mut().expect("resident query");
        let mut scope = query.natives.pop().expect("native scope");
        query.saved_native_depth -= 1 + scope.parents.len() as u128;
        for resume in query.parents.0.iter_mut().rev() {
            resume.retire_raw_in_state(self.state, &self.runtime.0.poisoned)?;
        }
        let empty = std::mem::replace(&mut query.parents, scope.parents);
        query.spare_parents.push(empty);
        query.realm = scope.parent_realm;
        let (result, arguments) = scope.call.finish_completion_reusing(
            self.state,
            &self.runtime.0.poisoned,
            Ok(completion),
        );
        slots.recycle_native_argument_buffer(arguments);
        match result {
            Ok(completion) => {
                self.step = scope.resume.resume_in_state(
                    self.state,
                    &self.runtime.0.poisoned,
                    self.runtime.0.host_services.as_ref(),
                    completion,
                )?
            }
            Err(error) => {
                if !self.runtime.0.poisoned.get() {
                    scope
                        .resume
                        .retire_raw_in_state(self.state, &self.runtime.0.poisoned)?;
                }
                return Err(error);
            }
        }
        Ok(())
    }
    pub(in crate::engine::vm) fn finish_outer(
        &mut self,
        slots: &mut crate::engine::vm::stack::SlotStore,
    ) -> Result<Completion, RuntimeError> {
        let Step::Complete(value) = &mut self.step else {
            return Err(RuntimeError::Invariant("native completion omitted result"));
        };
        let completion = value.take().expect("native completion");
        if let Some(call) = self.call.take() {
            let (result, arguments) = call.finish_completion_reusing(
                self.state,
                &self.runtime.0.poisoned,
                Ok(completion),
            );
            slots.recycle_native_argument_buffer(arguments);
            result
        } else {
            Ok(completion)
        }
    }
    pub(in crate::engine::vm) fn callback_boundary(
        &mut self,
        selection: crate::engine::vm::call::ordinary::CallbackSelection,
        overflow: bool,
    ) {
        let Step::RawCall { inputs, resume } = &mut self.step else {
            unreachable!("raw callback selection")
        };
        let boundary = super::request::SelectedRawCallback {
            inputs: inputs.take().expect("raw callback inputs"),
            selection,
            overflow,
            resume: resume.take().expect("raw callback parent"),
        };
        self.step = Step::CallbackBoundary(Some(Box::new(boundary)));
    }
    pub(in crate::engine::vm) fn native_boundary(
        &mut self,
        call: crate::engine::vm::call::PreparedNativeCall,
        kind: crate::engine::builtins::continuation::NativeOperation,
    ) {
        let Step::RawCall { resume, .. } = &mut self.step else {
            unreachable!("raw callback native")
        };
        let boundary = super::request::PreparedNativeBoundary {
            call,
            kind,
            resume: resume.take().expect("native callback parent"),
        };
        self.step = Step::PreparedNativeBoundary(Some(Box::new(boundary)));
    }
}

impl<'a> RawNativeQuery<'a> {
    pub(in crate::engine::vm) fn for_call(
        runtime: &'a Runtime,
        state: &'a mut RuntimeState,
        call: crate::engine::vm::call::PreparedNativeCall,
        storage: &mut super::storage::QueryStorage,
        step: Step,
        return_to: crate::engine::vm::frame::ReturnTarget,
        instruction_depth: Option<usize>,
    ) -> Self {
        let finish = match instruction_depth {
            Some(depth) => super::Finish::ResidentCall {
                depth,
                tail: return_to.tail,
            },
            None => super::Finish::VmCall(return_to.value_use),
        };
        let realm = call.activation.realm;
        let query = storage.acquire(realm, Vec::new(), finish);
        Self::new(runtime, state, call, query, step)
    }
    pub(in crate::engine::vm) fn install_pending_native_scope(
        &mut self,
    ) -> Result<(), RuntimeError> {
        self.reserve_native_scope()?;
        let Step::RawCall { resume, .. } = &mut self.step else {
            unreachable!("nested native parent")
        };
        let resume = resume.take().expect("nested native parent");
        let call = self.pending_call.take().expect("nested native activation");
        // Capacity was reserved before surrendering either concrete owner.
        self.add_native_scope(call, resume);
        self.step = self.pending_step.take().expect("nested native progress");
        Ok(())
    }
}

pub(in crate::engine::vm) fn resident_query(
    storage: &mut super::storage::QueryStorage,
    realm: crate::engine::heap::ContextId,
    return_to: crate::engine::vm::frame::ReturnTarget,
    instruction_depth: Option<usize>,
) -> Query {
    let finish = match instruction_depth {
        Some(depth) => super::Finish::ResidentCall {
            depth,
            tail: return_to.tail,
        },
        None => super::Finish::VmCall(return_to.value_use),
    };
    storage.acquire(realm, Vec::new(), finish)
}
pub(in crate::engine::vm) fn recycle_resident_query(
    runtime: &Runtime,
    state: &mut RuntimeState,
    storage: &mut super::storage::QueryStorage,
    query: Query,
) -> Result<(), RuntimeError> {
    query.recycle_in_state(runtime, state, storage)
}

impl super::PendingProxyGet {
    pub(in crate::engine::vm) fn can_resume_raw_in_state(
        &self,
        operation: Option<crate::engine::vm::frame::OperationTarget>,
    ) -> bool {
        use crate::engine::vm::frame::OperationTarget;
        let compatible = |resume: &Resume| {
            resume.can_resume_in_state()
                || resume.can_number_in_state()
                || resume.can_string_in_state()
                || resume.can_arguments_in_state()
                || matches!(resume, Resume::StringValue { .. })
        };
        operation == Some(OperationTarget::PropertyGet(self.identity))
            && matches!(
                self.query.finish,
                Some(
                    super::Finish::Call { .. }
                        | super::Finish::ResidentCall { .. }
                        | super::Finish::VmCall(_)
                        | super::Finish::ComputedRead(_)
                        | super::Finish::PropertyKeyValue { .. }
                )
            )
            && self.resume.can_resume_in_state()
            && self.query.parents.0.iter().all(compatible)
            && self.query.natives.iter().all(|scope| {
                scope.resume.can_resume_in_state() && scope.parents.0.iter().all(compatible)
            })
    }
}
impl<'a> RawNativeQuery<'a> {
    /// A cold call callback still names its originating instruction; resident
    /// callbacks already published their continuation. Promote only that actual
    /// cold record before it can enter another callback or boundary.
    pub(in crate::engine::vm) fn commit_reply_continuation(
        &mut self,
        frame: &mut crate::engine::vm::frame::Frame,
    ) -> Result<crate::engine::vm::execute::FallthroughPc, crate::engine::api::Error> {
        let finish = self
            .query
            .as_mut()
            .expect("native reply query")
            .finish
            .as_mut()
            .expect("native reply continuation");
        let index = match finish {
            super::Finish::Call { .. } => frame.next_pc()?,
            super::Finish::ResidentCall { .. } | super::Finish::VmCall(_) => frame.resume_pc,
            super::Finish::ComputedRead(input) => input.fallthrough.index(),
            super::Finish::PropertyKeyValue { fallthrough, .. } => fallthrough.index(),
            _ => unreachable!("non-native query used native reply continuation"),
        };
        let fallthrough = crate::engine::vm::execute::FallthroughPc::from_committed_index(index)?;
        if let super::Finish::Call { depth, tail } = finish {
            *finish = super::Finish::ResidentCall {
                depth: *depth,
                tail: *tail,
            };
            frame.resume_pc = index;
        }
        Ok(fallthrough)
    }

    pub(in crate::engine::vm) fn from_query(
        runtime: &'a Runtime,
        state: &'a mut RuntimeState,
        query: Query,
        step: Step,
    ) -> Self {
        Self {
            runtime,
            state,
            call: None,
            query: Some(query),
            step,
            pending_step: None,
            pending_call: None,
        }
    }
    pub(in crate::engine::vm) fn return_placement(
        &self,
    ) -> (bool, crate::engine::vm::frame::ReturnValue, Option<usize>) {
        match self
            .query
            .as_ref()
            .expect("native query")
            .finish
            .as_ref()
            .expect("native final consumer")
        {
            super::Finish::Call { depth, tail } | super::Finish::ResidentCall { depth, tail } => (
                *tail,
                crate::engine::vm::frame::ReturnValue::Push,
                Some(*depth),
            ),
            super::Finish::VmCall(use_value) => (false, *use_value, None),
            super::Finish::ComputedRead(_) | super::Finish::PropertyKeyValue { .. } => {
                (false, crate::engine::vm::frame::ReturnValue::Push, None)
            }
            _ => unreachable!("non-native query used native result placement"),
        }
    }
}
