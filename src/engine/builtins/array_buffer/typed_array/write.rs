//! Integer indexed writes retain their owners, then reacquire buffer access.
use super::element::ElementStep;
#[cfg(test)]
use crate::engine::value::Value;
use crate::engine::{
    api::{runtime::Runtime, runtime_error::RuntimeError},
    builtins::native::TypedArrayElementKind,
    heap::ContextId,
    object::{DescriptorField, ObjectRef, OrdinaryPropertyDescriptor},
    value::{JsValue, conversion::NativeConversion},
};

pub(crate) enum TypedWriteStep {
    Complete(NativeConversion<bool>),
    Element {
        element: TypedArrayElementKind,
        value: JsValue,
        resume: TypedWriteResume,
    },
}
pub(crate) struct TypedWriteResume(Box<TypedWriteResumeState>);
impl std::ops::Deref for TypedWriteResume {
    type Target = TypedWriteResumeState;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for TypedWriteResume {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
const _: () = assert!(std::mem::size_of::<TypedWriteResume>() <= 8);
pub(crate) struct TypedWriteResumeState {
    object: ObjectRef,
    index: Option<u64>,
    value: Option<JsValue>,
}
impl Drop for TypedWriteResumeState {
    fn drop(&mut self) {
        if let Some(value) = self.value.take() {
            let _ = self.object.runtime().release_jsvalue(value);
        }
    }
}
impl TypedWriteStep {
    /// Retire an actual selected child rejected before scheduler publication.
    pub(crate) fn retire_at_boundary(self, runtime: &Runtime) -> Result<(), RuntimeError> {
        if runtime.skip_cleanup() {
            return Err(RuntimeError::Poisoned);
        }
        match self {
            Self::Complete(NativeConversion::Throw(value)) => runtime.release_jsvalue(value)?,
            Self::Complete(NativeConversion::Value(_)) => {}
            Self::Element { value, resume, .. } => {
                runtime.release_jsvalue(value)?;
                runtime.check_poison()?;
                drop(resume);
            }
        }
        runtime.check_poison()
    }
}

impl TypedWriteStep {
    pub(crate) fn set(
        runtime: &Runtime,
        object: ObjectRef,
        index: Option<u64>,
        value: JsValue,
    ) -> Result<Self, RuntimeError> {
        let resume = TypedWriteResume(Box::new(TypedWriteResumeState {
            object,
            index,
            value: Some(value),
        }));
        let element = runtime.typed_array_snapshot(&resume.object)?.element;
        Self::start_selected(runtime, element, resume)
    }

    /// Consume the exact metadata/index chosen by the State Set producer.
    /// All input kinds use the same Element child; no key or view is selected again.
    pub(crate) fn set_selected(
        runtime: &Runtime,
        object: ObjectRef,
        index: Option<u64>,
        element: TypedArrayElementKind,
        value: JsValue,
    ) -> Result<Self, RuntimeError> {
        Self::start_selected(
            runtime,
            element,
            TypedWriteResume(Box::new(TypedWriteResumeState {
                object,
                index,
                value: Some(value),
            })),
        )
    }

    fn start_selected(
        runtime: &Runtime,
        element: TypedArrayElementKind,
        resume: TypedWriteResume,
    ) -> Result<Self, RuntimeError> {
        let value = runtime.dup_jsvalue(resume.value.as_ref().expect("typed write value"))?;
        Ok(Self::Element {
            element,
            value,
            resume,
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
        Self::set(runtime, object, Some(index), runtime.unroot_value(value)?)
    }

    pub(crate) fn finish_context_free(
        self,
        runtime: &Runtime,
    ) -> Result<NativeConversion<bool>, RuntimeError> {
        match self {
            Self::Complete(result) => Ok(result),
            Self::Element {
                element,
                value,
                resume,
            } => {
                // This is the public context-free primitive conversion boundary.
                // No descriptor or storage value is re-admitted to the heap.
                let bytes = runtime.typed_array_convert_primitive_element_jsvalue(element, &value);
                runtime.release_jsvalue(value)?;
                let Self::Complete(result) =
                    resume.element(runtime, NativeConversion::Value(bytes?))?
                else {
                    return Err(RuntimeError::Invariant(
                        "TypedArray primitive write failed to complete",
                    ));
                };
                Ok(result)
            }
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
        Ok(TypedWriteStep::Complete(finish_element(
            runtime,
            &self.0.object,
            self.0.index,
            result,
        )?))
    }
}

fn finish_element(
    runtime: &Runtime,
    object: &ObjectRef,
    index: Option<u64>,
    result: NativeConversion<[u8; 8]>,
) -> Result<NativeConversion<bool>, RuntimeError> {
    let result = match result {
        NativeConversion::Throw(value) => NativeConversion::Throw(value),
        NativeConversion::Value(bytes) => {
            if let Some(index) = index {
                // Conversion may detach, resize, or replace the backing bytes.
                // Both Set and Define ignore a failed post-conversion write.
                let _ = runtime.typed_array_write_converted_index(object, index, &bytes)?;
            }
            NativeConversion::Value(true)
        }
    };
    Ok(result)
}

// S11 all-domain protocol bound; inline completion stays allocation-free.
const _: () = assert!(std::mem::size_of::<TypedWriteStep>() <= 64);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primitive_set_keeps_invalid_index_conversion_and_receiver_rules() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        assert_eq!(
            context
                .eval(
                    r#"(()=>{
            let a=new Uint8Array(1), errors=0;
            a[0]=257;
            try { a['-0']=Symbol(); } catch(e) { if(e instanceof TypeError)errors++; }
            try { a[NaN]=1n; } catch(e) { if(e instanceof TypeError)errors++; }
            let marker=Symbol(); a['01']=marker;
            let receiver={};
            let valid=Reflect.set(a,'0',19,receiver);
            let invalid=Reflect.set(a,'-0',marker,receiver);
            let b=new BigInt64Array(1); b[0]='7';
            try { b[1]=1; } catch(e) { if(e instanceof TypeError)errors++; }
            return errors===3 && a[0]===1 && a['01']===marker && b[0]===7n
                && valid && receiver[0]===19 && invalid && !('-0' in receiver);
        })()"#
                )
                .unwrap(),
            Value::Bool(true)
        );
    }

    #[test]
    fn canonical_set_covers_typed_index_receiver_and_ordinary_key_rules() {
        use crate::engine::object::operations::InternalSetResult;
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        for (key, input, same_receiver, expected) in [
            ("0", "257", true, "stored"),
            ("-0", "Symbol()", true, "throw"),
            ("NaN", "1n", true, "throw"),
            ("01", "Symbol()", true, "ordinary"),
            ("0", "Symbol()", false, "ordinary"),
            ("-0", "Symbol()", false, "ignore"),
            ("5", "Symbol()", false, "ignore"),
        ] {
            let Value::Object(object) = context.eval("new Uint8Array(1)").unwrap() else {
                panic!("expected typed array");
            };
            let receiver = if same_receiver {
                object.try_clone().expect("duplicate root")
            } else {
                runtime.new_object(None).unwrap()
            };
            let key = runtime.intern_property_key(key).unwrap();
            let value = context.eval(input).unwrap();
            // The actual canonical consumer owns both converted operands.
            let result = runtime
                .internal_set_jsvalue(
                    context.realm,
                    &object,
                    &key,
                    runtime.unroot_value(&value).unwrap(),
                    runtime
                        .into_jsvalue(Value::Object(receiver.try_clone().unwrap()))
                        .unwrap(),
                )
                .unwrap();
            assert!(matches!(
                (expected, &result),
                (
                    "stored" | "ordinary" | "ignore",
                    NativeConversion::Value(InternalSetResult::Accepted)
                ) | ("throw", NativeConversion::Throw(_))
            ));
            if let NativeConversion::Throw(value) = result {
                runtime.release_jsvalue(value).unwrap();
            }
            assert_eq!(
                runtime.typed_array_read_index(&object, 0).unwrap(),
                Some(Value::Int(if expected == "stored" { 1 } else { 0 }))
            );
            if expected == "ordinary" {
                assert_eq!(context.get_property(&receiver, &key).unwrap(), value);
            } else if expected == "ignore" {
                assert_eq!(
                    context.get_property(&receiver, &key).unwrap(),
                    Value::Undefined
                );
            }
        }
    }

    #[test]
    fn canonical_set_keeps_detached_conversion_and_accepts_object_requests() {
        use crate::engine::object::operations::InternalSetResult;
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        let Value::Object(buffer) = context.eval("globalThis.b=new ArrayBuffer(1); b").unwrap()
        else {
            panic!("expected buffer");
        };
        let Value::Object(object) = context.eval("new Uint8Array(b)").unwrap() else {
            panic!("expected typed array");
        };
        context.detach_array_buffer(&Value::Object(buffer)).unwrap();
        let key = runtime.intern_property_key("0").unwrap();
        let receiver = JsValue::Object(object.try_clone().expect("duplicate root").into_handle());
        assert!(matches!(
            runtime
                .internal_set_jsvalue(
                    context.realm,
                    &object,
                    &key,
                    JsValue::Int(257),
                    runtime.dup_jsvalue(&receiver).unwrap()
                )
                .unwrap(),
            NativeConversion::Value(InternalSetResult::Accepted)
        ));
        let bigint = runtime.into_jsvalue(context.eval("1n").unwrap()).unwrap();
        let NativeConversion::Throw(thrown) = runtime
            .internal_set_jsvalue(
                context.realm,
                &object,
                &key,
                runtime.dup_jsvalue(&bigint).unwrap(),
                runtime.dup_jsvalue(&receiver).unwrap(),
            )
            .unwrap()
        else {
            panic!("expected conversion throw");
        };
        runtime.release_jsvalue(thrown).unwrap();
        let other_receiver = JsValue::Object(runtime.new_object(None).unwrap().into_handle());
        assert!(matches!(
            runtime
                .internal_set_jsvalue(
                    context.realm,
                    &object,
                    &key,
                    runtime.dup_jsvalue(&bigint).unwrap(),
                    other_receiver
                )
                .unwrap(),
            NativeConversion::Value(InternalSetResult::Accepted)
        ));
        // A detached destination still converts a same-receiver object input.
        let value = runtime
            .into_jsvalue(
                context
                    .eval("globalThis.calls=0; ({valueOf(){calls++; return 257}})")
                    .unwrap(),
            )
            .unwrap();
        assert!(matches!(
            runtime
                .internal_set_jsvalue(
                    context.realm,
                    &object,
                    &key,
                    value,
                    runtime.dup_jsvalue(&receiver).unwrap()
                )
                .unwrap(),
            NativeConversion::Value(InternalSetResult::Accepted)
        ));
        assert_eq!(context.eval("calls").unwrap(), Value::Int(1));
        assert!(
            runtime
                .typed_array_read_index(&object, 0)
                .unwrap()
                .is_none()
        );
        runtime.release_jsvalue(bigint).unwrap();
        runtime.release_jsvalue(receiver).unwrap();
        assert!(runtime.0.state.borrow().active_frames.is_empty());
    }

    fn take_element(runtime: &Runtime, step: TypedWriteStep) -> TypedWriteResume {
        let TypedWriteStep::Element { value, resume, .. } = step else {
            panic!("expected element conversion")
        };
        runtime.release_jsvalue(value).unwrap();
        resume
    }
    #[test]
    fn typed_write_request_owns_view_buffer_and_value_until_abandonment() {
        for define in [false, true] {
            let runtime = Runtime::new();
            let weak = std::rc::Rc::downgrade(&runtime.0);
            let mut context = runtime.new_context().expect("create context");
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
                TypedWriteStep::set(
                    &runtime,
                    view,
                    Some(0),
                    runtime.into_jsvalue(Value::Object(value)).unwrap(),
                )
                .unwrap()
            };
            let resume = take_element(&runtime, step);
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
