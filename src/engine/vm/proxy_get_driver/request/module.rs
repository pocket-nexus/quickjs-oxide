//! Mechanical adapters for module domain operations.
use super::{DirectCallTarget, JsValue, Resume, Step};
use crate::engine::modules::import::ImportStep;
impl TryFrom<ImportStep> for Step {
    type Error = crate::engine::api::RuntimeError;
    fn try_from(step: ImportStep) -> Result<Self, Self::Error> {
        Ok({
            match step {
                ImportStep::Complete(result) => Self::Complete(Some(result)),
                ImportStep::String { value, resume } => Self::String {
                    value: Some(value),
                    resume: Some(Resume::Import(resume)),
                },
                ImportStep::Read {
                    object,
                    key,
                    resume,
                } => Self::Read {
                    receiver: Some(JsValue::Object(object.try_clone()?.into_handle())),
                    object: Some(object),
                    key: Some(key),
                    resume: Some(Resume::Import(resume)),
                },
                ImportStep::Keys { object, resume } => Self::Keys {
                    object: Some(object),
                    resume: Some(Resume::Import(resume)),
                },
                ImportStep::Enumerable {
                    object,
                    key,
                    resume,
                } => Self::SnapshotEnumerable {
                    object: Some(object),
                    key: Some(key),
                    resume: Some(Resume::Import(resume)),
                },
                ImportStep::Call {
                    callable,
                    reason,
                    resume,
                } => Self::Call {
                    target: Some(DirectCallTarget::Callable(callable)),
                    receiver: Some(JsValue::Undefined),
                    arguments: Some(vec![reason]),
                    resume: Some(Resume::Import(resume)),
                },
            }
        })
    }
}

impl TryFrom<crate::engine::modules::link::LinkStep> for Step {
    type Error = crate::engine::api::RuntimeError;
    fn try_from(step: crate::engine::modules::link::LinkStep) -> Result<Self, Self::Error> {
        Ok({
            use crate::engine::modules::link::LinkStep;
            match step {
                LinkStep::Complete(result) => Self::Complete(Some(result)),
                LinkStep::Call {
                    realm,
                    callable,
                    resume,
                } => Self::ModuleLink {
                    realm: Some(realm),
                    callable: Some(callable),
                    resume: Some(Resume::ModuleLink(resume)),
                },
            }
        })
    }
}

impl TryFrom<crate::engine::modules::body::BodyStep> for Step {
    type Error = crate::engine::api::RuntimeError;
    fn try_from(step: crate::engine::modules::body::BodyStep) -> Result<Self, Self::Error> {
        Ok({
            use crate::engine::modules::body::BodyStep;
            match step {
                BodyStep::Complete(result) => Self::Complete(Some(result)),
                BodyStep::Call { callable, resume } => Self::Call {
                    target: Some(DirectCallTarget::Callable(callable)),
                    receiver: Some(JsValue::Undefined),
                    arguments: Some(Vec::new()),
                    resume: Some(Resume::ModuleBody(resume)),
                },
                BodyStep::Promise { step, resume } => Self::PromiseOperation {
                    step: Some(step),
                    resume: Some(Resume::ModuleBody(resume)),
                },
            }
        })
    }
}

impl TryFrom<crate::engine::modules::evaluation::EvaluationStep> for Step {
    type Error = crate::engine::api::RuntimeError;
    fn try_from(
        step: crate::engine::modules::evaluation::EvaluationStep,
    ) -> Result<Self, Self::Error> {
        Ok({
            use crate::engine::modules::evaluation::EvaluationStep;
            match step {
                EvaluationStep::Complete(result) => Self::Complete(Some(result)),
                EvaluationStep::Call {
                    callable,
                    value,
                    resume,
                } => Self::Call {
                    target: Some(DirectCallTarget::Callable(callable)),
                    receiver: Some(JsValue::Undefined),
                    arguments: Some(vec![value]),
                    resume: Some(Resume::ModuleEvaluation(resume)),
                },
                EvaluationStep::Body { step, resume } => Self::ModuleBodyOperation {
                    step: Some(step),
                    resume: Some(Resume::ModuleEvaluation(resume)),
                },
            }
        })
    }
}

impl TryFrom<crate::engine::modules::callback::CallbackStep> for Step {
    type Error = crate::engine::api::RuntimeError;
    fn try_from(step: crate::engine::modules::callback::CallbackStep) -> Result<Self, Self::Error> {
        Ok({
            use crate::engine::modules::callback::CallbackStep;
            match step {
                CallbackStep::Complete(result) => Self::Complete(Some(result)),
                CallbackStep::Call {
                    callable,
                    value,
                    resume,
                } => Self::Call {
                    target: Some(DirectCallTarget::Callable(callable)),
                    receiver: Some(JsValue::Undefined),
                    arguments: Some(vec![value]),
                    resume: Some(Resume::ModuleCallback(resume)),
                },
                CallbackStep::Body { step, resume } => Self::ModuleBodyOperation {
                    step: Some(step),
                    resume: Some(Resume::ModuleCallback(resume)),
                },
                CallbackStep::Nested { step, resume } => Self::ModuleCallbackOperation {
                    step: Some(step),
                    resume: Some(Resume::ModuleCallback(resume)),
                },
            }
        })
    }
}
