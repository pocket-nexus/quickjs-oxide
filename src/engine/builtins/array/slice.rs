//! Slice and splice keep copied result and completed receiver mutations across replies.
#[cfg(feature = "stack-vm")]
use crate::engine::builtins::native::NativeFunctionId;
use crate::engine::{
    api::{error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError},
    builtins::native::ArraySliceKind,
    heap::ContextId,
    object::{
        DescriptorField, ObjectRef, OrdinaryPropertyDescriptor, PropertyKey,
        operations::{InternalDefineResult, InternalSetResult},
    },
    value::{Value, conversion::NativeConversion},
    vm::{
        Completion,
        call::{NativeArguments, NativeInvocation},
    },
};
#[derive(Clone, Copy)]
pub(crate) enum SliceKind {
    Slice,
    Splice,
    ToSpliced,
}
impl SliceKind {
    #[cfg(feature = "stack-vm")]
    pub(crate) fn for_target(target: NativeFunctionId) -> Option<Self> {
        match target {
            NativeFunctionId::ArrayPrototypeSlice(ArraySliceKind::Slice) => Some(Self::Slice),
            NativeFunctionId::ArrayPrototypeSlice(ArraySliceKind::Splice) => Some(Self::Splice),
            NativeFunctionId::ArrayPrototypeToSpliced => Some(Self::ToSpliced),
            _ => None,
        }
    }
}
pub(crate) enum SliceStep {
    Complete(Completion),
    Read {
        object: ObjectRef,
        key: PropertyKey,
        resume: SliceResume,
    },
    Number {
        value: Value,
        resume: SliceResume,
    },
    Has {
        object: ObjectRef,
        key: PropertyKey,
        resume: SliceResume,
    },
    Species {
        source: ObjectRef,
        length: u64,
        resume: SliceResume,
    },
    Define {
        object: ObjectRef,
        key: PropertyKey,
        descriptor: OrdinaryPropertyDescriptor,
        resume: SliceResume,
    },
    Set {
        object: ObjectRef,
        key: PropertyKey,
        value: Value,
        resume: SliceResume,
    },
    Delete {
        object: ObjectRef,
        key: PropertyKey,
        resume: SliceResume,
    },
    Copy {
        object: ObjectRef,
        to: u64,
        from: u64,
        count: u64,
        backwards: bool,
        resume: SliceResume,
    },
}
enum Phase {
    Length,
    LengthNumber,
    Start,
    End,
    Species,
    Has,
    Read,
    Define,
    ResultLength,
    Copy,
    Delete,
    Insert,
    FinalLength,
}
pub(crate) struct SliceResume {
    realm: ContextId,
    kind: SliceKind,
    phase: Phase,
    object: ObjectRef,
    arguments: Vec<Value>,
    actual: usize,
    length: u64,
    start: u64,
    count: u64,
    items: u64,
    new_length: u64,
    cursor: u64,
    result: Option<ObjectRef>,
    values: Vec<Value>,
}
impl SliceStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        kind: SliceKind,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        let NativeInvocation::Call { this_value } = invocation else {
            return Err(RuntimeError::Invariant(
                "Array slice requires generic invocation",
            ));
        };
        let object = match runtime.native_to_object(realm, this_value.clone())? {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => return Ok(Self::Complete(Completion::Throw(value))),
        };
        Ok(Self::Read {
            object: object.clone(),
            key: runtime.intern_property_key("length")?,
            resume: SliceResume {
                realm,
                kind,
                phase: Phase::Length,
                object,
                arguments: arguments.readable.clone(),
                actual: arguments.actual_arg_count,
                length: 0,
                start: 0,
                count: 0,
                items: arguments.actual_arg_count.saturating_sub(2) as u64,
                new_length: 0,
                cursor: 0,
                result: None,
                values: Vec::new(),
            },
        })
    }
}
impl SliceResume {
    fn argument(&self, index: usize) -> Value {
        self.arguments
            .get(index)
            .cloned()
            .unwrap_or(Value::Undefined)
    }
    fn result(&self) -> Result<ObjectRef, RuntimeError> {
        self.result
            .clone()
            .ok_or(RuntimeError::Invariant("Array slice result missing"))
    }
    pub(crate) fn resume(
        mut self,
        runtime: &Runtime,
        result: Completion,
    ) -> Result<SliceStep, RuntimeError> {
        let value = match result {
            Completion::Return(value) => value,
            Completion::Throw(value) => return Ok(SliceStep::Complete(Completion::Throw(value))),
        };
        match self.phase {
            Phase::Length => {
                self.phase = Phase::LengthNumber;
                Ok(SliceStep::Number {
                    value,
                    resume: self,
                })
            }
            Phase::Species => {
                let Value::Object(object) = value else {
                    return Err(RuntimeError::Invariant(
                        "ArraySpeciesCreate returned primitive",
                    ));
                };
                self.result = Some(object);
                self.collect(runtime)
            }
            Phase::Read => {
                if matches!(self.kind, SliceKind::ToSpliced) {
                    self.values[self.cursor as usize] = value;
                    self.cursor += 1;
                    return self.collect(runtime);
                }
                self.phase = Phase::Define;
                Ok(SliceStep::Define {
                    object: self.result()?,
                    key: runtime.property_key_for_index(self.cursor)?,
                    descriptor: OrdinaryPropertyDescriptor {
                        value: DescriptorField::Present(value),
                        writable: DescriptorField::Present(true),
                        enumerable: DescriptorField::Present(true),
                        configurable: DescriptorField::Present(true),
                        ..OrdinaryPropertyDescriptor::new()
                    },
                    resume: self,
                })
            }
            Phase::Copy => {
                self.cursor = self.length;
                self.delete_next(runtime)
            }
            _ => Err(RuntimeError::Invariant("Array slice value phase mismatch")),
        }
    }
    pub(crate) fn number(
        mut self,
        runtime: &Runtime,
        result: NativeConversion<f64>,
    ) -> Result<SliceStep, RuntimeError> {
        let number = match result {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => {
                return Ok(SliceStep::Complete(Completion::Throw(value)));
            }
        };
        match self.phase {
            Phase::LengthNumber => {
                self.length = Runtime::length_from_number(number);
                if matches!(self.kind, SliceKind::ToSpliced) && self.actual == 0 {
                    return self.end(runtime);
                }
                self.phase = Phase::Start;
                Ok(SliceStep::Number {
                    value: self.argument(0),
                    resume: self,
                })
            }
            Phase::Start => {
                let mut index = Runtime::int64_from_number(number);
                if index < 0 {
                    index += self.length as i64;
                }
                self.start = index.clamp(0, self.length as i64) as u64;
                self.end(runtime)
            }
            Phase::End => {
                let mut value = Runtime::int64_from_number(number);
                if matches!(self.kind, SliceKind::Slice) {
                    if value < 0 {
                        value += self.length as i64;
                    }
                    self.count =
                        (value.clamp(0, self.length as i64) as u64).saturating_sub(self.start);
                } else {
                    self.count = value.clamp(0, (self.length - self.start) as i64) as u64;
                }
                self.allocate(runtime)
            }
            _ => Err(RuntimeError::Invariant("Array slice number phase mismatch")),
        }
    }
    fn end(mut self, runtime: &Runtime) -> Result<SliceStep, RuntimeError> {
        let value = self.argument(1);
        let convert = match self.kind {
            SliceKind::Slice => self.actual > 1 && !matches!(value, Value::Undefined),
            _ => self.actual > 1,
        };
        if convert {
            self.phase = Phase::End;
            return Ok(SliceStep::Number {
                value,
                resume: self,
            });
        }
        self.count = if !matches!(self.kind, SliceKind::Slice) && self.actual == 0 {
            0
        } else {
            self.length - self.start
        };
        self.allocate(runtime)
    }
    fn allocate(mut self, runtime: &Runtime) -> Result<SliceStep, RuntimeError> {
        if !matches!(self.kind, SliceKind::Slice) {
            self.new_length = (self.length - self.count).saturating_add(self.items);
            if self.new_length > (1_u64 << 53) - 1 {
                return Ok(SliceStep::Complete(Completion::Throw(
                    runtime.new_native_error(
                        self.realm,
                        NativeErrorKind::Type,
                        if matches!(self.kind, SliceKind::ToSpliced) {
                            "invalid array length"
                        } else {
                            "Array loo long"
                        },
                    )?,
                )));
            }
        }
        if matches!(self.kind, SliceKind::ToSpliced) {
            self.values =
                match runtime.native_allocate_fast_array_values(self.realm, self.new_length)? {
                    NativeConversion::Value(value) => value,
                    NativeConversion::Throw(value) => {
                        return Ok(SliceStep::Complete(Completion::Throw(value)));
                    }
                };
            self.collect(runtime)
        } else {
            self.phase = Phase::Species;
            Ok(SliceStep::Species {
                source: self.object.clone(),
                length: self.count,
                resume: self,
            })
        }
    }
    fn source_index(&self) -> u64 {
        if matches!(self.kind, SliceKind::ToSpliced) {
            if self.cursor < self.start {
                self.cursor
            } else {
                self.cursor - self.items + self.count
            }
        } else {
            self.start + self.cursor
        }
    }
    fn collect(mut self, runtime: &Runtime) -> Result<SliceStep, RuntimeError> {
        if matches!(self.kind, SliceKind::ToSpliced) {
            if self.cursor == self.start {
                for index in 0..self.items {
                    self.values[(self.start + index) as usize] = self.argument(index as usize + 2);
                }
                self.cursor += self.items;
            }
            if self.cursor == self.new_length {
                return Ok(SliceStep::Complete(Completion::Return(Value::Object(
                    runtime.new_array_from_values(self.realm, self.values)?,
                ))));
            }
        } else if self.cursor == self.count {
            self.phase = Phase::ResultLength;
            return Ok(SliceStep::Set {
                object: self.result()?,
                key: runtime.intern_property_key("length")?,
                value: Value::number(self.count as f64),
                resume: self,
            });
        }
        self.phase = Phase::Has;
        Ok(SliceStep::Has {
            object: self.object.clone(),
            key: runtime.property_key_for_index(self.source_index())?,
            resume: self,
        })
    }
    pub(crate) fn boolean(
        mut self,
        runtime: &Runtime,
        result: NativeConversion<bool>,
    ) -> Result<SliceStep, RuntimeError> {
        let value = match result {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => {
                return Ok(SliceStep::Complete(Completion::Throw(value)));
            }
        };
        match self.phase {
            Phase::Has if value => {
                self.phase = Phase::Read;
                Ok(SliceStep::Read {
                    object: self.object.clone(),
                    key: runtime.property_key_for_index(self.source_index())?,
                    resume: self,
                })
            }
            Phase::Has => {
                self.cursor += 1;
                self.collect(runtime)
            }
            Phase::Delete => {
                if !value {
                    return Ok(SliceStep::Complete(Completion::Throw(
                        runtime.new_native_error(
                            self.realm,
                            NativeErrorKind::Type,
                            "could not delete property",
                        )?,
                    )));
                }
                self.delete_next(runtime)
            }
            _ => Err(RuntimeError::Invariant(
                "Array slice boolean phase mismatch",
            )),
        }
    }
    pub(crate) fn defined(
        mut self,
        runtime: &Runtime,
        result: NativeConversion<InternalDefineResult>,
    ) -> Result<SliceStep, RuntimeError> {
        if !matches!(self.phase, Phase::Define) {
            return Err(RuntimeError::Invariant("Array slice define phase mismatch"));
        }
        if let Some(value) =
            runtime.finish_create_indexed_data_property(self.realm, self.cursor, result)?
        {
            return Ok(SliceStep::Complete(Completion::Throw(value)));
        }
        self.cursor += 1;
        self.collect(runtime)
    }
    fn mutate(mut self, runtime: &Runtime) -> Result<SliceStep, RuntimeError> {
        if matches!(self.kind, SliceKind::Slice) {
            return self.complete();
        }
        if self.items != self.count {
            self.phase = Phase::Copy;
            return Ok(SliceStep::Copy {
                object: self.object.clone(),
                to: self.start + self.items,
                from: self.start + self.count,
                count: self.length - self.start - self.count,
                backwards: self.items > self.count,
                resume: self,
            });
        }
        self.cursor = 0;
        self.insert(runtime)
    }
    fn delete_next(mut self, runtime: &Runtime) -> Result<SliceStep, RuntimeError> {
        if self.cursor > self.new_length {
            self.cursor -= 1;
            self.phase = Phase::Delete;
            return Ok(SliceStep::Delete {
                object: self.object.clone(),
                key: runtime.property_key_for_index(self.cursor)?,
                resume: self,
            });
        }
        self.cursor = 0;
        self.insert(runtime)
    }
    fn insert(mut self, runtime: &Runtime) -> Result<SliceStep, RuntimeError> {
        if self.cursor < self.items {
            self.phase = Phase::Insert;
            return Ok(SliceStep::Set {
                object: self.object.clone(),
                key: runtime.property_key_for_index(self.start + self.cursor)?,
                value: self.argument(self.cursor as usize + 2),
                resume: self,
            });
        }
        self.phase = Phase::FinalLength;
        Ok(SliceStep::Set {
            object: self.object.clone(),
            key: runtime.intern_property_key("length")?,
            value: Value::number(self.new_length as f64),
            resume: self,
        })
    }
    pub(crate) fn set(
        mut self,
        runtime: &Runtime,
        key: PropertyKey,
        result: NativeConversion<InternalSetResult>,
    ) -> Result<SliceStep, RuntimeError> {
        if let Some(value) = runtime.finish_set_property_or_throw(self.realm, &key, result)? {
            return Ok(SliceStep::Complete(Completion::Throw(value)));
        }
        match self.phase {
            Phase::ResultLength => self.mutate(runtime),
            Phase::Insert => {
                self.cursor += 1;
                self.insert(runtime)
            }
            Phase::FinalLength => self.complete(),
            _ => Err(RuntimeError::Invariant("Array slice set phase mismatch")),
        }
    }
    fn complete(self) -> Result<SliceStep, RuntimeError> {
        Ok(SliceStep::Complete(Completion::Return(Value::Object(
            self.result()?,
        ))))
    }
}
pub(crate) fn finish(
    runtime: &Runtime,
    realm: ContextId,
    mut step: SliceStep,
) -> Result<Completion, RuntimeError> {
    loop {
        step = match step {
            SliceStep::Complete(result) => return Ok(result),
            SliceStep::Read {
                object,
                key,
                resume,
            } => resume.resume(
                runtime,
                runtime.get_property_in_realm(realm, &object, &key)?,
            )?,
            SliceStep::Number { value, resume } => {
                resume.number(runtime, runtime.native_to_number(realm, &value)?)?
            }
            SliceStep::Has {
                object,
                key,
                resume,
            } => resume.boolean(
                runtime,
                runtime.internal_has_property(realm, &object, &key)?,
            )?,
            SliceStep::Species {
                source,
                length,
                resume,
            } => resume.resume(
                runtime,
                super::species::finish(
                    runtime,
                    realm,
                    super::species::SpeciesStep::start(runtime, realm, &source, length)?,
                )?,
            )?,
            SliceStep::Define {
                object,
                key,
                descriptor,
                resume,
            } => resume.defined(
                runtime,
                runtime.internal_define_own_property(realm, &object, &key, &descriptor)?,
            )?,
            SliceStep::Set {
                object,
                key,
                value,
                resume,
            } => {
                let result = runtime.internal_set(
                    realm,
                    &object,
                    &key,
                    value,
                    Value::Object(object.clone()),
                )?;
                resume.set(runtime, key, result)?
            }
            SliceStep::Delete {
                object,
                key,
                resume,
            } => resume.boolean(
                runtime,
                runtime.internal_delete_property(realm, &object, &key)?,
            )?,
            SliceStep::Copy {
                object,
                to,
                from,
                count,
                backwards,
                resume,
            } => resume.resume(
                runtime,
                super::copy::finish(
                    runtime,
                    realm,
                    super::copy::CopyStep::start(
                        runtime, realm, object, to, from, count, backwards,
                    )?,
                )?,
            )?,
        };
    }
}
