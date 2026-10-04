//! One borrowed function-realm walk, kept live by the actual input owner.
use crate::engine::{
    api::{
        error::{Error, ErrorKind, NativeErrorKind, NativeErrorMessage},
        runtime_error::RuntimeError,
    },
    heap::{ContextId, ObjectId, ObjectPayload, runtime::RuntimeState},
    value::{JsValue, conversion::NativeConversion},
};
use std::cell::Cell;
pub(crate) enum FunctionRealmOutcome {
    Value(ContextId),
    CyclePublishedThrow(JsValue),
}
impl FunctionRealmOutcome {
    pub(super) fn into_conversion(self) -> NativeConversion<ContextId> {
        match self {
            Self::Value(value) => NativeConversion::Value(value),
            Self::CyclePublishedThrow(value) => NativeConversion::Throw(value),
        }
    }
}
impl RuntimeState {
    pub(crate) fn function_realm_from_jsvalue(
        &mut self,
        poisoned: &Cell<bool>,
        caller_realm: ContextId,
        value: &JsValue,
    ) -> Result<FunctionRealmOutcome, RuntimeError> {
        if poisoned.get() {
            return Err(RuntimeError::Poisoned);
        }
        self.heap.context(caller_realm)?;
        let result = match value {
            JsValue::Object(object) => {
                self.function_realm_object(poisoned, Some(caller_realm), *object, true)
            }
            _ => Ok(FunctionRealmOutcome::Value(caller_realm)),
        };
        if poisoned.get() {
            return Err(RuntimeError::Poisoned);
        }
        result
    }
    pub(super) fn function_realm_object(
        &mut self,
        poisoned: &Cell<bool>,
        caller_realm: Option<ContextId>,
        mut object: ObjectId,
        allow_non_function: bool,
    ) -> Result<FunctionRealmOutcome, RuntimeError> {
        // Each selected target is still a heap edge of the original live chain.
        // No JS/host callback or State exit occurs while relying on this proof.
        loop {
            match &self.heap.object(object)?.payload {
                ObjectPayload::NativeFunction { data, .. } => {
                    let defining = data.realm.ok_or(RuntimeError::Invariant(
                        "native function has no defining realm",
                    ))?;
                    let realm = if allow_non_function && data.target.uses_calling_realm() {
                        caller_realm.ok_or(RuntimeError::Invariant(
                            "raw function realm lookup had no fallback realm",
                        ))?
                    } else {
                        defining
                    };
                    self.heap.context(realm)?;
                    return Ok(FunctionRealmOutcome::Value(realm));
                }
                ObjectPayload::BytecodeFunction { bytecode, .. } => {
                    let realm = self.heap.function_bytecode(*bytecode)?.realm;
                    self.heap.context(realm)?;
                    return Ok(FunctionRealmOutcome::Value(realm));
                }
                ObjectPayload::BoundFunction { target, .. } => object = *target,
                ObjectPayload::Proxy(data) => {
                    if data.is_revoked {
                        let Some(realm) = caller_realm else {
                            return Err(RuntimeError::Engine(Error::new(
                                ErrorKind::Type,
                                "revoked proxy",
                            )));
                        };
                        return Ok(FunctionRealmOutcome::CyclePublishedThrow(JsValue::Object(
                            self.new_native_error_from_message(
                                poisoned,
                                realm,
                                NativeErrorKind::Type,
                                NativeErrorMessage::from_utf8("revoked proxy"),
                            )?,
                        )));
                    }
                    object = data.target;
                }
                _ if allow_non_function => {
                    return Ok(FunctionRealmOutcome::Value(caller_realm.ok_or(
                        RuntimeError::Invariant("raw function realm lookup had no fallback realm"),
                    )?));
                }
                _ => {
                    return Err(RuntimeError::Engine(Error::new(
                        ErrorKind::Type,
                        "not a function",
                    )));
                }
            }
        }
    }
}
