//! One builtin/abstract RegExp exec phase body under the admitted State.
use crate::engine::{
    api::{
        error::{NativeErrorKind, NativeErrorMessage},
        runtime::{Runtime, RuntimeUnwindGuard},
        runtime_error::RuntimeError,
    },
    atom::{Atom, pinned::PinnedAtom},
    builtins::native::RegExpNativeKind,
    heap::{ContextId, ObjectId, runtime::RuntimeState},
    object::{
        ObjectRef, OwnedRead, ReadStep, SetAction, SetProgress,
        own_properties::OwnPropertySelection,
    },
    value::{
        JsString, JsValue, Value,
        conversion::{NumberPrimitiveStep, StringPrimitiveStep},
    },
    vm::{
        Completion, ToPrimitiveHint,
        call::{NativeArguments, NativeInvocation},
    },
};
use crate::regexp::{
    CompiledRegExp, ExecError, RegExpFlags, RegExpMatch, execute_latin1_with_interrupt,
    execute_with_interrupt,
};
use std::{cell::Cell, rc::Rc};

impl Runtime {
    pub(crate) fn call_regexp_exec_native(
        &self,
        realm: ContextId,
        kind: RegExpNativeKind,
        invocation: NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        self.dispatch_borrowed_invocation(invocation, |invocation| {
            let step = RegExpExecStep::start(self, realm, kind, invocation, arguments)?;
            crate::engine::vm::execute_regexp_exec_step(self, realm, step)
                .map_err(RuntimeError::from)
        })
    }
    pub(crate) fn regexp_exec_abstract(
        &self,
        realm: ContextId,
        regexp: JsValue,
        input: JsValue,
    ) -> Result<Completion, RuntimeError> {
        let step = RegExpExecStep::abstract_exec(self, realm, regexp, input)?;
        crate::engine::vm::execute_regexp_exec_step(self, realm, step).map_err(RuntimeError::from)
    }
    // Larger, unmigrated RegExp protocols keep their real strict Set boundary.
    pub(crate) fn set_regexp_last_index(
        &self,
        realm: ContextId,
        object: &ObjectRef,
        value: i32,
    ) -> Result<Option<JsValue>, RuntimeError> {
        let key = self.pinned_property_key(PinnedAtom::LastIndex)?;
        self.set_property_or_throw(realm, object, &key, Value::Int(value))
    }
}

pub(crate) enum RegExpExecStep {
    Complete(Completion),
    CyclePublished(Completion),
    Read {
        resume: RegExpExecResume,
    },
    Primitive {
        resume: RegExpExecResume,
    },
    Call {
        resume: RegExpExecResume,
    },
    Set {
        progress: Box<SetProgress>,
        resume: RegExpExecResume,
    },
}
pub(crate) struct RegExpExecResume(Box<RegExpExecResumeState>);
struct RegExpExecResumeState {
    realm: ContextId,
    regexp: JsValue,
    input: JsValue,
    string_input: JsValue,
    converted: JsValue,
    test: bool,
    phase: ExecPhase,
    published: bool,
    key: Option<Atom>,
    read: Option<ReadStep>,
    receiver: Option<JsValue>,
    value: Option<JsValue>,
    hint: Option<ToPrimitiveHint>,
    callee: Option<ObjectId>,
    arguments: Vec<JsValue>,
}
enum ExecPhase {
    Method,
    Called,
    Input,
    LastIndex(JsString),
    AfterSet {
        input: JsString,
        program: Rc<CompiledRegExp>,
        matched: Option<RegExpMatch>,
    },
}
impl RegExpExecResume {
    fn new(realm: ContextId, test: bool) -> Self {
        Self(Box::new(RegExpExecResumeState {
            realm,
            regexp: JsValue::Undefined,
            input: JsValue::Undefined,
            string_input: JsValue::Undefined,
            converted: JsValue::Undefined,
            test,
            phase: ExecPhase::Input,
            published: false,
            key: None,
            read: None,
            receiver: None,
            value: None,
            hint: None,
            callee: None,
            arguments: Vec::new(),
        }))
    }
    pub(crate) fn take_publication(&mut self) -> bool {
        std::mem::take(&mut self.0.published)
    }
    pub(crate) fn take_read(&mut self) -> (ReadStep, Atom) {
        (
            self.0.read.take().expect("RegExp selected exec read"),
            self.0.exec_atom(),
        )
    }
    pub(crate) fn take_primitive(&mut self) -> (JsValue, ToPrimitiveHint) {
        (
            self.0.value.take().expect("RegExp conversion input"),
            self.0.hint.take().expect("RegExp conversion hint"),
        )
    }
    pub(crate) fn take_call(&mut self) -> (ObjectId, JsValue, Vec<JsValue>) {
        (
            self.0.callee.take().expect("RegExp exec callee"),
            self.0.receiver.take().expect("RegExp exec receiver"),
            std::mem::take(&mut self.0.arguments),
        )
    }
    pub(crate) fn resume_in_state(
        self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        completion: Completion,
    ) -> Result<RegExpExecStep, RuntimeError> {
        let mut owner = RegExpGuard::new(state, poisoned, self);
        let value = match completion {
            Completion::Throw(value) => return owner.complete(Completion::Throw(value), false),
            Completion::Return(value) => value,
        };
        let old = std::mem::replace(&mut owner.owner().converted, value);
        owner.state.release_owned_jsvalue(poisoned, old)?;
        owner.drive()
    }
    pub(crate) fn resume(
        self,
        runtime: &Runtime,
        completion: Completion,
    ) -> Result<RegExpExecStep, RuntimeError> {
        let _unwind = runtime.unwind_guard();
        let result = self.resume_in_state(
            &mut runtime.0.state.borrow_mut(),
            &runtime.0.poisoned,
            completion,
        );
        if runtime.is_poisoned() {
            return Err(RuntimeError::Poisoned);
        }
        result
    }
    pub(crate) fn set_in_state(
        self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        action: SetAction,
    ) -> Result<RegExpExecStep, RuntimeError> {
        let owner = RegExpGuard::new(state, poisoned, self);
        owner.finish_set(action)
    }
    pub(crate) fn set_boundary(
        self,
        runtime: &Runtime,
        action: SetAction,
    ) -> Result<RegExpExecStep, RuntimeError> {
        let _unwind = runtime.unwind_guard();
        let result = self.set_in_state(
            &mut runtime.0.state.borrow_mut(),
            &runtime.0.poisoned,
            action,
        );
        if runtime.is_poisoned() {
            return Err(RuntimeError::Poisoned);
        }
        result
    }
    pub(crate) fn retire_in_state(
        mut self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
    ) -> Result<(), RuntimeError> {
        self.0
            .retire_with(&mut |value| state.release_owned_jsvalue(poisoned, value))?;
        if let Some(ReadStep::Ready(read) | ReadStep::CyclePublished(read)) = self.0.read.take() {
            read.retire(state, poisoned)?;
        }
        Ok(())
    }
    pub(crate) fn retire_at_boundary(mut self, runtime: &Runtime) -> Result<(), RuntimeError> {
        self.0.retire_with(&mut |value| {
            if runtime.skip_cleanup() {
                return Err(RuntimeError::Poisoned);
            }
            runtime.release_jsvalue(value)?;
            runtime.check_poison()
        })?;
        if let Some(ReadStep::Ready(read) | ReadStep::CyclePublished(read)) = self.0.read.take() {
            read.retire_at_boundary(runtime)?;
        }
        Ok(())
    }
}
impl RegExpExecResumeState {
    // The permanent atom is supplied at construction by the canonical body.
    fn exec_atom(&self) -> Atom {
        self.key.expect("RegExp pinned exec key")
    }
    fn retire_with(
        &mut self,
        release: &mut impl FnMut(JsValue) -> Result<(), RuntimeError>,
    ) -> Result<(), RuntimeError> {
        for value in [
            &mut self.regexp,
            &mut self.input,
            &mut self.string_input,
            &mut self.converted,
        ] {
            release(std::mem::replace(value, JsValue::Undefined))?;
        }
        if let Some(value) = self.receiver.take() {
            release(value)?;
        }
        if let Some(value) = self.value.take() {
            release(value)?;
        }
        for value in &mut self.arguments {
            release(std::mem::replace(value, JsValue::Undefined))?;
        }
        self.arguments.clear();
        if let Some(callee) = self.callee.take() {
            release(JsValue::Object(callee))?;
        }
        Ok(())
    }
}
impl RegExpExecStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        kind: RegExpNativeKind,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        let _unwind = runtime.unwind_guard();
        runtime.check_poison()?;
        let result = Self::start_in_state(
            &mut runtime.0.state.borrow_mut(),
            &runtime.0.poisoned,
            realm,
            kind,
            invocation,
            arguments,
        );
        if runtime.is_poisoned() {
            return Err(RuntimeError::Poisoned);
        }
        result
    }
    pub(crate) fn start_in_state(
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
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
        let resume = RegExpExecResume::new(realm, kind == RegExpNativeKind::Test);
        let mut owner = RegExpGuard::new(state, poisoned, resume);
        let key = owner.state.pinned_atoms.get(PinnedAtom::Exec);
        owner.owner().key = Some(key);
        let input = owner.state.dup_jsvalue(arguments.readable.first().ok_or(
            RuntimeError::Invariant("RegExp exec/test input argv was not padded"),
        )?)?;
        owner.owner().input = input;
        let regexp = owner.state.dup_jsvalue(this_value)?;
        owner.owner().regexp = regexp;
        match kind {
            RegExpNativeKind::Exec => owner.builtin(),
            RegExpNativeKind::Test => owner.abstract_read(),
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
        let _unwind = runtime.unwind_guard();
        let result = Self::abstract_exec_in_state(
            &mut runtime.0.state.borrow_mut(),
            &runtime.0.poisoned,
            realm,
            regexp,
            input,
        );
        if runtime.is_poisoned() {
            return Err(RuntimeError::Poisoned);
        }
        result
    }
    pub(crate) fn abstract_exec_in_state(
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        realm: ContextId,
        regexp: JsValue,
        input: JsValue,
    ) -> Result<Self, RuntimeError> {
        let mut resume = RegExpExecResume::new(realm, false);
        resume.0.regexp = regexp;
        resume.0.input = input;
        resume.0.key = Some(state.pinned_atoms.get(PinnedAtom::Exec));
        RegExpGuard::new(state, poisoned, resume).abstract_read()
    }
    pub(crate) fn retire_in_state(
        self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
    ) -> Result<(), RuntimeError> {
        match self {
            Self::Complete(Completion::Return(value) | Completion::Throw(value))
            | Self::CyclePublished(Completion::Return(value) | Completion::Throw(value)) => {
                state.release_owned_jsvalue(poisoned, value)
            }
            Self::Read { resume } | Self::Primitive { resume } | Self::Call { resume } => {
                resume.retire_in_state(state, poisoned)
            }
            Self::Set { progress, resume } => {
                (*progress).retire_in_state(state, poisoned)?;
                resume.retire_in_state(state, poisoned)
            }
        }
    }
    pub(crate) fn retire_at_boundary(self, runtime: &Runtime) -> Result<(), RuntimeError> {
        match self {
            Self::Complete(Completion::Return(value) | Completion::Throw(value))
            | Self::CyclePublished(Completion::Return(value) | Completion::Throw(value)) => {
                runtime.release_jsvalue(value)?;
                runtime.check_poison()
            }
            Self::Read { resume } | Self::Primitive { resume } | Self::Call { resume } => {
                resume.retire_at_boundary(runtime)
            }
            Self::Set { progress, resume } => {
                (*progress).retire_at_boundary(runtime)?;
                resume.retire_at_boundary(runtime)
            }
        }
    }
}
struct RegExpGuard<'a> {
    state: &'a mut RuntimeState,
    poisoned: &'a Cell<bool>,
    resume: Option<RegExpExecResume>,
    output: Option<RegExpExecStep>,
}
impl<'a> RegExpGuard<'a> {
    fn new(
        state: &'a mut RuntimeState,
        poisoned: &'a Cell<bool>,
        resume: RegExpExecResume,
    ) -> Self {
        Self {
            state,
            poisoned,
            resume: Some(resume),
            output: None,
        }
    }
    fn owner(&mut self) -> &mut RegExpExecResumeState {
        &mut self.resume.as_mut().expect("RegExp owner").0
    }
    fn take(&mut self) -> RegExpExecResume {
        self.resume.take().expect("RegExp owner")
    }
    fn complete(
        mut self,
        result: Completion,
        published: bool,
    ) -> Result<RegExpExecStep, RuntimeError> {
        let published = published || self.owner().published;
        let result = if self.owner().test {
            match result {
                Completion::Return(value) => {
                    let is_null = matches!(value, JsValue::Null);
                    self.state.release_owned_jsvalue(self.poisoned, value)?;
                    Completion::Return(JsValue::Bool(!is_null))
                }
                result => result,
            }
        } else {
            result
        };
        self.output = Some(if published {
            RegExpExecStep::CyclePublished(result)
        } else {
            RegExpExecStep::Complete(result)
        });
        let resume = self.take();
        resume.retire_in_state(self.state, self.poisoned)?;
        Ok(self.output.take().expect("RegExp completion"))
    }
    fn fail(self, kind: NativeErrorKind, message: &str) -> Result<RegExpExecStep, RuntimeError> {
        let realm = self.resume.as_ref().expect("RegExp owner").0.realm;
        let error = self.state.new_native_error_from_message(
            self.poisoned,
            realm,
            kind,
            NativeErrorMessage::from_utf8(message),
        )?;
        self.complete(Completion::Throw(JsValue::Object(error)), true)
    }
    fn abstract_read(mut self) -> Result<RegExpExecStep, RuntimeError> {
        if matches!(self.owner().regexp, JsValue::Null | JsValue::Undefined) {
            let base = if matches!(self.owner().regexp, JsValue::Null) {
                "null"
            } else {
                "undefined"
            };
            return self.fail(
                NativeErrorKind::Type,
                &format!("cannot read property 'exec' of {base}"),
            );
        }
        let resume = self.resume.as_mut().expect("RegExp owner");
        resume.0.receiver = Some(self.state.dup_jsvalue(&resume.0.regexp)?);
        resume.0.phase = ExecPhase::Method;
        resume.0.read = Some(self.state.prepare_value_read_without_native_hint(
            self.poisoned,
            resume.0.realm,
            resume.0.receiver.as_ref().expect("exec read receiver"),
            resume.0.exec_atom(),
        )?);
        // The selected output owns its edges before the read input retires.
        self.state.release_owned_jsvalue(
            self.poisoned,
            resume.0.receiver.take().expect("exec read receiver"),
        )?;
        let value = match resume.0.read.as_mut().expect("exec read") {
            ReadStep::Ready(OwnedRead::Complete(value)) => {
                Some(value.take().unwrap_or(JsValue::Undefined))
            }
            ReadStep::CyclePublished(OwnedRead::Complete(value)) => {
                resume.0.published = true;
                Some(value.take().unwrap_or(JsValue::Undefined))
            }
            _ => None,
        };
        if let Some(value) = value {
            resume.0.read = None;
            resume.0.converted = value;
            self.drive()
        } else {
            Ok(RegExpExecStep::Read {
                resume: self.take(),
            })
        }
    }
    fn builtin(mut self) -> Result<RegExpExecStep, RuntimeError> {
        let resume = self.resume.as_mut().expect("RegExp owner");
        if !matches!(resume.0.regexp, JsValue::Object(_))
            || self
                .state
                .genuine_regexp_jsvalue(&resume.0.regexp)?
                .is_none()
        {
            return self.fail(NativeErrorKind::Type, "RegExp object expected");
        }
        resume.0.value = Some(self.state.dup_jsvalue(&resume.0.input)?);
        resume.0.phase = ExecPhase::Input;
        if matches!(resume.0.value, Some(JsValue::Object(_))) {
            resume.0.hint = Some(ToPrimitiveHint::String);
            Ok(RegExpExecStep::Primitive {
                resume: self.take(),
            })
        } else {
            resume.0.converted = resume.0.value.take().expect("RegExp input");
            self.drive()
        }
    }
    fn drive(mut self) -> Result<RegExpExecStep, RuntimeError> {
        match std::mem::replace(&mut self.owner().phase, ExecPhase::Called) {
            ExecPhase::Method => {
                let resume = self.resume.as_mut().expect("RegExp owner");
                if let JsValue::Object(object) = resume.0.converted {
                    if self.state.object_id_has_call_capability(object)? {
                        self.state.heap.retain_object(object)?;
                        resume.0.callee = Some(object);
                    }
                }
                self.state.release_owned_jsvalue(
                    self.poisoned,
                    std::mem::replace(&mut resume.0.converted, JsValue::Undefined),
                )?;
                if resume.0.callee.is_none() {
                    return self.builtin();
                }
                if resume.0.arguments.try_reserve_exact(1).is_err() {
                    return self.fail(NativeErrorKind::Internal, "out of memory");
                }
                resume
                    .0
                    .arguments
                    .push(self.state.dup_jsvalue(&resume.0.input)?);
                resume.0.receiver = Some(self.state.dup_jsvalue(&resume.0.regexp)?);
                Ok(RegExpExecStep::Call {
                    resume: self.take(),
                })
            }
            ExecPhase::Called => {
                if matches!(self.owner().converted, JsValue::Object(_) | JsValue::Null) {
                    let result = std::mem::replace(&mut self.owner().converted, JsValue::Undefined);
                    self.complete(Completion::Return(result), false)
                } else {
                    self.fail(
                        NativeErrorKind::Type,
                        "RegExp exec method must return an object or null",
                    )
                }
            }
            ExecPhase::Input => {
                let resume = self.resume.as_mut().expect("RegExp owner");
                if matches!(resume.0.converted, JsValue::Object(_)) {
                    return Err(RuntimeError::Invariant(
                        "RegExp input conversion returned an object",
                    ));
                }
                let input = match self.state.string_from_primitive_jsvalue_with_publication(
                    self.poisoned,
                    resume.0.realm,
                    &resume.0.converted,
                )? {
                    StringPrimitiveStep::Value(value) => value,
                    StringPrimitiveStep::Throw(value) => {
                        return self.complete(Completion::Throw(value), false);
                    }
                    StringPrimitiveStep::CyclePublishedThrow(value) => {
                        return self.complete(Completion::Throw(value), true);
                    }
                };
                resume.0.string_input = if matches!(resume.0.converted, JsValue::String(_)) {
                    std::mem::replace(&mut resume.0.converted, JsValue::Undefined)
                } else {
                    JsValue::String(self.state.heap.allocate_string(input.clone())?)
                };
                let JsValue::Object(object) = resume.0.regexp else {
                    return Err(RuntimeError::Invariant(
                        "RegExp input conversion lost its branded receiver",
                    ));
                };
                let atom = self.state.pinned_atoms.get(PinnedAtom::LastIndex);
                let ready = match self
                    .state
                    .select_own_property(self.poisoned, object, atom)?
                {
                    OwnPropertySelection::Ready(ready) => ready,
                    OwnPropertySelection::CyclePublished(ready) => {
                        resume.0.published = true;
                        ready
                    }
                    _ => {
                        return Err(RuntimeError::Invariant(
                            "genuine RegExp object had no lastIndex property",
                        ));
                    }
                };
                let descriptor = self
                    .state
                    .own_selected_property_descriptor(self.poisoned, ready)?;
                resume.0.value = Some(
                    self.state
                        .duplicate_owned_descriptor_data(self.poisoned, descriptor)?
                        .ok_or(RuntimeError::Invariant(
                            "RegExp lastIndex became an accessor",
                        ))?,
                );
                resume.0.phase = ExecPhase::LastIndex(input);
                if matches!(resume.0.value, Some(JsValue::Object(_))) {
                    resume.0.hint = Some(ToPrimitiveHint::Number);
                    Ok(RegExpExecStep::Primitive {
                        resume: self.take(),
                    })
                } else {
                    let value = resume.0.value.take().expect("RegExp lastIndex");
                    let previous = std::mem::replace(&mut resume.0.converted, value);
                    self.state.release_owned_jsvalue(self.poisoned, previous)?;
                    self.drive()
                }
            }
            ExecPhase::LastIndex(input) => {
                let resume = self.resume.as_mut().expect("RegExp owner");
                if matches!(resume.0.converted, JsValue::Object(_)) {
                    return Err(RuntimeError::Invariant(
                        "RegExp lastIndex conversion returned an object",
                    ));
                }
                let last_index = match self.state.number_from_primitive_jsvalue_with_publication(
                    self.poisoned,
                    resume.0.realm,
                    &resume.0.converted,
                )? {
                    NumberPrimitiveStep::Value(index) => Runtime::length_from_number(index),
                    NumberPrimitiveStep::CyclePublishedThrow(value) => {
                        return self.complete(Completion::Throw(value), true);
                    }
                };
                self.execute(input, last_index)
            }
            ExecPhase::AfterSet { .. } => Err(RuntimeError::Invariant(
                "RegExp Set phase resumed as a completion",
            )),
        }
    }
    fn execute(mut self, input: JsString, last_index: u64) -> Result<RegExpExecStep, RuntimeError> {
        let this_value = &self.resume.as_ref().expect("RegExp owner").0.regexp;
        // QuickJS keeps the branded RegExp identity across both coercions, but
        // reads `re->bytecode` only afterwards. Either conversion may call the
        // legacy `compile()` method, so snapshot the current program and flags
        // only after those observable calls have completed.
        let current =
            self.state
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
                    return self.fail(
                        NativeErrorKind::Internal,
                        "out of memory in regexp execution",
                    );
                }
                Err(ExecError::Interrupted) => {
                    return self.fail(NativeErrorKind::Internal, "interrupted");
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

        let last_index_write = match &matched {
            None => updates_last_index.then_some(0),
            Some(matched) => {
                // Validate capture zero before ANY result work, including the
                // nonglobal case, exactly as the original matcher consumer.
                let complete = matched.capture(0).ok_or(RuntimeError::Invariant(
                    "successful RegExp execution omitted capture zero",
                ))?;
                if updates_last_index {
                    Some(i32::try_from(complete.end).map_err(|_| {
                        RuntimeError::Invariant("RegExp match end exceeded signed String range")
                    })?)
                } else {
                    None
                }
            }
        };
        self.owner().phase = ExecPhase::AfterSet {
            input,
            program,
            matched,
        };
        if let Some(value) = last_index_write {
            let resume = self.resume.as_mut().expect("RegExp owner");
            let JsValue::Object(object) = resume.0.regexp else {
                return Err(RuntimeError::Invariant(
                    "RegExp lastIndex conversion lost its branded receiver",
                ));
            };
            let receiver = self.state.dup_jsvalue(&resume.0.regexp)?;
            let progress = self.state.start_set_borrowed(
                self.poisoned,
                Some(resume.0.realm),
                object,
                self.state.pinned_atoms.get(PinnedAtom::LastIndex),
                JsValue::Int(value),
                receiver,
            )?;
            match progress {
                SetProgress::Complete(action) if !matches!(action, SetAction::Call { .. }) => {
                    self.finish_set(action)
                }
                SetProgress::CyclePublished(action)
                    if !matches!(action, SetAction::Call { .. }) =>
                {
                    self.owner().published = true;
                    self.finish_set(action)
                }
                progress => Ok(RegExpExecStep::Set {
                    progress: Box::new(progress),
                    resume: self.take(),
                }),
            }
        } else {
            self.finish_match()
        }
    }
    fn finish_set(mut self, action: SetAction) -> Result<RegExpExecStep, RuntimeError> {
        let realm = self.owner().realm;
        let atom = self.state.pinned_atoms.get(PinnedAtom::LastIndex);
        match self.state.finish_set_property_or_throw_in_state(
            self.poisoned,
            realm,
            atom,
            action,
        )? {
            SetProgress::Complete(SetAction::Complete) => self.finish_match(),
            SetProgress::Complete(SetAction::Throw(value)) => {
                self.complete(Completion::Throw(value), false)
            }
            SetProgress::CyclePublished(SetAction::Throw(value)) => {
                self.complete(Completion::Throw(value), true)
            }
            progress => {
                progress.retire_in_state(self.state, self.poisoned)?;
                Err(RuntimeError::Invariant(
                    "strict RegExp lastIndex Set suspended after reply",
                ))
            }
        }
    }
    fn finish_match(mut self) -> Result<RegExpExecStep, RuntimeError> {
        let ExecPhase::AfterSet {
            input,
            program,
            matched,
        } = std::mem::replace(&mut self.owner().phase, ExecPhase::Called)
        else {
            return Err(RuntimeError::Invariant("RegExp result lost match phase"));
        };
        let Some(matched) = matched else {
            return self.complete(Completion::Return(JsValue::Null), false);
        };
        let resume = self.resume.as_ref().expect("RegExp owner");
        let result = self.state.build_regexp_result(
            self.poisoned,
            resume.0.realm,
            input,
            &resume.0.string_input,
            program,
            matched,
        )?;
        self.complete(Completion::Return(result), true)
    }
}
impl Drop for RegExpGuard<'_> {
    fn drop(&mut self) {
        if std::thread::panicking() {
            self.poisoned.set(true);
        }
        if self.poisoned.get() {
            return;
        }
        let _unwind = RuntimeUnwindGuard::from_flag(self.poisoned);
        if let Some(output) = self.output.take() {
            if output.retire_in_state(self.state, self.poisoned).is_err() {
                return;
            }
        }
        if let Some(resume) = self.resume.take() {
            let _ = resume.retire_in_state(self.state, self.poisoned);
        }
    }
}
const _: () = assert!(std::mem::size_of::<RegExpExecResume>() <= 8);
const _: () = assert!(std::mem::size_of::<RegExpExecStep>() <= 56);

#[cfg(test)]
mod local_exec_tests {
    use super::*;

    #[test]
    fn primitive_regexp_exec_completes_inside_its_domain() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
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
        let (RegExpExecStep::Complete(Completion::Return(value))
        | RegExpExecStep::CyclePublished(Completion::Return(value))) = step
        else {
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
        let mut context = runtime.new_context().expect("create context");
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

#[cfg(test)]
mod state_tests;
