//! String search and subrange coercions preserve their distinct position rules.

use crate::engine::builtins::native::NativeFunctionId;
use crate::engine::{
    api::{error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError},
    builtins::native::{StringIncludesKind, StringIndexOfKind, StringSubrangeKind},
    heap::ContextId,
    object::{ObjectRef, PropertyKey, WellKnownSymbol},
    value::{JsString, JsValue, conversion::NativeConversion},
    vm::{
        Completion, ToPrimitiveHint,
        call::{NativeArguments, NativeInvocation},
    },
};
#[derive(Clone, Copy)]
pub(crate) enum StringSearchKind {
    Index(StringIndexOfKind),
    Includes(StringIncludesKind),
    Subrange(StringSubrangeKind),
}
impl StringSearchKind {
    pub(crate) fn for_target(target: NativeFunctionId) -> Option<Self> {
        Some(match target {
            NativeFunctionId::StringPrototypeIndexOf(kind) => Self::Index(kind),
            NativeFunctionId::StringPrototypeIncludes(kind) => Self::Includes(kind),
            NativeFunctionId::StringPrototypeSubrange(kind) => Self::Subrange(kind),
            _ => return None,
        })
    }
}
pub(crate) enum StringSearchStep {
    Complete(Completion),
    Read {
        object: ObjectRef,
        key: PropertyKey,
        resume: StringSearchResume,
    },
    Primitive {
        value: JsValue,
        hint: ToPrimitiveHint,
        resume: StringSearchResume,
    },
}
pub(crate) struct StringSearchResume(Box<StringSearchResumeState>);
impl std::ops::Deref for StringSearchResume {
    type Target = StringSearchResumeState;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for StringSearchResume {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
const _: () = assert!(std::mem::size_of::<StringSearchResume>() <= 8);
pub(crate) struct StringSearchResumeState {
    runtime: Runtime,
    realm: ContextId,
    kind: StringSearchKind,
    first: JsValue,
    second: JsValue,
    actual: usize,
    phase: SearchPhase,
}
impl Drop for StringSearchResumeState {
    fn drop(&mut self) {
        for value in [&mut self.first, &mut self.second] {
            let _ = self
                .runtime
                .release_jsvalue(std::mem::replace(value, JsValue::Undefined));
        }
    }
}
enum SearchPhase {
    Source,
    Regexp(JsString),
    Needle(JsString),
    Position { source: JsString, needle: JsString },
    Start(JsString),
    End { source: JsString, start: f64 },
}
impl StringSearchStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        kind: StringSearchKind,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        let NativeInvocation::Call { this_value } = invocation else {
            return Err(RuntimeError::Invariant(
                "String search did not receive a call",
            ));
        };
        if matches!(this_value, JsValue::Undefined | JsValue::Null) {
            return Ok(Self::Complete(Completion::Throw(
                runtime.new_native_error_jsvalue(
                    realm,
                    NativeErrorKind::Type,
                    "null or undefined are forbidden",
                )?,
            )));
        }
        let mut resume = StringSearchResume(Box::new(StringSearchResumeState {
            runtime: runtime.clone(),
            realm,
            kind,
            first: JsValue::Undefined,
            second: JsValue::Undefined,
            actual: arguments.actual_arg_count,
            phase: SearchPhase::Source,
        }));
        resume.first = runtime.dup_jsvalue(arguments.readable.first().ok_or(
            RuntimeError::Invariant("String search first argument was not padded"),
        )?)?;
        resume.second = arguments
            .readable
            .get(1)
            .map(|value| runtime.dup_jsvalue(value))
            .transpose()?
            .unwrap_or(JsValue::Undefined);
        Ok(Self::Primitive {
            value: runtime.dup_jsvalue(this_value)?,
            hint: ToPrimitiveHint::String,
            resume,
        })
    }
}
impl StringSearchResume {
    fn primitive(
        mut self,
        value: JsValue,
        hint: ToPrimitiveHint,
        phase: SearchPhase,
    ) -> StringSearchStep {
        StringSearchStep::Primitive {
            value,
            hint,
            resume: {
                let updated_0 = phase;
                self.0.phase = updated_0;
                self
            },
        }
    }
    fn needle(self, runtime: &Runtime, source: JsString) -> Result<StringSearchStep, RuntimeError> {
        let value = runtime.dup_jsvalue(&self.0.first)?;
        Ok(self.primitive(value, ToPrimitiveHint::String, SearchPhase::Needle(source)))
    }
    fn finish_search(
        &self,
        runtime: &Runtime,
        source: JsString,
        needle: JsString,
        position: Option<f64>,
    ) -> Result<StringSearchStep, RuntimeError> {
        Ok(StringSearchStep::Complete(match self.0.kind {
            StringSearchKind::Index(kind) => {
                runtime.finish_string_index_of(kind, source, needle, position)?
            }
            StringSearchKind::Includes(kind) => {
                runtime.finish_string_includes(kind, source, needle, position)?
            }
            _ => {
                return Err(RuntimeError::Invariant(
                    "String position reply lost its kind",
                ));
            }
        }))
    }
    pub(crate) fn resume(
        mut self,
        runtime: &Runtime,
        result: Completion,
    ) -> Result<StringSearchStep, RuntimeError> {
        let value = match result {
            Completion::Return(value) => value,
            Completion::Throw(value) => {
                return Ok(StringSearchStep::Complete(Completion::Throw(value)));
            }
        };
        let realm = self.0.realm;
        match std::mem::replace(&mut self.0.phase, SearchPhase::Source) {
            SearchPhase::Source => {
                let source = match string_value(runtime, realm, value)? {
                    NativeConversion::Value(value) => value,
                    NativeConversion::Throw(value) => {
                        return Ok(StringSearchStep::Complete(Completion::Throw(value)));
                    }
                };
                match self.0.kind {
                    StringSearchKind::Subrange(_) => {
                        i32::try_from(source.len()).map_err(|_| {
                            RuntimeError::Invariant(
                                "String length exceeded QuickJS's signed index range",
                            )
                        })?;
                        let value = runtime.dup_jsvalue(&self.0.first)?;
                        Ok(self.primitive(
                            value,
                            ToPrimitiveHint::Number,
                            SearchPhase::Start(source),
                        ))
                    }
                    StringSearchKind::Includes(_) => {
                        if let JsValue::Object(id) = &self.0.first {
                            Ok(StringSearchStep::Read {
                                object: ObjectRef::from_borrowed_handle(runtime.clone(), *id)?,
                                key: PropertyKey::from(
                                    runtime.well_known_symbol(WellKnownSymbol::Match),
                                ),
                                resume: {
                                    let updated_0 = SearchPhase::Regexp(source);
                                    self.0.phase = updated_0;
                                    self
                                },
                            })
                        } else {
                            self.needle(runtime, source)
                        }
                    }
                    StringSearchKind::Index(_) => self.needle(runtime, source),
                }
            }
            SearchPhase::Regexp(source) => {
                let JsValue::Object(id) = &self.0.first else {
                    runtime.release_jsvalue(value)?;
                    return Err(RuntimeError::Invariant("String IsRegExp lost its object"));
                };
                let regexp = if matches!(value, JsValue::Undefined) {
                    ObjectRef::from_borrowed_handle(runtime.clone(), *id)
                        .map_err(RuntimeError::from)
                        .and_then(|object| runtime.native_object_has_regexp_brand(&object))
                } else {
                    runtime.value_to_boolean_jsvalue(&value)
                };
                runtime.release_jsvalue(value)?;
                let regexp = regexp?;
                if regexp {
                    return Ok(StringSearchStep::Complete(Completion::Throw(
                        runtime.new_native_error_jsvalue(
                            realm,
                            NativeErrorKind::Type,
                            "regexp not supported",
                        )?,
                    )));
                }
                {
                    let updated_0 = SearchPhase::Source;
                    self.0.phase = updated_0;
                    self
                }
                .needle(runtime, source)
            }
            SearchPhase::Needle(source) => {
                let needle = match string_value(runtime, realm, value)? {
                    NativeConversion::Value(value) => value,
                    NativeConversion::Throw(value) => {
                        return Ok(StringSearchStep::Complete(Completion::Throw(value)));
                    }
                };
                i32::try_from(source.len()).map_err(|_| {
                    RuntimeError::Invariant("String length exceeded QuickJS's signed index range")
                })?;
                i32::try_from(needle.len()).map_err(|_| {
                    RuntimeError::Invariant(
                        "String search length exceeded QuickJS's signed index range",
                    )
                })?;
                let next = {
                    let updated_0 = SearchPhase::Source;
                    self.0.phase = updated_0;
                    self
                };
                if next.actual > 1
                    && !(matches!(next.kind, StringSearchKind::Includes(_))
                        && matches!(next.second, JsValue::Undefined))
                {
                    let value = runtime.dup_jsvalue(&next.second)?;
                    Ok(next.primitive(
                        value,
                        ToPrimitiveHint::Number,
                        SearchPhase::Position { source, needle },
                    ))
                } else {
                    next.finish_search(runtime, source, needle, None)
                }
            }
            SearchPhase::Position { source, needle } => {
                let position = match number_value(runtime, realm, value)? {
                    NativeConversion::Value(value) => value,
                    NativeConversion::Throw(value) => {
                        return Ok(StringSearchStep::Complete(Completion::Throw(value)));
                    }
                };
                {
                    let updated_0 = SearchPhase::Source;
                    self.0.phase = updated_0;
                    self
                }
                .finish_search(runtime, source, needle, Some(position))
            }
            SearchPhase::Start(source) => {
                let start = match number_value(runtime, realm, value)? {
                    NativeConversion::Value(value) => value,
                    NativeConversion::Throw(value) => {
                        return Ok(StringSearchStep::Complete(Completion::Throw(value)));
                    }
                };
                let next = {
                    let updated_0 = SearchPhase::Source;
                    self.0.phase = updated_0;
                    self
                };
                if matches!(next.second, JsValue::Undefined) {
                    let StringSearchKind::Subrange(kind) = next.kind else {
                        return Err(RuntimeError::Invariant("String start reply lost its kind"));
                    };
                    Ok(StringSearchStep::Complete(
                        runtime.finish_string_subrange(kind, source, start, None)?,
                    ))
                } else {
                    let value = runtime.dup_jsvalue(&next.second)?;
                    Ok(next.primitive(
                        value,
                        ToPrimitiveHint::Number,
                        SearchPhase::End { source, start },
                    ))
                }
            }
            SearchPhase::End { source, start } => {
                let end = match number_value(runtime, realm, value)? {
                    NativeConversion::Value(value) => value,
                    NativeConversion::Throw(value) => {
                        return Ok(StringSearchStep::Complete(Completion::Throw(value)));
                    }
                };
                let StringSearchKind::Subrange(kind) = self.0.kind else {
                    return Err(RuntimeError::Invariant("String end reply lost its kind"));
                };
                Ok(StringSearchStep::Complete(runtime.finish_string_subrange(
                    kind,
                    source,
                    start,
                    Some(end),
                )?))
            }
        }
    }
}
fn string_value(
    runtime: &Runtime,
    realm: ContextId,
    value: JsValue,
) -> Result<NativeConversion<JsString>, RuntimeError> {
    let result = runtime.string_from_primitive_jsvalue(realm, &value);
    runtime.release_jsvalue(value)?;
    result
}
fn number_value(
    runtime: &Runtime,
    realm: ContextId,
    value: JsValue,
) -> Result<NativeConversion<f64>, RuntimeError> {
    let result = runtime.number_from_primitive_jsvalue(realm, &value);
    runtime.release_jsvalue(value)?;
    result
}
pub(super) fn finish(
    runtime: &Runtime,
    realm: ContextId,
    mut step: StringSearchStep,
) -> Result<Completion, RuntimeError> {
    loop {
        step = match step {
            StringSearchStep::Complete(result) => return Ok(result),
            StringSearchStep::Read {
                object,
                key,
                resume,
            } => resume.resume(
                runtime,
                runtime.get_property_in_realm(realm, &object, &key)?,
            )?,
            StringSearchStep::Primitive {
                value,
                hint,
                resume,
            } => {
                let result = if matches!(value, JsValue::Object(_)) {
                    runtime.to_primitive_jsvalue(realm, value, hint)?
                } else {
                    Completion::Return(value)
                };
                resume.resume(runtime, result)?
            }
        };
    }
}

// S11 all-domain protocol bound; inline completion stays allocation-free.
const _: () = assert!(std::mem::size_of::<StringSearchStep>() <= 64);
