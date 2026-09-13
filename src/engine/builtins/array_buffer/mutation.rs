//! Resize and transfer retain the branded buffer across length coercion.
use crate::engine::{
    api::{runtime::Runtime, runtime_error::RuntimeError},
    builtins::native::ArrayBufferNativeKind,
    heap::ContextId,
    object::ObjectRef,
    value::{Value, conversion::NativeConversion},
    vm::{
        Completion, ToPrimitiveHint,
        call::{NativeArguments, NativeInvocation},
    },
};
pub(crate) enum BufferMutationStep {
    Complete(Completion),
    Primitive {
        value: Value,
        resume: BufferMutationResume,
    },
}
pub(crate) struct BufferMutationResume {
    realm: ContextId,
    object: ObjectRef,
    kind: ArrayBufferNativeKind,
    shared: bool,
}
impl BufferMutationStep {
    pub(crate) fn start_grow(
        runtime: &Runtime,
        realm: ContextId,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        let NativeInvocation::Call { this_value } = invocation else {
            return Err(RuntimeError::Invariant(
                "SharedArrayBuffer.prototype.grow received a constructor invocation",
            ));
        };
        let object = match runtime.require_shared_array_buffer(realm, this_value.clone())? {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => return Ok(Self::Complete(Completion::Throw(value))),
        };
        Ok(Self::Primitive {
            value: arguments
                .readable
                .first()
                .ok_or(RuntimeError::Invariant(
                    "SharedArrayBuffer grow argument was not padded",
                ))?
                .clone(),
            resume: BufferMutationResume {
                realm,
                object,
                kind: ArrayBufferNativeKind::Resize,
                shared: true,
            },
        })
    }

    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        kind: ArrayBufferNativeKind,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        if !matches!(
            kind,
            ArrayBufferNativeKind::Resize
                | ArrayBufferNativeKind::Transfer
                | ArrayBufferNativeKind::TransferToFixedLength
        ) {
            return Err(RuntimeError::Invariant(
                "buffer mutation received an invalid operation",
            ));
        }
        let NativeInvocation::Call { this_value } = invocation else {
            return Err(RuntimeError::Invariant(
                if matches!(kind, ArrayBufferNativeKind::Resize) {
                    "ArrayBuffer.prototype.resize received a constructor invocation"
                } else {
                    "ArrayBuffer transfer received a constructor invocation"
                },
            ));
        };
        let object = match runtime.require_array_buffer(realm, this_value.clone())? {
            NativeConversion::Value(object) => object,
            NativeConversion::Throw(value) => return Ok(Self::Complete(Completion::Throw(value))),
        };
        if !matches!(kind, ArrayBufferNativeKind::Resize) {
            let initial = runtime.array_buffer_snapshot(&object)?;
            if arguments.actual_arg_count == 0
                || matches!(arguments.readable.first(), Some(Value::Undefined))
            {
                return Ok(Self::Complete(runtime.finish_array_buffer_transfer(
                    realm,
                    object,
                    u64::from(initial.byte_length),
                    matches!(kind, ArrayBufferNativeKind::TransferToFixedLength),
                )?));
            }
        }
        let value = arguments
            .readable
            .first()
            .ok_or(RuntimeError::Invariant(
                if matches!(kind, ArrayBufferNativeKind::Resize) {
                    "ArrayBuffer resize argument was not padded"
                } else {
                    "ArrayBuffer transfer argument was not padded"
                },
            ))?
            .clone();
        Ok(Self::Primitive {
            value,
            resume: BufferMutationResume {
                realm,
                object,
                kind,
                shared: false,
            },
        })
    }
}
impl BufferMutationResume {
    pub(crate) fn resume(
        self,
        runtime: &Runtime,
        result: Completion,
    ) -> Result<BufferMutationStep, RuntimeError> {
        let value = match result {
            Completion::Return(value) => value,
            Completion::Throw(value) => {
                return Ok(BufferMutationStep::Complete(Completion::Throw(value)));
            }
        };
        if matches!(value, Value::Object(_)) {
            return Err(RuntimeError::Invariant(
                "buffer length conversion returned an object",
            ));
        }
        let result = if matches!(self.kind, ArrayBufferNativeKind::Resize) {
            match runtime.native_to_int64(self.realm, &value)? {
                NativeConversion::Value(length) => {
                    if self.shared {
                        runtime.finish_shared_array_buffer_grow(self.realm, self.object, length)?
                    } else {
                        runtime.finish_array_buffer_resize(self.realm, self.object, length)?
                    }
                }
                NativeConversion::Throw(value) => Completion::Throw(value),
            }
        } else {
            match runtime.native_to_index(self.realm, &value)? {
                NativeConversion::Value(length) => runtime.finish_array_buffer_transfer(
                    self.realm,
                    self.object,
                    length,
                    matches!(self.kind, ArrayBufferNativeKind::TransferToFixedLength),
                )?,
                NativeConversion::Throw(value) => Completion::Throw(value),
            }
        };
        Ok(BufferMutationStep::Complete(result))
    }
}
pub(in crate::engine::builtins) fn finish(
    runtime: &Runtime,
    realm: ContextId,
    mut step: BufferMutationStep,
) -> Result<Completion, RuntimeError> {
    loop {
        step = match step {
            BufferMutationStep::Complete(result) => return Ok(result),
            BufferMutationStep::Primitive { value, resume } => {
                let result = if matches!(value, Value::Object(_)) {
                    runtime.to_primitive(realm, value, ToPrimitiveHint::Number)?
                } else {
                    Completion::Return(value)
                };
                resume.resume(runtime, result)?
            }
        };
    }
}
