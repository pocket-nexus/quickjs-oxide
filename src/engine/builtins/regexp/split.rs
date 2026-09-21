//! `RegExp.prototype[Symbol.split]`.

use super::match_protocol::advance_string_index;
use crate::engine::api::error::NativeErrorKind;
use crate::engine::api::runtime::Runtime;
use crate::engine::api::runtime_error::RuntimeError;
use crate::engine::heap::ContextId;
use crate::engine::object::{ObjectRef, PropertyKey, operations::InternalSetResult};
use crate::engine::value::conversion::NativeConversion;

use crate::engine::value::{JsString, JsValue, Value};
use crate::engine::vm::call::{ConstructorRef, NativeArguments, NativeInvocation};
use crate::engine::vm::{Completion, ToPrimitiveHint};

impl Runtime {
    /// Rust port of pinned QuickJS `js_regexp_Symbol_split`.
    pub(crate) fn call_regexp_symbol_split(
        &self,
        realm: ContextId,
        invocation: NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        self.dispatch_borrowed_invocation(invocation, |invocation| {
            finish(
                self,
                realm,
                RegExpSplitStep::start(self, realm, invocation, arguments)?,
            )
        })
    }

    fn append_regexp_split_value(
        &self,
        result: &ObjectRef,
        length: &mut u32,
        value: JsValue,
    ) -> Result<(), RuntimeError> {
        let index = *length;
        let Some(next) = index.checked_add(1) else {
            self.release_jsvalue(value)?;
            return Err(RuntimeError::Invariant(
                "RegExp split output index exceeded Uint32",
            ));
        };
        // The output is an intrinsic fresh Array, never the species-created
        // splitter and never exposed to exec/capture callbacks. Preserve the
        // original allocation and append timing using the shared constructor
        // kernel, which defines own C/W/E data without inherited setters.
        self.append_fresh_array_value_jsvalue(result, value)?;
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_execution_event("regexp_result.split_append");
        *length = next;
        Ok(())
    }
}

pub(crate) enum RegExpSplitStep {
    Complete(Completion),
    Primitive { resume: RegExpSplitResume },
    Read { resume: RegExpSplitResume },
    Species { resume: RegExpSplitResume },
    Construct { resume: RegExpSplitResume },
    Set { resume: RegExpSplitResume },
    Exec { resume: RegExpSplitResume },
}
pub(crate) struct RegExpSplitResume(Box<RegExpSplitResumeState>);
impl std::ops::Deref for RegExpSplitResume {
    type Target = RegExpSplitResumeState;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for RegExpSplitResume {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
const _: () = assert!(std::mem::size_of::<RegExpSplitResume>() <= 8);
pub(crate) struct RegExpSplitResumeState {
    limit_value: JsValue,
    input_value: JsValue,
    step_pending: RegExpSplitStepPending,
    realm: ContextId,
    phase: Phase,
}
impl Drop for RegExpSplitResumeState {
    fn drop(&mut self) {
        let runtime = &self.step_pending.runtime;
        let _ =
            runtime.release_jsvalue(std::mem::replace(&mut self.limit_value, JsValue::Undefined));
        let _ =
            runtime.release_jsvalue(std::mem::replace(&mut self.input_value, JsValue::Undefined));
    }
}
enum Phase {
    Vacant,
    Input {
        regexp: ObjectRef,
    },
    Species {
        regexp: ObjectRef,
        input: JsString,
    },
    Flags {
        regexp: ObjectRef,
        input: JsString,
        constructor: ConstructorRef,
    },
    FlagsPrimitive {
        regexp: ObjectRef,
        input: JsString,
        constructor: ConstructorRef,
    },
    Construct {
        input: JsString,
        unicode: bool,
    },
    Limit(SplitState),
    Empty(SplitState),
    Set(SplitState),
    Exec(SplitState),
    End {
        state: SplitState,
        matched: ObjectRef,
    },
    EndPrimitive {
        state: SplitState,
        matched: ObjectRef,
    },
    Count {
        state: SplitState,
        matched: ObjectRef,
    },
    CountPrimitive {
        state: SplitState,
        matched: ObjectRef,
    },
    Capture {
        state: SplitState,
        matched: ObjectRef,
        index: u64,
        count: u64,
    },
}
struct SplitState {
    input_value: JsValue,
    input: JsString,
    splitter: ObjectRef,
    result: ObjectRef,
    unicode: bool,
    limit: u32,
    length: u32,
    p: usize,
    q: usize,
}
impl Drop for SplitState {
    fn drop(&mut self) {
        let _ = self
            .splitter
            .runtime()
            .release_jsvalue(std::mem::replace(&mut self.input_value, JsValue::Undefined));
    }
}
impl SplitState {
    fn complete(self) -> RegExpSplitStep {
        RegExpSplitStep::Complete(Completion::Return(JsValue::Object(
            self.result.clone().into_handle(),
        )))
    }
    fn append(&mut self, runtime: &Runtime, value: JsValue) -> Result<(), RuntimeError> {
        runtime.append_regexp_split_value(&self.result, &mut self.length, value)
    }
    fn advance(&mut self) -> Result<(), RuntimeError> {
        self.q = usize::try_from(advance_string_index(
            &self.input,
            self.q as u64,
            self.unicode,
        ))
        .map_err(|_| RuntimeError::Invariant("advanced split index did not fit usize"))?;
        Ok(())
    }
    fn next(
        mut self,
        runtime: &Runtime,
        realm: ContextId,
    ) -> Result<RegExpSplitStep, RuntimeError> {
        if self.q >= self.input.len() {
            let value = Value::String(
                self.input
                    .sub_string(self.p.min(self.input.len()), self.input.len()),
            );
            self.append(runtime, runtime.into_jsvalue(value)?)?;
            return Ok(self.complete());
        }
        let value = JsValue::Int(i32::try_from(self.q).map_err(|_| {
            RuntimeError::Invariant("RegExp split index exceeded signed String range")
        })?);
        Ok(RegExpSplitStep::make_set(
            self.splitter.clone(),
            runtime.pinned_property_key(crate::engine::atom::pinned::PinnedAtom::LastIndex)?,
            value,
            RegExpSplitResume(Box::new(RegExpSplitResumeState {
                limit_value: JsValue::Undefined,
                input_value: JsValue::Undefined,
                step_pending: RegExpSplitStepPending::new(runtime),
                realm,
                phase: Phase::Set(self),
            })),
        ))
    }
    fn execute(
        self,
        runtime: &Runtime,
        realm: ContextId,
        empty: bool,
    ) -> Result<RegExpSplitStep, RuntimeError> {
        let input = runtime.dup_jsvalue(&self.input_value)?;
        let regexp = JsValue::Object(self.splitter.clone().into_handle());
        Ok(RegExpSplitStep::make_exec(
            regexp,
            input,
            RegExpSplitResume(Box::new(RegExpSplitResumeState {
                limit_value: JsValue::Undefined,
                input_value: JsValue::Undefined,
                step_pending: RegExpSplitStepPending::new(runtime),
                realm,
                phase: if empty {
                    Phase::Empty(self)
                } else {
                    Phase::Exec(self)
                },
            })),
        ))
    }
    fn captures(
        mut self,
        runtime: &Runtime,
        realm: ContextId,
        matched: ObjectRef,
        index: u64,
        count: u64,
    ) -> Result<RegExpSplitStep, RuntimeError> {
        if index >= count {
            self.q = self.p;
            return self.next(runtime, realm);
        }
        Ok(RegExpSplitStep::make_read(
            matched.clone(),
            runtime.intern_property_key(&index.to_string())?,
            RegExpSplitResume(Box::new(RegExpSplitResumeState {
                limit_value: JsValue::Undefined,
                input_value: JsValue::Undefined,
                step_pending: RegExpSplitStepPending::new(runtime),
                realm,
                phase: Phase::Capture {
                    state: self,
                    matched,
                    index,
                    count,
                },
            })),
        ))
    }
}
impl RegExpSplitStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        let NativeInvocation::Call { this_value } = invocation else {
            return Err(RuntimeError::Invariant(
                "RegExp @@split did not receive a generic invocation",
            ));
        };
        let JsValue::Object(id) = this_value else {
            return Ok(Self::Complete(Completion::Throw(
                runtime.new_native_error_jsvalue(realm, NativeErrorKind::Type, "not an object")?,
            )));
        };
        let regexp = ObjectRef::from_borrowed_handle(runtime.clone(), *id)?;
        let mut resume = RegExpSplitResume::new(runtime, realm, Phase::Input { regexp });
        resume.0.limit_value = runtime.dup_jsvalue(arguments.readable.get(1).ok_or(
            RuntimeError::Invariant("RegExp @@split limit argv was not padded"),
        )?)?;
        let input = runtime.dup_jsvalue(arguments.readable.first().ok_or(
            RuntimeError::Invariant("RegExp @@split input argv was not padded"),
        )?)?;
        Ok(Self::make_primitive(input, ToPrimitiveHint::String, resume))
    }
}
impl RegExpSplitResume {
    fn new(runtime: &Runtime, realm: ContextId, phase: Phase) -> Self {
        Self(Box::new(RegExpSplitResumeState {
            step_pending: RegExpSplitStepPending::new(runtime),
            realm,
            phase,
            limit_value: JsValue::Undefined,
            input_value: JsValue::Undefined,
        }))
    }
    pub(crate) fn species(
        mut self,
        runtime: &Runtime,
        result: NativeConversion<ConstructorRef>,
    ) -> Result<RegExpSplitStep, RuntimeError> {
        let constructor = match result {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => {
                return Ok(RegExpSplitStep::Complete(Completion::Throw(value)));
            }
        };
        let Phase::Species { regexp, input } = std::mem::replace(&mut self.0.phase, Phase::Vacant)
        else {
            return Err(RuntimeError::Invariant(
                "RegExp split species reply in wrong phase",
            ));
        };
        let object = regexp.clone();
        self.0.phase = Phase::Flags {
            regexp,
            input,
            constructor,
        };
        let key = runtime.pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Flags)?;
        Ok(RegExpSplitStep::make_read(object, key, self))
    }
    pub(crate) fn set(
        self,
        runtime: &Runtime,
        result: NativeConversion<InternalSetResult>,
    ) -> Result<RegExpSplitStep, RuntimeError> {
        let key =
            runtime.pinned_property_key(crate::engine::atom::pinned::PinnedAtom::LastIndex)?;
        let result = match runtime.finish_set_property_or_throw(self.0.realm, &key, result)? {
            Some(value) => Completion::Throw(value),
            None => Completion::Return(JsValue::Undefined),
        };
        self.resume(runtime, result)
    }
    fn string_reply(
        &mut self,
        runtime: &Runtime,
    ) -> Result<NativeConversion<JsString>, RuntimeError> {
        runtime.string_from_primitive_jsvalue(
            self.0.realm,
            self.0
                .step_pending
                .value
                .as_ref()
                .expect("split primitive reply"),
        )
    }
    fn number_reply(&mut self, runtime: &Runtime) -> Result<NativeConversion<f64>, RuntimeError> {
        let value = self
            .0
            .step_pending
            .value
            .take()
            .expect("split numeric reply");
        let number = runtime.number_from_primitive_jsvalue(self.0.realm, &value);
        runtime.release_jsvalue(value)?;
        number
    }
    pub(crate) fn resume(
        mut self,
        runtime: &Runtime,
        result: Completion,
    ) -> Result<RegExpSplitStep, RuntimeError> {
        let value = match result {
            Completion::Return(value) => value,
            Completion::Throw(value) => {
                return Ok(RegExpSplitStep::Complete(Completion::Throw(value)));
            }
        };
        self.0.step_pending.value = Some(value);
        let realm = self.0.realm;
        match std::mem::replace(&mut self.0.phase, Phase::Vacant) {
            Phase::Input { regexp } => {
                let input = match self.string_reply(runtime)? {
                    NativeConversion::Value(value) => value,
                    NativeConversion::Throw(value) => {
                        return Ok(RegExpSplitStep::Complete(Completion::Throw(value)));
                    }
                };
                let value = self.0.step_pending.value.take().unwrap();
                self.0.input_value = if matches!(value, JsValue::String(_)) {
                    value
                } else {
                    runtime.release_jsvalue(value)?;
                    runtime.into_jsvalue(Value::String(input.clone()))?
                };
                let object = regexp.clone();
                self.0.phase = Phase::Species { regexp, input };
                Ok(RegExpSplitStep::make_species(object, self))
            }
            Phase::Flags {
                regexp,
                input,
                constructor,
            } => {
                self.0.phase = Phase::FlagsPrimitive {
                    regexp,
                    input,
                    constructor,
                };
                let value = self.0.step_pending.value.take().unwrap();
                Ok(RegExpSplitStep::make_primitive(
                    value,
                    ToPrimitiveHint::String,
                    self,
                ))
            }
            Phase::FlagsPrimitive {
                regexp,
                input,
                constructor,
            } => {
                let flags = match self.string_reply(runtime)? {
                    NativeConversion::Value(value) => value,
                    NativeConversion::Throw(value) => {
                        return Ok(RegExpSplitStep::Complete(Completion::Throw(value)));
                    }
                };
                let unicode = flags
                    .utf16_units()
                    .any(|unit| unit == u16::from(b'u') || unit == u16::from(b'v'));
                let sticky = flags.utf16_units().any(|unit| unit == u16::from(b'y'));
                let flags = if sticky {
                    flags
                } else {
                    flags.try_concat(&JsString::from_static("y"))?
                };
                self.0.step_pending.arguments = Some(Vec::new());
                if self
                    .0
                    .step_pending
                    .arguments
                    .as_mut()
                    .unwrap()
                    .try_reserve_exact(2)
                    .is_err()
                {
                    return Ok(RegExpSplitStep::Complete(Completion::Throw(
                        runtime.new_native_error_jsvalue(
                            realm,
                            NativeErrorKind::Internal,
                            "out of memory",
                        )?,
                    )));
                }
                self.0
                    .step_pending
                    .arguments
                    .as_mut()
                    .unwrap()
                    .push(JsValue::Object(regexp.into_handle()));
                let value = self.0.step_pending.value.take().unwrap();
                let flags_value = if sticky && matches!(value, JsValue::String(_)) {
                    value
                } else {
                    runtime.release_jsvalue(value)?;
                    runtime.into_jsvalue(Value::String(flags))?
                };
                self.0
                    .step_pending
                    .arguments
                    .as_mut()
                    .unwrap()
                    .push(flags_value);
                self.0.step_pending.constructor = Some(constructor);
                self.0.phase = Phase::Construct { input, unicode };
                Ok(RegExpSplitStep::Construct { resume: self })
            }
            Phase::Construct { input, unicode } => {
                let splitter = match self.0.step_pending.value.take().unwrap() {
                    JsValue::Object(id) => ObjectRef::from_owned_handle(runtime.clone(), id),
                    value => {
                        runtime.release_jsvalue(value)?;
                        return Err(RuntimeError::Invariant(
                            "RegExp species constructor returned a primitive",
                        ));
                    }
                };
                let result = runtime.new_array(realm)?;
                let state = SplitState {
                    input,
                    input_value: std::mem::replace(&mut self.0.input_value, JsValue::Undefined),
                    splitter,
                    result,
                    unicode,
                    limit: u32::MAX,
                    length: 0,
                    p: 0,
                    q: 0,
                };
                if matches!(self.0.limit_value, JsValue::Undefined) {
                    return Self::after_limit(state, runtime, realm);
                }
                let value = std::mem::replace(&mut self.0.limit_value, JsValue::Undefined);
                self.0.phase = Phase::Limit(state);
                Ok(RegExpSplitStep::make_primitive(
                    value,
                    ToPrimitiveHint::Number,
                    self,
                ))
            }
            Phase::Limit(mut state) => {
                let number = match self.number_reply(runtime)? {
                    NativeConversion::Value(value) => value,
                    NativeConversion::Throw(value) => {
                        return Ok(RegExpSplitStep::Complete(Completion::Throw(value)));
                    }
                };
                state.limit = Runtime::to_uint32_number(number);
                Self::after_limit(state, runtime, realm)
            }
            Phase::Empty(mut state) => {
                let value = self.0.step_pending.value.take().unwrap();
                let valid = matches!(value, JsValue::Null | JsValue::Object(_));
                let empty = matches!(value, JsValue::Null);
                runtime.release_jsvalue(value)?;
                if !valid {
                    return Err(RuntimeError::Invariant(
                        "RegExpExec returned neither an object nor null",
                    ));
                }
                if empty {
                    let input = runtime.dup_jsvalue(&state.input_value)?;
                    state.append(runtime, input)?;
                }
                Ok(state.complete())
            }
            Phase::Set(state) => {
                runtime.release_jsvalue(self.0.step_pending.value.take().unwrap())?;
                state.execute(runtime, realm, false)
            }
            Phase::Exec(mut state) => match self.0.step_pending.value.take().unwrap() {
                JsValue::Null => {
                    state.advance()?;
                    state.next(runtime, realm)
                }
                JsValue::Object(id) => {
                    let matched = ObjectRef::from_owned_handle(runtime.clone(), id);
                    let object = state.splitter.clone();
                    self.0.phase = Phase::End { state, matched };
                    let key = runtime
                        .pinned_property_key(crate::engine::atom::pinned::PinnedAtom::LastIndex)?;
                    Ok(RegExpSplitStep::make_read(object, key, self))
                }
                value => {
                    runtime.release_jsvalue(value)?;
                    Err(RuntimeError::Invariant(
                        "RegExpExec returned neither an object nor null",
                    ))
                }
            },
            Phase::End { state, matched } => {
                self.0.phase = Phase::EndPrimitive { state, matched };
                let value = self.0.step_pending.value.take().unwrap();
                Ok(RegExpSplitStep::make_primitive(
                    value,
                    ToPrimitiveHint::Number,
                    self,
                ))
            }
            Phase::EndPrimitive { mut state, matched } => {
                let number = match self.number_reply(runtime)? {
                    NativeConversion::Value(value) => value,
                    NativeConversion::Throw(value) => {
                        return Ok(RegExpSplitStep::Complete(Completion::Throw(value)));
                    }
                };
                let end = usize::try_from(
                    Runtime::length_from_number(number).min(state.input.len() as u64),
                )
                .map_err(|_| RuntimeError::Invariant("split end index did not fit usize"))?;
                if end == state.p {
                    state.advance()?;
                    return state.next(runtime, realm);
                }
                let part = runtime
                    .into_jsvalue(Value::String(state.input.sub_string(state.p, state.q)))?;
                state.append(runtime, part)?;
                if state.length == state.limit {
                    return Ok(state.complete());
                }
                state.p = end;
                let object = matched.clone();
                self.0.phase = Phase::Count { state, matched };
                let key =
                    runtime.pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Length)?;
                Ok(RegExpSplitStep::make_read(object, key, self))
            }
            Phase::Count { state, matched } => {
                self.0.phase = Phase::CountPrimitive { state, matched };
                let value = self.0.step_pending.value.take().unwrap();
                Ok(RegExpSplitStep::make_primitive(
                    value,
                    ToPrimitiveHint::Number,
                    self,
                ))
            }
            Phase::CountPrimitive { state, matched } => {
                let number = match self.number_reply(runtime)? {
                    NativeConversion::Value(value) => value,
                    NativeConversion::Throw(value) => {
                        return Ok(RegExpSplitStep::Complete(Completion::Throw(value)));
                    }
                };
                state.captures(
                    runtime,
                    realm,
                    matched,
                    1,
                    Runtime::length_from_number(number),
                )
            }
            Phase::Capture {
                mut state,
                matched,
                index,
                count,
            } => {
                state.append(runtime, self.0.step_pending.value.take().unwrap())?;
                if state.length == state.limit {
                    return Ok(state.complete());
                }
                state.captures(runtime, realm, matched, index + 1, count)
            }
            Phase::Species { .. } | Phase::Vacant => Err(RuntimeError::Invariant(
                "RegExp split completion in species phase",
            )),
        }
    }
    fn after_limit(
        state: SplitState,
        runtime: &Runtime,
        realm: ContextId,
    ) -> Result<RegExpSplitStep, RuntimeError> {
        if state.limit == 0 {
            return Ok(state.complete());
        }
        if state.input.is_empty() {
            return state.execute(runtime, realm, true);
        }
        // Preserve key allocation before the first observable splitter write.
        runtime.pinned_property_key(crate::engine::atom::pinned::PinnedAtom::LastIndex)?;
        runtime.pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Length)?;
        state.next(runtime, realm)
    }
}
fn finish(
    runtime: &Runtime,
    realm: ContextId,
    mut step: RegExpSplitStep,
) -> Result<Completion, RuntimeError> {
    loop {
        step = match step {
            RegExpSplitStep::Complete(result) => return Ok(result),
            RegExpSplitStep::Primitive { mut resume } => {
                let value = resume.take_primitive_value();
                let hint = resume.take_primitive_hint();
                {
                    let result = if matches!(value, JsValue::Object(_)) {
                        runtime.to_primitive_jsvalue(realm, value, hint)?
                    } else {
                        Completion::Return(value)
                    };
                    resume.resume(runtime, result)?
                }
            }
            RegExpSplitStep::Read { mut resume } => {
                let object = resume.take_read_object();
                let key = resume.take_read_key();
                resume.resume(
                    runtime,
                    runtime.internal_get_jsvalue(
                        realm,
                        &object,
                        &key,
                        JsValue::Object(object.clone().into_handle()),
                    )?,
                )?
            }
            RegExpSplitStep::Species { mut resume } => {
                let regexp = resume.take_species_regexp();
                resume.species(runtime, runtime.regexp_species_constructor(realm, &regexp)?)?
            }
            RegExpSplitStep::Construct { mut resume } => {
                let constructor = resume.take_construct_constructor();
                let arguments = resume.take_construct_arguments();
                resume.resume(
                    runtime,
                    runtime.construct_internal_jsvalue(
                        realm,
                        &constructor,
                        crate::engine::vm::call::ConstructNewTarget::Validated(constructor.clone()),
                        arguments,
                    )?,
                )?
            }
            RegExpSplitStep::Set { mut resume } => {
                let object = resume.take_set_object();
                let key = resume.take_set_key();
                let value = resume.take_set_value();
                resume.set(
                    runtime,
                    runtime.internal_set_jsvalue(
                        realm,
                        &object,
                        &key,
                        value,
                        JsValue::Object(object.clone().into_handle()),
                    )?,
                )?
            }
            RegExpSplitStep::Exec { mut resume } => {
                let regexp = resume.take_exec_regexp();
                let input = resume.take_exec_input();
                resume.resume(runtime, runtime.regexp_exec_abstract(realm, regexp, input)?)?
            }
        }
    }
}

pub(crate) struct RegExpSplitStepPending {
    runtime: Runtime,
    value: Option<JsValue>,
    hint: Option<ToPrimitiveHint>,
    object: Option<ObjectRef>,
    key: Option<PropertyKey>,
    regexp: Option<ObjectRef>,
    constructor: Option<ConstructorRef>,
    arguments: Option<Vec<JsValue>>,
    exec_regexp: Option<JsValue>,
    input: Option<JsValue>,
}
impl RegExpSplitStepPending {
    fn new(runtime: &Runtime) -> Self {
        Self {
            runtime: runtime.clone(),
            value: None,
            hint: None,
            object: None,
            key: None,
            regexp: None,
            constructor: None,
            arguments: None,
            exec_regexp: None,
            input: None,
        }
    }

    /// Release the internal edges still owned when the request is abandoned
    /// before its step consumed them. Taken fields are empty here.
    fn release_owned(&mut self) {
        for value in [
            self.value.take(),
            self.exec_regexp.take(),
            self.input.take(),
        ]
        .into_iter()
        .flatten()
        {
            let _ = self.runtime.release_jsvalue(value);
        }
        for argument in self.arguments.take().into_iter().flatten() {
            let _ = self.runtime.release_jsvalue(argument);
        }
    }
}
impl Drop for RegExpSplitStepPending {
    fn drop(&mut self) {
        self.release_owned();
    }
}
impl RegExpSplitStep {
    pub(crate) fn make_primitive(
        value: JsValue,
        hint: ToPrimitiveHint,
        mut resume: RegExpSplitResume,
    ) -> Self {
        resume.0.step_pending.value = Some(value);
        resume.0.step_pending.hint = Some(hint);
        Self::Primitive { resume }
    }
    pub(crate) fn make_read(
        object: ObjectRef,
        key: PropertyKey,
        mut resume: RegExpSplitResume,
    ) -> Self {
        resume.0.step_pending.object = Some(object);
        resume.0.step_pending.key = Some(key);
        Self::Read { resume }
    }
    pub(crate) fn make_species(regexp: ObjectRef, mut resume: RegExpSplitResume) -> Self {
        resume.0.step_pending.regexp = Some(regexp);
        Self::Species { resume }
    }
    pub(crate) fn make_set(
        object: ObjectRef,
        key: PropertyKey,
        value: JsValue,
        mut resume: RegExpSplitResume,
    ) -> Self {
        resume.0.step_pending.object = Some(object);
        resume.0.step_pending.key = Some(key);
        resume.0.step_pending.value = Some(value);
        Self::Set { resume }
    }
    pub(crate) fn make_exec(
        regexp: JsValue,
        input: JsValue,
        mut resume: RegExpSplitResume,
    ) -> Self {
        resume.0.step_pending.exec_regexp = Some(regexp);
        resume.0.step_pending.input = Some(input);
        Self::Exec { resume }
    }
}
impl RegExpSplitResume {
    pub(crate) fn take_primitive_value(&mut self) -> JsValue {
        self.0
            .step_pending
            .value
            .take()
            .expect("RegExpSplitStep::Primitive lost value")
    }
    pub(crate) fn take_primitive_hint(&mut self) -> ToPrimitiveHint {
        self.0
            .step_pending
            .hint
            .take()
            .expect("RegExpSplitStep::Primitive lost hint")
    }

    pub(crate) fn take_read_object(&mut self) -> ObjectRef {
        self.0
            .step_pending
            .object
            .take()
            .expect("RegExpSplitStep::Read lost object")
    }
    pub(crate) fn take_read_key(&mut self) -> PropertyKey {
        self.0
            .step_pending
            .key
            .take()
            .expect("RegExpSplitStep::Read lost key")
    }

    pub(crate) fn take_species_regexp(&mut self) -> ObjectRef {
        self.0
            .step_pending
            .regexp
            .take()
            .expect("RegExpSplitStep::Species lost regexp")
    }

    pub(crate) fn take_construct_constructor(&mut self) -> ConstructorRef {
        self.0
            .step_pending
            .constructor
            .take()
            .expect("RegExpSplitStep::Construct lost constructor")
    }
    pub(crate) fn take_construct_arguments(&mut self) -> Vec<JsValue> {
        self.0
            .step_pending
            .arguments
            .take()
            .expect("RegExpSplitStep::Construct lost arguments")
    }

    pub(crate) fn take_set_object(&mut self) -> ObjectRef {
        self.0
            .step_pending
            .object
            .take()
            .expect("RegExpSplitStep::Set lost object")
    }
    pub(crate) fn take_set_key(&mut self) -> PropertyKey {
        self.0
            .step_pending
            .key
            .take()
            .expect("RegExpSplitStep::Set lost key")
    }
    pub(crate) fn take_set_value(&mut self) -> JsValue {
        self.0
            .step_pending
            .value
            .take()
            .expect("RegExpSplitStep::Set lost value")
    }

    pub(crate) fn take_exec_regexp(&mut self) -> JsValue {
        self.0
            .step_pending
            .exec_regexp
            .take()
            .expect("RegExpSplitStep::Exec lost regexp")
    }
    pub(crate) fn take_exec_input(&mut self) -> JsValue {
        self.0
            .step_pending
            .input
            .take()
            .expect("RegExpSplitStep::Exec lost input")
    }
}

const _: () = assert!(std::mem::size_of::<RegExpSplitStep>() <= 64);

// S11 all-domain protocol bound; inline completion stays allocation-free.
const _: () = assert!(std::mem::size_of::<RegExpSplitStep>() <= 64);
