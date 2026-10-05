//! Shared effect-free Proxy own-property selection.
use super::*;
use crate::engine::{
    heap::{ObjectId, ObjectPayload, runtime::RuntimeState},
    object::{StateOwnPropertySnapshot, StateOwnedCompleteDescriptor},
};

#[must_use]
pub(crate) enum StateOwnPrefix {
    /// Consume this identity immediately under the same State access while
    /// the original source owner remains live. No JavaScript ran during the
    /// prefix, so its Proxy target edges pin the selected identity. A consumer
    /// crossing that scope or retiring the source must first acquire an owner.
    Direct(ObjectId),
    /// Exactly the selected handler read or trap. The remaining Own algorithm
    /// consumes these facts; it must not run GetMethod again.
    Effect(super::super::StateMethodStep),
    Throw(JsValue),
}
impl StateOwnPrefix {
    pub(crate) fn select(
        runtime: &Runtime,
        state: &mut RuntimeState,
        realm: ContextId,
        mut object: ObjectId,
    ) -> Result<Self, RuntimeError> {
        loop {
            if !matches!(state.heap.object(object)?.payload, ObjectPayload::Proxy(_)) {
                return Ok(Self::Direct(object));
            }
            match super::super::StateMethodStep::start(
                runtime,
                state,
                realm,
                object,
                "getOwnPropertyDescriptor",
            )? {
                super::super::StateMethodStep::Complete(selection)
                    if selection.target.is_none() =>
                {
                    object = selection.rooted.target;
                    selection.release_in_state(state, runtime)?;
                }
                super::super::StateMethodStep::Throw(value) => return Ok(Self::Throw(value)),
                selected => return Ok(Self::Effect(selected)),
            }
        }
    }
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn release_in_state(
        self,
        runtime: &Runtime,
        state: &mut RuntimeState,
    ) -> Result<(), RuntimeError> {
        match self {
            Self::Direct(_) => Ok(()),
            Self::Effect(method) => method.release_in_state(runtime, state),
            Self::Throw(value) => state.release_owned_jsvalue(&runtime.0.poisoned, value),
        }
    }
}

#[must_use]
pub(super) enum StateOwnDescriptor {
    Complete(Option<StateOwnedCompleteDescriptor>),
    /// Read backing storage after releasing State, then decode under a fresh
    /// State access. This service does not create a JavaScript continuation.
    Shared(crate::engine::builtins::SharedTypedRead),
}

/// Descriptor results acquire owners only at a consumer that needs a snapshot.
/// Pure permission/invariant consumers keep using the borrowed snapshot API.
pub(super) fn own_descriptor_in_state(
    runtime: &Runtime,
    state: &mut RuntimeState,
    object: ObjectId,
    atom: crate::engine::atom::Atom,
) -> Result<StateOwnDescriptor, RuntimeError> {
    let record = match state.own_property_snapshot_in_state(&runtime.0.poisoned, object, atom)? {
        StateOwnPropertySnapshot::Absent => return Ok(StateOwnDescriptor::Complete(None)),
        StateOwnPropertySnapshot::Borrowed(view) => view.record().clone(),
        StateOwnPropertySnapshot::Owned(owner) => {
            return Ok(StateOwnDescriptor::Complete(Some(owner)));
        }
        StateOwnPropertySnapshot::Shared(read) => return Ok(StateOwnDescriptor::Shared(read)),
        StateOwnPropertySnapshot::Proxy => {
            return Err(RuntimeError::Invariant("own prefix omitted a Proxy"));
        }
    };
    StateOwnedCompleteDescriptor::retain_in_state(state, &runtime.0.poisoned, &record)
        .map(|owner| StateOwnDescriptor::Complete(Some(owner)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::api::Value;

    #[test]
    fn own_prefix_forwards_empty_handlers_under_current_state_without_runtime_or_resume() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        let Value::Object(proxy) = context
            .eval("new Proxy(new Proxy({x:1},{}),new Proxy({},{}))")
            .unwrap()
        else {
            panic!()
        };
        let strong = std::rc::Rc::strong_count(&runtime.0);
        #[cfg(feature = "profiling")]
        let profile = crate::engine::api::profiling::CostProfile::start();
        let mut state = runtime.0.state.borrow_mut();
        let StateOwnPrefix::Direct(target) =
            StateOwnPrefix::select(&runtime, &mut state, context.realm, proxy.object_id()).unwrap()
        else {
            panic!()
        };
        let atom = state.atoms.intern("x").unwrap();
        let StateOwnDescriptor::Complete(Some(owner)) =
            own_descriptor_in_state(&runtime, &mut state, target, atom).unwrap()
        else {
            panic!()
        };
        assert!(matches!(
            owner.record(),
            crate::engine::object::property::CompletePropertyDescriptor::Data {
                value: crate::engine::heap::RawValue::Int(1),
                ..
            }
        ));
        owner
            .release_in_state(&mut state, &runtime.0.poisoned)
            .unwrap();
        state.release_atoms([atom]).unwrap();
        assert_eq!(std::rc::Rc::strong_count(&runtime.0), strong);
        assert_eq!(runtime.0.proxy_method_depth.get(), 0);
        #[cfg(feature = "profiling")]
        for event in [
            "method_resume_allocation",
            "get_resume_allocation",
            "legacy_method_resume_allocation",
        ] {
            assert!(
                !profile
                    .snapshot()
                    .owned_execution_events
                    .contains_key(event)
            );
        }
        assert!(!runtime.0.deferred_references.has_pending());
    }
    #[test]
    fn selected_own_getter_cancellation_restores_depth_and_has_no_runtime_owner() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        let Value::Object(proxy) = context
            .eval("new Proxy({}, {get getOwnPropertyDescriptor(){throw 42}})")
            .unwrap()
        else {
            panic!()
        };
        let strong = std::rc::Rc::strong_count(&runtime.0);
        let mut state = runtime.0.state.borrow_mut();
        let selected =
            StateOwnPrefix::select(&runtime, &mut state, context.realm, proxy.object_id()).unwrap();
        assert!(matches!(
            selected,
            StateOwnPrefix::Effect(super::super::super::StateMethodStep::Read { .. })
        ));
        assert_eq!(std::rc::Rc::strong_count(&runtime.0), strong);
        assert_eq!(runtime.0.proxy_method_depth.get(), 1);
        selected.release_in_state(&runtime, &mut state).unwrap();
        assert_eq!(runtime.0.proxy_method_depth.get(), 0);
        assert!(!runtime.is_poisoned());
        assert!(!runtime.0.deferred_references.has_pending());
    }
}

#[cfg(test)]
#[test]
fn shared_own_snapshot_hands_backing_service_outside_state_without_query() {
    use crate::engine::{api::Value, heap::RawValue};
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let Value::Object(proxy)=context.eval("var ownShared=new Int32Array(new SharedArrayBuffer(4));ownShared[0]=37;new Proxy(ownShared,{})").unwrap() else{panic!()};
    let key = runtime.property_key_for_index(0).unwrap();
    #[cfg(feature = "profiling")]
    let profile = crate::engine::api::profiling::CostProfile::start();
    let token = {
        let mut state = runtime.0.state.borrow_mut();
        let StateOwnPrefix::Direct(target) =
            StateOwnPrefix::select(&runtime, &mut state, context.realm, proxy.object_id()).unwrap()
        else {
            panic!()
        };
        let StateOwnDescriptor::Shared(token) =
            own_descriptor_in_state(&runtime, &mut state, target, key.atom()).unwrap()
        else {
            panic!()
        };
        token
    };
    let (element, bytes) = token.read().unwrap();
    let mut state = runtime.0.state.borrow_mut();
    let value = state.decode_typed_index(element, bytes).unwrap();
    assert!(matches!(value, JsValue::Int(37)));
    state
        .release_owned_jsvalue(&runtime.0.poisoned, value)
        .unwrap();
    drop(state);
    let result = super::ProxyOwnStep::start(
        &runtime,
        context.realm,
        proxy.try_clone().unwrap(),
        key.try_clone().unwrap(),
    )
    .unwrap();
    let super::ProxyOwnStep::Complete(NativeConversion::Value(Some(descriptor))) = result else {
        panic!()
    };
    assert!(matches!(
        descriptor.record(),
        crate::engine::object::property::CompletePropertyDescriptor::Data {
            value: RawValue::Int(37),
            writable: true,
            enumerable: true,
            configurable: true
        }
    ));
    drop(descriptor);
    #[cfg(feature = "profiling")]
    for event in [
        "query_creation",
        "method_resume_allocation",
        "get_resume_allocation",
        "legacy_method_resume_allocation",
    ] {
        assert!(
            !profile
                .snapshot()
                .owned_execution_events
                .contains_key(event)
        );
    }
    assert_eq!(runtime.0.proxy_method_depth.get(), 0);
    assert!(!runtime.0.deferred_references.has_pending());
    assert!(!runtime.is_poisoned());
}

#[cfg(test)]
#[test]
fn selected_own_trap_argument_failure_releases_prior_owners() {
    use crate::engine::api::Value;
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let Value::Object(proxy) = context
        .eval("new Proxy({}, {getOwnPropertyDescriptor(){return undefined}})")
        .unwrap()
    else {
        panic!()
    };
    let key = PropertyKey::from(runtime.new_symbol(None).unwrap());
    let strong = std::rc::Rc::strong_count(&runtime.0);
    let argument_key = key.try_clone().unwrap();
    let (selected, target, handler, target_count, handler_count, index) = {
        let mut state = runtime.0.state.borrow_mut();
        let ObjectPayload::Proxy(data) = state.heap.object(proxy.object_id()).unwrap().payload
        else {
            panic!()
        };
        let target_count = state.heap.object_strong_count(data.target).unwrap();
        let handler_count = state.heap.object_strong_count(data.handler).unwrap();
        let StateOwnPrefix::Effect(selected) =
            StateOwnPrefix::select(&runtime, &mut state, context.realm, proxy.object_id()).unwrap()
        else {
            panic!()
        };
        let index = state.atoms.unbrand(key.atom()).unwrap();
        state.atoms.set_ref_count_for_test(index, u32::MAX);
        (
            selected,
            data.target,
            data.handler,
            target_count,
            handler_count,
            index,
        )
    };
    let result =
        super::ProxyOwnStep::start_selected_method(&runtime, context.realm, argument_key, selected);
    let state = runtime.0.state.borrow_mut();
    state.atoms.set_ref_count_for_test(index, 1);
    assert!(matches!(
        result,
        Err(RuntimeError::Atom(
            crate::engine::atom::AtomError::RefCountOverflow(_)
        ))
    ));
    assert_eq!(state.heap.object_strong_count(target), Ok(target_count));
    assert_eq!(state.heap.object_strong_count(handler), Ok(handler_count));
    assert_eq!(std::rc::Rc::strong_count(&runtime.0), strong);
    assert_eq!(runtime.0.proxy_method_depth.get(), 0);
    assert!(!runtime.0.deferred_references.has_pending());
    assert!(!runtime.is_poisoned());
}
