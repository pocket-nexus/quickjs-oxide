//! Mechanical adapters for vm domain requests.
use super::{Completion, Resume, Step, Value};

impl From<crate::engine::vm::environment_bindings::operation::EnvironmentStep> for Step {
    fn from(step: crate::engine::vm::environment_bindings::operation::EnvironmentStep) -> Self {
        use crate::engine::vm::environment_bindings::operation::EnvironmentStep as T;
        match step {
            T::Complete(result) => Self::Complete(result),
            T::Read {
                object,
                key,
                receiver,
                resume,
            } => Self::Read {
                receiver,
                object,
                key,
                resume: Resume::Environment(resume),
            },
            T::Has {
                object,
                key,
                resume,
            } => Self::Has {
                object,
                key,
                resume: Resume::Environment(resume),
            },
            T::Delete {
                object,
                key,
                resume,
            } => Self::Delete {
                object,
                key,
                resume: Resume::Environment(resume),
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
                resume: Resume::Environment(resume),
            },
        }
    }
}

impl From<crate::engine::vm::numeric::operation::NumericStep> for Step {
    fn from(step: crate::engine::vm::numeric::operation::NumericStep) -> Self {
        use crate::engine::vm::numeric::operation::NumericStep as T;
        match step {
            T::Complete { value, previous } => Self::NumericComplete { value, previous },
            T::Throw(value) => Self::Complete(Completion::Throw(value)),
            T::Primitive {
                value,
                hint,
                resume,
            } => Self::Primitive {
                value,
                hint,
                resume: Resume::VmNumeric(resume),
            },
            T::HtmlDda { value, resume } => Self::NumericHtmlDda { value, resume },
        }
    }
}

impl From<crate::engine::vm::for_in::operation::ForInStep> for Step {
    fn from(step: crate::engine::vm::for_in::operation::ForInStep) -> Self {
        use crate::engine::vm::for_in::operation::ForInStep as T;
        match step {
            T::Complete { value, done } => Self::ForInComplete { value, done },
            T::Throw(value) => Self::Complete(Completion::Throw(value)),
            T::Keys { object, resume } => Self::Keys {
                object,
                resume: Resume::ForIn(resume),
            },
            T::Enumerable {
                object,
                key,
                resume,
            } => Self::SnapshotEnumerable {
                object,
                key,
                resume: Resume::ForIn(resume),
            },
            T::Own {
                object,
                key,
                resume,
            } => Self::OwnFlag {
                object,
                key,
                enumerable: false,
                resume: Resume::ForIn(resume),
            },
            T::Prototype { object, resume } => Self::GetPrototype {
                object,
                resume: Resume::ForIn(resume),
            },
        }
    }
}
