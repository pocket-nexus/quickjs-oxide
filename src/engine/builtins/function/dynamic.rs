//! Dynamic Function source fragments and eval results survive every conversion and prototype lookup.
use crate::engine::{
    api::{error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError},
    builtins::native::DynamicFunctionKind,
    code::dynamic_source::DynamicSourceBuilder,
    heap::ContextId,
    object::{ObjectRef, PropertyKey},
    value::{JsString, Value, conversion::NativeConversion},
    vm::{
        Completion,
        call::{NativeArguments, NativeInvocation},
    },
};
pub(crate) enum DynamicFunctionStep {
    Complete(Completion),
    String {
        value: Value,
        resume: DynamicFunctionResume,
    },
    Eval {
        source: JsString,
        resume: DynamicFunctionResume,
    },
    Read {
        receiver: Value,
        key: PropertyKey,
        resume: DynamicFunctionResume,
    },
}
enum Phase {
    Parameters,
    Body,
    Eval,
    Prototype,
}
pub(crate) struct DynamicFunctionResume {
    realm: ContextId,
    kind: DynamicFunctionKind,
    new_target: Value,
    arguments: Vec<Value>,
    index: usize,
    source: Option<DynamicSourceBuilder>,
    phase: Phase,
    value: Value,
}
impl DynamicFunctionStep {
    pub(crate) fn start(
        _runtime: &Runtime,
        realm: ContextId,
        kind: DynamicFunctionKind,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        let NativeInvocation::Construct { new_target } = invocation else {
            return Err(RuntimeError::Invariant(
                "Function constructor requires constructor-or-function invocation",
            ));
        };
        let mut source = DynamicSourceBuilder::new();
        source.push_str("(")?;
        if matches!(
            kind,
            DynamicFunctionKind::Async | DynamicFunctionKind::AsyncGenerator
        ) {
            source.push_str("async ")?;
        }
        source.push_str("function")?;
        if matches!(
            kind,
            DynamicFunctionKind::Generator | DynamicFunctionKind::AsyncGenerator
        ) {
            source.push_str("*")?;
        }
        source.push_str(" anonymous(")?;
        DynamicFunctionResume {
            realm,
            kind,
            new_target: new_target.clone(),
            arguments: arguments.readable[..arguments.actual_arg_count].to_vec(),
            index: 0,
            source: Some(source),
            phase: Phase::Parameters,
            value: Value::Undefined,
        }
        .parameter()
    }
}
impl DynamicFunctionResume {
    fn source(&mut self) -> Result<&mut DynamicSourceBuilder, RuntimeError> {
        self.source
            .as_mut()
            .ok_or(RuntimeError::Invariant("Function source builder missing"))
    }
    fn parameter(mut self) -> Result<DynamicFunctionStep, RuntimeError> {
        if self.index < self.arguments.len().saturating_sub(1) {
            if self.index != 0 {
                self.source()?.push_str(",")?;
            }
            return Ok(DynamicFunctionStep::String {
                value: self.arguments[self.index].clone(),
                resume: self,
            });
        }
        self.source()?.push_str("\n) {\n")?;
        if let Some(value) = self.arguments.last().cloned() {
            self.phase = Phase::Body;
            return Ok(DynamicFunctionStep::String {
                value,
                resume: self,
            });
        }
        self.eval()
    }
    pub(crate) fn string(
        mut self,
        result: NativeConversion<JsString>,
    ) -> Result<DynamicFunctionStep, RuntimeError> {
        let value = match result {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => {
                return Ok(DynamicFunctionStep::Complete(Completion::Throw(value)));
            }
        };
        self.source()?.push_js_string(&value)?;
        match self.phase {
            Phase::Parameters => {
                self.index += 1;
                self.parameter()
            }
            Phase::Body => self.eval(),
            _ => Err(RuntimeError::Invariant(
                "Function source string reply phase mismatch",
            )),
        }
    }
    fn eval(mut self) -> Result<DynamicFunctionStep, RuntimeError> {
        self.source()?.push_str("\n})")?;
        let source = self
            .source
            .take()
            .ok_or(RuntimeError::Invariant("Function source builder missing"))?
            .finish()?;
        self.phase = Phase::Eval;
        Ok(DynamicFunctionStep::Eval {
            source,
            resume: self,
        })
    }
    pub(crate) fn resume(
        mut self,
        runtime: &Runtime,
        result: Completion,
    ) -> Result<DynamicFunctionStep, RuntimeError> {
        let value = match result {
            Completion::Return(value) => value,
            Completion::Throw(value) => {
                return Ok(DynamicFunctionStep::Complete(Completion::Throw(value)));
            }
        };
        match self.phase {
            Phase::Eval => {
                if matches!(self.new_target, Value::Undefined) {
                    return Ok(DynamicFunctionStep::Complete(Completion::Return(value)));
                }
                self.value = value;
                self.phase = Phase::Prototype;
                Ok(DynamicFunctionStep::Read {
                    receiver: self.new_target.clone(),
                    key: runtime.intern_property_key("prototype")?,
                    resume: self,
                })
            }
            Phase::Prototype => {
                let prototype = if let Value::Object(object) = value {
                    object
                } else {
                    let realm =
                        match runtime.function_realm_from_value(self.realm, &self.new_target)? {
                            NativeConversion::Value(realm) => realm,
                            NativeConversion::Throw(value) => {
                                return Ok(DynamicFunctionStep::Complete(Completion::Throw(value)));
                            }
                        };
                    let prototype = {
                        let state = runtime.0.state.borrow();
                        let context = state.heap.context(realm)?;
                        match self.kind {
                            DynamicFunctionKind::Normal => context.function_prototype,
                            DynamicFunctionKind::Generator => context.generator.ok_or(RuntimeError::Invariant("dynamic GeneratorFunction realm has no Generator intrinsics"))?.function_prototype,
                            DynamicFunctionKind::Async => context.async_function.ok_or(RuntimeError::Invariant("dynamic AsyncFunction realm has no AsyncFunction intrinsics"))?.function_prototype,
                            DynamicFunctionKind::AsyncGenerator => context.async_generator.ok_or(RuntimeError::Invariant("dynamic AsyncGeneratorFunction realm has no AsyncGenerator intrinsics"))?.function_prototype,
                        }
                    };
                    ObjectRef::from_borrowed_handle(runtime.clone(), prototype)?
                };
                let Value::Object(function) = self.value else {
                    return Ok(DynamicFunctionStep::Complete(Completion::Throw(
                        runtime.new_native_error(
                            self.realm,
                            NativeErrorKind::Type,
                            "not an object",
                        )?,
                    )));
                };
                if !runtime.set_prototype_of(&function, Some(&prototype))? {
                    return Ok(DynamicFunctionStep::Complete(Completion::Throw(
                        runtime.new_native_error(
                            self.realm,
                            NativeErrorKind::Type,
                            "prototype is immutable",
                        )?,
                    )));
                }
                Ok(DynamicFunctionStep::Complete(Completion::Return(
                    Value::Object(function),
                )))
            }
            _ => Err(RuntimeError::Invariant(
                "Function constructor value reply phase mismatch",
            )),
        }
    }
}
pub(crate) fn finish(
    runtime: &Runtime,
    realm: ContextId,
    mut step: DynamicFunctionStep,
) -> Result<Completion, RuntimeError> {
    loop {
        step = match step {
            DynamicFunctionStep::Complete(result) => return Ok(result),
            DynamicFunctionStep::String { value, resume } => {
                resume.string(runtime.native_to_dynamic_source_fragment(realm, &value)?)?
            }
            DynamicFunctionStep::Eval { source, resume } => resume.resume(
                runtime,
                runtime.execute_indirect_string_eval(realm, &source)?,
            )?,
            DynamicFunctionStep::Read {
                receiver,
                key,
                resume,
            } => resume.resume(
                runtime,
                runtime.get_value_property_in_realm(realm, receiver, &key)?,
            )?,
        };
    }
}
