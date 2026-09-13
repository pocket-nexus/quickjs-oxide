//! Builtin and abstract RegExp execution.

use crate::engine::api::error::NativeErrorKind;
use crate::engine::api::runtime::Runtime;
use crate::engine::api::runtime_error::RuntimeError;
use crate::engine::builtins::native::RegExpNativeKind;
use crate::engine::heap::ContextId;

use crate::engine::object::{CompleteOrdinaryPropertyDescriptor, ObjectRef, PropertyKey};
use crate::engine::value::conversion::NativeConversion;
use crate::engine::value::{JsString, Value};
use crate::engine::vm::{Completion, ToPrimitiveHint};

use crate::engine::vm::call::{DirectCallTarget, NativeArguments, NativeInvocation};
use crate::regexp::{ExecError, RegExpFlags, execute_with_interrupt};

impl Runtime {
    pub(crate) fn call_regexp_exec_native(
        &self,
        realm: ContextId,
        kind: RegExpNativeKind,
        invocation: NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        finish(
            self,
            realm,
            RegExpExecStep::start(self, realm, kind, &invocation, arguments)?,
        )
    }
    pub(crate) fn regexp_exec_abstract(
        &self,
        realm: ContextId,
        regexp: Value,
        input: Value,
    ) -> Result<Completion, RuntimeError> {
        finish(
            self,
            realm,
            RegExpExecStep::abstract_exec(self, realm, regexp, input)?,
        )
    }
    fn finish_builtin_regexp_exec(
        &self,
        realm: ContextId,
        object: &ObjectRef,
        input: JsString,
        last_index: u64,
    ) -> Result<Completion, RuntimeError> {
        let this_value = &Value::Object(object.clone());
        // QuickJS keeps the branded RegExp identity across both coercions, but
        // reads `re->bytecode` only afterwards. Either conversion may call the
        // legacy `compile()` method, so snapshot the current program and flags
        // only after those observable calls have completed.
        let current = self
            .genuine_regexp(this_value)?
            .ok_or(RuntimeError::Invariant(
                "branded RegExp lost its compiled payload during exec coercion",
            ))?;
        let program = current.program;
        let flags = program.flags();
        let updates_last_index =
            flags.contains(RegExpFlags::GLOBAL) || flags.contains(RegExpFlags::STICKY);
        let start = if updates_last_index { last_index } else { 0 };
        let input_units = input.utf16_units().collect::<Vec<_>>();

        let matched = if start > input_units.len() as u64 {
            None
        } else {
            match execute_with_interrupt(
                program.as_ref(),
                &input_units,
                usize::try_from(start).expect("RegExp start was bounded by String length"),
                // The runtime interrupt callback is not exposed at this layer
                // yet.  Keep the executor boundary interrupt-aware now so a
                // later host hook is a closure substitution rather than a
                // semantic rewrite of builtin exec.
                || false,
            ) {
                Ok(value) => value,
                Err(ExecError::OutOfMemory) => {
                    return Ok(Completion::Throw(self.new_native_error(
                        realm,
                        NativeErrorKind::Internal,
                        "out of memory in regexp execution",
                    )?));
                }
                Err(ExecError::Interrupted) => {
                    return Ok(Completion::Throw(self.new_native_error(
                        realm,
                        NativeErrorKind::Internal,
                        "interrupted",
                    )?));
                }
                Err(ExecError::InvalidProgram(_)) => {
                    return Err(RuntimeError::Invariant(
                        "compiled RegExp program failed executor validation",
                    ));
                }
                Err(ExecError::StartOutOfBounds { .. }) => {
                    return Err(RuntimeError::Invariant(
                        "bounded RegExp start was rejected by executor",
                    ));
                }
            }
        };

        let Some(matched) = matched else {
            if updates_last_index
                && let Some(exception) = self.set_regexp_last_index(realm, object, 0)?
            {
                return Ok(Completion::Throw(exception));
            }
            return Ok(Completion::Return(Value::Null));
        };

        let complete = matched.capture(0).ok_or(RuntimeError::Invariant(
            "successful RegExp execution omitted capture zero",
        ))?;
        if updates_last_index {
            let end = i32::try_from(complete.end).map_err(|_| {
                RuntimeError::Invariant("RegExp match end exceeded signed String range")
            })?;
            // This write happens before any result/indices allocation.
            if let Some(exception) = self.set_regexp_last_index(realm, object, end)? {
                return Ok(Completion::Throw(exception));
            }
        }

        self.build_regexp_result(realm, input, program, matched)
            .map(Completion::Return)
    }

    fn regexp_last_index_value(&self, object: &ObjectRef) -> Result<Value, RuntimeError> {
        let key = self.intern_property_key("lastIndex")?;
        let descriptor = self
            .get_own_property(object, &key)?
            .ok_or(RuntimeError::Invariant(
                "genuine RegExp object had no lastIndex property",
            ))?;
        let CompleteOrdinaryPropertyDescriptor::Data { value, .. } = descriptor else {
            return Err(RuntimeError::Invariant(
                "RegExp lastIndex became an accessor",
            ));
        };
        Ok(value)
    }

    pub(crate) fn set_regexp_last_index(
        &self,
        realm: ContextId,
        object: &ObjectRef,
        value: i32,
    ) -> Result<Option<Value>, RuntimeError> {
        let key = self.intern_property_key("lastIndex")?;
        self.set_property_or_throw(realm, object, &key, Value::Int(value))
    }
}

/// Shared RegExpExec owns the selected exec method and branded fallback state.
pub(crate) enum RegExpExecStep {
    Complete(Completion),
    Read {
        receiver: Value,
        key: PropertyKey,
        resume: RegExpExecResume,
    },
    Primitive {
        value: Value,
        hint: ToPrimitiveHint,
        resume: RegExpExecResume,
    },
    Call {
        target: DirectCallTarget,
        receiver: Value,
        arguments: Vec<Value>,
        resume: RegExpExecResume,
    },
}
pub(crate) struct RegExpExecResume {
    realm: ContextId,
    regexp: Value,
    input: Value,
    test: bool,
    phase: ExecPhase,
}
enum ExecPhase {
    Method,
    Called,
    Input,
    LastIndex(JsString),
}
impl RegExpExecStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        kind: RegExpNativeKind,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        let NativeInvocation::Call { this_value } = invocation else {
            return Err(RuntimeError::Invariant(
                "RegExp exec/test did not receive a generic invocation",
            ));
        };
        let input = arguments
            .readable
            .first()
            .ok_or(RuntimeError::Invariant(
                "RegExp exec/test input argv was not padded",
            ))?
            .clone();
        match kind {
            RegExpNativeKind::Exec => RegExpExecResume {
                realm,
                regexp: this_value.clone(),
                input,
                test: false,
                phase: ExecPhase::Input,
            }
            .builtin(runtime),
            RegExpNativeKind::Test => {
                Self::abstract_start(runtime, realm, this_value.clone(), input, true)
            }
            _ => Err(RuntimeError::Invariant(
                "non-exec RegExp selector reached exec dispatch",
            )),
        }
    }
    pub(crate) fn abstract_exec(
        runtime: &Runtime,
        realm: ContextId,
        regexp: Value,
        input: Value,
    ) -> Result<Self, RuntimeError> {
        Self::abstract_start(runtime, realm, regexp, input, false)
    }
    fn abstract_start(
        runtime: &Runtime,
        realm: ContextId,
        regexp: Value,
        input: Value,
        test: bool,
    ) -> Result<Self, RuntimeError> {
        let key = runtime.intern_property_key("exec")?;
        if matches!(regexp, Value::Null | Value::Undefined) {
            let base = if matches!(regexp, Value::Null) {
                "null"
            } else {
                "undefined"
            };
            return Ok(Self::Complete(Completion::Throw(
                runtime.new_native_error(
                    realm,
                    NativeErrorKind::Type,
                    &format!("cannot read property 'exec' of {base}"),
                )?,
            )));
        }
        Ok(Self::Read {
            receiver: regexp.clone(),
            key,
            resume: RegExpExecResume {
                realm,
                regexp,
                input,
                test,
                phase: ExecPhase::Method,
            },
        })
    }
}
impl RegExpExecResume {
    fn complete(&self, result: Completion) -> RegExpExecStep {
        RegExpExecStep::Complete(match result {
            Completion::Return(value) if self.test => {
                Completion::Return(Value::Bool(!matches!(value, Value::Null)))
            }
            result => result,
        })
    }
    fn builtin(self, runtime: &Runtime) -> Result<RegExpExecStep, RuntimeError> {
        if !matches!(&self.regexp, Value::Object(_))
            || runtime.genuine_regexp(&self.regexp)?.is_none()
        {
            return Ok(self.complete(Completion::Throw(runtime.new_native_error(
                self.realm,
                NativeErrorKind::Type,
                "RegExp object expected",
            )?)));
        }
        Ok(RegExpExecStep::Primitive {
            value: self.input.clone(),
            hint: ToPrimitiveHint::String,
            resume: Self {
                phase: ExecPhase::Input,
                ..self
            },
        })
    }
    pub(crate) fn resume(
        self,
        runtime: &Runtime,
        result: Completion,
    ) -> Result<RegExpExecStep, RuntimeError> {
        let value = match result {
            Completion::Return(value) => value,
            Completion::Throw(value) => return Ok(self.complete(Completion::Throw(value))),
        };
        match self.phase {
            ExecPhase::Method => {
                let callable = match &value {
                    Value::Object(object) => runtime.as_callable(object)?,
                    _ => None,
                };
                let Some(callable) = callable else {
                    return self.builtin(runtime);
                };
                let mut arguments = Vec::new();
                if arguments.try_reserve_exact(1).is_err() {
                    return Ok(self.complete(Completion::Throw(runtime.new_native_error(
                        self.realm,
                        NativeErrorKind::Internal,
                        "out of memory",
                    )?)));
                }
                arguments.push(self.input.clone());
                Ok(RegExpExecStep::Call {
                    target: DirectCallTarget::Callable(callable),
                    receiver: self.regexp.clone(),
                    arguments,
                    resume: Self {
                        phase: ExecPhase::Called,
                        ..self
                    },
                })
            }
            ExecPhase::Called => {
                if matches!(value, Value::Object(_) | Value::Null) {
                    Ok(self.complete(Completion::Return(value)))
                } else {
                    Ok(self.complete(Completion::Throw(runtime.new_native_error(
                        self.realm,
                        NativeErrorKind::Type,
                        "RegExp exec method must return an object or null",
                    )?)))
                }
            }
            ExecPhase::Input => {
                if matches!(value, Value::Object(_)) {
                    return Err(RuntimeError::Invariant(
                        "RegExp input conversion returned an object",
                    ));
                }
                let input = match runtime.native_to_js_string(self.realm, &value)? {
                    NativeConversion::Value(input) => input,
                    NativeConversion::Throw(value) => {
                        return Ok(self.complete(Completion::Throw(value)));
                    }
                };
                let Value::Object(object) = &self.regexp else {
                    return Err(RuntimeError::Invariant(
                        "RegExp input conversion lost its branded receiver",
                    ));
                };
                let value = runtime.regexp_last_index_value(object)?;
                Ok(RegExpExecStep::Primitive {
                    value,
                    hint: ToPrimitiveHint::Number,
                    resume: Self {
                        phase: ExecPhase::LastIndex(input),
                        ..self
                    },
                })
            }
            ExecPhase::LastIndex(input) => {
                if matches!(value, Value::Object(_)) {
                    return Err(RuntimeError::Invariant(
                        "RegExp lastIndex conversion returned an object",
                    ));
                }
                let last_index = match runtime.native_to_length(self.realm, &value)? {
                    NativeConversion::Value(index) => index,
                    NativeConversion::Throw(value) => {
                        return Ok(RegExpExecStep::Complete(Completion::Throw(value)));
                    }
                };
                let Value::Object(object) = &self.regexp else {
                    return Err(RuntimeError::Invariant(
                        "RegExp lastIndex conversion lost its branded receiver",
                    ));
                };
                let result =
                    runtime.finish_builtin_regexp_exec(self.realm, object, input, last_index)?;
                Ok(Self {
                    phase: ExecPhase::Called,
                    ..self
                }
                .complete(result))
            }
        }
    }
}
fn finish(
    runtime: &Runtime,
    realm: ContextId,
    mut step: RegExpExecStep,
) -> Result<Completion, RuntimeError> {
    loop {
        step = match step {
            RegExpExecStep::Complete(result) => return Ok(result),
            RegExpExecStep::Read {
                receiver,
                key,
                resume,
            } => resume.resume(
                runtime,
                runtime.get_value_property_in_realm(realm, receiver, &key)?,
            )?,
            RegExpExecStep::Primitive {
                value,
                hint,
                resume,
            } => {
                let result = if matches!(value, Value::Object(_)) {
                    runtime.to_primitive(realm, value, hint)?
                } else {
                    Completion::Return(value)
                };
                resume.resume(runtime, result)?
            }
            RegExpExecStep::Call {
                target,
                receiver,
                arguments,
                resume,
            } => {
                let DirectCallTarget::Callable(callable) = target else {
                    return Err(RuntimeError::Invariant(
                        "RegExp exec requested an invalid call target",
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
