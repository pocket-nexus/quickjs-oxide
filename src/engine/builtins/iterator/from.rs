//! Iterator.from acquires next before the ordinary instance check.
use crate::engine::{
    api::{error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError},
    heap::ContextId,
    object::{CallableRef, ObjectRef, PropertyKey, WellKnownSymbol},
    value::{JsValue, conversion::NativeConversion},
    vm::{
        Completion,
        call::{NativeArguments, NativeInvocation},
    },
};
pub(crate) enum FromStep {
    Complete(Completion),
    Read { resume: FromResume },
    Call { resume: FromResume },
    Instance { resume: FromResume },
}
pub(crate) struct FromResume(Box<FromResumeState>);
impl std::ops::Deref for FromResume {
    type Target = FromResumeState;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for FromResume {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
const _: () = assert!(std::mem::size_of::<FromResume>() <= 8);
pub(crate) struct FromResumeState {
    runtime: Runtime,
    pending_effect: FromStepPending,
    realm: ContextId,
    phase: Phase,
    iterator: Option<JsValue>,
    next: Option<JsValue>,
}
enum Phase {
    Method,
    Iterator,
    Next,
    Instance,
}
impl Drop for FromResumeState {
    fn drop(&mut self) {
        for value in [
            self.iterator.take(),
            self.next.take(),
            self.pending_effect.read_receiver.take(),
            self.pending_effect.call_receiver.take(),
            self.pending_effect.instance_value.take(),
        ]
        .into_iter()
        .flatten()
        {
            let _ = self.runtime.release_jsvalue(value);
        }
    }
}
impl FromStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        if !matches!(invocation, NativeInvocation::Call { .. }) {
            return Err(RuntimeError::Invariant(
                "Iterator.from did not receive a generic invocation",
            ));
        }
        let input = arguments.readable.first().ok_or(RuntimeError::Invariant(
            "Iterator.from argument was not padded",
        ))?;
        if !matches!(input, JsValue::Object(_) | JsValue::String(_)) {
            return Ok(Self::Complete(Completion::Throw(
                runtime.new_native_error_jsvalue(
                    realm,
                    NativeErrorKind::Type,
                    "Iterator.from called on non-object",
                )?,
            )));
        }
        let mut resume = FromResume(Box::new(FromResumeState {
            runtime: runtime.clone(),
            pending_effect: FromStepPending::default(),
            realm,
            phase: Phase::Method,
            iterator: Some(runtime.dup_jsvalue(input)?),
            next: None,
        }));
        let key = PropertyKey::from(runtime.well_known_symbol(WellKnownSymbol::Iterator));
        let receiver = runtime.dup_jsvalue(resume.iterator.as_ref().expect("iterator input"))?;
        resume.pending_effect.read_receiver = Some(receiver);
        resume.pending_effect.read_key = Some(key);
        Ok(Self::Read { resume })
    }
}
impl FromResume {
    fn next(mut self, runtime: &Runtime) -> Result<FromStep, RuntimeError> {
        self.phase = Phase::Next;
        let key = runtime.pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Next)?;
        let receiver = runtime.dup_jsvalue(self.iterator.as_ref().expect("iterator input"))?;
        Ok(FromStep::request_read(receiver, key, self))
    }
    pub(crate) fn resume(
        mut self,
        runtime: &Runtime,
        reply: Completion,
    ) -> Result<FromStep, RuntimeError> {
        let value = match reply {
            Completion::Return(value) => value,
            Completion::Throw(value) => return Ok(FromStep::Complete(Completion::Throw(value))),
        };
        match std::mem::replace(&mut self.phase, Phase::Iterator) {
            Phase::Method => {
                if matches!(value, JsValue::Undefined | JsValue::Null) {
                    return self.next(runtime);
                }
                let callable = runtime.iterator_callable_jsvalue(self.realm, &value);
                runtime.release_jsvalue(value)?;
                let callable = match callable? {
                    NativeConversion::Value(callable) => callable,
                    NativeConversion::Throw(value) => {
                        return Ok(FromStep::Complete(Completion::Throw(
                            runtime.into_jsvalue(value)?,
                        )));
                    }
                };
                let input = self.iterator.take().expect("iterator input");
                Ok(FromStep::request_call(callable, input, self))
            }
            Phase::Iterator => {
                if !matches!(value, JsValue::Object(_)) {
                    runtime.release_jsvalue(value)?;
                    return Ok(FromStep::Complete(Completion::Throw(
                        runtime.new_native_error_jsvalue(
                            self.realm,
                            NativeErrorKind::Type,
                            "not an object",
                        )?,
                    )));
                }
                self.iterator = Some(value);
                self.next(runtime)
            }
            Phase::Next => {
                self.next = Some(value);
                let constructor = runtime.iterator_realm_data(self.realm)?.constructor;
                let constructor = CallableRef::from_validated_object(
                    ObjectRef::from_borrowed_handle(runtime.clone(), constructor)?,
                );
                self.phase = Phase::Instance;
                let iterator =
                    runtime.dup_jsvalue(self.iterator.as_ref().expect("iterator input"))?;
                Ok(FromStep::request_instance(constructor, iterator, self))
            }
            Phase::Instance => {
                let instance = runtime.value_to_boolean_jsvalue(&value);
                runtime.release_jsvalue(value)?;
                let result = if instance? {
                    self.iterator.take().expect("iterator input")
                } else {
                    JsValue::Object(
                        runtime
                            .new_iterator_wrap(
                                self.realm,
                                self.iterator.as_ref().expect("iterator input"),
                                self.next.as_ref().expect("iterator next"),
                            )?
                            .into_handle(),
                    )
                };
                Ok(FromStep::Complete(Completion::Return(result)))
            }
        }
    }
}
pub(crate) fn finish(
    runtime: &Runtime,
    realm: ContextId,
    mut step: FromStep,
) -> Result<Completion, RuntimeError> {
    loop {
        step = match step {
            FromStep::Complete(result) => return Ok(result),
            FromStep::Read { mut resume } => {
                let receiver = runtime.root_and_release_jsvalue(resume.take_read_receiver())?;
                let key = resume.take_read_key();
                resume.resume(
                    runtime,
                    runtime.get_value_property_in_realm(realm, receiver, &key)?,
                )?
            }
            FromStep::Call { mut resume } => {
                let callable = resume.take_call_callable();
                let receiver = resume.take_call_receiver();
                resume.resume(
                    runtime,
                    runtime.call_internal_jsvalue(realm, &callable, receiver, Vec::new())?,
                )?
            }
            FromStep::Instance { mut resume } => {
                let constructor = resume.take_instance_constructor();
                let value = runtime.root_and_release_jsvalue(resume.take_instance_value())?;
                resume.resume(
                    runtime,
                    runtime.ordinary_is_instance_of(realm, &constructor, value)?,
                )?
            }
        };
    }
}

#[derive(Default)]
struct FromStepPending {
    read_receiver: Option<JsValue>,
    read_key: Option<PropertyKey>,
    call_callable: Option<CallableRef>,
    call_receiver: Option<JsValue>,
    instance_constructor: Option<CallableRef>,
    instance_value: Option<JsValue>,
}
impl FromStep {
    pub(crate) fn request_read(
        receiver: JsValue,
        key: PropertyKey,
        mut resume: FromResume,
    ) -> Self {
        resume.0.pending_effect.read_receiver = Some(receiver);
        resume.0.pending_effect.read_key = Some(key);
        Self::Read { resume }
    }
    pub(crate) fn request_call(
        callable: CallableRef,
        receiver: JsValue,
        mut resume: FromResume,
    ) -> Self {
        resume.0.pending_effect.call_callable = Some(callable);
        resume.0.pending_effect.call_receiver = Some(receiver);
        Self::Call { resume }
    }
    pub(crate) fn request_instance(
        constructor: CallableRef,
        value: JsValue,
        mut resume: FromResume,
    ) -> Self {
        resume.0.pending_effect.instance_constructor = Some(constructor);
        resume.0.pending_effect.instance_value = Some(value);
        Self::Instance { resume }
    }
}
impl FromResume {
    pub(crate) fn take_read_receiver(&mut self) -> JsValue {
        self.0
            .pending_effect
            .read_receiver
            .take()
            .expect("FromStep Read receiver")
    }
    pub(crate) fn take_read_key(&mut self) -> PropertyKey {
        self.0
            .pending_effect
            .read_key
            .take()
            .expect("FromStep Read key")
    }
    pub(crate) fn take_call_callable(&mut self) -> CallableRef {
        self.0
            .pending_effect
            .call_callable
            .take()
            .expect("FromStep Call callable")
    }
    pub(crate) fn take_call_receiver(&mut self) -> JsValue {
        self.0
            .pending_effect
            .call_receiver
            .take()
            .expect("FromStep Call receiver")
    }
    pub(crate) fn take_instance_constructor(&mut self) -> CallableRef {
        self.0
            .pending_effect
            .instance_constructor
            .take()
            .expect("FromStep Instance constructor")
    }
    pub(crate) fn take_instance_value(&mut self) -> JsValue {
        self.0
            .pending_effect
            .instance_value
            .take()
            .expect("FromStep Instance value")
    }
}
const _: () = assert!(std::mem::size_of::<FromStep>() <= 56);

// S11 all-domain protocol bound; inline completion stays allocation-free.
const _: () = assert!(std::mem::size_of::<FromStep>() <= 64);
