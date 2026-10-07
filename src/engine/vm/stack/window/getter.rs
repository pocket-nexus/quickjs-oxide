//! A selected getter transfers its existing edges to the ordinary installer.
//! The caller slot is not padded with a synthetic callee or argument buffer.
use super::{Error, FrameBinding, FrameExecution, JsValue, Runtime};
use crate::engine::{
    heap::runtime::RuntimeState,
    vm::{
        call::ordinary::DirectSelection,
        execute::FallthroughPc,
        frame::{Frame, ReturnOwner, ReturnTarget, ReturnValue},
        property_driver::SelectedNamedRead,
    },
};

impl FrameExecution<'_> {
    /// A real getter call may continue in the current exclusive segment. Native,
    /// bound and Proxy callees retain their explicit effect boundary for now.
    /// Failure before publication leaves the selection in execution storage.
    #[cold]
    #[inline(never)]
    pub(in crate::engine::vm) fn enter_selected_getter(
        &mut self,
        runtime: &Runtime,
        state: &mut RuntimeState,
        keep_receiver: bool,
        fallthrough: FallthroughPc,
    ) -> Result<bool, Error> {
        let Some(SelectedNamedRead::Getter(selected)) = self.execution.selected_named_read.as_ref()
        else {
            return Ok(false);
        };
        if !self.execution.frames.can_push() || runtime.bytecode_call_would_overflow() {
            return Ok(false);
        }
        let DirectSelection::Ordinary(selected_call) =
            DirectSelection::select_in_state(runtime, state, selected.getter_id())
                .map_err(super::super::runtime_error_to_vm_error)?
        else {
            return Ok(false);
        };
        let call = selected_call
            .authenticate_slot_in_state(runtime, state)
            .map_err(super::super::runtime_error_to_vm_error)?;
        let (function, executable, closure) = call.into_slot_parts();
        let execution = &mut *self.execution;
        execution
            .call_storage
            .reserve_depth(execution.frames.depth() + 1)?;
        let (flags, flag_bytes) = if executable.has_captured_locals {
            execution
                .call_storage
                .capture_flags(executable.local_definitions.len())?
        } else {
            (Vec::new(), 0)
        };
        let mut publication = execution.frames.prepare_push()?;
        let (parent, frame) = publication
            .current_frame_mut()
            .expect("getter publication retains its caller");
        let caller_realm = frame.executable.realm;
        // Check the source before preparing any owned child local. Nothing can
        // mutate this window or invoke JS before we transfer its selection.
        execution.slots.peek_current(&frame.window, 0)?;
        let window = execution.slots.prepare_ordinary_window_in_state(
            runtime,
            state,
            &executable.frame_layout(),
            &frame.window,
            0,
            function,
            executable.observes_arguments,
        )?;
        // The shared initializer has finished all allocations and retains.
        // Every subsequent producer operation is infallible until the child
        // owns the getter, receiver and initialized window.
        let retired_receiver = if keep_receiver {
            None
        } else {
            let Some(FrameBinding::Direct(value)) = execution.slots.slots[window.start - 1].take()
            else {
                unreachable!("checked getter input is a direct operand")
            };
            Some(value)
        };
        let Some(SelectedNamedRead::Getter(selected)) = execution.selected_named_read.take() else {
            unreachable!("exclusive getter publication preserves its selection")
        };
        let (function, receiver) = selected.into_parts();
        let window = execution.slots.publish_ordinary_window(
            &mut frame.cold.window,
            usize::from(!keep_receiver),
            window,
        );
        frame.resume_pc = fallthrough.index();
        let (mut cold, frame_bytes) = execution.call_storage.vacant(caller_realm);
        cold.return_to = Some(ReturnTarget {
            value_use: ReturnValue::Push,
            owner: ReturnOwner::Frame(parent),
            tail: false,
            operation: None,
        });
        cold.entry_guard = None;
        cold.function =
            crate::engine::vm::closure::FrameFunction::shared(runtime, function, closure).into();
        cold.input =
            crate::engine::vm::CallInput::new(runtime, receiver, JsValue::Undefined, None).into();
        cold.reusable_captured_locals = flags;
        cold.executable = executable.into();
        cold.window = window.into();
        publication.install(Frame {
            property_generation: 0,
            iterator_generation: 0,
            caller_realm,
            active_frame: crate::engine::vm::frames::ActiveFrameToken::unmaterialized(),
            fault_pc: 0,
            resume_pc: 0,
            cold,
        });
        // This edge can retire only after the child has published its receiver.
        // Destructive cleanup errors poison State; no caller protocol is replayed.
        if let Some(receiver) = retired_receiver {
            state
                .release_owned_jsvalue(&runtime.0.poisoned, receiver)
                .map_err(super::super::runtime_error_to_vm_error)?;
        }
        #[cfg(feature = "profiling")]
        {
            crate::engine::api::profiling::record_owned_call_storage(frame_bytes, flag_bytes, 0);
            crate::engine::api::profiling::record_owned_execution_event(
                "named_read.getter_entered_in_state",
            );
        }
        #[cfg(not(feature = "profiling"))]
        let _ = (frame_bytes, flag_bytes);
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{
        api::Value,
        code::exec_opcode::Opcode,
        vm::{execute::execute_frame, property_driver::read_completion_tests::read_fixture},
    };

    #[test]
    fn named_getter_install_failure_leaves_inputs_and_effect_owned() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        let Value::Object(object) = context
            .eval("globalThis.getterCalls=0; ({get x(){getterCalls++;return 7}})")
            .unwrap()
        else {
            panic!("object")
        };
        let object_id = object.object_id();
        let (mut execution, id) = read_fixture(
            &runtime,
            &mut context,
            "(function(o){return o.x})",
            Opcode::GetFieldCached,
        );
        let frame = execution.frames.current_mut(id).unwrap();
        execution
            .slots
            .push(&mut frame.window, JsValue::Object(object.into_handle()))
            .unwrap();
        let pc = frame.resume_pc;
        let depth = frame.window.depth;
        let end = execution.slots.active_end;
        execution.slots.limit = end;
        let error = execute_frame(&runtime, &mut execution, id).unwrap_err();
        assert_eq!(error.message(), "execution slot limit exceeded");
        let frame = execution.frames.current_mut(id).unwrap();
        assert_eq!(frame.resume_pc, pc);
        assert_eq!(frame.window.depth, depth);
        assert_eq!(execution.slots.active_end, end);
        assert!(matches!(
            execution.slots.peek(&frame.window, 0),
            Ok(JsValue::Object(id)) if *id == object_id
        ));
        assert!(matches!(
            execution.selected_named_read,
            Some(SelectedNamedRead::Getter(_))
        ));
        drop(execution);
        assert_eq!(context.eval("getterCalls").unwrap(), Value::Int(0));
        assert!(!runtime.is_poisoned());
        assert!(runtime.0.state.borrow().heap.object(object_id).is_err());
        assert!(!runtime.0.deferred_references.has_pending());
    }

    #[test]
    fn named_getter_preserves_receiver_and_last_owner_through_return_and_throw() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        for source in [
            "(function(){let o={v:7,get x(){return function(){return this.v}}}; return o.x()===7})()",
            "(function(){let child={v:7};let o={child,get x(){return this.child}};let r=o.x;o=null;child=null;return r.v===7})()",
            "(function(){let n=0;let o={get x(){n++;throw 7}};try{o.x}catch(e){return n===1&&e===7}return false})()",
            "(function(){let o={v:7,get x(){return this.v}};let p=Object.create(o);p.v=9;return p.x===9})()",
        ] {
            assert_eq!(context.eval(source).unwrap(), Value::Bool(true), "{source}");
            assert!(!runtime.is_poisoned());
            assert!(runtime.0.state.borrow().active_frames.is_empty());
        }
    }
}
