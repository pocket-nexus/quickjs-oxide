//! Mechanical adapters for function domain requests.
use super::{DirectCallTarget, Resume, Step, Value};

impl From<crate::engine::builtins::ArgumentsStep> for Step {
    fn from(step: crate::engine::builtins::ArgumentsStep) -> Self {
        use crate::engine::builtins::ArgumentsStep;
        match step {
            ArgumentsStep::Complete(result) => Self::ArgumentsComplete(result),
            ArgumentsStep::Read {
                object,
                key,
                resume,
            } => Self::Read {
                receiver: Value::Object(object.clone()),
                object,
                key,
                resume: Resume::Arguments(resume),
            },
            ArgumentsStep::Number { value, resume } => Self::Number {
                value,
                resume: Resume::Arguments(resume),
            },
        }
    }
}

impl From<crate::engine::builtins::InvokeStep> for Step {
    fn from(step: crate::engine::builtins::InvokeStep) -> Self {
        use crate::engine::builtins::InvokeStep;
        match step {
            InvokeStep::Complete(result) => Self::Complete(result),
            InvokeStep::Construct {
                target,
                new_target,
                arguments,
            } => Self::Construct {
                target,
                new_target,
                arguments,
                resume: Resume::Identity,
            },
            InvokeStep::Arguments { value, resume } => Self::Arguments {
                value,
                resume: Resume::Invoke(resume),
            },
            InvokeStep::Call {
                target,
                receiver,
                arguments,
            } => Self::Call {
                target,
                receiver,
                arguments,
                resume: Resume::Identity,
            },
        }
    }
}

impl From<crate::engine::builtins::InstanceStep> for Step {
    fn from(step: crate::engine::builtins::InstanceStep) -> Self {
        use crate::engine::builtins::InstanceStep as T;
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
                resume: Resume::Instance(resume),
            },
            T::Call {
                callable,
                receiver,
                arguments,
                resume,
                ..
            } => Self::Call {
                target: DirectCallTarget::Callable(callable),
                receiver,
                arguments,
                resume: Resume::Instance(resume),
            },
            T::Prototype { object, resume } => Self::GetPrototype {
                object,
                resume: Resume::Instance(resume),
            },
        }
    }
}

impl From<crate::engine::vm::call::prototype::ProtoSourceStep> for Step {
    fn from(step: crate::engine::vm::call::prototype::ProtoSourceStep) -> Self {
        use crate::engine::vm::call::prototype::ProtoSourceStep as T;
        match step {
            T::Complete(result) => Self::ConstructorSourceComplete(result),
            T::ReadValue {
                receiver,
                key,
                resume,
            } => Self::ReadValue {
                receiver,
                key,
                resume: Resume::ConstructorSource(resume),
            },
        }
    }
}

impl From<crate::engine::builtins::BindStep> for Step {
    fn from(step: crate::engine::builtins::BindStep) -> Self {
        use crate::engine::builtins::BindStep as T;
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
                resume: Resume::Bind(resume),
            },
            T::Own {
                object,
                key,
                resume,
            } => Self::OwnFlag {
                object,
                key,
                enumerable: false,
                resume: Resume::Bind(resume),
            },
        }
    }
}

impl From<crate::engine::builtins::FunctionTextStep> for Step {
    fn from(step: crate::engine::builtins::FunctionTextStep) -> Self {
        use crate::engine::builtins::FunctionTextStep as T;
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
                resume: Resume::FunctionText(resume),
            },
            T::String { value, resume } => Self::String {
                value,
                resume: Resume::FunctionText(resume),
            },
        }
    }
}

impl From<crate::engine::builtins::DynamicFunctionStep> for Step {
    fn from(step: crate::engine::builtins::DynamicFunctionStep) -> Self {
        use crate::engine::builtins::DynamicFunctionStep as T;
        match step {
            T::Complete(result) => Self::Complete(result),
            T::Read {
                receiver,
                key,
                resume,
            } => Self::ReadValue {
                receiver,
                key,
                resume: Resume::DynamicFunction(resume),
            },
            T::String { value, resume } => Self::String {
                value,
                resume: Resume::DynamicFunction(resume),
            },
            T::Eval { source, resume } => Self::IndirectEval {
                source,
                resume: Resume::DynamicFunction(resume),
            },
        }
    }
}

impl From<crate::engine::builtins::WeakConstructorStep> for Step {
    fn from(step: crate::engine::builtins::WeakConstructorStep) -> Self {
        use crate::engine::builtins::WeakConstructorStep as T;
        match step {
            T::Complete(result) => Self::Complete(result),
            T::Prototype { new_target, resume } => Self::ConstructorSource {
                new_target,
                resume: Resume::WeakConstructor(resume),
            },
        }
    }
}
