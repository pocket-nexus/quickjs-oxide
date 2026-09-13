//! Mechanical adapters for scalar domain requests.
use super::{DirectCallTarget, Resume, Step, ToPrimitiveHint, Value};

impl From<crate::engine::builtins::MathStep> for Step {
    fn from(step: crate::engine::builtins::MathStep) -> Self {
        use crate::engine::builtins::MathStep as T;
        match step {
            T::Complete(result) => Self::Complete(result),
            T::Number { value, resume } => Self::Number {
                value,
                resume: Resume::Math(resume),
            },
        }
    }
}

impl From<crate::engine::builtins::GlobalStep> for Step {
    fn from(step: crate::engine::builtins::GlobalStep) -> Self {
        use crate::engine::builtins::GlobalStep as T;
        match step {
            T::Complete(result) => Self::Complete(result),
            T::Number { value, resume } => Self::Number {
                value,
                resume: Resume::Global(resume),
            },
            T::String { value, resume } => Self::String {
                value,
                resume: Resume::Global(resume),
            },
        }
    }
}

impl From<crate::engine::builtins::NumericStep> for Step {
    fn from(step: crate::engine::builtins::NumericStep) -> Self {
        use crate::engine::builtins::NumericStep as T;
        match step {
            T::Complete(result) => Self::Complete(result),
            T::Number { value, resume } => Self::Number {
                value,
                resume: Resume::Numeric(resume),
            },
            T::Primitive { value, resume } => Self::Primitive {
                value,
                hint: ToPrimitiveHint::Number,
                resume: Resume::NumericPrimitive(resume),
            },
        }
    }
}

impl From<crate::engine::builtins::ScalarTextStep> for Step {
    fn from(step: crate::engine::builtins::ScalarTextStep) -> Self {
        use crate::engine::builtins::ScalarTextStep as T;
        match step {
            T::Complete(result) => Self::Complete(result),
            T::Number { value, resume } => Self::Number {
                value,
                resume: Resume::ScalarText(resume),
            },
            T::String { value, resume } => Self::String {
                value,
                resume: Resume::ScalarText(resume),
            },
        }
    }
}

impl From<crate::engine::builtins::PrimitiveConstructorStep> for Step {
    fn from(step: crate::engine::builtins::PrimitiveConstructorStep) -> Self {
        use crate::engine::builtins::PrimitiveConstructorStep as T;
        match step {
            T::Complete(result) => Self::Complete(result),
            T::String { value, resume } => Self::String {
                value,
                resume: Resume::PrimitiveConstructor(resume),
            },
            T::Read {
                receiver,
                key,
                resume,
            } => Self::ReadValue {
                receiver,
                key,
                resume: Resume::PrimitiveConstructor(resume),
            },
            T::Primitive { value, resume } => Self::Primitive {
                value,
                hint: ToPrimitiveHint::Number,
                resume: Resume::PrimitiveConstructorValue(resume),
            },
        }
    }
}

impl From<crate::engine::builtins::DateConstructorStep> for Step {
    fn from(step: crate::engine::builtins::DateConstructorStep) -> Self {
        use crate::engine::builtins::DateConstructorStep as T;
        match step {
            T::Complete(result) => Self::Complete(result),
            T::Number { value, resume } => Self::Number {
                value,
                resume: Resume::DateConstructor(resume),
            },
            T::String { value, resume } => Self::String {
                value,
                resume: Resume::DateConstructor(resume),
            },
            T::Read {
                receiver,
                key,
                resume,
            } => Self::ReadValue {
                receiver,
                key,
                resume: Resume::DateConstructor(resume),
            },
            T::Primitive { value, resume } => Self::Primitive {
                value,
                hint: ToPrimitiveHint::Default,
                resume: Resume::DateConstructorPrimitive(resume),
            },
        }
    }
}

impl From<crate::engine::builtins::DatePrototypeStep> for Step {
    fn from(step: crate::engine::builtins::DatePrototypeStep) -> Self {
        use crate::engine::builtins::DatePrototypeStep as T;
        match step {
            T::Complete(result) => Self::Complete(result),
            T::Number { value, resume } => Self::Number {
                value,
                resume: Resume::DatePrototype(resume),
            },
            T::Primitive {
                value,
                hint,
                resume,
            } => Self::Primitive {
                value,
                hint,
                resume: Resume::DatePrototype(resume),
            },
            T::OrdinaryPrimitive { object, hint } => Self::OrdinaryPrimitive { object, hint },
            T::Read {
                object,
                key,
                resume,
            } => Self::Read {
                receiver: Value::Object(object.clone()),
                object,
                key,
                resume: Resume::DatePrototype(resume),
            },
            T::Call { callable, receiver } => Self::Call {
                target: DirectCallTarget::Callable(callable),
                receiver,
                arguments: Vec::new(),
                resume: Resume::Identity,
            },
        }
    }
}

impl From<crate::engine::builtins::ErrorStep> for Step {
    fn from(step: crate::engine::builtins::ErrorStep) -> Self {
        use crate::engine::builtins::ErrorStep as T;
        match step {
            T::Complete(result) => Self::Complete(result),
            T::Read {
                receiver,
                key,
                resume,
            } => Self::ReadValue {
                receiver,
                key,
                resume: Resume::Error(resume),
            },
            T::String { value, resume } => Self::String {
                value,
                resume: Resume::Error(resume),
            },
            T::Has {
                object,
                key,
                resume,
            } => Self::Has {
                object,
                key,
                resume: Resume::Error(resume),
            },
            T::Aggregate { iterable, resume } => Self::Aggregate {
                iterable,
                resume: Resume::Error(resume),
            },
        }
    }
}

impl From<crate::engine::builtins::SumStep> for Step {
    fn from(step: crate::engine::builtins::SumStep) -> Self {
        use crate::engine::builtins::SumStep as T;
        match step {
            T::Complete(result) => Self::Complete(result),
            T::Read {
                receiver,
                key,
                resume,
            } => Self::ReadValue {
                receiver,
                key,
                resume: Resume::Sum(resume),
            },
            T::Call {
                callable,
                receiver,
                resume,
            } => Self::Call {
                target: DirectCallTarget::Callable(callable),
                receiver,
                arguments: Vec::new(),
                resume: Resume::Sum(resume),
            },
            T::Next {
                iterator,
                next,
                resume,
            } => Self::IteratorNext {
                iterator,
                method: next,
                resume: Resume::Sum(resume),
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

impl From<crate::engine::builtins::AggregateStep> for Step {
    fn from(step: crate::engine::builtins::AggregateStep) -> Self {
        use crate::engine::builtins::AggregateStep as T;
        match step {
            T::Complete(result) => Self::Complete(result),
            T::Read {
                receiver,
                key,
                resume,
            } => Self::ReadValue {
                receiver,
                key,
                resume: Resume::Aggregate(resume),
            },
            T::Call {
                callable,
                receiver,
                resume,
            } => Self::Call {
                target: DirectCallTarget::Callable(callable),
                receiver,
                arguments: Vec::new(),
                resume: Resume::Aggregate(resume),
            },
            T::Next {
                iterator,
                next,
                resume,
            } => Self::IteratorNext {
                iterator,
                method: next,
                resume: Resume::Aggregate(resume),
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
