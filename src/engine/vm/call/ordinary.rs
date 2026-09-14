//! Authenticated direct ordinary calls. No wrapper dispatch or materialized argv.
use crate::engine::{
    api::{Error, runtime::Runtime, runtime_error::RuntimeError},
    code::{
        function::metadata::FunctionKind, rooted::FunctionBytecodeRef,
        runtime::PublishedFunctionSnapshot,
    },
    heap::ObjectPayload,
    object::ObjectRef,
    value::Value,
    vm::{closure::ClosureSlots, frames::ActiveFrameGuard},
};

// Only this module can authenticate or construct this witness.
pub(in crate::engine::vm) struct OrdinaryCall {
    function: ObjectRef,
    executable: PublishedFunctionSnapshot,
    closure: ClosureSlots,
}
impl OrdinaryCall {
    pub(in crate::engine::vm) fn authenticate(
        runtime: &Runtime,
        value: &Value,
    ) -> Result<Option<Self>, RuntimeError> {
        let Value::Object(function) = value else {
            return Ok(None);
        };
        if !function.belongs_to(runtime) {
            return Ok(None);
        }
        let (bytecode, closure) = {
            let state = runtime.0.state.borrow();
            let object = state.heap.object(function.object_id())?;
            let ObjectPayload::BytecodeFunction {
                bytecode,
                closure_slots,
                ..
            } = &object.payload
            else {
                return Ok(None);
            };
            let data = state.heap.function_bytecode(*bytecode)?;
            if data.metadata.function_kind != FunctionKind::Normal {
                return Ok(None);
            }
            if closure_slots.len() != usize::from(data.metadata.closure_count) {
                return Err(RuntimeError::Invariant(
                    "function object closure slot count does not match bytecode metadata",
                ));
            }
            (*bytecode, closure_slots.clone())
        };
        let bytecode = FunctionBytecodeRef::from_borrowed_handle(runtime.clone(), bytecode)?;
        let executable = runtime.snapshot_function_bytecode_owned(bytecode)?;
        Ok(Some(Self {
            function: function.clone(),
            executable,
            closure: ClosureSlots::shared(function.clone(), closure),
        }))
    }
    pub(in crate::engine::vm) fn function(&self) -> &ObjectRef {
        &self.function
    }
    pub(in crate::engine::vm) fn executable(&self) -> &PublishedFunctionSnapshot {
        &self.executable
    }
    pub(in crate::engine::vm) fn register(
        &self,
        runtime: &Runtime,
    ) -> Result<ActiveFrameGuard, RuntimeError> {
        runtime.push_ordinary_active_frame(self)
    }
    pub(in crate::engine::vm) fn install(
        self,
        runtime: &Runtime,
        execution: &mut crate::engine::vm::execution::RunningExecution,
        parent: crate::engine::vm::frame::FrameId,
        count: usize,
        method: bool,
        tail: bool,
    ) -> Result<(), Error> {
        use crate::engine::vm::{
            exception::runtime_error_to_vm_error,
            frame::{Frame, ReturnOwner, ReturnTarget, ReturnValue},
        };
        let depth = execution.frames.depth() + 1;
        execution.call_storage.reserve_depth(depth)?;
        let frame = execution.frames.current_mut(parent)?;
        let caller_realm = frame.executable.realm;
        let resume = frame
            .fault_pc
            .checked_add(1)
            .ok_or_else(|| Error::internal("call resume PC overflow"))?;
        let receiver = if method {
            crate::engine::vm::stack::copy_value(execution.slots.peek(&frame.window, count + 1)?)?
        } else {
            Value::Undefined
        };
        let callee_global = runtime
            .global_object_for_realm(self.executable.realm)
            .map_err(runtime_error_to_vm_error)?;
        let guard = self.register(runtime).map_err(runtime_error_to_vm_error)?;
        let (flags, flag_bytes) = execution
            .call_storage
            .capture_flags(self.executable.local_definitions.len())?;
        let prepared = execution.frames.prepare_push()?;
        let mut prepared = prepared;
        let frame = prepared.current_mut(parent)?;
        let window = execution.slots.push_ordinary_frame(
            &self.executable.frame_layout(),
            &mut frame.window,
            count,
            method,
            &self.function,
            self.executable.metadata.function_name_local,
            self.executable.observes_arguments,
        )?;
        frame.resume_pc = resume;
        let (mut cold, frame_bytes) = execution.call_storage.vacant(caller_realm);
        cold.property_generation = 0;
        cold.iterator_generation = 0;
        cold.return_to = Some(ReturnTarget {
            value_use: ReturnValue::Push,
            owner: ReturnOwner::Frame(parent),
            tail,
            operation: None,
        });
        cold.active_frame = guard.token();
        cold.entry_guard = Some(guard);
        cold.caller_realm = caller_realm;
        cold.function = self.function.into();
        cold.closure_slots = self.closure;
        cold.reusable_captured_locals = flags;
        cold.input = crate::engine::vm::CallInput {
            this_value: receiver,
            new_target: Value::Undefined,
            callee_global,
        }
        .into();
        prepared.install(Frame {
            executable: self.executable,
            window,
            fault_pc: 0,
            resume_pc: 0,
            cold,
        });
        #[cfg(feature = "profiling")]
        {
            crate::engine::api::profiling::record_owned_call_storage(frame_bytes, flag_bytes, 0);
            crate::engine::api::profiling::record_owned_execution_event(
                "ordinary_call_authenticated",
            );
        }
        #[cfg(not(feature = "profiling"))]
        let _ = (frame_bytes, flag_bytes);
        Ok(())
    }
}
