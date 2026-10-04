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
use crate::engine::object::{DescriptorField, ObjectRef, PropertyKey};
use crate::engine::value::conversion::NativeConversion;
use crate::engine::value::{JsValue, Value};

pub(crate) mod read;
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
        let _operation = self.operation()?;
        self.prepare_set_property_with_receiver_in_realm(
            None,
            object,
            key,
            value,
            Value::Object(object.try_clone().expect("duplicate root")),
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

    #[cfg(test)]
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
        let receiver = match self.into_jsvalue(receiver) {
            Ok(receiver) => receiver,
            Err(error) => {
                self.release_jsvalue(value)?;
                return Err(error);
            }
        };
        let mut step = SetStep::start(
            self,
            realm,
            object.try_clone().expect("duplicate root"),
            key.try_clone().expect("duplicate root"),
            value,
            receiver,
        )?;
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
    pub(crate) fn finish_prepared_read(
        &self,
        realm: ContextId,
        key: &PropertyKey,
        read: OrdinaryRead,
    ) -> Result<NativeConversion<Option<Value>>, RuntimeError> {
        Ok(match self.finish_prepared_read_jsvalue(realm, key, read)? {
            NativeConversion::Value(value) => NativeConversion::Value(
                value
                    .map(|v| self.root_and_release_jsvalue(v))
                    .transpose()?,
            ),
            NativeConversion::Throw(value) => NativeConversion::Throw(value),
        })
    }

    pub(crate) fn finish_prepared_read_jsvalue(
        &self,
        realm: ContextId,
        key: &PropertyKey,
        read: OrdinaryRead,
    ) -> Result<NativeConversion<Option<JsValue>>, RuntimeError> {
        use crate::engine::vm::Completion;
        let completion = match read {
            OrdinaryRead::Complete(value) => return Ok(NativeConversion::Value(value)),
            OrdinaryRead::Call { getter, receiver } => {
                self.call_internal_jsvalue(realm, &getter, receiver, Vec::new())?
            }
            OrdinaryRead::Special {
                kind,
                object,
                receiver,
            } => {
                if !matches!(kind, SpecialKind::Proxy) {
                    self.release_jsvalue(receiver)?;
                    return Err(RuntimeError::Invariant(
                        "prepared property read left a non-Proxy storage boundary",
                    ));
                }
                self.proxy_get_jsvalue(realm, &object, key, receiver)?
            }
        };
        Ok(match completion {
            Completion::Return(value) => NativeConversion::Value(Some(value)),
            Completion::Throw(value) => NativeConversion::Throw(value),
        })
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
        native: Option<&mut Option<crate::engine::object::LinkedNativeSelection>>,
    ) -> Result<OrdinaryRead, RuntimeError> {
        let _operation = self.operation()?;
        self.validate_object_and_key(object, key)?;
        let step = self.0.state.borrow_mut().prepare_ordinary_read_in_state(
            &self.0.poisoned,
            self.domain_id(),
            object.object_id(),
            key.atom(),
            receiver,
            false,
            native,
        )?;
        self.finish_read_boundary(step)
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
