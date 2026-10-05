//! Mechanical adapters publish only an already selected conversion effect.
use super::{DirectCallTarget, NumberStep, Resume, Runtime, Step};

impl Step {
    pub(in crate::engine::vm::proxy_get_driver) fn from_number(
        runtime: &Runtime,
        step: NumberStep,
    ) -> Result<Self, crate::engine::api::RuntimeError> {
        Ok(match step {
            NumberStep::Complete(result) => Self::NumberComplete(Some(result)),
            NumberStep::Read { mut resume } => {
                let (effect, atom) = resume.take_state_read();
                Self::StateRead {
                    effect: Some(effect),
                    atom,
                    resume: Some(Resume::Number(resume)),
                }
            }
            NumberStep::Call { mut resume } => {
                let callable = resume.take_call_callable(runtime);
                let receiver = resume.take_call_receiver();
                let arguments = resume.take_call_arguments();
                Self::Call {
                    target: Some(DirectCallTarget::Callable(callable)),
                    receiver: Some(receiver),
                    arguments: Some(arguments),
                    resume: Some(Resume::Number(resume)),
                }
            }
        })
    }
    pub(in crate::engine::vm::proxy_get_driver) fn from_primitive(
        runtime: &Runtime,
        step: crate::engine::value::conversion::primitive::PrimitiveStep,
    ) -> Result<Self, crate::engine::api::RuntimeError> {
        use crate::engine::value::conversion::primitive::PrimitiveStep;
        Ok(match step {
            PrimitiveStep::Complete(result) => Self::Complete(Some(result)),
            PrimitiveStep::Get { mut resume } => {
                let (effect, atom) = resume.take_state_read();
                Self::StateRead {
                    effect: Some(effect),
                    atom,
                    resume: Some(Resume::Primitive(resume)),
                }
            }
            PrimitiveStep::Call { mut resume } => {
                let callable = resume.take_callable(runtime);
                let receiver = resume.take_receiver();
                let arguments = resume.take_arguments();
                Self::Call {
                    target: Some(DirectCallTarget::Callable(callable)),
                    receiver: Some(receiver),
                    arguments: Some(arguments),
                    resume: Some(Resume::Primitive(resume)),
                }
            }
        })
    }
}
