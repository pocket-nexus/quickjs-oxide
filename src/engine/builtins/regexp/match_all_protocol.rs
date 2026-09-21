//! MatchAll setup owns its matcher and flags across observable operations.
use crate::engine::{
    api::{error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError},
    heap::ContextId,
    object::{ObjectRef, PropertyKey, operations::InternalSetResult},
    value::{JsString, JsValue, Value, conversion::NativeConversion},
    vm::{
        Completion, ToPrimitiveHint,
        call::{ConstructorRef, NativeArguments, NativeInvocation},
    },
};
pub(crate) enum RegExpMatchAllStep {
    Complete(Completion),
    Primitive { resume: RegExpMatchAllResume },
    Read { resume: RegExpMatchAllResume },
    Species { resume: RegExpMatchAllResume },
    Construct { resume: RegExpMatchAllResume },
    Set { resume: RegExpMatchAllResume },
}
pub(crate) struct RegExpMatchAllResume(Box<RegExpMatchAllResumeState>);
impl std::ops::Deref for RegExpMatchAllResume {
    type Target = RegExpMatchAllResumeState;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for RegExpMatchAllResume {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
const _: () = assert!(std::mem::size_of::<RegExpMatchAllResume>() <= 8);
pub(crate) struct RegExpMatchAllResumeState {
    step_pending: RegExpMatchAllStepPending,
    realm: ContextId,
    regexp: ObjectRef,
    phase: Phase,
}
enum Phase {
    Input,
    Species(JsString),
    Flags {
        input: JsString,
        constructor: ConstructorRef,
    },
    FlagsPrimitive {
        input: JsString,
        constructor: ConstructorRef,
    },
    Construct {
        input: JsString,
        flags: JsString,
    },
    LastIndex {
        input: JsString,
        flags: JsString,
        matcher: ObjectRef,
    },
    LastIndexPrimitive {
        input: JsString,
        flags: JsString,
        matcher: ObjectRef,
    },
    Set {
        input: JsString,
        flags: JsString,
        matcher: ObjectRef,
    },
}
impl RegExpMatchAllStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        let NativeInvocation::Call { this_value } = invocation else {
            return Err(RuntimeError::Invariant(
                "RegExp @@matchAll did not receive a generic invocation",
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
                "RegExp @@matchAll input argv was not padded",
            ))?)?,
            ToPrimitiveHint::String,
            RegExpMatchAllResume(Box::new(RegExpMatchAllResumeState {
                step_pending: RegExpMatchAllStepPending::new(runtime),
                realm,
                regexp,
                phase: Phase::Input,
            })),
        ))
    }
}
impl RegExpMatchAllResume {
    pub(crate) fn species(
        mut self,
        runtime: &Runtime,
        result: NativeConversion<ConstructorRef>,
    ) -> Result<RegExpMatchAllStep, RuntimeError> {
        let constructor = match result {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => {
                return Ok(RegExpMatchAllStep::Complete(Completion::Throw(value)));
            }
        };
        let Phase::Species(input) = self.0.phase else {
            return Err(RuntimeError::Invariant(
                "RegExp matchAll species reply in wrong phase",
            ));
        };
        Ok(RegExpMatchAllStep::make_read(
            self.0.regexp.clone(),
            runtime.pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Flags)?,
            {
                let updated_0 = Phase::Flags { input, constructor };
                self.0.phase = updated_0;
                self
            },
        ))
    }
    pub(crate) fn set(
        self,
        runtime: &Runtime,
        result: NativeConversion<InternalSetResult>,
    ) -> Result<RegExpMatchAllStep, RuntimeError> {
        let key =
            runtime.pinned_property_key(crate::engine::atom::pinned::PinnedAtom::LastIndex)?;
        let completion = match runtime.finish_set_property_or_throw(self.0.realm, &key, result)? {
            Some(value) => Completion::Throw(value),
            None => Completion::Return(JsValue::Undefined),
        };
        self.resume(runtime, completion)
    }
    pub(crate) fn resume(
        mut self,
        runtime: &Runtime,
        result: Completion,
    ) -> Result<RegExpMatchAllStep, RuntimeError> {
        let value = match result {
            Completion::Return(value) => value,
            Completion::Throw(value) => {
                return Ok(RegExpMatchAllStep::Complete(Completion::Throw(value)));
            }
        };
        match self.0.phase {
            Phase::Input => {
                let input = match runtime.native_to_js_string_jsvalue(self.0.realm, value)? {
                    NativeConversion::Value(value) => value,
                    NativeConversion::Throw(value) => {
                        return Ok(RegExpMatchAllStep::Complete(Completion::Throw(value)));
                    }
                };
                Ok(RegExpMatchAllStep::make_species(self.0.regexp.clone(), {
                    let updated_0 = Phase::Species(input);
                    self.0.phase = updated_0;
                    self
                }))
            }
            Phase::Flags { input, constructor } => Ok(RegExpMatchAllStep::make_primitive(
                value,
                ToPrimitiveHint::String,
                {
                    let updated_0 = Phase::FlagsPrimitive { input, constructor };
                    self.0.phase = updated_0;
                    self
                },
            )),
            Phase::FlagsPrimitive { input, constructor } => {
                let (flags, flags_value) =
                    match super::replace::converted_string_owned(runtime, self.0.realm, value)? {
                        NativeConversion::Value(value) => value,
                        NativeConversion::Throw(value) => {
                            return Ok(RegExpMatchAllStep::Complete(Completion::Throw(value)));
                        }
                    };
                let mut arguments = Vec::new();
                if arguments.try_reserve_exact(2).is_err() {
                    runtime.release_jsvalue(flags_value)?;
                    return Ok(RegExpMatchAllStep::Complete(Completion::Throw(
                        runtime.new_native_error_jsvalue(
                            self.0.realm,
                            NativeErrorKind::Internal,
                            "out of memory",
                        )?,
                    )));
                }
                arguments.push(JsValue::Object(self.0.regexp.clone().into_handle()));
                arguments.push(flags_value);
                Ok(RegExpMatchAllStep::make_construct(
                    constructor,
                    arguments,
                    {
                        let updated_0 = Phase::Construct { input, flags };
                        self.0.phase = updated_0;
                        self
                    },
                ))
            }
            Phase::Construct { input, flags } => {
                let JsValue::Object(matcher) = value else {
                    runtime.release_jsvalue(value)?;
                    return Err(RuntimeError::Invariant(
                        "RegExp matchAll species constructor returned a primitive",
                    ));
                };
                let matcher = ObjectRef::from_owned_handle(runtime.clone(), matcher);
                Ok(RegExpMatchAllStep::make_read(
                    self.0.regexp.clone(),
                    runtime
                        .pinned_property_key(crate::engine::atom::pinned::PinnedAtom::LastIndex)?,
                    {
                        let updated_0 = Phase::LastIndex {
                            input,
                            flags,
                            matcher,
                        };
                        self.0.phase = updated_0;
                        self
                    },
                ))
            }
            Phase::LastIndex {
                input,
                flags,
                matcher,
            } => Ok(RegExpMatchAllStep::make_primitive(
                value,
                ToPrimitiveHint::Number,
                {
                    let updated_0 = Phase::LastIndexPrimitive {
                        input,
                        flags,
                        matcher,
                    };
                    self.0.phase = updated_0;
                    self
                },
            )),
            Phase::LastIndexPrimitive {
                input,
                flags,
                matcher,
            } => {
                let length = match runtime.native_to_number_jsvalue(self.0.realm, value)? {
                    NativeConversion::Value(value) => Runtime::length_from_number(value),
                    NativeConversion::Throw(value) => {
                        return Ok(RegExpMatchAllStep::Complete(Completion::Throw(value)));
                    }
                };
                Ok(RegExpMatchAllStep::make_set(
                    matcher.clone(),
                    runtime
                        .pinned_property_key(crate::engine::atom::pinned::PinnedAtom::LastIndex)?,
                    runtime.into_jsvalue(Value::number(length as f64))?,
                    {
                        let updated_0 = Phase::Set {
                            input,
                            flags,
                            matcher,
                        };
                        self.0.phase = updated_0;
                        self
                    },
                ))
            }
            Phase::Set {
                input,
                flags,
                matcher,
            } => {
                runtime.release_jsvalue(value)?;
                let global = flags.utf16_units().any(|unit| unit == u16::from(b'g'));
                let full_unicode = flags
                    .utf16_units()
                    .any(|unit| unit == u16::from(b'u') || unit == u16::from(b'v'));
                Ok(RegExpMatchAllStep::Complete(Completion::Return(
                    runtime.into_jsvalue(Value::Object(runtime.new_regexp_string_iterator(
                        self.0.realm,
                        &matcher,
                        input,
                        global,
                        full_unicode,
                    )?))?,
                )))
            }
            Phase::Species(_) => {
                runtime.release_jsvalue(value)?;
                Err(RuntimeError::Invariant(
                    "RegExp matchAll completion in species phase",
                ))
            }
        }
    }
}
pub(super) fn finish(
    runtime: &Runtime,
    realm: ContextId,
    mut step: RegExpMatchAllStep,
) -> Result<Completion, RuntimeError> {
    loop {
        step = match step {
            RegExpMatchAllStep::Complete(result) => return Ok(result),
            RegExpMatchAllStep::Primitive { mut resume } => {
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
            RegExpMatchAllStep::Read { mut resume } => {
                let object = resume.take_read_object();
                let key = resume.take_read_key();
                resume.resume(
                    runtime,
                    runtime.get_property_in_realm(realm, &object, &key)?,
                )?
            }
            RegExpMatchAllStep::Species { mut resume } => {
                let regexp = resume.take_species_regexp();
                resume.species(runtime, runtime.regexp_species_constructor(realm, &regexp)?)?
            }
            RegExpMatchAllStep::Construct { mut resume } => {
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
            RegExpMatchAllStep::Set { mut resume } => {
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
        }
    }
}

pub(crate) struct RegExpMatchAllStepPending {
    runtime: Runtime,
    value: Option<JsValue>,
    hint: Option<ToPrimitiveHint>,
    object: Option<ObjectRef>,
    key: Option<PropertyKey>,
    regexp: Option<ObjectRef>,
    constructor: Option<ConstructorRef>,
    arguments: Option<Vec<JsValue>>,
}
impl RegExpMatchAllStepPending {
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
        }
    }

    /// Release the internal edges still owned when the request is abandoned
    /// before its step consumed them. Taken fields are empty here.
    fn release_owned(&mut self) {
        if let Some(value) = self.value.take() {
            let _ = self.runtime.release_jsvalue(value);
        }
        for argument in self.arguments.take().into_iter().flatten() {
            let _ = self.runtime.release_jsvalue(argument);
        }
    }
}
impl Drop for RegExpMatchAllStepPending {
    fn drop(&mut self) {
        self.release_owned();
    }
}
impl RegExpMatchAllStep {
    pub(crate) fn make_primitive(
        value: JsValue,
        hint: ToPrimitiveHint,
        mut resume: RegExpMatchAllResume,
    ) -> Self {
        resume.0.step_pending.value = Some(value);
        resume.0.step_pending.hint = Some(hint);
        Self::Primitive { resume }
    }
    pub(crate) fn make_read(
        object: ObjectRef,
        key: PropertyKey,
        mut resume: RegExpMatchAllResume,
    ) -> Self {
        resume.0.step_pending.object = Some(object);
        resume.0.step_pending.key = Some(key);
        Self::Read { resume }
    }
    pub(crate) fn make_species(regexp: ObjectRef, mut resume: RegExpMatchAllResume) -> Self {
        resume.0.step_pending.regexp = Some(regexp);
        Self::Species { resume }
    }
    pub(crate) fn make_construct(
        constructor: ConstructorRef,
        arguments: Vec<JsValue>,
        mut resume: RegExpMatchAllResume,
    ) -> Self {
        resume.0.step_pending.constructor = Some(constructor);
        resume.0.step_pending.arguments = Some(arguments);
        Self::Construct { resume }
    }
    pub(crate) fn make_set(
        object: ObjectRef,
        key: PropertyKey,
        value: JsValue,
        mut resume: RegExpMatchAllResume,
    ) -> Self {
        resume.0.step_pending.object = Some(object);
        resume.0.step_pending.key = Some(key);
        resume.0.step_pending.value = Some(value);
        Self::Set { resume }
    }
}
impl RegExpMatchAllResume {
    pub(crate) fn take_primitive_value(&mut self) -> JsValue {
        self.0
            .step_pending
            .value
            .take()
            .expect("RegExpMatchAllStep::Primitive lost value")
    }
    pub(crate) fn take_primitive_hint(&mut self) -> ToPrimitiveHint {
        self.0
            .step_pending
            .hint
            .take()
            .expect("RegExpMatchAllStep::Primitive lost hint")
    }

    pub(crate) fn take_read_object(&mut self) -> ObjectRef {
        self.0
            .step_pending
            .object
            .take()
            .expect("RegExpMatchAllStep::Read lost object")
    }
    pub(crate) fn take_read_key(&mut self) -> PropertyKey {
        self.0
            .step_pending
            .key
            .take()
            .expect("RegExpMatchAllStep::Read lost key")
    }

    pub(crate) fn take_species_regexp(&mut self) -> ObjectRef {
        self.0
            .step_pending
            .regexp
            .take()
            .expect("RegExpMatchAllStep::Species lost regexp")
    }

    pub(crate) fn take_construct_constructor(&mut self) -> ConstructorRef {
        self.0
            .step_pending
            .constructor
            .take()
            .expect("RegExpMatchAllStep::Construct lost constructor")
    }
    pub(crate) fn take_construct_arguments(&mut self) -> Vec<JsValue> {
        self.0
            .step_pending
            .arguments
            .take()
            .expect("RegExpMatchAllStep::Construct lost arguments")
    }

    pub(crate) fn take_set_object(&mut self) -> ObjectRef {
        self.0
            .step_pending
            .object
            .take()
            .expect("RegExpMatchAllStep::Set lost object")
    }
    pub(crate) fn take_set_key(&mut self) -> PropertyKey {
        self.0
            .step_pending
            .key
            .take()
            .expect("RegExpMatchAllStep::Set lost key")
    }
    pub(crate) fn take_set_value(&mut self) -> JsValue {
        self.0
            .step_pending
            .value
            .take()
            .expect("RegExpMatchAllStep::Set lost value")
    }
}

const _: () = assert!(std::mem::size_of::<RegExpMatchAllStep>() <= 64);

// S11 all-domain protocol bound; inline completion stays allocation-free.
const _: () = assert!(std::mem::size_of::<RegExpMatchAllStep>() <= 64);
