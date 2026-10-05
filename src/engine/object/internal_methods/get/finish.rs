//! Embedding boundary drives the same selected effects as the VM.
use super::*;
use crate::engine::object::CallableRef;

impl Runtime {
    pub(crate) fn finish_proxy_get_step(
        &self,
        realm: ContextId,
        step: ProxyGetStep,
    ) -> Result<Completion, RuntimeError> {
        match step {
            ProxyGetStep::Complete(completion) => Ok(completion),
            ProxyGetStep::Effect(effect) => self.finish_state_read_effect(realm, effect, None),
        }
    }

    pub(crate) fn finish_proxy_get_effect(
        &self,
        realm: ContextId,
        effect: ProxyGetEffect,
    ) -> Result<Completion, RuntimeError> {
        let mut pending = PendingScope {
            runtime: self,
            resume: None,
        };
        let step = match effect {
            ProxyGetEffect::Read(resume) => {
                pending.resume = Some(resume);
                let (effect, atom) = pending.resume.as_mut().unwrap().take_read();
                let reply = self.finish_state_read_effect(realm, effect, Some(atom))?;
                pending.resume.take().unwrap().resume(self, reply)?
            }
            ProxyGetEffect::Call(resume) => {
                pending.resume = Some(resume);
                let (target, receiver, arguments) =
                    pending.resume.as_mut().unwrap().take_call(self);
                let reply = match target {
                    DirectCallTarget::Callable(callable) => {
                        self.call_internal_jsvalue(realm, &callable, receiver, arguments)?
                    }
                    DirectCallTarget::NonCallableProxy(proxy) => {
                        self.call_proxy_jsvalue(realm, &proxy, receiver, arguments)?
                    }
                };
                pending.resume.take().unwrap().resume(self, reply)?
            }
            ProxyGetEffect::PreparedDescriptor(resume) => {
                pending.resume = Some(resume);
                let (method, atom) = pending.resume.as_mut().unwrap().take_prepared_descriptor();
                let key = PropertyKey::from_owned_atom(self.clone(), atom);
                let step =
                    super::super::ProxyOwnStep::start_selected_method(self, realm, key, method)?;
                let reply = self.finish_proxy_own_step(realm, step)?;
                pending.resume.take().unwrap().descriptor(self, reply)?
            }
        };
        self.finish_proxy_get_step(realm, step)
    }

    pub(crate) fn finish_state_read_effect(
        &self,
        realm: ContextId,
        effect: StateReadEffect,
        atom: Option<Atom>,
    ) -> Result<Completion, RuntimeError> {
        let mut scope = EffectScope {
            runtime: self,
            effect: Some(effect),
            atom,
        };
        match scope.effect.as_ref().unwrap() {
            StateReadEffect::Getter { .. } => {
                scope.release_atom();
                let StateReadEffect::Getter { callee, receiver } = scope.effect.take().unwrap()
                else {
                    unreachable!()
                };
                let callable = CallableRef::from_validated_object(ObjectRef::from_owned_handle(
                    self.clone(),
                    callee,
                ));
                self.call_internal_jsvalue(realm, &callable, receiver, Vec::new())
            }
            StateReadEffect::Get(_) => {
                scope.release_atom();
                let StateReadEffect::Get(effect) = scope.effect.take().unwrap() else {
                    unreachable!()
                };
                self.finish_proxy_get_effect(realm, effect)
            }
            StateReadEffect::Proxy { object, receiver } => {
                let atom = scope
                    .atom
                    .ok_or(RuntimeError::Invariant("selected Proxy read lost its key"))?;
                let mut state = self.0.state.borrow_mut();
                let step = ProxyGetStep::start_in_state(
                    self,
                    &mut state,
                    realm,
                    *object,
                    atom,
                    receiver,
                    Vec::new(),
                )?;
                scope.release_in_state(&mut state)?;
                drop(state);
                self.finish_proxy_get_step(realm, step)
            }
            StateReadEffect::Shared(_) => {
                scope.release_atom();
                let StateReadEffect::Shared(read) = scope.effect.take().unwrap() else {
                    unreachable!()
                };
                let (element, bytes) = read.read()?;
                let value = self
                    .0
                    .state
                    .borrow_mut()
                    .decode_typed_index(element, bytes)?;
                Ok(Completion::Return(value))
            }
        }
    }
}

struct PendingScope<'a> {
    runtime: &'a Runtime,
    resume: Option<ProxyGetResume>,
}
impl Drop for PendingScope<'_> {
    fn drop(&mut self) {
        if let Some(resume) = self.resume.take() {
            resume.release_owned(self.runtime);
        }
    }
}
struct EffectScope<'a> {
    runtime: &'a Runtime,
    effect: Option<StateReadEffect>,
    atom: Option<Atom>,
}
impl EffectScope<'_> {
    fn release_atom(&mut self) {
        if let Some(atom) = self.atom.take() {
            self.runtime.release_atom_handle(atom);
        }
    }
    fn release_in_state(&mut self, state: &mut RuntimeState) -> Result<(), RuntimeError> {
        if let Some(effect) = self.effect.take() {
            effect.release_in_state(state, self.runtime)?;
        }
        if let Some(atom) = self.atom.take() {
            state
                .release_atoms([atom])
                .inspect_err(|_| self.runtime.0.poisoned.set(true))?;
        }
        if self.runtime.0.poisoned.get() {
            return Err(RuntimeError::Poisoned);
        }
        Ok(())
    }
}
impl Drop for EffectScope<'_> {
    fn drop(&mut self) {
        if !self.runtime.skip_cleanup() && (self.effect.is_some() || self.atom.is_some()) {
            let _unwind = self.runtime.unwind_guard();
            let mut state = self.runtime.0.state.borrow_mut();
            let _ = self.release_in_state(&mut state);
        }
    }
}
