//! Consume a read selected under State. Receiver and callee are never looked
//! up again after publishing the callback, even if intervening code mutates it.
use super::{
    Completion, DirectCallTarget, Error, Next, ProxyGetStep, Query, ReturnOwner, RunningExecution,
    Runtime, Step, runtime_error_to_vm_error,
};
use crate::engine::object::{CallableRef, ObjectRef, PropertyKey, StateReadEffect};

#[cold]
#[inline(never)]
pub(super) fn dispatch(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    _owner: ReturnOwner,
    _identity: u64,
    query: &mut Query,
    step: &mut Step,
) -> Result<Next, Error> {
    let Step::StateRead {
        effect,
        atom,
        resume,
    } = step
    else {
        return Err(Error::internal(
            "selected read dispatcher received another request",
        ));
    };
    match effect.as_ref().expect("selected read effect") {
        StateReadEffect::Getter { .. } => {
            let StateReadEffect::Getter { callee, receiver } = effect.take().unwrap() else {
                unreachable!();
            };
            runtime.release_atom_handle(atom.take().expect("selected read atom"));
            // The common callable ABI is an explicit boundary for native and
            // bound callees. The read itself has no public input/result root.
            let target = DirectCallTarget::Callable(CallableRef::from_validated_object(
                ObjectRef::from_owned_handle(runtime.clone(), callee),
            ));
            *step = Step::Call {
                target: Some(target),
                receiver: Some(receiver),
                arguments: Some(Vec::new()),
                resume: resume.take(),
            };
            Ok(Next::Invoke)
        }
        StateReadEffect::Proxy { .. } => {
            // This remaining ABI is restricted to an actually selected Proxy
            // holder. Its method prefix is migrated by the shared GetMethod
            // batch; an ordinary getter never enters that lookup protocol.
            query
                .parents
                .try_reserve(1)
                .map_err(|_| Error::internal("property continuation allocation failed"))?;
            let arguments = execution.slots.take_argument_buffer(3)?;
            let StateReadEffect::Proxy { object, receiver } = effect.take().unwrap() else {
                unreachable!();
            };
            let object = ObjectRef::from_owned_handle(runtime.clone(), object);
            let key = PropertyKey::from_owned_atom(runtime.clone(), atom.take().unwrap());
            query
                .parents
                .push(resume.take().expect("selected read resume"));
            *step = ProxyGetStep::start_buffered(
                runtime,
                query.realm,
                object,
                key,
                receiver,
                arguments,
            )
            .map_err(runtime_error_to_vm_error)?
            .try_into()?;
            Ok(Next::Continue)
        }
        StateReadEffect::Shared(_) => {
            let StateReadEffect::Shared(read) = effect.take().unwrap() else {
                unreachable!();
            };
            // The backing lock ends before taking State. No JS can run during
            // this service; its resulting edge belongs to the completion.
            let (element, bytes) = read.read().map_err(runtime_error_to_vm_error)?;
            runtime.release_atom_handle(atom.take().unwrap());
            let value = runtime
                .0
                .state
                .borrow_mut()
                .decode_typed_index(element, bytes)
                .map_err(runtime_error_to_vm_error)?;
            *step = resume
                .take()
                .expect("selected read resume")
                .resume(runtime, Completion::Return(value))
                .map_err(runtime_error_to_vm_error)?;
            Ok(Next::Continue)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{object::ReadBoundary, value::JsValue};

    #[test]
    fn abandoned_selected_read_releases_its_exact_owner_edges() {
        let runtime = Runtime::new();
        let callee = runtime.new_object(None).unwrap().into_handle();
        let receiver = runtime.new_object(None).unwrap().into_handle();
        let key = runtime.intern_property_key("test").unwrap();
        let atom = key.atom();
        let effect = {
            let mut state = runtime.0.state.borrow_mut();
            state.atoms.retain(atom).unwrap();
            StateReadEffect::prepare(
                &mut state,
                &runtime.0.poisoned,
                ReadBoundary::Getter(callee),
                &JsValue::Object(receiver),
            )
            .unwrap()
        };
        let mut step = Step::StateRead {
            effect: Some(effect),
            atom: Some(atom),
            resume: Some(super::super::Resume::Identity),
        };
        step.release_owned(&runtime);
        let state = runtime.0.state.borrow();
        assert_eq!(state.heap.object_strong_count(callee).unwrap(), 1);
        assert_eq!(state.heap.object_strong_count(receiver).unwrap(), 1);
        assert!(!runtime.0.deferred_references.has_pending());
        drop(state);
        runtime.release_jsvalue(JsValue::Object(callee)).unwrap();
        runtime.release_jsvalue(JsValue::Object(receiver)).unwrap();
    }
}
