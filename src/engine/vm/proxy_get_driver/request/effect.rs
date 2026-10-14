//! The one adapter from the shared builtin effect transport to scheduler steps.
use super::{DirectCallTarget, JsValue, NativeSetWait, Resume, Step};
use crate::engine::api::RuntimeError;
use crate::engine::api::runtime::Runtime;
use crate::engine::builtins::effect::{Continuation, Effect, EffectStep, Reply};
use crate::engine::vm::call::ConstructNewTarget;

impl TryFrom<EffectStep> for Step {
    type Error = RuntimeError;
    fn try_from(step: EffectStep) -> Result<Self, Self::Error> {
        let mut then = match step {
            EffectStep::Complete(result) => return Ok(Self::Complete(Some(result))),
            EffectStep::Close {
                iterator,
                completion,
            } => {
                return Ok(Self::IteratorClose {
                    iterator: Some(iterator),
                    completion: Some(completion),
                });
            }
            EffectStep::Effect(then) => then,
        };
        Ok(match then.take_pending()? {
            Effect::ReadValue { receiver, key } => Self::ReadValue {
                receiver: Some(receiver),
                key: Some(key),
                resume: Some(Resume::Native(then)),
            },
            Effect::Number { value } => Self::Number {
                value: Some(value),
                resume: Some(Resume::Native(then)),
            },
            Effect::Call {
                callable,
                receiver,
                arguments,
            } => Self::Call {
                target: Some(DirectCallTarget::Callable(callable)),
                receiver: Some(receiver),
                arguments: Some(arguments),
                resume: Some(Resume::Native(then)),
            },
            Effect::Construct { target, arguments } => {
                let new_target = target.try_clone()?;
                Self::Construct {
                    new_target: Some(ConstructNewTarget::Validated(new_target)),
                    target: Some(target),
                    arguments: Some(arguments),
                    resume: Some(Resume::Native(then)),
                }
            }
            Effect::ParseIterator { result } => Self::ParseIterator {
                result: Some(result),
                resume: Some(Resume::Native(then)),
            },
            Effect::Define {
                object,
                key,
                descriptor,
            } => Self::Define {
                object: Some(object),
                key: Some(key),
                descriptor: Some((*descriptor).into()),
                resume: Some(Resume::Native(then)),
            },
            Effect::Set { object, key, value } => {
                let handles = object
                    .try_clone()
                    .and_then(|receiver| Ok((receiver, key.try_clone()?)));
                let (receiver, scheduler_key) = match handles {
                    Ok(handles) => handles,
                    Err(error) => {
                        let _ = object.runtime().release_jsvalue(value);
                        return Err(error);
                    }
                };
                Self::Set {
                    receiver: Some(JsValue::Object(receiver.into_handle())),
                    object: Some(object),
                    key: Some(scheduler_key),
                    value: Some(value),
                    resume: Some(Resume::NativeSet(Box::new(NativeSetWait { then, key }))),
                }
            }
        })
    }
}

/// Hand a VM reply to a builtin on the shared transport and schedule its next step.
pub(super) fn reply_native(
    then: Box<dyn Continuation>,
    runtime: &Runtime,
    reply: Reply,
) -> Result<Step, RuntimeError> {
    then.reply(runtime, reply).and_then(Step::try_from)
}
