//! ToNumber uses the shared ToPrimitive protocol and preserves its error realm.
use super::primitive::{PrimitiveResume, PrimitiveStep};
use super::*;
use crate::engine::object::CallableRef;

pub(crate) enum NumberStep {
    Complete(NativeConversion<f64>),
    Read {
        object: ObjectRef,
        key: PropertyKey,
        resume: NumberResume,
    },
    Call {
        callable: CallableRef,
        receiver: Value,
        arguments: Vec<Value>,
        resume: NumberResume,
    },
}
pub(crate) struct NumberResume {
    realm: ContextId,
    primitive: PrimitiveResume,
}
impl NumberStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        value: Value,
    ) -> Result<Self, RuntimeError> {
        from_primitive(
            runtime,
            realm,
            PrimitiveResume::start(runtime, realm, value, ToPrimitiveHint::Number),
        )
    }
}
fn from_primitive(
    runtime: &Runtime,
    realm: ContextId,
    step: PrimitiveStep,
) -> Result<NumberStep, RuntimeError> {
    Ok(match step {
        PrimitiveStep::Complete(Completion::Throw(value)) => {
            NumberStep::Complete(NativeConversion::Throw(value))
        }
        PrimitiveStep::Complete(Completion::Return(value)) => {
            NumberStep::Complete(runtime.number_from_primitive(realm, &value)?)
        }
        PrimitiveStep::Get {
            object,
            key,
            resume,
        } => NumberStep::Read {
            object,
            key,
            resume: NumberResume {
                realm,
                primitive: resume,
            },
        },
        PrimitiveStep::Call {
            callable,
            receiver,
            arguments,
            resume,
        } => NumberStep::Call {
            callable,
            receiver,
            arguments,
            resume: NumberResume {
                realm,
                primitive: resume,
            },
        },
    })
}
impl NumberResume {
    pub(crate) fn resume(
        self,
        runtime: &Runtime,
        completion: Completion,
    ) -> Result<NumberStep, RuntimeError> {
        from_primitive(
            runtime,
            self.realm,
            self.primitive.resume(runtime, completion)?,
        )
    }
}
