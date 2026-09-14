//! Mechanical adapters for array domain requests.
use super::{DirectCallTarget, Resume, Step, Value};

impl From<crate::engine::builtins::ArrayMutationStep> for Step {
    fn from(step: crate::engine::builtins::ArrayMutationStep) -> Self {
        use crate::engine::builtins::ArrayMutationStep as T;
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
                resume: Resume::ArrayMutation(resume),
            },
            T::Number { value, resume } => Self::Number {
                value,
                resume: Resume::ArrayMutation(resume),
            },
            T::Copy {
                object,
                to,
                from,
                count,
                backwards,
                resume,
            } => Self::ArrayCopy {
                object,
                to,
                from,
                count,
                backwards,
                resume: Resume::ArrayMutation(resume),
            },
            T::Set {
                object,
                key,
                value,
                resume,
            } => Self::Set {
                receiver: Value::Object(object.clone()),
                object,
                value,
                key: key.clone(),
                resume: Resume::ArrayMutationSet { key, resume },
            },
            T::Delete {
                object,
                key,
                resume,
            } => Self::Delete {
                object,
                key,
                resume: Resume::ArrayMutation(resume),
            },
        }
    }
}

impl From<crate::engine::builtins::ArrayCallbackStep> for Step {
    fn from(step: crate::engine::builtins::ArrayCallbackStep) -> Self {
        use crate::engine::builtins::ArrayCallbackStep as T;
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
                resume: Resume::ArrayCallback(resume),
            },
            T::Number { value, resume } => Self::Number {
                value,
                resume: Resume::ArrayCallback(resume),
            },
            T::Has {
                object,
                key,
                resume,
            } => Self::Has {
                object,
                key,
                resume: Resume::ArrayCallback(resume),
            },
            T::Call {
                callable,
                receiver,
                arguments,
                resume,
            } => Self::Call {
                target: DirectCallTarget::Callable(callable),
                receiver,
                arguments,
                resume: Resume::ArrayCallback(resume),
            },
            T::Species {
                source,
                length,
                resume,
            } => Self::ArraySpecies {
                source,
                length,
                resume: Resume::ArrayCallback(resume),
            },
            T::Define {
                object,
                key,
                descriptor,
                resume,
            } => Self::Define {
                object,
                key,
                descriptor,
                resume: Resume::ArrayCallback(resume),
            },
        }
    }
}

impl From<crate::engine::builtins::ArraySpeciesStep> for Step {
    fn from(step: crate::engine::builtins::ArraySpeciesStep) -> Self {
        use crate::engine::builtins::ArraySpeciesStep as T;
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
                resume: Resume::ArraySpecies(resume),
            },
            T::Construct { target, arguments } => Self::Construct {
                new_target: crate::engine::vm::call::ConstructNewTarget::Validated(target.clone()),
                target,
                arguments,
                resume: Resume::Identity,
            },
        }
    }
}

impl From<crate::engine::builtins::ArrayNextStep> for Step {
    fn from(step: crate::engine::builtins::ArrayNextStep) -> Self {
        use crate::engine::builtins::ArrayNextStep as T;
        match step {
            T::Complete(result) => Self::NativeRawComplete(result),
            T::PreparedRead { read, key, resume } => Self::PreparedRead {
                read,
                key,
                resume: Resume::ArrayNext(resume),
            },
            T::Read {
                object,
                key,
                resume,
            } => Self::Read {
                receiver: Value::Object(object.clone()),
                object,
                key,
                resume: Resume::ArrayNext(resume),
            },
            T::Number { value, resume } => Self::Number {
                value,
                resume: Resume::ArrayNext(resume),
            },
        }
    }
}

impl From<crate::engine::builtins::ArraySortStep> for Step {
    fn from(step: crate::engine::builtins::ArraySortStep) -> Self {
        use crate::engine::builtins::ArraySortStep as T;
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
                resume: Resume::ArraySort(resume),
            },
            T::Number { value, resume } => Self::Number {
                value,
                resume: Resume::ArraySort(resume),
            },
            T::Has {
                object,
                key,
                resume,
            } => Self::Has {
                object,
                key,
                resume: Resume::ArraySort(resume),
            },
            T::Set {
                object,
                key,
                value,
                resume,
            } => Self::Set {
                receiver: Value::Object(object.clone()),
                object,
                key: key.clone(),
                value,
                resume: Resume::ArraySortSet { key, resume },
            },
            T::Delete {
                object,
                key,
                resume,
            } => Self::Delete {
                object,
                key,
                resume: Resume::ArraySort(resume),
            },
            T::String { value, resume } => Self::String {
                value,
                resume: Resume::ArraySort(resume),
            },
            T::Call {
                callable,
                arguments,
                resume,
            } => Self::Call {
                target: DirectCallTarget::Callable(callable),
                receiver: Value::Undefined,
                arguments: arguments,
                resume: Resume::ArraySort(resume),
            },
        }
    }
}

impl From<crate::engine::builtins::ArrayIndexedStep> for Step {
    fn from(step: crate::engine::builtins::ArrayIndexedStep) -> Self {
        use crate::engine::builtins::ArrayIndexedStep as T;
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
                resume: Resume::ArrayIndexed(resume),
            },
            T::Number { value, resume } => Self::Number {
                value,
                resume: Resume::ArrayIndexed(resume),
            },
            T::Has {
                object,
                key,
                resume,
            } => Self::Has {
                object,
                key,
                resume: Resume::ArrayIndexed(resume),
            },
            T::Set {
                object,
                key,
                value,
                resume,
            } => Self::Set {
                receiver: Value::Object(object.clone()),
                object,
                key: key.clone(),
                value,
                resume: Resume::ArrayIndexedSet { key, resume },
            },
            T::Copy {
                object,
                to,
                from,
                count,
                backwards,
                resume,
            } => Self::ArrayCopy {
                object,
                to,
                from,
                count,
                backwards,
                resume: Resume::ArrayIndexed(resume),
            },
        }
    }
}

impl From<crate::engine::builtins::ArrayReverseStep> for Step {
    fn from(step: crate::engine::builtins::ArrayReverseStep) -> Self {
        use crate::engine::builtins::ArrayReverseStep as T;
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
                resume: Resume::ArrayReverse(resume),
            },
            T::Number { value, resume } => Self::Number {
                value,
                resume: Resume::ArrayReverse(resume),
            },
            T::Has {
                object,
                key,
                resume,
            } => Self::Has {
                object,
                key,
                resume: Resume::ArrayReverse(resume),
            },
            T::Set {
                object,
                key,
                value,
                resume,
            } => Self::Set {
                receiver: Value::Object(object.clone()),
                object,
                key: key.clone(),
                value,
                resume: Resume::ArrayReverseSet { key, resume },
            },
            T::Delete {
                object,
                key,
                resume,
            } => Self::Delete {
                object,
                key,
                resume: Resume::ArrayReverse(resume),
            },
        }
    }
}

impl From<crate::engine::builtins::ArrayStringStep> for Step {
    fn from(step: crate::engine::builtins::ArrayStringStep) -> Self {
        use crate::engine::builtins::ArrayStringStep as T;
        match step {
            T::Complete(result) => Self::Complete(result),
            T::Read {
                receiver,
                key,
                resume,
            } => Self::ReadValue {
                receiver,
                key,
                resume: Resume::ArrayString(resume),
            },
            T::Number { value, resume } => Self::Number {
                value,
                resume: Resume::ArrayString(resume),
            },
            T::String { value, resume } => Self::String {
                value,
                resume: Resume::ArrayString(resume),
            },
            T::Call {
                callable,
                receiver,
                resume,
            } => Self::Call {
                target: DirectCallTarget::Callable(callable),
                receiver: receiver,
                arguments: Vec::new(),
                resume: Resume::ArrayString(resume),
            },
            T::ObjectTag { receiver } => Self::ObjectTag { receiver },
        }
    }
}

impl From<crate::engine::builtins::ArrayBuildStep> for Step {
    fn from(step: crate::engine::builtins::ArrayBuildStep) -> Self {
        use crate::engine::builtins::ArrayBuildStep as T;
        match step {
            T::Complete(result) => Self::Complete(result),
            T::Read {
                receiver,
                key,
                resume,
            } => Self::ReadValue {
                receiver,
                key,
                resume: Resume::ArrayBuild(resume),
            },
            T::Number { value, resume } => Self::Number {
                value,
                resume: Resume::ArrayBuild(resume),
            },
            T::Call {
                callable,
                receiver,
                arguments,
                resume,
            } => Self::Call {
                target: DirectCallTarget::Callable(callable),
                receiver: receiver,
                arguments: arguments,
                resume: Resume::ArrayBuild(resume),
            },
            T::Set {
                object,
                key,
                value,
                resume,
            } => Self::Set {
                receiver: Value::Object(object.clone()),
                object,
                key: key.clone(),
                value,
                resume: Resume::ArrayBuildSet { key, resume },
            },
            T::Close {
                iterator,
                completion,
            } => Self::IteratorClose {
                iterator,
                completion,
            },
            T::Construct {
                target,
                arguments,
                resume,
            } => Self::Construct {
                new_target: crate::engine::vm::call::ConstructNewTarget::Validated(target.clone()),
                target,
                arguments,
                resume: Resume::ArrayBuild(resume),
            },
            T::Parse { result, resume } => Self::ParseIterator {
                result,
                resume: Resume::ArrayBuild(resume),
            },
            T::Define {
                object,
                key,
                descriptor,
                resume,
            } => Self::Define {
                object,
                key,
                descriptor,
                resume: Resume::ArrayBuild(resume),
            },
        }
    }
}

impl From<crate::engine::builtins::ArrayCopyStep> for Step {
    fn from(step: crate::engine::builtins::ArrayCopyStep) -> Self {
        use crate::engine::builtins::ArrayCopyStep as T;
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
                resume: Resume::ArrayCopy(resume),
            },
            T::Has {
                object,
                key,
                resume,
            } => Self::Has {
                object,
                key,
                resume: Resume::ArrayCopy(resume),
            },
            T::Set {
                object,
                key,
                value,
                resume,
            } => Self::Set {
                receiver: Value::Object(object.clone()),
                object,
                key: key.clone(),
                value,
                resume: Resume::ArrayCopySet { key, resume },
            },
            T::Delete {
                object,
                key,
                resume,
            } => Self::Delete {
                object,
                key,
                resume: Resume::ArrayCopy(resume),
            },
        }
    }
}

impl From<crate::engine::builtins::ArrayConcatStep> for Step {
    fn from(step: crate::engine::builtins::ArrayConcatStep) -> Self {
        use crate::engine::builtins::ArrayConcatStep as T;
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
                resume: Resume::ArrayConcat(resume),
            },
            T::Number { value, resume } => Self::Number {
                value,
                resume: Resume::ArrayConcat(resume),
            },
            T::Has {
                object,
                key,
                resume,
            } => Self::Has {
                object,
                key,
                resume: Resume::ArrayConcat(resume),
            },
            T::Set {
                object,
                key,
                value,
                resume,
            } => Self::Set {
                receiver: Value::Object(object.clone()),
                object,
                key: key.clone(),
                value,
                resume: Resume::ArrayConcatSet { key, resume },
            },
            T::Species { source, resume } => Self::ArraySpecies {
                source,
                length: 0,
                resume: Resume::ArrayConcat(resume),
            },
            T::Define {
                object,
                key,
                descriptor,
                resume,
            } => Self::Define {
                object,
                key,
                descriptor,
                resume: Resume::ArrayConcat(resume),
            },
        }
    }
}

impl From<crate::engine::builtins::ArrayFlattenStep> for Step {
    fn from(step: crate::engine::builtins::ArrayFlattenStep) -> Self {
        use crate::engine::builtins::ArrayFlattenStep as T;
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
                resume: Resume::ArrayFlatten(resume),
            },
            T::Number { value, resume } => Self::Number {
                value,
                resume: Resume::ArrayFlatten(resume),
            },
            T::Has {
                object,
                key,
                resume,
            } => Self::Has {
                object,
                key,
                resume: Resume::ArrayFlatten(resume),
            },
            T::Call {
                callable,
                receiver,
                arguments,
                resume,
            } => Self::Call {
                target: DirectCallTarget::Callable(callable),
                receiver: receiver,
                arguments: arguments,
                resume: Resume::ArrayFlatten(resume),
            },
            T::Species { source, resume } => Self::ArraySpecies {
                source,
                length: 0,
                resume: Resume::ArrayFlatten(resume),
            },
            T::Define {
                object,
                key,
                descriptor,
                resume,
            } => Self::Define {
                object,
                key,
                descriptor,
                resume: Resume::ArrayFlatten(resume),
            },
        }
    }
}

impl From<crate::engine::builtins::ArrayConstructorStep> for Step {
    fn from(step: crate::engine::builtins::ArrayConstructorStep) -> Self {
        use crate::engine::builtins::ArrayConstructorStep as T;
        match step {
            T::Complete(result) => Self::Complete(result),
            T::Read {
                receiver,
                key,
                resume,
            } => Self::ReadValue {
                receiver,
                key,
                resume: Resume::ArrayConstructor(resume),
            },
            T::Set {
                object,
                key,
                value,
                resume,
            } => Self::Set {
                receiver: Value::Object(object.clone()),
                object,
                key: key.clone(),
                value,
                resume: Resume::ArrayConstructorSet { key, resume },
            },
        }
    }
}

impl From<crate::engine::builtins::ArraySliceStep> for Step {
    fn from(step: crate::engine::builtins::ArraySliceStep) -> Self {
        use crate::engine::builtins::ArraySliceStep as T;
        match step {
            T::Complete(result) => Self::Complete(result),
            T::PreparedRead { read, key, resume } => Self::PreparedRead {
                read,
                key,
                resume: Resume::ArraySlice(resume),
            },
            T::PreparedHas { probe, key, resume } => Self::PreparedHas {
                probe,
                key,
                resume: Resume::ArraySlice(resume),
            },
            T::Read {
                object,
                key,
                resume,
            } => Self::Read {
                receiver: Value::Object(object.clone()),
                object,
                key,
                resume: Resume::ArraySlice(resume),
            },
            T::Number { value, resume } => Self::Number {
                value,
                resume: Resume::ArraySlice(resume),
            },
            T::Has {
                object,
                key,
                resume,
            } => Self::Has {
                object,
                key,
                resume: Resume::ArraySlice(resume),
            },
            T::Set {
                object,
                key,
                value,
                resume,
            } => Self::Set {
                receiver: Value::Object(object.clone()),
                object,
                key: key.clone(),
                value,
                resume: Resume::ArraySliceSet { key, resume },
            },
            T::Delete {
                object,
                key,
                resume,
            } => Self::Delete {
                object,
                key,
                resume: Resume::ArraySlice(resume),
            },
            T::Species {
                source,
                length,
                resume,
            } => Self::ArraySpecies {
                source,
                length,
                resume: Resume::ArraySlice(resume),
            },
            T::Define {
                object,
                key,
                descriptor,
                resume,
            } => Self::Define {
                object,
                key,
                descriptor,
                resume: Resume::ArraySlice(resume),
            },
            T::Copy {
                object,
                to,
                from,
                count,
                backwards,
                resume,
            } => Self::ArrayCopy {
                object,
                to,
                from,
                count,
                backwards,
                resume: Resume::ArraySlice(resume),
            },
        }
    }
}
