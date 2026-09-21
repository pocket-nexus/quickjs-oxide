//! RegExp species selection keeps the defining-realm default rooted across getters.
use crate::engine::{
    api::{error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError},
    heap::ContextId,
    object::{ObjectRef, PropertyKey, WellKnownSymbol},
    value::{JsValue, conversion::NativeConversion},
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
pub(crate) struct RegExpSpeciesResume(Box<RegExpSpeciesResumeState>);
impl std::ops::Deref for RegExpSpeciesResume {
    type Target = RegExpSpeciesResumeState;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for RegExpSpeciesResume {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
const _: () = assert!(std::mem::size_of::<RegExpSpeciesResume>() <= 8);
pub(crate) struct RegExpSpeciesResumeState {
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
            key: runtime
                .pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Constructor)?,
            resume: RegExpSpeciesResume(Box::new(RegExpSpeciesResumeState {
                realm,
                default,
                species: false,
            })),
        })
    }
}
impl RegExpSpeciesResume {
    pub(crate) fn resume(
        mut self,
        runtime: &Runtime,
        result: Completion,
    ) -> Result<RegExpSpeciesStep, RuntimeError> {
        let value = match result {
            Completion::Return(value) => value,
            Completion::Throw(value) => {
                return Ok(RegExpSpeciesStep::Complete(NativeConversion::Throw(
                    runtime.root_and_release_jsvalue(value)?,
                )));
            }
        };
        if self.0.species {
            let result = if matches!(value, JsValue::Null | JsValue::Undefined) {
                NativeConversion::Value(self.0.default)
            } else if !matches!(value, JsValue::Object(_)) {
                {
                    let error = runtime.new_not_constructor_error_jsvalue(self.0.realm, &value);
                    runtime.release_jsvalue(value)?;
                    NativeConversion::Throw(error?)
                }
            } else {
                runtime.constructor_from_jsvalue(self.0.realm, value)?
            };
            return Ok(RegExpSpeciesStep::Complete(result));
        }
        if matches!(value, JsValue::Undefined) {
            return Ok(RegExpSpeciesStep::Complete(NativeConversion::Value(
                self.0.default,
            )));
        }
        let JsValue::Object(id) = value else {
            runtime.release_jsvalue(value)?;
            return Ok(RegExpSpeciesStep::Complete(NativeConversion::Throw(
                runtime.new_native_error(self.0.realm, NativeErrorKind::Type, "not an object")?,
            )));
        };
        Ok(RegExpSpeciesStep::Read {
            object: ObjectRef::from_owned_handle(runtime.clone(), id),
            key: PropertyKey::from(runtime.well_known_symbol(WellKnownSymbol::Species)),
            resume: {
                let updated_0 = true;
                self.0.species = updated_0;
                self
            },
        })
    }
}

// S11 all-domain protocol bound; inline completion stays allocation-free.
const _: () = assert!(std::mem::size_of::<RegExpSpeciesStep>() <= 64);
