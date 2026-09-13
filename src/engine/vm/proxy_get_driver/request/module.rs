//! Mechanical adapters for module domain operations.
use super::{DirectCallTarget, Resume, Step, Value};
use crate::engine::modules::import::ImportStep;
impl From<ImportStep> for Step {
    fn from(step: ImportStep) -> Self {
        match step {
            ImportStep::Complete(result) => Self::Complete(result),
            ImportStep::String { value, resume } => Self::String {
                value,
                resume: Resume::Import(resume),
            },
            ImportStep::Read {
                object,
                key,
                resume,
            } => Self::Read {
                receiver: Value::Object(object.clone()),
                object,
                key,
                resume: Resume::Import(resume),
            },
            ImportStep::Keys { object, resume } => Self::Keys {
                object,
                resume: Resume::Import(resume),
            },
            ImportStep::Enumerable {
                object,
                key,
                resume,
            } => Self::SnapshotEnumerable {
                object,
                key,
                resume: Resume::Import(resume),
            },
            ImportStep::Call {
                callable,
                reason,
                resume,
            } => Self::Call {
                target: DirectCallTarget::Callable(callable),
                receiver: Value::Undefined,
                arguments: vec![reason],
                resume: Resume::Import(resume),
            },
        }
    }
}

impl From<crate::engine::modules::link::LinkStep> for Step {
    fn from(step: crate::engine::modules::link::LinkStep) -> Self {
        use crate::engine::modules::link::LinkStep;
        match step {
            LinkStep::Complete(result) => Self::Complete(result),
            LinkStep::Call {
                realm,
                callable,
                resume,
            } => Self::ModuleLink {
                realm,
                callable,
                resume: Resume::ModuleLink(resume),
            },
        }
    }
}

impl From<crate::engine::modules::body::BodyStep> for Step {
    fn from(step: crate::engine::modules::body::BodyStep) -> Self {
        use crate::engine::modules::body::BodyStep;
        match step {
            BodyStep::Complete(result) => Self::Complete(result),
            BodyStep::Call { callable, resume } => Self::Call {
                target: DirectCallTarget::Callable(callable),
                receiver: Value::Undefined,
                arguments: Vec::new(),
                resume: Resume::ModuleBody(resume),
            },
            BodyStep::Promise { step, resume } => Self::PromiseOperation {
                step,
                resume: Resume::ModuleBody(resume),
            },
        }
    }
}

impl From<crate::engine::modules::evaluation::EvaluationStep> for Step {
    fn from(step: crate::engine::modules::evaluation::EvaluationStep) -> Self {
        use crate::engine::modules::evaluation::EvaluationStep;
        match step {
            EvaluationStep::Complete(result) => Self::Complete(result),
            EvaluationStep::Call {
                callable,
                value,
                resume,
            } => Self::Call {
                target: DirectCallTarget::Callable(callable),
                receiver: Value::Undefined,
                arguments: vec![value],
                resume: Resume::ModuleEvaluation(resume),
            },
            EvaluationStep::Body { step, resume } => Self::ModuleBodyOperation {
                step,
                resume: Resume::ModuleEvaluation(resume),
            },
        }
    }
}

impl From<crate::engine::modules::callback::CallbackStep> for Step {
    fn from(step: crate::engine::modules::callback::CallbackStep) -> Self {
        use crate::engine::modules::callback::CallbackStep;
        match step {
            CallbackStep::Complete(result) => Self::Complete(result),
            CallbackStep::Call {
                callable,
                value,
                resume,
            } => Self::Call {
                target: DirectCallTarget::Callable(callable),
                receiver: Value::Undefined,
                arguments: vec![value],
                resume: Resume::ModuleCallback(resume),
            },
            CallbackStep::Body { step, resume } => Self::ModuleBodyOperation {
                step,
                resume: Resume::ModuleCallback(resume),
            },
            CallbackStep::Nested { step, resume } => Self::ModuleCallbackOperation {
                step,
                resume: Resume::ModuleCallback(resume),
            },
        }
    }
}
