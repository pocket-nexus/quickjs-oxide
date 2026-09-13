//! MatchAll setup owns its matcher and flags across observable operations.
use crate::engine::{
    api::{error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError},
    heap::ContextId,
    object::{ObjectRef, PropertyKey, operations::InternalSetResult},
    value::{JsString, Value, conversion::NativeConversion},
    vm::{
        Completion, ToPrimitiveHint,
        call::{ConstructorRef, NativeArguments, NativeInvocation},
    },
};
pub(crate) enum RegExpMatchAllStep {
    Complete(Completion),
    Primitive {
        value: Value,
        hint: ToPrimitiveHint,
        resume: RegExpMatchAllResume,
    },
    Read {
        object: ObjectRef,
        key: PropertyKey,
        resume: RegExpMatchAllResume,
    },
    Species {
        regexp: ObjectRef,
        resume: RegExpMatchAllResume,
    },
    Construct {
        constructor: ConstructorRef,
        arguments: Vec<Value>,
        resume: RegExpMatchAllResume,
    },
    Set {
        object: ObjectRef,
        key: PropertyKey,
        value: Value,
        resume: RegExpMatchAllResume,
    },
}
pub(crate) struct RegExpMatchAllResume {
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
        let Value::Object(regexp) = this_value else {
            return Ok(Self::Complete(Completion::Throw(
                runtime.new_native_error(realm, NativeErrorKind::Type, "not an object")?,
            )));
        };
        Ok(Self::Primitive {
            value: arguments
                .readable
                .first()
                .ok_or(RuntimeError::Invariant(
                    "RegExp @@matchAll input argv was not padded",
                ))?
                .clone(),
            hint: ToPrimitiveHint::String,
            resume: RegExpMatchAllResume {
                realm,
                regexp: regexp.clone(),
                phase: Phase::Input,
            },
        })
    }
}
impl RegExpMatchAllResume {
    pub(crate) fn species(
        self,
        runtime: &Runtime,
        result: NativeConversion<ConstructorRef>,
    ) -> Result<RegExpMatchAllStep, RuntimeError> {
        let constructor = match result {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => {
                return Ok(RegExpMatchAllStep::Complete(Completion::Throw(value)));
            }
        };
        let Phase::Species(input) = self.phase else {
            return Err(RuntimeError::Invariant(
                "RegExp matchAll species reply in wrong phase",
            ));
        };
        Ok(RegExpMatchAllStep::Read {
            object: self.regexp.clone(),
            key: runtime.intern_property_key("flags")?,
            resume: Self {
                phase: Phase::Flags { input, constructor },
                ..self
            },
        })
    }
    pub(crate) fn set(
        self,
        runtime: &Runtime,
        result: NativeConversion<InternalSetResult>,
    ) -> Result<RegExpMatchAllStep, RuntimeError> {
        let key = runtime.intern_property_key("lastIndex")?;
        let completion = match runtime.finish_set_property_or_throw(self.realm, &key, result)? {
            Some(value) => Completion::Throw(value),
            None => Completion::Return(Value::Undefined),
        };
        self.resume(runtime, completion)
    }
    pub(crate) fn resume(
        self,
        runtime: &Runtime,
        result: Completion,
    ) -> Result<RegExpMatchAllStep, RuntimeError> {
        let value = match result {
            Completion::Return(value) => value,
            Completion::Throw(value) => {
                return Ok(RegExpMatchAllStep::Complete(Completion::Throw(value)));
            }
        };
        match self.phase {
            Phase::Input => {
                let input = match runtime.native_to_js_string(self.realm, &value)? {
                    NativeConversion::Value(value) => value,
                    NativeConversion::Throw(value) => {
                        return Ok(RegExpMatchAllStep::Complete(Completion::Throw(value)));
                    }
                };
                Ok(RegExpMatchAllStep::Species {
                    regexp: self.regexp.clone(),
                    resume: Self {
                        phase: Phase::Species(input),
                        ..self
                    },
                })
            }
            Phase::Flags { input, constructor } => Ok(RegExpMatchAllStep::Primitive {
                value,
                hint: ToPrimitiveHint::String,
                resume: Self {
                    phase: Phase::FlagsPrimitive { input, constructor },
                    ..self
                },
            }),
            Phase::FlagsPrimitive { input, constructor } => {
                let flags = match runtime.native_to_js_string(self.realm, &value)? {
                    NativeConversion::Value(value) => value,
                    NativeConversion::Throw(value) => {
                        return Ok(RegExpMatchAllStep::Complete(Completion::Throw(value)));
                    }
                };
                let mut arguments = Vec::new();
                if arguments.try_reserve_exact(2).is_err() {
                    return Ok(RegExpMatchAllStep::Complete(Completion::Throw(
                        runtime.new_native_error(
                            self.realm,
                            NativeErrorKind::Internal,
                            "out of memory",
                        )?,
                    )));
                }
                arguments.push(Value::Object(self.regexp.clone()));
                arguments.push(Value::String(flags.clone()));
                Ok(RegExpMatchAllStep::Construct {
                    constructor,
                    arguments,
                    resume: Self {
                        phase: Phase::Construct { input, flags },
                        ..self
                    },
                })
            }
            Phase::Construct { input, flags } => {
                let Value::Object(matcher) = value else {
                    return Err(RuntimeError::Invariant(
                        "RegExp matchAll species constructor returned a primitive",
                    ));
                };
                Ok(RegExpMatchAllStep::Read {
                    object: self.regexp.clone(),
                    key: runtime.intern_property_key("lastIndex")?,
                    resume: Self {
                        phase: Phase::LastIndex {
                            input,
                            flags,
                            matcher,
                        },
                        ..self
                    },
                })
            }
            Phase::LastIndex {
                input,
                flags,
                matcher,
            } => Ok(RegExpMatchAllStep::Primitive {
                value,
                hint: ToPrimitiveHint::Number,
                resume: Self {
                    phase: Phase::LastIndexPrimitive {
                        input,
                        flags,
                        matcher,
                    },
                    ..self
                },
            }),
            Phase::LastIndexPrimitive {
                input,
                flags,
                matcher,
            } => {
                let length = match runtime.native_to_length(self.realm, &value)? {
                    NativeConversion::Value(value) => value,
                    NativeConversion::Throw(value) => {
                        return Ok(RegExpMatchAllStep::Complete(Completion::Throw(value)));
                    }
                };
                Ok(RegExpMatchAllStep::Set {
                    object: matcher.clone(),
                    key: runtime.intern_property_key("lastIndex")?,
                    value: Value::number(length as f64),
                    resume: Self {
                        phase: Phase::Set {
                            input,
                            flags,
                            matcher,
                        },
                        ..self
                    },
                })
            }
            Phase::Set {
                input,
                flags,
                matcher,
            } => {
                let global = flags.utf16_units().any(|unit| unit == u16::from(b'g'));
                let full_unicode = flags
                    .utf16_units()
                    .any(|unit| unit == u16::from(b'u') || unit == u16::from(b'v'));
                Ok(RegExpMatchAllStep::Complete(Completion::Return(
                    Value::Object(runtime.new_regexp_string_iterator(
                        self.realm,
                        &matcher,
                        input,
                        global,
                        full_unicode,
                    )?),
                )))
            }
            Phase::Species(_) => Err(RuntimeError::Invariant(
                "RegExp matchAll completion in species phase",
            )),
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
            RegExpMatchAllStep::Primitive {
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
            RegExpMatchAllStep::Read {
                object,
                key,
                resume,
            } => resume.resume(
                runtime,
                runtime.get_property_in_realm(realm, &object, &key)?,
            )?,
            RegExpMatchAllStep::Species { regexp, resume } => {
                resume.species(runtime, runtime.regexp_species_constructor(realm, &regexp)?)?
            }
            RegExpMatchAllStep::Construct {
                constructor,
                arguments,
                resume,
            } => resume.resume(
                runtime,
                runtime.construct_constructor_internal(
                    realm,
                    &constructor,
                    &constructor,
                    &arguments,
                )?,
            )?,
            RegExpMatchAllStep::Set {
                object,
                key,
                value,
                resume,
            } => resume.set(
                runtime,
                runtime.internal_set(realm, &object, &key, value, Value::Object(object.clone()))?,
            )?,
        }
    }
}
