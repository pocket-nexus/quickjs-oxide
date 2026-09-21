//! Static and intrinsic PromiseResolve share the constructor identity fast path.
use super::{
    Completion, ContextId, JsValue, NativeConversion, ObjectRef, Phase, PromiseNativeKind,
    PromiseResume, PromiseStep, Runtime, RuntimeError,
};
use crate::engine::heap::ObjectPayload;

pub(super) struct ResolveState {
    runtime: Runtime,
    constructor: ObjectRef,
    argument: JsValue,
    pub(super) kind: PromiseNativeKind,
}
impl ResolveState {
    pub(super) fn take_argument(&mut self) -> JsValue {
        std::mem::replace(&mut self.argument, JsValue::Undefined)
    }
}
impl Drop for ResolveState {
    fn drop(&mut self) {
        let argument = self.take_argument();
        let _ = self.runtime.release_jsvalue(argument);
    }
}
impl PromiseStep {
    pub(super) fn static_resolve_borrowed(
        runtime: &Runtime,
        realm: ContextId,
        kind: PromiseNativeKind,
        receiver: &JsValue,
        argument: &JsValue,
    ) -> Result<Self, RuntimeError> {
        let mut inputs =
            super::InvocationState::new(runtime, runtime.dup_jsvalue(receiver)?, Vec::new());
        inputs.arguments.push(runtime.dup_jsvalue(argument)?);
        Self::static_resolve_jsvalue(
            runtime,
            realm,
            kind,
            inputs.take_receiver(),
            inputs.arguments.pop().unwrap(),
        )
    }
    pub(crate) fn static_resolve_jsvalue(
        runtime: &Runtime,
        realm: ContextId,
        kind: PromiseNativeKind,
        receiver: JsValue,
        argument: JsValue,
    ) -> Result<Self, RuntimeError> {
        let mut inputs = super::InvocationState::new(runtime, receiver, vec![argument]);
        let JsValue::Object(id) = inputs.receiver else {
            return super::capability::error(runtime, realm, "not an object");
        };
        let constructor = ObjectRef::from_borrowed_handle(runtime.clone(), id)?;
        let state = ResolveState {
            runtime: runtime.clone(),
            constructor,
            argument: inputs.arguments.pop().unwrap(),
            kind,
        };
        let is_promise = if kind == PromiseNativeKind::Resolve
            && let JsValue::Object(id) = &state.argument
        {
            matches!(
                runtime.0.state.borrow().heap.object(*id)?.payload,
                ObjectPayload::Promise(_)
            )
        } else {
            false
        };
        if kind == PromiseNativeKind::Resolve && is_promise {
            let key = runtime
                .pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Constructor)?;
            let receiver = runtime.dup_jsvalue(&state.argument)?;
            return Ok(Self::request_read(
                receiver,
                key,
                Box::new(PromiseResume {
                    runtime: runtime.clone(),
                    pending_effect: super::PromiseStepPending::default(),
                    realm,
                    phase: Phase::StaticConstructor(state),
                }),
            ));
        }
        create(runtime, realm, state)
    }
}
pub(super) fn constructor(
    runtime: &Runtime,
    realm: ContextId,
    mut state: ResolveState,
    completion: Completion,
) -> Result<PromiseStep, RuntimeError> {
    match completion {
        Completion::Throw(value) => Ok(PromiseStep::Complete(Completion::Throw(value))),
        Completion::Return(value) => {
            let same =
                matches!(&value, JsValue::Object(id) if *id == state.constructor.object_id());
            runtime.release_jsvalue(value)?;
            if same {
                Ok(PromiseStep::Complete(Completion::Return(
                    state.take_argument(),
                )))
            } else {
                create(runtime, realm, state)
            }
        }
    }
}
fn create(
    runtime: &Runtime,
    realm: ContextId,
    state: ResolveState,
) -> Result<PromiseStep, RuntimeError> {
    let constructor = match runtime.constructor_from_jsvalue(
        realm,
        JsValue::Object(state.constructor.clone().into_handle()),
    )? {
        NativeConversion::Throw(value) => {
            return Ok(PromiseStep::Complete(Completion::Throw(value)));
        }
        NativeConversion::Value(constructor) => constructor,
    };
    Box::new(PromiseResume {
        runtime: runtime.clone(),
        pending_effect: super::PromiseStepPending::default(),
        realm,
        phase: Phase::StaticCapability(state),
    })
    .capability(runtime, Some(constructor))
}
