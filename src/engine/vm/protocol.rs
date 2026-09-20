use crate::engine::{
    api::Error,
    object::ObjectRef,
    value::{JsValue, Value},
};

/// Caller state attached to one original direct-eval invocation.
///
/// The VM constructs this only after the realm-local original-eval identity
/// gate succeeds. `this_value` is the caller-visible binding: primitive String
/// input therefore triggers the caller frame's lazy sloppy-`this`
/// normalization before crossing the runtime boundary, while non-String input
/// retains the raw call value and cannot allocate a wrapper merely to be
/// returned unchanged.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct DirectEvalInvocation {
    pub input: Value,
    pub environment: u16,
    pub this_value: Value,
    pub caller_strict: bool,
}

pub(crate) struct CallInput {
    runtime: crate::engine::api::runtime::Runtime,
    pub this_value: JsValue,
    pub new_target: JsValue,
    pub callee_global: Option<ObjectRef>,
}

impl CallInput {
    pub(in crate::engine::vm) fn new(
        runtime: &crate::engine::api::runtime::Runtime,
        this_value: JsValue,
        new_target: JsValue,
        callee_global: Option<ObjectRef>,
    ) -> Self {
        Self {
            runtime: runtime.clone(),
            this_value,
            new_target,
            callee_global,
        }
    }
}

impl Drop for CallInput {
    /// Release the two internal call edges the frame still owns when the cold
    /// owner is recycled. Releases are defer-safe and never run JavaScript;
    /// consumed slots have already been replaced with `Undefined`.
    fn drop(&mut self) {
        let this_value = std::mem::replace(&mut self.this_value, JsValue::Undefined);
        let _ = self.runtime.release_jsvalue(this_value);
        let new_target = std::mem::replace(&mut self.new_target, JsValue::Undefined);
        let _ = self.runtime.release_jsvalue(new_target);
    }
}

impl CallInput {
    pub(in crate::engine::vm) fn callee_global(
        &mut self,
        runtime: &crate::engine::api::runtime::Runtime,
        realm: crate::engine::heap::ContextId,
    ) -> Result<&ObjectRef, Error> {
        if self.callee_global.is_none() {
            self.callee_global = Some(
                runtime
                    .global_object_for_realm(realm)
                    .map_err(crate::engine::vm::exception::runtime_error_to_vm_error)?,
            );
        }
        Ok(self.callee_global.as_ref().unwrap())
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
