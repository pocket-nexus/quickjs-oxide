//! TypedArray iteration owns callbacks, species results, and live element reads.
#[cfg(test)]
use super::*;
use crate::engine::{
    api::{error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError},
    builtins::native::{ArrayIterationKind, TypedArrayElementKind},
    heap::ContextId,
    object::{CallableRef, DescriptorField, ObjectRef, OrdinaryPropertyDescriptor, PropertyKey},
    value::{Value, conversion::NativeConversion},
    vm::{
        Completion,
        call::{DirectCallTarget, NativeArguments, NativeInvocation},
    },
};
#[cfg(test)]
mod tests;
#[cfg(test)]
mod transform_tests;

pub(crate) enum TypedIterationStep {
    Complete(Completion),
    Species {
        source: ObjectRef,
        element: TypedArrayElementKind,
        length: u64,
        resume: TypedIterationResume,
    },
    Call {
        target: DirectCallTarget,
        receiver: Value,
        arguments: Vec<Value>,
        resume: TypedIterationResume,
    },
    Element {
        element: TypedArrayElementKind,
        value: Value,
        resume: TypedIterationResume,
    },
    Read {
        object: ObjectRef,
        key: PropertyKey,
        resume: TypedIterationResume,
    },
}
pub(crate) struct TypedIterationResume {
    realm: ContextId,
    phase: IterationPhase,
}
struct IterationInput {
    target: ObjectRef,
    callback: CallableRef,
    this_arg: Value,
    element: TypedArrayElementKind,
    length: u64,
}
struct IterationState {
    input: IterationInput,
    mode: IterationMode,
    index: u64,
}
enum IterationMode {
    Every,
    Some,
    ForEach,
    Map(ObjectRef),
    Filter { selected: ObjectRef, length: u64 },
}
enum IterationPhase {
    MapSpecies(IterationInput),
    Called {
        state: IterationState,
        value: Value,
        index: u64,
    },
    Mapped {
        state: IterationState,
        index: u64,
    },
    FilterSpecies(ObjectRef),
    FilterMethod {
        target: ObjectRef,
        selected: ObjectRef,
    },
    FilterCalled(ObjectRef),
}
impl TypedIterationStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        kind: ArrayIterationKind,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        let NativeInvocation::Call { this_value } = invocation else {
            return Err(RuntimeError::Invariant(
                "TypedArray.prototype iteration received a constructor invocation",
            ));
        };
        let target = match runtime.require_typed_array(realm, this_value.clone())? {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => return Ok(Self::Complete(Completion::Throw(value))),
        };
        let length = match runtime.typed_array_validated_length(realm, &target)? {
            NativeConversion::Value(value) => u64::from(value),
            NativeConversion::Throw(value) => return Ok(Self::Complete(Completion::Throw(value))),
        };
        let callback = runtime.callable_from_value(
            arguments
                .readable
                .first()
                .ok_or(RuntimeError::Invariant(
                    "TypedArray iteration callback argv was not padded",
                ))?
                .clone(),
        )?;
        let this_arg = if arguments.actual_arg_count > 1 {
            arguments
                .readable
                .get(1)
                .ok_or(RuntimeError::Invariant(
                    "TypedArray iteration thisArg was missing",
                ))?
                .clone()
        } else {
            Value::Undefined
        };
        let element = runtime.typed_array_snapshot(&target)?.element;
        let input = IterationInput {
            target,
            callback,
            this_arg,
            element,
            length,
        };
        let mode = match kind {
            ArrayIterationKind::Every => IterationMode::Every,
            ArrayIterationKind::Some => IterationMode::Some,
            ArrayIterationKind::ForEach => IterationMode::ForEach,
            ArrayIterationKind::Map => {
                return Ok(Self::Species {
                    source: input.target.clone(),
                    element,
                    length,
                    resume: TypedIterationResume {
                        realm,
                        phase: IterationPhase::MapSpecies(input),
                    },
                });
            }
            ArrayIterationKind::Filter => IterationMode::Filter {
                selected: runtime.new_array(realm)?,
                length: 0,
            },
        };
        TypedIterationResume::next(
            runtime,
            realm,
            IterationState {
                input,
                mode,
                index: 0,
            },
        )
    }
}
impl TypedIterationResume {
    fn next(
        runtime: &Runtime,
        realm: ContextId,
        mut state: IterationState,
    ) -> Result<TypedIterationStep, RuntimeError> {
        if state.index == state.input.length {
            return Ok(TypedIterationStep::Complete(Completion::Return(
                match state.mode {
                    IterationMode::Every => Value::Bool(true),
                    IterationMode::Some => Value::Bool(false),
                    IterationMode::ForEach => Value::Undefined,
                    IterationMode::Map(target) => Value::Object(target),
                    IterationMode::Filter { selected, length } => {
                        return Ok(TypedIterationStep::Species {
                            source: state.input.target,
                            element: state.input.element,
                            length,
                            resume: Self {
                                realm,
                                phase: IterationPhase::FilterSpecies(selected),
                            },
                        });
                    }
                },
            )));
        }
        let index = state.index;
        state.index += 1;
        let value = runtime
            .typed_array_read_index(&state.input.target, index)?
            .unwrap_or(Value::Undefined);
        let mut arguments = Vec::new();
        if arguments.try_reserve_exact(3).is_err() {
            return iteration_oom(runtime, realm);
        }
        arguments.push(value.clone());
        arguments.push(Value::number(index as f64));
        arguments.push(Value::Object(state.input.target.clone()));
        Ok(TypedIterationStep::Call {
            target: DirectCallTarget::Callable(state.input.callback.clone()),
            receiver: state.input.this_arg.clone(),
            arguments,
            resume: Self {
                realm,
                phase: IterationPhase::Called {
                    state,
                    value,
                    index,
                },
            },
        })
    }
    pub(crate) fn species(
        self,
        runtime: &Runtime,
        result: NativeConversion<ObjectRef>,
    ) -> Result<TypedIterationStep, RuntimeError> {
        let target = match result {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => {
                return Ok(TypedIterationStep::Complete(Completion::Throw(value)));
            }
        };
        match self.phase {
            IterationPhase::MapSpecies(input) => Self::next(
                runtime,
                self.realm,
                IterationState {
                    input,
                    mode: IterationMode::Map(target),
                    index: 0,
                },
            ),
            IterationPhase::FilterSpecies(selected) => Ok(TypedIterationStep::Read {
                object: target.clone(),
                key: runtime.intern_property_key("set")?,
                resume: Self {
                    realm: self.realm,
                    phase: IterationPhase::FilterMethod { target, selected },
                },
            }),
            _ => Err(RuntimeError::Invariant(
                "TypedArray iteration received an unexpected species reply",
            )),
        }
    }
    pub(crate) fn element(
        self,
        runtime: &Runtime,
        result: NativeConversion<[u8; 8]>,
    ) -> Result<TypedIterationStep, RuntimeError> {
        let bytes = match result {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => {
                return Ok(TypedIterationStep::Complete(Completion::Throw(value)));
            }
        };
        let IterationPhase::Mapped { state, index } = self.phase else {
            return Err(RuntimeError::Invariant(
                "TypedArray iteration received an unexpected element reply",
            ));
        };
        let IterationMode::Map(target) = &state.mode else {
            return Err(RuntimeError::Invariant("TypedArray map lost its result"));
        };
        // Ignore an invalidated target index, exactly like the shared indexed Set.
        let _ = runtime.typed_array_write_converted_index(target, index, &bytes)?;
        Self::next(runtime, self.realm, state)
    }
    pub(crate) fn resume(
        self,
        runtime: &Runtime,
        result: Completion,
    ) -> Result<TypedIterationStep, RuntimeError> {
        let result = match result {
            Completion::Return(value) => value,
            Completion::Throw(value) => {
                return Ok(TypedIterationStep::Complete(Completion::Throw(value)));
            }
        };
        match self.phase {
            IterationPhase::Called {
                mut state,
                value,
                index,
            } => {
                match &mut state.mode {
                    IterationMode::Every if !runtime.value_to_boolean(&result)? => {
                        return Ok(TypedIterationStep::Complete(Completion::Return(
                            Value::Bool(false),
                        )));
                    }
                    IterationMode::Some if runtime.value_to_boolean(&result)? => {
                        return Ok(TypedIterationStep::Complete(Completion::Return(
                            Value::Bool(true),
                        )));
                    }
                    IterationMode::Every | IterationMode::Some | IterationMode::ForEach => {}
                    IterationMode::Map(target) => {
                        return Ok(TypedIterationStep::Element {
                            element: runtime.typed_array_snapshot(target)?.element,
                            value: result,
                            resume: Self {
                                realm: self.realm,
                                phase: IterationPhase::Mapped { state, index },
                            },
                        });
                    }
                    IterationMode::Filter { selected, length } => {
                        if runtime.value_to_boolean(&result)? {
                            let key = runtime.property_key_for_index(*length)?;
                            if !runtime.define_own_property(
                                selected,
                                &key,
                                &OrdinaryPropertyDescriptor {
                                    value: DescriptorField::Present(value),
                                    writable: DescriptorField::Present(true),
                                    enumerable: DescriptorField::Present(true),
                                    configurable: DescriptorField::Present(true),
                                    ..OrdinaryPropertyDescriptor::new()
                                },
                            )? {
                                return Err(RuntimeError::Invariant(
                                    "TypedArray filter temporary Array rejected a dense element",
                                ));
                            }
                            *length = length.checked_add(1).ok_or(RuntimeError::Invariant(
                                "TypedArray filter selected length overflowed u64",
                            ))?;
                        }
                    }
                }
                Self::next(runtime, self.realm, state)
            }
            IterationPhase::FilterMethod { target, selected } => {
                let callable = runtime.callable_from_value(result)?;
                let mut arguments = Vec::new();
                if arguments.try_reserve_exact(1).is_err() {
                    return iteration_oom(runtime, self.realm);
                }
                arguments.push(Value::Object(selected));
                Ok(TypedIterationStep::Call {
                    target: DirectCallTarget::Callable(callable),
                    receiver: Value::Object(target.clone()),
                    arguments,
                    resume: Self {
                        realm: self.realm,
                        phase: IterationPhase::FilterCalled(target),
                    },
                })
            }
            IterationPhase::FilterCalled(target) => Ok(TypedIterationStep::Complete(
                Completion::Return(Value::Object(target)),
            )),
            _ => Err(RuntimeError::Invariant(
                "TypedArray iteration received an untyped reply",
            )),
        }
    }
}
fn iteration_oom(runtime: &Runtime, realm: ContextId) -> Result<TypedIterationStep, RuntimeError> {
    Ok(TypedIterationStep::Complete(Completion::Throw(
        runtime.new_native_error(realm, NativeErrorKind::Internal, "out of memory")?,
    )))
}
impl Runtime {
    pub(crate) fn call_typed_array_iteration(
        &self,
        realm: ContextId,
        kind: ArrayIterationKind,
        invocation: NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        let mut step = TypedIterationStep::start(self, realm, kind, &invocation, arguments)?;
        loop {
            step = match step {
                TypedIterationStep::Complete(result) => return Ok(result),
                TypedIterationStep::Species {
                    source,
                    element,
                    length,
                    resume,
                } => resume.species(
                    self,
                    self.typed_array_species_create(realm, &source, element, length)?,
                )?,
                TypedIterationStep::Call {
                    target,
                    receiver,
                    arguments,
                    resume,
                } => {
                    let DirectCallTarget::Callable(callable) = target else {
                        return Err(RuntimeError::Invariant(
                            "TypedArray iteration requested an invalid call target",
                        ));
                    };
                    resume.resume(
                        self,
                        self.call_internal(realm, &callable, receiver, &arguments)?,
                    )?
                }
                TypedIterationStep::Element {
                    element,
                    value,
                    resume,
                } => resume.element(
                    self,
                    super::element::ElementStep::start(self, realm, element, value)?
                        .finish_sync(self, realm)?,
                )?,
                TypedIterationStep::Read {
                    object,
                    key,
                    resume,
                } => resume.resume(self, self.get_property_in_realm(realm, &object, &key)?)?,
            };
        }
    }
}
