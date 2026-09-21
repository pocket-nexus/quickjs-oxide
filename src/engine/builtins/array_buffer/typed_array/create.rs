//! TypedArray construction and static factories own inputs until all writes finish.
use crate::engine::{
    api::{error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError},
    builtins::native::TypedArrayElementKind,
    heap::ContextId,
    object::{CallableRef, ObjectRef, PropertyKey},
    value::{JsValue, Value, conversion::NativeConversion},
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
    Primitive { resume: TypedCreateResume },
    Prototype { resume: TypedCreateResume },
    Read { resume: TypedCreateResume },
    Method { resume: TypedCreateResume },
    Collect { resume: TypedCreateResume },
    Create { resume: TypedCreateResume },
    Call { resume: TypedCreateResume },
    Element { resume: TypedCreateResume },
}
pub(crate) struct TypedCreateResume(Box<TypedCreateResumeState>);
impl std::ops::Deref for TypedCreateResume {
    type Target = TypedCreateResumeState;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for TypedCreateResume {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
const _: () = assert!(std::mem::size_of::<TypedCreateResume>() <= 8);
pub(crate) struct TypedCreateResumeState {
    pending_effect: TypedCreateStepPending,
    realm: ContextId,
    phase: Phase,
}
enum Allocation {
    Intrinsic {
        prototype: ObjectRef,
        element: TypedArrayElementKind,
    },
    Static(JsValue),
}
enum Input {
    Values {
        runtime: Runtime,
        values: Vec<JsValue>,
    },
    Object(ObjectRef),
}
struct Factory {
    runtime: Runtime,
    source: JsValue,
    allocation: Allocation,
    mapper: Option<CallableRef>,
    this_arg: JsValue,
}
struct Population {
    runtime: Runtime,
    source: Input,
    target: ObjectRef,
    mapper: Option<CallableRef>,
    this_arg: JsValue,
    length: u64,
    index: u64,
}
enum ProtoPurpose {
    Length(u64),
    Buffer {
        source: ObjectRef,
        arguments: BufferArguments,
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
        arguments: BufferArguments,
    },
    BufferLength {
        prototype: ObjectRef,
        element: TypedArrayElementKind,
        source: ObjectRef,
        offset: u64,
    },
    Iterator {
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
struct BufferArguments {
    runtime: Runtime,
    offset: Option<JsValue>,
    length: Option<JsValue>,
}
impl Drop for BufferArguments {
    fn drop(&mut self) {
        for value in [self.offset.take(), self.length.take()]
            .into_iter()
            .flatten()
        {
            let _ = self.runtime.release_jsvalue(value);
        }
    }
}
impl Drop for Factory {
    fn drop(&mut self) {
        for value in [
            std::mem::replace(&mut self.source, JsValue::Undefined),
            std::mem::replace(&mut self.this_arg, JsValue::Undefined),
        ] {
            let _ = self.runtime.release_jsvalue(value);
        }
        if let Allocation::Static(value) =
            std::mem::replace(&mut self.allocation, Allocation::Static(JsValue::Undefined))
        {
            let _ = self.runtime.release_jsvalue(value);
        }
    }
}
impl Drop for Input {
    fn drop(&mut self) {
        if let Self::Values { runtime, values } = self {
            for value in values.drain(..) {
                let _ = runtime.release_jsvalue(value);
            }
        }
    }
}
impl Drop for Population {
    fn drop(&mut self) {
        let _ = self
            .runtime
            .release_jsvalue(std::mem::replace(&mut self.this_arg, JsValue::Undefined));
    }
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
        let first = arguments.readable.first().ok_or(RuntimeError::Invariant(
            "TypedArray constructor argv was not padded",
        ))?;
        let purpose = if let JsValue::Object(source) = first {
            let source = ObjectRef::from_borrowed_handle(runtime.clone(), *source)?;
            if !source.belongs_to(runtime) {
                return Err(RuntimeError::WrongRuntime("TypedArray constructor source"));
            }
            if runtime
                .snapshot_buffer_access_if_branded(&source)?
                .is_some()
            {
                let mut buffer_arguments = BufferArguments {
                    runtime: runtime.clone(),
                    offset: None,
                    length: None,
                };
                if arguments.actual_arg_count > 1 {
                    buffer_arguments.offset =
                        Some(runtime.dup_jsvalue(arguments.readable.get(1).ok_or(
                            RuntimeError::Invariant("TypedArray byteOffset argv was not padded"),
                        )?)?);
                }
                if arguments.actual_arg_count > 2
                    && !matches!(arguments.readable.get(2), Some(JsValue::Undefined))
                {
                    buffer_arguments.length =
                        Some(runtime.dup_jsvalue(arguments.readable.get(2).ok_or(
                            RuntimeError::Invariant("TypedArray length argv was not padded"),
                        )?)?);
                }
                ProtoPurpose::Buffer {
                    source,
                    arguments: buffer_arguments,
                }
            } else if let Some(snapshot) = runtime.typed_array_snapshot_if_branded(&source)? {
                let length = u64::from(runtime.typed_array_state_from_snapshot(snapshot)?.length);
                ProtoPurpose::Typed { source, length }
            } else {
                ProtoPurpose::Object(source)
            }
        } else {
            let length = match primitive_index(runtime, realm, runtime.dup_jsvalue(first)?)? {
                NativeConversion::Value(value) => value,
                NativeConversion::Throw(value) => {
                    return Ok(Self::Complete(Completion::Throw(
                        runtime.into_jsvalue(value)?,
                    )));
                }
            };
            ProtoPurpose::Length(length)
        };
        Ok(Self::request_prototype(
            runtime.dup_jsvalue(new_target)?,
            TypedCreateResume(Box::new(TypedCreateResumeState {
                pending_effect: TypedCreateStepPending::new(runtime.clone()),
                realm,
                phase: Phase::Prototype { element, purpose },
            })),
        ))
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
        let source = arguments.readable.first().ok_or(RuntimeError::Invariant(
            "TypedArray.from argv was not padded",
        ))?;
        let mapper = if arguments.actual_arg_count > 1
            && !matches!(arguments.readable[1], JsValue::Undefined)
        {
            let callable = match &arguments.readable[1] {
                JsValue::Object(id) => runtime.as_callable_object(*id)?,
                _ => None,
            };
            let Some(callable) = callable else {
                return Ok(Self::Complete(Completion::Throw(
                    runtime.new_native_error_jsvalue(
                        realm,
                        NativeErrorKind::Type,
                        "not a function",
                    )?,
                )));
            };
            Some(callable)
        } else {
            None
        };
        let message = match source {
            JsValue::Undefined => Some("cannot read property 'Symbol.iterator' of undefined"),
            JsValue::Null => Some("cannot read property 'Symbol.iterator' of null"),
            _ => None,
        };
        if let Some(message) = message {
            return Ok(Self::Complete(Completion::Throw(
                runtime.new_native_error_jsvalue(realm, NativeErrorKind::Type, message)?,
            )));
        }
        let mut factory = Factory {
            runtime: runtime.clone(),
            source: JsValue::Undefined,
            allocation: Allocation::Static(JsValue::Undefined),
            mapper,
            this_arg: JsValue::Undefined,
        };
        factory.allocation = Allocation::Static(runtime.dup_jsvalue(this_value)?);
        factory.source = runtime.dup_jsvalue(source)?;
        if arguments.actual_arg_count > 2 {
            factory.this_arg = runtime.dup_jsvalue(&arguments.readable[2])?;
        }
        TypedCreateResume::iterator(runtime, realm, factory)
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
        let mut source = Input::Values {
            runtime: runtime.clone(),
            values: Vec::new(),
        };
        let Input::Values { values, .. } = &mut source else {
            unreachable!()
        };
        if values
            .try_reserve_exact(arguments.actual_arg_count)
            .is_err()
        {
            return out_of_memory(runtime, realm);
        }
        for value in &arguments.readable[..arguments.actual_arg_count] {
            values.push(runtime.dup_jsvalue(value)?);
        }
        let factory = Factory {
            runtime: runtime.clone(),
            source: JsValue::Undefined,
            allocation: Allocation::Static(runtime.dup_jsvalue(this_value)?),
            mapper: None,
            this_arg: JsValue::Undefined,
        };
        TypedCreateResume::allocate(runtime, realm, source, factory, length)
    }
}

impl TypedCreateResume {
    fn iterator(
        runtime: &Runtime,
        realm: ContextId,
        factory: Factory,
    ) -> Result<TypedCreateStep, RuntimeError> {
        Ok(TypedCreateStep::request_method(
            runtime.dup_jsvalue(&factory.source)?,
            Self(Box::new(TypedCreateResumeState {
                pending_effect: TypedCreateStepPending::new(runtime.clone()),
                realm,
                phase: Phase::Iterator { factory },
            })),
        ))
    }
    pub(crate) fn method(
        self,
        runtime: &Runtime,
        result: NativeConversion<Option<CallableRef>>,
    ) -> Result<TypedCreateStep, RuntimeError> {
        let method = match result {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => {
                return Ok(TypedCreateStep::Complete(Completion::Throw(
                    runtime.into_jsvalue(value)?,
                )));
            }
        };
        let Phase::Iterator { mut factory } = self.0.phase else {
            return Err(RuntimeError::Invariant(
                "TypedArray iterator method reply in wrong phase",
            ));
        };
        if let Some(method) = method {
            let element = match &factory.allocation {
                Allocation::Intrinsic { element, .. } => *element,
                Allocation::Static(_) => TypedArrayElementKind::Uint8,
            };
            Ok(TypedCreateStep::request_collect(
                std::mem::replace(&mut factory.source, JsValue::Undefined),
                method,
                element,
                Self(Box::new(TypedCreateResumeState {
                    pending_effect: TypedCreateStepPending::new(runtime.clone()),
                    realm: self.0.realm,
                    phase: Phase::Collect(factory),
                })),
            ))
        } else {
            let source = match runtime.native_to_object_jsvalue(
                self.0.realm,
                std::mem::replace(&mut factory.source, JsValue::Undefined),
            )? {
                NativeConversion::Value(value) => value,
                NativeConversion::Throw(value) => {
                    return Ok(TypedCreateStep::Complete(Completion::Throw(
                        runtime.into_jsvalue(value)?,
                    )));
                }
            };
            let key =
                runtime.pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Length)?;
            Ok(TypedCreateStep::request_read(
                JsValue::Object(source.clone().into_handle()),
                key,
                Self(Box::new(TypedCreateResumeState {
                    pending_effect: TypedCreateStepPending::new(runtime.clone()),
                    realm: self.0.realm,
                    phase: Phase::Length { source, factory },
                })),
            ))
        }
    }

    pub(crate) fn prototype(
        self,
        runtime: &Runtime,
        result: NativeConversion<ConstructorPrototypeSource>,
    ) -> Result<TypedCreateStep, RuntimeError> {
        let Phase::Prototype { element, purpose } = self.0.phase else {
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
                return Ok(TypedCreateStep::Complete(Completion::Throw(
                    runtime.into_jsvalue(value)?,
                )));
            }
        };
        match purpose {
            ProtoPurpose::Length(length) => complete_object(
                runtime,
                runtime.new_typed_array_for_length(self.0.realm, &prototype, element, length)?,
            ),
            ProtoPurpose::Typed { source, length } => {
                let snapshot = runtime.typed_array_snapshot(&source)?;
                complete_object(
                    runtime,
                    runtime.typed_array_copy_into_new(
                        self.0.realm,
                        &prototype,
                        element,
                        &source,
                        snapshot,
                        length,
                    )?,
                )
            }
            ProtoPurpose::Object(source) => Self::iterator(
                runtime,
                self.0.realm,
                Factory {
                    runtime: runtime.clone(),
                    source: JsValue::Object(source.into_handle()),
                    allocation: Allocation::Intrinsic { prototype, element },
                    mapper: None,
                    this_arg: JsValue::Undefined,
                },
            ),
            ProtoPurpose::Buffer {
                source,
                mut arguments,
            } => {
                let offset = arguments.offset.take();
                let resume = Self(Box::new(TypedCreateResumeState {
                    pending_effect: TypedCreateStepPending::new(runtime.clone()),
                    realm: self.0.realm,
                    phase: Phase::Offset {
                        prototype,
                        element,
                        source,
                        arguments,
                    },
                }));
                if let Some(value) = offset {
                    Ok(TypedCreateStep::request_primitive(value, resume))
                } else {
                    resume.resume(runtime, Completion::Return(JsValue::Int(0)))
                }
            }
        }
    }
    pub(crate) fn collected(
        self,
        runtime: &Runtime,
        result: NativeConversion<Vec<JsValue>>,
    ) -> Result<TypedCreateStep, RuntimeError> {
        let values = match result {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => {
                return Ok(TypedCreateStep::Complete(Completion::Throw(
                    runtime.into_jsvalue(value)?,
                )));
            }
        };
        let length = values.len() as u64;
        let source = Input::Values {
            runtime: runtime.clone(),
            values,
        };
        let Phase::Collect(factory) = self.0.phase else {
            return Err(RuntimeError::Invariant(
                "TypedArray collected reply in wrong phase",
            ));
        };
        Self::allocate(runtime, self.0.realm, source, factory, length)
    }
    fn allocate(
        runtime: &Runtime,
        realm: ContextId,
        source: Input,
        mut factory: Factory,
        length: u64,
    ) -> Result<TypedCreateStep, RuntimeError> {
        match &factory.allocation {
            Allocation::Intrinsic { prototype, element } => {
                let target =
                    match runtime.new_typed_array_for_length(realm, prototype, *element, length)? {
                        NativeConversion::Value(value) => value,
                        NativeConversion::Throw(value) => {
                            return Ok(TypedCreateStep::Complete(Completion::Throw(
                                runtime.into_jsvalue(value)?,
                            )));
                        }
                    };
                Population {
                    runtime: runtime.clone(),
                    source,
                    target,
                    mapper: factory.mapper.take(),
                    this_arg: std::mem::replace(&mut factory.this_arg, JsValue::Undefined),
                    length,
                    index: 0,
                }
                .next(runtime, realm)
            }
            Allocation::Static(constructor) => {
                if !matches!(constructor, JsValue::Object(_)) {
                    return Ok(TypedCreateStep::Complete(Completion::Throw(
                        runtime.new_native_error_jsvalue(
                            realm,
                            NativeErrorKind::Type,
                            "not a function",
                        )?,
                    )));
                }
                Ok(TypedCreateStep::request_create(
                    runtime.dup_jsvalue(constructor)?,
                    length,
                    Self(Box::new(TypedCreateResumeState {
                        pending_effect: TypedCreateStepPending::new(runtime.clone()),
                        realm,
                        phase: Phase::Create {
                            source,
                            factory,
                            length,
                        },
                    })),
                ))
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
                return Ok(TypedCreateStep::Complete(Completion::Throw(
                    runtime.into_jsvalue(value)?,
                )));
            }
        };
        let Phase::Create {
            source,
            mut factory,
            length,
        } = self.0.phase
        else {
            return Err(RuntimeError::Invariant(
                "TypedArray create reply in wrong phase",
            ));
        };
        Population {
            runtime: runtime.clone(),
            source,
            target,
            mapper: factory.mapper.take(),
            this_arg: std::mem::replace(&mut factory.this_arg, JsValue::Undefined),
            length,
            index: 0,
        }
        .next(runtime, self.0.realm)
    }
    pub(crate) fn element(
        self,
        runtime: &Runtime,
        result: NativeConversion<[u8; 8]>,
    ) -> Result<TypedCreateStep, RuntimeError> {
        let bytes = match result {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => {
                return Ok(TypedCreateStep::Complete(Completion::Throw(
                    runtime.into_jsvalue(value)?,
                )));
            }
        };
        let Phase::Element(mut population) = self.0.phase else {
            return Err(RuntimeError::Invariant(
                "TypedArray create element reply in wrong phase",
            ));
        };
        runtime.typed_array_write_converted_index(&population.target, population.index, &bytes)?;
        population.index += 1;
        population.next(runtime, self.0.realm)
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
        match self.0.phase {
            Phase::Offset {
                prototype,
                element,
                source,
                mut arguments,
            } => {
                let offset = match primitive_index(runtime, self.0.realm, value)? {
                    NativeConversion::Value(value) => value,
                    NativeConversion::Throw(value) => {
                        return Ok(TypedCreateStep::Complete(Completion::Throw(
                            runtime.into_jsvalue(value)?,
                        )));
                    }
                };
                if offset % u64::from(element.byte_length()) != 0 {
                    return Ok(TypedCreateStep::Complete(Completion::Throw(
                        runtime.new_native_error_jsvalue(
                            self.0.realm,
                            NativeErrorKind::Range,
                            "invalid offset",
                        )?,
                    )));
                }
                if let Some(value) = arguments.length.take() {
                    Ok(TypedCreateStep::request_primitive(
                        value,
                        Self(Box::new(TypedCreateResumeState {
                            pending_effect: TypedCreateStepPending::new(runtime.clone()),
                            realm: self.0.realm,
                            phase: Phase::BufferLength {
                                prototype,
                                element,
                                source,
                                offset,
                            },
                        })),
                    ))
                } else {
                    complete_object(
                        runtime,
                        runtime.new_typed_array_constructor_view_from_coerced(
                            self.0.realm,
                            &prototype,
                            element,
                            &source,
                            offset,
                            None,
                        )?,
                    )
                }
            }
            Phase::BufferLength {
                prototype,
                element,
                source,
                offset,
            } => {
                let length = match primitive_index(runtime, self.0.realm, value)? {
                    NativeConversion::Value(value) => value,
                    NativeConversion::Throw(value) => {
                        return Ok(TypedCreateStep::Complete(Completion::Throw(
                            runtime.into_jsvalue(value)?,
                        )));
                    }
                };
                complete_object(
                    runtime,
                    runtime.new_typed_array_constructor_view_from_coerced(
                        self.0.realm,
                        &prototype,
                        element,
                        &source,
                        offset,
                        Some(length),
                    )?,
                )
            }
            Phase::Length { source, factory } => Ok(TypedCreateStep::request_primitive(
                value,
                Self(Box::new(TypedCreateResumeState {
                    pending_effect: TypedCreateStepPending::new(runtime.clone()),
                    realm: self.0.realm,
                    phase: Phase::LengthPrimitive { source, factory },
                })),
            )),
            Phase::LengthPrimitive { source, factory } => {
                let number = runtime.number_from_primitive_jsvalue(self.0.realm, &value);
                runtime.release_jsvalue(value)?;
                let length = match number? {
                    NativeConversion::Value(value) => Runtime::length_from_number(value),
                    NativeConversion::Throw(value) => {
                        return Ok(TypedCreateStep::Complete(Completion::Throw(
                            runtime.into_jsvalue(value)?,
                        )));
                    }
                };
                Self::allocate(
                    runtime,
                    self.0.realm,
                    Input::Object(source),
                    factory,
                    length,
                )
            }
            Phase::Index(population) => population.map(runtime, self.0.realm, value),
            Phase::Mapped(population) => population.convert(runtime, self.0.realm, value),
            _ => {
                runtime.release_jsvalue(value)?;
                Err(RuntimeError::Invariant(
                    "TypedArray create value reply in wrong phase",
                ))
            }
        }
    }
}
impl Population {
    fn next(self, runtime: &Runtime, realm: ContextId) -> Result<TypedCreateStep, RuntimeError> {
        if self.index == self.length {
            return Ok(TypedCreateStep::Complete(Completion::Return(
                JsValue::Object(self.target.clone().into_handle()),
            )));
        }
        match &self.source {
            Input::Object(source) => {
                let key = runtime.property_key_for_index(self.index)?;
                Ok(TypedCreateStep::request_read(
                    JsValue::Object(source.clone().into_handle()),
                    key,
                    TypedCreateResume(Box::new(TypedCreateResumeState {
                        pending_effect: TypedCreateStepPending::new(runtime.clone()),
                        realm,
                        phase: Phase::Index(self),
                    })),
                ))
            }
            Input::Values { values, .. } => {
                let value = runtime.dup_jsvalue(
                    &values[usize::try_from(self.index).map_err(|_| {
                        RuntimeError::Invariant("TypedArray materialized index overflowed usize")
                    })?],
                )?;
                self.map(runtime, realm, value)
            }
        }
    }
    fn map(
        self,
        runtime: &Runtime,
        realm: ContextId,
        value: JsValue,
    ) -> Result<TypedCreateStep, RuntimeError> {
        if let Some(mapper) = &self.mapper {
            let mut arguments = Vec::new();
            if arguments.try_reserve_exact(2).is_err() {
                runtime.release_jsvalue(value)?;
                return out_of_memory(runtime, realm);
            }
            let receiver = match runtime.dup_jsvalue(&self.this_arg) {
                Ok(value) => value,
                Err(error) => {
                    let _ = runtime.release_jsvalue(value);
                    return Err(error);
                }
            };
            arguments.push(value);
            arguments.push(
                crate::engine::value::number::operations::Number::compact(self.index as f64).into(),
            );
            Ok(TypedCreateStep::request_call(
                DirectCallTarget::Callable(mapper.clone()),
                receiver,
                arguments,
                TypedCreateResume(Box::new(TypedCreateResumeState {
                    pending_effect: TypedCreateStepPending::new(runtime.clone()),
                    realm,
                    phase: Phase::Mapped(self),
                })),
            ))
        } else {
            self.convert(runtime, realm, value)
        }
    }
    fn convert(
        self,
        runtime: &Runtime,
        realm: ContextId,
        value: JsValue,
    ) -> Result<TypedCreateStep, RuntimeError> {
        let element = match runtime.typed_array_snapshot(&self.target) {
            Ok(snapshot) => snapshot.element,
            Err(error) => {
                let _ = runtime.release_jsvalue(value);
                return Err(error);
            }
        };
        Ok(TypedCreateStep::request_element(
            element,
            value,
            TypedCreateResume(Box::new(TypedCreateResumeState {
                pending_effect: TypedCreateStepPending::new(runtime.clone()),
                realm,
                phase: Phase::Element(self),
            })),
        ))
    }
}
fn primitive_index(
    runtime: &Runtime,
    realm: ContextId,
    value: JsValue,
) -> Result<NativeConversion<u64>, RuntimeError> {
    let number = runtime.number_from_primitive_jsvalue(realm, &value);
    runtime.release_jsvalue(value)?;
    match number? {
        NativeConversion::Value(number) => runtime.index_from_number(realm, number),
        NativeConversion::Throw(value) => Ok(NativeConversion::Throw(value)),
    }
}
fn complete_object(
    runtime: &Runtime,
    result: NativeConversion<ObjectRef>,
) -> Result<TypedCreateStep, RuntimeError> {
    Ok(TypedCreateStep::Complete(match result {
        NativeConversion::Value(value) => {
            Completion::Return(runtime.into_jsvalue(Value::Object(value))?)
        }
        NativeConversion::Throw(value) => Completion::Throw(runtime.into_jsvalue(value)?),
    }))
}
fn out_of_memory(runtime: &Runtime, realm: ContextId) -> Result<TypedCreateStep, RuntimeError> {
    Ok(TypedCreateStep::Complete(Completion::Throw(
        runtime.new_native_error_jsvalue(realm, NativeErrorKind::Internal, "out of memory")?,
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
            TypedCreateStep::Primitive { mut resume } => {
                let value = resume.take_primitive_value();
                {
                    let result = if matches!(value, JsValue::Object(_)) {
                        runtime.to_primitive_jsvalue(realm, value, ToPrimitiveHint::Number)?
                    } else {
                        Completion::Return(value)
                    };
                    resume.resume(runtime, result)?
                }
            }
            TypedCreateStep::Prototype { mut resume } => {
                let new_target = resume.take_prototype_new_target();
                resume.prototype(
                    runtime,
                    finish_source(
                        runtime,
                        realm,
                        ProtoSourceStep::start(runtime, realm, new_target)?,
                    )?,
                )?
            }
            TypedCreateStep::Read { mut resume } => {
                let receiver = runtime.root_and_release_jsvalue(resume.take_read_receiver())?;
                let key = resume.take_read_key();
                resume.resume(
                    runtime,
                    runtime.get_value_property_in_realm(realm, receiver, &key)?,
                )?
            }
            TypedCreateStep::Method { mut resume } => {
                let source = resume.take_method_source();
                resume.method(runtime, runtime.typed_array_iterator_method(realm, source)?)?
            }
            TypedCreateStep::Collect { mut resume } => {
                let source = resume.take_collect_source();
                let method = resume.take_collect_method();
                let element = resume.take_collect_element();
                resume.collected(
                    runtime,
                    runtime.collect_typed_array_iterator(realm, source, &method, element)?,
                )?
            }
            TypedCreateStep::Create { mut resume } => {
                let constructor = resume.take_create_constructor();
                let length = resume.take_create_length();
                resume.created(
                    runtime,
                    runtime.typed_array_create_from_static_constructor(
                        realm,
                        constructor,
                        length,
                    )?,
                )?
            }
            TypedCreateStep::Call { mut resume } => {
                let target = resume.take_call_target();
                let DirectCallTarget::Callable(callable) = target else {
                    return Err(RuntimeError::Invariant(
                        "TypedArray create invalid call target",
                    ));
                };
                let receiver = resume.take_call_receiver();
                let arguments = resume.take_call_arguments();
                resume.resume(
                    runtime,
                    runtime.call_internal_jsvalue(realm, &callable, receiver, arguments)?,
                )?
            }
            TypedCreateStep::Element { mut resume } => {
                let element = resume.take_element_element();
                let value = resume.take_element_value();
                resume.element(
                    runtime,
                    super::element::ElementStep::start(runtime, realm, element, value)?
                        .finish_sync(runtime, realm)?,
                )?
            }
        };
    }
}

struct TypedCreateStepPending {
    runtime: Runtime,
    primitive_value: Option<JsValue>,
    prototype_new_target: Option<JsValue>,
    read_receiver: Option<JsValue>,
    read_key: Option<PropertyKey>,
    method_source: Option<JsValue>,
    collect_source: Option<JsValue>,
    collect_method: Option<CallableRef>,
    collect_element: Option<TypedArrayElementKind>,
    create_constructor: Option<JsValue>,
    create_length: Option<u64>,
    call_target: Option<DirectCallTarget>,
    call_receiver: Option<JsValue>,
    call_arguments: Option<Vec<JsValue>>,
    element_element: Option<TypedArrayElementKind>,
    element_value: Option<JsValue>,
}
impl TypedCreateStepPending {
    fn new(runtime: Runtime) -> Self {
        Self {
            runtime,
            primitive_value: None,
            prototype_new_target: None,
            read_receiver: None,
            read_key: None,
            method_source: None,
            collect_source: None,
            collect_method: None,
            collect_element: None,
            create_constructor: None,
            create_length: None,
            call_target: None,
            call_receiver: None,
            call_arguments: None,
            element_element: None,
            element_value: None,
        }
    }
}
impl Drop for TypedCreateStepPending {
    /// Release the internal edges still held when the request is abandoned.
    /// Consumption goes through `Option::take`; releases are defer-safe and
    /// nothrow.
    fn drop(&mut self) {
        for value in [
            self.primitive_value.take(),
            self.prototype_new_target.take(),
            self.read_receiver.take(),
            self.method_source.take(),
            self.collect_source.take(),
            self.create_constructor.take(),
            self.call_receiver.take(),
            self.element_value.take(),
        ]
        .into_iter()
        .flatten()
        {
            let _ = self.runtime.release_jsvalue(value);
        }
        if let Some(values) = self.call_arguments.take() {
            for value in values {
                let _ = self.runtime.release_jsvalue(value);
            }
        }
    }
}
impl TypedCreateStep {
    pub(crate) fn request_primitive(value: JsValue, mut resume: TypedCreateResume) -> Self {
        resume.0.pending_effect.primitive_value = Some(value);
        Self::Primitive { resume }
    }
    pub(crate) fn request_prototype(new_target: JsValue, mut resume: TypedCreateResume) -> Self {
        resume.0.pending_effect.prototype_new_target = Some(new_target);
        Self::Prototype { resume }
    }
    pub(crate) fn request_read(
        receiver: JsValue,
        key: PropertyKey,
        mut resume: TypedCreateResume,
    ) -> Self {
        resume.0.pending_effect.read_receiver = Some(receiver);
        resume.0.pending_effect.read_key = Some(key);
        Self::Read { resume }
    }
    pub(crate) fn request_method(source: JsValue, mut resume: TypedCreateResume) -> Self {
        resume.0.pending_effect.method_source = Some(source);
        Self::Method { resume }
    }
    pub(crate) fn request_collect(
        source: JsValue,
        method: CallableRef,
        element: TypedArrayElementKind,
        mut resume: TypedCreateResume,
    ) -> Self {
        resume.0.pending_effect.collect_source = Some(source);
        resume.0.pending_effect.collect_method = Some(method);
        resume.0.pending_effect.collect_element = Some(element);
        Self::Collect { resume }
    }
    pub(crate) fn request_create(
        constructor: JsValue,
        length: u64,
        mut resume: TypedCreateResume,
    ) -> Self {
        resume.0.pending_effect.create_constructor = Some(constructor);
        resume.0.pending_effect.create_length = Some(length);
        Self::Create { resume }
    }
    pub(crate) fn request_call(
        target: DirectCallTarget,
        receiver: JsValue,
        arguments: Vec<JsValue>,
        mut resume: TypedCreateResume,
    ) -> Self {
        resume.0.pending_effect.call_target = Some(target);
        resume.0.pending_effect.call_receiver = Some(receiver);
        resume.0.pending_effect.call_arguments = Some(arguments);
        Self::Call { resume }
    }
    pub(crate) fn request_element(
        element: TypedArrayElementKind,
        value: JsValue,
        mut resume: TypedCreateResume,
    ) -> Self {
        resume.0.pending_effect.element_element = Some(element);
        resume.0.pending_effect.element_value = Some(value);
        Self::Element { resume }
    }
}
impl TypedCreateResume {
    pub(crate) fn take_primitive_value(&mut self) -> JsValue {
        self.0
            .pending_effect
            .primitive_value
            .take()
            .expect("TypedCreateStep Primitive value")
    }
    pub(crate) fn take_prototype_new_target(&mut self) -> JsValue {
        self.0
            .pending_effect
            .prototype_new_target
            .take()
            .expect("TypedCreateStep Prototype new_target")
    }
    pub(crate) fn take_read_receiver(&mut self) -> JsValue {
        self.0
            .pending_effect
            .read_receiver
            .take()
            .expect("TypedCreateStep Read receiver")
    }
    pub(crate) fn take_read_key(&mut self) -> PropertyKey {
        self.0
            .pending_effect
            .read_key
            .take()
            .expect("TypedCreateStep Read key")
    }
    pub(crate) fn take_method_source(&mut self) -> JsValue {
        self.0
            .pending_effect
            .method_source
            .take()
            .expect("TypedCreateStep Method source")
    }
    pub(crate) fn take_collect_source(&mut self) -> JsValue {
        self.0
            .pending_effect
            .collect_source
            .take()
            .expect("TypedCreateStep Collect source")
    }
    pub(crate) fn take_collect_method(&mut self) -> CallableRef {
        self.0
            .pending_effect
            .collect_method
            .take()
            .expect("TypedCreateStep Collect method")
    }
    pub(crate) fn take_collect_element(&mut self) -> TypedArrayElementKind {
        self.0
            .pending_effect
            .collect_element
            .take()
            .expect("TypedCreateStep Collect element")
    }
    pub(crate) fn take_create_constructor(&mut self) -> JsValue {
        self.0
            .pending_effect
            .create_constructor
            .take()
            .expect("TypedCreateStep Create constructor")
    }
    pub(crate) fn take_create_length(&mut self) -> u64 {
        self.0
            .pending_effect
            .create_length
            .take()
            .expect("TypedCreateStep Create length")
    }
    pub(crate) fn take_call_target(&mut self) -> DirectCallTarget {
        self.0
            .pending_effect
            .call_target
            .take()
            .expect("TypedCreateStep Call target")
    }
    pub(crate) fn take_call_receiver(&mut self) -> JsValue {
        self.0
            .pending_effect
            .call_receiver
            .take()
            .expect("TypedCreateStep Call receiver")
    }
    pub(crate) fn take_call_arguments(&mut self) -> Vec<JsValue> {
        self.0
            .pending_effect
            .call_arguments
            .take()
            .expect("TypedCreateStep Call arguments")
    }
    pub(crate) fn take_element_element(&mut self) -> TypedArrayElementKind {
        self.0
            .pending_effect
            .element_element
            .take()
            .expect("TypedCreateStep Element element")
    }
    pub(crate) fn take_element_value(&mut self) -> JsValue {
        self.0
            .pending_effect
            .element_value
            .take()
            .expect("TypedCreateStep Element value")
    }
}
const _: () = assert!(std::mem::size_of::<TypedCreateStep>() <= 64);

// S11 all-domain protocol bound; inline completion stays allocation-free.
const _: () = assert!(std::mem::size_of::<TypedCreateStep>() <= 64);

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
            runtime: runtime.clone(),
            source: Input::Values {
                runtime: runtime.clone(),
                values: vec![
                    JsValue::Object(first.into_handle()),
                    JsValue::Object(second.into_handle()),
                ],
            },
            target,
            mapper: None,
            this_arg: JsValue::Undefined,
            length: 2,
            index: 0,
        };
        let TypedCreateStep::Element { mut resume } = population.next(&runtime, realm).unwrap()
        else {
            panic!("expected first conversion");
        };
        let _ = resume.take_element_element();
        runtime
            .release_jsvalue(resume.take_element_value())
            .unwrap();
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
