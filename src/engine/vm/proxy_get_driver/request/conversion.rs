//! Mechanical adapters for conversion domain requests.
use super::{DirectCallTarget, NumberStep, Resume, Step, Value};

impl From<NumberStep> for Step {
    fn from(step: NumberStep) -> Self {
        match step {
            NumberStep::Complete(result) => Self::NumberComplete(result),
            NumberStep::Read {
                object,
                key,
                resume,
            } => Self::Read {
                receiver: Value::Object(object.clone()),
                object,
                key,
                resume: Resume::Number(resume),
            },
            NumberStep::Call {
                callable,
                receiver,
                arguments,
                resume,
            } => Self::Call {
                target: DirectCallTarget::Callable(callable),
                receiver,
                arguments,
                resume: Resume::Number(resume),
            },
        }
    }
}

impl From<crate::engine::value::conversion::primitive::PrimitiveStep> for Step {
    fn from(step: crate::engine::value::conversion::primitive::PrimitiveStep) -> Self {
        use crate::engine::value::conversion::primitive::PrimitiveStep;
        match step {
            PrimitiveStep::Complete(result) => Self::Complete(result),
            PrimitiveStep::Get {
                object,
                key,
                resume,
            } => Self::Read {
                receiver: Value::Object(object.clone()),
                object,
                key,
                resume: Resume::Primitive(resume),
            },
            PrimitiveStep::Call {
                callable,
                receiver,
                arguments,
                resume,
            } => Self::Call {
                target: DirectCallTarget::Callable(callable),
                receiver,
                arguments,
                resume: Resume::Primitive(resume),
            },
        }
    }
}
