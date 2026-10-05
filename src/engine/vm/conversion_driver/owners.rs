//! Unpublished conversion owners borrow cleanup context outside State access.
use super::{ConversionTask, ConversionWait, Runtime};

pub(super) struct TaskScope<'a> {
    runtime: &'a Runtime,
    task: Option<ConversionTask>,
}
impl<'a> TaskScope<'a> {
    pub(super) fn new(runtime: &'a Runtime, task: ConversionTask) -> Self {
        Self {
            runtime,
            task: Some(task),
        }
    }
    pub(super) fn take(&mut self) -> ConversionTask {
        self.task.take().expect("conversion task owner")
    }
    pub(super) fn with_step(&mut self, step: super::PrimitiveStep) -> ConversionTask {
        self.take().with_step(step)
    }
}
impl std::ops::Deref for TaskScope<'_> {
    type Target = ConversionTask;
    fn deref(&self) -> &Self::Target {
        self.task.as_ref().expect("conversion task owner")
    }
}
impl std::ops::DerefMut for TaskScope<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.task.as_mut().expect("conversion task owner")
    }
}
impl Drop for TaskScope<'_> {
    fn drop(&mut self) {
        if let Some(task) = self.task.take() {
            task.release_owned(self.runtime);
        }
    }
}

pub(in crate::engine::vm) struct ConversionWaitScope<'a> {
    runtime: &'a Runtime,
    wait: Option<ConversionWait>,
}
impl<'a> ConversionWaitScope<'a> {
    pub(in crate::engine::vm) fn new(runtime: &'a Runtime, wait: ConversionWait) -> Self {
        Self {
            runtime,
            wait: Some(wait),
        }
    }
    pub(in crate::engine::vm) fn take(&mut self) -> ConversionWait {
        self.wait.take().expect("conversion wait owner")
    }
}
impl std::ops::Deref for ConversionWaitScope<'_> {
    type Target = ConversionWait;
    fn deref(&self) -> &Self::Target {
        self.wait.as_ref().expect("conversion wait owner")
    }
}
impl Drop for ConversionWaitScope<'_> {
    fn drop(&mut self) {
        if let Some(wait) = self.wait.take() {
            wait.release_owned(self.runtime);
        }
    }
}

pub(in crate::engine::vm) struct ConversionSlotScope<'a> {
    runtime: &'a Runtime,
    task: Option<ConversionTask>,
}
impl<'a> ConversionSlotScope<'a> {
    pub(in crate::engine::vm) fn new(runtime: &'a Runtime, task: Option<ConversionTask>) -> Self {
        Self { runtime, task }
    }
}
impl std::ops::Deref for ConversionSlotScope<'_> {
    type Target = Option<ConversionTask>;
    fn deref(&self) -> &Self::Target {
        &self.task
    }
}
impl std::ops::DerefMut for ConversionSlotScope<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.task
    }
}
impl Drop for ConversionSlotScope<'_> {
    fn drop(&mut self) {
        if let Some(task) = self.task.take() {
            task.release_owned(self.runtime);
        }
    }
}

/// The original property inputs remain local until ToPrimitive selects an
/// actual callback. This guard borrows cleanup context; it never owns Runtime
/// and never allocates the durable conversion record for a synchronous throw.
pub(super) struct KeyInputScope<'a> {
    runtime: &'a Runtime,
    base: Option<crate::engine::value::JsValue>,
    value: Option<crate::engine::value::JsValue>,
}
impl<'a> KeyInputScope<'a> {
    pub(super) fn new(
        runtime: &'a Runtime,
        base: crate::engine::value::JsValue,
        value: Option<crate::engine::value::JsValue>,
    ) -> Self {
        Self {
            runtime,
            base: Some(base),
            value,
        }
    }
    pub(super) fn take_base(&mut self) -> crate::engine::value::JsValue {
        self.base.take().expect("property conversion base owner")
    }
    pub(super) fn release_in_state(
        &mut self,
        state: &mut crate::engine::heap::runtime::RuntimeState,
    ) -> Result<(), crate::engine::api::RuntimeError> {
        if let Some(base) = self.base.take() {
            state.release_owned_jsvalue(&self.runtime.0.poisoned, base)?;
        }
        if let Some(value) = self.value.take() {
            state.release_owned_jsvalue(&self.runtime.0.poisoned, value)?;
        }
        Ok(())
    }
}
impl Drop for KeyInputScope<'_> {
    fn drop(&mut self) {
        if self.runtime.skip_cleanup() || (self.base.is_none() && self.value.is_none()) {
            return;
        }
        let _unwind = self.runtime.unwind_guard();
        if self
            .release_in_state(&mut self.runtime.0.state.borrow_mut())
            .is_err()
        {
            self.runtime.0.poisoned.set(true);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{
        api::Value,
        value::{
            JsValue,
            conversion::primitive::{PrimitiveResume, PrimitiveStep},
        },
        vm::{Completion, ToPrimitiveHint},
    };

    #[test]
    fn rejected_task_cleans_finish_and_unpublished_primitive_effect() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        let Value::Object(key) = context.eval("({valueOf(){return 7}})").unwrap() else {
            panic!("key")
        };
        let key_id = key.object_id();
        let base = runtime.new_object(None).unwrap();
        let base_id = base.object_id();
        let (execution, frame) =
            super::super::super::property_driver::read_completion_tests::read_fixture(
                &runtime,
                &mut context,
                "(function(o,k){return o[k]})",
                crate::engine::code::exec_opcode::Opcode::GetArrayElDense,
            );
        let step = PrimitiveResume::start(
            &runtime,
            context.realm,
            JsValue::Object(key.into_handle()),
            ToPrimitiveHint::Number,
        )
        .unwrap();
        let task = ConversionTask::new(
            &runtime,
            super::super::Finish::AddLeft(JsValue::Object(base.into_handle())),
            frame,
            1,
            step,
        );
        drop(TaskScope::new(&runtime, task));
        let state = runtime.0.state.borrow();
        assert!(state.heap.object(key_id).is_err());
        assert!(state.heap.object(base_id).is_err());
        assert!(!runtime.0.deferred_references.has_pending());
        drop(state);
        drop(execution);
    }

    #[cfg(feature = "profiling")]
    #[test]
    fn synchronous_object_key_rejection_has_no_conversion_task() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        let Value::Object(key) = context
            .eval("({[Symbol.toPrimitive]:null,valueOf:null,toString:null})")
            .unwrap()
        else {
            panic!("key")
        };
        let key_id = key.object_id();
        let base = runtime.new_object(None).unwrap();
        let base_id = base.object_id();
        let (mut execution, frame) =
            super::super::super::property_driver::read_completion_tests::read_fixture(
                &runtime,
                &mut context,
                "(function(o,k){return o[k]})",
                crate::engine::code::exec_opcode::Opcode::GetArrayElDense,
            );
        let parent = execution.frames.current_mut(frame).unwrap();
        let fallthrough = crate::engine::vm::execute::FallthroughPc::from_decoded(
            parent
                .executable
                .exec
                .decode_published(parent.resume_pc as u32)
                .unwrap(),
        );
        execution
            .slots
            .push(&mut parent.window, JsValue::Object(base.into_handle()))
            .unwrap();
        execution
            .slots
            .push(&mut parent.window, JsValue::Object(key.into_handle()))
            .unwrap();
        let profile = crate::engine::api::profiling::CostProfile::start();
        let _core = crate::engine::api::profiling::CoreExecutionScope::enter();
        let super::super::Progress::Complete(Completion::Throw(error)) =
            ConversionTask::start_property_read(
                &runtime,
                &mut execution,
                frame,
                1,
                false,
                false,
                fallthrough,
            )
            .unwrap()
        else {
            panic!("terminal key rejection")
        };
        let events = profile.snapshot().owned_execution_events;
        for event in [
            "conversion_task_allocated",
            "primitive.real_effect_resume_created",
            "query.read.acquired",
        ] {
            assert_eq!(events.get(event).copied().unwrap_or(0), 0, "{event}");
        }
        let state = runtime.0.state.borrow();
        assert!(state.heap.object(base_id).is_err());
        assert!(state.heap.object(key_id).is_err());
        assert!(!runtime.0.deferred_references.has_pending());
        drop(state);
        runtime.release_jsvalue(error).unwrap();
    }

    #[test]
    fn rejected_key_input_scope_retires_both_inputs_without_runtime_owner() {
        let runtime = Runtime::new();
        let base = runtime.new_object(None).unwrap().into_handle();
        let value = runtime.new_object(None).unwrap().into_handle();
        let owners = std::rc::Rc::strong_count(&runtime.0);
        let scope = KeyInputScope::new(
            &runtime,
            JsValue::Object(base),
            Some(JsValue::Object(value)),
        );
        assert_eq!(std::rc::Rc::strong_count(&runtime.0), owners);
        drop(scope);
        let state = runtime.0.state.borrow();
        assert!(state.heap.object(base).is_err());
        assert!(state.heap.object(value).is_err());
        assert!(!runtime.0.deferred_references.has_pending());
    }

    #[test]
    fn slot_scope_cleans_a_terminal_value_before_driver_unwind() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        let (execution, frame) =
            super::super::super::property_driver::read_completion_tests::read_fixture(
                &runtime,
                &mut context,
                "(function(o,k){return o[k]})",
                crate::engine::code::exec_opcode::Opcode::GetArrayElDense,
            );
        let result = runtime.new_object(None).unwrap();
        let id = result.object_id();
        let task = ConversionTask::new(
            &runtime,
            super::super::Finish::Plus,
            frame,
            1,
            PrimitiveStep::Complete(Completion::Throw(JsValue::Object(result.into_handle()))),
        );
        drop(ConversionSlotScope::new(&runtime, Some(task)));
        assert!(runtime.0.state.borrow().heap.object(id).is_err());
        assert!(!runtime.0.deferred_references.has_pending());
        drop(execution);
    }
}
