//! Mechanical adapters for buffer domain requests.
use super::{DirectCallTarget, ElementStep, Resume, Step, TypedWriteStep, Value};

impl From<ElementStep> for Step {
    fn from(step: ElementStep) -> Self {
        match step {
            ElementStep::Complete(result) => Self::ElementComplete(result),
            ElementStep::Read {
                object,
                key,
                resume,
            } => Self::Read {
                receiver: Value::Object(object.clone()),
                object,
                key,
                resume: Resume::Element(resume),
            },
            ElementStep::Call {
                callable,
                receiver,
                arguments,
                resume,
            } => Self::Call {
                target: DirectCallTarget::Callable(callable),
                receiver,
                arguments,
                resume: Resume::Element(resume),
            },
        }
    }
}

impl From<TypedWriteStep> for Step {
    fn from(step: TypedWriteStep) -> Self {
        match step {
            TypedWriteStep::Complete(result) => Self::TypedComplete(result),
            TypedWriteStep::Element {
                element,
                value,
                resume,
            } => Self::Element {
                element,
                value,
                resume: Resume::TypedElement(resume),
            },
        }
    }
}

impl From<crate::engine::builtins::DataViewAccessStep> for Step {
    fn from(step: crate::engine::builtins::DataViewAccessStep) -> Self {
        use crate::engine::builtins::DataViewAccessStep as T;
        match step {
            T::Complete(result) => Self::Complete(result),
            T::Primitive { value, resume } => Self::Primitive {
                value,
                hint: crate::engine::vm::ToPrimitiveHint::Number,
                resume: Resume::DataView(resume),
            },
        }
    }
}

impl From<crate::engine::builtins::BufferMutationStep> for Step {
    fn from(step: crate::engine::builtins::BufferMutationStep) -> Self {
        use crate::engine::builtins::BufferMutationStep as T;
        match step {
            T::Complete(result) => Self::Complete(result),
            T::Primitive { value, resume } => Self::Primitive {
                value,
                hint: crate::engine::vm::ToPrimitiveHint::Number,
                resume: Resume::BufferMutation(resume),
            },
        }
    }
}

impl From<crate::engine::builtins::TypedTraversalStep> for Step {
    fn from(step: crate::engine::builtins::TypedTraversalStep) -> Self {
        use crate::engine::builtins::TypedTraversalStep as T;
        match step {
            T::Complete(result) => Self::Complete(result),
            T::Call {
                target,
                receiver,
                arguments,
                resume,
            } => Self::Call {
                target: target,
                receiver: receiver,
                arguments: arguments,
                resume: Resume::TypedTraversal(resume),
            },
        }
    }
}

impl From<crate::engine::builtins::TypedSpeciesStep> for Step {
    fn from(step: crate::engine::builtins::TypedSpeciesStep) -> Self {
        use crate::engine::builtins::TypedSpeciesStep as T;
        match step {
            T::Complete(result) => Self::TypedSpeciesComplete(result),
            T::Read {
                object,
                key,
                resume,
            } => Self::Read {
                receiver: Value::Object(object.clone()),
                object,
                key,
                resume: Resume::TypedSpecies(resume),
            },
            T::Construct {
                constructor,
                arguments,
                resume,
            } => Self::Construct {
                new_target: crate::engine::vm::call::ConstructNewTarget::Validated(
                    constructor.clone(),
                ),
                target: constructor,
                arguments,
                resume: Resume::TypedSpecies(resume),
            },
        }
    }
}

impl From<crate::engine::builtins::TypedIterationStep> for Step {
    fn from(step: crate::engine::builtins::TypedIterationStep) -> Self {
        use crate::engine::builtins::TypedIterationStep as T;
        match step {
            T::Complete(result) => Self::Complete(result),
            T::Read {
                object,
                key,
                resume,
            } => Self::Read {
                receiver: Value::Object(object.clone()),
                object,
                key,
                resume: Resume::TypedIteration(resume),
            },
            T::Call {
                target,
                receiver,
                arguments,
                resume,
            } => Self::Call {
                target: target,
                receiver: receiver,
                arguments: arguments,
                resume: Resume::TypedIteration(resume),
            },
            T::Species {
                source,
                element,
                length,
                resume,
            } => Self::TypedSpecies {
                source,
                element,
                length,
                resume: Resume::TypedIteration(resume),
            },
            T::Element {
                element,
                value,
                resume,
            } => Self::Element {
                element,
                value,
                resume: Resume::TypedIteration(resume),
            },
        }
    }
}

impl From<crate::engine::builtins::TypedSortStep> for Step {
    fn from(step: crate::engine::builtins::TypedSortStep) -> Self {
        use crate::engine::builtins::TypedSortStep as T;
        match step {
            T::Complete(result) => Self::Complete(result),
            T::Call {
                callable,
                arguments,
                resume,
            } => Self::Call {
                target: DirectCallTarget::Callable(callable),
                receiver: Value::Undefined,
                arguments,
                resume: Resume::TypedSort(resume),
            },
            T::Number { value, resume } => Self::Number {
                value,
                resume: Resume::TypedSort(resume),
            },
        }
    }
}

impl From<crate::engine::builtins::BufferConstructorStep> for Step {
    fn from(step: crate::engine::builtins::BufferConstructorStep) -> Self {
        use crate::engine::builtins::BufferConstructorStep as T;
        match step {
            T::Complete(result) => Self::Complete(result),
            T::Read {
                object,
                key,
                resume,
            } => Self::Read {
                receiver: Value::Object(object.clone()),
                object,
                key,
                resume: Resume::BufferConstructor(resume),
            },
            T::Primitive { value, resume } => Self::Primitive {
                value,
                hint: crate::engine::vm::ToPrimitiveHint::Number,
                resume: Resume::BufferConstructor(resume),
            },
            T::Prototype { new_target, resume } => Self::ConstructorSource {
                new_target,
                resume: Resume::BufferConstructor(resume),
            },
        }
    }
}

impl From<crate::engine::builtins::DataViewConstructorStep> for Step {
    fn from(step: crate::engine::builtins::DataViewConstructorStep) -> Self {
        use crate::engine::builtins::DataViewConstructorStep as T;
        match step {
            T::Complete(result) => Self::Complete(result),
            T::Primitive { value, resume } => Self::Primitive {
                value,
                hint: crate::engine::vm::ToPrimitiveHint::Number,
                resume: Resume::DataViewConstructor(resume),
            },
            T::Prototype { new_target, resume } => Self::ConstructorSource {
                new_target,
                resume: Resume::DataViewConstructor(resume),
            },
        }
    }
}

impl From<crate::engine::builtins::TypedSetStep> for Step {
    fn from(step: crate::engine::builtins::TypedSetStep) -> Self {
        use crate::engine::builtins::TypedSetStep as T;
        match step {
            T::Complete(result) => Self::Complete(result),
            T::Read {
                object,
                key,
                resume,
            } => Self::Read {
                receiver: Value::Object(object.clone()),
                object,
                key,
                resume: Resume::TypedSet(resume),
            },
            T::Primitive { value, resume } => Self::Primitive {
                value,
                hint: crate::engine::vm::ToPrimitiveHint::Number,
                resume: Resume::TypedSet(resume),
            },
            T::Element {
                element,
                value,
                resume,
            } => Self::Element {
                element,
                value,
                resume: Resume::TypedSet(resume),
            },
        }
    }
}

impl From<crate::engine::builtins::TypedSearchStep> for Step {
    fn from(step: crate::engine::builtins::TypedSearchStep) -> Self {
        use crate::engine::builtins::TypedSearchStep as T;
        match step {
            T::Complete(result) => Self::Complete(result),
            T::Primitive { value, resume } => Self::Primitive {
                value,
                hint: crate::engine::vm::ToPrimitiveHint::Number,
                resume: Resume::TypedSearch(resume),
            },
        }
    }
}

impl From<crate::engine::builtins::TypedStringStep> for Step {
    fn from(step: crate::engine::builtins::TypedStringStep) -> Self {
        use crate::engine::builtins::TypedStringStep as T;
        match step {
            T::Complete(result) => Self::Complete(result),
            T::Primitive { value, resume } => Self::Primitive {
                value,
                hint: crate::engine::vm::ToPrimitiveHint::String,
                resume: Resume::TypedString(resume),
            },
            T::Read {
                receiver,
                key,
                resume,
            } => Self::ReadValue {
                receiver,
                key,
                resume: Resume::TypedString(resume),
            },
            T::Call {
                target,
                receiver,
                arguments,
                resume,
            } => Self::Call {
                target: target,
                receiver: receiver,
                arguments: arguments,
                resume: Resume::TypedString(resume),
            },
        }
    }
}

impl From<crate::engine::builtins::TypedSliceStep> for Step {
    fn from(step: crate::engine::builtins::TypedSliceStep) -> Self {
        use crate::engine::builtins::TypedSliceStep as T;
        match step {
            T::Complete(result) => Self::Complete(result),
            T::Primitive { value, resume } => Self::Primitive {
                value,
                hint: crate::engine::vm::ToPrimitiveHint::Number,
                resume: Resume::TypedSlice(resume),
            },
            T::Species {
                source,
                element,
                length,
                resume,
            } => Self::TypedSpecies {
                source,
                element,
                length,
                resume: Resume::TypedSlice(resume),
            },
            T::SpeciesView {
                source,
                element,
                buffer,
                offset,
                length,
                resume,
            } => Self::TypedSpeciesView {
                source,
                element,
                buffer,
                offset,
                length,
                resume: Resume::TypedSlice(resume),
            },
        }
    }
}

impl From<crate::engine::builtins::TypedMutationStep> for Step {
    fn from(step: crate::engine::builtins::TypedMutationStep) -> Self {
        use crate::engine::builtins::TypedMutationStep as T;
        match step {
            T::Complete(result) => Self::Complete(result),
            T::Primitive { value, resume } => Self::Primitive {
                value,
                hint: crate::engine::vm::ToPrimitiveHint::Number,
                resume: Resume::TypedMutation(resume),
            },
            T::Element {
                element,
                value,
                resume,
            } => Self::Element {
                element,
                value,
                resume: Resume::TypedMutation(resume),
            },
        }
    }
}

impl From<crate::engine::builtins::BufferSliceStep> for Step {
    fn from(step: crate::engine::builtins::BufferSliceStep) -> Self {
        use crate::engine::builtins::BufferSliceStep as T;
        match step {
            T::Complete(result) => Self::Complete(result),
            T::Read {
                object,
                key,
                resume,
            } => Self::Read {
                receiver: Value::Object(object.clone()),
                object,
                key,
                resume: Resume::BufferSlice(resume),
            },
            T::Primitive { value, resume } => Self::Primitive {
                value,
                hint: crate::engine::vm::ToPrimitiveHint::Number,
                resume: Resume::BufferSlice(resume),
            },
            T::Construct {
                constructor,
                arguments,
                resume,
            } => Self::Construct {
                new_target: crate::engine::vm::call::ConstructNewTarget::Validated(
                    constructor.clone(),
                ),
                target: constructor,
                arguments,
                resume: Resume::BufferSlice(resume),
            },
        }
    }
}

impl From<crate::engine::builtins::TypedWithStep> for Step {
    fn from(step: crate::engine::builtins::TypedWithStep) -> Self {
        use crate::engine::builtins::TypedWithStep as T;
        match step {
            T::Complete(result) => Self::Complete(result),
            T::Primitive { value, resume } => Self::Primitive {
                value,
                hint: crate::engine::vm::ToPrimitiveHint::Number,
                resume: Resume::TypedWith(resume),
            },
        }
    }
}

impl From<crate::engine::builtins::Uint8CodecStep> for Step {
    fn from(step: crate::engine::builtins::Uint8CodecStep) -> Self {
        use crate::engine::builtins::Uint8CodecStep as T;
        match step {
            T::Complete(result) => Self::Complete(result),
            T::Read {
                object,
                key,
                resume,
            } => Self::Read {
                receiver: Value::Object(object.clone()),
                object,
                key,
                resume: Resume::Uint8Codec(resume),
            },
        }
    }
}

impl From<crate::engine::builtins::TypedIteratorMethodStep> for Step {
    fn from(step: crate::engine::builtins::TypedIteratorMethodStep) -> Self {
        use crate::engine::builtins::TypedIteratorMethodStep as T;
        match step {
            T::Complete(result) => Self::TypedIteratorMethodComplete(result),
            T::Read {
                receiver,
                key,
                resume,
            } => Self::ReadValue {
                receiver,
                key,
                resume: Resume::TypedIteratorMethod(resume),
            },
        }
    }
}

impl From<crate::engine::builtins::TypedCollectStep> for Step {
    fn from(step: crate::engine::builtins::TypedCollectStep) -> Self {
        use crate::engine::builtins::TypedCollectStep as T;
        match step {
            T::Complete(result) => Self::TypedCollectComplete(result),
            T::Read {
                object,
                key,
                resume,
            } => Self::Read {
                receiver: Value::Object(object.clone()),
                object,
                key,
                resume: Resume::TypedCollect(resume),
            },
            T::Call {
                callable,
                receiver,
                resume,
            } => Self::Call {
                target: DirectCallTarget::Callable(callable),
                receiver: receiver,
                arguments: Vec::new(),
                resume: Resume::TypedCollect(resume),
            },
        }
    }
}

impl From<crate::engine::builtins::TypedCreateStep> for Step {
    fn from(step: crate::engine::builtins::TypedCreateStep) -> Self {
        use crate::engine::builtins::TypedCreateStep as T;
        match step {
            T::Complete(result) => Self::Complete(result),
            T::Read {
                receiver,
                key,
                resume,
            } => Self::ReadValue {
                receiver,
                key,
                resume: Resume::TypedCreate(resume),
            },
            T::Primitive { value, resume } => Self::Primitive {
                value,
                hint: crate::engine::vm::ToPrimitiveHint::Number,
                resume: Resume::TypedCreate(resume),
            },
            T::Call {
                target,
                receiver,
                arguments,
                resume,
            } => Self::Call {
                target: target,
                receiver: receiver,
                arguments: arguments,
                resume: Resume::TypedCreate(resume),
            },
            T::Prototype { new_target, resume } => Self::ConstructorSource {
                new_target,
                resume: Resume::TypedCreate(resume),
            },
            T::Method { source, resume } => Self::TypedIteratorMethod {
                source,
                resume: Resume::TypedCreate(resume),
            },
            T::Collect {
                source,
                method,
                element,
                resume,
            } => Self::TypedCollect {
                source,
                method,
                element,
                resume: Resume::TypedCreate(resume),
            },
            T::Create {
                constructor,
                length,
                resume,
            } => Self::TypedCreate {
                constructor,
                length,
                resume: Resume::TypedCreate(resume),
            },
            T::Element {
                element,
                value,
                resume,
            } => Self::Element {
                element,
                value,
                resume: Resume::TypedCreate(resume),
            },
        }
    }
}

impl From<crate::engine::builtins::AtomicsStep> for Step {
    fn from(step: crate::engine::builtins::AtomicsStep) -> Self {
        use crate::engine::builtins::AtomicsStep as T;
        match step {
            T::Complete(result) => Self::Complete(result),
            T::Primitive { value, resume } => Self::Primitive {
                value,
                hint: crate::engine::vm::ToPrimitiveHint::Number,
                resume: Resume::Atomics(resume),
            },
            T::Number { value, resume } => Self::Number {
                value,
                resume: Resume::Atomics(resume),
            },
        }
    }
}
