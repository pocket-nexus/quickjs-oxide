//! Function source presentation keeps a selected native name across its string conversion.
use crate::engine::{
    api::{error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError},
    code::function::metadata::FunctionKind,
    heap::{ContextId, ObjectPayload},
    object::{ObjectRef, PropertyKey},
    value::{JsString, JsValue, Value, conversion::NativeConversion},
    vm::{Completion, call::NativeInvocation},
};
pub(crate) enum FunctionTextStep {
    Complete(Completion),
    Read { resume: FunctionTextResume },
    String { resume: FunctionTextResume },
}
pub(crate) struct FunctionTextResume(Box<FunctionTextResumeState>);
impl std::ops::Deref for FunctionTextResume {
    type Target = FunctionTextResumeState;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for FunctionTextResume {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
const _: () = assert!(std::mem::size_of::<FunctionTextResume>() <= 8);
pub(crate) struct FunctionTextResumeState {
    pending_effect: FunctionTextStepPending,
    function: ObjectRef,
    kind: FunctionKind,
    converted: bool,
}
impl FunctionTextStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        invocation: &NativeInvocation,
    ) -> Result<Self, RuntimeError> {
        let NativeInvocation::Call { this_value } = invocation else {
            return Err(RuntimeError::Invariant(
                "Function.prototype.toString did not receive a generic invocation",
            ));
        };
        let JsValue::Object(id) = this_value else {
            return Ok(Self::Complete(Completion::Throw(
                runtime.new_native_error_jsvalue(realm, NativeErrorKind::Type, "not a function")?,
            )));
        };
        let function = ObjectRef::from_borrowed_handle(runtime.clone(), *id)?;

        let (is_callable, source, function_kind) = {
            let state = runtime.0.state.borrow();
            let object = state.heap.object(function.object_id())?;
            match &object.payload {
                ObjectPayload::BytecodeFunction { bytecode, .. } => {
                    let bytecode = state.heap.function_bytecode(*bytecode)?;
                    (
                        true,
                        bytecode
                            .debug
                            .as_ref()
                            .and_then(|debug| debug.source.clone()),
                        bytecode.metadata.function_kind,
                    )
                }
                ObjectPayload::NativeFunction { .. } | ObjectPayload::BoundFunction { .. } => {
                    (true, None, FunctionKind::Normal)
                }
                ObjectPayload::Proxy(data) => (data.is_callable, None, FunctionKind::Normal),
                ObjectPayload::Ordinary
                | ObjectPayload::ArrayBuffer(_)
                | ObjectPayload::SharedArrayBuffer(_)
                | ObjectPayload::DataView(_)
                | ObjectPayload::TypedArray(_)
                | ObjectPayload::AsyncFunctionState(_)
                | ObjectPayload::RawJson
                | ObjectPayload::Promise(_)
                | ObjectPayload::Date(_)
                | ObjectPayload::RegExp(_)
                | ObjectPayload::Array { .. }
                | ObjectPayload::Arguments { .. }
                | ObjectPayload::ArrayIterator { .. }
                | ObjectPayload::IteratorHelper(_)
                | ObjectPayload::IteratorWrap(_)
                | ObjectPayload::AsyncFromSyncIterator(_)
                | ObjectPayload::IteratorConcat(_)
                | ObjectPayload::Map { .. }
                | ObjectPayload::MapIterator { .. }
                | ObjectPayload::Set { .. }
                | ObjectPayload::WeakMap { .. }
                | ObjectPayload::WeakSet { .. }
                | ObjectPayload::WeakRef { .. }
                | ObjectPayload::FinalizationRegistry(_)
                | ObjectPayload::SetIterator { .. }
                | ObjectPayload::ForInIterator(_)
                | ObjectPayload::Primitive(_)
                | ObjectPayload::GlobalObject { .. }
                | ObjectPayload::Error
                | ObjectPayload::StringIterator { .. }
                | ObjectPayload::RegExpStringIterator { .. }
                | ObjectPayload::Generator { .. }
                | ObjectPayload::AsyncGenerator(_) => (false, None, FunctionKind::Normal),
            }
        };
        if !is_callable {
            return Ok(Self::Complete(Completion::Throw(
                runtime.new_native_error_jsvalue(realm, NativeErrorKind::Type, "not a function")?,
            )));
        }
        if let Some(source) = source {
            return Ok(Self::Complete(Completion::Return(
                runtime.unroot_value(&Value::String(JsString::try_from_bytes(&source)?))?,
            )));
        }

        Ok({
            let __pending_field_object = function.clone();
            let __pending_field_key =
                runtime.pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Name)?;
            let __pending_field_resume = FunctionTextResume(Box::new(FunctionTextResumeState {
                pending_effect: FunctionTextStepPending::default(),
                function,
                kind: function_kind,
                converted: false,
            }));
            Self::request_read(
                __pending_field_object,
                __pending_field_key,
                __pending_field_resume,
            )
        })
    }
}
impl FunctionTextResume {
    pub(crate) fn resume(
        mut self,
        runtime: &Runtime,
        result: Completion,
    ) -> Result<FunctionTextStep, RuntimeError> {
        if self.converted {
            return Err(RuntimeError::Invariant("Function name repeated reply"));
        }
        let value = match result {
            Completion::Return(value) => value,
            Completion::Throw(value) => {
                return Ok(FunctionTextStep::Complete(Completion::Throw(value)));
            }
        };
        self.converted = true;
        if matches!(value, JsValue::Undefined) {
            self.string(runtime, NativeConversion::Value(JsString::from_static("")))
        } else {
            Ok({
                let __pending_field_value = value;
                let __pending_field_resume = self;
                FunctionTextStep::request_string(__pending_field_value, __pending_field_resume)
            })
        }
    }
    pub(crate) fn string(
        self,
        runtime: &Runtime,
        result: NativeConversion<JsString>,
    ) -> Result<FunctionTextStep, RuntimeError> {
        if !self.converted {
            return Err(RuntimeError::Invariant("Function name string before Get"));
        }
        let name = match result {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => {
                return Ok(FunctionTextStep::Complete(Completion::Throw(
                    runtime.into_jsvalue(value)?,
                )));
            }
        };
        let prefix = match self.kind {
            FunctionKind::Normal => "function ",
            FunctionKind::Generator => "function *",
            FunctionKind::Async => "async function ",
            FunctionKind::AsyncGenerator => "async function *",
        };
        let value = JsString::from_static(prefix)
            .try_concat(&name)?
            .try_concat(&JsString::from_static("() {\n    [native code]\n}"))?;
        drop(self.0.function);
        Ok(FunctionTextStep::Complete(Completion::Return(
            runtime.unroot_value(&Value::String(value))?,
        )))
    }
}
pub(crate) fn finish(
    runtime: &Runtime,
    realm: ContextId,
    mut step: FunctionTextStep,
) -> Result<Completion, RuntimeError> {
    loop {
        step = match step {
            FunctionTextStep::Complete(result) => return Ok(result),
            FunctionTextStep::Read { mut resume } => {
                let object = resume.take_read_object();
                let key = resume.take_read_key();
                resume.resume(
                    runtime,
                    runtime.get_property_in_realm(realm, &object, &key)?,
                )?
            }
            FunctionTextStep::String { mut resume } => {
                let value = runtime.root_and_release_jsvalue(resume.take_string_value())?;
                resume.string(runtime, runtime.native_to_js_string(realm, &value)?)?
            }
        };
    }
}

#[derive(Default)]
struct FunctionTextStepPending {
    read_object: Option<ObjectRef>,
    read_key: Option<PropertyKey>,
    string_value: Option<JsValue>,
}
impl FunctionTextStep {
    pub(crate) fn request_read(
        object: ObjectRef,
        key: PropertyKey,
        mut resume: FunctionTextResume,
    ) -> Self {
        resume.0.pending_effect.read_object = Some(object);
        resume.0.pending_effect.read_key = Some(key);
        Self::Read { resume }
    }
    pub(crate) fn request_string(value: JsValue, mut resume: FunctionTextResume) -> Self {
        resume.0.pending_effect.string_value = Some(value);
        Self::String { resume }
    }
}
impl FunctionTextResume {
    pub(crate) fn take_read_object(&mut self) -> ObjectRef {
        self.0
            .pending_effect
            .read_object
            .take()
            .expect("FunctionTextStep Read object")
    }
    pub(crate) fn take_read_key(&mut self) -> PropertyKey {
        self.0
            .pending_effect
            .read_key
            .take()
            .expect("FunctionTextStep Read key")
    }
    pub(crate) fn take_string_value(&mut self) -> JsValue {
        self.0
            .pending_effect
            .string_value
            .take()
            .expect("FunctionTextStep String value")
    }
}
const _: () = assert!(std::mem::size_of::<FunctionTextStep>() <= 64);

// S11 all-domain protocol bound; inline completion stays allocation-free.
const _: () = assert!(std::mem::size_of::<FunctionTextStep>() <= 64);
