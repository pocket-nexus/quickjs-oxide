//! Element conversion owns ToPrimitive; no buffer credential crosses a callback.
use super::{typed_array_encode_bigint, typed_array_encode_number};
use crate::engine::atom::Atom;
use crate::engine::object::{CallableRef, StateReadEffect};
use crate::engine::value::conversion::primitive::{PrimitiveResume, PrimitiveStep};
use crate::engine::vm::ToPrimitiveHint;
use crate::engine::{
    api::{runtime::Runtime, runtime_error::RuntimeError},
    builtins::native::TypedArrayElementKind,
    heap::ContextId,
    value::{JsValue, conversion::NativeConversion},
    vm::Completion,
};

pub(crate) enum ElementStep {
    Complete(NativeConversion<[u8; 8]>),
    Read { resume: ElementResume },
    Call { resume: ElementResume },
}
pub(crate) struct ElementResume(Box<ElementResumeState>);
const _: () = assert!(size_of::<ElementResume>() <= 8);
struct ElementResumeState {
    realm: ContextId,
    element: TypedArrayElementKind,
    primitive: PrimitiveResume,
}
impl ElementStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        element: TypedArrayElementKind,
        value: JsValue,
    ) -> Result<Self, RuntimeError> {
        from_primitive(
            runtime,
            realm,
            element,
            PrimitiveResume::start(runtime, realm, value, ToPrimitiveHint::Number)?,
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
                Self::Read { resume } => {
                    let mut resume = ElementScope::new(runtime, resume);
                    let (effect, atom) = resume.take_state_read();
                    let completion = runtime.finish_primitive_read(realm, effect, atom)?;
                    resume.take().resume(runtime, completion)?
                }
                Self::Call { resume } => {
                    let mut resume = ElementScope::new(runtime, resume);
                    let callable = resume.take_call_callable(runtime);
                    let receiver = resume.take_call_receiver();
                    let arguments = resume.take_call_arguments();
                    let completion =
                        runtime.call_internal_jsvalue(realm, &callable, receiver, arguments)?;
                    resume.take().resume(runtime, completion)?
                }
            };
        }
    }
}
/// Shared primitive conversion after ToPrimitive has completed.
pub(super) fn encode_primitive(
    runtime: &Runtime,
    realm: ContextId,
    element: TypedArrayElementKind,
    value: JsValue,
) -> Result<NativeConversion<[u8; 8]>, RuntimeError> {
    let result = (|| {
        Ok(if element.is_bigint() {
            match runtime.bigint_from_primitive_jsvalue(realm, &value)? {
                NativeConversion::Value(bigint) => {
                    NativeConversion::Value(typed_array_encode_bigint(&bigint)?)
                }
                NativeConversion::Throw(value) => NativeConversion::Throw(value),
            }
        } else {
            match runtime.number_from_primitive_jsvalue(realm, &value)? {
                NativeConversion::Value(number) => {
                    NativeConversion::Value(typed_array_encode_number(element, number))
                }
                NativeConversion::Throw(value) => NativeConversion::Throw(value),
            }
        })
    })();
    runtime.release_jsvalue(value)?;
    result
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
            let bytes = encode_primitive(runtime, realm, element, value)?;
            ElementStep::Complete(bytes)
        }
        PrimitiveStep::Get { resume } => ElementStep::Read {
            resume: ElementResume(Box::new(ElementResumeState {
                realm,
                element,
                primitive: resume,
            })),
        },
        PrimitiveStep::Call { resume } => ElementStep::Call {
            resume: ElementResume(Box::new(ElementResumeState {
                realm,
                element,
                primitive: resume,
            })),
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
            self.0.realm,
            self.0.element,
            self.0.primitive.resume(runtime, completion)?,
        )
    }
    pub(crate) fn take_state_read(&mut self) -> (StateReadEffect, Atom) {
        self.0.primitive.take_state_read()
    }
    pub(crate) fn take_call_callable(&mut self, runtime: &Runtime) -> CallableRef {
        self.0.primitive.take_callable(runtime)
    }
    pub(crate) fn take_call_receiver(&mut self) -> JsValue {
        self.0.primitive.take_receiver()
    }
    pub(crate) fn take_call_arguments(&mut self) -> Vec<JsValue> {
        self.0.primitive.take_arguments()
    }
    pub(crate) fn release_owned(self, runtime: &Runtime) {
        self.0.primitive.release_owned(runtime);
    }
}
struct ElementScope<'a> {
    runtime: &'a Runtime,
    resume: Option<ElementResume>,
}
impl<'a> ElementScope<'a> {
    fn new(runtime: &'a Runtime, resume: ElementResume) -> Self {
        Self {
            runtime,
            resume: Some(resume),
        }
    }
    fn take(&mut self) -> ElementResume {
        self.resume.take().expect("element scope owner")
    }
}
impl std::ops::Deref for ElementScope<'_> {
    type Target = ElementResume;
    fn deref(&self) -> &Self::Target {
        self.resume.as_ref().expect("element scope owner")
    }
}
impl std::ops::DerefMut for ElementScope<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.resume.as_mut().expect("element scope owner")
    }
}
impl Drop for ElementScope<'_> {
    fn drop(&mut self) {
        if let Some(resume) = self.resume.take() {
            resume.release_owned(self.runtime);
        }
    }
}
const _: () = assert!(size_of::<ElementStep>() <= 64);
