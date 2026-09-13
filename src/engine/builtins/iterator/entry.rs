use crate::engine::api::error::{Error, ErrorKind, NativeErrorKind};
use crate::engine::api::runtime::Runtime;
use crate::engine::api::runtime_error::RuntimeError;
use crate::engine::heap::ContextId;
use crate::engine::object::operations::InternalDefineResult;

use crate::engine::object::{
    DescriptorField, OrdinaryPropertyDescriptor, PropertyKey, WellKnownSymbol,
};
use crate::engine::value::conversion::NativeConversion;
use crate::engine::value::{JsString, Value};
use crate::engine::vm::Completion;
use crate::engine::vm::call::{NativeArguments, NativeInvocation};

impl Runtime {
    pub(crate) fn call_iterator_prototype_iterator(
        &self,
        invocation: NativeInvocation,
    ) -> Result<Completion, RuntimeError> {
        let NativeInvocation::Call { this_value } = invocation else {
            return Err(RuntimeError::Invariant(
                "Iterator.prototype iterator did not receive a generic invocation",
            ));
        };
        Ok(Completion::Return(this_value))
    }

    pub(crate) fn call_iterator_prototype_to_string_tag_getter(
        &self,
        invocation: NativeInvocation,
    ) -> Result<Completion, RuntimeError> {
        let NativeInvocation::Getter { .. } = invocation else {
            return Err(RuntimeError::Invariant(
                "Iterator.prototype toStringTag getter received the wrong native invocation",
            ));
        };
        Ok(Completion::Return(Value::String(JsString::from_static(
            "Iterator",
        ))))
    }

    pub(crate) fn call_iterator_prototype_to_string_tag_setter(
        &self,
        realm: ContextId,
        invocation: NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        finish_tag(
            self,
            realm,
            TagSetterStep::start(self, realm, &invocation, arguments)?,
        )
    }
}

pub(crate) enum TagSetterStep {
    Complete(Completion),
    Own {
        object: crate::engine::object::ObjectRef,
        key: PropertyKey,
        resume: TagSetterResume,
    },
    Define {
        object: crate::engine::object::ObjectRef,
        key: PropertyKey,
        descriptor: OrdinaryPropertyDescriptor,
        resume: TagSetterResume,
    },
    Set {
        object: crate::engine::object::ObjectRef,
        key: PropertyKey,
        value: Value,
        resume: TagSetterResume,
    },
}
pub(crate) struct TagSetterResume {
    realm: ContextId,
    receiver: crate::engine::object::ObjectRef,
    key: PropertyKey,
    value: Value,
}
impl TagSetterStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        let NativeInvocation::Setter { this_value } = invocation else {
            return Err(RuntimeError::Invariant(
                "Iterator.prototype toStringTag setter received the wrong native invocation",
            ));
        };
        let Value::Object(receiver) = this_value else {
            return Err(RuntimeError::Engine(Error::new(
                ErrorKind::Type,
                "not an object",
            )));
        };
        let value = arguments
            .readable
            .first()
            .cloned()
            .ok_or(RuntimeError::Invariant(
                "Iterator.prototype toStringTag setter argv was not padded",
            ))?;
        let iterator_prototype = runtime
            .0
            .state
            .borrow()
            .heap
            .context(realm)?
            .iterator_prototype;
        if receiver.object_id() == iterator_prototype {
            return Err(RuntimeError::Engine(Error::new(
                ErrorKind::Type,
                "Cannot assign to read only property",
            )));
        }
        let key = PropertyKey::from(runtime.well_known_symbol(WellKnownSymbol::ToStringTag));
        Ok(Self::Own {
            object: receiver.clone(),
            key: key.clone(),
            resume: TagSetterResume {
                realm,
                receiver: receiver.clone(),
                key,
                value,
            },
        })
    }
}
impl TagSetterResume {
    pub(crate) fn boolean(
        self,
        reply: NativeConversion<bool>,
    ) -> Result<TagSetterStep, RuntimeError> {
        match reply {
            NativeConversion::Throw(value) => Ok(TagSetterStep::Complete(Completion::Throw(value))),
            NativeConversion::Value(true) => Ok(TagSetterStep::Set {
                object: self.receiver.clone(),
                key: self.key.clone(),
                value: self.value.clone(),
                resume: self,
            }),
            NativeConversion::Value(false) => Ok(TagSetterStep::Define {
                object: self.receiver.clone(),
                key: self.key.clone(),
                descriptor: OrdinaryPropertyDescriptor {
                    value: DescriptorField::Present(self.value.clone()),
                    writable: DescriptorField::Present(true),
                    enumerable: DescriptorField::Present(true),
                    configurable: DescriptorField::Present(true),
                    ..OrdinaryPropertyDescriptor::new()
                },
                resume: self,
            }),
        }
    }
    pub(crate) fn defined(
        self,
        runtime: &Runtime,
        reply: NativeConversion<InternalDefineResult>,
    ) -> Result<TagSetterStep, RuntimeError> {
        Ok(TagSetterStep::Complete(match reply {
            NativeConversion::Value(InternalDefineResult::Defined) => {
                Completion::Return(Value::Undefined)
            }
            NativeConversion::Value(InternalDefineResult::RejectedProxyTrap) => {
                Completion::Throw(runtime.new_native_error(
                    self.realm,
                    NativeErrorKind::Type,
                    "proxy: defineProperty exception",
                )?)
            }
            NativeConversion::Value(InternalDefineResult::RejectedOrdinary(target)) => {
                let message = if !runtime.has_own_property(&target, &self.key)?
                    && !runtime.is_extensible(&target)?
                {
                    "object is not extensible"
                } else {
                    "property is not configurable"
                };
                Completion::Throw(runtime.new_native_error(
                    self.realm,
                    NativeErrorKind::Type,
                    message,
                )?)
            }
            NativeConversion::Throw(value) => Completion::Throw(value),
        }))
    }
    pub(crate) fn set(
        self,
        runtime: &Runtime,
        reply: NativeConversion<crate::engine::object::operations::InternalSetResult>,
    ) -> Result<TagSetterStep, RuntimeError> {
        Ok(TagSetterStep::Complete(
            match runtime.finish_set_property_or_throw(self.realm, &self.key, reply)? {
                Some(value) => Completion::Throw(value),
                None => Completion::Return(Value::Undefined),
            },
        ))
    }
}
pub(crate) fn finish_tag(
    runtime: &Runtime,
    realm: ContextId,
    mut step: TagSetterStep,
) -> Result<Completion, RuntimeError> {
    loop {
        step = match step {
            TagSetterStep::Complete(result) => return Ok(result),
            TagSetterStep::Own {
                object,
                key,
                resume,
            } => resume.boolean(runtime.internal_has_own_property(realm, &object, &key)?)?,
            TagSetterStep::Define {
                object,
                key,
                descriptor,
                resume,
            } => resume.defined(
                runtime,
                runtime.internal_define_own_property(realm, &object, &key, &descriptor)?,
            )?,
            TagSetterStep::Set {
                object,
                key,
                value,
                resume,
            } => resume.set(
                runtime,
                runtime.internal_set(realm, &object, &key, value, Value::Object(object.clone()))?,
            )?,
        };
    }
}
