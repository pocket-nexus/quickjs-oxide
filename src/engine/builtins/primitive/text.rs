//! String scalar operations retain the converted receiver before argument coercions.

use crate::engine::builtins::native::NativeFunctionId;
use crate::engine::{
    api::{
        error::{Error, NativeErrorKind, NativeErrorMessage},
        runtime::Runtime,
        runtime_error::RuntimeError,
    },
    builtins::native::{StringCharAtKind, StringWellFormedKind},
    heap::{
        ContextId,
        runtime::{
            RuntimeState,
            owned_values::{OwnedValueGuard, OwnedValuesGuard},
        },
    },
    value::{JsString, JsValue, conversion::NativeConversion},
    vm::{
        Completion,
        call::{NativeArguments, NativeInvocation},
    },
};
use std::cell::Cell;

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
    realm: ContextId,
    kind: ScalarTextKind,
    phase: Phase,
    string: JsString,
    arguments: std::vec::IntoIter<JsValue>,
}
impl ScalarTextStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        kind: ScalarTextKind,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        let _unwind = runtime.unwind_guard();
        let result = Self::start_in_state(
            &mut runtime.0.state.borrow_mut(),
            &runtime.0.poisoned,
            realm,
            kind,
            invocation,
            arguments,
        );
        if result.is_ok() {
            runtime.check_poison()?;
        }
        result
    }
    pub(crate) fn start_in_state(
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
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
            return Ok(Self::Complete(Completion::Throw(JsValue::Object(
                state.new_native_error_from_message(
                    poisoned,
                    realm,
                    NativeErrorKind::Type,
                    NativeErrorMessage::from_utf8("null or undefined are forbidden"),
                )?,
            ))));
        }
        if let Some(result) =
            complete_primitive(state, poisoned, realm, kind, this_value, arguments)?
        {
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
        let mut values = Vec::new();
        if matches!(kind, ScalarTextKind::Concat) {
            values
                .try_reserve_exact(arguments.actual_arg_count)
                .map_err(|_| RuntimeError::Invariant("String concat argv allocation failed"))?;
        }
        let mut snapshots = OwnedValuesGuard::new(state, poisoned, values);
        let (state, values) = snapshots.parts();
        match kind {
            ScalarTextKind::Concat => {
                for value in &arguments.readable[..arguments.actual_arg_count] {
                    values.push(state.dup_jsvalue(value)?);
                }
            }
            _ => values.push(match arguments.readable.first() {
                Some(value) => state.dup_jsvalue(value)?,
                None => JsValue::Undefined,
            }),
        }
        // The complete argument snapshot precedes the independent receiver edge.
        let this_value = state.dup_jsvalue(this_value)?;
        Ok(Self::String {
            value: this_value,
            resume: ScalarTextResume(Box::new(ScalarTextResumeState {
                realm,
                kind,
                phase: Phase::Source,
                string: JsString::from_static(""),
                arguments: std::mem::take(values).into_iter(),
            })),
        })
    }
}
impl ScalarTextResume {
    pub(crate) fn string(
        self,
        runtime: &Runtime,
        result: NativeConversion<JsString>,
    ) -> Result<ScalarTextStep, RuntimeError> {
        let _unwind = runtime.unwind_guard();
        let result = self.string_in_state(
            &mut runtime.0.state.borrow_mut(),
            &runtime.0.poisoned,
            result,
        );
        if result.is_ok() {
            runtime.check_poison()?;
        }
        result
    }
    pub(crate) fn string_in_state(
        mut self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        result: NativeConversion<JsString>,
    ) -> Result<ScalarTextStep, RuntimeError> {
        // A throw is transferred before checking the phase, as in the general adapter.
        let string = match result {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => {
                return self.complete(state, poisoned, Completion::Throw(value));
            }
        };
        match self.0.phase {
            Phase::Source => self.0.string = string,
            Phase::Chunk => match self.0.string.try_concat(&string).map_err(Error::from) {
                Ok(string) => self.0.string = string,
                Err(error) => return self.fail(state, poisoned, error.into()),
            },
            _ => {
                return self.fail(
                    state,
                    poisoned,
                    RuntimeError::Invariant("String scalar string phase mismatch"),
                );
            }
        }
        match self.0.kind {
            ScalarTextKind::WellFormed(kind) => {
                let value = match kind {
                    StringWellFormedKind::IsWellFormed => {
                        JsValue::Bool(self.0.string.is_well_formed())
                    }
                    StringWellFormedKind::ToWellFormed => {
                        match state.heap.allocate_string(self.0.string.to_well_formed()) {
                            Ok(id) => JsValue::String(id),
                            Err(error) => return self.fail(state, poisoned, error.into()),
                        }
                    }
                };
                self.complete(state, poisoned, Completion::Return(value))
            }
            ScalarTextKind::Iterator => {
                let string = std::mem::replace(&mut self.0.string, JsString::from_static(""));
                match state.new_string_iterator(poisoned, self.0.realm, string) {
                    Ok(id) => {
                        self.complete(state, poisoned, Completion::Return(JsValue::Object(id)))
                    }
                    Err(error) => self.fail(state, poisoned, error),
                }
            }
            ScalarTextKind::Concat => self.concat(state, poisoned),
            _ => {
                self.0.phase = Phase::Index;
                Ok(ScalarTextStep::Number {
                    value: self.0.arguments.next().unwrap_or(JsValue::Undefined),
                    resume: self,
                })
            }
        }
    }
    fn concat(
        mut self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
    ) -> Result<ScalarTextStep, RuntimeError> {
        loop {
            match self.0.arguments.next() {
                None => {
                    let string = std::mem::replace(&mut self.0.string, JsString::from_static(""));
                    return match state.heap.allocate_string(string) {
                        Ok(id) => {
                            self.complete(state, poisoned, Completion::Return(JsValue::String(id)))
                        }
                        Err(error) => self.fail(state, poisoned, error.into()),
                    };
                }
                Some(JsValue::String(id)) => {
                    let chunk = state.heap.string(id).cloned();
                    // The raw chunk owner is retired before concatenation, even
                    // when payload lookup failed; existing chunks remain ropes.
                    if let Err(error) = state.release_owned_jsvalue(poisoned, JsValue::String(id)) {
                        return self.fail(state, poisoned, error);
                    }
                    let chunk = match chunk {
                        Ok(chunk) => chunk,
                        Err(error) => return self.fail(state, poisoned, error.into()),
                    };
                    match self.0.string.try_concat(&chunk).map_err(Error::from) {
                        Ok(string) => self.0.string = string,
                        Err(error) => return self.fail(state, poisoned, error.into()),
                    }
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
        let _unwind = runtime.unwind_guard();
        let result = self.number_in_state(
            &mut runtime.0.state.borrow_mut(),
            &runtime.0.poisoned,
            result,
        );
        if result.is_ok() {
            runtime.check_poison()?;
        }
        result
    }
    pub(crate) fn number_in_state(
        self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        result: NativeConversion<f64>,
    ) -> Result<ScalarTextStep, RuntimeError> {
        // Number validates Index before considering a supplied throw. This is
        // deliberately different from the string reply's phase rule.
        if !matches!(self.0.phase, Phase::Index) {
            if let NativeConversion::Throw(value) = result {
                let _ = state.release_owned_jsvalue(poisoned, value);
            }
            return self.fail(
                state,
                poisoned,
                RuntimeError::Invariant("String scalar index phase mismatch"),
            );
        }
        match result {
            NativeConversion::Throw(value) => {
                self.complete(state, poisoned, Completion::Throw(value))
            }
            NativeConversion::Value(number) => {
                match finish_index(state, self.kind, &self.string, number) {
                    Ok(completion) => self.complete(state, poisoned, completion),
                    Err(error) => self.fail(state, poisoned, error),
                }
            }
        }
    }
    fn complete(
        self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        completion: Completion,
    ) -> Result<ScalarTextStep, RuntimeError> {
        let (value, thrown) = match completion {
            Completion::Return(value) => (value, false),
            Completion::Throw(value) => (value, true),
        };
        let mut output = OwnedValueGuard::new(state, poisoned, value);
        let (state, output) = output.parts();
        self.retire_in_state(state, poisoned)?;
        let value = output.take().expect("ScalarText completion owner");
        Ok(ScalarTextStep::Complete(if thrown {
            Completion::Throw(value)
        } else {
            Completion::Return(value)
        }))
    }
    fn fail(
        self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        error: RuntimeError,
    ) -> Result<ScalarTextStep, RuntimeError> {
        self.retire_in_state(state, poisoned)?;
        Err(error)
    }
    pub(crate) fn retire_in_state(
        mut self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
    ) -> Result<(), RuntimeError> {
        if poisoned.get() {
            return Ok(());
        }
        self.0
            .retire_with(&mut |value| state.release_owned_jsvalue(poisoned, value))
    }
    pub(crate) fn retire_at_boundary(mut self, runtime: &Runtime) -> Result<(), RuntimeError> {
        self.0.retire_with(&mut |value| {
            if runtime.skip_cleanup() {
                return Err(RuntimeError::Poisoned);
            }
            runtime.release_jsvalue(value)?;
            if runtime.is_poisoned() {
                Err(RuntimeError::Poisoned)
            } else {
                Ok(())
            }
        })
    }
    pub(crate) fn release_owned(self, runtime: &Runtime) {
        // Legacy teardown already owns its operation boundary. Runtime's
        // release coordinator handles an unavailable State borrow here.
        let _ = self.retire_at_boundary(runtime);
    }
}
impl ScalarTextResumeState {
    fn retire_with(
        &mut self,
        release: &mut impl FnMut(JsValue) -> Result<(), RuntimeError>,
    ) -> Result<(), RuntimeError> {
        for value in self.arguments.by_ref() {
            release(value)?;
        }
        Ok(())
    }
}

/// Borrow primitive operands while the native activation still owns them.
/// Objects take the existing ordered conversion path; no JavaScript can run
/// between these primitive conversions, so no argument snapshot is required.
fn complete_primitive(
    state: &mut RuntimeState,
    poisoned: &Cell<bool>,
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
    let mut string = match state.string_from_primitive_jsvalue(poisoned, realm, receiver)? {
        NativeConversion::Value(value) => value,
        NativeConversion::Throw(value) => return Ok(Some(Completion::Throw(value))),
    };
    let value = match kind {
        ScalarTextKind::CharAt(_) | ScalarTextKind::CharCodeAt | ScalarTextKind::CodePointAt => {
            let number = match state.number_from_primitive_jsvalue(poisoned, realm, first)? {
                NativeConversion::Value(value) => value,
                NativeConversion::Throw(value) => return Ok(Some(Completion::Throw(value))),
            };
            return finish_index(state, kind, &string, number).map(Some);
        }
        ScalarTextKind::Concat => {
            for argument in inputs {
                let chunk = if let JsValue::String(id) = argument {
                    state.heap.string(*id)?.clone()
                } else {
                    match state.string_from_primitive_jsvalue(poisoned, realm, argument)? {
                        NativeConversion::Value(value) => value,
                        NativeConversion::Throw(value) => {
                            return Ok(Some(Completion::Throw(value)));
                        }
                    }
                };
                string = string.try_concat(&chunk).map_err(Error::from)?;
            }
            JsValue::String(state.heap.allocate_string(string)?)
        }
        ScalarTextKind::WellFormed(StringWellFormedKind::IsWellFormed) => {
            JsValue::Bool(string.is_well_formed())
        }
        ScalarTextKind::WellFormed(StringWellFormedKind::ToWellFormed) => {
            JsValue::String(state.heap.allocate_string(string.to_well_formed())?)
        }
        ScalarTextKind::Iterator => {
            JsValue::Object(state.new_string_iterator(poisoned, realm, string)?)
        }
    };
    Ok(Some(Completion::Return(value)))
}

/// The callback-free scalar kernel is shared by local and resumed execution.
fn finish_index(
    state: &mut RuntimeState,
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
                    StringCharAtKind::At => JsValue::Undefined,
                    StringCharAtKind::CharAt => {
                        JsValue::String(state.heap.allocate_string(JsString::from_static(""))?)
                    }
                }
            } else {
                JsValue::String(state.heap.allocate_string(
                    JsString::from_code_unit(string.code_unit_at(index as usize).ok_or(
                        RuntimeError::Invariant("validated String index missing code unit"),
                    )?),
                )?)
            }
        }
        ScalarTextKind::CharCodeAt => usize::try_from(index)
            .ok()
            .and_then(|index| string.code_unit_at(index))
            .map_or(JsValue::Float(f64::NAN), |unit| {
                JsValue::Int(i32::from(unit))
            }),
        ScalarTextKind::CodePointAt => usize::try_from(index)
            .ok()
            .and_then(|index| string.code_point_at(index))
            .map_or(JsValue::Undefined, |point| JsValue::Int(point as i32)),
        _ => return Err(RuntimeError::Invariant("String scalar index kind mismatch")),
    };
    Ok(Completion::Return(value))
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
                match runtime.native_to_js_string_jsvalue(realm, value) {
                    Ok(result) => resume.string(runtime, result)?,
                    Err(error) => {
                        resume.release_owned(runtime);
                        return Err(error);
                    }
                }
            }
            ScalarTextStep::Number { value, resume } => {
                match runtime.native_to_number_jsvalue(realm, value) {
                    Ok(result) => resume.number(runtime, result)?,
                    Err(error) => {
                        resume.release_owned(runtime);
                        return Err(error);
                    }
                }
            }
        };
    }
}

// S11 all-domain protocol bound; inline completion stays allocation-free.
const _: () = assert!(std::mem::size_of::<ScalarTextStep>() <= 64);

#[cfg(test)]
mod local_completion_tests {
    use super::*;
    use crate::engine::value::Value;

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

#[cfg(test)]
mod state_tests;
