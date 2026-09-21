use crate::engine::{api::Error, object::ObjectRef, value::JsValue};

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
