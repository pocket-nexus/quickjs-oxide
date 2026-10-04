//! Date construction owns converted fields before resolving the result prototype.
use super::{DEFAULT_DATE_FIELDS, MAX_DATE_ARGUMENTS, parsed_date_value};
use crate::engine::{
    api::{
        runtime::{Runtime, RuntimeUnwindGuard},
        runtime_error::RuntimeError,
    },
    atom::{Atom, pinned::PinnedAtom},
    builtins::{
        date::{
            calendar::{DateInputFields, set_date_fields_checked, time_clip},
            parse::parse_date_string,
        },
        native::DateNativeKind,
    },
    heap::{
        ContextId,
        runtime::{RuntimeState, owned_values::OwnedValueGuard},
    },
    host::HostServices,
    object::FunctionRealmOutcome,
    value::{JsString, JsValue, conversion::NativeConversion, number::operations::Number},
    vm::{
        Completion,
        call::{NativeArguments, NativeInvocation},
    },
};
use std::cell::Cell;
mod reply;

pub(crate) enum DateConstructorStep {
    Complete(Completion),
    CyclePublished(Completion),
    Primitive {
        value: JsValue,
        resume: DateConstructorResume,
    },
    String {
        value: JsValue,
        resume: DateConstructorResume,
    },
    Number {
        value: JsValue,
        resume: DateConstructorResume,
    },
    Read {
        realm: ContextId,
        receiver: JsValue,
        key: Atom,
        resume: DateConstructorResume,
    },
}
enum Phase {
    Single,
    Fields,
    Prototype,
    Parse,
}
pub(crate) struct DateConstructorResume(Box<DateConstructorResumeState>);
struct DateConstructorResumeState {
    converted: JsValue,
    prototype: Option<JsValue>,
    realm: ContextId,
    kind: DateNativeKind,
    new_target: JsValue,
    arguments: std::collections::VecDeque<JsValue>,
    fields: DateInputFields,
    index: usize,
    value: f64,
    phase: Phase,
}
impl DateConstructorStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        kind: DateNativeKind,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        let _unwind = runtime.unwind_guard();
        runtime.check_poison()?;
        let result = Self::start_in_state(
            &mut runtime.0.state.borrow_mut(),
            &runtime.0.poisoned,
            runtime.0.host_services.as_ref(),
            realm,
            kind,
            invocation,
            arguments,
        );
        runtime.check_poison()?;
        result
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
        let _unwind = RuntimeUnwindGuard::from_flag(poisoned);
        let result =
            Self::start_admitted(state, poisoned, host, realm, kind, invocation, arguments);
        if poisoned.get() {
            return Err(RuntimeError::Poisoned);
        }
        result
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
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_execution_event("date_constructor_state_start");
        let new_target = match (kind, invocation) {
            (DateNativeKind::Constructor, NativeInvocation::Construct { new_target }) => new_target,
            (
                DateNativeKind::Now | DateNativeKind::Parse | DateNativeKind::Utc,
                NativeInvocation::Call { .. },
            ) => &JsValue::Undefined,
            _ => {
                return Err(RuntimeError::Invariant(
                    "Date constructor/static invocation mismatch",
                ));
            }
        };
        if kind == DateNativeKind::Now {
            return state
                .call_date_readonly_native_with_publication(poisoned, host, realm, kind, invocation)
                .map(|step| match step {
                    crate::engine::builtins::continuation::NativeStep::Complete(completion) => {
                        Self::Complete(completion)
                    }
                    crate::engine::builtins::continuation::NativeStep::CyclePublishedComplete(
                        completion,
                    ) => Self::CyclePublished(completion),
                    _ => unreachable!("read-only Date.now is an immediate body"),
                });
        }
        if kind == DateNativeKind::Constructor && matches!(new_target, JsValue::Undefined) {
            return state.call_date_as_function(host).map(Self::Complete);
        }
        // Snapshot the real newTarget owner before reserving/copying argv, as
        // the legacy producer did. The native activation keeps the originals.
        let new_target = state.dup_jsvalue(new_target)?;
        let mut owner = DateConstructorGuard::new(
            state,
            poisoned,
            DateConstructorResume(Box::new(DateConstructorResumeState {
                converted: JsValue::Undefined,
                prototype: None,
                realm,
                kind,
                new_target,
                arguments: std::collections::VecDeque::new(),
                fields: DEFAULT_DATE_FIELDS,
                index: 0,
                value: f64::NAN,
                phase: Phase::Fields,
            })),
        );
        let count = arguments.actual_arg_count.min(MAX_DATE_ARGUMENTS);
        owner
            .resume
            .as_mut()
            .expect("Date constructor owner")
            .0
            .arguments
            .try_reserve(count)
            .map_err(|_| RuntimeError::Invariant("Date argv allocation failed"))?;
        let readable = arguments
            .readable
            .get(..count)
            .ok_or(RuntimeError::Invariant("Date actual arguments unreadable"))?;
        for value in readable {
            let value = owner.state.dup_jsvalue(value)?;
            owner
                .resume
                .as_mut()
                .expect("Date constructor owner")
                .0
                .arguments
                .push_back(value);
        }
        if kind == DateNativeKind::Parse {
            owner.resume.as_mut().expect("Date parse owner").0.phase = Phase::Parse;
            // Parse deliberately keeps the same independent first-argument
            // request owner in addition to the up-to-seven argument snapshot.
            let value = owner
                .state
                .dup_jsvalue(arguments.readable.first().unwrap_or(&JsValue::Undefined))?;
            return Ok(Self::String {
                value,
                resume: owner.take(),
            });
        }
        if arguments.actual_arg_count == 0 {
            if kind == DateNativeKind::Utc {
                return owner.complete(Completion::Return(JsValue::Float(f64::NAN)));
            }
            owner
                .resume
                .as_mut()
                .expect("Date constructor owner")
                .0
                .value = host.now_millis() as f64;
            return owner.prototype();
        }
        if kind == DateNativeKind::Constructor && arguments.actual_arg_count == 1 {
            let resume = owner.resume.as_mut().expect("Date constructor owner");
            resume.0.converted = resume
                .0
                .arguments
                .pop_front()
                .expect("one actual Date argument");
            if let Some(value) = owner.state.genuine_date_value(&resume.0.converted)? {
                resume.0.value = time_clip(value);
                return owner.prototype();
            }
            resume.0.phase = Phase::Single;
            let value = std::mem::replace(&mut resume.0.converted, JsValue::Undefined);
            return Ok(Self::Primitive {
                value,
                resume: owner.take(),
            });
        }
        owner.fields(host)
    }
    pub(crate) fn retire_in_state(
        self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
    ) -> Result<(), RuntimeError> {
        self.retire_with(&mut |value| state.release_owned_jsvalue(poisoned, value))
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
            Self::Primitive { value, mut resume }
            | Self::String { value, mut resume }
            | Self::Number { value, mut resume } => {
                release(value)?;
                resume.0.retire_with(release)
            }
            Self::Read {
                receiver,
                mut resume,
                ..
            } => {
                release(receiver)?;
                resume.0.retire_with(release)
            }
        }
    }
}
impl DateConstructorResumeState {
    fn retire_with(
        &mut self,
        release: &mut impl FnMut(JsValue) -> Result<(), RuntimeError>,
    ) -> Result<(), RuntimeError> {
        if let Some(value) = self.prototype.take() {
            release(value)?;
        }
        while let Some(value) = self.arguments.pop_front() {
            release(value)?;
        }
        release(std::mem::replace(&mut self.new_target, JsValue::Undefined))?;
        release(std::mem::replace(&mut self.converted, JsValue::Undefined))
    }
}
struct DateConstructorGuard<'a> {
    state: &'a mut RuntimeState,
    poisoned: &'a Cell<bool>,
    resume: Option<DateConstructorResume>,
}
impl<'a> DateConstructorGuard<'a> {
    fn new(
        state: &'a mut RuntimeState,
        poisoned: &'a Cell<bool>,
        resume: DateConstructorResume,
    ) -> Self {
        Self {
            state,
            poisoned,
            resume: Some(resume),
        }
    }
    fn take(&mut self) -> DateConstructorResume {
        self.resume.take().expect("Date constructor owner")
    }
    fn complete(&mut self, result: Completion) -> Result<DateConstructorStep, RuntimeError> {
        self.complete_step(DateConstructorStep::Complete(result))
    }
    fn complete_step(
        &mut self,
        step: DateConstructorStep,
    ) -> Result<DateConstructorStep, RuntimeError> {
        let mut output = DateConstructorStepGuard::new(self.state, self.poisoned, step);
        self.resume
            .take()
            .expect("Date constructor owner")
            .retire_in_state(output.state, self.poisoned)?;
        Ok(output.take())
    }
    fn prototype(&mut self) -> Result<DateConstructorStep, RuntimeError> {
        let resume = self.resume.as_mut().expect("Date prototype owner");
        resume.0.phase = Phase::Prototype;
        let receiver = self.state.dup_jsvalue(&resume.0.new_target)?;
        Ok(DateConstructorStep::Read {
            realm: resume.0.realm,
            receiver,
            key: self.state.pinned_atoms.get(PinnedAtom::Prototype),
            resume: self.take(),
        })
    }
    fn fields(&mut self, host: &dyn HostServices) -> Result<DateConstructorStep, RuntimeError> {
        let resume = self.resume.as_mut().expect("Date fields owner");
        if let Some(value) = resume.0.arguments.pop_front() {
            return Ok(DateConstructorStep::Number {
                value,
                resume: self.take(),
            });
        }
        resume.0.value = set_date_fields_checked(
            resume.0.fields,
            resume.0.kind == DateNativeKind::Constructor,
            |instant| host.timezone_offset_minutes(instant),
        );
        if resume.0.kind == DateNativeKind::Utc {
            let value = resume.0.value;
            return self.complete(Completion::Return(Number::compact(value).into()));
        }
        self.prototype()
    }
}
impl Drop for DateConstructorGuard<'_> {
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
struct DateConstructorStepGuard<'a> {
    state: &'a mut RuntimeState,
    poisoned: &'a Cell<bool>,
    step: Option<DateConstructorStep>,
}
impl<'a> DateConstructorStepGuard<'a> {
    fn new(
        state: &'a mut RuntimeState,
        poisoned: &'a Cell<bool>,
        step: DateConstructorStep,
    ) -> Self {
        Self {
            state,
            poisoned,
            step: Some(step),
        }
    }
    fn take(&mut self) -> DateConstructorStep {
        self.step.take().expect("Date constructor output")
    }
}
impl Drop for DateConstructorStepGuard<'_> {
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
pub(crate) fn finish(
    runtime: &Runtime,
    realm: ContextId,
    step: DateConstructorStep,
) -> Result<Completion, RuntimeError> {
    match step {
        DateConstructorStep::Complete(completion)
        | DateConstructorStep::CyclePublished(completion) => {
            runtime.check_poison()?;
            Ok(completion)
        }
        step => crate::engine::vm::execute_date_constructor_step(runtime, realm, step)
            .map_err(RuntimeError::from),
    }
}
const _: () = assert!(std::mem::size_of::<DateConstructorResume>() <= 8);
const _: () = assert!(std::mem::size_of::<DateConstructorStep>() <= 56);
#[cfg(test)]
mod tests;
