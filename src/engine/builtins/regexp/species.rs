//! RegExp species selection keeps the defining-realm default rooted across getters.
use crate::engine::{
    api::{error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError},
    heap::ContextId,
    object::{ObjectRef, PropertyKey, WellKnownSymbol},
    value::{Value, conversion::NativeConversion},
    vm::{Completion, call::ConstructorRef},
};
pub(crate) enum RegExpSpeciesStep {
    Complete(NativeConversion<ConstructorRef>),
    Read {
        object: ObjectRef,
        key: PropertyKey,
        resume: RegExpSpeciesResume,
    },
}
pub(crate) struct RegExpSpeciesResume {
    realm: ContextId,
    default: ConstructorRef,
    species: bool,
}
impl RegExpSpeciesStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        regexp: ObjectRef,
    ) -> Result<Self, RuntimeError> {
        let default_id = runtime.regexp_realm_data(realm)?.constructor;
        let default = ConstructorRef::from_validated_object(ObjectRef::from_borrowed_handle(
            runtime.clone(),
            default_id,
        )?);
        Ok(Self::Read {
            object: regexp,
            key: runtime.intern_property_key("constructor")?,
            resume: RegExpSpeciesResume {
                realm,
                default,
                species: false,
            },
        })
    }
}
impl RegExpSpeciesResume {
    pub(crate) fn resume(
        self,
        runtime: &Runtime,
        result: Completion,
    ) -> Result<RegExpSpeciesStep, RuntimeError> {
        let value = match result {
            Completion::Return(value) => value,
            Completion::Throw(value) => {
                return Ok(RegExpSpeciesStep::Complete(NativeConversion::Throw(value)));
            }
        };
        if self.species {
            let result = if matches!(value, Value::Null | Value::Undefined) {
                NativeConversion::Value(self.default)
            } else if !matches!(value, Value::Object(_)) {
                NativeConversion::Throw(runtime.new_not_constructor_error(self.realm, &value)?)
            } else {
                runtime.constructor_from_value(self.realm, value)?
            };
            return Ok(RegExpSpeciesStep::Complete(result));
        }
        if matches!(value, Value::Undefined) {
            return Ok(RegExpSpeciesStep::Complete(NativeConversion::Value(
                self.default,
            )));
        }
        let Value::Object(object) = value else {
            return Ok(RegExpSpeciesStep::Complete(NativeConversion::Throw(
                runtime.new_native_error(self.realm, NativeErrorKind::Type, "not an object")?,
            )));
        };
        Ok(RegExpSpeciesStep::Read {
            object,
            key: PropertyKey::from(runtime.well_known_symbol(WellKnownSymbol::Species)),
            resume: Self {
                species: true,
                ..self
            },
        })
    }
}
