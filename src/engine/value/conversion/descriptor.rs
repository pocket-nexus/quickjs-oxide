//! Pinned js_obj_to_desc order, with each Has/Get exposed as a resumable step.
use crate::engine::api::{error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError};
use crate::engine::heap::ContextId;
use crate::engine::object::{
    AccessorValue, DescriptorField, ObjectRef, OwnedPropertyDescriptor, PropertyKey,
};
use crate::engine::value::{JsValue, Value, conversion::NativeConversion};
use crate::engine::vm::Completion;

pub(crate) enum DescriptorStep {
    Complete(DescriptorResume),
    Throw(Value),
    Has { resume: DescriptorResume },
    Read { resume: DescriptorResume },
}

pub(crate) struct DescriptorResume(Box<DescriptorResumeState>);
impl std::ops::Deref for DescriptorResume {
    type Target = DescriptorResumeState;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for DescriptorResume {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
const _: () = assert!(std::mem::size_of::<DescriptorResume>() <= 8);
pub(crate) struct DescriptorResumeState {
    pending_effect: DescriptorStepPending,
    state: State,
    phase: Phase,
}
enum Phase {
    Has(PropertyKey),
    Read,
}
struct State {
    realm: ContextId,
    object: ObjectRef,
    descriptor: OwnedPropertyDescriptor,
    field: usize,
}
const FIELDS: [&str; 6] = [
    "enumerable",
    "configurable",
    "value",
    "writable",
    "get",
    "set",
];

impl DescriptorStep {
    pub(crate) fn start_jsvalue(
        runtime: &Runtime,
        realm: ContextId,
        value: JsValue,
    ) -> Result<Self, RuntimeError> {
        let JsValue::Object(id) = value else {
            runtime.release_jsvalue(value)?;
            return invalid(runtime, realm, "not an object");
        };
        Self::from_object(
            runtime,
            realm,
            ObjectRef::from_owned_handle(runtime.clone(), id),
        )
    }

    #[cfg(test)]
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        value: Value,
    ) -> Result<Self, RuntimeError> {
        let Value::Object(object) = value else {
            return invalid(runtime, realm, "not an object");
        };
        Self::from_object(runtime, realm, object)
    }

    fn from_object(
        runtime: &Runtime,
        realm: ContextId,
        object: ObjectRef,
    ) -> Result<Self, RuntimeError> {
        DescriptorResume(Box::new(DescriptorResumeState {
            pending_effect: DescriptorStepPending::new(runtime),
            state: State {
                realm,
                object,
                descriptor: OwnedPropertyDescriptor::new(runtime),
                field: 0,
            },
            phase: Phase::Read,
        }))
        .next(runtime)
    }
}

fn invalid(
    runtime: &Runtime,
    realm: ContextId,
    message: &'static str,
) -> Result<DescriptorStep, RuntimeError> {
    Ok(DescriptorStep::Throw(runtime.new_native_error(
        realm,
        NativeErrorKind::Type,
        message,
    )?))
}

impl DescriptorResume {
    fn next(mut self, runtime: &Runtime) -> Result<DescriptorStep, RuntimeError> {
        let Some(name) = FIELDS.get(self.state.field) else {
            if self.state.descriptor.is_mixed_descriptor() {
                return invalid(
                    runtime,
                    self.state.realm,
                    "cannot have setter/getter and value or writable",
                );
            }
            return Ok(DescriptorStep::Complete(self));
        };
        let key = runtime.intern_property_key(name)?;
        self.phase = Phase::Has(key.clone());
        let object = self.state.object.clone();
        Ok(DescriptorStep::request_has(object, key, self))
    }
    pub(crate) fn take_descriptor(self) -> OwnedPropertyDescriptor {
        self.0.state.descriptor
    }
}

impl DescriptorResume {
    pub(crate) fn has(
        mut self,
        runtime: &Runtime,
        result: NativeConversion<bool>,
    ) -> Result<DescriptorStep, RuntimeError> {
        let Phase::Has(key) = std::mem::replace(&mut self.0.phase, Phase::Read) else {
            return Err(RuntimeError::Invariant(
                "descriptor Get continuation received a Has reply",
            ));
        };
        let state = &mut self.0.state;
        // Pinned C treats the -1 Has result as present. A subsequent Get can
        // replace the exception, including for getter/setter descriptor fields.
        if matches!(result, NativeConversion::Value(false)) {
            state.field += 1;
            return self.next(runtime);
        }
        let object = state.object.clone();
        let receiver = JsValue::Object(state.object.clone().into_handle());
        Ok(DescriptorStep::request_read(object, key, receiver, self))
    }

    pub(crate) fn read(
        mut self,
        runtime: &Runtime,
        completion: Completion,
    ) -> Result<DescriptorStep, RuntimeError> {
        if !matches!(self.0.phase, Phase::Read) {
            return Err(RuntimeError::Invariant(
                "descriptor Has continuation received a Get reply",
            ));
        }
        let state = &mut self.0.state;
        let value = match completion {
            Completion::Return(value) => value,
            Completion::Throw(value) => {
                if state.field >= 4 {
                    drop(runtime.root_and_release_jsvalue(value)?);
                    return invalid(
                        runtime,
                        state.realm,
                        if state.field == 4 {
                            "invalid getter"
                        } else {
                            "invalid setter"
                        },
                    );
                }
                return Ok(DescriptorStep::Throw(
                    runtime.root_and_release_jsvalue(value)?,
                ));
            }
        };
        match state.field {
            0 | 1 | 3 => {
                let boolean = runtime.value_to_boolean_jsvalue(&value);
                runtime.release_jsvalue(value)?;
                let flag = DescriptorField::Present(boolean?);
                match state.field {
                    0 => state.descriptor.enumerable = flag,
                    1 => state.descriptor.configurable = flag,
                    3 => state.descriptor.writable = flag,
                    _ => unreachable!(),
                }
            }
            2 => state.descriptor.value = DescriptorField::Present(value),
            4 | 5 => {
                let error_message = if state.field == 4 {
                    "invalid getter"
                } else {
                    "invalid setter"
                };
                let accessor = match value {
                    JsValue::Undefined => Some(AccessorValue::Undefined),
                    JsValue::Object(id) => {
                        let object = ObjectRef::from_owned_handle(runtime.clone(), id);
                        runtime.as_callable(&object)?.map(AccessorValue::Callable)
                    }
                    value => {
                        runtime.release_jsvalue(value)?;
                        None
                    }
                };
                let Some(accessor) = accessor else {
                    return invalid(runtime, state.realm, error_message);
                };
                if state.field == 4 {
                    state.descriptor.get = DescriptorField::Present(accessor);
                } else {
                    state.descriptor.set = DescriptorField::Present(accessor);
                }
            }
            _ => {
                runtime.release_jsvalue(value)?;
                return Err(RuntimeError::Invariant(
                    "descriptor field cursor is outside its protocol",
                ));
            }
        }
        state.field += 1;
        self.next(runtime)
    }
}

struct DescriptorStepPending {
    runtime: Runtime,
    has_object: Option<ObjectRef>,
    has_key: Option<PropertyKey>,
    read_object: Option<ObjectRef>,
    read_key: Option<PropertyKey>,
    read_receiver: Option<JsValue>,
}
impl DescriptorStepPending {
    fn new(runtime: &Runtime) -> Self {
        Self {
            runtime: runtime.clone(),
            has_object: None,
            has_key: None,
            read_object: None,
            read_key: None,
            read_receiver: None,
        }
    }
}
impl Drop for DescriptorStepPending {
    /// Release the internal edges still held when the descriptor request is
    /// abandoned before conversion. Consumption goes through `Option::take`;
    /// releases are defer-safe and nothrow, and never run JavaScript.
    fn drop(&mut self) {
        if let Some(value) = self.read_receiver.take() {
            let _ = self.runtime.release_jsvalue(value);
        }
    }
}
impl DescriptorStep {
    pub(crate) fn request_has(
        object: ObjectRef,
        key: PropertyKey,
        mut resume: DescriptorResume,
    ) -> Self {
        resume.0.pending_effect.has_object = Some(object);
        resume.0.pending_effect.has_key = Some(key);
        Self::Has { resume }
    }
    pub(crate) fn request_read(
        object: ObjectRef,
        key: PropertyKey,
        receiver: JsValue,
        mut resume: DescriptorResume,
    ) -> Self {
        resume.0.pending_effect.read_object = Some(object);
        resume.0.pending_effect.read_key = Some(key);
        resume.0.pending_effect.read_receiver = Some(receiver);
        Self::Read { resume }
    }
}
impl DescriptorResume {
    pub(crate) fn take_has_object(&mut self) -> ObjectRef {
        self.0
            .pending_effect
            .has_object
            .take()
            .expect("DescriptorStep Has object")
    }
    pub(crate) fn take_has_key(&mut self) -> PropertyKey {
        self.0
            .pending_effect
            .has_key
            .take()
            .expect("DescriptorStep Has key")
    }
    pub(crate) fn take_read_object(&mut self) -> ObjectRef {
        self.0
            .pending_effect
            .read_object
            .take()
            .expect("DescriptorStep Read object")
    }
    pub(crate) fn take_read_key(&mut self) -> PropertyKey {
        self.0
            .pending_effect
            .read_key
            .take()
            .expect("DescriptorStep Read key")
    }
    pub(crate) fn take_read_receiver(&mut self) -> JsValue {
        self.0
            .pending_effect
            .read_receiver
            .take()
            .expect("DescriptorStep Read receiver")
    }
}
const _: () = assert!(std::mem::size_of::<DescriptorStep>() <= 64);

// S11 all-domain protocol bound; inline completion stays allocation-free.
const _: () = assert!(std::mem::size_of::<DescriptorStep>() <= 64);

#[cfg(test)]
mod tests {
    use super::*;

    fn take_has(step: DescriptorStep) -> DescriptorResume {
        let DescriptorStep::Has { mut resume } = step else {
            panic!("expected presence request");
        };
        drop(resume.take_has_object());
        drop(resume.take_has_key());
        resume
    }

    fn take_read(runtime: &Runtime, step: DescriptorStep) -> DescriptorResume {
        let DescriptorStep::Read { mut resume } = step else {
            panic!("expected value read");
        };
        drop(resume.take_read_object());
        drop(resume.take_read_key());
        runtime
            .release_jsvalue(resume.take_read_receiver())
            .unwrap();
        resume
    }

    #[test]
    fn descriptor_completion_reuses_the_original_resume_allocation() {
        let runtime = Runtime::new();
        let context = runtime.new_context();
        let object = runtime.new_object(None).unwrap();
        let mut resume = take_has(
            DescriptorStep::start(&runtime, context.realm, Value::Object(object)).unwrap(),
        );
        let address = &*resume.0 as *const DescriptorResumeState;
        for _ in 0..5 {
            resume = take_has(
                resume
                    .has(&runtime, NativeConversion::Value(false))
                    .unwrap(),
            );
            assert_eq!(&*resume.0 as *const DescriptorResumeState, address);
        }
        let DescriptorStep::Complete(resume) = resume
            .has(&runtime, NativeConversion::Value(false))
            .unwrap()
        else {
            panic!("descriptor complete")
        };
        assert_eq!(&*resume.0 as *const DescriptorResumeState, address);
        assert!(!resume.take_descriptor().is_mixed_descriptor());
    }

    #[test]
    fn abandoned_descriptor_keeps_then_releases_its_source_and_selected_value() {
        let runtime = Runtime::new();
        let weak = std::rc::Rc::downgrade(&runtime.0);
        let context = runtime.new_context();
        let object = runtime.new_object(None).unwrap();
        let object_id = object.object_id();
        let value = runtime.new_object(None).unwrap();
        let value_id = value.object_id();
        let mut step =
            DescriptorStep::start(&runtime, context.realm, Value::Object(object)).unwrap();
        // A source with only a value field: park immediately after its Get
        // reply, while the remaining writable/get/set probes are pending.
        for _ in 0..2 {
            step = take_has(step)
                .has(&runtime, NativeConversion::Value(false))
                .unwrap();
        }
        let resume = take_read(
            &runtime,
            take_has(step)
                .has(&runtime, NativeConversion::Value(true))
                .unwrap(),
        );
        let step = resume
            .read(
                &runtime,
                Completion::Return(JsValue::Object(value.into_handle())),
            )
            .unwrap();
        runtime.run_gc().unwrap();
        assert!(runtime.0.state.borrow().heap.object(object_id).is_ok());
        assert!(runtime.0.state.borrow().heap.object(value_id).is_ok());
        drop(step);
        runtime.run_gc().unwrap();
        assert!(runtime.0.state.borrow().heap.object(object_id).is_err());
        assert!(runtime.0.state.borrow().heap.object(value_id).is_err());
        drop(context);
        drop(runtime);
        assert!(weak.upgrade().is_none());
    }

    #[test]
    fn descriptor_reply_kind_is_checked_before_applying_it() {
        let runtime = Runtime::new();
        let context = runtime.new_context();
        let object = runtime.new_object(None).unwrap();
        let id = object.object_id();
        let DescriptorStep::Has { mut resume } =
            DescriptorStep::start(&runtime, context.realm, Value::Object(object)).unwrap()
        else {
            panic!("expected initial Has");
        };
        drop(resume.take_has_object());
        drop(resume.take_has_key());
        assert!(
            resume
                .read(&runtime, Completion::Return(JsValue::Int(1)))
                .is_err()
        );
        runtime.run_gc().unwrap();
        assert!(runtime.0.state.borrow().heap.object(id).is_err());
    }
}
