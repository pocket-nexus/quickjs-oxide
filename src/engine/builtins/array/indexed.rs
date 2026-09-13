//! Indexed Array algorithms expose property and numeric conversion boundaries.
#[cfg(feature = "stack-vm")]
use crate::engine::builtins::native::NativeFunctionId;
use crate::engine::{
    api::{error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError},
    builtins::native::ArraySearchKind,
    heap::ContextId,
    object::{ObjectRef, PropertyKey, operations::InternalSetResult},
    value::{Value, conversion::NativeConversion},
    vm::{
        Completion,
        call::{NativeArguments, NativeInvocation},
    },
};
#[derive(Clone, Copy)]
pub(crate) enum IndexedKind {
    At,
    With,
    Fill,
    CopyWithin,
    Search(ArraySearchKind),
    ToReversed,
}
impl IndexedKind {
    #[cfg(feature = "stack-vm")]
    pub(crate) fn for_target(target: NativeFunctionId) -> Option<Self> {
        Some(match target {
            NativeFunctionId::ArrayPrototypeAt => Self::At,
            NativeFunctionId::ArrayPrototypeWith => Self::With,
            NativeFunctionId::ArrayPrototypeFill => Self::Fill,
            NativeFunctionId::ArrayPrototypeCopyWithin => Self::CopyWithin,
            NativeFunctionId::ArrayPrototypeSearch(kind) => Self::Search(kind),
            NativeFunctionId::ArrayPrototypeToReversed => Self::ToReversed,
            _ => return None,
        })
    }
}
pub(crate) enum IndexedStep {
    Complete(Completion),
    Copy {
        object: ObjectRef,
        to: u64,
        from: u64,
        count: u64,
        backwards: bool,
        resume: IndexedResume,
    },
    Read {
        object: ObjectRef,
        key: PropertyKey,
        resume: IndexedResume,
    },
    Number {
        value: Value,
        resume: IndexedResume,
    },
    Has {
        object: ObjectRef,
        key: PropertyKey,
        resume: IndexedResume,
    },
    Set {
        object: ObjectRef,
        key: PropertyKey,
        value: Value,
        resume: IndexedResume,
    },
}
enum Phase {
    Length,
    LengthNumber,
    Bound(usize),
    Has,
    Read,
    Write,
    Copy,
}
pub(crate) struct IndexedResume {
    realm: ContextId,
    kind: IndexedKind,
    object: ObjectRef,
    arguments: Vec<Value>,
    actual: usize,
    phase: Phase,
    length: i64,
    bounds: [i64; 3],
    index: i64,
    end: i64,
    direction: i64,
    values: Vec<Value>,
    replacement: i64,
}
impl IndexedStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        kind: IndexedKind,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        let NativeInvocation::Call { this_value } = invocation else {
            return Err(RuntimeError::Invariant(
                "Array indexed method requires generic invocation",
            ));
        };
        let object = match runtime.native_to_object(realm, this_value.clone())? {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => return Ok(Self::Complete(Completion::Throw(value))),
        };
        Ok(Self::Read {
            object: object.clone(),
            key: runtime.intern_property_key("length")?,
            resume: IndexedResume {
                realm,
                kind,
                object,
                arguments: arguments.readable.clone(),
                actual: arguments.actual_arg_count,
                phase: Phase::Length,
                length: 0,
                bounds: [0; 3],
                index: 0,
                end: 0,
                direction: 1,
                values: Vec::new(),
                replacement: -1,
            },
        })
    }
}
impl IndexedResume {
    fn argument(&self, index: usize) -> Value {
        self.arguments
            .get(index)
            .cloned()
            .unwrap_or(Value::Undefined)
    }
    pub(crate) fn resume(
        mut self,
        runtime: &Runtime,
        result: Completion,
    ) -> Result<IndexedStep, RuntimeError> {
        let value = match result {
            Completion::Return(value) => value,
            Completion::Throw(value) => return Ok(IndexedStep::Complete(Completion::Throw(value))),
        };
        match self.phase {
            Phase::Length => {
                self.phase = Phase::LengthNumber;
                Ok(IndexedStep::Number {
                    value,
                    resume: self,
                })
            }
            Phase::Read => match self.kind {
                IndexedKind::At => Ok(IndexedStep::Complete(Completion::Return(value))),
                IndexedKind::With | IndexedKind::ToReversed => {
                    let output = if matches!(self.kind, IndexedKind::ToReversed) {
                        self.length - self.index - 1
                    } else {
                        self.index
                    };
                    self.values[output as usize] = value;
                    self.advance(runtime)
                }
                IndexedKind::Search(kind) => {
                    let search = self.argument(0);
                    let found = if kind == ArraySearchKind::Includes {
                        search.same_value_zero(&value)
                    } else {
                        search.strict_equal(&value)
                    };
                    if found {
                        Ok(IndexedStep::Complete(Completion::Return(
                            if kind == ArraySearchKind::Includes {
                                Value::Bool(true)
                            } else {
                                Value::number(self.index as f64)
                            },
                        )))
                    } else {
                        self.advance(runtime)
                    }
                }
                _ => Err(RuntimeError::Invariant("Array indexed read kind mismatch")),
            },
            Phase::Copy => self.complete(runtime),
            _ => Err(RuntimeError::Invariant(
                "Array indexed value phase mismatch",
            )),
        }
    }
    pub(crate) fn number(
        mut self,
        runtime: &Runtime,
        result: NativeConversion<f64>,
    ) -> Result<IndexedStep, RuntimeError> {
        let number = match result {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => {
                return Ok(IndexedStep::Complete(Completion::Throw(value)));
            }
        };
        match self.phase {
            Phase::LengthNumber => {
                self.length = Runtime::length_from_number(number) as i64;
                self.bounds[2] = self.length;
                if matches!(self.kind, IndexedKind::Search(_)) && self.length == 0 {
                    return self.complete(runtime);
                }
                self.bound(runtime, 0)
            }
            Phase::Bound(index) => {
                let mut value = Runtime::int64_from_number(number);
                if !matches!(self.kind, IndexedKind::At | IndexedKind::With) {
                    if value < 0 {
                        value += self.length;
                    }
                    value =
                        if matches!(self.kind, IndexedKind::Search(ArraySearchKind::LastIndexOf)) {
                            value.clamp(-1, self.length - 1)
                        } else {
                            value.clamp(0, self.length)
                        };
                }
                self.bounds[index] = value;
                self.bound(runtime, index + 1)
            }
            _ => Err(RuntimeError::Invariant(
                "Array indexed number phase mismatch",
            )),
        }
    }
    fn bound(mut self, runtime: &Runtime, index: usize) -> Result<IndexedStep, RuntimeError> {
        let argument = match self.kind {
            IndexedKind::At | IndexedKind::With if index == 0 => Some(0),
            IndexedKind::Fill if index < 2 => Some(index + 1),
            IndexedKind::CopyWithin if index < 3 => Some(index),
            IndexedKind::Search(_) if index == 0 => Some(1),
            _ => None,
        };
        if let Some(argument) = argument {
            let value = self.argument(argument);
            let omitted = match self.kind {
                IndexedKind::Fill => argument >= self.actual || matches!(value, Value::Undefined),
                IndexedKind::CopyWithin if argument == 2 => {
                    argument >= self.actual || matches!(value, Value::Undefined)
                }
                IndexedKind::Search(_) => argument >= self.actual,
                _ => false,
            };
            if omitted {
                self.bounds[index] = match self.kind {
                    IndexedKind::Fill if index == 1 => self.length,
                    IndexedKind::CopyWithin => self.length,
                    IndexedKind::Search(ArraySearchKind::LastIndexOf) => self.length - 1,
                    _ => 0,
                };
                return self.bound(runtime, index + 1);
            }
            self.phase = Phase::Bound(index);
            return Ok(IndexedStep::Number {
                value,
                resume: self,
            });
        }
        self.index = self.bounds[0];
        self.end = self.length;
        match self.kind {
            IndexedKind::At | IndexedKind::With => {
                if self.index < 0 {
                    self.index += self.length;
                }
                if self.index < 0 || self.index >= self.length {
                    return Ok(IndexedStep::Complete(
                        if matches!(self.kind, IndexedKind::At) {
                            Completion::Return(Value::Undefined)
                        } else {
                            Completion::Throw(runtime.new_native_error(
                                self.realm,
                                NativeErrorKind::Range,
                                &format!("invalid array index: {}", self.index),
                            )?)
                        },
                    ));
                }
                if matches!(self.kind, IndexedKind::With) {
                    self.replacement = self.index;
                    self.index = 0;
                    if let Some(value) = self.allocate(runtime)? {
                        return Ok(IndexedStep::Complete(Completion::Throw(value)));
                    }
                }
            }
            IndexedKind::Fill => self.end = self.bounds[1].max(self.index),
            IndexedKind::CopyWithin => {
                let to = self.bounds[0];
                let from = self.bounds[1];
                let count = (self.bounds[2] - from).min(self.length - to).max(0);
                self.phase = Phase::Copy;
                return Ok(IndexedStep::Copy {
                    object: self.object.clone(),
                    to: to as u64,
                    from: from as u64,
                    count: count as u64,
                    backwards: from < to && to < from + count,
                    resume: self,
                });
            }
            IndexedKind::Search(ArraySearchKind::LastIndexOf) => {
                self.end = -1;
                self.direction = -1;
            }
            IndexedKind::ToReversed => {
                if let Some(value) = self.allocate(runtime)? {
                    return Ok(IndexedStep::Complete(Completion::Throw(value)));
                }
                self.index = self.length - 1;
                self.end = -1;
                self.direction = -1;
            }
            _ => {}
        }
        self.next(runtime)
    }
    fn allocate(&mut self, runtime: &Runtime) -> Result<Option<Value>, RuntimeError> {
        match runtime.native_allocate_fast_array_values(self.realm, self.length as u64)? {
            NativeConversion::Value(values) => {
                self.values = values;
                Ok(None)
            }
            NativeConversion::Throw(value) => Ok(Some(value)),
        }
    }
    fn advance(mut self, runtime: &Runtime) -> Result<IndexedStep, RuntimeError> {
        self.index += self.direction;
        self.next(runtime)
    }
    fn next(mut self, runtime: &Runtime) -> Result<IndexedStep, RuntimeError> {
        loop {
            if self.index == self.end {
                return self.complete(runtime);
            }
            if matches!(self.kind, IndexedKind::With) && self.index == self.replacement {
                self.values[self.index as usize] = self.argument(1);
                self.index += 1;
                continue;
            }
            let key = runtime.property_key_for_index(self.index as u64)?;
            if matches!(self.kind, IndexedKind::Fill) {
                self.phase = Phase::Write;
                return Ok(IndexedStep::Set {
                    object: self.object.clone(),
                    key,
                    value: self.argument(0),
                    resume: self,
                });
            }
            if matches!(self.kind, IndexedKind::Search(ArraySearchKind::Includes)) {
                self.phase = Phase::Read;
                return Ok(IndexedStep::Read {
                    object: self.object.clone(),
                    key,
                    resume: self,
                });
            }
            self.phase = Phase::Has;
            return Ok(IndexedStep::Has {
                object: self.object.clone(),
                key,
                resume: self,
            });
        }
    }
    fn complete(self, runtime: &Runtime) -> Result<IndexedStep, RuntimeError> {
        let value = match self.kind {
            IndexedKind::With | IndexedKind::ToReversed => {
                Value::Object(runtime.new_array_from_values(self.realm, self.values)?)
            }
            IndexedKind::Search(ArraySearchKind::Includes) => Value::Bool(false),
            IndexedKind::Search(_) => Value::Int(-1),
            IndexedKind::At => Value::Undefined,
            _ => Value::Object(self.object),
        };
        Ok(IndexedStep::Complete(Completion::Return(value)))
    }
    pub(crate) fn boolean(
        mut self,
        runtime: &Runtime,
        result: NativeConversion<bool>,
    ) -> Result<IndexedStep, RuntimeError> {
        let value = match result {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => {
                return Ok(IndexedStep::Complete(Completion::Throw(value)));
            }
        };
        match self.phase {
            Phase::Has => {
                if value {
                    self.phase = Phase::Read;
                    return Ok(IndexedStep::Read {
                        object: self.object.clone(),
                        key: runtime.property_key_for_index(self.index as u64)?,
                        resume: self,
                    });
                }
                if matches!(self.kind, IndexedKind::At) {
                    return self.complete(runtime);
                }
                self.advance(runtime)
            }
            _ => Err(RuntimeError::Invariant(
                "Array indexed boolean phase mismatch",
            )),
        }
    }
    pub(crate) fn set(
        self,
        runtime: &Runtime,
        key: PropertyKey,
        result: NativeConversion<InternalSetResult>,
    ) -> Result<IndexedStep, RuntimeError> {
        if !matches!(self.phase, Phase::Write) {
            return Err(RuntimeError::Invariant("Array indexed set phase mismatch"));
        }
        if let Some(value) = runtime.finish_set_property_or_throw(self.realm, &key, result)? {
            return Ok(IndexedStep::Complete(Completion::Throw(value)));
        }
        self.advance(runtime)
    }
}
pub(crate) fn finish(
    runtime: &Runtime,
    realm: ContextId,
    mut step: IndexedStep,
) -> Result<Completion, RuntimeError> {
    loop {
        step = match step {
            IndexedStep::Complete(result) => return Ok(result),
            IndexedStep::Copy {
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
            IndexedStep::Read {
                object,
                key,
                resume,
            } => resume.resume(
                runtime,
                runtime.get_property_in_realm(realm, &object, &key)?,
            )?,
            IndexedStep::Number { value, resume } => {
                resume.number(runtime, runtime.native_to_number(realm, &value)?)?
            }
            IndexedStep::Has {
                object,
                key,
                resume,
            } => resume.boolean(
                runtime,
                runtime.internal_has_property(realm, &object, &key)?,
            )?,
            IndexedStep::Set {
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
