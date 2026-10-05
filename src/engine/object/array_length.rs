//! Only object Array lengths need two observable ToNumber requests.
use crate::engine::api::{runtime::Runtime, runtime_error::RuntimeError};
use crate::engine::heap::{
    ContextId,
    runtime::{RuntimeState, owned_values::OwnedValueGuard},
};
use crate::engine::object::operations::ArrayLengthConversion;
#[cfg(test)]
use crate::engine::value::Value;
use crate::engine::value::{JsValue, conversion::NativeConversion};
use std::cell::Cell;
mod state;

pub(crate) enum ArrayLengthStep {
    Complete(ArrayLengthConversion),
    Number {
        value: JsValue,
        resume: ArrayLengthResume,
    },
}
#[must_use]
pub(crate) struct ArrayLengthResume(Box<ArrayLengthResumeState>);
struct ArrayLengthResumeState {
    realm: Option<ContextId>,
    original: JsValue,
    uint32: Option<u32>,
}
impl ArrayLengthStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: Option<ContextId>,
        value: JsValue,
    ) -> Result<Self, RuntimeError> {
        // Preserve the allocation/borrow-free scalar entry used by ordinary Set.
        match value {
            JsValue::Int(value) if value >= 0 => {
                return Ok(Self::Complete(ArrayLengthConversion::Length(value as u32)));
            }
            JsValue::Bool(value) => {
                return Ok(Self::Complete(ArrayLengthConversion::Length(u32::from(
                    value,
                ))));
            }
            JsValue::Null => return Ok(Self::Complete(ArrayLengthConversion::Length(0))),
            _ => {}
        }
        let mut state = runtime.0.state.borrow_mut();
        Self::start_in_state(&mut state, &runtime.0.poisoned, realm, value)
    }
    pub(crate) fn start_in_state(
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        realm: Option<ContextId>,
        value: JsValue,
    ) -> Result<Self, RuntimeError> {
        let mut original = OwnedValueGuard::new(state, poisoned, value);
        let (state, original) = original.parts();
        let value = original.as_ref().expect("Array length owner");
        if matches!(value, JsValue::Object(_)) {
            let value = state.dup_jsvalue(value)?;
            return Ok(Self::Number {
                value,
                resume: ArrayLengthResume(Box::new(ArrayLengthResumeState {
                    realm,
                    original: original.take().expect("Array length owner"),
                    uint32: None,
                })),
            });
        }
        let conversion = state.array_length_from_primitive(poisoned, realm, value)?;
        state.release_owned_jsvalue(poisoned, original.take().expect("Array length owner"))?;
        Ok(Self::Complete(conversion))
    }
}
impl ArrayLengthResume {
    pub(crate) fn release_in_state(
        self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
    ) -> Result<(), RuntimeError> {
        state.release_owned_jsvalue(poisoned, self.0.original)
    }
    pub(crate) fn release_owned(self, runtime: &Runtime) {
        if !runtime.skip_cleanup() {
            let _ = self.release_in_state(&mut runtime.0.state.borrow_mut(), &runtime.0.poisoned);
        }
    }
    pub(crate) fn number(
        self,
        runtime: &Runtime,
        result: NativeConversion<f64>,
    ) -> Result<ArrayLengthStep, RuntimeError> {
        self.number_in_state(
            &mut runtime.0.state.borrow_mut(),
            &runtime.0.poisoned,
            result,
        )
    }
    fn number_in_state(
        self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        result: NativeConversion<f64>,
    ) -> Result<ArrayLengthStep, RuntimeError> {
        let ArrayLengthResumeState {
            realm,
            original,
            uint32,
        } = *self.0;
        let mut original = OwnedValueGuard::new(state, poisoned, original);
        let (state, original) = original.parts();
        let number = match result {
            NativeConversion::Throw(value) => {
                state.release_owned_jsvalue(
                    poisoned,
                    original.take().expect("Array length owner"),
                )?;
                return Ok(ArrayLengthStep::Complete(ArrayLengthConversion::Throw(
                    value,
                )));
            }
            NativeConversion::Value(number) => number,
        };
        if let Some(uint32) = uint32 {
            let conversion = state.validate_array_length_number_in_state(
                poisoned,
                realm,
                number,
                Some(uint32),
            )?;
            state.release_owned_jsvalue(poisoned, original.take().expect("Array length owner"))?;
            return Ok(ArrayLengthStep::Complete(conversion));
        }
        let value = state.dup_jsvalue(original.as_ref().expect("Array length owner"))?;
        Ok(ArrayLengthStep::Number {
            value,
            resume: Self(Box::new(ArrayLengthResumeState {
                realm,
                original: original.take().expect("Array length owner"),
                uint32: Some(Runtime::to_uint32_number(number)),
            })),
        })
    }
}
const _: () = assert!(std::mem::size_of::<ArrayLengthResume>() <= 8);
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
            let context = runtime.new_context().expect("create context");
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
            resume.release_owned(&runtime);
            runtime.run_gc().unwrap();
            assert!(runtime.0.state.borrow().heap.object(id).is_err());
            drop(context);
            drop(runtime);
            assert!(weak.upgrade().is_none());
        }
    }
    #[test]
    fn primitive_lengths_complete_and_retire_input_without_runtime_owner() {
        let runtime = Runtime::new();
        let context = runtime.new_context().unwrap();
        let owned = runtime
            .into_jsvalue(Value::String(crate::engine::value::JsString::from_static(
                " 3 ",
            )))
            .unwrap();
        let JsValue::String(string) = owned else {
            panic!()
        };
        let rc = std::rc::Rc::strong_count(&runtime.0);
        let mut state = runtime.0.state.borrow_mut();
        let step = ArrayLengthStep::start_in_state(
            &mut state,
            &runtime.0.poisoned,
            Some(context.realm),
            JsValue::String(string),
        )
        .unwrap();
        assert!(matches!(
            step,
            ArrayLengthStep::Complete(ArrayLengthConversion::Length(3))
        ));
        assert!(state.heap.string(string).is_err());
        assert_eq!(std::rc::Rc::strong_count(&runtime.0), rc);
        assert!(!runtime.0.deferred_references.has_pending());
        let step = ArrayLengthStep::start_in_state(
            &mut state,
            &runtime.0.poisoned,
            Some(context.realm),
            JsValue::Undefined,
        )
        .unwrap();
        let ArrayLengthStep::Complete(ArrayLengthConversion::Throw(error)) = step else {
            panic!()
        };
        state
            .release_owned_jsvalue(&runtime.0.poisoned, error)
            .unwrap();
        let step = ArrayLengthStep::start_in_state(
            &mut state,
            &runtime.0.poisoned,
            Some(context.realm),
            JsValue::ShortBigInt(3),
        )
        .unwrap();
        let ArrayLengthStep::Complete(ArrayLengthConversion::Throw(error)) = step else {
            panic!()
        };
        state
            .release_owned_jsvalue(&runtime.0.poisoned, error)
            .unwrap();
    }

    #[test]
    fn object_length_resume_owns_only_js_edges_and_cleans_failed_second_retain() {
        use crate::engine::heap::RawId;
        let runtime = Runtime::new();
        let original = runtime.new_object(None).unwrap().into_execution_handle();
        let rc = std::rc::Rc::strong_count(&runtime.0);
        let resume = take_number(
            &runtime,
            ArrayLengthStep::start(&runtime, None, JsValue::Object(original)).unwrap(),
        );
        assert_eq!(std::rc::Rc::strong_count(&runtime.0), rc);
        let mut state = runtime.0.state.borrow_mut();
        state
            .heap
            .set_strong_count_for_test(RawId::Object(original), u32::MAX);
        let result = resume.number_in_state(
            &mut state,
            &runtime.0.poisoned,
            NativeConversion::Value(2.0),
        );
        assert!(matches!(
            result,
            Err(RuntimeError::Heap(
                crate::engine::heap::HeapError::Overflow { .. }
            ))
        ));
        assert_eq!(state.heap.object_strong_count(original), Ok(u32::MAX));
        state
            .heap
            .set_strong_count_for_test(RawId::Object(original), 1);
        state.release_jsvalue(JsValue::Object(original)).unwrap();
        assert!(!runtime.is_poisoned());
    }

    #[test]
    fn object_length_keeps_two_conversions_and_reload_after_each_callback() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        assert_eq!(context.eval(r#"(()=>{
            let count=0;let a=[1,2,3];
            const length={valueOf(){count++;a[5]=6;return count===1?2:3}};
            let threw=false;try{Object.defineProperty(a,'length',{value:length})}catch(e){threw=e instanceof RangeError}
            if(!threw || count!==2 || a.length!==6)return false;
            count=0;a=[1,2,3];
            a.length={valueOf(){count++;return 2}};
            return count===2 && a.length===2 && a[1]===2;
        })()"#).unwrap(),Value::Bool(true));
    }
}
