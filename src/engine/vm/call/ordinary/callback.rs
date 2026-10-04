//! Concrete callback owners shared by getter and conversion frame entry.
use super::{DirectSelection, OrdinaryCall};
use crate::engine::{
    api::{
        runtime::{Runtime, RuntimeUnwindGuard},
        runtime_error::RuntimeError,
    },
    heap::{ObjectId, runtime::RuntimeState},
    value::JsValue,
    vm::frames::NativeClassification,
};
use std::cell::Cell;

/// One payload selection, immediately consumed by the corresponding entry.
pub(in crate::engine::vm) enum CallbackSelection {
    Ordinary(OrdinaryCall),
    Native(NativeClassification),
    General,
}

/// The selected callee/receiver/argv, plus callback authentication's existing
/// checked callee edge. Named reads additionally preserve one original base
/// role; other callback producers leave that field empty.
pub(in crate::engine::vm) struct RawCallbackInputs {
    pub(in crate::engine::vm) selected_callee: Option<ObjectId>,
    pub(in crate::engine::vm) callback_callee: Option<ObjectId>,
    pub(in crate::engine::vm) receiver: Option<JsValue>,
    pub(in crate::engine::vm) arguments: Vec<JsValue>,
    pub(in crate::engine::vm) preserved_receiver: Option<JsValue>,
}

impl RawCallbackInputs {
    /// Inputs already own their admitted edges. No checked retain occurs here.
    pub(in crate::engine::vm) fn new(
        function: ObjectId,
        receiver: JsValue,
        arguments: Vec<JsValue>,
    ) -> Self {
        Self {
            selected_callee: Some(function),
            callback_callee: None,
            receiver: Some(receiver),
            arguments,
            preserved_receiver: None,
        }
    }

    /// The one complete Bound CALL loop. The terminal selection is carried
    /// directly into the ordinary/native installer, with no second chain walk.
    pub(in crate::engine::vm) fn normalize_bound_chain_in_state<'r>(
        &mut self,
        runtime: &'r Runtime,
        state: &mut RuntimeState,
        realm: crate::engine::heap::ContextId,
    ) -> Result<crate::engine::value::conversion::NativeConversion<DirectSelection<'r>>, RuntimeError>
    {
        let function = self.selected_callee.expect("selected callback callee");
        let selected = DirectSelection::select_in_state(runtime, state, function)?;
        self.normalize_bound_chain_from_selection_in_state(runtime, state, realm, selected)
    }

    pub(in crate::engine::vm) fn normalize_bound_chain_from_selection_in_state<'r>(
        &mut self,
        runtime: &'r Runtime,
        state: &mut RuntimeState,
        realm: crate::engine::heap::ContextId,
        mut selected: DirectSelection<'r>,
    ) -> Result<crate::engine::value::conversion::NativeConversion<DirectSelection<'r>>, RuntimeError>
    {
        use crate::engine::value::conversion::NativeConversion;
        loop {
            match selected {
                DirectSelection::Bound(bound) => {
                    match self.apply_bound_in_state(state, &runtime.0.poisoned, realm, bound)? {
                        NativeConversion::Value(()) => {}
                        NativeConversion::Throw(value) => {
                            return Ok(NativeConversion::Throw(value));
                        }
                    }
                }
                selected => return Ok(NativeConversion::Value(selected)),
            }
            let function = self.selected_callee.expect("normalized callback callee");
            selected = DirectSelection::select_in_state(runtime, state, function)?;
        }
    }

    /// Share direct callback authentication without a public callee wrapper.
    /// Preserve its independent checked final-callee promotion before overflow.
    pub(in crate::engine::vm) fn select_callback_in_state(
        &mut self,
        runtime: &Runtime,
        state: &mut RuntimeState,
        realm: crate::engine::heap::ContextId,
    ) -> Result<crate::engine::value::conversion::NativeConversion<CallbackSelection>, RuntimeError>
    {
        use crate::engine::value::conversion::NativeConversion;
        match self.normalize_bound_chain_in_state(runtime, state, realm)? {
            NativeConversion::Throw(value) => Ok(NativeConversion::Throw(value)),
            NativeConversion::Value(DirectSelection::Ordinary(selected)) => {
                let call = selected.authenticate_slot_in_state(runtime, state)?;
                let function = self.selected_callee.expect("normalized callback callee");
                state.heap.retain_object(function)?;
                self.callback_callee = Some(function);
                Ok(NativeConversion::Value(CallbackSelection::Ordinary(call)))
            }
            NativeConversion::Value(DirectSelection::Native(selected)) => {
                Ok(NativeConversion::Value(CallbackSelection::Native(
                    NativeClassification::classify_selected(selected),
                )))
            }
            NativeConversion::Value(DirectSelection::General) => {
                Ok(NativeConversion::Value(CallbackSelection::General))
            }
            NativeConversion::Value(DirectSelection::Bound(_)) => {
                unreachable!("normalized Bound chain")
            }
        }
    }

    pub(in crate::engine::vm) fn retire_selection(
        &mut self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
    ) -> Result<(), RuntimeError> {
        if let Some(function) = self.selected_callee.take() {
            state.release_owned_jsvalue(poisoned, JsValue::Object(function))?;
        }
        Ok(())
    }

    /// Normal retirement propagates the first cleanup failure; the remaining
    /// callback edges stay untouched after destructive poison.
    pub(in crate::engine::vm) fn retire(
        &mut self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
    ) -> Result<(), RuntimeError> {
        let _unwind = RuntimeUnwindGuard::from_flag(poisoned);
        self.retire_with(|value| state.release_owned_jsvalue(poisoned, value))
    }

    /// External Query teardown can encounter an existing State borrow. Only
    /// that real boundary coordinates releases; resident rollback uses State.
    pub(in crate::engine::vm) fn retire_at_boundary(
        &mut self,
        runtime: &Runtime,
    ) -> Result<(), RuntimeError> {
        if runtime.skip_cleanup() {
            return Err(RuntimeError::Poisoned);
        }
        let _unwind = runtime.unwind_guard();
        if let Ok(mut state) = runtime.0.state.try_borrow_mut() {
            self.retire(&mut state, &runtime.0.poisoned)
        } else {
            self.retire_with(|value| {
                runtime.release_jsvalue(value)?;
                // Runtime release coordinates ownership and records a fatal
                // cleanup error in its header. Observe it before the suffix.
                runtime.check_poison()
            })
        }
    }

    fn retire_with(
        &mut self,
        mut release: impl FnMut(JsValue) -> Result<(), RuntimeError>,
    ) -> Result<(), RuntimeError> {
        if let Some(function) = self.selected_callee.take() {
            release(JsValue::Object(function))?;
        }
        if let Some(function) = self.callback_callee.take() {
            release(JsValue::Object(function))?;
        }
        if let Some(receiver) = self.receiver.take() {
            release(receiver)?;
        }
        if let Some(receiver) = self.preserved_receiver.take() {
            release(receiver)?;
        }
        for slot in &mut self.arguments {
            let value = std::mem::replace(slot, JsValue::Undefined);
            release(value)?;
        }
        self.arguments.clear();
        Ok(())
    }
}

/// Armed only while the existing State lease protects this finite callback.
/// Taking the inputs transfers them to a frame or actual pending boundary.
pub(in crate::engine::vm) struct RawCallbackGuard<'a> {
    state: &'a mut RuntimeState,
    poisoned: &'a Cell<bool>,
    inputs: Option<RawCallbackInputs>,
}
impl<'a> RawCallbackGuard<'a> {
    pub(in crate::engine::vm) fn new(
        state: &'a mut RuntimeState,
        poisoned: &'a Cell<bool>,
        inputs: RawCallbackInputs,
    ) -> Self {
        Self {
            state,
            poisoned,
            inputs: Some(inputs),
        }
    }
    pub(in crate::engine::vm) fn parts(&mut self) -> (&mut RuntimeState, &mut RawCallbackInputs) {
        (
            self.state,
            self.inputs.as_mut().expect("guarded callback inputs"),
        )
    }
    pub(in crate::engine::vm) fn take(&mut self) -> RawCallbackInputs {
        self.inputs.take().expect("guarded callback inputs")
    }
    pub(in crate::engine::vm) fn retire(&mut self) -> Result<(), RuntimeError> {
        if let Some(inputs) = &mut self.inputs {
            inputs.retire(self.state, self.poisoned)?;
        }
        self.inputs = None;
        Ok(())
    }
}
impl Drop for RawCallbackGuard<'_> {
    fn drop(&mut self) {
        if self.inputs.is_none() {
            return;
        }
        if std::thread::panicking() {
            self.poisoned.set(true);
        }
        if self.poisoned.get() {
            return;
        }
        if self.retire().is_err() {
            self.poisoned.set(true);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{api::Value, heap::RawId};

    #[test]
    fn callback_selection_preserves_the_checked_ordinary_callee_role() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        let Value::Object(function) = context.eval("(function callback(){return 7})").unwrap()
        else {
            panic!("ordinary callback")
        };
        let function = function.into_handle();
        let mut state = runtime.0.state.borrow_mut();
        let before = state.heap.object_strong_count(function).unwrap();
        let mut owner = RawCallbackGuard::new(
            &mut state,
            &runtime.0.poisoned,
            RawCallbackInputs::new(function, JsValue::Int(42), Vec::new()),
        );
        {
            let (state, inputs) = owner.parts();
            let crate::engine::value::conversion::NativeConversion::Value(
                CallbackSelection::Ordinary(call),
            ) = inputs
                .select_callback_in_state(&runtime, state, context.realm_id())
                .unwrap()
            else {
                panic!("ordinary selection")
            };
            assert_eq!(state.heap.object_strong_count(function), Ok(before + 1));
            assert_eq!(inputs.callback_callee, Some(function));
            inputs.retire_selection(state, &runtime.0.poisoned).unwrap();
            assert_eq!(state.heap.object_strong_count(function), Ok(before));
            drop(call);
        }
        owner.retire().unwrap();
        drop(owner);
        assert!(state.heap.object(function).is_err());
    }

    #[test]
    fn ordinary_callback_authentication_overflow_keeps_inputs_until_explicit_retirement() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        let Value::Object(function) = context.eval("(function callback(){return 7})").unwrap()
        else {
            panic!("ordinary callback")
        };
        let function = function.into_handle();
        let receiver = runtime.new_object(None).unwrap().into_handle();
        let mut state = runtime.0.state.borrow_mut();
        let before = state.heap.object_strong_count(function).unwrap();
        state
            .heap
            .set_strong_count_for_test(RawId::Object(function), u32::MAX);
        let mut owner = RawCallbackGuard::new(
            &mut state,
            &runtime.0.poisoned,
            RawCallbackInputs::new(function, JsValue::Object(receiver), Vec::new()),
        );
        let (state, inputs) = owner.parts();
        assert!(
            inputs
                .select_callback_in_state(&runtime, state, context.realm_id())
                .is_err()
        );
        assert_eq!(inputs.selected_callee, Some(function));
        assert!(inputs.callback_callee.is_none());
        assert_eq!(inputs.receiver, Some(JsValue::Object(receiver)));
        assert_eq!(state.heap.object_strong_count(receiver), Ok(1));
        assert!(!runtime.is_poisoned());
        state
            .heap
            .set_strong_count_for_test(RawId::Object(function), before);
        owner.retire().unwrap();
    }

    #[test]
    fn native_callback_keeps_its_one_selected_owner_without_ordinary_authentication() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        let Value::Object(function) = context.eval("Math.clz32").unwrap() else {
            panic!("native callback")
        };
        let function = function.into_handle();
        let mut state = runtime.0.state.borrow_mut();
        let before = state.heap.object_strong_count(function).unwrap();
        let mut owner = RawCallbackGuard::new(
            &mut state,
            &runtime.0.poisoned,
            RawCallbackInputs::new(function, JsValue::Undefined, Vec::new()),
        );
        let (state, inputs) = owner.parts();
        let crate::engine::value::conversion::NativeConversion::Value(CallbackSelection::Native(
            selected,
        )) = inputs
            .select_callback_in_state(&runtime, state, context.realm_id())
            .unwrap()
        else {
            panic!("native selection")
        };
        assert_eq!(
            selected.target(),
            crate::engine::builtins::native::NativeFunctionId::MathClz32
        );
        assert_eq!(state.heap.object_strong_count(function), Ok(before));
        assert!(inputs.callback_callee.is_none());
        assert_eq!(inputs.selected_callee, Some(function));
        owner.retire().unwrap();
    }

    #[test]
    fn callback_retirement_preserves_argument_suffix_after_first_destructive_failure() {
        let runtime = Runtime::new();
        let invalid = runtime.new_object(None).unwrap().into_handle();
        let first = runtime.new_object(None).unwrap().into_handle();
        let suffix = runtime.new_object(None).unwrap().into_handle();
        let mut state = runtime.0.state.borrow_mut();
        state
            .heap
            .queue_release_for_test(RawId::Object(invalid))
            .unwrap();
        state
            .heap
            .set_strong_count_for_test(RawId::Object(invalid), 1);
        let mut inputs = RawCallbackInputs {
            selected_callee: None,
            callback_callee: None,
            receiver: None,
            preserved_receiver: None,
            arguments: vec![JsValue::Object(first), JsValue::Object(suffix)],
        };
        assert!(inputs.retire(&mut state, &runtime.0.poisoned).is_err());
        assert!(runtime.is_poisoned());
        assert_eq!(
            inputs.arguments,
            [JsValue::Undefined, JsValue::Object(suffix)]
        );
        assert_eq!(state.heap.object_strong_count(suffix), Ok(1));
    }
}
