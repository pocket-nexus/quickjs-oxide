//! QuickJS build_arg_list: raw carrier, fixed length and ordered indexed reads.
use crate::engine::{
    api::{
        error::{NativeErrorKind, NativeErrorMessage},
        runtime::Runtime,
        runtime_error::RuntimeError,
    },
    atom::{Atom, pinned::PinnedAtom},
    heap::{
        ContextId, HeapError, ObjectId,
        runtime::{
            RuntimeState,
            owned_values::{OwnedValueGuard, OwnedValuesGuard},
        },
    },
    value::{JsValue, conversion::NativeConversion},
    vm::Completion,
};
use std::cell::Cell;

const MAX_APPLY_ARGUMENTS: u64 = 65_534;

pub(crate) enum ArgumentsStep {
    Complete(NativeConversion<Vec<JsValue>>),
    /// Only a freshly allocated Type/Range Error produces this fact.
    CyclePublished(NativeConversion<Vec<JsValue>>),
    Read {
        resume: ArgumentsResume,
    },
    Number {
        resume: ArgumentsResume,
    },
}
pub(crate) struct ArgumentsResume(Box<ArgumentsResumeState>);
const _: () = assert!(std::mem::size_of::<ArgumentsResume>() <= 8);
struct ArgumentsResumeState {
    realm: ContextId,
    carrier: Option<ObjectId>,
    number_value: Option<JsValue>,
    requested_object: Option<ObjectId>,
    // Length is pinned; bounded list indices are immediate atoms. Neither
    // request introduces an independently allocated atom owner.
    requested_key: Option<Atom>,
    phase: Phase,
}
enum Phase {
    Length,
    Number,
    Item { length: usize, values: Vec<JsValue> },
}
enum Effect {
    Read,
    Number,
    Complete(NativeConversion<Vec<JsValue>>, bool),
}
impl ArgumentsStep {
    pub(crate) fn start_in_state(
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        realm: ContextId,
        value: JsValue,
    ) -> Result<Self, RuntimeError> {
        let mut input = OwnedValueGuard::new(state, poisoned, value);
        let (state, input) = input.parts();
        let Some(JsValue::Object(carrier)) = input.as_ref() else {
            state.release_owned_jsvalue(poisoned, input.take().expect("arguments input"))?;
            return Ok(Self::CyclePublished(NativeConversion::Throw(
                JsValue::Object(state.new_native_error_from_message(
                    poisoned,
                    realm,
                    NativeErrorKind::Type,
                    NativeErrorMessage::from_utf8("not a object"),
                )?),
            )));
        };
        let carrier = *carrier;
        if let Some(result) =
            state.prepare_fast_array_arguments_jsvalue(poisoned, realm, carrier)?
        {
            let fresh = matches!(result, NativeConversion::Throw(_));
            let owner = ArgumentsResume(Box::new(ArgumentsResumeState {
                realm,
                carrier: Some(carrier),
                number_value: None,
                requested_object: None,
                requested_key: None,
                phase: Phase::Length,
            }));
            input.take();
            return owner.finish_effect(state, poisoned, Ok(Effect::Complete(result, fresh)));
        }
        let JsValue::Object(requested_object) = state.dup_jsvalue(&JsValue::Object(carrier))?
        else {
            unreachable!()
        };
        let requested_key = state.pinned_atoms.get(PinnedAtom::Length);
        let resume = ArgumentsResume(Box::new(ArgumentsResumeState {
            realm,
            carrier: Some(carrier),
            number_value: None,
            requested_object: Some(requested_object),
            requested_key: Some(requested_key),
            phase: Phase::Length,
        }));
        input.take();
        Ok(Self::Read { resume })
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
            Self::Complete(NativeConversion::Throw(value))
            | Self::CyclePublished(NativeConversion::Throw(value)) => release(value),
            Self::Complete(NativeConversion::Value(values))
            | Self::CyclePublished(NativeConversion::Value(values)) => {
                for value in values {
                    release(value)?;
                }
                Ok(())
            }
            Self::Read { resume } | Self::Number { resume } => resume.retire_with(release),
        }
    }
    pub(crate) fn retire_at_boundary(self, runtime: &Runtime) -> Result<(), RuntimeError> {
        let _unwind = runtime.unwind_guard();
        if runtime.skip_cleanup() {
            return Err(RuntimeError::Poisoned);
        }
        if let Ok(mut state) = runtime.0.state.try_borrow_mut() {
            return self.retire_in_state(&mut state, &runtime.0.poisoned);
        }
        self.retire_with(&mut |value| {
            runtime.release_jsvalue(value)?;
            runtime.check_poison()
        })
    }
}
impl ArgumentsResume {
    pub(crate) fn read_in_state(&self) -> (ObjectId, Atom) {
        (
            self.0.requested_object.expect("argument read object"),
            self.0.requested_key.expect("argument read key"),
        )
    }
    pub(crate) fn take_read_in_state(&mut self) -> (ObjectId, Atom) {
        (
            self.0
                .requested_object
                .take()
                .expect("argument read object"),
            self.0.requested_key.take().expect("argument read key"),
        )
    }
    pub(crate) fn take_number_value(&mut self) -> JsValue {
        self.0.number_value.take().expect("arguments number")
    }
    pub(crate) fn read(
        self,
        runtime: &Runtime,
        result: Completion,
    ) -> Result<ArgumentsStep, RuntimeError> {
        let _unwind = runtime.unwind_guard();
        let result = self.resume_in_state(
            &mut runtime.0.state.borrow_mut(),
            &runtime.0.poisoned,
            result,
        );
        runtime.check_poison()?;
        result
    }
    pub(crate) fn number(
        self,
        runtime: &Runtime,
        result: NativeConversion<f64>,
    ) -> Result<ArgumentsStep, RuntimeError> {
        let _unwind = runtime.unwind_guard();
        let result = self.number_in_state(
            &mut runtime.0.state.borrow_mut(),
            &runtime.0.poisoned,
            result,
        );
        runtime.check_poison()?;
        result
    }
    pub(crate) fn resume_in_state(
        mut self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        completion: Completion,
    ) -> Result<ArgumentsStep, RuntimeError> {
        let result = self.advance_read(state, poisoned, completion);
        self.finish_effect(state, poisoned, result)
    }
    fn advance_read(
        &mut self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        result: Completion,
    ) -> Result<Effect, RuntimeError> {
        let value = match result {
            Completion::Throw(value) => {
                return Ok(Effect::Complete(NativeConversion::Throw(value), false));
            }
            Completion::Return(value) => value,
        };
        match &mut self.0.phase {
            Phase::Length => {
                self.0.phase = Phase::Number;
                self.0.number_value = Some(value);
                Ok(Effect::Number)
            }
            Phase::Item { values, .. } => {
                values.push(value);
                #[cfg(feature = "profiling")]
                crate::engine::api::profiling::record_call_buffer_moves("apply.indexed", 1);
                self.next(state)
            }
            Phase::Number => {
                state.release_owned_jsvalue(poisoned, value)?;
                Err(RuntimeError::Invariant(
                    "argument length received an untyped reply",
                ))
            }
        }
    }
    pub(crate) fn number_in_state(
        mut self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        result: NativeConversion<f64>,
    ) -> Result<ArgumentsStep, RuntimeError> {
        let next = self.advance_number(state, poisoned, result);
        self.finish_effect(state, poisoned, next)
    }
    fn advance_number(
        &mut self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        result: NativeConversion<f64>,
    ) -> Result<Effect, RuntimeError> {
        if !matches!(self.0.phase, Phase::Number) {
            if let NativeConversion::Throw(value) = result {
                state.release_owned_jsvalue(poisoned, value)?;
            }
            return Err(RuntimeError::Invariant(
                "argument number reply has no length phase",
            ));
        }
        let number = match result {
            NativeConversion::Throw(value) => {
                return Ok(Effect::Complete(NativeConversion::Throw(value), false));
            }
            NativeConversion::Value(number) => number,
        };
        let length = Runtime::length_from_number(number);
        if length > MAX_APPLY_ARGUMENTS {
            return Ok(Effect::Complete(
                NativeConversion::Throw(JsValue::Object(state.new_native_error_from_message(
                    poisoned,
                    self.0.realm,
                    NativeErrorKind::Range,
                    NativeErrorMessage::from_utf8(
                        "too many arguments in function call (only 65534 allowed)",
                    ),
                )?)),
                true,
            ));
        }
        let length = usize::try_from(length)
            .map_err(|_| RuntimeError::Invariant("argument-list length does not fit usize"))?;
        if let Some(values) = state.fast_array_like_values_jsvalue(
            poisoned,
            self.0.carrier.expect("arguments carrier"),
            length as u32,
        )? {
            return Ok(Effect::Complete(NativeConversion::Value(values), false));
        }
        let mut values = Vec::new();
        values
            .try_reserve_exact(length)
            .map_err(|_| HeapError::Allocation {
                operation: "reserving the argument-list snapshot",
            })?;
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_call_buffer_capacity(
            "apply.indexed",
            0,
            values.capacity(),
            size_of::<JsValue>(),
        );
        self.0.phase = Phase::Item { length, values };
        self.next(state)
    }
    fn next(&mut self, state: &mut RuntimeState) -> Result<Effect, RuntimeError> {
        let Phase::Item { length, values } = &mut self.0.phase else {
            unreachable!()
        };
        if values.len() == *length {
            return Ok(Effect::Complete(
                NativeConversion::Value(std::mem::take(values)),
                false,
            ));
        }
        let key =
            Atom::from_immediate_integer(values.len() as u32).expect("bounded argument index");
        let JsValue::Object(object) =
            state.dup_jsvalue(&JsValue::Object(self.0.carrier.expect("arguments carrier")))?
        else {
            unreachable!()
        };
        self.0.requested_key = Some(key);
        self.0.requested_object = Some(object);
        Ok(Effect::Read)
    }
    fn finish_effect(
        self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        result: Result<Effect, RuntimeError>,
    ) -> Result<ArgumentsStep, RuntimeError> {
        if poisoned.get() {
            return Err(RuntimeError::Poisoned);
        }
        match result {
            Ok(Effect::Read) => Ok(ArgumentsStep::Read { resume: self }),
            Ok(Effect::Number) => Ok(ArgumentsStep::Number { resume: self }),
            Ok(Effect::Complete(result, fresh)) => {
                let result = match result {
                    NativeConversion::Throw(value) => {
                        let mut value = OwnedValueGuard::new(state, poisoned, value);
                        let (state, value) = value.parts();
                        self.retire_in_state(state, poisoned)?;
                        NativeConversion::Throw(value.take().expect("arguments throw"))
                    }
                    NativeConversion::Value(values) => {
                        let mut values = OwnedValuesGuard::new(state, poisoned, values);
                        let (state, values) = values.parts();
                        self.retire_in_state(state, poisoned)?;
                        NativeConversion::Value(std::mem::take(values))
                    }
                };
                Ok(if fresh {
                    ArgumentsStep::CyclePublished(result)
                } else {
                    ArgumentsStep::Complete(result)
                })
            }
            Err(error) => {
                self.retire_in_state(state, poisoned)?;
                Err(error)
            }
        }
    }
    pub(crate) fn retire_in_state(
        self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
    ) -> Result<(), RuntimeError> {
        self.retire_with(&mut |value| state.release_owned_jsvalue(poisoned, value))
    }
    fn retire_with(
        mut self,
        release: &mut impl FnMut(JsValue) -> Result<(), RuntimeError>,
    ) -> Result<(), RuntimeError> {
        if let Some(value) = self.0.number_value.take() {
            release(value)?;
        }
        if let Phase::Item { values, .. } = &mut self.0.phase {
            for value in values.drain(..) {
                release(value)?;
            }
        }
        if let Some(carrier) = self.0.carrier.take() {
            release(JsValue::Object(carrier))?;
        }
        if let Some(requested) = self.0.requested_object.take() {
            release(JsValue::Object(requested))?;
        }
        self.0.requested_key.take();
        Ok(())
    }
    pub(crate) fn retire_at_boundary(self, runtime: &Runtime) -> Result<(), RuntimeError> {
        self.retire_with(&mut |value| {
            if runtime.skip_cleanup() {
                return Err(RuntimeError::Poisoned);
            }
            runtime.release_jsvalue(value)?;
            runtime.check_poison()
        })
    }
}
const _: () = assert!(std::mem::size_of::<ArgumentsStep>() <= 64);
