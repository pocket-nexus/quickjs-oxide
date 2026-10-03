use crate::engine::api::runtime_error::RuntimeError;
use crate::engine::heap::{ObjectId, RawValue, VarRefId};
use crate::engine::object::property::{CompletePropertyDescriptor, PropertyDescriptor};
use crate::engine::object::shape::PropertyFlags;
use crate::engine::object::{
    CallableRef, CompleteOrdinaryPropertyDescriptor, ObjectRef, OrdinaryPropertyDescriptor,
};
use crate::engine::value::{JsString, Value};

pub(crate) enum RawStringProperty {
    Missing,
    String(JsString),
    Other,
}

pub(crate) enum PropertySnapshot {
    Data {
        value: RawValue,
        flags: PropertyFlags,
    },
    VarRef {
        var_ref: VarRefId,
        flags: PropertyFlags,
    },
    Accessor {
        get: Option<ObjectId>,
        set: Option<ObjectId>,
        flags: PropertyFlags,
    },
    AutoInit,
}

#[cfg(test)]
pub(crate) enum PropertyGetAction {
    Complete(Value),
    Call {
        getter: CallableRef,
        receiver: Value,
    },
}

pub(crate) enum PropertySetAction {
    RejectedProxyTrap,
    Complete,
    Rejected(PropertySetRejection),
    Throw(crate::engine::value::JsValue),
    Call { payload: Box<PropertySetterCall> },
}

// Allocated only after selecting a real setter invocation.
pub(crate) struct PropertySetterCall {
    runtime: crate::engine::api::runtime::Runtime,
    setter: Option<CallableRef>,
    receiver: Option<crate::engine::value::JsValue>,
    argument: Option<crate::engine::value::JsValue>,
}
impl PropertySetterCall {
    pub(crate) fn new(
        runtime: &crate::engine::api::runtime::Runtime,
        setter: CallableRef,
        receiver: crate::engine::value::JsValue,
        argument: crate::engine::value::JsValue,
    ) -> Self {
        Self {
            runtime: runtime.clone(),
            setter: Some(setter),
            receiver: Some(receiver),
            argument: Some(argument),
        }
    }
    pub(crate) fn into_parts(
        mut self,
    ) -> (
        CallableRef,
        crate::engine::value::JsValue,
        crate::engine::value::JsValue,
    ) {
        (
            self.setter.take().expect("setter"),
            self.receiver.take().expect("setter receiver"),
            self.argument.take().expect("setter argument"),
        )
    }
}
impl Drop for PropertySetterCall {
    fn drop(&mut self) {
        if let Some(receiver) = self.receiver.take() {
            let _ = self.runtime.release_jsvalue(receiver);
        }
        if let Some(value) = self.argument.take() {
            let _ = self.runtime.release_jsvalue(value);
        }
    }
}
const _: () = assert!(std::mem::size_of::<PropertySetAction>() <= 64);

pub(crate) enum PropertyDefineOutcome {
    Defined(bool),
    Throw(crate::engine::value::JsValue),
}

pub(crate) enum ArrayLengthConversion {
    Length(u32),
    Throw(crate::engine::value::JsValue),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ArrayOwnKey {
    Length,
    Index(u32),
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PropertySetRejection {
    ReadOnly,
    ArrayLengthReadOnly,
    NotConfigurable,
    NoSetter,
    NotExtensible,
    NotObject,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum InternalSetResult {
    Accepted,
    Rejected(PropertySetRejection),
    RejectedProxyTrap,
}

#[derive(Debug)]
pub(crate) enum InternalDefineResult {
    Defined,
    RejectedOrdinary(ObjectRef),
    RejectedProxyTrap,
}

/// Descriptor validation consumes no owners. All inputs remain borrowed
/// until the effect-free algorithm has selected the result to publish.
#[derive(Clone, Copy)]
pub(crate) enum ValidationValue<'a> {
    Value(&'a Value),
    Callable(&'a CallableRef),
    Undefined,
}
impl ValidationValue<'_> {
    pub(crate) fn same_value(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Value(left), Self::Value(right)) => left.same_value(right),
            (Self::Callable(left), Self::Callable(right)) => left == right,
            (Self::Callable(left), Self::Value(Value::Object(right)))
            | (Self::Value(Value::Object(right)), Self::Callable(left)) => {
                left.as_object() == right
            }
            (Self::Undefined, Self::Undefined)
            | (Self::Undefined, Self::Value(Value::Undefined))
            | (Self::Value(Value::Undefined), Self::Undefined) => true,
            _ => false,
        }
    }
    fn into_value(self) -> Result<Value, RuntimeError> {
        match self {
            Self::Value(value) => value.try_clone(),
            Self::Callable(value) => Ok(Value::Object(value.as_object().try_clone()?)),
            Self::Undefined => Ok(Value::Undefined),
        }
    }
    fn into_callable(self) -> Result<CallableRef, RuntimeError> {
        match self {
            Self::Callable(value) => value.try_clone(),
            _ => Err(RuntimeError::Invariant(
                "validated accessor was not callable",
            )),
        }
    }
}

pub(crate) fn descriptor_to_validation_record(
    descriptor: &OrdinaryPropertyDescriptor,
) -> PropertyDescriptor<ValidationValue<'_>> {
    PropertyDescriptor {
        value: descriptor
            .value
            .as_ref()
            .into_option()
            .map(ValidationValue::Value),
        writable: descriptor.writable.as_ref().into_option().copied(),
        get: descriptor
            .get
            .as_ref()
            .into_option()
            .map(|accessor| accessor.as_callable().map(ValidationValue::Callable)),
        set: descriptor
            .set
            .as_ref()
            .into_option()
            .map(|accessor| accessor.as_callable().map(ValidationValue::Callable)),
        enumerable: descriptor.enumerable.as_ref().into_option().copied(),
        configurable: descriptor.configurable.as_ref().into_option().copied(),
    }
}

pub(crate) fn complete_to_validation_record(
    descriptor: &CompleteOrdinaryPropertyDescriptor,
) -> CompletePropertyDescriptor<ValidationValue<'_>> {
    match descriptor {
        CompleteOrdinaryPropertyDescriptor::Data {
            value,
            writable,
            enumerable,
            configurable,
        } => CompletePropertyDescriptor::Data {
            value: ValidationValue::Value(value),
            writable: *writable,
            enumerable: *enumerable,
            configurable: *configurable,
        },
        CompleteOrdinaryPropertyDescriptor::Accessor {
            get,
            set,
            enumerable,
            configurable,
        } => CompletePropertyDescriptor::Accessor {
            get: get.as_ref().map(ValidationValue::Callable),
            set: set.as_ref().map(ValidationValue::Callable),
            enumerable: *enumerable,
            configurable: *configurable,
        },
    }
}

pub(crate) fn validation_record_to_complete(
    descriptor: CompletePropertyDescriptor<ValidationValue<'_>>,
) -> Result<CompleteOrdinaryPropertyDescriptor, RuntimeError> {
    Ok(match descriptor {
        CompletePropertyDescriptor::Data {
            value,
            writable,
            enumerable,
            configurable,
        } => CompleteOrdinaryPropertyDescriptor::Data {
            value: value.into_value()?,
            writable,
            enumerable,
            configurable,
        },
        CompletePropertyDescriptor::Accessor {
            get,
            set,
            enumerable,
            configurable,
        } => CompleteOrdinaryPropertyDescriptor::Accessor {
            get: get.map(ValidationValue::into_callable).transpose()?,
            set: set.map(ValidationValue::into_callable).transpose()?,
            enumerable,
            configurable,
        },
    })
}
