//! The one Bound CALL payload promotion and argument merge algorithm.
//!
//! A selected source callee remains owned by its input record while these
//! immutable payload facts are used. Bound wrappers publish no active frame.
use crate::engine::{
    api::{
        error::{NativeErrorKind, NativeErrorMessage},
        runtime_error::RuntimeError,
    },
    heap::{
        ContextId, ObjectId, RawValue,
        runtime::{
            RuntimeState,
            owned_values::{OwnedValueGuard, OwnedValuesGuard},
        },
    },
    value::{JsValue, conversion::NativeConversion},
    vm::call::ordinary::{RawCallbackGuard, RawCallbackInputs},
};
use std::{cell::Cell, rc::Rc};

/// Immutable facts selected from the input's actual live Bound callee. This
/// shares its argument storage; only `snapshot_in_state` promotes heap edges.
pub(crate) struct BoundSelection {
    target: ObjectId,
    receiver: RawValue,
    arguments: Rc<[RawValue]>,
}
impl BoundSelection {
    pub(crate) fn from_payload(payload: &crate::engine::heap::ObjectPayload) -> Option<Self> {
        let crate::engine::heap::ObjectPayload::BoundFunction {
            target,
            this_value,
            arguments,
        } = payload
        else {
            return None;
        };
        Some(Self::new(*target, this_value.clone(), arguments.clone()))
    }
    pub(super) fn new(target: ObjectId, receiver: RawValue, arguments: Rc<[RawValue]>) -> Self {
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_call_buffer_share(
            "bound.raw_snapshot",
            arguments.len(),
            size_of::<RawValue>(),
        );
        Self {
            target,
            receiver,
            arguments,
        }
    }

    /// Preserve the actual payload promotion prefix: target, buffer reserve,
    /// receiver, then ascending arguments. On a failed argument retain the
    /// receiver retires first, preceding arguments next, and target last.
    pub(in crate::engine::vm) fn snapshot_in_state(
        self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
    ) -> Result<RawCallbackInputs, RuntimeError> {
        if poisoned.get() {
            return Err(RuntimeError::Poisoned);
        }
        let result = (|| {
            state.heap.retain_object(self.target)?;
            let mut target = OwnedValueGuard::new(state, poisoned, JsValue::Object(self.target));
            let (state, target_value) = target.parts();
            let mut arguments = Vec::new();
            arguments
                .try_reserve_exact(self.arguments.len())
                .map_err(|_| {
                    RuntimeError::Invariant("bound argument snapshot allocation failed")
                })?;
            let mut arguments = OwnedValuesGuard::new(state, poisoned, arguments);
            let (state, arguments_value) = arguments.parts();
            let receiver = promote(state, &self.receiver)?;
            let mut receiver = OwnedValueGuard::new(state, poisoned, receiver);
            let (state, receiver_value) = receiver.parts();
            for raw in self.arguments.iter() {
                arguments_value.push(promote(state, raw)?);
            }
            #[cfg(feature = "profiling")]
            {
                crate::engine::api::profiling::record_call_buffer_observed(
                    "bound.rooted_snapshot",
                    arguments_value.capacity(),
                    size_of::<JsValue>(),
                );
                crate::engine::api::profiling::record_call_buffer_js_value_copies(
                    "bound.rooted_snapshot",
                    arguments_value,
                );
            }
            let JsValue::Object(function) = target_value.take().expect("promoted Bound target")
            else {
                unreachable!()
            };
            Ok(RawCallbackInputs::new(
                function,
                receiver_value.take().expect("promoted Bound receiver"),
                std::mem::take(arguments_value),
            ))
        })();
        if poisoned.get() {
            return Err(RuntimeError::Poisoned);
        }
        result
    }
}

fn promote(state: &mut RuntimeState, raw: &RawValue) -> Result<JsValue, RuntimeError> {
    let value = JsValue::from_raw(raw.clone()).ok_or(RuntimeError::Invariant(
        "bound value was an internal sentinel",
    ))?;
    state.dup_jsvalue(&value)
}

impl RawCallbackInputs {
    /// Replace one outer Bound call with its target call. The concrete input
    /// record remains armed throughout destructive retirement and allocation.
    /// Throw is exclusively a freshly published overflow Error, so resident
    /// consumers carry its actual publication through the Query reply fact.
    pub(in crate::engine::vm) fn apply_bound_in_state(
        &mut self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        realm: ContextId,
        selected: BoundSelection,
    ) -> Result<NativeConversion<()>, RuntimeError> {
        let next = selected.snapshot_in_state(state, poisoned)?;
        let mut next = RawCallbackGuard::new(state, poisoned, next);
        let (state, next_inputs) = next.parts();
        self.apply_bound_snapshot_in_state(state, poisoned, realm, next_inputs)
    }

    /// Consume the already checked first payload snapshot before the caller's
    /// input transport. Later Bound links use this same replacement body.
    pub(in crate::engine::vm) fn apply_bound_snapshot_in_state(
        &mut self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        realm: ContextId,
        next_inputs: &mut RawCallbackInputs,
    ) -> Result<NativeConversion<()>, RuntimeError> {
        let receiver = self
            .receiver
            .replace(next_inputs.receiver.take().expect("Bound receiver"))
            .expect("outer Bound receiver");
        state.release_owned_jsvalue(poisoned, receiver)?;
        let arguments = state.concatenate_bound_arguments_jsvalue(
            poisoned,
            realm,
            std::mem::take(&mut next_inputs.arguments),
            std::mem::take(&mut self.arguments),
        )?;
        let NativeConversion::Value(arguments) = arguments else {
            let NativeConversion::Throw(value) = arguments else {
                unreachable!()
            };
            let mut value = OwnedValueGuard::new(state, poisoned, value);
            let (state, value) = value.parts();
            next_inputs.retire(state, poisoned)?;
            return Ok(NativeConversion::Throw(
                value.take().expect("Bound overflow diagnostic"),
            ));
        };
        self.arguments = arguments;
        let old = self
            .selected_callee
            .replace(next_inputs.selected_callee.take().expect("Bound target"))
            .expect("outer Bound callee");
        state.release_owned_jsvalue(poisoned, JsValue::Object(old))?;
        Ok(NativeConversion::Value(()))
    }
}

impl RuntimeState {
    /// Move existing argument owners, with the Bound prefix before caller
    /// arguments. Overflow retires both buffers in that order before allocating
    /// its caller-realm Error. A destructive failure quarantines the suffix.
    pub(crate) fn concatenate_bound_arguments_jsvalue(
        &mut self,
        poisoned: &Cell<bool>,
        realm: ContextId,
        bound_arguments: Vec<JsValue>,
        call_arguments: Vec<JsValue>,
    ) -> Result<NativeConversion<Vec<JsValue>>, RuntimeError> {
        const MAX_CALL_ARGUMENTS: usize = 65_534;
        let mut caller = OwnedValuesGuard::new(self, poisoned, call_arguments);
        let (state, caller_values) = caller.parts();
        let mut bound = OwnedValuesGuard::new(state, poisoned, bound_arguments);
        let (state, bound_values) = bound.parts();
        let total = bound_values.len().checked_add(caller_values.len());
        if total.is_none_or(|total| total > MAX_CALL_ARGUMENTS) {
            for slot in bound_values.iter_mut().chain(caller_values.iter_mut()) {
                let value = std::mem::replace(slot, JsValue::Undefined);
                state.release_owned_jsvalue(poisoned, value)?;
            }
            bound_values.clear();
            caller_values.clear();
            let error = state.new_native_error_from_message(
                poisoned,
                realm,
                NativeErrorKind::Internal,
                NativeErrorMessage::from_utf8("stack overflow"),
            )?;
            return Ok(NativeConversion::Throw(JsValue::Object(error)));
        }
        let total = total.expect("checked Bound argument count");
        let mut arguments = Vec::with_capacity(total);
        arguments.append(bound_values);
        arguments.append(caller_values);
        #[cfg(feature = "profiling")]
        {
            crate::engine::api::profiling::record_call_buffer_capacity(
                "bound.merge",
                0,
                arguments.capacity(),
                size_of::<JsValue>(),
            );
            crate::engine::api::profiling::record_call_buffer_moves("bound.merge", total);
        }
        Ok(NativeConversion::Value(arguments))
    }
}

#[cfg(test)]
mod tests;
