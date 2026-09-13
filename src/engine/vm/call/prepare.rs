//! Bytecode frame preparation owned by the call layer.
//! Publication remains authoritative; parameter padding, lexical initialization
//! and the named-function binding retain their existing ordering and semantics.

use crate::engine::api::runtime::Runtime;
use crate::engine::api::runtime_error::RuntimeError;
use crate::engine::code::rooted::FunctionBytecodeRef;
use crate::engine::code::runtime::{PublishedFunctionData, PublishedFunctionSnapshot};
use crate::engine::object::CallableRef;
use crate::engine::value::Value;
use crate::engine::vm::CallInput;
use crate::engine::vm::bindings::FrameBinding;
use crate::engine::vm::frames::ActiveFrameGuard;

pub(in crate::engine::vm) struct PreparedBytecodeFrame {
    pub executable: PublishedFunctionSnapshot,
    pub active_frame: ActiveFrameGuard,
    pub input: CallInput,
    pub arguments: Vec<FrameBinding>,
    pub locals: Vec<FrameBinding>,
}

impl Runtime {
    pub(in crate::engine::vm) fn prepare_bytecode_frame(
        &self,
        callable: &CallableRef,
        this_value: Value,
        new_target: Value,
        arguments: &[Value],
        bytecode: FunctionBytecodeRef,
    ) -> Result<PreparedBytecodeFrame, RuntimeError> {
        let executable = self.snapshot_function_bytecode(&bytecode)?;
        let PublishedFunctionData {
            local_definitions,
            metadata,
            realm,
            ..
        } = &*executable;
        let metadata = *metadata;
        let realm = *realm;
        let callee_global = self.global_object_for_realm(realm)?;
        let active_frame = self.push_bytecode_active_frame(
            callable.as_object().clone(),
            bytecode,
            realm,
            metadata.strict,
        )?;
        let argument_slots = executable.frame_layout().argument_slots(arguments.len());
        let mut frame_arguments = Vec::with_capacity(argument_slots);
        frame_arguments.extend(arguments.iter().cloned().map(FrameBinding::Direct));
        frame_arguments.resize_with(argument_slots, || FrameBinding::Direct(Value::Undefined));
        let mut frame_locals = local_definitions
            .iter()
            .map(|definition| {
                if definition.is_lexical {
                    FrameBinding::Uninitialized
                } else {
                    FrameBinding::Direct(Value::Undefined)
                }
            })
            .collect::<Vec<_>>();
        if let Some(index) = metadata.function_name_local {
            let binding =
                frame_locals
                    .get_mut(usize::from(index))
                    .ok_or(RuntimeError::Invariant(
                        "function-name local is outside the frame",
                    ))?;
            *binding = FrameBinding::Direct(Value::Object(callable.as_object().clone()));
        }
        Ok(PreparedBytecodeFrame {
            executable,
            active_frame,
            input: CallInput {
                this_value,
                new_target,
                callee_global,
            },
            arguments: frame_arguments,
            locals: frame_locals,
        })
    }
}
