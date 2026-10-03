//! Realm handle ownership and the shared JavaScript completion boundary.
//!
//! Public operations are implemented in responsibility-specific child modules.
//! Module APIs remain with module execution; host installers remain with their
//! respective qjs and Test262 implementations.

use crate::engine::api::error::NativeErrorKind;
use crate::engine::api::runtime::Runtime;
use crate::engine::api::runtime_error::RuntimeError;

use crate::engine::api::compile::Compilation;
#[cfg(feature = "test262-host")]
use crate::engine::builtins::native::NativeFunctionId;
use crate::engine::builtins::native::PrimitiveKind;
use crate::engine::code::rooted::FunctionBytecodeRef;
use crate::engine::compiler::CompileOptions;
use crate::engine::heap::ContextId;

use crate::engine::object::{
    CallableRef, CompleteOrdinaryPropertyDescriptor, ObjectRef, OrdinaryPropertyDescriptor,
    PropertyKey,
};
use crate::engine::value::Value;
use crate::engine::value::conversion::NativeConversion;
use crate::engine::vm::Completion;

mod calls;
mod objects;
mod realm;
mod script;
#[cfg(feature = "test262-host")]
mod test262;

pub use script::EvalOptions;

/// One realm and its execution state.
pub struct Context {
    pub(crate) runtime: Runtime,
    pub(crate) id: u64,
    pub(crate) realm: ContextId,
}

impl Context {
    /// Retain this realm, rejecting quarantined state.
    ///
    /// # Errors
    /// Returns `RuntimeError::Poisoned` after an engine unwind, or a checked
    /// reference-retention error such as counter overflow.
    pub fn try_clone(&self) -> Result<Self, RuntimeError> {
        self.runtime.check_poison()?;
        // Checked retention and Rc cloning do not unwind; diagnostics can.
        #[cfg(any(debug_assertions, feature = "profiling"))]
        let _unwind = self.runtime.unwind_guard();
        self.runtime.retain_context_handle(self.realm)?;
        Ok(Self {
            runtime: self.runtime.clone(),
            id: self.id,
            realm: self.realm,
        })
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        self.runtime.release_context_handle(self.realm);
    }
}

impl Context {
    #[must_use]
    pub const fn id(&self) -> u64 {
        self.id
    }

    /// Return the stable arena identity used by runtime jobs and host hooks.
    #[must_use]
    pub const fn realm_id(&self) -> ContextId {
        self.realm
    }

    #[must_use]
    pub const fn runtime(&self) -> &Runtime {
        &self.runtime
    }

    fn finish_completion(&mut self, completion: Completion) -> Result<Value, RuntimeError> {
        match completion {
            Completion::Return(value) => self.runtime.root_and_release_jsvalue(value),
            Completion::Throw(value) => {
                self.runtime.set_pending_exception_jsvalue(value)?;
                Err(RuntimeError::Exception)
            }
        }
    }

    /// Return whether this runtime currently carries a pending JavaScript
    /// exception completion.
    pub fn has_exception(&self) -> Result<bool, RuntimeError> {
        self.runtime.check_poison()?;
        let _unwind = self.runtime.unwind_guard();
        self.runtime.has_pending_exception()
    }

    /// Move the pending JavaScript exception value out of the runtime slot.
    pub fn take_exception(&mut self) -> Result<Option<Value>, RuntimeError> {
        self.runtime.check_poison()?;
        let entry_runtime = self.runtime.clone();
        let _operation = entry_runtime.operation()?;
        self.runtime.take_pending_exception()
    }

    /// Construct a realm-intrinsic native error value (the same
    /// `JS_ThrowReferenceError` / `JS_ThrowTypeError` … factory used by
    /// built-ins), capturing a backtrace exactly as a thrown native error
    /// would. This reads the realm's intrinsic constructor rather than the
    /// mutable global binding, so host callbacks that need to raise a
    /// specification-defined error are immune to global object tampering.
    pub fn new_native_error(
        &self,
        kind: NativeErrorKind,
        message: &str,
    ) -> Result<Value, RuntimeError> {
        self.runtime.check_poison()?;
        let entry_runtime = self.runtime.clone();
        let _operation = entry_runtime.operation()?;
        let error = self
            .runtime
            .new_native_error_jsvalue(self.realm, kind, message)?;
        self.runtime.root_and_release_jsvalue(error)
    }
}
