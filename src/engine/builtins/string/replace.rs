//! String replacement owns protocol lookup, conversions, and callback progress.

use super::super::replacement::{SubstitutionInput, SubstitutionMatch, SubstitutionStatus};
use super::scan_string_region;
use crate::engine::object::CallableRef;
use crate::engine::value::ReplacementStringBuffer;
use crate::engine::vm::{ToPrimitiveHint, call::DirectCallTarget};
use crate::engine::{
    api::{error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError},
    builtins::native::StringReplaceKind,
    heap::ContextId,
    object::{ObjectRef, PropertyKey, WellKnownSymbol},
    value::{JsString, Value, conversion::NativeConversion},
    vm::{
        Completion,
        call::{NativeArguments, NativeInvocation},
    },
};

pub(crate) enum StringReplaceStep {
    Complete(Completion),
    Read {
        object: ObjectRef,
        key: PropertyKey,
        resume: StringReplaceResume,
    },
    Primitive {
        value: Value,
        resume: StringReplaceResume,
    },
    Call {
        target: DirectCallTarget,
        receiver: Value,
        arguments: Vec<Value>,
        resume: StringReplaceResume,
    },
}
pub(crate) struct StringReplaceResume {
    realm: ContextId,
    selector: StringReplaceKind,
    receiver: Value,
    search_value: Value,
    replace_value: Value,
    phase: Phase,
}
enum Phase {
    Match,
    Flags,
    FlagsString,
    Method,
    ProtocolResult,
    Source(ReplacementStringBuffer),
    Search {
        output: ReplacementStringBuffer,
        source: JsString,
    },
    Replacement(ReplaceLoop),
    Callback {
        state: ReplaceLoop,
        position: usize,
    },
    CallbackString {
        state: ReplaceLoop,
        position: usize,
    },
}
struct ReplaceLoop {
    output: ReplacementStringBuffer,
    source: JsString,
    search: JsString,
    functional: Option<CallableRef>,
    replacement: Option<JsString>,
    end: usize,
    first: bool,
}
impl StringReplaceStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        selector: StringReplaceKind,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        let NativeInvocation::Call { this_value } = invocation else {
            return Err(RuntimeError::Invariant(
                "String replace family did not receive a generic-magic invocation",
            ));
        };
        if matches!(this_value, Value::Undefined | Value::Null) {
            return Ok(Self::Complete(Completion::Throw(
                runtime.new_native_error(
                    realm,
                    NativeErrorKind::Type,
                    "cannot convert to object",
                )?,
            )));
        }
        let search_value = arguments
            .readable
            .first()
            .ok_or(RuntimeError::Invariant(
                "String replace search argv was not padded",
            ))?
            .clone();
        let replace_value = arguments
            .readable
            .get(1)
            .ok_or(RuntimeError::Invariant(
                "String replace replacement argv was not padded",
            ))?
            .clone();
        let resume = StringReplaceResume {
            realm,
            selector,
            receiver: this_value.clone(),
            search_value,
            replace_value,
            phase: Phase::Method,
        };
        if let Value::Object(object) = &resume.search_value {
            if matches!(selector, StringReplaceKind::ReplaceAll) {
                return Ok(Self::Read {
                    object: object.clone(),
                    key: PropertyKey::from(runtime.well_known_symbol(WellKnownSymbol::Match)),
                    resume: StringReplaceResume {
                        phase: Phase::Match,
                        ..resume
                    },
                });
            }
            return resume.method(runtime);
        }
        Ok(resume.source())
    }
}
impl StringReplaceResume {
    fn method(self, runtime: &Runtime) -> Result<StringReplaceStep, RuntimeError> {
        let Value::Object(object) = &self.search_value else {
            return Err(RuntimeError::Invariant(
                "replacement protocol lost its object",
            ));
        };
        Ok(StringReplaceStep::Read {
            object: object.clone(),
            key: PropertyKey::from(runtime.well_known_symbol(WellKnownSymbol::Replace)),
            resume: Self {
                phase: Phase::Method,
                ..self
            },
        })
    }
    fn source(self) -> StringReplaceStep {
        // The allocation error is latched before observable fallback coercions.
        StringReplaceStep::Primitive {
            value: self.receiver.clone(),
            resume: Self {
                phase: Phase::Source(ReplacementStringBuffer::new(0)),
                ..self
            },
        }
    }
    pub(crate) fn resume(
        self,
        runtime: &Runtime,
        completion: Completion,
    ) -> Result<StringReplaceStep, RuntimeError> {
        let value = match completion {
            Completion::Return(value) => value,
            Completion::Throw(value) => {
                return Ok(StringReplaceStep::Complete(Completion::Throw(value)));
            }
        };
        let realm = self.realm;
        match self.phase {
            Phase::Match => {
                let Value::Object(object) = &self.search_value else {
                    return Err(RuntimeError::Invariant(
                        "replacement regexp check lost its object",
                    ));
                };
                let is_regexp = runtime.is_regexp_from_match(object, &value)?;
                if is_regexp {
                    Ok(StringReplaceStep::Read {
                        object: object.clone(),
                        key: runtime.intern_property_key("flags")?,
                        resume: Self {
                            phase: Phase::Flags,
                            ..self
                        },
                    })
                } else {
                    self.method(runtime)
                }
            }
            Phase::Flags => {
                if matches!(value, Value::Undefined | Value::Null) {
                    return Ok(StringReplaceStep::Complete(Completion::Throw(
                        runtime.new_native_error(
                            realm,
                            NativeErrorKind::Type,
                            "cannot convert to object",
                        )?,
                    )));
                }
                Ok(StringReplaceStep::Primitive {
                    value,
                    resume: Self {
                        phase: Phase::FlagsString,
                        ..self
                    },
                })
            }
            Phase::FlagsString => {
                let flags = match primitive_string(runtime, realm, value)? {
                    NativeConversion::Value(value) => value,
                    NativeConversion::Throw(value) => {
                        return Ok(StringReplaceStep::Complete(Completion::Throw(value)));
                    }
                };
                if !flags.utf16_units().any(|unit| unit == u16::from(b'g')) {
                    return Ok(StringReplaceStep::Complete(Completion::Throw(
                        runtime.new_native_error(
                            realm,
                            NativeErrorKind::Type,
                            "regexp must have the 'g' flag",
                        )?,
                    )));
                }
                self.method(runtime)
            }
            Phase::Method => {
                if matches!(value, Value::Undefined | Value::Null) {
                    return Ok(self.source());
                }
                let callable = match value {
                    Value::Object(object) => runtime.as_callable(&object)?,
                    _ => None,
                };
                let Some(callable) = callable else {
                    return Ok(StringReplaceStep::Complete(Completion::Throw(
                        runtime.new_native_error(realm, NativeErrorKind::Type, "not a function")?,
                    )));
                };
                let mut arguments = Vec::new();
                if arguments.try_reserve_exact(2).is_err() {
                    return Ok(StringReplaceStep::Complete(Completion::Throw(
                        runtime.new_native_error(
                            realm,
                            NativeErrorKind::Internal,
                            "out of memory",
                        )?,
                    )));
                }
                arguments.push(self.receiver.clone());
                arguments.push(self.replace_value.clone());
                Ok(StringReplaceStep::Call {
                    target: DirectCallTarget::Callable(callable),
                    receiver: self.search_value.clone(),
                    arguments,
                    resume: Self {
                        phase: Phase::ProtocolResult,
                        ..self
                    },
                })
            }
            Phase::ProtocolResult => Ok(StringReplaceStep::Complete(Completion::Return(value))),
            Phase::Source(output) => {
                let source = match primitive_string(runtime, realm, value)? {
                    NativeConversion::Value(value) => value,
                    NativeConversion::Throw(value) => {
                        return Ok(StringReplaceStep::Complete(Completion::Throw(value)));
                    }
                };
                Ok(StringReplaceStep::Primitive {
                    value: self.search_value.clone(),
                    resume: Self {
                        phase: Phase::Search { output, source },
                        ..self
                    },
                })
            }
            Phase::Search { output, source } => {
                let search = match primitive_string(runtime, realm, value)? {
                    NativeConversion::Value(value) => value,
                    NativeConversion::Throw(value) => {
                        return Ok(StringReplaceStep::Complete(Completion::Throw(value)));
                    }
                };
                let functional = match &self.replace_value {
                    Value::Object(object) => runtime.as_callable(object)?,
                    _ => None,
                };
                let state = ReplaceLoop {
                    output,
                    source,
                    search,
                    functional,
                    replacement: None,
                    end: 0,
                    first: true,
                };
                if state.functional.is_none() {
                    Ok(StringReplaceStep::Primitive {
                        value: self.replace_value.clone(),
                        resume: Self {
                            phase: Phase::Replacement(state),
                            ..self
                        },
                    })
                } else {
                    Self {
                        phase: Phase::ProtocolResult,
                        ..self
                    }
                    .next(runtime, state)
                }
            }
            Phase::Replacement(mut state) => {
                state.replacement = Some(match primitive_string(runtime, realm, value)? {
                    NativeConversion::Value(value) => value,
                    NativeConversion::Throw(value) => {
                        return Ok(StringReplaceStep::Complete(Completion::Throw(value)));
                    }
                });
                Self {
                    phase: Phase::ProtocolResult,
                    ..self
                }
                .next(runtime, state)
            }
            Phase::Callback { state, position } => Ok(StringReplaceStep::Primitive {
                value,
                resume: Self {
                    phase: Phase::CallbackString { state, position },
                    ..self
                },
            }),
            Phase::CallbackString {
                mut state,
                position,
            } => {
                let result = match primitive_string(runtime, realm, value)? {
                    NativeConversion::Value(value) => value,
                    NativeConversion::Throw(value) => {
                        return Ok(StringReplaceStep::Complete(Completion::Throw(value)));
                    }
                };
                state.output.append_js_string(&result);
                state.end = position + state.search.len();
                state.first = false;
                let next = Self {
                    phase: Phase::ProtocolResult,
                    ..self
                };
                if matches!(next.selector, StringReplaceKind::Replace) {
                    finish_buffer(runtime, realm, state)
                } else {
                    next.next(runtime, state)
                }
            }
        }
    }
    fn next(
        self,
        runtime: &Runtime,
        mut state: ReplaceLoop,
    ) -> Result<StringReplaceStep, RuntimeError> {
        loop {
            let position = if state.search.is_empty() {
                if state.first {
                    Some(0)
                } else if state.end >= state.source.len() {
                    None
                } else {
                    Some(state.end + 1)
                }
            } else {
                let stop = state.source.len().checked_sub(state.search.len());
                match stop {
                    Some(stop) if state.end <= stop => {
                        let start = i32::try_from(state.end).map_err(|_| {
                            RuntimeError::Invariant("String replace start exceeded signed range")
                        })?;
                        let stop = i32::try_from(stop).map_err(|_| {
                            RuntimeError::Invariant("String replace stop exceeded signed range")
                        })?;
                        usize::try_from(scan_string_region(
                            &state.source,
                            &state.search,
                            start,
                            stop,
                            1,
                        ))
                        .ok()
                    }
                    _ => None,
                }
            };
            let Some(position) = position else {
                if state.first {
                    return Ok(StringReplaceStep::Complete(Completion::Return(
                        Value::String(state.source),
                    )));
                }
                return finish_buffer(runtime, self.realm, state);
            };
            state
                .output
                .append_range(&state.source, state.end, position);
            if let Some(callable) = &state.functional {
                let position_value = Value::Int(i32::try_from(position).map_err(|_| {
                    RuntimeError::Invariant("String replace position exceeded signed range")
                })?);
                let mut arguments = Vec::new();
                if arguments.try_reserve_exact(3).is_err() {
                    return Ok(StringReplaceStep::Complete(Completion::Throw(
                        runtime.new_native_error(
                            self.realm,
                            NativeErrorKind::Internal,
                            "out of memory",
                        )?,
                    )));
                }
                arguments.push(Value::String(state.search.clone()));
                arguments.push(position_value);
                arguments.push(Value::String(state.source.clone()));
                return Ok(StringReplaceStep::Call {
                    target: DirectCallTarget::Callable(callable.clone()),
                    receiver: Value::Undefined,
                    arguments,
                    resume: Self {
                        phase: Phase::Callback { state, position },
                        ..self
                    },
                });
            }
            let substitution = runtime.append_get_substitution(
                self.realm,
                &mut state.output,
                SubstitutionInput {
                    matched: SubstitutionMatch::Converted(&state.search),
                    input: &state.source,
                    position,
                    captures: None,
                    named_captures: None,
                    replacement: state
                        .replacement
                        .as_ref()
                        .expect("non-functional replacement was not converted"),
                },
            )?;
            match substitution {
                Ok(SubstitutionStatus::Complete) => {}
                Ok(SubstitutionStatus::BufferFailed) => {
                    return match runtime.finish_replacement_buffer(self.realm, state.output)? {
                        NativeConversion::Value(_) => Err(RuntimeError::Invariant(
                            "failed replacement buffer unexpectedly completed",
                        )),
                        NativeConversion::Throw(value) => {
                            Ok(StringReplaceStep::Complete(Completion::Throw(value)))
                        }
                    };
                }
                Err(value) => return Ok(StringReplaceStep::Complete(Completion::Throw(value))),
            }
            state.end = position + state.search.len();
            state.first = false;
            if matches!(self.selector, StringReplaceKind::Replace) {
                return finish_buffer(runtime, self.realm, state);
            }
        }
    }
}
fn primitive_string(
    runtime: &Runtime,
    realm: ContextId,
    value: Value,
) -> Result<NativeConversion<JsString>, RuntimeError> {
    if matches!(value, Value::Object(_)) {
        return Err(RuntimeError::Invariant(
            "String replacement conversion received an object",
        ));
    }
    runtime.native_to_js_string(realm, &value)
}
fn finish_buffer(
    runtime: &Runtime,
    realm: ContextId,
    mut state: ReplaceLoop,
) -> Result<StringReplaceStep, RuntimeError> {
    state
        .output
        .append_range(&state.source, state.end, state.source.len());
    Ok(StringReplaceStep::Complete(
        match runtime.finish_replacement_buffer(realm, state.output)? {
            NativeConversion::Value(value) => Completion::Return(Value::String(value)),
            NativeConversion::Throw(value) => Completion::Throw(value),
        },
    ))
}
impl Runtime {
    pub(crate) fn call_string_prototype_replace(
        &self,
        realm: ContextId,
        selector: StringReplaceKind,
        invocation: NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        let mut step = StringReplaceStep::start(self, realm, selector, &invocation, arguments)?;
        loop {
            step = match step {
                StringReplaceStep::Complete(result) => return Ok(result),
                StringReplaceStep::Read {
                    object,
                    key,
                    resume,
                } => resume.resume(self, self.get_property_in_realm(realm, &object, &key)?)?,
                StringReplaceStep::Primitive { value, resume } => {
                    let result = if matches!(value, Value::Object(_)) {
                        self.to_primitive(realm, value, ToPrimitiveHint::String)?
                    } else {
                        Completion::Return(value)
                    };
                    resume.resume(self, result)?
                }
                StringReplaceStep::Call {
                    target,
                    receiver,
                    arguments,
                    resume,
                } => {
                    let DirectCallTarget::Callable(callable) = target else {
                        return Err(RuntimeError::Invariant(
                            "String replacement requested an invalid call target",
                        ));
                    };
                    resume.resume(
                        self,
                        self.call_internal(realm, &callable, receiver, &arguments)?,
                    )?
                }
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pending_replacer_roots_callback_and_receiver_until_abandonment() {
        let runtime = Runtime::new();
        let weak = std::rc::Rc::downgrade(&runtime.0);
        let mut context = runtime.new_context();
        let receiver = runtime.new_object(None).unwrap();
        let receiver_id = receiver.object_id();
        let callback = context.eval("(function(){return 'x'})").unwrap();
        let Value::Object(function) = &callback else {
            panic!("expected callback")
        };
        let callback_id = function.object_id();
        let invocation = NativeInvocation::Call {
            this_value: Value::Object(receiver),
        };
        let arguments = NativeArguments {
            actual_arg_count: 2,
            readable: vec![Value::String(JsString::from_static("a")), callback],
        };
        let StringReplaceStep::Primitive { resume, .. } = StringReplaceStep::start(
            &runtime,
            context.realm,
            StringReplaceKind::ReplaceAll,
            &invocation,
            &arguments,
        )
        .unwrap() else {
            panic!("expected source conversion")
        };
        drop(invocation);
        drop(arguments);
        let StringReplaceStep::Primitive { resume, .. } = resume
            .resume(
                &runtime,
                Completion::Return(Value::String(JsString::from_static("aa"))),
            )
            .unwrap()
        else {
            panic!("expected search conversion")
        };
        let StringReplaceStep::Call { resume, .. } = resume
            .resume(
                &runtime,
                Completion::Return(Value::String(JsString::from_static("a"))),
            )
            .unwrap()
        else {
            panic!("expected replacer call")
        };
        runtime.run_gc().unwrap();
        for id in [receiver_id, callback_id] {
            assert!(runtime.0.state.borrow().heap.object(id).is_ok());
        }
        drop(resume);
        runtime.run_gc().unwrap();
        for id in [receiver_id, callback_id] {
            assert!(runtime.0.state.borrow().heap.object(id).is_err());
        }
        drop(context);
        drop(runtime);
        assert!(weak.upgrade().is_none());
    }
}
