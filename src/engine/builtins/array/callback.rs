//! Array callback loops retain the captured length and reread each observable property.
#[cfg(feature = "stack-vm")]
use crate::engine::builtins::native::NativeFunctionId;
use crate::engine::{
    api::{error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError},
    builtins::native::{ArrayFindKind, ArrayIterationKind, ArrayReduceKind},
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
#[derive(Clone, Copy)]
pub(crate) enum CallbackKind {
    Iteration(ArrayIterationKind),
    Reduce(ArrayReduceKind),
    Find(ArrayFindKind),
}
impl CallbackKind {
    #[cfg(feature = "stack-vm")]
    pub(crate) fn for_target(target: NativeFunctionId) -> Option<Self> {
        match target {
            NativeFunctionId::ArrayPrototypeIteration(kind) => Some(Self::Iteration(kind)),
            NativeFunctionId::ArrayPrototypeReduce(kind) => Some(Self::Reduce(kind)),
            NativeFunctionId::ArrayPrototypeFind(kind) => Some(Self::Find(kind)),
            _ => None,
        }
    }
}
pub(crate) enum CallbackStep {
    Complete(Completion),
    Read {
        object: ObjectRef,
        key: PropertyKey,
        resume: CallbackResume,
    },
    Number {
        value: Value,
        resume: CallbackResume,
    },
    Has {
        object: ObjectRef,
        key: PropertyKey,
        resume: CallbackResume,
    },
    Call {
        callable: CallableRef,
        receiver: Value,
        arguments: Vec<Value>,
        resume: CallbackResume,
    },
    Species {
        source: ObjectRef,
        length: u64,
        resume: CallbackResume,
    },
    Define {
        object: ObjectRef,
        key: PropertyKey,
        descriptor: OrdinaryPropertyDescriptor,
        resume: CallbackResume,
    },
}
enum Phase {
    Length,
    Number,
    Species,
    Has,
    Read,
    Callback,
    Define,
}
pub(crate) struct CallbackResume {
    realm: ContextId,
    kind: CallbackKind,
    object: ObjectRef,
    original: Value,
    callback_value: Value,
    callback: Option<CallableRef>,
    this_arg: Value,
    accumulator: Option<Value>,
    result: Value,
    value: Value,
    phase: Phase,
    length: u64,
    cursor: u64,
    selected: u64,
}
impl CallbackStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        kind: CallbackKind,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        let NativeInvocation::Call { this_value } = invocation else {
            return Err(RuntimeError::Invariant(
                "Array callback requires generic invocation",
            ));
        };
        let object = match runtime.native_to_object(realm, this_value.clone())? {
            NativeConversion::Value(object) => object,
            NativeConversion::Throw(value) => return Ok(Self::Complete(Completion::Throw(value))),
        };
        let callback_value = arguments
            .readable
            .first()
            .ok_or(RuntimeError::Invariant(
                "Array callback argv was not padded",
            ))?
            .clone();
        let second = if arguments.actual_arg_count > 1 {
            Some(
                arguments
                    .readable
                    .get(1)
                    .ok_or(RuntimeError::Invariant(
                        "Array callback second argument missing",
                    ))?
                    .clone(),
            )
        } else {
            None
        };
        Ok(Self::Read {
            object: object.clone(),
            key: runtime.intern_property_key("length")?,
            resume: CallbackResume {
                realm,
                kind,
                object,
                original: this_value.clone(),
                callback_value,
                callback: None,
                this_arg: second.clone().unwrap_or(Value::Undefined),
                accumulator: if matches!(kind, CallbackKind::Reduce(_)) {
                    second
                } else {
                    None
                },
                result: Value::Undefined,
                value: Value::Undefined,
                phase: Phase::Length,
                length: 0,
                cursor: 0,
                selected: 0,
            },
        })
    }
}
impl CallbackResume {
    fn index(&self) -> u64 {
        match self.kind {
            CallbackKind::Reduce(ArrayReduceKind::ReduceRight)
            | CallbackKind::Find(ArrayFindKind::FindLast | ArrayFindKind::FindLastIndex) => {
                self.length - self.cursor - 1
            }
            _ => self.cursor,
        }
    }
    pub(crate) fn resume(
        mut self,
        runtime: &Runtime,
        result: Completion,
    ) -> Result<CallbackStep, RuntimeError> {
        let value = match result {
            Completion::Return(value) => value,
            Completion::Throw(value) => {
                return Ok(CallbackStep::Complete(Completion::Throw(value)));
            }
        };
        match self.phase {
            Phase::Length => {
                self.phase = Phase::Number;
                Ok(CallbackStep::Number {
                    value,
                    resume: self,
                })
            }
            Phase::Species => {
                if !matches!(value, Value::Object(_)) {
                    return Err(RuntimeError::Invariant(
                        "ArraySpeciesCreate returned a primitive",
                    ));
                }
                self.result = value;
                self.next(runtime)
            }
            Phase::Read => {
                if matches!(self.kind, CallbackKind::Reduce(_)) && self.accumulator.is_none() {
                    self.accumulator = Some(value);
                    self.cursor += 1;
                    return self.next(runtime);
                }
                let index = Value::number(self.index() as f64);
                let arguments = if let CallbackKind::Reduce(_) = self.kind {
                    vec![
                        self.accumulator
                            .take()
                            .ok_or(RuntimeError::Invariant("Array reduce accumulator missing"))?,
                        value.clone(),
                        index,
                        Value::Object(self.object.clone()),
                    ]
                } else {
                    vec![
                        value.clone(),
                        index,
                        if matches!(self.kind, CallbackKind::Find(_)) {
                            self.original.clone()
                        } else {
                            Value::Object(self.object.clone())
                        },
                    ]
                };
                self.value = value;
                self.phase = Phase::Callback;
                Ok(CallbackStep::Call {
                    callable: self
                        .callback
                        .as_ref()
                        .ok_or(RuntimeError::Invariant("Array callback missing"))?
                        .clone(),
                    receiver: if matches!(self.kind, CallbackKind::Reduce(_)) {
                        Value::Undefined
                    } else {
                        self.this_arg.clone()
                    },
                    arguments,
                    resume: self,
                })
            }
            Phase::Callback => {
                match self.kind {
                    CallbackKind::Reduce(_) => self.accumulator = Some(value),
                    CallbackKind::Find(kind) => {
                        if runtime.value_to_boolean(&value)? {
                            return Ok(CallbackStep::Complete(Completion::Return(match kind {
                                ArrayFindKind::Find | ArrayFindKind::FindLast => self.value,
                                _ => Value::number(self.index() as f64),
                            })));
                        }
                    }
                    CallbackKind::Iteration(kind) => match kind {
                        ArrayIterationKind::Every if !runtime.value_to_boolean(&value)? => {
                            return Ok(CallbackStep::Complete(Completion::Return(Value::Bool(
                                false,
                            ))));
                        }
                        ArrayIterationKind::Some if runtime.value_to_boolean(&value)? => {
                            return Ok(CallbackStep::Complete(Completion::Return(Value::Bool(
                                true,
                            ))));
                        }
                        ArrayIterationKind::Map => {
                            let index = self.index();
                            return self.define(runtime, index, value);
                        }
                        ArrayIterationKind::Filter if runtime.value_to_boolean(&value)? => {
                            let original = self.value.clone();
                            let index = self.selected;
                            return self.define(runtime, index, original);
                        }
                        _ => {}
                    },
                }
                self.value = Value::Undefined;
                self.cursor += 1;
                self.next(runtime)
            }
            _ => Err(RuntimeError::Invariant(
                "Array callback value reply phase mismatch",
            )),
        }
    }
    pub(crate) fn number(
        mut self,
        runtime: &Runtime,
        result: NativeConversion<f64>,
    ) -> Result<CallbackStep, RuntimeError> {
        if !matches!(self.phase, Phase::Number) {
            return Err(RuntimeError::Invariant(
                "Array callback number phase mismatch",
            ));
        }
        self.length = match result {
            NativeConversion::Value(value) => Runtime::length_from_number(value),
            NativeConversion::Throw(value) => {
                return Ok(CallbackStep::Complete(Completion::Throw(value)));
            }
        };
        self.callback = Some(runtime.callable_from_value(self.callback_value.clone())?);
        if let CallbackKind::Iteration(kind) = self.kind {
            self.result = match kind {
                ArrayIterationKind::Every => Value::Bool(true),
                ArrayIterationKind::Some => Value::Bool(false),
                _ => Value::Undefined,
            };
            if matches!(kind, ArrayIterationKind::Map | ArrayIterationKind::Filter) {
                self.phase = Phase::Species;
                return Ok(CallbackStep::Species {
                    source: self.object.clone(),
                    length: if kind == ArrayIterationKind::Map {
                        self.length
                    } else {
                        0
                    },
                    resume: self,
                });
            }
        }
        self.next(runtime)
    }
    fn next(mut self, runtime: &Runtime) -> Result<CallbackStep, RuntimeError> {
        if self.cursor == self.length {
            let result = match self.kind {
                CallbackKind::Iteration(_) => self.result,
                CallbackKind::Reduce(_) => match self.accumulator {
                    Some(value) => value,
                    None => {
                        return Ok(CallbackStep::Complete(Completion::Throw(
                            runtime.new_native_error(
                                self.realm,
                                NativeErrorKind::Type,
                                "empty array",
                            )?,
                        )));
                    }
                },
                CallbackKind::Find(ArrayFindKind::Find | ArrayFindKind::FindLast) => {
                    Value::Undefined
                }
                CallbackKind::Find(_) => Value::Int(-1),
            };
            return Ok(CallbackStep::Complete(Completion::Return(result)));
        }
        let key = runtime.property_key_for_index(self.index())?;
        if matches!(self.kind, CallbackKind::Find(_)) {
            self.phase = Phase::Read;
            Ok(CallbackStep::Read {
                object: self.object.clone(),
                key,
                resume: self,
            })
        } else {
            self.phase = Phase::Has;
            Ok(CallbackStep::Has {
                object: self.object.clone(),
                key,
                resume: self,
            })
        }
    }
    pub(crate) fn boolean(
        mut self,
        runtime: &Runtime,
        result: NativeConversion<bool>,
    ) -> Result<CallbackStep, RuntimeError> {
        if !matches!(self.phase, Phase::Has) {
            return Err(RuntimeError::Invariant(
                "Array callback boolean phase mismatch",
            ));
        }
        let present = match result {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => {
                return Ok(CallbackStep::Complete(Completion::Throw(value)));
            }
        };
        if !present {
            self.cursor += 1;
            return self.next(runtime);
        }
        let key = runtime.property_key_for_index(self.index())?;
        self.phase = Phase::Read;
        Ok(CallbackStep::Read {
            object: self.object.clone(),
            key,
            resume: self,
        })
    }
    fn define(
        mut self,
        runtime: &Runtime,
        index: u64,
        value: Value,
    ) -> Result<CallbackStep, RuntimeError> {
        let Value::Object(object) = &self.result else {
            return Err(RuntimeError::Invariant(
                "Array callback result was not an object",
            ));
        };
        let object = object.clone();
        self.phase = Phase::Define;
        Ok(CallbackStep::Define {
            object,
            key: runtime.property_key_for_index(index)?,
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
    ) -> Result<CallbackStep, RuntimeError> {
        if !matches!(self.phase, Phase::Define) {
            return Err(RuntimeError::Invariant(
                "Array callback define phase mismatch",
            ));
        }
        let filter = matches!(
            self.kind,
            CallbackKind::Iteration(ArrayIterationKind::Filter)
        );
        let index = if filter { self.selected } else { self.index() };
        if let Some(value) =
            runtime.finish_create_indexed_data_property(self.realm, index, result)?
        {
            return Ok(CallbackStep::Complete(Completion::Throw(value)));
        }
        if filter {
            self.selected += 1;
        }
        self.value = Value::Undefined;
        self.cursor += 1;
        self.next(runtime)
    }
}
pub(crate) fn finish(
    runtime: &Runtime,
    realm: ContextId,
    mut step: CallbackStep,
) -> Result<Completion, RuntimeError> {
    loop {
        step = match step {
            CallbackStep::Complete(result) => return Ok(result),
            CallbackStep::Read {
                object,
                key,
                resume,
            } => resume.resume(
                runtime,
                runtime.get_property_in_realm(realm, &object, &key)?,
            )?,
            CallbackStep::Number { value, resume } => {
                resume.number(runtime, runtime.native_to_number(realm, &value)?)?
            }
            CallbackStep::Has {
                object,
                key,
                resume,
            } => resume.boolean(
                runtime,
                runtime.internal_has_property(realm, &object, &key)?,
            )?,
            CallbackStep::Call {
                callable,
                receiver,
                arguments,
                resume,
            } => resume.resume(
                runtime,
                runtime.call_internal(realm, &callable, receiver, &arguments)?,
            )?,
            CallbackStep::Species {
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
            CallbackStep::Define {
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unpublished_species_result_and_mapper_survive_wait_then_release() {
        let runtime = Runtime::new();
        let weak = std::rc::Rc::downgrade(&runtime.0);
        let mut context = runtime.new_context();
        let mapper = context.eval("(function(value) { return value; })").unwrap();
        let Value::Object(mapper_object) = &mapper else {
            panic!("expected mapper");
        };
        let source = runtime.new_array(context.realm).unwrap();
        let target = runtime.new_object(None).unwrap();
        let ids = [
            source.object_id(),
            target.object_id(),
            mapper_object.object_id(),
        ];
        let invocation = NativeInvocation::Call {
            this_value: Value::Object(source),
        };
        let arguments = NativeArguments {
            actual_arg_count: 1,
            readable: vec![mapper],
        };
        let CallbackStep::Read { resume, .. } = CallbackStep::start(
            &runtime,
            context.realm,
            CallbackKind::Iteration(ArrayIterationKind::Map),
            &invocation,
            &arguments,
        )
        .unwrap() else {
            panic!("expected length read");
        };
        drop(invocation);
        drop(arguments);
        let CallbackStep::Number { resume, .. } = resume
            .resume(&runtime, Completion::Return(Value::Int(1)))
            .unwrap()
        else {
            panic!("expected length conversion");
        };
        let CallbackStep::Species { resume, .. } = resume
            .number(&runtime, NativeConversion::Value(1.0))
            .unwrap()
        else {
            panic!("expected species");
        };
        let CallbackStep::Has { resume, .. } = resume
            .resume(&runtime, Completion::Return(Value::Object(target)))
            .unwrap()
        else {
            panic!("expected indexed lookup");
        };
        runtime.run_gc().unwrap();
        for id in ids {
            assert!(runtime.0.state.borrow().heap.object(id).is_ok());
        }
        drop(resume);
        runtime.run_gc().unwrap();
        for id in ids {
            assert!(runtime.0.state.borrow().heap.object(id).is_err());
        }
        drop(context);
        drop(runtime);
        assert!(weak.upgrade().is_none());
    }
}
