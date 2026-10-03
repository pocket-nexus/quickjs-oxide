//! Mechanical adapters for buffer domain requests.
use super::JsValue;
use super::{DirectCallTarget, ElementStep, Resume, Step, TypedWriteStep};

impl TryFrom<ElementStep> for Step {
    type Error = crate::engine::api::RuntimeError;
    fn try_from(step: ElementStep) -> Result<Self, Self::Error> {
        Ok({
            match step {
                ElementStep::Complete(result) => Self::ElementComplete(Some(result)),
                ElementStep::Read { mut resume } => {
                    let object = resume.take_read_object();
                    let key = resume.take_read_key();
                    Self::Read {
                        receiver: Some(JsValue::Object(object.try_clone()?.into_handle())),
                        object: Some(object),
                        key: Some(key),
                        resume: Some(Resume::Element(resume)),
                    }
                }
                ElementStep::Call { mut resume } => {
                    let callable = resume.take_call_callable();
                    let receiver = resume.take_call_receiver();
                    let arguments = resume.take_call_arguments();
                    Self::Call {
                        target: Some(DirectCallTarget::Callable(callable)),
                        receiver: Some(receiver),
                        arguments: Some(arguments),
                        resume: Some(Resume::Element(resume)),
                    }
                }
            }
        })
    }
}

impl TryFrom<TypedWriteStep> for Step {
    type Error = crate::engine::api::RuntimeError;
    fn try_from(step: TypedWriteStep) -> Result<Self, Self::Error> {
        Ok({
            match step {
                TypedWriteStep::Complete(result) => Self::TypedComplete(Some(result)),
                TypedWriteStep::Element {
                    element,
                    value,
                    resume,
                } => Self::Element {
                    element: Some(element),
                    value: Some(value),
                    resume: Some(Resume::TypedElement(resume)),
                },
            }
        })
    }
}

impl TryFrom<crate::engine::builtins::DataViewAccessStep> for Step {
    type Error = crate::engine::api::RuntimeError;
    fn try_from(step: crate::engine::builtins::DataViewAccessStep) -> Result<Self, Self::Error> {
        Ok({
            use crate::engine::builtins::DataViewAccessStep as T;
            match step {
                T::Complete(result) => Self::Complete(Some(result)),
                T::Primitive { value, resume } => Self::Primitive {
                    value: Some(value),
                    hint: Some(crate::engine::vm::ToPrimitiveHint::Number),
                    resume: Some(Resume::DataView(resume)),
                },
            }
        })
    }
}

impl TryFrom<crate::engine::builtins::BufferMutationStep> for Step {
    type Error = crate::engine::api::RuntimeError;
    fn try_from(step: crate::engine::builtins::BufferMutationStep) -> Result<Self, Self::Error> {
        Ok({
            use crate::engine::builtins::BufferMutationStep as T;
            match step {
                T::Complete(result) => Self::Complete(Some(result)),
                T::Primitive { value, resume } => Self::Primitive {
                    value: Some(value),
                    hint: Some(crate::engine::vm::ToPrimitiveHint::Number),
                    resume: Some(Resume::BufferMutation(resume)),
                },
            }
        })
    }
}

impl TryFrom<crate::engine::builtins::TypedTraversalStep> for Step {
    type Error = crate::engine::api::RuntimeError;
    fn try_from(step: crate::engine::builtins::TypedTraversalStep) -> Result<Self, Self::Error> {
        Ok({
            use crate::engine::builtins::TypedTraversalStep as T;
            match step {
                T::Complete(result) => Self::Complete(Some(result)),
                T::Call { mut resume } => {
                    let target = resume.take_call_target();
                    let receiver = resume.take_call_receiver();
                    let arguments = resume.take_call_arguments();
                    Self::Call {
                        target: Some(target),
                        receiver: Some(receiver),
                        arguments: Some(arguments),
                        resume: Some(Resume::TypedTraversal(resume)),
                    }
                }
            }
        })
    }
}

impl TryFrom<crate::engine::builtins::TypedSpeciesStep> for Step {
    type Error = crate::engine::api::RuntimeError;
    fn try_from(step: crate::engine::builtins::TypedSpeciesStep) -> Result<Self, Self::Error> {
        Ok({
            use crate::engine::builtins::TypedSpeciesStep as T;
            match step {
                T::Complete(result) => Self::TypedSpeciesComplete(Some(result)),
                T::Read {
                    object,
                    key,
                    resume,
                } => Self::Read {
                    receiver: Some(JsValue::Object(object.try_clone()?.into_handle())),
                    object: Some(object),
                    key: Some(key),
                    resume: Some(Resume::TypedSpecies(resume)),
                },
                T::Construct {
                    constructor,
                    mut resume,
                } => Self::Construct {
                    new_target: Some(crate::engine::vm::call::ConstructNewTarget::Validated(
                        constructor.try_clone()?,
                    )),
                    target: Some(constructor),
                    arguments: Some(resume.take_arguments()),
                    resume: Some(Resume::TypedSpecies(resume)),
                },
            }
        })
    }
}

impl TryFrom<crate::engine::builtins::TypedIterationStep> for Step {
    type Error = crate::engine::api::RuntimeError;
    fn try_from(step: crate::engine::builtins::TypedIterationStep) -> Result<Self, Self::Error> {
        Ok({
            use crate::engine::builtins::TypedIterationStep as T;
            match step {
                T::Complete(result) => Self::Complete(Some(result)),
                T::Read { mut resume } => {
                    let object = resume.take_read_object();
                    let key = resume.take_read_key();
                    Self::Read {
                        receiver: Some(JsValue::Object(object.try_clone()?.into_handle())),
                        object: Some(object),
                        key: Some(key),
                        resume: Some(Resume::TypedIteration(resume)),
                    }
                }
                T::Call { mut resume } => {
                    let target = resume.take_call_target();
                    let receiver = resume.take_call_receiver();
                    let arguments = resume.take_call_arguments();
                    Self::Call {
                        target: Some(target),
                        receiver: Some(receiver),
                        arguments: Some(arguments),
                        resume: Some(Resume::TypedIteration(resume)),
                    }
                }
                T::Species { mut resume } => {
                    let source = resume.take_species_source();
                    let element = resume.take_species_element();
                    let length = resume.take_species_length();
                    Self::TypedSpecies {
                        source: Some(source),
                        element: Some(element),
                        length: Some(length),
                        resume: Some(Resume::TypedIteration(resume)),
                    }
                }
                T::Element { mut resume } => {
                    let element = resume.take_element_element();
                    let value = resume.take_element_value();
                    Self::Element {
                        element: Some(element),
                        value: Some(value),
                        resume: Some(Resume::TypedIteration(resume)),
                    }
                }
            }
        })
    }
}

impl TryFrom<crate::engine::builtins::TypedSortStep> for Step {
    type Error = crate::engine::api::RuntimeError;
    fn try_from(step: crate::engine::builtins::TypedSortStep) -> Result<Self, Self::Error> {
        Ok({
            use crate::engine::builtins::TypedSortStep as T;
            match step {
                T::Complete(result) => Self::Complete(Some(result)),
                T::Call {
                    callable,
                    arguments,
                    resume,
                } => Self::Call {
                    target: Some(DirectCallTarget::Callable(callable)),
                    receiver: Some(JsValue::Undefined),
                    arguments: Some(arguments),
                    resume: Some(Resume::TypedSort(resume)),
                },
                T::Number { value, resume } => Self::Number {
                    value: Some(value),
                    resume: Some(Resume::TypedSort(resume)),
                },
            }
        })
    }
}

impl TryFrom<crate::engine::builtins::BufferConstructorStep> for Step {
    type Error = crate::engine::api::RuntimeError;
    fn try_from(step: crate::engine::builtins::BufferConstructorStep) -> Result<Self, Self::Error> {
        Ok({
            use crate::engine::builtins::BufferConstructorStep as T;
            match step {
                T::Complete(result) => Self::Complete(Some(result)),
                T::Read {
                    object,
                    key,
                    resume,
                } => Self::Read {
                    receiver: Some(JsValue::Object(object.try_clone()?.into_handle())),
                    object: Some(object),
                    key: Some(key),
                    resume: Some(Resume::BufferConstructor(resume)),
                },
                T::Primitive { value, resume } => Self::Primitive {
                    value: Some(value),
                    hint: Some(crate::engine::vm::ToPrimitiveHint::Number),
                    resume: Some(Resume::BufferConstructor(resume)),
                },
                T::Prototype { new_target, resume } => Self::ConstructorSource {
                    new_target: Some(new_target),
                    resume: Some(Resume::BufferConstructor(resume)),
                },
            }
        })
    }
}

impl TryFrom<crate::engine::builtins::DataViewConstructorStep> for Step {
    type Error = crate::engine::api::RuntimeError;
    fn try_from(
        step: crate::engine::builtins::DataViewConstructorStep,
    ) -> Result<Self, Self::Error> {
        Ok({
            use crate::engine::builtins::DataViewConstructorStep as T;
            match step {
                T::Complete(result) => Self::Complete(Some(result)),
                T::Primitive { value, resume } => Self::Primitive {
                    value: Some(value),
                    hint: Some(crate::engine::vm::ToPrimitiveHint::Number),
                    resume: Some(Resume::DataViewConstructor(resume)),
                },
                T::Prototype { new_target, resume } => Self::ConstructorSource {
                    new_target: Some(new_target),
                    resume: Some(Resume::DataViewConstructor(resume)),
                },
            }
        })
    }
}

impl TryFrom<crate::engine::builtins::TypedSetStep> for Step {
    type Error = crate::engine::api::RuntimeError;
    fn try_from(step: crate::engine::builtins::TypedSetStep) -> Result<Self, Self::Error> {
        Ok({
            use crate::engine::builtins::TypedSetStep as T;
            match step {
                T::Complete(result) => Self::Complete(Some(result)),
                T::Read {
                    object,
                    key,
                    resume,
                } => Self::Read {
                    receiver: Some(JsValue::Object(object.try_clone()?.into_handle())),
                    object: Some(object),
                    key: Some(key),
                    resume: Some(Resume::TypedSet(resume)),
                },
                T::Primitive { value, resume } => Self::Primitive {
                    value: Some(value),
                    hint: Some(crate::engine::vm::ToPrimitiveHint::Number),
                    resume: Some(Resume::TypedSet(resume)),
                },
                T::Element {
                    element,
                    value,
                    resume,
                } => Self::Element {
                    element: Some(element),
                    value: Some(value),
                    resume: Some(Resume::TypedSet(resume)),
                },
            }
        })
    }
}

impl TryFrom<crate::engine::builtins::TypedSearchStep> for Step {
    type Error = crate::engine::api::RuntimeError;
    fn try_from(step: crate::engine::builtins::TypedSearchStep) -> Result<Self, Self::Error> {
        Ok({
            use crate::engine::builtins::TypedSearchStep as T;
            match step {
                T::Complete(result) => Self::Complete(Some(result)),
                T::Primitive { value, resume } => Self::Primitive {
                    value: Some(value),
                    hint: Some(crate::engine::vm::ToPrimitiveHint::Number),
                    resume: Some(Resume::TypedSearch(resume)),
                },
            }
        })
    }
}

impl TryFrom<crate::engine::builtins::TypedStringStep> for Step {
    type Error = crate::engine::api::RuntimeError;
    fn try_from(step: crate::engine::builtins::TypedStringStep) -> Result<Self, Self::Error> {
        Ok({
            use crate::engine::builtins::TypedStringStep as T;
            match step {
                T::Complete(result) => Self::Complete(Some(result)),
                T::Primitive { mut resume } => {
                    let value = resume.take_primitive_value();
                    Self::Primitive {
                        value: Some(value),
                        hint: Some(crate::engine::vm::ToPrimitiveHint::String),
                        resume: Some(Resume::TypedString(resume)),
                    }
                }
                T::Read { mut resume } => {
                    let receiver = resume.take_read_receiver();
                    let key = resume.take_read_key();
                    Self::ReadValue {
                        receiver: Some(receiver),
                        key: Some(key),
                        resume: Some(Resume::TypedString(resume)),
                    }
                }
                T::Call { mut resume } => {
                    let target = resume.take_call_target();
                    let receiver = resume.take_call_receiver();
                    let arguments = resume.take_call_arguments();
                    Self::Call {
                        target: Some(target),
                        receiver: Some(receiver),
                        arguments: Some(arguments),
                        resume: Some(Resume::TypedString(resume)),
                    }
                }
            }
        })
    }
}

impl TryFrom<crate::engine::builtins::TypedSliceStep> for Step {
    type Error = crate::engine::api::RuntimeError;
    fn try_from(step: crate::engine::builtins::TypedSliceStep) -> Result<Self, Self::Error> {
        Ok({
            use crate::engine::builtins::TypedSliceStep as T;
            match step {
                T::Complete(result) => Self::Complete(Some(result)),
                T::Primitive { mut resume } => {
                    let value = resume.take_primitive_value();
                    Self::Primitive {
                        value: Some(value),
                        hint: Some(crate::engine::vm::ToPrimitiveHint::Number),
                        resume: Some(Resume::TypedSlice(resume)),
                    }
                }
                T::Species { mut resume } => {
                    let source = resume.take_species_source();
                    let element = resume.take_species_element();
                    let length = resume.take_species_length();
                    Self::TypedSpecies {
                        source: Some(source),
                        element: Some(element),
                        length: Some(length),
                        resume: Some(Resume::TypedSlice(resume)),
                    }
                }
                T::SpeciesView { mut resume } => {
                    let source = resume.take_species_view_source();
                    let element = resume.take_species_view_element();
                    let buffer = resume.take_species_view_buffer();
                    let offset = resume.take_species_view_offset();
                    let length = resume.take_species_view_length();
                    Self::TypedSpeciesView {
                        source: Some(source),
                        element: Some(element),
                        buffer: Some(buffer),
                        offset: Some(offset),
                        length: Some(length),
                        resume: Some(Resume::TypedSlice(resume)),
                    }
                }
            }
        })
    }
}

impl TryFrom<crate::engine::builtins::TypedMutationStep> for Step {
    type Error = crate::engine::api::RuntimeError;
    fn try_from(step: crate::engine::builtins::TypedMutationStep) -> Result<Self, Self::Error> {
        Ok({
            use crate::engine::builtins::TypedMutationStep as T;
            match step {
                T::Complete(result) => Self::Complete(Some(result)),
                T::Primitive { value, resume } => Self::Primitive {
                    value: Some(value),
                    hint: Some(crate::engine::vm::ToPrimitiveHint::Number),
                    resume: Some(Resume::TypedMutation(resume)),
                },
                T::Element {
                    element,
                    value,
                    resume,
                } => Self::Element {
                    element: Some(element),
                    value: Some(value),
                    resume: Some(Resume::TypedMutation(resume)),
                },
            }
        })
    }
}

impl TryFrom<crate::engine::builtins::BufferSliceStep> for Step {
    type Error = crate::engine::api::RuntimeError;
    fn try_from(step: crate::engine::builtins::BufferSliceStep) -> Result<Self, Self::Error> {
        Ok({
            use crate::engine::builtins::BufferSliceStep as T;
            match step {
                T::Complete(result) => Self::Complete(Some(result)),
                T::Read {
                    object,
                    key,
                    resume,
                } => Self::Read {
                    receiver: Some(JsValue::Object(object.try_clone()?.into_handle())),
                    object: Some(object),
                    key: Some(key),
                    resume: Some(Resume::BufferSlice(resume)),
                },
                T::Primitive { value, resume } => Self::Primitive {
                    value: Some(value),
                    hint: Some(crate::engine::vm::ToPrimitiveHint::Number),
                    resume: Some(Resume::BufferSlice(resume)),
                },
                T::Construct {
                    constructor,
                    arguments,
                    resume,
                } => Self::Construct {
                    new_target: Some(crate::engine::vm::call::ConstructNewTarget::Validated(
                        constructor.try_clone()?,
                    )),
                    target: Some(constructor),
                    arguments: Some(arguments),
                    resume: Some(Resume::BufferSlice(resume)),
                },
            }
        })
    }
}

impl TryFrom<crate::engine::builtins::TypedWithStep> for Step {
    type Error = crate::engine::api::RuntimeError;
    fn try_from(step: crate::engine::builtins::TypedWithStep) -> Result<Self, Self::Error> {
        Ok({
            use crate::engine::builtins::TypedWithStep as T;
            match step {
                T::Complete(result) => Self::Complete(Some(result)),
                T::Primitive { value, resume } => Self::Primitive {
                    value: Some(value),
                    hint: Some(crate::engine::vm::ToPrimitiveHint::Number),
                    resume: Some(Resume::TypedWith(resume)),
                },
            }
        })
    }
}

impl TryFrom<crate::engine::builtins::Uint8CodecStep> for Step {
    type Error = crate::engine::api::RuntimeError;
    fn try_from(step: crate::engine::builtins::Uint8CodecStep) -> Result<Self, Self::Error> {
        Ok({
            use crate::engine::builtins::Uint8CodecStep as T;
            match step {
                T::Complete(result) => Self::Complete(Some(result)),
                T::Read {
                    object,
                    key,
                    resume,
                } => Self::Read {
                    receiver: Some(JsValue::Object(object.try_clone()?.into_handle())),
                    object: Some(object),
                    key: Some(key),
                    resume: Some(Resume::Uint8Codec(resume)),
                },
            }
        })
    }
}

impl TryFrom<crate::engine::builtins::TypedIteratorMethodStep> for Step {
    type Error = crate::engine::api::RuntimeError;
    fn try_from(
        step: crate::engine::builtins::TypedIteratorMethodStep,
    ) -> Result<Self, Self::Error> {
        Ok({
            use crate::engine::builtins::TypedIteratorMethodStep as T;
            match step {
                T::Complete(result) => Self::TypedIteratorMethodComplete(Some(result)),
                T::Read { key, mut resume } => Self::ReadValue {
                    receiver: Some(resume.take_receiver()),
                    key: Some(key),
                    resume: Some(Resume::TypedIteratorMethod(resume)),
                },
            }
        })
    }
}

impl TryFrom<crate::engine::builtins::TypedCollectStep> for Step {
    type Error = crate::engine::api::RuntimeError;
    fn try_from(step: crate::engine::builtins::TypedCollectStep) -> Result<Self, Self::Error> {
        Ok({
            use crate::engine::builtins::TypedCollectStep as T;
            match step {
                T::Complete(result) => Self::TypedCollectComplete(Some(result)),
                T::Read {
                    object,
                    key,
                    resume,
                } => Self::Read {
                    receiver: Some(JsValue::Object(object.try_clone()?.into_handle())),
                    object: Some(object),
                    key: Some(key),
                    resume: Some(Resume::TypedCollect(resume)),
                },
                T::Call {
                    callable,
                    mut resume,
                } => Self::Call {
                    target: Some(DirectCallTarget::Callable(callable)),
                    receiver: Some(resume.take_receiver()),
                    arguments: Some(Vec::new()),
                    resume: Some(Resume::TypedCollect(resume)),
                },
            }
        })
    }
}

impl TryFrom<crate::engine::builtins::TypedCreateStep> for Step {
    type Error = crate::engine::api::RuntimeError;
    fn try_from(step: crate::engine::builtins::TypedCreateStep) -> Result<Self, Self::Error> {
        Ok({
            use crate::engine::builtins::TypedCreateStep as T;
            match step {
                T::Complete(result) => Self::Complete(Some(result)),
                T::Read { mut resume } => {
                    let receiver = resume.take_read_receiver();
                    let key = resume.take_read_key();
                    Self::ReadValue {
                        receiver: Some(receiver),
                        key: Some(key),
                        resume: Some(Resume::TypedCreate(resume)),
                    }
                }
                T::Primitive { mut resume } => {
                    let value = resume.take_primitive_value();
                    Self::Primitive {
                        value: Some(value),
                        hint: Some(crate::engine::vm::ToPrimitiveHint::Number),
                        resume: Some(Resume::TypedCreate(resume)),
                    }
                }
                T::Call { mut resume } => {
                    let target = resume.take_call_target();
                    let receiver = resume.take_call_receiver();
                    let arguments = resume.take_call_arguments();
                    Self::Call {
                        target: Some(target),
                        receiver: Some(receiver),
                        arguments: Some(arguments),
                        resume: Some(Resume::TypedCreate(resume)),
                    }
                }
                T::Prototype { mut resume } => {
                    let new_target = resume.take_prototype_new_target();
                    Self::ConstructorSource {
                        new_target: Some(new_target),
                        resume: Some(Resume::TypedCreate(resume)),
                    }
                }
                T::Method { mut resume } => {
                    let source = resume.take_method_source();
                    Self::TypedIteratorMethod {
                        source: Some(source),
                        resume: Some(Resume::TypedCreate(resume)),
                    }
                }
                T::Collect { mut resume } => {
                    let source = resume.take_collect_source();
                    let method = resume.take_collect_method();
                    let element = resume.take_collect_element();
                    Self::TypedCollect {
                        source: Some(source),
                        method: Some(method),
                        element: Some(element),
                        resume: Some(Resume::TypedCreate(resume)),
                    }
                }
                T::Create { mut resume } => {
                    let constructor = resume.take_create_constructor();
                    let length = resume.take_create_length();
                    Self::TypedCreate {
                        constructor: Some(constructor),
                        length: Some(length),
                        resume: Some(Resume::TypedCreate(resume)),
                    }
                }
                T::Element { mut resume } => {
                    let element = resume.take_element_element();
                    let value = resume.take_element_value();
                    Self::Element {
                        element: Some(element),
                        value: Some(value),
                        resume: Some(Resume::TypedCreate(resume)),
                    }
                }
            }
        })
    }
}

impl TryFrom<crate::engine::builtins::AtomicsStep> for Step {
    type Error = crate::engine::api::RuntimeError;
    fn try_from(step: crate::engine::builtins::AtomicsStep) -> Result<Self, Self::Error> {
        Ok({
            use crate::engine::builtins::AtomicsStep as T;
            match step {
                T::Complete(result) => Self::Complete(Some(result)),
                T::Primitive { value, resume } => Self::Primitive {
                    value: Some(value),
                    hint: Some(crate::engine::vm::ToPrimitiveHint::Number),
                    resume: Some(Resume::Atomics(resume)),
                },
                T::Number { value, resume } => Self::Number {
                    value: Some(value),
                    resume: Some(Resume::Atomics(resume)),
                },
            }
        })
    }
}
