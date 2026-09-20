//! ArrayBuffer-family slicing shares range and species stages, then reacquires backing state.
use crate::engine::{
    api::{error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError},
    heap::ContextId,
    object::{ObjectRef, PropertyKey, WellKnownSymbol},
    value::{JsValue, Value, conversion::NativeConversion},
    vm::{
        Completion, ToPrimitiveHint,
        call::{ConstructorRef, NativeArguments, NativeInvocation},
    },
};
#[derive(Clone, Copy)]
pub(crate) enum BufferSliceKind {
    Array,
    Shared,
}
pub(crate) enum BufferSliceStep {
    Complete(Completion),
    Primitive {
        value: JsValue,
        resume: BufferSliceResume,
    },
    Read {
        object: ObjectRef,
        key: PropertyKey,
        resume: BufferSliceResume,
    },
    Construct {
        constructor: ConstructorRef,
        arguments: Vec<JsValue>,
        resume: BufferSliceResume,
    },
}
pub(crate) struct BufferSliceResume(Box<BufferSliceResumeState>);
impl std::ops::Deref for BufferSliceResume {
    type Target = BufferSliceResumeState;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for BufferSliceResume {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
const _: () = assert!(std::mem::size_of::<BufferSliceResume>() <= 8);
pub(crate) struct BufferSliceResumeState {
    realm: ContextId,
    source: ObjectRef,
    length: i64,
    kind: BufferSliceKind,
    phase: Phase,
}
enum Phase {
    Start(Option<Value>),
    End(i64),
    Constructor { start: i64, count: u32 },
    Species { start: i64, count: u32 },
    Construct { start: i64, count: u32 },
}
impl BufferSliceStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        kind: BufferSliceKind,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        let NativeInvocation::Call { this_value } = invocation else {
            return Err(RuntimeError::Invariant(
                "Buffer slice received a constructor invocation",
            ));
        };
        let this_value = runtime.root_value(this_value)?;
        let source = match kind {
            BufferSliceKind::Array => runtime.array_buffer_slice_source(realm, this_value)?,
            BufferSliceKind::Shared => runtime.shared_array_buffer_slice_source(realm, this_value)?,
        };
        let (source, length) = match source {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => {
                return Ok(Self::Complete(Completion::Throw(
                    runtime.into_jsvalue(value)?,
                )));
            }
        };
        let end = if (matches!(kind, BufferSliceKind::Array) && arguments.actual_arg_count < 2)
            || matches!(arguments.readable.get(1), Some(JsValue::Undefined))
        {
            None
        } else {
            Some(runtime.root_value(arguments.readable.get(1).ok_or(
                RuntimeError::Invariant("Buffer slice end argument was not padded"),
            )?)?)
        };
        Ok(Self::Primitive {
            value: runtime.dup_jsvalue(arguments.readable.first().ok_or(
                RuntimeError::Invariant("Buffer slice start argument was not padded"),
            )?)?,
            resume: BufferSliceResume(Box::new(BufferSliceResumeState {
                realm,
                source,
                length,
                kind,
                phase: Phase::Start(end),
            })),
        })
    }
}
impl BufferSliceResume {
    fn select(
        mut self,
        runtime: &Runtime,
        start: i64,
        end: i64,
    ) -> Result<BufferSliceStep, RuntimeError> {
        let count = u32::try_from((end - start).max(0))
            .map_err(|_| RuntimeError::Invariant("validated Buffer slice length overflowed u32"))?;
        Ok(BufferSliceStep::Read {
            object: self.0.source.clone(),
            key: runtime
                .pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Constructor)?,
            resume: {
                let updated_0 = Phase::Constructor { start, count };
                self.0.phase = updated_0;
                self
            },
        })
    }
    fn default(
        self,
        runtime: &Runtime,
        start: i64,
        count: u32,
    ) -> Result<BufferSliceStep, RuntimeError> {
        let result = match self.0.kind {
            BufferSliceKind::Array => runtime.allocate_array_buffer_slice(self.0.realm, count)?,
            BufferSliceKind::Shared => {
                runtime.allocate_shared_array_buffer_slice(self.0.realm, count)?
            }
        };
        let target = match result {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => {
                return Ok(BufferSliceStep::Complete(Completion::Throw(
                    runtime.into_jsvalue(value)?,
                )));
            }
        };
        self.copy(runtime, target, start, count)
    }
    fn copy(
        self,
        runtime: &Runtime,
        target: ObjectRef,
        start: i64,
        count: u32,
    ) -> Result<BufferSliceStep, RuntimeError> {
        Ok(BufferSliceStep::Complete(match self.0.kind {
            BufferSliceKind::Array => runtime.finish_array_buffer_slice(
                self.0.realm,
                self.0.source,
                target,
                start,
                count,
            )?,
            BufferSliceKind::Shared => runtime.finish_shared_array_buffer_slice(
                self.0.realm,
                self.0.source,
                target,
                start,
                count,
            )?,
        }))
    }
    pub(crate) fn resume(
        mut self,
        runtime: &Runtime,
        result: Completion,
    ) -> Result<BufferSliceStep, RuntimeError> {
        let value = match result {
            Completion::Return(value) => runtime.root_and_release_jsvalue(value)?,
            Completion::Throw(value) => {
                return Ok(BufferSliceStep::Complete(Completion::Throw(value)));
            }
        };
        match self.0.phase {
            Phase::Start(ref end) => {
                let start = match runtime.native_to_int64_clamp(
                    self.0.realm,
                    &value,
                    0,
                    self.0.length,
                    self.0.length,
                )? {
                    NativeConversion::Value(value) => value,
                    NativeConversion::Throw(value) => {
                        return Ok(BufferSliceStep::Complete(Completion::Throw(
                            runtime.into_jsvalue(value)?,
                        )));
                    }
                };
                if let Some(value) = end {
                    Ok(BufferSliceStep::Primitive {
                        value: runtime.into_jsvalue(value.clone())?,
                        resume: {
                            let updated_0 = Phase::End(start);
                            self.0.phase = updated_0;
                            self
                        },
                    })
                } else {
                    let length = self.0.length;
                    self.select(runtime, start, length)
                }
            }
            Phase::End(start) => {
                let end = match runtime.native_to_int64_clamp(
                    self.0.realm,
                    &value,
                    0,
                    self.0.length,
                    self.0.length,
                )? {
                    NativeConversion::Value(value) => value,
                    NativeConversion::Throw(value) => {
                        return Ok(BufferSliceStep::Complete(Completion::Throw(
                            runtime.into_jsvalue(value)?,
                        )));
                    }
                };
                self.select(runtime, start, end)
            }
            Phase::Constructor { start, count } => {
                if matches!(value, Value::Undefined) {
                    return self.default(runtime, start, count);
                }
                let Value::Object(object) = value else {
                    return Ok(BufferSliceStep::Complete(Completion::Throw(
                        runtime.new_native_error_jsvalue(
                            self.0.realm,
                            NativeErrorKind::Type,
                            "not an object",
                        )?,
                    )));
                };
                Ok(BufferSliceStep::Read {
                    object,
                    key: PropertyKey::from(runtime.well_known_symbol(WellKnownSymbol::Species)),
                    resume: {
                        let updated_0 = Phase::Species { start, count };
                        self.0.phase = updated_0;
                        self
                    },
                })
            }
            Phase::Species { start, count } => {
                if matches!(value, Value::Undefined | Value::Null) {
                    return self.default(runtime, start, count);
                }
                if !matches!(value, Value::Object(_)) {
                    return Ok(BufferSliceStep::Complete(Completion::Throw(
                        runtime.into_jsvalue(
                            runtime.new_not_constructor_error(self.0.realm, &value)?,
                        )?,
                    )));
                }
                let constructor = match runtime.constructor_from_value(self.0.realm, value)? {
                    NativeConversion::Value(value) => value,
                    NativeConversion::Throw(value) => {
                        return Ok(BufferSliceStep::Complete(Completion::Throw(
                            runtime.into_jsvalue(value)?,
                        )));
                    }
                };
                let mut arguments = Vec::new();
                if arguments.try_reserve_exact(1).is_err() {
                    return Ok(BufferSliceStep::Complete(Completion::Throw(
                        runtime.new_native_error_jsvalue(
                            self.0.realm,
                            NativeErrorKind::Internal,
                            "out of memory",
                        )?,
                    )));
                }
                arguments.push(JsValue::Int(
                    i32::try_from(count).expect("Buffer slice length is bounded by i32::MAX"),
                ));
                Ok(BufferSliceStep::Construct {
                    constructor,
                    arguments,
                    resume: {
                        let updated_0 = Phase::Construct { start, count };
                        self.0.phase = updated_0;
                        self
                    },
                })
            }
            Phase::Construct { start, count } => {
                let Value::Object(target) = value else {
                    return Ok(BufferSliceStep::Complete(Completion::Throw(
                        runtime.new_native_error_jsvalue(
                            self.0.realm,
                            NativeErrorKind::Type,
                            match self.0.kind {
                                BufferSliceKind::Array => "ArrayBuffer object expected",
                                BufferSliceKind::Shared => "SharedArrayBuffer object expected",
                            },
                        )?,
                    )));
                };
                self.copy(runtime, target, start, count)
            }
        }
    }
}
pub(in crate::engine::builtins) fn finish(
    runtime: &Runtime,
    realm: ContextId,
    mut step: BufferSliceStep,
) -> Result<Completion, RuntimeError> {
    loop {
        step = match step {
            BufferSliceStep::Complete(result) => return Ok(result),
            BufferSliceStep::Primitive { value, resume } => {
                let result = if matches!(value, JsValue::Object(_)) {
                    runtime.to_primitive_jsvalue(realm, value, ToPrimitiveHint::Number)?
                } else {
                    Completion::Return(value)
                };
                resume.resume(runtime, result)?
            }
            BufferSliceStep::Read {
                object,
                key,
                resume,
            } => resume.resume(
                runtime,
                runtime.get_property_in_realm(realm, &object, &key)?,
            )?,
            BufferSliceStep::Construct {
                constructor,
                arguments,
                resume,
            } => {
                let arguments = arguments
                    .into_iter()
                    .map(|value| runtime.root_and_release_jsvalue(value))
                    .collect::<Result<Vec<_>, _>>()?;
                resume.resume(
                    runtime,
                    runtime.construct_constructor_internal(
                        realm,
                        &constructor,
                        &constructor,
                        &arguments,
                    )?,
                )?
            }
        };
    }
}

// S11 all-domain protocol bound; inline completion stays allocation-free.
const _: () = assert!(std::mem::size_of::<BufferSliceStep>() <= 64);
