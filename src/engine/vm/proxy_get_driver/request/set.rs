//! Finite raw Set reply slots in the existing Query protocol.
use super::{Completion, JsValue, Resume, Runtime, Step};
use crate::engine::{
    api::RuntimeError,
    heap::runtime::RuntimeState,
    object::{SetAction, SetProgress, SetWait},
};
use std::cell::Cell;

/// Assignment ingress has only a Set result or a selected diagnostic. Query
/// and the local instruction adapt this same producer to their real storage.
#[must_use]
pub(in crate::engine::vm) enum ValueSetStart {
    Progress(SetProgress),
    Diagnostic(crate::engine::api::Error),
}

pub(in crate::engine::vm) fn start_value_set_in_state(
    state: &mut RuntimeState,
    poisoned: &Cell<bool>,
    realm: crate::engine::heap::ContextId,
    atom: crate::engine::atom::Atom,
    value: &mut Option<JsValue>,
    receiver: &mut Option<JsValue>,
) -> Result<ValueSetStart, RuntimeError> {
    let target = match receiver.as_ref().expect("write receiver") {
        JsValue::Object(id) => *id,
        JsValue::Null | JsValue::Undefined => {
            let suffix = if matches!(receiver.as_ref(), Some(JsValue::Null)) {
                "' of null"
            } else {
                "' of undefined"
            };
            // Historical assignment consumes value then base before formatting
            // the nullish diagnostic. A fatal first release stops the suffix.
            retire_value_set_inputs(state, poisoned, value, receiver)?;
            return state
                .native_atom_error(
                    crate::engine::api::error::ErrorKind::Type,
                    "cannot set property '",
                    atom,
                    suffix,
                )
                .map(ValueSetStart::Diagnostic);
        }
        primitive => {
            use crate::engine::builtins::native::PrimitiveKind;
            let kind = match primitive {
                JsValue::Bool(_) => PrimitiveKind::Boolean,
                JsValue::Int(_) | JsValue::Float(_) => PrimitiveKind::Number,
                JsValue::String(_) => PrimitiveKind::String,
                JsValue::ShortBigInt(_) | JsValue::BigInt(_) => PrimitiveKind::BigInt,
                JsValue::Symbol(_) => PrimitiveKind::Symbol,
                _ => unreachable!(),
            };
            state.primitive_prototype_id_for_realm(realm, kind)?
        }
    };
    state
        .start_set_borrowed(
            poisoned,
            Some(realm),
            target,
            atom,
            value.take().expect("write value"),
            receiver.take().expect("write receiver"),
        )
        .map(ValueSetStart::Progress)
}

pub(in crate::engine::vm) fn retire_value_set_inputs(
    state: &mut RuntimeState,
    poisoned: &Cell<bool>,
    value: &mut Option<JsValue>,
    receiver: &mut Option<JsValue>,
) -> Result<(), RuntimeError> {
    if poisoned.get() {
        return Err(RuntimeError::Poisoned);
    }
    if let Some(value) = value.take() {
        state
            .release_owned_jsvalue(poisoned, value)
            .map_err(|_| RuntimeError::Poisoned)?;
    }
    if let Some(receiver) = receiver.take() {
        state
            .release_owned_jsvalue(poisoned, receiver)
            .map_err(|_| RuntimeError::Poisoned)?;
    }
    Ok(())
}

/// Existing Query storage keeps its inputs armed through prototype selection.
pub(in crate::engine::vm::proxy_get_driver) fn start_value_set_step_in_state(
    state: &mut RuntimeState,
    poisoned: &Cell<bool>,
    realm: crate::engine::heap::ContextId,
    step: &mut Step,
) -> Result<(), RuntimeError> {
    let Step::ValueSet {
        atom,
        value,
        receiver,
    } = step
    else {
        return Err(RuntimeError::Invariant("local write lost its owned inputs"));
    };
    *step = match start_value_set_in_state(state, poisoned, realm, *atom, value, receiver)? {
        ValueSetStart::Progress(progress) => Step::SetProgress(Some(progress)),
        ValueSetStart::Diagnostic(error) => Step::WriteError(Some(error)),
    };
    Ok(())
}

/// Both consumers use the exact assignment formatter; Call is transported
/// separately before this helper, and a propagated Throw stays an owned result.
pub(in crate::engine::vm) fn finish_set_action_in_state(
    state: &mut RuntimeState,
    poisoned: &Cell<bool>,
    atom: crate::engine::atom::Atom,
    strict: bool,
    action: SetAction,
) -> Result<Completion, RuntimeError> {
    let result = match action.into_result() {
        Ok(result) => result,
        Err(action) => {
            action.retire(state, poisoned)?;
            return Err(RuntimeError::Invariant(
                "setter result bypassed callback consumer",
            ));
        }
    };
    state.finish_property_set_in_state(result, atom, strict)
}

pub(in crate::engine::vm) fn setter_call_step(
    function: crate::engine::heap::ObjectId,
    receiver: JsValue,
    argument: JsValue,
) -> Step {
    Step::RawCall {
        inputs: Some(crate::engine::vm::call::ordinary::RawCallbackInputs::new(
            function,
            receiver,
            vec![argument],
        )),
        resume: Some(Resume::Setter),
    }
}

/// Advance the actual Set owner through Continue phases. A terminal action or
/// selected wait remains armed in this slot until its real consumer takes it.
pub(in crate::engine::vm) fn advance_set_progress_in_state(
    state: &mut RuntimeState,
    poisoned: &Cell<bool>,
    progress: &mut Option<SetProgress>,
) -> Result<bool, RuntimeError> {
    loop {
        match progress.as_mut().expect("raw Set progress") {
            SetProgress::Waiting {
                phase: SetWait::Continue,
                ..
            } => {
                let SetProgress::Waiting { resume, .. } =
                    progress.take().expect("raw Set progress")
                else {
                    unreachable!()
                };
                *progress = Some(resume.advance_in_state(state, poisoned)?);
            }
            SetProgress::Waiting { resume, .. } => return Ok(resume.take_cycle_published()),
            SetProgress::Complete(_) => return Ok(false),
            SetProgress::CyclePublished(_) => return Ok(true),
        }
    }
}

impl From<SetProgress> for Step {
    fn from(progress: SetProgress) -> Self {
        Self::SetProgress(Some(progress))
    }
}

/// Only an object key transfers the original suffix before ToPrimitive. A
/// primitive key holds no base/value here: its checked duplicate is converted
/// while the original operands remain in the actual caller window.
#[must_use]
pub(in crate::engine::vm) struct WriteKeyInputs {
    pub(in crate::engine::vm) realm: crate::engine::heap::ContextId,
    pub(in crate::engine::vm) operands: Option<Box<WriteKeyOperands>>,
}
pub(in crate::engine::vm) struct WriteKeyOperands {
    pub(in crate::engine::vm) base: Option<JsValue>,
    pub(in crate::engine::vm) value: Option<JsValue>,
}
impl WriteKeyInputs {
    pub(in crate::engine::vm) fn object(realm: crate::engine::heap::ContextId) -> Self {
        Self {
            realm,
            operands: Some(Box::new(WriteKeyOperands {
                base: None,
                value: None,
            })),
        }
    }
    pub(in crate::engine::vm) fn retire_in_state(
        &mut self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
    ) -> Result<(), RuntimeError> {
        if poisoned.get() {
            return Err(RuntimeError::Poisoned);
        }
        let Some(operands) = &mut self.operands else {
            return Ok(());
        };
        if let Some(base) = operands.base.take() {
            state
                .release_owned_jsvalue(poisoned, base)
                .map_err(|_| RuntimeError::Poisoned)?;
        }
        if let Some(value) = operands.value.take() {
            state
                .release_owned_jsvalue(poisoned, value)
                .map_err(|_| RuntimeError::Poisoned)?;
        }
        Ok(())
    }
    pub(super) fn retire_at_boundary(&mut self, runtime: &Runtime) -> Result<(), RuntimeError> {
        runtime.check_poison()?;
        let Some(operands) = &mut self.operands else {
            return Ok(());
        };
        if let Some(base) = operands.base.take() {
            runtime.release_jsvalue(base)?;
            runtime.check_poison()?;
        }
        if let Some(value) = operands.value.take() {
            runtime.release_jsvalue(value)?;
            runtime.check_poison()?;
        }
        Ok(())
    }
    pub(super) fn reply_in_state(
        mut self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        reply: Completion,
    ) -> Result<Step, RuntimeError> {
        use crate::engine::value::conversion::property_key::PropertyKeyAtomStep;
        let _unwind = crate::engine::api::runtime::RuntimeUnwindGuard::from_flag(poisoned);
        let result = match reply {
            Completion::Throw(value) => {
                let mut value = crate::engine::heap::runtime::owned_values::OwnedValueGuard::new(
                    state, poisoned, value,
                );
                let (state, reply) = value.parts();
                self.retire_in_state(state, poisoned)?;
                return Ok(Step::Complete(Some(Completion::Throw(
                    reply.take().expect("write key throw"),
                ))));
            }
            Completion::Return(value) => state
                .property_key_from_primitive_jsvalue_with_publication(poisoned, self.realm, value),
        };
        if poisoned.get() {
            return Err(RuntimeError::Poisoned);
        }
        match result {
            Ok(PropertyKeyAtomStep::Value(atom)) => Ok(Step::WriteOperands {
                atom: Some(atom),
                input: Some(self),
            }),
            Ok(PropertyKeyAtomStep::CyclePublishedThrow(value)) => {
                let mut value = crate::engine::heap::runtime::owned_values::OwnedValueGuard::new(
                    state, poisoned, value,
                );
                let (state, reply) = value.parts();
                self.retire_in_state(state, poisoned)?;
                Ok(Step::CyclePublishedComplete(Some(Completion::Throw(
                    reply.take().expect("write suffix throw"),
                ))))
            }
            Err(error) => {
                self.retire_in_state(state, poisoned)?;
                Err(error)
            }
        }
    }
    pub(super) fn reply_at_boundary(
        self,
        runtime: &Runtime,
        reply: Completion,
    ) -> Result<Step, RuntimeError> {
        let mut owner = WriteKeyReplyGuard {
            runtime,
            input: Some(self),
            reply: Some(reply),
        };
        let _unwind = runtime.unwind_guard();
        runtime.check_poison()?;
        let mut state = match runtime.0.state.try_borrow_mut() {
            Ok(state) => state,
            Err(_) => {
                owner.retire()?;
                return Err(RuntimeError::Invariant(
                    "write key reply state is already borrowed",
                ));
            }
        };
        let result = owner
            .input
            .take()
            .expect("write key domain")
            .reply_in_state(
                &mut state,
                &runtime.0.poisoned,
                owner.reply.take().expect("write key reply"),
            );
        drop(state);
        runtime.check_poison()?;
        result
    }
}
struct WriteKeyReplyGuard<'a> {
    runtime: &'a Runtime,
    input: Option<WriteKeyInputs>,
    reply: Option<Completion>,
}
impl WriteKeyReplyGuard<'_> {
    fn retire(&mut self) -> Result<(), RuntimeError> {
        self.runtime.check_poison()?;
        if let Some(Completion::Return(value) | Completion::Throw(value)) = self.reply.take() {
            self.runtime.release_jsvalue(value)?;
            self.runtime.check_poison()?;
        }
        if let Some(input) = &mut self.input {
            input.retire_at_boundary(self.runtime)?;
        }
        Ok(())
    }
}
impl Drop for WriteKeyReplyGuard<'_> {
    fn drop(&mut self) {
        if !self.runtime.skip_cleanup() {
            let _ = self.retire();
        }
    }
}

impl Resume {
    pub(in crate::engine::vm::proxy_get_driver) fn can_set_in_state(&self) -> bool {
        matches!(
            self,
            Self::OrdinarySet(_) | Self::RootSet | Self::ArrayMutation(_) | Self::RegExpExec(_)
        )
    }
    pub(in crate::engine::vm::proxy_get_driver) fn set_in_state(
        self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        action: SetAction,
    ) -> Result<Step, RuntimeError> {
        match self {
            Self::RegExpExec(resume) => resume
                .set_in_state(state, poisoned, action)
                .and_then(Step::try_from),
            Self::OrdinarySet(resume) => resume
                .forward_in_state(state, poisoned, action)
                .map(Step::from),
            Self::ArrayMutation(resume) => {
                resume.set_in_state(state, poisoned, action).map(Step::from)
            }
            Self::RootSet => Ok(Step::Complete(Some(match action {
                SetAction::Throw(value) => Completion::Throw(value),
                SetAction::Complete => Completion::Return(JsValue::Bool(true)),
                SetAction::Rejected(_) | SetAction::RejectedProxyTrap => {
                    Completion::Return(JsValue::Bool(false))
                }
                action @ SetAction::Call { .. } => {
                    action.retire(state, poisoned)?;
                    return Err(RuntimeError::Invariant(
                        "setter call bypassed raw callback consumer",
                    ));
                }
            }))),
            mut resume => {
                action.retire(state, poisoned)?;
                resume.retire_raw_in_state(state, poisoned)?;
                Err(RuntimeError::Invariant(
                    "unmigrated Set reply consumer entered State",
                ))
            }
        }
    }
}

/// A selected legacy effect can fail before transferring its raw domain. This
/// guard borrows the actual boundary Runtime and owns only that one Set record.
struct BoundaryResume<'a> {
    runtime: &'a Runtime,
    resume: Option<crate::engine::object::SetResume>,
}
impl BoundaryResume<'_> {
    fn get(&mut self) -> &mut crate::engine::object::SetResume {
        self.resume.as_mut().expect("selected Set domain")
    }
    fn take(&mut self) -> crate::engine::object::SetResume {
        self.resume.take().expect("selected Set domain")
    }
    fn retire(&mut self) -> Result<(), RuntimeError> {
        self.runtime.check_poison()?;
        if let Some(resume) = self.resume.take() {
            resume.retire_at_boundary(self.runtime)?;
        }
        Ok(())
    }
}
impl Drop for BoundaryResume<'_> {
    fn drop(&mut self) {
        if !self.runtime.skip_cleanup() {
            let _ = self.retire();
        }
    }
}

/// Actual selected effects adapt only after State ends. No property/index or
/// prototype lookup occurs here, and each request edge transfers exactly once.
pub(in crate::engine::vm::proxy_get_driver) fn consume_boundary(
    runtime: &Runtime,
    query: &mut super::super::Query,
    progress: SetProgress,
) -> Result<Step, RuntimeError> {
    let SetProgress::Waiting { phase, resume } = progress else {
        return Ok(Step::from(progress));
    };
    let _unwind = runtime.unwind_guard();
    let mut owner = BoundaryResume {
        runtime,
        resume: Some(resume),
    };
    runtime.check_poison()?;
    let result = match phase {
        SetWait::Continue => owner.take().advance(runtime).and_then(Step::try_from),
        SetWait::Proxy => {
            let object = owner.get().take_object(runtime);
            let key = owner.get().take_key(runtime);
            let value = owner.get().take_value();
            let receiver = owner.get().take_receiver();
            Ok(Step::SetProxy {
                object: Some(object),
                key: Some(key),
                value: Some(value),
                receiver: Some(receiver),
                resume: Some(Resume::OrdinarySet(owner.take())),
            })
        }
        SetWait::Special => {
            if let Some(word) = owner.get().take_shared_word() {
                let word = match word.read() {
                    Ok(word) => word,
                    Err(error) => {
                        owner.retire()?;
                        return Err(error);
                    }
                };
                return owner.take().shared(runtime, word).and_then(Step::try_from);
            }
            let child = match owner.get().start_selected_typed(runtime) {
                Ok(child) => child,
                Err(error) => {
                    owner.retire()?;
                    return Err(error);
                }
            };
            if let Err(error) = owner.get().retire_request_at_boundary(runtime) {
                if !runtime.skip_cleanup() {
                    child.retire_at_boundary(runtime)?;
                }
                owner.retire()?;
                return Err(error);
            }
            if query.parents.try_reserve(1).is_err() {
                child.retire_at_boundary(runtime)?;
                owner.retire()?;
                return Err(RuntimeError::Invariant(
                    "property continuation allocation failed",
                ));
            }
            query.parents.push(Resume::SetTyped(owner.take()));
            child.try_into()
        }
        SetWait::ArrayLength => {
            // Exact selected request roles retire without a new root header or
            // length relookup. Domain/receiver protect the actual reply target.
            if let Err(error) = owner.get().retire_request_headers_at_boundary(runtime) {
                owner.retire()?;
                return Err(error);
            }
            Ok(Step::SetLength {
                value: Some(owner.get().take_value()),
                resume: Some(owner.take()),
            })
        }
        SetWait::Descriptor => Ok(Step::Descriptor {
            object: Some(owner.get().take_object(runtime)),
            key: Some(owner.get().take_key(runtime)),
            resume: Some(Resume::OrdinarySet(owner.take())),
        }),
        SetWait::Define => Ok(Step::Define {
            object: Some(owner.get().take_object(runtime)),
            key: Some(owner.get().take_key(runtime)),
            descriptor: Some(owner.get().take_descriptor(runtime).into()),
            resume: Some(Resume::OrdinarySet(owner.take())),
        }),
    };
    runtime.check_poison()?;
    result
}
