//! Concat preserves species selection and per-element spreadability/property order.
use crate::engine::{
    api::{error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError},
    heap::ContextId,
    object::{
        DescriptorField, ObjectRef, OrdinaryPropertyDescriptor, PropertyKey, WellKnownSymbol,
        operations::{InternalDefineResult, InternalSetResult},
    },
    value::{Value, conversion::NativeConversion},
    vm::{
        Completion,
        call::{NativeArguments, NativeInvocation},
    },
};
pub(crate) enum ConcatStep {
    Complete(Completion),
    Species {
        source: ObjectRef,
        resume: ConcatResume,
    },
    Read {
        object: ObjectRef,
        key: PropertyKey,
        resume: ConcatResume,
    },
    Number {
        value: Value,
        resume: ConcatResume,
    },
    Has {
        object: ObjectRef,
        key: PropertyKey,
        resume: ConcatResume,
    },
    Define {
        object: ObjectRef,
        key: PropertyKey,
        descriptor: OrdinaryPropertyDescriptor,
        resume: ConcatResume,
    },
    Set {
        object: ObjectRef,
        key: PropertyKey,
        value: Value,
        resume: ConcatResume,
    },
}
enum Phase {
    Species,
    Spread,
    Length,
    Number,
    Has,
    Read,
    Define(bool),
    Set,
}
pub(crate) struct ConcatResume {
    realm: ContextId,
    phase: Phase,
    result: Option<ObjectRef>,
    elements: std::vec::IntoIter<Value>,
    element: Value,
    next_index: u64,
    index: u64,
    length: u64,
}
impl ConcatStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        let NativeInvocation::Call { this_value } = invocation else {
            return Err(RuntimeError::Invariant(
                "Array concat requires generic invocation",
            ));
        };
        let source = match runtime.native_to_object(realm, this_value.clone())? {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => return Ok(Self::Complete(Completion::Throw(value))),
        };
        let mut elements = Vec::with_capacity(arguments.actual_arg_count + 1);
        elements.push(Value::Object(source.clone()));
        elements.extend(
            arguments.readable[..arguments.actual_arg_count]
                .iter()
                .cloned(),
        );
        Ok(Self::Species {
            source,
            resume: ConcatResume {
                realm,
                phase: Phase::Species,
                result: None,
                elements: elements.into_iter(),
                element: Value::Undefined,
                next_index: 0,
                index: 0,
                length: 0,
            },
        })
    }
}
impl ConcatResume {
    fn object(&self) -> Result<ObjectRef, RuntimeError> {
        match &self.element {
            Value::Object(object) => Ok(object.clone()),
            _ => Err(RuntimeError::Invariant(
                "Array concat spread source missing",
            )),
        }
    }
    fn result(&self) -> Result<ObjectRef, RuntimeError> {
        self.result
            .clone()
            .ok_or(RuntimeError::Invariant("Array concat result missing"))
    }
    fn too_long(&self, runtime: &Runtime) -> Result<ConcatStep, RuntimeError> {
        Ok(ConcatStep::Complete(Completion::Throw(
            runtime.new_native_error(self.realm, NativeErrorKind::Type, "Array loo long")?,
        )))
    }
    pub(crate) fn resume(
        mut self,
        runtime: &Runtime,
        result: Completion,
    ) -> Result<ConcatStep, RuntimeError> {
        let value = match result {
            Completion::Return(value) => value,
            Completion::Throw(value) => return Ok(ConcatStep::Complete(Completion::Throw(value))),
        };
        match self.phase {
            Phase::Species => {
                let Value::Object(object) = value else {
                    return Err(RuntimeError::Invariant(
                        "ArraySpeciesCreate returned a primitive",
                    ));
                };
                self.result = Some(object);
                self.next(runtime)
            }
            Phase::Spread => {
                let spread = if matches!(value, Value::Undefined) {
                    match runtime.internal_is_array(self.realm, &self.element)? {
                        NativeConversion::Value(value) => value,
                        NativeConversion::Throw(value) => {
                            return Ok(ConcatStep::Complete(Completion::Throw(value)));
                        }
                    }
                } else {
                    runtime.value_to_boolean(&value)?
                };
                if spread {
                    self.phase = Phase::Length;
                    Ok(ConcatStep::Read {
                        object: self.object()?,
                        key: runtime.intern_property_key("length")?,
                        resume: self,
                    })
                } else {
                    self.single(runtime)
                }
            }
            Phase::Length => {
                self.phase = Phase::Number;
                Ok(ConcatStep::Number {
                    value,
                    resume: self,
                })
            }
            Phase::Read => self.define(runtime, value, true),
            _ => Err(RuntimeError::Invariant("Array concat value phase mismatch")),
        }
    }
    fn next(mut self, runtime: &Runtime) -> Result<ConcatStep, RuntimeError> {
        let Some(element) = self.elements.next() else {
            self.phase = Phase::Set;
            return Ok(ConcatStep::Set {
                object: self.result()?,
                key: runtime.intern_property_key("length")?,
                value: Value::number(self.next_index as f64),
                resume: self,
            });
        };
        self.element = element;
        self.index = 0;
        if let Value::Object(object) = &self.element {
            let object = object.clone();
            self.phase = Phase::Spread;
            Ok(ConcatStep::Read {
                object,
                key: PropertyKey::from(
                    runtime.well_known_symbol(WellKnownSymbol::IsConcatSpreadable),
                ),
                resume: self,
            })
        } else {
            self.single(runtime)
        }
    }
    fn single(self, runtime: &Runtime) -> Result<ConcatStep, RuntimeError> {
        if self.next_index >= (1_u64 << 53) - 1 {
            return self.too_long(runtime);
        }
        let value = self.element.clone();
        self.define(runtime, value, false)
    }
    pub(crate) fn number(
        mut self,
        runtime: &Runtime,
        result: NativeConversion<f64>,
    ) -> Result<ConcatStep, RuntimeError> {
        if !matches!(self.phase, Phase::Number) {
            return Err(RuntimeError::Invariant(
                "Array concat number phase mismatch",
            ));
        }
        self.length = match result {
            NativeConversion::Value(value) => Runtime::length_from_number(value),
            NativeConversion::Throw(value) => {
                return Ok(ConcatStep::Complete(Completion::Throw(value)));
            }
        };
        if self.next_index.saturating_add(self.length) > (1_u64 << 53) - 1 {
            return self.too_long(runtime);
        }
        self.indexed(runtime)
    }
    fn indexed(mut self, runtime: &Runtime) -> Result<ConcatStep, RuntimeError> {
        if self.index == self.length {
            return self.next(runtime);
        }
        self.phase = Phase::Has;
        Ok(ConcatStep::Has {
            object: self.object()?,
            key: runtime.property_key_for_index(self.index)?,
            resume: self,
        })
    }
    pub(crate) fn boolean(
        mut self,
        runtime: &Runtime,
        result: NativeConversion<bool>,
    ) -> Result<ConcatStep, RuntimeError> {
        if !matches!(self.phase, Phase::Has) {
            return Err(RuntimeError::Invariant(
                "Array concat boolean phase mismatch",
            ));
        }
        let value = match result {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => {
                return Ok(ConcatStep::Complete(Completion::Throw(value)));
            }
        };
        if value {
            self.phase = Phase::Read;
            Ok(ConcatStep::Read {
                object: self.object()?,
                key: runtime.property_key_for_index(self.index)?,
                resume: self,
            })
        } else {
            self.index += 1;
            self.next_index += 1;
            self.indexed(runtime)
        }
    }
    fn define(
        mut self,
        runtime: &Runtime,
        value: Value,
        indexed: bool,
    ) -> Result<ConcatStep, RuntimeError> {
        self.phase = Phase::Define(indexed);
        Ok(ConcatStep::Define {
            object: self.result()?,
            key: runtime.property_key_for_index(self.next_index)?,
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
    pub(crate) fn defined(
        mut self,
        runtime: &Runtime,
        result: NativeConversion<InternalDefineResult>,
    ) -> Result<ConcatStep, RuntimeError> {
        let Phase::Define(indexed) = self.phase else {
            return Err(RuntimeError::Invariant(
                "Array concat define phase mismatch",
            ));
        };
        if let Some(value) =
            runtime.finish_create_indexed_data_property(self.realm, self.next_index, result)?
        {
            return Ok(ConcatStep::Complete(Completion::Throw(value)));
        }
        self.next_index += 1;
        if indexed {
            self.index += 1;
            self.indexed(runtime)
        } else {
            self.next(runtime)
        }
    }
    pub(crate) fn set(
        self,
        runtime: &Runtime,
        key: PropertyKey,
        result: NativeConversion<InternalSetResult>,
    ) -> Result<ConcatStep, RuntimeError> {
        if !matches!(self.phase, Phase::Set) {
            return Err(RuntimeError::Invariant("Array concat set phase mismatch"));
        }
        if let Some(value) = runtime.finish_set_property_or_throw(self.realm, &key, result)? {
            return Ok(ConcatStep::Complete(Completion::Throw(value)));
        }
        Ok(ConcatStep::Complete(Completion::Return(Value::Object(
            self.result()?,
        ))))
    }
}
pub(crate) fn finish(
    runtime: &Runtime,
    realm: ContextId,
    mut step: ConcatStep,
) -> Result<Completion, RuntimeError> {
    loop {
        step = match step {
            ConcatStep::Complete(result) => return Ok(result),
            ConcatStep::Species { source, resume } => resume.resume(
                runtime,
                super::species::finish(
                    runtime,
                    realm,
                    super::species::SpeciesStep::start(runtime, realm, &source, 0)?,
                )?,
            )?,
            ConcatStep::Read {
                object,
                key,
                resume,
            } => resume.resume(
                runtime,
                runtime.get_property_in_realm(realm, &object, &key)?,
            )?,
            ConcatStep::Number { value, resume } => {
                resume.number(runtime, runtime.native_to_number(realm, &value)?)?
            }
            ConcatStep::Has {
                object,
                key,
                resume,
            } => resume.boolean(
                runtime,
                runtime.internal_has_property(realm, &object, &key)?,
            )?,
            ConcatStep::Define {
                object,
                key,
                descriptor,
                resume,
            } => resume.defined(
                runtime,
                runtime.internal_define_own_property(realm, &object, &key, &descriptor)?,
            )?,
            ConcatStep::Set {
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
        };
    }
}
