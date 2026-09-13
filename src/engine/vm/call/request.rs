//! Owned, classified bytecode invocation at the driver boundary.
//! Preparing a frame cannot invoke JavaScript and borrows no caller storage.

use crate::engine::api::error::Error;
use crate::engine::api::runtime::Runtime;
use crate::engine::code::rooted::FunctionBytecodeRef;
use crate::engine::heap::ContextId;
use crate::engine::heap::roots::VarRefRoot;
use crate::engine::object::CallableRef;
use crate::engine::value::Value;
use crate::engine::vm::exception::runtime_error_to_vm_error;
use crate::engine::vm::frame::{FrameCold, FrameEntry, ReturnTarget};
use crate::engine::vm::stack::FrameStorage;

/// Classification, Runtime-domain validation and frame-budget checks precede
/// this request. Its values stay rooted independently of the parent window.
pub(in crate::engine::vm) struct BytecodeCallRequest {
    pub callable: CallableRef,
    pub receiver: Value,
    pub new_target: Value,
    pub arguments: Vec<Value>,
    pub bytecode: FunctionBytecodeRef,
    pub closure_slots: Vec<VarRefRoot>,
    pub caller_realm: ContextId,
    pub return_to: ReturnTarget,
}

impl BytecodeCallRequest {
    pub(in crate::engine::vm) fn prepare(self, runtime: &Runtime) -> Result<FrameEntry, Error> {
        let Self {
            callable,
            receiver,
            new_target,
            arguments,
            bytecode,
            closure_slots,
            caller_realm,
            return_to,
        } = self;
        let prepared = runtime
            .prepare_bytecode_frame(&callable, receiver, new_target, &arguments, bytecode)
            .map_err(runtime_error_to_vm_error)?;
        if closure_slots.len() != usize::from(prepared.executable.metadata.closure_count) {
            return Err(Error::internal(
                "function object closure slot count does not match bytecode metadata",
            ));
        }
        let local_count = prepared.locals.len();
        let entry = FrameEntry {
            executable: prepared.executable,
            cold: Box::new(FrameCold {
                regions: Vec::new(),
                constructor_wait: None,
                class_wait: None,
                has_binding_wait: None,
                iterator_wait: None,
                iterator_generation: 0,
                eval_arguments: None,
                constructor_return: None,
                conversion: None,
                normalized_this: None,
                return_to: Some(return_to),
                active_frame: prepared.active_frame.token(),
                entry_guard: Some(prepared.active_frame),
                caller_realm,
                function: callable.as_object().clone(),
                closure_slots,
                reusable_captured_locals: vec![false; local_count],
                input: prepared.input,
            }),
            storage: FrameStorage {
                original_arguments: arguments,
                parameters: prepared.arguments,
                locals: prepared.locals,
                operands: Vec::new(),
            },
        };
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_call_storage(
            size_of::<FrameCold>(),
            entry.cold.reusable_captured_locals.capacity() * size_of::<bool>(),
            entry.storage.original_arguments.capacity() * size_of::<Value>(),
        );
        Ok(entry)
    }
}

/// Callback classification consumes the bound chain without executing code.
/// Both property getters and ToPrimitive methods keep the same argument order
/// and innermost bound receiver before choosing their driver entry.
pub(in crate::engine::vm) struct NormalizedCallback {
    pub callable: CallableRef,
    pub receiver: Value,
    pub arguments: Vec<Value>,
    pub classification: super::CallableExecution,
}

pub(in crate::engine::vm) fn normalize_callback(
    runtime: &Runtime,
    realm: ContextId,
    mut callable: CallableRef,
    mut receiver: Value,
    mut arguments: Vec<Value>,
) -> Result<crate::engine::value::conversion::NativeConversion<NormalizedCallback>, Error> {
    use crate::engine::value::conversion::NativeConversion;
    loop {
        match runtime
            .bytecode_for_callable(&callable)
            .map_err(runtime_error_to_vm_error)?
        {
            super::CallableExecution::Bound {
                target,
                this_value,
                arguments: bound,
            } => {
                arguments = match runtime
                    .concatenate_bound_arguments(realm, &bound, &arguments)
                    .map_err(runtime_error_to_vm_error)?
                {
                    NativeConversion::Value(arguments) => arguments,
                    NativeConversion::Throw(value) => return Ok(NativeConversion::Throw(value)),
                };
                receiver = this_value;
                callable = target;
            }
            classification => {
                return Ok(NativeConversion::Value(NormalizedCallback {
                    callable,
                    receiver,
                    arguments,
                    classification,
                }));
            }
        }
    }
}
