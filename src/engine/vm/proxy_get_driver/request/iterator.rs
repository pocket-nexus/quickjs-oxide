//! Mechanical adapters for iterator domain requests.
use super::{Completion, DirectCallTarget, Resume, Step, Value};

impl From<crate::engine::builtins::IteratorCloseStep> for Step {
    fn from(step: crate::engine::builtins::IteratorCloseStep) -> Self {
        use crate::engine::builtins::IteratorCloseStep as T;
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
                resume: Resume::IteratorClose(resume),
            },
            T::Call {
                callable,
                iterator,
                resume,
            } => Self::Call {
                target: DirectCallTarget::Callable(callable),
                receiver: Value::Object(iterator),
                arguments: Vec::new(),
                resume: Resume::IteratorClose(resume),
            },
        }
    }
}

impl From<crate::engine::builtins::IteratorNextStep> for Step {
    fn from(step: crate::engine::builtins::IteratorNextStep) -> Self {
        use crate::engine::builtins::IteratorNextStep as T;
        match step {
            T::Complete(result) => Self::IteratorNextComplete(result),
            T::Read {
                object,
                key,
                resume,
            } => Self::Read {
                receiver: Value::Object(object.clone()),
                object,
                key,
                resume: Resume::IteratorNext(resume),
            },
            T::Call {
                callable,
                iterator,
                resume,
            } => Self::IteratorCall {
                callable,
                iterator,
                resume,
            },
        }
    }
}

impl From<crate::engine::builtins::IteratorConsumeStep> for Step {
    fn from(step: crate::engine::builtins::IteratorConsumeStep) -> Self {
        use crate::engine::builtins::IteratorConsumeStep as T;
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
                resume: Resume::IteratorConsume(resume),
            },
            T::Next {
                iterator,
                method,
                resume,
            } => Self::IteratorNext {
                iterator,
                method,
                resume: Resume::IteratorConsume(resume),
            },
            T::Call {
                callable,
                arguments,
                resume,
            } => Self::Call {
                target: DirectCallTarget::Callable(callable),
                receiver: Value::Undefined,
                arguments: arguments,
                resume: Resume::IteratorConsume(resume),
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

impl From<crate::engine::builtins::IteratorHelperStep> for Step {
    fn from(step: crate::engine::builtins::IteratorHelperStep) -> Self {
        use crate::engine::builtins::IteratorHelperStep as T;
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
                resume: Resume::IteratorHelper(resume),
            },
            T::Next {
                iterator,
                method,
                resume,
            } => Self::IteratorNext {
                iterator,
                method,
                resume: Resume::IteratorHelper(resume),
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
                resume: Resume::IteratorHelper(resume),
            },
            T::Close {
                iterator,
                completion,
                resume,
            } => Self::IteratorCloseWithResume {
                iterator,
                completion,
                resume: Resume::IteratorHelper(resume),
            },
        }
    }
}

impl From<crate::engine::builtins::IteratorCreateStep> for Step {
    fn from(step: crate::engine::builtins::IteratorCreateStep) -> Self {
        use crate::engine::builtins::IteratorCreateStep as T;
        match step {
            T::CloseInvalidCount { iterator, resume } => Self::IteratorCloseWithResume {
                iterator,
                completion: Completion::Throw(Value::Undefined),
                resume: Resume::IteratorInvalidCount(resume),
            },

            T::Complete(result) => Self::Complete(result),
            T::Read {
                object,
                key,
                resume,
            } => Self::Read {
                receiver: Value::Object(object.clone()),
                object,
                key,
                resume: Resume::IteratorCreate(resume),
            },
            T::Number { value, resume } => Self::Number {
                value,
                resume: Resume::IteratorCreate(resume),
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

impl From<crate::engine::builtins::IteratorFromStep> for Step {
    fn from(step: crate::engine::builtins::IteratorFromStep) -> Self {
        use crate::engine::builtins::IteratorFromStep as T;
        match step {
            T::Complete(result) => Self::Complete(result),
            T::Read {
                receiver,
                key,
                resume,
            } => Self::ReadValue {
                receiver,
                key,
                resume: Resume::IteratorFrom(resume),
            },
            T::Call {
                callable,
                receiver,
                resume,
            } => Self::Call {
                target: DirectCallTarget::Callable(callable),
                receiver: receiver,
                arguments: Vec::new(),
                resume: Resume::IteratorFrom(resume),
            },
            T::Instance {
                constructor,
                value,
                resume,
            } => Self::OrdinaryInstance {
                constructor,
                value,
                resume: Resume::IteratorFrom(resume),
            },
        }
    }
}

impl From<crate::engine::builtins::IteratorWrapStep> for Step {
    fn from(step: crate::engine::builtins::IteratorWrapStep) -> Self {
        use crate::engine::builtins::IteratorWrapStep as T;
        match step {
            T::Complete(result) => Self::Complete(result),
            T::Read {
                receiver,
                key,
                resume,
            } => Self::ReadValue {
                receiver,
                key,
                resume: Resume::IteratorWrap(resume),
            },
            T::Call {
                callable,
                receiver,
                resume,
            } => Self::Call {
                target: DirectCallTarget::Callable(callable),
                receiver: receiver,
                arguments: Vec::new(),
                resume: Resume::IteratorWrap(resume),
            },
            T::Next {
                iterator,
                method,
                resume,
            } => Self::IteratorNext {
                iterator,
                method,
                resume: Resume::IteratorWrap(resume),
            },
            T::Parse { result, resume } => Self::ParseIterator {
                result,
                resume: Resume::IteratorWrap(resume),
            },
        }
    }
}

impl From<crate::engine::builtins::IteratorConcatStep> for Step {
    fn from(step: crate::engine::builtins::IteratorConcatStep) -> Self {
        use crate::engine::builtins::IteratorConcatStep as T;
        match step {
            T::Complete(result) => Self::NativeRawComplete(result),
            T::Read {
                object,
                key,
                resume,
            } => Self::Read {
                receiver: Value::Object(object.clone()),
                object,
                key,
                resume: Resume::IteratorConcat(resume),
            },
            T::Call {
                callable,
                receiver,
                resume,
            } => Self::Call {
                target: DirectCallTarget::Callable(callable),
                receiver: receiver,
                arguments: Vec::new(),
                resume: Resume::IteratorConcat(resume),
            },
            T::Next {
                iterator,
                method,
                resume,
            } => Self::IteratorNext {
                iterator,
                method,
                resume: Resume::IteratorConcat(resume),
            },
        }
    }
}

impl From<crate::engine::builtins::IteratorConstructorStep> for Step {
    fn from(step: crate::engine::builtins::IteratorConstructorStep) -> Self {
        use crate::engine::builtins::IteratorConstructorStep as T;
        match step {
            T::Complete(result) => Self::Complete(result),
            T::Prototype { new_target, resume } => Self::ConstructorSource {
                new_target,
                resume: Resume::IteratorConstructor(resume),
            },
        }
    }
}

impl From<crate::engine::builtins::IteratorTagStep> for Step {
    fn from(step: crate::engine::builtins::IteratorTagStep) -> Self {
        use crate::engine::builtins::IteratorTagStep as T;
        match step {
            T::Complete(result) => Self::Complete(result),
            T::Own {
                object,
                key,
                resume,
            } => Self::OwnFlag {
                object,
                key,
                enumerable: false,
                resume: Resume::IteratorTag(resume),
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
                resume: Resume::IteratorTag(resume),
            },
            T::Set {
                object,
                key,
                value,
                resume,
            } => Self::Set {
                receiver: Value::Object(object.clone()),
                object,
                key,
                value,
                resume: Resume::IteratorTag(resume),
            },
        }
    }
}

impl From<crate::engine::builtins::MapCallbackStep> for Step {
    fn from(step: crate::engine::builtins::MapCallbackStep) -> Self {
        use crate::engine::builtins::MapCallbackStep as T;
        match step {
            T::Complete(result) => Self::Complete(result),
            T::Call {
                callable,
                receiver,
                arguments,
                resume,
            } => Self::Call {
                target: DirectCallTarget::Callable(callable),
                receiver,
                arguments,
                resume: Resume::MapCallback(resume),
            },
        }
    }
}

impl From<crate::engine::builtins::SetEachStep> for Step {
    fn from(step: crate::engine::builtins::SetEachStep) -> Self {
        use crate::engine::builtins::SetEachStep as T;
        match step {
            T::Complete(result) => Self::Complete(result),
            T::Call {
                callable,
                receiver,
                arguments,
                resume,
            } => Self::Call {
                target: DirectCallTarget::Callable(callable),
                receiver,
                arguments,
                resume: Resume::SetEach(resume),
            },
        }
    }
}

impl From<crate::engine::builtins::WeakComputedStep> for Step {
    fn from(step: crate::engine::builtins::WeakComputedStep) -> Self {
        use crate::engine::builtins::WeakComputedStep as T;
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
                resume: Resume::WeakComputed(resume),
            },
        }
    }
}

impl From<crate::engine::builtins::SetOperationStep> for Step {
    fn from(step: crate::engine::builtins::SetOperationStep) -> Self {
        use crate::engine::builtins::SetOperationStep as T;
        match step {
            T::Complete(result) => Self::Complete(result),
            T::Read {
                receiver,
                key,
                resume,
            } => Self::ReadValue {
                receiver,
                key,
                resume: Resume::SetOperation(resume),
            },
            T::Number { value, resume } => Self::Number {
                value,
                resume: Resume::SetOperation(resume),
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
                resume: Resume::SetOperation(resume),
            },
            T::Parse { result, resume } => Self::ParseIterator {
                result,
                resume: Resume::SetOperation(resume),
            },
        }
    }
}

impl From<crate::engine::builtins::CollectionStep> for Step {
    fn from(step: crate::engine::builtins::CollectionStep) -> Self {
        use crate::engine::builtins::CollectionStep as T;
        match step {
            T::Complete(result) => Self::Complete(result),
            T::Prototype { new_target, resume } => Self::ConstructorSource {
                new_target,
                resume: Resume::Collection(resume),
            },
            T::Read {
                receiver,
                key,
                resume,
            } => Self::ReadValue {
                receiver,
                key,
                resume: Resume::Collection(resume),
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
                resume: Resume::Collection(resume),
            },
            T::Next {
                iterator,
                method,
                resume,
            } => Self::IteratorNext {
                iterator,
                method,
                resume: Resume::Collection(resume),
            },
            T::Close {
                iterator,
                completion,
                resume,
            } => Self::IteratorCloseWithResume {
                iterator,
                completion,
                resume: Resume::Collection(resume),
            },
        }
    }
}
