//! Owned ToPrimitive phases. A reply consumes its continuation exactly once.
use super::*;
use crate::engine::{
    api::error::NativeErrorMessage,
    atom::{Atom, pinned::PinnedAtom},
    heap::{
        ObjectId,
        runtime::{RuntimeState, owned_values::OwnedValueGuard},
    },
    object::{CallableRef, ObjectRef},
    value::JsValue,
};
use std::cell::Cell;

pub(crate) enum PrimitiveStep {
    Get {
        resume: PrimitiveResume,
    },
    Call {
        resume: PrimitiveResume,
    },
    Complete(Completion),
    /// This completion owns a freshly published collectible Error node.
    CyclePublished(Completion),
}
pub(crate) struct PrimitiveResume(Box<PrimitiveResumeState>);
pub(crate) struct PrimitiveResumeState {
    object: Option<ObjectId>,
    realm: ContextId,
    hint: ToPrimitiveHint,
    phase: Phase,
    requested_object: Option<ObjectId>,
    // These are permanent runtime intrinsic atoms, borrowed for this domain.
    requested_key: Option<Atom>,
    requested_callable: Option<ObjectId>,
    requested_receiver: Option<JsValue>,
    requested_arguments: Vec<JsValue>,
}
#[derive(Clone, Copy)]
enum Phase {
    ExoticMethod,
    ExoticResult,
    OrdinaryMethod(bool),
    OrdinaryResult(bool),
}
enum Effect {
    Get,
    Call,
    Complete(Completion),
    /// This completion owns a freshly published collectible Error node.
    CyclePublished(Completion),
}

impl PrimitiveResume {
    pub(crate) fn get_in_state(&self) -> (ObjectId, Atom) {
        (
            self.0.requested_object.expect("primitive get object"),
            self.0.requested_key.expect("primitive get key"),
        )
    }
    pub(crate) fn take_get_in_state(&mut self) -> (ObjectId, Atom) {
        (
            self.0
                .requested_object
                .take()
                .expect("primitive get object"),
            self.0.requested_key.take().expect("primitive get key"),
        )
    }
    pub(crate) fn take_callable_in_state(&mut self) -> ObjectId {
        self.0
            .requested_callable
            .take()
            .expect("primitive call callee")
    }
    pub(crate) fn take_receiver(&mut self) -> JsValue {
        self.0
            .requested_receiver
            .take()
            .expect("primitive call receiver")
    }
    pub(crate) fn take_arguments(&mut self) -> Vec<JsValue> {
        std::mem::take(&mut self.0.requested_arguments)
    }
    /// Root adaptation occurs only in an existing unmigrated/embedding consumer.
    pub(crate) fn take_get(&mut self, runtime: &Runtime) -> (ObjectRef, PropertyKey) {
        let _unwind = runtime.unwind_guard();
        let (object, key) = self.take_get_in_state();
        (
            ObjectRef::from_owned_handle(runtime.clone(), object),
            PropertyKey::from_owned_atom(runtime.clone(), key),
        )
    }
    pub(crate) fn take_callable(&mut self, runtime: &Runtime) -> CallableRef {
        let _unwind = runtime.unwind_guard();
        CallableRef::from_validated_object(ObjectRef::from_owned_handle(
            runtime.clone(),
            self.take_callable_in_state(),
        ))
    }
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        value: JsValue,
        hint: ToPrimitiveHint,
    ) -> Result<PrimitiveStep, RuntimeError> {
        if !matches!(value, JsValue::Object(_)) {
            return Ok(PrimitiveStep::Complete(Completion::Return(value)));
        }
        let mut input = PrimitiveBoundaryGuard {
            runtime,
            step: Some(PrimitiveStep::Complete(Completion::Return(value))),
        };
        let _operation = runtime.operation()?;
        let PrimitiveStep::Complete(Completion::Return(value)) =
            input.step.take().expect("primitive boundary input")
        else {
            unreachable!()
        };
        Self::start_in_state(
            &mut runtime.0.state.borrow_mut(),
            &runtime.0.poisoned,
            realm,
            value,
            hint,
        )
    }
    pub(crate) fn start_in_state(
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        realm: ContextId,
        value: JsValue,
        hint: ToPrimitiveHint,
    ) -> Result<PrimitiveStep, RuntimeError> {
        let JsValue::Object(object) = value else {
            return Ok(PrimitiveStep::Complete(Completion::Return(value)));
        };
        let mut input = OwnedValueGuard::new(state, poisoned, JsValue::Object(object));
        let (state, input) = input.parts();
        let key = state.well_known_symbols[&WellKnownSymbol::ToPrimitive];
        let requested = state.dup_jsvalue(input.as_ref().expect("primitive receiver"))?;
        let JsValue::Object(requested) = requested else {
            unreachable!()
        };
        let resume = Self(Box::new(PrimitiveResumeState {
            object: Some(object),
            realm,
            hint,
            phase: Phase::ExoticMethod,
            requested_object: Some(requested),
            requested_key: Some(key),
            requested_callable: None,
            requested_receiver: None,
            requested_arguments: Vec::new(),
        }));
        input.take();
        Ok(PrimitiveStep::Get { resume })
    }
    pub(crate) fn ordinary(
        runtime: &Runtime,
        realm: ContextId,
        object: ObjectRef,
        hint: ToPrimitiveHint,
    ) -> Result<PrimitiveStep, RuntimeError> {
        let _unwind = runtime.unwind_guard();
        // Public-root transfer can coordinate or retain its original edge.
        // Complete it before the admitted State lease begins.
        let object = object.into_handle();
        Self::ordinary_in_state(
            &mut runtime.0.state.borrow_mut(),
            &runtime.0.poisoned,
            realm,
            object,
            hint,
        )
    }
    pub(crate) fn ordinary_in_state(
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        realm: ContextId,
        object: ObjectId,
        hint: ToPrimitiveHint,
    ) -> Result<PrimitiveStep, RuntimeError> {
        let mut owner = PrimitiveStateGuard::new(
            state,
            poisoned,
            Self(Box::new(PrimitiveResumeState {
                object: Some(object),
                realm,
                hint,
                phase: Phase::OrdinaryMethod(false),
                requested_object: None,
                requested_key: None,
                requested_callable: None,
                requested_receiver: None,
                requested_arguments: Vec::new(),
            })),
        );
        let (state, resume) = owner.parts();
        resume.0.read_ordinary_in_state(state, false)?;
        Ok(PrimitiveStep::Get {
            resume: owner.into_inner(),
        })
    }
    pub(crate) fn resume(
        self,
        runtime: &Runtime,
        completion: Completion,
    ) -> Result<PrimitiveStep, RuntimeError> {
        let _unwind = runtime.unwind_guard();
        self.resume_in_state(
            &mut runtime.0.state.borrow_mut(),
            &runtime.0.poisoned,
            completion,
        )
    }
    pub(crate) fn resume_in_state(
        self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        completion: Completion,
    ) -> Result<PrimitiveStep, RuntimeError> {
        let mut owner = PrimitiveStateGuard::new(state, poisoned, self);
        let effect = {
            let (state, resume) = owner.parts();
            resume.0.advance_in_state(state, poisoned, completion)?
        };
        Ok(match effect {
            Effect::Get => PrimitiveStep::Get {
                resume: owner.into_inner(),
            },
            Effect::Call => PrimitiveStep::Call {
                resume: owner.into_inner(),
            },
            Effect::Complete(completion) => owner.finish_complete(completion)?,
            Effect::CyclePublished(completion) => {
                let PrimitiveStep::Complete(completion) = owner.finish_complete(completion)? else {
                    unreachable!()
                };
                PrimitiveStep::CyclePublished(completion)
            }
        })
    }
    pub(crate) fn retire_in_state(
        mut self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
    ) -> Result<(), RuntimeError> {
        self.0.retire_in_state(state, poisoned)
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
    pub(super) fn retire_with(
        &mut self,
        release: &mut impl FnMut(JsValue) -> Result<(), RuntimeError>,
    ) -> Result<(), RuntimeError> {
        self.0.retire_with(release)
    }
    pub(crate) fn retire_at_boundary(mut self, runtime: &Runtime) -> Result<(), RuntimeError> {
        self.0.retire_with(&mut |value| {
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
}
impl PrimitiveResumeState {
    fn read_ordinary_in_state(
        &mut self,
        state: &mut RuntimeState,
        second: bool,
    ) -> Result<(), RuntimeError> {
        let string_first = matches!(self.hint, ToPrimitiveHint::String);
        let name = if string_first != second {
            PinnedAtom::ToString
        } else {
            PinnedAtom::ValueOf
        };
        let key = state.pinned_atoms.get(name);
        let JsValue::Object(object) =
            state.dup_jsvalue(&JsValue::Object(self.object.expect("primitive receiver")))?
        else {
            unreachable!()
        };
        self.phase = Phase::OrdinaryMethod(second);
        self.requested_object = Some(object);
        self.requested_key = Some(key);
        Ok(())
    }
    fn type_error(
        &self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        message: &str,
    ) -> Result<Effect, RuntimeError> {
        Ok(Effect::CyclePublished(Completion::Throw(JsValue::Object(
            state.new_native_error_from_message(
                poisoned,
                self.realm,
                NativeErrorKind::Type,
                NativeErrorMessage::from_utf8(message),
            )?,
        ))))
    }
    fn failed_method(
        &mut self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        second: bool,
    ) -> Result<Effect, RuntimeError> {
        if second {
            self.type_error(state, poisoned, "toPrimitive")
        } else {
            self.read_ordinary_in_state(state, true)?;
            Ok(Effect::Get)
        }
    }
    fn advance_in_state(
        &mut self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        completion: Completion,
    ) -> Result<Effect, RuntimeError> {
        let value = match completion {
            Completion::Throw(value) => return Ok(Effect::Complete(Completion::Throw(value))),
            Completion::Return(value) => value,
        };
        let mut reply = OwnedValueGuard::new(state, poisoned, value);
        let (state, reply) = reply.parts();
        match self.phase {
            Phase::ExoticMethod
                if matches!(reply.as_ref(), Some(JsValue::Undefined | JsValue::Null)) =>
            {
                state.release_owned_jsvalue(poisoned, reply.take().expect("primitive reply"))?;
                self.read_ordinary_in_state(state, false)?;
                Ok(Effect::Get)
            }
            Phase::ExoticMethod | Phase::OrdinaryMethod(_) => {
                let Some(JsValue::Object(method)) = reply.as_ref() else {
                    state
                        .release_owned_jsvalue(poisoned, reply.take().expect("primitive reply"))?;
                    return match self.phase {
                        Phase::ExoticMethod => self.type_error(state, poisoned, "not a function"),
                        Phase::OrdinaryMethod(second) => {
                            self.failed_method(state, poisoned, second)
                        }
                        _ => unreachable!(),
                    };
                };
                let method_id = *method;
                // Preserve both checked method and callable retains and their order.
                let method = state.dup_jsvalue(&JsValue::Object(method_id))?;
                let mut method_owner = OwnedValueGuard::new(state, poisoned, method);
                let (state, method_owner) = method_owner.parts();
                state.release_owned_jsvalue(poisoned, reply.take().expect("method reply"))?;
                if !state.object_id_has_call_capability(method_id)? {
                    let effect = match self.phase {
                        Phase::ExoticMethod => self.type_error(state, poisoned, "not a function"),
                        Phase::OrdinaryMethod(second) => {
                            self.failed_method(state, poisoned, second)
                        }
                        _ => unreachable!(),
                    };
                    if poisoned.get() {
                        return effect.and(Err(RuntimeError::Poisoned));
                    }
                    state.release_owned_jsvalue(
                        poisoned,
                        method_owner.take().expect("method temporary"),
                    )?;
                    return effect;
                }
                let callable =
                    state.dup_jsvalue(method_owner.as_ref().expect("method temporary"))?;
                let mut callable = OwnedValueGuard::new(state, poisoned, callable);
                let (state, callable) = callable.parts();
                let original = self.object.expect("primitive receiver");
                let receiver = if matches!(self.phase, Phase::ExoticMethod) {
                    let hint = JsString::from_static(match self.hint {
                        ToPrimitiveHint::String => "string",
                        ToPrimitiveHint::Number => "number",
                        ToPrimitiveHint::Default => "default",
                    });
                    let hint = JsValue::String(state.heap.allocate_string(hint)?);
                    // Allocation precedes receiver retain as before, now with an owner.
                    let mut hint = OwnedValueGuard::new(state, poisoned, hint);
                    let (state, hint) = hint.parts();
                    let receiver = state.dup_jsvalue(&JsValue::Object(original))?;
                    self.requested_arguments = vec![hint.take().expect("primitive hint owner")];
                    self.phase = Phase::ExoticResult;
                    receiver
                } else {
                    let Phase::OrdinaryMethod(second) = self.phase else {
                        unreachable!()
                    };
                    let receiver = state.dup_jsvalue(&JsValue::Object(original))?;
                    self.phase = Phase::OrdinaryResult(second);
                    receiver
                };
                let Some(JsValue::Object(function)) = callable.take() else {
                    unreachable!()
                };
                self.requested_callable = Some(function);
                self.requested_receiver = Some(receiver);
                state.release_owned_jsvalue(
                    poisoned,
                    method_owner.take().expect("method temporary"),
                )?;
                Ok(Effect::Call)
            }
            Phase::ExoticResult | Phase::OrdinaryResult(_) => {
                if matches!(reply.as_ref(), Some(JsValue::Object(_))) {
                    state
                        .release_owned_jsvalue(poisoned, reply.take().expect("primitive result"))?;
                    match self.phase {
                        Phase::ExoticResult => self.type_error(state, poisoned, "toPrimitive"),
                        Phase::OrdinaryResult(second) => {
                            self.failed_method(state, poisoned, second)
                        }
                        _ => unreachable!(),
                    }
                } else {
                    Ok(Effect::Complete(Completion::Return(
                        reply.take().expect("primitive result"),
                    )))
                }
            }
        }
    }
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
        if let Some(receiver) = self.requested_receiver.take() {
            release(receiver)?;
        }
        // Retire only the current edge; poisoning stops before the suffix.
        for value in &mut self.requested_arguments {
            release(std::mem::replace(value, JsValue::Undefined))?;
        }
        self.requested_arguments.clear();
        for owner in [
            &mut self.object,
            &mut self.requested_object,
            &mut self.requested_callable,
        ] {
            if let Some(id) = owner.take() {
                release(JsValue::Object(id))?;
            }
        }
        self.requested_key = None;
        Ok(())
    }
}
impl PrimitiveStep {
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
            Self::Complete(Completion::Return(value) | Completion::Throw(value))
            | Self::CyclePublished(Completion::Return(value) | Completion::Throw(value)) => {
                release(value)
            }
            Self::Get { resume } | Self::Call { resume } => resume.retire_at_boundary(runtime),
        }
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
            Self::Get { resume } | Self::Call { resume } => resume.retire_in_state(state, poisoned),
        }
    }
}

/// Concrete transient owner. It never reacquires State or keeps Runtime alive.
struct PrimitiveStateGuard<'a> {
    state: &'a mut RuntimeState,
    poisoned: &'a Cell<bool>,
    resume: Option<PrimitiveResume>,
}
impl<'a> PrimitiveStateGuard<'a> {
    fn new(state: &'a mut RuntimeState, poisoned: &'a Cell<bool>, resume: PrimitiveResume) -> Self {
        Self {
            state,
            poisoned,
            resume: Some(resume),
        }
    }
    fn parts(&mut self) -> (&mut RuntimeState, &mut PrimitiveResume) {
        (
            self.state,
            self.resume.as_mut().expect("primitive state owner"),
        )
    }
    fn into_inner(mut self) -> PrimitiveResume {
        self.resume.take().expect("primitive state owner")
    }
    fn finish_complete(mut self, completion: Completion) -> Result<PrimitiveStep, RuntimeError> {
        let (value, thrown) = match completion {
            Completion::Return(value) => (value, false),
            Completion::Throw(value) => (value, true),
        };
        let mut output = OwnedValueGuard::new(self.state, self.poisoned, value);
        let (state, output) = output.parts();
        self.resume
            .take()
            .expect("primitive state owner")
            .retire_in_state(state, self.poisoned)?;
        let value = output.take().expect("primitive completion owner");
        Ok(PrimitiveStep::Complete(if thrown {
            Completion::Throw(value)
        } else {
            Completion::Return(value)
        }))
    }
}
impl Drop for PrimitiveStateGuard<'_> {
    fn drop(&mut self) {
        if std::thread::panicking() {
            self.poisoned.set(true);
        }
        if self.poisoned.get() {
            return;
        }
        if let Some(resume) = self.resume.take() {
            let _ = resume.retire_in_state(self.state, self.poisoned);
        }
    }
}
struct PrimitiveBoundaryGuard<'a> {
    runtime: &'a Runtime,
    step: Option<PrimitiveStep>,
}
impl Drop for PrimitiveBoundaryGuard<'_> {
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
impl Runtime {
    pub(in crate::engine) fn finish_primitive_steps(
        &self,
        realm: ContextId,
        step: PrimitiveStep,
    ) -> Result<Completion, RuntimeError> {
        let mut owner = PrimitiveBoundaryGuard {
            runtime: self,
            step: Some(step),
        };
        loop {
            match owner.step.take().expect("primitive boundary progress") {
                PrimitiveStep::Complete(completion) => return Ok(completion),
                PrimitiveStep::CyclePublished(completion) => {
                    owner.step = Some(PrimitiveStep::CyclePublished(completion));
                    self.collect_if_requested()?;
                    let PrimitiveStep::CyclePublished(completion) =
                        owner.step.take().expect("published primitive completion")
                    else {
                        unreachable!()
                    };
                    return Ok(completion);
                }
                PrimitiveStep::Get { resume } => {
                    let mut pending = PrimitiveBoundaryGuard {
                        runtime: self,
                        step: Some(PrimitiveStep::Get { resume }),
                    };
                    let Some(PrimitiveStep::Get { resume }) = pending.step.as_mut() else {
                        unreachable!()
                    };
                    let (object, key) = resume.take_get(self);
                    let completion = self.get_property_in_realm(realm, &object, &key)?;
                    let Some(PrimitiveStep::Get { resume }) = pending.step.take() else {
                        unreachable!()
                    };
                    owner.step = Some(resume.resume(self, completion)?);
                }
                PrimitiveStep::Call { resume } => {
                    let mut pending = PrimitiveBoundaryGuard {
                        runtime: self,
                        step: Some(PrimitiveStep::Call { resume }),
                    };
                    let Some(PrimitiveStep::Call { resume }) = pending.step.as_mut() else {
                        unreachable!()
                    };
                    let callable = resume.take_callable(self);
                    let receiver = resume.take_receiver();
                    let arguments = resume.take_arguments();
                    let completion =
                        self.call_internal_jsvalue(realm, &callable, receiver, arguments)?;
                    let Some(PrimitiveStep::Call { resume }) = pending.step.take() else {
                        unreachable!()
                    };
                    owner.step = Some(resume.resume(self, completion)?);
                }
            }
        }
    }
    pub(crate) fn ordinary_to_primitive(
        &self,
        realm: ContextId,
        object: &ObjectRef,
        hint: ToPrimitiveHint,
    ) -> Result<Completion, RuntimeError> {
        let step = PrimitiveResume::ordinary(self, realm, object.try_clone()?, hint)?;
        self.finish_primitive_steps(realm, step)
    }
}
const _: () = assert!(std::mem::size_of::<PrimitiveResume>() <= 8);
const _: () = assert!(std::mem::size_of::<PrimitiveStep>() <= 64);

#[cfg(test)]
mod resident_request_tests {
    use super::*;
    #[test]
    fn property_and_call_requests_reuse_the_primitive_resume_allocation() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        let value = context.eval("({valueOf(){return 7}})").unwrap();
        let value = runtime.unroot_value(&value).unwrap();
        let PrimitiveStep::Get { mut resume } =
            PrimitiveResume::start(&runtime, context.realm, value, ToPrimitiveHint::Number)
                .expect("prepare primitive")
        else {
            panic!("first get")
        };
        let address = &*resume.0 as *const PrimitiveResumeState;
        let (object, key) = resume.take_get(&runtime);
        let completion = runtime
            .get_property_in_realm(context.realm, &object, &key)
            .unwrap();
        let PrimitiveStep::Get { mut resume } = resume.resume(&runtime, completion).unwrap() else {
            panic!("ordinary get")
        };
        assert_eq!(&*resume.0 as *const PrimitiveResumeState, address);
        let (object, key) = resume.take_get(&runtime);
        let completion = runtime
            .get_property_in_realm(context.realm, &object, &key)
            .unwrap();
        let PrimitiveStep::Call { mut resume } = resume.resume(&runtime, completion).unwrap()
        else {
            panic!("ordinary call")
        };
        assert_eq!(&*resume.0 as *const PrimitiveResumeState, address);
        let callable = resume.take_callable(&runtime);
        let receiver = runtime
            .root_and_release_jsvalue(resume.take_receiver())
            .unwrap();
        let arguments = resume
            .take_arguments()
            .into_iter()
            .map(|argument| runtime.root_and_release_jsvalue(argument).unwrap())
            .collect::<Vec<_>>();
        let completion = runtime
            .call_internal(context.realm, &callable, receiver, &arguments)
            .unwrap();
        assert!(matches!(
            resume.resume(&runtime, completion).unwrap(),
            PrimitiveStep::Complete(Completion::Return(JsValue::Int(7)))
        ));
    }
}
