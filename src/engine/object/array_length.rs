//! Pinned Array length conversion: non-numbers undergo two observable ToNumbers.
use crate::engine::api::{runtime::Runtime, runtime_error::RuntimeError};
use crate::engine::heap::ContextId;
use crate::engine::object::operations::ArrayLengthConversion;
#[cfg(test)]
use crate::engine::value::Value;
use crate::engine::value::{JsValue, conversion::NativeConversion};

pub(crate) enum ArrayLengthStep {
    Complete(ArrayLengthConversion),
    Number {
        value: JsValue,
        resume: ArrayLengthResume,
    },
}
pub(crate) struct ArrayLengthResume(Box<ArrayLengthResumeState>);
impl std::ops::Deref for ArrayLengthResume {
    type Target = ArrayLengthResumeState;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for ArrayLengthResume {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
const _: () = assert!(std::mem::size_of::<ArrayLengthResume>() <= 8);
pub(crate) struct ArrayLengthResumeState {
    runtime: Runtime,
    realm: Option<ContextId>,
    original: Option<JsValue>,
    uint32: Option<u32>,
}
impl Drop for ArrayLengthResumeState {
    fn drop(&mut self) {
        if let Some(value) = self.original.take() {
            let _ = self.runtime.release_jsvalue(value);
        }
    }
}
impl ArrayLengthStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: Option<ContextId>,
        value: JsValue,
    ) -> Result<Self, RuntimeError> {
        Ok(match value {
            JsValue::Int(value) if value >= 0 => {
                Self::Complete(ArrayLengthConversion::Length(value as u32))
            }
            JsValue::Bool(value) => Self::Complete(ArrayLengthConversion::Length(u32::from(value))),
            JsValue::Null => Self::Complete(ArrayLengthConversion::Length(0)),
            JsValue::Float(value) => {
                Self::Complete(runtime.validate_array_length_number(realm, value, None)?)
            }
            JsValue::Int(_) => Self::Complete(runtime.invalid_array_length(realm)?),
            value => {
                let resume = ArrayLengthResume(Box::new(ArrayLengthResumeState {
                    runtime: runtime.clone(),
                    realm,
                    original: Some(value),
                    uint32: None,
                }));
                let value = runtime
                    .dup_jsvalue(resume.original.as_ref().expect("Array length original"))?;
                Self::Number { value, resume }
            }
        })
    }
}
impl ArrayLengthResume {
    pub(crate) fn number(
        mut self,
        runtime: &Runtime,
        result: NativeConversion<f64>,
    ) -> Result<ArrayLengthStep, RuntimeError> {
        let number = match result {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => {
                return Ok(ArrayLengthStep::Complete(ArrayLengthConversion::Throw(
                    value,
                )));
            }
        };
        if let Some(uint32) = self.uint32 {
            return Ok(ArrayLengthStep::Complete(
                runtime.validate_array_length_number(self.realm, number, Some(uint32))?,
            ));
        }
        self.uint32 = Some(Runtime::to_uint32_number(number));
        let value = runtime.dup_jsvalue(self.original.as_ref().expect("Array length original"))?;
        Ok(ArrayLengthStep::Number {
            value,
            resume: self,
        })
    }
}

// S11 all-domain protocol bound; inline completion stays allocation-free.
const _: () = assert!(std::mem::size_of::<ArrayLengthStep>() <= 64);

#[cfg(test)]
mod tests {
    use super::*;

    fn take_number(runtime: &Runtime, step: ArrayLengthStep) -> ArrayLengthResume {
        let ArrayLengthStep::Number { value, resume } = step else {
            panic!("expected ToNumber request")
        };
        runtime.release_jsvalue(value).unwrap();
        resume
    }

    #[test]
    fn both_array_length_requests_root_original_until_reply_or_abandonment() {
        for second in [false, true] {
            let runtime = Runtime::new();
            let weak = std::rc::Rc::downgrade(&runtime.0);
            let context = runtime.new_context();
            let original = runtime.new_object(None).unwrap();
            let id = original.object_id();
            let mut resume = take_number(
                &runtime,
                ArrayLengthStep::start(
                    &runtime,
                    Some(context.realm),
                    runtime.into_jsvalue(Value::Object(original)).unwrap(),
                )
                .unwrap(),
            );
            if second {
                resume = take_number(
                    &runtime,
                    resume
                        .number(&runtime, NativeConversion::Value(1.0))
                        .unwrap(),
                );
            }
            runtime.run_gc().unwrap();
            assert!(runtime.0.state.borrow().heap.object(id).is_ok());
            drop(resume);
            runtime.run_gc().unwrap();
            assert!(runtime.0.state.borrow().heap.object(id).is_err());
            drop(context);
            drop(runtime);
            assert!(weak.upgrade().is_none());
        }
    }
}
