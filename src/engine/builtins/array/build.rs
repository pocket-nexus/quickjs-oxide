//! Array.from/of own constructor, iterator and definition replies in source order.
use crate::engine::{
    api::{error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError},
    builtins::{
        iterator::step::{CloseStep, NextStep, finish_close, finish_next},
        object::ObjectIteratorStep,
    },
    heap::ContextId,
    object::{
        CallableRef, DescriptorField, ObjectRef, OrdinaryPropertyDescriptor, PropertyKey,
        WellKnownSymbol,
        operations::{InternalDefineResult, InternalSetResult},
    },
    value::{Value, conversion::NativeConversion},
    vm::{
        Completion,
        call::{ConstructorRef, NativeArguments, NativeInvocation},
    },
};
#[derive(Clone, Copy)]
pub(crate) enum BuildKind {
    From,
    Of,
}
pub(crate) enum BuildStep {
    Complete(Completion),
    Read {
        receiver: Value,
        key: PropertyKey,
        resume: BuildResume,
    },
    Number {
        value: Value,
        resume: BuildResume,
    },
    Call {
        callable: CallableRef,
        receiver: Value,
        arguments: Vec<Value>,
        resume: BuildResume,
    },
    Construct {
        target: ConstructorRef,
        arguments: Vec<Value>,
        resume: BuildResume,
    },
    Parse {
        result: Completion,
        resume: BuildResume,
    },
    Define {
        object: ObjectRef,
        key: PropertyKey,
        descriptor: OrdinaryPropertyDescriptor,
        resume: BuildResume,
    },
    Set {
        object: ObjectRef,
        key: PropertyKey,
        value: Value,
        resume: BuildResume,
    },
    Close {
        iterator: ObjectRef,
        completion: Completion,
    },
}
pub(crate) struct BuildResume {
    realm: ContextId,
    constructor: Value,
    result: Option<ObjectRef>,
    mapfn: Option<CallableRef>,
    map_this: Value,
    mode: Mode,
    index: u64,
    phase: Phase,
}
enum Mode {
    Acquire(Value),
    Iterable {
        items: Value,
        method: CallableRef,
        iterator: Option<ObjectRef>,
        next: Option<CallableRef>,
    },
    ArrayLike {
        source: ObjectRef,
        length: u64,
    },
    Of {
        values: std::vec::IntoIter<Value>,
        length: u32,
    },
}
enum Phase {
    Method,
    Length,
    Number,
    Construct,
    Iterator,
    NextMethod,
    NextCall,
    Parse,
    Value,
    Map,
    Define,
    LengthSet,
}
impl BuildStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        kind: BuildKind,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        let NativeInvocation::Call { this_value } = invocation else {
            return Err(RuntimeError::Invariant(
                "Array builder did not receive a generic invocation",
            ));
        };
        if matches!(kind, BuildKind::Of) {
            let length = u32::try_from(arguments.actual_arg_count)
                .map_err(|_| RuntimeError::Invariant("Array.of argument count exceeded Uint32"))?;
            let resume = BuildResume {
                realm,
                constructor: this_value.clone(),
                result: None,
                mapfn: None,
                map_this: Value::Undefined,
                mode: Mode::Of {
                    values: arguments.readable[..arguments.actual_arg_count]
                        .to_vec()
                        .into_iter(),
                    length,
                },
                index: 0,
                phase: Phase::Construct,
            };
            return resume.construct(runtime, Some(Runtime::array_length_value(length)), false);
        }
        let items = arguments
            .readable
            .first()
            .cloned()
            .ok_or(RuntimeError::Invariant("Array.from argv was not padded"))?;
        let mapfn = if arguments.actual_arg_count > 1
            && !matches!(arguments.readable[1], Value::Undefined)
        {
            let callable = match &arguments.readable[1] {
                Value::Object(object) => runtime.as_callable(object)?,
                _ => None,
            };
            let Some(callable) = callable else {
                return Ok(Self::Complete(Completion::Throw(
                    runtime.new_native_error(realm, NativeErrorKind::Type, "not a function")?,
                )));
            };
            Some(callable)
        } else {
            None
        };
        let map_this = if arguments.actual_arg_count > 2 {
            arguments.readable[2].clone()
        } else {
            Value::Undefined
        };
        if matches!(items, Value::Null | Value::Undefined) {
            let base = if matches!(items, Value::Null) {
                "null"
            } else {
                "undefined"
            };
            return Ok(Self::Complete(Completion::Throw(
                runtime.new_native_error(
                    realm,
                    NativeErrorKind::Type,
                    &format!("cannot read property 'Symbol.iterator' of {base}"),
                )?,
            )));
        }
        Ok(Self::Read {
            receiver: items.clone(),
            key: PropertyKey::from(runtime.well_known_symbol(WellKnownSymbol::Iterator)),
            resume: BuildResume {
                realm,
                constructor: this_value.clone(),
                result: None,
                mapfn,
                map_this,
                mode: Mode::Acquire(items),
                index: 0,
                phase: Phase::Method,
            },
        })
    }
}
impl BuildResume {
    fn abrupt(self, value: Value) -> BuildStep {
        if matches!(self.phase, Phase::Map | Phase::Define)
            && let Mode::Iterable {
                iterator: Some(iterator),
                ..
            } = self.mode
        {
            return BuildStep::Close {
                iterator,
                completion: Completion::Throw(value),
            };
        }
        BuildStep::Complete(Completion::Throw(value))
    }
    fn result(&self) -> Result<ObjectRef, RuntimeError> {
        self.result
            .clone()
            .ok_or(RuntimeError::Invariant("Array builder result missing"))
    }
    fn construct(
        mut self,
        runtime: &Runtime,
        length: Option<Value>,
        initialize_length: bool,
    ) -> Result<BuildStep, RuntimeError> {
        self.phase = Phase::Construct;
        let constructor = std::mem::replace(&mut self.constructor, Value::Undefined);
        if let Value::Object(object) = &constructor
            && runtime.is_constructor(object)?
        {
            let target = match runtime.constructor_from_value(self.realm, constructor)? {
                NativeConversion::Value(target) => target,
                NativeConversion::Throw(value) => return Ok(self.abrupt(value)),
            };
            return Ok(BuildStep::Construct {
                target,
                arguments: length.into_iter().collect(),
                resume: self,
            });
        }
        // Numeric length and a new unpublished Array cannot invoke JS. Share
        // the same fallback allocation as ArraySpeciesCreate.
        let result = runtime
            .new_array_with_length(self.realm, if initialize_length { length } else { None })?;
        self.resume(runtime, result)
    }
    fn next(mut self, runtime: &Runtime) -> Result<BuildStep, RuntimeError> {
        match &mut self.mode {
            Mode::Iterable {
                iterator: Some(iterator),
                next: Some(next),
                ..
            } => {
                let callable = next.clone();
                let receiver = Value::Object(iterator.clone());
                self.phase = Phase::NextCall;
                // Array.from uses ordinary Call, not JS_IteratorNext2's raw
                // cproto fast path; parse its actual object result afterwards.
                Ok(BuildStep::Call {
                    callable,
                    receiver,
                    arguments: Vec::new(),
                    resume: self,
                })
            }
            Mode::ArrayLike { source, length } if self.index < *length => {
                let receiver = Value::Object(source.clone());
                self.phase = Phase::Value;
                Ok(BuildStep::Read {
                    receiver,
                    key: runtime.property_key_for_index(self.index)?,
                    resume: self,
                })
            }
            Mode::Of { values, .. } => {
                if let Some(value) = values.next() {
                    self.define(runtime, value)
                } else {
                    self.set_length(runtime)
                }
            }
            Mode::ArrayLike { .. } => self.set_length(runtime),
            _ => Err(RuntimeError::Invariant(
                "Array builder iteration not initialized",
            )),
        }
    }
    fn set_length(mut self, runtime: &Runtime) -> Result<BuildStep, RuntimeError> {
        let length = match self.mode {
            Mode::Of { length, .. } => length as u64,
            Mode::ArrayLike { length, .. } => length,
            _ => self.index,
        };
        self.phase = Phase::LengthSet;
        Ok(BuildStep::Set {
            object: self.result()?,
            key: runtime.intern_property_key("length")?,
            value: Value::number(length as f64),
            resume: self,
        })
    }
    fn map(mut self, runtime: &Runtime, value: Value) -> Result<BuildStep, RuntimeError> {
        if let Some(callable) = self.mapfn.clone() {
            self.phase = Phase::Map;
            return Ok(BuildStep::Call {
                callable,
                receiver: self.map_this.clone(),
                arguments: vec![value, Value::number(self.index as f64)],
                resume: self,
            });
        }
        self.define(runtime, value)
    }
    fn define(mut self, runtime: &Runtime, value: Value) -> Result<BuildStep, RuntimeError> {
        self.phase = Phase::Define;
        Ok(BuildStep::Define {
            object: self.result()?,
            key: runtime.property_key_for_index(self.index)?,
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
    pub(crate) fn resume(
        mut self,
        runtime: &Runtime,
        reply: Completion,
    ) -> Result<BuildStep, RuntimeError> {
        if matches!(self.phase, Phase::NextCall) {
            self.phase = Phase::Parse;
            return Ok(BuildStep::Parse {
                result: reply,
                resume: self,
            });
        }
        let value = match reply {
            Completion::Return(value) => value,
            Completion::Throw(value) => return Ok(self.abrupt(value)),
        };
        match self.phase {
            Phase::Method => {
                let Mode::Acquire(items) =
                    std::mem::replace(&mut self.mode, Mode::Acquire(Value::Undefined))
                else {
                    return Err(RuntimeError::Invariant("Array.from items missing"));
                };
                if matches!(value, Value::Undefined | Value::Null) {
                    let source = match runtime.native_to_object(self.realm, items)? {
                        NativeConversion::Value(source) => source,
                        NativeConversion::Throw(value) => return Ok(self.abrupt(value)),
                    };
                    self.mode = Mode::ArrayLike {
                        source: source.clone(),
                        length: 0,
                    };
                    self.phase = Phase::Length;
                    return Ok(BuildStep::Read {
                        receiver: Value::Object(source),
                        key: runtime.intern_property_key("length")?,
                        resume: self,
                    });
                }
                let callable = match value {
                    Value::Object(ref object) => runtime.as_callable(object)?,
                    _ => None,
                };
                let Some(method) = callable else {
                    let error = runtime.new_native_error(
                        self.realm,
                        NativeErrorKind::Type,
                        "value is not iterable",
                    )?;
                    return Ok(self.abrupt(error));
                };
                self.mode = Mode::Iterable {
                    items,
                    method,
                    iterator: None,
                    next: None,
                };
                self.construct(runtime, None, false)
            }
            Phase::Length => {
                self.phase = Phase::Number;
                Ok(BuildStep::Number {
                    value,
                    resume: self,
                })
            }
            Phase::Construct => {
                let Value::Object(result) = value else {
                    return Err(RuntimeError::Invariant(
                        "Array result constructor returned a primitive",
                    ));
                };
                self.result = Some(result);
                if let Mode::Iterable { items, method, .. } = &self.mode {
                    let receiver = items.clone();
                    let callable = method.clone();
                    self.phase = Phase::Iterator;
                    return Ok(BuildStep::Call {
                        callable,
                        receiver,
                        arguments: Vec::new(),
                        resume: self,
                    });
                }
                self.next(runtime)
            }
            Phase::Iterator => {
                let Value::Object(iterator) = value else {
                    let error = runtime.new_native_error(
                        self.realm,
                        NativeErrorKind::Type,
                        "not an object",
                    )?;
                    return Ok(self.abrupt(error));
                };
                let Mode::Iterable {
                    iterator: target, ..
                } = &mut self.mode
                else {
                    return Err(RuntimeError::Invariant("Array.from iterator mode missing"));
                };
                *target = Some(iterator.clone());
                self.phase = Phase::NextMethod;
                Ok(BuildStep::Read {
                    receiver: Value::Object(iterator),
                    key: runtime.intern_property_key("next")?,
                    resume: self,
                })
            }
            Phase::NextMethod => {
                let callable = match value {
                    Value::Object(ref object) => runtime.as_callable(object)?,
                    _ => None,
                };
                let Some(callable) = callable else {
                    let error = runtime.new_native_error(
                        self.realm,
                        NativeErrorKind::Type,
                        "not a function",
                    )?;
                    return Ok(self.abrupt(error));
                };
                let Mode::Iterable { next, .. } = &mut self.mode else {
                    return Err(RuntimeError::Invariant("Array.from iterator mode missing"));
                };
                *next = Some(callable);
                self.next(runtime)
            }
            Phase::Value => self.map(runtime, value),
            Phase::Map => self.define(runtime, value),
            _ => Err(RuntimeError::Invariant(
                "Array builder completion phase mismatch",
            )),
        }
    }
    pub(crate) fn number(
        mut self,
        runtime: &Runtime,
        reply: NativeConversion<f64>,
    ) -> Result<BuildStep, RuntimeError> {
        let number = match reply {
            NativeConversion::Value(number) => number,
            NativeConversion::Throw(value) => return Ok(self.abrupt(value)),
        };
        if !matches!(self.phase, Phase::Number) {
            return Err(RuntimeError::Invariant(
                "Array builder number phase mismatch",
            ));
        }
        let Mode::ArrayLike { length, .. } = &mut self.mode else {
            return Err(RuntimeError::Invariant(
                "Array builder length mode mismatch",
            ));
        };
        *length = Runtime::length_from_number(number);
        let value = Value::number(*length as f64);
        self.construct(runtime, Some(value), true)
    }
    pub(crate) fn parsed(
        self,
        runtime: &Runtime,
        reply: ObjectIteratorStep,
    ) -> Result<BuildStep, RuntimeError> {
        if !matches!(self.phase, Phase::Parse) {
            return Err(RuntimeError::Invariant(
                "Array builder next result phase mismatch",
            ));
        }
        match reply {
            ObjectIteratorStep::Throw(value) => Ok(self.abrupt(value)),
            ObjectIteratorStep::Done => self.set_length(runtime),
            ObjectIteratorStep::Yield(value) => self.map(runtime, value),
        }
    }
    pub(crate) fn defined(
        mut self,
        runtime: &Runtime,
        reply: NativeConversion<InternalDefineResult>,
    ) -> Result<BuildStep, RuntimeError> {
        if !matches!(self.phase, Phase::Define) {
            return Err(RuntimeError::Invariant(
                "Array builder definition phase mismatch",
            ));
        }
        if let Some(value) =
            runtime.finish_create_indexed_data_property(self.realm, self.index, reply)?
        {
            return Ok(self.abrupt(value));
        }
        self.index = self.index.checked_add(1).ok_or(RuntimeError::Invariant(
            "Array.from iterator index overflowed u64",
        ))?;
        self.next(runtime)
    }
    pub(crate) fn set(
        self,
        runtime: &Runtime,
        key: PropertyKey,
        reply: NativeConversion<InternalSetResult>,
    ) -> Result<BuildStep, RuntimeError> {
        if !matches!(self.phase, Phase::LengthSet) {
            return Err(RuntimeError::Invariant(
                "Array builder length-set phase mismatch",
            ));
        }
        if let Some(value) = runtime.finish_set_property_or_throw(self.realm, &key, reply)? {
            return Ok(self.abrupt(value));
        }
        Ok(BuildStep::Complete(Completion::Return(Value::Object(
            self.result()?,
        ))))
    }
}
pub(crate) fn finish(
    runtime: &Runtime,
    realm: ContextId,
    mut step: BuildStep,
) -> Result<Completion, RuntimeError> {
    loop {
        step = match step {
            BuildStep::Complete(result) => return Ok(result),
            BuildStep::Read {
                receiver,
                key,
                resume,
            } => resume.resume(
                runtime,
                runtime.get_value_property_in_realm(realm, receiver, &key)?,
            )?,
            BuildStep::Number { value, resume } => {
                resume.number(runtime, runtime.native_to_number(realm, &value)?)?
            }
            BuildStep::Call {
                callable,
                receiver,
                arguments,
                resume,
            } => resume.resume(
                runtime,
                runtime.call_internal(realm, &callable, receiver, &arguments)?,
            )?,
            BuildStep::Construct {
                target,
                arguments,
                resume,
            } => resume.resume(
                runtime,
                runtime.construct_constructor_internal(realm, &target, &target, &arguments)?,
            )?,
            BuildStep::Parse { result, resume } => resume.parsed(
                runtime,
                finish_next(
                    runtime,
                    realm,
                    NextStep::parse_result(runtime, realm, result)?,
                )?,
            )?,
            BuildStep::Define {
                object,
                key,
                descriptor,
                resume,
            } => resume.defined(
                runtime,
                runtime.internal_define_own_property(realm, &object, &key, &descriptor)?,
            )?,
            BuildStep::Set {
                object,
                key,
                value,
                resume,
            } => {
                let reply = runtime.internal_set(
                    realm,
                    &object,
                    &key,
                    value,
                    Value::Object(object.clone()),
                )?;
                resume.set(runtime, key, reply)?
            }
            BuildStep::Close {
                iterator,
                completion,
            } => {
                return finish_close(
                    runtime,
                    realm,
                    CloseStep::start(runtime, realm, iterator, completion)?,
                );
            }
        };
    }
}
