//! ToNumber consumes the canonical ToPrimitive phase and preserves its realm.
use super::primitive::{PrimitiveResume, PrimitiveStep};
use super::*;
use crate::engine::atom::Atom;
use crate::engine::object::{CallableRef, StateReadEffect};

pub(crate) enum NumberStep {
    Complete(NativeConversion<f64>),
    Read { resume: NumberResume },
    Call { resume: NumberResume },
}
pub(crate) struct NumberResume(PrimitiveResume);
const _: () = assert!(size_of::<NumberResume>() <= 8);
impl NumberStep {
    pub(crate) fn start_jsvalue(
        runtime: &Runtime,
        realm: ContextId,
        value: JsValue,
    ) -> Result<Self, RuntimeError> {
        from_primitive(
            runtime,
            realm,
            PrimitiveResume::start(runtime, realm, value, ToPrimitiveHint::Number)?,
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
            let converted = runtime.number_from_primitive_jsvalue(realm, &value);
            runtime.release_jsvalue(value)?;
            NumberStep::Complete(converted?)
        }
        PrimitiveStep::Get { resume } => NumberStep::Read {
            resume: NumberResume(resume),
        },
        PrimitiveStep::Call { resume } => NumberStep::Call {
            resume: NumberResume(resume),
        },
    })
}
impl NumberResume {
    pub(crate) fn resume(
        self,
        runtime: &Runtime,
        completion: Completion,
    ) -> Result<NumberStep, RuntimeError> {
        from_primitive(runtime, self.0.realm(), self.0.resume(runtime, completion)?)
    }
    pub(crate) fn take_state_read(&mut self) -> (StateReadEffect, Option<Atom>) {
        self.0.take_state_read()
    }
    pub(crate) fn take_call_callable(&mut self, runtime: &Runtime) -> CallableRef {
        self.0.take_callable(runtime)
    }
    pub(crate) fn take_call_receiver(&mut self) -> JsValue {
        self.0.take_receiver()
    }
    pub(crate) fn take_call_arguments(&mut self) -> Vec<JsValue> {
        self.0.take_arguments()
    }
    pub(crate) fn release_owned(self, runtime: &Runtime) {
        self.0.release_owned(runtime);
    }
}
pub(crate) struct NumberScope<'a> {
    runtime: &'a Runtime,
    resume: Option<NumberResume>,
}
impl<'a> NumberScope<'a> {
    pub(crate) fn new(runtime: &'a Runtime, resume: NumberResume) -> Self {
        Self {
            runtime,
            resume: Some(resume),
        }
    }
    pub(crate) fn take(&mut self) -> NumberResume {
        self.resume.take().expect("number scope owner")
    }
}
impl std::ops::Deref for NumberScope<'_> {
    type Target = NumberResume;
    fn deref(&self) -> &Self::Target {
        self.resume.as_ref().expect("number scope owner")
    }
}
impl std::ops::DerefMut for NumberScope<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.resume.as_mut().expect("number scope owner")
    }
}
impl Drop for NumberScope<'_> {
    fn drop(&mut self) {
        if let Some(resume) = self.resume.take() {
            resume.release_owned(self.runtime);
        }
    }
}
const _: () = assert!(size_of::<NumberStep>() <= 64);
