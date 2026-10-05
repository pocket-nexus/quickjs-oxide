//! Temporary scopes have the current State; resident Get records have no Runtime.
use super::*;

impl GetState {
    pub(super) fn release_in_state(
        mut self,
        runtime: &Runtime,
        state: &mut RuntimeState,
    ) -> Result<(), RuntimeError> {
        if let Some(request) = self.request.take() {
            match request {
                Request::Read { effect, atom } => {
                    effect.release_in_state(state, runtime)?;
                    release_atom(state, runtime, atom)?;
                }
                Request::Call {
                    target,
                    receiver,
                    arguments,
                } => {
                    state.release_owned_jsvalue(
                        &runtime.0.poisoned,
                        JsValue::Object(target.object()),
                    )?;
                    state.release_owned_jsvalue(&runtime.0.poisoned, receiver)?;
                    release_values(state, runtime, arguments)?;
                }
                Request::Descriptor { object, atom } => {
                    state.release_owned_jsvalue(&runtime.0.poisoned, JsValue::Object(object))?;
                    release_atom(state, runtime, atom)?;
                }
            }
        }
        if let Some(phase) = self.phase.take() {
            match phase {
                Phase::Method {
                    resume,
                    atom,
                    receiver,
                    arguments,
                } => {
                    resume.release_in_state(runtime, state)?;
                    release_atom(state, runtime, atom)?;
                    state.release_owned_jsvalue(&runtime.0.poisoned, receiver)?;
                    release_values(state, runtime, arguments)?;
                }
                Phase::Trap { rooted, atom } => {
                    rooted.release_in_state(state, runtime)?;
                    release_atom(state, runtime, atom)?;
                }
                Phase::Invariant { rooted, result } => {
                    rooted.release_in_state(state, runtime)?;
                    state.release_owned_jsvalue(&runtime.0.poisoned, result)?;
                }
            }
        }
        Ok(())
    }
}
fn release_values(
    state: &mut RuntimeState,
    runtime: &Runtime,
    values: Vec<JsValue>,
) -> Result<(), RuntimeError> {
    for value in values {
        state.release_owned_jsvalue(&runtime.0.poisoned, value)?;
    }
    Ok(())
}
fn release_atom(
    state: &mut RuntimeState,
    runtime: &Runtime,
    atom: Atom,
) -> Result<(), RuntimeError> {
    state
        .release_atoms([atom])
        .inspect_err(|_| runtime.0.poisoned.set(true))
}

pub(super) struct MethodScope<'a> {
    runtime: &'a Runtime,
    state: &'a mut RuntimeState,
    method: Option<StateMethodStep>,
}
impl<'a> MethodScope<'a> {
    pub(super) fn new(
        runtime: &'a Runtime,
        state: &'a mut RuntimeState,
        method: StateMethodStep,
    ) -> Self {
        Self {
            runtime,
            state,
            method: Some(method),
        }
    }
    pub(super) fn parts(&mut self) -> (&mut RuntimeState, &mut Option<StateMethodStep>) {
        (self.state, &mut self.method)
    }
    pub(super) fn finish(mut self, output: ProxyGetStep) -> Result<ProxyGetStep, RuntimeError> {
        self.cleanup()?;
        Ok(output)
    }
    pub(super) fn cleanup(&mut self) -> Result<(), RuntimeError> {
        if let Some(method) = self.method.take() {
            method.release_in_state(self.runtime, self.state)?;
        }
        if self.runtime.0.poisoned.get() {
            return Err(RuntimeError::Poisoned);
        }
        Ok(())
    }
}
impl Drop for MethodScope<'_> {
    fn drop(&mut self) {
        if !self.runtime.skip_cleanup()
            && let Some(method) = self.method.take()
        {
            let _unwind = self.runtime.unwind_guard();
            let _ = method.release_in_state(self.runtime, self.state);
        }
    }
}

pub(super) struct ResumeScope<'a> {
    runtime: &'a Runtime,
    state: &'a mut RuntimeState,
    record: Option<super::super::reuse::PooledBox<GetState>>,
}
impl<'a> ResumeScope<'a> {
    pub(super) fn new(
        runtime: &'a Runtime,
        state: &'a mut RuntimeState,
        record: super::super::reuse::PooledBox<GetState>,
    ) -> Self {
        Self {
            runtime,
            state,
            record: Some(record),
        }
    }
    pub(super) fn parts(&mut self) -> (&mut RuntimeState, &mut GetState) {
        (self.state, self.record.as_mut().unwrap())
    }
    pub(super) fn take(mut self) -> ProxyGetResume {
        ProxyGetResume(self.record.take().unwrap())
    }
    pub(super) fn finish(mut self, output: ProxyGetStep) -> Result<ProxyGetStep, RuntimeError> {
        if let Some(record) = self.record.take() {
            record
                .into_inner()
                .release_in_state(self.runtime, self.state)?;
        }
        if self.runtime.0.poisoned.get() {
            return Err(RuntimeError::Poisoned);
        }
        Ok(output)
    }
}
impl Drop for ResumeScope<'_> {
    fn drop(&mut self) {
        if !self.runtime.skip_cleanup()
            && let Some(record) = self.record.take()
        {
            let _unwind = self.runtime.unwind_guard();
            let _ = record
                .into_inner()
                .release_in_state(self.runtime, self.state);
        }
    }
}

pub(super) struct InputScope<'a> {
    state: &'a mut RuntimeState,
    runtime: &'a Runtime,
    atom: Option<Atom>,
    receiver: Option<JsValue>,
    arguments: Vec<JsValue>,
}
impl<'a> InputScope<'a> {
    pub(super) fn new(
        runtime: &'a Runtime,
        state: &'a mut RuntimeState,
        atom: Atom,
        receiver: JsValue,
        arguments: Vec<JsValue>,
    ) -> Self {
        Self {
            state,
            runtime,
            atom: Some(atom),
            receiver: Some(receiver),
            arguments,
        }
    }
    pub(super) fn parts(&mut self) -> (&mut RuntimeState, &JsValue, &mut Vec<JsValue>) {
        (
            self.state,
            self.receiver.as_ref().unwrap(),
            &mut self.arguments,
        )
    }
    pub(super) fn finish(mut self) -> Result<(), RuntimeError> {
        self.release()?;
        if self.runtime.0.poisoned.get() {
            Err(RuntimeError::Poisoned)
        } else {
            Ok(())
        }
    }
    fn release(&mut self) -> Result<(), RuntimeError> {
        if let Some(atom) = self.atom.take() {
            release_atom(self.state, self.runtime, atom)?;
        }
        if let Some(receiver) = self.receiver.take() {
            self.state
                .release_owned_jsvalue(&self.runtime.0.poisoned, receiver)?;
        }
        release_values(
            self.state,
            self.runtime,
            std::mem::take(&mut self.arguments),
        )
    }
}
impl Drop for InputScope<'_> {
    fn drop(&mut self) {
        if !self.runtime.skip_cleanup() {
            let _unwind = self.runtime.unwind_guard();
            let _ = self.release();
        }
    }
}
