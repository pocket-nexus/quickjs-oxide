//! Base constructor inputs move from the caller into the shared ordinary window.
use super::call::InstalledOrdinaryFrame;
use super::*;

impl SlotStore {
    /// The caller has selected a Base bytecode target and an own data prototype
    /// through this state access. Its receiver guard owns the second receiver
    /// edge; this transaction consumes that edge only after every checked copy
    /// and allocation has succeeded. The separate return receiver stays guarded
    /// until the complete child frame is installed.
    #[cfg(test)]
    #[allow(clippy::too_many_arguments)]
    pub(in crate::engine::vm) fn push_constructor_frame_in_state(
        &mut self,
        runtime: &Runtime,
        state: &mut crate::engine::heap::runtime::RuntimeState,
        layout: &FrameLayout<'_>,
        parent: &mut FrameWindow,
        count: usize,
        function: crate::engine::heap::ObjectId,
        observes_arguments: bool,
        receiver: &mut Option<JsValue>,
    ) -> Result<InstalledOrdinaryFrame, Error> {
        self.check_current(parent)?;
        let consumed = count
            .checked_add(2)
            .filter(|count| *count <= parent.depth)
            .ok_or_else(|| Error::internal("constructor exceeds caller operands"))?;
        for offset in 0..consumed {
            self.peek_current(parent, offset)?;
        }
        if !matches!(self.peek_current(parent, count + 1)?, JsValue::Object(id) if *id == function)
            || !matches!(self.peek_current(parent, count)?, JsValue::Object(_))
            || !matches!(receiver, Some(JsValue::Object(_)))
        {
            return Err(Error::internal(
                "constructor operands changed after selection",
            ));
        }
        self.push_current_constructor_frame_in_state(
            runtime,
            state,
            layout,
            parent,
            count,
            function,
            observes_arguments,
            receiver,
        )
    }

    /// The private frame transaction supplies unchanged preflighted inputs.
    /// The shared initializer completes all fallible work before their edges
    /// move into the newly published window.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn push_current_constructor_frame_in_state(
        &mut self,
        runtime: &Runtime,
        state: &mut crate::engine::heap::runtime::RuntimeState,
        layout: &FrameLayout<'_>,
        parent: &mut FrameWindow,
        count: usize,
        function: crate::engine::heap::ObjectId,
        observes_arguments: bool,
        receiver: &mut Option<JsValue>,
    ) -> Result<InstalledOrdinaryFrame, Error> {
        let consumed = count
            .checked_add(2)
            .filter(|count| *count <= parent.depth)
            .ok_or_else(|| Error::internal("constructor exceeds caller operands"))?;
        let prepared = self.prepare_ordinary_window_in_state(
            runtime,
            state,
            layout,
            parent,
            count,
            function,
            observes_arguments,
        )?;
        let start = prepared.start;
        let base = prepared.window.base;
        for index in 0..count {
            self.slots[base + index] = self.slots[start + index].take();
        }
        let Some(FrameBinding::Direct(JsValue::Object(function))) = self.slots[start - 2].take()
        else {
            unreachable!("selected constructor slot owns its function")
        };
        let Some(FrameBinding::Direct(new_target)) = self.slots[start - 1].take() else {
            unreachable!("selected newTarget slot owns its value")
        };
        let receiver = receiver
            .take()
            .expect("constructor receiver guard owns its edge");
        let input = crate::engine::vm::CallInput::new(runtime, receiver, new_target, None);
        let window = self.publish_ordinary_window(parent, consumed, prepared);
        Ok(InstalledOrdinaryFrame {
            window,
            function,
            input,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{
        code::{
            function::metadata::{ClosureVariableKind, VariableDefinition},
            runtime::PublishedFunctionSnapshot,
        },
        heap::RawId,
    };

    #[test]
    fn named_local_failure_preserves_aliased_constructor_inputs_and_receiver_guard() {
        for observes_arguments in [false, true] {
            let runtime = Runtime::new();
            let context = runtime.new_context().expect("create context");
            let callee = runtime.new_object(None).unwrap();
            let callee_id = callee.object_id();
            let argument = runtime.new_object(None).unwrap();
            let argument_id = argument.object_id();
            let receiver = runtime.new_object(None).unwrap();
            let receiver_id = receiver.object_id();
            let mut receiver = Some(JsValue::Object(receiver.into_handle()));
            let mut caller = PublishedFunctionSnapshot::empty_for_test(context.realm);
            caller.metadata.max_stack = 3;
            let mut child = PublishedFunctionSnapshot::empty_for_test(context.realm);
            child.metadata.argument_count = 1;
            child.metadata.local_count = 2;
            child.metadata.function_name_local = Some(1);
            let definition = VariableDefinition {
                name: None,
                is_lexical: false,
                is_const: false,
                is_parameter_initializer: false,
                kind: ClosureVariableKind::Normal,
            };
            child.local_definitions = std::rc::Rc::from([definition; 2]);
            let mut slots = SlotStore::new(32);
            let mut parent = slots
                .push_frame(
                    &runtime,
                    &caller.frame_layout(),
                    FrameStorage {
                        original_arguments: Vec::new(),
                        parameters: Vec::new(),
                        locals: Vec::new(),
                        operands: vec![
                            JsValue::Object(callee.try_clone().unwrap().into_handle()),
                            JsValue::Object(callee.try_clone().unwrap().into_handle()),
                            JsValue::Object(argument.into_handle()),
                        ],
                    },
                )
                .unwrap();
            let end = slots.active_end;
            let mut state = runtime.0.state.borrow_mut();
            let owners = state.heap.object_strong_count(callee_id).unwrap();
            state
                .heap
                .set_strong_count_for_test(RawId::Object(callee_id), u32::MAX);
            let failed = slots.push_constructor_frame_in_state(
                &runtime,
                &mut state,
                &child.frame_layout(),
                &mut parent,
                1,
                callee_id,
                observes_arguments,
                &mut receiver,
            );
            state
                .heap
                .set_strong_count_for_test(RawId::Object(callee_id), owners);
            assert!(
                failed
                    .err()
                    .unwrap()
                    .message()
                    .contains("retaining a heap reference")
            );
            assert!(!runtime.is_poisoned());
            assert_eq!(slots.active_end, end);
            assert_eq!(slots.depth(&parent), 3);
            assert!(slots.slots[end..].iter().all(Option::is_none));
            assert!(matches!(slots.peek(&parent, 2), Ok(JsValue::Object(id)) if *id == callee_id));
            assert!(matches!(slots.peek(&parent, 1), Ok(JsValue::Object(id)) if *id == callee_id));
            assert!(
                matches!(slots.peek(&parent, 0), Ok(JsValue::Object(id)) if *id == argument_id)
            );
            assert_eq!(state.heap.object_strong_count(argument_id), Ok(1));
            assert_eq!(state.heap.object_strong_count(receiver_id), Ok(1));
            state.release_jsvalue(receiver.take().unwrap()).unwrap();
            slots
                .clear_frame_owned_in_state(&mut state, &runtime.0.poisoned, parent)
                .unwrap();
            assert!(state.heap.object(argument_id).is_err());
            assert!(state.heap.object(receiver_id).is_err());
            assert_eq!(state.heap.object_strong_count(callee_id), Ok(1));
        }
    }
}
