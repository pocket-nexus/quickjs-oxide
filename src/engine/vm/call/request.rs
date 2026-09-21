//! Owned, classified bytecode invocation at the driver boundary.
//! Preparing a frame cannot invoke JavaScript and borrows no caller storage.

use crate::engine::api::error::Error;
use crate::engine::api::runtime::Runtime;
use crate::engine::code::rooted::FunctionBytecodeRef;
use crate::engine::heap::ContextId;
use crate::engine::object::CallableRef;
use crate::engine::value::JsValue;
use crate::engine::vm::exception::runtime_error_to_vm_error;
use crate::engine::vm::frame::{FrameCold, FrameEntry, ReturnTarget};
use crate::engine::vm::stack::FrameStorage;

/// Classification, Runtime-domain validation and frame-budget checks precede
/// this request. Its values stay rooted independently of the parent window.
pub(in crate::engine::vm) struct BytecodeCallRequest {
    pub callable: CallableRef,
    pub receiver: JsValue,
    pub new_target: JsValue,
    pub arguments: Vec<JsValue>,
    pub bytecode: FunctionBytecodeRef,
    pub closure_slots: crate::engine::vm::closure::ClosureSlots,
    pub caller_realm: ContextId,
    pub return_to: ReturnTarget,
}

impl BytecodeCallRequest {
    /// Release the internal edges owned by a request abandoned before
    /// [`Self::prepare`] consumed it (throw or overflow replies).
    pub(in crate::engine::vm) fn release_owned_values(
        &mut self,
        runtime: &Runtime,
    ) -> Result<(), crate::engine::api::runtime_error::RuntimeError> {
        let mut first_error = None;
        let new_target = std::mem::replace(&mut self.new_target, JsValue::Undefined);
        let receiver = std::mem::replace(&mut self.receiver, JsValue::Undefined);
        for value in self.arguments.drain(..).chain([new_target, receiver]) {
            if let Err(error) = runtime.release_jsvalue(value) {
                first_error.get_or_insert(error);
            }
        }
        first_error.map_or(Ok(()), Err)
    }

    pub(in crate::engine::vm) fn prepare(
        self,
        runtime: &Runtime,
        storage: &mut super::super::frame::CallStorage,
    ) -> Result<FrameEntry, Error> {
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
        if let Err(error) = storage.reserve() {
            let _ = runtime.release_jsvalue(receiver);
            let _ = runtime.release_jsvalue(new_target);
            for argument in arguments {
                let _ = runtime.release_jsvalue(argument);
            }
            return Err(error);
        }
        let mut arguments = arguments;
        let result = (|| {
            let prepared = runtime
                .prepare_owned_bytecode_frame(&callable, receiver, new_target, bytecode)
                .map_err(runtime_error_to_vm_error)?;
            if closure_slots.len() != usize::from(prepared.executable.metadata.closure_count) {
                return Err(Error::internal(
                    "function object closure slot count does not match bytecode metadata",
                ));
            }
            let local_count = prepared.executable.local_definitions.len();
            let (flags, flag_bytes) = if prepared.executable.has_captured_locals {
                storage.capture_flags(local_count)?
            } else {
                (Vec::new(), 0)
            };
            let active_frame = prepared.active_frame.token();
            let (cold, frame_bytes) = storage.install(FrameCold {
                rare: std::cell::OnceCell::new(),
                return_to: Some(return_to),
                entry_guard: Some(prepared.active_frame),
                function: (callable.into_object()).into(),
                closure_slots,
                reusable_captured_locals: flags,
                input: (prepared.input).into(),
            });
            let entry = FrameEntry {
                property_generation: 0,
                iterator_generation: 0,
                caller_realm,
                active_frame,

                initialize_bindings: true,
                executable: prepared.executable,
                cold,
                storage: FrameStorage {
                    original_arguments: std::mem::take(&mut arguments),
                    parameters: Vec::new(),
                    locals: Vec::new(),
                    operands: Vec::new(),
                },
            };
            #[cfg(feature = "profiling")]
            crate::engine::api::profiling::record_owned_execution_event(
                "call_callee_owner_transferred",
            );
            #[cfg(feature = "profiling")]
            crate::engine::api::profiling::record_owned_call_storage(
                frame_bytes,
                flag_bytes,
                entry.storage.original_arguments.capacity() * size_of::<JsValue>(),
            );
            #[cfg(not(feature = "profiling"))]
            let _ = (frame_bytes, flag_bytes);
            Ok(entry)
        })();
        for argument in arguments {
            let _ = runtime.release_jsvalue(argument);
        }
        result
    }
}

/// Callback classification consumes the bound chain without executing code.
/// Both property getters and ToPrimitive methods keep the same argument order
/// and innermost bound receiver before choosing their driver entry.
pub(in crate::engine::vm) struct NormalizedCallback {
    pub callable: CallableRef,
    pub receiver: JsValue,
    pub arguments: Vec<JsValue>,
    pub classification: super::CallableExecution,
}

pub(in crate::engine::vm) fn normalize_callback(
    runtime: &Runtime,
    realm: ContextId,
    mut callable: CallableRef,
    receiver: JsValue,
    arguments: Vec<JsValue>,
) -> Result<crate::engine::value::conversion::NativeConversion<NormalizedCallback>, Error> {
    use crate::engine::value::conversion::NativeConversion;
    let mut receiver = Some(receiver);
    let mut arguments = arguments;
    let result = (|| loop {
        match runtime
            .bytecode_for_callable(&callable)
            .map_err(runtime_error_to_vm_error)?
        {
            super::CallableExecution::Bound {
                target,
                this_value,
                arguments: bound,
            } => {
                runtime
                    .release_jsvalue(receiver.replace(this_value).expect("callback receiver"))
                    .map_err(runtime_error_to_vm_error)?;
                arguments = match runtime
                    .concatenate_bound_arguments_jsvalue(
                        realm,
                        bound,
                        std::mem::take(&mut arguments),
                    )
                    .map_err(runtime_error_to_vm_error)?
                {
                    NativeConversion::Value(arguments) => arguments,
                    NativeConversion::Throw(value) => return Ok(NativeConversion::Throw(value)),
                };
                callable = target;
            }
            classification => {
                return Ok(NativeConversion::Value(NormalizedCallback {
                    callable,
                    receiver: receiver.take().expect("callback receiver"),
                    arguments: std::mem::take(&mut arguments),
                    classification,
                }));
            }
        }
    })();
    if let Some(receiver) = receiver {
        let _ = runtime.release_jsvalue(receiver);
    }
    for argument in arguments {
        let _ = runtime.release_jsvalue(argument);
    }
    result
}
