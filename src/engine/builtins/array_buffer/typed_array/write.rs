//! Integer indexed writes retain their owners, then reacquire buffer access.
use super::element::ElementStep;
use crate::engine::{
    api::{runtime::Runtime, runtime_error::RuntimeError},
    builtins::native::TypedArrayElementKind,
    heap::ContextId,
    object::{DescriptorField, ObjectRef, OrdinaryPropertyDescriptor},
    value::{Value, conversion::NativeConversion},
};

pub(crate) enum TypedWriteStep {
    Complete(NativeConversion<bool>),
    Element {
        element: TypedArrayElementKind,
        value: Value,
        resume: TypedWriteResume,
    },
}
pub(crate) struct TypedWriteResume {
    object: ObjectRef,
    index: Option<u64>,
    _value: Value,
}
impl TypedWriteStep {
    pub(crate) fn set(
        runtime: &Runtime,
        object: ObjectRef,
        index: Option<u64>,
        value: Value,
    ) -> Result<Self, RuntimeError> {
        let element = runtime.typed_array_snapshot(&object)?.element;
        Ok(Self::Element {
            element,
            value: value.clone(),
            resume: TypedWriteResume {
                object,
                index,
                _value: value,
            },
        })
    }
    pub(crate) fn define(
        runtime: &Runtime,
        object: ObjectRef,
        index: u64,
        descriptor: &OrdinaryPropertyDescriptor,
    ) -> Result<Self, RuntimeError> {
        if descriptor.get.is_present()
            || descriptor.set.is_present()
            || matches!(descriptor.writable, DescriptorField::Present(false))
            || matches!(descriptor.enumerable, DescriptorField::Present(false))
            || matches!(descriptor.configurable, DescriptorField::Present(false))
        {
            return Ok(Self::Complete(NativeConversion::Value(false)));
        }
        let state = runtime.typed_array_state(&object)?;
        if state.out_of_bounds || index >= u64::from(state.length) {
            return Ok(Self::Complete(NativeConversion::Value(false)));
        }
        let DescriptorField::Present(value) = &descriptor.value else {
            return Ok(Self::Complete(NativeConversion::Value(true)));
        };
        Ok(Self::Element {
            element: state.snapshot.element,
            value: value.clone(),
            resume: TypedWriteResume {
                object,
                index: Some(index),
                _value: value.clone(),
            },
        })
    }
    /// Advance only a primitive input through the shared conversion and write
    /// kernels. Object inputs retain the original request for the owned driver.
    #[cfg(feature = "stack-vm")]
    pub(crate) fn complete_primitive(
        self,
        runtime: &Runtime,
        realm: ContextId,
    ) -> Result<Self, RuntimeError> {
        match self {
            Self::Element {
                element,
                value,
                resume,
            } if !matches!(value, Value::Object(_)) => {
                let ElementStep::Complete(result) =
                    ElementStep::start(runtime, realm, element, value)?
                else {
                    return Err(RuntimeError::Invariant(
                        "primitive element conversion suspended",
                    ));
                };
                resume.element(runtime, result)
            }
            step => Ok(step),
        }
    }

    pub(crate) fn finish_sync(
        self,
        runtime: &Runtime,
        realm: ContextId,
    ) -> Result<NativeConversion<bool>, RuntimeError> {
        match self {
            Self::Complete(result) => Ok(result),
            Self::Element {
                element,
                value,
                resume,
            } => {
                let bytes = ElementStep::start(runtime, realm, element, value)?
                    .finish_sync(runtime, realm)?;
                let Self::Complete(result) = resume.element(runtime, bytes)? else {
                    return Err(RuntimeError::Invariant(
                        "TypedArray write failed to complete",
                    ));
                };
                Ok(result)
            }
        }
    }
}
impl TypedWriteResume {
    pub(crate) fn element(
        self,
        runtime: &Runtime,
        result: NativeConversion<[u8; 8]>,
    ) -> Result<TypedWriteStep, RuntimeError> {
        let result = match result {
            NativeConversion::Throw(value) => NativeConversion::Throw(value),
            NativeConversion::Value(bytes) => {
                if let Some(index) = self.index {
                    // Conversion may detach, resize, or replace the backing bytes.
                    // Both Set and Define ignore a failed post-conversion write.
                    let _ =
                        runtime.typed_array_write_converted_index(&self.object, index, &bytes)?;
                }
                NativeConversion::Value(true)
            }
        };
        Ok(TypedWriteStep::Complete(result))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn take_element(step: TypedWriteStep) -> TypedWriteResume {
        let TypedWriteStep::Element { resume, .. } = step else {
            panic!("expected element conversion")
        };
        resume
    }
    #[test]
    fn typed_write_request_owns_view_buffer_and_value_until_abandonment() {
        for define in [false, true] {
            let runtime = Runtime::new();
            let weak = std::rc::Rc::downgrade(&runtime.0);
            let mut context = runtime.new_context();
            let Value::Object(view) = context.eval("new Uint8Array(1)").unwrap() else {
                panic!("expected view")
            };
            let view_id = view.object_id();
            let buffer_id = runtime.typed_array_snapshot(&view).unwrap().buffer;
            let value = runtime.new_object(None).unwrap();
            let value_id = value.object_id();
            let step = if define {
                TypedWriteStep::define(
                    &runtime,
                    view,
                    0,
                    &OrdinaryPropertyDescriptor {
                        value: DescriptorField::Present(Value::Object(value)),
                        ..OrdinaryPropertyDescriptor::new()
                    },
                )
                .unwrap()
            } else {
                TypedWriteStep::set(&runtime, view, Some(0), Value::Object(value)).unwrap()
            };
            let resume = take_element(step);
            runtime.run_gc().unwrap();
            for id in [view_id, buffer_id, value_id] {
                assert!(runtime.0.state.borrow().heap.object(id).is_ok());
            }
            drop(resume);
            runtime.run_gc().unwrap();
            for id in [view_id, buffer_id, value_id] {
                assert!(runtime.0.state.borrow().heap.object(id).is_err());
            }
            drop(context);
            drop(runtime);
            assert!(weak.upgrade().is_none());
        }
    }
}
