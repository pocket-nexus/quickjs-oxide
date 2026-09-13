//! Pinned js_obj_to_desc order, with each Has/Get exposed as a resumable step.
use crate::engine::api::{error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError};
use crate::engine::heap::ContextId;
use crate::engine::object::{
    AccessorValue, DescriptorField, ObjectRef, OrdinaryPropertyDescriptor, PropertyKey,
};
use crate::engine::value::{Value, conversion::NativeConversion};
use crate::engine::vm::Completion;

pub(crate) enum DescriptorStep {
    Complete(NativeConversion<OrdinaryPropertyDescriptor>),
    Has {
        object: ObjectRef,
        key: PropertyKey,
        resume: DescriptorResume,
    },
    Read {
        object: ObjectRef,
        key: PropertyKey,
        receiver: Value,
        resume: DescriptorResume,
    },
}

pub(crate) struct DescriptorResume {
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
    descriptor: OrdinaryPropertyDescriptor,
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
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        value: Value,
    ) -> Result<Self, RuntimeError> {
        let Value::Object(object) = value else {
            return invalid(runtime, realm, "not an object");
        };
        State {
            realm,
            object,
            descriptor: OrdinaryPropertyDescriptor::new(),
            field: 0,
        }
        .next(runtime)
    }
}

fn invalid(
    runtime: &Runtime,
    realm: ContextId,
    message: &'static str,
) -> Result<DescriptorStep, RuntimeError> {
    Ok(DescriptorStep::Complete(NativeConversion::Throw(
        runtime.new_native_error(realm, NativeErrorKind::Type, message)?,
    )))
}

impl State {
    fn next(self, runtime: &Runtime) -> Result<DescriptorStep, RuntimeError> {
        let Some(name) = FIELDS.get(self.field) else {
            if self.descriptor.is_mixed_descriptor() {
                return invalid(
                    runtime,
                    self.realm,
                    "cannot have setter/getter and value or writable",
                );
            }
            return Ok(DescriptorStep::Complete(NativeConversion::Value(
                self.descriptor,
            )));
        };
        let key = runtime.intern_property_key(name)?;
        Ok(DescriptorStep::Has {
            object: self.object.clone(),
            key: key.clone(),
            resume: DescriptorResume {
                state: self,
                phase: Phase::Has(key),
            },
        })
    }
}

impl DescriptorResume {
    pub(crate) fn has(
        self,
        runtime: &Runtime,
        result: NativeConversion<bool>,
    ) -> Result<DescriptorStep, RuntimeError> {
        let Phase::Has(key) = self.phase else {
            return Err(RuntimeError::Invariant(
                "descriptor Get continuation received a Has reply",
            ));
        };
        let mut state = self.state;
        // Pinned C treats the -1 Has result as present. A subsequent Get can
        // replace the exception, including for getter/setter descriptor fields.
        if matches!(result, NativeConversion::Value(false)) {
            state.field += 1;
            return state.next(runtime);
        }
        Ok(DescriptorStep::Read {
            object: state.object.clone(),
            key,
            receiver: Value::Object(state.object.clone()),
            resume: Self {
                state,
                phase: Phase::Read,
            },
        })
    }

    pub(crate) fn read(
        self,
        runtime: &Runtime,
        completion: Completion,
    ) -> Result<DescriptorStep, RuntimeError> {
        if !matches!(self.phase, Phase::Read) {
            return Err(RuntimeError::Invariant(
                "descriptor Has continuation received a Get reply",
            ));
        }
        let mut state = self.state;
        let value = match completion {
            Completion::Return(value) => value,
            Completion::Throw(value) => {
                if state.field >= 4 {
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
                return Ok(DescriptorStep::Complete(NativeConversion::Throw(value)));
            }
        };
        match state.field {
            0 => {
                state.descriptor.enumerable =
                    DescriptorField::Present(runtime.value_to_boolean(&value)?)
            }
            1 => {
                state.descriptor.configurable =
                    DescriptorField::Present(runtime.value_to_boolean(&value)?)
            }
            2 => state.descriptor.value = DescriptorField::Present(value),
            3 => {
                state.descriptor.writable =
                    DescriptorField::Present(runtime.value_to_boolean(&value)?)
            }
            4 | 5 => {
                let error_message = if state.field == 4 {
                    "invalid getter"
                } else {
                    "invalid setter"
                };
                let accessor = match value {
                    Value::Undefined => AccessorValue::Undefined,
                    Value::Object(object) => {
                        let Some(callable) = runtime.as_callable(&object)? else {
                            return invalid(runtime, state.realm, error_message);
                        };
                        AccessorValue::Callable(callable)
                    }
                    _ => return invalid(runtime, state.realm, error_message),
                };
                if state.field == 4 {
                    state.descriptor.get = DescriptorField::Present(accessor);
                } else {
                    state.descriptor.set = DescriptorField::Present(accessor);
                }
            }
            _ => {
                return Err(RuntimeError::Invariant(
                    "descriptor field cursor is outside its protocol",
                ));
            }
        }
        state.field += 1;
        state.next(runtime)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn take_has(step: DescriptorStep) -> DescriptorResume {
        let DescriptorStep::Has { resume, .. } = step else {
            panic!("expected presence request");
        };
        resume
    }

    fn take_read(step: DescriptorStep) -> DescriptorResume {
        let DescriptorStep::Read { resume, .. } = step else {
            panic!("expected value read");
        };
        resume
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
            take_has(step)
                .has(&runtime, NativeConversion::Value(true))
                .unwrap(),
        );
        let step = resume
            .read(&runtime, Completion::Return(Value::Object(value)))
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
        let DescriptorStep::Has { resume, .. } =
            DescriptorStep::start(&runtime, context.realm, Value::Object(object)).unwrap()
        else {
            panic!("expected initial Has");
        };
        assert!(
            resume
                .read(&runtime, Completion::Return(Value::Int(1)))
                .is_err()
        );
        runtime.run_gc().unwrap();
        assert!(runtime.0.state.borrow().heap.object(id).is_err());
    }
}
