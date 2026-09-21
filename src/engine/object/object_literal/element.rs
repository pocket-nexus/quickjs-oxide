//! Computed literal definitions convert the retained key once before own definition.
use crate::engine::{
    api::{Error, ErrorKind, runtime::Runtime, runtime_error::RuntimeError},
    heap::ContextId,
    object::{ObjectRef, OwnedPropertyDescriptor, PropertyKey, operations::InternalDefineResult},
    value::{JsValue, conversion::NativeConversion},
    vm::Completion,
};

pub(crate) enum LiteralDefinitionStep {
    Complete(Completion),
    Primitive { resume: LiteralDefinitionResume },
    Define { resume: LiteralDefinitionResume },
}
pub(crate) struct LiteralDefinitionResume(Box<LiteralDefinitionState>);
struct LiteralDefinitionState {
    runtime: Runtime,
    realm: Option<ContextId>,
    object: Option<ObjectRef>,
    value: Option<JsValue>,
    primitive: Option<JsValue>,
    key: Option<PropertyKey>,
    descriptor: Option<OwnedPropertyDescriptor>,
}
impl Drop for LiteralDefinitionState {
    fn drop(&mut self) {
        for value in [self.value.take(), self.primitive.take()]
            .into_iter()
            .flatten()
        {
            let _ = self.runtime.release_jsvalue(value);
        }
    }
}
const _: () = assert!(std::mem::size_of::<LiteralDefinitionStep>() <= 64);
const _: () = assert!(std::mem::size_of::<LiteralDefinitionResume>() <= 8);
impl LiteralDefinitionStep {
    pub(crate) fn define(
        object: ObjectRef,
        key: PropertyKey,
        descriptor: OwnedPropertyDescriptor,
    ) -> Self {
        let runtime = object.runtime().clone();
        Self::Define {
            resume: LiteralDefinitionResume(Box::new(LiteralDefinitionState {
                runtime,
                realm: None,
                object: Some(object),
                value: None,
                primitive: None,
                key: Some(key),
                descriptor: Some(descriptor),
            })),
        }
    }
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        object: ObjectRef,
        key: JsValue,
        value: JsValue,
    ) -> Result<Self, RuntimeError> {
        let mut resume = LiteralDefinitionResume(Box::new(LiteralDefinitionState {
            runtime: runtime.clone(),
            realm: Some(realm),
            object: Some(object),
            value: Some(value),
            primitive: Some(key),
            key: None,
            descriptor: None,
        }));
        if matches!(resume.0.primitive, Some(JsValue::Object(_))) {
            Ok(Self::Primitive { resume })
        } else {
            let key = resume.take_primitive();
            resume.resume(runtime, Completion::Return(key))
        }
    }
}

impl LiteralDefinitionResume {
    pub(crate) fn take_primitive(&mut self) -> JsValue {
        self.0.primitive.take().expect("literal primitive request")
    }
    pub(crate) fn take_define(&mut self) -> (ObjectRef, PropertyKey, OwnedPropertyDescriptor) {
        (
            self.0.object.take().expect("literal object"),
            self.0.key.take().expect("literal key"),
            self.0.descriptor.take().expect("literal descriptor"),
        )
    }
    pub(crate) fn resume(
        mut self,
        runtime: &Runtime,
        result: Completion,
    ) -> Result<LiteralDefinitionStep, RuntimeError> {
        let key = match result {
            Completion::Return(value) => value,
            Completion::Throw(value) => {
                return Ok(LiteralDefinitionStep::Complete(Completion::Throw(value)));
            }
        };
        let realm = self.0.realm.take().ok_or(RuntimeError::Invariant(
            "literal definition lost its key conversion owner",
        ))?;
        let key = match runtime.property_key_from_primitive_jsvalue(realm, key)? {
            NativeConversion::Value(key) => key,
            NativeConversion::Throw(value) => {
                return Ok(LiteralDefinitionStep::Complete(Completion::Throw(value)));
            }
        };
        self.0.key = Some(key);
        let object = self.0.object.as_ref().expect("literal object");
        let key = self.0.key.as_ref().expect("literal key");
        let internal_data = {
            let state = runtime.0.state.borrow();
            let data = state.heap.object(object.object_id())?;
            matches!(
                (data.kind, &data.payload),
                (
                    crate::engine::heap::ObjectKind::Ordinary,
                    crate::engine::heap::ObjectPayload::Ordinary
                )
            ) || (matches!(
                data.payload,
                crate::engine::heap::ObjectPayload::Array { .. }
            ) && state.atoms.array_index(key.atom())?.is_some())
        };
        if internal_data {
            use crate::engine::object::operations::PropertyDefineOutcome;
            return match runtime.define_selected_set_data(
                object,
                key,
                self.0.value.as_ref().expect("literal value"),
                false,
            )? {
                PropertyDefineOutcome::Defined(true) => Ok(LiteralDefinitionStep::Complete(
                    Completion::Return(JsValue::Undefined),
                )),
                PropertyDefineOutcome::Defined(false) => {
                    Err(Error::new(ErrorKind::Type, "property is not configurable").into())
                }
                PropertyDefineOutcome::Throw(value) => {
                    Ok(LiteralDefinitionStep::Complete(Completion::Throw(value)))
                }
            };
        }
        let value = self.0.value.take().expect("literal value");
        self.0.descriptor = Some(OwnedPropertyDescriptor::data(runtime, value));
        Ok(LiteralDefinitionStep::Define { resume: self })
    }
    pub(crate) fn defined(
        self,
        result: NativeConversion<InternalDefineResult>,
    ) -> Result<LiteralDefinitionStep, RuntimeError> {
        if self.0.realm.is_some() {
            if let NativeConversion::Throw(value) = result {
                let _ = self.0.runtime.release_jsvalue(value);
            }
            return Err(RuntimeError::Invariant(
                "literal definition reply has wrong owner",
            ));
        }
        Ok(LiteralDefinitionStep::Complete(match result {
            NativeConversion::Value(InternalDefineResult::Defined) => {
                Completion::Return(crate::engine::value::JsValue::Undefined)
            }
            NativeConversion::Throw(value) => Completion::Throw(value),
            NativeConversion::Value(InternalDefineResult::RejectedOrdinary(_)) => {
                return Err(Error::new(ErrorKind::Type, "property is not configurable").into());
            }
            NativeConversion::Value(InternalDefineResult::RejectedProxyTrap) => {
                return Err(RuntimeError::Invariant(
                    "literal own definition unexpectedly entered Proxy trap",
                ));
            }
        }))
    }
}

// S11 all-domain protocol bound; inline completion stays allocation-free.
const _: () = assert!(std::mem::size_of::<LiteralDefinitionStep>() <= 64);

#[cfg(test)]
mod resident_tests {
    use super::*;
    use crate::engine::value::Value;
    #[test]
    fn literal_conversion_and_definition_reuse_one_owner() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let Value::Object(object) = context.eval("new Proxy({}, {})").unwrap() else {
            panic!("object")
        };
        let key = context.eval("({toString(){return 'x'}})").unwrap();
        let primitive = context.eval("'x'").unwrap();
        let LiteralDefinitionStep::Primitive { mut resume } = LiteralDefinitionStep::start(
            &runtime,
            context.realm,
            object.clone(),
            runtime.unroot_value(&key).unwrap(),
            JsValue::Int(42),
        )
        .unwrap() else {
            panic!("primitive request")
        };
        let address = (&*resume.0) as *const LiteralDefinitionState;
        assert_eq!(
            runtime
                .root_and_release_jsvalue(resume.take_primitive())
                .unwrap(),
            key
        );
        let LiteralDefinitionStep::Define { mut resume } = resume
            .resume(
                &runtime,
                Completion::Return(runtime.unroot_value(&primitive).unwrap()),
            )
            .unwrap()
        else {
            panic!("define request")
        };
        assert_eq!((&*resume.0) as *const LiteralDefinitionState, address);
        let (target, key, _descriptor) = resume.take_define();
        assert_eq!(target, object);
        assert_eq!(key, runtime.intern_property_key("x").unwrap());
        assert!(matches!(
            resume
                .defined(NativeConversion::Value(InternalDefineResult::Defined))
                .unwrap(),
            LiteralDefinitionStep::Complete(Completion::Return(
                crate::engine::value::JsValue::Undefined
            ))
        ));
    }
}
