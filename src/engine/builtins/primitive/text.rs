//! String scalar operations retain the converted receiver before argument coercions.

use crate::engine::builtins::native::NativeFunctionId;
use crate::engine::{
    api::{
        error::{Error, NativeErrorKind},
        runtime::Runtime,
        runtime_error::RuntimeError,
    },
    builtins::native::{StringCharAtKind, StringWellFormedKind},
    heap::ContextId,
    value::{JsString, JsValue, Value, conversion::NativeConversion},
    vm::{
        Completion,
        call::{NativeArguments, NativeInvocation},
    },
};
#[derive(Clone, Copy)]
pub(crate) enum ScalarTextKind {
    CharAt(StringCharAtKind),
    CharCodeAt,
    CodePointAt,
    Concat,
    WellFormed(StringWellFormedKind),
    Iterator,
}
impl ScalarTextKind {
    pub(crate) fn for_target(target: NativeFunctionId) -> Option<Self> {
        Some(match target {
            NativeFunctionId::StringPrototypeCharAt(kind) => Self::CharAt(kind),
            NativeFunctionId::StringPrototypeCharCodeAt => Self::CharCodeAt,
            NativeFunctionId::StringPrototypeCodePointAt => Self::CodePointAt,
            NativeFunctionId::StringPrototypeConcat => Self::Concat,
            NativeFunctionId::StringPrototypeWellFormed(kind) => Self::WellFormed(kind),
            NativeFunctionId::StringPrototypeIterator => Self::Iterator,
            _ => return None,
        })
    }
}
pub(crate) enum ScalarTextStep {
    Complete(Completion),
    String {
        value: JsValue,
        resume: ScalarTextResume,
    },
    Number {
        value: JsValue,
        resume: ScalarTextResume,
    },
}
enum Phase {
    Source,
    Chunk,
    Index,
}
pub(crate) struct ScalarTextResume(Box<ScalarTextResumeState>);
impl std::ops::Deref for ScalarTextResume {
    type Target = ScalarTextResumeState;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for ScalarTextResume {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
const _: () = assert!(std::mem::size_of::<ScalarTextResume>() <= 8);
pub(crate) struct ScalarTextResumeState {
    runtime: Runtime,
    realm: ContextId,
    kind: ScalarTextKind,
    phase: Phase,
    string: JsString,
    arguments: std::vec::IntoIter<JsValue>,
}
impl Drop for ScalarTextResumeState {
    /// Release argument edges still owned when conversion abandons the call.
    /// Consumption drains the iterator; releases are defer-safe and nothrow.
    fn drop(&mut self) {
        for value in self.arguments.by_ref() {
            let _ = self.runtime.release_jsvalue(value);
        }
    }
}
impl ScalarTextStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        kind: ScalarTextKind,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        let NativeInvocation::Call { this_value } = invocation else {
            return Err(RuntimeError::Invariant(
                "String scalar method requires generic invocation",
            ));
        };
        if matches!(this_value, JsValue::Null | JsValue::Undefined) {
            return Ok(Self::Complete(Completion::Throw(
                runtime.new_native_error_jsvalue(
                    realm,
                    NativeErrorKind::Type,
                    "null or undefined are forbidden",
                )?,
            )));
        }
        if let Some(result) = complete_primitive(runtime, realm, kind, this_value, arguments)? {
            #[cfg(feature = "profiling")]
            crate::engine::api::profiling::record_owned_execution_event(
                "string_scalar_completed_without_state",
            );
            return Ok(Self::Complete(result));
        }
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_execution_event(
            "string_scalar_resident_allocated",
        );
        let arguments = match kind {
            ScalarTextKind::Concat => {
                let mut values = Vec::new();
                values
                    .try_reserve_exact(arguments.actual_arg_count)
                    .map_err(|_| RuntimeError::Invariant("String concat argv allocation failed"))?;
                for value in &arguments.readable[..arguments.actual_arg_count] {
                    match runtime.dup_jsvalue(value) {
                        Ok(value) => values.push(value),
                        Err(error) => {
                            for value in values {
                                let _ = runtime.release_jsvalue(value);
                            }
                            return Err(error);
                        }
                    }
                }
                values
            }
            _ => vec![match arguments.readable.first() {
                Some(value) => runtime.dup_jsvalue(value)?,
                None => JsValue::Undefined,
            }],
        };
        let this_value = match runtime.dup_jsvalue(this_value) {
            Ok(value) => value,
            Err(error) => {
                for value in arguments {
                    let _ = runtime.release_jsvalue(value);
                }
                return Err(error);
            }
        };
        Ok(Self::String {
            value: this_value,
            resume: ScalarTextResume(Box::new(ScalarTextResumeState {
                runtime: runtime.clone(),
                realm,
                kind,
                phase: Phase::Source,
                string: JsString::from_static(""),
                arguments: arguments.into_iter(),
            })),
        })
    }
}
impl ScalarTextResume {
    pub(crate) fn string(
        mut self,
        runtime: &Runtime,
        result: NativeConversion<JsString>,
    ) -> Result<ScalarTextStep, RuntimeError> {
        let string = match result {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => {
                return Ok(ScalarTextStep::Complete(Completion::Throw(value)));
            }
        };
        match self.0.phase {
            Phase::Source => self.0.string = string,
            Phase::Chunk => {
                self.0.string = self.0.string.try_concat(&string).map_err(Error::from)?
            }
            _ => {
                return Err(RuntimeError::Invariant(
                    "String scalar string phase mismatch",
                ));
            }
        }
        match self.0.kind {
            ScalarTextKind::WellFormed(kind) => {
                Ok(ScalarTextStep::Complete(Completion::Return(match kind {
                    StringWellFormedKind::IsWellFormed => {
                        JsValue::Bool(self.0.string.is_well_formed())
                    }
                    StringWellFormedKind::ToWellFormed => {
                        runtime.unroot_value(&Value::String(self.0.string.to_well_formed()))?
                    }
                })))
            }
            ScalarTextKind::Iterator => Ok(ScalarTextStep::Complete(Completion::Return(
                JsValue::Object(
                    runtime
                        .new_string_iterator(
                            self.0.realm,
                            std::mem::replace(&mut self.0.string, JsString::from_static("")),
                        )?
                        .into_handle(),
                ),
            ))),
            ScalarTextKind::Concat => self.concat(runtime),
            _ => {
                self.0.phase = Phase::Index;
                Ok(ScalarTextStep::Number {
                    value: self.0.arguments.next().unwrap_or(JsValue::Undefined),
                    resume: self,
                })
            }
        }
    }
    fn concat(mut self, runtime: &Runtime) -> Result<ScalarTextStep, RuntimeError> {
        loop {
            match self.0.arguments.next() {
                None => {
                    let string = std::mem::replace(&mut self.0.string, JsString::from_static(""));
                    return Ok(ScalarTextStep::Complete(Completion::Return(
                        runtime.unroot_value(&Value::String(string))?,
                    )));
                }
                Some(JsValue::String(id)) => {
                    let chunk = runtime.0.state.borrow().heap.string(id).cloned();
                    runtime.release_jsvalue(JsValue::String(id))?;
                    self.0.string = self.0.string.try_concat(&chunk?).map_err(Error::from)?;
                }
                Some(value) => {
                    self.0.phase = Phase::Chunk;
                    return Ok(ScalarTextStep::String {
                        value,
                        resume: self,
                    });
                }
            }
        }
    }
    pub(crate) fn number(
        self,
        runtime: &Runtime,
        result: NativeConversion<f64>,
    ) -> Result<ScalarTextStep, RuntimeError> {
        if !matches!(self.0.phase, Phase::Index) {
            if let NativeConversion::Throw(value) = result {
                let _ = runtime.release_jsvalue(value);
            }
            return Err(RuntimeError::Invariant(
                "String scalar index phase mismatch",
            ));
        }
        let number = match result {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => {
                return Ok(ScalarTextStep::Complete(Completion::Throw(value)));
            }
        };
        finish_index(runtime, self.kind, &self.string, number).map(ScalarTextStep::Complete)
    }
}

/// Borrow primitive operands while the native activation still owns them.
/// Objects take the existing ordered conversion path; no JavaScript can run
/// between these primitive conversions, so no argument snapshot is required.
fn complete_primitive(
    runtime: &Runtime,
    realm: ContextId,
    kind: ScalarTextKind,
    receiver: &JsValue,
    arguments: &NativeArguments,
) -> Result<Option<Completion>, RuntimeError> {
    if matches!(receiver, JsValue::Object(_)) {
        return Ok(None);
    }
    let first = arguments.readable.first().unwrap_or(&JsValue::Undefined);
    let inputs = match kind {
        ScalarTextKind::Concat => &arguments.readable[..arguments.actual_arg_count],
        ScalarTextKind::CharAt(_) | ScalarTextKind::CharCodeAt | ScalarTextKind::CodePointAt => {
            std::slice::from_ref(first)
        }
        _ => &[],
    };
    if inputs
        .iter()
        .any(|value| matches!(value, JsValue::Object(_)))
    {
        return Ok(None);
    }
    let mut string = match runtime.string_from_primitive_jsvalue(realm, receiver)? {
        NativeConversion::Value(value) => value,
        NativeConversion::Throw(value) => return Ok(Some(Completion::Throw(value))),
    };
    let value = match kind {
        ScalarTextKind::CharAt(_) | ScalarTextKind::CharCodeAt | ScalarTextKind::CodePointAt => {
            let number = match runtime.number_from_primitive_jsvalue(realm, first)? {
                NativeConversion::Value(value) => value,
                NativeConversion::Throw(value) => return Ok(Some(Completion::Throw(value))),
            };
            return finish_index(runtime, kind, &string, number).map(Some);
        }
        ScalarTextKind::Concat => {
            for argument in inputs {
                // Match the resumed concat path: existing string chunks can
                // remain ropes, without flattening them for conversion.
                let chunk = if let JsValue::String(id) = argument {
                    runtime.0.state.borrow().heap.string(*id)?.clone()
                } else {
                    match runtime.string_from_primitive_jsvalue(realm, argument)? {
                        NativeConversion::Value(value) => value,
                        NativeConversion::Throw(value) => {
                            return Ok(Some(Completion::Throw(value)));
                        }
                    }
                };
                string = string.try_concat(&chunk).map_err(Error::from)?;
            }
            runtime.into_jsvalue(Value::String(string))?
        }
        ScalarTextKind::WellFormed(StringWellFormedKind::IsWellFormed) => {
            JsValue::Bool(string.is_well_formed())
        }
        ScalarTextKind::WellFormed(StringWellFormedKind::ToWellFormed) => {
            runtime.into_jsvalue(Value::String(string.to_well_formed()))?
        }
        ScalarTextKind::Iterator => {
            JsValue::Object(runtime.new_string_iterator(realm, string)?.into_handle())
        }
    };
    Ok(Some(Completion::Return(value)))
}

/// The callback-free scalar kernel is shared by local and resumed execution.
fn finish_index(
    runtime: &Runtime,
    kind: ScalarTextKind,
    string: &JsString,
    number: f64,
) -> Result<Completion, RuntimeError> {
    let mut index = crate::engine::value::number::to_int32_sat(number);
    let value = match kind {
        ScalarTextKind::CharAt(kind) => {
            let length = i32::try_from(string.len()).map_err(|_| {
                RuntimeError::Invariant("String length exceeded QuickJS signed index range")
            })?;
            if kind == StringCharAtKind::At && index < 0 {
                index += length;
            }
            if index < 0 || index >= length {
                match kind {
                    StringCharAtKind::At => Value::Undefined,
                    StringCharAtKind::CharAt => Value::String(JsString::from_static("")),
                }
            } else {
                Value::String(JsString::from_code_unit(
                    string
                        .code_unit_at(index as usize)
                        .ok_or(RuntimeError::Invariant(
                            "validated String index missing code unit",
                        ))?,
                ))
            }
        }
        ScalarTextKind::CharCodeAt => usize::try_from(index)
            .ok()
            .and_then(|index| string.code_unit_at(index))
            .map_or(Value::Float(f64::NAN), |unit| Value::Int(i32::from(unit))),
        ScalarTextKind::CodePointAt => usize::try_from(index)
            .ok()
            .and_then(|index| string.code_point_at(index))
            .map_or(Value::Undefined, |point| Value::Int(point as i32)),
        _ => return Err(RuntimeError::Invariant("String scalar index kind mismatch")),
    };
    Ok(Completion::Return(runtime.into_jsvalue(value)?))
}

pub(crate) fn finish(
    runtime: &Runtime,
    realm: ContextId,
    mut step: ScalarTextStep,
) -> Result<Completion, RuntimeError> {
    loop {
        step = match step {
            ScalarTextStep::Complete(result) => return Ok(result),
            ScalarTextStep::String { value, resume } => {
                resume.string(runtime, runtime.native_to_js_string_jsvalue(realm, value)?)?
            }
            ScalarTextStep::Number { value, resume } => {
                resume.number(runtime, runtime.native_to_number_jsvalue(realm, value)?)?
            }
        };
    }
}

// S11 all-domain protocol bound; inline completion stays allocation-free.
const _: () = assert!(std::mem::size_of::<ScalarTextStep>() <= 64);

#[cfg(test)]
mod local_completion_tests {
    use super::*;

    #[test]
    fn scalar_text_primitives_complete_without_conversion_requests() {
        let runtime = Runtime::new();
        let context = runtime.new_context().expect("create context");
        let this_value = runtime
            .into_jsvalue(Value::String(JsString::from_static("abc")))
            .unwrap();
        let invocation = NativeInvocation::Call { this_value };
        let arguments = NativeArguments {
            actual_arg_count: 1,
            readable: vec![JsValue::Int(1)],
        };
        for (kind, expected) in [
            (ScalarTextKind::CharCodeAt, Value::Int(98)),
            (ScalarTextKind::CodePointAt, Value::Int(98)),
            (
                ScalarTextKind::CharAt(StringCharAtKind::At),
                Value::String(JsString::from_static("b")),
            ),
            (
                ScalarTextKind::Concat,
                Value::String(JsString::from_static("abc1")),
            ),
            (
                ScalarTextKind::WellFormed(StringWellFormedKind::IsWellFormed),
                Value::Bool(true),
            ),
        ] {
            let ScalarTextStep::Complete(Completion::Return(value)) =
                ScalarTextStep::start(&runtime, context.realm, kind, &invocation, &arguments)
                    .unwrap()
            else {
                panic!("primitive scalar string must complete locally")
            };
            assert_eq!(runtime.root_and_release_jsvalue(value).unwrap(), expected);
        }
        let NativeInvocation::Call { this_value } = invocation else {
            unreachable!()
        };
        runtime.release_jsvalue(this_value).unwrap();
    }

    #[test]
    fn scalar_text_local_concat_keeps_existing_rope_chunks() {
        let runtime = Runtime::new();
        let context = runtime.new_context().expect("create context");
        let leaf = JsString::try_from_utf8(&"x".repeat(1024)).unwrap();
        let chunk = leaf.try_concat(&leaf).unwrap();
        let mut before = 0;
        chunk.for_each_flat_leaf(|_| before += 1);
        assert_eq!(before, 2);
        let invocation = NativeInvocation::Call {
            this_value: runtime
                .into_jsvalue(Value::String(JsString::from_static("a")))
                .unwrap(),
        };
        let arguments = NativeArguments {
            actual_arg_count: 1,
            readable: vec![runtime.into_jsvalue(Value::String(chunk.clone())).unwrap()],
        };
        let ScalarTextStep::Complete(Completion::Return(value)) = ScalarTextStep::start(
            &runtime,
            context.realm,
            ScalarTextKind::Concat,
            &invocation,
            &arguments,
        )
        .unwrap() else {
            panic!("primitive concat must complete locally")
        };
        let mut after = 0;
        chunk.for_each_flat_leaf(|_| after += 1);
        assert_eq!(after, before, "concat must not linearize an existing chunk");
        let Value::String(result) = runtime.root_and_release_jsvalue(value).unwrap() else {
            panic!("string result")
        };
        assert_eq!(result.len(), 2049);
        let NativeInvocation::Call { this_value } = invocation else {
            unreachable!()
        };
        runtime.release_jsvalue(this_value).unwrap();
        for value in arguments.readable {
            runtime.release_jsvalue(value).unwrap();
        }
    }

    #[test]
    fn scalar_text_local_and_object_paths_keep_coercion_order_and_errors() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        assert_eq!(context.eval(r#"(() => {
            let trace = '';
            const receiver = { [Symbol.toPrimitive](hint) { trace += 'r:' + hint + ';'; return 'A\ud83d\ude00'; } };
            const index = { [Symbol.toPrimitive](hint) { trace += 'i:' + hint + ';'; return 1; } };
            if (String.prototype.codePointAt.call(receiver, index) !== 128512 || trace !== 'r:string;i:number;') return false;
            trace = '';
            const chunk = { toString() { trace += 'c;'; return 'z'; } };
            if (String.prototype.concat.call(receiver, 1, chunk, null) !== 'A\ud83d\ude001znull' || trace !== 'r:string;c;') return false;
            trace = '';
            const bad = { toString() { trace += 'bad;'; throw 42; } };
            try { 'a'.concat('b', bad, chunk); return false; } catch (error) { if (error !== 42 || trace !== 'bad;') return false; }
            trace = '';
            try { String.prototype.charCodeAt.call(Symbol(), index); return false; } catch (error) { if (!(error instanceof TypeError) || trace !== '') return false; }
            try { 'a'.concat(Symbol(), chunk); return false; } catch (error) { if (!(error instanceof TypeError) || trace !== '') return false; }
            for (const value of [Symbol(), 1n]) {
                try { 'a'.charCodeAt(value); return false; } catch (error) { if (!(error instanceof TypeError)) return false; }
            }
            return 'abc'.charCodeAt() === 97 && 'abc'.charCodeAt('1') === 98
                && 'abc'.at(-1) === 'c' && 'abc'.at(-Infinity) === undefined
                && 'abc'.charAt(Infinity) === '' && Number.isNaN('abc'.charCodeAt(-1))
                && '\ud83d\ude00'.codePointAt(0) === 128512 && '\ud800'.codePointAt(0) === 55296
                && 'a'.concat(null, undefined, true, 2n) === 'anullundefinedtrue2'
                && !'\ud800'.isWellFormed() && '\ud800'.toWellFormed() === '\ufffd'
                && Array.from('A\ud83d\ude00').join('|') === 'A|\ud83d\ude00';
        })()"#).unwrap(), Value::Bool(true));
    }
}
