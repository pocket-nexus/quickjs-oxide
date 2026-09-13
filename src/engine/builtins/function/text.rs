//! Function source presentation keeps a selected native name across its string conversion.
use crate::engine::{
    api::{error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError},
    code::function::metadata::FunctionKind,
    heap::{ContextId, ObjectPayload},
    object::{ObjectRef, PropertyKey},
    value::{JsString, Value, conversion::NativeConversion},
    vm::{Completion, call::NativeInvocation},
};
pub(crate) enum FunctionTextStep {
    Complete(Completion),
    Read {
        object: ObjectRef,
        key: PropertyKey,
        resume: FunctionTextResume,
    },
    String {
        value: Value,
        resume: FunctionTextResume,
    },
}
pub(crate) struct FunctionTextResume {
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
        let NativeInvocation::Call { this_value } = invocation.clone() else {
            return Err(RuntimeError::Invariant(
                "Function.prototype.toString did not receive a generic invocation",
            ));
        };
        let Value::Object(function) = this_value else {
            return Ok(Self::Complete(Completion::Throw(
                runtime.new_native_error(realm, NativeErrorKind::Type, "not a function")?,
            )));
        };

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
                runtime.new_native_error(realm, NativeErrorKind::Type, "not a function")?,
            )));
        }
        if let Some(source) = source {
            return Ok(Self::Complete(Completion::Return(Value::String(
                JsString::try_from_bytes(&source)?,
            ))));
        }

        Ok(Self::Read {
            object: function.clone(),
            key: runtime.intern_property_key("name")?,
            resume: FunctionTextResume {
                function,
                kind: function_kind,
                converted: false,
            },
        })
    }
}
impl FunctionTextResume {
    pub(crate) fn resume(mut self, result: Completion) -> Result<FunctionTextStep, RuntimeError> {
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
        if matches!(value, Value::Undefined) {
            self.string(NativeConversion::Value(JsString::from_static("")))
        } else {
            Ok(FunctionTextStep::String {
                value,
                resume: self,
            })
        }
    }
    pub(crate) fn string(
        self,
        result: NativeConversion<JsString>,
    ) -> Result<FunctionTextStep, RuntimeError> {
        if !self.converted {
            return Err(RuntimeError::Invariant("Function name string before Get"));
        }
        let name = match result {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => {
                return Ok(FunctionTextStep::Complete(Completion::Throw(value)));
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
        drop(self.function);
        Ok(FunctionTextStep::Complete(Completion::Return(
            Value::String(value),
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
            FunctionTextStep::Read {
                object,
                key,
                resume,
            } => resume.resume(runtime.get_property_in_realm(realm, &object, &key)?)?,
            FunctionTextStep::String { value, resume } => {
                resume.string(runtime.native_to_js_string(realm, &value)?)?
            }
        };
    }
}
