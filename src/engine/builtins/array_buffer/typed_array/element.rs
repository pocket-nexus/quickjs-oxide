//! Element conversion owns ToPrimitive; no buffer credential crosses a callback.
use super::*;
use crate::engine::object::CallableRef;
use crate::engine::value::conversion::primitive::{PrimitiveResume, PrimitiveStep};
use crate::engine::vm::ToPrimitiveHint;

pub(crate) enum ElementStep {
    Complete(NativeConversion<[u8; 8]>),
    Read {
        object: ObjectRef,
        key: PropertyKey,
        resume: ElementResume,
    },
    Call {
        callable: CallableRef,
        receiver: Value,
        arguments: Vec<Value>,
        resume: ElementResume,
    },
}
pub(crate) struct ElementResume {
    realm: ContextId,
    element: TypedArrayElementKind,
    primitive: PrimitiveResume,
}
impl ElementStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        element: TypedArrayElementKind,
        value: Value,
    ) -> Result<Self, RuntimeError> {
        from_primitive(
            runtime,
            realm,
            element,
            PrimitiveResume::start(runtime, realm, value, ToPrimitiveHint::Number),
        )
    }
    pub(crate) fn finish_sync(
        mut self,
        runtime: &Runtime,
        realm: ContextId,
    ) -> Result<NativeConversion<[u8; 8]>, RuntimeError> {
        loop {
            self = match self {
                Self::Complete(result) => return Ok(result),
                Self::Read {
                    object,
                    key,
                    resume,
                } => resume.resume(
                    runtime,
                    runtime.get_property_in_realm(realm, &object, &key)?,
                )?,
                Self::Call {
                    callable,
                    receiver,
                    arguments,
                    resume,
                } => resume.resume(
                    runtime,
                    runtime.call_internal(realm, &callable, receiver, &arguments)?,
                )?,
            };
        }
    }
}
fn from_primitive(
    runtime: &Runtime,
    realm: ContextId,
    element: TypedArrayElementKind,
    step: PrimitiveStep,
) -> Result<ElementStep, RuntimeError> {
    Ok(match step {
        PrimitiveStep::Complete(Completion::Throw(value)) => {
            ElementStep::Complete(NativeConversion::Throw(value))
        }
        PrimitiveStep::Complete(Completion::Return(value)) => {
            let bytes = if element.is_bigint() {
                match runtime.bigint_from_primitive(realm, value)? {
                    NativeConversion::Value(bigint) => {
                        NativeConversion::Value(typed_array_encode_bigint(&bigint)?)
                    }
                    NativeConversion::Throw(value) => NativeConversion::Throw(value),
                }
            } else {
                match runtime.number_from_primitive(realm, &value)? {
                    NativeConversion::Value(number) => {
                        NativeConversion::Value(typed_array_encode_number(element, number))
                    }
                    NativeConversion::Throw(value) => NativeConversion::Throw(value),
                }
            };
            ElementStep::Complete(bytes)
        }
        PrimitiveStep::Get {
            object,
            key,
            resume,
        } => ElementStep::Read {
            object,
            key,
            resume: ElementResume {
                realm,
                element,
                primitive: resume,
            },
        },
        PrimitiveStep::Call {
            callable,
            receiver,
            arguments,
            resume,
        } => ElementStep::Call {
            callable,
            receiver,
            arguments,
            resume: ElementResume {
                realm,
                element,
                primitive: resume,
            },
        },
    })
}
impl ElementResume {
    pub(crate) fn resume(
        self,
        runtime: &Runtime,
        completion: Completion,
    ) -> Result<ElementStep, RuntimeError> {
        from_primitive(
            runtime,
            self.realm,
            self.element,
            self.primitive.resume(runtime, completion)?,
        )
    }
}
