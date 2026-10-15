//! Mechanical adapters for module domain operations.
use super::{DirectCallTarget, JsValue, Resume, Step};
use crate::engine::modules::import::ImportStep;
step_from! {
    ImportStep => Import;
    String { value } => String;
    Read { object, key } => read;
    Keys { object } => Keys;
    Enumerable { object, key } => SnapshotEnumerable;
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
        T::Call {
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
}

step_from! {
    crate::engine::modules::link::LinkStep => ModuleLink;
    Call { realm, callable } => ModuleLink;
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
    }
}

step_from! {
    crate::engine::modules::body::BodyStep => ModuleBody;
    Promise { step } => PromiseOperation;
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
        T::Call { callable, resume } => Self::Call {
            target: Some(DirectCallTarget::Callable(callable)),
            receiver: Some(JsValue::Undefined),
            arguments: Some(Vec::new()),
            resume: Some(Resume::ModuleBody(resume)),
        },
    }
}

step_from! {
    crate::engine::modules::evaluation::EvaluationStep => ModuleEvaluation;
    Body { step } => ModuleBodyOperation;
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
        T::Call {
            callable,
            value,
            resume,
        } => Self::Call {
            target: Some(DirectCallTarget::Callable(callable)),
            receiver: Some(JsValue::Undefined),
            arguments: Some(vec![value]),
            resume: Some(Resume::ModuleEvaluation(resume)),
        },
    }
}

step_from! {
    crate::engine::modules::callback::CallbackStep => ModuleCallback;
    Body { step } => ModuleBodyOperation;
    Nested { step } => ModuleCallbackOperation;
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
        T::Call {
            callable,
            value,
            resume,
        } => Self::Call {
            target: Some(DirectCallTarget::Callable(callable)),
            receiver: Some(JsValue::Undefined),
            arguments: Some(vec![value]),
            resume: Some(Resume::ModuleCallback(resume)),
        },
    }
}
