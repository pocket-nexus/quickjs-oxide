//! Concrete Set roles, publication and first-fatal retirement.
use super::*;
use crate::engine::api::runtime::RuntimeUnwindGuard;

pub(super) struct SetActionGuard<'a> {
    state: &'a mut RuntimeState,
    poisoned: &'a Cell<bool>,
    action: Option<SetAction>,
}
impl<'a> SetActionGuard<'a> {
    pub(super) fn new(
        state: &'a mut RuntimeState,
        poisoned: &'a Cell<bool>,
        action: SetAction,
    ) -> Self {
        Self {
            state,
            poisoned,
            action: Some(action),
        }
    }
    pub(super) fn parts(&mut self) -> (&mut RuntimeState, &mut Option<SetAction>) {
        (self.state, &mut self.action)
    }
    pub(super) fn take(&mut self) -> SetAction {
        self.action.take().expect("owned Set action")
    }
}
impl Drop for SetActionGuard<'_> {
    fn drop(&mut self) {
        if std::thread::panicking() {
            self.poisoned.set(true);
        }
        if !self.poisoned.get()
            && let Some(action) = self.action.take()
        {
            let _ = action.retire(self.state, self.poisoned);
        }
    }
}

impl SetAction {
    pub(crate) fn retire(
        self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
    ) -> Result<(), RuntimeError> {
        if poisoned.get() {
            return Err(RuntimeError::Poisoned);
        }
        let _unwind = RuntimeUnwindGuard::from_flag(poisoned);
        self.retire_with(|value| state.release_owned_jsvalue(poisoned, value))
            .map_err(|_| RuntimeError::Poisoned)
    }
    pub(crate) fn retire_at_boundary(self, runtime: &Runtime) -> Result<(), RuntimeError> {
        if runtime.skip_cleanup() {
            return Err(RuntimeError::Poisoned);
        }
        let _unwind = runtime.unwind_guard();
        if let Ok(mut state) = runtime.0.state.try_borrow_mut() {
            self.retire(&mut state, &runtime.0.poisoned)
        } else {
            self.retire_with(|value| {
                runtime.release_jsvalue(value)?;
                runtime.check_poison()
            })
        }
    }
    fn retire_with(
        self,
        mut release: impl FnMut(JsValue) -> Result<(), RuntimeError>,
    ) -> Result<(), RuntimeError> {
        match self {
            Self::Throw(value) => release(value),
            Self::Call {
                function,
                receiver,
                argument,
            } => {
                // Before callback publication, the historical wrapper retires
                // its body before its checked callee. RawCall owns its distinct
                // callback ledger after transport.
                release(receiver)?;
                release(argument)?;
                release(JsValue::Object(function))
            }
            _ => Ok(()),
        }
    }
}

pub(super) fn retire_descriptor_reply(
    state: &mut RuntimeState,
    poisoned: &Cell<bool>,
    reply: NativeConversion<Option<CompletePropertyDescriptor<RawValue>>>,
) -> Result<(), RuntimeError> {
    if poisoned.get() {
        return Err(RuntimeError::Poisoned);
    }
    match reply {
        NativeConversion::Throw(value) => state.release_owned_jsvalue(poisoned, value),
        NativeConversion::Value(Some(record)) => {
            CompleteDescriptorGuard::from_owned_record(state, poisoned, record).retire()
        }
        NativeConversion::Value(None) => Ok(()),
    }
}

impl SetResumeState {
    pub(crate) fn retire(
        &mut self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
    ) -> Result<(), RuntimeError> {
        if poisoned.get() {
            return Err(RuntimeError::Poisoned);
        }
        let _unwind = RuntimeUnwindGuard::from_flag(poisoned);
        self.retire_with(|value| state.release_owned_jsvalue(poisoned, value))
            .map_err(|_| RuntimeError::Poisoned)
    }
    fn retire_with(
        &mut self,
        mut release: impl FnMut(JsValue) -> Result<(), RuntimeError>,
    ) -> Result<(), RuntimeError> {
        // Domain body first, then original target/key, phase and request fields.
        // Options stay armed until their exact role is retired; no drain iterator
        // removes an unconsumed suffix across a fallible release.
        if let Some(value) = self.value.take() {
            release(value)?;
        }
        if let Some(value) = self.receiver.take() {
            release(value)?;
        }
        if let Some(object) = self.target_owner.take() {
            release(JsValue::Object(object))?;
        }
        if let Some(atom) = self.key_owner.take() {
            release(JsValue::Symbol(crate::engine::atom::AtomIdx::from_raw(
                atom.raw(),
            )))?;
        }
        if let Some(object) = self.retiring_phase.take() {
            release(JsValue::Object(object))?;
        }
        if let Some(object) = self.phase_object.take() {
            release(JsValue::Object(object))?;
        }
        self.retire_request_with(release)?;
        self.shared = None;
        Ok(())
    }

    fn retire_request_with(
        &mut self,
        mut release: impl FnMut(JsValue) -> Result<(), RuntimeError>,
    ) -> Result<(), RuntimeError> {
        if let Some(object) = self.request_object.take() {
            release(JsValue::Object(object))?;
        }
        if let Some(atom) = self.request_key.take() {
            release(JsValue::Symbol(crate::engine::atom::AtomIdx::from_raw(
                atom.raw(),
            )))?;
        }
        if let Some(value) = self.request_value.take() {
            release(value)?;
        }
        if let Some(value) = self.request_receiver.take() {
            release(value)?;
        }
        if let Some(mut record) = self.request_descriptor.take() {
            if let Some(value) = record.value.take().and_then(JsValue::from_raw) {
                release(value)?;
            }
            if let Some(value) = record.get.take().flatten().and_then(JsValue::from_raw) {
                release(value)?;
            }
            if let Some(value) = record.set.take().flatten().and_then(JsValue::from_raw) {
                release(value)?;
            }
        }
        Ok(())
    }

    /// A public reply may be supplied without extracting the selected request
    /// headers. Retire those concrete roles while its reply and domain remain
    /// armed, before the next publication can replace any field.
    pub(super) fn retire_completed_request(
        &mut self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
    ) -> Result<(), RuntimeError> {
        if poisoned.get() {
            return Err(RuntimeError::Poisoned);
        }
        let _unwind = RuntimeUnwindGuard::from_flag(poisoned);
        self.retire_request_with(|value| state.release_owned_jsvalue(poisoned, value))
            .map_err(|_| RuntimeError::Poisoned)
    }

    fn adopt_phase_or_retain(
        &mut self,
        state: &mut RuntimeState,
        object: ObjectId,
    ) -> Result<ObjectId, RuntimeError> {
        if self.retiring_phase == Some(object) {
            Ok(self.retiring_phase.take().expect("matching phase owner"))
        } else {
            state.heap.retain_object(object)?;
            Ok(object)
        }
    }

    pub(super) fn publish(
        &mut self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        selected: Selected,
    ) -> Result<SetWait, RuntimeError> {
        // Only an actual waiting record needs these separate lifetime roles.
        if self.target_owner.is_none() {
            state.heap.retain_object(self.target)?;
            self.target_owner = Some(self.target);
        }
        if self.key_owner.is_none() {
            self.key_owner = Some(state.atoms.retain_shared(self.atom)?);
        }
        self.retiring_phase = self.phase_object.take();
        let result = (|| match selected {
            Selected::Walk(object) => {
                let object = self.adopt_phase_or_retain(state, object)?;
                self.phase_object = Some(object);
                self.phase = Phase::Walk;
                Ok(SetWait::Continue)
            }
            Selected::Proxy(object) => {
                let object = self.adopt_phase_or_retain(state, object)?;
                self.request_object = Some(object);
                self.request_key = Some(state.atoms.retain_shared(self.atom)?);
                self.request_value =
                    Some(state.dup_jsvalue(self.value.as_ref().expect("Set value"))?);
                self.request_receiver =
                    Some(state.dup_jsvalue(self.receiver.as_ref().expect("Set receiver"))?);
                self.phase = Phase::Forward;
                Ok(SetWait::Proxy)
            }
            Selected::Typed(object, index, element) => {
                state.heap.retain_object(object)?;
                self.request_object = Some(object);
                self.request_key = Some(state.atoms.retain_shared(self.atom)?);
                self.request_value =
                    Some(state.dup_jsvalue(self.value.as_ref().expect("Set value"))?);
                self.request_receiver =
                    Some(state.dup_jsvalue(self.receiver.as_ref().expect("Set receiver"))?);
                let object = self.adopt_phase_or_retain(state, object)?;
                self.phase_object = Some(object);
                self.typed = Some((index, element));
                self.phase = Phase::Special;
                Ok(SetWait::Special)
            }
            Selected::Shared(request) => {
                let object = match &request {
                    SharedRequest::TypedDecline { object, .. }
                    | SharedRequest::Own { object, .. }
                    | SharedRequest::Rejection { object, .. } => *object,
                };
                let object = self.adopt_phase_or_retain(state, object)?;
                self.phase_object = Some(object);
                self.shared = Some(request);
                self.phase = Phase::Special;
                Ok(SetWait::Special)
            }
            Selected::ArrayLength(object, initial) => {
                let object = self.adopt_phase_or_retain(state, object)?;
                self.request_object = Some(object);
                self.request_key = Some(state.atoms.retain_shared(self.atom)?);
                self.request_value =
                    Some(state.dup_jsvalue(self.value.as_ref().expect("Set value"))?);
                self.array_length_initial = Some(initial);
                self.phase = Phase::Forward;
                Ok(SetWait::ArrayLength)
            }
            Selected::Descriptor(object) => {
                let object = self.adopt_phase_or_retain(state, object)?;
                self.request_object = Some(object);
                self.request_key = Some(state.atoms.retain_shared(self.atom)?);
                self.phase = Phase::Receiver;
                Ok(SetWait::Descriptor)
            }
            Selected::Define(object, existing) => {
                state.heap.retain_object(object)?;
                self.request_object = Some(object);
                self.request_key = Some(state.atoms.retain_shared(self.atom)?);
                let value = state.dup_jsvalue(self.value.as_ref().expect("Set value"))?;
                self.request_descriptor = Some(PropertyDescriptor {
                    value: Some(value.into_raw()),
                    writable: (!existing).then_some(true),
                    enumerable: (!existing).then_some(true),
                    configurable: (!existing).then_some(true),
                    ..PropertyDescriptor::new()
                });
                let object = self.adopt_phase_or_retain(state, object)?;
                self.phase_object = Some(object);
                self.phase = Phase::Define;
                Ok(SetWait::Define)
            }
            Selected::Complete(_) => Err(RuntimeError::Invariant(
                "terminal Set was published as a wait",
            )),
        })();
        // On recoverable publication failure the old phase stays armed, so
        // domain retirement still precedes its suffix. Success can retire it
        // only after the new concrete roles have all been published.
        if result.is_ok()
            && !poisoned.get()
            && let Some(object) = self.retiring_phase.take()
        {
            state.release_owned_jsvalue(poisoned, JsValue::Object(object))?;
        }
        if poisoned.get() {
            Err(RuntimeError::Poisoned)
        } else {
            result
        }
    }
}

impl SetResume {
    pub(crate) fn retire_request_headers_at_boundary(
        &mut self,
        runtime: &Runtime,
    ) -> Result<(), RuntimeError> {
        runtime.check_poison()?;
        if let Some(object) = self.0.request_object.take() {
            runtime.release_jsvalue(JsValue::Object(object))?;
            runtime.check_poison()?;
        }
        if let Some(atom) = self.0.request_key.take() {
            runtime.release_jsvalue(JsValue::Symbol(crate::engine::atom::AtomIdx::from_raw(
                atom.raw(),
            )))?;
            runtime.check_poison()?;
        }
        Ok(())
    }
    pub(crate) fn retire_in_state(
        mut self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
    ) -> Result<(), RuntimeError> {
        self.0.retire(state, poisoned)
    }
    pub(crate) fn retire_at_boundary(mut self, runtime: &Runtime) -> Result<(), RuntimeError> {
        if runtime.skip_cleanup() {
            return Err(RuntimeError::Poisoned);
        }
        let _unwind = runtime.unwind_guard();
        if let Ok(mut state) = runtime.0.state.try_borrow_mut() {
            self.0.retire(&mut state, &runtime.0.poisoned)
        } else {
            self.0.retire_with(|value| {
                runtime.release_jsvalue(value)?;
                runtime.check_poison()
            })
        }
    }
    pub(crate) fn take_cycle_published(&mut self) -> bool {
        std::mem::take(&mut self.0.cycle_published)
    }
}

impl SetProgress {
    pub(crate) fn retire_in_state(
        self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
    ) -> Result<(), RuntimeError> {
        match self {
            Self::Complete(action) | Self::CyclePublished(action) => action.retire(state, poisoned),
            Self::Waiting { resume, .. } => resume.retire_in_state(state, poisoned),
        }
    }
    pub(crate) fn retire_at_boundary(self, runtime: &Runtime) -> Result<(), RuntimeError> {
        match self {
            Self::Complete(action) | Self::CyclePublished(action) => {
                action.retire_at_boundary(runtime)
            }
            Self::Waiting { resume, .. } => resume.retire_at_boundary(runtime),
        }
    }
}
