//! Resume only the semantic progress saved at a real Get effect.
use super::owner::ResumeScope;
use super::*;

impl ProxyGetResume {
    pub(crate) fn resume(
        self,
        runtime: &Runtime,
        completion: Completion,
    ) -> Result<ProxyGetStep, RuntimeError> {
        let mut state = runtime.0.state.borrow_mut();
        let mut scope = ResumeScope::new(runtime, &mut state, self.0);
        if let Completion::Throw(value) = completion {
            return scope.finish(ProxyGetStep::Complete(Completion::Throw(value)));
        }
        let Completion::Return(value) = completion else {
            unreachable!()
        };
        let (state, record) = scope.parts();
        let mut value_guard = OwnedValueGuard::new(state, &runtime.0.poisoned, value);
        let (state, value) = value_guard.parts();
        let realm = record.realm;
        let phase = record.phase.take().expect("Get reply phase");
        match phase {
            Phase::Method {
                resume,
                atom,
                receiver,
                arguments,
            } => {
                // Put original inputs in the resident record before the method
                // reply can fail. Only the method state itself is consumed.
                let mut inputs = owner::InputScope::new(runtime, state, atom, receiver, arguments);
                let (state, receiver, args) = inputs.parts();
                let method =
                    resume.resume(runtime, state, Completion::Return(value.take().unwrap()))?;
                let output = after_method(
                    runtime,
                    state,
                    realm,
                    atom,
                    receiver,
                    std::mem::take(args),
                    method,
                )?;
                inputs.finish()?;
                drop(value_guard);
                scope.finish(output)
            }
            Phase::Trap { rooted, atom } => {
                record.phase = Some(Phase::Trap { rooted, atom });
                let Phase::Trap { rooted, atom } = record.phase.as_ref().unwrap() else {
                    unreachable!()
                };
                let target = rooted.target;
                let checked = invariant::check_target(
                    state,
                    runtime,
                    target,
                    *atom,
                    value.as_ref().unwrap(),
                )?;
                let Some(violation) = checked else {
                    let _owner = state.dup_jsvalue(&JsValue::Object(target))?;
                    let Phase::Trap { rooted, atom } = record.phase.take().unwrap() else {
                        unreachable!()
                    };
                    record.phase = Some(Phase::Invariant {
                        rooted,
                        result: value.take().unwrap(),
                    });
                    record.request = Some(Request::Descriptor {
                        object: target,
                        atom,
                    });
                    drop(value_guard);
                    return Ok(ProxyGetStep::Effect(StateReadEffect::Get(
                        ProxyGetEffect::Descriptor(scope.take()),
                    )));
                };
                let output =
                    invariant::complete(state, runtime, realm, value.take().unwrap(), violation)?;
                drop(value_guard);
                scope.finish(output)
            }
            Phase::Invariant { rooted, result } => {
                record.phase = Some(Phase::Invariant { rooted, result });
                Err(RuntimeError::Invariant(
                    "Proxy Get descriptor phase received value reply",
                ))
            }
        }
    }
    pub(crate) fn descriptor(
        self,
        runtime: &Runtime,
        descriptor: NativeConversion<
            Option<crate::engine::object::OwnedCompletePropertyDescriptor>,
        >,
    ) -> Result<ProxyGetStep, RuntimeError> {
        let descriptor = match descriptor {
            NativeConversion::Value(v) => NativeConversion::Value(v.map(|d| d.into_state_owned())),
            NativeConversion::Throw(v) => NativeConversion::Throw(v),
        };
        let mut state = runtime.0.state.borrow_mut();
        let mut scope = ResumeScope::new(runtime, &mut state, self.0);
        let (state, record) = scope.parts();
        let result = match record.phase.as_mut() {
            Some(Phase::Invariant { result, .. }) => std::mem::replace(result, JsValue::Undefined),
            _ => {
                invariant::release_descriptor(state, runtime, descriptor)?;
                return Err(RuntimeError::Invariant(
                    "Proxy Get value phase received descriptor reply",
                ));
            }
        };
        let mut result_guard = OwnedValueGuard::new(state, &runtime.0.poisoned, result);
        let (state, result) = result_guard.parts();
        let output = match descriptor {
            NativeConversion::Throw(value) => ProxyGetStep::Complete(Completion::Throw(value)),
            NativeConversion::Value(descriptor) => {
                let violation = descriptor.as_ref().is_some_and(|d| {
                    invariant::violation(&state.heap, result.as_ref().unwrap(), d.record())
                });
                if let Some(d) = descriptor {
                    d.release_in_state(state, &runtime.0.poisoned)?;
                }
                invariant::complete(
                    state,
                    runtime,
                    record.realm,
                    result.take().unwrap(),
                    violation,
                )?
            }
        };
        drop(result_guard);
        scope.finish(output)
    }
    pub(crate) fn take_read(&mut self) -> (StateReadEffect, Atom) {
        let Some(Request::Read { effect, atom }) = self.0.request.take() else {
            unreachable!("Get read request")
        };
        (effect, atom)
    }
    pub(crate) fn take_call(
        &mut self,
        runtime: &Runtime,
    ) -> (DirectCallTarget, JsValue, Vec<JsValue>) {
        let Some(Request::Call {
            target,
            receiver,
            arguments,
        }) = self.0.request.take()
        else {
            unreachable!("Get call request")
        };
        (target.into_legacy(runtime), receiver, arguments)
    }
    pub(crate) fn take_descriptor(&mut self, runtime: &Runtime) -> (ObjectRef, PropertyKey) {
        let Some(Request::Descriptor { object, atom }) = self.0.request.take() else {
            unreachable!("Get descriptor request")
        };
        (
            ObjectRef::from_owned_handle(runtime.clone(), object),
            PropertyKey::from_owned_atom(runtime.clone(), atom),
        )
    }
    pub(crate) fn release_owned(self, runtime: &Runtime) {
        if !runtime.skip_cleanup() {
            let _unwind = runtime.unwind_guard();
            let mut state = runtime.0.state.borrow_mut();
            let _ = self.0.into_inner().release_in_state(runtime, &mut state);
        }
    }
    pub(crate) fn release_in_state(
        self,
        runtime: &Runtime,
        state: &mut RuntimeState,
    ) -> Result<(), RuntimeError> {
        self.0.into_inner().release_in_state(runtime, state)
    }
}
impl ProxyGetEffect {
    pub(crate) fn release_in_state(
        self,
        state: &mut RuntimeState,
        runtime: &Runtime,
    ) -> Result<(), RuntimeError> {
        match self {
            Self::Read(r) | Self::Call(r) | Self::Descriptor(r) => {
                r.release_in_state(runtime, state)
            }
        }
    }
}
