//! Direct published root-call preparation for the sole execution core.
use super::{
    Completion,
    frame::{FrameCold, FrameEntry},
    stack::FrameStorage,
};
use crate::engine::api::{Error, runtime::Runtime, runtime_error::RuntimeError};
use crate::engine::code::function::metadata::FunctionKind;
use crate::engine::code::rooted::FunctionBytecodeRef;
use crate::engine::heap::ContextId;
use crate::engine::object::CallableRef;
use crate::engine::value::JsValue;
impl Runtime {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn execute_bytecode_callable_jsvalue(
        &self,
        caller_realm: ContextId,
        callable: &CallableRef,
        this_value: JsValue,
        new_target: JsValue,
        arguments: Vec<JsValue>,
        bytecode: FunctionBytecodeRef,
        closure_slots: crate::engine::vm::closure::ClosureSlots,
    ) -> Result<Completion, RuntimeError> {
        let mut arguments = arguments;
        let validation = (|| {
            if self.bytecode_call_would_overflow() {
                return self
                    .bytecode_stack_overflow_completion(caller_realm, &bytecode)
                    .map(Some);
            }
            if !bytecode.belongs_to(self) {
                return Err(RuntimeError::WrongRuntime("function bytecode"));
            }
            Ok(None)
        })();
        match validation {
            Ok(None) => {}
            result => {
                let _ = self.release_jsvalue(this_value);
                let _ = self.release_jsvalue(new_target);
                for value in arguments.drain(..) {
                    let _ = self.release_jsvalue(value);
                }
                return result.map(|value| value.expect("root call rejection"));
            }
        }
        let entry = prepare_call(
            self,
            caller_realm,
            callable,
            this_value,
            new_target,
            arguments,
            bytecode,
            closure_slots,
        )?;
        let metadata = entry.executable.metadata;
        let module_link = metadata.is_module
            && matches!(
                entry.cold.input.this_value,
                crate::engine::value::JsValue::Bool(true)
            );
        if metadata.function_kind == FunctionKind::Async && !module_link {
            return self.start_async_bytecode_callable(caller_realm, entry);
        }
        let result = super::driver::execute(
            self.clone(),
            entry,
            super::execution::ExecutionLimits::for_runtime(self),
        )
        .map_err(RuntimeError::Engine)?;
        if matches!(
            metadata.function_kind,
            FunctionKind::Generator | FunctionKind::AsyncGenerator
        ) {
            return super::suspend::creation::GeneratorCreation {
                realm: caller_realm,
                callable: callable.clone(),
                asynchronous: metadata.function_kind == FunctionKind::AsyncGenerator,
            }
            .initial(
                self,
                result
                    .finish_suspending(self.clone())
                    .map_err(RuntimeError::Engine)?,
            )?
            .finish(self, caller_realm);
        }
        result.finish(self.clone()).map_err(RuntimeError::Engine)
    }
}

/// Normal root entries use the same direct window initialization as child
/// calls. The public borrowed argv needs an independent snapshot, but no
/// parameter/local binding vectors are built.
#[inline(never)]
#[allow(clippy::too_many_arguments)]
pub(in crate::engine::vm) fn prepare_call(
    runtime: &Runtime,
    caller_realm: crate::engine::heap::ContextId,
    callable: &crate::engine::object::CallableRef,
    receiver: JsValue,
    new_target: JsValue,
    arguments: Vec<JsValue>,
    bytecode: crate::engine::code::rooted::FunctionBytecodeRef,
    closure_slots: crate::engine::vm::closure::ClosureSlots,
) -> Result<FrameEntry, crate::engine::api::runtime_error::RuntimeError> {
    use crate::engine::api::runtime_error::RuntimeError;
    let prepared =
        match runtime.prepare_owned_bytecode_frame(callable, receiver, new_target, bytecode) {
            Ok(prepared) => prepared,
            Err(error) => {
                for value in arguments {
                    let _ = runtime.release_jsvalue(value);
                }
                return Err(error);
            }
        };
    if closure_slots.len() != usize::from(prepared.executable.metadata.closure_count) {
        for value in arguments {
            let _ = runtime.release_jsvalue(value);
        }
        return Err(RuntimeError::Engine(Error::internal(
            "function object closure slot count does not match bytecode metadata",
        )));
    }
    let original_arguments = arguments;
    let local_count = if prepared.executable.has_captured_locals {
        prepared.executable.local_definitions.len()
    } else {
        0
    };
    let active_token = prepared.active_frame.token();
    let cold = crate::engine::vm::frame::ColdFrame::new(FrameCold {
        rare: std::cell::OnceCell::new(),
        return_to: None,
        entry_guard: Some(prepared.active_frame),
        function: (callable.as_object().clone()).into(),
        closure_slots,
        reusable_captured_locals: vec![false; local_count],
        input: (prepared.input).into(),
    });
    #[cfg(feature = "profiling")]
    crate::engine::api::profiling::record_owned_call_storage(
        size_of::<crate::engine::vm::frame::FrameBody>(),
        cold.reusable_captured_locals.capacity(),
        original_arguments.capacity() * size_of::<JsValue>(),
    );
    let entry = FrameEntry {
        property_generation: 0,
        iterator_generation: 0,
        caller_realm,
        active_frame: active_token,

        initialize_bindings: true,
        executable: prepared.executable,
        cold,
        storage: FrameStorage {
            original_arguments,
            parameters: Vec::new(),
            locals: Vec::new(),
            operands: Vec::new(),
        },
    };
    Ok(entry)
}
