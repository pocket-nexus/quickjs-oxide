//! ArraySpeciesCreate owns constructor selection until its construction finishes.
use crate::engine::{
    api::{runtime::Runtime, runtime_error::RuntimeError},
    heap::ContextId,
    object::{ObjectRef, PropertyKey, WellKnownSymbol},
    value::{JsValue, Value, conversion::NativeConversion},
    vm::{Completion, call::ConstructorRef},
};
pub(crate) enum SpeciesStep {
    Complete(Completion),
    Read {
        object: ObjectRef,
        key: PropertyKey,
        resume: SpeciesResume,
    },
    Construct {
        target: ConstructorRef,
        arguments: Vec<JsValue>,
    },
}
enum Phase {
    Constructor,
    Species,
}
pub(crate) struct SpeciesResume {
    realm: ContextId,
    length: u64,
    phase: Phase,
}
impl SpeciesStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        source: &ObjectRef,
        length: u64,
    ) -> Result<Self, RuntimeError> {
        match runtime.internal_is_array_jsvalue(realm, &JsValue::Object(source.object_id()))? {
            NativeConversion::Throw(value) => Ok(Self::Complete(Completion::Throw(value))),
            NativeConversion::Value(false) => allocate(runtime, realm, length),
            NativeConversion::Value(true) => Ok(Self::Read {
                object: source.clone(),
                key: runtime
                    .pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Constructor)?,
                resume: SpeciesResume {
                    realm,
                    length,
                    phase: Phase::Constructor,
                },
            }),
        }
    }
}
fn allocate(runtime: &Runtime, realm: ContextId, length: u64) -> Result<SpeciesStep, RuntimeError> {
    // The fresh, unpublished Array and numeric length cannot call JavaScript.
    Ok(SpeciesStep::Complete(
        runtime.new_array_with_length(realm, Some(length as f64))?,
    ))
}
impl SpeciesResume {
    pub(crate) fn resume(
        mut self,
        runtime: &Runtime,
        result: Completion,
    ) -> Result<SpeciesStep, RuntimeError> {
        let constructor = match result {
            Completion::Return(value) => value,
            Completion::Throw(value) => return Ok(SpeciesStep::Complete(Completion::Throw(value))),
        };
        if matches!(self.phase, Phase::Constructor) {
            if let JsValue::Object(id) = constructor {
                let object = ObjectRef::from_owned_handle(runtime.clone(), id);
                if runtime.is_constructor(&object)? {
                    let constructor_realm = match runtime
                        .function_realm_from_jsvalue(self.realm, &JsValue::Object(id))?
                    {
                        NativeConversion::Value(realm) => realm,
                        NativeConversion::Throw(value) => {
                            return Ok(SpeciesStep::Complete(Completion::Throw(value)));
                        }
                    };
                    if constructor_realm != self.realm
                        && runtime
                            .0
                            .state
                            .borrow()
                            .heap
                            .context(constructor_realm)?
                            .array_constructor
                            == Some(id)
                    {
                        return allocate(runtime, self.realm, self.length);
                    }
                }
                self.phase = Phase::Species;
                return Ok(SpeciesStep::Read {
                    object,
                    key: PropertyKey::from(runtime.well_known_symbol(WellKnownSymbol::Species)),
                    resume: self,
                });
            }
        } else if matches!(constructor, JsValue::Null) {
            return allocate(runtime, self.realm, self.length);
        }
        if matches!(constructor, JsValue::Undefined) {
            return allocate(runtime, self.realm, self.length);
        }
        match runtime.constructor_from_jsvalue(self.realm, constructor)? {
            NativeConversion::Value(target) => Ok(SpeciesStep::Construct {
                target,
                arguments: vec![runtime.into_jsvalue(Value::number(self.length as f64))?],
            }),
            NativeConversion::Throw(value) => Ok(SpeciesStep::Complete(Completion::Throw(value))),
        }
    }
}
pub(crate) fn finish(
    runtime: &Runtime,
    realm: ContextId,
    mut step: SpeciesStep,
) -> Result<Completion, RuntimeError> {
    loop {
        step = match step {
            SpeciesStep::Complete(result) => return Ok(result),
            SpeciesStep::Read {
                object,
                key,
                resume,
            } => resume.resume(
                runtime,
                runtime.get_property_in_realm(realm, &object, &key)?,
            )?,
            SpeciesStep::Construct { target, arguments } => {
                return runtime.construct_internal_jsvalue(
                    realm,
                    &target,
                    crate::engine::vm::call::ConstructNewTarget::Validated(target.clone()),
                    arguments,
                );
            }
        };
    }
}

// S11 all-domain protocol bound; inline completion stays allocation-free.
const _: () = assert!(std::mem::size_of::<SpeciesStep>() <= 64);
