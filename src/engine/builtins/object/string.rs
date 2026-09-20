//! Object string conversions retain tag fallback and method lookup across callbacks.

use crate::engine::builtins::native::NativeFunctionId;
use crate::engine::{
    api::{error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError},
    heap::ContextId,
    object::{PropertyKey, WellKnownSymbol},
    value::{JsString, JsValue, Value, conversion::NativeConversion},
    vm::{
        Completion,
        call::{DirectCallTarget, NativeInvocation},
    },
};
#[derive(Clone, Copy)]
pub(crate) enum ObjectStringKind {
    Tag,
    Locale,
}
impl ObjectStringKind {
    pub(crate) fn for_target(target: NativeFunctionId) -> Option<Self> {
        Some(match target {
            NativeFunctionId::ObjectPrototypeToString => Self::Tag,
            NativeFunctionId::ObjectPrototypeToLocaleString => Self::Locale,
            _ => return None,
        })
    }
}
pub(crate) enum ObjectStringStep {
    Complete(Completion),
    Read { resume: ObjectStringResume },
    Call { resume: ObjectStringResume },
}
pub(crate) struct ObjectStringResume(Box<ObjectStringResumeState>);
impl std::ops::Deref for ObjectStringResume {
    type Target = ObjectStringResumeState;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for ObjectStringResume {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
const _: () = assert!(std::mem::size_of::<ObjectStringResume>() <= 8);
pub(crate) struct ObjectStringResumeState {
    runtime: Runtime,
    pending_effect: ObjectStringStepPending,
    realm: ContextId,
    receiver: JsValue,
    phase: Phase,
}
impl Drop for ObjectStringResumeState {
    /// Release the internal edges still owned when the request is abandoned.
    /// Drained fields are `None`/`Undefined` here; releases are defer-safe.
    fn drop(&mut self) {
        if let Some(value) = self.pending_effect.read_receiver.take() {
            let _ = self.runtime.release_jsvalue(value);
        }
        if let Some(value) = self.pending_effect.call_receiver.take() {
            let _ = self.runtime.release_jsvalue(value);
        }
        let receiver = std::mem::replace(&mut self.receiver, JsValue::Undefined);
        let _ = self.runtime.release_jsvalue(receiver);
    }
}
enum Phase {
    Tag(JsString),
    LocaleMethod,
    LocaleResult,
}
fn tag_string(runtime: &Runtime, tag: JsString) -> Result<ObjectStringStep, RuntimeError> {
    let value = JsString::from_static("[object ")
        .try_concat(&tag)?
        .try_concat(&JsString::from_static("]"))?;
    Ok(ObjectStringStep::Complete(Completion::Return(
        runtime.unroot_value(&Value::String(value))?,
    )))
}
impl ObjectStringStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        kind: ObjectStringKind,
        invocation: &NativeInvocation,
    ) -> Result<Self, RuntimeError> {
        let NativeInvocation::Call { this_value } = invocation else {
            return Err(RuntimeError::Invariant(
                "Object string conversion did not receive a call",
            ));
        };
        let this_value = runtime.dup_jsvalue(this_value)?;
        match kind {
            ObjectStringKind::Tag => {
                match this_value {
                    JsValue::Undefined => {
                        return tag_string(runtime, JsString::from_static("Undefined"));
                    }
                    JsValue::Null => return tag_string(runtime, JsString::from_static("Null")),
                    _ => {}
                }
                let object = match runtime
                    .native_to_object(realm, runtime.root_and_release_jsvalue(this_value)?)?
                {
                    NativeConversion::Value(object) => object,
                    NativeConversion::Throw(value) => {
                        return Ok(Self::Complete(Completion::Throw(
                            runtime.into_jsvalue(value)?,
                        )));
                    }
                };
                let tag = match runtime.object_default_to_string_tag(realm, &object)? {
                    NativeConversion::Value(tag) => tag,
                    NativeConversion::Throw(value) => {
                        return Ok(Self::Complete(Completion::Throw(
                            runtime.into_jsvalue(value)?,
                        )));
                    }
                };
                let receiver = runtime.into_jsvalue(Value::Object(object))?;
                Ok(Self::request_read(
                    runtime.dup_jsvalue(&receiver)?,
                    PropertyKey::from(runtime.well_known_symbol(WellKnownSymbol::ToStringTag)),
                    ObjectStringResume(Box::new(ObjectStringResumeState {
                        runtime: runtime.clone(),
                        pending_effect: ObjectStringStepPending::default(),
                        realm,
                        receiver,
                        phase: Phase::Tag(tag),
                    })),
                ))
            }
            ObjectStringKind::Locale => {
                if matches!(this_value, JsValue::Null | JsValue::Undefined) {
                    let message = if matches!(this_value, JsValue::Null) {
                        "cannot read property 'toString' of null"
                    } else {
                        "cannot read property 'toString' of undefined"
                    };
                    return Ok(Self::Complete(Completion::Throw(
                        runtime.new_native_error_jsvalue(realm, NativeErrorKind::Type, message)?,
                    )));
                }
                let seen = runtime.dup_jsvalue(&this_value)?;
                Ok(Self::request_read(
                    seen,
                    runtime
                        .pinned_property_key(crate::engine::atom::pinned::PinnedAtom::ToString)?,
                    ObjectStringResume(Box::new(ObjectStringResumeState {
                        runtime: runtime.clone(),
                        pending_effect: ObjectStringStepPending::default(),
                        realm,
                        receiver: this_value,
                        phase: Phase::LocaleMethod,
                    })),
                ))
            }
        }
    }
}
impl ObjectStringResume {
    pub(crate) fn resume(
        mut self,
        runtime: &Runtime,
        result: Completion,
    ) -> Result<ObjectStringStep, RuntimeError> {
        let value = match result {
            Completion::Return(value) => runtime.root_and_release_jsvalue(value)?,
            Completion::Throw(value) => {
                return Ok(ObjectStringStep::Complete(Completion::Throw(value)));
            }
        };
        match std::mem::replace(&mut self.0.phase, Phase::LocaleResult) {
            Phase::Tag(default_tag) => tag_string(
                runtime,
                match value {
                    Value::String(tag) => tag,
                    _ => default_tag,
                },
            ),
            Phase::LocaleResult => Ok(ObjectStringStep::Complete(Completion::Return(
                runtime.into_jsvalue(value)?,
            ))),
            Phase::LocaleMethod => {
                let callable = match value {
                    Value::Object(object) => runtime.as_callable(&object)?,
                    _ => None,
                };
                let Some(callable) = callable else {
                    return Ok(ObjectStringStep::Complete(Completion::Throw(
                        runtime.new_native_error_jsvalue(
                            self.0.realm,
                            NativeErrorKind::Type,
                            "not a function",
                        )?,
                    )));
                };
                Ok(ObjectStringStep::request_call(
                    DirectCallTarget::Callable(callable),
                    runtime.dup_jsvalue(&self.0.receiver)?,
                    self,
                ))
            }
        }
    }
}
pub(super) fn finish(
    runtime: &Runtime,
    realm: ContextId,
    mut step: ObjectStringStep,
) -> Result<Completion, RuntimeError> {
    loop {
        step = match step {
            ObjectStringStep::Complete(result) => return Ok(result),
            ObjectStringStep::Read { mut resume } => {
                let receiver = runtime.root_and_release_jsvalue(resume.take_read_receiver())?;
                let key = resume.take_read_key();
                resume.resume(
                    runtime,
                    runtime.get_value_property_in_realm(realm, receiver, &key)?,
                )?
            }
            ObjectStringStep::Call { mut resume } => {
                let target = resume.take_call_target();
                let receiver = runtime.root_and_release_jsvalue(resume.take_call_receiver())?;
                {
                    let result = match target {
                        DirectCallTarget::Callable(callable) => {
                            runtime.call_internal(realm, &callable, receiver, &[])?
                        }
                        DirectCallTarget::NonCallableProxy(proxy) => {
                            runtime.call_proxy(realm, &proxy, receiver, &[])?
                        }
                    };
                    resume.resume(runtime, result)?
                }
            }
        };
    }
}

#[derive(Default)]
struct ObjectStringStepPending {
    read_receiver: Option<JsValue>,
    read_key: Option<PropertyKey>,
    call_target: Option<DirectCallTarget>,
    call_receiver: Option<JsValue>,
}
impl ObjectStringStep {
    pub(crate) fn request_read(
        receiver: JsValue,
        key: PropertyKey,
        mut resume: ObjectStringResume,
    ) -> Self {
        resume.0.pending_effect.read_receiver = Some(receiver);
        resume.0.pending_effect.read_key = Some(key);
        Self::Read { resume }
    }
    pub(crate) fn request_call(
        target: DirectCallTarget,
        receiver: JsValue,
        mut resume: ObjectStringResume,
    ) -> Self {
        resume.0.pending_effect.call_target = Some(target);
        resume.0.pending_effect.call_receiver = Some(receiver);
        Self::Call { resume }
    }
}
impl ObjectStringResume {
    pub(crate) fn take_read_receiver(&mut self) -> JsValue {
        self.0
            .pending_effect
            .read_receiver
            .take()
            .expect("ObjectStringStep Read receiver")
    }
    pub(crate) fn take_read_key(&mut self) -> PropertyKey {
        self.0
            .pending_effect
            .read_key
            .take()
            .expect("ObjectStringStep Read key")
    }
    pub(crate) fn take_call_target(&mut self) -> DirectCallTarget {
        self.0
            .pending_effect
            .call_target
            .take()
            .expect("ObjectStringStep Call target")
    }
    pub(crate) fn take_call_receiver(&mut self) -> JsValue {
        self.0
            .pending_effect
            .call_receiver
            .take()
            .expect("ObjectStringStep Call receiver")
    }
}
const _: () = assert!(std::mem::size_of::<ObjectStringStep>() <= 64);

// S11 all-domain protocol bound; inline completion stays allocation-free.
const _: () = assert!(std::mem::size_of::<ObjectStringStep>() <= 64);
