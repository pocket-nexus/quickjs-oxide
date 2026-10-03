use crate::engine::{
    api::Error,
    heap::{ObjectId, runtime::RuntimeState},
    object::ObjectRef,
    value::JsValue,
};

/// Caller state attached to one original direct-eval invocation.
///
/// The VM constructs this only after the realm-local original-eval identity
/// gate succeeds. `this_value` is the caller-visible binding: primitive String
/// input therefore triggers the caller frame's lazy sloppy-`this`
/// normalization before crossing the runtime boundary, while non-String input
/// retains the raw call value and cannot allocate a wrapper merely to be
/// returned unchanged.
pub(crate) struct DirectEvalInvocation {
    runtime: crate::engine::api::runtime::Runtime,
    pub input: JsValue,
    pub environment: u16,
    pub this_value: JsValue,
    pub caller_strict: bool,
}

impl DirectEvalInvocation {
    pub(crate) fn new(
        runtime: &crate::engine::api::runtime::Runtime,
        environment: u16,
        caller_strict: bool,
    ) -> Self {
        Self {
            runtime: runtime.clone(),
            input: JsValue::Undefined,
            this_value: JsValue::Undefined,
            environment,
            caller_strict,
        }
    }
    pub(crate) fn take_input(&mut self) -> JsValue {
        std::mem::replace(&mut self.input, JsValue::Undefined)
    }
    pub(crate) fn take_this(&mut self) -> JsValue {
        std::mem::replace(&mut self.this_value, JsValue::Undefined)
    }
}
impl Drop for DirectEvalInvocation {
    fn drop(&mut self) {
        let input = self.take_input();
        let this_value = self.take_this();
        let _ = self.runtime.release_jsvalue(input);
        let _ = self.runtime.release_jsvalue(this_value);
    }
}

#[must_use]
pub(crate) struct CallInput {
    pub this_value: JsValue,
    pub new_target: JsValue,
    pub callee_global: Option<ObjectId>,
}

impl CallInput {
    pub(in crate::engine::vm) fn new(
        _runtime: &crate::engine::api::runtime::Runtime,
        this_value: JsValue,
        new_target: JsValue,
        callee_global: Option<ObjectRef>,
    ) -> Self {
        Self {
            this_value,
            new_target,
            callee_global: callee_global.map(ObjectRef::into_execution_handle),
        }
    }
}

impl CallInput {
    pub(in crate::engine::vm) fn release(
        &mut self,
        state: &mut RuntimeState,
    ) -> Result<(), crate::engine::api::runtime_error::RuntimeError> {
        let this_value = std::mem::replace(&mut self.this_value, JsValue::Undefined);
        state.release_jsvalue(this_value)?;
        let new_target = std::mem::replace(&mut self.new_target, JsValue::Undefined);
        state.release_jsvalue(new_target)?;
        if let Some(global) = self.callee_global.take() {
            state.release_object_handle(global)?;
        }
        Ok(())
    }
}

impl CallInput {
    pub(in crate::engine::vm) fn callee_global(
        &mut self,
        runtime: &crate::engine::api::runtime::Runtime,
        realm: crate::engine::heap::ContextId,
    ) -> Result<ObjectId, Error> {
        self.callee_global_in_state(&mut runtime.0.state.borrow_mut(), realm)
    }

    pub(in crate::engine::vm) fn callee_global_in_state(
        &mut self,
        state: &mut RuntimeState,
        realm: crate::engine::heap::ContextId,
    ) -> Result<ObjectId, Error> {
        if self.callee_global.is_none() {
            let global = state
                .heap
                .context(realm)
                .map_err(crate::engine::vm::exception::heap_error_to_vm_error)?
                .global_object;
            state
                .heap
                .retain_object(global)
                .map_err(crate::engine::vm::exception::heap_error_to_vm_error)?;
            self.callee_global = Some(global);
        }
        Ok(self.callee_global.unwrap())
    }
}

/// Admission guard outside a held state-borrow segment. This borrows Runtime;
/// resident CallInput owns only JS edges and is cleaned with current state.
pub(in crate::engine::vm) struct CallInputGuard<'a> {
    runtime: &'a crate::engine::api::Runtime,
    input: Option<CallInput>,
}
impl<'a> CallInputGuard<'a> {
    pub(in crate::engine::vm) fn new(
        runtime: &'a crate::engine::api::Runtime,
        input: CallInput,
    ) -> Self {
        Self {
            runtime,
            input: Some(input),
        }
    }
    pub(in crate::engine::vm) fn take(&mut self) -> CallInput {
        self.input.take().expect("call input transferred once")
    }
}
impl std::ops::Deref for CallInputGuard<'_> {
    type Target = CallInput;
    fn deref(&self) -> &Self::Target {
        self.input.as_ref().expect("admitted call input")
    }
}
impl std::ops::DerefMut for CallInputGuard<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.input.as_mut().expect("admitted call input")
    }
}
impl Drop for CallInputGuard<'_> {
    fn drop(&mut self) {
        if self.runtime.skip_cleanup() {
            return;
        }
        let _unwind = self.runtime.unwind_guard();
        if let Some(mut input) = self.input.take() {
            if input
                .release(&mut self.runtime.0.state.borrow_mut())
                .is_err()
            {
                self.runtime.0.poisoned.set(true);
            }
        }
    }
}

/// Pinned typeof result atoms; their order follows QuickJS quickjs-atom.h.
pub(crate) const TYPEOF_STATIC_ATOMS: [&str; 8] = [
    "function",
    "undefined",
    "number",
    "boolean",
    "string",
    "object",
    "symbol",
    "bigint",
];
