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
        let mut input = crate::engine::vm::protocol::CallInputGuard::new(
            runtime,
            crate::engine::vm::protocol::CallInput::new(runtime, receiver, new_target, None),
        );
        let mut arguments = crate::engine::vm::stack::FrameStorageGuard::new(
            runtime,
            FrameStorage {
                original_arguments: arguments,
                parameters: Vec::new(),
                locals: Vec::new(),
                operands: Vec::new(),
            },
        );
        storage.reserve()?;
        let input = input.take();
        let mut prepared = runtime
            .prepare_owned_bytecode_frame(&callable, input.this_value, input.new_target, bytecode)
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
        let function =
            crate::engine::vm::closure::FrameFunction::new(callable.into_object(), closure_slots)
                .map_err(runtime_error_to_vm_error)?;
        let (cold, frame_bytes) = storage.install(FrameCold {
            rare: std::cell::OnceCell::new(),
            return_to: Some(return_to),
            entry_guard: Some(prepared.active_frame.into_internal()),
            function: function.into(),
            reusable_captured_locals: flags,
            input: prepared.input.take().into(),
        });
        let entry = FrameEntry {
            property_generation: 0,
            iterator_generation: 0,
            caller_realm,
            active_frame,
            initialize_bindings: true,
            executable: prepared.executable,
            cold,
            storage: arguments.take(),
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
    }
}

/// Callback classification consumes the bound chain without executing code.
/// Both property getters and ToPrimitive methods keep the same argument order
/// and innermost bound receiver before choosing their driver entry.
pub(crate) struct NormalizedCallback {
    pub callable: CallableRef,
    pub receiver: JsValue,
    pub arguments: Vec<JsValue>,
    pub classification: super::CallableExecution,
}

pub(in crate::engine::vm) fn normalize_callback(
    runtime: &Runtime,
    realm: ContextId,
    callable: CallableRef,
    receiver: JsValue,
    arguments: Vec<JsValue>,
) -> Result<crate::engine::value::conversion::NativeConversion<NormalizedCallback>, Error> {
    let admission = runtime.operation().and_then(|_| {
        if callable.belongs_to(runtime) {
            Ok(())
        } else {
            Err(crate::engine::api::RuntimeError::WrongRuntime("callable"))
        }
    });
    if let Err(error) = admission {
        let mut inputs = super::ordinary::RawCallbackInputs {
            selected_callee: None,
            callback_callee: None,
            receiver: Some(receiver),
            arguments,
            preserved_receiver: None,
        };
        inputs
            .retire_at_boundary(runtime)
            .map_err(runtime_error_to_vm_error)?;
        drop(callable);
        runtime.check_poison().map_err(runtime_error_to_vm_error)?;
        return Err(runtime_error_to_vm_error(error));
    }
    let selected = match super::ordinary::DirectSelection::select_in_state(
        runtime,
        &runtime.0.state.borrow(),
        callable.as_object().object_id(),
    ) {
        Ok(selected) => selected,
        Err(error) => {
            let mut inputs = super::ordinary::RawCallbackInputs {
                selected_callee: None,
                callback_callee: None,
                receiver: Some(receiver),
                arguments,
                preserved_receiver: None,
            };
            inputs
                .retire_at_boundary(runtime)
                .map_err(runtime_error_to_vm_error)?;
            drop(callable);
            runtime.check_poison().map_err(runtime_error_to_vm_error)?;
            return Err(runtime_error_to_vm_error(error));
        }
    };
    normalize_callback_from_selection(runtime, realm, callable, receiver, arguments, selected)
        .map_err(runtime_error_to_vm_error)
}

fn normalize_callback_from_selection(
    runtime: &Runtime,
    realm: ContextId,
    callable: CallableRef,
    receiver: JsValue,
    arguments: Vec<JsValue>,
    selected: super::ordinary::DirectSelection<'_>,
) -> Result<
    crate::engine::value::conversion::NativeConversion<NormalizedCallback>,
    crate::engine::api::RuntimeError,
> {
    use super::ordinary::{RawCallbackGuard, RawCallbackInputs};
    use crate::engine::value::conversion::NativeConversion;
    let _unwind = runtime.unwind_guard();
    // The caller retains Runtime access independently. Transfer before taking
    // State; no rooted public owner or Drop lives under this lease.
    let function = callable.into_object().into_execution_handle();
    let inputs = {
        let mut state = runtime.0.state.borrow_mut();
        let mut owner = RawCallbackGuard::new(
            &mut state,
            &runtime.0.poisoned,
            RawCallbackInputs::new(function, receiver, arguments),
        );
        let result = {
            let (state, inputs) = owner.parts();
            inputs.normalize_bound_chain_from_selection_in_state(runtime, state, realm, selected)
        };
        match result {
            Err(error) => {
                owner.retire()?;
                return Err(error);
            }
            Ok(NativeConversion::Throw(value)) => {
                let (state, inputs) = owner.parts();
                let mut value = crate::engine::heap::runtime::owned_values::OwnedValueGuard::new(
                    state,
                    &runtime.0.poisoned,
                    value,
                );
                let (state, value) = value.parts();
                inputs.retire(state, &runtime.0.poisoned)?;
                return Ok(NativeConversion::Throw(
                    value.take().expect("Bound overflow diagnostic"),
                ));
            }
            Ok(NativeConversion::Value(_)) => owner.take(),
        }
    };
    runtime.check_poison()?;
    let mut inputs = inputs;
    let callable =
        CallableRef::from_validated_object(crate::engine::object::ObjectRef::from_owned_handle(
            runtime.clone(),
            inputs.selected_callee.take().expect("normalized callable"),
        ));
    // This is the actual legacy terminal boundary (Proxy, special bytecode, or
    // legacy native), not another Bound normalization implementation.
    let classification = match runtime.bytecode_for_callable(&callable) {
        Ok(classification) => classification,
        Err(error) => {
            inputs.retire_at_boundary(runtime)?;
            drop(callable);
            runtime.check_poison()?;
            return Err(error);
        }
    };
    Ok(NativeConversion::Value(NormalizedCallback {
        callable,
        receiver: inputs.receiver.take().expect("normalized receiver"),
        arguments: std::mem::take(&mut inputs.arguments),
        classification,
    }))
}
