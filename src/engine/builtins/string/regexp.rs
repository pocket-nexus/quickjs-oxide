//! RegExp-backed String prototype methods.

#[cfg(feature = "stack-vm")]
use crate::engine::builtins::native::NativeFunctionId;
use crate::engine::{
    api::{error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError},
    heap::ContextId,
    object::{ObjectRef, PropertyKey, WellKnownSymbol},
    value::{JsString, Value, conversion::NativeConversion},
    vm::{
        Completion, ToPrimitiveHint,
        call::{ConstructorRef, DirectCallTarget, NativeArguments, NativeInvocation},
    },
};

#[derive(Clone, Copy)]
pub(crate) enum StringProtocolKind {
    Match,
    MatchAll,
    Search,
}

impl StringProtocolKind {
    const fn symbol(self) -> WellKnownSymbol {
        match self {
            Self::Match => WellKnownSymbol::Match,
            Self::MatchAll => WellKnownSymbol::MatchAll,
            Self::Search => WellKnownSymbol::Search,
        }
    }

    const fn invocation_invariant(self) -> &'static str {
        match self {
            Self::Match => "String match did not receive a generic-magic invocation",
            Self::MatchAll => "String matchAll did not receive a generic-magic invocation",
            Self::Search => "String search did not receive a generic-magic invocation",
        }
    }

    const fn argument_invariant(self) -> &'static str {
        match self {
            Self::Match => "String match pattern argv was not padded",
            Self::MatchAll => "String matchAll pattern argv was not padded",
            Self::Search => "String search pattern argv was not padded",
        }
    }
}

impl Runtime {
    pub(crate) fn call_string_prototype_match(
        &self,
        realm: ContextId,
        invocation: NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        self.call_string_regexp_protocol(realm, invocation, arguments, StringProtocolKind::Match)
    }

    pub(crate) fn call_string_prototype_search(
        &self,
        realm: ContextId,
        invocation: NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        self.call_string_regexp_protocol(realm, invocation, arguments, StringProtocolKind::Search)
    }

    pub(crate) fn call_string_prototype_match_all(
        &self,
        realm: ContextId,
        invocation: NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        self.call_string_regexp_protocol(realm, invocation, arguments, StringProtocolKind::MatchAll)
    }

    /// Rust port of the `Symbol.match`, `Symbol.matchAll`, and `Symbol.search`
    /// branches in pinned
    /// QuickJS `js_string_match`.
    ///
    /// Object patterns may intercept the operation before receiver coercion.
    /// The fallback converts the receiver, constructs with the defining
    /// realm's retained intrinsic RegExp constructor, and dynamically invokes
    /// the selected protocol on that fresh value so prototype mutations remain
    /// observable.
    fn call_string_regexp_protocol(
        &self,
        realm: ContextId,
        invocation: NativeInvocation,
        arguments: &NativeArguments,
        protocol: StringProtocolKind,
    ) -> Result<Completion, RuntimeError> {
        finish(
            self,
            realm,
            StringProtocolStep::start(self, realm, protocol, &invocation, arguments)?,
        )
    }
}

impl StringProtocolKind {
    #[cfg(feature = "stack-vm")]
    pub(crate) fn for_target(target: NativeFunctionId) -> Option<Self> {
        Some(match target {
            NativeFunctionId::StringPrototypeMatch => Self::Match,
            NativeFunctionId::StringPrototypeMatchAll => Self::MatchAll,
            NativeFunctionId::StringPrototypeSearch => Self::Search,
            _ => return None,
        })
    }
}
pub(crate) enum StringProtocolStep {
    Complete(Completion),
    Read {
        object: ObjectRef,
        key: PropertyKey,
        resume: StringProtocolResume,
    },
    Primitive {
        value: Value,
        resume: StringProtocolResume,
    },
    Call {
        target: DirectCallTarget,
        receiver: Value,
        arguments: Vec<Value>,
        resume: StringProtocolResume,
    },
    Construct {
        constructor: ConstructorRef,
        arguments: Vec<Value>,
        resume: StringProtocolResume,
    },
}
pub(crate) struct StringProtocolResume {
    realm: ContextId,
    kind: StringProtocolKind,
    receiver: Value,
    pattern: Value,
    phase: ProtocolPhase,
}
enum ProtocolPhase {
    Method,
    Match(Value),
    Flags(Value),
    FlagsString(Value),
    Source,
    Constructed(JsString),
    ConstructMethod { regexp: ObjectRef, source: JsString },
    Called,
}
impl StringProtocolStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        kind: StringProtocolKind,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        let NativeInvocation::Call { this_value } = invocation else {
            return Err(RuntimeError::Invariant(kind.invocation_invariant()));
        };
        if matches!(this_value, Value::Null | Value::Undefined) {
            return Ok(Self::Complete(Completion::Throw(
                runtime.new_native_error(
                    realm,
                    NativeErrorKind::Type,
                    "cannot convert to object",
                )?,
            )));
        }
        let pattern = arguments
            .readable
            .first()
            .ok_or(RuntimeError::Invariant(kind.argument_invariant()))?
            .clone();
        let key = PropertyKey::from(runtime.well_known_symbol(kind.symbol()));
        let resume = StringProtocolResume {
            realm,
            kind,
            receiver: this_value.clone(),
            pattern,
            phase: ProtocolPhase::Method,
        };
        if let Value::Object(object) = &resume.pattern {
            Ok(Self::Read {
                object: object.clone(),
                key,
                resume,
            })
        } else {
            Ok(resume.source())
        }
    }
}
impl StringProtocolResume {
    fn source(self) -> StringProtocolStep {
        StringProtocolStep::Primitive {
            value: self.receiver.clone(),
            resume: Self {
                phase: ProtocolPhase::Source,
                ..self
            },
        }
    }
    fn selected(
        self,
        runtime: &Runtime,
        method: Value,
    ) -> Result<StringProtocolStep, RuntimeError> {
        if matches!(method, Value::Undefined | Value::Null) {
            return Ok(self.source());
        }
        let receiver = self.pattern.clone();
        let argument = self.receiver.clone();
        self.call(runtime, receiver, method, argument)
    }
    fn call(
        self,
        runtime: &Runtime,
        receiver: Value,
        method: Value,
        argument: Value,
    ) -> Result<StringProtocolStep, RuntimeError> {
        let callable = match method {
            Value::Object(object) => runtime.as_callable(&object)?,
            _ => None,
        };
        let Some(callable) = callable else {
            return Ok(StringProtocolStep::Complete(Completion::Throw(
                runtime.new_native_error(self.realm, NativeErrorKind::Type, "not a function")?,
            )));
        };
        let mut arguments = Vec::new();
        if arguments.try_reserve_exact(1).is_err() {
            return protocol_oom(runtime, self.realm);
        }
        arguments.push(argument);
        Ok(StringProtocolStep::Call {
            target: DirectCallTarget::Callable(callable),
            receiver,
            arguments,
            resume: Self {
                phase: ProtocolPhase::Called,
                ..self
            },
        })
    }
    pub(crate) fn resume(
        self,
        runtime: &Runtime,
        result: Completion,
    ) -> Result<StringProtocolStep, RuntimeError> {
        let value = match result {
            Completion::Return(value) => value,
            Completion::Throw(value) => {
                return Ok(StringProtocolStep::Complete(Completion::Throw(value)));
            }
        };
        let realm = self.realm;
        match self.phase {
            ProtocolPhase::Method => {
                if matches!(self.kind, StringProtocolKind::MatchAll) {
                    let Value::Object(object) = &self.pattern else {
                        return Err(RuntimeError::Invariant(
                            "String matchAll check lost its pattern object",
                        ));
                    };
                    Ok(StringProtocolStep::Read {
                        object: object.clone(),
                        key: PropertyKey::from(runtime.well_known_symbol(WellKnownSymbol::Match)),
                        resume: Self {
                            phase: ProtocolPhase::Match(value),
                            ..self
                        },
                    })
                } else {
                    self.selected(runtime, value)
                }
            }
            ProtocolPhase::Match(method) => {
                let Value::Object(object) = &self.pattern else {
                    return Err(RuntimeError::Invariant(
                        "String matchAll check lost its pattern object",
                    ));
                };
                let regexp = runtime.is_regexp_from_match(object, &value)?;
                if regexp {
                    Ok(StringProtocolStep::Read {
                        object: object.clone(),
                        key: runtime.intern_property_key("flags")?,
                        resume: Self {
                            phase: ProtocolPhase::Flags(method),
                            ..self
                        },
                    })
                } else {
                    Self {
                        phase: ProtocolPhase::Called,
                        ..self
                    }
                    .selected(runtime, method)
                }
            }
            ProtocolPhase::Flags(method) => {
                if matches!(value, Value::Undefined | Value::Null) {
                    return Ok(StringProtocolStep::Complete(Completion::Throw(
                        runtime.new_native_error(
                            realm,
                            NativeErrorKind::Type,
                            "cannot convert to object",
                        )?,
                    )));
                }
                Ok(StringProtocolStep::Primitive {
                    value,
                    resume: Self {
                        phase: ProtocolPhase::FlagsString(method),
                        ..self
                    },
                })
            }
            ProtocolPhase::FlagsString(method) => {
                let flags = match protocol_string(runtime, realm, value)? {
                    NativeConversion::Value(value) => value,
                    NativeConversion::Throw(value) => {
                        return Ok(StringProtocolStep::Complete(Completion::Throw(value)));
                    }
                };
                if !flags.utf16_units().any(|unit| unit == u16::from(b'g')) {
                    return Ok(StringProtocolStep::Complete(Completion::Throw(
                        runtime.new_native_error(
                            realm,
                            NativeErrorKind::Type,
                            "regexp must have the 'g' flag",
                        )?,
                    )));
                }
                Self {
                    phase: ProtocolPhase::Called,
                    ..self
                }
                .selected(runtime, method)
            }
            ProtocolPhase::Source => {
                let source = match protocol_string(runtime, realm, value)? {
                    NativeConversion::Value(value) => value,
                    NativeConversion::Throw(value) => {
                        return Ok(StringProtocolStep::Complete(Completion::Throw(value)));
                    }
                };
                let constructor = ObjectRef::from_borrowed_handle(
                    runtime.clone(),
                    runtime.regexp_realm_data(realm)?.constructor,
                )?;
                runtime
                    .as_callable(&constructor)?
                    .ok_or(RuntimeError::Invariant(
                        "realm RegExp constructor root was not callable",
                    ))?;
                let mut arguments = Vec::new();
                let all = matches!(self.kind, StringProtocolKind::MatchAll);
                if arguments
                    .try_reserve_exact(if all { 2 } else { 1 })
                    .is_err()
                {
                    return protocol_oom(runtime, realm);
                }
                arguments.push(self.pattern.clone());
                if all {
                    arguments.push(Value::String(JsString::from_static("g")));
                }
                Ok(StringProtocolStep::Construct {
                    constructor: ConstructorRef::from_validated_object(constructor),
                    arguments,
                    resume: Self {
                        phase: ProtocolPhase::Constructed(source),
                        ..self
                    },
                })
            }
            ProtocolPhase::Constructed(source) => {
                let Value::Object(regexp) = value else {
                    return Err(RuntimeError::Invariant(
                        "intrinsic RegExp constructor returned a primitive",
                    ));
                };
                Ok(StringProtocolStep::Read {
                    object: regexp.clone(),
                    key: PropertyKey::from(runtime.well_known_symbol(self.kind.symbol())),
                    resume: Self {
                        phase: ProtocolPhase::ConstructMethod { regexp, source },
                        ..self
                    },
                })
            }
            ProtocolPhase::ConstructMethod { regexp, source } => Self {
                phase: ProtocolPhase::Called,
                ..self
            }
            .call(runtime, Value::Object(regexp), value, Value::String(source)),
            ProtocolPhase::Called => Ok(StringProtocolStep::Complete(Completion::Return(value))),
        }
    }
}
fn protocol_string(
    runtime: &Runtime,
    realm: ContextId,
    value: Value,
) -> Result<NativeConversion<JsString>, RuntimeError> {
    if matches!(value, Value::Object(_)) {
        return Err(RuntimeError::Invariant(
            "String protocol conversion returned an object",
        ));
    }
    runtime.native_to_js_string(realm, &value)
}
fn protocol_oom(runtime: &Runtime, realm: ContextId) -> Result<StringProtocolStep, RuntimeError> {
    Ok(StringProtocolStep::Complete(Completion::Throw(
        runtime.new_native_error(realm, NativeErrorKind::Internal, "out of memory")?,
    )))
}
fn finish(
    runtime: &Runtime,
    realm: ContextId,
    mut step: StringProtocolStep,
) -> Result<Completion, RuntimeError> {
    loop {
        step = match step {
            StringProtocolStep::Complete(result) => return Ok(result),
            StringProtocolStep::Read {
                object,
                key,
                resume,
            } => resume.resume(
                runtime,
                runtime.get_property_in_realm(realm, &object, &key)?,
            )?,
            StringProtocolStep::Primitive { value, resume } => {
                let result = if matches!(value, Value::Object(_)) {
                    runtime.to_primitive(realm, value, ToPrimitiveHint::String)?
                } else {
                    Completion::Return(value)
                };
                resume.resume(runtime, result)?
            }
            StringProtocolStep::Call {
                target,
                receiver,
                arguments,
                resume,
            } => {
                let DirectCallTarget::Callable(callable) = target else {
                    return Err(RuntimeError::Invariant(
                        "String protocol requested an invalid call target",
                    ));
                };
                resume.resume(
                    runtime,
                    runtime.call_internal(realm, &callable, receiver, &arguments)?,
                )?
            }
            StringProtocolStep::Construct {
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
        };
    }
}
