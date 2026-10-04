//! One Array endpoint state machine; only selected observable effects suspend.
use crate::engine::{
    api::{
        error::{NativeErrorKind, NativeErrorMessage},
        runtime::{Runtime, RuntimeUnwindGuard},
        runtime_error::RuntimeError,
    },
    atom::{Atom, pinned::PinnedAtom},
    builtins::{
        SharedTypedOwnWord,
        native::{ArrayPopKind, ArrayPushKind, NativeFunctionId},
    },
    heap::{ContextId, ObjectId, runtime::RuntimeState},
    object::{
        ObjectRef, OwnedRead, PropertyKey, ReadStep, SetAction, SetProgress,
        delete::DeleteOwnProperty,
    },
    value::{
        JsValue, Value,
        conversion::{NativeConversion, ToObjectOutcome},
    },
    vm::{
        Completion,
        call::{NativeArguments, NativeInvocation},
    },
};
use std::cell::Cell;
#[derive(Clone, Copy)]
pub(crate) enum MutationKind {
    Push(ArrayPushKind),
    Pop(ArrayPopKind),
}
impl MutationKind {
    pub(crate) fn for_target(target: NativeFunctionId) -> Option<Self> {
        match target {
            NativeFunctionId::ArrayPrototypePush(kind) => Some(Self::Push(kind)),
            NativeFunctionId::ArrayPrototypePop(kind) => Some(Self::Pop(kind)),
            _ => None,
        }
    }
}
pub(crate) enum MutationStep {
    Complete(Completion),
    CyclePublished(Completion),
    Read {
        resume: MutationResume,
    },
    CyclePublishedRead {
        resume: MutationResume,
    },
    Number {
        value: JsValue,
        resume: MutationResume,
    },
    Copy {
        object: ObjectId,
        to: u64,
        from: u64,
        count: u64,
        backwards: bool,
        resume: MutationResume,
    },
    Set {
        progress: Box<SetProgress>,
        resume: MutationResume,
    },
    CyclePublishedSet {
        progress: Box<SetProgress>,
        resume: MutationResume,
    },
    Delete {
        object: ObjectId,
        key: Atom,
        resume: MutationResume,
    },
    SharedDelete {
        word: SharedTypedOwnWord,
        resume: MutationResume,
    },
}
enum Phase {
    Length,
    Number,
    Result,
    Copy,
    Write,
    DeleteLast,
    LengthWrite,
}
pub(crate) struct MutationResume(Box<MutationResumeState>);
struct MutationResumeState {
    realm: ContextId,
    kind: MutationKind,
    object: Option<ObjectId>,
    arguments: Vec<JsValue>,
    inline_argument: bool,
    phase: Phase,
    length: u64,
    new_length: u64,
    cursor: u64,
    result: JsValue,
    key: Option<Atom>,
    pending_value: Option<JsValue>,
    // Preserve the two checked receiver roles formerly acquired by the
    // public receiver and prepare_ordinary_read_borrowed conversion.
    read_receiver: Option<JsValue>,
    read_conversion: Option<JsValue>,
}
enum MutationAction {
    Complete(Completion),
    CyclePublished(Completion),
    Read(Atom),
    Number(JsValue),
    Copy {
        to: u64,
        from: u64,
        count: u64,
        backwards: bool,
    },
    Set {
        key: Atom,
        value: JsValue,
    },
    Delete(Atom),
}
impl MutationStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        kind: MutationKind,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        let NativeInvocation::Call { this_value } = invocation else {
            return Err(RuntimeError::Invariant(
                "Array mutation requires generic invocation",
            ));
        };
        // The legacy adapter checks this before ToObject's selective realm,
        // prototype promotion and allocation admission. State native entry is
        // already admitted and delegates the same consuming producer below.
        let input = runtime.dup_jsvalue(this_value)?;
        let (object, boxed) = match runtime.native_to_object_outcome(realm, input)? {
            ToObjectOutcome::Existing(object) => (object, false),
            ToObjectOutcome::Boxed(object) => (object, true),
            ToObjectOutcome::Throw(value) => {
                return Ok(Self::CyclePublished(Completion::Throw(value)));
            }
        };
        let mut owner = MutationBoundaryGuard {
            runtime,
            step: Some(Self::Read {
                resume: Self::owner(realm, kind, object),
            }),
            read: None,
        };
        // Only the original endpoint attempt admitted an operation here. The
        // other fixed methods snapshot argv before the later length read.
        let endpoint = matches!(kind, MutationKind::Pop(ArrayPopKind::Pop))
            || matches!(kind, MutationKind::Push(ArrayPushKind::Push))
                && arguments.actual_arg_count == 1;
        let _operation = if endpoint {
            Some(runtime.operation()?)
        } else {
            None
        };
        let _unwind = runtime.unwind_guard();
        let Some(Self::Read { resume }) = owner.step.take() else {
            unreachable!()
        };
        Self::start_from_object_in_state(
            &mut runtime.0.state.borrow_mut(),
            &runtime.0.poisoned,
            resume,
            boxed,
            arguments,
        )
    }
    pub(crate) fn start_in_state(
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        realm: ContextId,
        kind: MutationKind,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        let NativeInvocation::Call { this_value } = invocation else {
            return Err(RuntimeError::Invariant(
                "Array mutation requires generic invocation",
            ));
        };
        let input = state.dup_jsvalue(this_value)?;
        let (object, boxed) = match state.native_to_object_jsvalue(poisoned, realm, input)? {
            ToObjectOutcome::Existing(object) => (object, false),
            ToObjectOutcome::Boxed(object) => (object, true),
            ToObjectOutcome::Throw(value) => {
                return Ok(Self::CyclePublished(Completion::Throw(value)));
            }
        };
        Self::start_from_object_in_state(
            state,
            poisoned,
            Self::owner(realm, kind, object),
            boxed,
            arguments,
        )
    }
    fn start_from_object_in_state(
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        resume: MutationResume,
        boxed: bool,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        let kind = resume.0.kind;
        let object = resume.0.object.expect("Array ToObject owner");
        let mut owner = MutationGuard::new(state, poisoned, resume);

        let values =
            arguments
                .readable
                .get(..arguments.actual_arg_count)
                .ok_or(RuntimeError::Invariant(
                    "Array mutation argv was not padded",
                ))?;
        let inline = matches!(
            (kind, values),
            (
                MutationKind::Push(_),
                [JsValue::Undefined
                    | JsValue::Null
                    | JsValue::Bool(_)
                    | JsValue::Int(_)
                    | JsValue::Float(_)
                    | JsValue::ShortBigInt(_)]
            )
        );
        #[cfg(feature = "profiling")]
        if inline {
            crate::engine::api::profiling::record_owned_execution_event(
                "array_mutation_inline_argument",
            );
        }
        let completed = match (kind, values) {
            (MutationKind::Push(ArrayPushKind::Push), [value]) => {
                owner.state.try_dense_push(poisoned, object, value)?
            }
            (MutationKind::Pop(ArrayPopKind::Pop), _) => {
                owner.state.try_dense_pop(poisoned, object)?
            }
            _ => None,
        };
        if let Some(value) = completed {
            return owner.complete(Completion::Return(value), boxed);
        }
        if inline {
            let value = owner.state.dup_jsvalue(&values[0])?;
            let resume = owner.get();
            resume.0.inline_argument = true;
            resume.0.result = value;
        } else {
            owner
                .get()
                .0
                .arguments
                .try_reserve_exact(values.len())
                .map_err(|_| RuntimeError::Invariant("Array mutation argv allocation failed"))?;
            // Pop/Shift retain their original ignored-argument snapshots too.
            for value in values {
                let value = owner.state.dup_jsvalue(value)?;
                owner.get().0.arguments.push(value);
            }
        }
        let key = owner.state.pinned_atoms.get(PinnedAtom::Length);
        owner.get().0.key = Some(key);
        let resume = owner.take();
        Ok(if boxed {
            Self::CyclePublishedRead { resume }
        } else {
            Self::Read { resume }
        })
    }
    pub(crate) fn start_values_in_state(
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        realm: ContextId,
        kind: MutationKind,
        object: ObjectId,
        arguments: Vec<JsValue>,
    ) -> Result<Self, RuntimeError> {
        let mut resume = Self::owner(realm, kind, object);
        resume.0.arguments = arguments;
        Self::start_owned_values_in_state(state, poisoned, resume)
    }
    fn start_owned_values_in_state(
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        resume: MutationResume,
    ) -> Result<Self, RuntimeError> {
        let kind = resume.0.kind;
        let object = resume.0.object.expect("Array owned source");
        let mut owner = MutationGuard::new(state, poisoned, resume);
        let completed = match kind {
            MutationKind::Push(ArrayPushKind::Push) if owner.get().0.arguments.len() == 1 => {
                let value = owner.get().0.arguments[0].as_raw();
                let value = JsValue::from_raw(value).ok_or(RuntimeError::Invariant(
                    "Array mutation held internal value",
                ))?;
                owner.state.try_dense_push(poisoned, object, &value)?
            }
            MutationKind::Pop(ArrayPopKind::Pop) => owner.state.try_dense_pop(poisoned, object)?,
            _ => None,
        };
        if let Some(value) = completed {
            return owner.complete(Completion::Return(value), false);
        }
        let key = owner.state.pinned_atoms.get(PinnedAtom::Length);
        owner.get().0.key = Some(key);
        Ok(Self::Read {
            resume: owner.take(),
        })
    }
    fn owner(realm: ContextId, kind: MutationKind, object: ObjectId) -> MutationResume {
        MutationResume(Box::new(MutationResumeState {
            realm,
            kind,
            object: Some(object),
            arguments: Vec::new(),
            inline_argument: false,
            phase: Phase::Length,
            length: 0,
            new_length: 0,
            cursor: 0,
            result: JsValue::Undefined,
            key: None,
            pending_value: None,
            read_receiver: None,
            read_conversion: None,
        }))
    }
    pub(crate) fn retire_in_state(
        self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
    ) -> Result<(), RuntimeError> {
        match self {
            Self::Complete(Completion::Return(value) | Completion::Throw(value))
            | Self::CyclePublished(Completion::Return(value) | Completion::Throw(value)) => {
                state.release_owned_jsvalue(poisoned, value)
            }
            Self::Read { resume }
            | Self::CyclePublishedRead { resume }
            | Self::SharedDelete { resume, .. } => resume.retire_in_state(state, poisoned),
            Self::Number { value, resume } => {
                state.release_owned_jsvalue(poisoned, value)?;
                resume.retire_in_state(state, poisoned)
            }
            Self::Copy { object, resume, .. } => {
                state.release_owned_jsvalue(poisoned, JsValue::Object(object))?;
                resume.retire_in_state(state, poisoned)
            }
            Self::Delete {
                object,
                key,
                resume,
            } => {
                state.release_owned_jsvalue(poisoned, JsValue::Object(object))?;
                state
                    .atoms
                    .release(key)
                    .inspect_err(|_| poisoned.set(true))?;
                resume.retire_in_state(state, poisoned)
            }
            Self::Set { progress, resume } | Self::CyclePublishedSet { progress, resume } => {
                (*progress).retire_in_state(state, poisoned)?;
                resume.retire_in_state(state, poisoned)
            }
        }
    }
    pub(crate) fn retire_at_boundary(self, runtime: &Runtime) -> Result<(), RuntimeError> {
        if runtime.skip_cleanup() {
            return Err(RuntimeError::Poisoned);
        }
        let _unwind = runtime.unwind_guard();
        if let Ok(mut state) = runtime.0.state.try_borrow_mut() {
            return self.retire_in_state(&mut state, &runtime.0.poisoned);
        }
        match self {
            Self::Complete(Completion::Return(value) | Completion::Throw(value))
            | Self::CyclePublished(Completion::Return(value) | Completion::Throw(value)) => {
                runtime.release_jsvalue(value)?;
                runtime.check_poison()
            }
            Self::Read { resume }
            | Self::CyclePublishedRead { resume }
            | Self::SharedDelete { resume, .. } => resume.retire_at_boundary(runtime),
            Self::Number { value, resume } => {
                runtime.release_jsvalue(value)?;
                runtime.check_poison()?;
                resume.retire_at_boundary(runtime)
            }
            Self::Copy { object, resume, .. } => {
                runtime.release_jsvalue(JsValue::Object(object))?;
                runtime.check_poison()?;
                resume.retire_at_boundary(runtime)
            }
            Self::Delete {
                object,
                key,
                resume,
            } => {
                runtime.release_jsvalue(JsValue::Object(object))?;
                runtime.check_poison()?;
                runtime.release_atom_handle(key);
                runtime.check_poison()?;
                resume.retire_at_boundary(runtime)
            }
            Self::Set { progress, resume } | Self::CyclePublishedSet { progress, resume } => {
                (*progress).retire_at_boundary(runtime)?;
                resume.retire_at_boundary(runtime)
            }
        }
    }
}
impl MutationResume {
    pub(crate) fn key(&self) -> Atom {
        self.0.key.expect("Array mutation effect key")
    }
    pub(crate) fn take_read_key(&mut self) -> Atom {
        self.0.key.take().expect("Array mutation read key")
    }
    pub(crate) fn select_read_in_state(
        &mut self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        domain: u64,
    ) -> Result<ReadStep, RuntimeError> {
        let object = self.0.object.expect("Array source owner");
        if self.0.read_receiver.is_none() {
            self.0.read_receiver = Some(state.dup_jsvalue(&JsValue::Object(object))?);
        }
        if self.0.read_conversion.is_none() {
            self.0.read_conversion =
                Some(state.dup_jsvalue(self.0.read_receiver.as_ref().expect("read receiver"))?);
        }
        state.prepare_ordinary_read_in_state(
            poisoned,
            domain,
            object,
            self.key(),
            self.0.read_conversion.as_ref().expect("read conversion"),
            false,
            None,
        )
    }
    /// Selection owns its independent output before either checked temporary
    /// can retire. Shared selection calls this only after the actual byte read.
    pub(crate) fn finish_read_selection_in_state(
        &mut self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        read: &OwnedRead,
    ) -> Result<(), RuntimeError> {
        if let Some(value) = self.0.read_conversion.take() {
            state.release_owned_jsvalue(poisoned, value)?;
        }
        if !matches!(read, OwnedRead::Complete(_)) {
            if let Some(value) = self.0.read_receiver.take() {
                state.release_owned_jsvalue(poisoned, value)?;
            }
            if !matches!(read, OwnedRead::Proxy { .. }) {
                self.retire_key_in_state(state, poisoned)?;
            }
        }
        Ok(())
    }
    fn argument_count(&self) -> usize {
        if self.0.inline_argument {
            1
        } else {
            self.0.arguments.len()
        }
    }
    fn argument(&self, index: usize) -> Option<&JsValue> {
        if self.0.inline_argument {
            (index == 0).then_some(&self.0.result)
        } else {
            self.0.arguments.get(index)
        }
    }
    fn resume_once(
        &mut self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        reply: Completion,
    ) -> Result<MutationAction, RuntimeError> {
        let value = match reply {
            Completion::Return(value) => value,
            Completion::Throw(value) => {
                return Ok(MutationAction::Complete(Completion::Throw(value)));
            }
        };
        self.0.pending_value = Some(value);
        match self.0.phase {
            Phase::Length => {
                self.0.phase = Phase::Number;
                Ok(MutationAction::Number(
                    self.0.pending_value.take().expect("length reply"),
                ))
            }
            Phase::Result => {
                let previous = std::mem::replace(
                    &mut self.0.result,
                    self.0.pending_value.take().expect("pop reply"),
                );
                state.release_owned_jsvalue(poisoned, previous)?;
                self.copy_next(state)
            }
            Phase::Copy => {
                state.release_owned_jsvalue(
                    poisoned,
                    self.0.pending_value.take().expect("copy reply"),
                )?;
                self.copied(state)
            }
            _ => Err(RuntimeError::Invariant(
                "Array mutation received unexpected value reply",
            )),
        }
    }
    fn number_once(
        &mut self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        reply: NativeConversion<f64>,
    ) -> Result<MutationAction, RuntimeError> {
        if !matches!(self.0.phase, Phase::Number) {
            if let NativeConversion::Throw(value) = reply {
                state.release_owned_jsvalue(poisoned, value)?;
            }
            return Err(RuntimeError::Invariant(
                "Array mutation number phase mismatch",
            ));
        }
        self.0.length = match reply {
            NativeConversion::Value(number) => Runtime::length_from_number(number),
            NativeConversion::Throw(value) => {
                return Ok(MutationAction::Complete(Completion::Throw(value)));
            }
        };
        match self.0.kind {
            MutationKind::Push(_) => {
                self.0.new_length = self.0.length.saturating_add(self.argument_count() as u64);
                if self.0.new_length > (1_u64 << 53) - 1 {
                    return self.error(state, poisoned, "Array loo long");
                }
                self.copy_next(state)
            }
            MutationKind::Pop(kind) => {
                self.0.new_length = self.0.length.saturating_sub(1);
                if self.0.length == 0 {
                    return Ok(self.write_length(state));
                }
                let index = if kind == ArrayPopKind::Shift {
                    0
                } else {
                    self.0.new_length
                };
                self.0.phase = Phase::Result;
                Ok(MutationAction::Read(
                    state.property_key_atom_for_index(index)?,
                ))
            }
        }
    }
    fn error(
        &self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        message: &'static str,
    ) -> Result<MutationAction, RuntimeError> {
        let id = state.new_native_error_from_message(
            poisoned,
            self.0.realm,
            NativeErrorKind::Type,
            NativeErrorMessage::from_utf8(message),
        )?;
        Ok(MutationAction::CyclePublished(Completion::Throw(
            JsValue::Object(id),
        )))
    }
    fn copy_next(&mut self, state: &mut RuntimeState) -> Result<MutationAction, RuntimeError> {
        let (to, from, count, backwards) = match self.0.kind {
            MutationKind::Push(ArrayPushKind::Unshift) if self.argument_count() != 0 => {
                (self.argument_count() as u64, 0, self.0.length, true)
            }
            MutationKind::Pop(ArrayPopKind::Shift) => (0, 1, self.0.new_length, false),
            _ => return self.copied(state),
        };
        self.0.phase = Phase::Copy;
        Ok(MutationAction::Copy {
            to,
            from,
            count,
            backwards,
        })
    }
    fn copied(&mut self, state: &mut RuntimeState) -> Result<MutationAction, RuntimeError> {
        self.0.cursor = 0;
        match self.0.kind {
            MutationKind::Push(_) => self.write_next(state),
            MutationKind::Pop(_) => {
                self.0.phase = Phase::DeleteLast;
                Ok(MutationAction::Delete(
                    state.property_key_atom_for_index(self.0.new_length)?,
                ))
            }
        }
    }
    fn write_next(&mut self, state: &mut RuntimeState) -> Result<MutationAction, RuntimeError> {
        if let Some(value) = self.argument(self.0.cursor as usize) {
            // The checked element role precedes index formatting/reservation.
            self.0.pending_value = Some(state.dup_jsvalue(value)?);
            let from = match self.0.kind {
                MutationKind::Push(ArrayPushKind::Unshift) if self.argument_count() != 0 => 0,
                _ => self.0.length,
            };
            self.0.phase = Phase::Write;
            let key = state.property_key_atom_for_index(from + self.0.cursor)?;
            return Ok(MutationAction::Set {
                key,
                value: self.0.pending_value.take().expect("Array element owner"),
            });
        }
        let redundant = matches!(self.0.kind, MutationKind::Push(ArrayPushKind::Push))
            && self.0.new_length <= u64::from(u32::MAX)
            && matches!(Runtime::array_length_state_in_heap(&state.heap,self.0.object.expect("Array source"),state.pinned_atoms.get(PinnedAtom::Length))?,Some((length,true))if u64::from(length)==self.0.new_length);
        if redundant {
            Ok(self.complete())
        } else {
            Ok(self.write_length(state))
        }
    }
    fn write_length(&mut self, state: &RuntimeState) -> MutationAction {
        self.0.phase = Phase::LengthWrite;
        MutationAction::Set {
            key: state.pinned_atoms.get(PinnedAtom::Length),
            value: crate::engine::value::number::operations::Number::compact(
                self.0.new_length as f64,
            )
            .into(),
        }
    }
    fn complete(&mut self) -> MutationAction {
        MutationAction::Complete(Completion::Return(match self.0.kind {
            MutationKind::Push(_) => {
                crate::engine::value::number::operations::Number::compact(self.0.new_length as f64)
                    .into()
            }
            MutationKind::Pop(_) => std::mem::replace(&mut self.0.result, JsValue::Undefined),
        }))
    }
    fn boolean_once(
        &mut self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        reply: NativeConversion<bool>,
    ) -> Result<MutationAction, RuntimeError> {
        let value = match reply {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => {
                return Ok(MutationAction::Complete(Completion::Throw(value)));
            }
        };
        if !matches!(self.0.phase, Phase::DeleteLast) {
            return Err(RuntimeError::Invariant(
                "Array mutation boolean phase mismatch",
            ));
        }
        if !value {
            return self.error(state, poisoned, "could not delete property");
        }
        Ok(self.write_length(state))
    }
    fn set_once(
        &mut self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        action: SetAction,
    ) -> Result<MutationAction, RuntimeError> {
        let result = state.finish_set_property_or_throw_in_state(
            poisoned,
            self.0.realm,
            self.key(),
            action,
        )?;
        match result {
            SetProgress::Complete(SetAction::Throw(value)) => {
                return Ok(MutationAction::Complete(Completion::Throw(value)));
            }
            SetProgress::CyclePublished(SetAction::Throw(value)) => {
                return Ok(MutationAction::CyclePublished(Completion::Throw(value)));
            }
            SetProgress::Complete(SetAction::Complete) => {}
            other => {
                other.retire_in_state(state, poisoned)?;
                return Err(RuntimeError::Invariant(
                    "strict Array Set suspended after reply",
                ));
            }
        }
        self.retire_key_in_state(state, poisoned)?;
        match self.0.phase {
            Phase::Write => {
                self.0.cursor += 1;
                self.write_next(state)
            }
            Phase::LengthWrite => Ok(self.complete()),
            _ => Err(RuntimeError::Invariant("Array mutation set phase mismatch")),
        }
    }
    pub(crate) fn resume_in_state(
        self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        reply: Completion,
    ) -> Result<MutationStep, RuntimeError> {
        let mut owner = MutationGuard::new(state, poisoned, self);
        let action = owner.resume.as_mut().expect("Array parent").resume_once(
            owner.state,
            poisoned,
            reply,
        )?;
        owner.action = Some(action);
        // Complete read's first temporary ended only after the next semantic
        // action had been selected, as in the original inline driver.
        owner
            .resume
            .as_mut()
            .expect("Array parent")
            .retire_key_in_state(owner.state, poisoned)?;
        if let Some(value) = owner.get().0.read_receiver.take() {
            owner.state.release_owned_jsvalue(poisoned, value)?;
        }
        let action = owner.action.take().expect("selected read continuation");
        owner.drive(action)
    }
    pub(crate) fn number_in_state(
        self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        reply: NativeConversion<f64>,
    ) -> Result<MutationStep, RuntimeError> {
        let mut owner = MutationGuard::new(state, poisoned, self);
        let action = owner.resume.as_mut().expect("Array parent").number_once(
            owner.state,
            poisoned,
            reply,
        )?;
        owner.drive(action)
    }
    pub(crate) fn boolean_in_state(
        self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        reply: NativeConversion<bool>,
    ) -> Result<MutationStep, RuntimeError> {
        let mut owner = MutationGuard::new(state, poisoned, self);
        let action = owner.resume.as_mut().expect("Array parent").boolean_once(
            owner.state,
            poisoned,
            reply,
        )?;
        owner.action = Some(action);
        owner
            .resume
            .as_mut()
            .expect("Array parent")
            .retire_key_in_state(owner.state, poisoned)?;
        let action = owner.action.take().expect("selected delete continuation");
        owner.drive(action)
    }
    pub(crate) fn set_in_state(
        self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        action: SetAction,
    ) -> Result<MutationStep, RuntimeError> {
        let mut owner = MutationGuard::new(state, poisoned, self);
        let action =
            owner
                .resume
                .as_mut()
                .expect("Array parent")
                .set_once(owner.state, poisoned, action)?;
        owner.drive(action)
    }
    fn retire_key_in_state(
        &mut self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
    ) -> Result<(), RuntimeError> {
        if let Some(key) = self.0.key.take() {
            state
                .atoms
                .release(key)
                .inspect_err(|_| poisoned.set(true))?;
        }
        Ok(())
    }
    pub(crate) fn retire_in_state(
        mut self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
    ) -> Result<(), RuntimeError> {
        self.0
            .retire_with(&mut |value| state.release_owned_jsvalue(poisoned, value))?;
        self.retire_key_in_state(state, poisoned)?;
        if let Some(object) = self.0.object.take() {
            state.release_owned_jsvalue(poisoned, JsValue::Object(object))?;
        }
        Ok(())
    }
    pub(crate) fn retire_at_boundary(mut self, runtime: &Runtime) -> Result<(), RuntimeError> {
        self.0.retire_with(&mut |value| {
            runtime.release_jsvalue(value)?;
            runtime.check_poison()
        })?;
        if let Some(key) = self.0.key.take() {
            runtime.release_atom_handle(key);
            runtime.check_poison()?;
        }
        if let Some(object) = self.0.object.take() {
            runtime.release_jsvalue(JsValue::Object(object))?;
            runtime.check_poison()?;
        }
        Ok(())
    }
}
impl MutationResumeState {
    fn retire_with(
        &mut self,
        release: &mut impl FnMut(JsValue) -> Result<(), RuntimeError>,
    ) -> Result<(), RuntimeError> {
        for value in &mut self.arguments {
            release(std::mem::replace(value, JsValue::Undefined))?;
        }
        self.arguments.clear();
        release(std::mem::replace(&mut self.result, JsValue::Undefined))?;
        if let Some(value) = self.pending_value.take() {
            release(value)?;
        }
        if let Some(value) = self.read_conversion.take() {
            release(value)?;
        }
        if let Some(value) = self.read_receiver.take() {
            release(value)?;
        }
        Ok(())
    }
}
struct MutationGuard<'a> {
    state: &'a mut RuntimeState,
    poisoned: &'a Cell<bool>,
    resume: Option<MutationResume>,
    action: Option<MutationAction>,
}
impl<'a> MutationGuard<'a> {
    fn new(state: &'a mut RuntimeState, poisoned: &'a Cell<bool>, resume: MutationResume) -> Self {
        Self {
            state,
            poisoned,
            resume: Some(resume),
            action: None,
        }
    }
    fn get(&mut self) -> &mut MutationResume {
        self.resume.as_mut().expect("Array mutation owner")
    }
    fn take(&mut self) -> MutationResume {
        self.resume.take().expect("Array mutation owner")
    }
    fn complete(
        mut self,
        result: Completion,
        published: bool,
    ) -> Result<MutationStep, RuntimeError> {
        let step = if published {
            MutationStep::CyclePublished(result)
        } else {
            MutationStep::Complete(result)
        };
        let mut output = MutationStepGuard {
            state: self.state,
            poisoned: self.poisoned,
            step: Some(step),
        };
        self.resume
            .take()
            .expect("Array terminal owner")
            .retire_in_state(output.state, self.poisoned)?;
        Ok(output.step.take().expect("Array completion owner"))
    }
    fn drive(mut self, mut action: MutationAction) -> Result<MutationStep, RuntimeError> {
        // A successful materialization owns its next selected Set before the
        // caller services pressure. Do not consume that next completion here.
        let mut checkpoint_next_set = false;
        loop {
            return Ok(match action {
                MutationAction::Complete(result) => return self.complete(result, false),
                MutationAction::CyclePublished(result) => return self.complete(result, true),
                MutationAction::Read(key) => {
                    self.get().0.key = Some(key);
                    MutationStep::Read {
                        resume: self.take(),
                    }
                }
                MutationAction::Number(value) => MutationStep::Number {
                    value,
                    resume: self.take(),
                },
                MutationAction::Copy {
                    to,
                    from,
                    count,
                    backwards,
                } => {
                    let object = self.get().0.object.expect("Array copy source");
                    self.state.heap.retain_object(object)?;
                    MutationStep::Copy {
                        object,
                        to,
                        from,
                        count,
                        backwards,
                        resume: self.take(),
                    }
                }
                MutationAction::Set { key, value } => {
                    self.get().0.key = Some(key);
                    self.get().0.pending_value = Some(value);
                    let object = self.get().0.object.expect("Array Set source");
                    let receiver = self.state.dup_jsvalue(&JsValue::Object(object))?;
                    self.get().0.read_receiver = Some(receiver);
                    let realm = self.get().0.realm;
                    let value = self.get().0.pending_value.take().expect("Array Set value");
                    let receiver = self
                        .get()
                        .0
                        .read_receiver
                        .take()
                        .expect("Array Set receiver");
                    let progress = self.state.start_set_borrowed(
                        self.poisoned,
                        Some(realm),
                        object,
                        key,
                        value,
                        receiver,
                    )?;
                    if checkpoint_next_set {
                        return Ok(MutationStep::CyclePublishedSet {
                            progress: Box::new(progress),
                            resume: self.take(),
                        });
                    }
                    match progress {
                        SetProgress::Complete(result)
                            if !matches!(result, SetAction::Call { .. }) =>
                        {
                            action = self.resume.as_mut().expect("Array Set parent").set_once(
                                self.state,
                                self.poisoned,
                                result,
                            )?;
                            continue;
                        }
                        SetProgress::CyclePublished(set_action)
                            if !matches!(set_action, SetAction::Call { .. }) =>
                        {
                            let next = self.resume.as_mut().expect("Array Set parent").set_once(
                                self.state,
                                self.poisoned,
                                set_action,
                            )?;
                            match next {
                                MutationAction::Complete(result)
                                | MutationAction::CyclePublished(result) => {
                                    return self.complete(result, true);
                                }
                                next @ MutationAction::Set { .. } => {
                                    action = next;
                                    checkpoint_next_set = true;
                                    continue;
                                }
                                next => {
                                    self.action = Some(next);
                                    return Err(RuntimeError::Invariant(
                                        "Array Set publication fact selected an unexpected phase",
                                    ));
                                }
                            }
                        }
                        progress => MutationStep::Set {
                            progress: Box::new(progress),
                            resume: self.take(),
                        },
                    }
                }
                MutationAction::Delete(key) => {
                    self.get().0.key = Some(key);
                    let object = self.get().0.object.expect("Array delete source");
                    if matches!(
                        self.state.heap.object(object)?.payload,
                        crate::engine::heap::ObjectPayload::Proxy(_)
                    ) {
                        self.state.heap.retain_object(object)?;
                        let key = self.get().take_read_key();
                        MutationStep::Delete {
                            object,
                            key,
                            resume: self.take(),
                        }
                    } else {
                        match self
                            .state
                            .delete_own_property_in_state(self.poisoned, object, key)?
                        {
                            DeleteOwnProperty::Complete(value) => {
                                let next_action = self
                                    .resume
                                    .as_mut()
                                    .expect("Array delete parent")
                                    .boolean_once(
                                        self.state,
                                        self.poisoned,
                                        NativeConversion::Value(value),
                                    )?;
                                self.action = Some(next_action);
                                self.resume
                                    .as_mut()
                                    .expect("Array delete parent")
                                    .retire_key_in_state(self.state, self.poisoned)?;
                                action = self.action.take().expect("selected delete continuation");
                                continue;
                            }
                            DeleteOwnProperty::Shared(word) => MutationStep::SharedDelete {
                                word,
                                resume: self.take(),
                            },
                        }
                    }
                }
            });
        }
    }
}
impl Drop for MutationGuard<'_> {
    fn drop(&mut self) {
        if std::thread::panicking() {
            self.poisoned.set(true);
        }
        if self.poisoned.get() {
            return;
        }
        let _unwind = RuntimeUnwindGuard::from_flag(self.poisoned);
        if let Some(action) = self.action.take() {
            let result = match action {
                MutationAction::Complete(Completion::Return(value) | Completion::Throw(value))
                | MutationAction::CyclePublished(
                    Completion::Return(value) | Completion::Throw(value),
                )
                | MutationAction::Number(value) => {
                    self.state.release_owned_jsvalue(self.poisoned, value)
                }
                MutationAction::Set { key, value } => self
                    .state
                    .release_owned_jsvalue(self.poisoned, value)
                    .and_then(|()| {
                        self.state
                            .atoms
                            .release(key)
                            .map(|_| ())
                            .map_err(RuntimeError::from)
                    }),
                MutationAction::Read(key) | MutationAction::Delete(key) => self
                    .state
                    .atoms
                    .release(key)
                    .map(|_| ())
                    .map_err(RuntimeError::from),
                MutationAction::Copy { .. } => Ok(()),
            };
            if result.is_err() {
                self.poisoned.set(true);
                return;
            }
        }
        if let Some(resume) = self.resume.take() {
            let _ = resume.retire_in_state(self.state, self.poisoned);
        }
    }
}
struct MutationStepGuard<'a> {
    state: &'a mut RuntimeState,
    poisoned: &'a Cell<bool>,
    step: Option<MutationStep>,
}
impl Drop for MutationStepGuard<'_> {
    fn drop(&mut self) {
        if std::thread::panicking() {
            self.poisoned.set(true);
        }
        if !self.poisoned.get()
            && let Some(step) = self.step.take()
        {
            let _unwind = RuntimeUnwindGuard::from_flag(self.poisoned);
            let _ = step.retire_in_state(self.state, self.poisoned);
        }
    }
}

impl MutationResume {
    pub(crate) fn resume(
        self,
        runtime: &Runtime,
        reply: Completion,
    ) -> Result<MutationStep, RuntimeError> {
        runtime.check_poison()?;
        let _unwind = runtime.unwind_guard();
        self.resume_in_state(
            &mut runtime.0.state.borrow_mut(),
            &runtime.0.poisoned,
            reply,
        )
    }
    pub(crate) fn number(
        self,
        runtime: &Runtime,
        reply: NativeConversion<f64>,
    ) -> Result<MutationStep, RuntimeError> {
        runtime.check_poison()?;
        let _unwind = runtime.unwind_guard();
        self.number_in_state(
            &mut runtime.0.state.borrow_mut(),
            &runtime.0.poisoned,
            reply,
        )
    }
    pub(crate) fn boolean(
        self,
        runtime: &Runtime,
        reply: NativeConversion<bool>,
    ) -> Result<MutationStep, RuntimeError> {
        runtime.check_poison()?;
        let _unwind = runtime.unwind_guard();
        self.boolean_in_state(
            &mut runtime.0.state.borrow_mut(),
            &runtime.0.poisoned,
            reply,
        )
    }
    pub(crate) fn set_boundary(
        self,
        runtime: &Runtime,
        action: SetAction,
    ) -> Result<MutationStep, RuntimeError> {
        runtime.check_poison()?;
        let _unwind = runtime.unwind_guard();
        self.set_in_state(
            &mut runtime.0.state.borrow_mut(),
            &runtime.0.poisoned,
            action,
        )
    }
}
/// The synchronous embedding adapter owns its pending record before each real
/// callback/shared/Copy boundary. It delegates the same State phases as Query.
struct MutationBoundaryGuard<'a> {
    runtime: &'a Runtime,
    step: Option<MutationStep>,
    read: Option<ReadStep>,
}
impl Drop for MutationBoundaryGuard<'_> {
    fn drop(&mut self) {
        if self.runtime.skip_cleanup() {
            return;
        }
        let _unwind = self.runtime.unwind_guard();
        if let Some(ReadStep::Ready(read) | ReadStep::CyclePublished(read)) = self.read.take() {
            if read.retire_at_boundary(self.runtime).is_err() {
                return;
            }
        }
        if let Some(step) = self.step.take() {
            let _ = step.retire_at_boundary(self.runtime);
        }
    }
}
pub(crate) fn finish(
    runtime: &Runtime,
    realm: ContextId,
    step: MutationStep,
) -> Result<Completion, RuntimeError> {
    let mut owner = MutationBoundaryGuard {
        runtime,
        step: Some(step),
        read: None,
    };
    loop {
        runtime.check_poison()?;
        let current = owner.step.take().expect("Array boundary progress");
        let cycle_published = matches!(
            current,
            MutationStep::CyclePublishedRead { .. } | MutationStep::CyclePublishedSet { .. }
        );
        match current {
            MutationStep::Complete(result) => return Ok(result),
            MutationStep::CyclePublished(result) => {
                return runtime.finish_state_native_body_step(
                    realm,
                    crate::engine::builtins::continuation::NativeStep::CyclePublishedComplete(
                        result,
                    ),
                );
            }
            MutationStep::Read { resume } | MutationStep::CyclePublishedRead { resume } => {
                owner.step = Some(MutationStep::Read { resume });
                if cycle_published {
                    runtime.collect_if_requested()?;
                }
                let Some(MutationStep::Read { resume }) = owner.step.as_mut() else {
                    unreachable!()
                };
                let _operation = runtime.operation()?;
                let selected = resume.select_read_in_state(
                    &mut runtime.0.state.borrow_mut(),
                    &runtime.0.poisoned,
                    runtime.domain_id(),
                )?;
                owner.read = Some(selected);
                if matches!(owner.read, Some(ReadStep::CyclePublished(_))) {
                    runtime.collect_if_requested()?;
                }
                if let Some(ReadStep::Shared(_)) = owner.read.as_ref() {
                    let Some(ReadStep::Shared(word)) = owner.read.take() else {
                        unreachable!()
                    };
                    let word = word.read()?;
                    owner.read = Some(ReadStep::Ready(
                        runtime.0.state.borrow_mut().own_typed_read_word(word)?,
                    ));
                }
                let read = match owner.read.as_ref().expect("Array selected read") {
                    ReadStep::Ready(read) | ReadStep::CyclePublished(read) => read,
                    ReadStep::Shared(_) => unreachable!(),
                };
                let Some(MutationStep::Read { resume }) = owner.step.as_mut() else {
                    unreachable!()
                };
                resume.finish_read_selection_in_state(
                    &mut runtime.0.state.borrow_mut(),
                    &runtime.0.poisoned,
                    read,
                )?;
                // The genuine boundary is already selected; this adapter does
                // not walk the receiver/prototype or decode the key again.
                let read = match owner.read.take().expect("Array selected read") {
                    ReadStep::Ready(read) | ReadStep::CyclePublished(read) => read,
                    ReadStep::Shared(_) => unreachable!(),
                };
                let reply = match read {
                    OwnedRead::Complete(value) => {
                        Completion::Return(value.unwrap_or(JsValue::Undefined))
                    }
                    OwnedRead::Getter { function, receiver } => {
                        let callable = crate::engine::object::CallableRef::from_validated_object(
                            ObjectRef::from_owned_handle(runtime.clone(), function),
                        );
                        runtime.call_internal_jsvalue(realm, &callable, receiver, Vec::new())?
                    }
                    read @ OwnedRead::Proxy { .. } => {
                        let Some(MutationStep::Read { resume }) = owner.step.as_mut() else {
                            unreachable!()
                        };
                        let key =
                            PropertyKey::from_owned_atom(runtime.clone(), resume.take_read_key());
                        match runtime.finish_prepared_read(
                            realm,
                            &key,
                            runtime.adopt_prepared_read(read),
                        )? {
                            NativeConversion::Value(value) => Completion::Return(
                                runtime.into_jsvalue(value.unwrap_or(Value::Undefined))?,
                            ),
                            NativeConversion::Throw(value) => Completion::Throw(value),
                        }
                    }
                };
                let Some(MutationStep::Read { resume }) = owner.step.take() else {
                    unreachable!()
                };
                owner.step = Some(resume.resume(runtime, reply)?);
            }
            MutationStep::Number { value, resume } => {
                owner.step = Some(MutationStep::Read { resume });
                let reply = runtime.native_to_number_jsvalue(realm, value)?;
                let Some(MutationStep::Read { resume }) = owner.step.take() else {
                    unreachable!()
                };
                owner.step = Some(resume.number(runtime, reply)?);
            }
            MutationStep::Copy {
                object,
                to,
                from,
                count,
                backwards,
                resume,
            } => {
                owner.step = Some(MutationStep::Read { resume });
                let object = ObjectRef::from_owned_handle(runtime.clone(), object);
                let reply = super::copy::finish(
                    runtime,
                    realm,
                    super::copy::CopyStep::start(
                        runtime, realm, object, to, from, count, backwards,
                    )?,
                )?;
                let Some(MutationStep::Read { resume }) = owner.step.take() else {
                    unreachable!()
                };
                owner.step = Some(resume.resume(runtime, reply)?);
            }
            MutationStep::Set { progress, resume }
            | MutationStep::CyclePublishedSet { progress, resume } => {
                owner.step = Some(MutationStep::Set { progress, resume });
                if cycle_published {
                    runtime.collect_if_requested()?;
                }
                let Some(MutationStep::Set { progress, resume }) = owner.step.take() else {
                    unreachable!()
                };
                owner.step = Some(MutationStep::Read { resume });
                let mut pending = crate::engine::object::SetStep::from_progress(runtime, *progress);
                let action = loop {
                    let cycle_published = matches!(
                        pending,
                        crate::engine::object::SetStep::CyclePublishedComplete(_)
                    );
                    match pending {
                        crate::engine::object::SetStep::Complete(action)
                        | crate::engine::object::SetStep::CyclePublishedComplete(action) => {
                            if cycle_published {
                                let Some(MutationStep::Read { resume }) = owner.step.take() else {
                                    unreachable!()
                                };
                                owner.step = Some(MutationStep::Set {
                                    progress: Box::new(SetProgress::CyclePublished(
                                        SetAction::from_boundary(action),
                                    )),
                                    resume,
                                });
                                // Own the actual tagged diagnostic and parent
                                // before servicing this real legacy boundary.
                                runtime.collect_if_requested()?;
                                let Some(MutationStep::Set { progress, resume }) =
                                    owner.step.take()
                                else {
                                    unreachable!()
                                };
                                let SetProgress::CyclePublished(action) = *progress else {
                                    unreachable!()
                                };
                                owner.step = Some(MutationStep::Read { resume });
                                break action;
                            }

                            if let crate::engine::object::operations::PropertySetAction::Call {
                                payload,
                            } = action
                            {
                                let (function, receiver, argument) = payload.into_parts();
                                break match runtime.call_internal_jsvalue(
                                    realm,
                                    &function,
                                    receiver,
                                    vec![argument],
                                )? {
                                    Completion::Return(value) => {
                                        runtime.release_jsvalue(value)?;
                                        runtime.check_poison()?;
                                        SetAction::Complete
                                    }
                                    Completion::Throw(value) => SetAction::Throw(value),
                                };
                            }
                            break SetAction::from_boundary(action);
                        }
                        request => pending = request.finish_sync(runtime)?,
                    }
                };
                let Some(MutationStep::Read { resume }) = owner.step.take() else {
                    unreachable!()
                };
                owner.step = Some(resume.set_boundary(runtime, action)?);
            }
            MutationStep::Delete {
                object,
                key,
                resume,
            } => {
                owner.step = Some(MutationStep::Read { resume });
                let object = ObjectRef::from_owned_handle(runtime.clone(), object);
                let key = PropertyKey::from_owned_atom(runtime.clone(), key);
                let reply = runtime.internal_delete_property(realm, &object, &key)?;
                let Some(MutationStep::Read { resume }) = owner.step.take() else {
                    unreachable!()
                };
                owner.step = Some(resume.boolean(runtime, reply)?);
            }
            MutationStep::SharedDelete { word, resume } => {
                owner.step = Some(MutationStep::Read { resume });
                word.read()?;
                let Some(MutationStep::Read { resume }) = owner.step.take() else {
                    unreachable!()
                };
                owner.step = Some(resume.boolean(runtime, NativeConversion::Value(false))?);
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_mutation_keeps_selected_setter_and_proxy_once() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        assert_eq!(context.eval(r#"(()=>{
            let trace='', stored;
            const proto={set 0(value){trace+='s';stored=value;}};
            const target=Object.create(proto);target.length=0;
            Object.defineProperty(target,'length',{get(){trace+='g';return 0;},set(value){trace+='l'+value;},configurable:true});
            if(Array.prototype.push.call(target,7)!==1 || stored!==7 || trace!=='gsl1')return false;
            trace='';const data={length:0};
            const proxy=new Proxy(data,{get(o,k,r){if(k==='length')trace+='g';return Reflect.get(o,k,r);},set(o,k,v,r){trace+='s'+k;return Reflect.set(o,k,v,r);}});
            if(Array.prototype.push.call(proxy,8)!==1 || trace!=='gs0slength' || data[0]!==8)return false;
            trace=''; const pop=Object.create({get 1(){trace+='r';return 9;}});
            Object.defineProperty(pop,'length',{get(){trace+='g';return 2;},set(v){trace+='l'+v;}});
            return Array.prototype.pop.call(pop)===9 && trace==='grl1';
        })()"#).unwrap(), Value::Bool(true));
    }

    #[test]
    fn local_mutation_keeps_partial_effects_on_rejected_length_or_delete() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        assert_eq!(context.eval(r#"(()=>{
            const a=[1,2];Object.defineProperty(a,'1',{configurable:false});
            let rejected=false;try{a.pop();}catch(e){rejected=e instanceof TypeError;}
            if(!rejected || a.length!==2 || a[1]!==2)return false;
            const target={length:0};Object.defineProperty(target,'length',{writable:false});
            rejected=false;try{Array.prototype.push.call(target,3);}catch(e){rejected=e instanceof TypeError;}
            if(!rejected || target[0]!==3 || target.length!==0)return false;
            const b=[1];Object.defineProperty(b,'length',{writable:false});
            rejected=false;try{b.push(4);}catch(e){rejected=e instanceof TypeError;}
            return rejected && b.length===1 && !(1 in b);
        })()"#).unwrap(), Value::Bool(true));
    }

    #[test]
    fn push_inline_argument_uses_actual_count_and_retains_immediate_payload() {
        let runtime = Runtime::new();
        let context = runtime.new_context().expect("create context");
        #[cfg(feature = "profiling")]
        let profile = crate::engine::api::profiling::CostProfile::start();
        for value in [
            Value::Undefined,
            Value::Null,
            Value::Bool(true),
            Value::Int(42),
            Value::Float(-0.0),
        ] {
            let array = runtime.new_array(context.realm).unwrap();
            let invocation = NativeInvocation::Call {
                this_value: runtime
                    .unroot_value(&Value::Object(array.try_clone().expect("duplicate root")))
                    .unwrap(),
            };
            let arguments = NativeArguments {
                actual_arg_count: 1,
                readable: vec![runtime.unroot_value(&value).unwrap(), JsValue::Undefined],
            };
            let step = MutationStep::start(
                &runtime,
                context.realm,
                MutationKind::Push(ArrayPushKind::Push),
                &invocation,
                &arguments,
            )
            .unwrap();
            invocation.release(&runtime).unwrap();
            let result = finish(&runtime, context.realm, step).unwrap();
            assert!(matches!(result, Completion::Return(JsValue::Int(1))));
            let key = runtime.property_key_for_index(0).unwrap();
            let Completion::Return(actual) = runtime
                .get_property_in_realm(context.realm, &array, &key)
                .unwrap()
            else {
                panic!("element read threw")
            };
            assert!(
                runtime
                    .root_value(&actual)
                    .unwrap()
                    .same_quickjs_representation(&value)
            );
        }
        let array = runtime.new_array(context.realm).unwrap();
        let invocation = NativeInvocation::Call {
            this_value: runtime.unroot_value(&Value::Object(array)).unwrap(),
        };
        let arguments = NativeArguments {
            actual_arg_count: 0,
            readable: vec![JsValue::Undefined],
        };
        let step = MutationStep::start(
            &runtime,
            context.realm,
            MutationKind::Push(ArrayPushKind::Push),
            &invocation,
            &arguments,
        )
        .unwrap();
        invocation.release(&runtime).unwrap();
        assert!(matches!(
            finish(&runtime, context.realm, step).unwrap(),
            Completion::Return(JsValue::Int(0))
        ));
        #[cfg(feature = "profiling")]
        assert_eq!(
            profile
                .snapshot()
                .owned_execution_events
                .get("array_mutation_inline_argument")
                .copied()
                .unwrap_or(0),
            5
        );
    }

    #[test]
    fn push_inline_argument_preserves_observable_mutation_steps() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        let value = context.eval(r#"(function () {
            var trace = '', stored;
            var target = {
                get length() { trace += 'g'; return { valueOf: function () { trace += 'n'; return 0; } }; },
                set length(v) { trace += 'l' + v; },
                set 0(v) { trace += 's'; stored = v; }
            };
            if (Array.prototype.push.call(target, -0) !== 1 ||
                trace !== 'gnsl1' || 1 / stored !== -Infinity) return false;
            var a = [2, 3];
            if (a.unshift(1) !== 3 || a.join(',') !== '1,2,3') return false;
            var object = {}, b = [];
            if (b.push(object) !== 1 || b[0] !== object) return false;
            if (b.push(4, 5) !== 3 || b[1] !== 4 || b[2] !== 5) return false;
            var frozen = Object.freeze([]), threw = false;
            try { frozen.push(1); } catch (e) { threw = e instanceof TypeError; }
            return threw && frozen.length === 0;
        })()"#).unwrap();
        assert!(matches!(value, Value::Bool(true)));
    }
}

#[cfg(test)]
mod state_tests;
