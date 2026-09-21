//! TypedArray iteration owns callbacks, species results, and live element reads.
#[cfg(test)]
use super::*;
use crate::engine::{
    api::{error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError},
    builtins::native::{ArrayIterationKind, TypedArrayElementKind},
    heap::ContextId,
    object::{CallableRef, ObjectRef, PropertyKey},
    value::{JsValue, conversion::NativeConversion},
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
    Species { resume: TypedIterationResume },
    Call { resume: TypedIterationResume },
    Element { resume: TypedIterationResume },
    Read { resume: TypedIterationResume },
}
pub(crate) struct TypedIterationResume(Box<TypedIterationResumeState>);
impl std::ops::Deref for TypedIterationResume {
    type Target = TypedIterationResumeState;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for TypedIterationResume {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
const _: () = assert!(std::mem::size_of::<TypedIterationResume>() <= 8);
pub(crate) struct TypedIterationResumeState {
    pending_effect: TypedIterationStepPending,
    realm: ContextId,
    phase: IterationPhase,
}
struct IterationInput {
    target: ObjectRef,
    callback: CallableRef,
    this_arg: JsValue,
    held_value: JsValue,
    element: TypedArrayElementKind,
    length: u64,
}
impl Drop for IterationInput {
    fn drop(&mut self) {
        let runtime = self.target.runtime();
        let _ = runtime.release_jsvalue(std::mem::replace(&mut self.this_arg, JsValue::Undefined));
        let _ =
            runtime.release_jsvalue(std::mem::replace(&mut self.held_value, JsValue::Undefined));
    }
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
        let target = match runtime.require_typed_array_jsvalue(realm, this_value)? {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => {
                return Ok(Self::Complete(Completion::Throw(value)));
            }
        };
        let length = match runtime.typed_array_validated_length(realm, &target)? {
            NativeConversion::Value(value) => u64::from(value),
            NativeConversion::Throw(value) => {
                return Ok(Self::Complete(Completion::Throw(value)));
            }
        };
        let callback_value = arguments.readable.first().ok_or(RuntimeError::Invariant(
            "TypedArray iteration callback argv was not padded",
        ))?;
        let callback = if let JsValue::Object(id) = callback_value {
            runtime.as_callable_object(*id)?
        } else {
            None
        }
        .ok_or_else(|| {
            RuntimeError::Engine(crate::engine::api::Error::new(
                crate::engine::api::ErrorKind::Type,
                "not a function",
            ))
        })?;
        let element = runtime.typed_array_snapshot(&target)?.element;
        let mut input = IterationInput {
            target,
            callback,
            this_arg: JsValue::Undefined,
            held_value: JsValue::Undefined,
            element,
            length,
        };
        if arguments.actual_arg_count > 1 {
            input.this_arg = runtime.dup_jsvalue(arguments.readable.get(1).ok_or(
                RuntimeError::Invariant("TypedArray iteration thisArg was missing"),
            )?)?;
        }
        let mode = match kind {
            ArrayIterationKind::Every => IterationMode::Every,
            ArrayIterationKind::Some => IterationMode::Some,
            ArrayIterationKind::ForEach => IterationMode::ForEach,
            ArrayIterationKind::Map => {
                return Ok(Self::request_species(
                    input.target.clone(),
                    element,
                    length,
                    TypedIterationResume(Box::new(TypedIterationResumeState {
                        pending_effect: TypedIterationStepPending::new(runtime.clone()),
                        realm,
                        phase: IterationPhase::MapSpecies(input),
                    })),
                ));
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
            let result = match state.mode {
                IterationMode::Every => JsValue::Bool(true),
                IterationMode::Some => JsValue::Bool(false),
                IterationMode::ForEach => JsValue::Undefined,
                IterationMode::Map(target) => JsValue::Object(target.into_handle()),
                IterationMode::Filter { selected, length } => {
                    return Ok(TypedIterationStep::request_species(
                        state.input.target.clone(),
                        state.input.element,
                        length,
                        Self(Box::new(TypedIterationResumeState {
                            pending_effect: TypedIterationStepPending::new(runtime.clone()),
                            realm,
                            phase: IterationPhase::FilterSpecies(selected),
                        })),
                    ));
                }
            };
            return Ok(TypedIterationStep::Complete(Completion::Return(result)));
        }
        let index = state.index;
        state.index += 1;
        state.input.held_value = runtime
            .typed_array_read_index_jsvalue(&state.input.target, index)?
            .unwrap_or(JsValue::Undefined);
        let mut resume = Self(Box::new(TypedIterationResumeState {
            pending_effect: TypedIterationStepPending::new(runtime.clone()),
            realm,
            phase: IterationPhase::Called { state, index },
        }));
        resume.0.pending_effect.call_arguments = Some(Vec::new());
        if resume
            .0
            .pending_effect
            .call_arguments
            .as_mut()
            .unwrap()
            .try_reserve_exact(3)
            .is_err()
        {
            return iteration_oom(runtime, realm);
        }
        let IterationPhase::Called { state, .. } = &resume.0.phase else {
            unreachable!()
        };
        resume.0.pending_effect.call_receiver = Some(runtime.dup_jsvalue(&state.input.this_arg)?);
        let value = runtime.dup_jsvalue(&state.input.held_value)?;
        resume
            .0
            .pending_effect
            .call_arguments
            .as_mut()
            .unwrap()
            .push(value);
        resume
            .0
            .pending_effect
            .call_arguments
            .as_mut()
            .unwrap()
            .push(crate::engine::value::number::operations::Number::compact(index as f64).into());
        resume
            .0
            .pending_effect
            .call_arguments
            .as_mut()
            .unwrap()
            .push(JsValue::Object(state.input.target.clone().into_handle()));
        resume.0.pending_effect.call_target =
            Some(DirectCallTarget::Callable(state.input.callback.clone()));
        Ok(TypedIterationStep::Call { resume })
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
        match self.0.phase {
            IterationPhase::MapSpecies(input) => Self::next(
                runtime,
                self.0.realm,
                IterationState {
                    input,
                    mode: IterationMode::Map(target),
                    index: 0,
                },
            ),
            IterationPhase::FilterSpecies(selected) => Ok(TypedIterationStep::request_read(
                target.clone(),
                runtime.pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Set)?,
                Self(Box::new(TypedIterationResumeState {
                    pending_effect: TypedIterationStepPending::new(runtime.clone()),
                    realm: self.0.realm,
                    phase: IterationPhase::FilterMethod { target, selected },
                })),
            )),
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
        let IterationPhase::Mapped { state, index } = self.0.phase else {
            return Err(RuntimeError::Invariant(
                "TypedArray iteration received an unexpected element reply",
            ));
        };
        let IterationMode::Map(target) = &state.mode else {
            return Err(RuntimeError::Invariant("TypedArray map lost its result"));
        };
        // Ignore an invalidated target index, exactly like the shared indexed Set.
        let _ = runtime.typed_array_write_converted_index(target, index, &bytes)?;
        Self::next(runtime, self.0.realm, state)
    }
    pub(crate) fn resume(
        mut self,
        runtime: &Runtime,
        result: Completion,
    ) -> Result<TypedIterationStep, RuntimeError> {
        let result = match result {
            Completion::Return(value) => value,
            Completion::Throw(value) => {
                return Ok(TypedIterationStep::Complete(Completion::Throw(value)));
            }
        };
        self.0.pending_effect.element_value = Some(result);
        match self.0.phase {
            IterationPhase::Called { mut state, index } => {
                if let IterationMode::Map(target) = &state.mode {
                    let element = runtime.typed_array_snapshot(target)?.element;
                    runtime.release_jsvalue(std::mem::replace(
                        &mut state.input.held_value,
                        JsValue::Undefined,
                    ))?;
                    let result = self.0.pending_effect.element_value.take().unwrap();
                    return Ok(TypedIterationStep::request_element(
                        element,
                        result,
                        Self(Box::new(TypedIterationResumeState {
                            pending_effect: TypedIterationStepPending::new(runtime.clone()),
                            realm: self.0.realm,
                            phase: IterationPhase::Mapped { state, index },
                        })),
                    ));
                }
                let truth = if matches!(state.mode, IterationMode::ForEach) {
                    false
                } else {
                    runtime.value_to_boolean_jsvalue(
                        self.0.pending_effect.element_value.as_ref().unwrap(),
                    )?
                };
                runtime.release_jsvalue(self.0.pending_effect.element_value.take().unwrap())?;
                match &mut state.mode {
                    IterationMode::Every if !truth => {
                        return Ok(TypedIterationStep::Complete(Completion::Return(
                            JsValue::Bool(false),
                        )));
                    }
                    IterationMode::Some if truth => {
                        return Ok(TypedIterationStep::Complete(Completion::Return(
                            JsValue::Bool(true),
                        )));
                    }
                    IterationMode::Filter { selected, length } if truth => {
                        let key = runtime.property_key_for_index(*length)?;
                        match runtime.define_selected_set_data(
                            selected,
                            &key,
                            &state.input.held_value,
                            false,
                        )? {
                            crate::engine::object::operations::PropertyDefineOutcome::Defined(
                                true,
                            ) => {}
                            crate::engine::object::operations::PropertyDefineOutcome::Defined(
                                false,
                            ) => {
                                return Err(RuntimeError::Invariant(
                                    "TypedArray filter temporary Array rejected a dense element",
                                ));
                            }
                            crate::engine::object::operations::PropertyDefineOutcome::Throw(
                                value,
                            ) => {
                                return Ok(TypedIterationStep::Complete(Completion::Throw(value)));
                            }
                        }
                        *length = length.checked_add(1).ok_or(RuntimeError::Invariant(
                            "TypedArray filter selected length overflowed u64",
                        ))?;
                    }
                    _ => {}
                }
                runtime.release_jsvalue(std::mem::replace(
                    &mut state.input.held_value,
                    JsValue::Undefined,
                ))?;
                Self::next(runtime, self.0.realm, state)
            }
            IterationPhase::FilterMethod { target, selected } => {
                let callable = match self.0.pending_effect.element_value.as_ref().unwrap() {
                    JsValue::Object(id) => runtime.as_callable_object(*id)?,
                    _ => None,
                }
                .ok_or_else(|| {
                    RuntimeError::Engine(crate::engine::api::Error::new(
                        crate::engine::api::ErrorKind::Type,
                        "not a function",
                    ))
                })?;
                runtime.release_jsvalue(self.0.pending_effect.element_value.take().unwrap())?;
                let mut arguments = Vec::new();
                if arguments.try_reserve_exact(1).is_err() {
                    return iteration_oom(runtime, self.0.realm);
                }
                arguments.push(JsValue::Object(selected.into_handle()));
                Ok(TypedIterationStep::request_call(
                    DirectCallTarget::Callable(callable),
                    JsValue::Object(target.clone().into_handle()),
                    arguments,
                    Self(Box::new(TypedIterationResumeState {
                        pending_effect: TypedIterationStepPending::new(runtime.clone()),
                        realm: self.0.realm,
                        phase: IterationPhase::FilterCalled(target),
                    })),
                ))
            }
            IterationPhase::FilterCalled(target) => Ok(TypedIterationStep::Complete(
                Completion::Return(JsValue::Object(target.into_handle())),
            )),
            _ => Err(RuntimeError::Invariant(
                "TypedArray iteration received an untyped reply",
            )),
        }
    }
}
fn iteration_oom(runtime: &Runtime, realm: ContextId) -> Result<TypedIterationStep, RuntimeError> {
    Ok(TypedIterationStep::Complete(Completion::Throw(
        runtime.new_native_error_jsvalue(realm, NativeErrorKind::Internal, "out of memory")?,
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
                TypedIterationStep::Species { mut resume } => {
                    let source = resume.take_species_source();
                    let element = resume.take_species_element();
                    let length = resume.take_species_length();
                    resume.species(
                        self,
                        self.typed_array_species_create(realm, &source, element, length)?,
                    )?
                }
                TypedIterationStep::Call { mut resume } => {
                    let target = resume.take_call_target();
                    let DirectCallTarget::Callable(callable) = target else {
                        return Err(RuntimeError::Invariant(
                            "TypedArray iteration requested an invalid call target",
                        ));
                    };
                    let receiver = resume.take_call_receiver();
                    let arguments = resume.take_call_arguments();
                    resume.resume(
                        self,
                        self.call_internal_jsvalue(realm, &callable, receiver, arguments)?,
                    )?
                }
                TypedIterationStep::Element { mut resume } => {
                    let element = resume.take_element_element();
                    let value = resume.take_element_value();
                    resume.element(
                        self,
                        super::element::ElementStep::start(self, realm, element, value)?
                            .finish_sync(self, realm)?,
                    )?
                }
                TypedIterationStep::Read { mut resume } => {
                    let object = resume.take_read_object();
                    let key = resume.take_read_key();
                    resume.resume(
                        self,
                        self.internal_get_jsvalue(
                            realm,
                            &object,
                            &key,
                            JsValue::Object(object.clone().into_handle()),
                        )?,
                    )?
                }
            };
        }
    }
}

struct TypedIterationStepPending {
    runtime: Runtime,
    species_source: Option<ObjectRef>,
    species_element: Option<TypedArrayElementKind>,
    species_length: Option<u64>,
    call_target: Option<DirectCallTarget>,
    call_receiver: Option<JsValue>,
    call_arguments: Option<Vec<JsValue>>,
    element_element: Option<TypedArrayElementKind>,
    element_value: Option<JsValue>,
    read_object: Option<ObjectRef>,
    read_key: Option<PropertyKey>,
}
impl TypedIterationStepPending {
    fn new(runtime: Runtime) -> Self {
        Self {
            runtime,
            species_source: None,
            species_element: None,
            species_length: None,
            call_target: None,
            call_receiver: None,
            call_arguments: None,
            element_element: None,
            element_value: None,
            read_object: None,
            read_key: None,
        }
    }
}
impl Drop for TypedIterationStepPending {
    /// Release the internal edges still held when the request is abandoned.
    /// Consumption goes through `Option::take`; releases are defer-safe and
    /// nothrow.
    fn drop(&mut self) {
        for value in [self.call_receiver.take(), self.element_value.take()]
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
impl TypedIterationStep {
    pub(crate) fn request_species(
        source: ObjectRef,
        element: TypedArrayElementKind,
        length: u64,
        mut resume: TypedIterationResume,
    ) -> Self {
        resume.0.pending_effect.species_source = Some(source);
        resume.0.pending_effect.species_element = Some(element);
        resume.0.pending_effect.species_length = Some(length);
        Self::Species { resume }
    }
    pub(crate) fn request_call(
        target: DirectCallTarget,
        receiver: JsValue,
        arguments: Vec<JsValue>,
        mut resume: TypedIterationResume,
    ) -> Self {
        resume.0.pending_effect.call_target = Some(target);
        resume.0.pending_effect.call_receiver = Some(receiver);
        resume.0.pending_effect.call_arguments = Some(arguments);
        Self::Call { resume }
    }
    pub(crate) fn request_element(
        element: TypedArrayElementKind,
        value: JsValue,
        mut resume: TypedIterationResume,
    ) -> Self {
        resume.0.pending_effect.element_element = Some(element);
        resume.0.pending_effect.element_value = Some(value);
        Self::Element { resume }
    }
    pub(crate) fn request_read(
        object: ObjectRef,
        key: PropertyKey,
        mut resume: TypedIterationResume,
    ) -> Self {
        resume.0.pending_effect.read_object = Some(object);
        resume.0.pending_effect.read_key = Some(key);
        Self::Read { resume }
    }
}
impl TypedIterationResume {
    pub(crate) fn take_species_source(&mut self) -> ObjectRef {
        self.0
            .pending_effect
            .species_source
            .take()
            .expect("TypedIterationStep Species source")
    }
    pub(crate) fn take_species_element(&mut self) -> TypedArrayElementKind {
        self.0
            .pending_effect
            .species_element
            .take()
            .expect("TypedIterationStep Species element")
    }
    pub(crate) fn take_species_length(&mut self) -> u64 {
        self.0
            .pending_effect
            .species_length
            .take()
            .expect("TypedIterationStep Species length")
    }
    pub(crate) fn take_call_target(&mut self) -> DirectCallTarget {
        self.0
            .pending_effect
            .call_target
            .take()
            .expect("TypedIterationStep Call target")
    }
    pub(crate) fn take_call_receiver(&mut self) -> JsValue {
        self.0
            .pending_effect
            .call_receiver
            .take()
            .expect("TypedIterationStep Call receiver")
    }
    pub(crate) fn take_call_arguments(&mut self) -> Vec<JsValue> {
        self.0
            .pending_effect
            .call_arguments
            .take()
            .expect("TypedIterationStep Call arguments")
    }
    pub(crate) fn take_element_element(&mut self) -> TypedArrayElementKind {
        self.0
            .pending_effect
            .element_element
            .take()
            .expect("TypedIterationStep Element element")
    }
    pub(crate) fn take_element_value(&mut self) -> JsValue {
        self.0
            .pending_effect
            .element_value
            .take()
            .expect("TypedIterationStep Element value")
    }
    pub(crate) fn take_read_object(&mut self) -> ObjectRef {
        self.0
            .pending_effect
            .read_object
            .take()
            .expect("TypedIterationStep Read object")
    }
    pub(crate) fn take_read_key(&mut self) -> PropertyKey {
        self.0
            .pending_effect
            .read_key
            .take()
            .expect("TypedIterationStep Read key")
    }
}
const _: () = assert!(std::mem::size_of::<TypedIterationStep>() <= 64);

// S11 all-domain protocol bound; inline completion stays allocation-free.
const _: () = assert!(std::mem::size_of::<TypedIterationStep>() <= 64);
