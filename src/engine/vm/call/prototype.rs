//! Constructor prototype acquisition shares explicit Get and fallback realm replies.
use super::ConstructorPrototypeSource;
use crate::engine::{
    api::{runtime::Runtime, runtime_error::RuntimeError},
    heap::ContextId,
    object::{ObjectRef, OrdinaryRead, PropertyKey},
    value::{JsValue, conversion::NativeConversion},
    vm::Completion,
};

impl Runtime {
    /// Resolve `Get(newTarget, "prototype")` synchronously when the lookup
    /// cannot re-enter user code (an ordinary data property, or absence with
    /// the realm fallback). Returns `None` when a getter or Proxy trap must
    /// run through the resumable [`ProtoSourceStep`] protocol instead.
    ///
    /// The constructor frame owns `new_target` for the whole construction, so
    /// the lookup borrows it without duplicating its edge. Selecting (and
    /// abandoning) a prepared read never invokes user code, so a decline is
    /// unobservable.
    pub(crate) fn constructor_prototype_source_now(
        &self,
        realm: ContextId,
        new_target: &JsValue,
    ) -> Result<Option<NativeConversion<ConstructorPrototypeSource>>, RuntimeError> {
        if matches!(new_target, JsValue::Undefined) {
            return Ok(Some(NativeConversion::Value(
                ConstructorPrototypeSource::Realm(realm),
            )));
        }
        let key = self.pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Prototype)?;
        match self.prepare_value_property_read_borrowed_jsvalue(realm, new_target, &key)? {
            OrdinaryRead::Complete(Some(JsValue::Object(prototype))) => Ok(Some(
                NativeConversion::Value(ConstructorPrototypeSource::Explicit(
                    ObjectRef::from_owned_handle(self.clone(), prototype),
                )),
            )),
            OrdinaryRead::Complete(value) => {
                if let Some(value) = value {
                    self.release_jsvalue(value)?;
                }
                Ok(Some(
                    match self.function_realm_from_jsvalue(realm, new_target)? {
                        NativeConversion::Value(realm) => {
                            NativeConversion::Value(ConstructorPrototypeSource::Realm(realm))
                        }
                        NativeConversion::Throw(value) => NativeConversion::Throw(value),
                    },
                ))
            }
            read @ (OrdinaryRead::Call { .. } | OrdinaryRead::Special { .. }) => {
                read.release(self);
                Ok(None)
            }
        }
    }
}
pub(crate) enum ProtoSourceStep {
    Complete(NativeConversion<ConstructorPrototypeSource>),
    ReadValue {
        key: PropertyKey,
        resume: ProtoSourceResume,
    },
}
pub(crate) struct ProtoSourceResume(Box<ProtoSourceResumeState>);
impl std::ops::Deref for ProtoSourceResume {
    type Target = ProtoSourceResumeState;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for ProtoSourceResume {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
const _: () = assert!(std::mem::size_of::<ProtoSourceResume>() <= 8);
pub(crate) struct ProtoSourceResumeState {
    runtime: Runtime,
    realm: ContextId,
    new_target: JsValue,
    read_receiver: Option<JsValue>,
}
impl Drop for ProtoSourceResumeState {
    fn drop(&mut self) {
        if let Some(value) = self.read_receiver.take() {
            let _ = self.runtime.release_jsvalue(value);
        }
        let _ = self
            .runtime
            .release_jsvalue(std::mem::replace(&mut self.new_target, JsValue::Undefined));
    }
}
impl ProtoSourceStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        new_target: JsValue,
    ) -> Result<Self, RuntimeError> {
        // Complete the ordinary lookup in place: no resume Box, no duplicate
        // of the constructor frame's `new_target` edge. Only a getter or a
        // Proxy trap takes the resumable read below.
        match runtime.constructor_prototype_source_now(realm, &new_target) {
            Ok(Some(result)) => {
                runtime.release_jsvalue(new_target)?;
                return Ok(Self::Complete(result));
            }
            Ok(None) => {}
            Err(error) => {
                let _ = runtime.release_jsvalue(new_target);
                return Err(error);
            }
        }
        let mut resume = ProtoSourceResume(Box::new(ProtoSourceResumeState {
            runtime: runtime.clone(),
            realm,
            new_target,
            read_receiver: None,
        }));
        let key =
            runtime.pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Prototype)?;
        resume.0.read_receiver = Some(runtime.dup_jsvalue(&resume.0.new_target)?);
        Ok(Self::ReadValue { key, resume })
    }
}
impl ProtoSourceResume {
    pub(crate) fn take_read_receiver(&mut self) -> JsValue {
        self.0
            .read_receiver
            .take()
            .expect("constructor prototype read receiver")
    }
    pub(crate) fn resume(
        self,
        runtime: &Runtime,
        reply: Completion,
    ) -> Result<ProtoSourceStep, RuntimeError> {
        let result = match reply {
            Completion::Throw(value) => NativeConversion::Throw(value),
            Completion::Return(JsValue::Object(prototype)) => {
                NativeConversion::Value(ConstructorPrototypeSource::Explicit(
                    ObjectRef::from_owned_handle(runtime.clone(), prototype),
                ))
            }
            Completion::Return(value) => {
                runtime.release_jsvalue(value)?;
                match runtime.function_realm_from_jsvalue(self.0.realm, &self.0.new_target)? {
                    NativeConversion::Value(realm) => {
                        NativeConversion::Value(ConstructorPrototypeSource::Realm(realm))
                    }
                    NativeConversion::Throw(value) => NativeConversion::Throw(value),
                }
            }
        };
        Ok(ProtoSourceStep::Complete(result))
    }
}
pub(crate) fn finish(
    runtime: &Runtime,
    realm: ContextId,
    mut step: ProtoSourceStep,
) -> Result<NativeConversion<ConstructorPrototypeSource>, RuntimeError> {
    loop {
        step = match step {
            ProtoSourceStep::Complete(result) => return Ok(result),
            ProtoSourceStep::ReadValue { key, mut resume } => {
                let receiver = resume.take_read_receiver();
                resume.resume(
                    runtime,
                    runtime.get_value_property_in_realm_jsvalue(realm, receiver, &key)?,
                )?
            }
        };
    }
}

// S11 all-domain protocol bound; inline completion stays allocation-free.
const _: () = assert!(std::mem::size_of::<ProtoSourceStep>() <= 64);
