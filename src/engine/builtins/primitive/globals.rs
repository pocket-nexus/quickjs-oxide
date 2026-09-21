//! Global parsers and codecs keep their converted input until later argument conversion ends.

use crate::engine::builtins::native::NativeFunctionId;
use crate::engine::{
    api::{runtime::Runtime, runtime_error::RuntimeError},
    builtins::native::{
        GlobalNumberPredicateKind, GlobalUriCodecKind, NumberParseKind, SymbolRegistryKind,
    },
    heap::ContextId,
    value::{JsString, JsValue, Value, conversion::NativeConversion},
    vm::{
        Completion,
        call::{NativeArguments, NativeInvocation},
    },
};
#[derive(Clone, Copy)]
pub(crate) enum GlobalKind {
    Parse(NumberParseKind),
    Predicate(GlobalNumberPredicateKind),
    Uri(GlobalUriCodecKind),
    SymbolFor,
}
impl GlobalKind {
    pub(crate) fn for_target(target: NativeFunctionId) -> Option<Self> {
        Some(match target {
            NativeFunctionId::GlobalNumberParse(kind) => Self::Parse(kind),
            NativeFunctionId::GlobalNumberPredicate(kind) => Self::Predicate(kind),
            NativeFunctionId::GlobalUriCodec(kind) => Self::Uri(kind),
            NativeFunctionId::SymbolRegistry(SymbolRegistryKind::For) => Self::SymbolFor,
            _ => return None,
        })
    }
}
pub(crate) enum GlobalStep {
    Complete(Completion),
    String {
        value: JsValue,
        resume: GlobalResume,
    },
    Number {
        value: JsValue,
        resume: GlobalResume,
    },
}
pub(crate) struct GlobalResume(Box<GlobalResumeState>);
impl std::ops::Deref for GlobalResume {
    type Target = GlobalResumeState;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for GlobalResume {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
const _: () = assert!(std::mem::size_of::<GlobalResume>() <= 8);
pub(crate) struct GlobalResumeState {
    runtime: Runtime,
    realm: ContextId,
    kind: GlobalKind,
    radix: JsValue,
    input: Option<JsString>,
}

impl Drop for GlobalResumeState {
    /// The radix argument edge is duplicated into the later number stage but
    /// the state keeps its own copy; surrender it whenever the request is
    /// consumed or abandoned.
    fn drop(&mut self) {
        let radix = std::mem::replace(&mut self.radix, JsValue::Undefined);
        let _ = self.runtime.release_jsvalue(radix);
    }
}
impl GlobalStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        kind: GlobalKind,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        if !matches!(invocation, NativeInvocation::Call { .. }) {
            return Err(RuntimeError::Invariant(
                "global builtin requires generic invocation",
            ));
        }
        let radix = match arguments.readable.get(1) {
            Some(value) => runtime.dup_jsvalue(value)?,
            None => JsValue::Undefined,
        };
        let resume = GlobalResume(Box::new(GlobalResumeState {
            runtime: runtime.clone(),
            realm,
            kind,
            radix,
            input: None,
        }));
        // The resume owns the radix edge, so a failed input dup drops it on
        // the error path instead of leaking it.
        let value = runtime.dup_jsvalue(arguments.readable.first().ok_or(
            RuntimeError::Invariant("global builtin argv was not padded"),
        )?)?;
        Ok(if matches!(kind, GlobalKind::Predicate(_)) {
            Self::Number { value, resume }
        } else {
            Self::String { value, resume }
        })
    }
    /// Advance only primitive conversion stages, retaining the original request
    /// before an object lookup or callback. Parse input precedes radix conversion.
    pub(crate) fn advance_primitive(
        mut self,
        runtime: &Runtime,
        realm: ContextId,
    ) -> Result<Self, RuntimeError> {
        use crate::engine::value::conversion::number::NumberStep;
        loop {
            self = match self {
                Self::String { value, resume } if !matches!(value, JsValue::Object(_)) => {
                    let value = runtime.root_and_release_jsvalue(value)?;
                    resume.string(runtime, runtime.string_from_primitive(realm, &value)?)?
                }
                Self::Number { value, resume } if !matches!(value, JsValue::Object(_)) => {
                    let value = runtime.root_and_release_jsvalue(value)?;
                    let NumberStep::Complete(result) = NumberStep::start(runtime, realm, value)?
                    else {
                        return Err(RuntimeError::Invariant("primitive global number suspended"));
                    };
                    resume.number(runtime, result)?
                }
                Self::Complete(completion) => {
                    #[cfg(feature = "profiling")]
                    crate::engine::api::profiling::record_owned_execution_event(
                        "global_completed_without_waiting_state",
                    );
                    return Ok(Self::Complete(completion));
                }
                step => return Ok(step),
            };
        }
    }
}
impl GlobalResume {
    pub(crate) fn string(
        mut self,
        runtime: &Runtime,
        result: NativeConversion<JsString>,
    ) -> Result<GlobalStep, RuntimeError> {
        let input = match result {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => {
                return Ok(GlobalStep::Complete(Completion::Throw(
                    runtime.into_jsvalue(value)?,
                )));
            }
        };
        match self.0.kind {
            GlobalKind::Parse(NumberParseKind::ParseInt) => {
                self.0.input = Some(input);
                Ok(GlobalStep::Number {
                    value: runtime.dup_jsvalue(&self.0.radix)?,
                    resume: self,
                })
            }
            GlobalKind::Parse(NumberParseKind::ParseFloat) => {
                Ok(GlobalStep::Complete(Completion::Return(
                    crate::engine::value::number::operations::Number::compact(
                        crate::engine::value::number_parse::parse_float(&input),
                    )
                    .into(),
                )))
            }
            GlobalKind::Uri(kind) => Ok(GlobalStep::Complete(runtime.finish_global_uri_codec(
                self.0.realm,
                kind,
                input,
            )?)),
            GlobalKind::SymbolFor => {
                let symbol = runtime.symbol_for(&input)?;
                Ok(GlobalStep::Complete(Completion::Return(
                    runtime.unroot_value(&Value::Symbol(symbol))?,
                )))
            }
            _ => Err(RuntimeError::Invariant("global string reply mismatch")),
        }
    }
    pub(crate) fn number(
        mut self,
        runtime: &Runtime,
        result: NativeConversion<f64>,
    ) -> Result<GlobalStep, RuntimeError> {
        let number = match result {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => {
                return Ok(GlobalStep::Complete(Completion::Throw(
                    runtime.into_jsvalue(value)?,
                )));
            }
        };
        let value = match self.0.kind {
            GlobalKind::Parse(NumberParseKind::ParseInt) => {
                let input = self
                    .0
                    .input
                    .take()
                    .ok_or(RuntimeError::Invariant("parseInt converted input missing"))?;
                Value::number(crate::engine::value::number_parse::parse_int(
                    &input,
                    crate::engine::value::number::to_int32(number),
                ))
            }
            GlobalKind::Predicate(kind) => Value::Bool(match kind {
                GlobalNumberPredicateKind::IsNaN => number.is_nan(),
                GlobalNumberPredicateKind::IsFinite => number.is_finite(),
            }),
            _ => return Err(RuntimeError::Invariant("global number reply mismatch")),
        };
        Ok(GlobalStep::Complete(Completion::Return(
            runtime.into_jsvalue(value)?,
        )))
    }
}
pub(crate) fn finish(
    runtime: &Runtime,
    realm: ContextId,
    mut step: GlobalStep,
) -> Result<Completion, RuntimeError> {
    loop {
        step = match step {
            GlobalStep::Complete(result) => return Ok(result),
            GlobalStep::String { value, resume } => {
                let value = runtime.root_and_release_jsvalue(value)?;
                resume.string(runtime, runtime.native_to_js_string(realm, &value)?)?
            }
            GlobalStep::Number { value, resume } => {
                let value = runtime.root_and_release_jsvalue(value)?;
                resume.number(runtime, runtime.native_to_number(realm, &value)?)?
            }
        };
    }
}

// S11 all-domain protocol bound; inline completion stays allocation-free.
const _: () = assert!(std::mem::size_of::<GlobalStep>() <= 64);
