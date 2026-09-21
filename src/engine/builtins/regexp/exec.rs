//! Builtin and abstract RegExp execution.

use crate::engine::api::error::NativeErrorKind;
use crate::engine::api::runtime::Runtime;
use crate::engine::api::runtime_error::RuntimeError;
use crate::engine::builtins::native::RegExpNativeKind;
use crate::engine::heap::ContextId;

use crate::engine::object::{ObjectRef, PropertyKey};
use crate::engine::value::conversion::NativeConversion;
use crate::engine::value::{JsString, JsValue, Value};
use crate::engine::vm::{Completion, ToPrimitiveHint};

use crate::engine::vm::call::{DirectCallTarget, NativeArguments, NativeInvocation};
use crate::regexp::{
    ExecError, RegExpFlags, execute_latin1_with_interrupt, execute_with_interrupt,
};

impl Runtime {
    pub(crate) fn call_regexp_exec_native(
        &self,
        realm: ContextId,
        kind: RegExpNativeKind,
        invocation: NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        self.dispatch_borrowed_invocation(invocation, |invocation| {
            finish(
                self,
                realm,
                RegExpExecStep::start(self, realm, kind, invocation, arguments)?,
            )
        })
    }
    pub(crate) fn regexp_exec_abstract(
        &self,
        realm: ContextId,
        regexp: JsValue,
        input: JsValue,
    ) -> Result<Completion, RuntimeError> {
        finish(
            self,
            realm,
            RegExpExecStep::abstract_exec(self, realm, regexp, input)?,
        )
    }
    fn finish_builtin_regexp_exec(
        &self,
        realm: ContextId,
        object: &ObjectRef,
        input: JsString,
        input_value: &JsValue,
        last_index: u64,
    ) -> Result<Completion, RuntimeError> {
        let this_value = &JsValue::Object(object.object_id());
        // QuickJS keeps the branded RegExp identity across both coercions, but
        // reads `re->bytecode` only afterwards. Either conversion may call the
        // legacy `compile()` method, so snapshot the current program and flags
        // only after those observable calls have completed.
        let current = self
            .genuine_regexp_jsvalue(this_value)?
            .ok_or(RuntimeError::Invariant(
                "branded RegExp lost its compiled payload during exec coercion",
            ))?;
        let program = current.program;
        let flags = program.flags();
        let updates_last_index =
            flags.contains(RegExpFlags::GLOBAL) || flags.contains(RegExpFlags::STICKY);
        let start = if updates_last_index { last_index } else { 0 };
        let flat = input.linearize();
        let matched = if start > flat.len() as u64 {
            None
        } else {
            let start = usize::try_from(start).expect("RegExp start bounded by String length");
            let execution = if let Some(units) = flat.flat_latin1() {
                execute_latin1_with_interrupt(program.as_ref(), units, start, || false)
            } else {
                execute_with_interrupt(
                    program.as_ref(),
                    flat.flat_utf16().expect("linearized input"),
                    start,
                    || false,
                )
            };
            match execution {
                Ok(value) => value,
                Err(ExecError::OutOfMemory) => {
                    return Ok(Completion::Throw(self.new_native_error_jsvalue(
                        realm,
                        NativeErrorKind::Internal,
                        "out of memory in regexp execution",
                    )?));
                }
                Err(ExecError::Interrupted) => {
                    return Ok(Completion::Throw(self.new_native_error_jsvalue(
                        realm,
                        NativeErrorKind::Internal,
                        "interrupted",
                    )?));
                }
                Err(ExecError::InvalidProgram(_)) => {
                    return Err(RuntimeError::Invariant(
                        "compiled RegExp program failed executor validation",
                    ));
                }
                Err(ExecError::StartOutOfBounds { .. }) => {
                    return Err(RuntimeError::Invariant(
                        "bounded RegExp start was rejected by executor",
                    ));
                }
            }
        };

        let Some(matched) = matched else {
            if updates_last_index
                && let Some(exception) = self.set_regexp_last_index(realm, object, 0)?
            {
                return Ok(Completion::Throw(self.into_jsvalue(exception)?));
            }
            return Ok(Completion::Return(JsValue::Null));
        };

        let complete = matched.capture(0).ok_or(RuntimeError::Invariant(
            "successful RegExp execution omitted capture zero",
        ))?;
        if updates_last_index {
            let end = i32::try_from(complete.end).map_err(|_| {
                RuntimeError::Invariant("RegExp match end exceeded signed String range")
            })?;
            // This write happens before any result/indices allocation.
            if let Some(exception) = self.set_regexp_last_index(realm, object, end)? {
                return Ok(Completion::Throw(self.into_jsvalue(exception)?));
            }
        }

        Ok(Completion::Return(self.build_regexp_result(
            realm,
            input,
            input_value,
            program,
            matched,
        )?))
    }

    fn regexp_last_index_value(&self, object: &ObjectRef) -> Result<JsValue, RuntimeError> {
        let key = self.pinned_property_key(crate::engine::atom::pinned::PinnedAtom::LastIndex)?;
        let descriptor =
            self.get_own_property_owned(object, &key)?
                .ok_or(RuntimeError::Invariant(
                    "genuine RegExp object had no lastIndex property",
                ))?;
        let crate::engine::object::property::CompletePropertyDescriptor::Data { value, .. } =
            descriptor.record()
        else {
            return Err(RuntimeError::Invariant(
                "RegExp lastIndex became an accessor",
            ));
        };
        self.dup_jsvalue(
            &JsValue::from_raw(value.clone()).ok_or(RuntimeError::Invariant(
                "RegExp lastIndex has an internal sentinel",
            ))?,
        )
    }

    pub(crate) fn set_regexp_last_index(
        &self,
        realm: ContextId,
        object: &ObjectRef,
        value: i32,
    ) -> Result<Option<Value>, RuntimeError> {
        let key = self.pinned_property_key(crate::engine::atom::pinned::PinnedAtom::LastIndex)?;
        self.set_property_or_throw(realm, object, &key, Value::Int(value))
    }
}

/// Shared RegExpExec owns the selected exec method and branded fallback state.
pub(crate) enum RegExpExecStep {
    Complete(Completion),
    Read { resume: RegExpExecResume },
    Primitive { resume: RegExpExecResume },
    Call { resume: RegExpExecResume },
}
pub(crate) struct RegExpExecResume(Box<RegExpExecResumeState>);
impl std::ops::Deref for RegExpExecResume {
    type Target = RegExpExecResumeState;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for RegExpExecResume {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
const _: () = assert!(std::mem::size_of::<RegExpExecResume>() <= 8);
pub(crate) struct RegExpExecResumeState {
    step_pending: RegExpExecStepPending,
    realm: ContextId,
    regexp: JsValue,
    input: JsValue,
    string_input: JsValue,
    converted: JsValue,
    test: bool,
    phase: ExecPhase,
}
impl Drop for RegExpExecResumeState {
    fn drop(&mut self) {
        for value in [
            &mut self.regexp,
            &mut self.input,
            &mut self.string_input,
            &mut self.converted,
        ] {
            let _ = self
                .step_pending
                .runtime
                .release_jsvalue(std::mem::replace(value, JsValue::Undefined));
        }
    }
}
enum ExecPhase {
    Method,
    Called,
    Input,
    LastIndex(JsString),
}
impl RegExpExecStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        kind: RegExpNativeKind,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        let NativeInvocation::Call { this_value } = invocation else {
            return Err(RuntimeError::Invariant(
                "RegExp exec/test did not receive a generic invocation",
            ));
        };
        let mut resume = RegExpExecResume(Box::new(RegExpExecResumeState {
            step_pending: RegExpExecStepPending::new(runtime),
            realm,
            regexp: JsValue::Undefined,
            input: JsValue::Undefined,
            string_input: JsValue::Undefined,
            converted: JsValue::Undefined,
            test: kind == RegExpNativeKind::Test,
            phase: ExecPhase::Input,
        }));
        resume.input = runtime.dup_jsvalue(arguments.readable.first().ok_or(
            RuntimeError::Invariant("RegExp exec/test input argv was not padded"),
        )?)?;
        resume.regexp = runtime.dup_jsvalue(this_value)?;
        match kind {
            RegExpNativeKind::Exec => resume.builtin(runtime),
            RegExpNativeKind::Test => resume.abstract_read(runtime),
            _ => Err(RuntimeError::Invariant(
                "non-exec RegExp selector reached exec dispatch",
            )),
        }
    }
    pub(crate) fn abstract_exec(
        runtime: &Runtime,
        realm: ContextId,
        regexp: JsValue,
        input: JsValue,
    ) -> Result<Self, RuntimeError> {
        Self::abstract_start(runtime, realm, regexp, input, false)
    }
    fn abstract_start(
        runtime: &Runtime,
        realm: ContextId,
        regexp: JsValue,
        input: JsValue,
        test: bool,
    ) -> Result<Self, RuntimeError> {
        RegExpExecResume(Box::new(RegExpExecResumeState {
            step_pending: RegExpExecStepPending::new(runtime),
            realm,
            regexp,
            input,
            string_input: JsValue::Undefined,
            converted: JsValue::Undefined,
            test,
            phase: ExecPhase::Method,
        }))
        .abstract_read(runtime)
    }
}
impl RegExpExecResume {
    fn abstract_read(mut self, runtime: &Runtime) -> Result<RegExpExecStep, RuntimeError> {
        let key = runtime.pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Exec)?;
        if matches!(self.regexp, JsValue::Null | JsValue::Undefined) {
            let base = if matches!(self.regexp, JsValue::Null) {
                "null"
            } else {
                "undefined"
            };
            return Ok(RegExpExecStep::Complete(Completion::Throw(
                runtime.new_native_error_jsvalue(
                    self.realm,
                    NativeErrorKind::Type,
                    &format!("cannot read property 'exec' of {base}"),
                )?,
            )));
        }
        self.phase = ExecPhase::Method;
        Ok(RegExpExecStep::make_read(
            runtime.dup_jsvalue(&self.regexp)?,
            key,
            self,
        ))
    }
    fn complete(&mut self, result: Completion) -> RegExpExecStep {
        RegExpExecStep::Complete(match result {
            Completion::Return(value) if self.0.test => {
                // `test` only reports nullness; the freshly built result object
                // would otherwise be abandoned without an owner.
                let is_null = matches!(value, JsValue::Null);
                if !is_null {
                    let _ = self.0.step_pending.runtime.release_jsvalue(value);
                }
                Completion::Return(JsValue::Bool(!is_null))
            }
            result => result,
        })
    }
    fn builtin(mut self, runtime: &Runtime) -> Result<RegExpExecStep, RuntimeError> {
        if !matches!(&self.0.regexp, JsValue::Object(_))
            || runtime.genuine_regexp_jsvalue(&self.0.regexp)?.is_none()
        {
            return Ok(
                self.complete(Completion::Throw(runtime.new_native_error_jsvalue(
                    self.0.realm,
                    NativeErrorKind::Type,
                    "RegExp object expected",
                )?)),
            );
        }
        let input = runtime.dup_jsvalue(&self.0.input)?;
        let resume = {
            let updated_0 = ExecPhase::Input;
            self.0.phase = updated_0;
            self
        };
        if matches!(input, JsValue::Object(_)) {
            Ok(RegExpExecStep::make_primitive(
                input,
                ToPrimitiveHint::String,
                resume,
            ))
        } else {
            resume.resume(runtime, Completion::Return(input))
        }
    }
    pub(crate) fn resume(
        mut self,
        runtime: &Runtime,
        result: Completion,
    ) -> Result<RegExpExecStep, RuntimeError> {
        let value = match result {
            Completion::Return(value) => value,
            Completion::Throw(value) => return Ok(self.complete(Completion::Throw(value))),
        };
        let previous = std::mem::replace(&mut self.converted, value);
        runtime.release_jsvalue(previous)?;
        match std::mem::replace(&mut self.0.phase, ExecPhase::Called) {
            ExecPhase::Method => {
                let callable = match &self.converted {
                    JsValue::Object(object) => runtime.as_callable_object(*object)?,
                    _ => None,
                };
                let converted = std::mem::replace(&mut self.converted, JsValue::Undefined);
                runtime.release_jsvalue(converted)?;
                let Some(callable) = callable else {
                    return self.builtin(runtime);
                };
                let mut arguments = Vec::new();
                if arguments.try_reserve_exact(1).is_err() {
                    return Ok(self.complete(Completion::Throw(
                        runtime.new_native_error_jsvalue(
                            self.0.realm,
                            NativeErrorKind::Internal,
                            "out of memory",
                        )?,
                    )));
                }
                self.step_pending.arguments = Some(arguments);
                let argument = runtime.dup_jsvalue(&self.input)?;
                self.step_pending
                    .arguments
                    .as_mut()
                    .expect("exec argv owner")
                    .push(argument);
                let receiver = runtime.dup_jsvalue(&self.regexp)?;
                let arguments = self.step_pending.arguments.take().expect("exec argv owner");
                Ok(RegExpExecStep::make_call(
                    DirectCallTarget::Callable(callable),
                    receiver,
                    arguments,
                    {
                        let updated_0 = ExecPhase::Called;
                        self.0.phase = updated_0;
                        self
                    },
                ))
            }
            ExecPhase::Called => {
                if matches!(self.converted, JsValue::Object(_) | JsValue::Null) {
                    let value = std::mem::replace(&mut self.converted, JsValue::Undefined);
                    Ok(self.complete(Completion::Return(value)))
                } else {
                    Ok(
                        self.complete(Completion::Throw(runtime.new_native_error_jsvalue(
                            self.0.realm,
                            NativeErrorKind::Type,
                            "RegExp exec method must return an object or null",
                        )?)),
                    )
                }
            }
            ExecPhase::Input => {
                if matches!(self.converted, JsValue::Object(_)) {
                    return Err(RuntimeError::Invariant(
                        "RegExp input conversion returned an object",
                    ));
                }
                let input = match runtime
                    .string_from_primitive_jsvalue(self.0.realm, &self.converted)?
                {
                    NativeConversion::Value(input) => input,
                    NativeConversion::Throw(value) => {
                        return Ok(self.complete(Completion::Throw(runtime.into_jsvalue(value)?)));
                    }
                };
                self.string_input = if matches!(self.converted, JsValue::String(_)) {
                    std::mem::replace(&mut self.converted, JsValue::Undefined)
                } else {
                    runtime.into_jsvalue(Value::String(input.clone()))?
                };
                let JsValue::Object(object) = &self.0.regexp else {
                    return Err(RuntimeError::Invariant(
                        "RegExp input conversion lost its branded receiver",
                    ));
                };
                let object = ObjectRef::from_borrowed_handle(runtime.clone(), *object)?;
                let value = runtime.regexp_last_index_value(&object)?;
                let resume = {
                    let updated_0 = ExecPhase::LastIndex(input);
                    self.0.phase = updated_0;
                    self
                };
                if matches!(value, JsValue::Object(_)) {
                    Ok(RegExpExecStep::make_primitive(
                        value,
                        ToPrimitiveHint::Number,
                        resume,
                    ))
                } else {
                    // No callback: the branded receiver and input stay owned by
                    // this domain; the generic conversion/Query is never built.
                    resume.resume(runtime, Completion::Return(value))
                }
            }
            ExecPhase::LastIndex(input) => {
                if matches!(self.converted, JsValue::Object(_)) {
                    return Err(RuntimeError::Invariant(
                        "RegExp lastIndex conversion returned an object",
                    ));
                }
                let last_index =
                    match runtime.number_from_primitive_jsvalue(self.0.realm, &self.converted)? {
                        NativeConversion::Value(index) => Runtime::length_from_number(index),
                        NativeConversion::Throw(value) => {
                            return Ok(RegExpExecStep::Complete(Completion::Throw(
                                runtime.into_jsvalue(value)?,
                            )));
                        }
                    };
                let JsValue::Object(object) = &self.0.regexp else {
                    return Err(RuntimeError::Invariant(
                        "RegExp lastIndex conversion lost its branded receiver",
                    ));
                };
                let object = ObjectRef::from_borrowed_handle(runtime.clone(), *object)?;
                let result = runtime.finish_builtin_regexp_exec(
                    self.0.realm,
                    &object,
                    input,
                    &self.string_input,
                    last_index,
                )?;
                Ok({
                    let updated_0 = ExecPhase::Called;
                    self.0.phase = updated_0;
                    self
                }
                .complete(result))
            }
        }
    }
}
fn finish(
    runtime: &Runtime,
    realm: ContextId,
    mut step: RegExpExecStep,
) -> Result<Completion, RuntimeError> {
    loop {
        step = match step {
            RegExpExecStep::Complete(result) => return Ok(result),
            RegExpExecStep::Read { mut resume } => {
                let receiver = resume.take_read_receiver();
                let key = resume.take_read_key();
                resume.resume(
                    runtime,
                    runtime.get_value_property_in_realm_jsvalue(realm, receiver, &key)?,
                )?
            }
            RegExpExecStep::Primitive { mut resume } => {
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
            RegExpExecStep::Call { mut resume } => {
                let target = resume.take_call_target();
                let receiver = resume.take_call_receiver();
                let arguments = resume.take_call_arguments();
                {
                    let DirectCallTarget::Callable(callable) = target else {
                        runtime.release_jsvalue(receiver)?;
                        for value in arguments {
                            runtime.release_jsvalue(value)?;
                        }
                        return Err(RuntimeError::Invariant(
                            "RegExp exec requested an invalid call target",
                        ));
                    };
                    resume.resume(
                        runtime,
                        runtime.call_internal_jsvalue(realm, &callable, receiver, arguments)?,
                    )?
                }
            }
        };
    }
}

pub(crate) struct RegExpExecStepPending {
    runtime: Runtime,
    receiver: Option<JsValue>,
    key: Option<PropertyKey>,
    value: Option<JsValue>,
    hint: Option<ToPrimitiveHint>,
    target: Option<DirectCallTarget>,
    arguments: Option<Vec<JsValue>>,
}
impl RegExpExecStepPending {
    fn new(runtime: &Runtime) -> Self {
        Self {
            runtime: runtime.clone(),
            receiver: None,
            key: None,
            value: None,
            hint: None,
            target: None,
            arguments: None,
        }
    }

    /// Release the internal edges still owned when the request is abandoned
    /// before its step consumed them. Taken fields are empty here.
    fn release_owned(&mut self) {
        for value in [self.receiver.take(), self.value.take()]
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
impl Drop for RegExpExecStepPending {
    fn drop(&mut self) {
        self.release_owned();
    }
}
impl RegExpExecStep {
    pub(crate) fn make_read(
        receiver: JsValue,
        key: PropertyKey,
        mut resume: RegExpExecResume,
    ) -> Self {
        resume.0.step_pending.receiver = Some(receiver);
        resume.0.step_pending.key = Some(key);
        Self::Read { resume }
    }
    pub(crate) fn make_primitive(
        value: JsValue,
        hint: ToPrimitiveHint,
        mut resume: RegExpExecResume,
    ) -> Self {
        resume.0.step_pending.value = Some(value);
        resume.0.step_pending.hint = Some(hint);
        Self::Primitive { resume }
    }
    pub(crate) fn make_call(
        target: DirectCallTarget,
        receiver: JsValue,
        arguments: Vec<JsValue>,
        mut resume: RegExpExecResume,
    ) -> Self {
        resume.0.step_pending.target = Some(target);
        resume.0.step_pending.receiver = Some(receiver);
        resume.0.step_pending.arguments = Some(arguments);
        Self::Call { resume }
    }
}
impl RegExpExecResume {
    pub(crate) fn take_read_receiver(&mut self) -> JsValue {
        self.0
            .step_pending
            .receiver
            .take()
            .expect("RegExpExecStep::Read lost receiver")
    }
    pub(crate) fn take_read_key(&mut self) -> PropertyKey {
        self.0
            .step_pending
            .key
            .take()
            .expect("RegExpExecStep::Read lost key")
    }

    pub(crate) fn take_primitive_value(&mut self) -> JsValue {
        self.0
            .step_pending
            .value
            .take()
            .expect("RegExpExecStep::Primitive lost value")
    }
    pub(crate) fn take_primitive_hint(&mut self) -> ToPrimitiveHint {
        self.0
            .step_pending
            .hint
            .take()
            .expect("RegExpExecStep::Primitive lost hint")
    }

    pub(crate) fn take_call_target(&mut self) -> DirectCallTarget {
        self.0
            .step_pending
            .target
            .take()
            .expect("RegExpExecStep::Call lost target")
    }
    pub(crate) fn take_call_receiver(&mut self) -> JsValue {
        self.0
            .step_pending
            .receiver
            .take()
            .expect("RegExpExecStep::Call lost receiver")
    }
    pub(crate) fn take_call_arguments(&mut self) -> Vec<JsValue> {
        self.0
            .step_pending
            .arguments
            .take()
            .expect("RegExpExecStep::Call lost arguments")
    }
}

const _: () = assert!(std::mem::size_of::<RegExpExecStep>() <= 64);

// S11 all-domain protocol bound; inline completion stays allocation-free.
const _: () = assert!(std::mem::size_of::<RegExpExecStep>() <= 64);

#[cfg(test)]
mod local_exec_tests {
    use super::*;

    #[test]
    fn primitive_regexp_exec_completes_inside_its_domain() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let this_value = runtime
            .unroot_value(&context.eval("/a/g").unwrap())
            .unwrap();
        let invocation = NativeInvocation::Call { this_value };
        let arguments = NativeArguments {
            actual_arg_count: 1,
            readable: vec![
                runtime
                    .unroot_value(&Value::String(JsString::from_static("a")))
                    .unwrap(),
            ],
        };
        let step = RegExpExecStep::start(
            &runtime,
            context.realm,
            RegExpNativeKind::Exec,
            &invocation,
            &arguments,
        )
        .unwrap();
        let RegExpExecStep::Complete(Completion::Return(value)) = step else {
            panic!("primitive RegExp exec did not complete locally");
        };
        assert!(matches!(value, JsValue::Object(_)));
        runtime.release_jsvalue(value).unwrap();
        for value in arguments.readable {
            runtime.release_jsvalue(value).unwrap();
        }
        invocation.release(&runtime).unwrap();
    }

    #[test]
    fn regexp_local_conversion_preserves_reentry_and_live_program() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        assert_eq!(context.eval(r#"(()=>{
            let trace='', re=/a/g;
            const input={toString(){trace+='i';re.lastIndex={valueOf(){trace+='l';return 0}};return 'a'}};
            if(re.exec(input)[0]!=='a'||trace!=='il'||re.lastIndex!==1)return false;
            const marker={};re.lastIndex={valueOf(){throw marker}};
            try{re.exec('a');return false}catch(e){if(e!==marker)return false}
            const frozen=/a/g;Object.defineProperty(frozen,'lastIndex',{writable:false});
            try{frozen.exec('a');return false}catch(e){if(!(e instanceof TypeError))return false}
            return /é/.exec('é')[0]==='é' && /a/.test('a');
        })()"#).unwrap(),Value::Bool(true));
    }
}
