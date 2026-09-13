//! Flatten owns nested source frames and commits each output before visiting the next element.
use super::{ARRAY_FLATTEN_FRAME_LIMIT, ArrayFlattenFrame};
use crate::engine::{
    api::{error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError},
    builtins::native::ArrayFlattenKind,
    heap::ContextId,
    object::{
        CallableRef, DescriptorField, ObjectRef, OrdinaryPropertyDescriptor, PropertyKey,
        operations::InternalDefineResult,
    },
    value::{Value, conversion::NativeConversion},
    vm::{
        Completion,
        call::{NativeArguments, NativeInvocation},
    },
};
pub(crate) enum FlattenStep {
    Complete(Completion),
    Read {
        object: ObjectRef,
        key: PropertyKey,
        resume: FlattenResume,
    },
    Number {
        value: Value,
        resume: FlattenResume,
    },
    Has {
        object: ObjectRef,
        key: PropertyKey,
        resume: FlattenResume,
    },
    Species {
        source: ObjectRef,
        resume: FlattenResume,
    },
    Call {
        callable: CallableRef,
        receiver: Value,
        arguments: Vec<Value>,
        resume: FlattenResume,
    },
    Define {
        object: ObjectRef,
        key: PropertyKey,
        descriptor: OrdinaryPropertyDescriptor,
        resume: FlattenResume,
    },
}
enum Phase {
    Length,
    LengthNumber,
    Depth,
    Species,
    Has,
    Read,
    Mapper,
    NestedLength,
    NestedNumber,
    Define,
}
pub(crate) struct FlattenResume {
    realm: ContextId,
    kind: ArrayFlattenKind,
    phase: Phase,
    source: ObjectRef,
    source_index: u64,
    source_length: u64,
    depth: i32,
    argument: Value,
    mapper: Option<CallableRef>,
    mapper_this: Value,
    target: Option<ObjectRef>,
    frames: Vec<ArrayFlattenFrame>,
    element: Value,
    target_index: u64,
    target_limit: u64,
    frame_limit: usize,
    return_count: bool,
}
impl FlattenStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        kind: ArrayFlattenKind,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        let NativeInvocation::Call { this_value } = invocation else {
            return Err(RuntimeError::Invariant(
                "Array flatten requires generic invocation",
            ));
        };
        let source = match runtime.native_to_object(realm, this_value.clone())? {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => return Ok(Self::Complete(Completion::Throw(value))),
        };
        Ok(Self::Read {
            object: source.clone(),
            key: runtime.intern_property_key("length")?,
            resume: FlattenResume {
                realm,
                kind,
                phase: Phase::Length,
                source,
                source_index: 0,
                source_length: 0,
                depth: 1,
                argument: arguments
                    .readable
                    .first()
                    .cloned()
                    .unwrap_or(Value::Undefined),
                mapper: None,
                mapper_this: if arguments.actual_arg_count > 1 {
                    arguments
                        .readable
                        .get(1)
                        .cloned()
                        .unwrap_or(Value::Undefined)
                } else {
                    Value::Undefined
                },
                target: None,
                frames: Vec::new(),
                element: Value::Undefined,
                target_index: 0,
                target_limit: (1_u64 << 53) - 1,
                frame_limit: ARRAY_FLATTEN_FRAME_LIMIT,
                return_count: false,
            },
        })
    }
    #[allow(clippy::too_many_arguments)]
    #[cfg(test)]
    pub(super) fn start_into(
        runtime: &Runtime,
        realm: ContextId,
        target: ObjectRef,
        source: ObjectRef,
        length: u64,
        depth: i32,
        mapper: Option<CallableRef>,
        mapper_this: Value,
        target_limit: u64,
        frame_limit: usize,
    ) -> Result<Self, RuntimeError> {
        FlattenResume {
            realm,
            kind: ArrayFlattenKind::Flat,
            phase: Phase::Species,
            source,
            source_index: 0,
            source_length: length,
            depth,
            argument: Value::Undefined,
            mapper,
            mapper_this,
            target: Some(target),
            frames: Vec::new(),
            element: Value::Undefined,
            target_index: 0,
            target_limit,
            frame_limit,
            return_count: true,
        }
        .begin(runtime)
    }
}
impl FlattenResume {
    pub(crate) fn resume(
        mut self,
        runtime: &Runtime,
        result: Completion,
    ) -> Result<FlattenStep, RuntimeError> {
        let value = match result {
            Completion::Return(value) => value,
            Completion::Throw(value) => return Ok(FlattenStep::Complete(Completion::Throw(value))),
        };
        match self.phase {
            Phase::Length => {
                self.phase = Phase::LengthNumber;
                Ok(FlattenStep::Number {
                    value,
                    resume: self,
                })
            }
            Phase::Species => {
                let Value::Object(target) = value else {
                    return Err(RuntimeError::Invariant(
                        "ArraySpeciesCreate returned a primitive for flatten",
                    ));
                };
                self.target = Some(target);
                self.begin(runtime)
            }
            Phase::Read => {
                let apply = self
                    .frames
                    .last()
                    .ok_or(RuntimeError::Invariant("flatten current frame missing"))?
                    .apply_mapper;
                if apply {
                    self.phase = Phase::Mapper;
                    return Ok(FlattenStep::Call {
                        callable: self
                            .mapper
                            .as_ref()
                            .ok_or(RuntimeError::Invariant("flatten mapper missing"))?
                            .clone(),
                        receiver: self.mapper_this.clone(),
                        arguments: vec![
                            value,
                            Value::number(self.source_index as f64),
                            Value::Object(self.source.clone()),
                        ],
                        resume: self,
                    });
                }
                self.visit(runtime, value)
            }
            Phase::Mapper => self.visit(runtime, value),
            Phase::NestedLength => {
                self.phase = Phase::NestedNumber;
                Ok(FlattenStep::Number {
                    value,
                    resume: self,
                })
            }
            _ => Err(RuntimeError::Invariant(
                "Array flatten value phase mismatch",
            )),
        }
    }
    pub(crate) fn number(
        mut self,
        runtime: &Runtime,
        result: NativeConversion<f64>,
    ) -> Result<FlattenStep, RuntimeError> {
        let number = match result {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => {
                return Ok(FlattenStep::Complete(Completion::Throw(value)));
            }
        };
        match self.phase {
            Phase::LengthNumber => {
                self.source_length = Runtime::length_from_number(number);
                if self.kind == ArrayFlattenKind::FlatMap {
                    self.mapper = Some(runtime.callable_from_value(self.argument.clone())?);
                } else if !matches!(self.argument, Value::Undefined) {
                    self.phase = Phase::Depth;
                    return Ok(FlattenStep::Number {
                        value: self.argument.clone(),
                        resume: self,
                    });
                }
                self.species()
            }
            Phase::Depth => {
                self.depth = crate::engine::value::number::to_int32_sat(number);
                self.species()
            }
            Phase::NestedNumber => {
                if self.frames.len() >= self.frame_limit {
                    return self.overflow(runtime);
                }
                let Value::Object(source) = std::mem::replace(&mut self.element, Value::Undefined)
                else {
                    return Err(RuntimeError::Invariant("flatten nested object missing"));
                };
                self.frames.push(ArrayFlattenFrame {
                    source,
                    length: Runtime::length_from_number(number),
                    next_index: 0,
                    depth: self.depth - 1,
                    apply_mapper: false,
                });
                self.next(runtime)
            }
            _ => Err(RuntimeError::Invariant(
                "Array flatten number phase mismatch",
            )),
        }
    }
    fn species(mut self) -> Result<FlattenStep, RuntimeError> {
        self.phase = Phase::Species;
        Ok(FlattenStep::Species {
            source: self.source.clone(),
            resume: self,
        })
    }
    fn overflow(&self, runtime: &Runtime) -> Result<FlattenStep, RuntimeError> {
        Ok(FlattenStep::Complete(Completion::Throw(
            runtime.new_native_error(self.realm, NativeErrorKind::Internal, "stack overflow")?,
        )))
    }
    fn begin(mut self, runtime: &Runtime) -> Result<FlattenStep, RuntimeError> {
        if self.frame_limit == 0 {
            return self.overflow(runtime);
        }
        self.frames.push(ArrayFlattenFrame {
            source: self.source.clone(),
            length: self.source_length,
            next_index: 0,
            depth: self.depth,
            apply_mapper: self.mapper.is_some(),
        });
        self.next(runtime)
    }
    fn next(mut self, runtime: &Runtime) -> Result<FlattenStep, RuntimeError> {
        loop {
            let Some(frame) = self.frames.last_mut() else {
                return Ok(FlattenStep::Complete(Completion::Return(
                    if self.return_count {
                        Value::number(self.target_index as f64)
                    } else {
                        Value::Object(
                            self.target
                                .ok_or(RuntimeError::Invariant("flatten target missing"))?,
                        )
                    },
                )));
            };
            if frame.next_index == frame.length {
                self.frames.pop();
                continue;
            }
            self.source = frame.source.clone();
            self.source_index = frame.next_index;
            frame.next_index += 1;
            self.depth = frame.depth;
            self.phase = Phase::Has;
            return Ok(FlattenStep::Has {
                object: self.source.clone(),
                key: runtime.property_key_for_index(self.source_index)?,
                resume: self,
            });
        }
    }
    pub(crate) fn boolean(
        mut self,
        runtime: &Runtime,
        result: NativeConversion<bool>,
    ) -> Result<FlattenStep, RuntimeError> {
        if !matches!(self.phase, Phase::Has) {
            return Err(RuntimeError::Invariant(
                "Array flatten boolean phase mismatch",
            ));
        }
        let value = match result {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => {
                return Ok(FlattenStep::Complete(Completion::Throw(value)));
            }
        };
        if !value {
            return self.next(runtime);
        }
        self.phase = Phase::Read;
        Ok(FlattenStep::Read {
            object: self.source.clone(),
            key: runtime.property_key_for_index(self.source_index)?,
            resume: self,
        })
    }
    fn visit(mut self, runtime: &Runtime, element: Value) -> Result<FlattenStep, RuntimeError> {
        let flatten = if self.depth > 0 {
            match runtime.internal_is_array(self.realm, &element)? {
                NativeConversion::Value(value) => value,
                NativeConversion::Throw(value) => {
                    return Ok(FlattenStep::Complete(Completion::Throw(value)));
                }
            }
        } else {
            false
        };
        if flatten {
            let Value::Object(object) = &element else {
                return Err(RuntimeError::Invariant(
                    "IsArray accepted primitive flatten element",
                ));
            };
            let object = object.clone();
            self.element = element;
            self.phase = Phase::NestedLength;
            return Ok(FlattenStep::Read {
                object,
                key: runtime.intern_property_key("length")?,
                resume: self,
            });
        }
        if self.target_index >= self.target_limit {
            return Ok(FlattenStep::Complete(Completion::Throw(
                runtime.new_native_error(self.realm, NativeErrorKind::Type, "Array too long")?,
            )));
        }
        self.phase = Phase::Define;
        Ok(FlattenStep::Define {
            object: self
                .target
                .clone()
                .ok_or(RuntimeError::Invariant("flatten target missing"))?,
            key: runtime.property_key_for_index(self.target_index)?,
            descriptor: OrdinaryPropertyDescriptor {
                value: DescriptorField::Present(element),
                writable: DescriptorField::Present(true),
                enumerable: DescriptorField::Present(true),
                configurable: DescriptorField::Present(true),
                ..OrdinaryPropertyDescriptor::new()
            },
            resume: self,
        })
    }
    pub(crate) fn defined(
        mut self,
        runtime: &Runtime,
        result: NativeConversion<InternalDefineResult>,
    ) -> Result<FlattenStep, RuntimeError> {
        if !matches!(self.phase, Phase::Define) {
            return Err(RuntimeError::Invariant(
                "Array flatten define phase mismatch",
            ));
        }
        if let Some(value) =
            runtime.finish_create_indexed_data_property(self.realm, self.target_index, result)?
        {
            return Ok(FlattenStep::Complete(Completion::Throw(value)));
        }
        self.target_index += 1;
        self.next(runtime)
    }
}
pub(crate) fn finish(
    runtime: &Runtime,
    realm: ContextId,
    mut step: FlattenStep,
) -> Result<Completion, RuntimeError> {
    loop {
        step = match step {
            FlattenStep::Complete(result) => return Ok(result),
            FlattenStep::Read {
                object,
                key,
                resume,
            } => resume.resume(
                runtime,
                runtime.get_property_in_realm(realm, &object, &key)?,
            )?,
            FlattenStep::Number { value, resume } => {
                resume.number(runtime, runtime.native_to_number(realm, &value)?)?
            }
            FlattenStep::Has {
                object,
                key,
                resume,
            } => resume.boolean(
                runtime,
                runtime.internal_has_property(realm, &object, &key)?,
            )?,
            FlattenStep::Species { source, resume } => resume.resume(
                runtime,
                super::species::finish(
                    runtime,
                    realm,
                    super::species::SpeciesStep::start(runtime, realm, &source, 0)?,
                )?,
            )?,
            FlattenStep::Call {
                callable,
                receiver,
                arguments,
                resume,
            } => resume.resume(
                runtime,
                runtime.call_internal(realm, &callable, receiver, &arguments)?,
            )?,
            FlattenStep::Define {
                object,
                key,
                descriptor,
                resume,
            } => resume.defined(
                runtime,
                runtime.internal_define_own_property(realm, &object, &key, &descriptor)?,
            )?,
        };
    }
}
