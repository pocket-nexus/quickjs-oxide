//! Borrow current descriptors; no result root or descriptor transport is needed.
use super::*;
use crate::engine::{
    api::error::{NativeErrorKind, NativeErrorMessage},
    heap::{Heap, RawValue},
    object::{StateOwnPropertySnapshot, property::CompletePropertyDescriptor},
};
pub(super) fn violation(
    heap: &Heap,
    result: &JsValue,
    descriptor: &CompletePropertyDescriptor<RawValue>,
) -> bool {
    match descriptor {
        CompletePropertyDescriptor::Data {
            value,
            writable: false,
            configurable: false,
            ..
        } => !crate::engine::value::collection_key::same_value(heap, &result.as_raw(), value),
        CompletePropertyDescriptor::Accessor {
            get: None,
            configurable: false,
            ..
        } => !matches!(result, JsValue::Undefined),
        _ => false,
    }
}
pub(super) fn check_target(
    state: &mut RuntimeState,
    runtime: &Runtime,
    target: ObjectId,
    atom: Atom,
    result: &JsValue,
) -> Result<Option<bool>, RuntimeError> {
    Ok(
        match state.own_property_snapshot_in_state(&runtime.0.poisoned, target, atom)? {
            StateOwnPropertySnapshot::Absent => Some(false),
            StateOwnPropertySnapshot::Borrowed(s) => Some(violation(s.heap(), result, s.record())),
            StateOwnPropertySnapshot::Owned(s) => {
                let bad = violation(&state.heap, result, s.record());
                s.release_in_state(state, &runtime.0.poisoned)?;
                Some(bad)
            }
            // Integer-indexed descriptors are always writable/configurable. Their
            // value is irrelevant to the Get invariant, so no backing lock is read.
            StateOwnPropertySnapshot::Shared(_) => Some(false),
            StateOwnPropertySnapshot::Proxy => None,
        },
    )
}
pub(super) fn complete(
    state: &mut RuntimeState,
    runtime: &Runtime,
    realm: ContextId,
    result: JsValue,
    bad: bool,
) -> Result<ProxyGetStep, RuntimeError> {
    if !bad {
        return Ok(ProxyGetStep::Complete(Completion::Return(result)));
    }
    state.release_owned_jsvalue(&runtime.0.poisoned, result)?;
    let value = state.new_native_error_from_message(
        &runtime.0.poisoned,
        realm,
        NativeErrorKind::Type,
        NativeErrorMessage::from_utf8("proxy: inconsistent get"),
    )?;
    Ok(ProxyGetStep::Complete(Completion::Throw(JsValue::Object(
        value,
    ))))
}
pub(super) fn release_descriptor(
    state: &mut RuntimeState,
    runtime: &Runtime,
    descriptor: NativeConversion<Option<StateOwnedCompleteDescriptor>>,
) -> Result<(), RuntimeError> {
    match descriptor {
        NativeConversion::Throw(value) => state.release_owned_jsvalue(&runtime.0.poisoned, value),
        NativeConversion::Value(Some(d)) => d.release_in_state(state, &runtime.0.poisoned),
        NativeConversion::Value(None) => Ok(()),
    }
}
