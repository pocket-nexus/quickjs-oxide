//! Weak constructors retain validated inputs across new.target prototype lookup.
use super::WeakIntrinsicKind;
use crate::engine::{
    api::{error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError},
    heap::{ContextId, WeakCollectionKey},
    object::{CallableRef, ObjectRef},
    value::{JsValue, conversion::NativeConversion},
    vm::{
        Completion,
        call::{
            ConstructorPrototypeSource, NativeArguments, NativeInvocation,
            prototype::{ProtoSourceStep, finish as finish_source},
        },
    },
};
pub(crate) enum WeakConstructorStep {
    Complete(Completion),
    Prototype {
        new_target: JsValue,
        resume: WeakConstructorResume,
    },
}
pub(crate) struct WeakConstructorResume(Box<WeakConstructorResumeState>);
impl std::ops::Deref for WeakConstructorResume {
    type Target = WeakConstructorResumeState;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for WeakConstructorResume {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
const _: () = assert!(std::mem::size_of::<WeakConstructorResume>() <= 8);
pub(crate) struct WeakConstructorResumeState {
    runtime: Runtime,
    target_owner: JsValue,
    realm: ContextId,
    input: Input,
}
impl Drop for WeakConstructorResumeState {
    fn drop(&mut self) {
        let _ = self.runtime.release_jsvalue(std::mem::replace(
            &mut self.target_owner,
            JsValue::Undefined,
        ));
    }
}
enum Input {
    WeakRef { key: WeakCollectionKey },
    FinalizationRegistry(CallableRef),
}
impl WeakConstructorStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        kind: WeakIntrinsicKind,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        let NativeInvocation::Construct { new_target } = invocation else {
            return Err(RuntimeError::Invariant(match kind {
                WeakIntrinsicKind::WeakRef => {
                    "WeakRef constructor received the wrong native invocation"
                }
                WeakIntrinsicKind::FinalizationRegistry => {
                    "FinalizationRegistry constructor received the wrong native invocation"
                }
            }));
        };
        if matches!(new_target, JsValue::Undefined) {
            return Ok(Self::Complete(Completion::Throw(
                runtime.new_native_error_jsvalue(
                    realm,
                    NativeErrorKind::Type,
                    "constructor requires 'new'",
                )?,
            )));
        }
        let value = arguments
            .readable
            .first()
            .ok_or(RuntimeError::Invariant(match kind {
                WeakIntrinsicKind::WeakRef => "WeakRef target argv was not padded",
                WeakIntrinsicKind::FinalizationRegistry => {
                    "FinalizationRegistry callback argv was not padded"
                }
            }))?;
        let input = match kind {
            WeakIntrinsicKind::WeakRef => {
                let Some(key) = runtime.weak_target_key(value)? else {
                    return runtime
                        .invalid_weak_target(realm, "invalid target")
                        .map(Self::Complete);
                };
                Input::WeakRef { key }
            }
            WeakIntrinsicKind::FinalizationRegistry => {
                let callable = match value {
                    JsValue::Object(id) => runtime.as_callable_object(*id)?,
                    _ => None,
                };
                let Some(callable) = callable else {
                    return runtime
                        .invalid_weak_target(realm, "argument must be a function")
                        .map(Self::Complete);
                };
                Input::FinalizationRegistry(callable)
            }
        };
        let resume = WeakConstructorResume(Box::new(WeakConstructorResumeState {
            runtime: runtime.clone(),
            target_owner: runtime.dup_jsvalue(value)?,
            realm,
            input,
        }));
        Ok(Self::Prototype {
            new_target: runtime.dup_jsvalue(new_target)?,
            resume,
        })
    }
}
impl WeakConstructorResume {
    pub(crate) fn prototype(
        self,
        runtime: &Runtime,
        reply: NativeConversion<ConstructorPrototypeSource>,
    ) -> Result<WeakConstructorStep, RuntimeError> {
        let prototype = match reply {
            NativeConversion::Throw(value) => {
                return Ok(WeakConstructorStep::Complete(Completion::Throw(
                    runtime.into_jsvalue(value)?,
                )));
            }
            NativeConversion::Value(ConstructorPrototypeSource::Explicit(prototype)) => prototype,
            NativeConversion::Value(ConstructorPrototypeSource::Realm(realm)) => {
                let kind = match &self.0.input {
                    Input::WeakRef { .. } => WeakIntrinsicKind::WeakRef,
                    Input::FinalizationRegistry(_) => WeakIntrinsicKind::FinalizationRegistry,
                };
                ObjectRef::from_borrowed_handle(
                    runtime.clone(),
                    runtime.weak_intrinsic_prototype(realm, kind)?,
                )?
            }
        };
        let object = match &self.0.input {
            Input::WeakRef { key } => runtime.new_weak_ref_object(&prototype, *key)?,
            Input::FinalizationRegistry(callback) => {
                runtime.new_finalization_registry_object(&prototype, callback, self.0.realm)?
            }
        };
        Ok(WeakConstructorStep::Complete(Completion::Return(
            JsValue::Object(object.into_handle()),
        )))
    }
}
pub(super) fn finish(
    runtime: &Runtime,
    realm: ContextId,
    mut step: WeakConstructorStep,
) -> Result<Completion, RuntimeError> {
    loop {
        step = match step {
            WeakConstructorStep::Complete(result) => return Ok(result),
            WeakConstructorStep::Prototype { new_target, resume } => resume.prototype(
                runtime,
                finish_source(
                    runtime,
                    realm,
                    ProtoSourceStep::start(runtime, realm, new_target)?,
                )?,
            )?,
        };
    }
}

// S11 all-domain protocol bound; inline completion stays allocation-free.
const _: () = assert!(std::mem::size_of::<WeakConstructorStep>() <= 64);
