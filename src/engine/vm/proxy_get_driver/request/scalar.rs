//! Mechanical adapters for scalar domain requests.
use super::{DirectCallTarget, Resume, Step, ToPrimitiveHint};

impl TryFrom<crate::engine::builtins::MathStep> for Step {
    type Error = crate::engine::api::RuntimeError;
    fn try_from(step: crate::engine::builtins::MathStep) -> Result<Self, Self::Error> {
        Ok({
            use crate::engine::builtins::MathStep as T;
            match step {
                T::Complete(result) => Self::Complete(Some(result)),
                T::CyclePublished(result) => Self::CyclePublishedComplete(Some(result)),
                T::Number { value, resume } => Self::Number {
                    value: Some(value),
                    resume: Some(Resume::Math(resume)),
                },
            }
        })
    }
}

impl TryFrom<crate::engine::builtins::GlobalStep> for Step {
    type Error = crate::engine::api::RuntimeError;
    fn try_from(step: crate::engine::builtins::GlobalStep) -> Result<Self, Self::Error> {
        Ok({
            use crate::engine::builtins::GlobalStep as T;
            match step {
                T::Complete(result) => Self::Complete(Some(result)),
                T::Number { value, resume } => Self::Number {
                    value: Some(value),
                    resume: Some(Resume::Global(resume)),
                },
                T::String { value, resume } => Self::String {
                    value: Some(value),
                    resume: Some(Resume::Global(resume)),
                },
            }
        })
    }
}

impl TryFrom<crate::engine::builtins::NumericStep> for Step {
    type Error = crate::engine::api::RuntimeError;
    fn try_from(step: crate::engine::builtins::NumericStep) -> Result<Self, Self::Error> {
        Ok({
            use crate::engine::builtins::NumericStep as T;
            match step {
                T::Complete(result) => Self::Complete(Some(result)),
                T::CyclePublished(result) => Self::CyclePublishedComplete(Some(result)),
                T::Number { value, resume } => Self::Number {
                    value: Some(value),
                    resume: Some(Resume::Numeric(resume)),
                },
                T::Primitive { value, resume } => Self::Primitive {
                    value: Some(value),
                    hint: Some(ToPrimitiveHint::Number),
                    resume: Some(Resume::NumericPrimitive(resume)),
                },
            }
        })
    }
}

impl TryFrom<crate::engine::builtins::ScalarTextStep> for Step {
    type Error = crate::engine::api::RuntimeError;
    fn try_from(step: crate::engine::builtins::ScalarTextStep) -> Result<Self, Self::Error> {
        Ok({
            use crate::engine::builtins::ScalarTextStep as T;
            match step {
                T::Complete(result) => Self::Complete(Some(result)),
                T::CyclePublished(result) => Self::CyclePublishedComplete(Some(result)),
                T::Number { value, resume } => Self::Number {
                    value: Some(value),
                    resume: Some(Resume::ScalarText(resume)),
                },
                T::String { value, resume } => Self::String {
                    value: Some(value),
                    resume: Some(Resume::ScalarText(resume)),
                },
            }
        })
    }
}

impl TryFrom<crate::engine::builtins::PrimitiveConstructorStep> for Step {
    type Error = crate::engine::api::RuntimeError;
    fn try_from(
        step: crate::engine::builtins::PrimitiveConstructorStep,
    ) -> Result<Self, Self::Error> {
        Ok({
            use crate::engine::builtins::PrimitiveConstructorStep as T;
            match step {
                T::Complete(result) => Self::Complete(Some(result)),
                T::String { mut resume } => {
                    let value = resume.take_string_value();
                    Self::String {
                        value: Some(value),
                        resume: Some(Resume::PrimitiveConstructor(resume)),
                    }
                }
                T::Read { mut resume } => {
                    let receiver = resume.take_read_receiver();
                    let key = resume.take_read_key();
                    Self::ReadValue {
                        receiver: Some(receiver),
                        key: Some(key),
                        resume: Some(Resume::PrimitiveConstructor(resume)),
                    }
                }
                T::Primitive { mut resume } => {
                    let value = resume.take_primitive_value();
                    Self::Primitive {
                        value: Some(value),
                        hint: Some(ToPrimitiveHint::Number),
                        resume: Some(Resume::PrimitiveConstructorValue(resume)),
                    }
                }
            }
        })
    }
}

impl TryFrom<crate::engine::builtins::DateConstructorStep> for Step {
    type Error = crate::engine::api::RuntimeError;
    fn try_from(step: crate::engine::builtins::DateConstructorStep) -> Result<Self, Self::Error> {
        use crate::engine::builtins::DateConstructorStep as T;
        Ok(match step {
            T::Complete(value) => Self::Complete(Some(value)),
            T::CyclePublished(value) => Self::CyclePublishedComplete(Some(value)),
            T::Number { value, resume } => Self::Number {
                value: Some(value),
                resume: Some(Resume::DateConstructor(resume)),
            },
            T::String { value, resume } => Self::String {
                value: Some(value),
                resume: Some(Resume::DateConstructor(resume)),
            },
            T::Primitive { value, resume } => Self::Primitive {
                value: Some(value),
                hint: Some(ToPrimitiveHint::Default),
                resume: Some(Resume::DateConstructorPrimitive(resume)),
            },
            T::Read {
                realm,
                receiver,
                key,
                resume,
            } => Self::RawValueReadRequest {
                realm,
                selected: None,
                receiver: Some(receiver),
                key,
                resume: Some(Resume::DateConstructor(resume)),
            },
        })
    }
}

impl TryFrom<crate::engine::builtins::DatePrototypeStep> for Step {
    type Error = crate::engine::api::RuntimeError;
    fn try_from(step: crate::engine::builtins::DatePrototypeStep) -> Result<Self, Self::Error> {
        Ok({
            use crate::engine::builtins::DatePrototypeStep as T;
            match step {
                T::Complete(result) => Self::Complete(Some(result)),
                T::CyclePublished(result) => Self::CyclePublishedComplete(Some(result)),
                T::CyclePublishedPrimitive {
                    value,
                    hint,
                    resume,
                } => Self::CyclePublishedPrimitive {
                    value: Some(value),
                    hint: Some(hint),
                    resume: Some(Resume::DatePrototype(resume)),
                },
                T::Number { value, resume } => Self::Number {
                    value: Some(value),
                    resume: Some(Resume::DatePrototype(resume)),
                },
                T::Primitive {
                    value,
                    hint,
                    resume,
                } => Self::Primitive {
                    value: Some(value),
                    hint: Some(hint),
                    resume: Some(Resume::DatePrototype(resume)),
                },
                T::OrdinaryPrimitive { object, hint } => Self::OrdinaryPrimitive {
                    object: Some(object),
                    hint: Some(hint),
                },
                T::Read {
                    object,
                    key,
                    receiver,
                    resume,
                } => Self::RawReadRequest {
                    selected: None,
                    object: Some(object),
                    key,
                    receiver: Some(receiver),
                    resume: Some(Resume::DatePrototype(resume)),
                },
                T::Call { function, receiver } => Self::RawCall {
                    inputs: Some(crate::engine::vm::call::ordinary::RawCallbackInputs::new(
                        function,
                        receiver,
                        Vec::new(),
                    )),
                    resume: Some(Resume::Identity),
                },
            }
        })
    }
}

impl TryFrom<crate::engine::builtins::ErrorStep> for Step {
    type Error = crate::engine::api::RuntimeError;
    fn try_from(step: crate::engine::builtins::ErrorStep) -> Result<Self, Self::Error> {
        Ok({
            use crate::engine::builtins::ErrorStep as T;
            match step {
                T::Complete(result) => Self::Complete(Some(result)),
                T::Read { mut resume } => {
                    let receiver = resume.take_read_receiver();
                    let key = resume.take_read_key();
                    Self::ReadValue {
                        receiver: Some(receiver),
                        key: Some(key),
                        resume: Some(Resume::Error(resume)),
                    }
                }
                T::String { mut resume } => {
                    let value = resume.take_string_value();
                    Self::String {
                        value: Some(value),
                        resume: Some(Resume::Error(resume)),
                    }
                }
                T::Has { mut resume } => {
                    let object = resume.take_has_object();
                    let key = resume.take_has_key();
                    Self::Has {
                        object: Some(object),
                        key: Some(key),
                        resume: Some(Resume::Error(resume)),
                    }
                }
                T::Aggregate { mut resume } => {
                    let iterable = resume.take_aggregate_iterable();
                    Self::Aggregate {
                        iterable: Some(iterable),
                        resume: Some(Resume::Error(resume)),
                    }
                }
            }
        })
    }
}

impl TryFrom<crate::engine::builtins::SumStep> for Step {
    type Error = crate::engine::api::RuntimeError;
    fn try_from(step: crate::engine::builtins::SumStep) -> Result<Self, Self::Error> {
        Ok({
            use crate::engine::builtins::SumStep as T;
            match step {
                T::Complete(result) => Self::Complete(Some(result)),
                T::Read { mut resume } => {
                    let receiver = resume.take_read_receiver();
                    let key = resume.take_read_key();
                    Self::ReadValue {
                        receiver: Some(receiver),
                        key: Some(key),
                        resume: Some(Resume::Sum(resume)),
                    }
                }
                T::Call { mut resume } => {
                    let callable = resume.take_call_callable();
                    let receiver = resume.take_call_receiver();
                    Self::Call {
                        target: Some(DirectCallTarget::Callable(callable)),
                        receiver: Some(receiver),
                        arguments: Some(Vec::new()),
                        resume: Some(Resume::Sum(resume)),
                    }
                }
                T::Next { mut resume } => {
                    let iterator = resume.take_next_iterator();
                    let next = resume.take_next_next();
                    Self::IteratorNext {
                        iterator: Some(iterator),
                        method: Some(next),
                        resume: Some(Resume::Sum(resume)),
                    }
                }
                T::Close { mut resume } => {
                    let iterator = resume.take_close_iterator();
                    let completion = resume.take_close_completion();
                    Self::IteratorClose {
                        iterator: Some(iterator),
                        completion: Some(completion),
                    }
                }
            }
        })
    }
}

impl TryFrom<crate::engine::builtins::AggregateStep> for Step {
    type Error = crate::engine::api::RuntimeError;
    fn try_from(step: crate::engine::builtins::AggregateStep) -> Result<Self, Self::Error> {
        Ok({
            use crate::engine::builtins::AggregateStep as T;
            match step {
                T::Complete(result) => Self::Complete(Some(result)),
                T::Read {
                    receiver,
                    key,
                    resume,
                } => Self::ReadValue {
                    receiver: Some(receiver),
                    key: Some(key),
                    resume: Some(Resume::Aggregate(resume)),
                },
                T::Call {
                    callable,
                    receiver,
                    resume,
                } => Self::Call {
                    target: Some(DirectCallTarget::Callable(callable)),
                    receiver: Some(receiver),
                    arguments: Some(Vec::new()),
                    resume: Some(Resume::Aggregate(resume)),
                },
                T::Next {
                    iterator,
                    next,
                    resume,
                } => Self::IteratorNext {
                    iterator: Some(iterator),
                    method: Some(next),
                    resume: Some(Resume::Aggregate(resume)),
                },
                T::Close {
                    iterator,
                    completion,
                } => Self::IteratorClose {
                    iterator: Some(iterator),
                    completion: Some(completion),
                },
            }
        })
    }
}
