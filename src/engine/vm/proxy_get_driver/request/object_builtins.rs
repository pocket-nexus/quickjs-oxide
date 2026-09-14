//! Mechanical adapters for object builtins domain requests.
use super::{DirectCallTarget, Resume, Step, Value};

impl From<crate::engine::builtins::BuiltinPrototypeStep> for Step {
    fn from(step: crate::engine::builtins::BuiltinPrototypeStep) -> Self {
        use crate::engine::builtins::BuiltinPrototypeStep;
        match step {
            BuiltinPrototypeStep::Complete(result) => Self::Complete(result),
            BuiltinPrototypeStep::Get { object, resume } => Self::GetPrototype {
                object,
                resume: Resume::BuiltinPrototype(resume),
            },
            BuiltinPrototypeStep::Set {
                object,
                prototype,
                resume,
            } => Self::SetPrototype {
                object,
                prototype,
                resume: Resume::BuiltinPrototype(resume),
            },
        }
    }
}

impl From<crate::engine::builtins::PropertyStep> for Step {
    fn from(step: crate::engine::builtins::PropertyStep) -> Self {
        use crate::engine::builtins::PropertyStep;
        match step {
            PropertyStep::Complete(result) => Self::Complete(result),
            PropertyStep::Keys { object, resume } => Self::Keys {
                object,
                resume: Resume::Property(resume),
            },
            PropertyStep::Key { value, resume } => Self::Primitive {
                value,
                hint: crate::engine::vm::ToPrimitiveHint::String,
                resume: Resume::PropertyKey(resume),
            },
            PropertyStep::Convert { value, resume } => Self::Convert {
                value,
                resume: Resume::Property(resume),
            },
            PropertyStep::Read {
                object,
                key,
                receiver,
                resume,
            } => Self::Read {
                object,
                key,
                receiver,
                resume: Resume::Property(resume),
            },
            PropertyStep::Set {
                object,
                key,
                value,
                receiver,
                resume,
            } => Self::Set {
                object,
                key,
                value,
                receiver,
                resume: Resume::Property(resume),
            },
            PropertyStep::Has {
                object,
                key,
                resume,
            } => Self::Has {
                object,
                key,
                resume: Resume::Property(resume),
            },
            PropertyStep::Delete {
                object,
                key,
                resume,
            } => Self::Delete {
                object,
                key,
                resume: Resume::Property(resume),
            },
            PropertyStep::Define {
                object,
                key,
                descriptor,
                resume,
            } => Self::Define {
                object,
                key,
                descriptor,
                resume: Resume::Property(resume),
            },
            PropertyStep::Descriptor {
                object,
                key,
                resume,
            } => Self::Descriptor {
                object,
                key,
                resume: Resume::Property(resume),
            },
            PropertyStep::Extensible { object, resume } => Self::Extensible {
                object,
                resume: Resume::Property(resume),
            },
            PropertyStep::Prevent { object, resume } => Self::PreventExtensions {
                object,
                resume: Resume::Property(resume),
            },
        }
    }
}

impl From<crate::engine::builtins::PredicateStep> for Step {
    fn from(step: crate::engine::builtins::PredicateStep) -> Self {
        use crate::engine::builtins::PredicateStep;
        match step {
            PredicateStep::Complete(result) => Self::Complete(result),
            PredicateStep::Descriptor {
                object,
                key,
                resume,
            } => Self::Descriptor {
                object,
                key,
                resume: Resume::Predicate(resume),
            },
            PredicateStep::Prototype { object, resume } => Self::GetPrototype {
                object,
                resume: Resume::Predicate(resume),
            },
            PredicateStep::Define {
                object,
                key,
                descriptor,
                resume,
            } => Self::Define {
                object,
                key,
                descriptor,
                resume: Resume::Predicate(resume),
            },
            PredicateStep::Key { value, resume } => Self::Primitive {
                value,
                hint: crate::engine::vm::ToPrimitiveHint::String,
                resume: Resume::PredicateKey(resume),
            },
            PredicateStep::Own {
                object,
                key,
                enumerable,
                resume,
            } => Self::OwnFlag {
                object,
                key,
                enumerable,
                resume: Resume::Predicate(resume),
            },
        }
    }
}

impl From<crate::engine::builtins::DefinitionsStep> for Step {
    fn from(step: crate::engine::builtins::DefinitionsStep) -> Self {
        use crate::engine::builtins::DefinitionsStep;
        match step {
            DefinitionsStep::Complete(result) => Self::Complete(result),
            DefinitionsStep::Keys { object, resume } => Self::Keys {
                object,
                resume: Resume::Definitions(resume),
            },
            DefinitionsStep::Enumerable {
                object,
                key,
                resume,
            } => Self::SnapshotEnumerable {
                object,
                key,
                resume: Resume::Definitions(resume),
            },
            DefinitionsStep::Read {
                object,
                key,
                resume,
            } => Self::Read {
                receiver: Value::Object(object.clone()),
                object,
                key,
                resume: Resume::Definitions(resume),
            },
            DefinitionsStep::Convert { value, resume } => Self::Convert {
                value,
                resume: Resume::Definitions(resume),
            },
            DefinitionsStep::Define {
                object,
                key,
                descriptor,
                resume,
            } => Self::Define {
                object,
                key,
                descriptor,
                resume: Resume::Definitions(resume),
            },
        }
    }
}

impl From<crate::engine::builtins::ObjectStringStep> for Step {
    fn from(step: crate::engine::builtins::ObjectStringStep) -> Self {
        use crate::engine::builtins::ObjectStringStep;
        match step {
            ObjectStringStep::Complete(result) => Self::Complete(result),
            ObjectStringStep::Read {
                receiver,
                key,
                resume,
            } => Self::ReadValue {
                receiver,
                key,
                resume: Resume::ObjectString(resume),
            },
            ObjectStringStep::Call {
                target,
                receiver,
                resume,
            } => Self::Call {
                target,
                receiver,
                arguments: Vec::new(),
                resume: Resume::ObjectString(resume),
            },
        }
    }
}

impl From<crate::engine::builtins::ObjectIterationStep> for Step {
    fn from(step: crate::engine::builtins::ObjectIterationStep) -> Self {
        use crate::engine::builtins::ObjectIterationStep as T;
        match step {
            T::Complete(result) => Self::Complete(result),
            T::Read {
                receiver,
                key,
                resume,
            } => Self::ReadValue {
                receiver,
                key,
                resume: Resume::ObjectIteration(resume),
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
                resume: Resume::ObjectIteration(resume),
            },
            T::Next {
                iterator,
                method,
                resume,
            } => Self::IteratorNext {
                iterator,
                method,
                resume: Resume::ObjectIteration(resume),
            },
            T::Key { value, resume } => Self::Primitive {
                value,
                hint: crate::engine::vm::ToPrimitiveHint::String,
                resume: Resume::ObjectIterationKey(resume),
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
                resume: Resume::ObjectIteration(resume),
            },
            T::Push {
                object,
                value,
                resume,
            } => Self::ArrayPush {
                object,
                value,
                resume: Resume::ObjectIteration(resume),
            },
            T::Close {
                iterator,
                completion,
            } => Self::IteratorClose {
                iterator,
                completion,
            },
        }
    }
}

impl From<crate::engine::builtins::ObjectCopyStep> for Step {
    fn from(step: crate::engine::builtins::ObjectCopyStep) -> Self {
        use crate::engine::builtins::ObjectCopyStep as T;
        match step {
            T::Complete(result) => Self::Complete(result),
            #[cfg(feature = "stack-vm")]
            T::PreparedRead(prepared) => Self::PreparedRead {
                read: prepared.read,
                key: prepared.key,
                resume: Resume::ObjectCopy(prepared.resume),
            },
            T::Read {
                object,
                key,
                resume,
            } => Self::Read {
                receiver: Value::Object(object.clone()),
                object,
                key,
                resume: Resume::ObjectCopy(resume),
            },
            T::Keys { object, resume } => Self::Keys {
                object,
                resume: Resume::ObjectCopy(resume),
            },
            T::Enumerable {
                object,
                key,
                resume,
            } => Self::SnapshotEnumerable {
                object,
                key,
                resume: Resume::ObjectCopy(resume),
            },
        }
    }
}

impl From<crate::engine::builtins::ObjectConstructorStep> for Step {
    fn from(step: crate::engine::builtins::ObjectConstructorStep) -> Self {
        use crate::engine::builtins::ObjectConstructorStep as T;
        match step {
            T::Complete(result) => Self::Complete(result),
            T::Prototype { new_target, resume } => Self::ConstructorSource {
                new_target,
                resume: Resume::ObjectConstructor(resume),
            },
        }
    }
}

impl From<crate::engine::builtins::JsonParseStep> for Step {
    fn from(step: crate::engine::builtins::JsonParseStep) -> Self {
        use crate::engine::builtins::JsonParseStep as T;
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
                resume: Resume::JsonParse(resume),
            },
            T::String { value, resume } => Self::String {
                value,
                resume: Resume::JsonParse(resume),
            },
            T::Number { value, resume } => Self::Number {
                value,
                resume: Resume::JsonParse(resume),
            },
            T::Keys { object, resume } => Self::Keys {
                object,
                resume: Resume::JsonParse(resume),
            },
            T::Enumerable {
                object,
                key,
                resume,
            } => Self::SnapshotEnumerable {
                object,
                key,
                resume: Resume::JsonParse(resume),
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
                resume: Resume::JsonParse(resume),
            },
            T::Delete {
                object,
                key,
                resume,
            } => Self::Delete {
                object,
                key,
                resume: Resume::JsonParse(resume),
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
                resume: Resume::JsonParse(resume),
            },
        }
    }
}

impl From<crate::engine::builtins::JsonStringifyStep> for Step {
    fn from(step: crate::engine::builtins::JsonStringifyStep) -> Self {
        use crate::engine::builtins::JsonStringifyStep as T;
        match step {
            T::Complete(result) => Self::Complete(result),
            T::Read {
                receiver,
                key,
                resume,
            } => Self::ReadValue {
                receiver,
                key,
                resume: Resume::JsonStringify(resume),
            },
            T::String { value, resume } => Self::String {
                value,
                resume: Resume::JsonStringify(resume),
            },
            T::Number { value, resume } => Self::Number {
                value,
                resume: Resume::JsonStringify(resume),
            },
            T::Keys { object, resume } => Self::Keys {
                object,
                resume: Resume::JsonStringify(resume),
            },
            T::Enumerable {
                object,
                key,
                resume,
            } => Self::OwnFlag {
                object,
                key,
                enumerable: true,
                resume: Resume::JsonStringify(resume),
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
                resume: Resume::JsonStringify(resume),
            },
        }
    }
}
