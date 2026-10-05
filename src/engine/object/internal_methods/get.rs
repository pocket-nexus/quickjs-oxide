//! Proxy Get consumes the current State and publishes only actual effects.
mod finish;
mod invariant;
mod owner;
mod resume;
#[cfg(test)]
mod tests;
use super::method::{StateMethodResume, StateMethodStep, StateMethodTarget, StateRootedProxy};
use crate::engine::{
    api::{runtime::Runtime, runtime_error::RuntimeError},
    atom::{Atom, PropertyKeyKind},
    heap::{
        ContextId, ObjectId,
        runtime::{
            RuntimeState,
            owned_values::{OwnedValueGuard, OwnedValuesGuard},
        },
    },
    object::{ObjectRef, PropertyKey, ReadBoundary, StateOwnedCompleteDescriptor, StateReadEffect},
    value::{JsValue, conversion::NativeConversion},
    vm::{Completion, call::DirectCallTarget},
};
use owner::MethodScope;

#[must_use]
pub(crate) enum ProxyGetStep {
    Complete(Completion),
    Effect(StateReadEffect),
}
#[must_use]
pub(crate) enum ProxyGetEffect {
    Read(ProxyGetResume),
    Call(ProxyGetResume),
    Descriptor(ProxyGetResume),
}
pub(crate) struct ProxyGetResume(super::reuse::PooledBox<GetState>);
thread_local! {
    #[allow(clippy::vec_box)]
    static EMPTY: std::cell::RefCell<Vec<Box<Option<GetState>>>> = const { std::cell::RefCell::new(Vec::new()) };
}
impl super::reuse::Reusable for GetState {
    #[cfg(feature = "profiling")]
    const EVENT: &'static str = "get_resume_allocation";
    fn pool() -> &'static super::reuse::EmptyPool<Self> {
        &EMPTY
    }
}
struct GetState {
    realm: ContextId,
    phase: Option<Phase>,
    request: Option<Request>,
}
enum Phase {
    Method {
        resume: StateMethodResume,
        atom: Atom,
        receiver: JsValue,
        arguments: Vec<JsValue>,
    },
    Trap {
        rooted: StateRootedProxy,
        atom: Atom,
    },
    Invariant {
        rooted: StateRootedProxy,
        result: JsValue,
    },
}
enum Request {
    Read {
        effect: StateReadEffect,
        atom: Atom,
    },
    Call {
        target: StateMethodTarget,
        receiver: JsValue,
        arguments: Vec<JsValue>,
    },
    Descriptor {
        object: ObjectId,
        atom: Atom,
    },
}

impl ProxyGetStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        object: ObjectRef,
        key: PropertyKey,
        receiver: JsValue,
    ) -> Result<Self, RuntimeError> {
        Self::start_buffered(runtime, realm, object, key, receiver, Vec::new())
    }
    pub(crate) fn start_buffered(
        runtime: &Runtime,
        realm: ContextId,
        object: ObjectRef,
        key: PropertyKey,
        receiver: JsValue,
        arguments: Vec<JsValue>,
    ) -> Result<Self, RuntimeError> {
        // These are boundary owners. Their public destructors run after the
        // local State access ends; the internal receiver uses that same State.
        let mut state = runtime.0.state.borrow_mut();
        let mut receiver_guard = OwnedValueGuard::new(&mut state, &runtime.0.poisoned, receiver);
        let (state, receiver) = receiver_guard.parts();
        let mut arguments_guard = OwnedValuesGuard::new(state, &runtime.0.poisoned, arguments);
        let (state, arguments) = arguments_guard.parts();
        if !object.belongs_to(runtime) || !key.belongs_to(runtime) {
            return Err(RuntimeError::WrongRuntime("Proxy Get operands"));
        }
        let result = Self::start_in_state(
            runtime,
            state,
            realm,
            object.object_id(),
            key.atom(),
            receiver.as_ref().unwrap(),
            std::mem::take(arguments),
        )?;
        drop(arguments_guard);
        drop(receiver_guard);
        if runtime.0.poisoned.get() {
            return Err(RuntimeError::Poisoned);
        }
        Ok(result)
    }
    pub(crate) fn start_in_state(
        runtime: &Runtime,
        state: &mut RuntimeState,
        realm: ContextId,
        object: ObjectId,
        atom: Atom,
        receiver: &JsValue,
        arguments: Vec<JsValue>,
    ) -> Result<Self, RuntimeError> {
        debug_assert!(arguments.is_empty());
        // Empty trap forwarding needs neither recursion nor a resident phase.
        // Keep the next selected holder pinned until its State lookup finishes.
        let mut cursor = OwnedValueGuard::new(state, &runtime.0.poisoned, JsValue::Undefined);
        let mut current = object;
        loop {
            let (state, owner) = cursor.parts();
            let method = StateMethodStep::start(runtime, state, realm, current, "get")?;
            if let StateMethodStep::Complete(selection) = &method
                && selection.target.is_none()
            {
                let target = selection.rooted.target;
                let mut scope = MethodScope::new(runtime, state, method);
                let (state, _) = scope.parts();
                let mut boundary = None;
                let value = state.select_ordinary_read_in_state(
                    &runtime.0.poisoned,
                    target,
                    atom,
                    runtime.0.domain_id,
                    &mut boundary,
                    None,
                )?;
                if let Some(ReadBoundary::Special {
                    object,
                    kind: crate::engine::object::SpecialKind::Proxy,
                }) = boundary
                {
                    let next = state.dup_jsvalue(&JsValue::Object(object))?;
                    if let Some(previous) = owner.replace(next) {
                        state.release_owned_jsvalue(&runtime.0.poisoned, previous)?;
                    }
                    scope.cleanup()?;
                    current = object;
                    continue;
                }
                let output = if let Some(value) = value {
                    ProxyGetStep::Complete(Completion::Return(value))
                } else {
                    resolve_boundary(
                        runtime,
                        state,
                        realm,
                        atom,
                        receiver,
                        boundary.expect("Get selected boundary"),
                    )?
                };
                let output = scope.finish(output)?;
                drop(cursor);
                if runtime.0.poisoned.get() {
                    return Err(RuntimeError::Poisoned);
                }
                return Ok(output);
            }
            let output = after_method(runtime, state, realm, atom, receiver, arguments, method)?;
            drop(cursor);
            if runtime.0.poisoned.get() {
                return Err(RuntimeError::Poisoned);
            }
            return Ok(output);
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn release_in_state(
        self,
        runtime: &Runtime,
        state: &mut RuntimeState,
    ) -> Result<(), RuntimeError> {
        match self {
            Self::Complete(Completion::Return(v) | Completion::Throw(v)) => {
                state.release_owned_jsvalue(&runtime.0.poisoned, v)
            }
            Self::Effect(effect) => effect.release_in_state(state, runtime),
        }
    }
}

fn after_method(
    runtime: &Runtime,
    state: &mut RuntimeState,
    realm: ContextId,
    atom: Atom,
    receiver: &JsValue,
    arguments: Vec<JsValue>,
    method: StateMethodStep,
) -> Result<ProxyGetStep, RuntimeError> {
    let mut scope = MethodScope::new(runtime, state, method);
    let (state, method) = scope.parts();
    let result = match method.as_ref().unwrap() {
        StateMethodStep::Throw(_) => {
            let StateMethodStep::Throw(value) = method.take().unwrap() else {
                unreachable!()
            };
            ProxyGetStep::Complete(Completion::Throw(value))
        }
        StateMethodStep::Read { .. } => {
            let receiver = state.dup_jsvalue(receiver)?;
            let mut receiver = OwnedValueGuard::new(state, &runtime.0.poisoned, receiver);
            let (state, receiver) = receiver.parts();
            state.atoms.retain(atom)?;
            let StateMethodStep::Read {
                effect,
                atom: read_atom,
                resume,
            } = method.take().unwrap()
            else {
                unreachable!()
            };
            let resume = ProxyGetResume(super::reuse::PooledBox::new(GetState {
                realm,
                phase: Some(Phase::Method {
                    resume,
                    atom,
                    receiver: receiver.take().unwrap(),
                    arguments,
                }),
                request: Some(Request::Read {
                    effect,
                    atom: read_atom,
                }),
            }));
            ProxyGetStep::Effect(StateReadEffect::Get(ProxyGetEffect::Read(resume)))
        }
        StateMethodStep::Complete(selection) if selection.target.is_none() => {
            let object = selection.rooted.target;
            let mut boundary = None;
            if let Some(value) = state.select_ordinary_read_in_state(
                &runtime.0.poisoned,
                object,
                atom,
                runtime.0.domain_id,
                &mut boundary,
                None,
            )? {
                ProxyGetStep::Complete(Completion::Return(value))
            } else {
                resolve_boundary(
                    runtime,
                    state,
                    realm,
                    atom,
                    receiver,
                    boundary.expect("selected read boundary"),
                )?
            }
        }
        StateMethodStep::Complete(selection) => {
            let mut args = OwnedValuesGuard::new(state, &runtime.0.poisoned, arguments);
            let (state, args) = args.parts();
            args.try_reserve(3)
                .map_err(|_| RuntimeError::Invariant("proxy get arguments allocation failed"))?;
            args.push(state.dup_jsvalue(&JsValue::Object(selection.rooted.target))?);
            args.push(key_value(state, atom)?);
            args.push(state.dup_jsvalue(receiver)?);
            let handler = state.dup_jsvalue(&JsValue::Object(selection.rooted.handler))?;
            let mut handler = OwnedValueGuard::new(state, &runtime.0.poisoned, handler);
            let (state, handler) = handler.parts();
            state.atoms.retain(atom)?;
            let StateMethodStep::Complete(selection) = method.take().unwrap() else {
                unreachable!()
            };
            let resume = ProxyGetResume(super::reuse::PooledBox::new(GetState {
                realm,
                phase: Some(Phase::Trap {
                    rooted: selection.rooted,
                    atom,
                }),
                request: Some(Request::Call {
                    target: selection.target.unwrap(),
                    receiver: handler.take().unwrap(),
                    arguments: std::mem::take(args),
                }),
            }));
            ProxyGetStep::Effect(StateReadEffect::Get(ProxyGetEffect::Call(resume)))
        }
    };
    scope.finish(result)
}

pub(crate) fn resolve_boundary(
    runtime: &Runtime,
    state: &mut RuntimeState,
    realm: ContextId,
    atom: Atom,
    receiver: &JsValue,
    boundary: ReadBoundary,
) -> Result<ProxyGetStep, RuntimeError> {
    match boundary {
        ReadBoundary::Absent => Ok(ProxyGetStep::Complete(Completion::Return(
            JsValue::Undefined,
        ))),
        ReadBoundary::Special {
            object,
            kind: crate::engine::object::SpecialKind::Proxy,
        } => {
            ProxyGetStep::start_in_state(runtime, state, realm, object, atom, receiver, Vec::new())
        }
        boundary => Ok(ProxyGetStep::Effect(StateReadEffect::prepare(
            state,
            &runtime.0.poisoned,
            boundary,
            receiver,
        )?)),
    }
}

fn key_value(state: &mut RuntimeState, atom: Atom) -> Result<JsValue, RuntimeError> {
    match state.atoms.property_key_kind(atom)? {
        PropertyKeyKind::String => Ok(JsValue::String(
            state
                .heap
                .allocate_string(state.atoms.to_js_string(atom)?)?,
        )),
        PropertyKeyKind::Symbol => {
            let index = state.atoms.unbrand(atom)?;
            state.atoms.retain(atom)?;
            Ok(JsValue::Symbol(index))
        }
        PropertyKeyKind::Private => Err(RuntimeError::Invariant(
            "private key escaped into Proxy Get",
        )),
    }
}
