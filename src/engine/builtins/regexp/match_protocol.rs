//! RegExp @@match preserves global collection and empty-match progress.
use crate::engine::{
    api::{error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError},
    heap::ContextId,
    object::{ObjectRef, PropertyKey, operations::InternalSetResult},
    value::{JsString, JsValue, Value, conversion::NativeConversion},
    vm::{
        Completion, ToPrimitiveHint,
        call::{NativeArguments, NativeInvocation},
    },
};

pub(crate) fn advance_string_index(input: &JsString, index: u64, unicode: bool) -> u64 {
    let width = if unicode
        && usize::try_from(index)
            .ok()
            .filter(|index| *index < input.len())
            .and_then(|index| input.code_point_at(index))
            .is_some_and(|code_point| code_point > u32::from(u16::MAX))
    {
        2
    } else {
        1
    };
    index + width
}

pub(crate) enum RegExpMatchStep {
    Complete(Completion),
    Primitive { resume: RegExpMatchResume },
    Read { resume: RegExpMatchResume },
    Set { resume: RegExpMatchResume },
    Exec { resume: RegExpMatchResume },
}
pub(crate) struct RegExpMatchResume(Box<RegExpMatchResumeState>);
impl std::ops::Deref for RegExpMatchResume {
    type Target = RegExpMatchResumeState;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for RegExpMatchResume {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
const _: () = assert!(std::mem::size_of::<RegExpMatchResume>() <= 8);
pub(crate) struct RegExpMatchResumeState {
    step_pending: RegExpMatchStepPending,
    realm: ContextId,
    regexp: ObjectRef,
    converted: JsValue,
    input_value: JsValue,
    phase: MatchPhase,
}
impl Drop for RegExpMatchResumeState {
    fn drop(&mut self) {
        for value in [&mut self.converted, &mut self.input_value] {
            let _ = self
                .step_pending
                .runtime
                .release_jsvalue(std::mem::replace(value, JsValue::Undefined));
        }
    }
}
struct MatchCollection {
    input: JsString,
    unicode: bool,
    matches: ObjectRef,
    zero: PropertyKey,
    count: u32,
}
enum MatchPhase {
    Input,
    Flags(JsString),
    FlagsString(JsString),
    InitialSet { input: JsString, unicode: bool },
    Single,
    Exec(MatchCollection),
    Match(MatchCollection),
    MatchString(MatchCollection),
    LastIndex(MatchCollection),
    LastIndexNumber(MatchCollection),
    AdvancedSet(MatchCollection),
}
impl RegExpMatchStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        let NativeInvocation::Call { this_value } = invocation else {
            return Err(RuntimeError::Invariant(
                "RegExp @@match did not receive a generic invocation",
            ));
        };
        let JsValue::Object(regexp_id) = this_value else {
            return Ok(Self::Complete(Completion::Throw(
                runtime.new_native_error_jsvalue(realm, NativeErrorKind::Type, "not an object")?,
            )));
        };
        let regexp = ObjectRef::from_borrowed_handle(runtime.clone(), *regexp_id)?;
        Ok(Self::make_primitive(
            runtime.dup_jsvalue(arguments.readable.first().ok_or(RuntimeError::Invariant(
                "RegExp @@match input argv was not padded",
            ))?)?,
            ToPrimitiveHint::String,
            RegExpMatchResume(Box::new(RegExpMatchResumeState {
                step_pending: RegExpMatchStepPending::new(runtime),
                realm,
                regexp,
                converted: JsValue::Undefined,
                input_value: JsValue::Undefined,
                phase: MatchPhase::Input,
            })),
        ))
    }
}
impl RegExpMatchResume {
    fn execute(
        mut self,
        runtime: &Runtime,
        state: MatchCollection,
    ) -> Result<RegExpMatchStep, RuntimeError> {
        Ok(RegExpMatchStep::make_exec(
            JsValue::Object(self.0.regexp.clone().into_handle()),
            runtime.dup_jsvalue(&self.0.input_value)?,
            {
                let updated_0 = MatchPhase::Exec(state);
                self.0.phase = updated_0;
                self
            },
        ))
    }
    pub(crate) fn set(
        mut self,
        runtime: &Runtime,
        result: NativeConversion<InternalSetResult>,
    ) -> Result<RegExpMatchStep, RuntimeError> {
        let key =
            runtime.pinned_property_key(crate::engine::atom::pinned::PinnedAtom::LastIndex)?;
        if let Some(value) = runtime.finish_set_property_or_throw(self.0.realm, &key, result)? {
            return Ok(RegExpMatchStep::Complete(Completion::Throw(value)));
        }
        match std::mem::replace(&mut self.0.phase, MatchPhase::Single) {
            MatchPhase::InitialSet { input, unicode } => {
                let matches = runtime.new_array(self.0.realm)?;
                let zero = runtime
                    .pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Literal1)?;
                {
                    let updated_0 = MatchPhase::Single;
                    self.0.phase = updated_0;
                    self
                }
                .execute(
                    runtime,
                    MatchCollection {
                        input,
                        unicode,
                        matches,
                        zero,
                        count: 0,
                    },
                )
            }
            MatchPhase::AdvancedSet(state) => {
                let updated_0 = MatchPhase::Single;
                self.0.phase = updated_0;
                self
            }
            .execute(runtime, state),
            _ => Err(RuntimeError::Invariant(
                "RegExp match received an unexpected Set reply",
            )),
        }
    }
    pub(crate) fn resume(
        mut self,
        runtime: &Runtime,
        result: Completion,
    ) -> Result<RegExpMatchStep, RuntimeError> {
        let value = match result {
            Completion::Return(value) => value,
            Completion::Throw(value) => {
                return Ok(RegExpMatchStep::Complete(Completion::Throw(value)));
            }
        };
        let previous = std::mem::replace(&mut self.0.converted, value);
        runtime.release_jsvalue(previous)?;
        match std::mem::replace(&mut self.0.phase, MatchPhase::Single) {
            MatchPhase::Input => {
                let input =
                    match runtime.string_from_primitive_jsvalue(self.0.realm, &self.0.converted)? {
                        NativeConversion::Value(value) => value,
                        NativeConversion::Throw(value) => {
                            return Ok(RegExpMatchStep::Complete(Completion::Throw(value)));
                        }
                    };
                self.0.input_value = if matches!(self.0.converted, JsValue::String(_)) {
                    std::mem::replace(&mut self.0.converted, JsValue::Undefined)
                } else {
                    runtime.into_jsvalue(Value::String(input.clone()))?
                };
                Ok(RegExpMatchStep::make_read(
                    self.0.regexp.clone(),
                    runtime.pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Flags)?,
                    {
                        let updated_0 = MatchPhase::Flags(input);
                        self.0.phase = updated_0;
                        self
                    },
                ))
            }
            MatchPhase::Flags(input) => Ok(RegExpMatchStep::make_primitive(
                std::mem::replace(&mut self.0.converted, JsValue::Undefined),
                ToPrimitiveHint::String,
                {
                    let updated_0 = MatchPhase::FlagsString(input);
                    self.0.phase = updated_0;
                    self
                },
            )),
            MatchPhase::FlagsString(input) => {
                let flags =
                    match runtime.string_from_primitive_jsvalue(self.0.realm, &self.0.converted)? {
                        NativeConversion::Value(value) => value,
                        NativeConversion::Throw(value) => {
                            return Ok(RegExpMatchStep::Complete(Completion::Throw(value)));
                        }
                    };
                if !flags.utf16_units().any(|unit| unit == u16::from(b'g')) {
                    return Ok(RegExpMatchStep::make_exec(
                        JsValue::Object(self.0.regexp.clone().into_handle()),
                        runtime.dup_jsvalue(&self.0.input_value)?,
                        {
                            let updated_0 = MatchPhase::Single;
                            self.0.phase = updated_0;
                            self
                        },
                    ));
                }
                let unicode = flags
                    .utf16_units()
                    .any(|unit| unit == u16::from(b'u') || unit == u16::from(b'v'));
                Ok(RegExpMatchStep::make_set(
                    self.0.regexp.clone(),
                    runtime
                        .pinned_property_key(crate::engine::atom::pinned::PinnedAtom::LastIndex)?,
                    runtime.into_jsvalue(Value::Int(0))?,
                    {
                        let updated_0 = MatchPhase::InitialSet { input, unicode };
                        self.0.phase = updated_0;
                        self
                    },
                ))
            }
            MatchPhase::Single => Ok(RegExpMatchStep::Complete(Completion::Return(
                std::mem::replace(&mut self.0.converted, JsValue::Undefined),
            ))),
            MatchPhase::Exec(state) => {
                let result = match &self.0.converted {
                    JsValue::Null => {
                        return Ok(RegExpMatchStep::Complete(Completion::Return(
                            if state.count == 0 {
                                JsValue::Null
                            } else {
                                JsValue::Object(state.matches.into_handle())
                            },
                        )));
                    }
                    JsValue::Object(_) => {
                        let JsValue::Object(id) =
                            std::mem::replace(&mut self.0.converted, JsValue::Undefined)
                        else {
                            unreachable!()
                        };
                        ObjectRef::from_owned_handle(runtime.clone(), id)
                    }
                    _ => {
                        return Err(RuntimeError::Invariant(
                            "RegExpExec returned neither an object nor null",
                        ));
                    }
                };
                Ok(RegExpMatchStep::make_read(result, state.zero.clone(), {
                    let updated_0 = MatchPhase::Match(state);
                    self.0.phase = updated_0;
                    self
                }))
            }
            MatchPhase::Match(state) => Ok(RegExpMatchStep::make_primitive(
                std::mem::replace(&mut self.0.converted, JsValue::Undefined),
                ToPrimitiveHint::String,
                {
                    let updated_0 = MatchPhase::MatchString(state);
                    self.0.phase = updated_0;
                    self
                },
            )),
            MatchPhase::MatchString(mut state) => {
                let matched =
                    match runtime.string_from_primitive_jsvalue(self.0.realm, &self.0.converted)? {
                        NativeConversion::Value(value) => value,
                        NativeConversion::Throw(value) => {
                            return Ok(RegExpMatchStep::Complete(Completion::Throw(value)));
                        }
                    };
                let empty = matched.is_empty();
                let Some(next) = state.count.checked_add(1) else {
                    return Ok(RegExpMatchStep::Complete(Completion::Throw(
                        runtime.new_native_error_jsvalue(
                            self.0.realm,
                            NativeErrorKind::Range,
                            "invalid array length",
                        )?,
                    )));
                };
                // This Array is private until terminal completion, including
                // across custom exec/result callbacks. Its consecutive C/W/E
                // elements use the same constructor effect as builtin exec
                // results; inherited indexed setters must never be observed.
                let matched_value = if matches!(self.0.converted, JsValue::String(_)) {
                    std::mem::replace(&mut self.0.converted, JsValue::Undefined)
                } else {
                    runtime.into_jsvalue(Value::String(matched))?
                };
                runtime.append_fresh_array_value_jsvalue(&state.matches, matched_value)?;
                #[cfg(feature = "profiling")]
                crate::engine::api::profiling::record_owned_execution_event(
                    "regexp_result.match_append",
                );
                state.count = next;
                if empty {
                    Ok(RegExpMatchStep::make_read(
                        self.0.regexp.clone(),
                        runtime.pinned_property_key(
                            crate::engine::atom::pinned::PinnedAtom::LastIndex,
                        )?,
                        {
                            let updated_0 = MatchPhase::LastIndex(state);
                            self.0.phase = updated_0;
                            self
                        },
                    ))
                } else {
                    {
                        let updated_0 = MatchPhase::Single;
                        self.0.phase = updated_0;
                        self
                    }
                    .execute(runtime, state)
                }
            }
            MatchPhase::LastIndex(state) => Ok(RegExpMatchStep::make_primitive(
                std::mem::replace(&mut self.0.converted, JsValue::Undefined),
                ToPrimitiveHint::Number,
                {
                    let updated_0 = MatchPhase::LastIndexNumber(state);
                    self.0.phase = updated_0;
                    self
                },
            )),
            MatchPhase::LastIndexNumber(state) => {
                if matches!(self.0.converted, JsValue::Object(_)) {
                    return Err(RuntimeError::Invariant(
                        "RegExp match lastIndex conversion returned an object",
                    ));
                }
                let current =
                    match runtime.number_from_primitive_jsvalue(self.0.realm, &self.0.converted)? {
                        NativeConversion::Value(value) => Runtime::length_from_number(value),
                        NativeConversion::Throw(value) => {
                            return Ok(RegExpMatchStep::Complete(Completion::Throw(value)));
                        }
                    };
                let next = advance_string_index(&state.input, current, state.unicode);
                Ok(RegExpMatchStep::make_set(
                    self.0.regexp.clone(),
                    runtime
                        .pinned_property_key(crate::engine::atom::pinned::PinnedAtom::LastIndex)?,
                    runtime.into_jsvalue(Value::number(next as f64))?,
                    {
                        let updated_0 = MatchPhase::AdvancedSet(state);
                        self.0.phase = updated_0;
                        self
                    },
                ))
            }
            MatchPhase::InitialSet { .. } | MatchPhase::AdvancedSet(_) => Err(
                RuntimeError::Invariant("RegExp match Set received an untyped reply"),
            ),
        }
    }
}
impl Runtime {
    pub(crate) fn call_regexp_symbol_match(
        &self,
        realm: ContextId,
        invocation: NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        self.dispatch_borrowed_invocation(invocation, |invocation| {
            let mut step = RegExpMatchStep::start(self, realm, invocation, arguments)?;
            loop {
                step = match step {
                    RegExpMatchStep::Complete(result) => return Ok(result),
                    RegExpMatchStep::Primitive { mut resume } => {
                        let value = resume.take_primitive_value();
                        let hint = resume.take_primitive_hint();
                        {
                            let result = if matches!(value, JsValue::Object(_)) {
                                self.to_primitive_jsvalue(realm, value, hint)?
                            } else {
                                Completion::Return(value)
                            };
                            resume.resume(self, result)?
                        }
                    }
                    RegExpMatchStep::Read { mut resume } => {
                        let object = resume.take_read_object();
                        let key = resume.take_read_key();
                        resume.resume(self, self.get_property_in_realm(realm, &object, &key)?)?
                    }
                    RegExpMatchStep::Exec { mut resume } => {
                        let regexp = resume.take_exec_regexp();
                        let input = resume.take_exec_input();
                        resume.resume(self, self.regexp_exec_abstract(realm, regexp, input)?)?
                    }
                    RegExpMatchStep::Set { mut resume } => {
                        let object = resume.take_set_object();
                        let key = resume.take_set_key();
                        let value = resume.take_set_value();
                        resume.set(
                            self,
                            self.internal_set_jsvalue(
                                realm,
                                &object,
                                &key,
                                value,
                                JsValue::Object(object.clone().into_handle()),
                            )?,
                        )?
                    }
                };
            }
        })
    }
}

pub(crate) struct RegExpMatchStepPending {
    runtime: Runtime,
    value: Option<JsValue>,
    hint: Option<ToPrimitiveHint>,
    object: Option<ObjectRef>,
    key: Option<PropertyKey>,
    regexp: Option<JsValue>,
    input: Option<JsValue>,
}
impl RegExpMatchStepPending {
    fn new(runtime: &Runtime) -> Self {
        Self {
            runtime: runtime.clone(),
            value: None,
            hint: None,
            object: None,
            key: None,
            regexp: None,
            input: None,
        }
    }

    /// Release the internal edges still owned when the request is abandoned
    /// before its step consumed them. Taken fields are empty here.
    fn release_owned(&mut self) {
        for value in [self.value.take(), self.regexp.take(), self.input.take()]
            .into_iter()
            .flatten()
        {
            let _ = self.runtime.release_jsvalue(value);
        }
    }
}
impl Drop for RegExpMatchStepPending {
    fn drop(&mut self) {
        self.release_owned();
    }
}
impl RegExpMatchStep {
    pub(crate) fn make_primitive(
        value: JsValue,
        hint: ToPrimitiveHint,
        mut resume: RegExpMatchResume,
    ) -> Self {
        resume.0.step_pending.value = Some(value);
        resume.0.step_pending.hint = Some(hint);
        Self::Primitive { resume }
    }
    pub(crate) fn make_read(
        object: ObjectRef,
        key: PropertyKey,
        mut resume: RegExpMatchResume,
    ) -> Self {
        resume.0.step_pending.object = Some(object);
        resume.0.step_pending.key = Some(key);
        Self::Read { resume }
    }
    pub(crate) fn make_set(
        object: ObjectRef,
        key: PropertyKey,
        value: JsValue,
        mut resume: RegExpMatchResume,
    ) -> Self {
        resume.0.step_pending.object = Some(object);
        resume.0.step_pending.key = Some(key);
        resume.0.step_pending.value = Some(value);
        Self::Set { resume }
    }
    pub(crate) fn make_exec(
        regexp: JsValue,
        input: JsValue,
        mut resume: RegExpMatchResume,
    ) -> Self {
        resume.0.step_pending.regexp = Some(regexp);
        resume.0.step_pending.input = Some(input);
        Self::Exec { resume }
    }
}
impl RegExpMatchResume {
    pub(crate) fn take_primitive_value(&mut self) -> JsValue {
        self.0
            .step_pending
            .value
            .take()
            .expect("RegExpMatchStep::Primitive lost value")
    }
    pub(crate) fn take_primitive_hint(&mut self) -> ToPrimitiveHint {
        self.0
            .step_pending
            .hint
            .take()
            .expect("RegExpMatchStep::Primitive lost hint")
    }

    pub(crate) fn take_read_object(&mut self) -> ObjectRef {
        self.0
            .step_pending
            .object
            .take()
            .expect("RegExpMatchStep::Read lost object")
    }
    pub(crate) fn take_read_key(&mut self) -> PropertyKey {
        self.0
            .step_pending
            .key
            .take()
            .expect("RegExpMatchStep::Read lost key")
    }

    pub(crate) fn take_set_object(&mut self) -> ObjectRef {
        self.0
            .step_pending
            .object
            .take()
            .expect("RegExpMatchStep::Set lost object")
    }
    pub(crate) fn take_set_key(&mut self) -> PropertyKey {
        self.0
            .step_pending
            .key
            .take()
            .expect("RegExpMatchStep::Set lost key")
    }
    pub(crate) fn take_set_value(&mut self) -> JsValue {
        self.0
            .step_pending
            .value
            .take()
            .expect("RegExpMatchStep::Set lost value")
    }

    pub(crate) fn take_exec_regexp(&mut self) -> JsValue {
        self.0
            .step_pending
            .regexp
            .take()
            .expect("RegExpMatchStep::Exec lost regexp")
    }
    pub(crate) fn take_exec_input(&mut self) -> JsValue {
        self.0
            .step_pending
            .input
            .take()
            .expect("RegExpMatchStep::Exec lost input")
    }
}

const _: () = assert!(std::mem::size_of::<RegExpMatchStep>() <= 64);

// S11 all-domain protocol bound; inline completion stays allocation-free.
const _: () = assert!(std::mem::size_of::<RegExpMatchStep>() <= 64);
