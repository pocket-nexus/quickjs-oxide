//! ToNumber uses the canonical raw ToPrimitive protocol and defining error realm.
use super::primitive::{PrimitiveResume, PrimitiveStep};
use super::*;
use crate::engine::{
    atom::Atom,
    heap::{
        ObjectId,
        runtime::{RuntimeState, owned_values::OwnedValueGuard},
    },
    object::{CallableRef, ObjectRef},
    value::JsValue,
};
use std::cell::Cell;

pub(crate) enum NumberStep {
    Complete(NativeConversion<f64>),
    CyclePublished(NativeConversion<f64>),
    Read { resume: NumberResume },
    Call { resume: NumberResume },
}
pub(crate) struct NumberResume(Box<NumberResumeState>);
pub(crate) struct NumberResumeState {
    pending_effect: NumberStepPending,
    realm: ContextId,
    primitive: Option<PrimitiveResume>,
}
impl NumberStep {
    /// The actual boundary may overlap an admitted State lease. Coordinate
    /// those releases through the existing FIFO rather than quarantining a
    /// normal borrow conflict; destructive failure stops the remaining owners.
    pub(crate) fn retire_at_boundary(self, runtime: &Runtime) -> Result<(), RuntimeError> {
        if runtime.skip_cleanup() {
            return Err(RuntimeError::Poisoned);
        }
        let _unwind = runtime.unwind_guard();
        if let Ok(mut state) = runtime.0.state.try_borrow_mut() {
            return self.retire_in_state(&mut state, &runtime.0.poisoned);
        }
        let release = |value| {
            runtime.release_jsvalue(value)?;
            runtime.check_poison()
        };
        match self {
            Self::Complete(NativeConversion::Value(_))
            | Self::CyclePublished(NativeConversion::Value(_)) => Ok(()),
            Self::Complete(NativeConversion::Throw(value))
            | Self::CyclePublished(NativeConversion::Throw(value)) => release(value),
            Self::Read { resume } | Self::Call { resume } => resume.retire_at_boundary(runtime),
        }
    }

    pub(crate) fn start_jsvalue(
        runtime: &Runtime,
        realm: ContextId,
        value: JsValue,
    ) -> Result<Self, RuntimeError> {
        let step = PrimitiveResume::start(runtime, realm, value, ToPrimitiveHint::Number)?;
        let _unwind = runtime.unwind_guard();
        let step = from_primitive_in_state(
            &mut runtime.0.state.borrow_mut(),
            &runtime.0.poisoned,
            realm,
            step,
        )?;
        // Legacy synchronous consumers accept Complete. Consume the actual
        // publication at this external boundary while the existing finisher
        // keeps its throw owner armed; resident producers retain the tag.
        if matches!(step, Self::CyclePublished(_)) {
            Ok(Self::Complete(step.finish(runtime, realm)?))
        } else {
            Ok(step)
        }
    }
    pub(crate) fn start_jsvalue_in_state(
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        realm: ContextId,
        value: JsValue,
    ) -> Result<Self, RuntimeError> {
        let step = PrimitiveResume::start_in_state(
            state,
            poisoned,
            realm,
            value,
            ToPrimitiveHint::Number,
        )?;
        from_primitive_in_state(state, poisoned, realm, step)
    }
    /// Synchronous embedding consumer. Raw phases stay protected across the
    /// actual property/call boundary; the guard borrows the existing header.
    pub(crate) fn finish(
        self,
        runtime: &Runtime,
        realm: ContextId,
    ) -> Result<NativeConversion<f64>, RuntimeError> {
        let mut owner = NumberBoundaryGuard {
            runtime,
            step: Some(self),
        };
        loop {
            let step = owner.step.take().expect("number boundary progress");
            match step {
                Self::Complete(result) => return Ok(result),
                Self::CyclePublished(result) => {
                    owner.step = Some(Self::CyclePublished(result));
                    runtime.collect_if_requested()?;
                    let Self::CyclePublished(result) =
                        owner.step.take().expect("published number completion")
                    else {
                        unreachable!()
                    };
                    return Ok(result);
                }
                Self::Read { resume } => {
                    owner.step = Some(Self::Read { resume });
                    let Some(Self::Read { resume }) = owner.step.as_mut() else {
                        unreachable!()
                    };
                    let object = resume.take_read_object(runtime);
                    let key = resume.take_read_key(runtime);
                    let completion = runtime.get_property_in_realm(realm, &object, &key)?;
                    let Some(Self::Read { resume }) = owner.step.take() else {
                        unreachable!()
                    };
                    owner.step = Some(resume.resume(runtime, completion)?);
                }
                Self::Call { resume } => {
                    owner.step = Some(Self::Call { resume });
                    let Some(Self::Call { resume }) = owner.step.as_mut() else {
                        unreachable!()
                    };
                    let callable = resume.take_call_callable(runtime);
                    let receiver = resume.take_call_receiver();
                    let arguments = resume.take_call_arguments();
                    let completion =
                        runtime.call_internal_jsvalue(realm, &callable, receiver, arguments)?;
                    let Some(Self::Call { resume }) = owner.step.take() else {
                        unreachable!()
                    };
                    owner.step = Some(resume.resume(runtime, completion)?);
                }
            }
        }
    }
    pub(crate) fn retire_in_state(
        self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
    ) -> Result<(), RuntimeError> {
        match self {
            Self::Complete(NativeConversion::Throw(value))
            | Self::CyclePublished(NativeConversion::Throw(value)) => {
                state.release_owned_jsvalue(poisoned, value)
            }
            Self::Complete(NativeConversion::Value(_))
            | Self::CyclePublished(NativeConversion::Value(_)) => Ok(()),
            Self::Read { resume } | Self::Call { resume } => {
                resume.retire_in_state(state, poisoned)
            }
        }
    }
}
fn completed_primitive_in_state(
    state: &mut RuntimeState,
    poisoned: &Cell<bool>,
    realm: ContextId,
    completion: Completion,
) -> Result<NumberStep, RuntimeError> {
    let value = match completion {
        Completion::Throw(value) => {
            return Ok(NumberStep::Complete(NativeConversion::Throw(value)));
        }
        Completion::Return(value) => value,
    };
    let mut reply = OwnedValueGuard::new(state, poisoned, value);
    let (state, reply) = reply.parts();
    let converted = state.number_from_primitive_jsvalue_with_publication(
        poisoned,
        realm,
        reply.as_ref().expect("number primitive reply"),
    );
    if poisoned.get() {
        return Err(RuntimeError::Poisoned);
    }
    match converted {
        Ok(super::NumberPrimitiveStep::CyclePublishedThrow(value)) => {
            let mut output = OwnedValueGuard::new(state, poisoned, value);
            let (state, output) = output.parts();
            state.release_owned_jsvalue(poisoned, reply.take().expect("number primitive reply"))?;
            Ok(NumberStep::CyclePublished(NativeConversion::Throw(
                output.take().expect("number conversion diagnostic"),
            )))
        }
        converted => {
            state.release_owned_jsvalue(poisoned, reply.take().expect("number primitive reply"))?;
            converted.map(|value| NumberStep::Complete(value.into_conversion()))
        }
    }
}

fn from_primitive_in_state(
    state: &mut RuntimeState,
    poisoned: &Cell<bool>,
    realm: ContextId,
    step: PrimitiveStep,
) -> Result<NumberStep, RuntimeError> {
    Ok(match step {
        PrimitiveStep::CyclePublished(completion) => {
            match completed_primitive_in_state(state, poisoned, realm, completion)? {
                NumberStep::Complete(value) | NumberStep::CyclePublished(value) => {
                    NumberStep::CyclePublished(value)
                }
                NumberStep::Read { .. } | NumberStep::Call { .. } => {
                    unreachable!("primitive suffix does not wait")
                }
            }
        }
        PrimitiveStep::Complete(completion) => {
            completed_primitive_in_state(state, poisoned, realm, completion)?
        }
        PrimitiveStep::Get { mut resume } => {
            let (object, key) = resume.take_get_in_state();
            NumberStep::Read {
                resume: NumberResume(Box::new(NumberResumeState {
                    pending_effect: NumberStepPending {
                        read_object: Some(object),
                        read_key: Some(key),
                        ..NumberStepPending::default()
                    },
                    realm,
                    primitive: Some(resume),
                })),
            }
        }
        PrimitiveStep::Call { mut resume } => {
            let callable = resume.take_callable_in_state();
            let receiver = resume.take_receiver();
            let arguments = resume.take_arguments();
            NumberStep::Call {
                resume: NumberResume(Box::new(NumberResumeState {
                    pending_effect: NumberStepPending {
                        call_callable: Some(callable),
                        call_receiver: Some(receiver),
                        call_arguments: Some(arguments),
                        ..NumberStepPending::default()
                    },
                    realm,
                    primitive: Some(resume),
                })),
            }
        }
    })
}
impl NumberResume {
    pub(crate) fn resume(
        self,
        runtime: &Runtime,
        completion: Completion,
    ) -> Result<NumberStep, RuntimeError> {
        let _unwind = runtime.unwind_guard();
        self.resume_in_state(
            &mut runtime.0.state.borrow_mut(),
            &runtime.0.poisoned,
            completion,
        )
    }
    pub(crate) fn resume_in_state(
        mut self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        completion: Completion,
    ) -> Result<NumberStep, RuntimeError> {
        let result = self
            .0
            .primitive
            .take()
            .expect("number primitive owner")
            .resume_in_state(state, poisoned, completion)
            .and_then(|step| from_primitive_in_state(state, poisoned, self.0.realm, step));
        // Pending request owners are retired after the next semantic state has
        // been formed, matching the old pending-field Drop timing.
        if !poisoned.get() {
            self.0.pending_effect.retire_in_state(state, poisoned)?;
        }
        result
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
        self.0.pending_effect.retire_with(release)?;
        if let Some(mut primitive) = self.0.primitive.take() {
            primitive.retire_with(release)?;
        }
        Ok(())
    }
    pub(crate) fn retire_at_boundary(self, runtime: &Runtime) -> Result<(), RuntimeError> {
        self.retire_with(&mut |value| {
            if runtime.skip_cleanup() {
                return Err(RuntimeError::Poisoned);
            }
            runtime.release_jsvalue(value)?;
            if runtime.is_poisoned() {
                Err(RuntimeError::Poisoned)
            } else {
                Ok(())
            }
        })
    }
    pub(crate) fn release_owned(self, runtime: &Runtime) {
        if runtime.skip_cleanup() {
            return;
        }
        let _unwind = runtime.unwind_guard();
        if let Ok(mut state) = runtime.0.state.try_borrow_mut() {
            let _ = self.retire_in_state(&mut state, &runtime.0.poisoned);
        } else {
            let _ = self.retire_at_boundary(runtime);
        }
    }
    pub(crate) fn read_in_state(&self) -> (ObjectId, Atom) {
        (
            self.0
                .pending_effect
                .read_object
                .expect("NumberStep Read object"),
            self.0.pending_effect.read_key.expect("NumberStep Read key"),
        )
    }
    pub(crate) fn take_read_in_state(&mut self) -> (ObjectId, Atom) {
        (
            self.0
                .pending_effect
                .read_object
                .take()
                .expect("NumberStep Read object"),
            self.0
                .pending_effect
                .read_key
                .take()
                .expect("NumberStep Read key"),
        )
    }
    pub(crate) fn take_call_in_state(&mut self) -> (ObjectId, JsValue, Vec<JsValue>) {
        (
            self.0
                .pending_effect
                .call_callable
                .take()
                .expect("NumberStep Call callable"),
            self.take_call_receiver(),
            self.take_call_arguments(),
        )
    }
    pub(crate) fn take_read_object(&mut self, runtime: &Runtime) -> ObjectRef {
        let _unwind = runtime.unwind_guard();
        ObjectRef::from_owned_handle(
            runtime.clone(),
            self.0
                .pending_effect
                .read_object
                .take()
                .expect("NumberStep Read object"),
        )
    }
    pub(crate) fn take_read_key(&mut self, runtime: &Runtime) -> PropertyKey {
        let _unwind = runtime.unwind_guard();
        PropertyKey::from_owned_atom(
            runtime.clone(),
            self.0
                .pending_effect
                .read_key
                .take()
                .expect("NumberStep Read key"),
        )
    }
    pub(crate) fn take_call_callable(&mut self, runtime: &Runtime) -> CallableRef {
        let _unwind = runtime.unwind_guard();
        CallableRef::from_validated_object(ObjectRef::from_owned_handle(
            runtime.clone(),
            self.0
                .pending_effect
                .call_callable
                .take()
                .expect("NumberStep Call callable"),
        ))
    }
    pub(crate) fn take_call_receiver(&mut self) -> JsValue {
        self.0
            .pending_effect
            .call_receiver
            .take()
            .expect("NumberStep Call receiver")
    }
    pub(crate) fn take_call_arguments(&mut self) -> Vec<JsValue> {
        self.0
            .pending_effect
            .call_arguments
            .take()
            .expect("NumberStep Call arguments")
    }
}
#[derive(Default)]
struct NumberStepPending {
    read_object: Option<ObjectId>,
    read_key: Option<Atom>,
    call_callable: Option<ObjectId>,
    call_receiver: Option<JsValue>,
    call_arguments: Option<Vec<JsValue>>,
}
impl NumberStepPending {
    fn retire_in_state(
        &mut self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
    ) -> Result<(), RuntimeError> {
        self.retire_with(&mut |value| state.release_owned_jsvalue(poisoned, value))
    }
    fn retire_with(
        &mut self,
        release: &mut impl FnMut(JsValue) -> Result<(), RuntimeError>,
    ) -> Result<(), RuntimeError> {
        if let Some(receiver) = self.call_receiver.take() {
            release(receiver)?;
        }
        if let Some(arguments) = &mut self.call_arguments {
            for argument in arguments {
                release(std::mem::replace(argument, JsValue::Undefined))?;
            }
        }
        self.call_arguments = None;
        if let Some(object) = self.read_object.take() {
            release(JsValue::Object(object))?;
        }
        self.read_key = None;
        if let Some(function) = self.call_callable.take() {
            release(JsValue::Object(function))?;
        }
        Ok(())
    }
}
const _: () = assert!(std::mem::size_of::<NumberResume>() <= 8);
const _: () = assert!(std::mem::size_of::<NumberStep>() <= 64);

struct NumberBoundaryGuard<'a> {
    runtime: &'a Runtime,
    step: Option<NumberStep>,
}
impl Drop for NumberBoundaryGuard<'_> {
    fn drop(&mut self) {
        if self.runtime.skip_cleanup() {
            return;
        }
        let _unwind = self.runtime.unwind_guard();
        if let Some(step) = self.step.take() {
            let _ = step.retire_at_boundary(self.runtime);
        }
    }
}
