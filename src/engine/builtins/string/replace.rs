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
    value::{JsString, JsValue, Value, conversion::NativeConversion},
    vm::{
        Completion,
        call::{NativeArguments, NativeInvocation},
    },
};

pub(crate) enum StringReplaceStep {
    Complete(Completion),
    PreparedRead { resume: StringReplaceResume },
    Primitive { resume: StringReplaceResume },
    Call { resume: StringReplaceResume },
}
pub(crate) struct StringReplaceResume(Box<StringReplaceResumeState>);
impl std::ops::Deref for StringReplaceResume {
    type Target = StringReplaceResumeState;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for StringReplaceResume {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
const _: () = assert!(std::mem::size_of::<StringReplaceResume>() <= 8);
pub(crate) struct StringReplaceResumeState {
    runtime: Runtime,
    step_pending: StringReplaceStepPending,
    realm: ContextId,
    selector: StringReplaceKind,
    receiver: JsValue,
    search_value: JsValue,
    replace_value: JsValue,
    phase: Phase,
    output: Option<ReplacementStringBuffer>,
    source: Option<JsString>,
    source_value: JsValue,
    search_text: JsValue,
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
impl Drop for StringReplaceResumeState {
    /// Release the internal edges the pending effect still owns when the
    /// request is abandoned. Consumption goes through `Option::take`, so a
    /// drained field is `None` here; releases are defer-safe and nothrow.
    fn drop(&mut self) {
        for value in [
            &mut self.receiver,
            &mut self.search_value,
            &mut self.replace_value,
            &mut self.source_value,
            &mut self.search_text,
        ] {
            let _ = self
                .runtime
                .release_jsvalue(std::mem::replace(value, JsValue::Undefined));
        }
        if let Some(read) = self.step_pending.read.take() {
            read.release(&self.runtime);
        }
        if let Some(value) = self.step_pending.value.take() {
            let _ = self.runtime.release_jsvalue(value);
        }
        if let Some(value) = self.step_pending.receiver.take() {
            let _ = self.runtime.release_jsvalue(value);
        }
        if let Some(values) = self.step_pending.arguments.take() {
            for value in values {
                let _ = self.runtime.release_jsvalue(value);
            }
        }
    }
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
    PreparedRead {
        read: crate::engine::object::OrdinaryRead,
        key: PropertyKey,
    },
    Primitive(JsValue),
    Call {
        target: DirectCallTarget,
        receiver: JsValue,
        arguments: Vec<JsValue>,
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
        if matches!(this_value, JsValue::Undefined | JsValue::Null) {
            return Ok(Self::Complete(Completion::Throw(
                runtime.new_native_error_jsvalue(
                    realm,
                    NativeErrorKind::Type,
                    "cannot convert to object",
                )?,
            )));
        }
        let mut resume = StringReplaceResumeState {
            runtime: runtime.clone(),
            step_pending: StringReplaceStepPending::default(),
            realm,
            selector,
            receiver: JsValue::Undefined,
            search_value: JsValue::Undefined,
            replace_value: JsValue::Undefined,
            phase: Phase::Method,
            output: None,
            source: None,
            source_value: JsValue::Undefined,
            search_text: JsValue::Undefined,
            cursor: None,
        };
        resume.receiver = runtime.dup_jsvalue(this_value)?;
        resume.search_value = runtime.dup_jsvalue(arguments.readable.first().ok_or(
            RuntimeError::Invariant("String replace search argv was not padded"),
        )?)?;
        resume.replace_value = runtime.dup_jsvalue(arguments.readable.get(1).ok_or(
            RuntimeError::Invariant("String replace replacement argv was not padded"),
        )?)?;
        let action = if matches!(resume.search_value, JsValue::Object(_)) {
            if matches!(selector, StringReplaceKind::ReplaceAll) {
                resume.phase = Phase::Match;
                StringReplaceAction::Read(PropertyKey::from(
                    runtime.well_known_symbol(WellKnownSymbol::Match),
                ))
            } else {
                resume.method(runtime)?
            }
        } else {
            resume.source()?
        };
        let action = resume.advance_local(runtime, action)?;
        if let StringReplaceAction::Complete(result) = action {
            return Ok(Self::Complete(result));
        }
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_execution_event(
            "stringreplace_resident_allocated",
        );
        StringReplaceResume(Box::new(resume)).publish(runtime, action)
    }
}
impl StringReplaceResume {
    /// Only the already-selected @@replace protocol call is eligible for the
    /// VM local native handoff; functional replacers remain real calls.
    pub(crate) fn awaits_protocol_result(&self) -> bool {
        matches!(self.0.phase, Phase::ProtocolResult)
    }

    pub(crate) fn resume(
        mut self,
        runtime: &Runtime,
        completion: Completion,
    ) -> Result<StringReplaceStep, RuntimeError> {
        let action = self.0.advance(runtime, completion)?;
        let action = self.0.advance_local(runtime, action)?;
        self.publish(runtime, action)
    }
    fn publish(
        self,
        _runtime: &Runtime,
        action: StringReplaceAction,
    ) -> Result<StringReplaceStep, RuntimeError> {
        Ok(match action {
            StringReplaceAction::Complete(result) => StringReplaceStep::Complete(result),
            StringReplaceAction::PreparedRead { read, key } => {
                StringReplaceStep::make_preparedread(read, key, self)
            }
            StringReplaceAction::Primitive(value) => StringReplaceStep::make_primitive(value, self),
            StringReplaceAction::Call {
                target,
                receiver,
                arguments,
            } => StringReplaceStep::make_call(target, receiver, arguments, self),
            StringReplaceAction::Read(_) => {
                return Err(RuntimeError::Invariant(
                    "local replacement read was not selected",
                ));
            }
        })
    }
}
impl StringReplaceResumeState {
    fn method(&mut self, runtime: &Runtime) -> Result<StringReplaceAction, RuntimeError> {
        if !matches!(self.search_value, JsValue::Object(_)) {
            return Err(RuntimeError::Invariant(
                "replacement protocol lost its object",
            ));
        }
        self.phase = Phase::Method;
        Ok(StringReplaceAction::Read(PropertyKey::from(
            runtime.well_known_symbol(WellKnownSymbol::Replace),
        )))
    }
    fn source(&mut self) -> Result<StringReplaceAction, RuntimeError> {
        // Latch buffer allocation failure before observable fallback coercions.
        self.output = Some(ReplacementStringBuffer::new(0));
        self.phase = Phase::Source;
        Ok(StringReplaceAction::Primitive(
            self.runtime.dup_jsvalue(&self.receiver)?,
        ))
    }
    fn advance_local(
        &mut self,
        runtime: &Runtime,
        mut action: StringReplaceAction,
    ) -> Result<StringReplaceAction, RuntimeError> {
        loop {
            action = match action {
                StringReplaceAction::Complete(result) => {
                    return Ok(StringReplaceAction::Complete(result));
                }
                StringReplaceAction::Read(key) => {
                    let JsValue::Object(id) = &self.search_value else {
                        return Err(RuntimeError::Invariant("replacement read lost its object"));
                    };
                    let object = ObjectRef::from_borrowed_handle(runtime.clone(), *id)?;
                    match runtime.prepare_ordinary_read_selected(
                        &object,
                        &key,
                        &self.search_value,
                        None,
                    )? {
                        crate::engine::object::OrdinaryRead::Complete(value) => {
                            #[cfg(feature = "profiling")]
                            crate::engine::api::profiling::record_owned_execution_event(
                                "stringreplace_read_local",
                            );
                            self.advance(
                                runtime,
                                Completion::Return(value.unwrap_or(JsValue::Undefined)),
                            )?
                        }
                        read => {
                            return Ok(StringReplaceAction::PreparedRead { read, key });
                        }
                    }
                }
                StringReplaceAction::Primitive(value) if !matches!(value, JsValue::Object(_)) => {
                    #[cfg(feature = "profiling")]
                    crate::engine::api::profiling::record_owned_execution_event(
                        "stringreplace_primitive_local",
                    );
                    self.advance(runtime, Completion::Return(value))?
                }
                action @ (StringReplaceAction::Primitive(_)
                | StringReplaceAction::Call { .. }
                | StringReplaceAction::PreparedRead { .. }) => return Ok(action),
            };
        }
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
                let JsValue::Object(id) = &self.search_value else {
                    runtime.release_jsvalue(value)?;
                    return Err(RuntimeError::Invariant(
                        "replacement regexp check lost its object",
                    ));
                };
                let regexp = if matches!(value, JsValue::Undefined) {
                    ObjectRef::from_borrowed_handle(runtime.clone(), *id)
                        .map_err(RuntimeError::from)
                        .and_then(|object| runtime.native_object_has_regexp_brand(&object))
                } else {
                    runtime.value_to_boolean_jsvalue(&value)
                };
                runtime.release_jsvalue(value)?;
                if regexp? {
                    self.phase = Phase::Flags;
                    Ok(StringReplaceAction::Read(runtime.pinned_property_key(
                        crate::engine::atom::pinned::PinnedAtom::Flags,
                    )?))
                } else {
                    self.method(runtime)
                }
            }
            Phase::Flags => {
                if matches!(value, JsValue::Undefined | JsValue::Null) {
                    return Ok(StringReplaceAction::Complete(Completion::Throw(
                        runtime.new_native_error_jsvalue(
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
                        runtime.new_native_error_jsvalue(
                            realm,
                            NativeErrorKind::Type,
                            "regexp must have the 'g' flag",
                        )?,
                    )));
                }
                self.method(runtime)
            }
            Phase::Method => {
                if matches!(value, JsValue::Undefined | JsValue::Null) {
                    return self.source();
                }
                let callable = match value {
                    JsValue::Object(id) => {
                        runtime.as_callable(&ObjectRef::from_owned_handle(runtime.clone(), id))?
                    }
                    value => {
                        runtime.release_jsvalue(value)?;
                        None
                    }
                };
                let Some(callable) = callable else {
                    return Ok(StringReplaceAction::Complete(Completion::Throw(
                        runtime.new_native_error_jsvalue(
                            realm,
                            NativeErrorKind::Type,
                            "not a function",
                        )?,
                    )));
                };
                let mut arguments = Vec::new();
                if arguments.try_reserve_exact(2).is_err() {
                    return Ok(StringReplaceAction::Complete(Completion::Throw(
                        runtime.new_native_error_jsvalue(
                            realm,
                            NativeErrorKind::Internal,
                            "out of memory",
                        )?,
                    )));
                }
                self.step_pending.arguments = Some(arguments);
                self.step_pending
                    .arguments
                    .as_mut()
                    .expect("protocol arguments")
                    .push(runtime.dup_jsvalue(&self.receiver)?);
                self.step_pending
                    .arguments
                    .as_mut()
                    .expect("protocol arguments")
                    .push(runtime.dup_jsvalue(&self.replace_value)?);
                let receiver = runtime.dup_jsvalue(&self.search_value)?;
                let arguments = self
                    .step_pending
                    .arguments
                    .take()
                    .expect("protocol arguments");
                self.phase = Phase::ProtocolResult;
                Ok(StringReplaceAction::Call {
                    target: DirectCallTarget::Callable(callable),
                    receiver,
                    arguments,
                })
            }
            Phase::ProtocolResult => Ok(StringReplaceAction::Complete(Completion::Return(value))),
            Phase::Source => {
                self.source = Some(match primitive_string_owned(runtime, realm, value)? {
                    NativeConversion::Value((text, handle)) => {
                        runtime
                            .release_jsvalue(std::mem::replace(&mut self.source_value, handle))?;
                        text
                    }
                    NativeConversion::Throw(value) => {
                        return Ok(StringReplaceAction::Complete(Completion::Throw(value)));
                    }
                });
                self.phase = Phase::Search;
                Ok(StringReplaceAction::Primitive(
                    runtime.dup_jsvalue(&self.search_value)?,
                ))
            }
            Phase::Search => {
                let search = match primitive_string_owned(runtime, realm, value)? {
                    NativeConversion::Value((text, handle)) => {
                        runtime
                            .release_jsvalue(std::mem::replace(&mut self.search_text, handle))?;
                        text
                    }
                    NativeConversion::Throw(value) => {
                        return Ok(StringReplaceAction::Complete(Completion::Throw(value)));
                    }
                };
                let functional = match &self.replace_value {
                    JsValue::Object(id) => runtime
                        .as_callable(&ObjectRef::from_borrowed_handle(runtime.clone(), *id)?)?,
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
                    Ok(StringReplaceAction::Primitive(
                        runtime.dup_jsvalue(&self.replace_value)?,
                    ))
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
                    self.cursor
                        .take()
                        .ok_or(RuntimeError::Invariant("replacement cursor disappeared"))?;
                    return Ok(StringReplaceAction::Complete(Completion::Return(
                        std::mem::replace(&mut self.source_value, JsValue::Undefined),
                    )));
                }
                return self.finish_buffer(runtime);
            };
            state
                .output
                .append_range(&state.source, state.end, position);
            if let Some(callable) = &state.functional {
                let position_value = JsValue::Int(i32::try_from(position).map_err(|_| {
                    RuntimeError::Invariant("String replace position exceeded signed range")
                })?);
                let mut arguments = Vec::new();
                if arguments.try_reserve_exact(3).is_err() {
                    return Ok(StringReplaceAction::Complete(Completion::Throw(
                        runtime.new_native_error_jsvalue(
                            self.realm,
                            NativeErrorKind::Internal,
                            "out of memory",
                        )?,
                    )));
                }
                self.step_pending.arguments = Some(arguments);
                self.step_pending
                    .arguments
                    .as_mut()
                    .expect("replacement arguments")
                    .push(runtime.dup_jsvalue(&self.search_text)?);
                self.step_pending
                    .arguments
                    .as_mut()
                    .expect("replacement arguments")
                    .push(position_value);
                self.step_pending
                    .arguments
                    .as_mut()
                    .expect("replacement arguments")
                    .push(runtime.dup_jsvalue(&self.source_value)?);
                let arguments = self
                    .step_pending
                    .arguments
                    .take()
                    .expect("replacement arguments");
                self.phase = Phase::Callback { position };
                return Ok(StringReplaceAction::Call {
                    target: DirectCallTarget::Callable(callable.clone()),
                    receiver: JsValue::Undefined,
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
                Err(value) => {
                    return Ok(StringReplaceAction::Complete(Completion::Throw(value)));
                }
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
                NativeConversion::Value(value) => {
                    Completion::Return(runtime.into_jsvalue(Value::String(value))?)
                }
                NativeConversion::Throw(value) => Completion::Throw(value),
            },
        ))
    }
}
fn primitive_string(
    runtime: &Runtime,
    realm: ContextId,
    value: JsValue,
) -> Result<NativeConversion<JsString>, RuntimeError> {
    let result = runtime.string_from_primitive_jsvalue(realm, &value);
    runtime.release_jsvalue(value)?;
    result
}
fn primitive_string_owned(
    runtime: &Runtime,
    realm: ContextId,
    value: JsValue,
) -> Result<NativeConversion<(JsString, JsValue)>, RuntimeError> {
    let converted = runtime.string_from_primitive_jsvalue(realm, &value);
    match converted {
        Ok(NativeConversion::Value(text)) => {
            if matches!(value, JsValue::String(_)) {
                return Ok(NativeConversion::Value((text, value)));
            }
            runtime.release_jsvalue(value)?;
            let handle = runtime.into_jsvalue(Value::String(text.clone()))?;
            Ok(NativeConversion::Value((text, handle)))
        }
        Ok(NativeConversion::Throw(error)) => {
            runtime.release_jsvalue(value)?;
            Ok(NativeConversion::Throw(error))
        }
        Err(error) => {
            runtime.release_jsvalue(value)?;
            Err(error)
        }
    }
}
impl Runtime {
    pub(crate) fn call_string_prototype_replace(
        &self,
        realm: ContextId,
        selector: StringReplaceKind,
        invocation: NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        self.dispatch_borrowed_invocation(invocation, |invocation| {
            let mut step = StringReplaceStep::start(self, realm, selector, invocation, arguments)?;
            loop {
                step = match step {
                    StringReplaceStep::Complete(result) => return Ok(result),
                    StringReplaceStep::PreparedRead { mut resume } => {
                        let read = resume.take_preparedread_read();
                        let key = resume.take_preparedread_key();
                        {
                            let result =
                                match self.finish_prepared_read_jsvalue(realm, &key, read)? {
                                    NativeConversion::Value(value) => {
                                        Completion::Return(value.unwrap_or(JsValue::Undefined))
                                    }
                                    NativeConversion::Throw(value) => Completion::Throw(value),
                                };
                            resume.resume(self, result)?
                        }
                    }
                    StringReplaceStep::Primitive { mut resume } => {
                        let value = resume.take_primitive_value();
                        {
                            let result = if matches!(value, JsValue::Object(_)) {
                                self.to_primitive_jsvalue(realm, value, ToPrimitiveHint::String)?
                            } else {
                                Completion::Return(value)
                            };
                            resume.resume(self, result)?
                        }
                    }
                    StringReplaceStep::Call { mut resume } => {
                        let target = resume.take_call_target();
                        let receiver = resume.take_call_receiver();
                        let arguments = resume.take_call_arguments();
                        {
                            let DirectCallTarget::Callable(callable) = target else {
                                self.release_jsvalue(receiver)?;
                                for value in arguments {
                                    self.release_jsvalue(value)?;
                                }
                                return Err(RuntimeError::Invariant(
                                    "String replacement requested an invalid call target",
                                ));
                            };
                            resume.resume(
                                self,
                                self.call_internal_jsvalue(realm, &callable, receiver, arguments)?,
                            )?
                        }
                    }
                };
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "profiling")]
    #[test]
    fn completed_string_replacement_never_allocates_a_resident_owner() {
        let runtime = Runtime::new();
        let context = runtime.new_context();
        let invocation = NativeInvocation::Call {
            this_value: runtime
                .into_jsvalue(Value::String(JsString::from_static("aba")))
                .unwrap(),
        };
        let arguments = NativeArguments {
            actual_arg_count: 2,
            readable: vec![
                runtime
                    .into_jsvalue(Value::String(JsString::from_static("a")))
                    .unwrap(),
                runtime
                    .into_jsvalue(Value::String(JsString::from_static("$&x")))
                    .unwrap(),
            ],
        };
        let profile = crate::engine::api::profiling::CostProfile::start();
        let StringReplaceStep::Complete(Completion::Return(value)) = StringReplaceStep::start(
            &runtime,
            context.realm,
            StringReplaceKind::ReplaceAll,
            &invocation,
            &arguments,
        )
        .unwrap() else {
            panic!("primitive replace must complete locally")
        };
        assert_eq!(
            runtime.root_and_release_jsvalue(value).unwrap(),
            Value::String(JsString::from_static("axbax"))
        );
        assert_eq!(
            profile
                .snapshot()
                .owned_execution_events
                .get("stringreplace_resident_allocated")
                .copied()
                .unwrap_or(0),
            0
        );
        invocation.release(&runtime).unwrap();
        for value in arguments.readable {
            runtime.release_jsvalue(value).unwrap();
        }
    }

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
            this_value: runtime.into_jsvalue(Value::Object(receiver)).unwrap(),
        };
        let arguments = NativeArguments {
            actual_arg_count: 2,
            readable: vec![
                runtime
                    .into_jsvalue(Value::String(JsString::from_static("a")))
                    .unwrap(),
                runtime.into_jsvalue(callback).unwrap(),
            ],
        };
        let StringReplaceStep::Primitive { mut resume } = StringReplaceStep::start(
            &runtime,
            context.realm,
            StringReplaceKind::ReplaceAll,
            &invocation,
            &arguments,
        )
        .unwrap() else {
            panic!("expected source conversion")
        };
        runtime
            .release_jsvalue(resume.take_primitive_value())
            .unwrap();
        let address = (&*resume.0) as *const StringReplaceResumeState;
        {
            let NativeInvocation::Call { this_value } = invocation else {
                unreachable!()
            };
            runtime.release_jsvalue(this_value).unwrap();
            for value in arguments.readable {
                runtime.release_jsvalue(value).unwrap();
            }
        }
        // Source is a real Object conversion wait. Its primitive reply now
        // advances search conversion locally to the actual replacer callback.
        let StringReplaceStep::Call { mut resume } = resume
            .resume(
                &runtime,
                Completion::Return(
                    runtime
                        .into_jsvalue(Value::String(JsString::from_static("aa")))
                        .unwrap(),
                ),
            )
            .unwrap()
        else {
            panic!("expected replacer call")
        };
        drop(resume.take_call_target());
        runtime
            .release_jsvalue(resume.take_call_receiver())
            .unwrap();
        for value in resume.take_call_arguments() {
            runtime.release_jsvalue(value).unwrap();
        }
        assert_eq!((&*resume.0) as *const StringReplaceResumeState, address);
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

#[derive(Default)]
pub(crate) struct StringReplaceStepPending {
    read: Option<crate::engine::object::OrdinaryRead>,
    key: Option<PropertyKey>,
    value: Option<JsValue>,
    target: Option<DirectCallTarget>,
    receiver: Option<JsValue>,
    arguments: Option<Vec<JsValue>>,
}
impl StringReplaceStep {
    pub(crate) fn make_preparedread(
        read: crate::engine::object::OrdinaryRead,
        key: PropertyKey,
        mut resume: StringReplaceResume,
    ) -> Self {
        resume.0.step_pending.read = Some(read);
        resume.0.step_pending.key = Some(key);
        Self::PreparedRead { resume }
    }
    pub(crate) fn make_primitive(value: JsValue, mut resume: StringReplaceResume) -> Self {
        resume.0.step_pending.value = Some(value);
        Self::Primitive { resume }
    }
    pub(crate) fn make_call(
        target: DirectCallTarget,
        receiver: JsValue,
        arguments: Vec<JsValue>,
        mut resume: StringReplaceResume,
    ) -> Self {
        resume.0.step_pending.target = Some(target);
        resume.0.step_pending.receiver = Some(receiver);
        resume.0.step_pending.arguments = Some(arguments);
        Self::Call { resume }
    }
}
impl StringReplaceResume {
    pub(crate) fn take_preparedread_read(&mut self) -> crate::engine::object::OrdinaryRead {
        self.0
            .step_pending
            .read
            .take()
            .expect("StringReplaceStep::PreparedRead lost read")
    }
    pub(crate) fn take_preparedread_key(&mut self) -> PropertyKey {
        self.0
            .step_pending
            .key
            .take()
            .expect("StringReplaceStep::PreparedRead lost key")
    }

    pub(crate) fn take_primitive_value(&mut self) -> JsValue {
        self.0
            .step_pending
            .value
            .take()
            .expect("StringReplaceStep::Primitive lost value")
    }

    pub(crate) fn take_call_target(&mut self) -> DirectCallTarget {
        self.0
            .step_pending
            .target
            .take()
            .expect("StringReplaceStep::Call lost target")
    }
    pub(crate) fn take_call_receiver(&mut self) -> JsValue {
        self.0
            .step_pending
            .receiver
            .take()
            .expect("StringReplaceStep::Call lost receiver")
    }
    pub(crate) fn take_call_arguments(&mut self) -> Vec<JsValue> {
        self.0
            .step_pending
            .arguments
            .take()
            .expect("StringReplaceStep::Call lost arguments")
    }
}

const _: () = assert!(std::mem::size_of::<StringReplaceStep>() <= 64);

// S11 all-domain protocol bound; inline completion stays allocation-free.
const _: () = assert!(std::mem::size_of::<StringReplaceStep>() <= 64);
