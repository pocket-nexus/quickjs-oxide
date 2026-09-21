//! Own-property predicates retain their distinct receiver/key conversion order.

use crate::engine::builtins::native::NativeFunctionId;
use crate::engine::builtins::native::ObjectAccessorKind;
use crate::engine::object::{
    AccessorValue, CallableRef, DescriptorField, OwnedPropertyDescriptor,
    operations::InternalDefineResult,
};
use crate::engine::{
    api::{runtime::Runtime, runtime_error::RuntimeError},
    heap::ContextId,
    object::{ObjectRef, PropertyKey},
    value::{JsValue, conversion::NativeConversion},
    vm::{
        Completion, ToPrimitiveHint,
        call::{NativeArguments, NativeInvocation},
    },
};
#[derive(Clone, Copy)]
pub(crate) enum PredicateKind {
    Define(ObjectAccessorKind),
    Lookup(ObjectAccessorKind),
    HasOwn,
    PrototypeHasOwn,
    Enumerable,
}
impl PredicateKind {
    pub(crate) fn for_target(target: NativeFunctionId) -> Option<Self> {
        Some(match target {
            NativeFunctionId::ObjectPrototypeDefineAccessor(kind) => Self::Define(kind),
            NativeFunctionId::ObjectPrototypeLookupAccessor(kind) => Self::Lookup(kind),
            NativeFunctionId::ObjectHasOwn => Self::HasOwn,
            NativeFunctionId::ObjectPrototypeHasOwnProperty => Self::PrototypeHasOwn,
            NativeFunctionId::ObjectPrototypePropertyIsEnumerable => Self::Enumerable,
            _ => return None,
        })
    }
}
pub(crate) enum PredicateStep {
    Descriptor { resume: PredicateResume },
    Prototype { resume: PredicateResume },
    Define { resume: PredicateResume },
    Complete(Completion),
    Key { resume: PredicateResume },
    Own { resume: PredicateResume },
}
pub(crate) struct PredicateResume(Box<PredicateResumeState>);
impl std::ops::Deref for PredicateResume {
    type Target = PredicateResumeState;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for PredicateResume {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
const _: () = assert!(std::mem::size_of::<PredicateResume>() <= 8);
pub(crate) struct PredicateResumeState {
    runtime: Runtime,
    pending_effect: PredicateStepPending,
    realm: ContextId,
    receiver: JsValue,
    kind: PredicateKind,
    phase: Phase,
}
enum Phase {
    Key { accessor: Option<CallableRef> },
    Own(PropertyKey),
    Prototype(PropertyKey),
}
impl Drop for PredicateResumeState {
    fn drop(&mut self) {
        let value = std::mem::replace(&mut self.receiver, JsValue::Undefined);
        let _ = self.runtime.release_jsvalue(value);
        if let Some(value) = self.pending_effect.key_value.take() {
            let _ = self.runtime.release_jsvalue(value);
        }
    }
}
impl PredicateStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        kind: PredicateKind,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        let NativeInvocation::Call { this_value } = invocation else {
            return Err(RuntimeError::Invariant(
                "own predicate did not receive a call",
            ));
        };
        let (receiver, key) = if matches!(kind, PredicateKind::HasOwn) {
            let value = match arguments.readable.first() {
                Some(value) => value,
                None => {
                    return Err(RuntimeError::Invariant("hasOwn target argv was not padded"));
                }
            };
            let object =
                match runtime.native_to_object_jsvalue(realm, runtime.dup_jsvalue(value)?)? {
                    NativeConversion::Value(object) => object,
                    NativeConversion::Throw(value) => {
                        return Ok(Self::Complete(Completion::Throw(
                            runtime.into_jsvalue(value)?,
                        )));
                    }
                };
            (
                JsValue::Object(object.into_handle()),
                arguments.readable.get(1),
            )
        } else if matches!(kind, PredicateKind::Define(_) | PredicateKind::Lookup(_)) {
            let object =
                match runtime.native_to_object_jsvalue(realm, runtime.dup_jsvalue(this_value)?)? {
                    NativeConversion::Value(object) => object,
                    NativeConversion::Throw(value) => {
                        return Ok(Self::Complete(Completion::Throw(
                            runtime.into_jsvalue(value)?,
                        )));
                    }
                };
            (
                JsValue::Object(object.into_handle()),
                arguments.readable.first(),
            )
        } else {
            (runtime.dup_jsvalue(this_value)?, arguments.readable.first())
        };
        let mut owner = PredicateResume(Box::new(PredicateResumeState {
            runtime: runtime.clone(),
            pending_effect: Default::default(),
            realm,
            receiver,
            kind,
            phase: Phase::Key { accessor: None },
        }));
        let accessor = if matches!(kind, PredicateKind::Define(_)) {
            let value = arguments
                .readable
                .get(1)
                .ok_or(RuntimeError::Invariant("accessor argv was not padded"))?;
            let callable = match value {
                JsValue::Object(id) => {
                    let object = crate::engine::object::ObjectRef::from_borrowed_handle(
                        runtime.clone(),
                        *id,
                    )?;
                    runtime.as_callable(&object)?
                }
                _ => None,
            };
            let Some(callable) = callable else {
                return Ok(Self::Complete(Completion::Throw(
                    runtime.new_native_error_jsvalue(
                        realm,
                        crate::engine::api::error::NativeErrorKind::Type,
                        "not a function",
                    )?,
                )));
            };
            Some(callable)
        } else {
            None
        };
        let value = match key {
            Some(key) => runtime.dup_jsvalue(key)?,
            None => {
                return Err(RuntimeError::Invariant(
                    "own predicate key argv was not padded",
                ));
            }
        };
        owner.0.phase = Phase::Key { accessor };
        Ok(Self::request_key(value, owner))
    }
}
impl PredicateResume {
    pub(crate) fn key(
        mut self,
        runtime: &Runtime,
        result: Completion,
    ) -> Result<PredicateStep, RuntimeError> {
        let Phase::Key { accessor } =
            std::mem::replace(&mut self.0.phase, Phase::Key { accessor: None })
        else {
            return Err(RuntimeError::Invariant(
                "own predicate received a second key reply",
            ));
        };
        let value = match result {
            Completion::Return(value) => value,
            Completion::Throw(value) => {
                return Ok(PredicateStep::Complete(Completion::Throw(value)));
            }
        };
        let key = match runtime.property_key_from_primitive_jsvalue(self.0.realm, value)? {
            NativeConversion::Value(key) => key,
            NativeConversion::Throw(value) => {
                return Ok(PredicateStep::Complete(Completion::Throw(
                    runtime.into_jsvalue(value)?,
                )));
            }
        };
        let object = match runtime
            .native_to_object_jsvalue(self.0.realm, runtime.dup_jsvalue(&self.0.receiver)?)?
        {
            NativeConversion::Value(object) => object,
            NativeConversion::Throw(value) => {
                return Ok(PredicateStep::Complete(Completion::Throw(
                    runtime.into_jsvalue(value)?,
                )));
            }
        };
        let resume = {
            let updated_0 = Phase::Own(key.clone());
            self.0.phase = updated_0;
            self
        };
        Ok(match resume.kind {
            PredicateKind::Define(kind) => {
                let accessor = accessor.ok_or(RuntimeError::Invariant(
                    "accessor definition lost its callable",
                ))?;
                let mut descriptor = OwnedPropertyDescriptor::new(runtime);
                descriptor.enumerable = DescriptorField::Present(true);
                descriptor.configurable = DescriptorField::Present(true);
                match kind {
                    ObjectAccessorKind::Getter => {
                        descriptor.get = DescriptorField::Present(AccessorValue::Callable(accessor))
                    }
                    ObjectAccessorKind::Setter => {
                        descriptor.set = DescriptorField::Present(AccessorValue::Callable(accessor))
                    }
                }
                PredicateStep::request_define(object, key, descriptor, resume)
            }
            PredicateKind::Lookup(_) => PredicateStep::request_descriptor(object, key, resume),
            _ => PredicateStep::request_own(
                object,
                key,
                matches!(resume.kind, PredicateKind::Enumerable),
                resume,
            ),
        })
    }
    pub(crate) fn boolean(
        self,
        runtime: &Runtime,
        result: NativeConversion<bool>,
    ) -> Result<PredicateStep, RuntimeError> {
        if !matches!(self.0.phase, Phase::Own(_))
            || matches!(
                self.0.kind,
                PredicateKind::Define(_) | PredicateKind::Lookup(_)
            )
        {
            return Err(RuntimeError::Invariant(
                "own predicate received a boolean before key conversion",
            ));
        }
        Ok(PredicateStep::Complete(match result {
            NativeConversion::Value(value) => Completion::Return(JsValue::Bool(value)),
            NativeConversion::Throw(value) => Completion::Throw(runtime.into_jsvalue(value)?),
        }))
    }
}
impl PredicateResume {
    pub(crate) fn defined(
        mut self,
        runtime: &Runtime,
        result: NativeConversion<InternalDefineResult>,
    ) -> Result<PredicateStep, RuntimeError> {
        let Phase::Own(key) = std::mem::replace(&mut self.0.phase, Phase::Key { accessor: None })
        else {
            return Err(RuntimeError::Invariant(
                "accessor definition has wrong phase",
            ));
        };
        if !matches!(self.0.kind, PredicateKind::Define(_)) {
            return Err(RuntimeError::Invariant(
                "accessor definition has wrong kind",
            ));
        }
        Ok(PredicateStep::Complete(
            match runtime.finish_define_property_or_throw(self.0.realm, &key, result)? {
                Some(value) => Completion::Throw(runtime.into_jsvalue(value)?),
                None => Completion::Return(JsValue::Undefined),
            },
        ))
    }
    pub(crate) fn descriptor(
        mut self,
        runtime: &Runtime,
        result: NativeConversion<Option<crate::engine::object::OwnedCompletePropertyDescriptor>>,
    ) -> Result<PredicateStep, RuntimeError> {
        let Phase::Own(key) = std::mem::replace(&mut self.0.phase, Phase::Key { accessor: None })
        else {
            return Err(RuntimeError::Invariant("accessor lookup has wrong phase"));
        };
        let PredicateKind::Lookup(kind) = self.0.kind else {
            return Err(RuntimeError::Invariant("accessor lookup has wrong kind"));
        };
        let descriptor = match result {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => {
                return Ok(PredicateStep::Complete(Completion::Throw(
                    runtime.into_jsvalue(value)?,
                )));
            }
        };
        let Some(descriptor) = descriptor else {
            let JsValue::Object(object) = &self.0.receiver else {
                return Err(RuntimeError::Invariant("accessor lookup lost its object"));
            };
            return Ok(PredicateStep::request_prototype(
                ObjectRef::from_borrowed_handle(runtime.clone(), *object)?,
                {
                    let updated_0 = Phase::Prototype(key);
                    self.0.phase = updated_0;
                    self
                },
            ));
        };
        use crate::engine::object::property::CompletePropertyDescriptor;
        let value = match descriptor.record() {
            CompletePropertyDescriptor::Data { .. } => JsValue::Undefined,
            CompletePropertyDescriptor::Accessor { get, set, .. } => {
                let value = match kind {
                    ObjectAccessorKind::Getter => get,
                    ObjectAccessorKind::Setter => set,
                };
                match value {
                    Some(value) => runtime.dup_jsvalue(
                        &JsValue::from_raw(value.clone())
                            .expect("descriptor contains initialized values"),
                    )?,
                    None => JsValue::Undefined,
                }
            }
        };
        Ok(PredicateStep::Complete(Completion::Return(value)))
    }
    pub(crate) fn prototype(
        mut self,
        runtime: &Runtime,
        result: NativeConversion<Option<ObjectRef>>,
    ) -> Result<PredicateStep, RuntimeError> {
        let Phase::Prototype(key) =
            std::mem::replace(&mut self.0.phase, Phase::Key { accessor: None })
        else {
            return Err(RuntimeError::Invariant(
                "accessor prototype reply has wrong phase",
            ));
        };
        Ok(match result {
            NativeConversion::Throw(value) => {
                PredicateStep::Complete(Completion::Throw(runtime.into_jsvalue(value)?))
            }
            NativeConversion::Value(None) => {
                PredicateStep::Complete(Completion::Return(JsValue::Undefined))
            }
            NativeConversion::Value(Some(object)) => {
                PredicateStep::request_descriptor(object.clone(), key.clone(), {
                    let updated_0 = JsValue::Object(object.into_handle());
                    let updated_1 = Phase::Own(key);
                    runtime.release_jsvalue(std::mem::replace(&mut self.0.receiver, updated_0))?;
                    self.0.phase = updated_1;
                    self
                })
            }
        })
    }
}

pub(in crate::engine::builtins) fn finish(
    runtime: &Runtime,
    realm: ContextId,
    mut step: PredicateStep,
) -> Result<Completion, RuntimeError> {
    loop {
        step = match step {
            PredicateStep::Complete(result) => return Ok(result),
            PredicateStep::Descriptor { mut resume } => {
                let object = resume.take_descriptor_object();
                let key = resume.take_descriptor_key();
                resume.descriptor(
                    runtime,
                    runtime.internal_get_own_property_owned(realm, &object, &key)?,
                )?
            }
            PredicateStep::Prototype { mut resume } => {
                let object = resume.take_prototype_object();
                resume.prototype(runtime, runtime.internal_get_prototype_of(realm, &object)?)?
            }
            PredicateStep::Define { mut resume } => {
                let object = resume.take_define_object();
                let key = resume.take_define_key();
                let descriptor = resume.take_define_descriptor();
                resume.defined(
                    runtime,
                    runtime.internal_define_owned_property(realm, &object, &key, descriptor)?,
                )?
            }
            PredicateStep::Key { mut resume } => {
                let value = resume.take_key_value();
                resume.key(
                    runtime,
                    runtime.to_primitive_jsvalue(realm, value, ToPrimitiveHint::String)?,
                )?
            }
            PredicateStep::Own { mut resume } => {
                let object = resume.take_own_object();
                let key = resume.take_own_key();
                let enumerable = resume.take_own_enumerable();
                resume.boolean(
                    runtime,
                    if enumerable {
                        runtime.internal_own_property_is_enumerable(realm, &object, &key)?
                    } else {
                        runtime.internal_has_own_property(realm, &object, &key)?
                    },
                )?
            }
        };
    }
}

#[derive(Default)]
struct PredicateStepPending {
    descriptor_object: Option<ObjectRef>,
    descriptor_key: Option<PropertyKey>,
    prototype_object: Option<ObjectRef>,
    define_object: Option<ObjectRef>,
    define_key: Option<PropertyKey>,
    define_descriptor: Option<OwnedPropertyDescriptor>,
    key_value: Option<JsValue>,
    own_object: Option<ObjectRef>,
    own_key: Option<PropertyKey>,
    own_enumerable: Option<bool>,
}
impl PredicateStep {
    pub(crate) fn request_descriptor(
        object: ObjectRef,
        key: PropertyKey,
        mut resume: PredicateResume,
    ) -> Self {
        resume.0.pending_effect.descriptor_object = Some(object);
        resume.0.pending_effect.descriptor_key = Some(key);
        Self::Descriptor { resume }
    }
    pub(crate) fn request_prototype(object: ObjectRef, mut resume: PredicateResume) -> Self {
        resume.0.pending_effect.prototype_object = Some(object);
        Self::Prototype { resume }
    }
    pub(crate) fn request_define(
        object: ObjectRef,
        key: PropertyKey,
        descriptor: OwnedPropertyDescriptor,
        mut resume: PredicateResume,
    ) -> Self {
        resume.0.pending_effect.define_object = Some(object);
        resume.0.pending_effect.define_key = Some(key);
        resume.0.pending_effect.define_descriptor = Some(descriptor);
        Self::Define { resume }
    }
    pub(crate) fn request_key(value: JsValue, mut resume: PredicateResume) -> Self {
        resume.0.pending_effect.key_value = Some(value);
        Self::Key { resume }
    }
    pub(crate) fn request_own(
        object: ObjectRef,
        key: PropertyKey,
        enumerable: bool,
        mut resume: PredicateResume,
    ) -> Self {
        resume.0.pending_effect.own_object = Some(object);
        resume.0.pending_effect.own_key = Some(key);
        resume.0.pending_effect.own_enumerable = Some(enumerable);
        Self::Own { resume }
    }
}
impl PredicateResume {
    pub(crate) fn take_descriptor_object(&mut self) -> ObjectRef {
        self.0
            .pending_effect
            .descriptor_object
            .take()
            .expect("PredicateStep Descriptor object")
    }
    pub(crate) fn take_descriptor_key(&mut self) -> PropertyKey {
        self.0
            .pending_effect
            .descriptor_key
            .take()
            .expect("PredicateStep Descriptor key")
    }
    pub(crate) fn take_prototype_object(&mut self) -> ObjectRef {
        self.0
            .pending_effect
            .prototype_object
            .take()
            .expect("PredicateStep Prototype object")
    }
    pub(crate) fn take_define_object(&mut self) -> ObjectRef {
        self.0
            .pending_effect
            .define_object
            .take()
            .expect("PredicateStep Define object")
    }
    pub(crate) fn take_define_key(&mut self) -> PropertyKey {
        self.0
            .pending_effect
            .define_key
            .take()
            .expect("PredicateStep Define key")
    }
    pub(crate) fn take_define_descriptor(&mut self) -> OwnedPropertyDescriptor {
        self.0
            .pending_effect
            .define_descriptor
            .take()
            .expect("PredicateStep Define descriptor")
    }
    pub(crate) fn take_key_value(&mut self) -> JsValue {
        self.0
            .pending_effect
            .key_value
            .take()
            .expect("PredicateStep Key value")
    }
    pub(crate) fn take_own_object(&mut self) -> ObjectRef {
        self.0
            .pending_effect
            .own_object
            .take()
            .expect("PredicateStep Own object")
    }
    pub(crate) fn take_own_key(&mut self) -> PropertyKey {
        self.0
            .pending_effect
            .own_key
            .take()
            .expect("PredicateStep Own key")
    }
    pub(crate) fn take_own_enumerable(&mut self) -> bool {
        self.0
            .pending_effect
            .own_enumerable
            .take()
            .expect("PredicateStep Own enumerable")
    }
}
const _: () = assert!(std::mem::size_of::<PredicateStep>() <= 64);

// S11 all-domain protocol bound; inline completion stays allocation-free.
const _: () = assert!(std::mem::size_of::<PredicateStep>() <= 64);
