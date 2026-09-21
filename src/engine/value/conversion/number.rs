//! ToNumber uses the shared ToPrimitive protocol and preserves its error realm.
use super::primitive::{PrimitiveResume, PrimitiveStep};
use super::*;
use crate::engine::object::CallableRef;
use crate::engine::value::JsValue;

pub(crate) enum NumberStep {
    Complete(NativeConversion<f64>),
    Read { resume: NumberResume },
    Call { resume: NumberResume },
}
pub(crate) struct NumberResume(Box<NumberResumeState>);
impl std::ops::Deref for NumberResume {
    type Target = NumberResumeState;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for NumberResume {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
const _: () = assert!(std::mem::size_of::<NumberResume>() <= 8);
pub(crate) struct NumberResumeState {
    pending_effect: NumberStepPending,
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
            PrimitiveResume::start(
                runtime,
                realm,
                runtime.unroot_value(&value)?,
                ToPrimitiveHint::Number,
            ),
        )
    }
}
fn from_primitive(
    runtime: &Runtime,
    realm: ContextId,
    step: PrimitiveStep,
) -> Result<NumberStep, RuntimeError> {
    Ok(match step {
        PrimitiveStep::Complete(Completion::Throw(value)) => NumberStep::Complete(
            NativeConversion::Throw(runtime.root_and_release_jsvalue(value)?),
        ),
        PrimitiveStep::Complete(Completion::Return(value)) => {
            let value = runtime.root_and_release_jsvalue(value)?;
            NumberStep::Complete(runtime.number_from_primitive(realm, &value)?)
        }
        PrimitiveStep::Get { mut resume } => {
            let (object, key) = resume.take_get();
            NumberStep::request_read(
                object,
                key,
                NumberResume(Box::new(NumberResumeState {
                    pending_effect: NumberStepPending::new(runtime),
                    realm,
                    primitive: resume,
                })),
            )
        }
        PrimitiveStep::Call { mut resume } => {
            let callable = resume.take_callable();
            let receiver = resume.take_receiver();
            let arguments = resume.take_arguments();
            NumberStep::request_call(
                callable,
                receiver,
                arguments,
                NumberResume(Box::new(NumberResumeState {
                    pending_effect: NumberStepPending::new(runtime),
                    realm,
                    primitive: resume,
                })),
            )
        }
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
            self.0.realm,
            self.0.primitive.resume(runtime, completion)?,
        )
    }
}

struct NumberStepPending {
    runtime: Runtime,
    read_object: Option<ObjectRef>,
    read_key: Option<PropertyKey>,
    call_callable: Option<CallableRef>,
    call_receiver: Option<JsValue>,
    call_arguments: Option<Vec<JsValue>>,
}
impl NumberStepPending {
    fn new(runtime: &Runtime) -> Self {
        Self {
            runtime: runtime.clone(),
            read_object: None,
            read_key: None,
            call_callable: None,
            call_receiver: None,
            call_arguments: None,
        }
    }

    /// Release every edge that was not consumed by a completed step.
    fn release_owned(&mut self) {
        if let Some(receiver) = self.call_receiver.take() {
            let _ = self.runtime.release_jsvalue(receiver);
        }
        for argument in self.call_arguments.take().into_iter().flatten() {
            let _ = self.runtime.release_jsvalue(argument);
        }
    }
}
impl Drop for NumberStepPending {
    fn drop(&mut self) {
        self.release_owned();
    }
}
impl NumberStep {
    pub(crate) fn request_read(
        object: ObjectRef,
        key: PropertyKey,
        mut resume: NumberResume,
    ) -> Self {
        resume.0.pending_effect.read_object = Some(object);
        resume.0.pending_effect.read_key = Some(key);
        Self::Read { resume }
    }
    pub(crate) fn request_call(
        callable: CallableRef,
        receiver: JsValue,
        arguments: Vec<JsValue>,
        mut resume: NumberResume,
    ) -> Self {
        resume.0.pending_effect.call_callable = Some(callable);
        resume.0.pending_effect.call_receiver = Some(receiver);
        resume.0.pending_effect.call_arguments = Some(arguments);
        Self::Call { resume }
    }
}
impl NumberResume {
    pub(crate) fn take_read_object(&mut self) -> ObjectRef {
        self.0
            .pending_effect
            .read_object
            .take()
            .expect("NumberStep Read object")
    }
    pub(crate) fn take_read_key(&mut self) -> PropertyKey {
        self.0
            .pending_effect
            .read_key
            .take()
            .expect("NumberStep Read key")
    }
    pub(crate) fn take_call_callable(&mut self) -> CallableRef {
        self.0
            .pending_effect
            .call_callable
            .take()
            .expect("NumberStep Call callable")
    }
    pub(crate) fn take_call_receiver(&mut self) -> JsValue {
        self.0
            .pending_effect
            .call_receiver
            .take()
            .expect("NumberStep Call receiver")
    }
    pub(crate) fn take_call_arguments(&mut self) -> Vec<JsValue> {
        self.0
            .pending_effect
            .call_arguments
            .take()
            .expect("NumberStep Call arguments")
    }
}
const _: () = assert!(std::mem::size_of::<NumberStep>() <= 64);

// S11 all-domain protocol bound; inline completion stays allocation-free.
const _: () = assert!(std::mem::size_of::<NumberStep>() <= 64);
