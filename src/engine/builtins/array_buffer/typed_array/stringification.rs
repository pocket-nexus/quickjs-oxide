//! `%TypedArray%.prototype` stringification algorithms.
//!
//! Pinned QuickJS uses a dedicated TypedArray kernel rather than the generic
//! Array join path. It validates the branded view up front, snapshots the old
//! element count, and then keeps resizable-buffer changes observable without
//! consulting ordinary `length` or indexed properties.

use crate::engine::{
    api::{error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError},
    builtins::native::ArrayJoinKind,
    heap::ContextId,
    object::{ObjectRef, PropertyKey},
    value::{JsString, JsStringBuilder, Value, conversion::NativeConversion},
    vm::{
        Completion, ToPrimitiveHint,
        call::{DirectCallTarget, NativeArguments, NativeInvocation},
    },
};

#[cfg(test)]
mod tests;

impl Runtime {
    pub(crate) fn call_typed_array_join(
        &self,
        realm: ContextId,
        kind: ArrayJoinKind,
        invocation: NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        self.call_typed_array_join_with_string_limit(
            realm,
            kind,
            invocation,
            arguments,
            JsString::MAX_LEN,
        )
    }

    fn call_typed_array_join_with_string_limit(
        &self,
        realm: ContextId,
        kind: ArrayJoinKind,
        invocation: NativeInvocation,
        arguments: &NativeArguments,
        string_limit: usize,
    ) -> Result<Completion, RuntimeError> {
        finish(
            self,
            realm,
            TypedStringStep::start_with_limit(
                self,
                realm,
                kind,
                &invocation,
                arguments,
                string_limit,
            )?,
        )
    }
}
pub(crate) enum TypedStringStep {
    Complete(Completion),
    Primitive {
        value: Value,
        resume: TypedStringResume,
    },
    Read {
        receiver: Value,
        key: PropertyKey,
        resume: TypedStringResume,
    },
    Call {
        target: DirectCallTarget,
        receiver: Value,
        arguments: Vec<Value>,
        resume: TypedStringResume,
    },
}
pub(crate) struct TypedStringResume {
    realm: ContextId,
    target: ObjectRef,
    kind: ArrayJoinKind,
    initial_length: u64,
    current_length: u64,
    index: u64,
    separator: JsString,
    output: JsStringBuilder,
    phase: Phase,
}
enum Phase {
    Separator,
    LocaleMethod(Value),
    LocaleResult,
    Element,
}
impl TypedStringStep {
    #[cfg(feature = "stack-vm")]
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        kind: ArrayJoinKind,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        Self::start_with_limit(
            runtime,
            realm,
            kind,
            invocation,
            arguments,
            JsString::MAX_LEN,
        )
    }
    fn start_with_limit(
        runtime: &Runtime,
        realm: ContextId,
        kind: ArrayJoinKind,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
        limit: usize,
    ) -> Result<Self, RuntimeError> {
        let NativeInvocation::Call { this_value } = invocation else {
            return Err(RuntimeError::Invariant(
                "TypedArray stringification received a constructor invocation",
            ));
        };
        let target = match runtime.require_typed_array(realm, this_value.clone())? {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => return Ok(Self::Complete(Completion::Throw(value))),
        };
        let length = match runtime.typed_array_validated_length(realm, &target)? {
            NativeConversion::Value(value) => u64::from(value),
            NativeConversion::Throw(value) => return Ok(Self::Complete(Completion::Throw(value))),
        };
        let state = TypedStringResume {
            realm,
            target,
            kind,
            initial_length: length,
            current_length: length,
            index: 0,
            separator: JsString::from_static(","),
            output: JsStringBuilder::with_limit(0, limit),
            phase: Phase::Separator,
        };
        if matches!(kind, ArrayJoinKind::Join)
            && arguments.actual_arg_count != 0
            && !matches!(arguments.readable.first(), Some(Value::Undefined))
        {
            return Ok(Self::Primitive {
                value: arguments
                    .readable
                    .first()
                    .ok_or(RuntimeError::Invariant(
                        "TypedArray.join separator argv was not padded",
                    ))?
                    .clone(),
                resume: state,
            });
        }
        state.next(runtime)
    }
}
impl TypedStringResume {
    fn next(mut self, runtime: &Runtime) -> Result<TypedStringStep, RuntimeError> {
        while self.index < self.initial_length.min(self.current_length) {
            if self.index != 0 {
                self.output.push_js_string(&self.separator)?;
            }
            let Some(element) = runtime.typed_array_read_index(&self.target, self.index)? else {
                self.index += 1;
                continue;
            };
            match self.kind {
                ArrayJoinKind::Join => {
                    // Integer-indexed storage returns only primitive numeric values.
                    let string = match runtime.native_to_js_string(self.realm, &element)? {
                        NativeConversion::Value(value) => value,
                        NativeConversion::Throw(value) => {
                            return Ok(TypedStringStep::Complete(Completion::Throw(value)));
                        }
                    };
                    self.output.push_js_string(&string)?;
                    self.index += 1;
                }
                ArrayJoinKind::ToLocaleString => {
                    let key = runtime.intern_property_key("toLocaleString")?;
                    self.phase = Phase::LocaleMethod(element.clone());
                    return Ok(TypedStringStep::Read {
                        receiver: element,
                        key,
                        resume: self,
                    });
                }
            }
        }
        for _ in self.current_length.max(1)..self.initial_length {
            self.output.push_js_string(&self.separator)?;
        }
        Ok(TypedStringStep::Complete(Completion::Return(
            Value::String(self.output.finish()?),
        )))
    }
    pub(crate) fn resume(
        mut self,
        runtime: &Runtime,
        result: Completion,
    ) -> Result<TypedStringStep, RuntimeError> {
        let value = match result {
            Completion::Return(value) => value,
            Completion::Throw(value) => {
                return Ok(TypedStringStep::Complete(Completion::Throw(value)));
            }
        };
        match self.phase {
            Phase::Separator => {
                self.separator = match runtime.native_to_js_string(self.realm, &value)? {
                    NativeConversion::Value(value) => value,
                    NativeConversion::Throw(value) => {
                        return Ok(TypedStringStep::Complete(Completion::Throw(value)));
                    }
                };
                self.current_length = u64::from(runtime.typed_array_state(&self.target)?.length);
                self.next(runtime)
            }
            Phase::LocaleMethod(receiver) => {
                let callable = match value {
                    Value::Object(object) => runtime.as_callable(&object)?,
                    _ => None,
                };
                let Some(callable) = callable else {
                    return Ok(TypedStringStep::Complete(Completion::Throw(
                        runtime.new_native_error(
                            self.realm,
                            NativeErrorKind::Type,
                            "not a function",
                        )?,
                    )));
                };
                self.phase = Phase::LocaleResult;
                Ok(TypedStringStep::Call {
                    target: DirectCallTarget::Callable(callable),
                    receiver,
                    arguments: Vec::new(),
                    resume: self,
                })
            }
            Phase::LocaleResult => {
                self.phase = Phase::Element;
                Ok(TypedStringStep::Primitive {
                    value,
                    resume: self,
                })
            }
            Phase::Element => {
                let string = match runtime.native_to_js_string(self.realm, &value)? {
                    NativeConversion::Value(value) => value,
                    NativeConversion::Throw(value) => {
                        return Ok(TypedStringStep::Complete(Completion::Throw(value)));
                    }
                };
                self.output.push_js_string(&string)?;
                self.index += 1;
                self.next(runtime)
            }
        }
    }
}
fn finish(
    runtime: &Runtime,
    realm: ContextId,
    mut step: TypedStringStep,
) -> Result<Completion, RuntimeError> {
    loop {
        step = match step {
            TypedStringStep::Complete(result) => return Ok(result),
            TypedStringStep::Primitive { value, resume } => {
                let result = if matches!(value, Value::Object(_)) {
                    runtime.to_primitive(realm, value, ToPrimitiveHint::String)?
                } else {
                    Completion::Return(value)
                };
                resume.resume(runtime, result)?
            }
            TypedStringStep::Read {
                receiver,
                key,
                resume,
            } => resume.resume(
                runtime,
                runtime.get_value_property_in_realm(realm, receiver, &key)?,
            )?,
            TypedStringStep::Call {
                target,
                receiver,
                arguments,
                resume,
            } => {
                let DirectCallTarget::Callable(callable) = target else {
                    return Err(RuntimeError::Invariant(
                        "TypedArray stringification requested invalid call target",
                    ));
                };
                resume.resume(
                    runtime,
                    runtime.call_internal(realm, &callable, receiver, &arguments)?,
                )?
            }
        };
    }
}
