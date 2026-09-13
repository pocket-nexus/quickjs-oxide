//! TypedArray construction and static factories own inputs until all writes finish.
use crate::engine::{
    api::{error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError},
    builtins::native::TypedArrayElementKind,
    heap::ContextId,
    object::{CallableRef, ObjectRef, PropertyKey},
    value::{Value, conversion::NativeConversion},
    vm::{
        Completion, ToPrimitiveHint,
        call::{
            ConstructorPrototypeSource, DirectCallTarget, NativeArguments, NativeInvocation,
            prototype::{ProtoSourceStep, finish as finish_source},
        },
    },
};
pub(crate) enum TypedCreateStep {
    Complete(Completion),
    Primitive {
        value: Value,
        resume: TypedCreateResume,
    },
    Prototype {
        new_target: Value,
        resume: TypedCreateResume,
    },
    Read {
        receiver: Value,
        key: PropertyKey,
        resume: TypedCreateResume,
    },
    Method {
        source: Value,
        resume: TypedCreateResume,
    },
    Collect {
        source: Value,
        method: CallableRef,
        element: TypedArrayElementKind,
        resume: TypedCreateResume,
    },
    Create {
        constructor: Value,
        length: u64,
        resume: TypedCreateResume,
    },
    Call {
        target: DirectCallTarget,
        receiver: Value,
        arguments: Vec<Value>,
        resume: TypedCreateResume,
    },
    Element {
        element: TypedArrayElementKind,
        value: Value,
        resume: TypedCreateResume,
    },
}
pub(crate) struct TypedCreateResume {
    realm: ContextId,
    phase: Phase,
}
enum Allocation {
    Intrinsic {
        prototype: ObjectRef,
        element: TypedArrayElementKind,
    },
    Static(Value),
}
enum Input {
    Values(Vec<Value>),
    Object(ObjectRef),
}
struct Factory {
    allocation: Allocation,
    mapper: Option<CallableRef>,
    this_arg: Value,
}
struct Population {
    source: Input,
    target: ObjectRef,
    mapper: Option<CallableRef>,
    this_arg: Value,
    length: u64,
    index: u64,
}
enum ProtoPurpose {
    Length(u64),
    Buffer {
        source: ObjectRef,
        offset: Option<Value>,
        length: Option<Value>,
    },
    Typed {
        source: ObjectRef,
        length: u64,
    },
    Object(ObjectRef),
}
enum Phase {
    Prototype {
        element: TypedArrayElementKind,
        purpose: ProtoPurpose,
    },
    Offset {
        prototype: ObjectRef,
        element: TypedArrayElementKind,
        source: ObjectRef,
        length: Option<Value>,
    },
    BufferLength {
        prototype: ObjectRef,
        element: TypedArrayElementKind,
        source: ObjectRef,
        offset: u64,
    },
    Iterator {
        source: Value,
        factory: Factory,
    },
    Collect(Factory),
    Length {
        source: ObjectRef,
        factory: Factory,
    },
    LengthPrimitive {
        source: ObjectRef,
        factory: Factory,
    },
    Create {
        source: Input,
        factory: Factory,
        length: u64,
    },
    Index(Population),
    Mapped(Population),
    Element(Population),
}
impl TypedCreateStep {
    pub(crate) fn constructor(
        runtime: &Runtime,
        realm: ContextId,
        element: TypedArrayElementKind,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        let NativeInvocation::Construct { new_target } = invocation else {
            return Err(RuntimeError::Invariant(
                "TypedArray constructor did not receive a constructor invocation",
            ));
        };
        let first = arguments
            .readable
            .first()
            .ok_or(RuntimeError::Invariant(
                "TypedArray constructor argv was not padded",
            ))?
            .clone();
        let purpose = if let Value::Object(source) = first {
            if !source.belongs_to(runtime) {
                return Err(RuntimeError::WrongRuntime("TypedArray constructor source"));
            }
            if runtime
                .snapshot_buffer_access_if_branded(&source)?
                .is_some()
            {
                let offset = if arguments.actual_arg_count > 1 {
                    Some(
                        arguments
                            .readable
                            .get(1)
                            .ok_or(RuntimeError::Invariant(
                                "TypedArray byteOffset argv was not padded",
                            ))?
                            .clone(),
                    )
                } else {
                    None
                };
                let length = if arguments.actual_arg_count > 2
                    && !matches!(arguments.readable.get(2), Some(Value::Undefined))
                {
                    Some(
                        arguments
                            .readable
                            .get(2)
                            .ok_or(RuntimeError::Invariant(
                                "TypedArray length argv was not padded",
                            ))?
                            .clone(),
                    )
                } else {
                    None
                };
                ProtoPurpose::Buffer {
                    source,
                    offset,
                    length,
                }
            } else if let Some(snapshot) = runtime.typed_array_snapshot_if_branded(&source)? {
                let length = u64::from(runtime.typed_array_state_from_snapshot(snapshot)?.length);
                ProtoPurpose::Typed { source, length }
            } else {
                ProtoPurpose::Object(source)
            }
        } else {
            let length = match runtime.native_to_index(realm, &first)? {
                NativeConversion::Value(value) => value,
                NativeConversion::Throw(value) => {
                    return Ok(Self::Complete(Completion::Throw(value)));
                }
            };
            ProtoPurpose::Length(length)
        };
        Ok(Self::Prototype {
            new_target: new_target.clone(),
            resume: TypedCreateResume {
                realm,
                phase: Phase::Prototype { element, purpose },
            },
        })
    }
    pub(crate) fn from(
        runtime: &Runtime,
        realm: ContextId,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        let NativeInvocation::Call { this_value } = invocation else {
            return Err(RuntimeError::Invariant(
                "TypedArray.from received a constructor invocation",
            ));
        };
        let source = arguments
            .readable
            .first()
            .ok_or(RuntimeError::Invariant(
                "TypedArray.from argv was not padded",
            ))?
            .clone();
        let mapper = if arguments.actual_arg_count > 1
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
        let this_arg = if arguments.actual_arg_count > 2 {
            arguments.readable[2].clone()
        } else {
            Value::Undefined
        };
        let message = match source {
            Value::Undefined => Some("cannot read property 'Symbol.iterator' of undefined"),
            Value::Null => Some("cannot read property 'Symbol.iterator' of null"),
            _ => None,
        };
        if let Some(message) = message {
            return Ok(Self::Complete(Completion::Throw(
                runtime.new_native_error(realm, NativeErrorKind::Type, message)?,
            )));
        }
        TypedCreateResume::iterator(
            runtime,
            realm,
            source,
            Factory {
                allocation: Allocation::Static(this_value.clone()),
                mapper,
                this_arg,
            },
        )
    }
    pub(crate) fn of(
        runtime: &Runtime,
        realm: ContextId,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        let NativeInvocation::Call { this_value } = invocation else {
            return Err(RuntimeError::Invariant(
                "TypedArray.of received a constructor invocation",
            ));
        };
        let length = u64::try_from(arguments.actual_arg_count)
            .map_err(|_| RuntimeError::Invariant("TypedArray.of argc overflowed u64"))?;
        let mut values = Vec::new();
        if values
            .try_reserve_exact(arguments.actual_arg_count)
            .is_err()
        {
            return out_of_memory(runtime, realm);
        }
        values.extend(
            arguments.readable[..arguments.actual_arg_count]
                .iter()
                .cloned(),
        );
        TypedCreateResume::allocate(
            runtime,
            realm,
            Input::Values(values),
            Factory {
                allocation: Allocation::Static(this_value.clone()),
                mapper: None,
                this_arg: Value::Undefined,
            },
            length,
        )
    }
}
impl TypedCreateResume {
    fn iterator(
        _runtime: &Runtime,
        realm: ContextId,
        source: Value,
        factory: Factory,
    ) -> Result<TypedCreateStep, RuntimeError> {
        Ok(TypedCreateStep::Method {
            source: source.clone(),
            resume: Self {
                realm,
                phase: Phase::Iterator { source, factory },
            },
        })
    }
    pub(crate) fn method(
        self,
        runtime: &Runtime,
        result: NativeConversion<Option<CallableRef>>,
    ) -> Result<TypedCreateStep, RuntimeError> {
        let method = match result {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => {
                return Ok(TypedCreateStep::Complete(Completion::Throw(value)));
            }
        };
        let Phase::Iterator { source, factory } = self.phase else {
            return Err(RuntimeError::Invariant(
                "TypedArray iterator method reply in wrong phase",
            ));
        };
        if let Some(method) = method {
            let element = match &factory.allocation {
                Allocation::Intrinsic { element, .. } => *element,
                Allocation::Static(_) => TypedArrayElementKind::Uint8,
            };
            Ok(TypedCreateStep::Collect {
                source,
                method,
                element,
                resume: Self {
                    realm: self.realm,
                    phase: Phase::Collect(factory),
                },
            })
        } else {
            let source = match runtime.native_to_object(self.realm, source)? {
                NativeConversion::Value(value) => value,
                NativeConversion::Throw(value) => {
                    return Ok(TypedCreateStep::Complete(Completion::Throw(value)));
                }
            };
            Ok(TypedCreateStep::Read {
                receiver: Value::Object(source.clone()),
                key: runtime.intern_property_key("length")?,
                resume: Self {
                    realm: self.realm,
                    phase: Phase::Length { source, factory },
                },
            })
        }
    }

    pub(crate) fn prototype(
        self,
        runtime: &Runtime,
        result: NativeConversion<ConstructorPrototypeSource>,
    ) -> Result<TypedCreateStep, RuntimeError> {
        let Phase::Prototype { element, purpose } = self.phase else {
            return Err(RuntimeError::Invariant(
                "TypedArray create prototype reply in wrong phase",
            ));
        };
        let prototype = match result {
            NativeConversion::Value(ConstructorPrototypeSource::Explicit(value)) => value,
            NativeConversion::Value(ConstructorPrototypeSource::Realm(realm)) => {
                runtime.typed_array_default_prototype(realm, element)?
            }
            NativeConversion::Throw(value) => {
                return Ok(TypedCreateStep::Complete(Completion::Throw(value)));
            }
        };
        match purpose {
            ProtoPurpose::Length(length) => complete_object(
                runtime.new_typed_array_for_length(self.realm, &prototype, element, length)?,
            ),
            ProtoPurpose::Typed { source, length } => {
                let snapshot = runtime.typed_array_snapshot(&source)?;
                complete_object(runtime.typed_array_copy_into_new(
                    self.realm, &prototype, element, &source, snapshot, length,
                )?)
            }
            ProtoPurpose::Object(source) => Self::iterator(
                runtime,
                self.realm,
                Value::Object(source),
                Factory {
                    allocation: Allocation::Intrinsic { prototype, element },
                    mapper: None,
                    this_arg: Value::Undefined,
                },
            ),
            ProtoPurpose::Buffer {
                source,
                offset,
                length,
            } => {
                let resume = Self {
                    realm: self.realm,
                    phase: Phase::Offset {
                        prototype,
                        element,
                        source,
                        length,
                    },
                };
                if let Some(value) = offset {
                    Ok(TypedCreateStep::Primitive { value, resume })
                } else {
                    resume.resume(runtime, Completion::Return(Value::Int(0)))
                }
            }
        }
    }
    pub(crate) fn collected(
        self,
        runtime: &Runtime,
        result: NativeConversion<Vec<Value>>,
    ) -> Result<TypedCreateStep, RuntimeError> {
        let values = match result {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => {
                return Ok(TypedCreateStep::Complete(Completion::Throw(value)));
            }
        };
        let Phase::Collect(factory) = self.phase else {
            return Err(RuntimeError::Invariant(
                "TypedArray collected reply in wrong phase",
            ));
        };
        let length = u64::try_from(values.len())
            .map_err(|_| RuntimeError::Invariant("TypedArray iterable length overflowed u64"))?;
        Self::allocate(runtime, self.realm, Input::Values(values), factory, length)
    }
    fn allocate(
        runtime: &Runtime,
        realm: ContextId,
        source: Input,
        factory: Factory,
        length: u64,
    ) -> Result<TypedCreateStep, RuntimeError> {
        match &factory.allocation {
            Allocation::Intrinsic { prototype, element } => {
                let target =
                    match runtime.new_typed_array_for_length(realm, prototype, *element, length)? {
                        NativeConversion::Value(value) => value,
                        NativeConversion::Throw(value) => {
                            return Ok(TypedCreateStep::Complete(Completion::Throw(value)));
                        }
                    };
                Population {
                    source,
                    target,
                    mapper: factory.mapper,
                    this_arg: factory.this_arg,
                    length,
                    index: 0,
                }
                .next(runtime, realm)
            }
            Allocation::Static(constructor) => {
                if !matches!(constructor, Value::Object(_)) {
                    return Ok(TypedCreateStep::Complete(Completion::Throw(
                        runtime.new_native_error(realm, NativeErrorKind::Type, "not a function")?,
                    )));
                }
                Ok(TypedCreateStep::Create {
                    constructor: constructor.clone(),
                    length,
                    resume: Self {
                        realm,
                        phase: Phase::Create {
                            source,
                            factory,
                            length,
                        },
                    },
                })
            }
        }
    }
    pub(crate) fn created(
        self,
        runtime: &Runtime,
        result: NativeConversion<ObjectRef>,
    ) -> Result<TypedCreateStep, RuntimeError> {
        let target = match result {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => {
                return Ok(TypedCreateStep::Complete(Completion::Throw(value)));
            }
        };
        let Phase::Create {
            source,
            factory,
            length,
        } = self.phase
        else {
            return Err(RuntimeError::Invariant(
                "TypedArray create reply in wrong phase",
            ));
        };
        Population {
            source,
            target,
            mapper: factory.mapper,
            this_arg: factory.this_arg,
            length,
            index: 0,
        }
        .next(runtime, self.realm)
    }
    pub(crate) fn element(
        self,
        runtime: &Runtime,
        result: NativeConversion<[u8; 8]>,
    ) -> Result<TypedCreateStep, RuntimeError> {
        let bytes = match result {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => {
                return Ok(TypedCreateStep::Complete(Completion::Throw(value)));
            }
        };
        let Phase::Element(mut population) = self.phase else {
            return Err(RuntimeError::Invariant(
                "TypedArray create element reply in wrong phase",
            ));
        };
        runtime.typed_array_write_converted_index(&population.target, population.index, &bytes)?;
        population.index += 1;
        population.next(runtime, self.realm)
    }
    pub(crate) fn resume(
        self,
        runtime: &Runtime,
        result: Completion,
    ) -> Result<TypedCreateStep, RuntimeError> {
        let value = match result {
            Completion::Return(value) => value,
            Completion::Throw(value) => {
                return Ok(TypedCreateStep::Complete(Completion::Throw(value)));
            }
        };
        match self.phase {
            Phase::Offset {
                prototype,
                element,
                source,
                length,
            } => {
                let offset = match runtime.native_to_index(self.realm, &value)? {
                    NativeConversion::Value(value) => value,
                    NativeConversion::Throw(value) => {
                        return Ok(TypedCreateStep::Complete(Completion::Throw(value)));
                    }
                };
                if offset % u64::from(element.byte_length()) != 0 {
                    return Ok(TypedCreateStep::Complete(Completion::Throw(
                        runtime.new_native_error(
                            self.realm,
                            NativeErrorKind::Range,
                            "invalid offset",
                        )?,
                    )));
                }
                if let Some(value) = length {
                    Ok(TypedCreateStep::Primitive {
                        value,
                        resume: Self {
                            realm: self.realm,
                            phase: Phase::BufferLength {
                                prototype,
                                element,
                                source,
                                offset,
                            },
                        },
                    })
                } else {
                    complete_object(runtime.new_typed_array_constructor_view_from_coerced(
                        self.realm, &prototype, element, &source, offset, None,
                    )?)
                }
            }
            Phase::BufferLength {
                prototype,
                element,
                source,
                offset,
            } => {
                let length = match runtime.native_to_index(self.realm, &value)? {
                    NativeConversion::Value(value) => value,
                    NativeConversion::Throw(value) => {
                        return Ok(TypedCreateStep::Complete(Completion::Throw(value)));
                    }
                };
                complete_object(runtime.new_typed_array_constructor_view_from_coerced(
                    self.realm,
                    &prototype,
                    element,
                    &source,
                    offset,
                    Some(length),
                )?)
            }
            Phase::Length { source, factory } => Ok(TypedCreateStep::Primitive {
                value,
                resume: Self {
                    realm: self.realm,
                    phase: Phase::LengthPrimitive { source, factory },
                },
            }),
            Phase::LengthPrimitive { source, factory } => {
                let length = match runtime.native_to_length(self.realm, &value)? {
                    NativeConversion::Value(value) => value,
                    NativeConversion::Throw(value) => {
                        return Ok(TypedCreateStep::Complete(Completion::Throw(value)));
                    }
                };
                Self::allocate(runtime, self.realm, Input::Object(source), factory, length)
            }
            Phase::Index(population) => population.map(runtime, self.realm, value),
            Phase::Mapped(population) => population.convert(runtime, self.realm, value),
            _ => Err(RuntimeError::Invariant(
                "TypedArray create value reply in wrong phase",
            )),
        }
    }
}
impl Population {
    fn next(self, runtime: &Runtime, realm: ContextId) -> Result<TypedCreateStep, RuntimeError> {
        if self.index == self.length {
            return Ok(TypedCreateStep::Complete(Completion::Return(
                Value::Object(self.target),
            )));
        }
        match &self.source {
            Input::Object(source) => Ok(TypedCreateStep::Read {
                receiver: Value::Object(source.clone()),
                key: runtime.property_key_for_index(self.index)?,
                resume: TypedCreateResume {
                    realm,
                    phase: Phase::Index(self),
                },
            }),
            Input::Values(values) => {
                let value = values[usize::try_from(self.index).map_err(|_| {
                    RuntimeError::Invariant("TypedArray materialized index overflowed usize")
                })?]
                .clone();
                self.map(runtime, realm, value)
            }
        }
    }
    fn map(
        self,
        runtime: &Runtime,
        realm: ContextId,
        value: Value,
    ) -> Result<TypedCreateStep, RuntimeError> {
        if let Some(mapper) = &self.mapper {
            let mut arguments = Vec::new();
            if arguments.try_reserve_exact(2).is_err() {
                return out_of_memory(runtime, realm);
            }
            arguments.push(value);
            arguments.push(Value::number(self.index as f64));
            Ok(TypedCreateStep::Call {
                target: DirectCallTarget::Callable(mapper.clone()),
                receiver: self.this_arg.clone(),
                arguments,
                resume: TypedCreateResume {
                    realm,
                    phase: Phase::Mapped(self),
                },
            })
        } else {
            self.convert(runtime, realm, value)
        }
    }
    fn convert(
        self,
        runtime: &Runtime,
        realm: ContextId,
        value: Value,
    ) -> Result<TypedCreateStep, RuntimeError> {
        let element = runtime.typed_array_snapshot(&self.target)?.element;
        Ok(TypedCreateStep::Element {
            element,
            value,
            resume: TypedCreateResume {
                realm,
                phase: Phase::Element(self),
            },
        })
    }
}
fn complete_object(result: NativeConversion<ObjectRef>) -> Result<TypedCreateStep, RuntimeError> {
    Ok(TypedCreateStep::Complete(match result {
        NativeConversion::Value(value) => Completion::Return(Value::Object(value)),
        NativeConversion::Throw(value) => Completion::Throw(value),
    }))
}
fn out_of_memory(runtime: &Runtime, realm: ContextId) -> Result<TypedCreateStep, RuntimeError> {
    Ok(TypedCreateStep::Complete(Completion::Throw(
        runtime.new_native_error(realm, NativeErrorKind::Internal, "out of memory")?,
    )))
}
pub(super) fn finish(
    runtime: &Runtime,
    realm: ContextId,
    mut step: TypedCreateStep,
) -> Result<Completion, RuntimeError> {
    loop {
        step = match step {
            TypedCreateStep::Complete(result) => return Ok(result),
            TypedCreateStep::Primitive { value, resume } => {
                let result = if matches!(value, Value::Object(_)) {
                    runtime.to_primitive(realm, value, ToPrimitiveHint::Number)?
                } else {
                    Completion::Return(value)
                };
                resume.resume(runtime, result)?
            }
            TypedCreateStep::Prototype { new_target, resume } => resume.prototype(
                runtime,
                finish_source(
                    runtime,
                    realm,
                    ProtoSourceStep::start(runtime, realm, new_target)?,
                )?,
            )?,
            TypedCreateStep::Read {
                receiver,
                key,
                resume,
            } => resume.resume(
                runtime,
                runtime.get_value_property_in_realm(realm, receiver, &key)?,
            )?,
            TypedCreateStep::Method { source, resume } => {
                resume.method(runtime, runtime.typed_array_iterator_method(realm, source)?)?
            }
            TypedCreateStep::Collect {
                source,
                method,
                element,
                resume,
            } => resume.collected(
                runtime,
                runtime.collect_typed_array_iterator(realm, source, &method, element)?,
            )?,
            TypedCreateStep::Create {
                constructor,
                length,
                resume,
            } => resume.created(
                runtime,
                runtime.typed_array_create_from_static_constructor(realm, constructor, length)?,
            )?,
            TypedCreateStep::Call {
                target,
                receiver,
                arguments,
                resume,
            } => {
                let DirectCallTarget::Callable(callable) = target else {
                    return Err(RuntimeError::Invariant(
                        "TypedArray create invalid call target",
                    ));
                };
                resume.resume(
                    runtime,
                    runtime.call_internal(realm, &callable, receiver, &arguments)?,
                )?
            }
            TypedCreateStep::Element {
                element,
                value,
                resume,
            } => resume.element(
                runtime,
                runtime.typed_array_convert_element(realm, element, &value)?,
            )?,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn materialized_factory_values_survive_processed_writes_and_abandonment() {
        let runtime = Runtime::new();
        let weak = std::rc::Rc::downgrade(&runtime.0);
        let mut context = runtime.new_context();
        let Value::Object(target) = context.eval("new Uint8Array(2)").unwrap() else {
            panic!("expected view");
        };
        let target_id = target.object_id();
        let buffer_id = runtime.typed_array_snapshot(&target).unwrap().buffer;
        let first = runtime.new_object(None).unwrap();
        let second = runtime.new_object(None).unwrap();
        let first_id = first.object_id();
        let second_id = second.object_id();
        let realm = context.realm;
        let population = Population {
            source: Input::Values(vec![Value::Object(first), Value::Object(second)]),
            target,
            mapper: None,
            this_arg: Value::Undefined,
            length: 2,
            index: 0,
        };
        let TypedCreateStep::Element { resume, .. } = population.next(&runtime, realm).unwrap()
        else {
            panic!("expected first conversion");
        };
        let step = resume
            .element(&runtime, NativeConversion::Value([0; 8]))
            .unwrap();
        assert!(matches!(step, TypedCreateStep::Element { .. }));
        runtime.run_gc().unwrap();
        for id in [first_id, second_id, target_id, buffer_id] {
            assert!(runtime.0.state.borrow().heap.object(id).is_ok());
        }
        drop(step);
        runtime.run_gc().unwrap();
        for id in [first_id, second_id, target_id, buffer_id] {
            assert!(runtime.0.state.borrow().heap.object(id).is_err());
        }
        drop(context);
        drop(runtime);
        assert!(weak.upgrade().is_none());
    }
}
