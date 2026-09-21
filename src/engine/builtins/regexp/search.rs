//! RegExp search restores lastIndex only after successful execution and reread.
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
pub(crate) enum RegExpSearchStep {
    Complete(Completion),
    Primitive { resume: RegExpSearchResume },
    Read { resume: RegExpSearchResume },
    Set { resume: RegExpSearchResume },
    Exec { resume: RegExpSearchResume },
}
pub(crate) struct RegExpSearchResume(Box<RegExpSearchResumeState>);
impl std::ops::Deref for RegExpSearchResume {
    type Target = RegExpSearchResumeState;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for RegExpSearchResume {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
const _: () = assert!(std::mem::size_of::<RegExpSearchResume>() <= 8);
pub(crate) struct RegExpSearchResumeState {
    step_pending: RegExpSearchStepPending,
    realm: ContextId,
    regexp: ObjectRef,
    previous: JsValue,
    result: JsValue,
    converted: JsValue,
    phase: SearchPhase,
}
impl Drop for RegExpSearchResumeState {
    fn drop(&mut self) {
        for value in [&mut self.previous, &mut self.result, &mut self.converted] {
            let _ = self
                .step_pending
                .runtime
                .release_jsvalue(std::mem::replace(value, JsValue::Undefined));
        }
    }
}
enum SearchPhase {
    Input,
    Previous(JsString),
    InitialSet(JsString),
    Exec,
    Current,
    Restored,
    Index,
}
impl RegExpSearchStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        let NativeInvocation::Call { this_value } = invocation else {
            return Err(RuntimeError::Invariant(
                "RegExp @@search did not receive a generic invocation",
            ));
        };
        let JsValue::Object(regexp) = this_value else {
            return Ok(Self::Complete(Completion::Throw(
                runtime.new_native_error_jsvalue(realm, NativeErrorKind::Type, "not an object")?,
            )));
        };
        let regexp = ObjectRef::from_borrowed_handle(runtime.clone(), *regexp)?;
        Ok(Self::make_primitive(
            runtime.dup_jsvalue(arguments.readable.first().ok_or(RuntimeError::Invariant(
                "RegExp @@search input argv was not padded",
            ))?)?,
            RegExpSearchResume(Box::new(RegExpSearchResumeState {
                step_pending: RegExpSearchStepPending::new(runtime),
                realm,
                regexp,
                previous: JsValue::Undefined,
                result: JsValue::Undefined,
                converted: JsValue::Undefined,
                phase: SearchPhase::Input,
            })),
        ))
    }
}
impl RegExpSearchResume {
    fn execute(
        mut self,
        runtime: &Runtime,
        input: JsString,
    ) -> Result<RegExpSearchStep, RuntimeError> {
        let input = runtime.into_jsvalue(Value::String(input))?;
        self.phase = SearchPhase::Exec;
        Ok(RegExpSearchStep::make_exec(
            JsValue::Object(self.regexp.clone().into_handle()),
            input,
            self,
        ))
    }
    fn result(mut self, runtime: &Runtime) -> Result<RegExpSearchStep, RuntimeError> {
        match &self.result {
            JsValue::Null => Ok(RegExpSearchStep::Complete(Completion::Return(
                JsValue::Int(-1),
            ))),
            JsValue::Object(_) => {
                let key =
                    runtime.pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Index)?;
                let JsValue::Object(id) = std::mem::replace(&mut self.result, JsValue::Undefined)
                else {
                    unreachable!()
                };
                let object = ObjectRef::from_owned_handle(runtime.clone(), id);
                self.phase = SearchPhase::Index;
                Ok(RegExpSearchStep::make_read(object, key, self))
            }
            _ => Err(RuntimeError::Invariant(
                "RegExpExec returned neither an object nor null",
            )),
        }
    }
    pub(crate) fn resume(
        mut self,
        runtime: &Runtime,
        result: Completion,
    ) -> Result<RegExpSearchStep, RuntimeError> {
        let value = match result {
            Completion::Return(value) => value,
            Completion::Throw(value) => {
                return Ok(RegExpSearchStep::Complete(Completion::Throw(value)));
            }
        };
        let old = std::mem::replace(&mut self.converted, value);
        runtime.release_jsvalue(old)?;
        match std::mem::replace(&mut self.phase, SearchPhase::Index) {
            SearchPhase::Input => {
                let input =
                    match runtime.string_from_primitive_jsvalue(self.realm, &self.converted)? {
                        NativeConversion::Value(value) => value,
                        NativeConversion::Throw(value) => {
                            return Ok(RegExpSearchStep::Complete(Completion::Throw(value)));
                        }
                    };
                let key = runtime
                    .pinned_property_key(crate::engine::atom::pinned::PinnedAtom::LastIndex)?;
                self.phase = SearchPhase::Previous(input);
                Ok(RegExpSearchStep::make_read(self.regexp.clone(), key, self))
            }
            SearchPhase::Previous(input) => {
                self.previous = std::mem::replace(&mut self.converted, JsValue::Undefined);
                let zero = crate::engine::value::collection_key::same_value(
                    &runtime.0.state.borrow().heap,
                    &self.previous.as_raw(),
                    &JsValue::Int(0).as_raw(),
                );
                if zero {
                    return self.execute(runtime, input);
                }
                let key = runtime
                    .pinned_property_key(crate::engine::atom::pinned::PinnedAtom::LastIndex)?;
                self.phase = SearchPhase::InitialSet(input);
                Ok(RegExpSearchStep::make_set(
                    self.regexp.clone(),
                    key,
                    JsValue::Int(0),
                    self,
                ))
            }
            SearchPhase::Exec => {
                self.result = std::mem::replace(&mut self.converted, JsValue::Undefined);
                let key = runtime
                    .pinned_property_key(crate::engine::atom::pinned::PinnedAtom::LastIndex)?;
                self.phase = SearchPhase::Current;
                Ok(RegExpSearchStep::make_read(self.regexp.clone(), key, self))
            }
            SearchPhase::Current => {
                let equal = crate::engine::value::collection_key::same_value(
                    &runtime.0.state.borrow().heap,
                    &self.converted.as_raw(),
                    &self.previous.as_raw(),
                );
                if equal {
                    return self.result(runtime);
                }
                let key = runtime
                    .pinned_property_key(crate::engine::atom::pinned::PinnedAtom::LastIndex)?;
                let previous = std::mem::replace(&mut self.previous, JsValue::Undefined);
                self.phase = SearchPhase::Restored;
                Ok(RegExpSearchStep::make_set(
                    self.regexp.clone(),
                    key,
                    previous,
                    self,
                ))
            }
            SearchPhase::Index => Ok(RegExpSearchStep::Complete(Completion::Return(
                std::mem::replace(&mut self.converted, JsValue::Undefined),
            ))),
            _ => Err(RuntimeError::Invariant(
                "RegExp search Set received an untyped reply",
            )),
        }
    }
    pub(crate) fn set(
        mut self,
        runtime: &Runtime,
        result: NativeConversion<InternalSetResult>,
    ) -> Result<RegExpSearchStep, RuntimeError> {
        let key =
            runtime.pinned_property_key(crate::engine::atom::pinned::PinnedAtom::LastIndex)?;
        if let Some(value) = runtime.finish_set_property_or_throw(self.realm, &key, result)? {
            return Ok(RegExpSearchStep::Complete(Completion::Throw(value)));
        }
        match std::mem::replace(&mut self.phase, SearchPhase::Index) {
            SearchPhase::InitialSet(input) => self.execute(runtime, input),
            SearchPhase::Restored => self.result(runtime),
            _ => Err(RuntimeError::Invariant(
                "RegExp search received an unexpected Set reply",
            )),
        }
    }
}
impl Runtime {
    pub(crate) fn call_regexp_symbol_search(
        &self,
        realm: ContextId,
        invocation: NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        self.dispatch_borrowed_invocation(invocation, |invocation| {
            let mut step = RegExpSearchStep::start(self, realm, invocation, arguments)?;
            loop {
                step = match step {
                    RegExpSearchStep::Complete(result) => return Ok(result),
                    RegExpSearchStep::Primitive { mut resume } => {
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
                    RegExpSearchStep::Read { mut resume } => {
                        let object = resume.take_read_object();
                        let key = resume.take_read_key();
                        resume.resume(self, self.get_property_in_realm(realm, &object, &key)?)?
                    }
                    RegExpSearchStep::Exec { mut resume } => {
                        let regexp = resume.take_exec_regexp();
                        let input = resume.take_exec_input();
                        resume.resume(self, self.regexp_exec_abstract(realm, regexp, input)?)?
                    }
                    RegExpSearchStep::Set { mut resume } => {
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

pub(crate) struct RegExpSearchStepPending {
    runtime: Runtime,
    value: Option<JsValue>,
    object: Option<ObjectRef>,
    key: Option<PropertyKey>,
    regexp: Option<JsValue>,
    input: Option<JsValue>,
}
impl RegExpSearchStepPending {
    fn new(runtime: &Runtime) -> Self {
        Self {
            runtime: runtime.clone(),
            value: None,
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
impl Drop for RegExpSearchStepPending {
    fn drop(&mut self) {
        self.release_owned();
    }
}
impl RegExpSearchStep {
    pub(crate) fn make_primitive(value: JsValue, mut resume: RegExpSearchResume) -> Self {
        resume.0.step_pending.value = Some(value);
        Self::Primitive { resume }
    }
    pub(crate) fn make_read(
        object: ObjectRef,
        key: PropertyKey,
        mut resume: RegExpSearchResume,
    ) -> Self {
        resume.0.step_pending.object = Some(object);
        resume.0.step_pending.key = Some(key);
        Self::Read { resume }
    }
    pub(crate) fn make_set(
        object: ObjectRef,
        key: PropertyKey,
        value: JsValue,
        mut resume: RegExpSearchResume,
    ) -> Self {
        resume.0.step_pending.object = Some(object);
        resume.0.step_pending.key = Some(key);
        resume.0.step_pending.value = Some(value);
        Self::Set { resume }
    }
    pub(crate) fn make_exec(
        regexp: JsValue,
        input: JsValue,
        mut resume: RegExpSearchResume,
    ) -> Self {
        resume.0.step_pending.regexp = Some(regexp);
        resume.0.step_pending.input = Some(input);
        Self::Exec { resume }
    }
}
impl RegExpSearchResume {
    pub(crate) fn take_primitive_value(&mut self) -> JsValue {
        self.0
            .step_pending
            .value
            .take()
            .expect("RegExpSearchStep::Primitive lost value")
    }

    pub(crate) fn take_read_object(&mut self) -> ObjectRef {
        self.0
            .step_pending
            .object
            .take()
            .expect("RegExpSearchStep::Read lost object")
    }
    pub(crate) fn take_read_key(&mut self) -> PropertyKey {
        self.0
            .step_pending
            .key
            .take()
            .expect("RegExpSearchStep::Read lost key")
    }

    pub(crate) fn take_set_object(&mut self) -> ObjectRef {
        self.0
            .step_pending
            .object
            .take()
            .expect("RegExpSearchStep::Set lost object")
    }
    pub(crate) fn take_set_key(&mut self) -> PropertyKey {
        self.0
            .step_pending
            .key
            .take()
            .expect("RegExpSearchStep::Set lost key")
    }
    pub(crate) fn take_set_value(&mut self) -> JsValue {
        self.0
            .step_pending
            .value
            .take()
            .expect("RegExpSearchStep::Set lost value")
    }

    pub(crate) fn take_exec_regexp(&mut self) -> JsValue {
        self.0
            .step_pending
            .regexp
            .take()
            .expect("RegExpSearchStep::Exec lost regexp")
    }
    pub(crate) fn take_exec_input(&mut self) -> JsValue {
        self.0
            .step_pending
            .input
            .take()
            .expect("RegExpSearchStep::Exec lost input")
    }
}

const _: () = assert!(std::mem::size_of::<RegExpSearchStep>() <= 64);

// S11 all-domain protocol bound; inline completion stays allocation-free.
const _: () = assert!(std::mem::size_of::<RegExpSearchStep>() <= 64);
