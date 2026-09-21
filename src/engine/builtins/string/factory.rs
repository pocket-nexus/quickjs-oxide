//! String factories share conversion order and String.raw's latched buffer error.

use crate::engine::builtins::native::NativeFunctionId;
use crate::engine::{
    api::{error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError},
    builtins::native::StringStaticKind,
    heap::ContextId,
    object::{ObjectRef, PropertyKey},
    value::{JsString, JsStringBuilder, JsValue, Value, conversion::NativeConversion},
    vm::{Completion, call::NativeArguments},
};
#[derive(Clone, Copy)]
pub(crate) enum StringFactoryKind {
    Static(StringStaticKind),
    #[cfg(feature = "test262-host")]
    CodePointRange,
}
impl StringFactoryKind {
    pub(crate) fn for_target(target: NativeFunctionId) -> Option<Self> {
        match target {
            NativeFunctionId::StringStatic(kind) => Some(Self::Static(kind)),
            #[cfg(feature = "test262-host")]
            NativeFunctionId::StringCodePointRange => Some(Self::CodePointRange),
            _ => None,
        }
    }
}
pub(crate) enum StringFactoryStep {
    Complete(Completion),
    Number {
        value: JsValue,
        resume: StringFactoryResume,
    },
    String {
        value: JsValue,
        resume: StringFactoryResume,
    },
    Read {
        object: ObjectRef,
        key: PropertyKey,
        resume: StringFactoryResume,
    },
}
enum Phase {
    Characters,
    Raw,
    Length,
    Chunk,
    Substitution,
    #[cfg(feature = "test262-host")]
    RangeStart,
    #[cfg(feature = "test262-host")]
    RangeEnd(u32),
}
pub(crate) struct StringFactoryResume(Box<StringFactoryResumeState>);
impl std::ops::Deref for StringFactoryResume {
    type Target = StringFactoryResumeState;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for StringFactoryResume {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
const _: () = assert!(std::mem::size_of::<StringFactoryResume>() <= 8);
pub(crate) struct StringFactoryResumeState {
    runtime: Runtime,
    realm: ContextId,
    kind: StringFactoryKind,
    arguments: Vec<JsValue>,
    actual: usize,
    cooked: Option<ObjectRef>,
    raw: Option<ObjectRef>,
    chunk: JsValue,
    length_value: JsValue,
    length: u64,
    index: u64,
    builder: Option<JsStringBuilder>,
    limit: usize,
    phase: Phase,
}
impl Drop for StringFactoryResumeState {
    fn drop(&mut self) {
        for value in self.arguments.drain(..) {
            let _ = self.runtime.release_jsvalue(value);
        }
        for value in [&mut self.chunk, &mut self.length_value] {
            let _ = self
                .runtime
                .release_jsvalue(std::mem::replace(value, JsValue::Undefined));
        }
    }
}
impl StringFactoryStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        kind: StringFactoryKind,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        Self::with_limit(runtime, realm, kind, arguments, JsString::MAX_LEN)
    }
    pub(crate) fn with_limit(
        runtime: &Runtime,
        realm: ContextId,
        kind: StringFactoryKind,
        arguments: &NativeArguments,
        limit: usize,
    ) -> Result<Self, RuntimeError> {
        let mut resume = StringFactoryResume(Box::new(StringFactoryResumeState {
            runtime: runtime.clone(),
            realm,
            kind,
            arguments: Vec::with_capacity(arguments.readable.len()),
            actual: arguments.actual_arg_count,
            cooked: None,
            raw: None,
            chunk: JsValue::Undefined,
            length_value: JsValue::Undefined,
            length: 0,
            index: 0,
            builder: None,
            limit,
            phase: Phase::Characters,
        }));
        for value in &arguments.readable {
            resume.arguments.push(runtime.dup_jsvalue(value)?);
        }
        match kind {
            StringFactoryKind::Static(StringStaticKind::Raw) => {
                let template = resume.arguments.first().ok_or(RuntimeError::Invariant(
                    "String.raw template argv was not padded",
                ))?;
                let cooked = match runtime
                    .native_to_object_jsvalue(realm, runtime.dup_jsvalue(template)?)?
                {
                    NativeConversion::Value(value) => value,
                    NativeConversion::Throw(value) => {
                        return Ok(Self::Complete(Completion::Throw(
                            runtime.into_jsvalue(value)?,
                        )));
                    }
                };
                resume.cooked = Some(cooked.clone());
                resume.phase = Phase::Raw;
                Ok(Self::Read {
                    object: cooked,
                    key: runtime
                        .pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Raw)?,
                    resume,
                })
            }
            StringFactoryKind::Static(_) => {
                resume.builder = Some(JsStringBuilder::new(arguments.actual_arg_count));
                resume.next(runtime)
            }
            #[cfg(feature = "test262-host")]
            StringFactoryKind::CodePointRange => {
                resume.phase = Phase::RangeStart;
                let value = resume
                    .arguments
                    .first()
                    .map(|value| runtime.dup_jsvalue(value))
                    .transpose()?
                    .ok_or(RuntimeError::Invariant(
                        "String codePointRange start argv was not padded",
                    ))?;
                Ok(Self::Number { value, resume })
            }
        }
    }
}
impl StringFactoryResume {
    fn abrupt(self, value: JsValue) -> StringFactoryStep {
        StringFactoryStep::Complete(Completion::Throw(value))
    }
    fn complete(mut self, runtime: &Runtime) -> Result<StringFactoryStep, RuntimeError> {
        let builder = self
            .0
            .builder
            .take()
            .ok_or(RuntimeError::Invariant("String factory lost builder"))?;
        Ok(StringFactoryStep::Complete(Completion::Return(
            runtime.into_jsvalue(Value::String(builder.finish()?))?,
        )))
    }
    fn builder(&mut self) -> Result<&mut JsStringBuilder, RuntimeError> {
        self.0
            .builder
            .as_mut()
            .ok_or(RuntimeError::Invariant("String factory lost builder"))
    }
    fn next(mut self, runtime: &Runtime) -> Result<StringFactoryStep, RuntimeError> {
        if matches!(
            self.0.kind,
            StringFactoryKind::Static(StringStaticKind::Raw)
        ) {
            runtime.release_jsvalue(std::mem::replace(&mut self.0.chunk, JsValue::Undefined))?;
            if self.0.index == self.0.length {
                return self.complete(runtime);
            }
            self.0.phase = Phase::Chunk;
            return Ok(StringFactoryStep::Read {
                object: self
                    .0
                    .raw
                    .as_ref()
                    .ok_or(RuntimeError::Invariant("String.raw lost raw object"))?
                    .clone(),
                key: runtime.intern_property_key(&self.0.index.to_string())?,
                resume: self,
            });
        }
        while self.0.index < self.0.actual as u64 {
            let value = self
                .0
                .arguments
                .get(self.0.index as usize)
                .map(|value| runtime.dup_jsvalue(value))
                .transpose()?
                .ok_or(RuntimeError::Invariant("String factory argument missing"))?;
            if matches!(
                self.0.kind,
                StringFactoryKind::Static(StringStaticKind::FromCodePoint)
            ) {
                if let JsValue::Int(value) = value {
                    if !(0..=0x10_ffff).contains(&value) {
                        let error = runtime.new_native_error_jsvalue(
                            self.0.realm,
                            NativeErrorKind::Range,
                            "invalid code point",
                        )?;
                        return Ok(self.abrupt(error));
                    }
                    self.builder()?.push_code_point(value as u32)?;
                    self.0.index += 1;
                    continue;
                }
            }
            return Ok(StringFactoryStep::Number {
                value,
                resume: self,
            });
        }
        self.complete(runtime)
    }
    pub(crate) fn number(
        mut self,
        runtime: &Runtime,
        result: NativeConversion<f64>,
    ) -> Result<StringFactoryStep, RuntimeError> {
        let number = match result {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => {
                return Ok(self.abrupt(runtime.into_jsvalue(value)?));
            }
        };
        match self.0.phase {
            Phase::Characters => {
                let code_point = if matches!(
                    self.0.kind,
                    StringFactoryKind::Static(StringStaticKind::FromCharCode)
                ) {
                    (crate::engine::value::number::to_int32(number) as u32) & 0xffff
                } else {
                    if !number.is_finite()
                        || number < 0.0
                        || number > 0x10_ffff as f64
                        || number.fract() != 0.0
                    {
                        let error = runtime.new_native_error_jsvalue(
                            self.0.realm,
                            NativeErrorKind::Range,
                            "invalid code point",
                        )?;
                        return Ok(self.abrupt(error));
                    }
                    number as u32
                };
                self.builder()?.push_code_point(code_point)?;
                self.0.index += 1;
                self.next(runtime)
            }
            Phase::Length => {
                self.0.length = Runtime::length_from_number(number);
                self.0.builder = Some(JsStringBuilder::with_limit(0, self.0.limit));
                self.next(runtime)
            }
            #[cfg(feature = "test262-host")]
            Phase::RangeStart => {
                self.0.phase = Phase::RangeEnd(Runtime::to_uint32_number(number));
                let value = self
                    .0
                    .arguments
                    .get(1)
                    .map(|value| runtime.dup_jsvalue(value))
                    .transpose()?
                    .ok_or(RuntimeError::Invariant(
                        "String codePointRange end argv was not padded",
                    ))?;
                Ok(StringFactoryStep::Number {
                    value,
                    resume: self,
                })
            }
            #[cfg(feature = "test262-host")]
            Phase::RangeEnd(start) => {
                let end = Runtime::to_uint32_number(number).min(0x11_0000);
                let start = start.min(end);
                let length = usize::try_from(end - start + end.saturating_sub(start.max(0x1_0000)))
                    .map_err(|_| {
                        RuntimeError::Invariant("codePointRange length did not fit usize")
                    })?;
                let mut builder = JsStringBuilder::try_with_exact_capacity(length)?;
                for point in start..end {
                    builder.push_code_point(point)?;
                }
                Ok(StringFactoryStep::Complete(Completion::Return(
                    runtime.into_jsvalue(Value::String(builder.finish()?))?,
                )))
            }
            _ => Err(RuntimeError::Invariant(
                "String factory number phase mismatch",
            )),
        }
    }
    pub(crate) fn resume(
        mut self,
        runtime: &Runtime,
        result: Completion,
    ) -> Result<StringFactoryStep, RuntimeError> {
        let value = match result {
            Completion::Return(value) => value,
            Completion::Throw(value) => return Ok(self.abrupt(value)),
        };
        match self.0.phase {
            Phase::Raw => {
                let raw = match runtime.native_to_object_jsvalue(self.0.realm, value)? {
                    NativeConversion::Value(value) => value,
                    NativeConversion::Throw(value) => {
                        return Ok(self.abrupt(runtime.into_jsvalue(value)?));
                    }
                };
                self.0.raw = Some(raw.clone());
                self.0.phase = Phase::Length;
                Ok(StringFactoryStep::Read {
                    object: raw,
                    key: runtime
                        .pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Length)?,
                    resume: self,
                })
            }
            Phase::Length => {
                runtime.release_jsvalue(std::mem::replace(&mut self.0.length_value, value))?;
                let value = runtime.dup_jsvalue(&self.0.length_value)?;
                Ok(StringFactoryStep::Number {
                    value,
                    resume: self,
                })
            }
            Phase::Chunk => {
                runtime.release_jsvalue(std::mem::replace(&mut self.0.chunk, value))?;
                let value = runtime.dup_jsvalue(&self.0.chunk)?;
                Ok(StringFactoryStep::String {
                    value,
                    resume: self,
                })
            }
            _ => {
                runtime.release_jsvalue(value)?;
                Err(RuntimeError::Invariant(
                    "String factory read phase mismatch",
                ))
            }
        }
    }
    pub(crate) fn string(
        mut self,
        runtime: &Runtime,
        result: NativeConversion<JsString>,
    ) -> Result<StringFactoryStep, RuntimeError> {
        let value = match result {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => {
                return Ok(self.abrupt(runtime.into_jsvalue(value)?));
            }
        };
        match self.0.phase {
            Phase::Chunk => {
                let append = self.builder()?.push_js_string(&value);
                let next = self.0.index + 1;
                let substitution = usize::try_from(next)
                    .ok()
                    .filter(|index| next < self.0.length && *index < self.0.actual);
                let Some(index) = substitution else {
                    // Raw append failure is latched; subsequent Get/ToString
                    // still run and a later user throw can replace that error.
                    let _ = append;
                    self.0.index += 1;
                    return self.next(runtime);
                };
                append?;
                self.0.phase = Phase::Substitution;
                let value = self
                    .0
                    .arguments
                    .get(index)
                    .map(|value| runtime.dup_jsvalue(value))
                    .transpose()?
                    .ok_or(RuntimeError::Invariant(
                        "String.raw substitution argv was not readable",
                    ))?;
                Ok(StringFactoryStep::String {
                    value,
                    resume: self,
                })
            }
            Phase::Substitution => {
                self.builder()?.push_js_string(&value)?;
                self.0.index += 1;
                self.next(runtime)
            }
            _ => Err(RuntimeError::Invariant(
                "String factory string phase mismatch",
            )),
        }
    }
}
pub(crate) fn finish(
    runtime: &Runtime,
    realm: ContextId,
    mut step: StringFactoryStep,
) -> Result<Completion, RuntimeError> {
    loop {
        step = match step {
            StringFactoryStep::Complete(result) => return Ok(result),
            StringFactoryStep::Number { value, resume } => {
                resume.number(runtime, runtime.native_to_number_jsvalue(realm, value)?)?
            }
            StringFactoryStep::String { value, resume } => {
                resume.string(runtime, runtime.native_to_js_string_jsvalue(realm, value)?)?
            }
            StringFactoryStep::Read {
                object,
                key,
                resume,
            } => resume.resume(
                runtime,
                runtime.get_property_in_realm(realm, &object, &key)?,
            )?,
        };
    }
}

// S11 all-domain protocol bound; inline completion stays allocation-free.
const _: () = assert!(std::mem::size_of::<StringFactoryStep>() <= 64);
