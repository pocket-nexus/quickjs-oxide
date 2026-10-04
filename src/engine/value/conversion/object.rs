//! ToObject shares its checked prototype preparation and consuming allocation.
use super::{NativeConversion, PrimitiveKind, Runtime, RuntimeError};
use crate::engine::{
    api::{
        error::{NativeErrorKind, NativeErrorMessage},
        runtime::RuntimeUnwindGuard,
    },
    heap::{
        ContextId, ObjectId,
        runtime::{RuntimeState, owned_values::OwnedValueGuard},
    },
    object::ObjectRef,
    value::JsValue,
};
use std::cell::Cell;

pub(crate) enum ToObjectOutcome {
    Existing(ObjectId),
    Boxed(ObjectId),
    Throw(JsValue),
}
enum ToObjectPreparation {
    Complete(ToObjectOutcome),
    Prototype {
        prototype: ObjectId,
        kind: PrimitiveKind,
        value: Option<JsValue>,
    },
    Box {
        prototype: Option<ObjectId>,
        kind: PrimitiveKind,
        value: Option<JsValue>,
    },
}
impl ToObjectPreparation {
    fn retire_with(
        self,
        mut release: impl FnMut(JsValue) -> Result<(), RuntimeError>,
    ) -> Result<(), RuntimeError> {
        match self {
            Self::Complete(ToObjectOutcome::Existing(object) | ToObjectOutcome::Boxed(object)) => {
                release(JsValue::Object(object))
            }
            Self::Complete(ToObjectOutcome::Throw(value)) => release(value),
            Self::Prototype { value, .. } => {
                if let Some(value) = value {
                    release(value)?;
                }
                Ok(())
            }
            Self::Box {
                prototype, value, ..
            } => {
                if let Some(value) = value {
                    release(value)?;
                }
                if let Some(prototype) = prototype {
                    release(JsValue::Object(prototype))?;
                }
                Ok(())
            }
        }
    }
}
impl RuntimeState {
    fn prepare_to_object_jsvalue(
        &mut self,
        poisoned: &Cell<bool>,
        realm: ContextId,
        value: JsValue,
    ) -> Result<ToObjectPreparation, RuntimeError> {
        match self.prepare_to_object_admitted(poisoned, realm, value) {
            Err(_) if poisoned.get() => Err(RuntimeError::Poisoned),
            result => result,
        }
    }
    fn prepare_to_object_admitted(
        &mut self,
        poisoned: &Cell<bool>,
        realm: ContextId,
        value: JsValue,
    ) -> Result<ToObjectPreparation, RuntimeError> {
        let kind = match value {
            JsValue::Object(object) => {
                return Ok(ToObjectPreparation::Complete(ToObjectOutcome::Existing(
                    object,
                )));
            }
            JsValue::Undefined | JsValue::Null => {
                return Ok(ToObjectPreparation::Complete(ToObjectOutcome::Throw(
                    JsValue::Object(self.new_native_error_from_message(
                        poisoned,
                        realm,
                        NativeErrorKind::Type,
                        NativeErrorMessage::from_utf8("cannot convert to object"),
                    )?),
                )));
            }
            JsValue::Bool(_) => PrimitiveKind::Boolean,
            JsValue::Int(_) | JsValue::Float(_) => PrimitiveKind::Number,
            JsValue::String(_) => PrimitiveKind::String,
            JsValue::BigInt(_) | JsValue::ShortBigInt(_) => PrimitiveKind::BigInt,
            JsValue::Symbol(_) => PrimitiveKind::Symbol,
        };
        let mut input = OwnedValueGuard::new(self, poisoned, value);
        let (state, input) = input.parts();
        let prototype = state.primitive_prototype_id_for_realm(realm, kind)?;
        Ok(ToObjectPreparation::Prototype {
            prototype,
            kind,
            value: input.take(),
        })
    }
    fn retain_to_object_prototype(
        &mut self,
        poisoned: &Cell<bool>,
        prepared: ToObjectPreparation,
    ) -> Result<ToObjectPreparation, RuntimeError> {
        match self.retain_to_object_admitted(poisoned, prepared) {
            Err(_) if poisoned.get() => Err(RuntimeError::Poisoned),
            result => result,
        }
    }
    fn retain_to_object_admitted(
        &mut self,
        poisoned: &Cell<bool>,
        prepared: ToObjectPreparation,
    ) -> Result<ToObjectPreparation, RuntimeError> {
        let ToObjectPreparation::Prototype {
            prototype,
            kind,
            value,
        } = prepared
        else {
            return Ok(prepared);
        };
        let mut input =
            OwnedValueGuard::new(self, poisoned, value.expect("ToObject primitive input"));
        let (state, input) = input.parts();
        state.heap.retain_object(prototype)?;
        Ok(ToObjectPreparation::Box {
            prototype: Some(prototype),
            kind,
            value: input.take(),
        })
    }
    fn finish_to_object_jsvalue(
        &mut self,
        poisoned: &Cell<bool>,
        prepared: ToObjectPreparation,
    ) -> Result<ToObjectOutcome, RuntimeError> {
        match self.finish_to_object_admitted(poisoned, prepared) {
            Err(_) if poisoned.get() => Err(RuntimeError::Poisoned),
            result => result,
        }
    }
    fn finish_to_object_admitted(
        &mut self,
        poisoned: &Cell<bool>,
        mut prepared: ToObjectPreparation,
    ) -> Result<ToObjectOutcome, RuntimeError> {
        let _unwind = RuntimeUnwindGuard::from_flag(poisoned);
        let result = match &mut prepared {
            ToObjectPreparation::Complete(_) => {
                let ToObjectPreparation::Complete(result) = prepared else {
                    unreachable!()
                };
                return Ok(result);
            }
            ToObjectPreparation::Prototype { .. } => {
                unreachable!("ToObject prototype was not retained")
            }
            ToObjectPreparation::Box {
                prototype,
                kind,
                value,
            } => self.new_primitive_object_jsvalue(
                poisoned,
                prototype.expect("ToObject prototype owner"),
                *kind,
                value.take().expect("ToObject primitive input"),
                false,
            ),
        };
        let object = match result {
            Ok(object) => object,
            Err(error) => {
                if poisoned.get() {
                    return Err(RuntimeError::Poisoned);
                }
                prepared.retire_with(|value| self.release_owned_jsvalue(poisoned, value))?;
                return Err(error);
            }
        };
        let mut output = OwnedValueGuard::new(self, poisoned, JsValue::Object(object));
        let (state, output) = output.parts();
        prepared.retire_with(|value| state.release_owned_jsvalue(poisoned, value))?;
        output.take();
        Ok(ToObjectOutcome::Boxed(object))
    }
    /// Consumes the input on every normal exit. The resident caller already
    /// owns its admission and executes these same two phases under one lease.
    pub(crate) fn native_to_object_jsvalue(
        &mut self,
        poisoned: &Cell<bool>,
        realm: ContextId,
        value: JsValue,
    ) -> Result<ToObjectOutcome, RuntimeError> {
        let prepared = self.prepare_to_object_jsvalue(poisoned, realm, value)?;
        let prepared = match self.retain_to_object_prototype(poisoned, prepared) {
            Err(_) if poisoned.get() => return Err(RuntimeError::Poisoned),
            result => result?,
        };
        self.finish_to_object_jsvalue(poisoned, prepared)
    }
}

struct ToObjectBoundaryGuard<'a> {
    runtime: &'a Runtime,
    prepared: Option<ToObjectPreparation>,
}
impl Drop for ToObjectBoundaryGuard<'_> {
    fn drop(&mut self) {
        if self.runtime.skip_cleanup() {
            return;
        }
        let _unwind = self.runtime.unwind_guard();
        if let Some(prepared) = self.prepared.take() {
            if let Ok(mut state) = self.runtime.0.state.try_borrow_mut() {
                let _ = prepared.retire_with(|value| {
                    state.release_owned_jsvalue(&self.runtime.0.poisoned, value)
                });
            } else {
                let _ = prepared.retire_with(|value| {
                    self.runtime.release_jsvalue(value)?;
                    self.runtime.check_poison()
                });
            }
        }
    }
}
impl Runtime {
    /// Internal ToObject keeps the historical direct object-owner return and
    /// checked prototype preparation before primitive allocation admission.
    pub(crate) fn native_to_object_jsvalue(
        &self,
        realm: ContextId,
        value: JsValue,
    ) -> Result<NativeConversion<ObjectRef>, RuntimeError> {
        if let JsValue::Object(object) = value {
            return Ok(NativeConversion::Value(ObjectRef::from_owned_handle(
                self.clone(),
                object,
            )));
        }
        if matches!(value, JsValue::Undefined | JsValue::Null) {
            let _operation = self.operation()?;
            let result = self.0.state.borrow_mut().native_to_object_jsvalue(
                &self.0.poisoned,
                realm,
                value,
            )?;
            let ToObjectOutcome::Throw(value) = result else {
                unreachable!("nullish ToObject throws")
            };
            return Ok(NativeConversion::Throw(value));
        }
        let mut owner = ToObjectBoundaryGuard {
            runtime: self,
            prepared: None,
        };
        {
            let _unwind = self.unwind_guard();
            owner.prepared = Some(self.0.state.borrow_mut().prepare_to_object_jsvalue(
                &self.0.poisoned,
                realm,
                value,
            )?);
        }
        // The old ObjectRef adapter checked the header after realm lookup,
        // before its temporary prototype retain and allocation admission.
        self.check_poison()?;
        owner.prepared = Some(
            self.0.state.borrow_mut().retain_to_object_prototype(
                &self.0.poisoned,
                owner
                    .prepared
                    .take()
                    .expect("ToObject primitive preparation"),
            )?,
        );
        let _operation = self.operation()?;
        let result = self.0.state.borrow_mut().finish_to_object_jsvalue(
            &self.0.poisoned,
            owner.prepared.take().expect("ToObject prepared owner"),
        )?;
        Ok(match result {
            ToObjectOutcome::Existing(object) | ToObjectOutcome::Boxed(object) => {
                NativeConversion::Value(ObjectRef::from_owned_handle(self.clone(), object))
            }
            ToObjectOutcome::Throw(value) => NativeConversion::Throw(value),
        })
    }
}

#[cfg(test)]
mod tests;
