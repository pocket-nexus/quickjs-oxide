//! Mechanical adapters for vm domain requests.
use super::{Completion, JsValue, Resume, Step};

step_from! {
    crate::engine::vm::environment_bindings::operation::EnvironmentStep => Environment;
    Read => Read { object: take_read_object, key: take_read_key, receiver: take_read_receiver };
    Has => Has { object: take_has_object, key: take_has_key };
    Delete => Delete { object: take_delete_object, key: take_delete_key };
    Set => set(take_set_object, take_set_key, take_set_value);
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
    }
}

step_from! {
    crate::engine::vm::numeric::operation::NumericStep => VmNumeric;
    Primitive { value, hint } => Primitive;
    raw {
        T::Complete { value, previous } => Self::NumericComplete {
            value: Some(value),
            previous: Some(previous),
        },
        T::Throw(value) => Self::Complete(Some(Completion::Throw(value))),
    }
}

step_from! {
    crate::engine::vm::for_in::operation::ForInStep => ForIn;
    Keys { object } => Keys;
    Enumerable { object, key } => SnapshotEnumerable;
    Own { object, key } => OwnFlag { enumerable: false };
    Prototype { object } => GetPrototype;
    raw {
        T::Complete { value, done } => Self::ForInComplete {
            value: Some(value),
            done: Some(done),
        },
        T::Throw(value) => Self::Complete(Some(Completion::Throw(value))),
    }
}

step_from! {
    crate::engine::vm::suspend::creation::CreationStep => GeneratorPrototype;
    raw {
        T::Complete(completion) => {
            Self::Complete(Some(completion))
        }
        T::Read {
            object,
            key,
            resume,
        } => Self::Read {
            receiver: Some(crate::engine::value::JsValue::Object(
                object.try_clone()?.into_handle(),
            )),
            object: Some(object),
            key: Some(key),
            resume: Some(super::Resume::GeneratorPrototype(resume)),
        },
    }
}

impl From<crate::engine::vm::async_function::AsyncStep> for super::Step {
    fn from(step: crate::engine::vm::async_function::AsyncStep) -> Self {
        use crate::engine::vm::async_function::AsyncStep as S;
        match step {
            S::Complete(completion) => Self::Complete(Some(completion)),
            S::Run { mut resume } => {
                let activation = resume.take_run_activation();
                let input = resume.take_run_input();
                Self::ResumeFrame {
                    activation: Some(activation),
                    input: Some(input),
                    resume: Some(super::Resume::Async(resume)),
                }
            }
            S::Resolve { mut resume } => {
                let value = resume.take_resolve_value();
                let realm = resume.take_resolve_realm();
                Self::IntrinsicPromiseResolve {
                    value: Some(value),
                    realm: Some(realm),
                    resume: Some(super::Resume::Async(resume)),
                }
            }
            S::Call { mut resume } => {
                let callable = resume.take_call_callable();
                let value = resume.take_call_value();
                Self::Call {
                    target: Some(super::DirectCallTarget::Callable(callable)),
                    receiver: Some(crate::engine::value::JsValue::Undefined),
                    arguments: Some(vec![value]),
                    resume: Some(super::Resume::Async(resume)),
                }
            }
        }
    }
}

impl From<crate::engine::vm::async_generator::AsyncGeneratorStep> for super::Step {
    fn from(step: crate::engine::vm::async_generator::AsyncGeneratorStep) -> Self {
        use crate::engine::vm::async_generator::AsyncGeneratorStep as S;
        match step {
            S::Complete(completion) => Self::Complete(Some(completion)),
            S::Run { mut resume } => {
                let activation = resume.take_run_activation();
                let input = resume.take_run_input();
                Self::ResumeFrame {
                    activation: Some(activation),
                    input: Some(input),
                    resume: Some(super::Resume::AsyncGenerator(resume)),
                }
            }
            S::Call { mut resume } => {
                let callable = resume.take_call_callable();
                let value = resume.take_call_value();
                Self::Call {
                    target: Some(super::DirectCallTarget::Callable(callable)),
                    receiver: Some(JsValue::Undefined),
                    arguments: Some(vec![value]),
                    resume: Some(super::Resume::AsyncGenerator(resume)),
                }
            }
            S::Resolve { mut resume } => {
                let value = resume.take_resolve_value();
                let realm = resume.take_resolve_realm();
                Self::IntrinsicPromiseResolve {
                    value: Some(value),
                    realm: Some(realm),
                    resume: Some(super::Resume::AsyncGenerator(resume)),
                }
            }
        }
    }
}
