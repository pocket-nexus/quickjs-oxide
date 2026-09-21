//! Non-species copying `%TypedArray%.prototype` algorithms.
//!
//! Pinned QuickJS implements `with` and `toReversed` by allocating the source
//! element class in the builtin's defining realm, copying the branded source,
//! and then applying the requested mutation to that private result.  The
//! source's public constructor and `Symbol.species` are never observed.

use super::{TypedArraySnapshot, typed_array_absolute_byte_offset};
use crate::engine::{
    api::{error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError},
    builtins::native::TypedArrayElementKind,
    heap::ContextId,
    object::ObjectRef,
    value::{JsValue, conversion::NativeConversion},
    vm::{
        Completion, ToPrimitiveHint,
        call::{NativeArguments, NativeInvocation},
    },
};

#[cfg(test)]
use crate::engine::value::Value;
#[cfg(test)]
mod tests;

impl Runtime {
    pub(crate) fn call_typed_array_with(
        &self,
        realm: ContextId,
        invocation: NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        finish(
            self,
            realm,
            TypedWithStep::start(self, realm, &invocation, arguments)?,
        )
    }
    fn finish_typed_with(
        &self,
        realm: ContextId,
        source: ObjectRef,
        element: TypedArrayElementKind,
        initial_length: u64,
        index: i64,
        replacement: &JsValue,
    ) -> Result<Completion, RuntimeError> {
        let current = self.typed_array_state(&source)?;
        if current.out_of_bounds || index < 0 || index >= i64::from(current.length) {
            return Ok(Completion::Throw(self.new_native_error_jsvalue(
                realm,
                NativeErrorKind::Range,
                "invalid array index",
            )?));
        }

        // The allocation retains the pre-coercion length. If a tracking RAB
        // shrank, QuickJS fills the missing numeric tail through ordinary
        // element conversion; a BigInt tail consequently throws.
        let target =
            match self.typed_array_copy_to_default(realm, &source, element, initial_length)? {
                NativeConversion::Value(value) => value,
                NativeConversion::Throw(value) => {
                    return Ok(Completion::Throw(self.into_jsvalue(value)?));
                }
            };
        let index = u64::try_from(index)
            .map_err(|_| RuntimeError::Invariant("validated TypedArray.with index was negative"))?;
        match super::write::TypedWriteStep::set(
            self,
            target.clone(),
            Some(index),
            self.dup_jsvalue(replacement)?,
        )?
        .finish_sync(self, realm)?
        {
            NativeConversion::Value(_) => {}
            NativeConversion::Throw(value) => {
                return Ok(Completion::Throw(self.into_jsvalue(value)?));
            }
        }
        Ok(Completion::Return(JsValue::Object(target.into_handle())))
    }

    pub(crate) fn call_typed_array_to_reversed(
        &self,
        realm: ContextId,
        invocation: &NativeInvocation,
    ) -> Result<Completion, RuntimeError> {
        let NativeInvocation::Call { this_value } = invocation else {
            return Err(RuntimeError::Invariant(
                "TypedArray.prototype.toReversed received a constructor invocation",
            ));
        };
        let source = match self.require_typed_array_jsvalue(realm, this_value)? {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => {
                return Ok(Completion::Throw(self.into_jsvalue(value)?));
            }
        };
        let state = self.typed_array_state(&source)?;
        let target = match self.typed_array_copy_to_default(
            realm,
            &source,
            state.snapshot.element,
            u64::from(state.length),
        )? {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => {
                return Ok(Completion::Throw(self.into_jsvalue(value)?));
            }
        };
        let target_state = self.typed_array_state(&target)?;
        if target_state.length > 1 {
            let count = usize::try_from(target_state.length).map_err(|_| {
                RuntimeError::Invariant("TypedArray.toReversed length overflowed usize")
            })?;
            let width = usize::from(target_state.snapshot.element.byte_length());
            let start = typed_array_absolute_byte_offset(target_state.snapshot, 0)?;
            let byte_count = count.checked_mul(width).ok_or(RuntimeError::Invariant(
                "TypedArray.toReversed byte count overflowed usize",
            ))?;
            let access = self.snapshot_buffer_access(target_state.snapshot.buffer)?;
            self.with_buffer_range_mut(&access, start, byte_count, |bytes| {
                for left in 0..(count / 2) {
                    let right = count - 1 - left;
                    for byte in 0..width {
                        bytes.swap(left * width + byte, right * width + byte);
                    }
                }
            })?;
        }
        Ok(Completion::Return(JsValue::Object(target.into_handle())))
    }

    /// QuickJS's internal same-class TypedArray constructor used by copying
    /// methods. The target always has the builtin's defining-realm prototype.
    pub(crate) fn typed_array_copy_to_default(
        &self,
        realm: ContextId,
        source: &ObjectRef,
        element: TypedArrayElementKind,
        length: u64,
    ) -> Result<NativeConversion<ObjectRef>, RuntimeError> {
        let prototype = self.typed_array_default_prototype(realm, element)?;
        let source_snapshot = self.typed_array_snapshot(source)?;
        self.typed_array_copy_into_new(realm, &prototype, element, source, source_snapshot, length)
    }

    /// Shared tail of QuickJS `js_typed_array_constructor_ta`: validate the
    /// source around target allocation, preserve same-class machine words when
    /// the complete requested range remains live, and otherwise perform the
    /// element-by-element conversion path.
    pub(crate) fn typed_array_copy_into_new(
        &self,
        realm: ContextId,
        prototype: &ObjectRef,
        target_element: TypedArrayElementKind,
        source: &ObjectRef,
        source_snapshot: TypedArraySnapshot,
        length: u64,
    ) -> Result<NativeConversion<ObjectRef>, RuntimeError> {
        let source_state = self.typed_array_state_from_snapshot(source_snapshot)?;
        if source_state.out_of_bounds {
            return Ok(NativeConversion::Throw(self.new_native_error(
                realm,
                NativeErrorKind::Type,
                "ArrayBuffer is detached or resized",
            )?));
        }
        let target =
            match self.new_typed_array_for_length(realm, prototype, target_element, length)? {
                NativeConversion::Value(value) => value,
                NativeConversion::Throw(value) => return Ok(NativeConversion::Throw(value)),
            };

        let source_state = self.typed_array_state(source)?;
        if source_state.out_of_bounds {
            return Ok(NativeConversion::Throw(self.new_native_error(
                realm,
                NativeErrorKind::Type,
                "ArrayBuffer is detached or resized",
            )?));
        }
        let target_state = self.typed_array_state(&target)?;
        if source_snapshot.element == target_element && u64::from(source_state.length) >= length {
            let byte_count = usize::try_from(
                length
                    .checked_mul(u64::from(target_element.byte_length()))
                    .ok_or(RuntimeError::Invariant(
                        "TypedArray copy byte length overflowed u64",
                    ))?,
            )
            .map_err(|_| RuntimeError::Invariant("TypedArray copy byte length overflowed usize"))?;
            let source_start = usize::try_from(source_snapshot.byte_offset).map_err(|_| {
                RuntimeError::Invariant("TypedArray source byteOffset overflowed usize")
            })?;
            let target_start =
                usize::try_from(target_state.snapshot.byte_offset).map_err(|_| {
                    RuntimeError::Invariant("TypedArray target byteOffset overflowed usize")
                })?;
            let source_access = self.snapshot_buffer_access(source_snapshot.buffer)?;
            let target_access = self.snapshot_buffer_access(target_state.snapshot.buffer)?;
            self.move_buffer_range(
                &source_access,
                &target_access,
                source_start,
                target_start,
                byte_count,
            )?;
        } else {
            for index in 0..length {
                let value = self
                    .typed_array_read_index_jsvalue(source, index)?
                    .unwrap_or(JsValue::Undefined);
                match super::write::TypedWriteStep::set(self, target.clone(), Some(index), value)?
                    .finish_sync(self, realm)?
                {
                    NativeConversion::Value(_) => {}
                    NativeConversion::Throw(value) => {
                        return Ok(NativeConversion::Throw(value));
                    }
                }
            }
        }
        Ok(NativeConversion::Value(target))
    }
}
pub(crate) enum TypedWithStep {
    Complete(Completion),
    Primitive {
        value: JsValue,
        resume: TypedWithResume,
    },
}
pub(crate) struct TypedWithResume(Box<TypedWithResumeState>);
impl std::ops::Deref for TypedWithResume {
    type Target = TypedWithResumeState;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for TypedWithResume {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
const _: () = assert!(std::mem::size_of::<TypedWithResume>() <= 8);
pub(crate) struct TypedWithResumeState {
    replacement: JsValue,
    realm: ContextId,
    source: ObjectRef,
    element: TypedArrayElementKind,
    length: i64,
    phase: WithPhase,
}
impl Drop for TypedWithResumeState {
    fn drop(&mut self) {
        let _ = self
            .source
            .runtime()
            .release_jsvalue(std::mem::replace(&mut self.replacement, JsValue::Undefined));
    }
}
enum WithPhase {
    Index,
    Replacement(i64),
}
impl TypedWithStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        let NativeInvocation::Call { this_value } = invocation else {
            return Err(RuntimeError::Invariant(
                "TypedArray.prototype.with received a constructor invocation",
            ));
        };
        let source = match runtime.require_typed_array_jsvalue(realm, this_value)? {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => {
                return Ok(Self::Complete(Completion::Throw(
                    runtime.into_jsvalue(value)?,
                )));
            }
        };
        let initial = runtime.typed_array_state(&source)?;
        if initial.out_of_bounds {
            return Ok(Self::Complete(Completion::Throw(
                runtime.new_native_error_jsvalue(
                    realm,
                    NativeErrorKind::Type,
                    "ArrayBuffer is detached",
                )?,
            )));
        }
        let mut resume = TypedWithResume(Box::new(TypedWithResumeState {
            realm,
            source,
            element: initial.snapshot.element,
            length: i64::from(initial.length),
            replacement: JsValue::Undefined,
            phase: WithPhase::Index,
        }));
        resume.0.replacement = runtime.dup_jsvalue(arguments.readable.get(1).ok_or(
            RuntimeError::Invariant("TypedArray.with replacement argv was not padded"),
        )?)?;
        let value = runtime.dup_jsvalue(arguments.readable.first().ok_or(
            RuntimeError::Invariant("TypedArray.with index argv was not padded"),
        )?)?;
        Ok(Self::Primitive { value, resume })
    }
}
impl TypedWithResume {
    pub(crate) fn resume(
        mut self,
        runtime: &Runtime,
        result: Completion,
    ) -> Result<TypedWithStep, RuntimeError> {
        let value = match result {
            Completion::Return(value) => value,
            Completion::Throw(value) => {
                return Ok(TypedWithStep::Complete(Completion::Throw(value)));
            }
        };
        match self.0.phase {
            WithPhase::Index => {
                let number = runtime.number_from_primitive_jsvalue(self.0.realm, &value);
                runtime.release_jsvalue(value)?;
                let index = match number? {
                    NativeConversion::Value(number) => Runtime::int64_from_number(number),
                    NativeConversion::Throw(value) => {
                        return Ok(TypedWithStep::Complete(Completion::Throw(
                            runtime.into_jsvalue(value)?,
                        )));
                    }
                };
                let index = if index < 0 {
                    self.0.length + index
                } else {
                    index
                };
                Ok(TypedWithStep::Primitive {
                    value: std::mem::replace(&mut self.0.replacement, JsValue::Undefined),
                    resume: {
                        let updated_0 = WithPhase::Replacement(index);
                        self.0.phase = updated_0;
                        self
                    },
                })
            }
            WithPhase::Replacement(index) => {
                self.0.replacement = value;
                Ok(TypedWithStep::Complete(runtime.finish_typed_with(
                    self.0.realm,
                    self.0.source.clone(),
                    self.0.element,
                    self.0.length as u64,
                    index,
                    &self.0.replacement,
                )?))
            }
        }
    }
}
fn finish(
    runtime: &Runtime,
    realm: ContextId,
    mut step: TypedWithStep,
) -> Result<Completion, RuntimeError> {
    loop {
        step = match step {
            TypedWithStep::Complete(result) => return Ok(result),
            TypedWithStep::Primitive { value, resume } => {
                let result = if matches!(value, JsValue::Object(_)) {
                    runtime.to_primitive_jsvalue(realm, value, ToPrimitiveHint::Number)?
                } else {
                    Completion::Return(value)
                };
                resume.resume(runtime, result)?
            }
        };
    }
}

// S11 all-domain protocol bound; inline completion stays allocation-free.
const _: () = assert!(std::mem::size_of::<TypedWithStep>() <= 64);
