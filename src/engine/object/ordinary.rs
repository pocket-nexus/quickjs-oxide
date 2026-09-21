//! Ordinary Set semantics. Storage probes finish before observable calls;
//! exceptional receivers retain the internal-method dispatch contract.
use crate::engine::api::runtime::Runtime;
use crate::engine::api::runtime_error::RuntimeError;
use crate::engine::heap::ContextId;
use crate::engine::object::operations::{
    ArrayOwnKey, InternalDefineResult, InternalSetResult, PropertyDefineOutcome, PropertySetAction,
    PropertySetRejection,
};
use crate::engine::object::ordinary_storage::{SetProbe, SpecialKind};
use crate::engine::object::{
    CompleteOrdinaryPropertyDescriptor, DescriptorField, ObjectRef, OrdinaryPropertyDescriptor,
    PropertyKey,
};
use crate::engine::value::conversion::NativeConversion;
use crate::engine::value::{JsValue, Value};

mod set;

pub(crate) use set::SetResume;
pub(crate) use set::SetStep;

impl Runtime {
    #[cfg(test)]
    pub(crate) fn prepare_set_property(
        &self,
        object: &ObjectRef,
        key: &PropertyKey,
        value: Value,
    ) -> Result<PropertySetAction, RuntimeError> {
        let _operation = self.operation();
        self.prepare_set_property_with_receiver_in_realm(
            None,
            object,
            key,
            value,
            Value::Object(object.clone()),
        )
    }

    #[cfg(test)]
    pub(crate) fn prepare_set_property_with_receiver(
        &self,
        object: &ObjectRef,
        key: &PropertyKey,
        value: Value,
        receiver: Value,
    ) -> Result<PropertySetAction, RuntimeError> {
        self.prepare_set_property_with_receiver_in_realm(None, object, key, value, receiver)
    }

    pub(crate) fn prepare_set_property_with_receiver_in_realm(
        &self,
        realm: Option<ContextId>,
        object: &ObjectRef,
        key: &PropertyKey,
        value: Value,
        receiver: Value,
    ) -> Result<PropertySetAction, RuntimeError> {
        self.validate_object_and_key(object, key)?;
        self.validate_value_domain(&value, "property value")?;
        self.validate_value_domain(&receiver, "property receiver")?;
        let value = self.into_jsvalue(value)?;
        let mut step = SetStep::start(self, realm, object.clone(), key.clone(), value, receiver)?;
        loop {
            match step {
                SetStep::Complete(action) => return Ok(action),
                request => step = request.finish_sync(self)?,
            }
        }
    }
}

pub(crate) fn set_completion(result: NativeConversion<InternalSetResult>) -> PropertySetAction {
    match result {
        NativeConversion::Throw(value) => PropertySetAction::Throw(value),
        NativeConversion::Value(InternalSetResult::Accepted) => PropertySetAction::Complete,
        NativeConversion::Value(InternalSetResult::Rejected(reason)) => {
            PropertySetAction::Rejected(reason)
        }
        NativeConversion::Value(InternalSetResult::RejectedProxyTrap) => {
            PropertySetAction::RejectedProxyTrap
        }
    }
}

impl Runtime {
    /// Ordinary nodes are iterative. A special node delegates exactly once and
    /// preserves the distinction between missing and an observed undefined.
    pub(super) fn get_ordinary_chain(
        &self,
        realm: ContextId,
        object: &ObjectRef,
        key: &PropertyKey,
        receiver: Value,
    ) -> Result<NativeConversion<Option<Value>>, RuntimeError> {
        let read = self.prepare_ordinary_read(object, key, receiver)?;
        self.finish_prepared_read(realm, key, read)
    }

    pub(crate) fn finish_prepared_read(
        &self,
        realm: ContextId,
        key: &PropertyKey,
        read: OrdinaryRead,
    ) -> Result<NativeConversion<Option<Value>>, RuntimeError> {
        use crate::engine::vm::Completion;
        match read {
            OrdinaryRead::Complete(value) => {
                // Boundary rooting: the internal value's edges are duplicated
                // into the public root, then its own edges are released.
                let value = match value {
                    Some(value) => {
                        let rooted = self.root_value(&value)?;
                        self.release_jsvalue(value)?;
                        Some(rooted)
                    }
                    None => None,
                };
                Ok(NativeConversion::Value(value))
            }
            OrdinaryRead::Call { getter, receiver } => Ok(
                match self.call_internal_jsvalue(realm, &getter, receiver, Vec::new())? {
                    Completion::Return(value) => {
                        NativeConversion::Value(Some(self.root_and_release_jsvalue(value)?))
                    }
                    Completion::Throw(value) => {
                        NativeConversion::Throw(self.root_and_release_jsvalue(value)?)
                    }
                },
            ),
            OrdinaryRead::Special {
                kind,
                object,
                receiver,
            } => {
                let receiver = self.root_and_release_jsvalue(receiver)?;
                self.get_special_or_missing(kind, realm, &object, key, receiver)
            }
        }
    }

    /// Finish lookup through non-Proxy storage without invoking an accessor.
    /// Every returned owner remains valid after the lookup borrows end; a
    /// caller can schedule the selected getter without repeating the lookup.
    pub(crate) fn prepare_ordinary_read(
        &self,
        object: &ObjectRef,
        key: &PropertyKey,
        receiver: Value,
    ) -> Result<OrdinaryRead, RuntimeError> {
        self.prepare_ordinary_read_borrowed(object, key, &receiver)
    }

    /// Lookup borrows its already-rooted receiver; only a waiting result needs
    /// another owner. Data reads do not acquire a temporary receiver root.
    pub(crate) fn prepare_ordinary_read_borrowed(
        &self,
        object: &ObjectRef,
        key: &PropertyKey,
        receiver: &Value,
    ) -> Result<OrdinaryRead, RuntimeError> {
        let receiver = self.unroot_value(receiver)?;
        let result = self.prepare_ordinary_read_selected(object, key, &receiver, None);
        self.release_jsvalue(receiver)?;
        result
    }
    pub(crate) fn prepare_ordinary_read_selected(
        &self,
        object: &ObjectRef,
        key: &PropertyKey,
        receiver: &JsValue,
        mut native: Option<&mut Option<crate::engine::object::LinkedNativeSelection>>,
    ) -> Result<OrdinaryRead, RuntimeError> {
        let _operation = self.operation();
        self.validate_object_and_key(object, key)?;
        use crate::engine::object::ordinary_storage::ReadProbe;
        let mut prototype = None;
        loop {
            let current = prototype.as_ref().unwrap_or(object);
            match self.ordinary_read_probe_selected(current, key, native.as_deref_mut())? {
                ReadProbe::Value(value) => {
                    return Ok(OrdinaryRead::Complete(Some(value)));
                }
                ReadProbe::Getter(None) => {
                    return Ok(OrdinaryRead::Complete(Some(JsValue::Undefined)));
                }
                ReadProbe::Getter(Some(getter)) => {
                    return Ok(OrdinaryRead::Call {
                        getter,
                        receiver: self.dup_jsvalue(receiver)?,
                    });
                }
                ReadProbe::Missing(Some(next)) => prototype = Some(next),
                ReadProbe::Missing(None) => return Ok(OrdinaryRead::Complete(None)),
                ReadProbe::Special(kind @ SpecialKind::Proxy) => {
                    return Ok(OrdinaryRead::Special {
                        kind,
                        object: current.clone(),
                        receiver: self.dup_jsvalue(receiver)?,
                    });
                }
                ReadProbe::Special(kind) => {
                    // Integer-indexed exotic Get is terminal, including
                    // invalid/detached indices. It must not inspect a prototype.
                    if matches!(kind, SpecialKind::TypedArray)
                        && let Some(numeric) = self.typed_array_canonical_numeric_index(key)?
                    {
                        let value = match numeric {
                            crate::engine::builtins::CanonicalNumericIndex::Valid(index) => self
                                .typed_array_read_index(current, index)?
                                .unwrap_or(Value::Undefined),
                            crate::engine::builtins::CanonicalNumericIndex::Invalid => {
                                Value::Undefined
                            }
                        };
                        return Ok(OrdinaryRead::Complete(Some(self.unroot_value(&value)?)));
                    }
                    // Reuse the full storage kernel for Array holes, String,
                    // Arguments, namespace live cells and lazy own properties.
                    // Materializing a descriptor does not invoke its getter.
                    if let Some(own) = self.get_own_property_in_operation(current, key)? {
                        return Ok(match own {
                            CompleteOrdinaryPropertyDescriptor::Data { value, .. } => {
                                OrdinaryRead::Complete(Some(self.unroot_value(&value)?))
                            }
                            CompleteOrdinaryPropertyDescriptor::Accessor {
                                get: Some(getter),
                                ..
                            } => OrdinaryRead::Call {
                                getter,
                                receiver: self.dup_jsvalue(receiver)?,
                            },
                            CompleteOrdinaryPropertyDescriptor::Accessor { get: None, .. } => {
                                OrdinaryRead::Complete(Some(JsValue::Undefined))
                            }
                        });
                    }
                    // A non-Proxy object's prototype lookup has no user call.
                    // A Proxy reached on the next iteration is still returned
                    // as an explicit unresolved boundary with the same receiver.
                    let Some(next) = self.get_prototype_of(current)? else {
                        return Ok(OrdinaryRead::Complete(None));
                    };
                    prototype = Some(next);
                }
            }
        }
    }
}

/// A rooted ordinary lookup result, ready for an explicit caller to consume.
/// Data values and selected receivers each carry one internal owned edge.
pub(crate) enum OrdinaryRead {
    Complete(Option<crate::engine::value::JsValue>),
    Call {
        getter: crate::engine::object::CallableRef,
        receiver: JsValue,
    },
    Special {
        kind: SpecialKind,
        object: ObjectRef,
        receiver: JsValue,
    },
}

impl OrdinaryRead {
    /// Release an abandoned lookup reply without invoking the selected getter.
    pub(crate) fn release(self, runtime: &Runtime) {
        let value = match self {
            Self::Complete(value) => value,
            Self::Call { receiver, .. } | Self::Special { receiver, .. } => Some(receiver),
        };
        if let Some(value) = value {
            let _ = runtime.release_jsvalue(value);
        }
    }
}
