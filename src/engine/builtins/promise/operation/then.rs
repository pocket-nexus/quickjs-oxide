//! Public then preserves species/capability effects before inspecting handlers.
use super::{
    Completion, ContextId, NativeArguments, NativeConversion, NativeInvocation, ObjectRef, Phase,
    PromiseResume, PromiseStep, Runtime, RuntimeError, Value,
};
use crate::engine::{
    heap::ObjectPayload,
    object::{PropertyKey, WellKnownSymbol},
};
impl PromiseStep {
    pub(super) fn then(
        runtime: &Runtime,
        realm: ContextId,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        let NativeInvocation::Call { this_value } = invocation else {
            return Err(RuntimeError::Invariant(
                "Promise.prototype.then received a constructor invocation",
            ));
        };
        let Value::Object(promise) = this_value else {
            return super::capability::error(runtime, realm, "not a promise");
        };
        if !matches!(
            runtime
                .0
                .state
                .borrow()
                .heap
                .object(promise.object_id())?
                .payload,
            ObjectPayload::Promise(_)
        ) {
            return super::capability::error(runtime, realm, "not a promise");
        }
        let handlers = [
            arguments
                .readable
                .first()
                .cloned()
                .ok_or(RuntimeError::Invariant(
                    "Promise.then fulfill argv was not padded",
                ))?,
            arguments
                .readable
                .get(1)
                .cloned()
                .ok_or(RuntimeError::Invariant(
                    "Promise.then reject argv was not padded",
                ))?,
        ];
        Ok(Self::Read {
            receiver: Value::Object(promise.clone()),
            key: runtime.intern_property_key("constructor")?,
            resume: Box::new(PromiseResume {
                realm,
                phase: Phase::ThenConstructor {
                    promise: promise.clone(),
                    handlers,
                },
            }),
        })
    }
}
pub(super) fn constructor(
    runtime: &Runtime,
    realm: ContextId,
    promise: ObjectRef,
    handlers: [Value; 2],
    result: Completion,
) -> Result<PromiseStep, RuntimeError> {
    match result {
        Completion::Throw(value) => Ok(PromiseStep::Complete(Completion::Throw(value))),
        Completion::Return(Value::Undefined) => Box::new(PromiseResume {
            realm,
            phase: Phase::ThenCapability { promise, handlers },
        })
        .capability(runtime, None),
        Completion::Return(Value::Object(constructor)) => Ok(PromiseStep::Read {
            receiver: Value::Object(constructor),
            key: PropertyKey::from(runtime.well_known_symbol(WellKnownSymbol::Species)),
            resume: Box::new(PromiseResume {
                realm,
                phase: Phase::ThenSpecies { promise, handlers },
            }),
        }),
        Completion::Return(_) => super::capability::error(runtime, realm, "not an object"),
    }
}
pub(super) fn species(
    runtime: &Runtime,
    realm: ContextId,
    promise: ObjectRef,
    handlers: [Value; 2],
    result: Completion,
) -> Result<PromiseStep, RuntimeError> {
    let constructor = match result {
        Completion::Throw(value) => return Ok(PromiseStep::Complete(Completion::Throw(value))),
        Completion::Return(Value::Undefined | Value::Null) => None,
        Completion::Return(value) => match runtime.constructor_from_value(realm, value)? {
            NativeConversion::Throw(value) => {
                return Ok(PromiseStep::Complete(Completion::Throw(value)));
            }
            NativeConversion::Value(constructor) => Some(constructor),
        },
    };
    Box::new(PromiseResume {
        realm,
        phase: Phase::ThenCapability { promise, handlers },
    })
    .capability(runtime, constructor)
}
