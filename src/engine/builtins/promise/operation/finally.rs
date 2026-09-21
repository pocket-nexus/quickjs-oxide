//! Finally retains QuickJS's absent-species sentinel through the later resolve.
use super::{PromiseResume, PromiseStep, capability};
use crate::engine::api::{runtime::Runtime, runtime_error::RuntimeError};
use crate::engine::builtins::native::{NativeFunctionId, PromiseNativeKind};
use crate::engine::heap::PromiseReactionKind;
use crate::engine::heap::{ContextId, InternalCallableData};
use crate::engine::object::WellKnownSymbol;
use crate::engine::object::{ObjectRef, PropertyKey};
use crate::engine::value::{JsValue, conversion::NativeConversion};
use crate::engine::vm::{
    Completion,
    call::{NativeArguments, NativeInvocation},
};

pub(super) enum Phase {
    Constructor(FinallyInputs),
    Species(FinallyInputs),
    Callback(FinallyCapture),
    Resolved(FinallyCapture),
}
pub(super) struct FinallyInputs {
    runtime: Runtime,
    receiver: ObjectRef,
    callback: JsValue,
}
impl Drop for FinallyInputs {
    fn drop(&mut self) {
        let value = std::mem::replace(&mut self.callback, JsValue::Undefined);
        let _ = self.runtime.release_jsvalue(value);
    }
}
/// Owns the settlement across the callback and PromiseResolve suspensions.
/// The thunk copies its original handle before this suspension owner releases it.
pub(super) struct FinallyCapture {
    runtime: Runtime,
    constructor: Option<ObjectRef>,
    settlement: JsValue,
    kind: PromiseReactionKind,
}
impl Drop for FinallyCapture {
    fn drop(&mut self) {
        let settlement = std::mem::replace(&mut self.settlement, JsValue::Undefined);
        self.runtime
            .release_jsvalue(settlement)
            .expect("finally settlement release failed");
    }
}

fn continuation(runtime: &Runtime, realm: ContextId, phase: Phase) -> Box<PromiseResume> {
    Box::new(PromiseResume {
        runtime: runtime.clone(),
        pending_effect: super::PromiseStepPending::default(),
        realm,
        phase: super::Phase::Finally(phase),
    })
}
impl PromiseStep {
    pub(super) fn invoke_then(
        runtime: &Runtime,
        realm: ContextId,
        receiver: JsValue,
        arguments: Vec<JsValue>,
    ) -> Result<Self, RuntimeError> {
        let inputs = super::InvocationState::new(runtime, receiver, arguments);
        let key = runtime.pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Then)?;
        let receiver = runtime.dup_jsvalue(&inputs.receiver)?;
        Ok(Self::request_read(
            receiver,
            key,
            Box::new(PromiseResume {
                runtime: runtime.clone(),
                pending_effect: super::PromiseStepPending::default(),
                realm,
                phase: super::Phase::InvokeThen(inputs),
            }),
        ))
    }
}

pub(super) fn start(
    runtime: &Runtime,
    realm: ContextId,
    target: NativeFunctionId,
    invocation: &NativeInvocation,
    arguments: &NativeArguments,
) -> Result<PromiseStep, RuntimeError> {
    let NativeInvocation::Call { this_value } = invocation else {
        return Err(RuntimeError::Invariant(
            "Promise finally received constructor invocation",
        ));
    };
    let argument = arguments.readable.first().ok_or(RuntimeError::Invariant(
        "Promise finally argv was not padded",
    ))?;
    if target == NativeFunctionId::Promise(PromiseNativeKind::Finally) {
        let JsValue::Object(receiver_id) = this_value else {
            return capability::error(runtime, realm, "not an object");
        };
        let receiver = ObjectRef::from_borrowed_handle(runtime.clone(), *receiver_id)?;
        return Ok({
            let inputs = FinallyInputs {
                runtime: runtime.clone(),
                receiver,
                callback: runtime.dup_jsvalue(argument)?,
            };
            let __pending_field_key = runtime
                .pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Constructor)?;
            let __pending_field_resume = continuation(runtime, realm, Phase::Constructor(inputs));
            let __pending_field_receiver = runtime.dup_jsvalue(this_value)?;
            PromiseStep::request_read(
                __pending_field_receiver,
                __pending_field_key,
                __pending_field_resume,
            )
        });
    }
    let NativeFunctionId::PromiseFinallyHandler(kind) = target else {
        return Err(RuntimeError::Invariant("wrong finally operation"));
    };
    let active = runtime.active_function()?;
    let internal = runtime
        .0
        .state
        .borrow()
        .heap
        .native_internal_callable(active.object_id())?
        .ok_or(RuntimeError::Invariant(
            "Promise finally handler had no internal capture",
        ))?;
    let InternalCallableData::PromiseFinallyHandler {
        constructor,
        on_finally,
    } = internal
    else {
        return Err(RuntimeError::Invariant(
            "Promise finally handler had the wrong internal capture",
        ));
    };
    let callback = ObjectRef::from_borrowed_handle(runtime.clone(), on_finally)?;
    let callable = runtime
        .as_callable(&callback)?
        .ok_or(RuntimeError::Invariant(
            "Promise finally callback was no longer callable",
        ))?;
    // Root the capture before the callback can detach its last external owner.
    let constructor = match constructor {
        Some(id) => Some(ObjectRef::from_borrowed_handle(runtime.clone(), id)?),
        None => None,
    };
    Ok({
        let __pending_field_callable = callable;
        let __pending_field_receiver = JsValue::Undefined;
        let __pending_field_arguments = Vec::new();
        let __pending_field_resume = continuation(
            runtime,
            realm,
            Phase::Callback(FinallyCapture {
                runtime: runtime.clone(),
                constructor,
                settlement: runtime.dup_jsvalue(argument)?,
                kind,
            }),
        );
        PromiseStep::request_call(
            __pending_field_callable,
            __pending_field_receiver,
            __pending_field_arguments,
            __pending_field_resume,
        )
    })
}
pub(super) fn resume(
    runtime: &Runtime,
    realm: ContextId,
    phase: Phase,
    completion: Completion,
) -> Result<PromiseStep, RuntimeError> {
    let value = match completion {
        Completion::Throw(value) => return Ok(PromiseStep::Complete(Completion::Throw(value))),
        Completion::Return(value) => value,
    };
    match phase {
        Phase::Constructor(inputs) => match value {
            JsValue::Undefined => handlers(runtime, realm, inputs, None),
            JsValue::Object(constructor_id) => Ok({
                let constructor = ObjectRef::from_owned_handle(runtime.clone(), constructor_id);
                let __pending_field_receiver = JsValue::Object(constructor.into_handle());
                let __pending_field_key =
                    PropertyKey::from(runtime.well_known_symbol(WellKnownSymbol::Species));
                let __pending_field_resume = continuation(runtime, realm, Phase::Species(inputs));
                PromiseStep::request_read(
                    __pending_field_receiver,
                    __pending_field_key,
                    __pending_field_resume,
                )
            }),
            value => {
                runtime.release_jsvalue(value)?;
                capability::error(runtime, realm, "not an object")
            }
        },
        Phase::Species(inputs) => {
            let constructor = match value {
                JsValue::Undefined | JsValue::Null => None,
                value => match runtime.constructor_from_jsvalue(realm, value)? {
                    NativeConversion::Throw(value) => {
                        return Ok(PromiseStep::Complete(Completion::Throw(
                            runtime.into_jsvalue(value)?,
                        )));
                    }
                    NativeConversion::Value(constructor) => Some(constructor),
                },
            };
            handlers(runtime, realm, inputs, constructor)
        }
        Phase::Callback(mut capture) => {
            let constructor = capture
                .constructor
                .take()
                .map_or(JsValue::Undefined, |object| {
                    JsValue::Object(object.into_handle())
                });
            let step = Box::new(PromiseStep::static_resolve_jsvalue(
                runtime,
                realm,
                PromiseNativeKind::Resolve,
                constructor,
                value,
            )?);
            let resume = continuation(runtime, realm, Phase::Resolved(capture));
            Ok(PromiseStep::request_nested(step, resume))
        }
        Phase::Resolved(capture) => {
            let thunk = runtime.new_internal_promise_function(
                realm,
                NativeFunctionId::PromiseFinallyThunk(capture.kind),
                0,
                0,
                InternalCallableData::PromiseFinallyThunk {
                    value: capture.settlement.as_raw(),
                },
            );
            // The callable allocation retains the capture before its owner drops.
            drop(capture);
            let thunk = match thunk {
                Ok(thunk) => thunk,
                Err(error) => {
                    runtime.release_jsvalue(value)?;
                    return Err(error);
                }
            };
            PromiseStep::invoke_then(
                runtime,
                realm,
                value,
                vec![JsValue::Object(thunk.as_object().clone().into_handle())],
            )
        }
    }
}
fn handlers(
    runtime: &Runtime,
    realm: ContextId,
    inputs: FinallyInputs,
    constructor: Option<crate::engine::vm::call::ConstructorRef>,
) -> Result<PromiseStep, RuntimeError> {
    let handlers =
        runtime.prepare_promise_finally_handlers(realm, constructor, &inputs.callback)?;
    PromiseStep::invoke_then(
        runtime,
        realm,
        JsValue::Object(inputs.receiver.clone().into_handle()),
        handlers.into(),
    )
}
