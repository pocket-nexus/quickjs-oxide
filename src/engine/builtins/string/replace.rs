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
    PreparedRead {
        read: crate::engine::object::OrdinaryRead,
        key: PropertyKey,
        resume: StringReplaceResume,
    },
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
    output: Option<ReplacementStringBuffer>,
    source: Option<JsString>,
    cursor: Option<ReplaceLoop>,
}
#[derive(Clone, Copy)]
enum Phase {
    Match,
    Flags,
    FlagsString,
    Method,
    ProtocolResult,
    Source,
    Search,
    Replacement,
    Callback { position: usize },
    CallbackString { position: usize },
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
// The resident resume owns all accumulated state. Only this small next effect
// crosses local phases; an owned Step is formed at a real waiting boundary.
enum StringReplaceAction {
    Complete(Completion),
    Read(PropertyKey),
    Primitive(Value),
    Call {
        target: DirectCallTarget,
        receiver: Value,
        arguments: Vec<Value>,
    },
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
        let mut resume = StringReplaceResume {
            realm,
            selector,
            receiver: this_value.clone(),
            search_value,
            replace_value,
            phase: Phase::Method,
            output: None,
            source: None,
            cursor: None,
        };
        let action = if matches!(resume.search_value, Value::Object(_)) {
            if matches!(selector, StringReplaceKind::ReplaceAll) {
                resume.phase = Phase::Match;
                StringReplaceAction::Read(PropertyKey::from(
                    runtime.well_known_symbol(WellKnownSymbol::Match),
                ))
            } else {
                resume.method(runtime)?
            }
        } else {
            resume.source()
        };
        resume.deliver(runtime, action)
    }
}
impl StringReplaceResume {
    fn method(&mut self, runtime: &Runtime) -> Result<StringReplaceAction, RuntimeError> {
        if !matches!(self.search_value, Value::Object(_)) {
            return Err(RuntimeError::Invariant(
                "replacement protocol lost its object",
            ));
        }
        self.phase = Phase::Method;
        Ok(StringReplaceAction::Read(PropertyKey::from(
            runtime.well_known_symbol(WellKnownSymbol::Replace),
        )))
    }
    fn source(&mut self) -> StringReplaceAction {
        // Latch buffer allocation failure before observable fallback coercions.
        self.output = Some(ReplacementStringBuffer::new(0));
        self.phase = Phase::Source;
        StringReplaceAction::Primitive(self.receiver.clone())
    }
    fn deliver(
        mut self,
        runtime: &Runtime,
        mut action: StringReplaceAction,
    ) -> Result<StringReplaceStep, RuntimeError> {
        loop {
            action = match action {
                StringReplaceAction::Complete(result) => {
                    return Ok(StringReplaceStep::Complete(result));
                }
                StringReplaceAction::Read(key) => {
                    let Value::Object(object) = &self.search_value else {
                        return Err(RuntimeError::Invariant("replacement read lost its object"));
                    };
                    match runtime.prepare_ordinary_read_borrowed(
                        object,
                        &key,
                        &self.search_value,
                    )? {
                        crate::engine::object::OrdinaryRead::Complete(value) => {
                            #[cfg(all(feature = "stack-vm", feature = "profiling"))]
                            crate::engine::api::profiling::record_owned_execution_event(
                                "stringreplace_read_local",
                            );
                            self.advance(
                                runtime,
                                Completion::Return(value.unwrap_or(Value::Undefined)),
                            )?
                        }
                        read => {
                            return Ok(StringReplaceStep::PreparedRead {
                                read,
                                key,
                                resume: self,
                            });
                        }
                    }
                }
                StringReplaceAction::Primitive(value) if !matches!(value, Value::Object(_)) => {
                    #[cfg(all(feature = "stack-vm", feature = "profiling"))]
                    crate::engine::api::profiling::record_owned_execution_event(
                        "stringreplace_primitive_local",
                    );
                    self.advance(runtime, Completion::Return(value))?
                }
                StringReplaceAction::Primitive(value) => {
                    return Ok(StringReplaceStep::Primitive {
                        value,
                        resume: self,
                    });
                }
                StringReplaceAction::Call {
                    target,
                    receiver,
                    arguments,
                } => {
                    return Ok(StringReplaceStep::Call {
                        target,
                        receiver,
                        arguments,
                        resume: self,
                    });
                }
            };
        }
    }
    pub(crate) fn resume(
        mut self,
        runtime: &Runtime,
        completion: Completion,
    ) -> Result<StringReplaceStep, RuntimeError> {
        let action = self.advance(runtime, completion)?;
        self.deliver(runtime, action)
    }
    fn advance(
        &mut self,
        runtime: &Runtime,
        completion: Completion,
    ) -> Result<StringReplaceAction, RuntimeError> {
        let value = match completion {
            Completion::Return(value) => value,
            Completion::Throw(value) => {
                return Ok(StringReplaceAction::Complete(Completion::Throw(value)));
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
                if runtime.is_regexp_from_match(object, &value)? {
                    self.phase = Phase::Flags;
                    Ok(StringReplaceAction::Read(
                        runtime.intern_property_key("flags")?,
                    ))
                } else {
                    self.method(runtime)
                }
            }
            Phase::Flags => {
                if matches!(value, Value::Undefined | Value::Null) {
                    return Ok(StringReplaceAction::Complete(Completion::Throw(
                        runtime.new_native_error(
                            realm,
                            NativeErrorKind::Type,
                            "cannot convert to object",
                        )?,
                    )));
                }
                self.phase = Phase::FlagsString;
                Ok(StringReplaceAction::Primitive(value))
            }
            Phase::FlagsString => {
                let flags = match primitive_string(runtime, realm, value)? {
                    NativeConversion::Value(value) => value,
                    NativeConversion::Throw(value) => {
                        return Ok(StringReplaceAction::Complete(Completion::Throw(value)));
                    }
                };
                if !flags.utf16_units().any(|unit| unit == u16::from(b'g')) {
                    return Ok(StringReplaceAction::Complete(Completion::Throw(
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
                    return Ok(StringReplaceAction::Complete(Completion::Throw(
                        runtime.new_native_error(realm, NativeErrorKind::Type, "not a function")?,
                    )));
                };
                let mut arguments = Vec::new();
                if arguments.try_reserve_exact(2).is_err() {
                    return Ok(StringReplaceAction::Complete(Completion::Throw(
                        runtime.new_native_error(
                            realm,
                            NativeErrorKind::Internal,
                            "out of memory",
                        )?,
                    )));
                }
                arguments.push(self.receiver.clone());
                arguments.push(self.replace_value.clone());
                self.phase = Phase::ProtocolResult;
                Ok(StringReplaceAction::Call {
                    target: DirectCallTarget::Callable(callable),
                    receiver: self.search_value.clone(),
                    arguments,
                })
            }
            Phase::ProtocolResult => Ok(StringReplaceAction::Complete(Completion::Return(value))),
            Phase::Source => {
                self.source = Some(match primitive_string(runtime, realm, value)? {
                    NativeConversion::Value(value) => value,
                    NativeConversion::Throw(value) => {
                        return Ok(StringReplaceAction::Complete(Completion::Throw(value)));
                    }
                });
                self.phase = Phase::Search;
                Ok(StringReplaceAction::Primitive(self.search_value.clone()))
            }
            Phase::Search => {
                let search = match primitive_string(runtime, realm, value)? {
                    NativeConversion::Value(value) => value,
                    NativeConversion::Throw(value) => {
                        return Ok(StringReplaceAction::Complete(Completion::Throw(value)));
                    }
                };
                let functional = match &self.replace_value {
                    Value::Object(object) => runtime.as_callable(object)?,
                    _ => None,
                };
                self.cursor = Some(ReplaceLoop {
                    output: self
                        .output
                        .take()
                        .ok_or(RuntimeError::Invariant("replacement buffer disappeared"))?,
                    source: self
                        .source
                        .take()
                        .ok_or(RuntimeError::Invariant("replacement source disappeared"))?,
                    search,
                    functional,
                    replacement: None,
                    end: 0,
                    first: true,
                });
                if self
                    .cursor
                    .as_ref()
                    .is_some_and(|state| state.functional.is_none())
                {
                    self.phase = Phase::Replacement;
                    Ok(StringReplaceAction::Primitive(self.replace_value.clone()))
                } else {
                    self.next(runtime)
                }
            }
            Phase::Replacement => {
                let replacement = match primitive_string(runtime, realm, value)? {
                    NativeConversion::Value(value) => value,
                    NativeConversion::Throw(value) => {
                        return Ok(StringReplaceAction::Complete(Completion::Throw(value)));
                    }
                };
                self.cursor
                    .as_mut()
                    .ok_or(RuntimeError::Invariant("replacement cursor disappeared"))?
                    .replacement = Some(replacement);
                self.next(runtime)
            }
            Phase::Callback { position } => {
                self.phase = Phase::CallbackString { position };
                Ok(StringReplaceAction::Primitive(value))
            }
            Phase::CallbackString { position } => {
                let result = match primitive_string(runtime, realm, value)? {
                    NativeConversion::Value(value) => value,
                    NativeConversion::Throw(value) => {
                        return Ok(StringReplaceAction::Complete(Completion::Throw(value)));
                    }
                };
                let state = self
                    .cursor
                    .as_mut()
                    .ok_or(RuntimeError::Invariant("replacement cursor disappeared"))?;
                state.output.append_js_string(&result);
                state.end = position + state.search.len();
                state.first = false;
                if matches!(self.selector, StringReplaceKind::Replace) {
                    self.finish_buffer(runtime)
                } else {
                    self.next(runtime)
                }
            }
        }
    }
    fn next(&mut self, runtime: &Runtime) -> Result<StringReplaceAction, RuntimeError> {
        loop {
            let state = self
                .cursor
                .as_mut()
                .ok_or(RuntimeError::Invariant("replacement cursor disappeared"))?;
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
                    let state = self
                        .cursor
                        .take()
                        .ok_or(RuntimeError::Invariant("replacement cursor disappeared"))?;
                    return Ok(StringReplaceAction::Complete(Completion::Return(
                        Value::String(state.source),
                    )));
                }
                return self.finish_buffer(runtime);
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
                    return Ok(StringReplaceAction::Complete(Completion::Throw(
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
                self.phase = Phase::Callback { position };
                return Ok(StringReplaceAction::Call {
                    target: DirectCallTarget::Callable(callable.clone()),
                    receiver: Value::Undefined,
                    arguments,
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
                    let state = self
                        .cursor
                        .take()
                        .ok_or(RuntimeError::Invariant("replacement cursor disappeared"))?;
                    return match runtime.finish_replacement_buffer(self.realm, state.output)? {
                        NativeConversion::Value(_) => Err(RuntimeError::Invariant(
                            "failed replacement buffer unexpectedly completed",
                        )),
                        NativeConversion::Throw(value) => {
                            Ok(StringReplaceAction::Complete(Completion::Throw(value)))
                        }
                    };
                }
                Err(value) => return Ok(StringReplaceAction::Complete(Completion::Throw(value))),
            }
            state.end = position + state.search.len();
            state.first = false;
            if matches!(self.selector, StringReplaceKind::Replace) {
                return self.finish_buffer(runtime);
            }
        }
    }
    fn finish_buffer(&mut self, runtime: &Runtime) -> Result<StringReplaceAction, RuntimeError> {
        let mut state = self
            .cursor
            .take()
            .ok_or(RuntimeError::Invariant("replacement cursor disappeared"))?;
        state
            .output
            .append_range(&state.source, state.end, state.source.len());
        Ok(StringReplaceAction::Complete(
            match runtime.finish_replacement_buffer(self.realm, state.output)? {
                NativeConversion::Value(value) => Completion::Return(Value::String(value)),
                NativeConversion::Throw(value) => Completion::Throw(value),
            },
        ))
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
                StringReplaceStep::PreparedRead { read, key, resume } => {
                    let result = match self.finish_prepared_read(realm, &key, read)? {
                        NativeConversion::Value(value) => {
                            Completion::Return(value.unwrap_or(Value::Undefined))
                        }
                        NativeConversion::Throw(value) => Completion::Throw(value),
                    };
                    resume.resume(self, result)?
                }
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
        // Source is a real Object conversion wait. Its primitive reply now
        // advances search conversion locally to the actual replacer callback.
        let StringReplaceStep::Call { resume, .. } = resume
            .resume(
                &runtime,
                Completion::Return(Value::String(JsString::from_static("aa"))),
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

#[cfg(test)]
mod local_replace_tests {
    use super::*;

    #[test]
    fn selected_replace_getter_is_consumed_once_with_reentry() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        assert_eq!(
            context
                .eval(
                    r#"(()=>{
            let reads=0,calls=0,seen='';
            const search={get [Symbol.replace](){
                reads++;seen+='g';Object.defineProperty(this,Symbol.replace,{value(){throw 99}});
                return function(input,replacement){calls++;seen+='c';return input+replacement};
            }};
            if('a'.replace(search,'b')!=='ab'||reads!==1||calls!==1||seen!=='gc')return false;
            const re=/a/g;let trace='';
            re.exec=function(input){trace+='e';return null};
            if('a'.replace(re,'b')!=='a'||trace!=='e')return false;
            return 'ab'.replace(/a/,'$&$&')==='aab' && 'aa'.replace(/a/g,()=> 'b')==='bb'
                && 'ab'.replace(/(?<x>a)/,'$<x>$<x>')==='aab';
        })()"#
                )
                .unwrap(),
            Value::Bool(true)
        );
    }
}
