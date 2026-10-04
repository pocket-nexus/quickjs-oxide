//! Date conversion phases own raw edges; setter snapshots retain their specified timing.
use super::date_input_fields;
use crate::engine::{
    api::{
        error::NativeErrorKind,
        runtime::{Runtime, RuntimeUnwindGuard},
        runtime_error::RuntimeError,
    },
    atom::{Atom, pinned::PinnedAtom},
    builtins::{
        date::calendar::{DateFields, get_date_fields, set_date_fields, time_clip},
        native::{DateNativeKind, DateSetFieldKind},
    },
    heap::{
        ContextId, ObjectId,
        runtime::{RuntimeState, owned_values::OwnedValueGuard},
    },
    host::HostServices,
    object::{CallableRef, ObjectRef, PropertyKey},
    value::{
        JsString, JsValue,
        conversion::{NativeConversion, ToObjectOutcome},
    },
    vm::{
        Completion, ToPrimitiveHint,
        call::{NativeArguments, NativeInvocation},
    },
};
use std::cell::Cell;

pub(crate) enum DatePrototypeStep {
    Complete(Completion),
    CyclePublished(Completion),
    CyclePublishedPrimitive {
        value: JsValue,
        hint: ToPrimitiveHint,
        resume: DatePrototypeResume,
    },
    Number {
        value: JsValue,
        resume: DatePrototypeResume,
    },
    Primitive {
        value: JsValue,
        hint: ToPrimitiveHint,
        resume: DatePrototypeResume,
    },
    OrdinaryPrimitive {
        object: ObjectId,
        hint: ToPrimitiveHint,
    },
    Read {
        object: ObjectId,
        key: Atom,
        receiver: JsValue,
        resume: DatePrototypeResume,
    },
    Call {
        function: ObjectId,
        receiver: JsValue,
    },
}
enum Phase {
    Time,
    Field {
        field: DateSetFieldKind,
        fields: DateFields,
        had_fields: bool,
        all_finite: bool,
    },
    Year,
    JsonPrimitive,
    JsonMethod,
}
pub(crate) struct DatePrototypeResume(Box<DatePrototypeResumeState>);
struct DatePrototypeResumeState {
    realm: ContextId,
    object: Option<ObjectId>,
    phase: Phase,
    arguments: std::collections::VecDeque<JsValue>,
    reply: Option<JsValue>,
    converted: usize,
    actual: usize,
}
impl DatePrototypeStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        kind: DateNativeKind,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        let _unwind = runtime.unwind_guard();
        Self::start_in_state(
            &mut runtime.0.state.borrow_mut(),
            &runtime.0.poisoned,
            runtime.0.host_services.as_ref(),
            realm,
            kind,
            invocation,
            arguments,
        )
    }
    pub(crate) fn start_in_state(
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        host: &dyn HostServices,
        realm: ContextId,
        kind: DateNativeKind,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        match Self::start_admitted(state, poisoned, host, realm, kind, invocation, arguments) {
            Err(_) if poisoned.get() => Err(RuntimeError::Poisoned),
            result => result,
        }
    }
    fn start_admitted(
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        host: &dyn HostServices,
        realm: ContextId,
        kind: DateNativeKind,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        let _unwind = RuntimeUnwindGuard::from_flag(poisoned);
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_execution_event("date_prototype_state_start");
        let NativeInvocation::Call { this_value } = invocation else {
            return Err(RuntimeError::Invariant(
                "Date prototype requires generic invocation",
            ));
        };
        if kind == DateNativeKind::ToPrimitive {
            let JsValue::Object(object) = this_value else {
                return Ok(Self::CyclePublished(Completion::Throw(state.date_error(
                    poisoned,
                    realm,
                    NativeErrorKind::Type,
                    "not an object",
                )?)));
            };
            state.heap.retain_object(*object)?;
            let mut original = OwnedValueGuard::new(state, poisoned, JsValue::Object(*object));
            let (state, original_value) = original.parts();
            let hint = match arguments.readable.first() {
                Some(JsValue::String(id)) => {
                    let value = state.heap.string(*id)?;
                    if *value == JsString::from_static("number")
                        || *value == JsString::from_static("integer")
                    {
                        Some(ToPrimitiveHint::Number)
                    } else if *value == JsString::from_static("string")
                        || *value == JsString::from_static("default")
                    {
                        Some(ToPrimitiveHint::String)
                    } else {
                        None
                    }
                }
                Some(_) => None,
                None => {
                    return Err(RuntimeError::Invariant(
                        "Date native argument vector was not padded to readable arity",
                    ));
                }
            };
            let step = if let Some(hint) = hint {
                state.heap.retain_object(*object)?;
                Self::OrdinaryPrimitive {
                    object: *object,
                    hint,
                }
            } else {
                Self::CyclePublished(Completion::Throw(state.date_error(
                    poisoned,
                    realm,
                    NativeErrorKind::Type,
                    "invalid hint",
                )?))
            };
            let mut output = DateStepGuard::new(state, poisoned, step);
            output.state.release_owned_jsvalue(
                poisoned,
                original_value.take().expect("Date hint receiver temporary"),
            )?;
            return Ok(output.take());
        }
        if kind == DateNativeKind::ToJson {
            let input = state.dup_jsvalue(this_value)?;
            let (object, cycle_published) =
                match state.native_to_object_jsvalue(poisoned, realm, input)? {
                    ToObjectOutcome::Existing(object) => (object, false),
                    ToObjectOutcome::Boxed(object) => (object, true),
                    ToObjectOutcome::Throw(value) => {
                        return Ok(Self::CyclePublished(Completion::Throw(value)));
                    }
                };
            let mut owner = DateResumeGuard::new(
                state,
                poisoned,
                DatePrototypeResume::new(realm, object, Phase::JsonPrimitive, 0),
            );
            let value = owner.state.dup_jsvalue(&JsValue::Object(object))?;
            let resume = owner.take();
            return Ok(if cycle_published {
                Self::CyclePublishedPrimitive {
                    value,
                    hint: ToPrimitiveHint::Number,
                    resume,
                }
            } else {
                Self::Primitive {
                    value,
                    hint: ToPrimitiveHint::Number,
                    resume,
                }
            });
        }
        let (object, value) =
            match state.date_this_time_value_jsvalue(poisoned, realm, this_value)? {
                super::super::state::DateThisStep::Value(value) => value,
                super::super::state::DateThisStep::CyclePublishedThrow(value) => {
                    return Ok(Self::CyclePublished(Completion::Throw(value)));
                }
            };
        let mut original = OwnedValueGuard::new(state, poisoned, JsValue::Object(object));
        let (state, original_value) = original.parts();
        let (phase, count) = match kind {
            DateNativeKind::SetTime => (Phase::Time, 1),
            DateNativeKind::SetYear => (Phase::Year, 1),
            DateNativeKind::SetField(field) => {
                let first = usize::from(field.first_field());
                let end = usize::from(field.end_field());
                let fields =
                    get_date_fields(value, field.uses_local_time(), first == 0, |instant| {
                        host.timezone_offset_minutes(instant)
                    });
                let had_fields = fields.is_some();
                (
                    Phase::Field {
                        field,
                        fields: fields.unwrap_or([0.0; 9]),
                        had_fields,
                        all_finite: had_fields,
                    },
                    arguments.actual_arg_count.min(end.saturating_sub(first)),
                )
            }
            _ => {
                return Err(RuntimeError::Invariant(
                    "pure Date method reached callback operation",
                ));
            }
        };
        state.heap.retain_object(object)?;
        let mut owner = DateResumeGuard::new(
            state,
            poisoned,
            DatePrototypeResume::new(realm, object, phase, arguments.actual_arg_count),
        );
        owner
            .resume
            .as_mut()
            .expect("Date owner")
            .0
            .arguments
            .try_reserve(count)
            .map_err(|_| RuntimeError::Invariant("Date setter argv allocation failed"))?;
        let inputs = arguments
            .readable
            .get(..count)
            .ok_or(RuntimeError::Invariant("Date setter argv was not padded"))?;
        for value in inputs {
            let value = owner.state.dup_jsvalue(value)?;
            owner
                .resume
                .as_mut()
                .expect("Date owner")
                .0
                .arguments
                .push_back(value);
        }
        let step = owner.next(host)?;
        let mut output = DateStepGuard::new(state, poisoned, step);
        output.state.release_owned_jsvalue(
            poisoned,
            original_value
                .take()
                .expect("Date brand receiver temporary"),
        )?;
        Ok(output.take())
    }
    pub(crate) fn retire_in_state(
        self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
    ) -> Result<(), RuntimeError> {
        self.retire_with(&mut |value| state.release_owned_jsvalue(poisoned, value))
    }
    pub(crate) fn retire_at_boundary(self, runtime: &Runtime) -> Result<(), RuntimeError> {
        if runtime.skip_cleanup() {
            return Err(RuntimeError::Poisoned);
        }
        let _unwind = runtime.unwind_guard();
        if let Ok(mut state) = runtime.0.state.try_borrow_mut() {
            self.retire_in_state(&mut state, &runtime.0.poisoned)
        } else {
            self.retire_with(&mut |value| {
                runtime.release_jsvalue(value)?;
                runtime.check_poison()
            })
        }
    }
    fn retire_with(
        self,
        release: &mut impl FnMut(JsValue) -> Result<(), RuntimeError>,
    ) -> Result<(), RuntimeError> {
        match self {
            Self::Complete(Completion::Return(value) | Completion::Throw(value))
            | Self::CyclePublished(Completion::Return(value) | Completion::Throw(value)) => {
                release(value)
            }
            Self::Number { value, mut resume }
            | Self::CyclePublishedPrimitive {
                value, mut resume, ..
            }
            | Self::Primitive {
                value, mut resume, ..
            } => {
                release(value)?;
                resume.0.retire_with(release)
            }
            Self::OrdinaryPrimitive { object, .. } => release(JsValue::Object(object)),
            Self::Read {
                object,
                receiver,
                mut resume,
                ..
            } => {
                release(receiver)?;
                release(JsValue::Object(object))?;
                resume.0.retire_with(release)
            }
            Self::Call { function, receiver } => {
                release(JsValue::Object(function))?;
                release(receiver)
            }
        }
    }
}
impl DatePrototypeResume {
    fn new(realm: ContextId, object: ObjectId, phase: Phase, actual: usize) -> Self {
        Self(Box::new(DatePrototypeResumeState {
            realm,
            object: Some(object),
            phase,
            arguments: std::collections::VecDeque::new(),
            reply: Some(JsValue::Undefined),
            converted: 0,
            actual,
        }))
    }
    pub(crate) fn number(
        self,
        runtime: &Runtime,
        result: NativeConversion<f64>,
    ) -> Result<DatePrototypeStep, RuntimeError> {
        let _unwind = runtime.unwind_guard();
        self.number_in_state(
            &mut runtime.0.state.borrow_mut(),
            &runtime.0.poisoned,
            runtime.0.host_services.as_ref(),
            result,
        )
    }
    pub(crate) fn number_in_state(
        self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        host: &dyn HostServices,
        result: NativeConversion<f64>,
    ) -> Result<DatePrototypeStep, RuntimeError> {
        match self.number_admitted(state, poisoned, host, result) {
            Err(_) if poisoned.get() => Err(RuntimeError::Poisoned),
            result => result,
        }
    }
    fn number_admitted(
        self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        host: &dyn HostServices,
        result: NativeConversion<f64>,
    ) -> Result<DatePrototypeStep, RuntimeError> {
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_execution_event(
            "date_prototype_state_number_reply",
        );
        let mut owner = DateResumeGuard::new(state, poisoned, self);
        let value = match result {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => return owner.complete(Completion::Throw(value)),
        };
        let resume = owner.resume.as_mut().expect("Date numeric owner");
        let object = resume.0.object.expect("Date setter receiver");
        match &mut resume.0.phase {
            Phase::Time => {
                let result = owner
                    .state
                    .set_date_this_time_value(object, time_clip(value))?;
                owner.complete(result)
            }
            Phase::Year => {
                let result = owner.state.finish_date_set_year(host, object, value)?;
                owner.complete(result)
            }
            Phase::Field {
                field,
                fields,
                all_finite,
                ..
            } => {
                if !value.is_finite() {
                    *all_finite = false;
                }
                fields[usize::from(field.first_field()) + resume.0.converted] = value.trunc();
                resume.0.converted += 1;
                owner.next(host)
            }
            _ => Err(RuntimeError::Invariant("Date setter number phase mismatch")),
        }
    }
    pub(crate) fn resume(
        self,
        runtime: &Runtime,
        result: Completion,
    ) -> Result<DatePrototypeStep, RuntimeError> {
        let _unwind = runtime.unwind_guard();
        self.resume_in_state(
            &mut runtime.0.state.borrow_mut(),
            &runtime.0.poisoned,
            result,
        )
    }
    pub(crate) fn resume_in_state(
        self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        result: Completion,
    ) -> Result<DatePrototypeStep, RuntimeError> {
        match self.resume_admitted(state, poisoned, result) {
            Err(_) if poisoned.get() => Err(RuntimeError::Poisoned),
            result => result,
        }
    }
    fn resume_admitted(
        self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        result: Completion,
    ) -> Result<DatePrototypeStep, RuntimeError> {
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_execution_event(
            "date_prototype_state_completion_reply",
        );
        let mut owner = DateResumeGuard::new(state, poisoned, self);
        let value = match result {
            Completion::Return(value) => value,
            Completion::Throw(value) => return owner.complete(Completion::Throw(value)),
        };
        let resume = owner.resume.as_mut().expect("Date reply owner");
        if let Some(previous) = resume.0.reply.replace(value) {
            owner.state.release_owned_jsvalue(poisoned, previous)?;
        }
        let object = resume.0.object.expect("Date JSON receiver");
        match resume.0.phase {
            Phase::JsonPrimitive => {
                if matches!(resume.0.reply, Some(JsValue::Float(value)) if !value.is_finite()) {
                    return owner.complete(Completion::Return(JsValue::Null));
                }
                resume.0.phase = Phase::JsonMethod;
                owner.state.heap.retain_object(object)?;
                let mut request =
                    OwnedValueGuard::new(owner.state, poisoned, JsValue::Object(object));
                let (state, request_object) = request.parts();
                let receiver = state.dup_jsvalue(&JsValue::Object(object))?;
                request_object.take();
                let key = state.pinned_atoms.get(PinnedAtom::ToISOString);
                drop(request);
                Ok(DatePrototypeStep::Read {
                    object,
                    key,
                    receiver,
                    resume: owner.take(),
                })
            }
            Phase::JsonMethod => {
                let function = match resume.0.reply.as_ref().expect("Date JSON method") {
                    JsValue::Object(function)
                        if owner.state.object_id_has_call_capability(*function)? =>
                    {
                        Some(*function)
                    }
                    _ => None,
                };
                let Some(function) = function else {
                    let value = owner.state.date_error(
                        poisoned,
                        resume.0.realm,
                        NativeErrorKind::Type,
                        "object needs toISOString method",
                    )?;
                    return owner.complete_step(DatePrototypeStep::CyclePublished(
                        Completion::Throw(value),
                    ));
                };
                owner.state.heap.retain_object(function)?;
                let mut function_owner =
                    OwnedValueGuard::new(owner.state, poisoned, JsValue::Object(function));
                let (state, function_value) = function_owner.parts();
                let receiver = state.dup_jsvalue(&JsValue::Object(object))?;
                function_value.take();
                let mut output = DateStepGuard::new(
                    state,
                    poisoned,
                    DatePrototypeStep::Call { function, receiver },
                );
                owner
                    .resume
                    .take()
                    .expect("Date JSON owner")
                    .retire_in_state(output.state, poisoned)?;
                Ok(output.take())
            }
            _ => Err(RuntimeError::Invariant(
                "Date prototype value phase mismatch",
            )),
        }
    }
    pub(crate) fn retire_in_state(
        mut self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
    ) -> Result<(), RuntimeError> {
        self.0
            .retire_with(&mut |value| state.release_owned_jsvalue(poisoned, value))
    }
    pub(crate) fn retire_at_boundary(mut self, runtime: &Runtime) -> Result<(), RuntimeError> {
        self.0.retire_with(&mut |value| {
            runtime.release_jsvalue(value)?;
            runtime.check_poison()
        })
    }
}
impl DatePrototypeResumeState {
    fn retire_with(
        &mut self,
        release: &mut impl FnMut(JsValue) -> Result<(), RuntimeError>,
    ) -> Result<(), RuntimeError> {
        while let Some(value) = self.arguments.pop_front() {
            release(value)?;
        }
        if let Some(value) = self.reply.take() {
            release(value)?;
        }
        if let Some(object) = self.object.take() {
            release(JsValue::Object(object))?;
        }
        Ok(())
    }
}
struct DateResumeGuard<'a> {
    state: &'a mut RuntimeState,
    poisoned: &'a Cell<bool>,
    resume: Option<DatePrototypeResume>,
}
impl<'a> DateResumeGuard<'a> {
    fn new(
        state: &'a mut RuntimeState,
        poisoned: &'a Cell<bool>,
        resume: DatePrototypeResume,
    ) -> Self {
        Self {
            state,
            poisoned,
            resume: Some(resume),
        }
    }
    fn take(&mut self) -> DatePrototypeResume {
        self.resume.take().expect("Date resume owner")
    }
    fn complete(self, result: Completion) -> Result<DatePrototypeStep, RuntimeError> {
        self.complete_step(DatePrototypeStep::Complete(result))
    }
    fn complete_step(mut self, step: DatePrototypeStep) -> Result<DatePrototypeStep, RuntimeError> {
        let mut output = DateStepGuard::new(self.state, self.poisoned, step);
        self.resume
            .take()
            .expect("Date terminal owner")
            .retire_in_state(output.state, self.poisoned)?;
        Ok(output.take())
    }
    fn next(mut self, host: &dyn HostServices) -> Result<DatePrototypeStep, RuntimeError> {
        let resume = self.resume.as_mut().expect("Date setter owner");
        if let Some(value) = resume.0.arguments.pop_front() {
            return Ok(DatePrototypeStep::Number {
                value,
                resume: self.take(),
            });
        }
        let Phase::Field {
            field,
            fields,
            had_fields,
            all_finite,
        } = &resume.0.phase
        else {
            return Err(RuntimeError::Invariant(
                "Date setter numeric result missing",
            ));
        };
        if !had_fields {
            return self.complete(Completion::Return(JsValue::Float(f64::NAN)));
        }
        let value = if *all_finite && resume.0.actual > 0 {
            set_date_fields(
                &date_input_fields(fields),
                field.uses_local_time(),
                |instant| host.timezone_offset_minutes(instant),
            )
        } else {
            f64::NAN
        };
        let result = self
            .state
            .set_date_this_time_value(resume.0.object.expect("Date setter receiver"), value)?;
        self.complete(result)
    }
}
impl Drop for DateResumeGuard<'_> {
    fn drop(&mut self) {
        if std::thread::panicking() {
            self.poisoned.set(true);
        }
        if !self.poisoned.get()
            && let Some(resume) = self.resume.take()
        {
            let _ = resume.retire_in_state(self.state, self.poisoned);
        }
    }
}
struct DateStepGuard<'a> {
    state: &'a mut RuntimeState,
    poisoned: &'a Cell<bool>,
    step: Option<DatePrototypeStep>,
}
impl<'a> DateStepGuard<'a> {
    fn new(state: &'a mut RuntimeState, poisoned: &'a Cell<bool>, step: DatePrototypeStep) -> Self {
        Self {
            state,
            poisoned,
            step: Some(step),
        }
    }
    fn take(&mut self) -> DatePrototypeStep {
        self.step.take().expect("Date output owner")
    }
}
impl Drop for DateStepGuard<'_> {
    fn drop(&mut self) {
        if std::thread::panicking() {
            self.poisoned.set(true);
        }
        if !self.poisoned.get()
            && let Some(step) = self.step.take()
        {
            let _ = step.retire_in_state(self.state, self.poisoned);
        }
    }
}
struct DateBoundaryGuard<'a> {
    runtime: &'a Runtime,
    step: Option<DatePrototypeStep>,
}
impl Drop for DateBoundaryGuard<'_> {
    fn drop(&mut self) {
        if !self.runtime.skip_cleanup()
            && let Some(step) = self.step.take()
        {
            let _ = step.retire_at_boundary(self.runtime);
        }
    }
}
pub(crate) fn finish(
    runtime: &Runtime,
    realm: ContextId,
    step: DatePrototypeStep,
) -> Result<Completion, RuntimeError> {
    let mut owner = DateBoundaryGuard {
        runtime,
        step: Some(step),
    };
    loop {
        match owner.step.take().expect("Date boundary progress") {
            DatePrototypeStep::Complete(result) | DatePrototypeStep::CyclePublished(result) => {
                return Ok(result);
            }
            DatePrototypeStep::Number { value, resume } => {
                owner.step = Some(DatePrototypeStep::Number {
                    value: JsValue::Undefined,
                    resume,
                });
                let result = runtime.native_to_number_jsvalue(realm, value)?;
                let Some(DatePrototypeStep::Number { resume, .. }) = owner.step.take() else {
                    unreachable!()
                };
                owner.step = Some(resume.number(runtime, result)?);
            }
            DatePrototypeStep::Primitive {
                value,
                hint,
                resume,
            }
            | DatePrototypeStep::CyclePublishedPrimitive {
                value,
                hint,
                resume,
            } => {
                owner.step = Some(DatePrototypeStep::Primitive {
                    value: JsValue::Undefined,
                    hint,
                    resume,
                });
                let result = runtime.to_primitive_jsvalue(realm, value, hint)?;
                let Some(DatePrototypeStep::Primitive { resume, .. }) = owner.step.take() else {
                    unreachable!()
                };
                owner.step = Some(resume.resume(runtime, result)?);
            }
            DatePrototypeStep::OrdinaryPrimitive { object, hint } => {
                let object = ObjectRef::from_owned_handle(runtime.clone(), object);
                return runtime.ordinary_to_primitive(realm, &object, hint);
            }
            DatePrototypeStep::Read {
                object,
                key,
                receiver,
                resume,
            } => {
                owner.step = Some(DatePrototypeStep::Primitive {
                    value: receiver,
                    hint: ToPrimitiveHint::Number,
                    resume,
                });
                let object = ObjectRef::from_owned_handle(runtime.clone(), object);
                let key = PropertyKey::from_borrowed_atom(runtime.clone(), key)?;
                let Some(DatePrototypeStep::Primitive { value, .. }) = owner.step.as_mut() else {
                    unreachable!()
                };
                let receiver = std::mem::replace(value, JsValue::Undefined);
                let result = runtime.internal_get_jsvalue(realm, &object, &key, receiver)?;
                let Some(DatePrototypeStep::Primitive { resume, .. }) = owner.step.take() else {
                    unreachable!()
                };
                owner.step = Some(resume.resume(runtime, result)?);
            }
            DatePrototypeStep::Call { function, receiver } => {
                let callable = CallableRef::from_validated_object(ObjectRef::from_owned_handle(
                    runtime.clone(),
                    function,
                ));
                return runtime.call_internal_jsvalue(realm, &callable, receiver, Vec::new());
            }
        }
    }
}
const _: () = assert!(std::mem::size_of::<DatePrototypeResume>() <= 8);
const _: () = assert!(std::mem::size_of::<DatePrototypeStep>() <= 56);

#[cfg(test)]
mod tests;
