//! Owned descriptor request across observable DefineOwnProperty suspension.
use super::property::PropertyDescriptor;
use super::{AccessorValue, DescriptorField, OrdinaryPropertyDescriptor};
use crate::engine::{
    api::{runtime::Runtime, runtime_error::RuntimeError},
    heap::RawValue,
    value::JsValue,
};

pub(crate) struct OwnedPropertyDescriptor {
    runtime: Runtime,
    pub value: DescriptorField<JsValue>,
    pub writable: DescriptorField<bool>,
    pub get: DescriptorField<AccessorValue>,
    pub set: DescriptorField<AccessorValue>,
    pub enumerable: DescriptorField<bool>,
    pub configurable: DescriptorField<bool>,
}
impl OwnedPropertyDescriptor {
    pub(crate) fn new(runtime: &Runtime) -> Self {
        Self {
            runtime: runtime.clone(),
            value: DescriptorField::Absent,
            writable: DescriptorField::Absent,
            get: DescriptorField::Absent,
            set: DescriptorField::Absent,
            enumerable: DescriptorField::Absent,
            configurable: DescriptorField::Absent,
        }
    }
    pub(crate) fn data(runtime: &Runtime, value: JsValue) -> Self {
        let mut descriptor = Self::new(runtime);
        descriptor.value = DescriptorField::Present(value);
        descriptor.writable = DescriptorField::Present(true);
        descriptor.enumerable = DescriptorField::Present(true);
        descriptor.configurable = DescriptorField::Present(true);
        descriptor
    }
    pub(crate) fn from_public(
        runtime: &Runtime,
        source: &OrdinaryPropertyDescriptor,
    ) -> Result<Self, RuntimeError> {
        runtime.validate_descriptor_domains(source)?;
        let mut result = Self::new(runtime);
        result.writable = source.writable;
        result.get = source.get.clone();
        result.set = source.set.clone();
        result.enumerable = source.enumerable;
        result.configurable = source.configurable;
        if let DescriptorField::Present(value) = &source.value {
            result.value = DescriptorField::Present(runtime.unroot_value(value)?);
        }
        Ok(result)
    }
    pub(crate) fn raw_record(&self) -> PropertyDescriptor<RawValue> {
        PropertyDescriptor {
            value: self.value.as_ref().into_option().map(JsValue::as_raw),
            writable: self.writable.as_ref().into_option().copied(),
            get: self.get.as_ref().into_option().map(|v| {
                v.as_callable()
                    .map(|c| RawValue::Object(c.as_object().object_id()))
            }),
            set: self.set.as_ref().into_option().map(|v| {
                v.as_callable()
                    .map(|c| RawValue::Object(c.as_object().object_id()))
            }),
            enumerable: self.enumerable.as_ref().into_option().copied(),
            configurable: self.configurable.as_ref().into_option().copied(),
        }
    }
    pub(crate) fn attributes_public(&self) -> OrdinaryPropertyDescriptor {
        OrdinaryPropertyDescriptor {
            value: DescriptorField::Absent,
            writable: self.writable,
            get: self.get.clone(),
            set: self.set.clone(),
            enumerable: self.enumerable,
            configurable: self.configurable,
        }
    }
    pub(crate) fn is_mixed_descriptor(&self) -> bool {
        (self.value.is_present() || self.writable.is_present())
            && (self.get.is_present() || self.set.is_present())
    }
}
impl Drop for OwnedPropertyDescriptor {
    fn drop(&mut self) {
        if let DescriptorField::Present(value) =
            std::mem::replace(&mut self.value, DescriptorField::Absent)
        {
            let _ = self.runtime.release_jsvalue(value);
        }
    }
}

impl Runtime {
    pub(crate) fn proxy_descriptor_object_owned(
        &self,
        realm: crate::engine::heap::ContextId,
        descriptor: &OwnedPropertyDescriptor,
    ) -> Result<super::ObjectRef, RuntimeError> {
        let object = self.new_ordinary_object_in_realm(realm)?;
        let define = |name: &str, value: &JsValue| -> Result<(), RuntimeError> {
            let key = self.intern_property_key(name)?;
            match self.define_selected_set_data(&object, &key, value, false)? {
                super::operations::PropertyDefineOutcome::Defined(true) => Ok(()),
                _ => Err(RuntimeError::Invariant(
                    "fresh descriptor object rejected a field",
                )),
            }
        };
        for (name, accessor) in [("get", &descriptor.get), ("set", &descriptor.set)] {
            if let DescriptorField::Present(value) = accessor {
                let value = value.as_callable().map_or(JsValue::Undefined, |v| {
                    JsValue::Object(v.as_object().object_id())
                });
                define(name, &value)?;
            }
        }
        if let DescriptorField::Present(value) = &descriptor.value {
            define("value", value)?;
        }
        for (name, flag) in [
            ("writable", &descriptor.writable),
            ("enumerable", &descriptor.enumerable),
            ("configurable", &descriptor.configurable),
        ] {
            if let DescriptorField::Present(value) = flag {
                define(name, &JsValue::Bool(*value))?;
            }
        }
        Ok(object)
    }
}

/// Admission to the shared Define request: existing public descriptor producers
/// are converted once; migrated producers retain their internal edges directly.
pub(crate) enum DefinitionInput {
    Public(OrdinaryPropertyDescriptor),
    Owned(OwnedPropertyDescriptor),
}
impl From<OrdinaryPropertyDescriptor> for DefinitionInput {
    fn from(value: OrdinaryPropertyDescriptor) -> Self {
        Self::Public(value)
    }
}
impl From<OwnedPropertyDescriptor> for DefinitionInput {
    fn from(value: OwnedPropertyDescriptor) -> Self {
        Self::Owned(value)
    }
}
impl DefinitionInput {
    pub(crate) fn into_owned(
        self,
        runtime: &Runtime,
    ) -> Result<OwnedPropertyDescriptor, RuntimeError> {
        match self {
            Self::Owned(value) => Ok(value),
            Self::Public(value) => OwnedPropertyDescriptor::from_public(runtime, &value),
        }
    }
}

impl Runtime {
    pub(crate) fn prepare_array_length_definition_owned(
        &self,
        realm: Option<crate::engine::heap::ContextId>,
        object: &super::ObjectRef,
        key: &super::PropertyKey,
        descriptor: &OwnedPropertyDescriptor,
    ) -> Result<Option<super::ArrayLengthStep>, RuntimeError> {
        if self.array_own_key(object, key)? != super::operations::ArrayOwnKey::Length {
            return Ok(None);
        }
        self.validate_object_and_key(object, key)?;
        if descriptor.is_mixed_descriptor() {
            return Err(super::property::PropertyDefinitionError::InvalidDescriptor.into());
        }
        let DescriptorField::Present(value) = &descriptor.value else {
            return Ok(None);
        };
        super::ArrayLengthStep::start(self, realm, self.dup_jsvalue(value)?).map(Some)
    }
    pub(crate) fn prepare_typed_array_definition_owned(
        &self,
        object: &super::ObjectRef,
        key: &super::PropertyKey,
        descriptor: &OwnedPropertyDescriptor,
    ) -> Result<Option<crate::engine::builtins::TypedWriteStep>, RuntimeError> {
        use crate::engine::{
            builtins::{CanonicalNumericIndex, TypedWriteStep},
            value::conversion::NativeConversion,
        };
        if !self.typed_array_is_object(object)? {
            return Ok(None);
        }
        self.validate_object_and_key(object, key)?;
        if descriptor.is_mixed_descriptor() {
            return Err(super::property::PropertyDefinitionError::InvalidDescriptor.into());
        }
        let Some(index) = self.typed_array_canonical_numeric_index(key)? else {
            return Ok(None);
        };
        let CanonicalNumericIndex::Valid(index) = index else {
            return Ok(Some(TypedWriteStep::Complete(NativeConversion::Value(
                false,
            ))));
        };
        if descriptor.get.is_present()
            || descriptor.set.is_present()
            || matches!(descriptor.writable, DescriptorField::Present(false))
            || matches!(descriptor.enumerable, DescriptorField::Present(false))
            || matches!(descriptor.configurable, DescriptorField::Present(false))
        {
            return Ok(Some(TypedWriteStep::Complete(NativeConversion::Value(
                false,
            ))));
        }
        let state = self.typed_array_state(object)?;
        if state.out_of_bounds || index >= u64::from(state.length) {
            return Ok(Some(TypedWriteStep::Complete(NativeConversion::Value(
                false,
            ))));
        }
        let DescriptorField::Present(value) = &descriptor.value else {
            return Ok(Some(TypedWriteStep::Complete(NativeConversion::Value(
                true,
            ))));
        };
        TypedWriteStep::set(self, object.clone(), Some(index), self.dup_jsvalue(value)?).map(Some)
    }
}

/// Complete own-property reply owns its raw edges across invariant queries.
pub(crate) struct OwnedCompletePropertyDescriptor {
    runtime: Runtime,
    record: super::property::CompletePropertyDescriptor<RawValue>,
}
impl OwnedCompletePropertyDescriptor {
    pub(crate) fn from_raw(
        runtime: &Runtime,
        source: &super::property::CompletePropertyDescriptor<RawValue>,
    ) -> Result<Self, RuntimeError> {
        use super::property::CompletePropertyDescriptor;
        let mut owned = Self {
            runtime: runtime.clone(),
            record: CompletePropertyDescriptor::Accessor {
                get: None,
                set: None,
                enumerable: false,
                configurable: false,
            },
        };
        match source {
            CompletePropertyDescriptor::Data {
                value,
                writable,
                enumerable,
                configurable,
            } => {
                let value =
                    runtime
                        .dup_jsvalue(&JsValue::from_raw(value.clone()).ok_or(
                            RuntimeError::Invariant("descriptor held internal sentinel"),
                        )?)?;
                owned.record = CompletePropertyDescriptor::Data {
                    value: value.into_raw(),
                    writable: *writable,
                    enumerable: *enumerable,
                    configurable: *configurable,
                };
            }
            CompletePropertyDescriptor::Accessor {
                get,
                set,
                enumerable,
                configurable,
            } => {
                let CompletePropertyDescriptor::Accessor {
                    get: owned_get,
                    set: owned_set,
                    enumerable: e,
                    configurable: c,
                } = &mut owned.record
                else {
                    unreachable!()
                };
                *e = *enumerable;
                *c = *configurable;
                if let Some(value) = get {
                    *owned_get = Some(
                        runtime
                            .dup_jsvalue(
                                &JsValue::from_raw(value.clone())
                                    .ok_or(RuntimeError::Invariant("invalid getter sentinel"))?,
                            )?
                            .into_raw(),
                    );
                }
                if let Some(value) = set {
                    *owned_set = Some(
                        runtime
                            .dup_jsvalue(
                                &JsValue::from_raw(value.clone())
                                    .ok_or(RuntimeError::Invariant("invalid setter sentinel"))?,
                            )?
                            .into_raw(),
                    );
                }
            }
        }
        Ok(owned)
    }
    pub(crate) fn record(&self) -> &super::property::CompletePropertyDescriptor<RawValue> {
        &self.record
    }
    /// Consume a data descriptor and transfer its owned value edge out.
    ///
    /// The descriptor already owns one duplicated edge for the value; moving
    /// it to the caller instead of duplicating again avoids a retain plus a
    /// queued release on ordinary data reads. The vacated slot is left as an
    /// immediate so this descriptor's `Drop` releases nothing for it.
    /// Accessor records return `None` and stay untouched.
    pub(crate) fn into_data_value(mut self) -> Option<JsValue> {
        use super::property::CompletePropertyDescriptor;
        match &mut self.record {
            CompletePropertyDescriptor::Data { value, .. } => {
                JsValue::from_raw(std::mem::replace(value, RawValue::Undefined))
            }
            CompletePropertyDescriptor::Accessor { .. } => None,
        }
    }
    pub(crate) fn configurable(&self) -> bool {
        self.record.configurable()
    }
    pub(crate) fn enumerable(&self) -> bool {
        self.record.enumerable()
    }
    pub(crate) fn to_public(
        &self,
    ) -> Result<super::CompleteOrdinaryPropertyDescriptor, RuntimeError> {
        use super::property::CompletePropertyDescriptor;
        Ok(match &self.record {
            CompletePropertyDescriptor::Data {
                value,
                writable,
                enumerable,
                configurable,
            } => super::CompleteOrdinaryPropertyDescriptor::Data {
                value: self.runtime.root_raw_value(value.clone())?,
                writable: *writable,
                enumerable: *enumerable,
                configurable: *configurable,
            },
            CompletePropertyDescriptor::Accessor {
                get,
                set,
                enumerable,
                configurable,
            } => {
                let callable = |raw: &Option<RawValue>| -> Result<_, RuntimeError> {
                    raw.as_ref()
                        .map(|v| match v {
                            RawValue::Object(id) => Ok(super::CallableRef::from_validated_object(
                                super::ObjectRef::from_borrowed_handle(self.runtime.clone(), *id)?,
                            )),
                            _ => Err(RuntimeError::Invariant(
                                "complete descriptor accessor is not an object",
                            )),
                        })
                        .transpose()
                };
                super::CompleteOrdinaryPropertyDescriptor::Accessor {
                    get: callable(get)?,
                    set: callable(set)?,
                    enumerable: *enumerable,
                    configurable: *configurable,
                }
            }
        })
    }
    pub(crate) fn from_public(
        runtime: &Runtime,
        source: &super::CompleteOrdinaryPropertyDescriptor,
    ) -> Result<Self, RuntimeError> {
        use super::property::CompletePropertyDescriptor;
        let converted;
        let record = match source {
            super::CompleteOrdinaryPropertyDescriptor::Data {
                value,
                writable,
                enumerable,
                configurable,
            } => {
                converted = runtime.raw_property_value(value)?;
                CompletePropertyDescriptor::Data {
                    value: converted.raw(),
                    writable: *writable,
                    enumerable: *enumerable,
                    configurable: *configurable,
                }
            }
            super::CompleteOrdinaryPropertyDescriptor::Accessor {
                get,
                set,
                enumerable,
                configurable,
            } => CompletePropertyDescriptor::Accessor {
                get: get
                    .as_ref()
                    .map(|v| RawValue::Object(v.as_object().object_id())),
                set: set
                    .as_ref()
                    .map(|v| RawValue::Object(v.as_object().object_id())),
                enumerable: *enumerable,
                configurable: *configurable,
            },
        };
        Self::from_raw(runtime, &record)
    }
}
impl Drop for OwnedCompletePropertyDescriptor {
    fn drop(&mut self) {
        use super::property::CompletePropertyDescriptor;
        match &self.record {
            CompletePropertyDescriptor::Data { value, .. } => {
                if let Some(value) = JsValue::from_raw(value.clone()) {
                    let _ = self.runtime.release_jsvalue(value);
                }
            }
            CompletePropertyDescriptor::Accessor { get, set, .. } => {
                for value in get.iter().chain(set.iter()) {
                    if let Some(value) = JsValue::from_raw(value.clone()) {
                        let _ = self.runtime.release_jsvalue(value);
                    }
                }
            }
        }
    }
}

impl Runtime {
    pub(crate) fn public_descriptor_result(
        &self,
        value: crate::engine::value::conversion::NativeConversion<
            Option<OwnedCompletePropertyDescriptor>,
        >,
    ) -> Result<
        crate::engine::value::conversion::NativeConversion<
            Option<super::CompleteOrdinaryPropertyDescriptor>,
        >,
        RuntimeError,
    > {
        use crate::engine::value::conversion::NativeConversion;
        Ok(match value {
            NativeConversion::Throw(value) => NativeConversion::Throw(value),
            NativeConversion::Value(value) => {
                NativeConversion::Value(value.as_ref().map(|v| v.to_public()).transpose()?)
            }
        })
    }
}
