//! RegExp String Iterator next retains its matcher snapshot across observable work.
use super::match_protocol::advance_string_index;
use crate::engine::{
    api::{error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError},
    heap::{ContextId, ObjectPayload},
    object::{ObjectRef, PropertyKey, operations::InternalSetResult},
    value::{JsString, Value, conversion::NativeConversion},
    vm::{
        Completion, ToPrimitiveHint,
        call::{NativeInvocation, NativeInvokeOutcome},
    },
};

pub(crate) enum RegExpIteratorStep {
    Complete(NativeInvokeOutcome),
    Exec {
        regexp: Value,
        input: Value,
        resume: RegExpIteratorResume,
    },
    Read {
        object: ObjectRef,
        key: PropertyKey,
        resume: RegExpIteratorResume,
    },
    String {
        value: Value,
        resume: RegExpIteratorResume,
    },
    Primitive {
        value: Value,
        resume: RegExpIteratorResume,
    },
    Set {
        object: ObjectRef,
        key: PropertyKey,
        value: Value,
        resume: RegExpIteratorResume,
    },
}
enum Phase {
    Exec,
    MatchString,
    LastIndex,
    Advance,
    Set,
}
pub(crate) struct RegExpIteratorResume {
    realm: ContextId,
    iterator: ObjectRef,
    regexp: ObjectRef,
    string: JsString,
    global: bool,
    full_unicode: bool,
    matched: Option<ObjectRef>,
    // These getter results remain live through the following conversions and
    // lastIndex setter, as in the synchronous algorithm's local bindings.
    match_value: Value,
    index_value: Value,
    phase: Phase,
}
impl RegExpIteratorStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        invocation: &NativeInvocation,
    ) -> Result<Self, RuntimeError> {
        let NativeInvocation::Call { this_value } = invocation else {
            return Err(RuntimeError::Invariant(
                "RegExp String Iterator next did not receive an iterator-next invocation",
            ));
        };
        let iterator = match this_value {
            Value::Object(iterator)
                if matches!(
                    runtime
                        .0
                        .state
                        .borrow()
                        .heap
                        .object(iterator.object_id())?
                        .payload,
                    ObjectPayload::RegExpStringIterator { .. }
                ) =>
            {
                iterator.clone()
            }
            _ => {
                return Ok(Self::Complete(NativeInvokeOutcome::Completion(
                    Completion::Throw(runtime.new_native_error(
                        realm,
                        NativeErrorKind::Type,
                        "RegExp String Iterator object expected",
                    )?),
                )));
            }
        };
        let (regexp_id, string, global, full_unicode, done) = runtime
            .0
            .state
            .borrow()
            .heap
            .regexp_string_iterator_state(iterator.object_id())?;
        if done {
            return Ok(Self::Complete(NativeInvokeOutcome::IteratorNextRaw {
                value: Value::Undefined,
                done: true,
            }));
        }
        let regexp = ObjectRef::from_borrowed_handle(runtime.clone(), regexp_id)?;
        Ok(Self::Exec {
            regexp: Value::Object(regexp.clone()),
            input: Value::String(string.clone()),
            resume: RegExpIteratorResume {
                realm,
                iterator,
                regexp,
                string,
                global,
                full_unicode,
                matched: None,
                match_value: Value::Undefined,
                index_value: Value::Undefined,
                phase: Phase::Exec,
            },
        })
    }
}
impl RegExpIteratorResume {
    fn abrupt(self, value: Value) -> RegExpIteratorStep {
        RegExpIteratorStep::Complete(NativeInvokeOutcome::Completion(Completion::Throw(value)))
    }
    fn yielded(self) -> Result<RegExpIteratorStep, RuntimeError> {
        Ok(RegExpIteratorStep::Complete(
            NativeInvokeOutcome::IteratorNextRaw {
                value: Value::Object(
                    self.matched
                        .ok_or(RuntimeError::Invariant("RegExp iterator lost match result"))?,
                ),
                done: false,
            },
        ))
    }
    pub(crate) fn resume(
        mut self,
        runtime: &Runtime,
        reply: Completion,
    ) -> Result<RegExpIteratorStep, RuntimeError> {
        let value = match reply {
            Completion::Return(value) => value,
            Completion::Throw(value) => return Ok(self.abrupt(value)),
        };
        match self.phase {
            Phase::Exec => {
                match value {
                    Value::Null => {
                        runtime
                            .0
                            .state
                            .borrow_mut()
                            .heap
                            .finish_regexp_string_iterator(self.iterator.object_id())?;
                        return Ok(RegExpIteratorStep::Complete(
                            NativeInvokeOutcome::IteratorNextRaw {
                                value: Value::Undefined,
                                done: true,
                            },
                        ));
                    }
                    Value::Object(matched) => self.matched = Some(matched),
                    _ => {
                        return Err(RuntimeError::Invariant(
                            "RegExpExec returned neither an object nor null",
                        ));
                    }
                }
                if !self.global {
                    runtime
                        .0
                        .state
                        .borrow_mut()
                        .heap
                        .finish_regexp_string_iterator(self.iterator.object_id())?;
                    return self.yielded();
                }
                self.phase = Phase::MatchString;
                Ok(RegExpIteratorStep::Read {
                    object: self.matched.as_ref().unwrap().clone(),
                    key: runtime.intern_property_key("0")?,
                    resume: self,
                })
            }
            Phase::MatchString => {
                self.match_value = value.clone();
                Ok(RegExpIteratorStep::String {
                    value,
                    resume: self,
                })
            }
            Phase::LastIndex => {
                self.index_value = value.clone();
                self.phase = Phase::Advance;
                Ok(RegExpIteratorStep::Primitive {
                    value,
                    resume: self,
                })
            }
            Phase::Advance => {
                if matches!(value, Value::Object(_)) {
                    return Err(RuntimeError::Invariant(
                        "RegExp iterator length conversion returned an object",
                    ));
                }
                let current = match runtime.native_to_length(self.realm, &value)? {
                    NativeConversion::Value(value) => value,
                    NativeConversion::Throw(value) => return Ok(self.abrupt(value)),
                };
                let next = advance_string_index(&self.string, current, self.full_unicode);
                self.phase = Phase::Set;
                Ok(RegExpIteratorStep::Set {
                    object: self.regexp.clone(),
                    key: runtime.intern_property_key("lastIndex")?,
                    value: Value::number(next as f64),
                    resume: self,
                })
            }
            Phase::Set => Err(RuntimeError::Invariant(
                "RegExp iterator set needs typed reply",
            )),
        }
    }
    pub(crate) fn string(
        mut self,
        runtime: &Runtime,
        reply: NativeConversion<JsString>,
    ) -> Result<RegExpIteratorStep, RuntimeError> {
        if !matches!(self.phase, Phase::MatchString) {
            return Err(RuntimeError::Invariant(
                "RegExp iterator string phase mismatch",
            ));
        }
        let string = match reply {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => return Ok(self.abrupt(value)),
        };
        if !string.is_empty() {
            return self.yielded();
        }
        self.phase = Phase::LastIndex;
        Ok(RegExpIteratorStep::Read {
            object: self.regexp.clone(),
            key: runtime.intern_property_key("lastIndex")?,
            resume: self,
        })
    }
    pub(crate) fn set(
        self,
        runtime: &Runtime,
        key: PropertyKey,
        reply: NativeConversion<InternalSetResult>,
    ) -> Result<RegExpIteratorStep, RuntimeError> {
        if !matches!(self.phase, Phase::Set) {
            return Err(RuntimeError::Invariant(
                "RegExp iterator set phase mismatch",
            ));
        }
        if let Some(value) = runtime.finish_set_property_or_throw(self.realm, &key, reply)? {
            return Ok(self.abrupt(value));
        }
        self.yielded()
    }
}
pub(crate) fn finish(
    runtime: &Runtime,
    realm: ContextId,
    mut step: RegExpIteratorStep,
) -> Result<NativeInvokeOutcome, RuntimeError> {
    loop {
        step = match step {
            RegExpIteratorStep::Complete(result) => return Ok(result),
            RegExpIteratorStep::Exec {
                regexp,
                input,
                resume,
            } => resume.resume(runtime, runtime.regexp_exec_abstract(realm, regexp, input)?)?,
            RegExpIteratorStep::Read {
                object,
                key,
                resume,
            } => resume.resume(
                runtime,
                runtime.get_property_in_realm(realm, &object, &key)?,
            )?,
            RegExpIteratorStep::String { value, resume } => {
                resume.string(runtime, runtime.native_to_js_string(realm, &value)?)?
            }
            RegExpIteratorStep::Primitive { value, resume } => resume.resume(
                runtime,
                runtime.to_primitive(realm, value, ToPrimitiveHint::Number)?,
            )?,
            RegExpIteratorStep::Set {
                object,
                key,
                value,
                resume,
            } => resume.set(
                runtime,
                key.clone(),
                runtime.internal_set(realm, &object, &key, value, Value::Object(object.clone()))?,
            )?,
        };
    }
}
